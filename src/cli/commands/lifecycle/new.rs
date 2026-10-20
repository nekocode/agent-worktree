// ===========================================================================
// wt new - Create a new worktree
// ===========================================================================

use std::path::Path;

use clap::Args;
use clap_complete::engine::ArgValueCompleter;

use crate::cli::{write_path_file, Error, Result};
use crate::complete;
use crate::config::Config;
use crate::git;
use crate::meta::{self, WorktreeMeta};
use crate::process;
use crate::util;

#[derive(Args)]
pub struct NewArgs {
    /// Branch name (random name like 'swift-fox' if not provided)
    branch: Option<String>,

    /// Base branch to create from and merge back to (default: current branch)
    #[arg(long, value_name = "BRANCH", add = ArgValueCompleter::new(complete::complete_branches))]
    base: Option<String>,
}

/// Result of the shared worktree-creation path.
pub struct CreatedWorktree {
    pub branch: String,
    pub base_branch: String,
    pub path: std::path::PathBuf,
    pub repo_root: std::path::PathBuf,
}

/// Shared by `wt new` and `wt run`: resolve base branch, create the worktree
/// and its metadata, copy configured files, and run post_create hooks.
pub fn create_worktree_core(
    branch_arg: Option<String>,
    base_arg: Option<&str>,
    config: &Config,
) -> Result<CreatedWorktree> {
    // Ensure we're in a git repo
    let repo_root = git::repo_root()?;
    let workspace_id = git::workspace_id()?;
    let workspace_dir = config.workspaces_dir.join(&workspace_id);

    // Determine trunk branch
    let trunk = config.resolve_trunk();

    // Resolve base branch: --base flag > current branch > trunk.
    // Determines both the checkout starting point and the default merge/sync target.
    let base_branch = if let Some(b) = base_arg {
        if !git::branch_exists(b)? {
            return Err(Error::Other(format!("Branch '{b}' does not exist")));
        }
        b.to_string()
    } else {
        // Detached HEAD falls back to trunk.
        git::current_branch()
            .ok()
            .filter(|b| b != "HEAD")
            .unwrap_or_else(|| trunk.clone())
    };

    // Generate or use provided branch name
    let branch = branch_arg.unwrap_or_else(|| {
        util::generate_unique_branch_name(|n| git::branch_exists(n).unwrap_or(false))
    });

    let wt_path = workspace_dir.join(&branch);

    // Create workspace directory if needed
    std::fs::create_dir_all(&workspace_dir).map_err(|e| Error::Other(e.to_string()))?;

    git::create_worktree(&wt_path, &branch, &base_branch)?;

    let meta = WorktreeMeta::new(base_branch);
    let meta_path = meta::meta_path(&workspace_dir, &branch);
    meta.save(&meta_path)
        .map_err(|e| Error::Other(e.to_string()))?;

    // `git worktree add` leaves submodule dirs empty; init them so hooks and
    // agents see a complete checkout. Failure warns instead of aborting —
    // e.g. an offline optional submodule shouldn't kill worktree creation.
    if config.submodules && wt_path.join(".gitmodules").exists() {
        // Each worktree re-clones every submodule over the network, so this
        // can run for minutes. Say so up front, then let git's own progress
        // stream through — silence here reads as a hang.
        eprintln!("Initializing submodules (network clone, may take a while)...");
        if let Err(e) = git::submodule_update_init(&wt_path, config.submodule_jobs) {
            eprintln!("Warning: submodule init failed: {e}");
            eprintln!("Run 'git submodule update --init --recursive' in the worktree manually.");
        }
    }

    // A relative core.hooksPath resolves against each command's CWD, so git
    // hooks silently break inside linked worktrees — surface it at creation.
    if let Some(hooks_path) = git::hooks_path() {
        if !Path::new(&hooks_path).is_absolute() {
            eprintln!(
                "Warning: core.hooksPath '{hooks_path}' is relative and may not resolve inside worktrees."
            );
        }
    }

    // Copy files from main repo
    copy_files(&repo_root, &wt_path, config)?;

    // Run post_create hooks. On failure, leave the worktree in place — the
    // user usually wants to fix the hook (e.g. install missing tool) and
    // resume manually rather than have us silently rm a half-created tree.
    if !config.hooks.post_create.is_empty() {
        eprintln!("Running post-create hooks...");
        let env = process::HookEnv {
            main_repo: &repo_root,
            worktree: &wt_path,
            branch: &branch,
            base_branch: &meta.base_branch,
        };
        if let Err(e) = process::run_hooks(&config.hooks.post_create, &wt_path, &env) {
            eprintln!();
            eprintln!("post_create hook failed: {e}");
            eprintln!("Worktree '{branch}' was created at: {}", wt_path.display());
            eprintln!("Fix the hook and `cd` in manually, or run 'wt rm {branch}' to discard.");
            return Err(Error::Other(format!("post_create hook failed: {e}")));
        }
    }

    Ok(CreatedWorktree {
        branch,
        base_branch: meta.base_branch,
        path: wt_path,
        repo_root,
    })
}

pub fn run(args: NewArgs, config: &Config, path_file: Option<&Path>) -> Result<()> {
    let created = create_worktree_core(args.branch, args.base.as_deref(), config)?;

    // Write path for shell integration
    if path_file.is_some() {
        write_path_file(path_file, &created.path)?;
    } else {
        eprintln!(
            "Created worktree: {} (from {})",
            created.branch, created.base_branch
        );
        eprintln!("Path: {}", created.path.display());
    }

    Ok(())
}

/// Reject patterns that could escape the repo root.
///
/// Without this guard, a malicious `.agent-worktree.toml` could exfiltrate
/// host files into the worktree via `/abs/path` or `..` traversal — the
/// downstream `strip_prefix` would silently skip mismatches.
fn validate_copy_pattern(pattern: &str) -> Result<()> {
    if pattern.starts_with('/') {
        return Err(Error::Other(format!(
            "copy_files pattern '{pattern}' cannot start with '/' (absolute path)"
        )));
    }
    if pattern.split(['/', '\\']).any(|seg| seg == "..") {
        return Err(Error::Other(format!(
            "copy_files pattern '{pattern}' cannot contain '..'"
        )));
    }
    Ok(())
}

fn copy_files(from: &Path, to: &Path, config: &Config) -> Result<()> {
    use ignore::overrides::OverrideBuilder;
    use ignore::WalkBuilder;

    if config.copy_files.is_empty() {
        return Ok(());
    }

    for pattern in &config.copy_files {
        validate_copy_pattern(pattern)?;
    }

    // Build gitignore-style matcher
    // Patterns work like .gitignore: "*.md" matches all .md files, "/*.md" matches only root
    let mut builder = OverrideBuilder::new(from);
    for pattern in &config.copy_files {
        builder
            .add(pattern)
            .map_err(|e| Error::Other(format!("invalid pattern '{}': {}", pattern, e)))?;
    }
    let overrides = builder.build().map_err(|e| Error::Other(e.to_string()))?;

    // follow_links=false: a symlink in the repo could otherwise pull files
    // from outside the repo into the worktree.
    let walker = WalkBuilder::new(from)
        .overrides(overrides)
        .standard_filters(false)
        .follow_links(false)
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        // `path.is_file()` below traverses symlinks, so a symlink entry
        // pointing outside the repo would materialize external content into
        // the worktree — exactly what this sandbox promises to prevent.
        // Skip symlink entries entirely; share paths via post_create hooks.
        if entry.path_is_symlink() {
            if let Ok(rel) = path.strip_prefix(from) {
                eprintln!("Warning: skipping symlink {} (not copied)", rel.display());
            }
            continue;
        }
        if path.is_file() {
            let rel = match path.strip_prefix(from) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!(
                        "Warning: failed to strip prefix for {}: {e}",
                        path.display()
                    );
                    continue;
                }
            };
            let dest = to.join(rel);

            if let Some(parent) = dest.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "Warning: failed to create directory {}: {e}",
                        parent.display()
                    );
                    continue;
                }
            }

            if let Err(e) = copy_file_reflink(path, &dest) {
                eprintln!("Warning: failed to copy {}: {e}", rel.display());
            }
        }
    }

    Ok(())
}

/// Copy `from` to `to` using copy-on-write (reflink/clonefile/DUPLICATE_EXTENTS)
/// where the filesystem supports it, falling back to a plain content copy
/// otherwise. The source mtime is preserved: Linux FICLONE, ReFS
/// DUPLICATE_EXTENTS and copy_file_range all stamp "now", which makes
/// mtime-based build caches (cargo target/, tsc incremental) look stale.
fn copy_file_reflink(from: &Path, to: &Path) -> std::io::Result<()> {
    let modified = std::fs::metadata(from)?.modified()?;

    // Blind-remove instead of checking `to.exists()`: exists() follows
    // symlinks and reports false for a broken symlink, which would leave the
    // link in place and make the create_new inside reflink fail.
    match std::fs::remove_file(to) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        other => other?,
    }
    reflink_copy::reflink_or_copy(from, to)?;

    // The copy may be read-only, so never open it for writing. Unix futimens
    // only needs ownership, not a writable fd; Windows SetFileInformationByHandle
    // only needs FILE_WRITE_ATTRIBUTES.
    let mut options = std::fs::OpenOptions::new();
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
        options.access_mode(FILE_WRITE_ATTRIBUTES);
    }
    #[cfg(not(windows))]
    options.read(true);
    options.open(to)?.set_modified(modified)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_copy_pattern_accepts_relative_glob() {
        assert!(validate_copy_pattern(".env").is_ok());
        assert!(validate_copy_pattern(".env.*").is_ok());
        assert!(validate_copy_pattern("config/*.toml").is_ok());
        assert!(validate_copy_pattern("**/.secret").is_ok());
    }

    #[test]
    fn validate_copy_pattern_rejects_absolute_path() {
        let err = validate_copy_pattern("/etc/passwd").unwrap_err();
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn validate_copy_pattern_rejects_parent_traversal() {
        let err = validate_copy_pattern("../secrets").unwrap_err();
        assert!(err.to_string().contains(".."));

        let err = validate_copy_pattern("config/../../etc/passwd").unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn validate_copy_pattern_rejects_backslash_traversal() {
        // Windows-style path separator should still be rejected.
        let err = validate_copy_pattern("..\\secrets").unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    /// Write `contents` with a fixed past mtime and return it. The sub-second
    /// part is a multiple of 100ns so it round-trips through NTFS precision.
    fn write_with_old_mtime(path: &Path, contents: &str) -> std::time::SystemTime {
        std::fs::write(path, contents).unwrap();
        filetime::set_file_mtime(
            path,
            filetime::FileTime::from_unix_time(1_600_000_000, 123_456_700),
        )
        .unwrap();
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    fn mtime(path: &Path) -> std::time::SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn copy_file_reflink_preserves_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        let modified = write_with_old_mtime(&src, "contents");

        copy_file_reflink(&src, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "contents");
        assert_eq!(mtime(&dest), modified);
    }

    #[test]
    fn copy_file_reflink_preserves_mtime_of_readonly_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        let modified = write_with_old_mtime(&src, "contents");
        let mut permissions = std::fs::metadata(&src).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&src, permissions).unwrap();

        copy_file_reflink(&src, &dest).unwrap();
        assert_eq!(mtime(&dest), modified);
        assert!(std::fs::metadata(&dest).unwrap().permissions().readonly());
    }

    #[test]
    fn copy_file_reflink_overwrites_existing_dest() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        let modified = write_with_old_mtime(&src, "new");
        std::fs::write(&dest, "stale").unwrap();

        copy_file_reflink(&src, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new");
        assert_eq!(mtime(&dest), modified);
    }

    #[cfg(unix)]
    #[test]
    fn copy_file_reflink_replaces_broken_symlink_dest() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.txt");
        let dest = dir.path().join("dest.txt");
        let modified = write_with_old_mtime(&src, "new");
        std::os::unix::fs::symlink(dir.path().join("missing"), &dest).unwrap();

        copy_file_reflink(&src, &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new");
        assert!(!std::fs::symlink_metadata(&dest).unwrap().is_symlink());
        assert_eq!(mtime(&dest), modified);
    }
}
