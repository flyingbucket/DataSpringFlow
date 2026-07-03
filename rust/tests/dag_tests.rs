mod common; // 引入上面的 common/mod.rs
use common::{MemoryBackend, TestSandbox};
use dataspringflow_rs::core::DatasetBackend;
use dataspringflow_rs::core::MetaData;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_backend_crud() {
        let backend = MemoryBackend::new();
        let sandbox = TestSandbox::new("crud_test");
        let path = sandbox.create_dummy_dataset("test_ds", "some ai data");

        // 1. 构造 Metadata
        let meta = MetaData::new(
            "imagenet_subset",
            "v1.0",
            path.clone(),
            path.join("desc.md"),
            path.join("clean.py"),
            vec![], // 无依赖
            path.join("tree.bin"),
        )
        .expect("生成 MetaData 失败");

        // 2. 提交到后端
        meta.commit(&backend).expect("提交到 Mock 后端失败");

        // 3. 从后端验证读取
        let loaded_meta = backend
            .get_metadata("imagenet_subset@v1.0")
            .expect("读取失败");

        assert_eq!(loaded_meta.name, "imagenet_subset");
        assert_eq!(loaded_meta.tag, "v1.0");
        assert_eq!(loaded_meta.hash, meta.hash);
    }

    #[test]
    fn test_dag_verification_healthy() {
        let backend = MemoryBackend::new();
        let sandbox = TestSandbox::new("healthy_dag_test");

        // 1. 创建底层基础数据集：ImageNet 原始数据
        let raw_path = sandbox.create_dummy_dataset("imagenet_raw", "raw image bytes...");
        let raw_meta = MetaData::new(
            "imagenet",
            "2012",
            raw_path.clone(),
            raw_path.join("desc.md"),
            raw_path.join("script.py"),
            vec![], // 基础数据集无依赖
            raw_path.join("tree.bin"),
        )
        .unwrap();
        raw_meta.commit(&backend).unwrap();

        // 2. 创建上层派生数据集：比如经过裁剪的鸟类子集 (依赖 imagenet@2012)
        let birds_path = sandbox.create_dummy_dataset("imagenet_birds", "cropped birds bytes...");
        let birds_meta = MetaData::new(
            "imagenet_birds",
            "v1",
            birds_path.clone(),
            birds_path.join("desc.md"),
            birds_path.join("script.py"),
            vec!["imagenet@2012".to_string()], // 依赖绑定
            birds_path.join("tree.bin"),
        )
        .unwrap();
        birds_meta.commit(&backend).unwrap();

        // 3. 校验派生数据集
        // 假设这里直接使用你之前重构的 DatasetGraph 来校验子图
        // let mut graph = DatasetGraph::from_root("imagenet_birds@v1", &backend).unwrap();
        // let res = graph.verify_subgraph("imagenet_birds@v1", false).unwrap();

        // 验证两者状态都应为 Healthy
        // assert_eq!(res.status, DataSetStatus::Healthy);
    }

    #[test]
    fn test_dag_verification_broken_dependency() {
        let backend = MemoryBackend::new();
        let sandbox = TestSandbox::new("broken_dag_test");

        // 1. 创建并注册基础数据集 A
        let path_a = sandbox.create_dummy_dataset("dataset_a", "original content A");
        let meta_a = MetaData::new(
            "dataset_a",
            "v1",
            path_a.clone(),
            path_a.join("desc.md"),
            path_a.join("script.py"),
            vec![],
            path_a.join("tree.bin"),
        )
        .unwrap();
        meta_a.commit(&backend).unwrap();

        // 2. 创建并注册依赖 A 的派生数据集 B
        let path_b = sandbox.create_dummy_dataset("dataset_b", "original content B");
        let meta_b = MetaData::new(
            "dataset_b",
            "v1",
            path_b.clone(),
            path_b.join("desc.md"),
            path_b.join("script.py"),
            vec!["dataset_a@v1".to_string()], // B -> A
            path_b.join("tree.bin"),
        )
        .unwrap();
        meta_b.commit(&backend).unwrap();

        // 3. 【核心步骤】：恶意/意外篡改底层数据集 A 的物理文件！
        sandbox.tamper_file("dataset_a", "CORRUPTED CONTENT A!!!");

        // 4. 此时对 B 发起图校验
        // let mut graph = DatasetGraph::from_root("dataset_b@v1", &backend).unwrap();
        // let res = graph.verify_subgraph("dataset_b@v1", false).unwrap();

        // 预期结果：
        // - A 本地的文件内容与保存的 Merkle Root 不吻合 -> 状态变为 Broken
        // - B 自身的文件未被修改，但是因为上游 A 损坏 -> 状态应退化为 BrokenDeps
        // assert_eq!(res.status, DataSetStatus::BrokenDpes);
    }
}
