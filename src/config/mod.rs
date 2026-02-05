// ===========================================================================
// config - Configuration Loading & Merging
// ===========================================================================

use std::path::{Path, PathBuf};

use directories::BaseDirs;
use serde::{Deserialize, Serialize};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read config: {0}")]
    Read(#[from] std::io::Error),

    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),

    #[error("failed to serialize config: {0}")]
    Serialize(#[from] toml::ser::Error),

    #[error("home directory not found")]
    NoHome,
}

// ---------------------------------------------------------------------------
// Global Config (~/.agent-worktree/config.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    #[serde(default)]
    pub general: GeneralConfig,

    #[serde(default)]
    pub hooks: HooksConfig,
}

// ---------------------------------------------------------------------------
// Project Config (.agent-worktree.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    #[serde(default)]
    pub general: ProjectGeneralConfig,

    #[serde(default)]
    pub hooks: HooksConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_merge_strategy")]
    pub merge_strategy: MergeStrategy,

    #[serde(default)]
    pub copy_files: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectGeneralConfig {
    pub trunk: Option<String>,

    #[serde(default)]
    pub copy_files: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default)]
    pub post_create: Vec<String>,

    #[serde(default)]
    pub pre_merge: Vec<String>,

    #[serde(default)]
    pub post_merge: Vec<String>,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    #[default]
    Squash,
    Merge,
    Rebase,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SyncStrategy {
    #[default]
    Rebase,
    Merge,
}

fn default_merge_strategy() -> MergeStrategy {
    MergeStrategy::Squash
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            merge_strategy: MergeStrategy::Squash,
            copy_files: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Merged Config (runtime)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Config {
    pub base_dir: PathBuf,
    pub workspaces_dir: PathBuf,
    pub merge_strategy: MergeStrategy,
    pub copy_files: Vec<String>,
    pub hooks: HooksConfig,
    pub trunk: Option<String>,
}

impl Config {
    /// Load and merge global + project config
    pub fn load() -> Result<Self> {
        let base_dir = Self::base_dir()?;
        let workspaces_dir = base_dir.join("workspaces");

        let global = Self::load_global(&base_dir)?;
        let project = Self::load_project()?;

        // Merge: project overrides global
        let merge_strategy = global.general.merge_strategy;
        let mut copy_files = global.general.copy_files;
        copy_files.extend(project.general.copy_files);

        let hooks = HooksConfig {
            post_create: merge_hooks(&global.hooks.post_create, &project.hooks.post_create),
            pre_merge: merge_hooks(&global.hooks.pre_merge, &project.hooks.pre_merge),
            post_merge: merge_hooks(&global.hooks.post_merge, &project.hooks.post_merge),
        };

        Ok(Self {
            base_dir,
            workspaces_dir,
            merge_strategy,
            copy_files,
            hooks,
            trunk: project.general.trunk,
        })
    }

    pub fn base_dir() -> Result<PathBuf> {
        let base = BaseDirs::new().ok_or(Error::NoHome)?;
        Ok(base.home_dir().join(".agent-worktree"))
    }

    fn load_global(base_dir: &Path) -> Result<GlobalConfig> {
        let path = base_dir.join("config.toml");
        if !path.exists() {
            return Ok(GlobalConfig::default());
        }
        let content = std::fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    fn load_project() -> Result<ProjectConfig> {
        let path = Path::new(".agent-worktree.toml");
        if !path.exists() {
            return Ok(ProjectConfig::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }
}

fn merge_hooks(global: &[String], project: &[String]) -> Vec<String> {
    if project.is_empty() {
        global.to_vec()
    } else {
        project.to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_config_defaults() {
        let config = GlobalConfig::default();
        assert_eq!(config.general.merge_strategy, MergeStrategy::Squash);
        assert!(config.general.copy_files.is_empty());
        assert!(config.hooks.post_create.is_empty());
    }

    #[test]
    fn test_project_config_defaults() {
        let config = ProjectConfig::default();
        assert!(config.general.trunk.is_none());
        assert!(config.general.copy_files.is_empty());
    }

    #[test]
    fn test_global_config_parse() {
        let toml = r#"
[general]
merge_strategy = "rebase"
copy_files = ["*.secret"]

[hooks]
post_create = ["npm install"]
pre_merge = ["npm test"]
"#;
        let config: GlobalConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.general.merge_strategy, MergeStrategy::Rebase);
        assert_eq!(config.general.copy_files, vec!["*.secret"]);
        assert_eq!(config.hooks.post_create, vec!["npm install"]);
        assert_eq!(config.hooks.pre_merge, vec!["npm test"]);
    }

    #[test]
    fn test_project_config_parse() {
        let toml = r#"
[general]
trunk = "develop"
copy_files = [".env", ".env.local"]

[hooks]
post_create = ["pnpm install"]
"#;
        let config: ProjectConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.general.trunk, Some("develop".to_string()));
        assert_eq!(config.general.copy_files, vec![".env", ".env.local"]);
        assert_eq!(config.hooks.post_create, vec!["pnpm install"]);
    }

    #[test]
    fn test_merge_hooks_empty_project() {
        let global = vec!["global-hook".to_string()];
        let project: Vec<String> = vec![];
        let merged = merge_hooks(&global, &project);
        assert_eq!(merged, vec!["global-hook"]);
    }

    #[test]
    fn test_merge_hooks_project_overrides() {
        let global = vec!["global-hook".to_string()];
        let project = vec!["project-hook".to_string()];
        let merged = merge_hooks(&global, &project);
        assert_eq!(merged, vec!["project-hook"]);
    }

    #[test]
    fn test_merge_strategy_roundtrip() {
        // Test via GlobalConfig since toml can't serialize bare enums
        let toml_squash = r#"[general]
merge_strategy = "squash"
"#;
        let config: GlobalConfig = toml::from_str(toml_squash).unwrap();
        assert_eq!(config.general.merge_strategy, MergeStrategy::Squash);

        let toml_rebase = r#"[general]
merge_strategy = "rebase"
"#;
        let config: GlobalConfig = toml::from_str(toml_rebase).unwrap();
        assert_eq!(config.general.merge_strategy, MergeStrategy::Rebase);
    }

    // =========================================================================
    // Additional tests for better coverage
    // =========================================================================

    #[test]
    fn test_default_merge_strategy() {
        let strategy = default_merge_strategy();
        assert_eq!(strategy, MergeStrategy::Squash);
    }

    #[test]
    fn test_merge_strategy_merge() {
        let toml = r#"[general]
merge_strategy = "merge"
"#;
        let config: GlobalConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.general.merge_strategy, MergeStrategy::Merge);
    }

    #[test]
    fn test_sync_strategy_defaults() {
        assert_eq!(SyncStrategy::default(), SyncStrategy::Rebase);
    }

    #[test]
    fn test_sync_strategy_parse() {
        #[derive(Deserialize)]
        struct TestConfig {
            sync: SyncStrategy,
        }
        let toml = r#"sync = "merge""#;
        let config: TestConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.sync, SyncStrategy::Merge);

        let toml = r#"sync = "rebase""#;
        let config: TestConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.sync, SyncStrategy::Rebase);
    }

    #[test]
    fn test_hooks_config_defaults() {
        let hooks = HooksConfig::default();
        assert!(hooks.post_create.is_empty());
        assert!(hooks.pre_merge.is_empty());
        assert!(hooks.post_merge.is_empty());
    }

    #[test]
    fn test_hooks_config_with_post_merge() {
        let toml = r#"
[hooks]
post_merge = ["git push", "notify-team"]
"#;
        let config: GlobalConfig = toml::from_str(toml).unwrap();
        assert_eq!(
            config.hooks.post_merge,
            vec!["git push", "notify-team"]
        );
    }

    #[test]
    fn test_general_config_default() {
        let general = GeneralConfig::default();
        assert_eq!(general.merge_strategy, MergeStrategy::Squash);
        assert!(general.copy_files.is_empty());
    }

    #[test]
    fn test_project_general_config_defaults() {
        let general = ProjectGeneralConfig::default();
        assert!(general.trunk.is_none());
        assert!(general.copy_files.is_empty());
    }

    #[test]
    fn test_config_base_dir() {
        let result = Config::base_dir();
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.to_string_lossy().contains(".agent-worktree"));
    }

    #[test]
    fn test_error_display() {
        let err = Error::NoHome;
        assert_eq!(err.to_string(), "home directory not found");
    }

    #[test]
    fn test_global_config_serialize() {
        let config = GlobalConfig {
            general: GeneralConfig {
                merge_strategy: MergeStrategy::Rebase,
                copy_files: vec![".env".to_string()],
            },
            hooks: HooksConfig {
                post_create: vec!["npm install".to_string()],
                pre_merge: vec![],
                post_merge: vec![],
            },
        };
        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("rebase"));
        assert!(serialized.contains(".env"));
        assert!(serialized.contains("npm install"));
    }

    #[test]
    fn test_project_config_serialize() {
        let config = ProjectConfig {
            general: ProjectGeneralConfig {
                trunk: Some("develop".to_string()),
                copy_files: vec![".env.local".to_string()],
            },
            hooks: HooksConfig::default(),
        };
        let serialized = toml::to_string(&config).unwrap();
        assert!(serialized.contains("develop"));
        assert!(serialized.contains(".env.local"));
    }

    #[test]
    fn test_merge_hooks_both_empty() {
        let global: Vec<String> = vec![];
        let project: Vec<String> = vec![];
        let merged = merge_hooks(&global, &project);
        assert!(merged.is_empty());
    }

    #[test]
    fn test_global_config_clone() {
        let config = GlobalConfig::default();
        let cloned = config.clone();
        assert_eq!(cloned.general.merge_strategy, config.general.merge_strategy);
    }

    #[test]
    fn test_merge_strategy_equality() {
        assert_eq!(MergeStrategy::Squash, MergeStrategy::Squash);
        assert_ne!(MergeStrategy::Squash, MergeStrategy::Merge);
        assert_ne!(MergeStrategy::Merge, MergeStrategy::Rebase);
    }

    #[test]
    fn test_sync_strategy_equality() {
        assert_eq!(SyncStrategy::Rebase, SyncStrategy::Rebase);
        assert_ne!(SyncStrategy::Rebase, SyncStrategy::Merge);
    }

    #[test]
    fn test_merge_strategy_copy() {
        let strategy = MergeStrategy::Squash;
        let copied = strategy;
        assert_eq!(strategy, copied);
    }

    #[test]
    fn test_config_debug() {
        let config = GlobalConfig::default();
        let debug = format!("{:?}", config);
        assert!(debug.contains("GlobalConfig"));
    }
}
