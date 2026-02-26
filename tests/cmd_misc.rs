// ===========================================================================
// Integration Tests - Misc (Error Handling, Version, Unknown Commands)
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_not_in_git_repo() {
    let dir = tempdir().unwrap();

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt ls");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not") || stderr.contains("git"));
}

#[test]
fn test_unknown_command() {
    let output = Command::new(wt_binary())
        .arg("unknown-command")
        .output()
        .expect("Failed to execute wt unknown-command");

    assert!(!output.status.success());
}

#[test]
fn test_version_output() {
    let output = Command::new(wt_binary())
        .arg("--version")
        .output()
        .expect("wt --version failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("wt") || stdout.contains("0."));
}
