// ===========================================================================
// wt init - Initialize project configuration
// ===========================================================================

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::cli::{Error, Result};
use crate::complete;
use crate::config::{MergeStrategy, ProjectConfig};
use crate::git;

#[derive(Args)]
pub struct InitArgs {
    /// Main branch name (auto-detected: main > master)
    #[arg(long, value_name = "BRANCH", add = ArgValueCompleter::new(complete::complete_branches))]
    trunk: Option<String>,

    /// Default merge strategy
    #[arg(long, value_enum, value_name = "STRATEGY")]
    merge_strategy: Option<MergeStrategy>,

    /// Files to copy from main repo to new worktrees (can be repeated)
    #[arg(long, value_name = "PATTERN")]
    copy_files: Vec<String>,
}

pub fn run(args: InitArgs) -> Result<()> {
    let config_path = Path::new(".agent-worktree.toml");

    if config_path.exists() {
        return Err(Error::Other("Config file already exists".into()));
    }

    // Detect trunk if not specified
    let trunk = args
        .trunk
        .or_else(|| git::detect_trunk().ok())
        .unwrap_or_else(|| "main".into());

    let mut config = ProjectConfig::default();
    config.general.trunk = Some(trunk.clone());
    config.general.merge_strategy = args.merge_strategy;
    if !args.copy_files.is_empty() {
        config.general.copy_files = args.copy_files;
    }

    let content = toml::to_string_pretty(&config).map_err(|e| Error::Other(e.to_string()))?;

    std::fs::write(config_path, content).map_err(|e| Error::Other(e.to_string()))?;

    eprintln!("Created .agent-worktree.toml");
    eprintln!("Trunk branch: {trunk}");
    if let Some(ref strategy) = config.general.merge_strategy {
        eprintln!("Merge strategy: {strategy:?}");
    }
    if !config.general.copy_files.is_empty() {
        eprintln!("Copy files: {}", config.general.copy_files.join(", "));
    }

    Ok(())
}
