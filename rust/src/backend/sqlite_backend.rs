use crate::backend::{BackendError, BackendResult, DatasetBackend, capture_backtrace};
use crate::core::MetaData;
use r2d2::Pool;
use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Error;
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
    false
}
fn default_synchronous() -> String {
    "NORMAL".to_string()
}
fn default_foreign_keys() -> bool {
    true
}

impl SqliteConfig {
    // 初始化时强制要求传入必填字段
    pub fn new(db_path: PathBuf) -> Self {
        Self {
            db_path,
            pool_size: default_pool_size(),
            busy_timeout_ms: default_busy_timeout_ms(),
            wal: default_wal(),
            synchronous: default_synchronous(),
            foreign_keys: default_foreign_keys(),
        }
    }

    // 可选字段的流式链式设置方法
    pub fn pool_size(mut self, size: u32) -> Self {
        self.pool_size = size;
        self
    }

    pub fn busy_timeout(mut self, ms: u64) -> Self {
        self.busy_timeout_ms = ms;
        self
    }

    // 最终装装配出不可变的真正配置
    pub fn build(self) -> SqliteConfig {
        SqliteConfig {
            db_path: self.db_path,
            pool_size: self.pool_size,
            busy_timeout_ms: self.busy_timeout_ms,
            wal: self.wal,
            synchronous: self.synchronous,
            foreign_keys: self.foreign_keys,
        }
    }
}
pub struct SqliteBackend {
    cfg: SqliteConfig,
    pool: Pool<SqliteConnectionManager>,
}

impl SqliteBackend {
    pub fn new(cfg: SqliteConfig) -> BackendResult<Self> {
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

        let backend = Self {
            cfg: cfg.clone(),
            pool,
        };
        backend.init()?;
        Ok(backend)
    }

    fn conn(&self) -> BackendResult<PooledConnection<SqliteConnectionManager>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| Error::other(format!("get sqlite connection from pool failed: {e}")))?;

        self.apply_pragmas(&conn)?;
        Ok(conn)
    }

    fn apply_pragmas(&self, conn: &Connection) -> BackendResult<()> {
        conn.busy_timeout(Duration::from_millis(self.cfg.busy_timeout_ms))
            .map_err(|e| Error::other(format!("set busy_timeout failed: {e}")))?;

        let wal_mode = if self.cfg.wal { "WAL" } else { "DELETE" };
        conn.pragma_update(None, "journal_mode", wal_mode)
            .map_err(|e| {
                capture_backtrace();
                BackendError::SetPragma {
                    message: format!("journal_mode {e}"),
                }
            })?;

        let sync = self.cfg.synchronous.to_uppercase();
        if sync != "NORMAL" && sync != "FULL" {
            return Err(BackendError::InvalidConfig {
                message: format!(
                    "synchronous value {}, expected NORMAL or FULL",
                    self.cfg.synchronous
                ),
            });
        }
        conn.pragma_update(None, "synchronous", &sync)
            .map_err(|e| {
                capture_backtrace();
                BackendError::SetPragma {
                    message: format!("synchronous: {e}"),
                }
            })?;

        let fk = if self.cfg.foreign_keys { "ON" } else { "OFF" };
        conn.pragma_update(None, "foreign_keys", fk).map_err(|e| {
            capture_backtrace();
            BackendError::SetPragma {
                message: format!("foreign_keys: {e}"),
            }
        })?;

        Ok(())
    }

    fn init(&self) -> BackendResult<()> {
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
                owner             TEXT NOT NULL,
                dependencies_json TEXT NOT NULL,
                merkle_tree_path  TEXT NOT NULL
            );

            CREATE UNIQUE INDEX IF NOT EXISTS idx_datasets_name_tag
            ON datasets(name, tag);
            "#,
        )
        .map_err(|e| {
            capture_backtrace();
            log::error!("init schema failed");
            BackendError::StorageError { source: e }
        })?;
        Ok(())
    }
}

impl DatasetBackend for SqliteBackend {
    fn get_metadata(&self, id: &str) -> BackendResult<MetaData> {
        let conn = self.conn()?;

        let raw_data = conn
        .query_row(
            r#"
            SELECT name, tag, hash, path, description_path, script_path, owner, dependencies_json, merkle_tree_path
            FROM datasets WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?, // dependencies_json
                    row.get::<_, String>(8)?,
                ))
            },
        )
        .optional()
        .map_err(BackendError::from)?;

        // 3. 检查数据是否存在
        let (
            name,
            tag,
            hash,
            path,
            description_path,
            script_path,
            owner,
            dependencies_json,
            merkle_tree_path,
        ) = raw_data.ok_or_else(|| {
            capture_backtrace();
            BackendError::DatasetNotFound { id: id.to_string() }
        })?;

        let dependencies: Vec<String> = serde_json::from_str(&dependencies_json).map_err(|e| {
            capture_backtrace();
            BackendError::SerializationError {
                message: e.to_string(),
            }
        })?;

        Ok(MetaData {
            name,
            tag,
            hash,
            path: PathBuf::from(path),
            description_path: PathBuf::from(description_path),
            script_path: PathBuf::from(script_path),
            owner,
            dependencies,
            merkle_tree_path: PathBuf::from(merkle_tree_path),
        })
    }

    fn save_metadata(&self, metadata: &MetaData) -> BackendResult<()> {
        let mut conn = self.conn()?;

        let deps_json = serde_json::to_string(&metadata.dependencies).map_err(|e| {
            capture_backtrace();
            BackendError::SerializationError {
                message: format!("serialize deps failed: {e}"),
            }
        })?;

        // ACID: 用事务保证原子性；失败自动回滚
        let tx = conn.transaction().map_err(|e| BackendError::PoolError {
            message: format!("save metadata failed: {e}"),
        })?;

        tx.execute(
            r#"
            INSERT INTO datasets (
                id, name, tag, hash, path, description_path, script_path, owner, dependencies_json, merkle_tree_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                tag = excluded.tag,
                hash = excluded.hash,
                path = excluded.path,
                description_path = excluded.description_path,
                script_path = excluded.script_path,
                owner = excluded.owner,
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
                metadata.owner,
                deps_json,
                metadata.merkle_tree_path.to_string_lossy(),
            ],
        )?;

        tx.commit()?;

        Ok(())
    }

    /// 检查是否有任何数据集依赖了指定的 target_id
    fn check_is_referenced(&self, target_id: &str) -> BackendResult<Vec<String>> {
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

    fn list_all_metadata(&self) -> BackendResult<Vec<MetaData>> {
        let conn = self.conn()?;

        // 准备查询语句，捞出表内的全部元数据字段
        let mut stmt = conn
            .prepare(
                r#"
                SELECT name, tag, hash, path, description_path, script_path, owner, dependencies_json, merkle_tree_path
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
                let owner: String = row.get(6)?;
                let dependencies_json: String = row.get(7)?;
                let merkle_tree_path: String = row.get(8)?;

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
                    owner,
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

    fn delete_metadata(&self, id: &str) -> BackendResult<()> {
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
            capture_backtrace();
            return Err(BackendError::DatasetNotFound { id: id.to_string() });
        }

        tx.commit()
            .map_err(|e| io::Error::other(format!("commit delete transaction failed: {e}")))?;

        Ok(())
    }
}
