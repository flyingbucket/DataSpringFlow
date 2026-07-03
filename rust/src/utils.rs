use crate::merkle::HashRes;
use directories::ProjectDirs;
use std::env;
use std::path::PathBuf;

pub fn hashres_to_hex(bytes: HashRes) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);

    for &b in &bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }

    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum InstallMode {
    User,
    Global,
    Custom, // 环境变量指定或 CLI 参数指定
}

#[derive(Debug, Clone)]
pub struct AppEnv {
    pub mode: InstallMode,
    pub config_path: PathBuf,
    pub db_path: PathBuf,
}

impl AppEnv {
    /// 动态探测当前的运行环境与安装模式
    pub fn resolve() -> Self {
        // P0: 检查是否存在环境变量显式覆盖
        let env_config = env::var_os("DSF_CONFIG_PATH").map(PathBuf::from);
        let env_db = env::var_os("DSF_DB_PATH").map(PathBuf::from);

        if let (Some(config_path), Some(db_path)) = (env_config.clone(), env_db.clone()) {
            return Self {
                mode: InstallMode::Custom,
                config_path,
                db_path,
            };
        }

        // P1: 检查用户目录下是否已存在配置，或当前是以普通用户身份运行
        if let Some(proj_dirs) = ProjectDirs::from("org", "dataspringflow", "dsf") {
            let user_config = proj_dirs.config_dir().join("config.yaml");
            let user_db = proj_dirs.data_dir().join("dsf.db");

            // 如果用户配置文件存在，或者当前非 root 用户，默认使用 User 模式
            if user_config.exists() || !Self::is_root() {
                return Self {
                    mode: InstallMode::User,
                    config_path: env_config.unwrap_or(user_config),
                    db_path: env_db.unwrap_or(user_db),
                };
            }
        }

        // P2: 全局系统级默认路径 (Sudo / System-wide)
        Self {
            mode: InstallMode::Global,
            config_path: env_config
                .unwrap_or_else(|| PathBuf::from("/etc/dataspringflow/config.yaml")),
            db_path: env_db.unwrap_or_else(|| PathBuf::from("/var/lib/dataspringflow/dsf.db")),
        }
    }

    /// 简单的 root 权限探测 (Linux/macOS)
    #[cfg(unix)]
    fn is_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(not(unix))]
    fn is_root() -> bool {
        false
    }

    /// 显式返回全局安装默认路径（不做环境探测）
    pub fn global_default() -> Self {
        Self {
            mode: InstallMode::Global,
            config_path: PathBuf::from("/etc/dataspringflow/config.yaml"),
            db_path: PathBuf::from("/var/lib/dataspringflow/dsf.db"),
        }
    }
}
