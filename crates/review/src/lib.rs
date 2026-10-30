//! `yaygui-review`: everything needed to review an AUR package before it is
//! built.
//!
//! - [`AurGit`] clones or updates an AUR package's git repository into a
//!   private cache, lists its files and diffs two commits. Git is spawned with
//!   argument arrays and never runs repository hooks.
//! - [`ReviewStore`] is the persistent review history (pkgbase → approved
//!   commits), stored as JSON in the app's data directory.
//! - [`review_status`] turns a stored approval plus what is known about the
//!   package now into a [`yaygui_core::ReviewStatus`].

mod git;
mod store;

pub use git::{AUR_BASE_URL, AurGit, Fetched, RepoFile, ReviewError, is_commit_hash};
pub use store::{Approval, ReviewStore, StoreError, review_status};
