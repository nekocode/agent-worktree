// ===========================================================================
// Integration Tests - Snap Mode
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_new_with_snap_outputs_two_lines() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "snap-test",
            "-s",
            "echo hello",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(output.status.success());
    let content = read_path_file(&path_file);
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("snap-test"));
    assert_eq!(lines[1], "echo hello");

    let _ = Command::new(wt_binary())
        .args(["rm", "snap-test", "-f"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output();
}

#[test]
fn test_new_with_snap_creates_metadata() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "snap-meta-test",
            "-s",
            "agent",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(output.status.success());

    let workspaces_dir = home.join(".agent-worktree").join("workspaces");
    let workspace_dir = std::fs::read_dir(&workspaces_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().starts_with("repo-"))
        .expect("workspace directory not found");
    let meta_path = workspace_dir.path().join("snap-meta-test.toml");
    assert!(meta_path.exists());

    let content = std::fs::read_to_string(&meta_path).unwrap();
    assert!(content.contains("snap"));
    assert!(content.contains("agent"));

    let _ = Command::new(wt_binary())
        .args(["rm", "snap-meta-test", "-f"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output();
}

#[test]
fn test_snap_continue_not_in_worktree() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["snap-continue"])
        .current_dir(dir.path())
        .output()
        .expect("wt snap-continue failed");

    assert!(!output.status.success(), "snap-continue should fail outside worktree");
}
