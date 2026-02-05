// ===========================================================================
// wt ls - List worktrees
// ===========================================================================

use crate::cli::Result;
use crate::config::Config;
use crate::git;

pub fn run(config: &Config) -> Result<()> {
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);

    if !wt_dir.exists() {
        eprintln!("No worktrees for this project.");
        return Ok(());
    }

    // Get all worktrees from git
    let worktrees = git::list_worktrees()?;

    // Filter to our managed worktrees
    let managed: Vec<_> = worktrees
        .iter()
        .filter(|wt| wt.path.starts_with(&wt_dir))
        .collect();

    if managed.is_empty() {
        eprintln!("No worktrees for this project.");
        return Ok(());
    }

    // Get current branch for highlighting
    let current = git::current_branch().ok();
    let home = dirs::home_dir();

    println!("  {:<18} PATH", "BRANCH");
    println!("{}", "-".repeat(60));

    for wt in managed {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = current.as_deref() == Some(branch);
        let marker = if is_current { "* " } else { "  " };

        // Replace home dir with ~ for shorter display
        let path_display = match &home {
            Some(h) if wt.path.starts_with(h) => {
                format!("~/{}", wt.path.strip_prefix(h).unwrap().display())
            }
            _ => wt.path.display().to_string(),
        };

        println!("{}{:<18} {}", marker, branch, path_display);
    }

    Ok(())
}
