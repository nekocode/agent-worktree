// ===========================================================================
// wt new - Create a new worktree
// ===========================================================================

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::cli::{write_path_file, write_path_file_lines, Error, Result};
use crate::complete;
use crate::config::Config;
use crate::git;
use crate::meta::{self, WorktreeMeta};
use crate::process;
use crate::util;

#[derive(Args)]
pub struct NewArgs {
    /// Branch name (random name like 'swift-fox' if not provided)
    branch: Option<String>,

    /// Base branch to create from and merge back to (default: current branch)
    #[arg(long, value_name = "BRANCH", add = ArgValueCompleter::new(complete::complete_branches))]
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
    let trunk = config.resolve_trunk();

    // Resolve base branch: --base flag > current branch > trunk
    // base_branch 决定了 checkout 起点和 merge/sync 的默认目标
    let base_branch = if let Some(ref b) = args.base {
        // --base 必须是已存在的分支
        if !git::branch_exists(b)? {
            return Err(Error::Other(format!("Branch '{b}' does not exist")));
        }
        b.clone()
    } else {
        // 默认使用当前分支，detached HEAD 时 fallback 到 trunk
        git::current_branch()
            .ok()
            .filter(|b| b != "HEAD")
            .unwrap_or_else(|| trunk.clone())
    };

    // Generate or use provided branch name
    let branch = args.branch.unwrap_or_else(|| {
        util::generate_unique_branch_name(|n| git::branch_exists(n).unwrap_or(false))
    });

    // Worktree path
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(&branch);

    // Create workspace directory if needed
    std::fs::create_dir_all(&wt_dir).map_err(|e| Error::Other(e.to_string()))?;

    git::create_worktree(&wt_path, &branch, &base_branch)?;

    // Get base commit for metadata
    let base_commit = git::current_commit().unwrap_or_default();

    // Create metadata
    // 仅当 base_branch ≠ trunk 时才持久化，避免冗余
    let mut meta = WorktreeMeta::new(base_commit, trunk.clone());
    // with_base_branch 会 move base_branch，先保存用于日志输出
    let base_display = base_branch.clone();
    if base_branch != trunk {
        meta = meta.with_base_branch(base_branch);
    }
    if let Some(ref cmd) = args.snap {
        meta = meta.with_snap(cmd.clone());
    }

    let meta_path = meta::meta_path(&wt_dir, &branch);
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
            write_path_file_lines(path_file, &[&wt_path.display().to_string(), &cmd])?;
        } else {
            return Err(Error::Other(
                "Snap mode requires shell integration. Run 'wt setup' first.".into(),
            ));
        }
        return Ok(());
    }

    // Write path for shell integration
    if path_file.is_some() {
        write_path_file(path_file, &wt_path)?;
    } else {
        eprintln!("Created worktree: {branch} (from {base_display})");
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
    let overrides = builder.build().map_err(|e| Error::Other(e.to_string()))?;

    // Walk directory with overrides (only matching files)
    let walker = WalkBuilder::new(from)
        .overrides(overrides)
        .standard_filters(false) // Don't apply .gitignore
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path.is_file() {
            let rel = match path.strip_prefix(from) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to strip prefix for {}: {e}",
                        path.display()
                    );
                    continue;
                }
            };
            let dest = to.join(rel);

            if let Some(parent) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "Warning: failed to create directory {}: {e}",
                        parent.display()
                    );
                    continue;
                }
            }

            if let Err(e) = std::fs::copy(path, &dest) {
                eprintln!("Warning: failed to copy {}: {e}", rel.display());
            }
        }
    }

    Ok(())
}
