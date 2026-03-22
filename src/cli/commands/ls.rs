// ===========================================================================
// wt ls - List worktrees with git status info
// ===========================================================================

use std::collections::HashSet;

use clap::Args;

use chrono::{DateTime, Utc};

use crate::cli::Result;
use crate::config::Config;
use crate::git;
use crate::meta;

#[derive(Args)]
pub struct LsArgs {
    /// Show full path for each worktree
    #[arg(short, long)]
    pub long: bool,
}

pub fn run(args: LsArgs, config: &Config) -> Result<()> {
    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);

    if !wt_dir.exists() {
        eprintln!("No worktrees for this project.");
        return Ok(());
    }

    let worktrees = git::list_worktrees()?;

    let managed: Vec<_> = worktrees
        .iter()
        .filter(|wt| wt.path.starts_with(&wt_dir))
        .collect();

    if managed.is_empty() {
        eprintln!("No worktrees for this project.");
        return Ok(());
    }

    let trunk = config.resolve_trunk();
    // 一次性获取所有本地分支，避免 N 次 subprocess
    let known_branches: HashSet<String> = git::local_branches()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let current = git::current_branch().ok();
    let home = dirs::home_dir();

    let mut rows: Vec<Row> = Vec::new();
    for wt in &managed {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = current.as_deref() == Some(branch);

        let meta_path = meta::meta_path_with_fallback(&wt_dir, branch);
        let loaded_meta = meta::WorktreeMeta::load(&meta_path).ok();

        let base_branch = loaded_meta.as_ref().and_then(|m| m.base_branch.clone());
        let created_at = loaded_meta.as_ref().map(|m| m.created_at);

        let effective_target = meta::resolve_target_branch(
            None,
            base_branch.as_deref(),
            |b| known_branches.contains(b),
            &trunk,
        );

        let uncommitted = git::uncommitted_count_in(&wt.path).unwrap_or(0);
        let commits = git::commit_count(&effective_target, branch).unwrap_or(0);

        let c = git::diff_shortstat(&effective_target, branch)
            .unwrap_or(git::DiffStat { insertions: 0, deletions: 0 });
        let u = git::diff_shortstat_in(&wt.path)
            .unwrap_or(git::DiffStat { insertions: 0, deletions: 0 });

        let path = if args.long {
            Some(shorten_path(&wt.path, &home))
        } else {
            None
        };

        rows.push(Row {
            branch: branch.to_string(),
            base_branch,
            is_current,
            uncommitted,
            commits,
            insertions: c.insertions + u.insertions,
            deletions: c.deletions + u.deletions,
            path,
            created_at,
        });
    }

    // 按创建时间排序（新的在前），无 meta 的排最后
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at));

    print_table(&rows);
    Ok(())
}

struct Row {
    branch: String,
    base_branch: Option<String>,
    is_current: bool,
    uncommitted: usize,
    commits: usize,
    insertions: usize,
    deletions: usize,
    path: Option<String>,
    created_at: Option<DateTime<Utc>>,
}

fn print_table(rows: &[Row]) {
    let bw = rows.iter().map(|r| r.branch.len()).max().unwrap_or(6).max(6);
    let show_path = rows.iter().any(|r| r.path.is_some());
    let show_base = rows.iter().any(|r| r.base_branch.is_some());

    let sw = if show_base {
        rows.iter()
            .filter_map(|r| r.base_branch.as_ref().map(|s| s.len()))
            .max()
            .unwrap_or(6)
            .max(6)
    } else {
        0
    };

    // 表头
    let mut header = format!("  {:<bw$}", "BRANCH", bw = bw);
    if show_base {
        header.push_str(&format!("   {:<sw$}", "BASE", sw = sw));
    }
    header.push_str(&format!("   {:>8}   {:>7}   {:>10}", "UNCOMMIT", "COMMITS", "DIFF"));
    if show_path {
        header.push_str("   PATH");
    }
    println!("{header}");

    let sep_len = 2 + bw + 3 + 8 + 3 + 7 + 3 + 10
        + if show_base { 3 + sw } else { 0 }
        + if show_path { 40 } else { 0 };
    println!("{}", "-".repeat(sep_len));

    for row in rows {
        let marker = if row.is_current { "* " } else { "  " };

        let diff = if row.insertions == 0 && row.deletions == 0 {
            "-".to_string()
        } else {
            format!("+{} -{}", row.insertions, row.deletions)
        };

        let mut line = format!("{}{:<bw$}", marker, row.branch, bw = bw);
        if show_base {
            let src = row.base_branch.as_deref().unwrap_or("-");
            line.push_str(&format!("   {:<sw$}", src, sw = sw));
        }
        line.push_str(&format!("   {:>8}   {:>7}   {:>10}", row.uncommitted, row.commits, diff));

        if let Some(ref path) = row.path {
            println!("{line}   {path}");
        } else {
            println!("{line}");
        }
    }
}

fn shorten_path(path: &std::path::Path, home: &Option<std::path::PathBuf>) -> String {
    match home {
        Some(h) if path.starts_with(h) => {
            format!("~/{}", path.strip_prefix(h).unwrap().display())
        }
        _ => path.display().to_string(),
    }
}
