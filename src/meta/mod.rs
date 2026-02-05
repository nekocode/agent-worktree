// ===========================================================================
// meta - Worktree Metadata (.status.toml)
// ===========================================================================

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
        let path = dir.path().join("test.status.toml");

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
}
