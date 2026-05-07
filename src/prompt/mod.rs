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

/// Choice for snap mode when worktree has committed changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapMergeChoice {
    Merge,
    Exit,
}

/// Parse single character input to merge choice
pub fn parse_snap_merge_choice(input: &str) -> Option<SnapMergeChoice> {
    match input.trim().to_lowercase().as_str() {
        "m" => Some(SnapMergeChoice::Merge),
        "q" => Some(SnapMergeChoice::Exit),
        _ => None,
    }
}

/// Bound on retries. Prevents infinite loops when stdin is
/// non-interactive and `read_line` returns 0 bytes (EOF) repeatedly.
const MAX_PROMPT_ATTEMPTS: usize = 5;

/// Show options, read user input, parse to a typed choice.
///
/// Re-prompts on unrecognized input rather than treating a stray Enter or
/// typo as cancellation — silent default-to-Exit during a snap loop is too
/// destructive to be the failure mode for a wrong keystroke.
fn read_choice<T>(lines: &[&str], prompt_text: &str, parse: fn(&str) -> Option<T>) -> Result<T> {
    use std::io::{self, Write};
    eprintln!();
    for line in lines {
        eprintln!("{line}");
    }
    eprintln!();
    for _ in 0..MAX_PROMPT_ATTEMPTS {
        eprint!("{prompt_text}");
        io::stderr().flush().map_err(Error::Io)?;
        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input).map_err(Error::Io)?;
        // EOF (Ctrl+D, closed stdin, non-interactive caller): give up.
        if bytes == 0 {
            return Err(Error::Cancelled);
        }
        if let Some(choice) = parse(&input) {
            return Ok(choice);
        }
        eprintln!("Invalid input. Please choose one of the options shown.");
    }
    Err(Error::Cancelled)
}

pub fn snap_merge_prompt() -> Result<SnapMergeChoice> {
    read_choice(
        &[
            "Worktree has committed changes (not yet merged).",
            "",
            "  [m] Merge into trunk",
            "  [q] Exit snap mode",
        ],
        "[m/q]: ",
        parse_snap_merge_choice,
    )
}

pub fn snap_exit_prompt() -> Result<SnapExitChoice> {
    read_choice(
        &[
            "Worktree has uncommitted changes.",
            "",
            "  [r] Reopen agent (let agent commit)",
            "  [q] Exit snap mode (commit manually)",
        ],
        "[r/q]: ",
        parse_snap_choice,
    )
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

    // -----------------------------------------------------------------------
    // SnapMergeChoice tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_snap_merge_choice_m() {
        assert_eq!(parse_snap_merge_choice("m"), Some(SnapMergeChoice::Merge));
        assert_eq!(parse_snap_merge_choice("M"), Some(SnapMergeChoice::Merge));
        assert_eq!(parse_snap_merge_choice(" m "), Some(SnapMergeChoice::Merge));
    }

    #[test]
    fn test_parse_snap_merge_choice_q() {
        assert_eq!(parse_snap_merge_choice("q"), Some(SnapMergeChoice::Exit));
        assert_eq!(parse_snap_merge_choice("Q"), Some(SnapMergeChoice::Exit));
        assert_eq!(parse_snap_merge_choice(" q\n"), Some(SnapMergeChoice::Exit));
    }

    #[test]
    fn test_parse_snap_merge_choice_invalid() {
        assert_eq!(parse_snap_merge_choice(""), None);
        assert_eq!(parse_snap_merge_choice("x"), None);
        assert_eq!(parse_snap_merge_choice("merge"), None);
    }
}
