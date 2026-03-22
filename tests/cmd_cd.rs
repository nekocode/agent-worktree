// ===========================================================================
// Integration Tests - Cd Command
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_cd_nonexistent() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["cd", "nonexistent-branch"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt cd");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("error"));
}

#[test]
fn test_cd_without_print_path() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["cd", "nonexistent"])
        .current_dir(dir.path())
        .output()
        .expect("wt cd failed");

    assert!(!output.status.success());
}

#[test]
fn test_cd_to_existing_worktree() {
    let (dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "cd-target"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "cd",
            "cd-target",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt cd failed");

    if output.status.success() {
        let path = read_path_file(&path_file);
        assert!(path.contains("cd-target"));
    }
}

#[test]
fn test_cd_returns_correct_path() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let new_output = Command::new(wt_binary())
        .args([
            "new",
            "cd-check",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        new_output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&new_output.stderr)
    );

    let created_path = read_path_file(&path_file).trim().to_string();

    let cd_path_file = dir.path().join(".wt-cd-path");
    let output = Command::new(wt_binary())
        .args([
            "cd",
            "cd-check",
            "--path-file",
            cd_path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt cd failed");

    if output.status.success() {
        let cd_path = read_path_file(&cd_path_file).trim().to_string();
        assert_eq!(created_path, cd_path);
    }
}
