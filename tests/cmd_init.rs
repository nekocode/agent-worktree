// ===========================================================================
// Integration Tests - Init Command
// ===========================================================================

mod common;

use std::process::Command;
use tempfile::tempdir;

use common::{setup_git_repo, wt_binary};

#[test]
fn test_init_creates_config() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["init", "--trunk", "develop"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt init");

    assert!(output.status.success());

    let config_path = dir.path().join(".agent-worktree.toml");
    assert!(config_path.exists());

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("trunk"));
    assert!(content.contains("develop"));
}

#[test]
fn test_init_already_exists() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    Command::new(wt_binary())
        .arg("init")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt init");

    let output = Command::new(wt_binary())
        .arg("init")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt init");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn test_init_with_default_trunk() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("init")
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let config_path = dir.path().join(".agent-worktree.toml");
    assert!(config_path.exists());

    let content = std::fs::read_to_string(&config_path).unwrap();
    assert!(content.contains("trunk") && content.contains("main"));
}

#[test]
fn test_init_with_custom_trunk() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["init", "--trunk", "develop"])
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join(".agent-worktree.toml")).unwrap();
    assert!(content.contains("develop"));
}

#[test]
fn test_init_multiple_options() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["init", "--trunk", "master"])
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join(".agent-worktree.toml")).unwrap();
    assert!(content.contains("master"));
}

#[test]
fn test_init_with_merge_strategy() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["init", "--merge-strategy", "rebase"])
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join(".agent-worktree.toml")).unwrap();
    assert!(
        content.contains("rebase"),
        "Expected rebase in config, got: {content}"
    );
}

#[test]
fn test_init_with_copy_files() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["init", "--copy-files", ".env", "--copy-files", ".env.*"])
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join(".agent-worktree.toml")).unwrap();
    assert!(
        content.contains(".env"),
        "Expected .env in config, got: {content}"
    );
}

#[test]
fn test_init_with_all_options() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args([
            "init",
            "--trunk",
            "develop",
            "--merge-strategy",
            "squash",
            "--copy-files",
            ".env",
        ])
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join(".agent-worktree.toml")).unwrap();
    assert!(content.contains("develop"));
    assert!(content.contains("squash"));
    assert!(content.contains(".env"));
}
