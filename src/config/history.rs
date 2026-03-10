//! Track recently used config files in ~/.config/phasma/history.json.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;

const MAX_RECENT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEntry {
    path: String,
    /// Seconds since UNIX epoch.
    timestamp: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HistoryFile {
    recent: Vec<HistoryEntry>,
}

fn history_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "phasma", "phasma")
        .map(|d| d.config_local_dir().join("history.json"))
}

fn load_history() -> HistoryFile {
    let Some(p) = history_path() else {
        return HistoryFile::default();
    };
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_history(h: &HistoryFile) {
    let Some(p) = history_path() else { return };
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(h) {
        let _ = std::fs::write(p, json);
    }
}

/// Record a config path as recently used.
pub fn push_recent(path: &str) {
    let mut h = load_history();

    // Canonicalize the path for deduplication
    let canonical = std::fs::canonicalize(path)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.to_string());

    // Remove existing entry for this path
    h.recent.retain(|e| e.path != canonical);

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    h.recent.insert(
        0,
        HistoryEntry {
            path: canonical,
            timestamp: now,
        },
    );

    // Trim to max
    h.recent.truncate(MAX_RECENT);

    save_history(&h);
}
