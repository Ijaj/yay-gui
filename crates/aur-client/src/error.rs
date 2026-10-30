use std::time::Duration;

/// Errors returned by [`crate::AurClient`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The search term is shorter than the 2 characters the RPC requires.
    #[error("search query must be at least 2 characters")]
    QueryTooShort,
    /// The RPC answered with `type: "error"`.
    #[error("AUR RPC error: {0}")]
    Rpc(String),
    /// HTTP 429; the daily request limit was hit or requests are too fast.
    #[error("rate limited by AUR (retry after {retry_after:?})")]
    RateLimited {
        /// Value of the `Retry-After` header, when present and numeric.
        retry_after: Option<Duration>,
    },
    /// Any other non-success HTTP status.
    #[error("unexpected HTTP status {0}")]
    Status(u16),
    /// Transport-level or body-decoding failure.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// The configured base URL is invalid.
    #[error("invalid base URL: {0}")]
    InvalidUrl(String),
}
