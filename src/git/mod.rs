// ===========================================================================
// git - Git Operations via CLI
// ===========================================================================

mod repo;
mod worktree;
mod branch;
mod ops;

pub use repo::*;
pub use worktree::*;
pub use branch::*;
pub use ops::*;

use std::path::Path;
use std::process::Command;

pub type Result<T> = std::result::Result<T, Error>;

/// 安全的 Path -> &str 转换，替代 .to_str().unwrap()
pub(crate) fn path_str(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or_else(|| Error::Command(format!("path contains invalid UTF-8: {}", path.display())))
}

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

/// 执行 git 命令，失败时从 stderr+stdout 提取错误信息
fn run(args: &[&str]) -> Result<()> {
    let output = Command::new("git").args(args).output()?;
    if !output.status.success() {
        return Err(Error::Command(extract_error(&output)));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
