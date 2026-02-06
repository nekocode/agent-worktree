// ===========================================================================
// wt merge - Merge current worktree to trunk
// ===========================================================================

use std::path::Path;

use clap::{Args, ValueEnum};

use crate::cli::{write_path_file, Error, Result};
use crate::config::{Config, MergeStrategy};
use crate::git;
use crate::process;

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
        return Ok(());
    }

    // Handle continue
    if args.r#continue {
        eprintln!("Continuing merge...");
        std::env::set_current_dir(&main_repo).map_err(|e| Error::Other(e.to_string()))?;
        continue_merge()?;
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

    // Switch to main repo for merge operations
    // (can't checkout trunk in worktree if main repo has it checked out)
    std::env::set_current_dir(&main_repo).map_err(|e| Error::Other(e.to_string()))?;

    // Execute merge based on strategy
    match strategy {
        MergeStrategy::Squash => squash_merge(&current, &trunk)?,
        MergeStrategy::Merge => regular_merge(&current, &trunk)?,
        MergeStrategy::Rebase => rebase_merge(&current, &trunk)?,
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

/// Squash merge: combine all commits into one
fn squash_merge(branch: &str, trunk: &str) -> Result<()> {
    // Collect commit log before switching branches
    let log = git::log_oneline(trunk, branch).unwrap_or_default();

    // Switch to trunk
    git::checkout(trunk)?;

    // Squash merge
    git::merge(branch, true, false, None)?;

    // Check if there are staged changes to commit
    if git::has_staged_changes()? {
        let msg = build_merge_message(branch, &log);
        git::commit(&msg)?;
        eprintln!("Squash merged {branch} into {trunk}");
    } else {
        eprintln!("Nothing to merge: {branch} is already up to date with {trunk}");
    }

    Ok(())
}

/// Regular merge: preserve history
fn regular_merge(branch: &str, trunk: &str) -> Result<()> {
    // Collect commit log before switching branches
    let log = git::log_oneline(trunk, branch).unwrap_or_default();

    // Switch to trunk
    git::checkout(trunk)?;

    // Merge with no-ff to preserve branch history
    let msg = build_merge_message(branch, &log);
    git::merge(branch, false, true, Some(&msg))?;

    eprintln!("Merged {branch} into {trunk}");
    Ok(())
}

/// Rebase merge: linear history
fn rebase_merge(branch: &str, trunk: &str) -> Result<()> {
    // First rebase branch onto trunk (already on feature branch)
    git::rebase(trunk)?;

    // Then fast-forward trunk
    git::checkout(trunk)?;
    git::merge(branch, false, false, None)?;

    eprintln!("Rebased and merged {branch} into {trunk}");
    Ok(())
}

/// Abort in-progress merge/rebase
fn abort_merge() -> Result<()> {
    // Try aborting merge first
    let merge_abort = std::process::Command::new("git")
        .args(["merge", "--abort"])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    if merge_abort.status.success() {
        eprintln!("Merge aborted.");
        return Ok(());
    }

    // Try aborting rebase
    let rebase_abort = std::process::Command::new("git")
        .args(["rebase", "--abort"])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    if rebase_abort.status.success() {
        eprintln!("Rebase aborted.");
        return Ok(());
    }

    Err(Error::Other("No merge or rebase in progress".into()))
}

/// Continue in-progress merge/rebase
fn continue_merge() -> Result<()> {
    // Try continuing rebase first (more common to have conflicts during rebase)
    let rebase_continue = std::process::Command::new("git")
        .args(["rebase", "--continue"])
        .output()
        .map_err(|e| Error::Other(e.to_string()))?;

    if rebase_continue.status.success() {
        eprintln!("Rebase continued.");
        return Ok(());
    }

    // Try continuing merge (need to commit)
    if git::has_uncommitted_changes()? {
        let merge_continue = std::process::Command::new("git")
            .args(["commit", "--no-edit"])
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

        // Delete branch
        git::delete_branch(branch, false).ok();

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
