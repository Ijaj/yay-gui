//! Review-pinning overlay: a private `XDG_CONFIG_HOME` whose `yay/init.lua`
//! loads the user's config and then aborts builds whose commit differs from
//! the reviewed one.
//!
//! The Lua hook API is unverified, see `overlay_init.lua.tmpl`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

const TEMPLATE: &str = include_str!("overlay_init.lua.tmpl");

/// Errors creating the overlay.
#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    /// Filesystem error.
    #[error("overlay i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// An approved commit is not a 40 or 64 digit lowercase hex hash.
    #[error("invalid commit hash for {pkgbase:?}")]
    BadCommit {
        /// Offending package base.
        pkgbase: String,
    },
}

/// A private config dir that lives as long as this value.
pub struct Overlay {
    dir: TempDir,
}

impl Overlay {
    /// Builds the overlay. `user_config_dir` is the user's real config home
    /// (usually `~/.config`); `<it>/yay/init.lua` is loaded first and
    /// `<it>/yay/config.json` is copied when present. `approved` maps pkgbase
    /// to the reviewed git commit.
    ///
    /// # Errors
    /// Fails on I/O errors or a malformed commit hash.
    pub fn create(
        user_config_dir: &Path,
        approved: &BTreeMap<String, String>,
    ) -> Result<Self, OverlayError> {
        for (pkgbase, commit) in approved {
            let ok = matches!(commit.len(), 40 | 64)
                && commit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
            if !ok {
                return Err(OverlayError::BadCommit {
                    pkgbase: pkgbase.clone(),
                });
            }
        }
        let dir = tempfile::Builder::new()
            .prefix("yaygui-overlay-")
            .tempdir()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))?;
        }
        let yay_dir = dir.path().join("yay");
        std::fs::create_dir(&yay_dir)?;

        let user_yay = user_config_dir.join("yay");
        let user_init = user_yay.join("init.lua");
        let user_init_lit = if user_init.is_file() {
            lua_string_literal(&user_init.to_string_lossy())
        } else {
            "nil".to_owned()
        };
        let user_cfg = user_yay.join("config.json");
        if user_cfg.is_file() {
            std::fs::copy(&user_cfg, yay_dir.join("config.json"))?;
        }

        let mut entries = String::new();
        for (pkgbase, commit) in approved {
            let _ = writeln!(
                entries,
                "  [{}] = {},",
                lua_string_literal(pkgbase),
                lua_string_literal(commit)
            );
        }
        let init = TEMPLATE
            .replace("@@USER_INIT@@", &user_init_lit)
            .replace("@@APPROVED@@", entries.trim_end());
        std::fs::write(yay_dir.join("init.lua"), init)?;
        Ok(Self { dir })
    }

    /// The directory to use as `XDG_CONFIG_HOME`.
    #[must_use]
    pub fn config_home(&self) -> &Path {
        self.dir.path()
    }

    /// Path of the generated `init.lua`.
    #[must_use]
    pub fn init_lua_path(&self) -> PathBuf {
        self.dir.path().join("yay/init.lua")
    }

    /// Environment variables to put in `RunSpec::env`.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        vec![(
            "XDG_CONFIG_HOME".to_owned(),
            self.dir.path().to_string_lossy().into_owned(),
        )]
    }
}

/// Renders `s` as a double-quoted Lua string literal. Everything outside
/// printable ASCII is written as 3-digit decimal escapes per byte (valid in
/// Lua 5.1+), so quotes, newlines, `]]`, NULs and UTF-8 cannot break out.
pub(crate) fn lua_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for &b in s.as_bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(char::from(b)),
            _ => {
                let _ = write!(out, "\\{b:03}");
            }
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const H1: &str = "0123456789abcdef0123456789abcdef01234567";

    #[test]
    fn escapes_hostile_input() {
        assert_eq!(lua_string_literal("a\"b"), r#""a\"b""#);
        assert_eq!(lua_string_literal("a\\b"), r#""a\\b""#);
        assert_eq!(lua_string_literal("x\ny"), r#""x\010y""#);
        assert_eq!(lua_string_literal("]]--[["), r#""]]--[[""#); // inert inside "..."
        assert_eq!(lua_string_literal("\0"), r#""\000""#);
        assert_eq!(lua_string_literal("é"), r#""\195\169""#);
        let evil = "\"]] os.execute('x') --\n\r";
        let lit = lua_string_literal(evil);
        assert!(!lit[1..lit.len() - 1].contains(['\n', '\r']));
        // every interior quote is escaped
        let inner = &lit[1..lit.len() - 1];
        let mut prev_backslashes = 0;
        for c in inner.chars() {
            if c == '"' {
                assert_eq!(prev_backslashes % 2, 1);
            }
            prev_backslashes = if c == '\\' { prev_backslashes + 1 } else { 0 };
        }
    }

    #[test]
    fn creates_overlay_with_user_config() {
        let user = tempfile::tempdir().unwrap();
        std::fs::create_dir(user.path().join("yay")).unwrap();
        std::fs::write(user.path().join("yay/init.lua"), "-- mine").unwrap();
        std::fs::write(user.path().join("yay/config.json"), "{\"a\":1}").unwrap();
        let approved = BTreeMap::from([("pk\"g]]".to_owned(), H1.to_owned())]);
        let o = Overlay::create(user.path(), &approved).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        assert!(init.contains("pcall(dofile, user_init)"));
        assert!(init.contains(&lua_string_literal(
            &user.path().join("yay/init.lua").to_string_lossy()
        )));
        assert!(init.contains(&format!("[\"pk\\\"g]]\"] = \"{H1}\"")));
        assert!(init.contains("UNVERIFIED"));
        assert!(!init.contains("@@"));
        let cfg = std::fs::read_to_string(o.config_home().join("yay/config.json")).unwrap();
        assert_eq!(cfg, "{\"a\":1}");
        assert_eq!(o.env()[0].0, "XDG_CONFIG_HOME");
        let p = o.config_home().to_owned();
        drop(o);
        assert!(!p.exists());
    }

    #[test]
    fn works_without_user_config() {
        let user = tempfile::tempdir().unwrap();
        let o = Overlay::create(user.path(), &BTreeMap::new()).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        assert!(init.contains("local user_init = nil"));
        assert!(!o.config_home().join("yay/config.json").exists());
    }

    #[test]
    fn rejects_bad_commit() {
        let user = tempfile::tempdir().unwrap();
        let approved = BTreeMap::from([("p".to_owned(), "abc\") evil(".to_owned())]);
        assert!(matches!(
            Overlay::create(user.path(), &approved),
            Err(OverlayError::BadCommit { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dir_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let user = tempfile::tempdir().unwrap();
        let o = Overlay::create(user.path(), &BTreeMap::new()).unwrap();
        let mode = std::fs::metadata(o.config_home())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
    }
}
