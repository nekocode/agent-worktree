// ===========================================================================
// shell - Shell Integration Installation
// ===========================================================================

use std::path::PathBuf;

use directories::BaseDirs;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("home directory not found")]
    NoHome,

    #[error("unsupported shell: {0}")]
    UnsupportedShell(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

impl Shell {
    pub fn detect() -> Option<Self> {
        // Windows: default to PowerShell
        #[cfg(windows)]
        {
            return Some(Shell::PowerShell);
        }

        // Unix: check $SHELL
        #[cfg(not(windows))]
        {
            std::env::var("SHELL")
                .ok()
                .and_then(|s| Self::from_path(&s))
        }
    }

    pub fn from_path(path: &str) -> Option<Self> {
        let path_lower = path.to_lowercase();
        if path_lower.ends_with("bash") {
            Some(Shell::Bash)
        } else if path_lower.ends_with("zsh") {
            Some(Shell::Zsh)
        } else if path_lower.ends_with("fish") {
            Some(Shell::Fish)
        } else if path_lower.contains("powershell") || path_lower.ends_with("pwsh") {
            Some(Shell::PowerShell)
        } else {
            None
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            "powershell" | "pwsh" => Some(Shell::PowerShell),
            _ => None,
        }
    }

    pub fn config_file(&self) -> Result<PathBuf> {
        let base = BaseDirs::new().ok_or(Error::NoHome)?;
        let home = base.home_dir();

        Ok(match self {
            Shell::Bash => home.join(".bashrc"),
            Shell::Zsh => home.join(".zshrc"),
            Shell::Fish => home.join(".config/fish/config.fish"),
            // PowerShell profile: $HOME/Documents/PowerShell/Microsoft.PowerShell_profile.ps1
            Shell::PowerShell => {
                #[cfg(windows)]
                {
                    home.join("Documents")
                        .join("PowerShell")
                        .join("Microsoft.PowerShell_profile.ps1")
                }
                #[cfg(not(windows))]
                {
                    // On Unix, pwsh uses ~/.config/powershell/Microsoft.PowerShell_profile.ps1
                    home.join(".config")
                        .join("powershell")
                        .join("Microsoft.PowerShell_profile.ps1")
                }
            }
        })
    }

    pub fn wrapper_script(&self) -> &'static str {
        match self {
            Shell::Bash | Shell::Zsh => BASH_ZSH_WRAPPER,
            Shell::Fish => FISH_WRAPPER,
            Shell::PowerShell => POWERSHELL_WRAPPER,
        }
    }
}

const MARKER_BEGIN: &str = "# === agent-worktree BEGIN ===";
const MARKER_END: &str = "# === agent-worktree END ===";

const BASH_ZSH_WRAPPER: &str = r#"# === agent-worktree BEGIN ===
wt() {
  if ! command -v wt &>/dev/null; then
    echo "wt: command not found. Install: npm install -g agent-worktree" >&2
    return 1
  fi
  case "$1" in
    cd|main)
      local path
      path="$(command wt "$@" --print-path)" || return $?
      cd "$path"
      ;;
    new|rm|move|merge|clean)
      local path
      path="$(command wt "$@" --print-path)" || return $?
      [[ -n "$path" ]] && cd "$path"
      ;;
    *)
      command wt "$@"
      ;;
  esac
}
# === agent-worktree END ==="#;

const FISH_WRAPPER: &str = r#"# === agent-worktree BEGIN ===
function wt
  if not command -v wt &>/dev/null
    echo "wt: command not found. Install: npm install -g agent-worktree" >&2
    return 1
  end
  switch $argv[1]
    case cd main
      set -l path (command wt $argv --print-path)
      and cd $path
    case new rm move merge clean
      set -l path (command wt $argv --print-path)
      and test -n "$path"
      and cd $path
    case '*'
      command wt $argv
  end
end
# === agent-worktree END ==="#;

const POWERSHELL_WRAPPER: &str = r#"# === agent-worktree BEGIN ===
function wt {
  $wtPath = Get-Command wt.exe -ErrorAction SilentlyContinue
  if (-not $wtPath) {
    Write-Error "wt: command not found. Install: npm install -g agent-worktree"
    return
  }
  switch ($args[0]) {
    { $_ -in 'cd', 'main' } {
      $path = & wt.exe @args --print-path
      if ($LASTEXITCODE -eq 0 -and $path) {
        Set-Location $path
      }
    }
    { $_ -in 'new', 'rm', 'move', 'merge', 'clean' } {
      $path = & wt.exe @args --print-path
      if ($LASTEXITCODE -eq 0 -and $path) {
        Set-Location $path
      }
    }
    default {
      & wt.exe @args
    }
  }
}
# === agent-worktree END ==="#;

/// Install shell wrapper to config file
pub fn install(shell: Shell) -> Result<()> {
    let config_path = shell.config_file()?;
    let wrapper = shell.wrapper_script();

    // Ensure parent directory exists (for fish)
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Read existing content or empty
    let content = std::fs::read_to_string(&config_path).unwrap_or_default();

    // Remove old wrapper if present
    let content = remove_wrapper(&content);

    // Append new wrapper with blank lines before and after
    let new_content = if content.is_empty() {
        format!("{wrapper}\n")
    } else if content.ends_with('\n') {
        format!("{content}\n{wrapper}\n")
    } else {
        format!("{content}\n\n{wrapper}\n")
    };

    std::fs::write(&config_path, new_content)?;

    Ok(())
}

/// Remove existing wrapper from content
fn remove_wrapper(content: &str) -> String {
    let mut result = String::new();
    let mut in_wrapper = false;

    for line in content.lines() {
        if line.contains(MARKER_BEGIN) {
            in_wrapper = true;
            continue;
        }
        if line.contains(MARKER_END) {
            in_wrapper = false;
            continue;
        }
        if !in_wrapper {
            result.push_str(line);
            result.push('\n');
        }
    }

    // Remove trailing newlines
    while result.ends_with("\n\n") {
        result.pop();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_from_path() {
        assert_eq!(Shell::from_path("/bin/bash"), Some(Shell::Bash));
        assert_eq!(Shell::from_path("/usr/bin/zsh"), Some(Shell::Zsh));
        assert_eq!(
            Shell::from_path("/opt/homebrew/bin/fish"),
            Some(Shell::Fish)
        );
        assert_eq!(
            Shell::from_path("C:\\Program Files\\PowerShell\\7\\pwsh.exe"),
            Some(Shell::PowerShell)
        );
        assert_eq!(Shell::from_path("/bin/sh"), None);
    }

    #[test]
    fn test_shell_from_name() {
        assert_eq!(Shell::from_name("bash"), Some(Shell::Bash));
        assert_eq!(Shell::from_name("ZSH"), Some(Shell::Zsh));
        assert_eq!(Shell::from_name("Fish"), Some(Shell::Fish));
        assert_eq!(Shell::from_name("powershell"), Some(Shell::PowerShell));
        assert_eq!(Shell::from_name("pwsh"), Some(Shell::PowerShell));
        assert_eq!(Shell::from_name("ksh"), None);
    }

    #[test]
    fn test_wrapper_script_contains_markers() {
        let bash_wrapper = Shell::Bash.wrapper_script();
        assert!(bash_wrapper.contains(MARKER_BEGIN));
        assert!(bash_wrapper.contains(MARKER_END));

        let fish_wrapper = Shell::Fish.wrapper_script();
        assert!(fish_wrapper.contains(MARKER_BEGIN));
        assert!(fish_wrapper.contains(MARKER_END));

        let ps_wrapper = Shell::PowerShell.wrapper_script();
        assert!(ps_wrapper.contains(MARKER_BEGIN));
        assert!(ps_wrapper.contains(MARKER_END));
    }

    #[test]
    fn test_remove_wrapper_empty() {
        let result = remove_wrapper("");
        assert_eq!(result, "");
    }

    #[test]
    fn test_remove_wrapper_no_wrapper() {
        let content = "alias ll='ls -la'\nexport PATH=$PATH:/usr/local/bin\n";
        let result = remove_wrapper(content);
        assert_eq!(result, content);
    }

    #[test]
    fn test_remove_wrapper_with_wrapper() {
        let content = r#"alias ll='ls -la'
# === agent-worktree BEGIN ===
wt() { ... }
# === agent-worktree END ===
export PATH=$PATH:/usr/local/bin
"#;
        let result = remove_wrapper(content);
        assert!(result.contains("alias ll"));
        assert!(result.contains("export PATH"));
        assert!(!result.contains("agent-worktree"));
        assert!(!result.contains("wt()"));
    }

    #[test]
    fn test_remove_wrapper_only_wrapper() {
        let content = r#"# === agent-worktree BEGIN ===
wt() { ... }
# === agent-worktree END ===
"#;
        let result = remove_wrapper(content);
        assert!(!result.contains("agent-worktree"));
    }
}
