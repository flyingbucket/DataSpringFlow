use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use colored::Colorize;
use dialoguer::Confirm;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::net::IpAddr;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use directories::ProjectDirs;

use crate::utils::*;
use dsf_core::backend::{
    BackendAddr, GlobalBackendAddr, SqliteBackend, SqliteConfig, StackedBackendConfig,
    build_backend_auto,
};
use dsf_core::config::{AppConfig, InstallMode};
use dsf_core::service::{DSFService, RegisterOptions};
use dsf_core::utils::*;
use dsf_web::run_server;

#[derive(Parser, Debug)]
#[command(
    name = "dsf",
    about = "DataSpringFlow: dataset assets managment tool featuring DAG linage and blake hash verification.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialization and installation
    Init {
        /// Global installation (/etc/dataspringflow + /var/lib/dataspringflow)
        #[arg(long, default_value_t = false)]
        global: bool,
    },

    /// Show current application environment and backend database configurations
    ShowConfig,

    /// Grant DSFadmin privileges to a user
    Grant {
        /// The username to grant privileges. If omitted, uses current user.
        username: Option<String>,
    },

    /// Query dataset status
    Query {
        /// Dataset ID in format: name@tag
        id: String,
        /// Verification Level
        #[arg(short, long, value_enum, default_value_t = VerifyLevel::SelfOnly)]
        level: VerifyLevel,
        /// Show differences on verification failure
        #[arg(long, default_value_t = false)]
        show_diff: bool,
        /// Query specifically against the global registry instead of private
        #[arg(long, default_value_t = false)]
        global: bool,
    },

    /// Register new dataset
    Register {
        #[arg(long)]
        name: String,
        #[arg(long)]
        tag: String,
        #[arg(long)]
        path: PathBuf,
        #[arg(long)]
        script_path: PathBuf,
        #[arg(long)]
        owner_nickname: Option<String>,
        #[arg(long)]
        description_path: Option<PathBuf>,
        #[arg(long = "deps")]
        dependencies: Vec<String>,
        /// Non-interactive mode: force heal when broken dependencies are detected
        #[arg(long, default_value_t = false)]
        force_heal: bool,
        /// Register dataset directly to the global public registry
        #[arg(long, default_value_t = false)]
        global: bool,
    },

    /// Recalculate and update dataset hashes
    Update {
        id: String,
        /// Target the global public registry
        #[arg(long, default_value_t = false)]
        global: bool,
    },

    /// Delete a dataset entry
    Delete {
        id: String,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long, default_value_t = false)]
        yes: bool,
        /// Target the global public registry
        #[arg(long, default_value_t = false)]
        global: bool,
    },

    /// Start the DataSpringFlow Web UI server
    Serve {
        /// Host address to bind
        #[arg(long, default_value = "0.0.0.0")]
        host: IpAddr,

        /// Port to listen on
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum, Debug)]
pub enum VerifyLevel {
    MetaOnly,
    SelfOnly,
    Deep,
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Init { global } => handle_init(global),
        Commands::ShowConfig => handle_show_config(),
        Commands::Query {
            id,
            level,
            show_diff,
            global,
        } => handle_query(&id, level, show_diff, global),
        Commands::Grant { username } => handle_grant(username),
        Commands::Register {
            name,
            tag,
            path,
            script_path,
            owner_nickname,
            description_path,
            dependencies,
            force_heal,
            global,
        } => {
            let opts = RegisterOptions {
                name,
                tag,
                path,
                script_path,
                owner_nickname,
                description_path,
                dependencies,
                force_heal,
            };
            handle_register(opts, global)
        }
        Commands::Update { id, global } => handle_update(&id, global),
        Commands::Delete {
            id,
            force,
            yes,
            global,
        } => handle_delete(&id, force, yes, global),

        Commands::Serve { host, port } => {
            handle_serve(host, port).await?;
            Ok(())
        }
    }
}

/// Resolves the CLI `--global` flag into the StackedBackend Routing Address
fn get_target_addr(global: bool) -> Option<BackendAddr> {
    if global {
        Some(BackendAddr::Global {
            addr: GlobalBackendAddr::Sqlite {
                config_path: PathBuf::from("/etc/dataspringflow/config.yaml"),
            },
        })
    } else {
        None
    }
}

fn handle_init(global_flag: bool) -> Result<()> {
    let mode = if global_flag {
        InstallMode::Global
    } else {
        InstallMode::User
    };

    match mode {
        InstallMode::Global => init_global()?,
        InstallMode::User => init_user()?,
    }

    println!(
        "{}",
        "\nInitialization finished successfully.".green().bold()
    );
    Ok(())
}

fn init_global() -> Result<()> {
    if !is_root() {
        bail!("{}", "Error: 'init --global' requires root privileges. Please run with 'sudo dsf init --global'".red().bold());
    }

    println!(
        "{}",
        "\n[Global Setup] Initializing system directories and database...".cyan()
    );

    let config_path = PathBuf::from("/etc/dataspringflow/config.yaml");
    let data_path = PathBuf::from("/var/lib/dataspringflow");
    let db_file_path = data_path.join("dsf.db");

    fs::create_dir_all(&data_path)?;
    fs::create_dir_all(data_path.join("merkle"))?;
    fs::create_dir_all(data_path.join("descriptions"))?;
    fs::create_dir_all("/etc/dataspringflow")?;

    // creating group DSFadmin
    println!("{}", "Creating admin gropu DSFadmin".cyan());
    let _ = Command::new("groupadd").arg("-f").arg("DSFadmin").status();

    // get and write config file
    let sqlite_cfg = SqliteConfig::new(db_file_path.clone());
    let final_config = AppConfig {
        mode: InstallMode::Global,
        config_path: Some(config_path.clone()),
        backend: StackedBackendConfig {
            private_sqlite_cfg: sqlite_cfg.clone(),
            global_repos: vec![],
        },
    };
    let config_yaml = serde_yaml::to_string(&final_config)?;
    fs::write(&config_path, &config_yaml)?;

    // config file 644
    fs::set_permissions(&config_path, fs::Permissions::from_mode(0o644))?;

    println!("{}", "Migrating global database schemas...".cyan());
    SqliteBackend::new(sqlite_cfg)?;

    let fix_perm_script = format!(
        r#"
chown -R root:DSFadmin /etc/dataspringflow /var/lib/dataspringflow
chmod 755 /etc/dataspringflow
chmod 2775 /var/lib/dataspringflow
find /var/lib/dataspringflow -type d -exec chmod 2775 {{}} \;
chmod 664 "{db_file}" "{db_file}"* 2>/dev/null || true
"#,
        db_file = db_file_path.display()
    );
    Command::new("sh")
        .arg("-c")
        .arg(&fix_perm_script)
        .status()?;

    println!(
        "{}",
        "Global environment and database initialized successfully!"
            .green()
            .bold()
    );
    println!(
        "{}",
        "Next step: Please run 'sudo dsf grant <username>' to authorize developers.".yellow()
    );

    Ok(())
}

fn init_user() -> Result<()> {
    println!("{}", "\n[User Setup] Configuring private sandbox...".cyan());

    let proj_dirs = ProjectDirs::from("io", "flyingbucket", "dataspringflow").context(
        "Failed to determine standard user directories (XDG_CONFIG_HOME / XDG_DATA_HOME)",
    )?;

    let config_dir = proj_dirs.config_dir();
    let data_dir = proj_dirs.data_dir();
    let config_path = config_dir.join("config.yaml");

    fs::create_dir_all(config_dir).context("Failed to create user config dir")?;
    fs::create_dir_all(data_dir.join("merkle")).context("Failed to create user data dir")?;
    fs::create_dir_all(data_dir.join("descriptions")).context("Failed to create user data dir")?;

    let mut sqlite_cfg = SqliteConfig::new(data_dir.join("dsf.db"));
    sqlite_cfg.wal = true;

    // Detect if global is installed to automatically mount it in StackedBackend
    let mut global_repos = vec![];
    let is_global = is_global_installed("DSFadmin");
    if is_global {
        global_repos.push(GlobalBackendAddr::Sqlite {
            config_path: PathBuf::from("/etc/dataspringflow/config.yaml"),
        });
    }

    let stacked_cfg = StackedBackendConfig {
        private_sqlite_cfg: sqlite_cfg.clone(),
        global_repos,
    };

    let final_config = AppConfig {
        mode: InstallMode::User,
        config_path: Some(config_path.clone()),
        backend: stacked_cfg,
    };

    let config_yaml = serde_yaml::to_string(&final_config)?;
    fs::write(&config_path, config_yaml).context("Failed to write user config file")?;

    println!("{}", "Initializing user database backend...".cyan());
    SqliteBackend::new(sqlite_cfg).context("Failed to initialize user database")?;

    if is_global {
        println!(
            "{}",
            "Global registry detected. Applying strict ACLs to allow DSFadmin audit traversal..."
                .cyan()
        );

        let home_dir = proj_dirs
            .data_dir()
            .ancestors()
            .nth(3)
            .unwrap_or(Path::new("/"));
        if let Err(e) = ensure_acl_permissions(home_dir, data_dir, "DSFadmin") {
            println!(
                "Warning: Failed to set ACL permissions: {}. DSFadmin will not be able to audit your private datasets.",
                e
            );
        } else {
            println!("{}", "ACL permissions applied successfully.".cyan());
        }
    }

    Ok(())
}

fn is_global_installed(group_name: &str) -> bool {
    let file = match File::open("/etc/group") {
        Ok(f) => f,
        Err(_) => return false,
    };

    let reader = BufReader::new(file);
    let group_exists = reader.lines().any(|line| {
        if let Ok(l) = line {
            l.split(':').next() == Some(group_name)
        } else {
            false
        }
    });
    Path::new("/etc/dataspringflow/config.yaml").exists() && group_exists
}

fn ensure_acl_permissions(home: &Path, data_dir: &Path, group: &str) -> Result<()> {
    let run = |args: &[&str], path: &Path| -> Result<()> {
        let output = Command::new("setfacl").args(args).arg(path).output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("setfacl {} failed: {}", args.join(" "), stderr.trim())
        }
        Ok(())
    };

    // Safe traversal (+x only) for parent directories
    let local = home.join(".local");
    let share = local.join("share");

    if home.exists() {
        run(&["-m", &format!("g:{}:x", group)], home)?;
    }
    if local.exists() {
        run(&["-m", &format!("g:{}:x", group)], &local)?;
    }
    if share.exists() {
        run(&["-m", &format!("g:{}:x", group)], &share)?;
    }

    // Read/Execute (+rX) for the target data dir and its descendants
    let rule = format!("g:{}:rX", group);
    run(&["-d", "-m", &rule], data_dir)?;
    run(&["-R", "-m", &rule], data_dir)?;

    Ok(())
}

fn handle_grant(username: Option<String>) -> Result<()> {
    let final_name = username.unwrap_or_else(|| get_username().unwrap_or_default());

    if final_name.is_empty() {
        bail!("Could not determine username.");
    }
    let status = Command::new("sudo")
        .arg("usermod")
        .arg("-aG")
        .arg("DSFadmin")
        .arg(&final_name)
        .status()?;

    if !status.success() {
        bail!("Failed to grant privileges to user: {}", final_name);
    }

    println!("Successfully granted DSFadmin privileges to {}", final_name);
    Ok(())
}

fn handle_show_config() -> Result<()> {
    println!(
        "{}",
        "=== DataSpringFlow Current Configuration ==="
            .green()
            .bold()
    );

    let app_cfg = match AppConfig::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("{}", "Failed to load configuration:".red().bold());
            println!("{}", e.to_string().yellow());
            return Ok(());
        }
    };

    let mode_str = match app_cfg.mode {
        InstallMode::User => "User",
        InstallMode::Global => "Global",
    };
    println!("{:<25} {}", "Environment Mode:".bold(), mode_str);

    let path_str = app_cfg
        .config_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Not set / In-memory".to_string());
    println!("{:<25} {}", "Config File Path:".bold(), path_str);

    let sqlite_cfg = &app_cfg.backend.private_sqlite_cfg;
    println!(
        "{:<25} {}",
        "Private DB Path:".bold(),
        sqlite_cfg.db_path.display().to_string().cyan()
    );

    if !app_cfg.backend.global_repos.is_empty() {
        println!(
            "{:<25} {:?}",
            "Mounted Global Repos:".bold(),
            app_cfg.backend.global_repos
        );
    } else {
        println!("{:<25} {}", "Mounted Global Repos:".bold(), "None".dimmed());
    }

    println!(
        "{}",
        "===================================================="
            .green()
            .bold()
    );
    Ok(())
}

fn handle_query(id: &str, level: VerifyLevel, show_diff: bool, global: bool) -> Result<()> {
    validate_dataset_id(id)?;
    let backend = build_backend_auto()?;
    let service = DSFService::new(backend);
    let target = get_target_addr(global);

    match level {
        VerifyLevel::MetaOnly => {
            let meta_results = service.query_meta(id, None);
            match meta_results {
                Ok(metas) => {
                    for scoped_meta in metas {
                        let (addr, m) = (scoped_meta.0, scoped_meta.1);
                        let scope_str = match addr {
                            BackendAddr::Private { username } => {
                                format!("Private Sandbox ({})", username).cyan()
                            }
                            BackendAddr::Global { .. } => "Global Registry".green(),
                        };
                        println!("{} [{}]", "Dataset exists".green(), scope_str);
                        println!("id: {}", m.id());
                        println!("path: {}", m.path.display());
                        println!("hash: {}", m.hash);
                        println!("deps: {:?}", m.dependencies);
                        println!("---");
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    println!("{}", "Dataset doesn't exist".red().bold());
                }
                Err(e) => return Err(e.into()),
            }
        }
        VerifyLevel::SelfOnly => {
            let res = service.verify_self(id, show_diff, target.as_ref())?;
            print_query(id, res.status, &res.dep_status);
        }
        VerifyLevel::Deep => {
            let res = service.verify_deep(id, show_diff, target.as_ref())?;
            print_query(id, res.status, &res.dep_status);
        }
    }
    Ok(())
}

fn handle_register(opts: RegisterOptions, global: bool) -> Result<()> {
    let service = DSFService::new(build_backend_auto()?);
    let target = get_target_addr(global);
    service.register(opts, target.as_ref())?;
    println!("{}", "Successfully registered dataset.".green());
    Ok(())
}

fn handle_update(id: &str, global: bool) -> Result<()> {
    let backend = build_backend_auto()?;
    let service = DSFService::new(backend);
    let target = get_target_addr(global);

    service.update_merkle(id, target.as_ref())?;

    let metas = service.query_meta(id, None)?;
    if let Some(meta) = metas.first() {
        println!(
            "{}",
            format!("updated dataset {}, new hash: \n{}", id, meta.1.hash,).green()
        );
    }
    Ok(())
}

fn handle_delete(id: &str, force: bool, yes: bool, global: bool) -> Result<()> {
    let service = DSFService::new(build_backend_auto()?);
    let target = get_target_addr(global);

    if !yes {
        let metas = service.query_meta(id, None)?;
        if let Some(meta_to_delete) = metas.first() {
            let m = &meta_to_delete.1;
            let scope_str = if global {
                "global registry"
            } else {
                "private sandbox"
            };

            let ok = Confirm::new()
                .with_prompt(format!(
                    "id: {}\nPath: {:?}\nAre you sure you want to delete this dataset from the {}?\nnote: deletes metadata only, actual data on disk will be safe.",
                    id, m.path, scope_str
                ))
                .default(false)
                .interact()?;
            if !ok {
                bail!("Deletion cancelled by user.");
            }
        }
    }
    service.delete_metadata(id, force, target.as_ref())?;
    println!(
        "{}",
        format!("Successfully deleted metadata for {id}").green()
    );
    Ok(())
}

async fn handle_serve(host: IpAddr, port: u16) -> anyhow::Result<()> {
    let backend = build_backend_auto()?;
    let service = DSFService::new(backend);

    // 调用之前定义的 run_server
    run_server(service, host, port).await?;

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use tempfile::tempdir;

    #[test]
    fn parse_query_default_level() {
        let cli = Cli::parse_from(["dsf", "query", "mnist@v1"]);
        match cli.command {
            Commands::Query {
                id,
                level,
                show_diff,
                global,
            } => {
                assert_eq!(id, "mnist@v1");
                assert_eq!(level, VerifyLevel::SelfOnly);
                assert!(!show_diff);
                assert!(!global);
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn parse_init_flags() {
        let cli = Cli::parse_from(["dsf", "init", "--global"]);
        match cli.command {
            Commands::Init { global } => {
                assert!(global);
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn validate_dataset_id_accepts_normal_form() {
        assert!(validate_dataset_id("name@tag").is_ok());
    }

    #[test]
    fn ensure_exists_fails_for_missing_path() {
        let dir = tempdir().expect("create temp dir");
        let missing = dir.path().join("not_found.txt");
        let err = ensure_exists(&missing, "--path").unwrap_err().to_string();
        assert!(err.contains("--path"));
        assert!(err.contains("doesn't exist"));
    }
}
