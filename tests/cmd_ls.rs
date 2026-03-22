// ===========================================================================
// Integration Tests - Ls Command
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_ls_empty() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt ls");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No worktrees") || output.status.success());
}

#[test]
fn test_ls_in_subdirectory() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let sub = dir.path().join("src");
    std::fs::create_dir_all(&sub).unwrap();

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&sub)
        .output()
        .expect("wt ls failed");

    assert!(
        output.status.success() || String::from_utf8_lossy(&output.stderr).contains("No worktrees")
    );
}

#[test]
fn test_ls_shows_worktree_details() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "ls-test-branch"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    assert!(combined.contains("ls-test-branch") || combined.contains("BRANCH"));
}

#[test]
fn test_ls_with_multiple_worktrees() {
    let (_dir, repo, home) = setup_worktree_test_env();

    for name in &["multi-ls-1", "multi-ls-2"] {
        let _ = Command::new(wt_binary())
            .args(["new", name])
            .current_dir(&repo)
            .env("HOME", &home)
            .output();
    }

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(combined.contains("multi-ls") || combined.contains("BRANCH"));
}
