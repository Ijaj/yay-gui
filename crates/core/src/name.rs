//! Validated package names.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Maximum accepted length of a package name.
pub const MAX_NAME_LEN: usize = 255;

/// Why a package name was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidPackageName {
    /// The name was empty.
    #[error("package name is empty")]
    Empty,
    /// The name exceeded [`MAX_NAME_LEN`].
    #[error("package name is longer than {MAX_NAME_LEN} characters")]
    TooLong,
    /// The name starts with `-` or `.`.
    #[error("package name must not start with '-' or '.'")]
    BadStart,
    /// The name contains a character outside `a-z 0-9 @ . _ + -`.
    #[error("package name contains invalid character {0:?}")]
    BadChar(char),
}

/// A package name validated against pacman/AUR naming rules.
///
/// Allowed: lowercase alphanumerics and `@ . _ + -`; not empty, not starting
/// with `-` or `.`, at most 255 bytes. A valid name can never look like a flag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct PackageName(String);

impl PackageName {
    /// Validates and wraps `s`.
    pub fn new(s: impl Into<String>) -> Result<Self, InvalidPackageName> {
        let s = s.into();
        validate(&s)?;
        Ok(Self(s))
    }

    /// The name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn validate(s: &str) -> Result<(), InvalidPackageName> {
    if s.is_empty() {
        return Err(InvalidPackageName::Empty);
    }
    if s.len() > MAX_NAME_LEN {
        return Err(InvalidPackageName::TooLong);
    }
    if s.starts_with(['-', '.']) {
        return Err(InvalidPackageName::BadStart);
    }
    if let Some(c) = s
        .chars()
        .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || "@._+-".contains(*c)))
    {
        return Err(InvalidPackageName::BadChar(c));
    }
    Ok(())
}

impl FromStr for PackageName {
    type Err = InvalidPackageName;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl TryFrom<String> for PackageName {
    type Error = InvalidPackageName;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::new(s)
    }
}

impl From<PackageName> for String {
    fn from(n: PackageName) -> Self {
        n.0
    }
}

impl fmt::Display for PackageName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for PackageName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn accepts_valid() {
        for n in [
            "yay",
            "lib32-glibc",
            "gtk2+extra",
            "python_foo.bar",
            "a@b",
            "7zip",
        ] {
            assert!(n.parse::<PackageName>().is_ok(), "{n}");
        }
    }

    #[test]
    fn rejects_invalid() {
        assert_eq!("".parse::<PackageName>(), Err(InvalidPackageName::Empty));
        assert_eq!(
            "-S".parse::<PackageName>(),
            Err(InvalidPackageName::BadStart)
        );
        assert_eq!(
            ".x".parse::<PackageName>(),
            Err(InvalidPackageName::BadStart)
        );
        assert_eq!(
            "Foo".parse::<PackageName>(),
            Err(InvalidPackageName::BadChar('F'))
        );
        assert!("a b".parse::<PackageName>().is_err());
        assert!("a;rm".parse::<PackageName>().is_err());
        assert!("a/b".parse::<PackageName>().is_err());
        assert!("é".parse::<PackageName>().is_err());
        assert!("a".repeat(256).parse::<PackageName>().is_err());
        assert!("a".repeat(255).parse::<PackageName>().is_ok());
    }

    #[test]
    fn serde_validates() {
        let n: PackageName = serde_json::from_str("\"yay\"").unwrap();
        assert_eq!(serde_json::to_string(&n).unwrap(), "\"yay\"");
        assert!(serde_json::from_str::<PackageName>("\"--noconfirm\"").is_err());
    }

    proptest! {
        #[test]
        fn accepted_never_starts_with_dash(s in "\\PC{0,40}") {
            if let Ok(n) = s.parse::<PackageName>() {
                prop_assert!(!n.as_str().starts_with('-'));
                prop_assert!(!n.as_str().is_empty());
            }
        }

        #[test]
        fn dash_prefixed_always_rejected(s in "-[a-z0-9@._+-]{0,40}") {
            prop_assert!(s.parse::<PackageName>().is_err());
        }
    }
}
