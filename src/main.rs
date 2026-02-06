use agent_worktree::cli::Cli;
use agent_worktree::config::Config;
use agent_worktree::update;
use clap::Parser;
use std::thread::JoinHandle;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    // Check for updates (once per day), runs in background
    let base_dir = Config::base_dir().ok();
    let update_handle = base_dir.as_ref().and_then(|dir| {
        if update::should_check(dir) {
            Some(spawn_update_check(dir.clone()))
        } else {
            None
        }
    });

    let cli = Cli::parse();
    let result = cli.run();

    // Wait for update check to complete before exiting
    if let Some(handle) = update_handle {
        let _ = handle.join();
    }

    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn spawn_update_check(base_dir: std::path::PathBuf) -> JoinHandle<()> {
    std::thread::spawn(move || {
        if let Ok(Some(latest)) = update::check_update(VERSION) {
            eprintln!(
                "\x1b[33mA new version of agent-worktree is available: {} -> {}\x1b[0m",
                VERSION, latest
            );
            eprintln!("\x1b[33mRun `wt update` to update\x1b[0m");
        }
        // Mark that we checked (ignore errors)
        let _ = update::mark_checked(&base_dir);
    })
}
