// ===========================================================================
// update - Version Update Check
// ===========================================================================

use std::path::Path;
use std::time::{Duration, SystemTime};

pub type Result<T> = std::result::Result<T, Error>;

const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours
const MARKER_FILE: &str = "last_update_check";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(String),

    #[error("parse error: {0}")]
    Parse(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Check if we should perform update check (once per day)
pub fn should_check(base_dir: &Path) -> bool {
    let marker = base_dir.join(MARKER_FILE);
    if !marker.exists() {
        return true;
    }

    marker
        .metadata()
        .and_then(|m| m.modified())
        .map(|mtime| SystemTime::now().duration_since(mtime).unwrap_or_default() > CHECK_INTERVAL)
        .unwrap_or(true)
}

/// Mark that we've checked for updates
pub fn mark_checked(base_dir: &Path) -> Result<()> {
    let marker = base_dir.join(MARKER_FILE);
    std::fs::write(&marker, "")?;
    Ok(())
}

/// Compare versions: returns true if latest > current
pub fn compare_versions(current: &str, latest: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let current_parts = parse(current);
    let latest_parts = parse(latest);

    for i in 0..current_parts.len().max(latest_parts.len()) {
        let c = current_parts.get(i).copied().unwrap_or(0);
        let l = latest_parts.get(i).copied().unwrap_or(0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

/// Check for updates from npm registry
/// Returns Some(latest_version) if update available, None otherwise
pub fn check_update(current_version: &str) -> Result<Option<String>> {
    let url = "https://registry.npmjs.org/agent-worktree/latest";

    let agent = ureq::Agent::new_with_config(
        ureq::config::Config::builder()
            .timeout_global(Some(std::time::Duration::from_secs(5)))
            .build(),
    );

    let body: String = agent
        .get(url)
        .call()
        .map_err(|e| Error::Network(e.to_string()))?
        .body_mut()
        .read_to_string()
        .map_err(|e| Error::Parse(e.to_string()))?;

    // Parse JSON to get version field
    let version = body
        .split("\"version\":")
        .nth(1)
        .and_then(|s: &str| s.split('"').nth(1))
        .ok_or_else(|| Error::Parse("version field not found".into()))?;

    if compare_versions(current_version, version) {
        Ok(Some(version.to_string()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn test_should_check_no_marker_file() {
        // No marker file = should check
        let temp = TempDir::new().unwrap();
        assert!(should_check(temp.path()));
    }

    #[test]
    fn test_should_check_fresh_marker() {
        // Fresh marker (just created) = should NOT check
        let temp = TempDir::new().unwrap();
        let marker = temp.path().join("last_update_check");
        std::fs::write(&marker, "").unwrap();
        assert!(!should_check(temp.path()));
    }

    #[test]
    fn test_should_check_stale_marker() {
        // Marker older than 24 hours = should check
        let temp = TempDir::new().unwrap();
        let marker = temp.path().join("last_update_check");
        std::fs::write(&marker, "").unwrap();

        // Set mtime to 25 hours ago
        let old_time = std::time::SystemTime::now() - std::time::Duration::from_secs(25 * 60 * 60);
        filetime::set_file_mtime(&marker, filetime::FileTime::from_system_time(old_time)).unwrap();

        assert!(should_check(temp.path()));
    }

    #[test]
    fn test_mark_checked_creates_marker() {
        let temp = TempDir::new().unwrap();
        let marker = temp.path().join("last_update_check");
        assert!(!marker.exists());

        mark_checked(temp.path()).unwrap();

        assert!(marker.exists());
    }

    #[test]
    fn test_check_update_same_version_returns_none() {
        // Mock this by testing version comparison logic
        let result = compare_versions("0.4.5", "0.4.5");
        assert!(!result);
    }

    #[test]
    fn test_check_update_older_version_returns_none() {
        let result = compare_versions("0.4.5", "0.4.4");
        assert!(!result);
    }

    #[test]
    fn test_check_update_newer_version_returns_true() {
        let result = compare_versions("0.4.5", "0.4.6");
        assert!(result);

        let result = compare_versions("0.4.5", "0.5.0");
        assert!(result);

        let result = compare_versions("0.4.5", "1.0.0");
        assert!(result);
    }

    #[test]
    fn test_compare_versions_edge_cases() {
        // Different segment counts
        assert!(compare_versions("0.4", "0.4.1"));
        assert!(!compare_versions("0.4.1", "0.4"));

        // Large numbers
        assert!(compare_versions("0.9.9", "0.10.0"));
    }
}
