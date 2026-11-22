//! Installed packages and the shared pacman database cache.

use std::sync::Arc;

use crate::installed::{DbCache, InstalledResponse, SharedDb, installed_response};

pub struct InstalledService {
    /// The pacman databases, reloaded when pacman changes them.
    pub(crate) db: Arc<DbCache>,
}

impl InstalledService {
    /// The current pacman databases; `Err` when they cannot be read (e.g. not on Arch).
    pub(crate) async fn load_db(&self, force: bool) -> Result<SharedDb, String> {
        let cache = self.db.clone();
        tauri::async_runtime::spawn_blocking(move || cache.get(force))
            .await
            .unwrap_or_else(|e| Err(format!("internal error: {e}")))
    }

    pub(crate) async fn db(&self) -> Option<SharedDb> {
        self.load_db(false).await.ok()
    }

    /// Every installed package. `refresh` re-reads the database from disk.
    pub async fn installed(&self, refresh: bool) -> InstalledResponse {
        let db = self.load_db(refresh).await;
        tauri::async_runtime::spawn_blocking(move || installed_response(db))
            .await
            .unwrap_or_else(|e| InstalledResponse {
                error: Some(format!("internal error: {e}")),
                ..Default::default()
            })
    }
}
