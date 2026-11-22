//! Container guard: the harness installs and removes real packages, so it
//! refuses to start anywhere but inside the disposable test container.

use aurdeck_core::{SafetyError, ensure_not_root};

/// Environment variable the Dockerfile sets; nothing else does.
pub const ENV_FLAG: &str = "AURDECK_ARCH_HARNESS";

/// Everything the guard looks at, injected so it can be unit-tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardInputs {
    /// Value of `AURDECK_ARCH_HARNESS`, if set.
    pub env_flag: Option<String>,
    /// `/.dockerenv` exists.
    pub dockerenv: bool,
    /// `/run/.containerenv` exists.
    pub containerenv: bool,
    /// Real uid of this process.
    pub uid: u32,
}

/// Why the harness refuses to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    /// The opt-in variable is missing or not `1`.
    MissingEnv,
    /// No container marker file.
    NotInContainer,
    /// Running as root.
    Root(SafetyError),
}

impl std::fmt::Display for GuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEnv => write!(f, "{ENV_FLAG}=1 is not set; only the test image sets it"),
            Self::NotInContainer => write!(
                f,
                "neither /.dockerenv nor /run/.containerenv exists; this harness installs and \
                 removes real packages and runs only inside the disposable Arch container"
            ),
            Self::Root(e) => write!(f, "{e}"),
        }
    }
}

/// Checks all three conditions; every one must hold.
///
/// # Errors
/// Returns the first violated condition.
pub fn check(i: &GuardInputs) -> Result<(), GuardError> {
    if i.env_flag.as_deref() != Some("1") {
        return Err(GuardError::MissingEnv);
    }
    if !(i.dockerenv || i.containerenv) {
        return Err(GuardError::NotInContainer);
    }
    ensure_not_root(i.uid).map_err(GuardError::Root)
}

/// Reads the real inputs from the running process.
///
/// # Errors
/// Fails when the uid cannot be determined (treated as unsafe).
pub fn read_inputs() -> Result<GuardInputs, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("cannot read /proc/self/status: {e}"))?;
    let uid = parse_uid(&status).ok_or("cannot determine the real uid")?;
    Ok(GuardInputs {
        env_flag: std::env::var(ENV_FLAG).ok(),
        dockerenv: std::path::Path::new("/.dockerenv").exists(),
        containerenv: std::path::Path::new("/run/.containerenv").exists(),
        uid,
    })
}

/// Real uid from the text of `/proc/self/status` (`Uid:\treal\teffective\t...`).
fn parse_uid(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|l| l.strip_prefix("Uid:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok() -> GuardInputs {
        GuardInputs {
            env_flag: Some("1".into()),
            dockerenv: true,
            containerenv: false,
            uid: 1000,
        }
    }

    #[test]
    fn all_conditions_pass() {
        assert_eq!(check(&ok()), Ok(()));
        let podman = GuardInputs {
            dockerenv: false,
            containerenv: true,
            ..ok()
        };
        assert_eq!(check(&podman), Ok(()));
    }

    #[test]
    fn env_must_be_exactly_one() {
        for v in [None, Some(""), Some("0"), Some("true"), Some("11")] {
            let i = GuardInputs {
                env_flag: v.map(String::from),
                ..ok()
            };
            assert_eq!(check(&i), Err(GuardError::MissingEnv), "{v:?}");
        }
    }

    #[test]
    fn needs_a_container_marker() {
        let i = GuardInputs {
            dockerenv: false,
            containerenv: false,
            ..ok()
        };
        assert_eq!(check(&i), Err(GuardError::NotInContainer));
    }

    #[test]
    fn refuses_root() {
        let i = GuardInputs { uid: 0, ..ok() };
        assert_eq!(check(&i), Err(GuardError::Root(SafetyError::RunningAsRoot)));
    }

    #[test]
    fn parses_proc_status() {
        let s = "Name:\tx\nUid:\t1000\t1000\t1000\t1000\nGid:\t1\t1\t1\t1\n";
        assert_eq!(parse_uid(s), Some(1000));
        assert_eq!(parse_uid("Uid:\t0\t1000\t0\t0\n"), Some(0));
        assert_eq!(parse_uid("Name:\tx\n"), None);
        assert_eq!(parse_uid("Uid:\tabc\n"), None);
    }
}
