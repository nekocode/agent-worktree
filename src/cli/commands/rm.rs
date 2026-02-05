// ===========================================================================
// wt rm - Remove a worktree
// ===========================================================================

use clap::Args;

use crate::cli::{Error, Result};
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct RmArgs {
    /// Branch name to remove (use '.' for current worktree)
    branch: String,

    /// Force removal even with uncommitted changes
    #[arg(short, long)]
    force: bool,
}

pub fn run(args: RmArgs, config: &Config, print_path: bool) -> Result<()> {
    // Get main repo path BEFORE any destructive operations
    let main_path = git::repo_root()?;
    let repo_name = git::repo_name()?;
    let wt_dir = config.workspaces_dir.join(&repo_name);

    // Resolve '.' to current branch
    let branch = if args.branch == "." {
        git::current_branch()?
    } else {
        args.branch
    };

    let wt_path = wt_dir.join(&branch);

    if !wt_path.exists() {
        return Err(Error::Git(git::Error::WorktreeNotFound(branch.clone())));
    }

    // Check if we're inside the worktree being removed
    let inside_target = git::is_cwd_inside(&wt_path);

    // Remove worktree
    git::remove_worktree(&wt_path, args.force)?;

    // Switch to main repo before deleting branch (avoid "not in repo" error)
    std::env::set_current_dir(&main_path).ok();

    // Delete branch
    git::delete_branch(&branch, args.force).ok();

    // Remove metadata
    let meta_path = wt_dir.join(format!("{branch}.status.toml"));
    std::fs::remove_file(meta_path).ok();

    eprintln!("Removed worktree: {branch}");

    // If we were inside the removed worktree, output main repo path for shell to cd
    if print_path && inside_target {
        println!("{}", main_path.display());
    }

    Ok(())
}
