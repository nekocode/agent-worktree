// ===========================================================================
// Integration Tests - Help & Strategy Display
// ===========================================================================

mod common;

use std::process::Command;

use common::wt_binary;

#[test]
fn test_help_output() {
    let output = Command::new(wt_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute wt --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Git worktree workflow tool"));
    assert!(stdout.contains("new"));
    assert!(stdout.contains("ls"));
    assert!(stdout.contains("cd"));
    assert!(stdout.contains("merge"));
}

#[test]
fn test_new_help() {
    let output = Command::new(wt_binary())
        .args(["new", "--help"])
        .output()
        .expect("Failed to execute wt new --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Create a new worktree"));
    assert!(stdout.contains("--base"));
    assert!(stdout.contains("--snap"));
}

#[test]
fn test_merge_help() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("Failed to execute wt merge --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Merge"));
    assert!(stdout.contains("--strategy"));
    assert!(stdout.contains("--into"));
    assert!(stdout.contains("--keep"));
}

#[test]
fn test_sync_help() {
    let output = Command::new(wt_binary())
        .args(["sync", "--help"])
        .output()
        .expect("Failed to execute wt sync --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Sync"));
    assert!(stdout.contains("--strategy"));
    assert!(stdout.contains("--continue"));
    assert!(stdout.contains("--abort"));
}

#[test]
fn test_mv_help() {
    let output = Command::new(wt_binary())
        .args(["mv", "--help"])
        .output()
        .expect("Failed to execute wt mv --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rename"));
    assert!(stdout.contains("OLD_BRANCH"));
    assert!(stdout.contains("NEW_BRANCH"));
}

#[test]
fn test_clean_help() {
    let output = Command::new(wt_binary())
        .args(["clean", "--help"])
        .output()
        .expect("Failed to execute wt clean --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("diff") || stdout.contains("Remove"));
}

#[test]
fn test_setup_help() {
    let output = Command::new(wt_binary())
        .args(["setup", "--help"])
        .output()
        .expect("Failed to execute wt setup --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("shell"));
    assert!(stdout.contains("integration"));
}

#[test]
fn test_merge_help_shows_strategies() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("wt merge --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("squash") || stdout.contains("strategy"));
}

#[test]
fn test_merge_strategy_squash() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("wt merge --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("squash"));
}

#[test]
fn test_merge_strategy_rebase() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("wt merge --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rebase"));
}

#[test]
fn test_sync_strategy_options() {
    let output = Command::new(wt_binary())
        .args(["sync", "--help"])
        .output()
        .expect("wt sync --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rebase") || stdout.contains("strategy"));
}
