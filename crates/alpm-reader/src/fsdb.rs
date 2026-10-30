//! Pure-Rust reader for the on-disk pacman databases.

use crate::PackageDb;
use crate::types::{InstalledFilter, LocalPackage, OptDepend, Reason, RepoPackage};
use regex::{Regex, RegexBuilder};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Errors while opening or parsing databases.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid database {path}: {msg}")]
    Invalid { path: PathBuf, msg: String },
}

/// Where to find the databases and which repos to read (in priority order).
#[derive(Debug, Clone)]
pub struct FsConfig {
    /// Pacman `DBPath`, default `/var/lib/pacman`.
    pub dbpath: PathBuf,
    /// Repo names, default `core`, `extra`, `multilib`.
    pub repos: Vec<String>,
}

impl Default for FsConfig {
    fn default() -> Self {
        Self {
            dbpath: PathBuf::from("/var/lib/pacman"),
            repos: ["core", "extra", "multilib"].map(String::from).to_vec(),
        }
    }
}

/// Extract `[repo]` section names (excluding `[options]`) from pacman.conf text.
pub fn repos_from_pacman_conf(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split('#').next().unwrap_or("").trim())
        .filter_map(|l| l.strip_prefix('[')?.strip_suffix(']'))
        .map(str::trim)
        .filter(|n| !n.is_empty() && *n != "options")
        .map(String::from)
        .collect()
}

/// Parsed `%FIELD%` blocks of a `desc` file.
type Desc = HashMap<String, Vec<String>>;

fn parse_desc(text: &str) -> Desc {
    let mut map = Desc::new();
    let mut cur: Option<&mut Vec<String>> = None;
    for line in text.lines() {
        let line = line.trim_end();
        if line.len() > 2 && line.starts_with('%') && line.ends_with('%') {
            cur = Some(map.entry(line[1..line.len() - 1].to_string()).or_default());
        } else if !line.is_empty()
            && let Some(v) = cur.as_mut()
        {
            v.push(line.to_string());
        }
    }
    map
}

fn first(d: &Desc, k: &str) -> Option<String> {
    d.get(k).and_then(|v| v.first()).cloned()
}
fn all(d: &Desc, k: &str) -> Vec<String> {
    d.get(k).cloned().unwrap_or_default()
}
fn num<T: std::str::FromStr + Default>(d: &Desc, k: &str) -> T {
    first(d, k).and_then(|s| s.parse().ok()).unwrap_or_default()
}

/// Dependency string without version constraint or description (`a>=1: why` -> `a`).
fn dep_name(s: &str) -> &str {
    let s = s.split(':').next().unwrap_or(s);
    s.split(['<', '>', '=']).next().unwrap_or(s).trim()
}

fn repo_package(d: &Desc, repo: &str) -> Option<RepoPackage> {
    Some(RepoPackage {
        name: first(d, "NAME")?,
        version: first(d, "VERSION")?,
        description: first(d, "DESC").unwrap_or_default(),
        repo: repo.to_string(),
        url: first(d, "URL"),
        licenses: all(d, "LICENSE"),
        groups: all(d, "GROUPS"),
        depends: all(d, "DEPENDS"),
        optdepends: all(d, "OPTDEPENDS")
            .iter()
            .map(|s| match s.split_once(':') {
                Some((n, r)) => OptDepend {
                    name: n.trim().to_string(),
                    reason: Some(r.trim().to_string()).filter(|r| !r.is_empty()),
                },
                None => OptDepend {
                    name: s.trim().to_string(),
                    reason: None,
                },
            })
            .collect(),
        makedepends: all(d, "MAKEDEPENDS"),
        provides: all(d, "PROVIDES"),
        conflicts: all(d, "CONFLICTS"),
        replaces: all(d, "REPLACES"),
        installed_size: num(d, "ISIZE"),
        download_size: num(d, "CSIZE"),
        build_date: num(d, "BUILDDATE"),
        packager: first(d, "PACKAGER"),
    })
}

/// Read `path` as a tar archive, auto-detecting gzip / zstd by magic bytes,
/// and return the text of every `<dir>/desc` entry.
fn read_sync_archive(path: &Path) -> Result<Vec<String>, DbError> {
    let io = |source| DbError::Io {
        path: path.to_path_buf(),
        source,
    };
    let bytes = std::fs::read(path).map_err(io)?;
    let reader: Box<dyn Read + '_> = match bytes.as_slice() {
        [0x1f, 0x8b, ..] => Box::new(flate2::read::MultiGzDecoder::new(bytes.as_slice())),
        [0x28, 0xb5, 0x2f, 0xfd, ..] => Box::new(
            ruzstd::decoding::StreamingDecoder::new(bytes.as_slice()).map_err(|e| {
                DbError::Invalid {
                    path: path.to_path_buf(),
                    msg: e.to_string(),
                }
            })?,
        ),
        // Uncompressed tar.
        _ => Box::new(bytes.as_slice()),
    };
    let mut out = Vec::new();
    let mut ar = tar::Archive::new(reader);
    for entry in ar.entries().map_err(io)? {
        let mut entry = entry.map_err(io)?;
        let is_desc = entry
            .path()
            .map_err(io)?
            .file_name()
            .is_some_and(|n| n == "desc");
        if is_desc {
            let mut s = String::new();
            entry.read_to_string(&mut s).map_err(io)?;
            out.push(s);
        }
    }
    Ok(out)
}

struct LocalEntry {
    pkg: LocalPackage,
    depends: Vec<String>,
    optdepends: Vec<String>,
    provides: Vec<String>,
}

/// Direct reader of `<dbpath>/local` and `<dbpath>/sync/<repo>.db`.
///
/// Databases are loaded into memory by [`FsDb::open`]; call it again to refresh.
pub struct FsDb {
    sync: Vec<RepoPackage>,
    local: Vec<LocalEntry>,
}

impl FsDb {
    /// Load the local DB and all configured sync DBs. Sync DBs that do not
    /// exist (repo never synced) are skipped.
    pub fn open(config: &FsConfig) -> Result<Self, DbError> {
        let mut sync = Vec::new();
        for repo in &config.repos {
            let path = config.dbpath.join("sync").join(format!("{repo}.db"));
            if !path.exists() {
                continue;
            }
            let mut pkgs: Vec<_> = read_sync_archive(&path)?
                .iter()
                .filter_map(|t| repo_package(&parse_desc(t), repo))
                .collect();
            pkgs.sort_by(|a, b| a.name.cmp(&b.name));
            sync.extend(pkgs);
        }
        let local = Self::read_local(&config.dbpath.join("local"), &sync)?;
        Ok(Self { sync, local })
    }

    fn read_local(dir: &Path, sync: &[RepoPackage]) -> Result<Vec<LocalEntry>, DbError> {
        let io = |path: &Path, source| DbError::Io {
            path: path.to_path_buf(),
            source,
        };
        let in_sync: HashSet<&str> = sync.iter().map(|p| p.name.as_str()).collect();
        let mut local = Vec::new();
        let rd = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(local),
            Err(e) => return Err(io(dir, e)),
        };
        for ent in rd {
            let ent = ent.map_err(|e| io(dir, e))?;
            let desc_path = ent.path().join("desc");
            let text = match std::fs::read_to_string(&desc_path) {
                Ok(t) => t,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io(&desc_path, e)),
            };
            let d = parse_desc(&text);
            let (Some(name), Some(version)) = (first(&d, "NAME"), first(&d, "VERSION")) else {
                continue;
            };
            let reason = if first(&d, "REASON").as_deref() == Some("1") {
                Reason::Dependency
            } else {
                Reason::Explicit
            };
            local.push(LocalEntry {
                pkg: LocalPackage {
                    foreign: !in_sync.contains(name.as_str()),
                    name,
                    version,
                    description: first(&d, "DESC").unwrap_or_default(),
                    reason,
                    install_date: first(&d, "INSTALLDATE").and_then(|s| s.parse().ok()),
                    installed_size: num(&d, "SIZE"),
                    required_by: Vec::new(),
                    optional_for: Vec::new(),
                },
                depends: all(&d, "DEPENDS")
                    .iter()
                    .map(|s| dep_name(s).to_string())
                    .collect(),
                optdepends: all(&d, "OPTDEPENDS")
                    .iter()
                    .map(|s| dep_name(s).to_string())
                    .collect(),
                provides: all(&d, "PROVIDES")
                    .iter()
                    .map(|s| dep_name(s).to_string())
                    .collect(),
            });
        }
        local.sort_by(|a, b| a.pkg.name.cmp(&b.pkg.name));
        // Reverse dependency maps (a dep may be satisfied via a provide).
        let mut req: HashMap<String, Vec<String>> = HashMap::new();
        let mut opt: HashMap<String, Vec<String>> = HashMap::new();
        let names: HashMap<&str, Vec<&str>> = local
            .iter()
            .flat_map(|e| {
                std::iter::once(e.pkg.name.as_str())
                    .chain(e.provides.iter().map(String::as_str))
                    .map(move |n| (n, e.pkg.name.as_str()))
            })
            .fold(HashMap::new(), |mut m, (k, v)| {
                m.entry(k).or_default().push(v);
                m
            });
        for e in &local {
            for (deps, out) in [(&e.depends, &mut req), (&e.optdepends, &mut opt)] {
                for d in deps {
                    for target in names.get(d.as_str()).into_iter().flatten() {
                        let v = out.entry((*target).to_string()).or_default();
                        if *target != e.pkg.name && !v.contains(&e.pkg.name) {
                            v.push(e.pkg.name.clone());
                        }
                    }
                }
            }
        }
        for e in &mut local {
            e.pkg.required_by = req.remove(&e.pkg.name).unwrap_or_default();
            e.pkg.optional_for = opt.remove(&e.pkg.name).unwrap_or_default();
        }
        Ok(local)
    }
}

fn term_regex(term: &str) -> Regex {
    let build = |s: &str| RegexBuilder::new(s).case_insensitive(true).build();
    build(term).unwrap_or_else(|_| build(&regex::escape(term)).expect("escaped regex is valid"))
}

impl PackageDb for FsDb {
    fn search(&self, query: &str) -> Vec<RepoPackage> {
        let terms: Vec<Regex> = query.split_whitespace().map(term_regex).collect();
        self.sync
            .iter()
            .filter(|p| {
                terms
                    .iter()
                    .all(|r| r.is_match(&p.name) || r.is_match(&p.description))
            })
            .cloned()
            .collect()
    }

    fn sync_package(&self, name: &str) -> Option<RepoPackage> {
        self.sync.iter().find(|p| p.name == name).cloned()
    }

    fn local_package(&self, name: &str) -> Option<LocalPackage> {
        self.local
            .iter()
            .find(|e| e.pkg.name == name)
            .map(|e| e.pkg.clone())
    }

    fn installed(&self, filter: InstalledFilter) -> Vec<LocalPackage> {
        self.local
            .iter()
            .map(|e| &e.pkg)
            .filter(|p| match filter {
                InstalledFilter::All => true,
                InstalledFilter::Explicit => p.reason == Reason::Explicit,
                InstalledFilter::Foreign => p.foreign,
                InstalledFilter::Deps => p.reason == Reason::Dependency,
                InstalledFilter::Orphans => {
                    p.reason == Reason::Dependency
                        && p.required_by.is_empty()
                        && p.optional_for.is_empty()
                }
            })
            .cloned()
            .collect()
    }

    fn required_by(&self, name: &str) -> Vec<String> {
        self.local_package(name)
            .map(|p| p.required_by)
            .unwrap_or_default()
    }
}
