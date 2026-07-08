use crate::backend::SqliteConfig;
use colored::Colorize;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::backtrace::Backtrace;
use std::env;
use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;
use strum::{Display, EnumIter};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallMode {
    #[default]
    User,
    Global,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, EnumIter, Display)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BackendConfig {
    Sqlite(SqliteConfig),
    // Yaml(YamlConfig), // future impl
    // Remote(RemoteConfig), //future impl
}

// 自动为 BackendConfig 赋予默认值（默认使用 SQLite）
impl Default for BackendConfig {
    fn default() -> Self {
        Self::Sqlite(SqliteConfig::default())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(skip)]
    pub mode: InstallMode,
    #[serde(skip)]
    pub config_path: Option<PathBuf>,

    // YAML 文件里真正关心的核心字段
    #[serde(default)]
    pub backend: BackendConfig,
}

impl AppConfig {
    pub(crate) fn resolve_path() -> (InstallMode, PathBuf) {
        // P0: 优先检查环境变量覆盖 (DSF_CONFIG_PATH)
        if let Some(path) = env::var_os("DSF_CONFIG_PATH").map(PathBuf::from) {
            return (InstallMode::Custom, path);
        }

        // P1: XDG 标准用户目录探测
        if let Some(proj_dirs) = ProjectDirs::from("org", "dataspringflow", "dsf") {
            let user_config = proj_dirs.config_dir().join("config.yaml");
            if user_config.exists() || !is_root() {
                return (InstallMode::User, user_config);
            }
        }

        // P2: 兜底全局目录
        (
            InstallMode::Global,
            PathBuf::from("/etc/dataspringflow/config.yaml"),
        )
    }

    pub(crate) fn load() -> io::Result<Self> {
        let (mode, config_path) = Self::resolve_path();
        println!("{}", "--- DEBUG: Configuration load triggered ---".red());
        println!("Backtrace:\n{}", Backtrace::capture());
        if !config_path.exists() {
            println!(
                "{}",
                "Warning: config path doesn't exist, using sqlite default config."
                    .yellow()
                    .bold()
            );
            return Ok(Self {
                mode,
                config_path: None,
                backend: BackendConfig::default(),
            });
        }

        let content = fs::read_to_string(&config_path).map_err(|e| {
            Error::other(format!(
                "Read config failed ({}): {e}",
                config_path.display()
            ))
        })?;

        let mut app_cfg = serde_yaml::from_str::<Self>(&content).map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("Parse config yaml failed ({}): {e}", config_path.display()),
            )
        })?;

        // 填入运行时上下文
        app_cfg.mode = mode;
        app_cfg.config_path = Some(config_path);

        Ok(app_cfg)
    }
}

#[cfg(unix)]
pub(crate) fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}

#[cfg(not(unix))]
pub(crate) fn is_root() -> bool {
    false
}
