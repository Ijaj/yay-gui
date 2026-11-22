//! Package search across the pacman databases and the AUR.

use std::sync::Arc;

use aurdeck_aur_client::{AurClient, SearchBy};

use super::installed::InstalledService;
use crate::search::{SearchResponse, merge_results};

pub struct SearchService {
    pub(crate) aur: AurClient,
    pub(crate) installed: Arc<InstalledService>,
}

impl SearchService {
    /// Search repos and the AUR concurrently. Never fails: AUR problems are
    /// reported in `aur_error`.
    pub async fn search(&self, query: &str) -> SearchResponse {
        let query = query.trim().to_owned();
        if query.is_empty() {
            return SearchResponse::default();
        }

        let db = self.installed.db().await;
        let repo_task = {
            let db = db.clone();
            let q = query.clone();
            tauri::async_runtime::spawn_blocking(move || {
                db.map(|db| db.search(&q)).unwrap_or_default()
            })
        };
        let aur_fut = self.aur.search(&query, SearchBy::NameDesc);
        let (repo, aur) = tokio::join!(repo_task, aur_fut);
        let repo = repo.unwrap_or_default();
        let aur = aur.map_err(|e| e.to_string());

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
