// ===========================================================================
// prompt - Interactive User Input
// ===========================================================================

use dialoguer::Confirm;

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
    Reopen,
    Exit,
}

/// Parse single character input to choice
pub fn parse_snap_choice(input: &str) -> Option<SnapExitChoice> {
    match input.trim().to_lowercase().as_str() {
        "r" => Some(SnapExitChoice::Reopen),
        "q" => Some(SnapExitChoice::Exit),
        _ => None,
    }
}

pub fn snap_exit_prompt() -> Result<SnapExitChoice> {
    use std::io::{self, Write};

    eprintln!();
    eprintln!("Worktree has uncommitted changes.");
    eprintln!();
    eprintln!("  [r] Reopen agent (let agent commit)");
    eprintln!("  [q] Exit snap mode (commit manually)");
    eprintln!();
    eprint!("[r/q]: ");
    io::stderr().flush().map_err(Error::Io)?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(Error::Io)?;

    parse_snap_choice(&input).ok_or(Error::Cancelled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::Cancelled;
        assert_eq!(err.to_string(), "user cancelled");
    }

    #[test]
    fn test_snap_exit_choice_equality() {
        assert_eq!(SnapExitChoice::Reopen, SnapExitChoice::Reopen);
        assert_eq!(SnapExitChoice::Exit, SnapExitChoice::Exit);
        assert_ne!(SnapExitChoice::Reopen, SnapExitChoice::Exit);
    }

    #[test]
    fn test_snap_exit_choice_clone() {
        let choice = SnapExitChoice::Reopen;
        let cloned = choice;
        assert_eq!(choice, cloned);
    }

    #[test]
    fn test_snap_exit_choice_debug() {
        let choice = SnapExitChoice::Reopen;
        let debug = format!("{:?}", choice);
        assert_eq!(debug, "Reopen");
    }

    #[test]
    fn test_snap_exit_choice_all_variants() {
        let variants = [SnapExitChoice::Reopen, SnapExitChoice::Exit];
        for v in variants {
            let _copy: SnapExitChoice = v;
            assert!(v == v);
        }
    }

    #[test]
    fn test_parse_snap_choice_r() {
        assert_eq!(parse_snap_choice("r"), Some(SnapExitChoice::Reopen));
        assert_eq!(parse_snap_choice("R"), Some(SnapExitChoice::Reopen));
        assert_eq!(parse_snap_choice(" r "), Some(SnapExitChoice::Reopen));
    }

    #[test]
    fn test_parse_snap_choice_q() {
        assert_eq!(parse_snap_choice("q"), Some(SnapExitChoice::Exit));
        assert_eq!(parse_snap_choice("Q"), Some(SnapExitChoice::Exit));
        assert_eq!(parse_snap_choice(" q\n"), Some(SnapExitChoice::Exit));
    }

    #[test]
    fn test_parse_snap_choice_invalid() {
        assert_eq!(parse_snap_choice(""), None);
        assert_eq!(parse_snap_choice("x"), None);
        assert_eq!(parse_snap_choice("c"), None);
        assert_eq!(parse_snap_choice("reopen"), None);
    }
}
