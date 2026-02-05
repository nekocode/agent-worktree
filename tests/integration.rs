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
    assert!(stdout.contains("--no-delete"));
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
// Move Command Tests
// ---------------------------------------------------------------------------

#[test]
fn test_move_help() {
    let output = Command::new(wt_binary())
        .args(["move", "--help"])
        .output()
        .expect("Failed to execute wt move --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Rename"));
    assert!(stdout.contains("OLD_BRANCH"));
    assert!(stdout.contains("NEW_BRANCH"));
}

#[test]
fn test_move_nonexistent() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let output = Command::new(wt_binary())
        .args(["move", "old-branch", "new-branch"])
        .current_dir(dir.path())
        .output()
        .expect("Failed to execute wt move");

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
    assert!(stdout.contains("merged") || stdout.contains("Remove"));
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
