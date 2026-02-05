// ===========================================================================
// cli - Command Line Interface
// ===========================================================================

mod commands;

use clap::{Parser, Subcommand};

use crate::config::Config;

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

    /// Print path only (for shell integration)
    #[arg(long, global = true, hide = true)]
    print_path: bool,
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
}

impl Cli {
    pub fn run(self) -> Result<()> {
        let config = Config::load()?;

        match self.command {
            Command::New(args) => commands::new::run(args, &config, self.print_path),
            Command::Ls => commands::ls::run(&config),
            Command::Cd(args) => commands::cd::run(args, &config, self.print_path),
            Command::Main => commands::main::run(&config, self.print_path),
            Command::Rm(args) => commands::rm::run(args, &config, self.print_path),
            Command::Clean => commands::clean::run(&config, self.print_path),
            Command::Merge(args) => commands::merge::run(args, &config, self.print_path),
            Command::Sync(args) => commands::sync::run(args, &config),
            Command::Mv(args) => commands::r#move::run(args, &config, self.print_path),
            Command::Setup(args) => commands::setup::run(args),
            Command::Init(args) => commands::init::run(args),
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
    fn test_cli_parse_with_print_path() {
        let cli = Cli::try_parse_from(["wt", "--print-path", "main"]);
        assert!(cli.is_ok());
        let cli = cli.unwrap();
        assert!(cli.print_path);
    }
}
