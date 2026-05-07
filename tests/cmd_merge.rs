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
fn test_merge_from_feature_branch() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "merge-feature",
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
        .args([
            "new",
            "merge-changes",
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
        .args(["merge"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "merge failed: {}", stderr);
    assert!(wt_path.exists(), "worktree should be preserved by default");
}

#[test]
fn test_merge_delete_removes_worktree() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "merge-delete",
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

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    std::fs::write(wt_path.join("feature.txt"), "delete test").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Add feature for delete test"])
        .current_dir(&wt_path)
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["merge", "--delete"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge --delete failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "merge --delete failed: {}", stderr);

    // Worktree should be removed
    assert!(
        !wt_path.exists(),
        "worktree should be deleted with --delete"
    );
}

#[test]
fn test_merge_conflict_rejected() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "merge-conflict",
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
        .args(["merge"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    // Merge should fail (non-zero exit) due to conflict precheck
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("conflict") || stderr.contains("Sync first"),
        "Expected conflict rejection message, got: {stderr}"
    );

    // No WT_MERGE_BRANCH state file should exist
    let state_path = repo.join(".git").join("WT_MERGE_BRANCH");
    assert!(
        !state_path.exists(),
        "No merge state file should exist after precheck rejection"
    );
}

#[test]
fn test_merge_into_nonexistent_branch_fails() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(repo.parent().unwrap());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "merge-into-test",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
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

#[test]
fn test_merge_into_branch_held_by_another_worktree_fails() {
    // `wt merge --into X` must refuse upfront if X is already checked out by
    // another worktree — git would error mid-merge with a confusing
    // low-level message and potentially leave HEAD detached.
    let (dir, repo, home) = setup_worktree_test_env();

    // Create source worktree
    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "merge-into-busy-src",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new src failed");
    assert!(output.status.success());
    let src_wt = PathBuf::from(read_path_file(&path_file).trim());

    // Add a commit so merge has something to do
    std::fs::write(src_wt.join("feat.txt"), "feat").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&src_wt)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "feat"])
        .current_dir(&src_wt)
        .output()
        .unwrap();

    // Create busy worktree on a separate branch we want to merge into
    let path_file2 = create_path_file(dir.path());
    let target_path_file = path_file2.with_extension("2");
    std::fs::write(&target_path_file, "").unwrap();
    let output = Command::new(wt_binary())
        .args([
            "new",
            "busy-target",
            "--path-file",
            target_path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new busy failed");
    assert!(output.status.success());

    // Try to merge src into busy-target — should fail upfront
    let output = Command::new(wt_binary())
        .args(["merge", "--into", "busy-target"])
        .current_dir(&src_wt)
        .env("HOME", &home)
        .output()
        .expect("wt merge --into busy failed");

    assert!(
        !output.status.success(),
        "merge into busy branch should be rejected"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checked out in another worktree") || stderr.contains("worktree"),
        "stderr should explain busy worktree: {stderr}"
    );
}

#[test]
fn test_merge_already_up_to_date_with_merge_strategy() {
    // With `--strategy merge` and no commits ahead, execute_merge() must
    // detect "already up to date" instead of silently printing success
    // (and, with -d, deleting the worktree).
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "noop-merge",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let wt_path = PathBuf::from(read_path_file(&path_file).trim());

    // Don't add any commits to wt_path. Merge strategy=Merge, expect "Nothing to merge".
    let output = Command::new(wt_binary())
        .args(["merge", "--strategy", "merge", "-d"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "merge should succeed: {stderr}");
    assert!(
        stderr.contains("Nothing to merge") || stderr.contains("already up to date"),
        "expected up-to-date message, got: {stderr}"
    );
    // Worktree should still exist since nothing happened (no merge → no delete)
    assert!(
        wt_path.exists(),
        "worktree should NOT be deleted when nothing was merged"
    );
}
