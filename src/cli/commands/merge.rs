// ===========================================================================
// wt merge - Merge current worktree to trunk
// ===========================================================================

use clap::{Args, ValueEnum};

use crate::cli::{Error, Result};
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
    #[arg(long)]
    no_delete: bool,

    /// Continue merge after resolving conflicts
    #[arg(long)]
    r#continue: bool,

    /// Abort merge and restore previous state
    #[arg(long)]
    abort: bool,

    /// Skip pre-merge hooks
    #[arg(long)]
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

pub fn run(args: MergeArgs, config: &Config, print_path: bool) -> Result<()> {
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
    let repo_name = git::repo_name()?;
    let wt_path = config.workspaces_dir.join(&repo_name).join(&current);
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

    // Clean up unless --no-delete
    if !args.no_delete {
        cleanup_worktree(&current, config)?;
    }

    eprintln!("Merge complete.");

    // Output main repo path for shell to cd if we were inside worktree
    if print_path && inside_worktree {
        println!("{}", main_repo.display());
    }

    Ok(())
}

/// Squash merge: combine all commits into one
fn squash_merge(branch: &str, trunk: &str) -> Result<()> {
    // Switch to trunk
    git::checkout(trunk)?;

    // Squash merge
    git::merge(branch, true, false)?;

    // Check if there are staged changes to commit
    if git::has_staged_changes()? {
        let msg = format!("Merge branch '{}' (squashed)", branch);
        git::commit(&msg)?;
        eprintln!("Squash merged {branch} into {trunk}");
    } else {
        eprintln!("Nothing to merge: {branch} is already up to date with {trunk}");
    }

    Ok(())
}

/// Regular merge: preserve history
fn regular_merge(branch: &str, trunk: &str) -> Result<()> {
    // Switch to trunk
    git::checkout(trunk)?;

    // Merge with no-ff to preserve branch history
    git::merge(branch, false, true)?;

    eprintln!("Merged {branch} into {trunk}");
    Ok(())
}

/// Rebase merge: linear history
fn rebase_merge(branch: &str, trunk: &str) -> Result<()> {
    // First rebase branch onto trunk (already on feature branch)
    git::rebase(trunk)?;

    // Then fast-forward trunk
    git::checkout(trunk)?;
    git::merge(branch, false, false)?;

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
    let repo_name = git::repo_name()?;
    let wt_dir = config.workspaces_dir.join(&repo_name);
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
