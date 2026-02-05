use agent_worktree::cli::Cli;
use agent_worktree::update;
use clap::Parser;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Check for updates (10% chance)
    if update::should_check() {
        check_for_update();
    }

    let cli = Cli::parse();
    if let Err(e) = cli.run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn check_for_update() {
    // Run in background thread to not block CLI
    std::thread::spawn(|| {
        if let Ok(Some(latest)) = update::check_update(VERSION) {
            eprintln!(
                "\x1b[33mA new version of agent-worktree is available: {} -> {}\x1b[0m",
                VERSION, latest
            );
            eprintln!("\x1b[33mRun `npm install -g agent-worktree` to update\x1b[0m");
            eprintln!();
        }
    });
    // Give the thread a moment to print before CLI output
    std::thread::sleep(std::time::Duration::from_millis(50));
}
