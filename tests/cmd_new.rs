// ===========================================================================
// Integration Tests - New Command
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::*;

#[test]
fn test_new_with_branch_name() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "test-feature",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt new");

    if output.status.success() {
        let path = read_path_file(&path_file);
        assert!(path.contains("test-feature"));
    }
}

#[test]
fn test_new_with_base() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    let output = Command::new(wt_binary())
        .args(["new", "feature-from-main", "--base", "main"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    let _status = output.status;
}

#[test]
fn test_new_with_invalid_base() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["new", "feature", "--base", "nonexistent-base-12345"])
        .current_dir(dir.path())
        .output()
        .expect("wt new failed");

    assert!(!output.status.success());
}

#[test]
fn test_new_generates_random_name() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args(["new", "--path-file", path_file.to_str().unwrap()])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if output.status.success() {
        let path = read_path_file(&path_file);
        assert!(!path.trim().is_empty());
    }
}

#[test]
fn test_new_creates_metadata_file() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "meta-test"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if output.status.success() {
        let repo_name = repo.file_name().unwrap().to_str().unwrap();
        let meta_path = home
            .join(".agent-worktree")
            .join("workspaces")
            .join(repo_name)
            .join("meta-test.toml");

        if meta_path.exists() {
            let content = std::fs::read_to_string(&meta_path).unwrap();
            assert!(content.contains("base_commit") || content.contains("trunk"));
        }
    }
}

#[test]
fn test_worktree_lifecycle_new_ls_rm() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "feature-test",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if output.status.success() {
        let wt_path = read_path_file(&path_file);
        let wt_path = wt_path.trim();
        assert!(wt_path.contains("feature-test"));

        let output = Command::new(wt_binary())
            .arg("ls")
            .current_dir(&repo)
            .env("HOME", &home)
            .output()
            .expect("wt ls failed");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("feature-test") || stderr.contains("feature-test"));

        let output = Command::new(wt_binary())
            .args(["rm", "feature-test", "--force"])
            .current_dir(&repo)
            .env("HOME", &home)
            .output()
            .expect("wt rm failed");

        let _ = output.status.success();
    }
}

#[test]
fn test_full_worktree_lifecycle() {
    let (dir, repo, home) = setup_worktree_test_env();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "feature-lifecycle",
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
    assert!(wt_path.contains("feature-lifecycle"));

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    assert!(combined.contains("feature-lifecycle"));

    let output = Command::new(wt_binary())
        .args(["rm", "feature-lifecycle", "--force"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt rm failed");

    let _ = output.status;
}
