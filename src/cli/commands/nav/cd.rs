// ===========================================================================
// wt cd - Change to worktree directory (no args = return to main repo)
// ===========================================================================

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::cli::{write_path_file, Error, Result};
use crate::complete;
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct CdArgs {
    /// Branch name to switch to (omit to return to main repo)
    #[arg(add = ArgValueCompleter::new(complete::complete_worktrees))]
    branch: Option<String>,
}

pub fn run(args: CdArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let Some(branch) = args.branch else {
        // No branch specified → return to main repo root
        let repo_root = git::repo_root()?;
        if path_file.is_some() {
            write_path_file(path_file, &repo_root)?;
        } else {
            eprintln!("Returning to main repo");
        }
        return Ok(());
    };

    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(&branch);

    if !wt_path.exists() {
        return Err(Error::Git(git::Error::WorktreeNotFound(branch)));
    }

    if path_file.is_some() {
        write_path_file(path_file, &wt_path)?;
    } else {
        eprintln!("Switching to: {}", branch);
    }

    Ok(())
}
