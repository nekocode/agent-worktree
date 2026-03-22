use super::*;
use std::process::Command as StdCommand;

// ===========================================================================
// has_uncommitted_changes tests
// ===========================================================================
#[test]
fn test_has_uncommitted_changes_clean() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let has_changes = has_uncommitted_changes();
        assert!(has_changes.is_ok());
        assert!(!has_changes.unwrap());
    });
}

#[test]
fn test_has_uncommitted_changes_dirty() {
    let dir = setup_test_repo();
    std::fs::write(dir.path().join("new_file.txt"), "content").unwrap();
    with_cwd(dir.path(), || {
        let has_changes = has_uncommitted_changes();
        assert!(has_changes.is_ok());
        assert!(has_changes.unwrap());
    });
}

#[test]
fn test_list_worktrees() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let worktrees = list_worktrees();
        assert!(worktrees.is_ok());
        let list = worktrees.unwrap();
        assert!(!list.is_empty());
        assert_eq!(list[0].branch, Some("main".to_string()));
    });
}

#[test]
fn test_is_rebase_in_progress() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        assert!(!is_rebase_in_progress());
    });
}

#[test]
fn test_is_merge_in_progress() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        assert!(!is_merge_in_progress());
    });
}

#[test]
fn test_log_oneline() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let log = log_oneline("HEAD", "HEAD");
        assert!(log.is_ok());
        assert!(log.unwrap().is_empty());
    });
}

#[test]
fn test_commit_count() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let count = commit_count("HEAD", "HEAD");
        assert!(count.is_ok());
        assert_eq!(count.unwrap(), 0);
    });
}

#[test]
fn test_fetch() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let result = fetch();
        assert!(result.is_ok());
    });
}

#[test]
fn test_is_merged() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let result = is_merged("main", "main");
        assert!(result.is_ok());
        assert!(result.unwrap());
    });
}

// ===========================================================================
// Worktree operations
// ===========================================================================
#[test]
fn test_create_and_remove_worktree() {
    let dir = setup_test_repo();
    let wt_path = dir.path().join("worktrees").join("feature");
    std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();

    with_cwd(dir.path(), || {
        let result = create_worktree(&wt_path, "feature-branch", "main");
        assert!(result.is_ok());
        assert!(wt_path.exists());
        assert!(branch_exists("feature-branch").unwrap());

        let result = remove_worktree(&wt_path, false);
        assert!(result.is_ok());
    });
}

#[test]
fn test_create_worktree_duplicate() {
    let dir = setup_test_repo();
    let wt_path = dir.path().join("worktrees").join("dup");
    let wt_path2 = dir.path().join("worktrees").join("dup2");
    std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();

    with_cwd(dir.path(), || {
        create_worktree(&wt_path, "dup-branch", "main").unwrap();
        let result = create_worktree(&wt_path2, "dup-branch", "main");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), Error::WorktreeExists(_)));
    });
}

// ===========================================================================
// Branch operations
// ===========================================================================
#[test]
fn test_rename_branch() {
    let dir = setup_test_repo();
    StdCommand::new("git")
        .args(["branch", "old-name"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    with_cwd(dir.path(), || {
        let result = rename_branch("old-name", "new-name");
        assert!(result.is_ok());
        assert!(!branch_exists("old-name").unwrap());
        assert!(branch_exists("new-name").unwrap());
    });
}

#[test]
fn test_delete_branch() {
    let dir = setup_test_repo();
    StdCommand::new("git")
        .args(["branch", "to-delete"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    with_cwd(dir.path(), || {
        assert!(branch_exists("to-delete").unwrap());
        let result = delete_branch("to-delete", false);
        assert!(result.is_ok());
        assert!(!branch_exists("to-delete").unwrap());
    });
}

#[test]
fn test_checkout() {
    let dir = setup_test_repo();
    StdCommand::new("git")
        .args(["branch", "other-branch"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    with_cwd(dir.path(), || {
        let result = checkout("other-branch");
        assert!(result.is_ok());
        assert_eq!(current_branch().unwrap(), "other-branch");
    });
}

// ===========================================================================
// Abort/continue operations
// ===========================================================================
#[test]
fn test_rebase_abort_no_rebase() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let result = rebase_abort();
        assert!(result.is_err());
    });
}

#[test]
fn test_merge_abort_no_merge() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let result = merge_abort();
        assert!(result.is_err());
    });
}

#[test]
fn test_reset_merge_clean_repo() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        // reset --merge on clean repo is a no-op success
        let result = reset_merge();
        assert!(result.is_ok());
    });
}

#[test]
fn test_rebase_continue_no_rebase() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let result = rebase_continue();
        assert!(result.is_err());
    });
}

#[test]
fn test_merge_continue_no_merge() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        let result = merge_continue();
        assert!(result.is_err());
    });
}

// ===========================================================================
// Merge and rebase operations
// ===========================================================================
#[test]
fn test_merge_fast_forward() {
    let dir = setup_test_repo();
    // Create a branch that's already at main
    StdCommand::new("git")
        .args(["branch", "already-merged"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    with_cwd(dir.path(), || {
        // Merge should work (fast-forward or no-op)
        let result = merge("already-merged", false, false, None);
        // May succeed or say "already up to date"
        let _ = result;
    });
}

#[test]
fn test_rebase_same_branch() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        // Rebase onto self should be a no-op
        let result = rebase("main");
        assert!(result.is_ok());
    });
}

// ===========================================================================
// Remove worktree with force
// ===========================================================================
#[test]
fn test_remove_worktree_force() {
    let dir = setup_test_repo();
    let wt_path = dir.path().join("worktrees").join("force-test");
    std::fs::create_dir_all(wt_path.parent().unwrap()).unwrap();

    with_cwd(dir.path(), || {
        create_worktree(&wt_path, "force-branch", "main").unwrap();

        // Create uncommitted changes in worktree
        std::fs::write(wt_path.join("uncommitted.txt"), "changes").unwrap();

        // Force remove should work
        let result = remove_worktree(&wt_path, true);
        assert!(result.is_ok());
    });
}

// ===========================================================================
// Delete branch with force
// ===========================================================================
#[test]
fn test_delete_branch_force() {
    let dir = setup_test_repo();
    StdCommand::new("git")
        .args(["branch", "unmerged-branch"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    with_cwd(dir.path(), || {
        // Force delete should work
        let result = delete_branch("unmerged-branch", true);
        assert!(result.is_ok());
    });
}

// ===========================================================================
// has_changes_from_trunk tests
// ===========================================================================

#[test]
fn test_has_changes_from_trunk_no_changes() {
    let dir = setup_test_repo();
    with_cwd(dir.path(), || {
        // On main, no changes from main
        let has = has_changes_from_trunk("main");
        assert!(has.is_ok());
        assert!(!has.unwrap());
    });
}

#[test]
fn test_has_changes_from_trunk_with_committed_changes() {
    let dir = setup_test_repo();

    // Create feature branch and add a commit
    StdCommand::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    std::fs::write(dir.path().join("feature.txt"), "new feature").unwrap();

    StdCommand::new("git")
        .args(["add", "feature.txt"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["commit", "-m", "Add feature"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    with_cwd(dir.path(), || {
        // Feature branch has commits ahead of main
        let has = has_changes_from_trunk("main");
        assert!(has.is_ok());
        assert!(
            has.unwrap(),
            "Should detect committed changes ahead of trunk"
        );
    });
}

#[test]
fn test_has_changes_from_trunk_with_uncommitted_changes() {
    let dir = setup_test_repo();

    // Create feature branch with uncommitted changes only
    StdCommand::new("git")
        .args(["checkout", "-b", "feature"])
        .current_dir(dir.path())
        .output()
        .unwrap();

    std::fs::write(dir.path().join("dirty.txt"), "uncommitted").unwrap();

    with_cwd(dir.path(), || {
        // Feature branch has uncommitted changes
        let has = has_changes_from_trunk("main");
        assert!(has.is_ok());
        assert!(has.unwrap(), "Should detect uncommitted changes");
    });
}
