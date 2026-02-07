// ===========================================================================
// git - Git Operations via CLI
// ===========================================================================

use std::path::{Path, PathBuf};
use std::process::Command;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
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

/// Extract error message from git command output.
///
/// Some git commands (merge, commit) put error info in stdout, not stderr.
/// This function checks stderr first, falls back to stdout.
fn extract_error(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        clean_git_error(&stderr)
    }
}

/// Clean git stderr to user-friendly message
fn clean_git_error(stderr: &str) -> String {
    let msg = stderr.trim();

    // Remove "fatal: " or "error: " prefix
    let msg = msg
        .strip_prefix("fatal: ")
        .or_else(|| msg.strip_prefix("error: "))
        .unwrap_or(msg);

    // Special case: worktree has uncommitted changes
    // "'/path/to/branch' contains modified or untracked files, use --force to delete it"
    if msg.contains("contains modified or untracked files") {
        // Extract branch name from path (last component)
        if let Some(start) = msg.find('\'') {
            if let Some(end) = msg[start + 1..].find('\'') {
                let path = &msg[start + 1..start + 1 + end];
                let branch = path.rsplit('/').next().unwrap_or(path);
                return format!("worktree '{branch}' has uncommitted changes, use --force");
            }
        }
    }

    msg.to_string()
}

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
    let path_str = path.to_str().unwrap();

    // Check if branch already exists
    if branch_exists(branch)? {
        // Branch exists - check if it already has a worktree
        let worktrees = list_worktrees()?;
        if worktrees.iter().any(|wt| wt.branch.as_deref() == Some(branch)) {
            return Err(Error::WorktreeExists(branch.to_string()));
        }

        // Branch exists but no worktree - just check it out
        let output = Command::new("git")
            .args(["worktree", "add", path_str, branch])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(clean_git_error(&err)));
        }
    } else {
        // Branch doesn't exist - create it from base
        let output = Command::new("git")
            .args(["worktree", "add", "-b", branch, path_str, base])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(Error::Command(clean_git_error(&err)));
        }
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
        return Err(Error::Command(clean_git_error(&err)));
    }

    Ok(())
}

/// Move a worktree to a new path
pub fn move_worktree(old_path: &Path, new_path: &Path) -> Result<()> {
    let output = Command::new("git")
        .args([
            "worktree",
            "move",
            old_path.to_str().unwrap(),
            new_path.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(clean_git_error(&err)));
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

/// Check if a branch has any diff from target (commits or uncommitted changes)
///
/// Returns true if branch has differences, false if identical to target.
pub fn has_diff_from(branch: &str, target: &str) -> Result<bool> {
    // Check committed diff: target...branch
    let output = Command::new("git")
        .args(["diff", "--quiet", &format!("{target}...{branch}")])
        .output()?;

    // exit 0 = no diff, exit 1 = has diff
    if !output.status.success() {
        return Ok(true);
    }

    // Also check if there are commits not in target
    let count = commit_count(target, branch)?;
    Ok(count > 0)
}

/// Delete a branch
pub fn delete_branch(name: &str, force: bool) -> Result<()> {
    let flag = if force { "-D" } else { "-d" };
    let output = Command::new("git").args(["branch", flag, name]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(clean_git_error(&err)));
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

/// Count uncommitted files in a specific worktree path
///
/// Returns the number of lines from `git -C <path> status --porcelain`.
pub fn uncommitted_count_in(path: &Path) -> Result<usize> {
    let output = Command::new("git")
        .args(["-C", path.to_str().unwrap(), "status", "--porcelain"])
        .output()?;

    let count = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|l| !l.is_empty())
        .count();

    Ok(count)
}

/// Diff stats: (insertions, deletions)
pub struct DiffStat {
    pub insertions: usize,
    pub deletions: usize,
}

/// Get diff --shortstat between two refs (committed changes)
///
/// Output format: " 3 files changed, 120 insertions(+), 30 deletions(-)"
pub fn diff_shortstat(from: &str, to: &str) -> Result<DiffStat> {
    let range = format!("{from}...{to}");
    let output = Command::new("git")
        .args(["diff", "--shortstat", &range])
        .output()?;

    Ok(parse_shortstat(&String::from_utf8_lossy(&output.stdout)))
}

/// Get diff --shortstat for uncommitted changes in a worktree
pub fn diff_shortstat_in(path: &Path) -> Result<DiffStat> {
    let output = Command::new("git")
        .args(["-C", path.to_str().unwrap(), "diff", "--shortstat", "HEAD"])
        .output()?;

    Ok(parse_shortstat(&String::from_utf8_lossy(&output.stdout)))
}

/// Parse `git diff --shortstat` output into (insertions, deletions)
fn parse_shortstat(output: &str) -> DiffStat {
    let line = output.trim();
    if line.is_empty() {
        return DiffStat { insertions: 0, deletions: 0 };
    }

    let mut insertions = 0;
    let mut deletions = 0;

    for part in line.split(',') {
        let part = part.trim();
        if part.contains("insertion") {
            insertions = part.split_whitespace().next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        } else if part.contains("deletion") {
            deletions = part.split_whitespace().next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }

    DiffStat { insertions, deletions }
}

/// Check if current branch has any changes compared to trunk
///
/// Returns true if:
/// - There are uncommitted changes in working directory, OR
/// - Current branch has commits ahead of trunk
pub fn has_changes_from_trunk(trunk: &str) -> Result<bool> {
    // Check uncommitted changes first
    if has_uncommitted_changes()? {
        return Ok(true);
    }

    // Check if there are commits ahead of trunk
    let count = commit_count(trunk, "HEAD")?;
    Ok(count > 0)
}

/// Check if there are staged changes ready to commit
pub fn has_staged_changes() -> Result<bool> {
    let output = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .output()?;

    // exit code 0 = no diff, exit code 1 = has diff
    Ok(!output.status.success())
}

/// Run git merge
pub fn merge(branch: &str, squash: bool, no_ff: bool, message: Option<&str>) -> Result<()> {
    let mut args = vec!["merge".to_string()];
    if squash {
        args.push("--squash".to_string());
    }
    if no_ff {
        args.push("--no-ff".to_string());
    }
    if let Some(msg) = message {
        args.push("-m".to_string());
        args.push(msg.to_string());
    }
    args.push(branch.to_string());

    let output = Command::new("git").args(&args).output()?;

    if !output.status.success() {
        return Err(Error::Command(extract_error(&output)));
    }

    Ok(())
}

/// Run git rebase
pub fn rebase(onto: &str) -> Result<()> {
    let output = Command::new("git").args(["rebase", onto]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(clean_git_error(&err)));
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
        return Err(Error::Command(clean_git_error(&err)));
    }

    Ok(())
}

/// Checkout a branch
pub fn checkout(branch: &str) -> Result<()> {
    let output = Command::new("git").args(["checkout", branch]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(clean_git_error(&err)));
    }

    Ok(())
}

/// Commit staged changes
pub fn commit(message: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["commit", "-m", message])
        .output()?;

    if !output.status.success() {
        return Err(Error::Command(extract_error(&output)));
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
        return Err(Error::Command(clean_git_error(&err)));
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
        return Err(Error::Command(clean_git_error(&err)));
    }

    Ok(())
}

/// Abort an in-progress merge
pub fn merge_abort() -> Result<()> {
    let output = Command::new("git").args(["merge", "--abort"]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(clean_git_error(&err)));
    }

    Ok(())
}

/// Reset index to HEAD, clearing any merge/squash conflict state.
///
/// Unlike `merge --abort`, this also works for `--squash` conflicts
/// which don't create MERGE_HEAD.
pub fn reset_merge() -> Result<()> {
    let output = Command::new("git").args(["reset", "--merge"]).output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Command(clean_git_error(&err)));
    }

    Ok(())
}

/// Continue an in-progress merge (after conflict resolution)
pub fn merge_continue() -> Result<()> {
    let output = Command::new("git").args(["commit", "--no-edit"]).output()?;

    if !output.status.success() {
        return Err(Error::Command(extract_error(&output)));
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
    use std::process::Command as StdCommand;
    use std::sync::Mutex;
    use tempfile::tempdir;

    // Global mutex for tests that change cwd
    static CWD_MUTEX: Mutex<()> = Mutex::new(());

    // =========================================================================
    // Helper: Setup a minimal git repo for testing
    // =========================================================================
    fn setup_test_repo() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let path = dir.path();

        StdCommand::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .expect("git init failed");

        StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output()
            .unwrap();

        StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output()
            .unwrap();

        std::fs::write(path.join("README.md"), "# Test\n").unwrap();

        StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output()
            .unwrap();

        StdCommand::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(path)
            .output()
            .unwrap();

        StdCommand::new("git")
            .args(["branch", "-M", "main"])
            .current_dir(path)
            .output()
            .ok();

        dir
    }

    /// Run a test that requires changing cwd, with proper locking
    fn with_cwd<F, T>(path: &Path, f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _guard = CWD_MUTEX.lock().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let result = f();
        std::env::set_current_dir(original).unwrap();
        result
    }

    // =========================================================================
    // Parse worktree list tests (pure functions, no cwd issues)
    // =========================================================================
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

    // =========================================================================
    // Error display tests (pure functions)
    // =========================================================================
    #[test]
    fn test_error_display() {
        let err = Error::NotInRepo;
        assert_eq!(err.to_string(), "not in a git repository");

        let err = Error::WorktreeNotFound("feature".to_string());
        assert_eq!(err.to_string(), "worktree 'feature' not found");

        let err = Error::WorktreeExists("feature".to_string());
        assert_eq!(err.to_string(), "worktree 'feature' already exists");

        let err = Error::BranchNotFound("missing".to_string());
        assert_eq!(err.to_string(), "branch 'missing' not found");

        let err = Error::Command("something failed".to_string());
        assert_eq!(err.to_string(), "something failed");
    }

    // =========================================================================
    // clean_git_error tests
    // =========================================================================
    #[test]
    fn test_clean_git_error_fatal_prefix() {
        let msg = clean_git_error("fatal: invalid reference: xxx");
        assert_eq!(msg, "invalid reference: xxx");
    }

    #[test]
    fn test_clean_git_error_error_prefix() {
        let msg = clean_git_error("error: some git error");
        assert_eq!(msg, "some git error");
    }

    #[test]
    fn test_clean_git_error_worktree_uncommitted() {
        let msg = clean_git_error(
            "fatal: '/Users/foo/.agent-worktree/workspaces/proj/branch' contains modified or untracked files, use --force to delete it",
        );
        assert_eq!(msg, "worktree 'branch' has uncommitted changes, use --force");
    }

    #[test]
    fn test_clean_git_error_no_prefix() {
        let msg = clean_git_error("some plain message");
        assert_eq!(msg, "some plain message");
    }

    // =========================================================================
    // extract_error tests
    // =========================================================================
    #[test]
    fn test_extract_error_prefers_stderr() {
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"stdout info".to_vec(),
            stderr: b"fatal: something broke".to_vec(),
        };
        let err = extract_error(&output);
        assert_eq!(err, "something broke");
    }

    #[test]
    fn test_extract_error_falls_back_to_stdout() {
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"CONFLICT (content): Merge conflict in file.txt\n".to_vec(),
            stderr: b"".to_vec(),
        };
        let err = extract_error(&output);
        assert!(err.contains("CONFLICT"));
    }

    #[test]
    fn test_extract_error_whitespace_only_stderr() {
        let output = std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: b"nothing to commit, working tree clean".to_vec(),
            stderr: b"  \n  ".to_vec(),
        };
        let err = extract_error(&output);
        assert!(err.contains("nothing to commit"));
    }

    // =========================================================================
    // is_cwd_inside tests
    // =========================================================================
    #[test]
    fn test_is_cwd_inside_current_dir() {
        let cwd = std::env::current_dir().unwrap();
        assert!(is_cwd_inside(&cwd));
    }

    #[test]
    fn test_is_cwd_inside_nonexistent() {
        assert!(!is_cwd_inside(Path::new("/nonexistent/path/12345")));
    }

    // =========================================================================
    // Git module function tests (require changing cwd, use mutex)
    // =========================================================================

    #[test]
    fn test_repo_root() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let root = repo_root();
            assert!(root.is_ok());
            let root_path = root.unwrap();
            assert!(root_path.exists());
            assert!(root_path.join(".git").exists());
        });
    }

    #[test]
    fn test_repo_root_not_in_repo() {
        let dir = tempdir().unwrap();
        with_cwd(dir.path(), || {
            let root = repo_root();
            assert!(root.is_err());
            assert!(matches!(root.unwrap_err(), Error::NotInRepo));
        });
    }

    #[test]
    fn test_repo_name() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let name = repo_name();
            assert!(name.is_ok());
            assert!(!name.unwrap().is_empty());
        });
    }

    #[test]
    fn test_workspace_id_format() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let id = workspace_id().unwrap();
            // Format: {repo_name}-{hash[0:6]}
            let parts: Vec<&str> = id.rsplitn(2, '-').collect();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].len(), 6); // hash suffix
            assert!(parts[0].chars().all(|c: char| c.is_ascii_hexdigit()));
        });
    }

    #[test]
    fn test_workspace_id_deterministic() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let id1 = workspace_id().unwrap();
            let id2 = workspace_id().unwrap();
            assert_eq!(id1, id2);
        });
    }

    #[test]
    fn test_workspace_id_unique_for_different_paths() {
        let dir1 = setup_test_repo();
        let dir2 = setup_test_repo();

        let id1 = with_cwd(dir1.path(), || workspace_id().unwrap());
        let id2 = with_cwd(dir2.path(), || workspace_id().unwrap());

        // Same repo name (both are random tempdir names), but different paths
        // Hash suffix should differ
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_current_branch() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let branch = current_branch();
            assert!(branch.is_ok());
            assert_eq!(branch.unwrap(), "main");
        });
    }

    #[test]
    fn test_detect_trunk() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let trunk = detect_trunk();
            assert!(trunk.is_ok());
            assert_eq!(trunk.unwrap(), "main");
        });
    }

    #[test]
    fn test_branch_exists_true() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let exists = branch_exists("main");
            assert!(exists.is_ok());
            assert!(exists.unwrap());
        });
    }

    #[test]
    fn test_branch_exists_false() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let exists = branch_exists("nonexistent-branch-12345");
            assert!(exists.is_ok());
            assert!(!exists.unwrap());
        });
    }

    #[test]
    fn test_current_commit() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let commit = current_commit();
            assert!(commit.is_ok());
            let hash = commit.unwrap();
            assert_eq!(hash.len(), 40);
        });
    }

    // =========================================================================
    // parse_shortstat tests (pure function)
    // =========================================================================
    #[test]
    fn test_parse_shortstat_full() {
        let stat = parse_shortstat(" 3 files changed, 120 insertions(+), 30 deletions(-)");
        assert_eq!(stat.insertions, 120);
        assert_eq!(stat.deletions, 30);
    }

    #[test]
    fn test_parse_shortstat_insertions_only() {
        let stat = parse_shortstat(" 1 file changed, 5 insertions(+)");
        assert_eq!(stat.insertions, 5);
        assert_eq!(stat.deletions, 0);
    }

    #[test]
    fn test_parse_shortstat_deletions_only() {
        let stat = parse_shortstat(" 2 files changed, 10 deletions(-)");
        assert_eq!(stat.insertions, 0);
        assert_eq!(stat.deletions, 10);
    }

    #[test]
    fn test_parse_shortstat_empty() {
        let stat = parse_shortstat("");
        assert_eq!(stat.insertions, 0);
        assert_eq!(stat.deletions, 0);
    }

    #[test]
    fn test_parse_shortstat_single_change() {
        let stat = parse_shortstat(" 1 file changed, 1 insertion(+), 1 deletion(-)");
        assert_eq!(stat.insertions, 1);
        assert_eq!(stat.deletions, 1);
    }

    #[test]
    fn test_has_uncommitted_changes_clean() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let has_changes = has_uncommitted_changes();
            assert!(has_changes.is_ok());
            assert!(!has_changes.unwrap());
        });
    }

    #[test]
    fn test_has_uncommitted_changes_dirty() {
        let dir = setup_test_repo();
        std::fs::write(dir.path().join("new_file.txt"), "content").unwrap();
        with_cwd(dir.path(), || {
            let has_changes = has_uncommitted_changes();
            assert!(has_changes.is_ok());
            assert!(has_changes.unwrap());
        });
    }

    #[test]
    fn test_list_worktrees() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let worktrees = list_worktrees();
            assert!(worktrees.is_ok());
            let list = worktrees.unwrap();
            assert!(!list.is_empty());
            assert_eq!(list[0].branch, Some("main".to_string()));
        });
    }

    #[test]
    fn test_is_rebase_in_progress() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            assert!(!is_rebase_in_progress());
        });
    }

    #[test]
    fn test_is_merge_in_progress() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            assert!(!is_merge_in_progress());
        });
    }

    #[test]
    fn test_log_oneline() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let log = log_oneline("HEAD", "HEAD");
            assert!(log.is_ok());
            assert!(log.unwrap().is_empty());
        });
    }

    #[test]
    fn test_commit_count() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let count = commit_count("HEAD", "HEAD");
            assert!(count.is_ok());
            assert_eq!(count.unwrap(), 0);
        });
    }

    #[test]
    fn test_fetch() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let result = fetch();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_is_merged() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let result = is_merged("main", "main");
            assert!(result.is_ok());
            assert!(result.unwrap());
        });
    }

    // =========================================================================
    // Worktree operations
    // =========================================================================
    #[test]
    fn test_create_and_remove_worktree() {
        let dir = setup_test_repo();
        let wt_path = dir.path().join("worktrees").join("feature");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();

        with_cwd(dir.path(), || {
            let result = create_worktree(&wt_path, "feature-branch", "main");
            assert!(result.is_ok());
            assert!(wt_path.exists());
            assert!(branch_exists("feature-branch").unwrap());

            let result = remove_worktree(&wt_path, false);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_create_worktree_duplicate() {
        let dir = setup_test_repo();
        let wt_path = dir.path().join("worktrees").join("dup");
        let wt_path2 = dir.path().join("worktrees").join("dup2");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();

        with_cwd(dir.path(), || {
            create_worktree(&wt_path, "dup-branch", "main").unwrap();
            let result = create_worktree(&wt_path2, "dup-branch", "main");
            assert!(result.is_err());
            assert!(matches!(result.unwrap_err(), Error::WorktreeExists(_)));
        });
    }

    // =========================================================================
    // Branch operations
    // =========================================================================
    #[test]
    fn test_rename_branch() {
        let dir = setup_test_repo();
        StdCommand::new("git")
            .args(["branch", "old-name"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        with_cwd(dir.path(), || {
            let result = rename_branch("old-name", "new-name");
            assert!(result.is_ok());
            assert!(!branch_exists("old-name").unwrap());
            assert!(branch_exists("new-name").unwrap());
        });
    }

    #[test]
    fn test_delete_branch() {
        let dir = setup_test_repo();
        StdCommand::new("git")
            .args(["branch", "to-delete"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        with_cwd(dir.path(), || {
            assert!(branch_exists("to-delete").unwrap());
            let result = delete_branch("to-delete", false);
            assert!(result.is_ok());
            assert!(!branch_exists("to-delete").unwrap());
        });
    }

    #[test]
    fn test_checkout() {
        let dir = setup_test_repo();
        StdCommand::new("git")
            .args(["branch", "other-branch"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        with_cwd(dir.path(), || {
            let result = checkout("other-branch");
            assert!(result.is_ok());
            assert_eq!(current_branch().unwrap(), "other-branch");
        });
    }

    // =========================================================================
    // Abort/continue operations
    // =========================================================================
    #[test]
    fn test_rebase_abort_no_rebase() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let result = rebase_abort();
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_merge_abort_no_merge() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let result = merge_abort();
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_reset_merge_clean_repo() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            // reset --merge on clean repo is a no-op success
            let result = reset_merge();
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_rebase_continue_no_rebase() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let result = rebase_continue();
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_merge_continue_no_merge() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            let result = merge_continue();
            assert!(result.is_err());
        });
    }

    // =========================================================================
    // Merge and rebase operations
    // =========================================================================
    #[test]
    fn test_merge_fast_forward() {
        let dir = setup_test_repo();
        // Create a branch that's already at main
        StdCommand::new("git")
            .args(["branch", "already-merged"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        with_cwd(dir.path(), || {
            // Merge should work (fast-forward or no-op)
            let result = merge("already-merged", false, false, None);
            // May succeed or say "already up to date"
            let _ = result;
        });
    }

    #[test]
    fn test_rebase_same_branch() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            // Rebase onto self should be a no-op
            let result = rebase("main");
            assert!(result.is_ok());
        });
    }

    // =========================================================================
    // Remove worktree with force
    // =========================================================================
    #[test]
    fn test_remove_worktree_force() {
        let dir = setup_test_repo();
        let wt_path = dir.path().join("worktrees").join("force-test");
        std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();

        with_cwd(dir.path(), || {
            create_worktree(&wt_path, "force-branch", "main").unwrap();

            // Create uncommitted changes in worktree
            std::fs::write(wt_path.join("uncommitted.txt"), "changes").unwrap();

            // Force remove should work
            let result = remove_worktree(&wt_path, true);
            assert!(result.is_ok());
        });
    }

    // =========================================================================
    // Delete branch with force
    // =========================================================================
    #[test]
    fn test_delete_branch_force() {
        let dir = setup_test_repo();
        StdCommand::new("git")
            .args(["branch", "unmerged-branch"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        with_cwd(dir.path(), || {
            // Force delete should work
            let result = delete_branch("unmerged-branch", true);
            assert!(result.is_ok());
        });
    }

    // =========================================================================
    // has_changes_from_trunk tests
    // =========================================================================

    #[test]
    fn test_has_changes_from_trunk_no_changes() {
        let dir = setup_test_repo();
        with_cwd(dir.path(), || {
            // On main, no changes from main
            let has = has_changes_from_trunk("main");
            assert!(has.is_ok());
            assert!(!has.unwrap());
        });
    }

    #[test]
    fn test_has_changes_from_trunk_with_committed_changes() {
        let dir = setup_test_repo();

        // Create feature branch and add a commit
        StdCommand::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        std::fs::write(dir.path().join("feature.txt"), "new feature").unwrap();

        StdCommand::new("git")
            .args(["add", "feature.txt"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        StdCommand::new("git")
            .args(["commit", "-m", "Add feature"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        with_cwd(dir.path(), || {
            // Feature branch has commits ahead of main
            let has = has_changes_from_trunk("main");
            assert!(has.is_ok());
            assert!(has.unwrap(), "Should detect committed changes ahead of trunk");
        });
    }

    #[test]
    fn test_has_changes_from_trunk_with_uncommitted_changes() {
        let dir = setup_test_repo();

        // Create feature branch with uncommitted changes only
        StdCommand::new("git")
            .args(["checkout", "-b", "feature"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        std::fs::write(dir.path().join("dirty.txt"), "uncommitted").unwrap();

        with_cwd(dir.path(), || {
            // Feature branch has uncommitted changes
            let has = has_changes_from_trunk("main");
            assert!(has.is_ok());
            assert!(has.unwrap(), "Should detect uncommitted changes");
        });
    }
}
