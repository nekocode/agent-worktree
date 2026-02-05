// ===========================================================================
// wt cd - Change to worktree directory
// ===========================================================================

use clap::Args;

use crate::cli::{Error, Result};
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct CdArgs {
    /// Branch name to switch to
    branch: String,
}

pub fn run(args: CdArgs, config: &Config, print_path: bool) -> Result<()> {
    let repo_name = git::repo_name()?;
    let wt_dir = config.workspaces_dir.join(&repo_name);
    let wt_path = wt_dir.join(&args.branch);

    if !wt_path.exists() {
        return Err(Error::Git(git::Error::WorktreeNotFound(args.branch)));
    }

    if print_path {
        println!("{}", wt_path.display());
    } else {
        eprintln!("Switching to: {}", args.branch);
        println!("{}", wt_path.display());
    }

    Ok(())
}
