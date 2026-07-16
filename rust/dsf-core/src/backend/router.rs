use crate::backend::{BackendError, BackendRef, BackendResult};
use crate::backend::{DatasetBackend, RemoteBackend, SqliteBackend, SqliteConfig};
use crate::config::AppConfig;
use crate::core::{DataSetBusyStatus, MetaData, MetaDataError};
use crate::utils::get_username;

use serde::{Deserialize, Serialize};

use std::fmt;
use std::fs;
use std::io::{self};
use std::path::PathBuf;

pub enum GlobalBackend {
    Sqlite(SqliteBackend),
    Remote(RemoteBackend),
}

impl DatasetBackend for GlobalBackend {
    fn get_metadata(&self, id: &str) -> BackendResult<MetaData> {
        match self {
            GlobalBackend::Sqlite(be) => be.get_metadata(id),
            GlobalBackend::Remote(be) => be.get_metadata(id),
        }
    }

    fn mark_status(&self, id: &str, status: DataSetBusyStatus) -> BackendResult<()> {
        match self {
            GlobalBackend::Sqlite(be) => be.mark_status(id, status),
            GlobalBackend::Remote(be) => be.mark_status(id, status),
        }
    }
    fn save_metadata(&self, metadata: &MetaData) -> BackendResult<()> {
        match self {
            GlobalBackend::Sqlite(be) => be.save_metadata(metadata),
            GlobalBackend::Remote(be) => be.save_metadata(metadata),
        }
    }
    fn check_is_referenced(&self, target_id: &str) -> BackendResult<Vec<String>> {
        match self {
            GlobalBackend::Sqlite(be) => be.check_is_referenced(target_id),
            GlobalBackend::Remote(be) => be.check_is_referenced(target_id),
        }
    }
    fn list_all_metadata(&self) -> BackendResult<Vec<MetaData>> {
        match self {
            GlobalBackend::Sqlite(be) => be.list_all_metadata(),
            GlobalBackend::Remote(be) => be.list_all_metadata(),
        }
    }
    fn delete_metadata(&self, id: &str) -> BackendResult<()> {
        match self {
            GlobalBackend::Sqlite(be) => be.delete_metadata(id),
            GlobalBackend::Remote(be) => be.delete_metadata(id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum GlobalBackendAddr {
    Sqlite {
        config_path: PathBuf,
    },
    Remote {
        server_url: String, // future impl
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackendAddr {
    Private { username: String },
    Global { addr: GlobalBackendAddr },
}

impl fmt::Display for GlobalBackendAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobalBackendAddr::Sqlite { config_path } => {
                write!(f, "Global(Sqlite: {})", config_path.display())
            }
            GlobalBackendAddr::Remote { server_url } => {
                write!(f, "Global(Remote: {})", server_url)
            }
        }
    }
}

impl fmt::Display for BackendAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendAddr::Private { username } => {
                write!(f, "Private(owner: {})", username)
            }
            BackendAddr::Global { addr } => {
                // 直接委托给 GlobalBackendAddr 的 Display 实现
                write!(f, "{}", addr)
            }
        }
    }
}
impl GlobalBackendAddr {
    pub fn resolve_to_backend(&self) -> BackendResult<GlobalBackend> {
        match self {
            GlobalBackendAddr::Sqlite { config_path } => {
                let content = fs::read_to_string(config_path).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "Failed to read global backend config at {}: {}",
                            config_path.display(),
                            e
                        ),
                    )
                })?;

                let target_app_cfg: AppConfig = serde_yaml::from_str(&content).map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Failed to parse global config: {}", e),
                    )
                })?;

                let true_sqlite_cfg = target_app_cfg.backend.private_sqlite_cfg;

                log::info!(
                    "Successfully resolved global backend with DB: {}",
                    true_sqlite_cfg.db_path.display()
                );
                Ok(GlobalBackend::Sqlite(SqliteBackend::new(true_sqlite_cfg)?))
            }
            GlobalBackendAddr::Remote { server_url } => {
                let _ = server_url;
                Err(BackendError::Unsupported {
                    message: "Remote backend feature is currently unimplemented. Stay tuned!"
                        .to_string(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackedBackendConfig {
    pub private_sqlite_cfg: SqliteConfig,
    pub global_repos: Vec<GlobalBackendAddr>,
}
impl StackedBackendConfig {
    pub fn new(private_sqlite_cfg: SqliteConfig, global_repos: Vec<GlobalBackendAddr>) -> Self {
        Self {
            private_sqlite_cfg,
            global_repos,
        }
    }
}

pub struct ScopedMetaData(pub BackendAddr, pub MetaData);
pub struct ScopedId(pub BackendAddr, pub String);

impl fmt::Display for ScopedId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.0, self.1)
    }
}

type LocalGlobalComb = (GlobalBackendAddr, SqliteBackend);
type RemoteGlobalComb = (GlobalBackendAddr, GlobalBackend);

pub struct StackedBackend {
    cfg: StackedBackendConfig,
    pub private_be: SqliteBackend,
    pub local_global_be: Option<LocalGlobalComb>,
    pub reachable_remote_be: Vec<RemoteGlobalComb>,
}

impl StackedBackend {
    pub fn new(cfg: StackedBackendConfig) -> BackendResult<Self> {
        let (local_global_be, reachable_remote_be) =
            StackedBackend::resolve_all_global_idiomatic(&cfg)?;
        let private_be = SqliteBackend::new(cfg.private_sqlite_cfg.clone())?;
        Ok(Self {
            cfg,
            private_be,
            local_global_be,
            reachable_remote_be,
        })
    }

    fn resolve_all_global_idiomatic(
        cfg: &StackedBackendConfig,
    ) -> BackendResult<(Option<LocalGlobalComb>, Vec<RemoteGlobalComb>)> {
        let (sqlite, remotes) = cfg.global_repos.iter().fold(
            (None, Vec::new()),
            |(mut sqlite_acc, mut remotes_acc), g| {
                match g.resolve_to_backend() {
                    Ok(be) => match be {
                        GlobalBackend::Sqlite(bac) => {
                            if sqlite_acc.is_none() {
                                sqlite_acc = Some((g.clone(), bac));
                            }
                        }
                        GlobalBackend::Remote(bac) => {
                            if bac.reachable() {
                                remotes_acc.push((g.clone(), GlobalBackend::Remote(bac)));
                            }
                        }
                    },
                    Err(e) => {
                        log::warn!("Skipping broken backend: {e}");
                    }
                }
                (sqlite_acc, remotes_acc)
            },
        );

        Ok((sqlite, remotes))
    }

    pub fn get_backend_by_addr<'a>(
        &'a self,
        target_backend: Option<&BackendAddr>,
    ) -> BackendResult<BackendRef<'a>> {
        match target_backend {
            // 无指定，或指定为私有层
            None | Some(BackendAddr::Private { .. }) => Ok(&self.private_be as BackendRef<'a>),

            // 指定为 Global，且内部地址为 Sqlite 变体
            Some(BackendAddr::Global {
                addr: GlobalBackendAddr::Sqlite { .. },
            }) => self
                .local_global_be
                .as_ref()
                .map(|be| &be.1 as BackendRef<'a>)
                .ok_or_else(|| BackendError::Unsupported {
                    message: "Local global sqlite backend is not configured".to_string(),
                }),

            // 指定为 Global，且内部地址为 Remote 变体
            Some(BackendAddr::Global {
                addr: GlobalBackendAddr::Remote { server_url },
            }) => {
                // 在预筛过的远程可达列表中，根据 server_url 匹配对应的已解析后端
                let found_backend = self
                    .reachable_remote_be
                    .iter()
                    .find(|(global_addr, _)| {
                        if let GlobalBackendAddr::Remote {
                            server_url: cur_url,
                        } = global_addr
                        {
                            cur_url == server_url
                        } else {
                            false
                        }
                    })
                    .map(|(_, be)| be);

                match found_backend {
                    Some(GlobalBackend::Remote(_remote_be)) => Err(BackendError::Unsupported {
                        message: "Remote backend is currently unimplemented".to_string(),
                    }),
                    // 因为这里已经是 Remote 变体的分支，按理说搜出来的后端不应该是 Sqlite
                    Some(GlobalBackend::Sqlite(_)) => Err(BackendError::BackendNotFound),
                    None => Err(BackendError::BackendNotFound),
                }
            }
        }
    }

    /// Mark MetaData status to ensure disk data and backend metadata consistency
    pub fn mark_status(
        &self,
        id: &str,
        busy_status: DataSetBusyStatus,
        target_backend: Option<&BackendAddr>,
    ) -> BackendResult<()> {
        let backend = self.get_backend_by_addr(target_backend)?;
        backend.mark_status(id, busy_status)?;
        Ok(())
    }

    /// Retrieves the corresponding metadata by the dataset ID.
    pub fn get_metadata(
        &self,
        id: &str,
        target_backend: Option<&BackendAddr>,
    ) -> BackendResult<Vec<ScopedMetaData>> {
        if let Some(backend_addr) = target_backend {
            let backend = self.get_backend_by_addr(target_backend)?;
            let mut all_meta = Vec::new();
            let meta = backend.get_metadata(id)?;
            all_meta.push(ScopedMetaData(backend_addr.clone(), meta));
            return Ok(all_meta);
        }
        let mut all_meta = Vec::new();
        match self.private_be.get_metadata(id) {
            Ok(meta) => {
                let addr = BackendAddr::Private {
                    username: meta.owner.clone(),
                };
                all_meta.push(ScopedMetaData(addr, meta));
            }
            Err(BackendError::DatasetNotFound { .. }) => {
                // 私有后端无此数据，继续进入下方的公有层查询
            }
            // 剩下的所有其他错误（如 StorageError, PermissionDenied 等）直接向上抛出
            Err(e) => return Err(e),
        }

        if let Some((global_addr, sqlite_be)) = &self.local_global_be {
            match sqlite_be.get_metadata(id) {
                Ok(meta) => {
                    all_meta.push(ScopedMetaData(
                        BackendAddr::Global {
                            addr: global_addr.clone(),
                        },
                        meta,
                    ));
                }
                Err(BackendError::DatasetNotFound { .. }) => {}
                Err(e) => return Err(e),
            }
        }

        // 遍历所有全局公有后端
        for (global_addr, backend) in &self.reachable_remote_be {
            let res = match backend {
                GlobalBackend::Sqlite(sqlite_be) => sqlite_be.get_metadata(id),
                GlobalBackend::Remote(_) => continue, // TODO: 暂不支持远程读取
            };

            match res {
                Ok(meta) => {
                    all_meta.push(ScopedMetaData(
                        BackendAddr::Global {
                            addr: global_addr.clone(),
                        },
                        meta,
                    ));
                }
                Err(BackendError::DatasetNotFound { .. }) => continue,
                Err(e) => return Err(e), // 返回真实的系统错误（如磁盘损坏等）
            }
        }

        if !all_meta.is_empty() {
            return Ok(all_meta);
        }
        log::error!("Dataset metadata not found in any stacked backend: {}", id);
        Err(BackendError::DatasetNotFound { id: id.to_string() })
    }

    /// Saves or updates the dataset metadata.
    /// saves to private backend if target_backend set to None
    pub fn save_metadata(
        &self,
        metadata: &MetaData,
        target_backend: Option<&BackendAddr>,
    ) -> BackendResult<()> {
        let backend_handel = self.get_backend_by_addr(target_backend)?;
        backend_handel.save_metadata(metadata)
    }

    /// Checks if any datasets ON THIS SERVER depend on the specified `target_id`.
    ///
    /// Returns a list of dataset IDs that reference the target.
    /// Since this tool manages metadata only and doesn't support massive real data transfering,
    /// any dataset should not depend on datasets from remote server.
    /// So every server has it's own DAG instance and manages datasets on this server only.
    /// To build a dataset that
    /// depends on remote datasets, mirror them to local server first and build your new dataset
    /// based on local mirrors.
    pub fn check_is_referenced(&self, target_id: &str) -> Result<Vec<ScopedId>, MetaDataError> {
        let mut references = Vec::new();

        // 查询私有后端是否有依赖该 target_id 的派生数据集
        let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())
            .map_err(|e| e.to_metadata_error())?;
        let username = get_username()?;
        if let Ok(refs) = private_be.check_is_referenced(target_id) {
            for id in refs {
                references.push(ScopedId(
                    BackendAddr::Private {
                        username: username.clone(),
                    },
                    id,
                ));
            }
        }
        // 本地全局公有后端
        if let Some((global_addr, sqlite_be)) = &self.local_global_be
            && let Ok(refs) = sqlite_be.check_is_referenced(target_id)
        {
            for id in refs {
                references.push(ScopedId(
                    BackendAddr::Global {
                        addr: global_addr.clone(),
                    },
                    id,
                ));
            }
        }
        // 依次查询当前服务器配置的所有公有后端
        for (global_addr, backend) in &self.reachable_remote_be {
            if let GlobalBackend::Sqlite(sqlite_be) = backend
                && let Ok(refs) = sqlite_be.check_is_referenced(target_id)
            {
                for id in refs {
                    references.push(ScopedId(
                        BackendAddr::Global {
                            addr: global_addr.clone(),
                        },
                        id,
                    ));
                }
            }
        }

        Ok(references)
    }

    /// Lists all available dataset metadata from the backend.
    pub fn list_all_metadata(&self) -> BackendResult<Vec<ScopedMetaData>> {
        let mut unique_metas = Vec::new();

        let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())?;
        if let Ok(metas) = private_be.list_all_metadata() {
            for meta in metas {
                let owner = meta.owner.clone();
                unique_metas.push(ScopedMetaData(
                    BackendAddr::Private { username: owner },
                    meta,
                ));
            }
        }

        // 本地全局公有后端
        if let Some((global_addr, sqlite_be)) = &self.local_global_be
            && let Ok(metas) = sqlite_be.list_all_metadata()
        {
            for meta in metas {
                unique_metas.push(ScopedMetaData(
                    BackendAddr::Global {
                        addr: global_addr.clone(),
                    },
                    meta,
                ));
            }
        }

        for (global_addr, backend) in &self.reachable_remote_be {
            if let GlobalBackend::Sqlite(sqlite_be) = backend
                && let Ok(metas) = sqlite_be.list_all_metadata()
            {
                for meta in metas {
                    unique_metas.push(ScopedMetaData(
                        BackendAddr::Global {
                            addr: global_addr.clone(),
                        },
                        meta,
                    ));
                }
            }
        }

        Ok(unique_metas)
    }

    /// Deletes the metadata associated with the specified dataset ID.
    /// note: this mucntion only deletes the metadata and detach this dataset from backend regisitration,
    /// real data on disk will be safe
    pub fn delete_metadata(&self, id: &str) -> BackendResult<()> {
        // 从私有可写层擦除
        let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())?;
        match private_be.delete_metadata(id) {
            Ok(_) => return Ok(()),

            Err(BackendError::DatasetNotFound { .. }) => {
                // 当前私有空间不存在该数据，放行至全局空间探测
            }
            Err(e) => return Err(e),
        }

        // 本地全局公有后端
        if let Some((_, sqlite_be)) = &self.local_global_be {
            match sqlite_be.delete_metadata(id) {
                Ok(_) => return Ok(()),
                Err(BackendError::DatasetNotFound { .. }) => {}
                Err(e) => return Err(e), // 如果没有写权限，这里抛出 PermissionDenied
            }
        }
        // 从所有global后端中找到本地global sqlite后端并尝试删除
        for (_, backend) in &self.reachable_remote_be {
            let GlobalBackend::Sqlite(backend) = backend else {
                continue;
            };
            let res = backend.delete_metadata(id);
            match res {
                Ok(_) => return Ok(()),
                Err(BackendError::DatasetNotFound { .. }) => continue,
                // 如果普通用户尝试删除全局数据，这里会直接将 SQLite Driver 抛出的 io::ErrorKind::PermissionDenied 透传回前端
                Err(e) => return Err(e),
            }
        }

        // 若经历整个 Stack 都未能命中删除id
        log::error!(
            "Cannot delete dataset: id '{}' not found in any level of StackBackend",
            id
        );
        Err(BackendError::DatasetNotFound { id: id.to_string() })
    }

    /// 专为本地 DAG 构建提供：在当前服务器的本地后端中定位数据集归属。
    /// 优先级：私有库 (private_be) > 本地全局库 (local_global_be)。
    /// 找到后立即短路返回对应单体数据库的 BackendRef 句柄
    pub fn resolve_local_backend<'a>(&'a self, id: &str) -> BackendResult<BackendRef<'a>> {
        if self.private_be.get_metadata(id).is_ok() {
            return Ok(&self.private_be as BackendRef<'a>);
        }

        if let Some((_, sqlite_be)) = &self.local_global_be
            && sqlite_be.get_metadata(id).is_ok()
        {
            return Ok(sqlite_be as BackendRef<'a>);
        }
        log::error!("Dataset not found");
        Err(BackendError::DatasetNotFound { id: id.to_string() })
    }
}
