use crate::backend::DatasetBackend;
use crate::config::{AppConfig, BackendConfig};
use crate::core::MetaData;
use r2d2::Pool;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Error;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub db_path: PathBuf,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_busy_timeout_ms")]
    pub busy_timeout_ms: u64,
    #[serde(default = "default_wal")]
    pub wal: bool,
    #[serde(default = "default_synchronous")]
    pub synchronous: String, // "NORMAL" | "FULL"
    #[serde(default = "default_foreign_keys")]
    pub foreign_keys: bool,
}

fn default_pool_size() -> u32 {
    8
}
fn default_busy_timeout_ms() -> u64 {
    5000
}
fn default_wal() -> bool {
    true
}
fn default_synchronous() -> String {
    "NORMAL".to_string()
}
fn default_foreign_keys() -> bool {
    true
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("./datasets.db"),
            pool_size: default_pool_size(),
            busy_timeout_ms: default_busy_timeout_ms(),
            wal: default_wal(),
            synchronous: default_synchronous(),
            foreign_keys: default_foreign_keys(),
        }
    }
}

pub struct SqliteBackend {
    cfg: SqliteConfig,
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteBackend {
    /// 自动从配置文件加载：
    /// 1) $HOME/.config/dsf/config.yaml
    /// 2) /etc/dsf/config.yaml
    /// 3) 都没有则默认配置
    pub fn new() -> io::Result<Self> {
        let backend_cfg = AppConfig::load()?.backend;

        #[allow(unreachable_patterns)]
        match backend_cfg {
            BackendConfig::Sqlite(sqlite_cfg) => Self::from_config(sqlite_cfg),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Failed to initialize SqliteBackend: The active backend in config.yaml is NOT sqlite.",
            )),
        }
    }

    /// 允许在测试/特殊场景直接传配置
    pub fn from_config(cfg: SqliteConfig) -> io::Result<Self> {
        if let Some(parent) = cfg.db_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }

        let manager = SqliteConnectionManager::file(&cfg.db_path);
        let pool = Pool::builder()
            .max_size(cfg.pool_size)
            .build(manager)
            .map_err(|e| Error::other(format!("build sqlite pool failed: {e}")))?;

        let backend = Self { cfg, pool };
        backend.init()?;
        Ok(backend)
    }

    fn conn(&self) -> io::Result<PooledConnection<SqliteConnectionManager>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| Error::other(format!("get sqlite connection from pool failed: {e}")))?;

        self.apply_pragmas(&conn)?;
        Ok(conn)
    }

    fn apply_pragmas(&self, conn: &Connection) -> io::Result<()> {
        conn.busy_timeout(Duration::from_millis(self.cfg.busy_timeout_ms))
            .map_err(|e| Error::other(format!("set busy_timeout failed: {e}")))?;

        let wal_mode = if self.cfg.wal { "WAL" } else { "DELETE" };
        conn.pragma_update(None, "journal_mode", wal_mode)
            .map_err(|e| Error::other(format!("set journal_mode failed: {e}")))?;

        let sync = self.cfg.synchronous.to_uppercase();
        if sync != "NORMAL" && sync != "FULL" {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!(
                    "invalid synchronous value: {}, expected NORMAL or FULL",
                    self.cfg.synchronous
                ),
            ));
        }
        conn.pragma_update(None, "synchronous", &sync)
            .map_err(|e| Error::other(format!("set synchronous failed: {e}")))?;

        let fk = if self.cfg.foreign_keys { "ON" } else { "OFF" };
        conn.pragma_update(None, "foreign_keys", fk)
            .map_err(|e| Error::other(format!("set foreign_keys failed: {e}")))?;

        Ok(())
    }

    fn init(&self) -> io::Result<()> {
        let conn = self.conn()?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS datasets (
                id                TEXT PRIMARY KEY,
                name              TEXT NOT NULL,
                tag               TEXT NOT NULL,
                hash              TEXT NOT NULL,
                path              TEXT NOT NULL,
                description_path  TEXT NOT NULL,
                script_path       TEXT NOT NULL,
                dependencies_json TEXT NOT NULL,
                merkle_tree_path  TEXT NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_datasets_name_tag
            ON datasets(name, tag);
            "#,
        )
        .map_err(|e| Error::other(format!("init schema failed: {e}")))?;
        Ok(())
    }
}

impl DatasetBackend for SqliteBackend {
    fn get_metadata(&self, id: &str) -> io::Result<MetaData> {
        let conn = self.conn()?;

        let row = conn
            .query_row(
                r#"
                SELECT name, tag, hash, path, description_path, script_path, dependencies_json, merkle_tree_path
                FROM datasets
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    let name: String = row.get(0)?;
                    let tag: String = row.get(1)?;
                    let hash: String = row.get(2)?;
                    let path: String = row.get(3)?;
                    let description_path: String = row.get(4)?;
                    let script_path: String = row.get(5)?;
                    let dependencies_json: String = row.get(6)?;
                    let merkle_tree_path: String = row.get(7)?;

                    let dependencies: Vec<String> = serde_json::from_str(&dependencies_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                    Ok(MetaData {
                        name,
                        tag,
                        hash,
                        path: PathBuf::from(path),
                        description_path: PathBuf::from(description_path),
                        script_path: PathBuf::from(script_path),
                        dependencies,
                        merkle_tree_path: PathBuf::from(merkle_tree_path),
                    })
                },
            )
            .optional()
            .map_err(|e| Error::other(format!("query metadata failed: {e}")))?;

        row.ok_or_else(|| {
            Error::new(
                ErrorKind::NotFound,
                format!("dataset metadata not found: {id}"),
            )
        })
    }

    fn save_metadata(&self, metadata: &MetaData) -> io::Result<()> {
        let mut conn = self.conn()?;

        let deps_json = serde_json::to_string(&metadata.dependencies).map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("serialize deps failed: {e}"),
            )
        })?;

        // ACID: 用事务保证原子性；失败自动回滚
        let tx = conn
            .transaction()
            .map_err(|e| Error::other(format!("begin transaction failed: {e}")))?;

        tx.execute(
            r#"
            INSERT INTO datasets (
                id, name, tag, hash, path, description_path, script_path, dependencies_json, merkle_tree_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                tag = excluded.tag,
                hash = excluded.hash,
                path = excluded.path,
                description_path = excluded.description_path,
                script_path = excluded.script_path,
                dependencies_json = excluded.dependencies_json,
                merkle_tree_path = excluded.merkle_tree_path
            "#,
            params![
                metadata.id(),
                metadata.name,
                metadata.tag,
                metadata.hash,
                metadata.path.to_string_lossy(),
                metadata.description_path.to_string_lossy(),
                metadata.script_path.to_string_lossy(),
                deps_json,
                metadata.merkle_tree_path.to_string_lossy(),
            ],
        )
        .map_err(|e| Error::other(format!("save metadata failed: {e}")))?;

        tx.commit()
            .map_err(|e| Error::other(format!("commit transaction failed: {e}")))?;

        Ok(())
    }

    /// 检查是否有任何数据集依赖了指定的 target_id
    fn check_is_referenced(&self, target_id: &str) -> io::Result<Vec<String>> {
        let conn = self.conn()?;
        // 在 JSON 数组中查找，构造带双引号的子串，防止短名字误匹配（如匹配 "id1" 不会命中 "id10"）
        let pattern = format!("%\"{}\"%", target_id);

        let mut stmt = conn
            .prepare("SELECT name, tag FROM datasets WHERE dependencies_json LIKE ?1")
            .map_err(|e| Error::other(e.to_string()))?;

        let rows = stmt
            .query_map([pattern], |row| {
                let name: String = row.get(0)?;
                let tag: String = row.get(1)?;
                Ok(format!("{}@{}", name, tag))
            })
            .map_err(|e| Error::other(e.to_string()))?;

        let mut parents = Vec::new();
        for r in rows {
            parents.push(r.map_err(|e| Error::other(e.to_string()))?);
        }
        Ok(parents)
    }

    fn list_all_metadata(&self) -> io::Result<Vec<MetaData>> {
        let conn = self.conn()?;

        // 准备查询语句，捞出表内的全部元数据字段
        let mut stmt = conn
            .prepare(
                r#"
                SELECT name, tag, hash, path, description_path, script_path, dependencies_json, merkle_tree_path
                FROM datasets
                "#,
            )
            .map_err(|e| io::Error::other(format!("prepare list_all_metadata statement failed: {e}")))?;

        // 利用 query_map 进行流式行列映射
        let metadata_iter = stmt
            .query_map([], |row| {
                let name: String = row.get(0)?;
                let tag: String = row.get(1)?;
                let hash: String = row.get(2)?;
                let path: String = row.get(3)?;
                let description_path: String = row.get(4)?;
                let script_path: String = row.get(5)?;
                let dependencies_json: String = row.get(6)?;
                let merkle_tree_path: String = row.get(7)?;

                // 反序列化 JSON 依赖树数组
                let dependencies: Vec<String> =
                    serde_json::from_str(&dependencies_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                Ok(MetaData {
                    name,
                    tag,
                    hash,
                    path: PathBuf::from(path),
                    description_path: PathBuf::from(description_path),
                    script_path: PathBuf::from(script_path),
                    dependencies,
                    merkle_tree_path: PathBuf::from(merkle_tree_path),
                })
            })
            .map_err(|e| io::Error::other(format!("query_map all metadata failed: {e}")))?;

        // 收集迭代器中的结果，并把可能存在的错误（如中间某行反序列化失败）向上抛出
        let mut all_metadata = Vec::new();
        for meta_res in metadata_iter {
            all_metadata.push(
                meta_res
                    .map_err(|e| io::Error::other(format!("parse row failed in list_all: {e}")))?,
            );
        }

        Ok(all_metadata)
    }

    fn delete_metadata(&self, id: &str) -> io::Result<()> {
        let mut conn = self.conn()?;

        let tx = conn
            .transaction()
            .map_err(|e| io::Error::other(format!("begin delete transaction failed: {e}")))?;

        let rows_affected = tx
            .execute("DELETE FROM datasets WHERE id = ?1", params![id])
            .map_err(|e| io::Error::other(format!("execute delete statement failed: {e}")))?;

        // 如果影响行数为 0，说明这个 id 本就不存在。
        // 在工业级设计中，可以选择抛出 NotFound 错误，也可以静默当作 Ok(()) 成功。
        // 这里推荐严格抛出 NotFound，给前端 CLI 或者业务层提供精准的反馈控制。
        if rows_affected == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("cannot delete dataset: id '{}' not found in database", id),
            ));
        }

        tx.commit()
            .map_err(|e| io::Error::other(format!("commit delete transaction failed: {e}")))?;

        Ok(())
    }
}
