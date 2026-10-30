use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tokio::time::Instant;

/// Minimal TTL cache. Uses `tokio::time::Instant` so tests can pause time.
pub(crate) struct TtlCache<V> {
    ttl: Duration,
    map: Mutex<HashMap<String, (Instant, V)>>,
}

impl<V: Clone> TtlCache<V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            map: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        if self.ttl.is_zero() {
            return None;
        }
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(key) {
            Some((at, v)) if at.elapsed() < self.ttl => Some(v.clone()),
            Some(_) => {
                map.remove(key);
                None
            }
            None => None,
        }
    }

    pub fn put(&self, key: String, value: V) {
        if self.ttl.is_zero() {
            return;
        }
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let ttl = self.ttl;
        map.retain(|_, (at, _)| at.elapsed() < ttl);
        map.insert(key, (Instant::now(), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn entries_expire_after_ttl() {
        let c = TtlCache::new(Duration::from_secs(60));
        c.put("k".into(), 1);
        tokio::time::advance(Duration::from_secs(59)).await;
        assert_eq!(c.get("k"), Some(1));
        tokio::time::advance(Duration::from_secs(2)).await;
        assert_eq!(c.get("k"), None);
    }

    #[tokio::test(start_paused = true)]
    async fn zero_ttl_disables() {
        let c = TtlCache::new(Duration::ZERO);
        c.put("k".into(), 1);
        assert_eq!(c.get("k"), None);
    }
}
