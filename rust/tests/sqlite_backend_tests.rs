use dataspringflow_rs::backend::DatasetBackend;
use dataspringflow_rs::backend::{SqliteBackend, SqliteConfig};
use dataspringflow_rs::core::MetaData;
use std::io::ErrorKind;
use std::path::PathBuf;
use tempfile::tempdir;

/// 辅助函数：快速生成用于测试的假数据集元数据
fn create_dummy_metadata(name: &str, tag: &str) -> MetaData {
    MetaData {
        name: name.to_string(),
        tag: tag.to_string(),
        hash: "deadbeef1234567890".to_string(),
        path: PathBuf::from(format!("/mock/path/to/{}", name)),
        description_path: PathBuf::from(format!("/mock/desc/{}.md", name)),
        script_path: PathBuf::from(format!("/mock/scripts/{}.py", name)),
        dependencies: vec!["base_dataset@v1.0".to_string(), "labels@v2.0".to_string()],
        merkle_tree_path: PathBuf::from(format!("/mock/merkle/{}.bincode", name)),
    }
}

#[test]
fn test_backend_init_and_table_creation() {
    // 创建一个临时目录，测试结束后自动销毁
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_init.db");

    let mut cfg = SqliteConfig::default();
    cfg.db_path = db_path.clone();

    // 测试：初始化是否成功（这里会隐式调用 init() 创建表结构）
    let backend = SqliteBackend::new(cfg).expect("初始化 SQLite 后端失败");

    // 验证：数据库文件确实被物理创建在了临时目录中
    assert!(db_path.exists(), "数据库文件未生成");

    // 能够正常拿到连接说明 pragmas 等设置都没有报错
    let _conn = backend.get_metadata("dummy@v1.0");
}

#[test]
fn test_save_and_get_metadata() {
    let dir = tempdir().unwrap();
    let mut cfg = SqliteConfig::default();
    cfg.db_path = dir.path().join("test_crud.db");
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta = create_dummy_metadata("imagenet", "v1.0");

    // 1. 测试保存
    backend.save_metadata(&meta).expect("保存 Metadata 失败");

    // 2. 测试读取
    let loaded_meta = backend
        .get_metadata(&meta.id())
        .expect("读取 Metadata 失败");

    // 3. 字段比对验证（确保存入和取出的数据序列化/反序列化无损）
    assert_eq!(loaded_meta.name, "imagenet");
    assert_eq!(loaded_meta.tag, "v1.0");
    assert_eq!(loaded_meta.hash, "deadbeef1234567890");

    // 验证依赖列表被正确从 JSON 还原成了 Vec<String>
    assert_eq!(loaded_meta.dependencies.len(), 2);
    assert_eq!(loaded_meta.dependencies[0], "base_dataset@v1.0");
}

#[test]
fn test_upsert_overwrite_logic() {
    let dir = tempdir().unwrap();
    let mut cfg = SqliteConfig::default();
    cfg.db_path = dir.path().join("test_upsert.db");
    let backend = SqliteBackend::new(cfg).unwrap();

    let mut meta = create_dummy_metadata("cifar10", "v1.0");

    // 初次保存
    backend.save_metadata(&meta).unwrap();

    // 模拟使用者重置了同一版本的数据集，更新了特征值
    meta.hash = "new_hash_9999".to_string();
    meta.dependencies.push("new_dep@v1.0".to_string());

    // 再次保存（因为 ID 相同，应该触发 SQLite 的 ON CONFLICT DO UPDATE）
    backend.save_metadata(&meta).expect("UPSERT 覆盖写入失败");

    // 重新读取并验证
    let updated_meta = backend.get_metadata(&meta.id()).unwrap();

    // 断言内容确实被更新了，且没有产生报错
    assert_eq!(updated_meta.hash, "new_hash_9999");
    assert_eq!(
        updated_meta.dependencies.len(),
        3,
        "依赖项列表应该已被覆盖为 3 个"
    );
}

#[test]
fn test_get_not_found_error_mapping() {
    let dir = tempdir().unwrap();
    let mut cfg = SqliteConfig::default();
    cfg.db_path = dir.path().join("test_not_found.db");
    let backend = SqliteBackend::new(cfg).unwrap();

    // 尝试读取一个根本不存在的数据集
    let res = backend.get_metadata("ghost_dataset@v9.9");

    // 必须返回 Err
    assert!(res.is_err(), "读取不存在的数据集应该返回错误");

    // 验证我们底层是否将 rusqlite 的空行错误优雅转化为了系统标准 NotFound 错误
    let err = res.unwrap_err();
    assert_eq!(
        err.kind(),
        ErrorKind::NotFound,
        "错误类型应该被映射为 NotFound"
    );
}

#[test]
fn test_delete_metadata_success() {
    let dir = tempdir().unwrap();
    let mut cfg = SqliteConfig::default();
    cfg.db_path = dir.path().join("test_delete_success.db");
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta = create_dummy_metadata("mnist", "v1.0");
    let id = meta.id();

    // 1. 先存进去，并验证它确实存在
    backend.save_metadata(&meta).unwrap();
    assert!(backend.get_metadata(&id).is_ok());

    // 2. 执行删除，应该顺畅通过并 COMMIT 事务
    backend.delete_metadata(&id).expect("物理删除元数据失败");

    // 3. 再次读取，数据库里应该没有它了，且返回 NotFound 错误
    let res = backend.get_metadata(&id);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn test_delete_metadata_not_found_strict_blocking() {
    let dir = tempdir().unwrap();
    let mut cfg = SqliteConfig::default();
    cfg.db_path = dir.path().join("test_delete_err.db");
    let backend = SqliteBackend::new(cfg).unwrap();

    // 尝试删除一个根本没有被注册过的幽灵数据集
    let fake_id = "never_existed_dataset@v1.0";
    let res = backend.delete_metadata(fake_id);

    // 核心校验：根据工业级规范，rows_affected 为 0 时必须报错，绝不静默当作 Ok(())
    assert!(res.is_err(), "删除不存在的 ID 时，不应该静默 Ok(()) 成功");

    // 验证错误码是否被我们写的业务分支完美捕获并转化为系统级 NotFound
    let err = res.unwrap_err();
    assert_eq!(
        err.kind(),
        ErrorKind::NotFound,
        "影响行数为 0 时，必须对外输出精准的 NotFound 报错信息"
    );
}

#[test]
fn test_list_all_metadata_or_empty() {
    let dir = tempdir().unwrap();
    let mut cfg = SqliteConfig::default();
    cfg.db_path = dir.path().join("test_list.db");
    let backend = SqliteBackend::new(cfg).unwrap();

    // 1. 数据库刚初始化时，全量列表应该是空的
    let empty_list = backend.list_all_metadata().expect("读取空列表报错");
    assert!(empty_list.is_empty(), "全新数据库的列表应当为空");

    // 2. 批量塞入两条数据集
    let meta1 = create_dummy_metadata("coco", "v2017");
    let meta2 = create_dummy_metadata("voc", "v2012");
    backend.save_metadata(&meta1).unwrap();
    backend.save_metadata(&meta2).unwrap();

    // 3. 重新获取列表，验证长度和数据完备性
    let all_datasets = backend.list_all_metadata().expect("加载全量列表失败");
    assert_eq!(all_datasets.len(), 2, "数据库中应有且仅有 2 条数据集记录");

    // 验证内容是否正确包含
    let has_coco = all_datasets
        .iter()
        .any(|m| m.name == "coco" && m.tag == "v2017");
    let has_voc = all_datasets
        .iter()
        .any(|m| m.name == "voc" && m.tag == "v2012");
    assert!(has_coco && has_voc, "全量列表中的数据集信息与预期不符");
}
