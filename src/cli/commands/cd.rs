// ===========================================================================
// wt cd - Change to worktree directory
// ===========================================================================

use std::path::Path;

use clap::Args;

use crate::cli::{write_path_file, Error, Result};
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct CdArgs {
    /// Branch name to switch to
    branch: String,
}

pub fn run(args: CdArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(&args.branch);

    if !wt_path.exists() {
        return Err(Error::Git(git::Error::WorktreeNotFound(args.branch)));
    }

    if path_file.is_some() {
        write_path_file(path_file, &wt_path)?;
    } else {
        eprintln!("Switching to: {}", args.branch);
    }

    Ok(())
}
