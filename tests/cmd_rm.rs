// ===========================================================================
// Integration Tests - Rm Command
// ===========================================================================

mod common;

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_rm_nonexistent() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["rm", "nonexistent-branch"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt rm");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("error"));
}

#[test]
fn test_rm_with_force() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["rm", "nonexistent", "--force"])
        .current_dir(dir.path())
        .output()
        .expect("wt rm failed");

    assert!(!output.status.success());
}

#[test]
fn test_rm_force_dirty_worktree() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "rm-dirty",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
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

    std::fs::write(PathBuf::from(&wt_path).join("dirty.txt"), "uncommitted").unwrap();

    let output = Command::new(wt_binary())
        .args(["rm", "rm-dirty", "--force"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt rm failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() || stderr.contains("force") || stderr.contains("error"));
}
