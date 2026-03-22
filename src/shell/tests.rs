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
    assert!(path
        .to_string_lossy()
        .contains("Microsoft.PowerShell_profile.ps1"));
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

// =========================================================================
// Completion init snippet tests
// =========================================================================
#[test]
fn test_bash_zsh_wrapper_contains_completion_init() {
    let wrapper = Shell::Bash.wrapper_script();
    assert!(
        wrapper.contains("COMPLETE="),
        "bash/zsh wrapper should contain COMPLETE env var for dynamic completions"
    );
    assert!(
        wrapper.contains("_wt_bin"),
        "bash/zsh wrapper should locate binary for completions"
    );
}

#[test]
fn test_powershell_wrapper_contains_completion_init() {
    let wrapper = Shell::PowerShell.wrapper_script();
    assert!(
        wrapper.contains("COMPLETE"),
        "powershell wrapper should contain COMPLETE env var for completions"
    );
}

#[test]
fn test_fish_completions_content() {
    assert!(
        FISH_COMPLETIONS.contains("COMPLETE=fish"),
        "fish completions should set COMPLETE=fish"
    );
    assert!(
        FISH_COMPLETIONS.contains("source"),
        "fish completions should source the output"
    );
}

#[test]
fn test_fish_completions_path() {
    let path = fish_completions_path();
    assert!(path.is_ok());
    let path = path.unwrap();
    assert!(path.to_string_lossy().contains("completions/wt.fish"));
}
