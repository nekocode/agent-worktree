// ===========================================================================
// update - Version Update Check
// ===========================================================================

use rand::Rng;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(String),

    #[error("parse error: {0}")]
    Parse(String),
}

/// Check if we should perform update check (10% probability)
pub fn should_check() -> bool {
    rand::rng().random_ratio(1, 10)
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

    #[test]
    fn test_should_check_returns_bool() {
        // Run multiple times to verify it returns bool (not panic)
        for _ in 0..100 {
            let _ = should_check();
        }
    }

    #[test]
    fn test_should_check_roughly_10_percent() {
        // Statistical test: run 1000 times, expect ~100 hits (10%)
        // Allow 5-20% range to account for randomness
        let hits: usize = (0..1000).filter(|_| should_check()).count();
        assert!(hits >= 50 && hits <= 200, "Expected ~10% but got {}%", hits / 10);
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
