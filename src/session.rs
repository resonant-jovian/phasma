use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub config_path: Option<String>,
    pub active_tab: usize,
    pub colormap: String,
    pub projection_axis: usize,
    pub theme: String,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            config_path: None,
            active_tab: 0,
            colormap: "viridis".to_string(),
            projection_axis: 2,
            theme: "dark".to_string(),
        }
    }
}

fn session_path() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("com", "phasma", "phasma")
        .map(|d| d.config_local_dir().join("session.toml"))
}

pub fn load() -> Session {
    let Some(sp) = session_path() else { return Session::default() };
    std::fs::read_to_string(&sp)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

/// Best-effort save — silently ignores any errors.
pub fn save(s: &Session) {
    let Some(sp) = session_path() else { return };
    if let Some(parent) = sp.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = toml::to_string_pretty(s) {
        let _ = std::fs::write(sp, text);
    }
}
