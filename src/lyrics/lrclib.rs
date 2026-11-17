//! LRCLIB client. It deliberately shells out to `curl` instead of linking a
//! TLS stack, which keeps the binary small and avoids a C toolchain. `curl` is
//! available on WSL, almost every Linux distribution, and Windows 10+.

use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

pub const USER_AGENT: &str = concat!(
    "ytlrc/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/kristiantobudi/ytlrc)"
);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Record {
    #[serde(default)]
    pub id: i64,
    #[serde(default, rename = "trackName")]
    pub track_name: String,
    #[serde(default, rename = "artistName")]
    pub artist_name: String,
    #[serde(default)]
    pub instrumental: bool,
    #[serde(default, rename = "plainLyrics")]
    pub plain_lyrics: Option<String>,
    #[serde(default, rename = "syncedLyrics")]
    pub synced_lyrics: Option<String>,
    #[serde(default)]
    pub duration: f64,
}

pub async fn lookup(artist: &str, track: &str, duration: f64) -> anyhow::Result<Option<Record>> {
    let artist = artist.trim();
    let track = track.trim();
    if track.is_empty() {
        return Ok(None);
    }

    if !artist.is_empty() && !artist.eq_ignore_ascii_case("unknown") {
        if let Some(record) = get(artist, track, duration).await? {
            return Ok(Some(record));
        }
        if duration >= 1.0 {
            if let Some(record) = get(artist, track, 0.0).await? {
                return Ok(Some(record));
            }
        }
    }

    search(artist, track, duration).await
}

async fn get(artist: &str, track: &str, duration: f64) -> anyhow::Result<Option<Record>> {
    let mut url = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}",
        urlencode(artist),
        urlencode(track)
    );
    if duration >= 1.0 {
        url.push_str(&format!("&duration={}", duration.round() as i64));
    }

    match http_get(&url).await? {
        (200, body) => Ok(serde_json::from_str(&body).ok()),
        (400 | 404, _) => Ok(None),
        (429, _) => {
            anyhow::bail!("LRCLIB is rate limiting requests (429). Please try again shortly.")
        }
        (status, body) => anyhow::bail!("LRCLIB returned {}: {}", status, short(&body)),
    }
}

pub async fn search(artist: &str, track: &str, duration: f64) -> anyhow::Result<Option<Record>> {
    let query = format!("{} {}", artist, track);
    let url = format!(
        "https://lrclib.net/api/search?q={}",
        urlencode(query.trim())
    );

    match http_get(&url).await? {
        (200, body) => {
            let records: Vec<Record> = serde_json::from_str(&body).unwrap_or_default();
            Ok(best_match(records, track, artist, duration))
        }
        (400 | 404, _) => Ok(None),
        (429, _) => {
            anyhow::bail!("LRCLIB is rate limiting requests (429). Please try again shortly.")
        }
        (status, body) => anyhow::bail!("LRCLIB returned {}: {}", status, short(&body)),
    }
}

fn best_match(records: Vec<Record>, track: &str, artist: &str, duration: f64) -> Option<Record> {
    let wanted = normalize(track);
    let wanted_artist = normalize(artist);
    let mut best: Option<(f64, Record)> = None;

    for record in records {
        let score = score(&record, &wanted, &wanted_artist, duration);
        if best
            .as_ref()
            .map(|(best_score, _)| score > *best_score)
            .unwrap_or(true)
        {
            best = Some((score, record));
        }
    }

    match best {
        Some((score, record)) if score > 0.0 => Some(record),
        _ => None,
    }
}

fn score(record: &Record, wanted: &str, wanted_artist: &str, duration: f64) -> f64 {
    let mut score = 0.0;
    let name = normalize(&record.track_name);

    if !wanted.is_empty() && name == wanted {
        score += 100.0;
    } else if !wanted.is_empty() && (name.contains(wanted) || wanted.contains(&name)) {
        score += 40.0;
    }

    if !wanted_artist.is_empty() {
        let record_artist = normalize(&record.artist_name);
        if !record_artist.is_empty() {
            if record_artist == *wanted_artist {
                score += 40.0;
            } else if record_artist.contains(wanted_artist)
                || wanted_artist.contains(&record_artist)
            {
                score += 20.0;
            }
        }
    }

    if record.synced_lyrics.is_some() {
        score += 20.0;
    }

    if duration >= 1.0 && record.duration >= 1.0 {
        let diff = (record.duration - duration).abs();
        score += if diff <= 2.0 {
            80.0
        } else if diff <= 8.0 {
            35.0
        } else if diff <= 30.0 {
            5.0
        } else {
            -30.0
        };
    }

    score
}

async fn http_get(url: &str) -> anyhow::Result<(u16, String)> {
    let output = Command::new("curl")
        .arg("--silent")
        .arg("--show-error")
        .arg("--max-time")
        .arg("12")
        .arg("--user-agent")
        .arg(USER_AGENT)
        .arg("--write-out")
        .arg("\n%{http_code}")
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|err| anyhow::anyhow!("failed to run curl: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("curl failed: {}", stderr.trim());
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let (body, status) = text
        .rsplit_once('\n')
        .ok_or_else(|| anyhow::anyhow!("invalid curl response"))?;
    let status = status.trim().parse::<u16>().unwrap_or(0);
    Ok((status, body.to_string()))
}

fn short(text: &str) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.chars().count() > 160 {
        trimmed.chars().take(160).collect::<String>() + "…"
    } else {
        trimmed
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

fn urlencode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(track: &str, artist: &str, synced: bool, duration: f64) -> Record {
        Record {
            id: 1,
            track_name: track.to_string(),
            artist_name: artist.to_string(),
            instrumental: false,
            plain_lyrics: Some("plain".to_string()),
            synced_lyrics: if synced {
                Some("[00:01.00] line".to_string())
            } else {
                None
            },
            duration,
        }
    }

    #[test]
    fn exact_title_and_artist_with_synced_lyrics_wins_over_partial_match() {
        let exact = record("Never Gonna Give You Up", "Rick Astley", true, 213.0);
        let partial = record(
            "Never Gonna Give You Up (Remix)",
            "Rick Astley Cover",
            false,
            213.0,
        );
        let best = best_match(
            vec![partial.clone(), exact.clone()],
            "Never Gonna Give You Up",
            "Rick Astley",
            213.0,
        );
        assert_eq!(best.unwrap().track_name, exact.track_name);
    }

    #[test]
    fn duration_mismatch_beyond_30s_scores_lower_than_a_close_match() {
        let wanted_duration = 60.0;
        let close = score(
            &record("Hello", "Adele", true, 61.0),
            &normalize("Hello"),
            &normalize("Adele"),
            wanted_duration,
        );
        let far_off = score(
            &record("Hello", "Adele", true, 300.0),
            &normalize("Hello"),
            &normalize("Adele"),
            wanted_duration,
        );
        assert!(
            far_off < close,
            "expected a large duration gap ({far_off}) to score lower than a close one ({close})"
        );
    }

    #[test]
    fn a_wrong_track_with_a_wrong_duration_scores_non_positive() {
        let score = score(
            &record("Completely Different", "Nobody", false, 9999.0),
            &normalize("Hello"),
            &normalize("Adele"),
            60.0,
        );
        assert!(
            score <= 0.0,
            "expected no partial match to win, got {score}"
        );
    }

    #[test]
    fn best_match_returns_none_when_every_candidate_scores_non_positive() {
        let wrong = record("Completely Different Song", "Nobody", false, 9999.0);
        let best = best_match(vec![wrong], "Hello", "Adele", 60.0);
        assert!(best.is_none());
    }

    #[test]
    fn best_match_returns_none_for_an_empty_candidate_list() {
        assert!(best_match(Vec::new(), "Hello", "Adele", 200.0).is_none());
    }

    #[test]
    fn synced_lyrics_are_scored_above_plain_only_otherwise_identical_records() {
        let synced = record("Song", "Artist", true, 0.0);
        let plain_only = record("Song", "Artist", false, 0.0);
        let synced_score = score(&synced, &normalize("Song"), &normalize("Artist"), 0.0);
        let plain_score = score(&plain_only, &normalize("Song"), &normalize("Artist"), 0.0);
        assert!(synced_score > plain_score);
    }

    #[test]
    fn normalize_ignores_case_and_punctuation() {
        assert_eq!(normalize("Hello, World!"), normalize("hello world"));
    }

    #[test]
    fn urlencode_escapes_spaces_and_reserved_characters() {
        assert_eq!(urlencode("a b&c"), "a+b%26c");
        assert_eq!(urlencode("safe-_.~chars"), "safe-_.~chars");
    }
}
