//! Container guard: the harness installs and removes real packages, so it
//! refuses to start anywhere but inside the disposable test image.

use std::os::unix::fs::MetadataExt as _;

use aurdeck_core::{SafetyError, ensure_not_root};

/// Environment variable the Dockerfile sets; nothing else does.
pub const ENV_FLAG: &str = "AURDECK_ARCH_HARNESS";
/// Root-owned file the Dockerfile writes. A distrobox or toolbox on an Arch
/// host has the container marker files but not this one.
pub const IMAGE_MARKER: &str = "/etc/aurdeck-arch-harness-image";
/// The only user and home directory the test image has.
pub const EXPECTED_USER: &str = "dev";
/// Home directory of [`EXPECTED_USER`].
pub const EXPECTED_HOME: &str = "/home/dev";

/// Everything the guard looks at, injected so it can be unit-tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuardInputs {
    /// Value of `AURDECK_ARCH_HARNESS`, if set.
    pub env_flag: Option<String>,
    /// `/.dockerenv` exists.
    pub dockerenv: bool,
    /// `/run/.containerenv` exists.
    pub containerenv: bool,
    /// [`IMAGE_MARKER`] exists, is owned by root and is not writable by others.
    pub image_marker: bool,
    /// Real uid of this process.
    pub uid: u32,
    /// Name of that uid in `/etc/passwd`.
    pub user: Option<String>,
    /// Value of `$HOME`.
    pub home: Option<String>,
}

/// Why the harness refuses to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardError {
    /// The opt-in variable is missing or not `1`.
    MissingEnv,
    /// No container marker file.
    NotInContainer,
    /// The image marker file is missing or not root-owned.
    NotTestImage,
    /// Running as root.
    Root(SafetyError),
    /// Not the image's `dev` user.
    WrongUser,
    /// `$HOME` is not `/home/dev`.
    WrongHome,
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
            Self::NotTestImage => write!(
                f,
                "{IMAGE_MARKER} is missing or not root-owned; this is not the image built by tests/arch/Dockerfile"
            ),
            Self::Root(e) => write!(f, "{e}"),
            Self::WrongUser => write!(f, "the user is not {EXPECTED_USER:?}"),
            Self::WrongHome => write!(f, "$HOME is not {EXPECTED_HOME}"),
        }
    }
}

/// Checks every condition; all must hold.
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
    if !i.image_marker {
        return Err(GuardError::NotTestImage);
    }
    ensure_not_root(i.uid).map_err(GuardError::Root)?;
    if i.user.as_deref() != Some(EXPECTED_USER) {
        return Err(GuardError::WrongUser);
    }
    if i.home.as_deref() != Some(EXPECTED_HOME) {
        return Err(GuardError::WrongHome);
    }
    Ok(())
}

/// Reads the real inputs from the running process.
///
/// # Errors
/// Fails when the uid cannot be determined (treated as unsafe).
pub fn read_inputs() -> Result<GuardInputs, String> {
    let status = std::fs::read_to_string("/proc/self/status")
        .map_err(|e| format!("cannot read /proc/self/status: {e}"))?;
    let uid = parse_uid(&status).ok_or("cannot determine the real uid")?;
    let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
    let image_marker = std::fs::metadata(IMAGE_MARKER)
        .is_ok_and(|m| m.is_file() && m.uid() == 0 && m.mode() & 0o022 == 0);
    Ok(GuardInputs {
        env_flag: std::env::var(ENV_FLAG).ok(),
        dockerenv: std::path::Path::new("/.dockerenv").exists(),
        containerenv: std::path::Path::new("/run/.containerenv").exists(),
        image_marker,
        uid,
        user: passwd_name(&passwd, uid),
        home: std::env::var("HOME").ok(),
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

/// User name of `uid` in `/etc/passwd` text.
fn passwd_name(passwd: &str, uid: u32) -> Option<String> {
    passwd.lines().find_map(|l| {
        let mut f = l.split(':');
        let name = f.next()?;
        let _pw = f.next()?;
        (f.next()?.parse::<u32>().ok()? == uid).then(|| name.to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok() -> GuardInputs {
        GuardInputs {
            env_flag: Some("1".into()),
            dockerenv: true,
            containerenv: false,
            image_marker: true,
            uid: 1000,
            user: Some("dev".into()),
            home: Some("/home/dev".into()),
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
    fn needs_the_image_marker() {
        // A distrobox or toolbox on an Arch host: container marker, no image marker.
        let i = GuardInputs {
            dockerenv: false,
            containerenv: true,
            image_marker: false,
            ..ok()
        };
        assert_eq!(check(&i), Err(GuardError::NotTestImage));
    }

    #[test]
    fn refuses_root() {
        let i = GuardInputs {
            uid: 0,
            user: Some("root".into()),
            ..ok()
        };
        assert_eq!(check(&i), Err(GuardError::Root(SafetyError::RunningAsRoot)));
    }

    #[test]
    fn user_must_be_dev() {
        for u in [None, Some("alice"), Some("root"), Some("Dev")] {
            let i = GuardInputs {
                user: u.map(String::from),
                ..ok()
            };
            assert_eq!(check(&i), Err(GuardError::WrongUser), "{u:?}");
        }
    }

    #[test]
    fn home_must_be_home_dev() {
        for h in [None, Some("/home/alice"), Some("/root"), Some("/home/dev/")] {
            let i = GuardInputs {
                home: h.map(String::from),
                ..ok()
            };
            assert_eq!(check(&i), Err(GuardError::WrongHome), "{h:?}");
        }
    }

    #[test]
    fn parses_proc_status() {
        let s = "Name:\tx\nUid:\t1000\t1000\t1000\t1000\nGid:\t1\t1\t1\t1\n";
        assert_eq!(parse_uid(s), Some(1000));
        assert_eq!(parse_uid("Uid:\t0\t1000\t0\t0\n"), Some(0));
        assert_eq!(parse_uid("Name:\tx\n"), None);
        assert_eq!(parse_uid("Uid:\tabc\n"), None);
    }

    #[test]
    fn parses_passwd() {
        let p = "root:x:0:0::/root:/bin/bash\ndev:x:1000:1000::/home/dev:/bin/bash\n";
        assert_eq!(passwd_name(p, 1000).as_deref(), Some("dev"));
        assert_eq!(passwd_name(p, 0).as_deref(), Some("root"));
        assert_eq!(passwd_name(p, 5), None);
        assert_eq!(passwd_name("garbage", 0), None);
    }
}
