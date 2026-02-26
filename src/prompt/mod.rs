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

/// 通用选择提示：显示选项，读取用户输入，解析结果
fn read_choice<T>(lines: &[&str], prompt_text: &str, parse: fn(&str) -> Option<T>) -> Result<T> {
    use std::io::{self, Write};
    eprintln!();
    for line in lines {
        eprintln!("{line}");
    }
    eprintln!();
    eprint!("{prompt_text}");
    io::stderr().flush().map_err(Error::Io)?;
    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(Error::Io)?;
    parse(&input).ok_or(Error::Cancelled)
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
