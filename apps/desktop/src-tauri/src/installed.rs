//! Installed-packages DTOs and the pacman database cache.
//!
//! Serialization matches `apps/desktop/src/api/types.ts` exactly.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use serde::Serialize;
use yaygui_alpm_reader::{
    FsConfig, FsDb, InstalledFilter, LocalPackage, PackageDb, Reason, repos_from_pacman_conf,
};

pub type SharedDb = Arc<dyn PackageDb + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    /// Sync repo that has this package; `null` for foreign (AUR or local) packages.
    pub repo: Option<String>,
    /// Installed explicitly rather than as a dependency.
    pub explicit: bool,
    /// A dependency nothing requires or optionally requires any more (`pacman -Qdt`).
    pub orphan: bool,
    /// Unix seconds.
    pub install_date: Option<i64>,
    /// Bytes.
    pub installed_size: u64,
    pub required_by: Vec<String>,
    pub optional_for: Vec<String>,
}

impl From<LocalPackage> for InstalledPackage {
    fn from(p: LocalPackage) -> Self {
        let explicit = p.reason == Reason::Explicit;
        Self {
            orphan: !explicit && p.required_by.is_empty() && p.optional_for.is_empty(),
            explicit,
            repo: if p.foreign { None } else { p.repo },
            name: p.name,
            version: p.version,
            description: p.description,
            install_date: p.install_date,
            installed_size: p.installed_size,
            required_by: p.required_by,
            optional_for: p.optional_for,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledResponse {
    pub packages: Vec<InstalledPackage>,
    /// Set when the pacman database could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn installed_response(db: Result<SharedDb, String>) -> InstalledResponse {
    match db {
        Ok(db) => InstalledResponse {
            packages: db
                .installed(InstalledFilter::All)
                .into_iter()
                .map(Into::into)
                .collect(),
            error: None,
        },
        Err(e) => InstalledResponse {
            packages: Vec::new(),
            error: Some(e),
        },
    }
}

/// Sync repos from pacman.conf text, falling back to the Arch defaults.
pub fn repos_or_default(pacman_conf: Option<&str>) -> Vec<String> {
    let repos = pacman_conf.map(repos_from_pacman_conf).unwrap_or_default();
    if repos.is_empty() {
        FsConfig::default().repos
    } else {
        repos
    }
}

/// Modification times of the files pacman rewrites on every change.
type Stamp = Vec<Option<SystemTime>>;

fn mtime(p: &Path) -> Option<SystemTime> {
    std::fs::metadata(p).and_then(|m| m.modified()).ok()
}

/// The pacman databases, reloaded when pacman has changed them.
///
/// Installing, upgrading or removing a package adds or removes a directory
/// in `local/`, and `-Sy` rewrites `sync/<repo>.db`; both change an mtime.
pub struct DbCache {
    config: FsConfig,
    cached: Mutex<Option<(Stamp, Result<SharedDb, String>)>>,
}

impl DbCache {
    pub fn new(config: FsConfig) -> Self {
        Self {
            config,
            cached: Mutex::new(None),
        }
    }

    fn stamp(&self) -> Stamp {
        let dbpath: &PathBuf = &self.config.dbpath;
        std::iter::once(dbpath.join("local"))
            .chain(
                self.config
                    .repos
                    .iter()
                    .map(|r| dbpath.join("sync").join(format!("{r}.db"))),
            )
            .map(|p| mtime(&p))
            .collect()
    }

    /// The current databases. Blocking: may re-read them from disk. `force`
    /// re-reads even when nothing seems to have changed (e.g. `pacman -D`
    /// rewrites a `desc` file in place).
    pub fn get(&self, force: bool) -> Result<SharedDb, String> {
        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let stamp = self.stamp();
        if let Some((s, db)) = &*cached
            && !force
            && *s == stamp
        {
            return db.clone();
        }
        let db = FsDb::open(&self.config)
            .map(|db| Arc::new(db) as SharedDb)
            .map_err(|e| format!("the pacman database could not be read: {e}"));
        if let Err(e) = &db {
            eprintln!("yaygui: {e}");
        }
        *cached = Some((stamp, db.clone()));
        db
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn local(name: &str) -> LocalPackage {
        LocalPackage {
            name: name.into(),
            version: "1.0-1".into(),
            description: "d".into(),
            reason: Reason::Explicit,
            install_date: Some(1_700_000_000),
            installed_size: 2048,
            foreign: false,
            repo: Some("extra".into()),
            required_by: Vec::new(),
            optional_for: Vec::new(),
        }
    }

    #[test]
    fn shape_matches_ts_contract() {
        let p: InstalledPackage = local("vim").into();
        assert_eq!(
            serde_json::to_value(&p).expect("serializes"),
            json!({
                "name": "vim", "version": "1.0-1", "description": "d",
                "repo": "extra", "explicit": true, "orphan": false,
                "installDate": 1_700_000_000, "installedSize": 2048,
                "requiredBy": [], "optionalFor": []
            })
        );
    }

    #[test]
    fn orphans_are_unneeded_dependencies() {
        let dep = |required: &[&str], optional: &[&str]| -> InstalledPackage {
            LocalPackage {
                reason: Reason::Dependency,
                required_by: required.iter().map(|s| (*s).into()).collect(),
                optional_for: optional.iter().map(|s| (*s).into()).collect(),
                ..local("lib")
            }
            .into()
        };
        assert!(dep(&[], &[]).orphan);
        assert!(!dep(&["vim"], &[]).orphan);
        assert!(!dep(&[], &["vim"]).orphan);
        // Explicit packages are never orphans.
        assert!(!InstalledPackage::from(local("vim")).orphan);
    }

    #[test]
    fn foreign_packages_have_no_repo() {
        let p: InstalledPackage = LocalPackage {
            foreign: true,
            repo: None,
            ..local("yay")
        }
        .into();
        assert_eq!(p.repo, None);
        let j = serde_json::to_value(&p).expect("serializes");
        assert_eq!(j["repo"], serde_json::Value::Null);
    }

    #[test]
    fn error_is_reported() {
        let r = installed_response(Err("boom".into()));
        assert_eq!(
            serde_json::to_value(&r).expect("serializes"),
            json!({"packages": [], "error": "boom"})
        );
    }

    #[test]
    fn repos_come_from_pacman_conf() {
        let conf = "[options]\n[core]\n[extra]\n[chaotic-aur]\n";
        assert_eq!(
            repos_or_default(Some(conf)),
            ["core", "extra", "chaotic-aur"]
        );
        assert_eq!(
            repos_or_default(Some("[options]\n")),
            FsConfig::default().repos
        );
        assert_eq!(repos_or_default(None), FsConfig::default().repos);
    }

    fn write_local(dbpath: &Path, name: &str) {
        let d = dbpath.join("local").join(format!("{name}-1.0-1"));
        std::fs::create_dir_all(&d).expect("mkdir");
        std::fs::write(
            d.join("desc"),
            format!("%NAME%\n{name}\n\n%VERSION%\n1.0-1\n\n"),
        )
        .expect("write desc");
    }

    fn set_mtime(p: &Path, secs: u64) {
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        std::fs::File::open(p)
            .and_then(|f| f.set_modified(t))
            .expect("set mtime");
    }

    #[test]
    fn cache_reloads_when_the_local_db_changes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let local_dir = tmp.path().join("local");
        write_local(tmp.path(), "vim");
        set_mtime(&local_dir, 1_000);
        let cache = DbCache::new(FsConfig {
            dbpath: tmp.path().into(),
            repos: vec!["core".into()],
        });
        let count = |force| {
            cache
                .get(force)
                .expect("db")
                .installed(InstalledFilter::All)
                .len()
        };
        assert_eq!(count(false), 1);

        // Same mtime: the cached copy is used until a forced reload.
        write_local(tmp.path(), "git");
        set_mtime(&local_dir, 1_000);
        assert_eq!(count(false), 1);
        assert_eq!(count(true), 2);

        // A changed mtime (what pacman causes) reloads by itself.
        write_local(tmp.path(), "nano");
        set_mtime(&local_dir, 2_000);
        assert_eq!(count(false), 3);
    }
}
