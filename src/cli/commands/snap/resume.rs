// ===========================================================================
// wt snap-continue - Continue snap mode after agent exits
// ===========================================================================

use std::path::{Path, PathBuf};

// Exit codes consumed by the shell wrapper's snap loop.
// Keep in sync with the `case $continue_status` blocks in src/shell/mod.rs.
pub const EXIT_DONE: i32 = 0;
pub const EXIT_REOPEN: i32 = 2;
pub const EXIT_PRESERVE: i32 = 3;

use crate::cli::{write_path_file, Error, Result};
use crate::config::Config;
use crate::git;
use crate::meta::{self, WorktreeMeta};
use crate::process;
use crate::prompt::{self, SnapExitChoice, SnapMergeChoice};

// ===========================================================================
// Public Types
// ===========================================================================

/// Action to take after snap mode agent exits
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapAction {
    /// Cleanup and return to main (no changes)
    CleanupNoChanges,
    /// Merge changes and cleanup
    MergeAndCleanup,
    /// Exit snap mode but preserve worktree for manual handling
    ExitPreserve,
    /// Reopen the agent
    Reopen,
}

/// Context for snap-continue operation
#[derive(Debug)]
pub struct SnapContext {
    pub cwd: PathBuf,
    pub branch: String,
    pub merge_target: String,
    pub repo_root: PathBuf,
    pub has_uncommitted: bool,
    pub has_commits_ahead: bool,
}

// ===========================================================================
// Entry Point
// ===========================================================================

/// Run snap-continue command.
pub fn run(config: &Config, path_file: Option<&Path>) -> Result<()> {
    let ctx = gather_context(config)?;
    let action = determine_action(&ctx)?;
    execute_action(&ctx, &action, config, path_file)
}

// ===========================================================================
// Pure Logic (Testable)
// ===========================================================================

/// Gather context from git state
pub fn gather_context(config: &Config) -> Result<SnapContext> {
    let cwd = std::env::current_dir().map_err(|e| Error::Other(e.to_string()))?;
    let branch = git::current_branch()?;
    let workspace_id = git::workspace_id()?;
    let repo_root = git::repo_root()?;

    // Load metadata to get base_branch (fallback to legacy .status.toml).
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let meta_path = meta::meta_path_with_fallback(&wt_dir, &branch);
    let loaded_meta = WorktreeMeta::load(&meta_path).ok();

    // Resolve trunk lazily — `resolve_trunk` shells out when not configured,
    // and is only needed when meta is missing.
    //
    // If the worktree was created from a real base branch that has since
    // been deleted, refuse rather than silently merging into trunk —
    // landing commits on the wrong branch is a worse failure mode than an
    // explicit error that points the user at `wt merge --into <branch>`.
    let merge_target = match loaded_meta.as_ref().map(|m| m.base_branch.as_str()) {
        Some(bb) if git::branch_exists(bb).unwrap_or(false) => bb.to_string(),
        Some(bb) => {
            return Err(Error::Other(format!(
                "Base branch '{bb}' no longer exists.\n\
                 Resolve manually with: wt merge --into <branch>"
            )));
        }
        None => config.resolve_trunk(),
    };

    let has_uncommitted = git::has_uncommitted_changes().unwrap_or(false);
    let has_commits_ahead = git::commit_count(&merge_target, "HEAD").unwrap_or(0) > 0;

    Ok(SnapContext {
        cwd,
        branch,
        merge_target,
        repo_root,
        has_uncommitted,
        has_commits_ahead,
    })
}

/// Determine action based on context and user choice
pub fn determine_action(ctx: &SnapContext) -> Result<SnapAction> {
    // No changes at all → cleanup
    if !ctx.has_uncommitted && !ctx.has_commits_ahead {
        return Ok(SnapAction::CleanupNoChanges);
    }

    // Only committed changes → prompt merge or exit
    if !ctx.has_uncommitted && ctx.has_commits_ahead {
        return match prompt::snap_merge_prompt() {
            Ok(SnapMergeChoice::Merge) => Ok(SnapAction::MergeAndCleanup),
            Ok(SnapMergeChoice::Exit) | Err(_) => Ok(SnapAction::ExitPreserve),
        };
    }

    // Has uncommitted changes → prompt reopen or exit
    match prompt::snap_exit_prompt() {
        Ok(SnapExitChoice::Reopen) => Ok(SnapAction::Reopen),
        Ok(SnapExitChoice::Exit) | Err(_) => Ok(SnapAction::ExitPreserve),
    }
}

/// Determine action without prompt (for testing)
#[cfg(test)]
pub fn determine_action_with_choice(
    has_uncommitted: bool,
    has_commits_ahead: bool,
    exit_choice: Option<SnapExitChoice>,
    merge_choice: Option<SnapMergeChoice>,
) -> SnapAction {
    // No changes at all → cleanup
    if !has_uncommitted && !has_commits_ahead {
        return SnapAction::CleanupNoChanges;
    }

    // Only committed changes → use merge choice
    if !has_uncommitted && has_commits_ahead {
        return match merge_choice {
            Some(SnapMergeChoice::Merge) => SnapAction::MergeAndCleanup,
            Some(SnapMergeChoice::Exit) | None => SnapAction::ExitPreserve,
        };
    }

    // Has uncommitted changes → use exit choice
    match exit_choice {
        Some(SnapExitChoice::Reopen) => SnapAction::Reopen,
        Some(SnapExitChoice::Exit) | None => SnapAction::ExitPreserve,
    }
}

/// Remove worktree, branch, and metadata.
///
/// Uses non-force removal so that any untracked files left in the worktree
/// (build artifacts, .env, agent-generated scratch) cause the cleanup to
/// fail loudly instead of silently deleting work.
pub fn cleanup_worktree(wt_path: &Path, branch: &str, config: &Config) -> Result<()> {
    git::remove_worktree(wt_path, false)?;
    git::delete_branch(branch, true).ok();

    // Remove metadata
    if let Ok(workspace_id) = git::workspace_id() {
        let wt_dir = config.workspaces_dir.join(&workspace_id);
        meta::remove_meta(&wt_dir, branch);
    }

    Ok(())
}

// ===========================================================================
// Side Effects (Hard to Test)
// ===========================================================================

/// Execute action with side effects
fn execute_action(
    ctx: &SnapContext,
    action: &SnapAction,
    config: &Config,
    path_file: Option<&Path>,
) -> Result<()> {
    match action {
        SnapAction::CleanupNoChanges => {
            eprintln!("No changes detected. Cleaning up...");
            cleanup_worktree(&ctx.cwd, &ctx.branch, config)?;
            write_path_file(path_file, &ctx.repo_root)?;
            std::process::exit(EXIT_DONE);
        }
        SnapAction::MergeAndCleanup => {
            // Shared across pre_merge/post_merge: same worktree, branch, target.
            let hook_env = process::HookEnv {
                main_repo: &ctx.repo_root,
                worktree: &ctx.cwd,
                branch: &ctx.branch,
                base_branch: &ctx.merge_target,
            };

            // Run pre-merge hooks
            if !config.hooks.pre_merge.is_empty() {
                eprintln!("Running pre-merge hooks...");
                process::run_hooks(&config.hooks.pre_merge, &ctx.cwd, &hook_env)
                    .map_err(|e| Error::Other(e.to_string()))?;
            }

            eprintln!("Merging {} into {}...", ctx.branch, ctx.merge_target);

            std::env::set_current_dir(&ctx.repo_root).map_err(|e| Error::Other(e.to_string()))?;
            git::checkout(&ctx.merge_target)?;

            if !git::dry_run_merge(&ctx.branch, config.merge_strategy.is_squash())? {
                git::checkout(&ctx.merge_target).ok();
                let _ = std::env::set_current_dir(&ctx.cwd);
                super::super::merge::print_conflict_hint();
                eprintln!();
                eprintln!(
                    "Conflicts in worktree '{}'. Resolve there, then 'wt merge'.",
                    ctx.branch
                );
                std::process::exit(EXIT_PRESERVE);
            }

            if let Err(e) = super::super::merge::execute_merge(
                &ctx.branch,
                &ctx.merge_target,
                config.merge_strategy,
            ) {
                eprintln!("Merge failed: {e}");
                let _ = git::reset_merge();
                let _ = git::checkout(&ctx.merge_target);
                let _ = std::env::set_current_dir(&ctx.cwd);
                eprintln!(
                    "Worktree '{}' preserved. Inspect there and retry.",
                    ctx.branch
                );
                std::process::exit(EXIT_PRESERVE);
            }

            eprintln!("Merged {} into {}", ctx.branch, ctx.merge_target);

            // Match pre_merge CWD so hooks see the same context across phases.
            if !config.hooks.post_merge.is_empty() {
                eprintln!("Running post-merge hooks...");
                process::run_hooks(&config.hooks.post_merge, &ctx.cwd, &hook_env)
                    .map_err(|e| Error::Other(e.to_string()))?;
            }

            cleanup_worktree(&ctx.cwd, &ctx.branch, config)?;
            write_path_file(path_file, &ctx.repo_root)?;
            std::process::exit(EXIT_DONE);
        }
        SnapAction::Reopen => {
            eprintln!("Reopening agent...");
            std::process::exit(EXIT_REOPEN);
        }
        SnapAction::ExitPreserve => {
            eprintln!();
            eprintln!("Exiting snap mode. Worktree preserved.");
            eprintln!();
            eprintln!("Your changes are safe. To continue later:");
            eprintln!("  git add . && git commit -m 'your message'");
            eprintln!("  wt merge    # merge and cleanup");
            eprintln!();
            std::process::exit(EXIT_PRESERVE);
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // determine_action_with_choice tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_determine_no_changes() {
        // No uncommitted, no commits ahead → cleanup
        let action = determine_action_with_choice(false, false, Some(SnapExitChoice::Exit), None);
        assert_eq!(action, SnapAction::CleanupNoChanges);
    }

    #[test]
    fn test_determine_only_commits_ahead_merge() {
        // No uncommitted but has commits ahead, user chooses merge
        let action = determine_action_with_choice(false, true, None, Some(SnapMergeChoice::Merge));
        assert_eq!(action, SnapAction::MergeAndCleanup);
    }

    #[test]
    fn test_determine_only_commits_ahead_exit() {
        // No uncommitted but has commits ahead, user chooses exit
        let action = determine_action_with_choice(false, true, None, Some(SnapMergeChoice::Exit));
        assert_eq!(action, SnapAction::ExitPreserve);
    }

    #[test]
    fn test_determine_only_commits_ahead_no_choice_defaults_to_exit() {
        // No uncommitted but has commits ahead, no choice → exit
        let action = determine_action_with_choice(false, true, None, None);
        assert_eq!(action, SnapAction::ExitPreserve);
    }

    #[test]
    fn test_determine_uncommitted_reopen() {
        // Has uncommitted, user chooses reopen → reopen
        let action = determine_action_with_choice(true, false, Some(SnapExitChoice::Reopen), None);
        assert_eq!(action, SnapAction::Reopen);
    }

    #[test]
    fn test_determine_uncommitted_exit() {
        // Has uncommitted, user chooses exit → preserve worktree
        let action = determine_action_with_choice(true, true, Some(SnapExitChoice::Exit), None);
        assert_eq!(action, SnapAction::ExitPreserve);
    }

    #[test]
    fn test_determine_uncommitted_none_defaults_to_exit() {
        // Has uncommitted, no choice → preserve worktree
        let action = determine_action_with_choice(true, false, None, None);
        assert_eq!(action, SnapAction::ExitPreserve);
    }
}
