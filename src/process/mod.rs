// ===========================================================================
// process - External Process Management (Agents & Hooks)
// ===========================================================================

use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to spawn process: {0}")]
    Spawn(#[from] std::io::Error),

    #[error("process failed with status: {0}")]
    Failed(ExitStatus),

    #[error("hook '{0}' failed")]
    HookFailed(String),
}

/// Run a command in the specified directory, inheriting stdio
pub fn run_interactive(command: &str, cwd: &Path) -> Result<ExitStatus> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", command])
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?
    } else {
        Command::new("sh")
            .args(["-c", command])
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()?
    };

    Ok(status)
}

/// Run a hook command
pub fn run_hook(command: &str, cwd: &Path) -> Result<()> {
    let status = run_interactive(command, cwd)?;

    if !status.success() {
        return Err(Error::HookFailed(command.to_string()));
    }

    Ok(())
}

/// Run multiple hooks in sequence
pub fn run_hooks(hooks: &[String], cwd: &Path) -> Result<()> {
    for hook in hooks {
        eprintln!("Running hook: {hook}");
        run_hook(hook, cwd)?;
    }
    Ok(())
}
