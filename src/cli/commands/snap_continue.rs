// ===========================================================================
// wt snap-continue - Continue snap mode after agent exits
// ===========================================================================
//
// Exit codes:
// - 0: Done (merged or cleaned up), path_file contains repo root
// - 1: Error
// - 2: Reopen agent (shell wrapper should loop)
// - 3: Exit snap mode, stay in worktree (no cd)

use std::path::{Path, PathBuf};

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
    pub trunk: String,
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

    // Load metadata to get trunk (fallback to legacy .status.toml)
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let meta_path = meta::meta_path_with_fallback(&wt_dir, &branch);

    let meta = WorktreeMeta::load(&meta_path).ok();
    let trunk = meta
        .as_ref()
        .map(|m| m.trunk.clone())
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    let has_uncommitted = git::has_uncommitted_changes().unwrap_or(false);
    let has_commits_ahead = git::commit_count(&trunk, "HEAD").unwrap_or(0) > 0;

    Ok(SnapContext {
        cwd,
        branch,
        trunk,
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

/// Remove worktree, branch, and metadata
pub fn cleanup_worktree(wt_path: &Path, branch: &str, config: &Config) -> Result<()> {
    git::remove_worktree(wt_path, true)?;
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
            std::process::exit(0);
        }
        SnapAction::MergeAndCleanup => {
            // Run pre-merge hooks
            if !config.hooks.pre_merge.is_empty() {
                eprintln!("Running pre-merge hooks...");
                process::run_hooks(&config.hooks.pre_merge, &ctx.cwd)
                    .map_err(|e| Error::Other(e.to_string()))?;
            }

            eprintln!("Merging {} into {}...", ctx.branch, ctx.trunk);

            // Switch to trunk in main repo
            std::env::set_current_dir(&ctx.repo_root)
                .map_err(|e| Error::Other(e.to_string()))?;
            git::checkout(&ctx.trunk)?;

            if let Err(e) = super::merge::execute_merge(&ctx.branch, &ctx.trunk, config.merge_strategy) {
                eprintln!("Merge conflict:\n{e}");
                eprintln!();
                // Clean up main repo: reset --merge covers both regular and squash conflicts
                git::reset_merge().ok();
                git::rebase_abort().ok();
                git::checkout(&ctx.branch).ok();
                std::env::set_current_dir(&ctx.cwd).ok();
                eprintln!("Worktree preserved. To merge manually:");
                eprintln!("  wt merge");
                std::process::exit(3);
            }

            eprintln!("Merged {} into {}", ctx.branch, ctx.trunk);

            // Run post-merge hooks
            if !config.hooks.post_merge.is_empty() {
                eprintln!("Running post-merge hooks...");
                process::run_hooks(&config.hooks.post_merge, &ctx.repo_root)
                    .map_err(|e| Error::Other(e.to_string()))?;
            }

            cleanup_worktree(&ctx.cwd, &ctx.branch, config)?;
            write_path_file(path_file, &ctx.repo_root)?;
            std::process::exit(0);
        }
        SnapAction::Reopen => {
            eprintln!("Reopening agent...");
            std::process::exit(2);
        }
        SnapAction::ExitPreserve => {
            eprintln!();
            eprintln!("Exiting snap mode. Worktree preserved.");
            eprintln!();
            eprintln!("Your changes are safe. To continue later:");
            eprintln!("  git add . && git commit -m 'your message'");
            eprintln!("  wt merge    # merge and cleanup");
            eprintln!();
            // Exit code 3: exit snap mode, stay in worktree (no cd)
            std::process::exit(3);
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
    // SnapAction tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_snap_action_equality() {
        assert_eq!(SnapAction::Reopen, SnapAction::Reopen);
        assert_eq!(SnapAction::ExitPreserve, SnapAction::ExitPreserve);
        assert_ne!(SnapAction::Reopen, SnapAction::MergeAndCleanup);
        assert_ne!(SnapAction::ExitPreserve, SnapAction::Reopen);
    }

    #[test]
    fn test_snap_action_clone() {
        let action = SnapAction::MergeAndCleanup;
        let cloned = action.clone();
        assert_eq!(action, cloned);
    }

    #[test]
    fn test_snap_action_debug() {
        let action = SnapAction::CleanupNoChanges;
        let debug = format!("{:?}", action);
        assert!(debug.contains("CleanupNoChanges"));
    }

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
        let action = determine_action_with_choice(
            false,
            true,
            None,
            Some(SnapMergeChoice::Merge),
        );
        assert_eq!(action, SnapAction::MergeAndCleanup);
    }

    #[test]
    fn test_determine_only_commits_ahead_exit() {
        // No uncommitted but has commits ahead, user chooses exit
        let action = determine_action_with_choice(
            false,
            true,
            None,
            Some(SnapMergeChoice::Exit),
        );
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

    // -----------------------------------------------------------------------
    // SnapContext tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_snap_context_debug() {
        let ctx = SnapContext {
            cwd: PathBuf::from("/tmp/test"),
            branch: "feature".to_string(),
            trunk: "main".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            has_uncommitted: true,
            has_commits_ahead: false,
        };
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("feature"));
        assert!(debug.contains("main"));
    }
}
