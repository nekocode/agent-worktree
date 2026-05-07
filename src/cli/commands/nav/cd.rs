// ===========================================================================
// wt cd - Change to worktree directory
// ===========================================================================

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::cli::{write_path_file, Error, Result};
use crate::complete;
use crate::config::Config;
use crate::git;

#[derive(Args)]
pub struct CdArgs {
    /// Branch name to switch to (omit to return to main repo)
    #[arg(add = ArgValueCompleter::new(complete::complete_worktrees))]
    branch: Option<String>,
}

pub fn run(args: CdArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    // `wt cd` only makes sense behind the shell wrapper — a child process
    // can't change its parent shell's CWD. Without a path_file the wrapper
    // isn't installed (or the binary was invoked directly), so refuse loudly
    // instead of pretending to switch.
    if path_file.is_none() {
        return Err(Error::Other(
            "Shell integration not installed. Run 'wt setup' first.".into(),
        ));
    }

    let Some(branch) = args.branch else {
        let repo_root = git::repo_root()?;
        write_path_file(path_file, &repo_root)?;
        return Ok(());
    };

    let workspace_id = git::workspace_id()?;
    let wt_dir = config.workspaces_dir.join(&workspace_id);
    let wt_path = wt_dir.join(&branch);

    if !wt_path.exists() {
        return Err(Error::Git(git::Error::WorktreeNotFound(branch)));
    }

    write_path_file(path_file, &wt_path)?;
    Ok(())
}
