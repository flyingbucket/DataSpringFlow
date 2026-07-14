use dsf_core::backend::{SqliteConfig, StackedBackend, StackedBackendConfig};
use dsf_core::service::{DSFService, RegisterOptions};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// 辅助函数：快速生成测试所需的物理文件
fn create_dummy_req_files(base_dir: &Path, name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let ds_path = base_dir.join(format!("{}_data", name));
    let script_path = base_dir.join(format!("{}_script.py", name));
    let desc_path = base_dir.join(format!("{}_desc.md", name));

    fs::create_dir_all(&ds_path).unwrap();
    File::create(&script_path).unwrap();
    File::create(&desc_path).unwrap();

    (ds_path, script_path, desc_path)
}

/// 建立一个干净的容器内局部 StackedBackend
fn setup_test_service(base_dir: &Path) -> DSFService {
    let db_path = base_dir.join("test_dsf.db");
    let sqlite_cfg = SqliteConfig::new(db_path);

    let stacked_cfg = StackedBackendConfig::new(sqlite_cfg, vec![]);

    // 初始化并构建
    let backend = StackedBackend::new(stacked_cfg).expect("Failed to init StackedBackend");
    DSFService::new(backend)
}

#[test]
fn test_service_register_and_query_success() {
    let tmp = tempdir().unwrap();
    let base_dir = tmp.path().to_path_buf();
    let service = setup_test_service(&base_dir);

    let (ds_path, script_path, desc_path) = create_dummy_req_files(&base_dir, "imagenet");

    let opts = RegisterOptions {
        name: "imagenet".to_string(),
        tag: "v1.0".to_string(),
        path: ds_path,
        description_path: Some(desc_path),
        script_path,
        owner_nickname: None,
        dependencies: vec![],
        force_heal: false,
    };

    // 适配 target_backend: Option<&BackendAddr>
    let reg_res = service.register(opts, None);
    assert!(reg_res.is_ok(), "数据集注册失败: {:?}", reg_res.err());

    // 验证元数据查询 (返回 Vec<ScopedMetaData>)
    let scoped_metas = service
        .query_meta("imagenet@v1.0", None)
        .expect("应该能查到元数据");
    assert!(!scoped_metas.is_empty());

    let scoped_meta = &scoped_metas[0];

    assert_eq!(scoped_meta.1.name, "imagenet");
    assert_eq!(scoped_meta.1.tag, "v1.0");
}

#[test]
fn test_service_register_missing_dependency() {
    let tmp = tempdir().unwrap();
    let base_dir = tmp.path().to_path_buf();
    let service = setup_test_service(&base_dir);

    let (ds_path, script_path, desc_path) = create_dummy_req_files(&base_dir, "resnet_data");

    let opts = RegisterOptions {
        name: "resnet_data".to_string(),
        tag: "v1".to_string(),
        path: ds_path,
        description_path: Some(desc_path),
        script_path,
        owner_nickname: Some("john".to_string()),
        dependencies: vec!["ghost_dataset@v1".to_string()], // 依赖不存在
        force_heal: false,
    };

    let reg_res = service.register(opts, None);
    assert!(reg_res.is_err());
    assert!(
        reg_res
            .unwrap_err()
            .to_string()
            .contains("Dependency dataset does not exist")
    );
}

#[test]
fn test_service_delete_with_reference_protection() {
    let tmp = tempdir().unwrap();
    let base_dir = tmp.path().to_path_buf();
    let service = setup_test_service(&base_dir);

    // 1. 注册基础数据集 (base@v1)
    let (b_data, b_script, b_desc) = create_dummy_req_files(&base_dir, "base");
    service
        .register(
            RegisterOptions {
                name: "base".to_string(),
                tag: "v1".to_string(),
                path: b_data,
                description_path: Some(b_desc),
                script_path: b_script,
                owner_nickname: Some("Mick".to_string()),
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 2. 注册派生数据集 (derived@v1)，依赖于 base@v1
    let (d_data, d_script, d_desc) = create_dummy_req_files(&base_dir, "derived");
    service
        .register(
            RegisterOptions {
                name: "derived".to_string(),
                tag: "v1".to_string(),
                path: d_data,
                description_path: Some(d_desc),
                script_path: d_script,
                owner_nickname: Some("Mick".to_string()),
                dependencies: vec!["base@v1".to_string()],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 3. 尝试安全删除 base@v1，应该被防呆机制拦截
    let del_res = service.delete_metadata("base@v1", false, None);
    assert!(del_res.is_err());
    assert!(
        del_res
            .unwrap_err()
            .to_string()
            .contains("Deletion blocked, dataset is referenced")
    );

    // 4. 开启 force=true 强制删除
    let force_del_res = service.delete_metadata("base@v1", true, None);
    assert!(force_del_res.is_ok(), "强制删除应该成功");

    // 确保已被级联清除或无法查到
    assert!(
        service.query_meta("base@v1", None).is_err()
            || service.query_meta("base@v1", None).unwrap().is_empty()
    );
}

#[test]
fn test_service_check_is_referenced_returns_referrers() {
    let tmp = tempdir().unwrap();
    let base_dir = tmp.path().to_path_buf();
    let service = setup_test_service(&base_dir);

    let (b_data, b_script, b_desc) = create_dummy_req_files(&base_dir, "base");
    service
        .register(
            RegisterOptions {
                name: "base".to_string(),
                tag: "v1".to_string(),
                path: b_data,
                description_path: Some(b_desc),
                script_path: b_script,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    let (d1_data, d1_script, d1_desc) = create_dummy_req_files(&base_dir, "derived1");
    service
        .register(
            RegisterOptions {
                name: "derived1".to_string(),
                tag: "v1".to_string(),
                path: d1_data,
                description_path: Some(d1_desc),
                script_path: d1_script,
                owner_nickname: None,
                dependencies: vec!["base@v1".to_string()],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    let refs = service.check_is_referenced("base@v1").unwrap();
    // 现在的 refs 返回的是 ScopedId 数组，可以通过关联方法转换为字符串比较
    let mut ref_ids: Vec<String> = refs.into_iter().map(|r| r.1).collect();
    ref_ids.sort();

    assert_eq!(ref_ids, vec!["derived1@v1".to_string()]);
}

#[test]
fn test_service_verify_deep_and_self() {
    let tmp = tempdir().unwrap();
    let base_dir = tmp.path().to_path_buf();
    let service = setup_test_service(&base_dir);

    let (b_data, b_script, b_desc) = create_dummy_req_files(&base_dir, "it_vd_base");
    service
        .register(
            RegisterOptions {
                name: "it_vd_base".to_string(),
                tag: "v1".to_string(),
                path: b_data,
                description_path: Some(b_desc),
                script_path: b_script,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 先计算生成初始 Merkle 树条目
    service.update_merkle("it_vd_base@v1", None).unwrap();

    let res = service.verify_self("it_vd_base@v1", false, None);
    assert!(res.is_ok());
}
