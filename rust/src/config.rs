use crate::backend::StackedBackendConfig;
use colored::Colorize;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMode {
    #[default]
    User,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(skip)]
    pub mode: InstallMode,
    #[serde(skip)]
    pub config_path: Option<PathBuf>,
    pub backend: StackedBackendConfig,
}

impl AppConfig {
    pub(crate) fn resolve_path() -> io::Result<(InstallMode, PathBuf)> {
        // P1: XDG 标准用户目录探测
        let proj_dirs =
            ProjectDirs::from("io", "flyingbucket", "dataspringflow").ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "Failed to resolve user home directory".red().to_string(),
                )
            })?;

        let user_cfg_path = proj_dirs.config_dir().join("config.yaml");
        let global_cfg_path = PathBuf::from("/etc/dataspringflow/config.yaml");

        if user_cfg_path.exists() {
            Ok((InstallMode::User, user_cfg_path))
        } else if global_cfg_path.exists() {
            Ok((InstallMode::Global, global_cfg_path))
        } else {
            let err_msg = format!(
                "{}\n{} Run `{}` first.",
                "Neither user installation nor global installation detected on this server.".red(),
                "Suggestion:".bright_blue().bold(),
                "dsf init".green().underline()
            );

            Err(io::Error::new(io::ErrorKind::NotFound, err_msg))
        }
    }
    pub(crate) fn load() -> io::Result<Self> {
        let (mode, config_path) = Self::resolve_path()?;

        let content = fs::read_to_string(&config_path).map_err(|e| {
            Error::other(format!(
                "Read config failed ({}): {e}",
                config_path.display()
            ))
        })?;

        let mut app_cfg = serde_yaml::from_str::<Self>(&content).map_err(|e| {
            let err_msg = format!(
                "{}\n({}):{e}",
                "Parse config yaml failed".red(),
                config_path.display()
            );
            Error::new(ErrorKind::InvalidData, err_msg)
        })?;

        app_cfg.mode = mode;
        app_cfg.config_path = Some(config_path);

        Ok(app_cfg)
    }

    pub(crate) fn get_local_global_path() -> io::Result<PathBuf> {
        let local_global = PathBuf::from("/etc/dataspringflow/config.yaml");
        if !local_global.exists() {
            let err_msg = format!(
                "{}: {}",
                "Local global sqlite config file not found".red().bold(),
                local_global.to_string_lossy()
            );
            return Err(io::Error::new(io::ErrorKind::NotFound, err_msg));
        }
        Ok(local_global)
    }
}
