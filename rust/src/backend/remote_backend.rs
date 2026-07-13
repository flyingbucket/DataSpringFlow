use crate::backend::DatasetBackend;
use serde::{Deserialize, Serialize};

pub struct RemoteBackend {}

impl RemoteBackend {
    pub fn reachable(&self) -> bool {
        todo!()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteConfig {}
impl DatasetBackend for RemoteBackend {
    fn get_metadata(&self, id: &str) -> std::io::Result<crate::core::MetaData> {
        let _ = id;
        todo!()
    }

    fn save_metadata(&self, metadata: &crate::core::MetaData) -> std::io::Result<()> {
        let _ = metadata;
        todo!()
    }

    fn check_is_referenced(&self, target_id: &str) -> std::io::Result<Vec<String>> {
        let _ = target_id;
        todo!()
    }

    fn list_all_metadata(&self) -> std::io::Result<Vec<crate::core::MetaData>> {
        todo!()
    }

    fn delete_metadata(&self, id: &str) -> std::io::Result<()> {
        let _ = id;
        todo!()
    }
}
