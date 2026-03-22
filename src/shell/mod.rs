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

// ---------------------------------------------------------------------------
// Shell Wrapper 脚本
//
// 协议约定（修改 snap 行为时，三套脚本必须同步更新）：
// - snap-continue 退出码: 0=完成(cd 到 main), 2=重新打开 agent, 3=退出(留在 worktree)
// - path_file 格式: 单行=目标路径, 双行=第一行路径+第二行命令(snap 模式)
// ---------------------------------------------------------------------------

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
# Dynamic completions: call binary directly to bypass wt function
if [[ -n "$ZSH_VERSION" ]]; then
  _wt_bin=$(whence -p wt 2>/dev/null)
  [[ -n "$_wt_bin" ]] && source <(COMPLETE=zsh "$_wt_bin" 2>/dev/null) 2>/dev/null
else
  _wt_bin=$(type -P wt 2>/dev/null)
  [[ -n "$_wt_bin" ]] && source <(COMPLETE=bash "$_wt_bin" 2>/dev/null) 2>/dev/null
fi
unset _wt_bin
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
# Dynamic completions: call binary directly to bypass wt function
$_wtBin = Get-Command wt -CommandType Application -ErrorAction SilentlyContinue |
  Select-Object -ExpandProperty Source -First 1
if ($_wtBin) {
  $env:COMPLETE = "powershell"
  & $_wtBin | Out-String | Invoke-Expression
  Remove-Item Env:\COMPLETE -ErrorAction SilentlyContinue
}
Remove-Variable _wtBin -ErrorAction SilentlyContinue
# === agent-worktree END ==="#;

// Fish completions go to a dedicated file (auto-sourced by fish)
const FISH_COMPLETIONS: &str = r#"# Dynamic completions for wt (auto-generated by wt setup)
set -l _wt_bin (type --force-path wt 2>/dev/null)
if test -n "$_wt_bin"
  COMPLETE=fish $_wt_bin | source
end
"#;

/// Fish completions file path: ~/.config/fish/completions/wt.fish
fn fish_completions_path() -> Result<PathBuf> {
    let base = BaseDirs::new().ok_or(Error::NoHome)?;
    Ok(base.home_dir().join(".config/fish/completions/wt.fish"))
}

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

    // Fish: also install dedicated completions file
    if shell == Shell::Fish {
        let completions_path = fish_completions_path()?;
        if let Some(parent) = completions_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&completions_path, FISH_COMPLETIONS)?;
    }

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
mod tests;
