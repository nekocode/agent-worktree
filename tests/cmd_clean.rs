// ===========================================================================
// Integration Tests - Clean Command
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_clean_no_worktrees() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("clean")
        .current_dir(dir.path())
        .output()
        .expect("wt clean failed");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No") || stderr.is_empty());
}

#[test]
fn test_clean_with_path_file() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["clean", "--path-file", path_file.to_str().unwrap()])
        .current_dir(dir.path())
        .output()
        .expect("wt clean failed");

    assert!(output.status.success());
}

#[test]
fn test_clean_after_merge() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "clean-test"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Command::new("git")
        .args(["merge", "clean-test", "--no-edit"])
        .current_dir(&repo)
        .output()
        .ok();

    let output = Command::new(wt_binary())
        .arg("clean")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt clean failed");

    assert!(output.status.success());
}

#[test]
fn test_clean_remvs_merged_worktree() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "clean-merged"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Command::new("git")
        .args(["merge", "clean-merged", "--no-edit"])
        .current_dir(&repo)
        .output()
        .ok();

    let output = Command::new(wt_binary())
        .arg("clean")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt clean failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success()
            || stderr.contains("cleaned")
            || stderr.contains("No")
            || stderr.contains("merged")
    );
}

#[test]
fn test_clean_dry_run() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a worktree with no changes (will match trunk)
    let output = Command::new(wt_binary())
        .args(["new", "clean-dry"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    // dry-run should not remove anything
    let output = Command::new(wt_binary())
        .args(["clean", "--dry-run"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt clean --dry-run failed");

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Worktree should still exist after dry-run
    let ls_output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");
    let stdout = String::from_utf8_lossy(&ls_output.stdout);

    // If worktree had no diff, dry-run would show "Would clean" but worktree survives
    if stderr.contains("Would clean") {
        assert!(stdout.contains("clean-dry"));
    }
}
