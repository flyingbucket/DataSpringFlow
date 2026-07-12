use crate::backend::{DatasetBackend, DynBackend, RemoteBackend, SqliteBackend, SqliteConfig};
use crate::config::AppConfig;
use crate::core::{MetaData, MetaDataError};
use crate::utils::get_username;
// use crate::backend::DynBackend;

use serde::{Deserialize, Serialize};

use std::fs;
use std::io::{self};
use std::path::PathBuf;

pub enum GlobalBackend {
    Sqlite(SqliteBackend),
    Remote(RemoteBackend),
}

pub enum BackendInstence {
    Global(GlobalBackend),
    Private(SqliteBackend),
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

impl GlobalBackendAddr {
    pub fn resolve_to_backend(&self) -> io::Result<GlobalBackend> {
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

                println!(
                    "Successfully resolved global backend with DB: {}",
                    true_sqlite_cfg.db_path.display()
                );
                Ok(GlobalBackend::Sqlite(SqliteBackend::new(true_sqlite_cfg)?))
            }
            GlobalBackendAddr::Remote { server_url } => {
                let _ = server_url;
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Remote backend feature is currently unimplemented. Stay tuned!",
                ))
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackedBackendConfig {
    pub(crate) private_sqlite_cfg: SqliteConfig,
    pub(crate) global_repos: Vec<GlobalBackendAddr>,
}

pub struct ScopedMetaData(pub BackendAddr, pub MetaData);
pub struct ScopedId(pub BackendAddr, pub String);

pub struct StackedBackend {
    cfg: StackedBackendConfig,
    reachable_global_be: Vec<(GlobalBackendAddr, GlobalBackend)>,
}

impl StackedBackend {
    pub fn new(cfg: StackedBackendConfig) -> io::Result<Self> {
        let reachable_global_be = StackedBackend::resolve_all_global_idiomatic(&cfg)?;
        Ok(Self {
            cfg,
            reachable_global_be,
        })
    }

    fn resolve_all_global_idiomatic(
        cfg: &StackedBackendConfig,
    ) -> io::Result<Vec<(GlobalBackendAddr, GlobalBackend)>> {
        let all_global = cfg
            .global_repos
            .iter()
            .filter_map(|g| match g.resolve_to_backend() {
                Ok(be) => match be {
                    GlobalBackend::Remote(bac) => {
                        let reachable = bac.reachable();
                        if reachable {
                            Some((g.clone(), GlobalBackend::Remote(bac)))
                        } else {
                            None
                        }
                    }
                    GlobalBackend::Sqlite(bac) => Some((g.clone(), GlobalBackend::Sqlite(bac))),
                },
                Err(e) => {
                    eprintln!("Warning: Skipping broken backend: {e}");
                    None
                }
            })
            .collect();

        Ok(all_global)
    }

    pub fn get_backend_by_addr(
        &self,
        target_backend: Option<&BackendAddr>,
    ) -> io::Result<DynBackend> {
        match target_backend {
            None | Some(BackendAddr::Private { .. }) => {
                let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())?;
                Ok(Box::new(private_be))
            }

            Some(BackendAddr::Global { addr }) => {
                // 在预筛过的可达列表里查找对应的已解析后端
                let found_backend = self
                    .reachable_global_be
                    .iter()
                    .find(|(global_addr, _)| global_addr == addr)
                    .map(|(_, be)| be);

                match found_backend {
                    Some(GlobalBackend::Sqlite(_sqlite_be)) => {
                        // 本地全局的 Sqlite，重新 new 一个物理后端实例以获得所有权
                        let backend = addr.resolve_to_backend()?;
                        if let GlobalBackend::Sqlite(sqlite_be) = backend {
                            Ok(Box::new(sqlite_be))
                        } else {
                            unreachable!();
                        }
                    }
                    Some(GlobalBackend::Remote(_remote_be)) => {
                        // 未来实现 Remote 时，如果 Remote 实现了 Clone，可以直接 Box::new(remote_be.clone())
                        Err(io::Error::new(
                            io::ErrorKind::Unsupported,
                            "Remote backend is currently unimplemented",
                        ))
                    }
                    None => Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!(
                            "Target global backend is either unreachable or not found in config"
                        ),
                    )),
                }
            }
        }
    }
    /// Retrieves the corresponding metadata by the dataset ID.
    pub fn get_metadata(&self, id: &str) -> io::Result<Vec<ScopedMetaData>> {
        let mut all_meta = Vec::new();
        let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())?;
        match private_be.get_metadata(id) {
            Ok(meta) => {
                let addr = BackendAddr::Private {
                    username: meta.owner.clone(),
                };
                all_meta.push(ScopedMetaData(addr, meta));
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // 私有后端无此数据，继续进入下方的公有层查询
            }
            Err(e) => return Err(e),
        }

        // 遍历所有全局公有后端
        for (global_addr, backend) in &self.reachable_global_be {
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
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue, // 没找到则继续找下一个公有节点
                Err(e) => return Err(e), // 返回真实的系统错误（如磁盘损坏等）
            }
        }

        if !all_meta.is_empty() {
            return Ok(all_meta);
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Dataset metadata not found in any stacked backend: {}", id),
        ))
    }

    /// Saves or updates the dataset metadata.
    /// saves to private backend if target_backend set to None
    pub fn save_metadata(
        &self,
        metadata: &MetaData,
        target_backend: Option<&BackendAddr>,
    ) -> io::Result<()> {
        let backend_handel = self.get_backend_by_addr(target_backend)?;
        backend_handel.as_ref().save_metadata(metadata)
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
        let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())?;
        let username = get_username()?;
        // let username = get_username().map_err(|e| {
        //     io::Error::new(
        //         io::ErrorKind::Other,
        //         format!("OS username unavailable: {:?}", e),
        //     )
        // })?;
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

        // 依次查询当前服务器配置的所有公有后端
        for (global_addr, backend) in &self.reachable_global_be {
            if let GlobalBackend::Sqlite(sqlite_be) = backend {
                if let Ok(refs) = sqlite_be.check_is_referenced(target_id) {
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
        }

        Ok(references)
    }

    /// Lists all available dataset metadata from the backend.
    pub fn list_all_metadata(&self) -> io::Result<Vec<ScopedMetaData>> {
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

        for (global_addr, backend) in &self.reachable_global_be {
            if let GlobalBackend::Sqlite(sqlite_be) = backend {
                if let Ok(metas) = sqlite_be.list_all_metadata() {
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
        }

        Ok(unique_metas)
    }

    /// Deletes the metadata associated with the specified dataset ID.
    /// note: this mucntion only deletes the metadata and detach this dataset from backend regisitration,
    /// real data on disk will be safe
    pub fn delete_metadata(&self, id: &str) -> io::Result<()> {
        // 从私有可写层擦除
        let private_be = SqliteBackend::new(self.cfg.private_sqlite_cfg.clone())?;
        match private_be.delete_metadata(id) {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // 当前私有空间不存在该数据，放行至全局空间探测
            }
            Err(e) => return Err(e),
        }

        // 从所有global后端中找到本地global sqlite后端并尝试删除
        for (_, backend) in &self.reachable_global_be {
            let GlobalBackend::Sqlite(backend) = backend else {
                continue;
            };
            let res = backend.delete_metadata(id);
            match res {
                Ok(_) => return Ok(()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                // 如果普通用户尝试删除全局数据，这里会直接将 SQLite Driver 抛出的 io::ErrorKind::PermissionDenied 透传回前端
                Err(e) => return Err(e),
            }
        }

        // 若经历整个 Stack 都未能命中删除id
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Cannot delete dataset: id '{}' not found in any level of StackBackend",
                id
            ),
        ))
    }
}
