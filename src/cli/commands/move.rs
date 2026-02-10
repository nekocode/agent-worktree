// ===========================================================================
// wt move - Rename worktree branch
// ===========================================================================

use std::path::Path;

use clap::Args;

use crate::cli::{write_path_file, Error, Result};
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct MoveArgs {
    /// Current branch name (use '.' for current worktree)
    old_branch: String,

    /// New branch name
    new_branch: String,
}

pub fn run(args: MoveArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);

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

    // Move worktree to new path (updates git's internal tracking)
    git::move_worktree(&old_path, &new_path)?;

    // Rename branch
    git::rename_branch(&old_branch, &args.new_branch)?;

    // Rename metadata file (find old with fallback, write new format)
    let old_meta = crate::meta::meta_path_with_fallback(&wt_dir, &old_branch);
    let new_meta = crate::meta::meta_path(&wt_dir, &args.new_branch);
    std::fs::rename(old_meta, new_meta).ok();

    eprintln!("Renamed {} -> {}", old_branch, args.new_branch);

    // If we were inside the renamed worktree, write new path for shell to cd
    if path_file.is_some() && inside_target {
        write_path_file(path_file, &new_path)?;
    }

    Ok(())
}
