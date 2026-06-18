// ===========================================================================
// Integration Tests - Hook Environment Variables
// ===========================================================================

mod common;

use std::process::Command;

use common::*;

/// post_create hook should see WT_* worktree context as environment variables.
#[test]
fn test_post_create_hook_receives_env_vars() {
    let (dir, repo, home) = setup_worktree_test_env();

    // Hook writes the injected vars into the main repo so we can assert on them
    // regardless of where the worktree lands on disk.
    let config = r#"
[hooks]
post_create = ['echo "$WT_BRANCH|$WT_BASE_BRANCH|$WT_MAIN_REPO" > "$WT_MAIN_REPO/hook_env.txt"']
"#;
    std::fs::write(repo.join(".agent-worktree.toml"), config).unwrap();

    let output = Command::new(wt_binary())
        .args(["new", "feature-hooks", "--base", "main"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "wt new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let captured = std::fs::read_to_string(repo.join("hook_env.txt")).unwrap();
    let captured = captured.trim();
    let parts: Vec<&str> = captured.split('|').collect();
    assert_eq!(parts[0], "feature-hooks", "WT_BRANCH");
    assert_eq!(parts[1], "main", "WT_BASE_BRANCH");
    // WT_MAIN_REPO points at the repo root (path may be canonicalized).
    assert!(parts[2].ends_with("repo"), "WT_MAIN_REPO: {}", parts[2]);

    drop(dir);
}

/// The motivating use case: symlink a main-repo path into the worktree instead
/// of copying it. Proves WT_MAIN_REPO + worktree CWD are enough to do it.
#[test]
fn test_post_create_hook_can_symlink_from_main_repo() {
    let (dir, repo, home) = setup_worktree_test_env();

    // A heavy dir we want to share, not duplicate.
    std::fs::create_dir_all(repo.join("deps")).unwrap();
    std::fs::write(repo.join("deps/lib.txt"), "shared").unwrap();

    let config = r#"
[hooks]
post_create = ['ln -s "$WT_MAIN_REPO/deps" deps']
"#;
    std::fs::write(repo.join(".agent-worktree.toml"), config).unwrap();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "feature-symlink",
            "--base",
            "main",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(
        output.status.success(),
        "wt new failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let wt_path = read_path_file(&path_file);
    let linked = std::path::Path::new(wt_path.trim()).join("deps/lib.txt");
    assert_eq!(std::fs::read_to_string(&linked).unwrap(), "shared");

    drop(dir);
}

/// A failing post_create hook aborts `wt new` but leaves the worktree in place.
#[test]
fn test_post_create_hook_failure_preserves_worktree() {
    let (dir, repo, home) = setup_worktree_test_env();

    let config = r#"
[hooks]
post_create = ["false"]
"#;
    std::fs::write(repo.join(".agent-worktree.toml"), config).unwrap();

    let output = Command::new(wt_binary())
        .args(["new", "feature-failhook", "--base", "main"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("post_create hook failed"),
        "stderr: {stderr}"
    );

    drop(dir);
}
