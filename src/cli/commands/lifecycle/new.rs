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
    let workspace_dir = config.workspaces_dir.join(&workspace_id);

    // Nested snap stacks two loops in the parent shell and breaks cwd tracking
    // when the inner one finishes.
    if args.snap.is_some() && git::is_cwd_inside(&workspace_dir) {
        return Err(Error::Other(
            "Refusing to start snap mode inside an existing worktree.\n\
             Run 'wt cd' to return to the main repo, then retry."
                .into(),
        ));
    }

    // Determine trunk branch
    let trunk = config.resolve_trunk();

    // Resolve base branch: --base flag > current branch > trunk.
    // Determines both the checkout starting point and the default merge/sync target.
    let base_branch = if let Some(ref b) = args.base {
        if !git::branch_exists(b)? {
            return Err(Error::Other(format!("Branch '{b}' does not exist")));
        }
        b.clone()
    } else {
        // Detached HEAD falls back to trunk.
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
    let wt_dir = &workspace_dir;
    let wt_path = wt_dir.join(&branch);

    // Create workspace directory if needed
    std::fs::create_dir_all(wt_dir).map_err(|e| Error::Other(e.to_string()))?;

    git::create_worktree(&wt_path, &branch, &base_branch)?;

    let meta = WorktreeMeta::new(base_branch);
    let meta_path = meta::meta_path(wt_dir, &branch);
    meta.save(&meta_path)
        .map_err(|e| Error::Other(e.to_string()))?;

    // Copy files from main repo
    copy_files(&repo_root, &wt_path, config)?;

    // Run post_create hooks. On failure, leave the worktree in place — the
    // user usually wants to fix the hook (e.g. install missing tool) and
    // resume manually rather than have us silently rm a half-created tree.
    if !config.hooks.post_create.is_empty() {
        eprintln!("Running post-create hooks...");
        let env = process::HookEnv {
            main_repo: &repo_root,
            worktree: &wt_path,
            branch: &branch,
            base_branch: &meta.base_branch,
        };
        if let Err(e) = process::run_hooks(&config.hooks.post_create, &wt_path, &env) {
            eprintln!();
            eprintln!("post_create hook failed: {e}");
            eprintln!("Worktree '{branch}' was created at: {}", wt_path.display());
            eprintln!("Fix the hook and `cd` in manually, or run 'wt rm {branch}' to discard.");
            return Err(Error::Other(format!("post_create hook failed: {e}")));
        }
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
        eprintln!("Created worktree: {branch} (from {})", meta.base_branch);
        eprintln!("Path: {}", wt_path.display());
    }

    Ok(())
}

/// Reject patterns that could escape the repo root.
///
/// Without this guard, a malicious `.agent-worktree.toml` could exfiltrate
/// host files into the worktree via `/abs/path` or `..` traversal — the
/// downstream `strip_prefix` would silently skip mismatches.
fn validate_copy_pattern(pattern: &str) -> Result<()> {
    if pattern.starts_with('/') {
        return Err(Error::Other(format!(
            "copy_files pattern '{pattern}' cannot start with '/' (absolute path)"
        )));
    }
    if pattern.split(['/', '\\']).any(|seg| seg == "..") {
        return Err(Error::Other(format!(
            "copy_files pattern '{pattern}' cannot contain '..'"
        )));
    }
    Ok(())
}

fn copy_files(from: &Path, to: &Path, config: &Config) -> Result<()> {
    use ignore::overrides::OverrideBuilder;
    use ignore::WalkBuilder;

    if config.copy_files.is_empty() {
        return Ok(());
    }

    for pattern in &config.copy_files {
        validate_copy_pattern(pattern)?;
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

    // follow_links=false: a symlink in the repo could otherwise pull files
    // from outside the repo into the worktree.
    let walker = WalkBuilder::new(from)
        .overrides(overrides)
        .standard_filters(false)
        .follow_links(false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_copy_pattern_accepts_relative_glob() {
        assert!(validate_copy_pattern(".env").is_ok());
        assert!(validate_copy_pattern(".env.*").is_ok());
        assert!(validate_copy_pattern("config/*.toml").is_ok());
        assert!(validate_copy_pattern("**/.secret").is_ok());
    }

    #[test]
    fn validate_copy_pattern_rejects_absolute_path() {
        let err = validate_copy_pattern("/etc/passwd").unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn validate_copy_pattern_rejects_parent_traversal() {
        let err = validate_copy_pattern("../secrets").unwrap_err();
        assert!(err.to_string().contains(".."));

        let err = validate_copy_pattern("config/../../etc/passwd").unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn validate_copy_pattern_rejects_backslash_traversal() {
        // Windows-style path separator should still be rejected.
        let err = validate_copy_pattern("..\\secrets").unwrap_err();
        assert!(err.to_string().contains(".."));
    }
}
