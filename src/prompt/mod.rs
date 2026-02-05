// ===========================================================================
// prompt - Interactive User Input
// ===========================================================================

use dialoguer::{Confirm, Select};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("user cancelled")]
    Cancelled,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Ask for confirmation
pub fn confirm(message: &str) -> Result<bool> {
    Confirm::new()
        .with_prompt(message)
        .default(false)
        .interact()
        .map_err(|_| Error::Cancelled)
}

/// Present options after agent exits with uncommitted changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapExitChoice {
    Commit,
    Reopen,
    Discard,
}

pub fn snap_exit_prompt() -> Result<SnapExitChoice> {
    let items = &[
        "[c] Commit changes and merge",
        "[r] Reopen agent to continue",
        "[x] Discard changes and exit",
    ];

    let selection = Select::new()
        .with_prompt("Worktree has uncommitted changes")
        .items(items)
        .default(0)
        .interact()
        .map_err(|_| Error::Cancelled)?;

    Ok(match selection {
        0 => SnapExitChoice::Commit,
        1 => SnapExitChoice::Reopen,
        _ => SnapExitChoice::Discard,
    })
}
