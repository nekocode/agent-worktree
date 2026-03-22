// ===========================================================================
// complete - Dynamic Completion Candidates
//
// Completers are fail-silent: errors return empty list.
// They run at tab time via CompleteEnv, so git calls reflect real repo state.
// ===========================================================================

use std::ffi::OsStr;

use clap_complete::engine::CompletionCandidate;

/// Complete worktree branch names (for cd/rm/mv)
pub fn complete_worktrees(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return vec![];
    };

    let Ok(worktrees) = crate::git::list_worktrees() else {
        return vec![];
    };

    // Main worktree is not a valid cd/rm/mv target
    worktrees
        .iter()
        .skip(1)
        .filter_map(|wt| wt.branch.as_deref())
        .filter(|b| b.starts_with(prefix))
        .map(CompletionCandidate::new)
        .collect()
}

/// Complete local git branch names (for --base/--into/--from/--trunk)
pub fn complete_branches(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return vec![];
    };

    let Ok(branches) = crate::git::local_branches() else {
        return vec![];
    };

    branches
        .iter()
        .filter(|b| b.starts_with(prefix))
        .map(|b| CompletionCandidate::new(b.as_str()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_worktrees_does_not_panic() {
        // Should not panic regardless of CWD (inside or outside repo)
        let _ = complete_worktrees(OsStr::new(""));
    }

    #[test]
    fn complete_branches_does_not_panic() {
        let _ = complete_branches(OsStr::new(""));
    }

    #[test]
    fn complete_worktrees_handles_invalid_utf8() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let invalid = OsStr::from_bytes(&[0xff, 0xfe]);
            let result = complete_worktrees(invalid);
            assert!(result.is_empty());
        }
    }

    #[test]
    fn complete_branches_handles_invalid_utf8() {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            let invalid = OsStr::from_bytes(&[0xff, 0xfe]);
            let result = complete_branches(invalid);
            assert!(result.is_empty());
        }
    }
}
