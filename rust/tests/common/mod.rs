use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Error, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::RwLock;

use dataspringflow_rs::backend::DatasetBackend;
use dataspringflow_rs::core::MetaData;

/// 纯内存 Mock 后端
pub struct MemoryBackend {
    store: RwLock<HashMap<String, MetaData>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
        }
    }
}

impl DatasetBackend for MemoryBackend {
    fn get_metadata(&self, id: &str) -> io::Result<MetaData> {
        let store = self.store.read().unwrap();
        store
            .get(id)
            .cloned()
            .ok_or_else(|| Error::new(ErrorKind::NotFound, format!("Mock: ID {} not found", id)))
    }

    fn save_metadata(&self, metadata: &MetaData) -> io::Result<()> {
        let mut store = self.store.write().unwrap();
        store.insert(metadata.id(), metadata.clone());
        Ok(())
    }

    fn check_is_referenced(&self, target_id: &str) -> io::Result<Vec<String>> {
        let _ = target_id;
        todo!()
    }

    fn list_all_metadata(&self) -> io::Result<Vec<MetaData>> {
        todo!()
    }

    fn delete_metadata(&self, id: &str) -> io::Result<()> {
        let _ = id;
        todo!()
    }
}

pub struct TestSandbox {
    base_dir: PathBuf,
}

impl TestSandbox {
    pub fn new(test_name: &str) -> Self {
        let mut base_dir = std::env::temp_dir();
        base_dir.push("dsf_tests");
        base_dir.push(test_name);

        // 如果存在旧的残留，先清空
        let _ = fs::remove_dir_all(&base_dir);
        fs::create_dir_all(&base_dir).expect("无法创建测试沙盒目录");

        Self { base_dir }
    }

    /// 在沙盒中生成一个伪造的数据集文件夹
    pub fn create_dummy_dataset(&self, folder_name: &str, file_content: &str) -> PathBuf {
        let ds_path = self.base_dir.join(folder_name);
        fs::create_dir_all(&ds_path).unwrap();

        let file_path = ds_path.join("data.txt");
        let mut file = File::create(file_path).unwrap();
        file.write_all(file_content.as_bytes()).unwrap();

        ds_path
    }

    /// 模拟篡改磁盘上的数据（用来测试损毁校验）
    pub fn tamper_file(&self, folder_name: &str, new_content: &str) {
        let file_path = self.base_dir.join(folder_name).join("data.txt");
        let mut file = File::create(file_path).unwrap();
        file.write_all(new_content.as_bytes()).unwrap();
    }
}

impl Drop for TestSandbox {
    fn drop(&mut self) {
        // 测试结束后自动清理临时磁盘垃圾
        let _ = fs::remove_dir_all(&self.base_dir);
    }
}
