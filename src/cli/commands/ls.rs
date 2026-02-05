// ===========================================================================
// wt ls - List worktrees
// ===========================================================================

use crate::cli::Result;
use crate::config::Config;
use crate::git;

pub fn run(config: &Config) -> Result<()> {
    let repo_name = git::repo_name()?;
    let wt_dir = config.workspaces_dir.join(&repo_name);

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

    println!("{:<20} {:<12} PATH", "BRANCH", "STATUS");
    println!("{}", "-".repeat(60));

    for wt in managed {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = current.as_deref() == Some(branch);

        let status = if is_current { "* current" } else { "" };

        let marker = if is_current { "* " } else { "  " };

        println!(
            "{}{:<18} {:<12} {}",
            marker,
            branch,
            status,
            wt.path.display()
        );
    }

    Ok(())
}
