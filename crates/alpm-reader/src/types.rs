//! Plain-data package types.

use serde::{Deserialize, Serialize};

/// An optional dependency: package name plus the reason text, if any.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptDepend {
    pub name: String,
    pub reason: Option<String>,
}

/// A package from a sync (repository) database.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub url: Option<String>,
    pub licenses: Vec<String>,
    pub groups: Vec<String>,
    pub depends: Vec<String>,
    pub optdepends: Vec<OptDepend>,
    pub makedepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
    /// Installed size in bytes.
    pub installed_size: u64,
    /// Download (compressed) size in bytes.
    pub download_size: u64,
    /// Build date, Unix seconds.
    pub build_date: i64,
    pub packager: Option<String>,
}

/// Why a package was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Reason {
    Explicit,
    Dependency,
}

/// An installed package from the local database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub reason: Reason,
    /// Install date, Unix seconds.
    pub install_date: Option<i64>,
    /// Installed size in bytes.
    pub installed_size: u64,
    /// Not present in any sync database.
    pub foreign: bool,
    pub required_by: Vec<String>,
    pub optional_for: Vec<String>,
}

/// Selector for [`PackageDb::installed`](crate::PackageDb::installed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstalledFilter {
    /// Everything installed (`-Q`).
    All,
    /// Explicitly installed (`-Qe`).
    Explicit,
    /// Not in any sync DB (`-Qm`).
    Foreign,
    /// Unneeded dependencies (`-Qdt`).
    Orphans,
    /// Installed as dependencies (`-Qd`).
    Deps,
}
