// ===========================================================================
// wt snap-continue - Continue snap mode after agent exits
// ===========================================================================
//
// Exit codes:
// - 0: Done (merged or discarded), stdout contains repo root path
// - 1: Error
// - 2: Reopen agent (shell wrapper should loop)

use std::path::{Path, PathBuf};

use crate::cli::{write_path_file, Error, Result};
use crate::config::{Config, MergeStrategy};
use crate::git;
use crate::meta::WorktreeMeta;
use crate::process;
use crate::prompt::{self, SnapExitChoice};

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
    /// Discard changes and cleanup
    DiscardAndCleanup,
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
    pub has_changes: bool,
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
    let repo_name = git::repo_name()?;
    let repo_root = git::repo_root()?;

    // Load metadata to get trunk
    let meta_path = config
        .workspaces_dir
        .join(&repo_name)
        .join(format!("{}.status.toml", branch));

    let meta = WorktreeMeta::load(&meta_path).ok();
    let trunk = meta
        .as_ref()
        .map(|m| m.trunk.clone())
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    // Check for both uncommitted changes AND commits ahead of trunk
    let has_changes = git::has_changes_from_trunk(&trunk).unwrap_or(false);

    Ok(SnapContext {
        cwd,
        branch,
        trunk,
        repo_root,
        has_changes,
    })
}

/// Determine action based on context and user choice
pub fn determine_action(ctx: &SnapContext) -> Result<SnapAction> {
    if !ctx.has_changes {
        return Ok(SnapAction::CleanupNoChanges);
    }

    match prompt::snap_exit_prompt() {
        Ok(SnapExitChoice::Commit) => Ok(SnapAction::MergeAndCleanup),
        Ok(SnapExitChoice::Reopen) => Ok(SnapAction::Reopen),
        Ok(SnapExitChoice::Discard) | Err(_) => Ok(SnapAction::DiscardAndCleanup),
    }
}

/// Determine action without prompt (for testing)
#[cfg(test)]
pub fn determine_action_with_choice(
    has_changes: bool,
    choice: Option<SnapExitChoice>,
) -> SnapAction {
    if !has_changes {
        return SnapAction::CleanupNoChanges;
    }

    match choice {
        Some(SnapExitChoice::Commit) => SnapAction::MergeAndCleanup,
        Some(SnapExitChoice::Reopen) => SnapAction::Reopen,
        Some(SnapExitChoice::Discard) | None => SnapAction::DiscardAndCleanup,
    }
}

/// Perform merge operation
pub fn perform_merge(branch: &str, trunk: &str, strategy: MergeStrategy) -> Result<()> {
    match strategy {
        MergeStrategy::Squash => {
            git::merge(branch, true, false)?;
            git::commit(&format!("Merge branch '{}'", branch))?;
        }
        MergeStrategy::Merge => {
            git::merge(branch, false, true)?;
        }
        MergeStrategy::Rebase => {
            git::checkout(branch)?;
            git::rebase(trunk)?;
            git::checkout(trunk)?;
            git::merge(branch, false, true)?;
        }
    }
    Ok(())
}

/// Remove worktree, branch, and metadata
pub fn cleanup_worktree(wt_path: &Path, branch: &str, config: &Config) -> Result<()> {
    let repo_root = git::repo_root().ok();

    git::remove_worktree(wt_path, true)?;
    git::delete_branch(branch, true).ok();

    // Remove metadata
    if let Some(root) = repo_root {
        let repo_name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let meta_path = config
            .workspaces_dir
            .join(repo_name)
            .join(format!("{branch}.status.toml"));
        std::fs::remove_file(meta_path).ok();
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

            perform_merge(&ctx.branch, &ctx.trunk, config.merge_strategy)?;

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
        SnapAction::DiscardAndCleanup => {
            eprintln!("Discarding changes...");
            cleanup_worktree(&ctx.cwd, &ctx.branch, config)?;
            write_path_file(path_file, &ctx.repo_root)?;
            std::process::exit(0);
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
        assert_ne!(SnapAction::Reopen, SnapAction::MergeAndCleanup);
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
        let action = determine_action_with_choice(false, Some(SnapExitChoice::Commit));
        assert_eq!(action, SnapAction::CleanupNoChanges);
    }

    #[test]
    fn test_determine_commit() {
        let action = determine_action_with_choice(true, Some(SnapExitChoice::Commit));
        assert_eq!(action, SnapAction::MergeAndCleanup);
    }

    #[test]
    fn test_determine_reopen() {
        let action = determine_action_with_choice(true, Some(SnapExitChoice::Reopen));
        assert_eq!(action, SnapAction::Reopen);
    }

    #[test]
    fn test_determine_discard() {
        let action = determine_action_with_choice(true, Some(SnapExitChoice::Discard));
        assert_eq!(action, SnapAction::DiscardAndCleanup);
    }

    #[test]
    fn test_determine_none_defaults_to_discard() {
        let action = determine_action_with_choice(true, None);
        assert_eq!(action, SnapAction::DiscardAndCleanup);
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
            has_changes: true,
        };
        let debug = format!("{:?}", ctx);
        assert!(debug.contains("feature"));
        assert!(debug.contains("main"));
    }
}
