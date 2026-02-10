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
}

impl WorktreeMeta {
    pub fn new(base_commit: String, trunk: String) -> Self {
        Self {
            created_at: Utc::now(),
            base_commit,
            trunk,
            snap_command: None,
        }
    }

    pub fn with_snap(mut self, command: String) -> Self {
        self.snap_command = Some(command);
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
}
