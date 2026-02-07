// ===========================================================================
// wt ls - List worktrees with git status info
// ===========================================================================

use clap::Args;

use crate::cli::Result;
use crate::config::Config;
use crate::git;

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

    let trunk = config
        .trunk
        .clone()
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    let current = git::current_branch().ok();
    let home = dirs::home_dir();

    let mut rows: Vec<Row> = Vec::new();
    for wt in &managed {
        let branch = wt.branch.as_deref().unwrap_or("(detached)");
        let is_current = current.as_deref() == Some(branch);

        let uncommitted = git::uncommitted_count_in(&wt.path).unwrap_or(0);
        let commits = git::commit_count(&trunk, branch).unwrap_or(0);

        // diff = committed + uncommitted
        let c = git::diff_shortstat(&trunk, branch)
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
            is_current,
            uncommitted,
            commits,
            insertions: c.insertions + u.insertions,
            deletions: c.deletions + u.deletions,
            path,
        });
    }

    print_table(&rows);
    Ok(())
}

struct Row {
    branch: String,
    is_current: bool,
    uncommitted: usize,
    commits: usize,
    insertions: usize,
    deletions: usize,
    path: Option<String>,
}

fn print_table(rows: &[Row]) {
    let bw = rows.iter().map(|r| r.branch.len()).max().unwrap_or(6).max(6);
    let show_path = rows.iter().any(|r| r.path.is_some());

    // 表头
    let header = format!(
        "  {:<bw$}   {:>8}   {:>7}   {:>10}",
        "BRANCH", "UNCOMMIT", "COMMITS", "DIFF",
        bw = bw,
    );
    if show_path {
        println!("{header}   PATH");
    } else {
        println!("{header}");
    }

    let sep_len = if show_path { bw + 80 } else { bw + 36 };
    println!("{}", "-".repeat(sep_len));

    for row in rows {
        let marker = if row.is_current { "* " } else { "  " };

        let diff = if row.insertions == 0 && row.deletions == 0 {
            "-".to_string()
        } else {
            format!("+{} -{}", row.insertions, row.deletions)
        };

        let line = format!(
            "{}{:<bw$}   {:>8}   {:>7}   {:>10}",
            marker, row.branch, row.uncommitted, row.commits, diff,
            bw = bw,
        );

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
