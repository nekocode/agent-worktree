// ===========================================================================
// meta - Worktree Metadata ({branch}.toml)
// ===========================================================================

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// 路径工具 — 文件名从 .status.toml 迁移到 .toml，保持向前兼容
// ---------------------------------------------------------------------------

/// 新格式路径: {wt_dir}/{branch}.toml
pub fn meta_path(wt_dir: &Path, branch: &str) -> PathBuf {
    wt_dir.join(format!("{branch}.toml"))
}

/// 兼容加载: 先找 .toml，fallback .status.toml
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

/// 删除 meta 文件（新旧格式都尝试）
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeMeta {
    pub created_at: DateTime<Utc>,
    pub base_commit: String,
    pub trunk: String,

    #[serde(default)]
    pub snap_command: Option<String>,

    #[serde(default)]
    pub base_branch: Option<String>,
}

impl WorktreeMeta {
    pub fn new(base_commit: String, trunk: String) -> Self {
        Self {
            created_at: Utc::now(),
            base_commit,
            trunk,
            snap_command: None,
            base_branch: None,
        }
    }

    pub fn with_snap(mut self, command: String) -> Self {
        self.snap_command = Some(command);
        self
    }

    pub fn with_base_branch(mut self, branch: String) -> Self {
        self.base_branch = Some(branch);
        self
    }

    /// Load from file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    /// Save to file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// 目标分支解析 — CLI 覆盖 > base_branch（若存在） > trunk
// ---------------------------------------------------------------------------

/// 从 meta 文件解析 merge/sync 目标（封装常见的 load → extract → resolve 流程）
///
/// 优先级: cli_override > meta.base_branch (if exists) > trunk
pub fn resolve_effective_target(
    wt_dir: &Path,
    branch: &str,
    cli_override: Option<&str>,
    branch_exists: impl Fn(&str) -> bool,
    trunk: &str,
) -> String {
    let meta_path = meta_path_with_fallback(wt_dir, branch);
    let loaded = WorktreeMeta::load(&meta_path).ok();
    let base = loaded.as_ref().and_then(|m| m.base_branch.as_deref());
    resolve_target_branch(cli_override, base, branch_exists, trunk)
}

/// 解析 merge/sync 的目标分支（纯逻辑，无 I/O）
///
/// 优先级: cli_override > base_branch (if branch still exists) > trunk
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
        let meta = WorktreeMeta::new("abc123".to_string(), "main".to_string());
        assert_eq!(meta.base_commit, "abc123");
        assert_eq!(meta.trunk, "main");
        assert!(meta.snap_command.is_none());
    }

    #[test]
    fn test_with_snap() {
        let meta = WorktreeMeta::new("abc123".to_string(), "main".to_string())
            .with_snap("claude".to_string());
        assert_eq!(meta.snap_command, Some("claude".to_string()));
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.toml");

        let meta = WorktreeMeta::new("def456".to_string(), "develop".to_string())
            .with_snap("aider".to_string());

        meta.save(&path).unwrap();

        let loaded = WorktreeMeta::load(&path).unwrap();
        assert_eq!(loaded.base_commit, "def456");
        assert_eq!(loaded.trunk, "develop");
        assert_eq!(loaded.snap_command, Some("aider".to_string()));
    }

    #[test]
    fn test_parse_toml() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_commit = "abc1234"
trunk = "main"
"#;
        let meta: WorktreeMeta = toml::from_str(toml).unwrap();
        assert_eq!(meta.base_commit, "abc1234");
        assert_eq!(meta.trunk, "main");
        assert!(meta.snap_command.is_none());
    }

    #[test]
    fn test_parse_toml_with_snap() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_commit = "abc1234"
trunk = "main"
snap_command = "claude --model opus"
"#;
        let meta: WorktreeMeta = toml::from_str(toml).unwrap();
        assert_eq!(meta.snap_command, Some("claude --model opus".to_string()));
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
        // Neither exists → returns new format path
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
    // base_branch 字段
    // -----------------------------------------------------------------------

    #[test]
    fn test_with_base_branch() {
        let meta = WorktreeMeta::new("abc".to_string(), "main".to_string())
            .with_base_branch("feature-a".to_string());
        assert_eq!(meta.base_branch, Some("feature-a".to_string()));
    }

    #[test]
    fn test_parse_toml_without_base_branch() {
        // 旧格式 TOML 缺少 base_branch → 反序列化为 None（向前兼容）
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_commit = "abc1234"
trunk = "main"
"#;
        let meta: WorktreeMeta = toml::from_str(toml).unwrap();
        assert!(meta.base_branch.is_none());
    }

    #[test]
    fn test_parse_toml_with_base_branch() {
        let toml = r#"
created_at = "2024-01-15T10:30:00Z"
base_commit = "abc1234"
trunk = "main"
base_branch = "feature-x"
"#;
        let meta: WorktreeMeta = toml::from_str(toml).unwrap();
        assert_eq!(meta.base_branch, Some("feature-x".to_string()));
    }

    #[test]
    fn test_base_branch_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.toml");

        let meta = WorktreeMeta::new("abc".to_string(), "main".to_string())
            .with_base_branch("dev".to_string());
        meta.save(&path).unwrap();

        let loaded = WorktreeMeta::load(&path).unwrap();
        assert_eq!(loaded.base_branch, Some("dev".to_string()));
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
        let result = resolve_target_branch(
            None,
            Some("feature-a"),
            |_| false, // branch 已删除
            "main",
        );
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
        let meta = WorktreeMeta::new("abc".to_string(), "main".to_string())
            .with_base_branch("feature-a".to_string());
        meta.save(&meta_path(dir.path(), "my-branch")).unwrap();

        let result =
            resolve_effective_target(dir.path(), "my-branch", None, |b| b == "feature-a", "main");
        assert_eq!(result, "feature-a");
    }

    #[test]
    fn test_effective_target_no_meta_falls_back_to_trunk() {
        let dir = tempdir().unwrap();
        // 没有 meta 文件 → fallback 到 trunk
        let result = resolve_effective_target(dir.path(), "my-branch", None, |_| true, "main");
        assert_eq!(result, "main");
    }

    #[test]
    fn test_effective_target_cli_override_wins() {
        let dir = tempdir().unwrap();
        let meta = WorktreeMeta::new("abc".to_string(), "main".to_string())
            .with_base_branch("feature-a".to_string());
        meta.save(&meta_path(dir.path(), "my-branch")).unwrap();

        let result =
            resolve_effective_target(dir.path(), "my-branch", Some("release"), |_| true, "main");
        assert_eq!(result, "release");
    }
}
