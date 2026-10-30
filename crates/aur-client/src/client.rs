use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::TtlCache;
use crate::error::Error;
use crate::models::{Envelope, Package};
use crate::rate_limit::RateLimiter;

/// Maximum total URL length for an info request.
const MAX_URL_LEN: usize = 4400;
/// Maximum package names per info request.
const MAX_NAMES_PER_REQUEST: usize = 200;

/// Field matched by a search (`by=` parameter of the RPC).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchBy {
    /// Package name only.
    Name,
    /// Name and description (RPC default).
    NameDesc,
    /// Maintainer name.
    Maintainer,
    /// Dependencies.
    Depends,
    /// Build dependencies.
    MakeDepends,
    /// Optional dependencies.
    OptDepends,
    /// Check dependencies.
    CheckDepends,
    /// Provides.
    Provides,
    /// Conflicts.
    Conflicts,
    /// Replaces.
    Replaces,
    /// Keywords.
    Keywords,
    /// Groups.
    Groups,
    /// Submitter name.
    Submitter,
    /// Co-maintainer name.
    Comaintainers,
}

impl SearchBy {
    /// The value used in the `by` query parameter.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::NameDesc => "name-desc",
            Self::Maintainer => "maintainer",
            Self::Depends => "depends",
            Self::MakeDepends => "makedepends",
            Self::OptDepends => "optdepends",
            Self::CheckDepends => "checkdepends",
            Self::Provides => "provides",
            Self::Conflicts => "conflicts",
            Self::Replaces => "replaces",
            Self::Keywords => "keywords",
            Self::Groups => "groups",
            Self::Submitter => "submitter",
            Self::Comaintainers => "comaintainers",
        }
    }
}

/// Client configuration.
#[derive(Debug, Clone)]
pub struct AurConfig {
    /// Base URL without trailing path (default `https://aur.archlinux.org`).
    pub base_url: String,
    /// `User-Agent` header (default `yaygui/<version>`).
    pub user_agent: String,
    /// Per-request timeout (default 15 s).
    pub timeout: Duration,
    /// How long cached results stay valid; zero disables caching (default 5 min).
    pub cache_ttl: Duration,
    /// Minimum spacing between requests; zero disables limiting (default 1 s,
    /// i.e. at most 3600 req/h, well under the 4000 req/day-per-IP budget in
    /// practice because the cache absorbs repeats).
    pub min_request_interval: Duration,
}

impl Default for AurConfig {
    fn default() -> Self {
        Self {
            base_url: "https://aur.archlinux.org".to_owned(),
            user_agent: concat!("yaygui/", env!("CARGO_PKG_VERSION")).to_owned(),
            timeout: Duration::from_secs(15),
            cache_ttl: Duration::from_secs(300),
            min_request_interval: Duration::from_secs(1),
        }
    }
}

struct Inner {
    http: reqwest::Client,
    base: String,
    limiter: RateLimiter,
    info_cache: TtlCache<Package>,
    search_cache: TtlCache<Vec<Package>>,
}

/// AUR RPC v5 client. Cheap to clone; clones share cache and rate limiter.
#[derive(Clone)]
pub struct AurClient {
    inner: Arc<Inner>,
}

impl AurClient {
    /// Builds a client from `config`.
    pub fn new(config: AurConfig) -> Result<Self, Error> {
        let base = config.base_url.trim_end_matches('/').to_owned();
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(Error::InvalidUrl(config.base_url));
        }
        let http = reqwest::Client::builder()
            .user_agent(config.user_agent)
            .timeout(config.timeout)
            .build()?;
        Ok(Self {
            inner: Arc::new(Inner {
                http,
                base,
                limiter: RateLimiter::new(config.min_request_interval),
                info_cache: TtlCache::new(config.cache_ttl),
                search_cache: TtlCache::new(config.cache_ttl),
            }),
        })
    }

    /// Searches packages. Queries under 2 characters are rejected locally.
    pub async fn search(&self, query: &str, by: SearchBy) -> Result<Vec<Package>, Error> {
        if query.chars().count() < 2 {
            return Err(Error::QueryTooShort);
        }
        let key = format!("{}\0{query}", by.as_str());
        if let Some(hit) = self.inner.search_cache.get(&key) {
            return Ok(hit);
        }
        let url = format!(
            "{}/rpc/v5/search/{}?by={}",
            self.inner.base,
            percent_encode(query),
            by.as_str()
        );
        let results = self.fetch(&url).await?;
        self.inner.search_cache.put(key, results.clone());
        Ok(results)
    }

    /// Fetches full details for `names`. Duplicates are removed, requests are
    /// batched to stay under the URL length limit, and cached names are not
    /// re-fetched. Unknown packages are simply absent from the result, which
    /// follows the order of `names`.
    pub async fn info<S: AsRef<str>>(&self, names: &[S]) -> Result<Vec<Package>, Error> {
        let mut seen = HashSet::new();
        let ordered: Vec<&str> = names
            .iter()
            .map(AsRef::as_ref)
            .filter(|n| !n.is_empty() && seen.insert(*n))
            .collect();

        let mut found: HashMap<String, Package> = HashMap::new();
        let mut missing: Vec<&str> = Vec::new();
        for name in &ordered {
            match self.inner.info_cache.get(name) {
                Some(p) => {
                    found.insert((*name).to_owned(), p);
                }
                None => missing.push(name),
            }
        }

        let prefix = format!("{}/rpc/v5/info?", self.inner.base);
        for batch in batch_names(&missing, prefix.len()) {
            let mut url = prefix.clone();
            for (i, name) in batch.iter().enumerate() {
                if i > 0 {
                    url.push('&');
                }
                let _ = write!(url, "arg[]={}", percent_encode(name));
            }
            for pkg in self.fetch(&url).await? {
                self.inner.info_cache.put(pkg.name.clone(), pkg.clone());
                found.insert(pkg.name.clone(), pkg);
            }
        }

        Ok(ordered.iter().filter_map(|n| found.remove(*n)).collect())
    }

    async fn fetch(&self, url: &str) -> Result<Vec<Package>, Error> {
        self.inner.limiter.acquire().await;
        let resp = self.inner.http.get(url).send().await?;
        let status = resp.status();
        if status.as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.trim().parse::<u64>().ok())
                .map(Duration::from_secs);
            return Err(Error::RateLimited { retry_after });
        }
        if !status.is_success() {
            return Err(Error::Status(status.as_u16()));
        }
        let env: Envelope = resp.json().await?;
        if env.kind == "error" {
            return Err(Error::Rpc(
                env.error.unwrap_or_else(|| "unknown error".to_owned()),
            ));
        }
        Ok(env.results)
    }
}

/// Percent-encodes everything except RFC 3986 unreserved characters.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// Splits names into batches so each URL (prefix + `arg[]=name` joined by
/// `&`) stays within [`MAX_URL_LEN`] and [`MAX_NAMES_PER_REQUEST`].
fn batch_names<'a>(names: &[&'a str], prefix_len: usize) -> Vec<Vec<&'a str>> {
    let mut batches: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut len = prefix_len;
    for &name in names {
        // "arg[]=" (6) + encoded name + "&" separator (1)
        let add = 7 + percent_encode(name).len();
        if !cur.is_empty() && (len + add > MAX_URL_LEN || cur.len() >= MAX_NAMES_PER_REQUEST) {
            batches.push(std::mem::take(&mut cur));
            len = prefix_len;
        }
        cur.push(name);
        len += add;
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved() {
        assert_eq!(percent_encode("a b/c+d"), "a%20b%2Fc%2Bd");
        assert_eq!(percent_encode("lib32-foo_1.0~x"), "lib32-foo_1.0~x");
    }

    #[test]
    fn batches_by_count_and_length() {
        let names: Vec<String> = (0..450).map(|i| format!("p{i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let b = batch_names(&refs, 40);
        assert_eq!(b.iter().map(Vec::len).collect::<Vec<_>>(), [200, 200, 50]);

        let long = "x".repeat(100);
        let refs = vec![long.as_str(); 100];
        for batch in batch_names(&refs, 40) {
            assert!(40 + batch.len() * 107 <= MAX_URL_LEN);
        }
    }
}
