//! Persistent review history.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use yaygui_core::ReviewStatus;

use crate::git::is_commit_hash;

const FORMAT_VERSION: u32 = 1;

/// Errors reading or writing the review store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Filesystem error.
    #[error("review store i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// The file exists but is not valid review-store JSON.
    #[error("review store {path} is corrupt: {source}")]
    Corrupt {
        /// File path.
        path: PathBuf,
        /// Parse error.
        source: serde_json::Error,
    },
    /// The commit is not a full lowercase hex hash.
    #[error("invalid commit hash {0:?}")]
    BadCommit(String),
}

/// One approval of a package base at a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    /// Approved git commit.
    pub commit: String,
    /// Unix seconds when the user approved it.
    pub approved_at: i64,
    /// AUR maintainer at the time, if known.
    pub maintainer: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StoreFile {
    version: u32,
    /// pkgbase -> approvals, oldest first.
    approvals: BTreeMap<String, Vec<Approval>>,
}

/// Review history backed by a JSON file. Every change is written atomically.
#[derive(Debug)]
pub struct ReviewStore {
    path: PathBuf,
    data: StoreFile,
}

impl ReviewStore {
    /// Opens `path`; a missing file is an empty store.
    ///
    /// # Errors
    /// Fails on I/O errors or a corrupt file (it is never silently replaced).
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StoreError> {
        let path = path.into();
        let data = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|source| StoreError::Corrupt {
                path: path.clone(),
                source,
            })?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => StoreFile {
                version: FORMAT_VERSION,
                ..StoreFile::default()
            },
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path, data })
    }

    /// Most recent approval of `pkgbase`.
    #[must_use]
    pub fn latest(&self, pkgbase: &str) -> Option<&Approval> {
        self.data.approvals.get(pkgbase).and_then(|v| v.last())
    }

    /// All approvals of `pkgbase`, oldest first.
    #[must_use]
    pub fn history(&self, pkgbase: &str) -> &[Approval] {
        self.data.approvals.get(pkgbase).map_or(&[], Vec::as_slice)
    }

    /// Records an approval and saves the store.
    ///
    /// # Errors
    /// Fails on a malformed commit hash or when the file cannot be written.
    pub fn approve(
        &mut self,
        pkgbase: &str,
        commit: &str,
        maintainer: Option<String>,
        now: i64,
    ) -> Result<(), StoreError> {
        if !is_commit_hash(commit) {
            return Err(StoreError::BadCommit(commit.to_owned()));
        }
        let list = self.data.approvals.entry(pkgbase.to_owned()).or_default();
        if list.last().is_some_and(|a| a.commit == commit) {
            list.pop(); // re-approving the same commit just refreshes it
        }
        list.push(Approval {
            commit: commit.to_owned(),
            approved_at: now,
            maintainer,
        });
        self.save()
    }

    fn save(&self) -> Result<(), StoreError> {
        let dir = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        let json = serde_json::to_vec_pretty(&self.data).map_err(std::io::Error::other)?;
        tmp.write_all(&json)?;
        tmp.as_file().sync_all()?;
        tmp.persist(&self.path).map_err(|e| e.error)?;
        Ok(())
    }
}

/// Review status from the latest approval and what is known about the package now.
///
/// With the fetched commit the answer is exact. Without it, an AUR
/// `LastModified` later than the approval means the package changed since.
/// The review-pinning hook re-checks the exact commit at build time.
#[must_use]
pub fn review_status(
    latest: Option<&Approval>,
    current_commit: Option<&str>,
    last_modified: Option<i64>,
) -> ReviewStatus {
    let Some(a) = latest else {
        return ReviewStatus::NotReviewed;
    };
    let changed = match current_commit {
        Some(c) => c != a.commit,
        None => last_modified.is_some_and(|m| m > a.approved_at),
    };
    if changed {
        ReviewStatus::ChangedSinceReview
    } else {
        ReviewStatus::Approved {
            commit: a.commit.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const C1: &str = "1111111111111111111111111111111111111111";
    const C2: &str = "2222222222222222222222222222222222222222";

    #[test]
    fn missing_file_is_empty_and_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/reviews.json");
        let mut s = ReviewStore::open(&path).unwrap();
        assert!(s.latest("foo").is_none());
        s.approve("foo", C1, Some("alice".into()), 100).unwrap();
        s.approve("foo", C2, None, 200).unwrap();
        s.approve("foo", C2, None, 300).unwrap();
        let s = ReviewStore::open(&path).unwrap();
        assert_eq!(s.history("foo").len(), 2);
        assert_eq!(s.latest("foo").unwrap().commit, C2);
        assert_eq!(s.latest("foo").unwrap().approved_at, 300);
        assert_eq!(s.history("foo")[0].maintainer.as_deref(), Some("alice"));
    }

    #[test]
    fn corrupt_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reviews.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(matches!(
            ReviewStore::open(&path),
            Err(StoreError::Corrupt { .. })
        ));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
    }

    #[test]
    fn rejects_bad_commit() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = ReviewStore::open(dir.path().join("r.json")).unwrap();
        assert!(matches!(
            s.approve("foo", "HEAD", None, 1),
            Err(StoreError::BadCommit(_))
        ));
    }

    #[test]
    fn status_rules() {
        let a = Approval {
            commit: C1.into(),
            approved_at: 100,
            maintainer: None,
        };
        let ok = ReviewStatus::Approved { commit: C1.into() };
        assert_eq!(
            review_status(None, Some(C1), None),
            ReviewStatus::NotReviewed
        );
        assert_eq!(review_status(Some(&a), Some(C1), Some(999)), ok);
        assert_eq!(
            review_status(Some(&a), Some(C2), None),
            ReviewStatus::ChangedSinceReview
        );
        assert_eq!(review_status(Some(&a), None, Some(50)), ok);
        assert_eq!(review_status(Some(&a), None, None), ok);
        assert_eq!(
            review_status(Some(&a), None, Some(101)),
            ReviewStatus::ChangedSinceReview
        );
    }
}
