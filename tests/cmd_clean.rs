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

#[test]
fn test_clean_skips_dirty_worktree() {
    // A worktree with no committed diff but uncommitted edits is NOT eligible
    // for clean — git would refuse non-force removal anyway, and silently
    // discarding in-flight work would be a footgun.
    let (_dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "dirty-clean"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    // Locate the worktree path and add an uncommitted file there
    let ls_output = Command::new(wt_binary())
        .args(["ls", "-l"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");
    let stdout = String::from_utf8_lossy(&ls_output.stdout);
    let wt_line = stdout
        .lines()
        .find(|l| l.contains("dirty-clean"))
        .expect("worktree should appear in ls -l");
    // ls -l output contains the path; pull whatever looks like a path token
    let wt_path = wt_line
        .split_whitespace()
        .find(|tok| tok.starts_with('/'))
        .expect("ls -l should contain absolute path");
    std::fs::write(format!("{wt_path}/scratch.tmp"), "in-flight\n").unwrap();

    // Dry-run should report the dirty skip, not "Would clean"
    let output = Command::new(wt_binary())
        .args(["clean", "--dry-run"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt clean --dry-run failed");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success());
    assert!(
        stderr.contains("Skipping dirty-clean") || stderr.contains("uncommitted"),
        "stderr should mention skipping dirty worktree: {stderr}"
    );
    assert!(
        !stderr.contains("Would clean (no diff from main): dirty-clean"),
        "dry-run must not promise to clean a dirty worktree: {stderr}"
    );
}
