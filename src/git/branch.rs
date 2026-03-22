// ===========================================================================
// git/branch - 分支操作 + 状态检查
// ===========================================================================

use std::path::Path;
use std::process::Command;

use super::{run, Result};

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
    run(&["branch", flag, name])
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
        .args(["-C", super::path_str(path)?, "status", "--porcelain"])
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
        .args(["-C", super::path_str(path)?, "diff", "--shortstat", "HEAD"])
        .output()?;

    Ok(parse_shortstat(&String::from_utf8_lossy(&output.stdout)))
}

/// Parse `git diff --shortstat` output into (insertions, deletions)
pub(super) fn parse_shortstat(output: &str) -> DiffStat {
    let line = output.trim();
    if line.is_empty() {
        return DiffStat {
            insertions: 0,
            deletions: 0,
        };
    }

    let mut insertions = 0;
    let mut deletions = 0;

    for part in line.split(',') {
        let part = part.trim();
        if part.contains("insertion") {
            insertions = part
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        } else if part.contains("deletion") {
            deletions = part
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }

    DiffStat {
        insertions,
        deletions,
    }
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

/// Rename branch
pub fn rename_branch(old: &str, new: &str) -> Result<()> {
    run(&["branch", "-m", old, new])
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
