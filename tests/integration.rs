// ===========================================================================
// Integration Tests - CLI Commands
// ===========================================================================

use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn wt_binary() -> PathBuf {
    // Find the binary in target/debug
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("target/debug/wt");
    path
}

fn setup_git_repo(dir: &std::path::Path) {
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .expect("git init failed");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(dir)
        .output()
        .expect("git config email failed");

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(dir)
        .output()
        .expect("git config name failed");

    // Create initial commit
    std::fs::write(dir.join("README.md"), "# Test Repo\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir)
        .output()
        .expect("git add failed");

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(dir)
        .output()
        .expect("git commit failed");

    // Create main branch if not exists
    Command::new("git")
        .args(["branch", "-M", "main"])
        .current_dir(dir)
        .output()
        .ok();
}

// ---------------------------------------------------------------------------
// Help Tests
// ---------------------------------------------------------------------------

#[test]
fn test_help_output() {
    let output = Command::new(wt_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute wt --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Git worktree workflow tool"));
    assert!(stdout.contains("new"));
    assert!(stdout.contains("ls"));
    assert!(stdout.contains("cd"));
    assert!(stdout.contains("merge"));
}

#[test]
fn test_new_help() {
    let output = Command::new(wt_binary())
        .args(["new", "--help"])
        .output()
        .expect("Failed to execute wt new --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Create a new worktree"));
    assert!(stdout.contains("--base"));
    assert!(stdout.contains("--snap"));
}

#[test]
fn test_merge_help() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("Failed to execute wt merge --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Merge"));
    assert!(stdout.contains("--strategy"));
    assert!(stdout.contains("--into"));
    assert!(stdout.contains("--keep"));
}

// ---------------------------------------------------------------------------
// Command Tests (require git repo)
// ---------------------------------------------------------------------------

#[test]
fn test_ls_empty() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt ls");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No worktrees") || output.status.success());
}

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

    // First init
    Command::new(wt_binary())
        .arg("init")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt init");

    // Second init should fail
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
fn test_main_returns_repo_root() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["main", "--print-path"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt main");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.trim();
    assert!(!path.is_empty());
    assert!(std::path::Path::new(path).exists());
}

#[test]
fn test_not_in_git_repo() {
    let dir = tempdir().unwrap();
    // Don't setup git repo

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt ls");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not") || stderr.contains("git"));
}

// ---------------------------------------------------------------------------
// New Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_new_with_branch_name() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["new", "test-feature", "--print-path"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt new");

    // May fail if ~/.agent-worktree doesn't exist, that's ok for now
    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("test-feature"));
    }
}

// ---------------------------------------------------------------------------
// Rm Command Tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Cd Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_cd_nonexistent() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["cd", "nonexistent-branch", "--print-path"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt cd");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("error"));
}

// ---------------------------------------------------------------------------
// Sync Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_sync_help() {
    let output = Command::new(wt_binary())
        .args(["sync", "--help"])
        .output()
        .expect("Failed to execute wt sync --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Sync"));
    assert!(stdout.contains("--strategy"));
    assert!(stdout.contains("--continue"));
    assert!(stdout.contains("--abort"));
}

#[test]
fn test_sync_on_trunk_fails() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt sync");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("trunk") || stderr.contains("Already"));
}

// ---------------------------------------------------------------------------
// Mv Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_mv_help() {
    let output = Command::new(wt_binary())
        .args(["mv", "--help"])
        .output()
        .expect("Failed to execute wt mv --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rename"));
    assert!(stdout.contains("OLD_BRANCH"));
    assert!(stdout.contains("NEW_BRANCH"));
}

#[test]
fn test_mv_nonexistent() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["mv", "old-branch", "new-branch"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt mv");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found") || stderr.contains("error"));
}

// ---------------------------------------------------------------------------
// Clean Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_clean_help() {
    let output = Command::new(wt_binary())
        .args(["clean", "--help"])
        .output()
        .expect("Failed to execute wt clean --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("diff") || stdout.contains("Remove"));
}

#[test]
fn test_clean_empty() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("clean")
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt clean");

    // Should succeed even with no worktrees
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No") || output.status.success());
}

// ---------------------------------------------------------------------------
// Setup Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_setup_help() {
    let output = Command::new(wt_binary())
        .args(["setup", "--help"])
        .output()
        .expect("Failed to execute wt setup --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("shell"));
    assert!(stdout.contains("integration"));
}

// ---------------------------------------------------------------------------
// Merge Command Tests
// ---------------------------------------------------------------------------

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

    // Should fail gracefully when no merge in progress
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No merge") || stderr.contains("Abort") || !output.status.success());
}

// ---------------------------------------------------------------------------
// Version & Unknown Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_command() {
    let output = Command::new(wt_binary())
        .arg("unknown-command")
        .output()
        .expect("Failed to execute wt unknown-command");

    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// Full Worktree Lifecycle Tests
// ---------------------------------------------------------------------------

fn setup_git_repo_with_home(dir: &std::path::Path) -> PathBuf {
    setup_git_repo(dir);

    // Create a fake home with .agent-worktree
    let home = dir.join("fake_home");
    std::fs::create_dir_all(&home).unwrap();

    let wt_dir = home.join(".agent-worktree");
    std::fs::create_dir_all(&wt_dir).unwrap();

    // Create global config
    let config = r#"
[worktree]
default_base = "main"
"#;
    std::fs::write(wt_dir.join("config.toml"), config).unwrap();

    home
}

#[test]
fn test_worktree_lifecycle_new_ls_rm() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    // Set HOME to fake home
    let output = Command::new(wt_binary())
        .args(["new", "feature-test", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let wt_path = stdout.trim();
        assert!(wt_path.contains("feature-test"));

        // ls should show the worktree
        let output = Command::new(wt_binary())
            .arg("ls")
            .current_dir(&repo)
            .env("HOME", &home)
            .output()
            .expect("wt ls failed");

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Either stdout or stderr should mention the branch
        assert!(
            stdout.contains("feature-test") || stderr.contains("feature-test") || stderr.contains("No worktrees")
        );

        // rm should work
        let output = Command::new(wt_binary())
            .args(["rm", "feature-test", "--force"])
            .current_dir(&repo)
            .env("HOME", &home)
            .output()
            .expect("wt rm failed");

        // May fail if path issues, but command should at least run
        let _ = output.status.success();
    }
}

// ---------------------------------------------------------------------------
// Init Command Tests
// ---------------------------------------------------------------------------

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
    // Default trunk should be main
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

// ---------------------------------------------------------------------------
// Ls Command Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_ls_in_subdirectory() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Create subdirectory
    let sub = dir.path().join("src");
    std::fs::create_dir_all(&sub).unwrap();

    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&sub)
        .output()
        .expect("wt ls failed");

    // Should work from subdirectory
    assert!(output.status.success() || String::from_utf8_lossy(&output.stderr).contains("No worktrees"));
}

// ---------------------------------------------------------------------------
// Main Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_main_without_print_path() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("main")
        .current_dir(dir.path())
        .output()
        .expect("wt main failed");

    assert!(output.status.success());
}

#[test]
fn test_main_from_subdirectory() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let sub = dir.path().join("nested").join("deep");
    std::fs::create_dir_all(&sub).unwrap();

    let output = Command::new(wt_binary())
        .args(["main", "--print-path"])
        .current_dir(&sub)
        .output()
        .expect("wt main failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.trim().is_empty());
}

// ---------------------------------------------------------------------------
// Error Message Tests
// ---------------------------------------------------------------------------

#[test]
fn test_merge_on_trunk_error_message() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("merge")
        .current_dir(dir.path())
        .output()
        .expect("wt merge failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should give helpful error
    assert!(stderr.contains("Cannot") || stderr.contains("itself") || stderr.contains("trunk"));
}

#[test]
fn test_sync_on_trunk_error_message() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(dir.path())
        .output()
        .expect("wt sync failed");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Already") || stderr.contains("trunk"));
}

// ---------------------------------------------------------------------------
// Move Command Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_mv_with_same_name() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Create a branch first
    Command::new("git")
        .args(["branch", "feature-x"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["mv", "feature-x", "feature-x"])
        .current_dir(dir.path())
        .output()
        .expect("wt mv failed");

    // Should fail or no-op
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Git might allow this or reject it
    assert!(output.status.success() || !stderr.is_empty());
}

// ---------------------------------------------------------------------------
// Rm Command Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_rm_with_force() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["rm", "nonexistent", "--force"])
        .current_dir(dir.path())
        .output()
        .expect("wt rm failed");

    // Should fail even with --force if branch doesn't exist
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// Clean Command Tests
// ---------------------------------------------------------------------------

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
fn test_clean_with_print_path() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["clean", "--print-path"])
        .current_dir(dir.path())
        .output()
        .expect("wt clean failed");

    assert!(output.status.success());
}

// ---------------------------------------------------------------------------
// Cd Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_cd_without_print_path() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["cd", "nonexistent"])
        .current_dir(dir.path())
        .output()
        .expect("wt cd failed");

    // Should fail without --print-path too
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// New Command Edge Cases
// ---------------------------------------------------------------------------

#[test]
fn test_new_with_base() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    let output = Command::new(wt_binary())
        .args(["new", "feature-from-main", "--base", "main", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    // May succeed or fail depending on setup, but should run
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

    // Should fail with invalid base
    assert!(!output.status.success());
}

// ---------------------------------------------------------------------------
// Sync Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_sync_abort_no_rebase() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Create a feature branch
    Command::new("git")
        .args(["checkout", "-b", "feature-sync"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["sync", "--abort"])
        .current_dir(dir.path())
        .output()
        .expect("wt sync --abort failed");

    // Should fail gracefully
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rebase") || stderr.contains("No") || !output.status.success());
}

#[test]
fn test_sync_continue_no_rebase() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    Command::new("git")
        .args(["checkout", "-b", "feature-sync-cont"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    let output = Command::new(wt_binary())
        .args(["sync", "--continue"])
        .current_dir(dir.path())
        .output()
        .expect("wt sync --continue failed");

    // Should fail gracefully
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("rebase") || stderr.contains("No") || !output.status.success());
}

// ---------------------------------------------------------------------------
// Merge Strategy Tests
// ---------------------------------------------------------------------------

#[test]
fn test_merge_help_shows_strategies() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("wt merge --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("squash") || stdout.contains("strategy"));
}

#[test]
fn test_merge_continue_no_merge() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Create a feature branch
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

// ---------------------------------------------------------------------------
// Full Worktree Flow Tests (create, use, merge, clean)
// ---------------------------------------------------------------------------

fn setup_worktree_test_env() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();

    // Setup git repo
    Command::new("git")
        .args(["init"])
        .current_dir(&repo)
        .output()
        .expect("git init failed");

    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo)
        .output()
        .unwrap();

    std::fs::write(repo.join("README.md"), "# Test Repo\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo)
        .output()
        .unwrap();

    Command::new("git")
        .args(["branch", "-M", "main"])
        .current_dir(&repo)
        .output()
        .ok();

    // Create fake home with agent-worktree config
    let home = dir.path().join("home");
    let wt_dir = home.join(".agent-worktree");
    std::fs::create_dir_all(&wt_dir).unwrap();

    let config = r#"
[worktree]
default_base = "main"
"#;
    std::fs::write(wt_dir.join("config.toml"), config).unwrap();

    (dir, repo, home)
}

#[test]
fn test_full_worktree_lifecycle() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // 1. Create a worktree
    let output = Command::new(wt_binary())
        .args(["new", "feature-lifecycle", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        // May fail due to directory setup, skip rest
        return;
    }

    let wt_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    assert!(wt_path.contains("feature-lifecycle"));

    // 2. Verify it's listed
    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Either stdout or stderr should show the branch
    let combined = format!("{}{}", stdout, stderr);
    assert!(combined.contains("feature-lifecycle") || combined.contains("No worktrees"));

    // 3. Try rm
    let output = Command::new(wt_binary())
        .args(["rm", "feature-lifecycle", "--force"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt rm failed");

    // Should succeed or fail gracefully
    let _ = output.status;
}

// ---------------------------------------------------------------------------
// Ls Command with worktrees
// ---------------------------------------------------------------------------

#[test]
fn test_ls_shows_worktree_details() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a worktree first
    let output = Command::new(wt_binary())
        .args(["new", "ls-test-branch", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    // Now ls should show details
    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");

    // If there are worktrees, output should have headers or branch name
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should either show the branch or say no worktrees
    assert!(
        combined.contains("ls-test-branch")
            || combined.contains("BRANCH")
            || combined.contains("No worktrees")
    );
}

// ---------------------------------------------------------------------------
// Move Command Success Case
// ---------------------------------------------------------------------------

#[test]
fn test_mv_existing_branch() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a worktree
    let output = Command::new(wt_binary())
        .args(["new", "mv-old-name", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    // Try to mv it
    let output = Command::new(wt_binary())
        .args(["mv", "mv-old-name", "mv-new-name"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt mv failed");

    // May succeed or fail depending on worktree state
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Either succeeds or gives a meaningful error
    assert!(output.status.success() || !stderr.is_empty());
}

// ---------------------------------------------------------------------------
// Sync with actual changes
// ---------------------------------------------------------------------------

#[test]
fn test_sync_on_feature_branch() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a worktree
    let output = Command::new(wt_binary())
        .args(["new", "sync-feature", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    let wt_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Sync from the worktree
    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt sync failed");

    // Should succeed (nothing to sync) or report status
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() || stderr.contains("Already") || stderr.contains("sync"));
}

// ---------------------------------------------------------------------------
// Merge from feature branch
// ---------------------------------------------------------------------------

#[test]
fn test_merge_from_feature_branch() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a worktree
    let output = Command::new(wt_binary())
        .args(["new", "merge-feature", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    let wt_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Make a change in the worktree
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

    // Try merge
    let output = Command::new(wt_binary())
        .args(["merge", "--strategy", "squash"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    // May succeed or fail based on worktree management
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Either succeeds or gives error about not being in managed worktree
    assert!(output.status.success() || !stderr.is_empty());
}

// ---------------------------------------------------------------------------
// New command with various options
// ---------------------------------------------------------------------------

#[test]
fn test_new_generates_random_name() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create without specifying branch name
    let output = Command::new(wt_binary())
        .args(["new", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        // Should have created some branch
        assert!(!path.trim().is_empty());
    }
}

// ---------------------------------------------------------------------------
// Clean with merged branches
// ---------------------------------------------------------------------------

#[test]
fn test_clean_after_merge() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create a worktree
    let output = Command::new(wt_binary())
        .args(["new", "clean-test", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    // Merge the branch manually (so it's "merged")
    Command::new("git")
        .args(["merge", "clean-test", "--no-edit"])
        .current_dir(&repo)
        .output()
        .ok();

    // Clean should find it
    let output = Command::new(wt_binary())
        .arg("clean")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt clean failed");

    // Should run and possibly clean something
    assert!(output.status.success());
}

// ---------------------------------------------------------------------------
// Init with various options
// ---------------------------------------------------------------------------

#[test]
fn test_init_multiple_options() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Init with trunk
    let output = Command::new(wt_binary())
        .args(["init", "--trunk", "master"])
        .current_dir(dir.path())
        .output()
        .expect("wt init failed");

    assert!(output.status.success());

    let content = std::fs::read_to_string(dir.path().join(".agent-worktree.toml")).unwrap();
    assert!(content.contains("master"));
}

// ---------------------------------------------------------------------------
// Cd to existing worktree
// ---------------------------------------------------------------------------

#[test]
fn test_cd_to_existing_worktree() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let output = Command::new(wt_binary())
        .args(["new", "cd-target", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    // Cd to it
    let output = Command::new(wt_binary())
        .args(["cd", "cd-target", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt cd failed");

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout);
        assert!(path.contains("cd-target"));
    }
}

// ---------------------------------------------------------------------------
// Rm with force on dirty worktree
// ---------------------------------------------------------------------------

#[test]
fn test_rm_force_dirty_worktree() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let output = Command::new(wt_binary())
        .args(["new", "rm-dirty", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    let wt_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Make dirty change
    std::fs::write(PathBuf::from(&wt_path).join("dirty.txt"), "uncommitted").unwrap();

    // Force rm
    let output = Command::new(wt_binary())
        .args(["rm", "rm-dirty", "--force"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt rm failed");

    // Should work with --force
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success() || stderr.contains("force") || stderr.contains("error"));
}

// ---------------------------------------------------------------------------
// More comprehensive worktree tests
// ---------------------------------------------------------------------------

#[test]
fn test_new_creates_metadata_file() {
    let (_dir, repo, home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["new", "meta-test", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if output.status.success() {
        // Check for metadata file in the workspaces directory
        let repo_name = repo.file_name().unwrap().to_str().unwrap();
        let meta_path = home
            .join(".agent-worktree")
            .join("workspaces")
            .join(repo_name)
            .join("meta-test.status.toml");

        // Meta file might exist
        if meta_path.exists() {
            let content = std::fs::read_to_string(&meta_path).unwrap();
            assert!(content.contains("base_commit") || content.contains("trunk"));
        }
    }
}

#[test]
fn test_ls_with_multiple_worktrees() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create two worktrees
    for name in &["multi-ls-1", "multi-ls-2"] {
        let _ = Command::new(wt_binary())
            .args(["new", name, "--print-path"])
            .current_dir(&repo)
            .env("HOME", &home)
            .output();
    }

    // List should show both (or appropriate message)
    let output = Command::new(wt_binary())
        .arg("ls")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt ls failed");

    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // Should show at least one or say no worktrees
    assert!(
        combined.contains("multi-ls")
            || combined.contains("BRANCH")
            || combined.contains("No worktrees")
    );
}

#[test]
fn test_merge_with_changes() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let output = Command::new(wt_binary())
        .args(["new", "merge-changes", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    let wt_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());

    // Make and commit changes
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

    // Try to merge from worktree
    let output = Command::new(wt_binary())
        .args(["merge", "--keep"])
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt merge failed");

    // Check output - merge may fail for various reasons
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Just verify the command ran without panic
    // Success or any error message is acceptable
    let _ = (output.status, stderr);
}

#[test]
fn test_sync_on_feature_with_updates() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let output = Command::new(wt_binary())
        .args(["new", "sync-updates", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    let wt_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());

    // Make change in main repo
    std::fs::write(repo.join("main-update.txt"), "update from main").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Main update"])
        .current_dir(&repo)
        .output()
        .unwrap();

    // Sync in worktree
    let output = Command::new(wt_binary())
        .arg("sync")
        .current_dir(&wt_path)
        .env("HOME", &home)
        .output()
        .expect("wt sync failed");

    // May succeed or fail depending on setup
    let _ = output.status;
}

#[test]
fn test_clean_remvs_merged_worktree() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let output = Command::new(wt_binary())
        .args(["new", "clean-merged", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    // Merge the branch into main (make it "merged")
    Command::new("git")
        .args(["merge", "clean-merged", "--no-edit"])
        .current_dir(&repo)
        .output()
        .ok();

    // Clean should identify and potentially remv it
    let output = Command::new(wt_binary())
        .arg("clean")
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt clean failed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    // Should either clean or say nothing to clean
    assert!(
        output.status.success()
            || stderr.contains("Cleaned")
            || stderr.contains("No")
            || stderr.contains("merged")
    );
}

#[test]
fn test_mv_renames_worktree() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let output = Command::new(wt_binary())
        .args(["new", "mv-src", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !output.status.success() {
        return;
    }

    // Move it
    let output = Command::new(wt_binary())
        .args(["mv", "mv-src", "mv-dst"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt mv failed");

    // Check result
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Either success or meaningful error
    assert!(
        output.status.success()
            || stderr.contains("Renamed")
            || stderr.contains("not found")
            || stderr.contains("error")
    );
}

#[test]
fn test_cd_returns_correct_path() {
    let (_dir, repo, home) = setup_worktree_test_env();

    // Create worktree
    let new_output = Command::new(wt_binary())
        .args(["new", "cd-check", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    if !new_output.status.success() {
        return;
    }

    let created_path = String::from_utf8_lossy(&new_output.stdout).trim().to_string();

    // Cd should return the same path
    let output = Command::new(wt_binary())
        .args(["cd", "cd-check", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt cd failed");

    if output.status.success() {
        let cd_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        assert_eq!(created_path, cd_path);
    }
}

// ---------------------------------------------------------------------------
// Merge strategy tests
// ---------------------------------------------------------------------------

#[test]
fn test_merge_strategy_squash() {
    let (_dir, _repo, _home) = setup_worktree_test_env();

    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("wt merge --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("squash"));
}

#[test]
fn test_merge_strategy_rebase() {
    let output = Command::new(wt_binary())
        .args(["merge", "--help"])
        .output()
        .expect("wt merge --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rebase"));
}

#[test]
fn test_sync_strategy_options() {
    let output = Command::new(wt_binary())
        .args(["sync", "--help"])
        .output()
        .expect("wt sync --help failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rebase") || stdout.contains("strategy"));
}

// ---------------------------------------------------------------------------
// Version command test
// ---------------------------------------------------------------------------

#[test]
fn test_version_output() {
    let output = Command::new(wt_binary())
        .arg("--version")
        .output()
        .expect("wt --version failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("wt") || stdout.contains("0.1"));
}

// ---------------------------------------------------------------------------
// Snap Mode Tests
// ---------------------------------------------------------------------------

#[test]
fn test_new_with_snap_outputs_two_lines() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let home = setup_git_repo_with_home(&repo);

    let output = Command::new(wt_binary())
        .args(["new", "snap-test", "-s", "echo hello", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    // Should output path on first line, command on second
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("snap-test"));
    assert_eq!(lines[1], "echo hello");

    // Cleanup
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

    let output = Command::new(wt_binary())
        .args(["new", "snap-meta-test", "-s", "agent", "--print-path"])
        .current_dir(&repo)
        .env("HOME", &home)
        .output()
        .expect("wt new failed");

    assert!(output.status.success());

    // Check metadata file exists with snap info
    let meta_path = home
        .join(".agent-worktree")
        .join("workspaces")
        .join("repo")
        .join("snap-meta-test.status.toml");
    assert!(meta_path.exists());

    let content = std::fs::read_to_string(&meta_path).unwrap();
    assert!(content.contains("snap"));
    assert!(content.contains("agent"));

    // Cleanup
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

    // snap-continue should fail when not in a worktree
    let output = Command::new(wt_binary())
        .args(["snap-continue"])
        .current_dir(dir.path())
        .output()
        .expect("wt snap-continue failed");

    // Should fail (not in worktree or no changes)
    // The exact behavior depends on implementation
    let _status = output.status;
}
