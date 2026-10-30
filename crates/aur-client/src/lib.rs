//! Async client for the [AUR RPC v5](https://aur.archlinux.org/rpc/v5) API.
//!
//! Features: typed models, search/info with URL-length-aware batching,
//! an in-memory TTL cache and a simple client-side rate limiter.

mod cache;
mod client;
mod error;
mod models;
mod rate_limit;

pub use client::{AurClient, AurConfig, SearchBy};
pub use error::Error;
pub use models::Package;
