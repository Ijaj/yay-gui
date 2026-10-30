//! Shared application state: AUR client and the (optional) pacman DB.

use std::sync::Arc;

use yaygui_alpm_reader::{FsConfig, FsDb, PackageDb};
use yaygui_aur_client::{AurClient, AurConfig, SearchBy};

use crate::search::{SearchResponse, merge_results};

type SharedDb = Arc<dyn PackageDb + Send + Sync>;

pub struct AppState {
    aur: AurClient,
    /// `None` when the pacman databases cannot be read (e.g. not on Arch).
    db: Option<SharedDb>,
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
        Ok(Self { aur, db })
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
}
