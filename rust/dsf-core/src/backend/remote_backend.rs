use crate::backend::{BackendResult, DatasetBackend};
use crate::core::DataSetBusyStatus;
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
    fn get_metadata(&self, id: &str) -> BackendResult<crate::core::MetaData> {
        let _ = id;
        todo!()
    }

    fn mark_status(&self, id: &str, status: DataSetBusyStatus) -> BackendResult<()> {
        let _ = status;
        let _ = id;
        todo!()
    }
    fn save_metadata(&self, metadata: &crate::core::MetaData) -> BackendResult<()> {
        let _ = metadata;
        todo!()
    }

    fn check_is_referenced(&self, target_id: &str) -> BackendResult<Vec<String>> {
        let _ = target_id;
        todo!()
    }

    fn list_all_metadata(&self) -> BackendResult<Vec<crate::core::MetaData>> {
        todo!()
    }

    fn delete_metadata(&self, id: &str) -> BackendResult<()> {
        let _ = id;
        todo!()
    }
}
