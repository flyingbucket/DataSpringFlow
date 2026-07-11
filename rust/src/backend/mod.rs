use crate::config::AppConfig;
use crate::core::MetaData;

use std::io;
use thiserror::Error;

mod sqlite_backend;
pub use sqlite_backend::{SqliteBackend, SqliteConfig};

mod remote_backend;
pub use remote_backend::{RemoteBackend, RemoteConfig};

mod router;
pub use router::{
    BackendAddr, GlobalBackend, GlobalBackendAddr, ScopedId, ScopedMetaData, StackedBackend,
    StackedBackendConfig,
};

pub trait DatasetBackend {
    /// Retrieves the corresponding metadata by the dataset ID.
    fn get_metadata(&self, id: &str) -> io::Result<MetaData>;
    /// Saves or updates the dataset metadata.
    fn save_metadata(&self, metadata: &MetaData) -> io::Result<()>;
    /// Checks if any datasets depend on the specified `target_id`.
    ///
    /// Returns a list of dataset IDs that reference the target.
    fn check_is_referenced(&self, target_id: &str) -> io::Result<Vec<String>>;
    /// Lists all available dataset metadata from the backend.
    fn list_all_metadata(&self) -> io::Result<Vec<MetaData>>;
    /// Deletes the metadata associated with the specified dataset ID.
    /// note: this mucntion only deletes the metadata and detach this dataset from backend regisitration,
    /// real data on disk will be safe
    fn delete_metadata(&self, id: &str) -> io::Result<()>;
}
pub type DynBackend = Box<dyn DatasetBackend + Send + Sync>;
pub type BackendRef<'a> = &'a (dyn DatasetBackend + Send + Sync);

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Data set not found: {id}")]
    NotFound { id: String },

    #[error("Backend connection broken: {0}")]
    ConnectionError(String),

    #[error("Underlying storage execution error: {0}")]
    StorageError(#[from] rusqlite::Error),

    #[error("Metadata serialization/deserialization failed: {0}")]
    SerializationError(String),

    #[error("General I/O error: {0}")]
    Io(#[from] std::io::Error),
}
pub type BackendResult<T> = Result<T, BackendError>;

pub fn build_backend_auto() -> io::Result<StackedBackend> {
    let cfg = AppConfig::load()?;
    StackedBackend::new(cfg.backend)
}
