// ===========================================================================
// git/ops - Git 执行操作
// ===========================================================================

use std::process::Command;

use super::{run, Result};

/// Run git merge
pub fn merge(branch: &str, squash: bool, no_ff: bool, message: Option<&str>) -> Result<()> {
    let mut args = vec!["merge"];
    if squash {
        args.push("--squash");
    }
    if no_ff {
        args.push("--no-ff");
    }
    if let Some(msg) = message {
        args.push("-m");
        args.push(msg);
    }
    args.push(branch);
    run(&args)
}

/// Dry-run merge to check for conflicts without leaving state.
///
/// Mirrors the real merge strategy: a `--no-ff` dry-run that passes can
/// still fail under `--squash` (different three-way ancestor), silently
/// leaving the repo half-merged.
///
/// Returns Ok(true) if merge would be clean, Ok(false) on conflict.
pub fn dry_run_merge(branch: &str, squash: bool) -> Result<bool> {
    let result = if squash {
        run(&["merge", "--squash", "--no-commit", branch])
    } else {
        run(&["merge", "--no-commit", "--no-ff", branch])
    };
    let clean = result.is_ok();
    if squash {
        // `git merge --squash` never sets MERGE_HEAD, so `--abort` errors.
        let _ = run(&["reset", "--hard", "HEAD"]);
    } else {
        let _ = run(&["merge", "--abort"]);
    }
    Ok(clean)
}

/// Run git rebase
pub fn rebase(onto: &str) -> Result<()> {
    run(&["rebase", onto])
}

/// Checkout a branch
pub fn checkout(branch: &str) -> Result<()> {
    run(&["checkout", branch])
}

/// Commit staged changes
pub fn commit(message: &str) -> Result<()> {
    run(&["commit", "-m", message])
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
    run(&["rebase", "--abort"])
}

/// Continue an in-progress rebase
pub fn rebase_continue() -> Result<()> {
    run(&["rebase", "--continue"])
}

/// Abort an in-progress merge
pub fn merge_abort() -> Result<()> {
    run(&["merge", "--abort"])
}

/// Reset index to HEAD, clearing any merge/squash conflict state.
///
/// Unlike `merge --abort`, this also works for `--squash` conflicts
/// which don't create MERGE_HEAD.
pub fn reset_merge() -> Result<()> {
    run(&["reset", "--merge"])
}

/// Continue an in-progress merge (after conflict resolution)
pub fn merge_continue() -> Result<()> {
    run(&["commit", "--no-edit"])
}

/// 获取 git 目录路径
fn git_dir() -> Option<std::path::PathBuf> {
    Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| std::path::PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()))
}

/// Check if a rebase is in progress
pub fn is_rebase_in_progress() -> bool {
    git_dir().is_some_and(|d| d.join("rebase-merge").exists() || d.join("rebase-apply").exists())
}

/// Check if a merge is in progress
pub fn is_merge_in_progress() -> bool {
    git_dir().is_some_and(|d| d.join("MERGE_HEAD").exists())
}
