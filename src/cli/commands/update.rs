// ===========================================================================
// cli/commands/update - Self-update Command
// ===========================================================================

use crate::cli;
use crate::update;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 更新行为：纯逻辑，不涉及 IO
#[derive(Debug)]
pub enum UpdateAction {
    AlreadyUpToDate,
    UpdateAvailable(String),
}

/// 根据版本检查结果决定行为
pub fn determine_action(check_result: update::Result<Option<String>>) -> cli::Result<UpdateAction> {
    match check_result {
        Ok(Some(latest)) => Ok(UpdateAction::UpdateAvailable(latest)),
        Ok(None) => Ok(UpdateAction::AlreadyUpToDate),
        Err(e) => Err(cli::Error::Other(format!("failed to check for updates: {e}"))),
    }
}

/// 构造 npm install 命令参数
pub fn npm_install_args() -> Vec<&'static str> {
    vec!["install", "-g", "agent-worktree@latest"]
}

pub fn run() -> cli::Result<()> {
    eprintln!("Checking for updates...");

    let action = determine_action(update::check_update(VERSION))?;

    match action {
        UpdateAction::AlreadyUpToDate => {
            eprintln!("Already up to date ({})", VERSION);
        }
        UpdateAction::UpdateAvailable(latest) => {
            eprintln!("Updating agent-worktree: {} -> {}", VERSION, latest);

            let status = std::process::Command::new("npm")
                .args(npm_install_args())
                .status()
                .map_err(|e| cli::Error::Other(format!("failed to run npm: {e}")))?;

            if !status.success() {
                return Err(cli::Error::Other("npm install failed".into()));
            }

            eprintln!("Updated successfully!");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_action_no_update() {
        let result = determine_action(Ok(None));
        assert!(matches!(result, Ok(UpdateAction::AlreadyUpToDate)));
    }

    #[test]
    fn test_determine_action_has_update() {
        let result = determine_action(Ok(Some("1.0.0".to_string())));
        match result {
            Ok(UpdateAction::UpdateAvailable(v)) => assert_eq!(v, "1.0.0"),
            _ => panic!("expected UpdateAvailable"),
        }
    }

    #[test]
    fn test_determine_action_network_error() {
        let result = determine_action(Err(update::Error::Network("timeout".into())));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("failed to check for updates"));
    }

    #[test]
    fn test_npm_install_args() {
        let args = npm_install_args();
        assert_eq!(args, vec!["install", "-g", "agent-worktree@latest"]);
    }
}
