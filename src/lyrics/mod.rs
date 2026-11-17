pub mod cache;
pub mod lrc;
pub mod lrclib;
pub mod title;

use cache::{Cache, CachedRecord, Entry};
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
            lrclib::lookup(&candidate.artist, &candidate.track, request.duration).await
        } else {
            lrclib::search(&candidate.artist, &candidate.track, request.duration).await
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
