// ===========================================================================
// wt sync - Sync current worktree with trunk
// ===========================================================================

use clap::{Args, ValueEnum};

use crate::cli::{Error, Result};
use crate::config::{Config, SyncStrategy};
use crate::git;

#[derive(Args)]
pub struct SyncArgs {
    /// Sync strategy (default: rebase)
    #[arg(short, long, value_enum)]
    strategy: Option<SyncStrategyArg>,

    /// Continue sync after resolving conflicts
    #[arg(long)]
    r#continue: bool,

    /// Abort sync and restore previous state
    #[arg(long)]
    abort: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum SyncStrategyArg {
    Rebase,
    Merge,
}

impl From<SyncStrategyArg> for SyncStrategy {
    fn from(arg: SyncStrategyArg) -> Self {
        match arg {
            SyncStrategyArg::Rebase => SyncStrategy::Rebase,
            SyncStrategyArg::Merge => SyncStrategy::Merge,
        }
    }
}

pub fn run(args: SyncArgs, config: &Config) -> Result<()> {
    if args.abort {
        if git::is_rebase_in_progress() {
            eprintln!("Aborting rebase...");
            git::rebase_abort()?;
            eprintln!("Rebase aborted.");
        } else if git::is_merge_in_progress() {
            eprintln!("Aborting merge...");
            git::merge_abort()?;
            eprintln!("Merge aborted.");
        } else {
            return Err(Error::Other("No sync in progress to abort".into()));
        }
        return Ok(());
    }

    if args.r#continue {
        if git::is_rebase_in_progress() {
            eprintln!("Continuing rebase...");
            git::rebase_continue()?;
            eprintln!("Rebase continued.");
        } else if git::is_merge_in_progress() {
            eprintln!("Continuing merge...");
            git::merge_continue()?;
            eprintln!("Merge continued.");
        } else {
            return Err(Error::Other("No sync in progress to continue".into()));
        }
        return Ok(());
    }

    let current = git::current_branch()?;
    let trunk = config
        .trunk
        .clone()
        .unwrap_or_else(|| git::detect_trunk().unwrap_or_else(|_| "main".into()));

    if current == trunk {
        return Err(Error::Other("Already on trunk branch".into()));
    }

    let strategy = args.strategy.map(SyncStrategy::from).unwrap_or_default();

    eprintln!("Syncing {current} with {trunk} ({strategy:?})...");

    match strategy {
        SyncStrategy::Rebase => {
            git::rebase(&trunk)?;
            eprintln!("Rebased onto {trunk}");
        }
        SyncStrategy::Merge => {
            git::merge(&trunk, false, false, None)?;
            eprintln!("Merged {trunk} into {current}");
        }
    }

    Ok(())
}
