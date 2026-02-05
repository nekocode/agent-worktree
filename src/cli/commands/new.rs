// ===========================================================================
// wt new - Create a new worktree
// ===========================================================================

use std::path::Path;

use clap::Args;

use crate::cli::{write_path_file, write_path_file_lines, Error, Result};
use crate::config::Config;
use crate::git;
use crate::meta::WorktreeMeta;
use crate::process;
use crate::prompt::{self, SnapExitChoice};
use crate::util;

#[derive(Args)]
pub struct NewArgs {
    /// Branch name (random name like 'swift-fox' if not provided)
    branch: Option<String>,

    /// Base commit or branch to create from (default: trunk)
    #[arg(long, value_name = "REF")]
    base: Option<String>,

    /// Run command in snap mode: create -> run -> merge -> cleanup
    #[arg(short, long, value_name = "CMD")]
    snap: Option<String>,
}

pub fn run(args: NewArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    // Ensure we're in a git repo
    let repo_root = git::repo_root()?;
    let workspace_id = git::workspace_id()?;

    // Determine trunk branch
    let trunk = config
        .trunk
        .clone()
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    // Determine base
    let base = args.base.as_deref().unwrap_or(&trunk);

    // Generate or use provided branch name
    let branch = args.branch.unwrap_or_else(|| {
        util::generate_unique_branch_name(|n| git::branch_exists(n).unwrap_or(false))
    });

    // Worktree path
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(&branch);

    // Create workspace directory if needed
    std::fs::create_dir_all(&wt_dir).map_err(|e| Error::Other(e.to_string()))?;

    // Create worktree
    git::create_worktree(&wt_path, &branch, base)?;

    // Get base commit for metadata
    let base_commit = git::current_commit().unwrap_or_default();

    // Create metadata
    let mut meta = WorktreeMeta::new(base_commit, trunk.clone());
    if let Some(ref cmd) = args.snap {
        meta = meta.with_snap(cmd.clone());
    }

    let meta_path = wt_dir.join(format!("{branch}.status.toml"));
    meta.save(&meta_path)
        .map_err(|e| Error::Other(e.to_string()))?;

    // Copy files from main repo
    copy_files(&repo_root, &wt_path, config)?;

    // Run post_create hooks
    if !config.hooks.post_create.is_empty() {
        eprintln!("Running post-create hooks...");
        process::run_hooks(&config.hooks.post_create, &wt_path)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    // Handle snap mode - write path + command for shell wrapper to execute
    if let Some(cmd) = args.snap {
        if path_file.is_some() {
            // Shell wrapper mode: write path on first line, command on second
            write_path_file_lines(path_file, &[&wt_path.display().to_string(), &cmd])?;
        } else {
            // Direct mode (deprecated, but keep for backward compat)
            run_snap_mode(&cmd, &wt_path, &branch, config, &trunk)?;
        }
        return Ok(());
    }

    // Write path for shell integration
    if path_file.is_some() {
        write_path_file(path_file, &wt_path)?;
    } else {
        eprintln!("Created worktree: {branch}");
        eprintln!("Path: {}", wt_path.display());
    }

    Ok(())
}

fn copy_files(from: &Path, to: &Path, config: &Config) -> Result<()> {
    use ignore::overrides::OverrideBuilder;
    use ignore::WalkBuilder;

    if config.copy_files.is_empty() {
        return Ok(());
    }

    // Build gitignore-style matcher
    // Patterns work like .gitignore: "*.md" matches all .md files, "/*.md" matches only root
    let mut builder = OverrideBuilder::new(from);
    for pattern in &config.copy_files {
        builder
            .add(pattern)
            .map_err(|e| Error::Other(format!("invalid pattern '{}': {}", pattern, e)))?;
    }
    let overrides = builder
        .build()
        .map_err(|e| Error::Other(e.to_string()))?;

    // Walk directory with overrides (only matching files)
    let walker = WalkBuilder::new(from)
        .overrides(overrides)
        .standard_filters(false) // Don't apply .gitignore
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() {
            let rel = path.strip_prefix(from).unwrap();
            let dest = to.join(rel);

            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).ok();
            }

            std::fs::copy(path, &dest).ok();
        }
    }

    Ok(())
}

fn run_snap_mode(
    cmd: &str,
    wt_path: &Path,
    branch: &str,
    config: &Config,
    trunk: &str,
) -> Result<()> {
    eprintln!("Entering snap mode: {cmd}");
    eprintln!("Worktree: {branch}");
    eprintln!("---");

    loop {
        // Run agent
        let status =
            process::run_interactive(cmd, wt_path).map_err(|e| Error::Other(e.to_string()))?;

        if !status.success() {
            eprintln!("Agent exited abnormally. Worktree preserved.");
            return Ok(());
        }

        // Check change state
        std::env::set_current_dir(wt_path).map_err(|e| Error::Other(e.to_string()))?;
        let has_uncommitted = git::has_uncommitted_changes().unwrap_or(false);
        let has_commits_ahead = git::commit_count(trunk, "HEAD").unwrap_or(0) > 0;

        // No changes at all → cleanup
        if !has_uncommitted && !has_commits_ahead {
            eprintln!("No changes detected. Cleaning up...");
            cleanup_worktree(wt_path, branch, config)?;
            return Ok(());
        }

        // Only committed changes → auto merge (no prompt)
        if !has_uncommitted && has_commits_ahead {
            do_merge(wt_path, branch, trunk, config)?;
            return Ok(());
        }

        // Has uncommitted changes → prompt user
        match prompt::snap_exit_prompt() {
            Ok(SnapExitChoice::Reopen) => {
                eprintln!("Reopening agent...");
                continue;
            }
            Ok(SnapExitChoice::Exit) | Err(_) => {
                eprintln!();
                eprintln!("Exiting snap mode. Worktree preserved.");
                eprintln!();
                eprintln!("Your changes are safe. To continue later:");
                eprintln!("  git add . && git commit -m 'your message'");
                eprintln!("  wt merge    # merge and cleanup");
                eprintln!();
                return Ok(());
            }
        }
    }
}

fn do_merge(wt_path: &Path, branch: &str, trunk: &str, config: &Config) -> Result<()> {
    // Run pre-merge hooks
    if !config.hooks.pre_merge.is_empty() {
        eprintln!("Running pre-merge hooks...");
        process::run_hooks(&config.hooks.pre_merge, wt_path)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    eprintln!("Merging {} into {}...", branch, trunk);

    let repo_root = git::repo_root()?;
    std::env::set_current_dir(&repo_root).map_err(|e| Error::Other(e.to_string()))?;
    git::checkout(trunk)?;

    match config.merge_strategy {
        crate::config::MergeStrategy::Squash => {
            git::merge(branch, true, false)?;
            git::commit(&format!("Merge branch '{}'", branch))?;
        }
        crate::config::MergeStrategy::Merge => {
            git::merge(branch, false, true)?;
        }
        crate::config::MergeStrategy::Rebase => {
            git::checkout(branch)?;
            git::rebase(trunk)?;
            git::checkout(trunk)?;
            git::merge(branch, false, true)?;
        }
    }

    eprintln!("Merged {} into {}", branch, trunk);

    // Run post-merge hooks
    if !config.hooks.post_merge.is_empty() {
        eprintln!("Running post-merge hooks...");
        process::run_hooks(&config.hooks.post_merge, &repo_root)
            .map_err(|e| Error::Other(e.to_string()))?;
    }

    cleanup_worktree(wt_path, branch, config)?;
    Ok(())
}

fn cleanup_worktree(wt_path: &Path, branch: &str, config: &Config) -> Result<()> {
    // Move back to main repo first
    git::remove_worktree(wt_path, true)?;
    git::delete_branch(branch, true).ok();

    // Remove metadata
    if let Ok(workspace_id) = git::workspace_id() {
        let meta_path = config
            .workspaces_dir
            .join(&workspace_id)
            .join(format!("{branch}.status.toml"));
        std::fs::remove_file(meta_path).ok();
    }

    Ok(())
}
