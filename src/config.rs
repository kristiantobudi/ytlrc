use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub theme: Option<String>,
}

pub fn load() -> Config {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

pub fn save_theme(name: &str) {
    let mut config = load();
    config.theme = Some(name.to_string());

    if let Some(path) = path() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(raw) = serde_json::to_string_pretty(&config) {
            let _ = fs::write(path, raw);
        }
    }
}

fn path() -> Option<PathBuf> {
    directories::ProjectDirs::from("dev", "ytlrc", "ytlrc")
        .map(|dirs| dirs.config_dir().join("config.json"))
}
