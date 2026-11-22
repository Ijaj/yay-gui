//! Shared application state: one service per feature (see `services/`).

use std::sync::{Arc, Mutex};

use aurdeck_alpm_reader::FsConfig;
use aurdeck_aur_client::{AurClient, AurConfig};
use aurdeck_review::{AurGit, ReviewStore};

use crate::installed::{DbCache, repos_or_default};
use crate::services::installed::InstalledService;
use crate::services::plan::PlanService;
use crate::services::reviews::ReviewService;
use crate::services::search::SearchService;
use crate::services::transactions::TransactionService;
use crate::services::xdg_dir;

pub struct AppState {
    pub(crate) installed: Arc<InstalledService>,
    pub(crate) search: SearchService,
    pub(crate) plan: Arc<PlanService>,
    pub(crate) reviews: ReviewService,
    pub(crate) transactions: TransactionService,
}

impl AppState {
    pub fn new() -> Result<Self, aurdeck_aur_client::Error> {
        let aur = AurClient::new(AurConfig::default())?;
        let pacman_conf = std::fs::read_to_string("/etc/pacman.conf").ok();
        let db = Arc::new(DbCache::new(FsConfig {
            repos: repos_or_default(pacman_conf.as_deref()),
            ..FsConfig::default()
        }));
        let data_dir = xdg_dir("XDG_DATA_HOME", ".local/share").join("aurdeck");
        let cache_dir = xdg_dir("XDG_CACHE_HOME", ".cache").join("aurdeck");
        let store = ReviewStore::open(data_dir.join("reviews.json")).map_err(|e| e.to_string());
        if let Err(e) = &store {
            eprintln!("aurdeck: {e}; reviews cannot be recorded until it is fixed");
        }
        let yay_override = if cfg!(debug_assertions) {
            std::env::var("AURDECK_YAY").ok()
        } else {
            None
        };
        let store = Arc::new(Mutex::new(store));
        let installed = Arc::new(InstalledService { db });
        let plan = Arc::new(PlanService {
            aur: aur.clone(),
            installed: installed.clone(),
            store: store.clone(),
        });
        Ok(Self {
            search: SearchService {
                aur: aur.clone(),
                installed: installed.clone(),
            },
            reviews: ReviewService {
                aur,
                git: AurGit::new(cache_dir.join("aur")),
                store,
                shown: Mutex::default(),
            },
            transactions: TransactionService {
                plan: plan.clone(),
                tx: Mutex::default(),
                starting: tokio::sync::Mutex::new(()),
                config_dir: xdg_dir("XDG_CONFIG_HOME", ".config"),
                record_path: data_dir.join("last-transaction.json"),
                yay_override,
            },
            installed,
            plan,
        })
    }
}
