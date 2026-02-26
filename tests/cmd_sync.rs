// ===========================================================================
// Integration Tests - Sync Command
// ===========================================================================

mod common;

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_sync_on_trunk_fails() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt sync");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trunk") || stderr.contains("Already"));
}

#[test]
fn test_sync_abort_no_rebase() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    Command::new("git")
        .args(["checkout", "-b", "feature-sync"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["sync", "--abort"])
        .current_dir(dir.path())
        .output()
        .expect("wt sync --abort failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rebase") || stderr.contains("No") || !output.status.success());
}

#[test]
fn test_sync_continue_no_rebase() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    Command::new("git")
        .args(["checkout", "-b", "feature-sync-cont"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["sync", "--continue"])
        .current_dir(dir.path())
        .output()
        .expect("wt sync --continue failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rebase") || stderr.contains("No") || !output.status.success());
}

#[test]
fn test_sync_on_feature_branch() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "sync-feature", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let wt_path = read_path_file(&path_file).trim().to_string();

    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt sync failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() || stderr.contains("Already") || stderr.contains("sync"));
}

#[test]
fn test_sync_on_feature_with_updates() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "sync-updates", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    std::fs::write(repo.join("main-update.txt"), "update from main").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Main update"])
        .current_dir(&repo)
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt sync failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "sync failed: {}", stderr);
}
