use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Payload sent by the browser extension through `POST /state`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StateUpdate {
    #[serde(alias = "video_id")]
    pub video_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub channel: Option<String>,
    pub duration: f64,
    pub position: f64,
    pub paused: bool,
    #[serde(alias = "is_ad")]
    pub is_ad: bool,
    pub source: Option<String>,
}

/// Playback state stored by the daemon, including the receive timestamp.
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub video_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub channel: Option<String>,
    pub duration: f64,
    pub position: f64,
    pub paused: bool,
    pub is_ad: bool,
    pub source: String,
    pub received_at: Instant,
}

impl PlaybackState {
    pub fn from_update(update: StateUpdate) -> Option<Self> {
        let title = update.title.trim().to_string();
        let video_id = update.video_id.trim().to_string();
        if title.is_empty() && video_id.is_empty() {
            return None;
        }
        Some(Self {
            video_id,
            title,
            artist: update
                .artist
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            channel: update
                .channel
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
            duration: if update.duration.is_finite() {
                update.duration.max(0.0)
            } else {
                0.0
            },
            position: if update.position.is_finite() {
                update.position.max(0.0)
            } else {
                0.0
            },
            paused: update.paused,
            is_ad: update.is_ad,
            source: update.source.unwrap_or_else(|| "youtube".to_string()),
            received_at: Instant::now(),
        })
    }

    /// Current playback position, extrapolated from the last update.
    pub fn position_now(&self) -> f64 {
        if self.paused || self.is_ad {
            return self.position;
        }
        let extra = self.received_at.elapsed().as_secs_f64();
        if self.duration > 0.0 {
            (self.position + extra).min(self.duration)
        } else {
            self.position + extra
        }
    }

    /// Track identity, used to detect song changes.
    ///
    /// For normal videos the video id plus the title is enough; the title is
    /// included so live streams or premieres switch lyrics even when the video
    /// stays the same. Without a video id, artist and title are both used.
    pub fn track_key(&self) -> String {
        let fingerprint: String = self
            .title
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect();

        if !self.video_id.is_empty() {
            format!("v:{}|{}", self.video_id, fingerprint)
        } else {
            let artist: String = self
                .artist
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect();
            format!("t:{}|{}", artist, fingerprint)
        }
    }

    pub fn view(&self) -> StateView {
        StateView {
            video_id: self.video_id.clone(),
            title: self.title.clone(),
            artist: self.artist.clone(),
            channel: self.channel.clone(),
            duration: self.duration,
            position: self.position_now(),
            paused: self.paused,
            is_ad: self.is_ad,
            source: self.source.clone(),
            updated_ago: self.received_at.elapsed().as_secs_f64(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateView {
    pub video_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub channel: Option<String>,
    pub duration: f64,
    pub position: f64,
    pub paused: bool,
    pub is_ad: bool,
    pub source: String,
    pub updated_ago: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(video_id: &str, title: &str) -> PlaybackState {
        PlaybackState {
            video_id: video_id.to_string(),
            title: title.to_string(),
            artist: None,
            channel: None,
            duration: 200.0,
            position: 0.0,
            paused: false,
            is_ad: false,
            source: "youtube".to_string(),
            received_at: Instant::now(),
        }
    }

    #[test]
    fn track_key_changes_when_title_changes_in_same_video() {
        let first = state("same-video", "Song A");
        let second = state("same-video", "Song B");
        assert_ne!(first.track_key(), second.track_key());
    }

    #[test]
    fn track_key_ignores_punctuation_and_case() {
        let first = state("abc", "Hello, World!");
        let second = state("abc", "hello world");
        assert_eq!(first.track_key(), second.track_key());
    }

    #[test]
    fn track_key_ignores_artist_when_video_id_is_known() {
        let mut first = state("abc", "Song");
        let mut second = state("abc", "Song");
        first.artist = Some("Channel A".to_string());
        second.artist = Some("Channel B".to_string());
        assert_eq!(first.track_key(), second.track_key());
    }
}
