// ===========================================================================
// meta - Worktree Metadata ({branch}.toml)
// ===========================================================================

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Path helpers — filename migrated from .status.toml to .toml; legacy still
// readable for forward compatibility.
// ---------------------------------------------------------------------------

/// New format path: {wt_dir}/{branch}.toml
pub fn meta_path(wt_dir: &Path, branch: &str) -> PathBuf {
    wt_dir.join(format!("{branch}.toml"))
}

/// Compatibility loader: prefer .toml, fallback to .status.toml.
pub fn meta_path_with_fallback(wt_dir: &Path, branch: &str) -> PathBuf {
    let new = meta_path(wt_dir, branch);
    if new.exists() {
        return new;
    }
    let legacy = wt_dir.join(format!("{branch}.status.toml"));
    if legacy.exists() {
        return legacy;
    }
    new
}

/// Remove meta file (try both new and legacy names).
pub fn remove_meta(wt_dir: &Path, branch: &str) {
    std::fs::remove_file(meta_path(wt_dir, branch)).ok();
    std::fs::remove_file(wt_dir.join(format!("{branch}.status.toml"))).ok();
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read metadata: {0}")]
    Read(#[from] std::io::Error),

    #[error("failed to parse metadata: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("failed to serialize metadata: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("metadata missing required field: base_branch")]
    MissingBaseBranch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeMeta {
    pub created_at: DateTime<Utc>,
    pub base_branch: String,
}

impl WorktreeMeta {
    pub fn new(base_branch: String) -> Self {
        Self {
            created_at: Utc::now(),
            base_branch,
        }
    }

    /// Load from file. Falls back to legacy schema (uses `trunk` when
    /// `base_branch` is absent) so pre-existing worktrees keep working.
    /// Unknown fields (e.g. dropped `base_commit`, `snap_command`, `trunk`)
    /// are silently ignored.
    pub fn load(path: &Path) -> Result<Self> {
        Self::parse(&std::fs::read_to_string(path)?)
    }

    fn parse(content: &str) -> Result<Self> {
        let raw: RawMeta = toml::from_str(content)?;
        let base_branch = raw
            .base_branch
            .or(raw.trunk)
            .ok_or(Error::MissingBaseBranch)?;
        Ok(Self {
            created_at: raw.created_at,
            base_branch,
        })
    }

    /// Save to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

/// Deserialization shim: tolerates legacy `trunk` field. Explicit `.or()`
/// in `parse` enforces base_branch-wins priority when both keys are present
/// (serde's `#[serde(alias)]` is order-dependent and would not guarantee it).
#[derive(Deserialize)]
struct RawMeta {
    created_at: DateTime<Utc>,
    #[serde(default)]
    base_branch: Option<String>,
    #[serde(default)]
    trunk: Option<String>,
}

// ---------------------------------------------------------------------------
// Target branch resolution — CLI override > base_branch (if exists) > trunk
// ---------------------------------------------------------------------------

/// Resolve merge/sync target by reading meta file.
///
/// Priority: cli_override > meta.base_branch (if branch exists) > trunk
pub fn resolve_effective_target(
    wt_dir: &Path,
    branch: &str,
    cli_override: Option<&str>,
    branch_exists: impl Fn(&str) -> bool,
    trunk: &str,
) -> String {
    let meta_path = meta_path_with_fallback(wt_dir, branch);
    let loaded = WorktreeMeta::load(&meta_path).ok();
    let base = loaded.as_ref().map(|m| m.base_branch.as_str());
    resolve_target_branch(cli_override, base, branch_exists, trunk)
}

/// Pure resolver (no I/O).
///
/// Priority: cli_override > base_branch (if branch still exists) > trunk
pub fn resolve_target_branch(
    cli_override: Option<&str>,
    base_branch: Option<&str>,
    branch_exists: impl Fn(&str) -> bool,
    trunk: &str,
) -> String {
    if let Some(target) = cli_override {
        return target.to_string();
    }
    if let Some(bb) = base_branch {
        if branch_exists(bb) {
            return bb.to_string();
        }
    }
    trunk.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_new_meta() {
        let meta = WorktreeMeta::new("main".to_string());
        assert_eq!(meta.base_branch, "main");
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.toml");

        let meta = WorktreeMeta::new("develop".to_string());
        meta.save(&path).unwrap();

        let loaded = WorktreeMeta::load(&path).unwrap();
        assert_eq!(loaded.base_branch, "develop");
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_branch = "feature-x"
"#;
        let meta = WorktreeMeta::parse(toml).unwrap();
        assert_eq!(meta.base_branch, "feature-x");
    }

    /// Legacy compatibility: old toml files have `trunk` but no `base_branch`.
    #[test]
    fn test_parse_legacy_uses_trunk_as_base_branch() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_commit = "abc1234"
trunk = "main"
snap_command = "claude"
"#;
        let meta = WorktreeMeta::parse(toml).unwrap();
        assert_eq!(meta.base_branch, "main");
    }

    /// New `base_branch` wins over legacy `trunk` when both present.
    #[test]
    fn test_parse_base_branch_wins_over_trunk() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_branch = "feature-x"
trunk = "main"
"#;
        let meta = WorktreeMeta::parse(toml).unwrap();
        assert_eq!(meta.base_branch, "feature-x");
    }

    #[test]
    fn test_parse_missing_base_and_trunk_fails() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
"#;
        assert!(matches!(
            WorktreeMeta::parse(toml),
            Err(Error::MissingBaseBranch)
        ));
    }

    #[test]
    fn test_meta_path() {
        let dir = std::path::PathBuf::from("/tmp/wt");
        assert_eq!(meta_path(&dir, "fox"), PathBuf::from("/tmp/wt/fox.toml"));
    }

    #[test]
    fn test_meta_path_with_fallback_new_exists() {
        let dir = tempdir().unwrap();
        let new = dir.path().join("br.toml");
        std::fs::write(&new, "x").unwrap();
        assert_eq!(meta_path_with_fallback(dir.path(), "br"), new);
    }

    #[test]
    fn test_meta_path_with_fallback_legacy() {
        let dir = tempdir().unwrap();
        let legacy = dir.path().join("br.status.toml");
        std::fs::write(&legacy, "x").unwrap();
        assert_eq!(meta_path_with_fallback(dir.path(), "br"), legacy);
    }

    #[test]
    fn test_meta_path_with_fallback_neither() {
        let dir = tempdir().unwrap();
        let expected = dir.path().join("br.toml");
        assert_eq!(meta_path_with_fallback(dir.path(), "br"), expected);
    }

    #[test]
    fn test_remove_meta() {
        let dir = tempdir().unwrap();
        let new = dir.path().join("br.toml");
        let legacy = dir.path().join("br.status.toml");
        std::fs::write(&new, "x").unwrap();
        std::fs::write(&legacy, "x").unwrap();

        remove_meta(dir.path(), "br");

        assert!(!new.exists());
        assert!(!legacy.exists());
    }

    // -----------------------------------------------------------------------
    // resolve_target_branch
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_cli_override_wins() {
        let result = resolve_target_branch(Some("release"), Some("feature-a"), |_| true, "main");
        assert_eq!(result, "release");
    }

    #[test]
    fn test_resolve_base_branch_exists() {
        let result = resolve_target_branch(None, Some("feature-a"), |b| b == "feature-a", "main");
        assert_eq!(result, "feature-a");
    }

    #[test]
    fn test_resolve_base_branch_deleted() {
        let result = resolve_target_branch(None, Some("feature-a"), |_| false, "main");
        assert_eq!(result, "main");
    }

    #[test]
    fn test_resolve_no_base_branch() {
        let result = resolve_target_branch(None, None, |_| true, "main");
        assert_eq!(result, "main");
    }

    // -----------------------------------------------------------------------
    // resolve_effective_target
    // -----------------------------------------------------------------------

    #[test]
    fn test_effective_target_reads_meta() {
        let dir = tempdir().unwrap();
        let meta = WorktreeMeta::new("feature-a".to_string());
        meta.save(&meta_path(dir.path(), "my-branch")).unwrap();

        let result =
            resolve_effective_target(dir.path(), "my-branch", None, |b| b == "feature-a", "main");
        assert_eq!(result, "feature-a");
    }

    #[test]
    fn test_effective_target_no_meta_falls_back_to_trunk() {
        let dir = tempdir().unwrap();
        let result = resolve_effective_target(dir.path(), "my-branch", None, |_| true, "main");
        assert_eq!(result, "main");
    }

    #[test]
    fn test_effective_target_cli_override_wins() {
        let dir = tempdir().unwrap();
        let meta = WorktreeMeta::new("feature-a".to_string());
        meta.save(&meta_path(dir.path(), "my-branch")).unwrap();

        let result =
            resolve_effective_target(dir.path(), "my-branch", Some("release"), |_| true, "main");
        assert_eq!(result, "release");
    }
}
