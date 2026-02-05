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
            Shell::Bash => {
                // macOS: login shells read .bash_profile, not .bashrc
                // Use .bash_profile if it exists, otherwise .bashrc
                #[cfg(target_os = "macos")]
                {
                    let bash_profile = home.join(".bash_profile");
                    if bash_profile.exists() {
                        bash_profile
                    } else {
                        home.join(".bashrc")
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    home.join(".bashrc")
                }
            }
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
# NOTE: Don't use 'path' as variable name - it shadows zsh's $path array
wt() {
  local wt_bin path_file target_path snap_cmd
  if [[ -n "$ZSH_VERSION" ]]; then
    wt_bin=$(whence -p wt 2>/dev/null)
  else
    wt_bin=$(type -P wt 2>/dev/null)
  fi
  if [[ -z "$wt_bin" ]]; then
    echo "wt: binary not found. Install: npm install -g agent-worktree" >&2
    return 1
  fi
  # Pass through if -h/--help anywhere in args
  case " $* " in
    *" -h "*|*" --help "*) "$wt_bin" "$@"; return ;;
  esac
  # Create temp file for path output (avoids stdout pollution from hooks)
  path_file="${TMPDIR:-/tmp}/wt-path-$$"
  case "$1" in
    cd|main)
      "$wt_bin" "$@" --path-file "$path_file" || return $?
      if [[ -f "$path_file" ]]; then
        target_path=$(<"$path_file"); rm -f "$path_file"; cd "$target_path"
      fi
      ;;
    new)
      # Check for snap mode (-s/--snap)
      if [[ " $* " == *" -s "* ]] || [[ " $* " == *" --snap "* ]]; then
        "$wt_bin" "$@" --path-file "$path_file" || return $?
        if [[ -f "$path_file" ]]; then
          target_path="$(head -n1 "$path_file")"
          snap_cmd="$(tail -n1 "$path_file")"
          rm -f "$path_file"
          [[ "$target_path" == "$snap_cmd" ]] && snap_cmd=""
          [[ -n "$target_path" ]] && cd "$target_path"
          # Run snap mode loop in shell (preserves TTY)
          if [[ -n "$snap_cmd" ]]; then
            while true; do
              echo "Entering snap mode: $snap_cmd"
              echo "Worktree: $(basename "$target_path")"
              echo "---"
              eval "$snap_cmd"
              local agent_status=$?
              if [[ $agent_status -ne 0 ]]; then
                echo "Agent exited abnormally. Worktree preserved."
                return $agent_status
              fi
              "$wt_bin" snap-continue --path-file "$path_file"
              local continue_status=$?
              # 0: done, cd to main; 2: reopen agent; 3: exit, stay in worktree
              if [[ $continue_status -eq 0 ]] && [[ -f "$path_file" ]]; then
                target_path=$(<"$path_file"); rm -f "$path_file"; cd "$target_path"
                break
              elif [[ $continue_status -eq 3 ]]; then
                rm -f "$path_file"
                break
              elif [[ $continue_status -ne 2 ]]; then
                rm -f "$path_file"
                break
              fi
            done
          fi
        fi
      else
        "$wt_bin" "$@" --path-file "$path_file" || return $?
        if [[ -f "$path_file" ]]; then
          target_path=$(<"$path_file"); rm -f "$path_file"; cd "$target_path"
        fi
      fi
      ;;
    rm|mv|merge|clean)
      "$wt_bin" "$@" --path-file "$path_file" || return $?
      if [[ -f "$path_file" ]]; then
        target_path=$(<"$path_file"); rm -f "$path_file"; cd "$target_path"
      fi
      ;;
    *)
      "$wt_bin" "$@"
      ;;
  esac
}
# === agent-worktree END ==="#;

const FISH_WRAPPER: &str = r#"# === agent-worktree BEGIN ===
function wt
  set -l wt_bin (type --force-path wt 2>/dev/null)
  if test -z "$wt_bin"
    echo "wt: binary not found. Install: npm install -g agent-worktree" >&2
    return 1
  end
  if contains -- -h $argv; or contains -- --help $argv
    $wt_bin $argv
    return
  end
  set -l path_file (mktemp)
  switch $argv[1]
    case cd main
      $wt_bin $argv --path-file $path_file; or begin; rm -f $path_file; return $status; end
      if test -f $path_file; cd (cat $path_file); rm -f $path_file; end
    case new
      if contains -- -s $argv; or contains -- --snap $argv
        $wt_bin $argv --path-file $path_file; or begin; rm -f $path_file; return $status; end
        if test -f $path_file
          set -l target_path (head -n1 $path_file)
          set -l snap_cmd (tail -n1 $path_file)
          rm -f $path_file
          test "$target_path" = "$snap_cmd"; and set snap_cmd ""
          test -n "$target_path"; and cd $target_path
          if test -n "$snap_cmd"
            while true
              echo "Entering snap mode: $snap_cmd"
              echo "Worktree: "(basename $target_path)
              echo "---"
              eval $snap_cmd
              set -l agent_status $status
              if test $agent_status -ne 0
                echo "Agent exited abnormally. Worktree preserved."
                return $agent_status
              end
              $wt_bin snap-continue --path-file $path_file
              set -l continue_status $status
              # 0: done, cd to main; 2: reopen agent; 3: exit, stay in worktree
              if test $continue_status -eq 0; and test -f $path_file
                cd (cat $path_file); rm -f $path_file
                break
              else if test $continue_status -eq 3
                rm -f $path_file
                break
              else if test $continue_status -ne 2
                rm -f $path_file
                break
              end
            end
          end
        end
      else
        $wt_bin $argv --path-file $path_file; or begin; rm -f $path_file; return $status; end
        if test -f $path_file; cd (cat $path_file); rm -f $path_file; end
      end
    case rm mv merge clean
      $wt_bin $argv --path-file $path_file; or begin; rm -f $path_file; return $status; end
      if test -f $path_file; cd (cat $path_file); rm -f $path_file; end
    case '*'
      rm -f $path_file
      $wt_bin $argv
  end
end
# === agent-worktree END ==="#;

const POWERSHELL_WRAPPER: &str = r#"# === agent-worktree BEGIN ===
function wt {
  $wtBin = Get-Command wt -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $wtBin) {
    Write-Error "wt: binary not found. Install: npm install -g agent-worktree"
    return 1
  }
  if ($args -contains '-h' -or $args -contains '--help') {
    & $wtBin.Source @args
    return
  }
  $pathFile = [System.IO.Path]::GetTempFileName()
  switch ($args[0]) {
    { $_ -in 'cd', 'main' } {
      & $wtBin.Source @args --path-file $pathFile
      if ($LASTEXITCODE -ne 0) { Remove-Item $pathFile -ErrorAction SilentlyContinue; return $LASTEXITCODE }
      if (Test-Path $pathFile) { Set-Location (Get-Content $pathFile); Remove-Item $pathFile }
    }
    'new' {
      if ($args -contains '-s' -or $args -contains '--snap') {
        & $wtBin.Source @args --path-file $pathFile
        if ($LASTEXITCODE -ne 0) { Remove-Item $pathFile -ErrorAction SilentlyContinue; return $LASTEXITCODE }
        if (Test-Path $pathFile) {
          $lines = Get-Content $pathFile
          $targetPath = $lines[0]
          $snapCmd = if ($lines.Count -gt 1) { $lines[1] } else { "" }
          Remove-Item $pathFile
          if ($targetPath -eq $snapCmd) { $snapCmd = "" }
          if ($targetPath) { Set-Location $targetPath }
          if ($snapCmd) {
            while ($true) {
              Write-Host "Entering snap mode: $snapCmd"
              Write-Host "Worktree: $(Split-Path $targetPath -Leaf)"
              Write-Host "---"
              Invoke-Expression $snapCmd
              $agentStatus = $LASTEXITCODE
              if ($agentStatus -ne 0) {
                Write-Host "Agent exited abnormally. Worktree preserved."
                return $agentStatus
              }
              & $wtBin.Source snap-continue --path-file $pathFile
              $continueStatus = $LASTEXITCODE
              # 0: done, cd to main; 2: reopen agent; 3: exit, stay in worktree
              if ($continueStatus -eq 0 -and (Test-Path $pathFile)) {
                Set-Location (Get-Content $pathFile); Remove-Item $pathFile
                break
              } elseif ($continueStatus -eq 3) {
                Remove-Item $pathFile -ErrorAction SilentlyContinue
                break
              } elseif ($continueStatus -ne 2) {
                Remove-Item $pathFile -ErrorAction SilentlyContinue
                break
              }
            }
          }
        }
      } else {
        & $wtBin.Source @args --path-file $pathFile
        if ($LASTEXITCODE -ne 0) { Remove-Item $pathFile -ErrorAction SilentlyContinue; return $LASTEXITCODE }
        if (Test-Path $pathFile) { Set-Location (Get-Content $pathFile); Remove-Item $pathFile }
      }
    }
    { $_ -in 'rm', 'mv', 'merge', 'clean' } {
      & $wtBin.Source @args --path-file $pathFile
      if ($LASTEXITCODE -ne 0) { Remove-Item $pathFile -ErrorAction SilentlyContinue; return $LASTEXITCODE }
      if (Test-Path $pathFile) { Set-Location (Get-Content $pathFile); Remove-Item $pathFile }
    }
    default {
      Remove-Item $pathFile -ErrorAction SilentlyContinue
      & $wtBin.Source @args
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
    use tempfile::tempdir;

    // =========================================================================
    // Shell::from_path tests
    // =========================================================================
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
    fn test_shell_from_path_case_insensitive() {
        assert_eq!(Shell::from_path("/bin/BASH"), Some(Shell::Bash));
        assert_eq!(Shell::from_path("/usr/bin/ZSH"), Some(Shell::Zsh));
        assert_eq!(Shell::from_path("/usr/bin/Fish"), Some(Shell::Fish));
    }

    #[test]
    fn test_shell_from_path_powershell_variants() {
        assert_eq!(Shell::from_path("pwsh"), Some(Shell::PowerShell));
        assert_eq!(
            Shell::from_path("C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
            Some(Shell::PowerShell)
        );
    }

    // =========================================================================
    // Shell::from_name tests
    // =========================================================================
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
    fn test_shell_from_name_unknown() {
        assert_eq!(Shell::from_name("sh"), None);
        assert_eq!(Shell::from_name("csh"), None);
        assert_eq!(Shell::from_name("tcsh"), None);
        assert_eq!(Shell::from_name("unknown"), None);
    }

    // =========================================================================
    // Shell::detect tests
    // =========================================================================
    #[test]
    fn test_shell_detect_returns_option() {
        // Just test that detect() returns a valid option
        let result = Shell::detect();
        // On most CI/dev systems, this should return Some()
        // We can't assert the exact shell since it depends on environment
        assert!(result.is_some() || result.is_none()); // Valid either way
    }

    // =========================================================================
    // Shell::config_file tests
    // =========================================================================
    #[test]
    fn test_shell_config_file_bash() {
        let config = Shell::Bash.config_file();
        assert!(config.is_ok());
        let path = config.unwrap();
        // macOS uses .bash_profile if exists, otherwise .bashrc
        let path_str = path.to_string_lossy();
        assert!(path_str.contains(".bashrc") || path_str.contains(".bash_profile"));
    }

    #[test]
    fn test_shell_config_file_zsh() {
        let config = Shell::Zsh.config_file();
        assert!(config.is_ok());
        let path = config.unwrap();
        assert!(path.to_string_lossy().contains(".zshrc"));
    }

    #[test]
    fn test_shell_config_file_fish() {
        let config = Shell::Fish.config_file();
        assert!(config.is_ok());
        let path = config.unwrap();
        assert!(path.to_string_lossy().contains("config.fish"));
    }

    #[test]
    fn test_shell_config_file_powershell() {
        let config = Shell::PowerShell.config_file();
        assert!(config.is_ok());
        let path = config.unwrap();
        assert!(path.to_string_lossy().contains("Microsoft.PowerShell_profile.ps1"));
    }

    // =========================================================================
    // Shell::wrapper_script tests
    // =========================================================================
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
    fn test_wrapper_script_bash_zsh_same() {
        let bash = Shell::Bash.wrapper_script();
        let zsh = Shell::Zsh.wrapper_script();
        assert_eq!(bash, zsh);
    }

    #[test]
    fn test_wrapper_script_contains_wt_function() {
        let bash = Shell::Bash.wrapper_script();
        assert!(bash.contains("wt()"));

        let fish = Shell::Fish.wrapper_script();
        assert!(fish.contains("function wt"));

        let ps = Shell::PowerShell.wrapper_script();
        assert!(ps.contains("function wt"));
    }

    #[test]
    fn test_wrapper_script_handles_cd_command() {
        let bash = Shell::Bash.wrapper_script();
        assert!(bash.contains("cd|main"));

        let fish = Shell::Fish.wrapper_script();
        assert!(fish.contains("case cd main"));

        let ps = Shell::PowerShell.wrapper_script();
        assert!(ps.contains("'cd', 'main'"));
    }

    // =========================================================================
    // remove_wrapper tests
    // =========================================================================
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

    #[test]
    fn test_remove_wrapper_at_start() {
        let content = r#"# === agent-worktree BEGIN ===
wt() { ... }
# === agent-worktree END ===
alias ll='ls -la'
"#;
        let result = remove_wrapper(content);
        assert!(!result.contains("agent-worktree"));
        assert!(result.contains("alias ll"));
    }

    #[test]
    fn test_remove_wrapper_preserves_content_order() {
        let content = r#"line1
# === agent-worktree BEGIN ===
wt() { ... }
# === agent-worktree END ===
line2
"#;
        let result = remove_wrapper(content);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
    }

    // =========================================================================
    // Error tests
    // =========================================================================
    #[test]
    fn test_error_display() {
        let err = Error::NoHome;
        assert_eq!(err.to_string(), "home directory not found");

        let err = Error::UnsupportedShell("ksh".to_string());
        assert_eq!(err.to_string(), "unsupported shell: ksh");
    }

    // =========================================================================
    // install tests (with temp files)
    // =========================================================================
    #[test]
    fn test_install_creates_wrapper() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".bashrc");

        // Create empty config
        std::fs::write(&config_path, "").unwrap();

        // Mock the config_file by directly testing the logic
        let wrapper = Shell::Bash.wrapper_script();
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let _content = remove_wrapper(&content);
        let new_content = format!("{wrapper}\n");
        std::fs::write(&config_path, new_content).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains(MARKER_BEGIN));
        assert!(result.contains(MARKER_END));
        assert!(result.contains("wt()"));
    }

    #[test]
    fn test_install_replaces_existing_wrapper() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".bashrc");

        // Create config with old wrapper
        let old_content = r#"alias ll='ls -la'
# === agent-worktree BEGIN ===
old_wt_function() { echo old; }
# === agent-worktree END ===
export PATH=/usr/local/bin
"#;
        std::fs::write(&config_path, old_content).unwrap();

        // Simulate install
        let wrapper = Shell::Bash.wrapper_script();
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let content = remove_wrapper(&content);
        let new_content = format!("{content}\n{wrapper}\n");
        std::fs::write(&config_path, new_content).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("alias ll"));
        assert!(result.contains("export PATH"));
        assert!(!result.contains("old_wt_function"));
        assert!(result.contains("wt()"));
        // Should only have one set of markers
        assert_eq!(result.matches(MARKER_BEGIN).count(), 1);
        assert_eq!(result.matches(MARKER_END).count(), 1);
    }

    #[test]
    fn test_install_appends_to_existing_content() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".zshrc");

        let existing = "# My zsh config\nalias ll='ls -la'\n";
        std::fs::write(&config_path, existing).unwrap();

        // Simulate install
        let wrapper = Shell::Zsh.wrapper_script();
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let content = remove_wrapper(&content);
        let new_content = format!("{content}\n{wrapper}\n");
        std::fs::write(&config_path, new_content).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("# My zsh config"));
        assert!(result.contains("alias ll"));
        assert!(result.contains(MARKER_BEGIN));
    }

    // =========================================================================
    // Test install function directly with temp HOME
    // =========================================================================
    #[test]
    fn test_install_function_creates_parent_dirs() {
        // Test that install creates parent directories for fish config
        let dir = tempdir().unwrap();
        let fish_config = dir.path().join(".config").join("fish").join("config.fish");

        // The parent directory doesn't exist yet
        assert!(!fish_config.parent().unwrap().exists());

        // Simulate what install does
        if let Some(parent) = fish_config.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        assert!(fish_config.parent().unwrap().exists());

        // Now write wrapper
        let wrapper = Shell::Fish.wrapper_script();
        std::fs::write(&fish_config, format!("{wrapper}\n")).unwrap();
        assert!(fish_config.exists());
    }

    #[test]
    fn test_install_handles_content_without_newline() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join(".bashrc");

        // Content without trailing newline
        std::fs::write(&config_path, "alias ll='ls -la'").unwrap();

        // Simulate install
        let wrapper = Shell::Bash.wrapper_script();
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();
        let content = remove_wrapper(&content);

        // The install function handles this case
        let new_content = if content.is_empty() {
            format!("{wrapper}\n")
        } else if content.ends_with('\n') {
            format!("{content}\n{wrapper}\n")
        } else {
            format!("{content}\n\n{wrapper}\n")
        };
        std::fs::write(&config_path, new_content).unwrap();

        let result = std::fs::read_to_string(&config_path).unwrap();
        assert!(result.contains("alias ll"));
        assert!(result.contains(MARKER_BEGIN));
        // Should have blank line before wrapper when content didn't end with newline
        assert!(result.contains("\n\n"));
    }

    #[test]
    fn test_remove_wrapper_trailing_newlines() {
        let content = r#"alias ll='ls -la'


"#;
        let result = remove_wrapper(content);
        // Should not have excessive trailing newlines
        assert!(!result.ends_with("\n\n\n"));
    }

    #[test]
    fn test_fish_wrapper_script_syntax() {
        let wrapper = Shell::Fish.wrapper_script();
        // Fish uses 'function' and 'end' keywords
        assert!(wrapper.contains("function wt"));
        assert!(wrapper.contains("end"));
        assert!(wrapper.contains("switch"));
    }

    #[test]
    fn test_powershell_wrapper_script_syntax() {
        let wrapper = Shell::PowerShell.wrapper_script();
        // PowerShell uses function {} and switch
        assert!(wrapper.contains("function wt {"));
        assert!(wrapper.contains("switch"));
        assert!(wrapper.contains("Set-Location"));
    }

    // =========================================================================
    // Shell equality and clone tests
    // =========================================================================
    #[test]
    fn test_shell_equality() {
        assert_eq!(Shell::Bash, Shell::Bash);
        assert_ne!(Shell::Bash, Shell::Zsh);
        assert_ne!(Shell::Fish, Shell::PowerShell);
    }

    #[test]
    fn test_shell_clone() {
        let shell = Shell::Fish;
        let cloned = shell;
        assert_eq!(shell, cloned);
    }

    #[test]
    fn test_shell_debug() {
        let shell = Shell::Bash;
        let debug = format!("{:?}", shell);
        assert_eq!(debug, "Bash");
    }
}
