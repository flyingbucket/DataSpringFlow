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
        owner: "mockuser$nobody".to_string(),
        dependencies: vec!["base_dataset@v1.0".to_string(), "labels@v2.0".to_string()],
        merkle_tree_path: PathBuf::from(format!("/mock/merkle/{}.bincode", name)),
    }
}

#[test]
fn test_backend_init_and_table_creation() {
    let dir = tempdir().unwrap();
    let db_path = dir.path().join("test_init.db");

    let cfg = SqliteConfig {
        db_path: db_path.clone(),
        ..Default::default()
    };

    let backend = SqliteBackend::new(cfg).expect("初始化 SQLite 后端失败");

    assert!(db_path.exists(), "数据库文件未生成");

    // 仅验证接口可调用且 NotFound 映射正确
    let res = backend.get_metadata("dummy@v1.0");
    assert!(res.is_err(), "不存在的数据不应返回 Ok");
    assert_eq!(res.unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn test_save_and_get_metadata() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig {
        db_path: dir.path().join("test_crud.db"),
        ..Default::default()
    };
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta = create_dummy_metadata("imagenet", "v1.0");

    backend.save_metadata(&meta).expect("保存 Metadata 失败");

    let loaded_meta = backend
        .get_metadata(&meta.id())
        .expect("读取 Metadata 失败");

    assert_eq!(loaded_meta.name, "imagenet");
    assert_eq!(loaded_meta.tag, "v1.0");
    assert_eq!(loaded_meta.hash, "deadbeef1234567890");
    assert_eq!(loaded_meta.owner, "mockuser$nobody");
    assert_eq!(loaded_meta.dependencies.len(), 2);
    assert_eq!(loaded_meta.dependencies[0], "base_dataset@v1.0");
    assert_eq!(loaded_meta.dependencies[1], "labels@v2.0");
}

#[test]
fn test_upsert_overwrite_logic() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig {
        db_path: dir.path().join("test_upsert.db"),
        ..Default::default()
    };
    let backend = SqliteBackend::new(cfg).unwrap();

    let mut meta = create_dummy_metadata("cifar10", "v1.0");

    backend.save_metadata(&meta).unwrap();

    meta.hash = "new_hash_9999".to_string();
    meta.owner = "student$wang".to_string();
    meta.dependencies.push("new_dep@v1.0".to_string());

    backend.save_metadata(&meta).expect("UPSERT 覆盖写入失败");

    let updated_meta = backend.get_metadata(&meta.id()).unwrap();

    assert_eq!(updated_meta.hash, "new_hash_9999");
    assert_eq!(updated_meta.owner, "student$wang");
    assert_eq!(
        updated_meta.dependencies.len(),
        3,
        "依赖项列表应该已被覆盖为 3 个"
    );
    assert_eq!(updated_meta.dependencies[2], "new_dep@v1.0");
}

#[test]
fn test_get_not_found_error_mapping() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig {
        db_path: dir.path().join("test_not_found.db"),
        ..Default::default()
    };
    let backend = SqliteBackend::new(cfg).unwrap();

    let res = backend.get_metadata("ghost_dataset@v9.9");

    assert!(res.is_err(), "读取不存在的数据集应该返回错误");

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
    let cfg = SqliteConfig {
        db_path: dir.path().join("test_delete_success.db"),
        ..Default::default()
    };
    let backend = SqliteBackend::new(cfg).unwrap();

    let meta = create_dummy_metadata("mnist", "v1.0");
    let id = meta.id();

    backend.save_metadata(&meta).unwrap();
    assert!(backend.get_metadata(&id).is_ok());

    backend.delete_metadata(&id).expect("物理删除元数据失败");

    let res = backend.get_metadata(&id);
    assert!(res.is_err());
    assert_eq!(res.unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn test_delete_metadata_not_found_strict_blocking() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig {
        db_path: dir.path().join("test_delete_err.db"),
        ..Default::default()
    };
    let backend = SqliteBackend::new(cfg).unwrap();

    let fake_id = "never_existed_dataset@v1.0";
    let res = backend.delete_metadata(fake_id);

    assert!(res.is_err(), "删除不存在的 ID 时，不应该静默 Ok(()) 成功");

    let err = res.unwrap_err();
    assert_eq!(
        err.kind(),
        ErrorKind::NotFound,
        "影响行数为 0 时，必须输出精准的 NotFound 报错信息"
    );
}

#[test]
fn test_list_all_metadata_or_empty() {
    let dir = tempdir().unwrap();
    let cfg = SqliteConfig {
        db_path: dir.path().join("test_list.db"),
        ..Default::default()
    };
    let backend = SqliteBackend::new(cfg).unwrap();

    let empty_list = backend.list_all_metadata().expect("读取空列表报错");
    assert!(empty_list.is_empty(), "全新数据库的列表应当为空");

    let mut meta1 = create_dummy_metadata("coco", "v2017");
    let mut meta2 = create_dummy_metadata("voc", "v2012");
    meta1.owner = "student$li".to_string();
    meta2.owner = "student$wang".to_string();

    backend.save_metadata(&meta1).unwrap();
    backend.save_metadata(&meta2).unwrap();

    let all_datasets = backend.list_all_metadata().expect("加载全量列表失败");
    assert_eq!(all_datasets.len(), 2, "数据库中应有且仅有 2 条数据集记录");

    let has_coco = all_datasets
        .iter()
        .any(|m| m.name == "coco" && m.tag == "v2017" && m.owner == "student$li");
    let has_voc = all_datasets
        .iter()
        .any(|m| m.name == "voc" && m.tag == "v2012" && m.owner == "student$wang");
    assert!(has_coco && has_voc, "全量列表中的数据集信息与预期不符");
}
