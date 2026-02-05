// ===========================================================================
// wt main - Return to main repository
// ===========================================================================

use std::path::Path;

use crate::cli::{write_path_file, Result};
use crate::config::Config;
use crate::git;

pub fn run(_config: &Config, path_file: Option<&Path>) -> Result<()> {
    let repo_root = git::repo_root()?;

    if path_file.is_some() {
        write_path_file(path_file, &repo_root)?;
    } else {
        eprintln!("Returning to main repo");
    }

    Ok(())
}
