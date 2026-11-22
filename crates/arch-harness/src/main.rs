//! `aurdeck-arch-harness <scenario> [--no-overlay]`: drives the same path the
//! desktop app takes (review store, AUR fetch and approval, planner, review
//! overlay, `start_transaction`) against real pacman and yay, without Tauri.
//!
//! It installs and removes real packages, so it only starts inside the
//! disposable container built by `tests/arch/Dockerfile`.

mod guard;
mod prompts;
mod scenario;
mod screen;

use std::process::ExitCode;

/// Which scenarios to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    RepoInstall,
    AurInstall,
    PinCommitMismatch,
    PinPkgbuildMismatch,
    PinUnapprovedBase,
    PinUnreviewedUpgrade,
    Remove,
    All,
}

/// Parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub struct Cli {
    pub which: Which,
    /// Diagnostic only: run the AUR build without the review-pin overlay.
    pub no_overlay: bool,
}

const USAGE: &str = "usage: aurdeck-arch-harness <repo-install|aur-install|pin-commit-mismatch|pin-pkgbuild-mismatch|pin-unapproved-base|pin-unreviewed-upgrade|remove|all> [--no-overlay]";

pub fn parse_args(args: &[String]) -> Result<Cli, String> {
    let mut which = None;
    let mut no_overlay = false;
    for a in args {
        match a.as_str() {
            "--no-overlay" => no_overlay = true,
            "repo-install"
            | "aur-install"
            | "pin-commit-mismatch"
            | "pin-unapproved-base"
            | "pin-unreviewed-upgrade"
            | "remove"
            | "all"
                if which.is_none() =>
            {
                which = Some(match a.as_str() {
                    "repo-install" => Which::RepoInstall,
                    "aur-install" => Which::AurInstall,
                    "pin-commit-mismatch" => Which::PinCommitMismatch,
                    "pin-pkgbuild-mismatch" => Which::PinPkgbuildMismatch,
                    "pin-unapproved-base" => Which::PinUnapprovedBase,
                    "pin-unreviewed-upgrade" => Which::PinUnreviewedUpgrade,
                    "remove" => Which::Remove,
                    _ => Which::All,
                });
            }
            other => return Err(format!("unexpected argument {other:?}\n{USAGE}")),
        }
    }
    let which = which.ok_or_else(|| USAGE.to_owned())?;
    Ok(Cli { which, no_overlay })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(&args) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let inputs = match guard::read_inputs() {
        Ok(i) => i,
        Err(e) => {
            eprintln!("aurdeck-arch-harness: refusing to start: {e}");
            return ExitCode::from(3);
        }
    };
    if let Err(e) = guard::check(&inputs) {
        eprintln!("aurdeck-arch-harness: refusing to start: {e}");
        return ExitCode::from(3);
    }
    match scenario::run(&cli) {
        Ok(()) => {
            println!("\nHARNESS OK");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("\nHARNESS FAILED: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_scenarios() {
        assert_eq!(
            parse_args(&a(&["all"])),
            Ok(Cli {
                which: Which::All,
                no_overlay: false
            })
        );
        assert_eq!(
            parse_args(&a(&["aur-install", "--no-overlay"])),
            Ok(Cli {
                which: Which::AurInstall,
                no_overlay: true
            })
        );
        assert_eq!(
            parse_args(&a(&["--no-overlay", "remove"])).map(|c| c.which),
            Ok(Which::Remove)
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse_args(&a(&[])).is_err());
        assert!(parse_args(&a(&["nope"])).is_err());
        assert!(parse_args(&a(&["all", "remove"])).is_err());
        assert!(parse_args(&a(&["all", "--yes"])).is_err());
    }
}
