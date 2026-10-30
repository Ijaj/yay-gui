//! Shared application state: AUR client, the (optional) pacman DB, the review
//! store and the running transaction.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::ipc::Channel;
use yaygui_alpm_reader::{FsConfig, FsDb, PackageDb};
use yaygui_aur_client::{AurClient, AurConfig, Package as AurPackage, SearchBy};
use yaygui_core::{PackageName, TransactionState};
use yaygui_review::{AurGit, ReviewStore, review_status};
use yaygui_yay_runner::{
    Overlay, TxConfig, TxEvent, TxHandle, pacman_lock_held, start_transaction,
};
use zeroize::Zeroizing;

use crate::plan::{
    self, Facts, MAX_AUR_PACKAGES, PlanErrorDto, PlanErrorKind, PlanOutcome, QueueItemDto,
};
use crate::review::ReviewBundle;
use crate::search::{SearchResponse, merge_results};
use crate::tx::{self, TxEventDto, TxRecord, Utf8Decoder};

type SharedDb = Arc<dyn PackageDb + Send + Sync>;

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `$XDG_<KIND>_HOME`, else `$HOME/<fallback>`.
fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
                .join(fallback)
        })
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub struct AppState {
    aur: AurClient,
    /// `None` when the pacman databases cannot be read (e.g. not on Arch).
    db: Option<SharedDb>,
    git: AurGit,
    /// `Err` holds the reason the review store could not be opened.
    store: Mutex<Result<ReviewStore, String>>,
    /// pkgbase -> commit last shown on the review screen (only that one can be approved).
    shown: Mutex<HashMap<String, (String, Option<String>)>>,
    tx: Mutex<Option<Arc<TxHandle>>>,
    starting: tokio::sync::Mutex<()>,
    config_dir: PathBuf,
    record_path: PathBuf,
    /// Debug builds only: run this instead of `yay` (`YAYGUI_YAY`).
    yay_override: Option<String>,
}

impl AppState {
    pub fn new() -> Result<Self, yaygui_aur_client::Error> {
        let aur = AurClient::new(AurConfig::default())?;
        let db: Option<SharedDb> = match FsDb::open(&FsConfig::default()) {
            Ok(db) => Some(Arc::new(db)),
            Err(e) => {
                eprintln!("yaygui: pacman database unavailable ({e}); repo results will be empty");
                None
            }
        };
        let data_dir = xdg_dir("XDG_DATA_HOME", ".local/share").join("yaygui");
        let cache_dir = xdg_dir("XDG_CACHE_HOME", ".cache").join("yaygui");
        let store = ReviewStore::open(data_dir.join("reviews.json")).map_err(|e| e.to_string());
        if let Err(e) = &store {
            eprintln!("yaygui: {e}; reviews cannot be recorded until it is fixed");
        }
        let yay_override = if cfg!(debug_assertions) {
            std::env::var("YAYGUI_YAY").ok()
        } else {
            None
        };
        Ok(Self {
            aur,
            db,
            git: AurGit::new(cache_dir.join("aur")),
            store: Mutex::new(store),
            shown: Mutex::default(),
            tx: Mutex::default(),
            starting: tokio::sync::Mutex::new(()),
            config_dir: xdg_dir("XDG_CONFIG_HOME", ".config"),
            record_path: data_dir.join("last-transaction.json"),
            yay_override,
        })
    }

    /// Search repos and the AUR concurrently. Never fails: AUR problems are
    /// reported in `aur_error`.
    pub async fn search(&self, query: &str) -> SearchResponse {
        let query = query.trim().to_owned();
        if query.is_empty() {
            return SearchResponse::default();
        }

        let repo_task = {
            let db = self.db.clone();
            let q = query.clone();
            tauri::async_runtime::spawn_blocking(move || {
                db.map(|db| db.search(&q)).unwrap_or_default()
            })
        };
        let aur_fut = self.aur.search(&query, SearchBy::NameDesc);
        let (repo, aur) = tokio::join!(repo_task, aur_fut);
        let repo = repo.unwrap_or_default();
        let aur = aur.map_err(|e| e.to_string());

        let db = self.db.clone();
        let merged = tauri::async_runtime::spawn_blocking(move || {
            merge_results(repo, aur, |name| {
                db.as_ref()
                    .and_then(|db| db.local_package(name))
                    .map(|p| p.version)
            })
        })
        .await;
        merged.unwrap_or_else(|e| SearchResponse {
            aur_error: Some(format!("internal error: {e}")),
            ..Default::default()
        })
    }

    /// Plans the queue. Errors are reported inside the response.
    pub async fn plan(&self, items: Vec<QueueItemDto>, skip_review: bool) -> PlanOutcome {
        let queue = match plan::parse_items(items) {
            Ok(q) => q,
            Err(e) => return PlanOutcome::error(e),
        };

        // "repo" claims come from the frontend: confirm them, so an AUR package
        // can never be passed off as a repo package to dodge review.
        let repo_names = plan::installs(&queue, false);
        let missing: Vec<String> = repo_names
            .iter()
            .filter(|n| {
                self.db
                    .as_ref()
                    .is_none_or(|db| db.sync_package(n).is_none())
            })
            .cloned()
            .collect();
        if !missing.is_empty() {
            return PlanOutcome::error(PlanErrorDto::new(
                PlanErrorKind::NotInRepos,
                format!("not in the sync databases: {}", missing.join(", ")),
                missing,
            ));
        }

        let (aur, dependency_of) = match self.resolve_aur(&queue, &repo_names).await {
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
        queue: &[yaygui_core::QueueItem],
        repo_names: &[String],
    ) -> Result<(HashMap<String, AurPackage>, HashMap<String, String>), PlanErrorDto> {
        let queued: HashSet<String> = plan::installs(queue, true).into_iter().collect();
        let mut aur: HashMap<String, AurPackage> = HashMap::new();
        let mut dependency_of: HashMap<String, String> = HashMap::new();
        let mut frontier: Vec<String> = queued.iter().cloned().collect();
        let satisfied = |d: &str| {
            repo_names.iter().any(|r| r == d)
                || self
                    .db
                    .as_ref()
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

    fn current_tx(&self) -> Result<Arc<TxHandle>, String> {
        lock(&self.tx)
            .clone()
            .filter(|t| !t.is_finished())
            .ok_or_else(|| "no transaction is running".to_owned())
    }

    /// Re-plans the queue (the frontend's plan is never trusted) and runs it.
    pub async fn start(
        &self,
        items: Vec<QueueItemDto>,
        skip_review: bool,
        size: (u16, u16),
        channel: Channel<TxEventDto>,
    ) -> Result<(), String> {
        let _guard = self.starting.lock().await;
        if self.current_tx().is_ok() {
            return Err("a transaction is already running".into());
        }
        let outcome = self.plan(items, skip_review).await;
        if !outcome.response.can_run {
            return Err(outcome.response.error.map_or_else(
                || {
                    if outcome.response.lock_held {
                        "the pacman database is locked by another program".to_owned()
                    } else {
                        "nothing to do".to_owned()
                    }
                },
                |e| e.message,
            ));
        }

        // The overlay lives inside the event sink, so it is removed when the run ends.
        let overlay = if outcome.approved.is_empty() {
            None
        } else {
            Some(Overlay::create(&self.config_dir, &outcome.approved).map_err(|e| e.to_string())?)
        };
        let config = TxConfig {
            program_override: self.yay_override.clone(),
            env: overlay.as_ref().map(Overlay::env).unwrap_or_default(),
            size,
        };

        let mut record = TxRecord {
            state: TransactionState::Approved,
            commands: outcome.response.commands.iter().map(Into::into).collect(),
            started_at: now(),
            updated_at: now(),
            interrupted: false,
        };
        let record_path = self.record_path.clone();
        let mut decoder = Utf8Decoder::default();
        let handle = start_transaction(outcome.commands, config, move |ev| {
            let _keep_alive = &overlay;
            if let TxEvent::State(s) = &ev {
                record.state = s.clone();
                record.updated_at = now();
                if let Err(e) = tx::save_record(&record_path, &record) {
                    eprintln!("yaygui: could not save transaction record: {e}");
                }
            }
            if let Some(dto) = tx::to_dto(ev, &mut decoder) {
                let _ = channel.send(dto);
            }
        });
        *lock(&self.tx) = Some(Arc::new(handle));
        Ok(())
    }

    pub fn answer_password(&self, password: Zeroizing<String>) -> Result<(), String> {
        self.current_tx()?
            .answer_password(password)
            .map_err(|e| e.to_string())
    }

    pub fn answer_yes_no(&self, yes: bool) -> Result<(), String> {
        self.current_tx()?
            .answer_yes_no(yes)
            .map_err(|e| e.to_string())
    }

    pub fn send_input(&self, text: &str) -> Result<(), String> {
        self.current_tx()?
            .write_input(text.as_bytes())
            .map_err(|e| e.to_string())
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        match self.current_tx() {
            Ok(t) => t.resize(rows, cols).map_err(|e| e.to_string()),
            Err(_) => Ok(()),
        }
    }

    pub fn cancel(&self) -> Result<(), String> {
        self.current_tx()?.cancel();
        Ok(())
    }

    pub fn last_transaction(&self) -> Option<TxRecord> {
        tx::load_record(&self.record_path, self.current_tx().is_ok())
    }
}
