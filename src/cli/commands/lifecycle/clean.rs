// ===========================================================================
// wt clean - Clean up worktrees with no diff from trunk
// ===========================================================================

use std::collections::HashSet;
use std::path::Path;

use clap::Args;

use crate::cli::{write_path_file, Result};
use crate::config::Config;
use crate::git;
use crate::meta;

#[derive(Args)]
pub struct CleanArgs {
    /// Preview which worktrees would be cleaned without removing them
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(args: CleanArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    // Get main repo path before any operations
    let main_path = git::repo_root()?;
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);

    if !wt_dir.exists() {
        eprintln!("No worktrees to clean.");
        return Ok(());
    }

    let trunk = config.resolve_trunk();
    let known_branches: HashSet<String> = git::local_branches()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let worktrees = git::list_worktrees()?;
    let mut cleaned = 0;
    let mut checked = 0;
    let mut cleaned_current = false;

    for wt in worktrees {
        if !wt.path.starts_with(&wt_dir) {
            continue;
        }

        let Some(branch) = wt.branch.as_ref() else {
            continue;
        };

        // Skip trunk
        if branch == &trunk {
            continue;
        }

        checked += 1;

        let target = meta::resolve_effective_target(
            &wt_dir,
            branch,
            None,
            |b| known_branches.contains(b),
            &trunk,
        );

        // Check if worktree has no diff from target
        if !git::has_diff_from(branch, &target).unwrap_or(true) {
            if args.dry_run {
                eprintln!("Would clean (no diff from {target}): {branch}");
                cleaned += 1;
                continue;
            }

            // Check if user is currently inside this worktree
            let inside = git::is_cwd_inside(&wt.path);

            eprintln!("Cleaning worktree (no diff from {target}): {branch}");

            if let Err(e) = git::remove_worktree(&wt.path, false) {
                eprintln!("Warning: failed to remove worktree {branch}: {e}");
                continue;
            }

            // Switch to main repo before deleting branch
            std::env::set_current_dir(&main_path).ok();
            git::delete_branch(branch, false).ok();

            crate::meta::remove_meta(&wt_dir, branch);

            cleaned += 1;

            if inside {
                cleaned_current = true;
            }
        }
    }

    let verb = if args.dry_run { "would be cleaned" } else { "cleaned" };

    if checked == 0 {
        eprintln!("No worktrees to clean.");
    } else if cleaned == 0 {
        eprintln!("No worktrees to clean (all have changes).");
    } else {
        eprintln!("{cleaned} worktree(s) {verb}.");
    }

    // Write main repo path for shell to cd if we were inside a cleaned worktree
    if !args.dry_run && path_file.is_some() && cleaned_current {
        write_path_file(path_file, &main_path)?;
    }

    Ok(())
}
