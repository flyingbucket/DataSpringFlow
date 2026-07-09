use crate::common::MemoryBackend;
use ::dataspringflow_rs::backend::build_backend_auto;
use dataspringflow_rs::service::{DSFService, RegisterOptions};
use std::fs::File;

mod common;
use common::TestSandbox;
/// 辅助函数：在沙盒中快速生成必须的脚本和描述文件，避免路径不存在报错
fn create_dummy_req_files(
    sandbox: &TestSandbox,
    name: &str,
) -> (std::path::PathBuf, std::path::PathBuf) {
    let script_path = sandbox.base_dir.join(format!("{}_script.py", name));
    let desc_path = sandbox.base_dir.join(format!("{}_desc.md", name));
    File::create(&script_path).unwrap();
    File::create(&desc_path).unwrap();
    (script_path, desc_path)
}

#[test]
fn test_service_register_and_query_success() {
    let sandbox = TestSandbox::new("test_service_register");

    // 修复：使用 Box 包装而不是 Arc，以匹配 DynBackend 的定义
    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    // 1. 生成物理文件沙盒
    let ds_path = sandbox.create_dummy_dataset("imagenet", "fake image data");
    let (script_path, desc_path) = create_dummy_req_files(&sandbox, "imagenet");

    // 2. 构造注册选项
    let opts = RegisterOptions {
        name: "imagenet".to_string(),
        tag: "v1.0".to_string(),
        path: ds_path,
        description_path: Some(desc_path),
        script_path,
        dependencies: vec![], // 无依赖
        force_heal: false,
        yes: false,
    };

    // 3. 执行注册
    let reg_res = service.register(opts);
    assert!(reg_res.is_ok(), "数据集注册失败: {:?}", reg_res.err());

    // 4. 验证元数据是否可被查询
    let meta = service
        .query_meta("imagenet@v1.0")
        .expect("应该能查到元数据");
    assert_eq!(meta.name, "imagenet");
    assert_eq!(meta.tag, "v1.0");
}

#[test]
fn test_service_register_missing_dependency() {
    let sandbox = TestSandbox::new("test_service_missing_dep");

    // 修复：使用 Box
    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    let ds_path = sandbox.create_dummy_dataset("resnet_data", "fake data");
    let (script_path, desc_path) = create_dummy_req_files(&sandbox, "resnet_data");

    // 尝试依赖一个不存在的数据集
    let opts = RegisterOptions {
        name: "resnet_data".to_string(),
        tag: "v1".to_string(),
        path: ds_path,
        description_path: Some(desc_path),
        script_path,
        dependencies: vec!["ghost_dataset@v1".to_string()], // 这个不存在！
        force_heal: false,
        yes: false,
    };

    let reg_res = service.register(opts);
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
    let sandbox = TestSandbox::new("test_service_delete_protection");

    // 修复：使用 Box
    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    // 1. 注册基础数据集 (base@v1)
    let base_path = sandbox.create_dummy_dataset("base", "base data");
    let (b_script, b_desc) = create_dummy_req_files(&sandbox, "base");
    service
        .register(RegisterOptions {
            name: "base".to_string(),
            tag: "v1".to_string(),
            path: base_path,
            description_path: Some(b_desc),
            script_path: b_script,
            dependencies: vec![],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    // 2. 注册派生数据集 (derived@v1)，依赖于 base@v1
    let derived_path = sandbox.create_dummy_dataset("derived", "derived data");
    let (d_script, d_desc) = create_dummy_req_files(&sandbox, "derived");
    service
        .register(RegisterOptions {
            name: "derived".to_string(),
            tag: "v1".to_string(),
            path: derived_path,
            description_path: Some(d_desc),
            script_path: d_script,
            dependencies: vec!["base@v1".to_string()],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    // 3. 尝试不带 force 删除 base@v1，应该被防呆机制拦截
    let del_res = service.delete_metadata("base@v1", false);
    assert!(del_res.is_err());
    assert!(
        del_res
            .unwrap_err()
            .to_string()
            .contains("Deletion blocked, dataset is referenced")
    );

    // 4. 开启 force=true，应该成功删除
    let force_del_res = service.delete_metadata("base@v1", true);
    assert!(force_del_res.is_ok(), "强制删除应该成功");

    // 确保真的被删除了
    assert!(service.query_meta("base@v1").is_err());
}

#[test]
fn test_service_check_is_referenced_returns_referrers() {
    let sandbox = TestSandbox::new("test_service_check_is_referenced");

    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    // base@v1
    let base_path = sandbox.create_dummy_dataset("base", "base data");
    let (base_script, base_desc) = create_dummy_req_files(&sandbox, "base");
    service
        .register(RegisterOptions {
            name: "base".to_string(),
            tag: "v1".to_string(),
            path: base_path,
            description_path: Some(base_desc),
            script_path: base_script,
            dependencies: vec![],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    // derived1@v1 depends on base@v1
    let d1_path = sandbox.create_dummy_dataset("derived1", "d1");
    let (d1_script, d1_desc) = create_dummy_req_files(&sandbox, "derived1");
    service
        .register(RegisterOptions {
            name: "derived1".to_string(),
            tag: "v1".to_string(),
            path: d1_path,
            description_path: Some(d1_desc),
            script_path: d1_script,
            dependencies: vec!["base@v1".to_string()],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    // derived2@v1 also depends on base@v1
    let d2_path = sandbox.create_dummy_dataset("derived2", "d2");
    let (d2_script, d2_desc) = create_dummy_req_files(&sandbox, "derived2");
    service
        .register(RegisterOptions {
            name: "derived2".to_string(),
            tag: "v1".to_string(),
            path: d2_path,
            description_path: Some(d2_desc),
            script_path: d2_script,
            dependencies: vec!["base@v1".to_string()],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    let mut refs = service.check_is_referenced("base@v1").unwrap();
    refs.sort();

    assert_eq!(
        refs,
        vec!["derived1@v1".to_string(), "derived2@v1".to_string()]
    );
}

#[test]
fn test_service_list_all_metadata_returns_all_registered() {
    let sandbox = TestSandbox::new("test_service_list_all_metadata");

    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    let p1 = sandbox.create_dummy_dataset("a", "a");
    let (s1, d1) = create_dummy_req_files(&sandbox, "a");
    service
        .register(RegisterOptions {
            name: "a".to_string(),
            tag: "v1".to_string(),
            path: p1,
            description_path: Some(d1),
            script_path: s1,
            dependencies: vec![],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    let p2 = sandbox.create_dummy_dataset("b", "b");
    let (s2, d2) = create_dummy_req_files(&sandbox, "b");
    service
        .register(RegisterOptions {
            name: "b".to_string(),
            tag: "v2".to_string(),
            path: p2,
            description_path: Some(d2),
            script_path: s2,
            dependencies: vec![],
            force_heal: false,
            yes: false,
        })
        .unwrap();

    let mut metas = service.list_all_metadata().unwrap();
    metas.sort_by_key(|m| m.id());

    let ids: Vec<String> = metas.into_iter().map(|m| m.id()).collect();
    assert_eq!(ids, vec!["a@v1".to_string(), "b@v2".to_string()]);
}

#[test]
fn test_service_update_merkle_invalid_id_should_fail_fast() {
    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    // 非法 dataset id，应该在 validate_dataset_id 阶段直接失败
    let res = service.update_merkle("not-a-valid-id");
    assert!(res.is_err());
}

#[test]
fn test_service_verify_self_invalid_id_should_fail() {
    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    let res = service.verify_self("bad-id", false);
    assert!(res.is_err());
}

#[test]
fn test_service_verify_deep_nonexistent_id_should_fail() {
    let backend = Box::new(MemoryBackend::new());
    let service = DSFService::new(backend);

    // verify_deep 不先做 validate_dataset_id，这里给合法但不存在的 id
    let res = service.verify_deep("ghost@v1", false);
    assert!(res.is_err());
}

fn make_real_backend() -> dataspringflow_rs::backend::DynBackend {
    build_backend_auto().expect("failed to build real backend from local DSF config")
}

fn register_dataset(
    service: &DSFService,
    sandbox: &TestSandbox,
    name: &str,
    tag: &str,
    deps: Vec<String>,
) -> String {
    let ds_path = sandbox.create_dummy_dataset(name, &format!("{name} content"));
    let (script_path, desc_path) = create_dummy_req_files(sandbox, name);

    service
        .register(RegisterOptions {
            name: name.to_string(),
            tag: tag.to_string(),
            path: ds_path,
            description_path: Some(desc_path),
            script_path,
            dependencies: deps,
            force_heal: false,
            yes: false,
        })
        .unwrap();

    format!("{name}@{tag}")
}

#[test]
fn test_service_update_merkle_success_real_backend() {
    let sandbox = TestSandbox::new("it_update_merkle_success");
    let backend = make_real_backend();
    let service = DSFService::new(backend);

    let id = register_dataset(&service, &sandbox, "it_um_base", "v1", vec![]);
    let res = service.update_merkle(&id);
    assert!(
        res.is_ok(),
        "update_merkle should succeed, got: {:?}",
        res.err()
    );
}

#[test]
fn test_service_verify_self_success_real_backend() {
    let sandbox = TestSandbox::new("it_verify_self_success");
    let backend = make_real_backend();
    let service = DSFService::new(backend);

    let id = register_dataset(&service, &sandbox, "it_vs_base", "v1", vec![]);
    let res = service.verify_self(&id, false);
    assert!(
        res.is_ok(),
        "verify_self should succeed, got: {:?}",
        res.err()
    );

    let v = res.unwrap();
    // 只断言“有结果且非崩溃”；状态枚举细节按你项目可再加精确断言
    assert!(
        matches!(
            v.status,
            dataspringflow_rs::core::DataSetStatus::Healthy
                | dataspringflow_rs::core::DataSetStatus::Broken
                | dataspringflow_rs::core::DataSetStatus::Unverified
        ),
        "unexpected status: {:?}",
        v.status
    );
}

#[test]
fn test_service_verify_deep_with_dependency_success_real_backend() {
    let sandbox = TestSandbox::new("it_verify_deep_success");
    let backend = make_real_backend();
    let service = DSFService::new(backend);

    let base_id = register_dataset(&service, &sandbox, "it_vd_base", "v1", vec![]);
    let derived_id = register_dataset(
        &service,
        &sandbox,
        "it_vd_derived",
        "v1",
        vec![base_id.clone()],
    );

    let res = service.verify_deep(&derived_id, false);
    assert!(
        res.is_ok(),
        "verify_deep should succeed, got: {:?}",
        res.err()
    );

    let v = res.unwrap();
    assert!(
        matches!(
            v.status,
            dataspringflow_rs::core::DataSetStatus::Healthy
                | dataspringflow_rs::core::DataSetStatus::Broken
                | dataspringflow_rs::core::DataSetStatus::Unverified
        ),
        "unexpected status: {:?}",
        v.status
    );
}

#[test]
fn test_service_check_is_referenced_success_real_backend() {
    let sandbox = TestSandbox::new("it_check_is_referenced");
    let backend = make_real_backend();
    let service = DSFService::new(backend);

    let base_id = register_dataset(&service, &sandbox, "it_ref_base", "v1", vec![]);
    let d1 = register_dataset(&service, &sandbox, "it_ref_d1", "v1", vec![base_id.clone()]);
    let d2 = register_dataset(&service, &sandbox, "it_ref_d2", "v1", vec![base_id.clone()]);

    let mut refs = service.check_is_referenced(&base_id).unwrap();
    refs.sort();

    let mut expected = vec![d1, d2];
    expected.sort();
    assert_eq!(refs, expected);
}

#[test]
fn test_service_list_all_metadata_contains_registered_real_backend() {
    let sandbox = TestSandbox::new("it_list_all_metadata");
    let backend = make_real_backend();
    let service = DSFService::new(backend);

    let a = register_dataset(&service, &sandbox, "it_list_a", "v1", vec![]);
    let b = register_dataset(&service, &sandbox, "it_list_b", "v1", vec![]);

    let metas = service.list_all_metadata().unwrap();
    let mut ids: Vec<String> = metas.into_iter().map(|m| m.id()).collect();
    ids.sort();

    assert!(ids.contains(&a), "list_all_metadata missing {}", a);
    assert!(ids.contains(&b), "list_all_metadata missing {}", b);
}
