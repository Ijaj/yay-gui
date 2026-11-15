//! Queue planning: resolves AUR dependencies and assembles the plan.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex};

use aurdeck_aur_client::{AurClient, Package as AurPackage};
use aurdeck_review::ReviewStore;
use aurdeck_yay_runner::pacman_lock_held;

use super::installed::InstalledService;
use super::lock;
use crate::installed::SharedDb;
use crate::plan::{
    self, Facts, MAX_AUR_PACKAGES, PlanErrorDto, PlanErrorKind, PlanOutcome, QueueItemDto,
};

pub struct PlanService {
    pub(crate) aur: AurClient,
    pub(crate) installed: Arc<InstalledService>,
    /// `Err` holds the reason the review store could not be opened.
    pub(crate) store: Arc<Mutex<Result<ReviewStore, String>>>,
}

impl PlanService {
    /// Plans the queue. Errors are reported inside the response.
    pub async fn plan(&self, items: Vec<QueueItemDto>, skip_review: bool) -> PlanOutcome {
        let queue = match plan::parse_items(items) {
            Ok(q) => q,
            Err(e) => return PlanOutcome::error(e),
        };
        let db = self.installed.db().await;

        // "repo" claims come from the frontend: confirm them, so an AUR package
        // can never be passed off as a repo package to dodge review.
        let repo_names = plan::installs(&queue, false);
        let missing: Vec<String> = repo_names
            .iter()
            .filter(|n| db.as_ref().is_none_or(|db| db.sync_package(n).is_none()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            return PlanOutcome::error(PlanErrorDto::new(
                PlanErrorKind::NotInRepos,
                format!("not in the sync databases: {}", missing.join(", ")),
                missing,
            ));
        }

        let (aur, dependency_of) = match self.resolve_aur(db.as_ref(), &queue, &repo_names).await {
            Ok(v) => v,
            Err(e) => return PlanOutcome::error(e),
        };
        let latest = |base: &str| match &*lock(&self.store) {
            Ok(s) => s.latest(base).cloned(),
            Err(_) => None,
        };
        plan::assemble(
            &queue,
            &Facts {
                aur: &aur,
                dependency_of: &dependency_of,
                latest: &latest,
                skip_review,
                lock_held: pacman_lock_held(Path::new("/")),
            },
        )
    }

    /// AUR info for queued AUR packages plus, transitively, their dependencies
    /// that neither the repos nor the system provide.
    async fn resolve_aur(
        &self,
        db: Option<&SharedDb>,
        queue: &[aurdeck_core::QueueItem],
        repo_names: &[String],
    ) -> Result<(HashMap<String, AurPackage>, HashMap<String, String>), PlanErrorDto> {
        let queued: HashSet<String> = plan::installs(queue, true).into_iter().collect();
        let mut aur: HashMap<String, AurPackage> = HashMap::new();
        let mut dependency_of: HashMap<String, String> = HashMap::new();
        let mut frontier: Vec<String> = queued.iter().cloned().collect();
        let satisfied = |d: &str| {
            repo_names.iter().any(|r| r == d)
                || db
                    .is_some_and(|db| db.sync_package(d).is_some() || db.local_package(d).is_some())
        };
        while !frontier.is_empty() {
            if aur.len() + frontier.len() > MAX_AUR_PACKAGES {
                return Err(PlanErrorDto::new(
                    PlanErrorKind::TooManyAurPackages,
                    format!("more than {MAX_AUR_PACKAGES} AUR packages would be built"),
                    Vec::new(),
                ));
            }
            let found = self.aur.info(&frontier).await.map_err(|e| {
                PlanErrorDto::new(PlanErrorKind::AurUnavailable, e.to_string(), Vec::new())
            })?;
            for p in found {
                aur.insert(p.name.clone(), p);
            }
            let mut next = Vec::new();
            for name in &frontier {
                let Some(p) = aur.get(name) else { continue };
                let root = dependency_of
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.clone());
                for d in plan::unresolved_deps(p, satisfied) {
                    if aur.contains_key(&d) || queued.contains(&d) || dependency_of.contains_key(&d)
                    {
                        continue;
                    }
                    dependency_of.insert(d.clone(), root.clone());
                    next.push(d);
                }
            }
            frontier = next;
        }
        // Names yay will not find in the AUR either (virtual provides) need no review.
        dependency_of.retain(|d, _| aur.contains_key(d));
        Ok((aur, dependency_of))
    }
}
