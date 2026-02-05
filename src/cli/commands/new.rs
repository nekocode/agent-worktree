// ===========================================================================
// wt new - Create a new worktree
// ===========================================================================

use std::path::Path;

use clap::Args;

use crate::cli::{Error, Result};
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
    #[arg(long, value_name = "CMD")]
    snap: Option<String>,
}

pub fn run(args: NewArgs, config: &Config, print_path: bool) -> Result<()> {
    // Ensure we're in a git repo
    let repo_root = git::repo_root()?;
    let repo_name = git::repo_name()?;

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
    let wt_dir = config.workspaces_dir.join(&repo_name);
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

    // Handle snap mode
    if let Some(cmd) = args.snap {
        run_snap_mode(&cmd, &wt_path, &branch, config, &trunk)?;
        return Ok(());
    }

    // Output path for shell integration
    if print_path {
        println!("{}", wt_path.display());
    } else {
        eprintln!("Created worktree: {branch}");
        eprintln!("Path: {}", wt_path.display());
        println!("{}", wt_path.display());
    }

    Ok(())
}

fn copy_files(from: &Path, to: &Path, config: &Config) -> Result<()> {
    for pattern in &config.copy_files {
        let entries = glob::glob(&from.join(pattern).to_string_lossy())
            .map_err(|e| Error::Other(e.to_string()))?;

        for entry in entries.flatten() {
            if entry.is_file() {
                let rel = entry.strip_prefix(from).unwrap();
                let dest = to.join(rel);

                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }

                std::fs::copy(&entry, &dest).ok();
            }
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

        // Check for uncommitted changes
        let has_changes = std::env::set_current_dir(wt_path)
            .ok()
            .and_then(|_| git::has_uncommitted_changes().ok())
            .unwrap_or(false);

        if !has_changes {
            // No changes, clean up
            eprintln!("No changes detected. Cleaning up...");
            cleanup_worktree(wt_path, branch, config)?;
            return Ok(());
        }

        // Prompt user
        match prompt::snap_exit_prompt() {
            Ok(SnapExitChoice::Commit) => {
                // Run pre-merge hooks
                if !config.hooks.pre_merge.is_empty() {
                    eprintln!("Running pre-merge hooks...");
                    process::run_hooks(&config.hooks.pre_merge, wt_path)
                        .map_err(|e| Error::Other(e.to_string()))?;
                }

                // Perform actual merge
                eprintln!("Merging {} into {}...", branch, trunk);

                // Get repo root to switch to trunk
                let repo_root = git::repo_root()?;

                // Switch to trunk in main repo
                std::env::set_current_dir(&repo_root).map_err(|e| Error::Other(e.to_string()))?;
                git::checkout(trunk)?;

                // Merge the branch
                match config.merge_strategy {
                    crate::config::MergeStrategy::Squash => {
                        git::merge(branch, true, false)?;
                        // Squash merge needs a commit
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
                return Ok(());
            }
            Ok(SnapExitChoice::Reopen) => {
                eprintln!("Reopening agent...");
                continue;
            }
            Ok(SnapExitChoice::Discard) | Err(_) => {
                eprintln!("Discarding changes...");
                cleanup_worktree(wt_path, branch, config)?;
                return Ok(());
            }
        }
    }
}

fn cleanup_worktree(wt_path: &Path, branch: &str, config: &Config) -> Result<()> {
    // Move back to main repo first
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
