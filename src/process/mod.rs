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

    #[error("hook '{0}' failed")]
    HookFailed(String),
}

/// Worktree context exposed to hooks as environment variables.
///
/// Lets hooks reference paths portably instead of hardcoding them — e.g.
/// `ln -s "$WT_MAIN_REPO/node_modules" node_modules` to share dependencies
/// without the disk/time cost of `copy_files`.
pub struct HookEnv<'a> {
    /// Main repository root (the common dir, not the worktree).
    pub main_repo: &'a Path,
    /// New worktree's absolute path.
    pub worktree: &'a Path,
    /// Worktree's branch name.
    pub branch: &'a str,
    /// Base branch (creation source for `new`, merge target for `merge`).
    pub base_branch: &'a str,
}

impl HookEnv<'_> {
    fn vars(&self) -> [(&'static str, String); 4] {
        [
            ("WT_MAIN_REPO", self.main_repo.display().to_string()),
            ("WT_WORKTREE", self.worktree.display().to_string()),
            ("WT_BRANCH", self.branch.to_string()),
            ("WT_BASE_BRANCH", self.base_branch.to_string()),
        ]
    }
}

/// Run a command in the specified directory, inheriting stdio.
///
/// `env` is layered on top of the inherited environment, so hooks see the
/// WT_* worktree context alongside the user's normal shell variables.
pub fn run_interactive(command: &str, cwd: &Path, env: &HookEnv) -> Result<ExitStatus> {
    let (shell, flag) = if cfg!(windows) {
        ("cmd", "/C")
    } else {
        ("sh", "-c")
    };
    let status = Command::new(shell)
        .args([flag, command])
        .current_dir(cwd)
        .envs(env.vars())
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    Ok(status)
}

/// Run a hook command
pub fn run_hook(command: &str, cwd: &Path, env: &HookEnv) -> Result<()> {
    let status = run_interactive(command, cwd, env)?;

    if !status.success() {
        return Err(Error::HookFailed(command.to_string()));
    }

    Ok(())
}

/// Run multiple hooks in sequence
pub fn run_hooks(hooks: &[String], cwd: &Path, env: &HookEnv) -> Result<()> {
    for hook in hooks {
        eprintln!("Running hook: {hook}...");
        run_hook(hook, cwd, env)?;
        eprintln!("Hook done: {hook}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // A throwaway HookEnv for tests that don't assert on env vars.
    fn dummy_env(cwd: &Path) -> HookEnv<'_> {
        HookEnv {
            main_repo: cwd,
            worktree: cwd,
            branch: "test-branch",
            base_branch: "main",
        }
    }

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
        let result = run_interactive("true", dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());
        assert!(result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_failure() {
        let dir = tempdir().unwrap();
        let result = run_interactive("false", dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());
        assert!(!result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_echo() {
        let dir = tempdir().unwrap();
        let result = run_interactive("echo hello", dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());
        assert!(result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_with_cwd() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "content").unwrap();
        // Test that cwd is respected
        let result = run_interactive("test -f test.txt", dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());
        assert!(result.unwrap().success());
    }

    #[test]
    fn test_run_interactive_nonexistent_cwd() {
        let nonexistent = std::path::Path::new("/nonexistent/path/12345");
        let result = run_interactive("true", nonexistent, &dummy_env(nonexistent));
        assert!(result.is_err());
    }

    // =========================================================================
    // HookEnv injection tests
    // =========================================================================
    #[test]
    fn test_hook_env_vars_mapping() {
        let env = HookEnv {
            main_repo: Path::new("/repo"),
            worktree: Path::new("/repo/wt/feature"),
            branch: "feature",
            base_branch: "develop",
        };
        let vars = env.vars();
        assert_eq!(vars[0], ("WT_MAIN_REPO", "/repo".to_string()));
        assert_eq!(vars[1], ("WT_WORKTREE", "/repo/wt/feature".to_string()));
        assert_eq!(vars[2], ("WT_BRANCH", "feature".to_string()));
        assert_eq!(vars[3], ("WT_BASE_BRANCH", "develop".to_string()));
    }

    #[test]
    fn test_run_hook_injects_env_vars() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("env.txt");
        let env = HookEnv {
            main_repo: Path::new("/main/repo"),
            worktree: dir.path(),
            branch: "swift-fox",
            base_branch: "trunk",
        };
        // Hook reads injected vars and writes them out for assertion.
        let cmd = format!(
            "echo \"$WT_MAIN_REPO|$WT_BRANCH|$WT_BASE_BRANCH\" > {}",
            out.display()
        );
        run_hook(&cmd, dir.path(), &env).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content.trim(), "/main/repo|swift-fox|trunk");
    }

    #[test]
    fn test_run_hook_worktree_var_is_injected() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("wt.txt");
        let env = dummy_env(dir.path());
        // $WT_WORKTREE carries the worktree path verbatim. ($PWD is not used:
        // current_dir sets the real cwd but does not rewrite the $PWD var.)
        let cmd = format!("echo \"$WT_WORKTREE\" > {}", out.display());
        run_hook(&cmd, dir.path(), &env).unwrap();
        let content = std::fs::read_to_string(&out).unwrap();
        assert_eq!(content.trim(), dir.path().display().to_string());
    }

    // =========================================================================
    // run_hook tests
    // =========================================================================
    #[test]
    fn test_run_hook_success() {
        let dir = tempdir().unwrap();
        let result = run_hook("true", dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_hook_failure() {
        let dir = tempdir().unwrap();
        let result = run_hook("false", dir.path(), &dummy_env(dir.path()));
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
        let result = run_hook(&cmd, dir.path(), &dummy_env(dir.path()));
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
        let result = run_hooks(&hooks, dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_run_hooks_single() {
        let dir = tempdir().unwrap();
        let hooks = vec!["true".to_string()];
        let result = run_hooks(&hooks, dir.path(), &dummy_env(dir.path()));
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
        let result = run_hooks(&hooks, dir.path(), &dummy_env(dir.path()));
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

        let result = run_hooks(&hooks, dir.path(), &dummy_env(dir.path()));
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

        let result = run_hooks(&hooks, dir.path(), &dummy_env(dir.path()));
        assert!(result.is_ok());

        let content = std::fs::read_to_string(&file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines, vec!["one", "two", "three"]);
    }
}
