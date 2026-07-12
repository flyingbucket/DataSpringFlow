use anyhow::Result;
use clap::Parser;

use dataspringflow_rs::cli::{Cli, run};

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    run(cli)?;
    Ok(())
}
