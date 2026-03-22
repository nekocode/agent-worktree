// ===========================================================================
// wt merge - Merge current worktree to trunk
// ===========================================================================

use std::path::{Path, PathBuf};

use clap::Args;

use crate::cli::{write_path_file, Error, Result};
use crate::config::{Config, MergeStrategy};
use crate::git;
use crate::meta;
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
    strategy: Option<MergeStrategy>,

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

pub fn run(args: MergeArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let main_repo = git::repo_root()?;

    if args.abort {
        return run_abort(&main_repo);
    }
    if args.r#continue {
        return run_continue(&main_repo, config, path_file);
    }
    run_merge(args, config, path_file, &main_repo)
}

fn run_abort(main_repo: &Path) -> Result<()> {
    eprintln!("Aborting merge...");
    std::env::set_current_dir(main_repo).map_err(|e| Error::Other(e.to_string()))?;
    abort_merge()?;
    clear_merge_state();
    Ok(())
}

fn run_continue(main_repo: &Path, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let branch = load_merge_state()?;
    std::env::set_current_dir(main_repo).map_err(|e| Error::Other(e.to_string()))?;
    continue_merge(branch.as_deref())?;
    clear_merge_state();

    if !config.hooks.post_merge.is_empty() {
        eprintln!("Running post-merge hooks...");
        process::run_hooks(&config.hooks.post_merge, main_repo)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    if let Some(branch) = branch {
        cleanup_worktree(&branch, config)?;
    }

    eprintln!("Merge complete.");
    if path_file.is_some() {
        write_path_file(path_file, main_repo)?;
    }
    Ok(())
}

fn run_merge(
    args: MergeArgs,
    config: &Config,
    path_file: Option<&Path>,
    main_repo: &Path,
) -> Result<()> {
    let current = git::current_branch()?;
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);

    // --into 指定的分支必须存在
    if let Some(ref branch) = args.into {
        if !git::branch_exists(branch)? {
            return Err(Error::Other(format!("Branch '{branch}' does not exist")));
        }
    }

    let target = meta::resolve_effective_target(
        &wt_dir,
        &current,
        args.into.as_deref(),
        |b| git::branch_exists(b).unwrap_or(false),
        &config.resolve_trunk(),
    );

    if current == target {
        return Err(Error::Other(format!(
            "Cannot merge {current} into itself"
        )));
    }

    if git::has_uncommitted_changes()? {
        return Err(Error::Other(
            "Uncommitted changes detected. Commit or stash first.".into(),
        ));
    }

    let wt_path = wt_dir.join(&current);
    let inside_worktree = git::is_cwd_inside(&wt_path);

    let strategy = args.strategy.unwrap_or(config.merge_strategy);

    if !args.skip_hooks && !config.hooks.pre_merge.is_empty() {
        let cwd = std::env::current_dir().map_err(|e| Error::Other(e.to_string()))?;
        eprintln!("Running pre-merge hooks...");
        process::run_hooks(&config.hooks.pre_merge, &cwd)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    let commit_count = git::commit_count(&target, &current).unwrap_or(0);
    eprintln!("Merging {current} into {target} ({commit_count} commits, {strategy:?})");

    std::env::set_current_dir(main_repo).map_err(|e| Error::Other(e.to_string()))?;

    if git::has_uncommitted_changes()?
        || git::is_merge_in_progress()
        || git::is_rebase_in_progress()
    {
        return Err(Error::Other(
            "Main repo has uncommitted changes or unresolved conflicts.".into(),
        ));
    }

    git::checkout(&target)?;

    match execute_merge(&current, &target, strategy) {
        Ok(false) => {
            eprintln!("Nothing to merge: {current} is already up to date with {target}");
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

            if path_file.is_some() && inside_worktree {
                write_path_file(path_file, main_repo)?;
            }
            return Ok(());
        }
        Ok(true) => {}
    }

    if !config.hooks.post_merge.is_empty() {
        eprintln!("Running post-merge hooks...");
        process::run_hooks(&config.hooks.post_merge, main_repo)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    if !args.keep {
        cleanup_worktree(&current, config)?;
    }

    eprintln!("Merge complete.");

    if path_file.is_some() && inside_worktree {
        write_path_file(path_file, main_repo)?;
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
    // 尝试继续 rebase
    if git::rebase_continue().is_ok() {
        eprintln!("Rebase continued.");
        return Ok(());
    }

    // 尝试继续 merge/squash（需要 commit）
    if git::has_uncommitted_changes()? {
        if !git::is_merge_in_progress() {
            // Squash merge 没有 MERGE_HEAD，需要自己构建 commit message
            if let Some(branch) = branch {
                let trunk = git::current_branch().unwrap_or_default();
                let log = git::log_oneline(&trunk, branch).unwrap_or_default();
                let msg = build_merge_message(branch, &log);
                git::commit(&msg)?;
            } else {
                git::merge_continue()?;
            }
        } else {
            // 常规 merge — message 已由 git merge -m 设置
            git::merge_continue()?;
        }
        eprintln!("Merge continued.");
        return Ok(());
    }

    Err(Error::Other(
        "No merge/rebase in progress or conflicts not resolved".into(),
    ))
}

/// Clean up worktree after successful merge
pub fn cleanup_worktree(branch: &str, config: &Config) -> Result<()> {
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
        crate::meta::remove_meta(&wt_dir, branch);
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
