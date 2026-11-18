pub mod cache;
pub mod lrc;
pub mod lrclib;
pub mod provider;
pub mod title;

use cache::{Cache, CachedRecord, Entry};
use provider::{Lrclib, Provider};
use title::ParsedTrack;

#[derive(Debug, Clone)]
pub struct FoundLyrics {
    pub track: String,
    pub artist: String,
    pub synced: bool,
    pub lines: Vec<lrc::LyricLine>,
    pub offset_ms: i64,
    pub plain: Option<String>,
    pub cached: bool,
}

#[derive(Debug, Clone)]
pub enum FetchOutcome {
    Found(FoundLyrics),
    NotFound(String),
    Error(String),
}

/// Lyrics lookup request. `deep` enables additional fallback queries.
pub struct Lookup<'a> {
    pub video_id: &'a str,
    pub parsed: &'a ParsedTrack,
    pub duration: f64,
    pub use_cache: bool,
    pub deep: bool,
    pub raw_title: &'a str,
    pub channel: Option<&'a str>,
}

pub async fn fetch(cache: &Cache, request: &Lookup<'_>) -> FetchOutcome {
    fetch_with(&Lrclib, cache, request).await
}

/// Same as [`fetch`] but against an explicit provider — the seam
/// [`provider::Provider`] exists for.
pub async fn fetch_with<P: Provider>(
    provider: &P,
    cache: &Cache,
    request: &Lookup<'_>,
) -> FetchOutcome {
    let key = Cache::key(
        request.video_id,
        &request.parsed.artist,
        &request.parsed.track,
    );

    if request.use_cache {
        if let Some(entry) = cache.get(&key) {
            if entry.not_found {
                return FetchOutcome::NotFound(entry.query);
            }
            if let Some(record) = entry.record {
                return FetchOutcome::Found(build(record, true));
            }
        }
    }

    let candidates = if request.deep {
        title::search_candidates(request.raw_title, request.channel)
    } else {
        vec![request.parsed.clone()]
    };

    let mut plain_fallback: Option<(CachedRecord, FoundLyrics)> = None;
    let mut last_error: Option<String> = None;

    for (index, candidate) in candidates.iter().enumerate() {
        let result = if index == 0 {
            provider
                .lookup(&candidate.artist, &candidate.track, request.duration)
                .await
        } else {
            provider
                .search(&candidate.artist, &candidate.track, request.duration)
                .await
        };

        match result {
            Ok(Some(record)) => {
                let cached = to_cached(&record);
                let found = build(cached.clone(), false);
                if found.synced {
                    store(cache, &key, request, &cached);
                    return FetchOutcome::Found(found);
                }
                if plain_fallback.is_none() {
                    plain_fallback = Some((cached, found));
                }
            }
            Ok(None) => {}
            Err(error) => last_error = Some(error.to_string()),
        }
    }

    if let Some((cached, found)) = plain_fallback {
        store(cache, &key, request, &cached);
        return FetchOutcome::Found(found);
    }

    if let Some(error) = last_error {
        // Network/rate-limit errors are not cached so the next attempt can retry.
        return FetchOutcome::Error(error);
    }

    let message = format!(
        "No lyrics found for \"{}\" — {}",
        request.parsed.track, request.parsed.artist
    );
    cache.put(
        &key,
        &Entry {
            fetched_at: cache::now(),
            query: message.clone(),
            not_found: true,
            record: None,
        },
    );
    FetchOutcome::NotFound(message)
}

fn to_cached(record: &lrclib::Record) -> CachedRecord {
    CachedRecord {
        track: record.track_name.clone(),
        artist: record.artist_name.clone(),
        synced_lyrics: record.synced_lyrics.clone(),
        plain_lyrics: record.plain_lyrics.clone(),
        instrumental: record.instrumental,
        duration: Some(record.duration),
    }
}

fn store(cache: &Cache, key: &str, request: &Lookup<'_>, record: &CachedRecord) {
    cache.put(
        key,
        &Entry {
            fetched_at: cache::now(),
            query: format!("{} — {}", request.parsed.artist, request.parsed.track),
            not_found: false,
            record: Some(record.clone()),
        },
    );
}

fn build(record: CachedRecord, cached: bool) -> FoundLyrics {
    let (lines, offset_ms, synced) = match record.synced_lyrics.as_deref().map(lrc::parse) {
        Some(parsed) if !parsed.lines.is_empty() => (parsed.lines, parsed.offset_ms, true),
        _ => (Vec::new(), 0, false),
    };

    let mut plain = record.plain_lyrics;
    if plain
        .as_deref()
        .map(|text| text.trim().is_empty())
        .unwrap_or(true)
        && lines.is_empty()
    {
        plain = Some("♪  (instrumental / no lyrics)".to_string());
    }

    FoundLyrics {
        track: record.track,
        artist: record.artist,
        synced,
        lines,
        offset_ms,
        plain,
        cached,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake provider for exercising `fetch_with` without touching the
    /// network — proves `Provider` is a usable seam, not just a trait that
    /// compiles.
    struct FakeProvider {
        record: Option<lrclib::Record>,
    }

    impl Provider for FakeProvider {
        async fn lookup(
            &self,
            _artist: &str,
            _track: &str,
            _duration: f64,
        ) -> anyhow::Result<Option<lrclib::Record>> {
            Ok(self.record.clone())
        }

        async fn search(
            &self,
            _artist: &str,
            _track: &str,
            _duration: f64,
        ) -> anyhow::Result<Option<lrclib::Record>> {
            Ok(self.record.clone())
        }
    }

    fn lookup(video_id: &str, artist: &str, track: &str) -> (ParsedTrack, String) {
        let parsed = ParsedTrack {
            artist: artist.to_string(),
            track: track.to_string(),
        };
        (parsed, video_id.to_string())
    }

    #[tokio::test]
    async fn fetch_with_a_custom_provider_returns_its_synced_lyrics() {
        let cache = cache::Cache::new().expect("cache dir");
        let provider = FakeProvider {
            record: Some(lrclib::Record {
                id: 1,
                track_name: "Song".to_string(),
                artist_name: "Artist".to_string(),
                instrumental: false,
                plain_lyrics: Some("plain".to_string()),
                synced_lyrics: Some("[00:01.00] line".to_string()),
                duration: 0.0,
            }),
        };
        let (parsed, video_id) = lookup("fake-video-provider-test", "Artist", "Song");
        let request = Lookup {
            video_id: &video_id,
            parsed: &parsed,
            duration: 0.0,
            use_cache: false,
            deep: false,
            raw_title: "Artist - Song",
            channel: None,
        };

        let outcome = fetch_with(&provider, &cache, &request).await;
        match outcome {
            FetchOutcome::Found(found) => {
                assert!(found.synced);
                assert_eq!(found.lines[0].text, "line");
            }
            other => panic!("expected Found, got {other:?}"),
        }
        cache.invalidate(&cache::Cache::key(&video_id, "Artist", "Song"));
    }

    #[tokio::test]
    async fn fetch_with_a_provider_that_finds_nothing_reports_not_found() {
        let cache = cache::Cache::new().expect("cache dir");
        let provider = FakeProvider { record: None };
        let (parsed, video_id) = lookup("fake-video-provider-miss", "Nobody", "Nothing");
        let request = Lookup {
            video_id: &video_id,
            parsed: &parsed,
            duration: 0.0,
            use_cache: false,
            deep: false,
            raw_title: "Nobody - Nothing",
            channel: None,
        };

        let outcome = fetch_with(&provider, &cache, &request).await;
        assert!(matches!(outcome, FetchOutcome::NotFound(_)));
        cache.invalidate(&cache::Cache::key(&video_id, "Nobody", "Nothing"));
    }
}
