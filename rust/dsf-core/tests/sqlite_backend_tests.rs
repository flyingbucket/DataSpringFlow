use dsf_core::backend::DatasetBackend;
use dsf_core::backend::{SqliteBackend, SqliteConfig};
use dsf_core::core::MetaData;
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
        owner: "mockuser$nobody".to_string(),
        dependencies: vec!["base_dataset@v1.0".to_string(), "labels@v2.0".to_string()],
        merkle_tree_path: PathBuf::from(format!("/mock/merkle/{}.bincode", name)),
    }
}

#[test]
fn test_backend_init_and_table_creation() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_init.db");

    // 使用我们重构后的标准配置构造方式
    let cfg = SqliteConfig::new(db_path.clone());
    let backend = SqliteBackend::new(cfg).expect("初始化 SqliteBackend 失败");

    // 验证数据库文件确实被真实创建
    assert!(db_path.exists(), "数据库文件应当在初始化后被真实创建");

    // 尝试调用一次 list_all_metadata，不报错说明表结构 (datasets) 已自动迁移成功
    let res = backend.list_all_metadata();
    assert!(res.is_ok(), "初始查询表结构应当成功");
}

#[test]
fn test_save_and_get_metadata_success() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig::new(dir.path().join("test_save.db"));
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta = create_dummy_metadata("mnist", "v1.0");

    // 1. 测试写入元数据
    backend.save_metadata(&meta).expect("保存元数据失败");

    // 2. 测试根据完整的 id (name@tag) 查询
    let id = "mnist@v1.0";
    let retrieved = backend
        .get_metadata(id)
        .expect("查询元数据失败，这里直接返回了 MetaData 实体");

    // 3. 字段严谨校验
    assert_eq!(retrieved.name, "mnist");
    assert_eq!(retrieved.tag, "v1.0");
    assert_eq!(retrieved.hash, "deadbeef1234567890");
    assert_eq!(
        retrieved.dependencies,
        vec!["base_dataset@v1.0".to_string(), "labels@v2.0".to_string()]
    );
}

#[test]
fn test_save_duplicate_metadata_should_overwrite() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig::new(dir.path().join("test_dup.db"));
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta1 = create_dummy_metadata("cifar10", "v1");
    let mut meta2 = create_dummy_metadata("cifar10", "v1");
    meta2.hash = "new_changed_hash_value_666".to_string(); // 修改某个字段用于验证覆盖

    backend.save_metadata(&meta1).unwrap();

    // 再次写入相同 id 的数据，应当成功（触发替换覆盖）
    let res = backend.save_metadata(&meta2);
    assert!(
        res.is_ok(),
        "由于使用 INSERT OR REPLACE，重复注册应当成功覆盖而不报错"
    );

    // 验证确实被覆盖为了 meta2 的内容
    let retrieved = backend.get_metadata("cifar10@v1").unwrap();
    assert_eq!(retrieved.hash, "new_changed_hash_value_666");
}

#[test]
fn test_get_metadata_not_found_returns_none() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig::new(dir.path().join("test_not_found.db"));
    let backend = SqliteBackend::new(cfg).unwrap();

    // 探测不存在的 ID
    let res = backend.get_metadata("ghost_dataset@v9.9");
    assert!(res.is_err(), "对于不存在的 ID，应该返回 Err");
    let backend_err = res.unwrap_err();
    let io_err = backend_err.to_io_error();
    assert_eq!(
        io_err.kind(),
        std::io::ErrorKind::NotFound,
        "必须精确返回 NotFound 错误"
    );
}

#[test]
fn test_delete_metadata_success_and_fail_subsequent() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig::new(dir.path().join("test_del.db"));
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta = create_dummy_metadata("imagenet", "2012");
    let id = "imagenet@2012";

    backend.save_metadata(&meta).unwrap();

    // 1. 首次删除应当成功
    backend.delete_metadata(id).expect("删除存在的元数据失败");

    // 确认已经被清除
    let check = backend.get_metadata(id);
    assert!(check.is_err(), "对于不存在的 ID，应该返回 Err");
    let backend_err = check.unwrap_err();
    let io_err = backend_err.to_io_error();
    assert_eq!(
        io_err.kind(),
        std::io::ErrorKind::NotFound,
        "必须精确返回 NotFound 错误"
    );

    // 2. 再次删除已经被清除的同一个 ID，根据 sqlite_backend.rs 中的行数匹配：
    // rows_affected == 0 时抛出精准的 NotFound
    let res = backend.delete_metadata(id);
    assert!(res.is_err(), "重复删除同一个 ID 应当报错");

    let backend_err = res.unwrap_err();
    let io_err = backend_err.to_io_error();
    assert_eq!(
        io_err.kind(),
        std::io::ErrorKind::NotFound,
        "必须精确返回 NotFound 错误"
    );
}

#[test]
fn test_list_all_metadata_or_empty() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig::new(dir.path().join("test_list.db"));
    let backend = SqliteBackend::new(cfg).unwrap();

    // 全新数据库初始列表应当为空
    let empty_list = backend.list_all_metadata().expect("读取空列表报错");
    assert!(empty_list.is_empty(), "全新数据库的列表应当为空");

    // 注入两条不同的测试数据
    let mut meta1 = create_dummy_metadata("coco", "v2017");
    let mut meta2 = create_dummy_metadata("voc", "v2012");
    meta1.owner = "student$li".to_string();
    meta2.owner = "student$wang".to_string();

    backend.save_metadata(&meta1).unwrap();
    backend.save_metadata(&meta2).unwrap();

    // 验证批量拉取
    let all_datasets = backend.list_all_metadata().expect("加载全量元数据失败");
    assert_eq!(all_datasets.len(), 2, "列表里应当恰好有两条记录");

    let names: Vec<String> = all_datasets.iter().map(|m| m.name.clone()).collect();
    assert!(names.contains(&"coco".to_string()));
    assert!(names.contains(&"voc".to_string()));
}
