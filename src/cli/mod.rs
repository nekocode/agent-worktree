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

    /// Remove all merged worktrees
    Clean,

    /// Merge current worktree into trunk
    Merge(commands::MergeArgs),

    /// Sync current worktree from trunk
    Sync(commands::SyncArgs),

    /// Rename a worktree branch
    Move(commands::MoveArgs),

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
            Command::Move(args) => commands::r#move::run(args, &config, self.print_path),
            Command::Setup(args) => commands::setup::run(args),
            Command::Init(args) => commands::init::run(args),
        }
    }
}
