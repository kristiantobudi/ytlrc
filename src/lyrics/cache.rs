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
/// Upper bound for the cache directory; older files are dropped first.
const MAX_FILES: usize = 500;
/// Partially written files older than this are leftovers from a crash.
const TMP_TTL_SECS: u64 = 60 * 60;

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

    /// Housekeeping at startup: drop expired entries, crashed temp files, and
    /// the oldest records above [`MAX_FILES`]. Returns the number of files
    /// removed so the daemon can log it.
    pub fn prune(&self) -> usize {
        self.prune_at(now(), FOUND_TTL_SECS, MAX_FILES)
    }

    fn prune_at(&self, now: u64, max_age_secs: u64, max_files: usize) -> usize {
        let Ok(read_dir) = fs::read_dir(&self.dir) else {
            return 0;
        };
        let mut kept: Vec<(u64, PathBuf)> = Vec::new();
        let mut removed = 0;

        for entry in read_dir.flatten() {
            let path = entry.path();
            let modified = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|elapsed| elapsed.as_secs())
                .unwrap_or(0);

            match path.extension().and_then(|value| value.to_str()) {
                Some("tmp") => {
                    if now.saturating_sub(modified) > TMP_TTL_SECS && fs::remove_file(&path).is_ok()
                    {
                        removed += 1;
                    }
                }
                Some("json") => {
                    if now.saturating_sub(modified) > max_age_secs {
                        if fs::remove_file(&path).is_ok() {
                            removed += 1;
                        }
                    } else {
                        kept.push((modified, path));
                    }
                }
                _ => {}
            }
        }

        if kept.len() > max_files {
            kept.sort_by_key(|(modified, _)| *modified);
            let excess = kept.len() - max_files;
            for (_, path) in kept.into_iter().take(excess) {
                if fs::remove_file(path).is_ok() {
                    removed += 1;
                }
            }
        }

        removed
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

    use std::time::Duration;

    fn temp_cache(name: &str) -> Cache {
        let dir = std::env::temp_dir().join(format!(
            "ytlrc-cache-test-{}-{name}-{}",
            std::process::id(),
            now()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Cache { dir }
    }

    fn write_file(cache: &Cache, name: &str, age_secs: u64) {
        let path = cache.dir.join(name);
        fs::write(&path, "{}").unwrap();
        let modified = SystemTime::now() - Duration::from_secs(age_secs);
        let file = fs::File::options().write(true).open(&path).unwrap();
        file.set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
    }

    #[test]
    fn prune_removes_expired_and_stale_tmp_files() {
        let cache = temp_cache("expired");
        write_file(&cache, "old.json", FOUND_TTL_SECS + 60);
        write_file(&cache, "new.json", 60);
        write_file(&cache, "leftover.tmp", TMP_TTL_SECS + 60);
        write_file(&cache, "fresh.tmp", 5);

        let removed = cache.prune_at(now(), FOUND_TTL_SECS, MAX_FILES);

        assert_eq!(removed, 2);
        assert!(!cache.dir.join("old.json").exists());
        assert!(cache.dir.join("new.json").exists());
        assert!(!cache.dir.join("leftover.tmp").exists());
        assert!(cache.dir.join("fresh.tmp").exists());
        let _ = fs::remove_dir_all(&cache.dir);
    }

    #[test]
    fn prune_caps_file_count_and_keeps_newest() {
        let cache = temp_cache("cap");
        for index in 0..5 {
            write_file(&cache, &format!("entry{index}.json"), 1000 - index * 10);
        }

        let removed = cache.prune_at(now(), FOUND_TTL_SECS, 2);

        assert_eq!(removed, 3);
        assert!(!cache.dir.join("entry0.json").exists());
        assert!(cache.dir.join("entry3.json").exists());
        assert!(cache.dir.join("entry4.json").exists());
        let _ = fs::remove_dir_all(&cache.dir);
    }
}
