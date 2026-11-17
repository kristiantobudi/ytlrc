//! Lyrics provider abstraction. [`lrclib`](crate::lyrics::lrclib) is the only
//! implementation today, but `fetch` in `mod.rs` talks to providers through
//! this trait (as a generic parameter, not a trait object — plain `async fn`
//! in traits isn't dyn-safe) so a second source could be added without
//! touching the cache/orchestration logic: implement [`Provider`] and pass it
//! to [`crate::lyrics::fetch`].
//!
//! ponytail: no second provider exists yet, so there's no provider list or
//! priority/fallback order between providers. Add one when a real second
//! source shows up — until then this is a single-impl seam, not a runtime
//! plugin system.

use std::future::Future;

use super::lrclib::Record;

pub trait Provider {
    /// Exact lookup by artist/track (optionally duration-aware).
    fn lookup(
        &self,
        artist: &str,
        track: &str,
        duration: f64,
    ) -> impl Future<Output = anyhow::Result<Option<Record>>> + Send;

    /// Fuzzy search, used as a fallback when `lookup` misses or for
    /// title-only queries.
    fn search(
        &self,
        artist: &str,
        track: &str,
        duration: f64,
    ) -> impl Future<Output = anyhow::Result<Option<Record>>> + Send;
}

/// The default (and currently only) provider: LRCLIB.
#[derive(Debug, Clone, Copy, Default)]
pub struct Lrclib;

impl Provider for Lrclib {
    async fn lookup(
        &self,
        artist: &str,
        track: &str,
        duration: f64,
    ) -> anyhow::Result<Option<Record>> {
        super::lrclib::lookup(artist, track, duration).await
    }

    async fn search(
        &self,
        artist: &str,
        track: &str,
        duration: f64,
    ) -> anyhow::Result<Option<Record>> {
        super::lrclib::search(artist, track, duration).await
    }
}
