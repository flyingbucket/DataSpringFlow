use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand, ValueEnum};
use colored::*;
use dialoguer::{Confirm, Input, Select};
use std::fs;
use std::path::{Path, PathBuf};

use crate::backend::SqliteBackend;
use crate::core::{DSFDataSet, DataSetStatus, DatasetBackend, MetaData};
use crate::dag::DatasetGraph;
use crate::merkle::FileMerkleTree;
use crate::utils::{AppEnv, hashres_to_hex};

#[derive(Parser, Debug)]
#[command(
    name = "dsf",
    about = "DataSpringFlow: AI 数据集版本与依赖管理工具",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 交互式初始化安装
    Init {
        /// 全局安装（/etc/dsf + /var/lib/dsf）
        #[arg(long, default_value_t = false)]
        global: bool,
        /// 非交互，直接按默认路径初始化
        #[arg(long, default_value_t = false)]
        yes: bool,
    },

    /// 查询数据集状态
    Status {
        /// 数据集 ID: name@tag
        id: String,
        /// 校验等级
        #[arg(short, long, value_enum, default_value_t = VerifyLevel::SelfOnly)]
        level: VerifyLevel,
        /// 失败时显示差异
        #[arg(long, default_value_t = false)]
        show_diff: bool,
    },

    /// 注册新数据集
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
        description_path: Option<PathBuf>,
        #[arg(long = "deps")]
        dependencies: Vec<String>,
        /// 非交互：发现损坏依赖时强制 heal
        #[arg(long, default_value_t = false)]
        force_heal: bool,
        /// 非交互确认
        #[arg(long, default_value_t = false)]
        yes: bool,
    },

    /// 重算并更新 hash
    Update { id: String },

    /// 删除数据集条目
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
        Commands::Init { global, yes } => handle_init(global, yes),
        Commands::Status {
            id,
            level,
            show_diff,
        } => handle_status(&id, level, show_diff),
        Commands::Register {
            name,
            tag,
            path,
            script_path,
            description_path,
            dependencies,
            force_heal,
            yes,
        } => handle_register(
            &name,
            &tag,
            path,
            script_path,
            description_path,
            dependencies,
            force_heal,
            yes,
        ),
        Commands::Update { id } => handle_update(&id),
        Commands::Delete { id, force, yes } => handle_delete(&id, force, yes),
    }
}

fn handle_init(global_flag: bool, yes: bool) -> Result<()> {
    // 1) 交互决定安装级别（如果没显式 --global）
    let global = if global_flag {
        true
    } else if yes {
        false
    } else {
        let items = vec![
            "User 安装 (~/.config + ~/.local/share)",
            "Global 安装 (/etc + /var/lib)",
        ];
        let idx = Select::new()
            .with_prompt("请选择安装模式")
            .items(&items)
            .default(0)
            .interact()?;
        idx == 1
    };

    // 2) 权限检查
    if global && !is_root() {
        bail!(
            "{}\n{}",
            "全局安装需要 root 权限".red().bold(),
            "请使用: sudo dsf init --global".yellow()
        );
    }

    // 3) 计算默认路径（你可按自己的 AppEnv 细节调整）
    let env = if global {
        AppEnv::global_default()
    } else {
        AppEnv::resolve()
    };

    let default_config = env.config_path.clone();
    let default_db = env.db_path.clone();

    // 4) 可交互修改路径
    let (config_path, db_path) = if yes {
        (default_config, default_db)
    } else {
        let config_path: String = Input::new()
            .with_prompt("配置文件路径")
            .default(default_config.display().to_string())
            .interact_text()?;
        let db_path: String = Input::new()
            .with_prompt("SQLite 数据库路径")
            .default(default_db.display().to_string())
            .interact_text()?;
        (PathBuf::from(config_path), PathBuf::from(db_path))
    };

    println!(
        "安装模式: {}",
        if global {
            "global".cyan()
        } else {
            "user".cyan()
        }
    );
    println!("配置文件: {}", config_path.display().to_string().cyan());
    println!("数据库:   {}", db_path.display().to_string().cyan());

    if !yes {
        let ok = Confirm::new()
            .with_prompt("确认写入并初始化？")
            .default(true)
            .interact()?;
        if !ok {
            bail!("已取消初始化");
        }
    }

    // 5) 创建目录并写配置
    if let Some(p) = config_path.parent() {
        fs::create_dir_all(p).with_context(|| format!("创建配置目录失败: {}", p.display()))?;
    }
    if let Some(p) = db_path.parent() {
        fs::create_dir_all(p).with_context(|| format!("创建数据库目录失败: {}", p.display()))?;
    }

    let yaml = format!(
        "sqlite:\n  db_path: \"{}\"\n  pool_size: 8\n  busy_timeout_ms: 5000\n  wal: true\n  synchronous: \"NORMAL\"\n  foreign_keys: true\n",
        db_path.display()
    );
    fs::write(&config_path, yaml)
        .with_context(|| format!("写配置失败: {}", config_path.display()))?;

    // 6) 让后端按配置初始化（如果你后端通过 DSF_CONFIG 读取，这里可 set env）
    unsafe { std::env::set_var("DSF_CONFIG", config_path.as_os_str()) };
    let _backend = SqliteBackend::new().context("初始化数据库失败")?;

    println!("{}", "✔ 初始化完成".green().bold());
    Ok(())
}

fn handle_status(id: &str, level: VerifyLevel, show_diff: bool) -> Result<()> {
    validate_dataset_id(id)?;

    let backend = SqliteBackend::new()?;
    match level {
        VerifyLevel::MetaOnly => {
            let meta = backend.get_metadata(id);
            match meta {
                Ok(m) => {
                    println!("{}", "存在".green());
                    println!("id: {}", m.id());
                    println!("path: {}", m.path.display());
                    println!("hash: {}", m.hash);
                    println!("deps: {:?}", m.dependencies);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    println!("{}", "不存在".red().bold());
                }
                Err(e) => return Err(e.into()),
            }
        }
        VerifyLevel::SelfOnly => {
            let mut ds = DSFDataSet::load_from_id(id, &backend)?;
            let res = ds.verify_single(show_diff, &[])?;
            print_status(id, res.status, &res.dep_status);
        }
        VerifyLevel::Deep => {
            let mut ds = DSFDataSet::load_from_id(id, &backend)?;
            let res = ds.verify(&backend, show_diff)?;
            print_status(id, res.status, &res.dep_status);
        }
    }
    Ok(())
}

fn handle_register(
    name: &str,
    tag: &str,
    path: PathBuf,
    script_path: PathBuf,
    description_path: Option<PathBuf>,
    dependencies: Vec<String>,
    force_heal: bool,
    yes: bool,
) -> Result<()> {
    validate_name_tag(name, tag)?;
    ensure_exists(&path, "--path")?;
    ensure_exists(&script_path, "--script-path")?;
    if let Some(ref d) = description_path {
        ensure_exists(d, "--description-path")?;
    }

    let backend = SqliteBackend::new()?;

    // A) 依赖必须存在
    for dep_id in &dependencies {
        validate_dataset_id(dep_id)?;
        if backend.get_metadata(dep_id).is_err() {
            bail!("依赖数据集不存在，拦截注册: {}", dep_id);
        }
    }

    // B) DAG 查环（预检查）
    let graph = DatasetGraph::from_root_with_deps(name, tag, &dependencies, &backend)?;
    graph.check_cycle()?;

    // C) 检查依赖健康
    let mut broken = Vec::new();
    for dep_id in &dependencies {
        let mut ds = DSFDataSet::load_from_id(dep_id, &backend)?;
        let res = ds.verify(&backend, false)?; // deep 更稳妥
        if res.status != DataSetStatus::Healthy {
            broken.push(dep_id.clone());
        }
    }

    // D) 依赖异常 -> heal 决策
    if !broken.is_empty() {
        println!("{}", "检测到依赖不健康：".yellow().bold());
        for b in &broken {
            println!("  - {}", b.red());
        }

        let do_heal = if force_heal || yes {
            true
        } else {
            Confirm::new()
                .with_prompt("是否锁死当前状态并强制 heal 依赖及其深层依赖？")
                .default(false)
                .interact()?
        };

        if !do_heal {
            bail!("用户取消 heal，注册终止");
        }

        // 简化版：先 heal 当前依赖列表（深层 heal 可后续在 DAG 里做拓扑遍历）
        for dep_id in &broken {
            let mut ds = DSFDataSet::load_from_id(dep_id, &backend)?;
            let mut new_merkle = FileMerkleTree::new(ds.metadata.path.clone())?;
            ds.metadata.hash = hashres_to_hex(new_merkle.get_hash()?);
            new_merkle.save_to_disk(&ds.metadata.merkle_tree_path)?;
            backend.save_metadata(&ds.metadata)?;
            println!("  ✔ healed {}", dep_id.green());
        }
    }

    // E) 注册新数据集
    let merkle_tree_path = build_default_merkle_path(name, tag)?;
    let meta = MetaData::new(
        name,
        tag,
        path,
        description_path.unwrap_or_default(),
        script_path,
        dependencies,
        merkle_tree_path,
    )?;
    backend.save_metadata(&meta)?;
    println!("{}", format!("✔ 注册成功: {}", meta.id()).green().bold());

    Ok(())
}

fn handle_update(id: &str) -> Result<()> {
    validate_dataset_id(id)?;
    let backend = SqliteBackend::new()?;

    let mut ds = DSFDataSet::load_from_id(id, &backend)?;
    let mut merkle = FileMerkleTree::new(ds.metadata.path.clone())?;
    ds.metadata.hash = hashres_to_hex(merkle.get_hash()?);
    merkle.save_to_disk(&ds.metadata.merkle_tree_path)?;
    backend.save_metadata(&ds.metadata)?;

    println!(
        "{}",
        format!("✔ 已更新 {}，新 hash: {}...", id, &ds.metadata.hash[..8]).green()
    );
    Ok(())
}

fn handle_delete(id: &str, force: bool, yes: bool) -> Result<()> {
    validate_dataset_id(id)?;
    let backend = SqliteBackend::new()?;

    // 依赖反查（需要 backend 提供 list_all_metadata）
    let all = backend
        .list_all_metadata()
        .context("后端暂未实现 list_all_metadata，无法做安全删除检查")?;

    let mut referrers = Vec::new();
    for m in &all {
        if m.dependencies.iter().any(|d| d == id) {
            referrers.push(m.id());
        }
    }

    if !referrers.is_empty() && !force {
        println!("{}", "删除被拦截：该数据集被以下条目依赖".red().bold());
        for r in referrers {
            println!("  - {}", r);
        }
        bail!("如需强制删除请使用 --force");
    }

    if !yes {
        let ok = Confirm::new()
            .with_prompt(format!("确认删除 {} ?", id))
            .default(false)
            .interact()?;
        if !ok {
            bail!("用户取消删除");
        }
    }

    backend
        .delete_metadata(id)
        .context("后端暂未实现 delete_metadata 或删除失败")?;
    println!("{}", format!("✔ 已删除 {}", id).green().bold());

    Ok(())
}

fn print_status(id: &str, status: DataSetStatus, dep_statuses: &[DataSetStatus]) {
    let s = fmt_status(status);
    println!("dataset: {}", id.cyan());
    println!("status:  {}", s);

    if dep_statuses.is_empty() {
        println!("deps:    []");
    } else {
        let rendered: Vec<String> = dep_statuses.iter().map(|s| fmt_status(*s)).collect();
        println!("deps:    [{}]", rendered.join(", "));
    }
}

fn fmt_status(s: DataSetStatus) -> String {
    match s {
        DataSetStatus::Healthy => "Healthy".green().to_string(),
        DataSetStatus::Broken => "Broken".red().to_string(),
        DataSetStatus::BrokenDpes => "BrokenDeps".yellow().to_string(),
        DataSetStatus::Unverified => "Unverified".normal().to_string(),
    }
}

fn validate_dataset_id(id: &str) -> Result<()> {
    let parts: Vec<&str> = id.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        bail!("非法 id: {}，格式必须为 name@tag", id);
    }
    Ok(())
}

fn validate_name_tag(name: &str, tag: &str) -> Result<()> {
    if name.is_empty() || tag.is_empty() {
        bail!("name/tag 不能为空");
    }
    if name.contains('@') || tag.contains('@') {
        bail!("name/tag 不允许包含 '@'");
    }
    Ok(())
}

fn ensure_exists(p: &Path, arg_name: &str) -> Result<()> {
    if !p.exists() {
        bail!("{} 指向的路径不存在: {}", arg_name, p.display());
    }
    Ok(())
}

fn build_default_merkle_path(name: &str, tag: &str) -> Result<PathBuf> {
    // 可按你的 AppEnv 改成更标准的位置
    let env = AppEnv::resolve();
    let base = env
        .db_path
        .parent()
        .ok_or_else(|| anyhow!("无法确定 db parent path"))?;
    let merkle_dir = base.join("merkle");
    fs::create_dir_all(&merkle_dir)?;
    Ok(merkle_dir.join(format!("{}@{}.merkle.bin", name, tag)))
}

#[cfg(unix)]
fn is_root() -> bool {
    unsafe { libc::geteuid() == 0 }
}
#[cfg(not(unix))]
fn is_root() -> bool {
    false
}
