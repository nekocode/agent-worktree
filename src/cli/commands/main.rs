// ===========================================================================
// wt main - Return to main repository
// ===========================================================================

use crate::cli::Result;
use crate::config::Config;
use crate::git;

pub fn run(_config: &Config, print_path: bool) -> Result<()> {
    let repo_root = git::repo_root()?;

    if print_path {
        println!("{}", repo_root.display());
    } else {
        eprintln!("Returning to main repo");
        println!("{}", repo_root.display());
    }

    Ok(())
}
