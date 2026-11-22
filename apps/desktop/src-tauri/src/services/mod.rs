//! Per-feature backend services. `AppState` owns one of each.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub mod installed;
pub mod plan;
pub mod reviews;
pub mod search;
pub mod transactions;

pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `$XDG_<KIND>_HOME`, else `$HOME/<fallback>`.
pub(crate) fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
                .join(fallback)
        })
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
