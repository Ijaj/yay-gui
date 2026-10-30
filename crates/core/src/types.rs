//! Core domain types.

use crate::name::PackageName;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Where a package comes from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A sync repository such as `core`, `extra` or `multilib`.
    Repo(String),
    /// The Arch User Repository.
    Aur,
    /// Installed locally with no known origin.
    Local,
}

/// A package together with its source.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PackageRef {
    /// Validated package name.
    pub name: PackageName,
    /// Where it comes from.
    pub source: Source,
}

/// How aggressively to remove a package.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoveMode {
    /// `-R`
    Plain,
    /// `-Rs`: also unneeded dependencies.
    WithDeps,
    /// `-Rns`: also dependencies and config files.
    WithDepsAndConfig,
}

/// An action queued by the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueueItem {
    /// Install (or update) a package.
    Install(PackageRef),
    /// Remove a package.
    Remove {
        /// Package to remove.
        name: PackageName,
        /// Removal mode.
        mode: RemoveMode,
    },
    /// Full system upgrade (`-Syu`).
    SystemUpgrade {
        /// Also rebuild VCS (`-git` etc.) packages (`--devel`).
        devel: bool,
    },
    /// Remove orphaned packages (`yay -Yc`).
    CleanOrphans,
}

/// Review state of an AUR package's PKGBUILD.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReviewStatus {
    /// Never reviewed.
    NotReviewed,
    /// Reviewed and approved at this git commit.
    Approved {
        /// Approved commit hash.
        commit: String,
    },
    /// Was approved, but the PKGBUILD repo changed since.
    ChangedSinceReview,
}

/// Review status per package; missing entries mean [`ReviewStatus::NotReviewed`].
pub type ReviewMap = HashMap<PackageName, ReviewStatus>;
