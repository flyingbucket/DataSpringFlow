#[cfg(test)]
mod tests {
    use dsf_core::backend::{StackedBackend, build_backend_auto};
    use dsf_core::core::DataSetStatus;
    use dsf_core::dag::*;
    use dsf_core::service::{DSFService, RegisterOptions};
    use std::path::PathBuf;

    /// RAII 容器测试沙盒：以 DSFAdmin 身份，通过真实的 DSFService 接口注册 Mock 数据集，
    /// 并在作用域结束或发生断言 panic 时，自动调用服务接口逆序回收所有写入的数据。
    pub(crate) struct TestSandbox {
        service: DSFService,
        backend: StackedBackend,
        registered_ids: Vec<String>,
        workspace_dir: PathBuf,
    }

    impl TestSandbox {
        pub(crate) fn new(test_name: &str) -> Self {
            let backend_for_service = build_backend_auto()
                .expect("Failed to initialize auto backend for service in container");
            let backend_for_dag = build_backend_auto()
                .expect("Failed to initialize auto backend for dag in container");
            let service = DSFService::new(backend_for_service);

            // 在容器可写分区创建当前测试专属的物理工作目录
            let workspace_dir = std::env::temp_dir()
                .join("dsf_admin_container_tests")
                .join(test_name);
            let _ = std::fs::remove_dir_all(&workspace_dir);
            std::fs::create_dir_all(&workspace_dir)
                .expect("Failed to create workspace directory in container");

            Self {
                service,
                backend: backend_for_dag,
                registered_ids: Vec::new(),
                workspace_dir,
            }
        }

        /// 使用 DSFService 官方接口注册一个真实的 Mock 数据集
        pub(crate) fn register_mock(&mut self, name: &str, tag: &str, deps: &[&str]) {
            let ds_dir = self.workspace_dir.join(format!("{}_{}", name, tag));
            std::fs::create_dir_all(&ds_dir).expect("Failed to create dataset dir");

            // 写入基础物理文件，确保通过 ensure_exists 与 MerkleTree 哈希计算
            let data_file = ds_dir.join("data.bin");
            std::fs::write(&data_file, format!("mock real data for {}@{}", name, tag))
                .expect("Failed to write mock data file");

            let script_path = ds_dir.join("run.py");
            std::fs::write(&script_path, "#!/usr/bin/env python3\nprint('mock script')")
                .expect("Failed to write script file");

            let desc_path = ds_dir.join("desc.md");
            std::fs::write(
                &desc_path,
                format!("# Dataset {}@{}\nMock description", name, tag),
            )
            .expect("Failed to write description file");

            let opts = RegisterOptions {
                name: name.to_string(),
                tag: tag.to_string(),
                path: ds_dir,
                description_path: Some(desc_path),
                script_path,
                owner_nickname: Some("DSFAdmin".to_string()),
                dependencies: deps.iter().map(|s| s.to_string()).collect(),
                force_heal: true, // 开启强制自愈，确保批量注册时不会因为依赖状态阻断
            };

            self.service.register(opts, None).unwrap_or_else(|e| {
                panic!("DSFService failed to register {}@{}: {:?}", name, tag, e)
            });

            self.registered_ids.push(format!("{}@{}", name, tag));
        }

        /// 主动删除特定 Mock 数据集并从追踪列表中移除
        pub(crate) fn delete_mock(&mut self, id: &str) {
            let _ = self.service.delete_metadata(id, true, None);
            self.registered_ids.retain(|x| x != id);
        }

        pub(crate) fn service(&self) -> &DSFService {
            &self.service
        }

        pub(crate) fn backend(&self) -> &StackedBackend {
            &self.backend
        }
    }

    impl Drop for TestSandbox {
        fn drop(&mut self) {
            // 严格按照注册的逆序（从顶层下游向底层依赖）依次调服务接口删除，防止触发引用依赖锁
            for id in self.registered_ids.iter().rev() {
                let _ = self.service.delete_metadata(id, true, None);
            }
            // 彻底清理测试在磁盘生成的物理文件
            let _ = std::fs::remove_dir_all(&self.workspace_dir);
        }
    }

    #[test]
    fn test_from_root_and_cycle_ok_for_acyclic_graph() {
        let mut sandbox = TestSandbox::new("acyclic_graph");

        // 严格遵循自底向上注册策略：基础依赖 C 和 D 必须最先通过 DSFService 注册
        sandbox.register_mock("C", "v1", &[]);
        sandbox.register_mock("D", "v1", &[]);
        sandbox.register_mock("B", "v1", &["D@v1"]);
        sandbox.register_mock("A", "v1", &["B@v1", "C@v1"]);

        let graph_res = DatasetGraph::from_root("A@v1", sandbox.backend());
        assert!(
            graph_res.is_ok(),
            "Failed to load graph from root: {:?}",
            graph_res.err()
        );

        let graph = graph_res.unwrap();
        assert_eq!(graph.datasets.len(), 4);

        let cycle_res = graph.check_cycle();
        assert!(
            cycle_res.is_ok(),
            "Acyclic graph should not trigger cycle error"
        );
    }

    #[test]
    fn test_cycle_detects_simple_cycle() {
        let mut sandbox = TestSandbox::new("simple_cycle");

        // 正常构建单向合法依赖：A@v1 -> B@v1
        sandbox.register_mock("B", "v1", &[]);
        sandbox.register_mock("A", "v1", &["B@v1"]);

        // 使用 from_root_with_deps 在内存图结构中动态注入反向边 (B@v1 -> A@v1) 形成环。
        // 这样既能精准验证 DAG 核心算法，又不会让非法环形依赖破坏 SQLite 真实后端的完整性。
        let graph_res =
            DatasetGraph::from_root_with_deps("B", "v1", &["A@v1".to_string()], sandbox.backend());
        assert!(
            graph_res.is_ok(),
            "Memory graph construction should succeed"
        );

        let graph = graph_res.unwrap();
        match graph.check_cycle() {
            Err(DatasetGraphError::CycleDetected { node, dep }) => {
                let pairs = [(node.as_str(), dep.as_str()), (dep.as_str(), node.as_str())];
                assert!(
                    pairs.contains(&("A@v1", "B@v1")),
                    "Unexpected cycle pair: {} -> {}",
                    node,
                    dep
                );
            }
            other => panic!("Expected CycleDetected from check_cycle, got: {:?}", other),
        }
    }

    #[test]
    fn test_cycle_detects_self_cycle() {
        let mut sandbox = TestSandbox::new("self_cycle");
        sandbox.register_mock("A", "v1", &[]);

        // 试图向图结构中注入自环依赖：A@v1 -> A@v1
        let graph_res =
            DatasetGraph::from_root_with_deps("A", "v1", &["A@v1".to_string()], sandbox.backend());
        assert!(graph_res.is_ok());

        let graph = graph_res.unwrap();
        match graph.check_cycle() {
            Err(DatasetGraphError::CycleDetected { node, dep }) => {
                assert_eq!(node, "A@v1");
                assert_eq!(dep, "A@v1");
            }
            other => panic!(
                "Expected self-loop CycleDetected from check_cycle, got: {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_topo_sort_and_verify_subgraph() {
        let mut sandbox = TestSandbox::new("topo_sort");
        sandbox.register_mock("C", "v1", &[]);
        sandbox.register_mock("B", "v1", &["C@v1"]);
        sandbox.register_mock("A", "v1", &["B@v1"]);

        let mut graph = DatasetGraph::from_root("A@v1", sandbox.backend()).unwrap();
        let verify_res = graph.verify_subgraph("A@v1", false);

        assert!(
            verify_res.is_ok(),
            "Deep subgraph verification failed: {:?}",
            verify_res.err()
        );
        assert_eq!(verify_res.unwrap().status, DataSetStatus::Healthy);
    }

    #[test]
    fn default_equals_new() {
        let a = DatasetGraph::new();
        let b = DatasetGraph::default();
        assert_eq!(a.adj_list.len(), b.adj_list.len());
        assert_eq!(a.datasets.len(), b.datasets.len());
    }
}

#[cfg(test)]
mod service_status_tests {
    use super::tests::TestSandbox;
    use dsf_core::core::DataSetBusyStatus;

    #[test]
    fn test_service_mark_status_success() {
        let mut sandbox = TestSandbox::new("status_success");
        sandbox.register_mock("imagenet", "v1", &[]);
        let id = "imagenet@v1";

        // 1. 验证通过 DSFService 成功标记为 Reading 状态
        sandbox
            .service()
            .mark_status(id, DataSetBusyStatus::Reading, None)
            .expect("Failed to mark status as Reading via DSFService");

        let queried = sandbox
            .service()
            .query_meta(id, None)
            .expect("Failed to query metadata");
        assert_eq!(queried[0].1.busy_status, DataSetBusyStatus::Reading);

        // 2. 验证状态覆盖：切换为 Deleting 状态
        sandbox
            .service()
            .mark_status(id, DataSetBusyStatus::Deleting, None)
            .expect("Failed to mark status as Deleting via DSFService");

        let queried_again = sandbox
            .service()
            .query_meta(id, None)
            .expect("Failed to query metadata again");

        assert_eq!(queried_again[0].1.busy_status, DataSetBusyStatus::Deleting);
    }

    #[test]
    fn test_service_mark_status_not_found() {
        let sandbox = TestSandbox::new("status_not_found");
        let fake_id = "non_existent_dataset@v99.0";

        // 尝试为一个未通过 service 注册的虚假 ID 打标
        let res = sandbox
            .service()
            .mark_status(fake_id, DataSetBusyStatus::Modifying, None);

        assert!(res.is_err(), "对不存在的ID打标应当被底层拦截并返回 Err");
    }

    #[test]
    fn test_service_mark_status_after_deletion() {
        let mut sandbox = TestSandbox::new("status_after_deletion");
        sandbox.register_mock("mnist", "v2", &[]);
        let id = "mnist@v2";

        // 主动调用 API 删除该数据集
        sandbox.delete_mock(id);

        // 已从全局后注销的数据集，必须阻断任何状态更新请求
        let res = sandbox
            .service()
            .mark_status(id, DataSetBusyStatus::Creating, None);
        assert!(res.is_err(), "已被彻底删除的数据集 ID 不允许更新状态");
    }
}
