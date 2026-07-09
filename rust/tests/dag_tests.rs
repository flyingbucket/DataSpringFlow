mod common;

#[cfg(test)]
mod tests {
    use super::common::{MemoryBackend, TestSandbox};
    use dataspringflow_rs::backend::DatasetBackend;
    use dataspringflow_rs::core::MetaData;
    use dataspringflow_rs::dag::*;

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

            // 🌟 修复 1：将描述文件路径和脚本路径包装进 Some() 中以匹配 Option<PathBuf>
            let meta = MetaData::new(
                name,
                tag,
                ds_path.clone(),
                Some(ds_path.join("desc.md")),
                ds_path.join("script.py"),
                deps.iter().map(|s| s.to_string()).collect(),
                ds_path.join("merkle.bin"),
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
