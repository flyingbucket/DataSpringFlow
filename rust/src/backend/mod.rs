use crate::config::{AppConfig, BackendConfig};
use crate::core::MetaData;

use std::io;
use thiserror::Error;

mod sqlite_backend;
pub use sqlite_backend::{SqliteBackend, SqliteConfig};

pub trait DatasetBackend {
    /// 根据数据集 ID 获取对应的元数据
    fn get_metadata(&self, id: &str) -> io::Result<MetaData>;

    /// 保存或更新数据集元数据
    fn save_metadata(&self, metadata: &MetaData) -> io::Result<()>;

    /// 检查是否有任何数据集依赖了指定的 target_id
    fn check_is_referenced(&self, target_id: &str) -> io::Result<Vec<String>>;

    fn list_all_metadata(&self) -> io::Result<Vec<MetaData>>;

    fn delete_metadata(&self, id: &str) -> io::Result<()>;
}

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Data set not found: {id}")]
    NotFound { id: String },

    #[error("Backend connection broken: {0}")]
    ConnectionError(String),

    #[error("底层存储执行错误: {0}")]
    StorageError(#[from] rusqlite::Error),

    #[error("元数据序列化/反序列化失败: {0}")]
    SerializationError(String),

    #[error("通用输入输出错误: {0}")]
    Io(#[from] std::io::Error),
}

pub type BackendResult<T> = Result<T, BackendError>;

pub type DynBackend = Box<dyn DatasetBackend + Send + Sync>;
pub type BackendRef<'a> = &'a (dyn DatasetBackend + Send + Sync);

pub fn build_backend_auto() -> io::Result<DynBackend> {
    let cfg = AppConfig::load()?;
    match &cfg.backend {
        BackendConfig::Sqlite(sqlite_cfg) => Ok(Box::new(SqliteBackend::new(sqlite_cfg.clone())?)),
        // BackendConfig::Yaml(yaml_cfg) => ...
        // BackendConfig::Remote(remote_cfg) => ...
    }
}
