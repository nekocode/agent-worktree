// ===========================================================================
// wt setup - Install shell integration
// ===========================================================================

use clap::{Args, ValueEnum};

use crate::cli::{Error, Result};
use crate::shell::{self, Shell};

#[derive(Clone, Copy, ValueEnum)]
pub enum ShellArg {
    Bash,
    Zsh,
    Fish,
    #[value(alias = "pwsh")]
    Powershell,
}

impl From<ShellArg> for Shell {
    fn from(arg: ShellArg) -> Self {
        match arg {
            ShellArg::Bash => Shell::Bash,
            ShellArg::Zsh => Shell::Zsh,
            ShellArg::Fish => Shell::Fish,
            ShellArg::Powershell => Shell::PowerShell,
        }
    }
}

#[derive(Args)]
pub struct SetupArgs {
    /// Shell type (auto-detected if not specified)
    #[arg(long, value_enum)]
    shell: Option<ShellArg>,
}

pub fn run(args: SetupArgs) -> Result<()> {
    let shell: Shell = if let Some(shell_arg) = args.shell {
        shell_arg.into()
    } else {
        Shell::detect()
            .ok_or_else(|| Error::Other("Cannot detect shell. Use --shell to specify.".into()))?
    };

    let config_path = shell
        .config_file()
        .map_err(|e| Error::Other(e.to_string()))?;

    shell::install(shell).map_err(|e| Error::Other(e.to_string()))?;

    eprintln!("Shell integration installed!");
    eprintln!("Config: {}", config_path.display());
    eprintln!();
    eprintln!("Restart your shell or run:");
    match shell {
        Shell::PowerShell => eprintln!("  . {}", config_path.display()),
        _ => eprintln!("  source {}", config_path.display()),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_arg_to_shell_bash() {
        let shell: Shell = ShellArg::Bash.into();
        assert_eq!(shell, Shell::Bash);
    }

    #[test]
    fn test_shell_arg_to_shell_zsh() {
        let shell: Shell = ShellArg::Zsh.into();
        assert_eq!(shell, Shell::Zsh);
    }

    #[test]
    fn test_shell_arg_to_shell_fish() {
        let shell: Shell = ShellArg::Fish.into();
        assert_eq!(shell, Shell::Fish);
    }

    #[test]
    fn test_shell_arg_to_shell_powershell() {
        let shell: Shell = ShellArg::Powershell.into();
        assert_eq!(shell, Shell::PowerShell);
    }

    #[test]
    fn test_shell_arg_clone() {
        let arg = ShellArg::Bash;
        let cloned = arg;
        assert!(matches!(cloned, ShellArg::Bash));
    }

    #[test]
    fn test_shell_arg_copy() {
        let arg = ShellArg::Fish;
        let _copied: ShellArg = arg;
        let _still_valid: ShellArg = arg; // Copy trait
    }
}
