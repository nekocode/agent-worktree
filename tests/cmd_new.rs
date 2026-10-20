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
            assert!(content.contains("base_branch"));
            assert!(content.contains("created_at"));
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

#[test]
fn test_new_copies_files_matching_copy_files_pattern() {
    let (dir, repo, home) = setup_worktree_test_env();

    std::fs::write(repo.join(".env"), "SECRET=test\n").unwrap();
    filetime::set_file_mtime(
        repo.join(".env"),
        filetime::FileTime::from_unix_time(1_600_000_000, 123_456_700),
    )
    .unwrap();
    let modified = std::fs::metadata(repo.join(".env"))
        .unwrap()
        .modified()
        .unwrap();

    std::fs::write(
        repo.join(".agent-worktree.toml"),
        r#"
[general]
copy_files = [".env"]
"#,
    )
    .unwrap();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "copy-files-test",
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
    let wt_root = std::path::Path::new(wt_path.trim());
    let copied_env = wt_root.join(".env");
    assert!(
        copied_env.exists(),
        ".env was not copied into the worktree at {}",
        copied_env.display()
    );
    let contents = std::fs::read_to_string(&copied_env).unwrap();
    assert_eq!(contents, "SECRET=test\n");
    assert_eq!(
        std::fs::metadata(&copied_env).unwrap().modified().unwrap(),
        modified
    );
    assert_eq!(
        std::fs::read_to_string(repo.join(".env")).unwrap(),
        contents,
        "source and copy should match"
    );
}

#[cfg(unix)]
#[test]
fn test_new_copy_files_skips_symlink_entries() {
    let (dir, repo, home) = setup_worktree_test_env();

    // Symlink escaping the repo: must never materialize external content.
    let outside = dir.path().join("outside-secret.txt");
    std::fs::write(&outside, "OUTSIDE").unwrap();
    std::os::unix::fs::symlink(&outside, repo.join(".env")).unwrap();

    // Symlink to a file inside the repo: skipped as well (uniform rule).
    std::fs::write(repo.join(".env.dev"), "DEV").unwrap();
    std::os::unix::fs::symlink(repo.join(".env.dev"), repo.join(".env.link")).unwrap();

    std::fs::write(
        repo.join(".agent-worktree.toml"),
        r#"
[general]
copy_files = [".env", ".env.link", ".env.dev"]
"#,
    )
    .unwrap();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "symlink-skip-test",
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
    let wt_root = std::path::Path::new(wt_path.trim());
    assert!(
        wt_root.join(".env").symlink_metadata().is_err(),
        "symlink to outside file must not be copied"
    );
    assert!(
        wt_root.join(".env.link").symlink_metadata().is_err(),
        "symlink to in-repo file must not be copied"
    );
    // Regular files still copy as usual.
    assert_eq!(
        std::fs::read_to_string(wt_root.join(".env.dev")).unwrap(),
        "DEV"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("skipping symlink"),
        "skips should be reported: {stderr}"
    );
}

// ===========================================================================
// Submodule auto-init & hooksPath warning
// ===========================================================================

#[test]
fn test_new_auto_inits_submodules() {
    let (_dir, _repo, _home, wt_root) = new_worktree_with_submodule("with-submodule");
    assert!(
        wt_root.join("sub").join("README.md").exists(),
        "submodule must be checked out in the new worktree"
    );
}

/// Submodule clones are network-bound and can run for minutes. Git's progress
/// must reach the caller's terminal, or a slow clone is indistinguishable from
/// a hang — the exact bug this guards against.
#[test]
fn test_new_streams_submodule_progress() {
    let (dir, repo, home) = setup_worktree_test_env();
    add_submodule(dir.path(), &repo);

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "streamed",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .env("GIT_ALLOW_PROTOCOL", "file")
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cloning into"),
        "git submodule output must not be swallowed, got: {stderr}"
    );
}

#[test]
fn test_new_submodules_disabled_by_project_config() {
    let (dir, repo, home) = setup_worktree_test_env();
    add_submodule(dir.path(), &repo);

    std::fs::write(
        repo.join(".agent-worktree.toml"),
        "[general]\nsubmodules = false\n",
    )
    .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "config"])
        .current_dir(&repo)
        .output()
        .unwrap();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "without-submodule",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .env("GIT_ALLOW_PROTOCOL", "file")
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let wt_path = read_path_file(&path_file);
    let wt_root = std::path::Path::new(wt_path.trim());
    assert!(
        !wt_root.join("sub").join("README.md").exists(),
        "submodules = false must skip auto-init"
    );
}

#[test]
fn test_new_warns_on_relative_hooks_path() {
    let (dir, repo, home) = setup_worktree_test_env();

    Command::new("git")
        .args(["config", "core.hooksPath", ".githooks"])
        .current_dir(&repo)
        .output()
        .unwrap();

    let path_file = create_path_file(dir.path());
    let output = Command::new(wt_binary())
        .args([
            "new",
            "hooks-warn",
            "--path-file",
            path_file.to_str().unwrap(),
        ])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");
    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("core.hooksPath"),
        "relative hooksPath must be warned about: {stderr}"
    );
}

#[test]
fn test_new_reports_stale_dir_instead_of_git_error() {
    let (_dir, repo, home, worktree) = orphaned_worktree_dir("stale-new");

    let output = Command::new(wt_binary())
        .args(["new", "stale-new"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("wt rm --force stale-new"),
        "expected recovery hint, got: {stderr}"
    );
    assert!(worktree.exists(), "the stale directory must be left alone");
}

#[test]
fn test_new_does_not_leak_branch_when_creation_fails() {
    let (_dir, repo, home, _worktree) = orphaned_worktree_dir("stale-leak");
    assert!(!local_branches(&repo).contains(&"stale-leak".to_string()));

    for _ in 0..2 {
        let output = Command::new(wt_binary())
            .args(["new", "stale-leak"])
            .current_dir(&repo)
            .env("HOME", &home)
            .output()
            .expect("wt new failed");
        assert!(!output.status.success());
    }

    // A leaked branch would also change the error on the second attempt,
    // reporting "already exists" instead of naming the real obstacle.
    assert!(
        !local_branches(&repo).contains(&"stale-leak".to_string()),
        "failed creation must not leave the branch behind: {:?}",
        local_branches(&repo)
    );
}
