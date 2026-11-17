use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Lyrics that were found are cached for a long time.
const FOUND_TTL_SECS: u64 = 60 * 60 * 24 * 30;
/// "Not found" results are only cached briefly so they are retried soon.
const MISS_TTL_SECS: u64 = 60 * 10;

#[derive(Debug, Clone)]
pub struct Cache {
    dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedRecord {
    pub track: String,
    pub artist: String,
    #[serde(default)]
    pub synced_lyrics: Option<String>,
    #[serde(default)]
    pub plain_lyrics: Option<String>,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default)]
    pub duration: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub fetched_at: u64,
    pub query: String,
    pub not_found: bool,
    #[serde(default)]
    pub record: Option<CachedRecord>,
}

impl Cache {
    pub fn new() -> Result<Self> {
        let dir = directories::ProjectDirs::from("dev", "ytlrc", "ytlrc")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| std::env::temp_dir().join("ytlrc-cache"));
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create cache directory at {}", dir.display()))?;
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Stable key for one track. The video id alone is not enough: live
    /// streams and DJ sets switch songs without changing the video, so the
    /// title is part of the key as well. Without a video id the artist is
    /// included too, to keep free-text searches apart.
    pub fn key(video_id: &str, artist: &str, track: &str) -> String {
        let fingerprint = |value: &str| -> String {
            value
                .trim()
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect()
        };

        let base = if !video_id.trim().is_empty() {
            format!("video:{}|{}", video_id.trim(), fingerprint(track))
        } else {
            format!("query:{}|{}", fingerprint(artist), fingerprint(track))
        };
        format!("{:016x}", fnv1a(base.as_bytes()))
    }

    pub fn get(&self, key: &str) -> Option<Entry> {
        let path = self.path(key);
        let raw = fs::read_to_string(&path).ok()?;
        let entry: Entry = serde_json::from_str(&raw).ok()?;
        let age = now().saturating_sub(entry.fetched_at);
        let ttl = if entry.not_found {
            MISS_TTL_SECS
        } else {
            FOUND_TTL_SECS
        };
        if age > ttl {
            let _ = fs::remove_file(path);
            return None;
        }
        Some(entry)
    }

    pub fn put(&self, key: &str, entry: &Entry) {
        let path = self.path(key);
        let tmp = path.with_extension("tmp");
        if let Ok(raw) = serde_json::to_string(entry) {
            if fs::write(&tmp, raw).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
    }

    pub fn invalidate(&self, key: &str) {
        let _ = fs::remove_file(self.path(key));
    }

    fn path(&self, key: &str) -> PathBuf {
        self.dir.join(format!("{key}.json"))
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_video_different_song_gets_different_key() {
        let first = Cache::key("live-video", "Artist A", "Song A");
        let second = Cache::key("live-video", "Artist A", "Song B");
        assert_ne!(first, second, "live streams must not reuse old lyrics");
    }

    #[test]
    fn key_ignores_case_and_punctuation() {
        let first = Cache::key("abc", "Rick Astley", "Never Gonna Give You Up");
        let second = Cache::key("abc", "rick astley", "never gonna give you up!");
        assert_eq!(first, second);
    }

    #[test]
    fn key_without_video_id_is_query_based() {
        assert_eq!(
            Cache::key("", "Adele", "Hello"),
            Cache::key("  ", "adele", "hello")
        );
    }

    #[test]
    fn key_ignores_artist_when_video_id_is_known() {
        assert_eq!(
            Cache::key("abc", "Channel A", "Hello"),
            Cache::key("abc", "Channel B", "Hello")
        );
    }
}
