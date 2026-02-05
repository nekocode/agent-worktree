// ===========================================================================
// cli - Command Line Interface
// ===========================================================================

mod commands;

use std::path::Path;

use clap::{Parser, Subcommand};

use crate::config::Config;

/// Write path to file for shell integration
pub fn write_path_file(path_file: Option<&Path>, path: &Path) -> Result<()> {
    if let Some(file) = path_file {
        std::fs::write(file, path.display().to_string())
            .map_err(|e| Error::Other(format!("failed to write path file: {}", e)))?;
    }
    Ok(())
}

/// Write multiple lines to path file (for snap mode)
pub fn write_path_file_lines(path_file: Option<&Path>, lines: &[&str]) -> Result<()> {
    if let Some(file) = path_file {
        std::fs::write(file, lines.join("\n"))
            .map_err(|e| Error::Other(format!("failed to write path file: {}", e)))?;
    }
    Ok(())
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("config error: {0}")]
    Config(#[from] crate::config::Error),

    #[error("git error: {0}")]
    Git(#[from] crate::git::Error),

    #[error("not in a git repository")]
    NotInRepo,

    #[error("{0}")]
    Other(String),
}

#[derive(Parser)]
#[command(
    name = "wt",
    version,
    about = "Git worktree workflow tool for AI agents",
    after_help = "Run 'wt setup' to install shell integration for cd/new/main commands."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Write target path to file (for shell integration)
    #[arg(long, global = true, hide = true, value_name = "FILE")]
    path_file: Option<std::path::PathBuf>,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new worktree and switch to it
    New(commands::NewArgs),

    /// List all worktrees for this project
    Ls,

    /// Switch to a worktree directory
    Cd(commands::CdArgs),

    /// Return to the main repository
    Main,

    /// Remove a worktree and its branch
    Rm(commands::RmArgs),

    /// Remove worktrees with no diff from trunk
    Clean,

    /// Merge current worktree into trunk
    Merge(commands::MergeArgs),

    /// Sync current worktree from trunk
    Sync(commands::SyncArgs),

    /// Rename a worktree branch
    Mv(commands::MoveArgs),

    /// Install shell integration (bash/zsh/fish)
    Setup(commands::SetupArgs),

    /// Create .agent-worktree.toml config file
    Init(commands::InitArgs),

    /// Continue snap mode after agent exits (internal use)
    #[command(hide = true)]
    SnapContinue,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let config = Config::load()?;
        let path_file = self.path_file.as_deref();

        match self.command {
            Command::New(args) => commands::new::run(args, &config, path_file),
            Command::Ls => commands::ls::run(&config),
            Command::Cd(args) => commands::cd::run(args, &config, path_file),
            Command::Main => commands::main::run(&config, path_file),
            Command::Rm(args) => commands::rm::run(args, &config, path_file),
            Command::Clean => commands::clean::run(&config, path_file),
            Command::Merge(args) => commands::merge::run(args, &config, path_file),
            Command::Sync(args) => commands::sync::run(args, &config),
            Command::Mv(args) => commands::r#move::run(args, &config, path_file),
            Command::Setup(args) => commands::setup::run(args),
            Command::Init(args) => commands::init::run(args),
            Command::SnapContinue => commands::snap_continue::run(&config, path_file),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::NotInRepo;
        assert_eq!(err.to_string(), "not in a git repository");

        let err = Error::Other("custom error".to_string());
        assert_eq!(err.to_string(), "custom error");
    }

    #[test]
    fn test_cli_parse_help() {
        // Verify CLI can parse --help without panicking
        let result = Cli::try_parse_from(["wt", "--help"]);
        assert!(result.is_err()); // --help causes early exit
    }

    #[test]
    fn test_cli_parse_version() {
        let result = Cli::try_parse_from(["wt", "--version"]);
        assert!(result.is_err()); // --version causes early exit
    }

    #[test]
    fn test_cli_parse_new() {
        let cli = Cli::try_parse_from(["wt", "new", "feature-branch"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_new_with_base() {
        let cli = Cli::try_parse_from(["wt", "new", "feature", "--base", "develop"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_ls() {
        let cli = Cli::try_parse_from(["wt", "ls"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_cd() {
        let cli = Cli::try_parse_from(["wt", "cd", "branch-name"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_main() {
        let cli = Cli::try_parse_from(["wt", "main"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_rm() {
        let cli = Cli::try_parse_from(["wt", "rm", "branch"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_rm_force() {
        let cli = Cli::try_parse_from(["wt", "rm", "branch", "--force"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_clean() {
        let cli = Cli::try_parse_from(["wt", "clean"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_merge() {
        let cli = Cli::try_parse_from(["wt", "merge"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_merge_with_strategy() {
        let cli = Cli::try_parse_from(["wt", "merge", "--strategy", "squash"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_sync() {
        let cli = Cli::try_parse_from(["wt", "sync"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_mv() {
        let cli = Cli::try_parse_from(["wt", "mv", "old", "new"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_setup() {
        let cli = Cli::try_parse_from(["wt", "setup"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_setup_with_shell() {
        let cli = Cli::try_parse_from(["wt", "setup", "--shell", "bash"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_init() {
        let cli = Cli::try_parse_from(["wt", "init"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_init_with_trunk() {
        let cli = Cli::try_parse_from(["wt", "init", "--trunk", "develop"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_with_path_file() {
        let cli = Cli::try_parse_from(["wt", "--path-file", "/tmp/test", "main"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert_eq!(cli.path_file, Some(std::path::PathBuf::from("/tmp/test")));
    }

    #[test]
    fn test_cli_parse_new_with_snap() {
        let cli = Cli::try_parse_from(["wt", "new", "-s", "claude"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_new_with_snap_long() {
        let cli = Cli::try_parse_from(["wt", "new", "--snap", "claude"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_new_with_snap_and_branch() {
        let cli = Cli::try_parse_from(["wt", "new", "my-branch", "-s", "agent"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_parse_snap_continue() {
        let cli = Cli::try_parse_from(["wt", "snap-continue"]);
        assert!(cli.is_ok());
    }

    #[test]
    fn test_cli_snap_continue_is_hidden() {
        // snap-continue should not appear in help
        let result = Cli::try_parse_from(["wt", "--help"]);
        // --help causes early exit but the command is still valid
        assert!(result.is_err());
    }
}
