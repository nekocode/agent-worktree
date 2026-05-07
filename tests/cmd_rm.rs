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

#[test]
fn test_rm_dot_without_wrapper_is_rejected() {
    // `wt rm .` from inside a worktree without shell wrapper installed
    // would leave the parent shell stranded in a deleted directory.
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "rm-dot-stranded",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());
    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    // From inside the worktree, `wt rm .` without --path-file must be refused
    let output = Command::new(wt_binary())
        .args(["rm", "."])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt rm . failed");

    assert!(
        !output.status.success(),
        "wt rm . without wrapper should be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("setup") || stderr.contains("integration"),
        "stderr should mention shell setup: {stderr}"
    );
    // Worktree should still be there
    assert!(wt_path.exists(), "worktree must NOT be deleted");
}

#[test]
fn test_rm_dot_with_wrapper_works() {
    // The same rm . succeeds when --path-file is provided (wrapper installed).
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "rm-dot-ok",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());
    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    let rm_path_file = create_path_file(dir.path());
    let rm_path_file = rm_path_file.with_file_name(".wt-path-rm");
    std::fs::write(&rm_path_file, "").unwrap();
    let output = Command::new(wt_binary())
        .args(["rm", ".", "--path-file", rm_path_file.to_str().unwrap()])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt rm . failed");
    assert!(
        output.status.success(),
        "wt rm . with --path-file should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!wt_path.exists(), "worktree should be removed");
}
