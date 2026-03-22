// ===========================================================================
// Integration Tests - Shell Completions
// ===========================================================================

mod common;

use std::process::Command;

use common::*;

#[test]
fn test_complete_bash_outputs_script() {
    let output = Command::new(wt_binary())
        .env("COMPLETE", "bash")
        .output()
        .expect("Failed to run with COMPLETE=bash");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("complete"),
        "bash output should contain 'complete'"
    );
    assert!(stdout.contains("wt"), "bash output should reference 'wt'");
}

#[test]
fn test_complete_zsh_outputs_script() {
    let output = Command::new(wt_binary())
        .env("COMPLETE", "zsh")
        .output()
        .expect("Failed to run with COMPLETE=zsh");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("compdef") || stdout.contains("wt"));
}

#[test]
fn test_complete_fish_outputs_script() {
    let output = Command::new(wt_binary())
        .env("COMPLETE", "fish")
        .output()
        .expect("Failed to run with COMPLETE=fish");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("complete"),
        "fish output should contain 'complete'"
    );
}

#[test]
fn test_complete_powershell_outputs_script() {
    let output = Command::new(wt_binary())
        .env("COMPLETE", "powershell")
        .output()
        .expect("Failed to run with COMPLETE=powershell");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Register-ArgumentCompleter") || stdout.contains("wt"),
        "powershell output should contain completer registration"
    );
}

#[test]
fn test_complete_dynamic_branches() {
    // Test dynamic branch completion inside a git repo
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a feature branch
    Command::new("git")
        .args(["branch", "feature-test"])
        .current_dir(&repo)
        .output()
        .expect("git branch failed");

    let output = Command::new(wt_binary())
        .env("COMPLETE", "bash")
        .env("HOME", &home)
        .env("_CLAP_COMPLETE_INDEX", "3")
        .env("_CLAP_COMPLETE_COMP_TYPE", "9")
        .env("_CLAP_COMPLETE_SPACE", "true")
        .args(["--", "wt", "new", "--base", ""])
        .current_dir(&repo)
        .output()
        .expect("Failed to run dynamic completion");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("feature-test"),
        "should complete branch name 'feature-test', got: {stdout}"
    );
    assert!(
        stdout.contains("main"),
        "should complete branch name 'main', got: {stdout}"
    );
}
