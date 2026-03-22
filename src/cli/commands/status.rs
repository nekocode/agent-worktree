// ===========================================================================
// wt status - Show current worktree information
// ===========================================================================

use crate::cli::{Error, Result};
use crate::config::Config;
use crate::git;
use crate::meta::{self, WorktreeMeta};

pub fn run(config: &Config) -> Result<()> {
    let current = git::current_branch()?;
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(&current);

    if !wt_path.exists() {
        return Err(Error::Other(format!(
            "Not in a managed worktree (branch: {current})"
        )));
    }

    let trunk = config.resolve_trunk();

    let meta_path = meta::meta_path_with_fallback(&wt_dir, &current);
    let loaded = WorktreeMeta::load(&meta_path).ok();

    let base_branch = loaded.as_ref().and_then(|m| m.base_branch.as_deref());
    let effective_target = meta::resolve_target_branch(
        None,
        base_branch,
        |b| git::branch_exists(b).unwrap_or(false),
        &trunk,
    );

    let uncommitted = git::uncommitted_count_in(&wt_path).unwrap_or(0);
    let commits = git::commit_count(&effective_target, &current).unwrap_or(0);

    let diff = git::diff_shortstat(&effective_target, &current).unwrap_or(git::DiffStat {
        insertions: 0,
        deletions: 0,
    });
    let unstaged = git::diff_shortstat_in(&wt_path).unwrap_or(git::DiffStat {
        insertions: 0,
        deletions: 0,
    });

    println!("Branch:       {current}");

    if let Some(bb) = base_branch {
        println!("Base branch:  {bb}");
    }

    println!("Trunk:        {trunk}");
    println!("Merge target: {effective_target}");

    if let Some(ref m) = loaded {
        println!(
            "Created:      {}",
            m.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        );
        if let Some(ref cmd) = m.snap_command {
            println!("Snap command: {cmd}");
        }
    }

    println!("Commits:      {commits}");
    println!("Uncommitted:  {uncommitted}");

    let total_ins = diff.insertions + unstaged.insertions;
    let total_del = diff.deletions + unstaged.deletions;
    if total_ins > 0 || total_del > 0 {
        println!("Diff:         +{total_ins} -{total_del}");
    } else {
        println!("Diff:         -");
    }

    println!("Path:         {}", wt_path.display());

    Ok(())
}
