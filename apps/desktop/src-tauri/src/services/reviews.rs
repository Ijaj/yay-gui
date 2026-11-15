//! Review screen: fetches the AUR repository and records approvals.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use aurdeck_aur_client::AurClient;
use aurdeck_core::PackageName;
use aurdeck_review::{AurGit, ReviewStore, review_status};

use super::{lock, now};
use crate::review::ReviewBundle;

pub struct ReviewService {
    pub(crate) aur: AurClient,
    pub(crate) git: AurGit,
    /// `Err` holds the reason the review store could not be opened.
    pub(crate) store: Arc<Mutex<Result<ReviewStore, String>>>,
    /// pkgbase -> commit last shown on the review screen (only that one can be approved).
    pub(crate) shown: Mutex<HashMap<String, (String, Option<String>)>>,
}

impl ReviewService {
    /// Fetches the package repository and prepares the review screen.
    pub async fn review(&self, name: String) -> Result<ReviewBundle, String> {
        let name: PackageName = name.parse().map_err(|e| format!("{e}"))?;
        let pkg = self
            .aur
            .info(&[name.as_str()])
            .await
            .map_err(|e| e.to_string())?
            .into_iter()
            .find(|p| p.name == name.as_str())
            .ok_or_else(|| format!("{} is not in the AUR", name.as_str()))?;
        let base: PackageName = pkg.package_base.parse().map_err(|e| format!("{e}"))?;

        let git = self.git.clone();
        let b = base.clone();
        let fetched = tauri::async_runtime::spawn_blocking(move || git.fetch(&b))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

        let previous = match &*lock(&self.store) {
            Ok(s) => s.latest(base.as_str()).cloned(),
            Err(e) => return Err(e.clone()),
        };
        let diff = match &previous {
            Some(p) if p.commit != fetched.commit => {
                let (git, b, from, to) = (
                    self.git.clone(),
                    base.clone(),
                    p.commit.clone(),
                    fetched.commit.clone(),
                );
                tauri::async_runtime::spawn_blocking(move || git.diff(&b, &from, &to))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?
            }
            _ => None,
        };
        let status = review_status(previous.as_ref(), Some(&fetched.commit), None);
        lock(&self.shown).insert(
            fetched.pkgbase.clone(),
            (fetched.commit.clone(), pkg.maintainer.clone()),
        );
        Ok(ReviewBundle {
            name: pkg.name,
            pkgbase: fetched.pkgbase,
            version: pkg.version,
            commit: fetched.commit,
            files: fetched.files,
            truncated: fetched.truncated,
            status: (&status).into(),
            previous,
            diff,
            maintainer: pkg.maintainer,
            submitter: pkg.submitter,
            url: pkg.url,
            votes: pkg.num_votes,
            popularity: pkg.popularity,
            out_of_date: pkg.out_of_date,
            first_submitted: pkg.first_submitted,
            last_modified: pkg.last_modified,
        })
    }

    /// Records approval of `commit`, which must be the one the review screen showed.
    pub fn approve(&self, pkgbase: &str, commit: &str) -> Result<(), String> {
        let maintainer = match lock(&self.shown).get(pkgbase) {
            Some((c, m)) if c == commit => m.clone(),
            _ => return Err("only the commit shown on the review screen can be approved".into()),
        };
        match &mut *lock(&self.store) {
            Ok(s) => s
                .approve(pkgbase, commit, maintainer, now())
                .map_err(|e| e.to_string()),
            Err(e) => Err(e.clone()),
        }
    }
}
