// ===========================================================================
// wt merge - Merge current worktree to trunk
// ===========================================================================

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::cli::{write_path_file, Error, Result};
use crate::complete;
use crate::config::{Config, MergeStrategy};
use crate::git;
use crate::meta;
use crate::process;

#[derive(Args)]
pub struct MergeArgs {
    /// Merge strategy (default: squash)
    #[arg(short, long, value_enum)]
    strategy: Option<MergeStrategy>,

    /// Target branch to merge into (default: trunk)
    #[arg(long, value_name = "BRANCH", add = ArgValueCompleter::new(complete::complete_branches))]
    into: Option<String>,

    /// Delete worktree after merge (default: keep)
    #[arg(short = 'd', long)]
    delete: bool,

    /// Skip pre-merge hooks
    #[arg(short = 'H', long)]
    skip_hooks: bool,
}

pub fn run(args: MergeArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let main_repo = git::repo_root()?;
    run_merge(args, config, path_file, &main_repo)
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

    // --into target must exist AND not be checked out elsewhere.
    // git refuses to checkout a branch that another worktree owns; without
    // the second check, merge would fail mid-flight with a confusing
    // low-level git error instead of a clear upfront message.
    if let Some(ref branch) = args.into {
        if !git::branch_exists(branch)? {
            return Err(Error::Other(format!("Branch '{branch}' does not exist")));
        }
        let main_canon = main_repo
            .canonicalize()
            .unwrap_or_else(|_| main_repo.to_path_buf());
        let conflict = git::list_worktrees()?.into_iter().find(|wt| {
            wt.branch.as_deref() == Some(branch.as_str())
                && wt.path.canonicalize().unwrap_or_else(|_| wt.path.clone()) != main_canon
        });
        if let Some(wt) = conflict {
            return Err(Error::Other(format!(
                "Branch '{branch}' is checked out in another worktree at {}.\n\
                 Switch that worktree off the branch, or merge from there directly.",
                wt.path.display()
            )));
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
        return Err(Error::Other(format!("Cannot merge {current} into itself")));
    }

    if git::has_uncommitted_changes()? {
        return Err(Error::Other(format!(
            "Worktree '{current}' has uncommitted changes. Commit or stash first."
        )));
    }

    let wt_path = wt_dir.join(&current);
    let inside_worktree = git::is_cwd_inside(&wt_path);

    let strategy = args.strategy.unwrap_or(config.merge_strategy);

    // Shared across pre_merge/post_merge: same worktree, branch, and target.
    let hook_env = process::HookEnv {
        main_repo,
        worktree: &wt_path,
        branch: &current,
        base_branch: &target,
    };

    if !args.skip_hooks && !config.hooks.pre_merge.is_empty() {
        eprintln!("Running pre-merge hooks...");
        // CWD = worktree so pre_merge and post_merge see the same context.
        process::run_hooks(&config.hooks.pre_merge, &wt_path, &hook_env)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    let commit_count = git::commit_count(&target, &current).unwrap_or(0);
    eprintln!("Merging {current} into {target} ({commit_count} commits, {strategy:?})");

    std::env::set_current_dir(main_repo).map_err(|e| Error::Other(e.to_string()))?;

    if git::has_uncommitted_changes()? {
        return Err(Error::Other(
            "Main repo has uncommitted changes. Commit or stash before merging.".into(),
        ));
    }
    if git::is_merge_in_progress() {
        return Err(Error::Other("Main repo has a merge in progress.".into()));
    }
    if git::is_rebase_in_progress() {
        return Err(Error::Other("Main repo has a rebase in progress.".into()));
    }

    // Capture main repo's current branch *before* we move HEAD, so we can
    // restore it if any subsequent step fails.
    let original_main_branch = git::current_branch().ok();

    git::checkout(&target)?;

    if !git::dry_run_merge(&current, strategy.is_squash())? {
        if let Some(orig) = &original_main_branch {
            let _ = git::checkout(orig);
        }
        print_conflict_hint();
        return Err(Error::Other("Merge aborted due to conflicts".into()));
    }

    match execute_merge(&current, &target, strategy) {
        Ok(false) => {
            eprintln!("Nothing to merge: {current} is already up to date with {target}");
            // Restore main repo to its prior branch — moving HEAD is a side
            // effect of the dry-run + checkout sequence; the user didn't
            // ask for it.
            if let Some(orig) = &original_main_branch {
                let _ = git::checkout(orig);
            }
            return Ok(());
        }
        Err(e) => {
            // Roll back any squash staging, then return HEAD to where it was.
            let _ = git::reset_merge();
            if let Some(orig) = &original_main_branch {
                let _ = git::checkout(orig);
            }
            return Err(e);
        }
        Ok(true) => {}
    }

    if !config.hooks.post_merge.is_empty() {
        eprintln!("Running post-merge hooks...");
        // Match pre_merge: CWD = worktree (still on disk, since cleanup
        // happens after this block).
        process::run_hooks(&config.hooks.post_merge, &wt_path, &hook_env)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    if args.delete {
        cleanup_worktree(&current, config)?;
        if inside_worktree {
            write_path_file(path_file, main_repo)?;
        }
    }

    eprintln!("Merge complete: {current} into {target}.");

    Ok(())
}

pub fn print_conflict_hint() {
    eprintln!("Merge would conflict. Sync first to resolve:");
    eprintln!("  wt sync");
    eprintln!("  wt merge");
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

/// Execute squash/merge. Caller must already be on trunk.
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
            // Detect "already up to date" before invoking git: when there
            // are no commits ahead, `git merge --no-ff` succeeds silently
            // without producing a merge commit. Returning Ok(true) in that
            // case would print "Merge complete" and (with -d) cleanup a
            // worktree even though nothing happened — caller relies on the
            // bool to know whether to proceed.
            if git::commit_count(trunk, branch)? == 0 {
                return Ok(false);
            }
            git::merge(branch, false, true, Some(&msg))?;
            Ok(true)
        }
    }
}

/// Clean up worktree after successful merge
pub fn cleanup_worktree(branch: &str, config: &Config) -> Result<()> {
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(branch);

    eprintln!("Cleaning up worktree: {branch}");

    git::remove_worktree(&wt_path, false).ok();

    // Force delete: squash merge rewrites history so -d thinks
    // the branch is "not fully merged" even though changes are in trunk
    git::delete_branch(branch, true).ok();

    crate::meta::remove_meta(&wt_dir, branch);

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
        assert_eq!(msg, "Initial implementation");
    }

    #[test]
    fn test_build_merge_message_empty_log() {
        let msg = build_merge_message("my-branch", "");
        assert_eq!(msg, "Merge branch 'my-branch'");
    }
}
