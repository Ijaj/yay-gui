//! Read-only pacman package database queries for YayGUI.
//!
//! Reads never go through `yay`. [`PackageDb`] is a backend-agnostic trait;
//! [`FsDb`] implements it in pure Rust by parsing the on-disk pacman databases,
//! and `AlpmDb` (feature `libalpm`) implements it via libalpm. [`vercmp`] is a
//! pure-Rust port of `alpm_pkg_vercmp`.

mod fsdb;
mod types;
mod version;

#[cfg(feature = "libalpm")]
mod alpmdb;

#[cfg(feature = "libalpm")]
pub use alpmdb::AlpmDb;
pub use fsdb::{DbError, FsConfig, FsDb, repos_from_pacman_conf};
pub use types::{InstalledFilter, LocalPackage, OptDepend, Reason, RepoPackage};
pub use version::vercmp;

use std::cmp::Ordering;

/// Backend-agnostic, synchronous, read-only view of the pacman databases.
pub trait PackageDb {
    /// Search sync DBs like `pacman -Ss`: whitespace-separated terms (regex,
    /// case-insensitive) that must all match the name or description.
    fn search(&self, query: &str) -> Vec<RepoPackage>;
    /// Look up a package in the sync DBs (first repo in order wins).
    fn sync_package(&self, name: &str) -> Option<RepoPackage>;
    /// Look up an installed package.
    fn local_package(&self, name: &str) -> Option<LocalPackage>;
    /// List installed packages matching `filter`, sorted by name.
    fn installed(&self, filter: InstalledFilter) -> Vec<LocalPackage>;
    /// Names of installed packages that depend on `name` (reverse deps).
    fn required_by(&self, name: &str) -> Vec<String>;
    /// Compare two versions with libalpm semantics.
    fn vercmp(&self, a: &str, b: &str) -> Ordering {
        vercmp(a, b)
    }
}
