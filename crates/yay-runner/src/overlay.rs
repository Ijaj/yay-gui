//! Review-pinning overlay: a private `XDG_CONFIG_HOME` whose `yay/init.lua`
//! loads the user's config and then aborts builds whose commit or PKGBUILD
//! differs from the reviewed one.
//!
//! Note: the commit-only [`Overlay::create`] (still used by the desktop app until
//! P0.5b) does NOT compare the PKGBUILD text, only the commit; use
//! [`Overlay::create_pinned`] for the full pin.
//!
//! The hook API is yay v13.1.0's real one, see `docs/yay-lua-hooks.md`.

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

/// What was approved in aurdeck for one package base.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedBase {
    /// Reviewed git commit (40 or 64 lowercase hex digits).
    pub commit: String,
    /// Reviewed PKGBUILD text at that commit. `None` only through the
    /// commit-only constructor [`Overlay::create`]: the text is then not compared.
    pub pkgbuild: Option<String>,
}

/// A private config dir that lives as long as this value.
pub struct Overlay {
    dir: TempDir,
}

impl Overlay {
    /// Commit-only pin, kept so existing callers compile unchanged. The
    /// PKGBUILD text is NOT compared. Prefer [`Overlay::create_pinned`] (not
    /// `#[deprecated]` only because that would break `-D warnings` in the
    /// desktop crate until P0.5b moves it).
    ///
    /// # Errors
    /// Fails on I/O errors or a malformed commit hash.
    pub fn create(
        user_config_dir: &Path,
        approved: &BTreeMap<String, String>,
    ) -> Result<Self, OverlayError> {
        let full = approved
            .iter()
            .map(|(base, commit)| {
                (
                    base.clone(),
                    ApprovedBase {
                        commit: commit.clone(),
                        pkgbuild: None,
                    },
                )
            })
            .collect();
        Self::create_pinned(user_config_dir, &full)
    }

    /// Builds the overlay. `user_config_dir` is the user's real config home
    /// (usually `~/.config`); `<it>/yay/init.lua` is loaded first,
    /// `<it>/yay` goes first on `package.path`, and `<it>/yay/config.json` is
    /// copied when present. `approved` maps pkgbase to the reviewed commit and
    /// PKGBUILD text.
    ///
    /// # Errors
    /// Fails on I/O errors or a malformed commit hash.
    pub fn create_pinned(
        user_config_dir: &Path,
        approved: &BTreeMap<String, ApprovedBase>,
    ) -> Result<Self, OverlayError> {
        for (pkgbase, entry) in approved {
            let ok = matches!(entry.commit.len(), 40 | 64)
                && entry
                    .commit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
            if !ok {
                return Err(OverlayError::BadCommit {
                    pkgbase: pkgbase.clone(),
                });
            }
        }
        let dir = tempfile::Builder::new()
            .prefix("aurdeck-overlay-")
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
        let user_dir_lit = if user_yay.is_dir() {
            lua_string_literal(&user_yay.to_string_lossy())
        } else {
            "nil".to_owned()
        };
        let user_cfg = user_yay.join("config.json");
        if user_cfg.is_file() {
            std::fs::copy(&user_cfg, yay_dir.join("config.json"))?;
        }

        let mut entries = String::new();
        for (pkgbase, entry) in approved {
            let text = match &entry.pkgbuild {
                Some(t) => format!("pkgbuild = {}", lua_text_literal(t)),
                None => "pkgbuild_unchecked = true".to_owned(),
            };
            let _ = writeln!(
                entries,
                "  [{}] = {{ commit = {}, {text} }},",
                lua_string_literal(pkgbase),
                lua_string_literal(&entry.commit),
            );
        }
        let init = fill(
            TEMPLATE,
            &[
                ("@@APPROVED@@", entries.trim_end()),
                ("@@USER_INIT@@", &user_init_lit),
                ("@@USER_DIR@@", &user_dir_lit),
            ],
        );
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

/// Replaces each placeholder, scanning the template once left to right, so
/// substituted text is never searched for further placeholders.
fn fill(template: &str, subs: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    'outer: while !rest.is_empty() {
        for (key, val) in subs {
            if let Some(tail) = rest.strip_prefix(key) {
                out.push_str(val);
                rest = tail;
                continue 'outer;
            }
        }
        let mut chars = rest.chars();
        if let Some(c) = chars.next() {
            out.push(c);
        }
        rest = chars.as_str();
    }
    out
}

/// Renders PKGBUILD text as a Lua long-bracket string whose level the text
/// cannot close. Lua drops a newline right after the opening bracket, so one
/// is always written there. Lua also turns CR LF and a lone CR into LF inside
/// long strings, and a NUL is fragile there: text with either is written as an
/// escaped quoted literal instead, which round-trips exactly.
pub(crate) fn lua_text_literal(s: &str) -> String {
    if s.contains(['\r', '\0']) {
        return lua_string_literal(s);
    }
    // One more `=` than the longest `]==..` run in the text.
    let bytes = s.as_bytes();
    let mut level = 1usize;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b']' {
            let n = bytes[i + 1..].iter().take_while(|&&c| c == b'=').count();
            level = level.max(n + 1);
        }
    }
    let eq = "=".repeat(level);
    format!("[{eq}[\n{s}]{eq}]")
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

    fn pinned(pkgbuild: &str) -> BTreeMap<String, ApprovedBase> {
        BTreeMap::from([(
            "pk\"g]]".to_owned(),
            ApprovedBase {
                commit: H1.to_owned(),
                pkgbuild: Some(pkgbuild.to_owned()),
            },
        )])
    }

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
    fn long_bracket_level_beats_the_text() {
        assert_eq!(lua_text_literal("plain"), "[=[\nplain]=]");
        let lit = lua_text_literal("a ]] b ]==] c");
        assert_eq!(lit, "[===[\na ]] b ]==] c]===]");
        // text ending in `]` and `=` runs cannot merge with the closer
        let lit = lua_text_literal("x]=");
        assert!(lit.starts_with("[==[\n") && lit.ends_with("]==]"));
        let lit = lua_text_literal("x]");
        assert_eq!(lit, "[=[\nx]]=]");
        // for every level used, the body never contains the closing sequence
        for text in ["]]", "]=]", "]==]", "]]]=]==]===]", "[[", "", "\n\n]"] {
            let lit = lua_text_literal(text);
            let level = lit.find('[').map(|_| lit[1..].find('[').unwrap()).unwrap();
            let closer = format!("]{}]", "=".repeat(level));
            let body = &lit[level + 3..lit.len() - level - 2];
            assert_eq!(body, text);
            assert!(!body.contains(&closer), "{text:?} -> {lit:?}");
        }
        // CR and NUL fall back to an escaped literal
        assert_eq!(lua_text_literal("a\r\nb"), r#""a\013\010b""#);
        assert_eq!(lua_text_literal("a\0b"), r#""a\000b""#);
    }

    #[test]
    fn creates_overlay_with_user_config() {
        let user = tempfile::tempdir().unwrap();
        std::fs::create_dir(user.path().join("yay")).unwrap();
        std::fs::write(user.path().join("yay/init.lua"), "-- mine").unwrap();
        std::fs::write(user.path().join("yay/config.json"), "{\"a\":1}").unwrap();
        let o = Overlay::create_pinned(user.path(), &pinned("pkgname=x\n]]\n]==]\n")).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        assert!(init.contains("pcall(dofile, user_init)"));
        let user_init = lua_string_literal(&user.path().join("yay/init.lua").to_string_lossy());
        assert!(init.contains(&format!("local user_init = {user_init}")));
        let user_dir = lua_string_literal(&user.path().join("yay").to_string_lossy());
        let path_line = init
            .lines()
            .find(|l| l.trim_start().starts_with("package.path ="))
            .unwrap();
        assert!(path_line.contains("user_dir"));
        assert!(init.contains(&format!("local user_dir = {user_dir}")));
        assert!(init.contains(&format!(
            "[\"pk\\\"g]]\"] = {{ commit = \"{H1}\", pkgbuild = [===[\npkgname=x\n]]\n]==]\n]===] }},"
        )));
        assert!(!init.contains("UNVERIFIED"));
        assert!(!init.contains("@@"));
        let cfg = std::fs::read_to_string(o.config_home().join("yay/config.json")).unwrap();
        assert_eq!(cfg, "{\"a\":1}");
        assert_eq!(o.env()[0].0, "XDG_CONFIG_HOME");
        let p = o.config_home().to_owned();
        drop(o);
        assert!(!p.exists());
    }

    #[test]
    fn generated_lua_uses_no_shell() {
        let user = tempfile::tempdir().unwrap();
        let o = Overlay::create_pinned(user.path(), &pinned("x")).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        for bad in [
            "io.popen",
            "os.execute",
            "require",
            "os.getenv",
            "loadstring",
        ] {
            assert!(!init.contains(bad), "{bad}");
        }
        assert!(init.contains("create_autocmd(\"AURPreInstall\""));
        assert!(init.contains("create_autocmd(\"AURPostDownload\""));
        assert!(!init.contains("yay.hook"));
        // the pin is registered after the user's init is loaded
        let load = init.find("pcall(dofile, user_init)").unwrap();
        let reg = init.find("create_autocmd(\"AURPreInstall\"").unwrap();
        assert!(load < reg);
    }

    #[test]
    fn placeholder_text_in_the_pkgbuild_is_not_expanded() {
        let user = tempfile::tempdir().unwrap();
        let o = Overlay::create_pinned(user.path(), &pinned("@@USER_INIT@@ @@USER_DIR@@")).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        assert!(init.contains("[=[\n@@USER_INIT@@ @@USER_DIR@@]=]"));
    }

    #[test]
    fn commit_only_constructor_marks_the_text_unchecked() {
        let user = tempfile::tempdir().unwrap();
        let approved = BTreeMap::from([("p".to_owned(), H1.to_owned())]);
        let o = Overlay::create(user.path(), &approved).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        assert!(init.contains(&format!(
            "[\"p\"] = {{ commit = \"{H1}\", pkgbuild_unchecked = true }},"
        )));
    }

    #[test]
    fn works_without_user_config() {
        let user = tempfile::tempdir().unwrap();
        let o = Overlay::create(user.path(), &BTreeMap::new()).unwrap();
        let init = std::fs::read_to_string(o.init_lua_path()).unwrap();
        assert!(init.contains("local user_init = nil"));
        assert!(init.contains("local user_dir = nil"));
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
        let mut full = pinned("x");
        full.get_mut("pk\"g]]").unwrap().commit = H1.to_uppercase();
        assert!(matches!(
            Overlay::create_pinned(user.path(), &full),
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
