use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use dialoguer::{Confirm, Input, Select};
use std::fs;
use std::path::PathBuf;
use strum::IntoEnumIterator;

use directories::ProjectDirs;

use crate::backend::{SqliteBackend, SqliteConfig, build_backend_auto};
use crate::config::{AppConfig, BackendConfig, InstallMode};
use crate::service::{DSFService, RegisterOptions};
use crate::utils::*;

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
    /// Interactive initialization and installation
    Init {
        /// Global installation (/etc/dataspringflow + /var/lib/dataspringflow)
        #[arg(long, default_value_t = false)]
        global: bool,
        ///  Non-interactive mode, initialize using default paths directly
        #[arg(long, default_value_t = false)]
        non_interactive: bool,
    },

    /// Show current application environment and backend database configurations
    ShowConfig,

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
        /// Non-interactive confirmation (skip prompts)
        #[arg(long, default_value_t = false)]
        yes: bool,
    },

    /// Recalculate and update dataset hashes
    Update { id: String },

    /// Delete a dataset entry
    Delete {
        id: String,
        #[arg(long, default_value_t = false)]
        force: bool,
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum, Debug)]
pub enum VerifyLevel {
    MetaOnly,
    SelfOnly,
    Deep,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Init {
            global,
            non_interactive,
        } => handle_init(global, non_interactive),
        Commands::ShowConfig => handle_show_config(),
        Commands::Query {
            id,
            level,
            show_diff,
        } => handle_query(&id, level, show_diff),
        Commands::Register {
            name,
            tag,
            path,
            script_path,
            owner_nickname,
            description_path,
            dependencies,
            force_heal,
            yes,
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
                yes,
            };
            handle_register(opts)
        }
        Commands::Update { id } => handle_update(&id),
        Commands::Delete { id, force, yes } => handle_delete(&id, force, yes),
    }
}

fn handle_init(global_flag: bool, non_inter: bool) -> Result<()> {
    // 1) 交互决定安装级别
    let mode = if global_flag {
        InstallMode::Global
    } else if non_inter {
        InstallMode::User
    } else {
        let items = vec![
            "User installation (~/.config/dataspringflow + ~/.local/share/dataspringflow)",
            "Global installation (/etc/dataspringflow + /var/lib/dataspringflow)",
        ];
        let idx = Select::new().items(&items).interact()?;
        if idx == 0 {
            InstallMode::User
        } else {
            InstallMode::Global
        }
    };

    let global = { mode == InstallMode::Global };

    // 2) 权限检查
    if global && !is_root() {
        bail!(
            "{}\n{}",
            "Error: Global installation requires root privileges."
                .red()
                .bold(),
            "Please use: sudo dsf init --global".yellow()
        );
    }

    let default_config: PathBuf;
    let default_data: PathBuf;

    if global {
        default_config = PathBuf::from("/etc/dataspringflow/config.yaml");
        default_data = PathBuf::from("/var/lib/dataspringflow/");
    } else {
        // XDG home dir
        if let Some(proj_dirs) = ProjectDirs::from("io", "flyingbucket", "dataspringflow") {
            default_config = proj_dirs.config_dir().join("config.yaml");
            default_data = proj_dirs.data_dir().to_path_buf();
        } else {
            // 没有家目录的环境特殊情况
            default_config = PathBuf::from("./config/config.yaml");
            default_data = PathBuf::from("./data");
            println!(
                "{}",
                "Warning: Failed to find OS standart project dir, using current working dir as a backup.\n 
                    Check your environment varible $HOME. If using a docker, set env $DSF_CONFIG_PATH and edit that file manully."
                    .yellow()
                    .bold()
            );
        }
    }

    // 4) 可交互修改路径
    let (config_path, data_path) = if non_inter {
        (default_config, default_data)
    } else {
        let config_path_str: String = Input::new()
            .with_prompt("Config file path")
            .default(default_config.display().to_string())
            .interact_text()?;
        let data_path_str: String = Input::new()
            .with_prompt("Path to metadata and merkle hash tree storage ")
            .default(default_data.display().to_string())
            .interact_text()?;
        (PathBuf::from(config_path_str), PathBuf::from(data_path_str))
    };

    let backend_choice = if non_inter {
        BackendConfig::Sqlite(SqliteConfig::default())
    } else {
        let variants: Vec<BackendConfig> = BackendConfig::iter().collect();
        let items: Vec<String> = variants.iter().map(|v| v.to_string()).collect();

        let idx = Select::new()
            .with_prompt("Select storage backend")
            .items(&items)
            .default(0)
            .interact()?;

        // 根据索引直接从变体列表中取回
        #[allow(unreachable_patterns)]
        match variants.get(idx) {
            Some(BackendConfig::Sqlite(_)) => BackendConfig::Sqlite(SqliteConfig::default()),
            Some(_) => bail!("Not implemented yet"),
            None => bail!("Invalid selection"),
        }
    };

    // 打印预览信息
    println!(
        "Installation mode: {}",
        if global {
            "global".cyan()
        } else {
            "user".cyan()
        }
    );
    println!(
        "Config file:       {}",
        config_path.display().to_string().cyan()
    );
    println!(
        "Data dir for metadata storage and merkle tree files:     {}",
        data_path.display().to_string().cyan()
    );

    if !non_inter {
        let ok = Confirm::new()
            .with_prompt("Confirm?")
            .default(true)
            .interact()?;
        if !ok {
            bail!("Installation and initialization terminated");
        }
    }

    if let Some(p) = config_path.parent() {
        fs::create_dir_all(p)
            .with_context(|| format!("Failed making config dir: {}", p.display()))?;
    }
    fs::create_dir_all(&data_path)
        .with_context(|| format!("Failed making data base dir: {}", data_path.display()))?;

    // 装配配置并写入 YAML
    let final_config = match backend_choice {
        BackendConfig::Sqlite(mut cfg) => {
            // 在这里根据之前探测到的 data_path 动态设置 sqlite 路径
            cfg.db_path = data_path.join("dsf.db");
            AppConfig {
                mode,
                config_path: Some(config_path.clone()),
                backend: BackendConfig::Sqlite(cfg),
            }
        }
    };

    let config_yaml =
        serde_yaml::to_string(&final_config).context("Failed to serialize AppConfig to YAML")?;

    fs::write(&config_path, config_yaml)
        .with_context(|| format!("Failed writing config file: {}", config_path.display()))?;

    #[allow(unreachable_patterns)]
    let _backend = match final_config.backend {
        BackendConfig::Sqlite(sqlite_cfg) => {
            SqliteBackend::new(sqlite_cfg).context("Failed initializing db file")?
        }
        _ => bail!("Unsupported backend type for initialization"),
    };

    println!("{}", "Initialization finished".green().bold());
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
        InstallMode::Custom => "Custom",
    };
    println!("{:<25} {}", "Environment Mode:".bold(), mode_str);

    let path_str = app_cfg
        .config_path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "Not set / In-memory".to_string());
    println!("{:<25} {}", "Config File Path:".bold(), path_str,);

    #[allow(unreachable_patterns)]
    match app_cfg.backend {
        BackendConfig::Sqlite(sqlite_cfg) => {
            println!(
                "{:<25} {}",
                "Backend Database Path:".bold(),
                sqlite_cfg.db_path.display().to_string().cyan()
            );

            println!(
                "\n{}",
                "--- Storage Backend (SQLite) Detailed Parameters ---"
                    .normal()
                    .dimmed()
            );

            if sqlite_cfg.db_path.exists() {
                println!(
                    "{:<25} {} (Concurrent Connections)",
                    "Connection Pool Size:", sqlite_cfg.pool_size
                );
                println!("{:<25} {} ms", "Busy Timeout:", sqlite_cfg.busy_timeout_ms);
                println!(
                    "{:<25} {}",
                    "Write-Ahead Log (WAL):",
                    if sqlite_cfg.wal {
                        "Enabled (True)".green()
                    } else {
                        "Disabled (False)".red()
                    }
                );
                println!(
                    "{:<25} {} (Balances performance & safety)",
                    "Synchronous Mode:", sqlite_cfg.synchronous
                );
                println!(
                    "{:<25} {}",
                    "Foreign Key Constraints:",
                    if sqlite_cfg.foreign_keys {
                        "Enforced (True)".green()
                    } else {
                        "Ignored (False)".red()
                    }
                );
            } else {
                println!(
                    "{}",
                    "Note: Database file has not been physically created yet. The paths above are active targets.\nPlease run `dsf init` first to initialize the environment."
                        .yellow()
                        .dimmed()
                );
            }
        }
        // BackendConfig::Yaml(yaml_cfg) => { ... }
        // BackendConfig::Remote(remote_cfg) => { ... }
        _ => {
            println!("{}", "Error: Unknown backend type.".red().bold(),);
        }
    }

    println!(
        "{}",
        "===================================================="
            .green()
            .bold()
    );
    Ok(())
}

fn handle_query(id: &str, level: VerifyLevel, show_diff: bool) -> Result<()> {
    validate_dataset_id(id)?;
    let backend = build_backend_auto()?;
    let service = DSFService::new(backend);

    match level {
        VerifyLevel::MetaOnly => {
            let meta = service.query_meta(id);
            match meta {
                Ok(m) => {
                    println!("{}", "Dataset exists".green());
                    println!("id: {}", m.id());
                    println!("path: {}", m.path.display());
                    println!("hash: {}", m.hash);
                    println!("deps: {:?}", m.dependencies);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    println!("{}", "Dataset doesn't exist".red().bold());
                }
                Err(e) => return Err(e.into()),
            }
        }
        VerifyLevel::SelfOnly => {
            let res = service.verify_self(id, show_diff)?;
            print_query(id, res.status, &res.dep_status);
        }
        VerifyLevel::Deep => {
            let res = service.verify_deep(id, show_diff)?;
            print_query(id, res.status, &res.dep_status);
        }
    }
    Ok(())
}
fn handle_register(opts: RegisterOptions) -> Result<()> {
    let service = DSFService::new(build_backend_auto()?);
    service.register(opts)?;
    Ok(())
}

fn handle_update(id: &str) -> Result<()> {
    let backend = build_backend_auto()?;
    let service = DSFService::new(backend);
    service.update_merkle(id)?;
    let meta = service.query_meta(id)?;

    println!(
        "{}",
        format!("updated dataset {}, new hash: \n{}", id, &meta.hash,).green()
    );
    Ok(())
}

fn handle_delete(id: &str, force: bool, yes: bool) -> Result<()> {
    let service = DSFService::new(build_backend_auto()?);
    let meta = service.query_meta(id)?;
    if !yes {
        let ok = Confirm::new()
            .with_prompt(format!(
                "id: {}\nPath: {:?}\nAre you sure you want to delete this dataset from the global registry?\nnote: deletes metadata only, actuall data on disk will be safe.",
                id, meta.path
            ))
            .default(false)
            .interact()?;
        if !ok {
            bail!("Deletion cancelled by user.");
        }
    }
    service.delete_metadata(id, force)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::DataSetStatus;
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
            } => {
                assert_eq!(id, "mnist@v1");
                assert_eq!(level, VerifyLevel::SelfOnly);
                assert!(!show_diff);
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn parse_query_deep_with_diff() {
        let cli = Cli::parse_from(["dsf", "query", "mnist@v1", "--level", "deep", "--show-diff"]);
        match cli.command {
            Commands::Query {
                id,
                level,
                show_diff,
            } => {
                assert_eq!(id, "mnist@v1");
                assert_eq!(level, VerifyLevel::Deep);
                assert!(show_diff);
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn parse_init_flags() {
        let cli = Cli::parse_from(["dsf", "init", "--global", "--non-interactive"]);
        match cli.command {
            Commands::Init {
                global,
                non_interactive,
            } => {
                assert!(global);
                assert!(non_interactive);
            }
            _ => panic!("expected Init"),
        }
    }

    #[test]
    fn parse_query_meta_only_level() {
        let cli = Cli::parse_from(["dsf", "query", "mnist@v1", "--level", "meta-only"]);
        match cli.command {
            Commands::Query {
                id,
                level,
                show_diff,
            } => {
                assert_eq!(id, "mnist@v1");
                assert_eq!(level, VerifyLevel::MetaOnly);
                assert!(!show_diff);
            }
            _ => panic!("expected Query"),
        }
    }

    #[test]
    fn parse_register_minimal_required_args() {
        let cli = Cli::parse_from([
            "dsf",
            "register",
            "--name",
            "mnist",
            "--tag",
            "v1",
            "--path",
            "/tmp/data",
            "--script-path",
            "/tmp/build.py",
        ]);
        match cli.command {
            Commands::Register {
                name,
                tag,
                path,
                script_path,
                owner_nickname,
                description_path,
                dependencies,
                force_heal,
                yes,
            } => {
                assert_eq!(name, "mnist");
                assert_eq!(tag, "v1");
                assert_eq!(path, PathBuf::from("/tmp/data"));
                assert_eq!(script_path, PathBuf::from("/tmp/build.py"));
                assert!(owner_nickname.is_none());
                assert!(description_path.is_none());
                assert!(dependencies.is_empty());
                assert!(!force_heal);
                assert!(!yes);
            }
            _ => panic!("expected Register"),
        }
    }

    #[test]
    fn parse_register_with_optional_args_and_multi_deps() {
        let cli = Cli::parse_from([
            "dsf",
            "register",
            "--name",
            "mnist",
            "--tag",
            "v2",
            "--path",
            "/tmp/data",
            "--script-path",
            "/tmp/build.py",
            "--description-path",
            "/tmp/desc.md",
            "--deps",
            "raw@v1",
            "--deps",
            "norm@v3",
            "--force-heal",
            "--yes",
        ]);
        match cli.command {
            Commands::Register {
                description_path,
                dependencies,
                force_heal,
                yes,
                ..
            } => {
                assert_eq!(description_path, Some(PathBuf::from("/tmp/desc.md")));
                assert_eq!(
                    dependencies,
                    vec!["raw@v1".to_string(), "norm@v3".to_string()]
                );
                assert!(force_heal);
                assert!(yes);
            }
            _ => panic!("expected Register"),
        }
    }

    #[test]
    fn parse_delete_flags() {
        let cli = Cli::parse_from(["dsf", "delete", "mnist@v1", "--force", "--yes"]);
        match cli.command {
            Commands::Delete { id, force, yes } => {
                assert_eq!(id, "mnist@v1");
                assert!(force);
                assert!(yes);
            }
            _ => panic!("expected Delete"),
        }
    }

    #[test]
    fn parse_update_command() {
        let cli = Cli::parse_from(["dsf", "update", "mnist@v1"]);
        match cli.command {
            Commands::Update { id } => assert_eq!(id, "mnist@v1"),
            _ => panic!("expected Update"),
        }
    }

    // ---------- validate_dataset_id ----------

    #[test]
    fn validate_dataset_id_accepts_normal_form() {
        assert!(validate_dataset_id("name@tag").is_ok());
        assert!(validate_dataset_id("dataset_01@2026-07-08").is_ok());
    }

    #[test]
    fn validate_dataset_id_rejects_missing_or_invalid_separator() {
        assert!(validate_dataset_id("nametag").is_err()); // no @
        assert!(validate_dataset_id("@tag").is_err()); // empty name
        assert!(validate_dataset_id("name@").is_err()); // empty tag
        assert!(validate_dataset_id("a@b@c").is_err()); // too many @
        assert!(validate_dataset_id("@").is_err()); // both empty
    }

    // ---------- validate_name_tag ----------

    #[test]
    fn validate_name_tag_accepts_valid_inputs() {
        assert!(validate_name_tag("mnist", "v1").is_ok());
        assert!(validate_name_tag("data-set", "2026").is_ok());
    }

    #[test]
    fn validate_name_tag_rejects_empty_or_contains_at() {
        assert!(validate_name_tag("", "v1").is_err());
        assert!(validate_name_tag("mnist", "").is_err());
        assert!(validate_name_tag("m@nist", "v1").is_err());
        assert!(validate_name_tag("mnist", "v@1").is_err());
    }

    // ---------- fmt_query ----------

    #[test]
    fn fmt_query_contains_status_text() {
        let healthy = fmt_query(DataSetStatus::Healthy);
        let broken = fmt_query(DataSetStatus::Broken);
        let broken_deps = fmt_query(DataSetStatus::BrokenDpes);
        let unverified = fmt_query(DataSetStatus::Unverified);

        // colored string 可能包含 ANSI，做 contains 即可
        assert!(healthy.contains("Healthy"));
        assert!(broken.contains("Broken"));
        assert!(broken_deps.contains("BrokenDeps"));
        assert!(unverified.contains("Unverified"));
    }

    // ---------- ensure_exists ----------

    #[test]
    fn ensure_exists_passes_for_existing_file_and_dir() {
        let dir = tempdir().expect("create temp dir");
        let file = dir.path().join("a.txt");
        std::fs::write(&file, "ok").expect("write temp file");

        assert!(ensure_exists(dir.path(), "--path").is_ok());
        assert!(ensure_exists(&file, "--script-path").is_ok());
    }

    #[test]
    fn ensure_exists_fails_for_missing_path() {
        let dir = tempdir().expect("create temp dir");
        let missing = dir.path().join("not_found.txt");

        let err = ensure_exists(&missing, "--path").unwrap_err().to_string();
        assert!(err.contains("--path"));
        assert!(err.contains("doesn't exist"));
    }

    // ---------- build_default_merkle_path ----------

    #[test]
    fn build_default_merkle_path_has_expected_file_name() {
        let p = build_default_merkle_path("mnist", "v1").expect("build path");
        let fname = p.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(fname, "mnist@v1.merkle.bin");
    }

    #[test]
    fn build_default_merkle_path_parent_dir_exists_after_call() {
        let p = build_default_merkle_path("abc", "t1").expect("build path");
        let parent = p.parent().expect("parent dir");
        assert!(parent.exists(), "parent dir should be created");
    }

    // ---------- smoke for print_query ----------

    #[test]
    fn print_query_smoke_no_panic() {
        print_query("mnist@v1", DataSetStatus::Healthy, &[]);
        print_query(
            "mnist@v1",
            DataSetStatus::BrokenDpes,
            &[DataSetStatus::Healthy, DataSetStatus::Broken],
        );
    }
}
