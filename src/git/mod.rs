// ===========================================================================
// git - Git Operations via CLI
// ===========================================================================

use std::path::{Path, PathBuf};
use std::process::Command;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("git command failed: {0}")]
    Command(String),

    #[error("not in a git repository")]
    NotInRepo,

    #[error("worktree '{0}' not found")]
    WorktreeNotFound(String),

    #[error("worktree '{0}' already exists")]
    WorktreeExists(String),

    #[error("branch '{0}' not found")]
    BranchNotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Get the root directory of the current git repository
pub fn repo_root() -> Result<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        return Err(Error::NotInRepo);
    }

    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(path))
}

/// Get the name of the current repository (directory name)
pub fn repo_name() -> Result<String> {
    let root = repo_root()?;
    root.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| Error::Command("cannot determine repo name".into()))
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

/// Create a new worktree
pub fn create_worktree(path: &Path, branch: &str, base: &str) -> Result<()> {
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            branch,
            path.to_str().unwrap(),
            base,
        ])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        if err.contains("already exists") {
            return Err(Error::WorktreeExists(branch.to_string()));
        }
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Remove a worktree
pub fn remove_worktree(path: &Path, force: bool) -> Result<()> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(path.to_str().unwrap());

    let output = Command::new("git").args(&args).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
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

/// Check if branch is merged into target
pub fn is_merged(branch: &str, target: &str) -> Result<bool> {
    let output = Command::new("git")
        .args(["branch", "--merged", target])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let merged = String::from_utf8_lossy(&output.stdout);
    Ok(merged
        .lines()
        .any(|l| l.trim().trim_start_matches("* ") == branch))
}

/// Delete a branch
pub fn delete_branch(name: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    let output = Command::new("git").args(["branch", flag, name]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Check for uncommitted changes
pub fn has_uncommitted_changes() -> Result<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;

    Ok(!output.stdout.is_empty())
}

/// Run git merge
pub fn merge(branch: &str, squash: bool, no_ff: bool) -> Result<()> {
    let mut args = vec!["merge"];
    if squash {
        args.push("--squash");
    }
    if no_ff {
        args.push("--no-ff");
    }
    args.push(branch);

    let output = Command::new("git").args(&args).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Run git rebase
pub fn rebase(onto: &str) -> Result<()> {
    let output = Command::new("git").args(["rebase", onto]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Rename branch
pub fn rename_branch(old: &str, new: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["branch", "-m", old, new])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Checkout a branch
pub fn checkout(branch: &str) -> Result<()> {
    let output = Command::new("git").args(["checkout", branch]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Commit staged changes
pub fn commit(message: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["commit", "-m", message])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Get short log of commits between two refs
pub fn log_oneline(from: &str, to: &str) -> Result<String> {
    let range = format!("{from}..{to}");
    let output = Command::new("git")
        .args(["log", "--oneline", &range])
        .output()?;

    if !output.status.success() {
        return Ok(String::new());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Get commit count between two refs
pub fn commit_count(from: &str, to: &str) -> Result<usize> {
    let range = format!("{from}..{to}");
    let output = Command::new("git")
        .args(["rev-list", "--count", &range])
        .output()?;

    if !output.status.success() {
        return Ok(0);
    }

    let count = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0);

    Ok(count)
}

/// Fetch updates from remote
pub fn fetch() -> Result<()> {
    let output = Command::new("git").args(["fetch", "--quiet"]).output()?;

    if !output.status.success() {
        // Fetch failing is often not critical, just warn
    }

    Ok(())
}

/// Abort an in-progress rebase
pub fn rebase_abort() -> Result<()> {
    let output = Command::new("git").args(["rebase", "--abort"]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Continue an in-progress rebase
pub fn rebase_continue() -> Result<()> {
    let output = Command::new("git")
        .args(["rebase", "--continue"])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Abort an in-progress merge
pub fn merge_abort() -> Result<()> {
    let output = Command::new("git").args(["merge", "--abort"]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Continue an in-progress merge (after conflict resolution)
pub fn merge_continue() -> Result<()> {
    let output = Command::new("git").args(["commit", "--no-edit"]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(err.to_string()));
    }

    Ok(())
}

/// Check if a rebase is in progress
pub fn is_rebase_in_progress() -> bool {
    let git_dir = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    if let Some(dir) = git_dir {
        let rebase_merge = Path::new(&dir).join("rebase-merge");
        let rebase_apply = Path::new(&dir).join("rebase-apply");
        return rebase_merge.exists() || rebase_apply.exists();
    }

    false
}

/// Check if a merge is in progress
pub fn is_merge_in_progress() -> bool {
    let git_dir = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    if let Some(dir) = git_dir {
        let merge_head = Path::new(&dir).join("MERGE_HEAD");
        return merge_head.exists();
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list_empty() {
        let result = parse_worktree_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_worktree_list_single() {
        let content = r#"worktree /path/to/repo
HEAD abc1234567890
branch refs/heads/main
"#;
        let result = parse_worktree_list(content);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, PathBuf::from("/path/to/repo"));
        assert_eq!(result[0].branch, Some("main".to_string()));
        assert_eq!(result[0].commit, Some("abc1234567890".to_string()));
        assert!(!result[0].is_bare);
    }

    #[test]
    fn test_parse_worktree_list_multiple() {
        let content = r#"worktree /path/to/main
HEAD abc123
branch refs/heads/main

worktree /path/to/feature
HEAD def456
branch refs/heads/feature-branch

worktree /path/to/detached
HEAD 789abc
detached
"#;
        let result = parse_worktree_list(content);
        assert_eq!(result.len(), 3);

        assert_eq!(result[0].branch, Some("main".to_string()));
        assert_eq!(result[1].branch, Some("feature-branch".to_string()));
        assert_eq!(result[2].branch, None); // detached HEAD
    }

    #[test]
    fn test_parse_worktree_list_bare() {
        let content = r#"worktree /path/to/bare.git
bare
"#;
        let result = parse_worktree_list(content);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_bare);
        assert!(result[0].branch.is_none());
    }

    #[test]
    fn test_error_display() {
        let err = Error::NotInRepo;
        assert_eq!(err.to_string(), "not in a git repository");

        let err = Error::WorktreeNotFound("feature".to_string());
        assert_eq!(err.to_string(), "worktree 'feature' not found");

        let err = Error::WorktreeExists("feature".to_string());
        assert_eq!(err.to_string(), "worktree 'feature' already exists");
    }

    #[test]
    fn test_is_cwd_inside() {
        // Current dir should be inside itself
        let cwd = std::env::current_dir().unwrap();
        assert!(is_cwd_inside(&cwd));

        // Current dir should not be inside a non-existent path
        assert!(!is_cwd_inside(Path::new("/nonexistent/path/12345")));

        // Current dir should be inside parent
        if let Some(parent) = cwd.parent() {
            assert!(is_cwd_inside(parent));
        }
    }
}
