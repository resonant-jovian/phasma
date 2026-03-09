use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: DateTime<Utc>,
    pub path: String,
    pub summary: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    entries: Vec<HistoryEntry>,
}

fn history_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("com", "phasma", "phasma")
        .map(|d| d.config_local_dir().join("history.toml"))
}

pub fn push(path: &str, summary: &str) {
    let Some(hp) = history_path() else { return };
    let mut hf = load_file(&hp);
    hf.entries.push(HistoryEntry {
        timestamp: Utc::now(),
        path: path.to_string(),
        summary: summary.to_string(),
    });
    // Keep last 50
    if hf.entries.len() > 50 {
        let excess = hf.entries.len() - 50;
        hf.entries.drain(..excess);
    }
    save_file(&hp, &hf);
}

pub fn list() -> Vec<HistoryEntry> {
    let Some(hp) = history_path() else { return Vec::new() };
    load_file(&hp).entries
}

fn load_file(path: &std::path::Path) -> HistoryFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_file(path: &std::path::Path, hf: &HistoryFile) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(s) = toml::to_string_pretty(hf) {
        let _ = std::fs::write(path, s);
    }
}
