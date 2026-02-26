// ===========================================================================
// git/worktree - Worktree CRUD
// ===========================================================================

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{path_str, run, Error, Result};

/// Create a new worktree
pub fn create_worktree(path: &Path, branch: &str, base: &str) -> Result<()> {
    let path_str = path_str(path)?;

    // Check if branch already exists
    if super::branch_exists(branch)? {
        // Branch exists - check if it already has a worktree
        let worktrees = list_worktrees()?;
        if worktrees.iter().any(|wt| wt.branch.as_deref() == Some(branch)) {
            return Err(Error::WorktreeExists(branch.to_string()));
        }

        // Branch exists but no worktree - just check it out
        run(&["worktree", "add", path_str, branch])?;
    } else {
        // Branch doesn't exist - create it from base
        run(&["worktree", "add", "-b", branch, path_str, base])?;
    }

    Ok(())
}

/// Remove a worktree
pub fn remove_worktree(path: &Path, force: bool) -> Result<()> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path_str(path)?);
    run(&args)
}

/// Move a worktree to a new path
pub fn move_worktree(old_path: &Path, new_path: &Path) -> Result<()> {
    run(&["worktree", "move", path_str(old_path)?, path_str(new_path)?])
}

/// List all worktrees
pub fn list_worktrees() -> Result<Vec<WorktreeInfo>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()?;

    if !output.status.success() {
        return Err(Error::NotInRepo);
    }

    let content = String::from_utf8_lossy(&output.stdout);
    Ok(parse_worktree_list(&content))
}

/// Parse git worktree list --porcelain output
pub fn parse_worktree_list(content: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current: Option<WorktreeInfo> = None;

    for line in content.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(wt) = current.take() {
                worktrees.push(wt);
            }
            current = Some(WorktreeInfo {
                path: PathBuf::from(path),
                branch: None,
                commit: None,
                is_bare: false,
            });
        } else if let Some(ref mut wt) = current {
            if let Some(branch) = line.strip_prefix("branch refs/heads/") {
                wt.branch = Some(branch.to_string());
            } else if let Some(commit) = line.strip_prefix("HEAD ") {
                wt.commit = Some(commit.to_string());
            } else if line == "bare" {
                wt.is_bare = true;
            }
        }
    }

    if let Some(wt) = current {
        worktrees.push(wt);
    }

    worktrees
}

#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub is_bare: bool,
}
