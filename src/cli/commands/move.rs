// ===========================================================================
// wt move - Rename worktree branch
// ===========================================================================

use clap::Args;

use crate::cli::{Error, Result};
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct MoveArgs {
    /// Current branch name (use '.' for current worktree)
    old_branch: String,

    /// New branch name
    new_branch: String,
}

pub fn run(args: MoveArgs, config: &Config, print_path: bool) -> Result<()> {
    let repo_name = git::repo_name()?;
    let wt_dir = config.workspaces_dir.join(&repo_name);

    // Resolve '.' to current branch
    let old_branch = if args.old_branch == "." {
        git::current_branch()?
    } else {
        args.old_branch
    };

    let old_path = wt_dir.join(&old_branch);
    let new_path = wt_dir.join(&args.new_branch);

    if !old_path.exists() {
        return Err(Error::Git(git::Error::WorktreeNotFound(old_branch.clone())));
    }

    if new_path.exists() {
        return Err(Error::Git(git::Error::WorktreeExists(
            args.new_branch.clone(),
        )));
    }

    // Check if we're inside the worktree being renamed
    let inside_target = git::is_cwd_inside(&old_path);

    // Rename branch
    git::rename_branch(&old_branch, &args.new_branch)?;

    // Rename worktree directory
    std::fs::rename(&old_path, &new_path).map_err(|e| Error::Other(e.to_string()))?;

    // Rename metadata file
    let old_meta = wt_dir.join(format!("{}.status.toml", old_branch));
    let new_meta = wt_dir.join(format!("{}.status.toml", args.new_branch));
    std::fs::rename(old_meta, new_meta).ok();

    eprintln!("Renamed {} -> {}", old_branch, args.new_branch);

    // If we were inside the renamed worktree, output new path for shell to cd
    if print_path && inside_target {
        println!("{}", new_path.display());
    }

    Ok(())
}
