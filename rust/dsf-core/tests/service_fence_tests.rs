use dsf_core::backend::{SqliteConfig, StackedBackend, StackedBackendConfig};
use dsf_core::core::{DataSetBusyStatus, DataSetStatus};
use dsf_core::service::{DSFService, RegisterOptions};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn create_dummy_req_files(base_dir: &Path, name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let ds_path = base_dir.join(format!("{}_data", name));
    let script_path = base_dir.join(format!("{}_script.py", name));
    let desc_path = base_dir.join(format!("{}_desc.md", name));

    fs::create_dir_all(&ds_path).unwrap();
    let mut file = File::create(ds_path.join("default.txt")).unwrap();
    writeln!(file, "initial data for {}", name).unwrap();

    File::create(&script_path).unwrap();
    File::create(&desc_path).unwrap();

    (ds_path, script_path, desc_path)
}

fn setup_test_service(base_dir: &Path) -> DSFService {
    let db_path = base_dir.join("test_dsf_fence.db");
    let sqlite_cfg = SqliteConfig::new(db_path);
    let stacked_cfg = StackedBackendConfig::new(sqlite_cfg, vec![]);
    let backend = StackedBackend::new(stacked_cfg).expect("Failed to init StackedBackend");
    DSFService::new(backend)
}

#[test]
fn test_service_verify_self_bridges_free_and_reading_to_healthy() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    let service = setup_test_service(base);

    let (ds_path, script_path, desc_path) = create_dummy_req_files(base, "ds_read");
    service
        .register(
            RegisterOptions {
                name: "ds_read".to_string(),
                tag: "v1".to_string(),
                path: ds_path,
                description_path: Some(desc_path),
                script_path,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 1. Free 状态桥接测试
    let res_free = service.verify_self("ds_read@v1", false, None).unwrap();
    assert_eq!(res_free.status, DataSetStatus::Healthy);

    // 2. 将状态更改为 Reading，仍应该安全桥接到 Healthy
    service
        .mark_status("ds_read@v1", DataSetBusyStatus::Reading, None)
        .unwrap();
    let res_reading = service.verify_self("ds_read@v1", false, None).unwrap();
    assert_eq!(res_reading.status, DataSetStatus::Healthy);
}

#[test]
fn test_service_verify_self_fences_modifying_status() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    let service = setup_test_service(base);

    let (ds_path, script_path, desc_path) = create_dummy_req_files(base, "ds_mod");
    service
        .register(
            RegisterOptions {
                name: "ds_mod".to_string(),
                tag: "v1".to_string(),
                path: ds_path,
                description_path: Some(desc_path),
                script_path,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 将状态标记为正在修改中 (Modifying)
    service
        .mark_status("ds_mod@v1", DataSetBusyStatus::Modifying, None)
        .unwrap();

    // 校验必须立刻触发栅栏被拦下
    let res = service.verify_self("ds_mod@v1", false, None).unwrap();
    assert_eq!(
        res.status,
        DataSetStatus::Busy(DataSetBusyStatus::Modifying)
    );
}

#[test]
fn test_service_update_merkle_blocked_by_fence_when_busy() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    let service = setup_test_service(base);

    let (ds_path, script_path, desc_path) = create_dummy_req_files(base, "ds_update");
    service
        .register(
            RegisterOptions {
                name: "ds_update".to_string(),
                tag: "v1".to_string(),
                path: ds_path.clone(),
                description_path: Some(desc_path),
                script_path,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 模拟脏写入并挂上 Creating 状态
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(ds_path.join("default.txt"))
        .unwrap();
    writeln!(file, "partial writing content...").unwrap();
    service
        .mark_status("ds_update@v1", DataSetBusyStatus::Creating, None)
        .unwrap();

    // 尝试调用 update_merkle 强行更新 Hash，必须被 fence 拒绝
    let update_res = service.update_merkle("ds_update@v1", None);
    assert!(
        update_res.is_err(),
        "防呆拦截失败：对处于 Creating/Modifying 状态的数据集调用 update_merkle 应当报错！"
    );
}

#[test]
fn test_service_register_blocked_when_dependency_is_busy_even_with_force_heal() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    let service = setup_test_service(base);

    // 1. 注册上游依赖 base@v1
    let (b_path, b_script, b_desc) = create_dummy_req_files(base, "base");
    service
        .register(
            RegisterOptions {
                name: "base".to_string(),
                tag: "v1".to_string(),
                path: b_path,
                description_path: Some(b_desc),
                script_path: b_script,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 2. 将 base@v1 标记为 Modifying（假设另一个进程正在修补它）
    service
        .mark_status("base@v1", DataSetBusyStatus::Modifying, None)
        .unwrap();

    // 3. 注册新数据集，依赖 base@v1，并且激进地开启 force_heal: true
    let (d_path, d_script, d_desc) = create_dummy_req_files(base, "derived");
    let reg_res = service.register(
        RegisterOptions {
            name: "derived".to_string(),
            tag: "v1".to_string(),
            path: d_path,
            description_path: Some(d_desc),
            script_path: d_script,
            owner_nickname: None,
            dependencies: vec!["base@v1".to_string()],
            force_heal: true, // 开启强行自愈！
        },
        None,
    );

    // 必须报错失败，不可盲目自愈正在写入的依赖数据集！
    assert!(
        reg_res.is_err(),
        "安全漏洞：不能对标记为 Modifying/Creating 的非健康依赖项进行 force_heal 覆盖！"
    );
}

#[test]
fn test_service_verify_deep_propagates_busy_dependency() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    let service = setup_test_service(base);

    let (b_path, b_script, b_desc) = create_dummy_req_files(base, "root_ds");
    service
        .register(
            RegisterOptions {
                name: "root_ds".to_string(),
                tag: "v1".to_string(),
                path: b_path,
                description_path: Some(b_desc),
                script_path: b_script,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    let (d_path, d_script, d_desc) = create_dummy_req_files(base, "child_ds");
    service
        .register(
            RegisterOptions {
                name: "child_ds".to_string(),
                tag: "v1".to_string(),
                path: d_path,
                description_path: Some(d_desc),
                script_path: d_script,
                owner_nickname: None,
                dependencies: vec!["root_ds@v1".to_string()],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 将底层上游依赖设为 Deleting
    service
        .mark_status("root_ds@v1", DataSetBusyStatus::Deleting, None)
        .unwrap();

    // 顶层进行深层校验 (verify_deep)，预期自身因依赖项非 Healthy 而呈现 BrokenDeps 状态
    let verify_res = service.verify_deep("child_ds@v1", false, None).unwrap();
    assert_eq!(verify_res.status, DataSetStatus::BrokenDeps);

    // 其依赖状态列表中的第一个元素，必然是那个处于 Deleting 的 Busy 状态
    assert_eq!(
        verify_res.dep_status[0],
        DataSetStatus::Busy(DataSetBusyStatus::Deleting)
    );
}

#[test]
fn test_service_mark_status_persistence_and_recovery() {
    let tmp = tempdir().unwrap();
    let base = tmp.path();
    let service = setup_test_service(base);

    let (ds_path, script_path, desc_path) = create_dummy_req_files(base, "persistent_ds");
    service
        .register(
            RegisterOptions {
                name: "persistent_ds".to_string(),
                tag: "v1".to_string(),
                path: ds_path,
                description_path: Some(desc_path),
                script_path,
                owner_nickname: None,
                dependencies: vec![],
                force_heal: false,
            },
            None,
        )
        .unwrap();

    // 标记为 Creating
    service
        .mark_status("persistent_ds@v1", DataSetBusyStatus::Creating, None)
        .unwrap();
    let metas = service.query_meta("persistent_ds@v1", None).unwrap();
    assert_eq!(metas[0].1.busy_status, DataSetBusyStatus::Creating);

    // 模拟写入完成后，恢复为 Free 状态
    service
        .mark_status("persistent_ds@v1", DataSetBusyStatus::Free, None)
        .unwrap();
    let metas_recovered = service.query_meta("persistent_ds@v1", None).unwrap();
    assert_eq!(metas_recovered[0].1.busy_status, DataSetBusyStatus::Free);

    // 恢复为 Free 状态后再次校验应当顺畅通过为 Healthy
    let res = service
        .verify_self("persistent_ds@v1", false, None)
        .unwrap();
    assert_eq!(res.status, DataSetStatus::Healthy);
}
