// ===========================================================================
// wt init - Initialize project configuration
// ===========================================================================

use std::path::Path;

use clap::Args;

use crate::cli::{Error, Result};
use crate::config::ProjectConfig;
use crate::git;

#[derive(Args)]
pub struct InitArgs {
    /// Main branch name (auto-detected: main > master)
    #[arg(long, value_name = "BRANCH")]
    trunk: Option<String>,
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

    let content = toml::to_string_pretty(&config).map_err(|e| Error::Other(e.to_string()))?;

    std::fs::write(config_path, content).map_err(|e| Error::Other(e.to_string()))?;

    eprintln!("Created .agent-worktree.toml");
    eprintln!("Trunk branch: {trunk}");

    Ok(())
}
