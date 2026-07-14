use crate::config::AppConfig;
use crate::core::{DataSetBusyStatus, MetaData, MetaDataError};
use crate::dag::DatasetGraphError;

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
    fn get_metadata(&self, id: &str) -> BackendResult<MetaData>;

    /// Mark MetaData status to ensure disk data and backend metadata consistency
    fn mark_status(&self, id: &str, status: DataSetBusyStatus) -> BackendResult<()>;
    /// Saves or updates the dataset metadata.
    fn save_metadata(&self, metadata: &MetaData) -> BackendResult<()>;
    /// Checks if any datasets depend on the specified `target_id`.
    ///
    /// Returns a list of dataset IDs that reference the target.
    fn check_is_referenced(&self, target_id: &str) -> BackendResult<Vec<String>>;
    /// Lists all available dataset metadata from the backend.
    fn list_all_metadata(&self) -> BackendResult<Vec<MetaData>>;
    /// Deletes the metadata associated with the specified dataset ID.
    /// note: this mucntion only deletes the metadata and detach this dataset from backend regisitration,
    /// real data on disk will be safe
    fn delete_metadata(&self, id: &str) -> BackendResult<()>;
}
pub type DynBackend = Box<dyn DatasetBackend + Send + Sync>;
pub type BackendRef<'a> = &'a (dyn DatasetBackend + Send + Sync);

#[derive(Error, Debug)]
pub enum BackendError {
    // 核心业务与状态错误
    #[error("Dataset not found: {id}")]
    DatasetNotFound { id: String },

    #[error("Target global backend is either unreachable or not found in config")]
    BackendNotFound,

    #[error("Dataset already exists: {id}")]
    AlreadyExists { id: String },

    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    #[error("Unsupported operation: {message}")]
    Unsupported { message: String },

    #[error("Invalid config: {message}")]
    InvalidConfig { message: String },

    // 存储与底层错误
    #[error("Database pool error: {message}")]
    PoolError { message: String },

    #[error("Failed setting pragma: {message}")]
    SetPragma { message: String },

    #[error("Database execution failed: {source}")]
    StorageError {
        #[from]
        source: rusqlite::Error,
    },

    #[error("Data serialization or parsing failed: {message}")]
    SerializationError { message: String },

    #[error("Internal I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Metadata core error: {source}")]
    MetaData {
        #[from]
        source: crate::core::MetaDataError,
    },
}

impl BackendError {
    pub fn to_io_error(self) -> io::Error {
        match self {
            BackendError::Io { source } => source,
            other => {
                let kind = match &other {
                    BackendError::DatasetNotFound { .. } => io::ErrorKind::NotFound,
                    BackendError::BackendNotFound => io::ErrorKind::NotFound,
                    BackendError::AlreadyExists { .. } => io::ErrorKind::AlreadyExists,
                    BackendError::PermissionDenied { .. } => io::ErrorKind::PermissionDenied,
                    _ => io::ErrorKind::Other,
                };
                io::Error::new(kind, other)
            }
        }
    }

    pub fn to_metadata_error(self) -> MetaDataError {
        match self {
            BackendError::MetaData { source } => source,
            other => {
                let io_err = other.to_io_error();
                MetaDataError::Io(io_err)
            }
        }
    }

    pub fn to_dag_error(self) -> DatasetGraphError {
        match self {
            BackendError::DatasetNotFound { id } => {
                DatasetGraphError::DatasetNotFound { node_id: id }
            }

            BackendError::MetaData { source } => {
                if let crate::core::MetaDataError::Io(io_err) = source {
                    DatasetGraphError::Io(io_err)
                } else {
                    let io_err =
                        std::io::Error::new(std::io::ErrorKind::InvalidData, source.to_string());
                    DatasetGraphError::Io(io_err)
                }
            }

            BackendError::Io { source } => DatasetGraphError::Io(source),
            other => {
                let io_err = other.to_io_error();
                DatasetGraphError::Io(io_err)
            }
        }
    }
}

pub type BackendResult<T> = Result<T, BackendError>;

pub fn capture_backtrace() {
    log::debug!("{}", std::backtrace::Backtrace::force_capture());
}

pub fn build_backend_auto() -> BackendResult<StackedBackend> {
    let cfg = AppConfig::load()?;
    StackedBackend::new(cfg.backend)
}
