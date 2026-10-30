use std::sync::Mutex;
use std::time::Duration;
use tokio::time::Instant;

/// Enforces a minimum interval between request starts. Callers reserve
/// consecutive slots, so concurrent callers are spaced out too.
pub(crate) struct RateLimiter {
    interval: Duration,
    next: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            next: Mutex::new(None),
        }
    }

    pub async fn acquire(&self) {
        if self.interval.is_zero() {
            return;
        }
        let slot = {
            let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
            let now = Instant::now();
            let slot = next.map_or(now, |n| n.max(now));
            *next = Some(slot + self.interval);
            slot
        };
        tokio::time::sleep_until(slot).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn spaces_calls_by_interval() {
        let rl = RateLimiter::new(Duration::from_secs(2));
        let start = Instant::now();
        rl.acquire().await;
        assert_eq!(start.elapsed(), Duration::ZERO);
        rl.acquire().await;
        rl.acquire().await;
        assert_eq!(start.elapsed(), Duration::from_secs(4));
    }

    #[tokio::test(start_paused = true)]
    async fn zero_interval_never_waits() {
        let rl = RateLimiter::new(Duration::ZERO);
        let start = Instant::now();
        for _ in 0..5 {
            rl.acquire().await;
        }
        assert_eq!(start.elapsed(), Duration::ZERO);
    }
}
