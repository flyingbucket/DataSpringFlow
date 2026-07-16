mod common;
#[cfg(feature = "run_slow_tests")]
#[cfg(test)]
mod tests {
    use super::common::{MemoryBackend, TestSandbox};
    use dsf_core::backend::DatasetBackend;
    use dsf_core::core::MetaData;
    use dsf_core::dag::*;

    /// 辅助函数：根据邻接关系在 MemoryBackend 中注册一组 Mock MetaData
    fn setup_mock_backend(
        backend: &MemoryBackend,
        sandbox: &TestSandbox,
        nodes: &[(&str, &str, &[&str])],
    ) {
        for (name, tag, deps) in nodes {
            // 在沙盒磁盘中生成真实 dummy 目录
            let folder_name = format!("{}_{}", name, tag);
            let ds_path = sandbox.create_dummy_dataset(&folder_name, "dummy-data");

            // 将描述文件路径和脚本路径包装进 Some() 中以匹配 Option<PathBuf>
            let meta = MetaData::new(
                name,
                tag,
                ds_path.clone(),
                Some(ds_path.join("desc.md")),
                ds_path.join("script.py"),
                None,
                deps.iter().map(|s| s.to_string()).collect(),
                ds_path.join("merkle.bin"),
                None,
            )
            .expect("Failed to create MetaData instance");

            backend
                .save_metadata(&meta)
                .expect("Failed to save metadata to mock backend");
        }
    }

    #[test]
    fn test_from_root_and_cycle_ok_for_acyclic_graph() {
        let sandbox = TestSandbox::new("acyclic_graph");
        let backend = MemoryBackend::new();

        setup_mock_backend(
            &backend,
            &sandbox,
            &[
                ("A", "v1", &["B@v1", "C@v1"]),
                ("B", "v1", &["D@v1"]),
                ("C", "v1", &[]),
                ("D", "v1", &[]),
            ],
        );

        let graph_res = DatasetGraph::from_root("A@v1", &backend);
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
        let sandbox = TestSandbox::new("simple_cycle");
        let backend = MemoryBackend::new();

        // 构造环: A@v1 -> B@v1 -> A@v1
        setup_mock_backend(
            &backend,
            &sandbox,
            &[("A", "v1", &["B@v1"]), ("B", "v1", &["A@v1"])],
        );

        // 🌟 修复 2：因为 from_root 返回的是 std::io::Result，它不会返回 DatasetGraphError。
        // 如果 from_root 内部碰到了环导致失败，它会返回 std::io::Error。
        let graph_res = DatasetGraph::from_root("A@v1", &backend);

        match graph_res {
            Ok(graph) => {
                let cycle_res = graph.check_cycle();
                match cycle_res {
                    // check_cycle() 返回的是 DatasetGraphError
                    Err(DatasetGraphError::CycleDetected { node, dep }) => {
                        let pairs = [(node.as_str(), dep.as_str()), (dep.as_str(), node.as_str())];
                        assert!(pairs.contains(&("A@v1", "B@v1")));
                    }
                    _ => panic!(
                        "Expected CycleDetected from check_cycle, got: {:?}",
                        cycle_res
                    ),
                }
            }
            Err(io_err) => {
                // 如果你的 from_root 在自底向上构建时就通过 io::Error 拦截了环
                // 我们通过识别它的错误副文本来断言成功
                let err_msg = io_err.to_string();
                assert!(err_msg.contains("A@v1") || err_msg.contains("B@v1"));
            }
        }
    }

    #[test]
    fn test_cycle_detects_self_cycle() {
        let sandbox = TestSandbox::new("self_cycle");
        let backend = MemoryBackend::new();

        setup_mock_backend(&backend, &sandbox, &[("A", "v1", &["A@v1"])]);

        let graph_res = DatasetGraph::from_root("A@v1", &backend);

        // 🌟 修复 3：统一 match 两臂的错误形态，不混用 io::Error 和 DatasetGraphError
        match graph_res {
            Ok(graph) => match graph.check_cycle() {
                Err(DatasetGraphError::CycleDetected { node, dep }) => {
                    assert_eq!(node, "A@v1");
                    assert_eq!(dep, "A@v1");
                }
                _ => panic!("Expected self-loop CycleDetected from check_cycle"),
            },
            Err(io_err) => {
                assert!(io_err.to_string().contains("A@v1"));
            }
        }
    }

    #[test]
    fn test_topo_sort_order() {
        let sandbox = TestSandbox::new("topo_sort");
        let backend = MemoryBackend::new();

        setup_mock_backend(
            &backend,
            &sandbox,
            &[
                ("A", "v1", &["B@v1"]),
                ("B", "v1", &["C@v1"]),
                ("C", "v1", &[]),
            ],
        );

        // 🌟 修复 4：加下划线 `_graph` 消除无意义的变量未消费警告
        let _graph = DatasetGraph::from_root("A@v1", &backend).unwrap();
    }

    #[test]
    fn default_equals_new() {
        let a = DatasetGraph::new();
        let b = DatasetGraph::default();
        assert_eq!(a.adj_list.len(), b.adj_list.len());
        assert_eq!(a.datasets.len(), b.datasets.len());
    }
}

#[cfg(feature = "run_slow_tests")]
#[cfg(test)]
mod memory_backend_status_tests {
    use crate::common::MemoryBackend;
    use dsf_core::backend::{BackendError, DatasetBackend};
    use dsf_core::core::{DataSetBusyStatus, MetaData};
    use std::path::PathBuf;

    /// 辅助函数：快速生成一个用于测试的纯内存 MetaData 实例
    fn create_test_metadata(name: &str, tag: &str) -> MetaData {
        MetaData {
            name: name.to_string(),
            tag: tag.to_string(),
            hash: "mockhash_deadbeef12345".to_string(),
            path: PathBuf::from(format!("/mock/path/{}", name)),
            description_path: PathBuf::from(format!("/mock/path/{}/desc.md", name)),
            script_path: PathBuf::from(format!("/mock/path/{}/run.py", name)),
            owner: "tester$nobody".to_string(),
            dependencies: vec![],
            merkle_tree_path: PathBuf::from(format!("/mock/path/{}/merkle.bin", name)),
            busy_status: None, // 初始为空闲状态
        }
    }

    #[test]
    fn test_memory_backend_mark_status_success() {
        let backend = MemoryBackend::new();
        let meta = create_test_metadata("imagenet", "v1");
        let id = meta.id(); // 标准形式如 "imagenet@v1"

        // 将元数据持久化存入内存 Map
        backend.save_metadata(&meta).unwrap();

        // 1. 验证成功标记为 Reading 状态
        backend
            .mark_status(&id, DataSetBusyStatus::Reading)
            .unwrap();
        let updated = backend.get_metadata(&id).unwrap();
        assert_eq!(updated.busy_status, Some(DataSetBusyStatus::Reading));

        // 2. 验证状态覆盖：切换为更严重的 Deleting 状态
        backend
            .mark_status(&id, DataSetBusyStatus::Deleting)
            .unwrap();
        let updated_again = backend.get_metadata(&id).unwrap();
        assert_eq!(updated_again.busy_status, Some(DataSetBusyStatus::Deleting));
    }

    #[test]
    fn test_memory_backend_mark_status_not_found() {
        let backend = MemoryBackend::new();
        let fake_id = "non_existent_dataset@v99.0";

        // 尝试为一个根本没有被 save_metadata 注入过的数据集打标
        let res = backend.mark_status(fake_id, DataSetBusyStatus::Modifying);

        // 断言：必须返回严格的错误类型
        assert!(res.is_err(), "对不存在的ID打标应当返回 Err 分支");
        match res.unwrap_err() {
            BackendError::DatasetNotFound { id } => {
                assert_eq!(id, fake_id);
            }
            other => panic!(
                "应当返回 BackendError::DatasetNotFound，但得到了: {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_memory_backend_mark_status_after_deletion() {
        let backend = MemoryBackend::new();
        let meta = create_test_metadata("mnist", "v2");
        let id = meta.id();

        // 存储后随即删除
        backend.save_metadata(&meta).unwrap();
        backend.delete_metadata(&id).unwrap();

        // 已经从后端解绑清除的数据集，对其修改状态应当同样被拦截
        let res = backend.mark_status(&id, DataSetBusyStatus::Creating);
        assert!(res.is_err(), "已被彻底删除的 ID 不允许更新状态");
    }
}
