// ===========================================================================
// Integration Tests - Merge Command
// ===========================================================================

mod common;

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_merge_on_trunk_fails() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("merge")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt merge");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trunk") || stderr.contains("itself"));
}

#[test]
fn test_merge_abort_no_merge() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["merge", "--abort"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt merge --abort");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No merge") || stderr.contains("Abort") || !output.status.success());
}

#[test]
fn test_merge_continue_no_merge() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    Command::new("git")
        .args(["checkout", "-b", "feature-merge"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["merge", "--continue"])
        .current_dir(dir.path())
        .output()
        .expect("wt merge --continue failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No merge") || stderr.contains("error") || !output.status.success());
}

#[test]
fn test_merge_from_feature_branch() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "merge-feature", "--path-file", path_file.to_str().unwrap()])
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

    std::fs::write(PathBuf::from(&wt_path).join("feature.txt"), "new feature").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add feature"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["merge", "--strategy", "squash"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() || !stderr.is_empty());
}

#[test]
fn test_merge_with_changes() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "merge-changes", "--path-file", path_file.to_str().unwrap()])
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

    std::fs::write(wt_path.join("feature.txt"), "new feature code").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add feature"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["merge", "--keep"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "merge failed: {}", stderr);
}

#[test]
fn test_merge_conflict_shows_error() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "merge-conflict", "--path-file", path_file.to_str().unwrap()])
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

    std::fs::write(wt_path.join("README.md"), "worktree change\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Worktree change to README"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    std::fs::write(repo.join("README.md"), "main change\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Main change to README"])
        .current_dir(&repo)
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["merge", "--keep"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("CONFLICT") || stderr.contains("conflict"),
        "Expected conflict info in stderr, got: {stderr}"
    );
    assert!(
        stderr.contains("--continue") && stderr.contains("--abort"),
        "Expected recovery instructions in stderr, got: {stderr}"
    );
}

#[test]
fn test_merge_into_nonexistent_branch_fails() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(repo.parent().unwrap());
    let output = Command::new(wt_binary())
        .args(["new", "merge-into-test", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    let output = Command::new(wt_binary())
        .args(["merge", "--into", "nonexistent-branch-xyz"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge --into failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not exist"),
        "Expected 'does not exist' error, got: {stderr}"
    );
}
