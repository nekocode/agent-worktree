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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // =========================================================================
    // Error tests
    // =========================================================================
    #[test]
    fn test_error_display() {
        let err = Error::HookFailed("npm install".to_string());
        assert_eq!(err.to_string(), "hook 'npm install' failed");
    }

    // =========================================================================
    // run_interactive tests
    // =========================================================================
    #[test]
    fn test_run_interactive_success() {
        let dir = tempdir().unwrap();
        let result = run_interactive("true", dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_failure() {
        let dir = tempdir().unwrap();
        let result = run_interactive("false", dir.path());
        assert!(result.is_ok());
        assert!(!result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_echo() {
        let dir = tempdir().unwrap();
        let result = run_interactive("echo hello", dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_with_cwd() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        // Test that cwd is respected
        let result = run_interactive("test -f test.txt", dir.path());
        assert!(result.is_ok());
        assert!(result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_nonexistent_cwd() {
        let nonexistent = std::path::Path::new("/nonexistent/path/12345");
        let result = run_interactive("true", nonexistent);
        assert!(result.is_err());
    }

    // =========================================================================
    // run_hook tests
    // =========================================================================
    #[test]
    fn test_run_hook_success() {
        let dir = tempdir().unwrap();
        let result = run_hook("true", dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_hook_failure() {
        let dir = tempdir().unwrap();
        let result = run_hook("false", dir.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::HookFailed(cmd) => assert_eq!(cmd, "false"),
            _ => panic!("Expected HookFailed error"),
        }
    }

    #[test]
    fn test_run_hook_creates_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("hook_created.txt");

        let cmd = format!("echo test > {}", file_path.display());
        let result = run_hook(&cmd, dir.path());
        assert!(result.is_ok());
        assert!(file_path.exists());
    }

    // =========================================================================
    // run_hooks tests
    // =========================================================================
    #[test]
    fn test_run_hooks_empty() {
        let dir = tempdir().unwrap();
        let hooks: Vec<String> = vec![];
        let result = run_hooks(&hooks, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_hooks_single() {
        let dir = tempdir().unwrap();
        let hooks = vec!["true".to_string()];
        let result = run_hooks(&hooks, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_hooks_multiple() {
        let dir = tempdir().unwrap();
        let hooks = vec![
            "true".to_string(),
            "echo hello".to_string(),
            "true".to_string(),
        ];
        let result = run_hooks(&hooks, dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_hooks_stops_on_failure() {
        let dir = tempdir().unwrap();
        let file1 = dir.path().join("file1.txt");
        let file2 = dir.path().join("file2.txt");

        let hooks = vec![
            format!("touch {}", file1.display()),
            "false".to_string(), // This will fail
            format!("touch {}", file2.display()),
        ];

        let result = run_hooks(&hooks, dir.path());
        assert!(result.is_err());
        assert!(file1.exists()); // First hook ran
        assert!(!file2.exists()); // Third hook didn't run
    }

    #[test]
    fn test_run_hooks_sequential_order() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("order.txt");

        let hooks = vec![
            format!("echo one >> {}", file.display()),
            format!("echo two >> {}", file.display()),
            format!("echo three >> {}", file.display()),
        ];

        let result = run_hooks(&hooks, dir.path());
        assert!(result.is_ok());

        let content = std::fs::read_to_string(&file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines, vec!["one", "two", "three"]);
    }
}
