// ===========================================================================
// wt merge - Merge current worktree to trunk
// ===========================================================================

use std::path::{Path, PathBuf};

use clap::{Args, ValueEnum};

use crate::cli::{write_path_file, Error, Result};
use crate::config::{Config, MergeStrategy};
use crate::git;
use crate::process;

// ---------------------------------------------------------------------------
// Merge state marker: remembers which branch is being merged across conflict
// ---------------------------------------------------------------------------

const MERGE_BRANCH_FILE: &str = "WT_MERGE_BRANCH";

fn merge_state_path() -> Result<PathBuf> {
    let root = git::repo_root()?;
    Ok(root.join(".git").join(MERGE_BRANCH_FILE))
}

fn save_merge_state(branch: &str) -> Result<()> {
    let path = merge_state_path()?;
    std::fs::write(&path, branch).map_err(|e| Error::Other(e.to_string()))
}

fn load_merge_state() -> Result<Option<String>> {
    let path = merge_state_path()?;
    match std::fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Other(e.to_string())),
    }
}

fn clear_merge_state() {
    if let Ok(path) = merge_state_path() {
        std::fs::remove_file(path).ok();
    }
}

#[derive(Args)]
pub struct MergeArgs {
    /// Merge strategy (default: squash)
    #[arg(short, long, value_enum)]
    strategy: Option<MergeStrategyArg>,

    /// Target branch to merge into (default: trunk)
    #[arg(long, value_name = "BRANCH")]
    into: Option<String>,

    /// Keep worktree after merge (don't cleanup)
    #[arg(short = 'k', long)]
    keep: bool,

    /// Continue merge after resolving conflicts
    #[arg(long)]
    r#continue: bool,

    /// Abort merge and restore previous state
    #[arg(long)]
    abort: bool,

    /// Skip pre-merge hooks
    #[arg(short = 'H', long)]
    skip_hooks: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum MergeStrategyArg {
    Squash,
    Merge,
    Rebase,
}

impl From<MergeStrategyArg> for MergeStrategy {
    fn from(arg: MergeStrategyArg) -> Self {
        match arg {
            MergeStrategyArg::Squash => MergeStrategy::Squash,
            MergeStrategyArg::Merge => MergeStrategy::Merge,
            MergeStrategyArg::Rebase => MergeStrategy::Rebase,
        }
    }
}

pub fn run(args: MergeArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    // Get main repo path first (before any operations)
    let main_repo = git::repo_root()?;

    // Handle abort
    if args.abort {
        eprintln!("Aborting merge...");
        std::env::set_current_dir(&main_repo).map_err(|e| Error::Other(e.to_string()))?;
        abort_merge()?;
        clear_merge_state();
        return Ok(());
    }

    // Handle continue: finish merge + cleanup worktree
    if args.r#continue {
        let branch = load_merge_state()?;
        std::env::set_current_dir(&main_repo).map_err(|e| Error::Other(e.to_string()))?;
        continue_merge(branch.as_deref())?;
        clear_merge_state();

        // Run post-merge hooks
        if !config.hooks.post_merge.is_empty() {
            eprintln!("Running post-merge hooks...");
            process::run_hooks(&config.hooks.post_merge, &main_repo)
                .map_err(|e| Error::Other(e.to_string()))?;
        }

        // Cleanup worktree
        if let Some(branch) = branch {
            cleanup_worktree(&branch, config)?;
        }

        eprintln!("Merge complete.");
        if path_file.is_some() {
            write_path_file(path_file, &main_repo)?;
        }
        return Ok(());
    }

    // Get current state
    let current = git::current_branch()?;
    let trunk = args
        .into
        .or_else(|| config.trunk.clone())
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    if current == trunk {
        return Err(Error::Other("Cannot merge trunk into itself".into()));
    }

    // Check for uncommitted changes
    if git::has_uncommitted_changes()? {
        return Err(Error::Other(
            "Uncommitted changes detected. Commit or stash first.".into(),
        ));
    }

    // Check if running from inside worktree
    let workspace_id = git::workspace_id()?;
    let wt_path = config.workspaces_dir.join(&workspace_id).join(&current);
    let inside_worktree = git::is_cwd_inside(&wt_path);

    let strategy = args
        .strategy
        .map(MergeStrategy::from)
        .unwrap_or(config.merge_strategy);

    // Run pre-merge hooks (in worktree, before switching to main)
    if !args.skip_hooks && !config.hooks.pre_merge.is_empty() {
        let cwd = std::env::current_dir().map_err(|e| Error::Other(e.to_string()))?;
        eprintln!("Running pre-merge hooks...");
        process::run_hooks(&config.hooks.pre_merge, &cwd)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    // Show what will be merged
    let commit_count = git::commit_count(&trunk, &current).unwrap_or(0);
    eprintln!("Merging {current} into {trunk} ({commit_count} commits, {strategy:?})");

    // Switch to main repo and checkout trunk
    std::env::set_current_dir(&main_repo).map_err(|e| Error::Other(e.to_string()))?;

    // Check for dirty state from a previous failed merge
    if git::has_uncommitted_changes()? || git::is_merge_in_progress() || git::is_rebase_in_progress() {
        return Err(Error::Other(
            "Main repo has uncommitted changes or unresolved conflicts.".into(),
        ));
    }

    git::checkout(&trunk)?;

    // Execute merge
    match execute_merge(&current, &trunk, strategy) {
        Ok(false) => {
            eprintln!("Nothing to merge: {current} is already up to date with {trunk}");
        }
        Err(e) => {
            save_merge_state(&current)?;
            eprintln!("Merge conflict:\n{e}");
            eprintln!();
            eprintln!("Resolve conflicts, then:");
            eprintln!("  git add <files>");
            eprintln!("  wt merge --continue");
            eprintln!();
            eprintln!("Or abort:  wt merge --abort");

            // Let shell cd to main repo so user can resolve conflicts there
            if path_file.is_some() && inside_worktree {
                write_path_file(path_file, &main_repo)?;
            }
            return Ok(());
        }
        Ok(true) => {}
    }

    // Run post-merge hooks (in main repo)
    if !config.hooks.post_merge.is_empty() {
        eprintln!("Running post-merge hooks...");
        process::run_hooks(&config.hooks.post_merge, &main_repo)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    // Clean up unless --keep
    if !args.keep {
        cleanup_worktree(&current, config)?;
    }

    eprintln!("Merge complete.");

    // Write main repo path for shell to cd if we were inside worktree
    if path_file.is_some() && inside_worktree {
        write_path_file(path_file, &main_repo)?;
    }

    Ok(())
}

/// Build commit message for squash merge
///
/// - Single commit: use that commit's message directly
/// - Multiple commits: "Merge branch 'x'" + list all commits
/// - No commits: "Merge branch 'x'"
pub fn build_merge_message(branch: &str, log: &str) -> String {
    let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();

    match lines.len() {
        0 => format!("Merge branch '{branch}'"),
        1 => {
            // Single commit → strip hash prefix, use message directly
            let line = lines[0];
            line.split_once(' ')
                .map(|(_, msg)| msg.to_string())
                .unwrap_or_else(|| format!("Merge branch '{branch}'"))
        }
        _ => {
            let mut msg = format!("Merge branch '{branch}'\n\n");
            for line in &lines {
                msg.push_str(&format!("* {line}\n"));
            }
            msg.trim_end().to_string()
        }
    }
}

/// Execute merge. Caller must already be on trunk.
///
/// Returns true if changes were merged, false if already up to date.
pub fn execute_merge(branch: &str, trunk: &str, strategy: MergeStrategy) -> Result<bool> {
    let log = git::log_oneline(trunk, branch).unwrap_or_default();
    let msg = build_merge_message(branch, &log);

    match strategy {
        MergeStrategy::Squash => {
            git::merge(branch, true, false, None)?;
            if git::has_staged_changes()? {
                git::commit(&msg)?;
                Ok(true)
            } else {
                Ok(false)
            }
        }
        MergeStrategy::Merge => {
            git::merge(branch, false, true, Some(&msg))?;
            Ok(true)
        }
        MergeStrategy::Rebase => {
            git::checkout(branch)?;
            git::rebase(trunk)?;
            git::checkout(trunk)?;
            git::merge(branch, false, false, None)?;
            Ok(true)
        }
    }
}

/// Abort in-progress merge/rebase
fn abort_merge() -> Result<()> {
    // Try merge --abort (works for regular merge with MERGE_HEAD)
    if git::merge_abort().is_ok() {
        eprintln!("Merge aborted.");
        return Ok(());
    }

    // Try rebase --abort
    if git::rebase_abort().is_ok() {
        eprintln!("Rebase aborted.");
        return Ok(());
    }

    // Fallback: reset --merge handles squash conflicts (no MERGE_HEAD)
    if git::reset_merge().is_ok() {
        eprintln!("Merge state reset.");
        return Ok(());
    }

    Err(Error::Other("No merge or rebase in progress".into()))
}

/// Continue in-progress merge/rebase
fn continue_merge(branch: Option<&str>) -> Result<()> {
    // Try continuing rebase first (more common to have conflicts during rebase)
    let rebase_continue = std::process::Command::new("git")
        .args(["rebase", "--continue"])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    if rebase_continue.status.success() {
        eprintln!("Rebase continued.");
        return Ok(());
    }

    // Try continuing merge/squash (need to commit)
    if git::has_uncommitted_changes()? {
        // Squash merge has no MERGE_HEAD — build our own message
        let args = if !git::is_merge_in_progress() {
            if let Some(branch) = branch {
                let trunk = git::current_branch().unwrap_or_default();
                let log = git::log_oneline(&trunk, branch).unwrap_or_default();
                let msg = build_merge_message(branch, &log);
                vec!["commit".to_string(), "-m".to_string(), msg]
            } else {
                vec!["commit".to_string(), "--no-edit".to_string()]
            }
        } else {
            // Regular merge — message already set by git merge -m
            vec!["commit".to_string(), "--no-edit".to_string()]
        };

        let merge_continue = std::process::Command::new("git")
            .args(&args)
            .output()
            .map_err(|e| Error::Other(e.to_string()))?;

        if merge_continue.status.success() {
            eprintln!("Merge continued.");
            return Ok(());
        }
    }

    Err(Error::Other(
        "No merge/rebase in progress or conflicts not resolved".into(),
    ))
}

/// Clean up worktree after successful merge
fn cleanup_worktree(branch: &str, config: &Config) -> Result<()> {
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(branch);

    if wt_path.exists() {
        eprintln!("Cleaning up worktree: {branch}");

        // Remove worktree
        git::remove_worktree(&wt_path, false).ok();

        // Force delete branch: squash merge rewrites history so -d thinks
        // the branch is "not fully merged" even though changes are in trunk
        git::delete_branch(branch, true).ok();

        // Remove metadata
        let meta_path = wt_dir.join(format!("{branch}.status.toml"));
        std::fs::remove_file(meta_path).ok();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_merge_message_with_commits() {
        let log = "abc1234 Add user authentication\ndef5678 Fix login edge case\n";
        let msg = build_merge_message("feature-auth", log);
        assert!(msg.starts_with("Merge branch 'feature-auth'\n"));
        assert!(msg.contains("abc1234 Add user authentication"));
        assert!(msg.contains("def5678 Fix login edge case"));
    }

    #[test]
    fn test_build_merge_message_single_commit() {
        let log = "abc1234 Initial implementation\n";
        let msg = build_merge_message("fix-bug", log);
        // Single commit → use that commit's message directly
        assert_eq!(msg, "Initial implementation");
    }

    #[test]
    fn test_build_merge_message_empty_log() {
        let msg = build_merge_message("my-branch", "");
        assert_eq!(msg, "Merge branch 'my-branch'");
    }
}
