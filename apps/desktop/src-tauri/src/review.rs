//! Review DTO: what the review screen shows for one AUR package.

use serde::Serialize;
use yaygui_core::ReviewStatus;
use yaygui_review::{Approval, RepoFile};

use crate::plan::ReviewState;

impl From<&ReviewStatus> for ReviewState {
    fn from(s: &ReviewStatus) -> Self {
        match s {
            ReviewStatus::NotReviewed => Self::NotReviewed,
            ReviewStatus::Approved { .. } => Self::Approved,
            ReviewStatus::ChangedSinceReview => Self::ChangedSinceReview,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewBundle {
    pub name: String,
    pub pkgbase: String,
    pub version: String,
    /// Commit the files below were read from; approve exactly this one.
    pub commit: String,
    pub files: Vec<RepoFile>,
    pub truncated: bool,
    pub status: ReviewState,
    /// The latest earlier approval, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous: Option<Approval>,
    /// Unified diff from `previous.commit` to `commit`; `None` when there is
    /// no earlier approval, it is the same commit, or the old commit is gone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
    pub maintainer: Option<String>,
    pub submitter: Option<String>,
    pub url: Option<String>,
    pub votes: u32,
    pub popularity: f64,
    pub out_of_date: Option<i64>,
    pub first_submitted: i64,
    pub last_modified: i64,
}
