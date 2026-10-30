//! libalpm-backed [`PackageDb`] (feature `libalpm`).
//!
//! UNVERIFIED: this module cannot be built or tested in CI without Arch
//! Linux (libalpm); it is written against `alpm` 4.0.x and may need tweaks.

use crate::PackageDb;
use crate::types::{InstalledFilter, LocalPackage, OptDepend, Reason, RepoPackage};
use alpm::{Alpm, Package, PackageReason, SigLevel};
use std::cmp::Ordering;

/// [`PackageDb`] on top of a libalpm handle.
pub struct AlpmDb {
    handle: Alpm,
}

impl AlpmDb {
    /// Open `root` / `dbpath` (usually `/` and `/var/lib/pacman`) and register
    /// the given sync repos in order.
    pub fn open(root: &str, dbpath: &str, repos: &[String]) -> alpm::Result<Self> {
        let handle = Alpm::new(root, dbpath)?;
        for repo in repos {
            handle.register_syncdb(repo.as_str(), SigLevel::USE_DEFAULT)?;
        }
        Ok(Self { handle })
    }

    fn repo_pkg(pkg: &Package, repo: &str) -> RepoPackage {
        let strs = |l: alpm::AlpmList<&str>| l.iter().map(String::from).collect();
        let deps = |l: alpm::AlpmList<&alpm::Dep>| l.iter().map(|d| d.to_string()).collect();
        RepoPackage {
            name: pkg.name().into(),
            version: pkg.version().to_string(),
            description: pkg.desc().unwrap_or_default().into(),
            repo: repo.into(),
            url: pkg.url().map(Into::into),
            licenses: strs(pkg.licenses()),
            groups: strs(pkg.groups()),
            depends: deps(pkg.depends()),
            optdepends: pkg
                .optdepends()
                .iter()
                .map(|d| OptDepend {
                    name: d.name().into(),
                    reason: d.desc().map(Into::into),
                })
                .collect(),
            makedepends: deps(pkg.makedepends()),
            provides: deps(pkg.provides()),
            conflicts: deps(pkg.conflicts()),
            replaces: deps(pkg.replaces()),
            installed_size: pkg.isize().max(0) as u64,
            download_size: pkg.download_size().max(0) as u64,
            build_date: pkg.build_date(),
            packager: pkg.packager().map(Into::into),
        }
    }

    fn local_pkg(&self, pkg: &Package) -> LocalPackage {
        let foreign = !self
            .handle
            .syncdbs()
            .iter()
            .any(|db| db.pkg(pkg.name()).is_ok());
        LocalPackage {
            name: pkg.name().into(),
            version: pkg.version().to_string(),
            description: pkg.desc().unwrap_or_default().into(),
            reason: match pkg.reason() {
                PackageReason::Explicit => Reason::Explicit,
                PackageReason::Depend => Reason::Dependency,
            },
            install_date: pkg.install_date(),
            installed_size: pkg.isize().max(0) as u64,
            foreign,
            required_by: pkg.required_by().iter().map(|s| s.to_string()).collect(),
            optional_for: pkg.optional_for().iter().map(|s| s.to_string()).collect(),
        }
    }
}

impl PackageDb for AlpmDb {
    fn search(&self, query: &str) -> Vec<RepoPackage> {
        let terms: Vec<&str> = query.split_whitespace().collect();
        let mut out: Vec<RepoPackage> = Vec::new();
        for db in self.handle.syncdbs() {
            // libalpm ANDs the terms and matches name/description, case-insensitively.
            if let Ok(found) = db.search(terms.iter().copied()) {
                out.extend(found.iter().map(|p| Self::repo_pkg(p, db.name())));
            }
        }
        out
    }

    fn sync_package(&self, name: &str) -> Option<RepoPackage> {
        self.handle
            .syncdbs()
            .iter()
            .find_map(|db| db.pkg(name).ok().map(|p| Self::repo_pkg(p, db.name())))
    }

    fn local_package(&self, name: &str) -> Option<LocalPackage> {
        self.handle
            .localdb()
            .pkg(name)
            .ok()
            .map(|p| self.local_pkg(p))
    }

    fn installed(&self, filter: InstalledFilter) -> Vec<LocalPackage> {
        let mut out: Vec<LocalPackage> = self
            .handle
            .localdb()
            .pkgs()
            .iter()
            .map(|p| self.local_pkg(p))
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
            .collect();
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn required_by(&self, name: &str) -> Vec<String> {
        self.handle
            .localdb()
            .pkg(name)
            .map(|p| p.required_by().iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn vercmp(&self, a: &str, b: &str) -> Ordering {
        alpm::vercmp(a, b)
    }
}
