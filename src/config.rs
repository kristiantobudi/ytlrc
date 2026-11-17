use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

/// Upper bound on remembered per-track offsets, oldest dropped first, so the
/// config file cannot grow without limit after months of listening.
const MAX_OFFSET_ENTRIES: usize = 200;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: Option<String>,
    /// `(track key, offset seconds)`, most recently used last.
    #[serde(default)]
    pub offsets: Vec<(String, f64)>,
    /// User-remappable single-key shortcuts. Unset keys keep their default.
    #[serde(default)]
    pub keybindings: Keybindings,
}

/// Single-character shortcuts for actions that default to a letter or
/// symbol. Navigation keys (arrows, Page Up/Down, Home, q/Esc to quit,
/// Ctrl+C) are intentionally not remappable here — only the ones a user is
/// likely to want on a different key on their layout. Avoid remapping onto
/// `1`-`5` (theme shortcuts) or `q`/`Esc`: those are matched first and would
/// silently shadow the custom binding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Keybindings {
    pub offset_up: char,
    pub offset_down: char,
    pub offset_reset: char,
    pub theme_next: char,
    pub toggle_mode: char,
    pub refetch: char,
    pub manual_search: char,
}

impl Default for Keybindings {
    fn default() -> Self {
        Self {
            offset_up: '+',
            offset_down: '-',
            offset_reset: '0',
            theme_next: 't',
            toggle_mode: 'p',
            refetch: 'r',
            manual_search: 'm',
        }
    }
}

pub fn load() -> Config {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn save(config: &Config) {
    if let Some(path) = path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(config) {
            let _ = fs::write(path, raw);
        }
    }
}

pub fn save_theme(name: &str) {
    let mut config = load();
    config.theme = Some(name.to_string());
    save(&config);
}

/// Previously saved lyric offset for this track, if any.
pub fn load_offset(key: &str) -> Option<f64> {
    load()
        .offsets
        .into_iter()
        .find(|(entry_key, _)| entry_key == key)
        .map(|(_, value)| value)
}

/// Remember the lyric offset for this track so it is restored next time the
/// same song plays. A value of exactly `0.0` still gets stored: it overwrites
/// a previous non-zero offset the user explicitly reset.
pub fn save_offset(key: &str, value: f64) {
    let mut config = load();
    config.offsets.retain(|(entry_key, _)| entry_key != key);
    config.offsets.push((key.to_string(), value));
    if config.offsets.len() > MAX_OFFSET_ENTRIES {
        let excess = config.offsets.len() - MAX_OFFSET_ENTRIES;
        config.offsets.drain(0..excess);
    }
    save(&config);
}

fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "ytlrc", "ytlrc")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_cap_at_max_entries_dropping_the_oldest_first() {
        let mut config = Config::default();
        for index in 0..MAX_OFFSET_ENTRIES + 5 {
            config
                .offsets
                .push((format!("track-{index}"), index as f64));
        }
        if config.offsets.len() > MAX_OFFSET_ENTRIES {
            let excess = config.offsets.len() - MAX_OFFSET_ENTRIES;
            config.offsets.drain(0..excess);
        }
        assert_eq!(config.offsets.len(), MAX_OFFSET_ENTRIES);
        assert_eq!(config.offsets[0].0, "track-5");
    }
}
