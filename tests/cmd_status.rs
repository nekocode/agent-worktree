// ===========================================================================
// Integration Tests - Status Command
// ===========================================================================

mod common;

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_status_on_trunk_fails() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("status")
        .current_dir(dir.path())
        .output()
        .expect("wt status failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Not in a managed worktree"));
}

#[test]
fn test_status_in_worktree() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "status-test", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    let output = Command::new(wt_binary())
        .arg("status")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt status failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Branch:"), "Expected Branch field, got: {stdout}");
    assert!(stdout.contains("status-test"), "Expected branch name, got: {stdout}");
    assert!(stdout.contains("Trunk:"), "Expected Trunk field, got: {stdout}");
    assert!(stdout.contains("Commits:"), "Expected Commits field, got: {stdout}");
    assert!(stdout.contains("Path:"), "Expected Path field, got: {stdout}");
}

#[test]
fn test_status_with_commits() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "status-commits", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    // Make a commit in the worktree
    std::fs::write(wt_path.join("new-file.txt"), "content").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Test commit"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .arg("status")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt status failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show at least 1 commit
    assert!(stdout.contains("Commits:"), "Expected Commits field, got: {stdout}");
}

#[test]
fn test_status_with_base_branch() {
    let (dir, repo, home) = setup_worktree_test_env();

    // Create a feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature-base"])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["checkout", "main"])
        .current_dir(&repo)
        .output()
        .unwrap();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "status-base", "--base", "feature-base", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    let output = Command::new(wt_binary())
        .arg("status")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt status failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Base branch:"), "Expected Base branch field, got: {stdout}");
    assert!(stdout.contains("feature-base"), "Expected base branch name, got: {stdout}");
}
