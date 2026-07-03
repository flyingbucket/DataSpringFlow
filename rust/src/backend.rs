use crate::core::*;
use std::io;
/// SQLite 后端实现
pub struct SqliteBackend {
    conn_str: String, // 或者 r2d2::Pool<SqliteConnectionManager> 等
}

impl DatasetBackend for SqliteBackend {
    fn get_metadata(&self, id: &str) -> io::Result<MetaData> {
        // TODO: 执行 SQL 查询: SELECT ... FROM datasets WHERE id = ?
        unimplemented!("SQLite 尚未实现")
    }

    fn save_metadata(&self, _metadata: &MetaData) -> io::Result<()> {
        unimplemented!()
    }
}

/// YAML 文件后端实现
pub struct YamlBackend {
    workspace_dir: std::path::PathBuf,
}

impl DatasetBackend for YamlBackend {
    fn get_metadata(&self, id: &str) -> io::Result<MetaData> {
        // TODO: 从目录中读取文件，如: "/path/to/yaml/{id}.yaml" 然后用 serde_yaml 反序列化
        unimplemented!("YAML 尚未实现")
    }

    fn save_metadata(&self, _metadata: &MetaData) -> io::Result<()> {
        unimplemented!()
    }
}
