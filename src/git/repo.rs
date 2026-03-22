// ===========================================================================
// git/repo - 仓库信息查询
// ===========================================================================

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Error, Result};

/// Get the root directory of the main git repository (not worktree)
///
/// Uses --git-common-dir to handle worktrees correctly.
pub fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()?;

    if !output.status.success() {
        return Err(Error::NotInRepo);
    }

    let git_dir = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());

    // Convert to absolute path if relative
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        std::env::current_dir()?.join(&git_dir)
    };

    // Canonicalize to resolve symlinks
    let git_dir = git_dir.canonicalize().map_err(|_| Error::NotInRepo)?;

    // Find the .git directory and return its parent
    let git_dir = if git_dir.ends_with(".git") {
        git_dir
    } else {
        // e.g. /path/to/repo/.git/worktrees/branch -> find .git
        let mut current = git_dir.as_path();
        loop {
            if current.ends_with(".git") {
                break;
            }
            current = current.parent().ok_or(Error::NotInRepo)?;
        }
        current.to_path_buf()
    };

    git_dir.parent().map(|p| p.to_path_buf()).ok_or(Error::NotInRepo)
}

/// Get the name of the current repository (directory name)
pub fn repo_name() -> Result<String> {
    let root = repo_root()?;
    root.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Command("cannot determine repo name".into()))
}

/// Get unique workspace ID for the current repository
///
/// Format: {repo_name}-{hash[0:6]} where hash is based on the absolute path.
/// This ensures repos with the same directory name but different paths get
/// unique workspace directories.
pub fn workspace_id() -> Result<String> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let root = repo_root()?;
    let name = repo_name()?;

    let mut hasher = DefaultHasher::new();
    root.hash(&mut hasher);
    let hash = hasher.finish();

    Ok(format!("{}-{:06x}", name, hash & 0xFFFFFF))
}

/// Get the current branch name
pub fn current_branch() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()?;

    if !output.status.success() {
        return Err(Error::NotInRepo);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Detect the trunk branch (main > master > default)
pub fn detect_trunk() -> Result<String> {
    for branch in ["main", "master"] {
        if branch_exists(branch)? {
            return Ok(branch.to_string());
        }
    }

    // Fall back to default branch from remote
    let output = Command::new("git")
        .args(["symbolic-ref", "refs/remotes/origin/HEAD"])
        .output()?;

    if output.status.success() {
        let full = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Some(branch) = full.strip_prefix("refs/remotes/origin/") {
            return Ok(branch.to_string());
        }
    }

    Ok("main".to_string())
}

/// List all local branch names (one subprocess instead of N branch_exists calls)
pub fn local_branches() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
        .output()?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

/// Check if a branch exists
pub fn branch_exists(name: &str) -> Result<bool> {
    let output = Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{name}"),
        ])
        .output()?;

    Ok(output.status.success())
}

/// Check if current working directory is inside the given path
pub fn is_cwd_inside(path: &Path) -> bool {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.canonicalize().ok())
        .and_then(|cwd| path.canonicalize().ok().map(|p| cwd.starts_with(p)))
        .unwrap_or(false)
}

/// Get current commit hash
pub fn current_commit() -> Result<String> {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output()?;

    if !output.status.success() {
        return Err(Error::NotInRepo);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
