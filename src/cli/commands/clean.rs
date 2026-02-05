// ===========================================================================
// wt clean - Clean up worktrees with no diff from trunk
// ===========================================================================

use crate::cli::Result;
use crate::config::Config;
use crate::git;

pub fn run(config: &Config, print_path: bool) -> Result<()> {
    // Get main repo path before any operations
    let main_path = git::repo_root()?;
    let repo_name = git::repo_name()?;
    let wt_dir = config.workspaces_dir.join(&repo_name);

    if !wt_dir.exists() {
        eprintln!("No worktrees to clean.");
        return Ok(());
    }

    let trunk = config
        .trunk
        .clone()
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    let worktrees = git::list_worktrees()?;
    let mut cleaned = 0;
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

        // Check if worktree has no diff from trunk
        if !git::has_diff_from(branch, &trunk).unwrap_or(true) {
            // Check if user is currently inside this worktree
            let inside = git::is_cwd_inside(&wt.path);

            eprintln!("Cleaning worktree (no diff from {trunk}): {branch}");

            git::remove_worktree(&wt.path, false).ok();

            // Switch to main repo before deleting branch
            std::env::set_current_dir(&main_path).ok();
            git::delete_branch(branch, false).ok();

            let meta_path = wt_dir.join(format!("{branch}.status.toml"));
            std::fs::remove_file(meta_path).ok();

            cleaned += 1;

            if inside {
                cleaned_current = true;
            }
        }
    }

    if cleaned == 0 {
        eprintln!("No worktrees to clean (all have changes).");
    } else {
        eprintln!("Cleaned {cleaned} worktree(s).");
    }

    // Output main repo path for shell to cd if we were inside a cleaned worktree
    if print_path && cleaned_current {
        println!("{}", main_path.display());
    }

    Ok(())
}
