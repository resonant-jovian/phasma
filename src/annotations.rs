//! Bookmark annotations for simulation time points.
//!
//! Annotations mark interesting moments in a simulation run (e.g. "first caustic",
//! "virial equilibrium reached"). They are persisted to an `annotations.toml` file
//! in the output directory and can be navigated with Ctrl+B.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub time: f64,
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnotationStore {
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    #[serde(skip)]
    file_path: Option<PathBuf>,
}

#[allow(dead_code)]
impl AnnotationStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load annotations from a TOML file, or return an empty store.
    pub fn load(path: &Path) -> Self {
        let mut store = if path.exists() {
            std::fs::read_to_string(path)
                .ok()
                .and_then(|s| toml::from_str::<AnnotationStore>(&s).ok())
                .unwrap_or_default()
        } else {
            Self::default()
        };
        store.file_path = Some(path.to_path_buf());
        store
    }

    /// Set the file path for persistence.
    pub fn set_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
    }

    /// Add a bookmark at the given simulation time.
    pub fn add(&mut self, time: f64, label: String) {
        self.annotations.push(Annotation { time, label });
        self.annotations
            .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    /// Remove annotation at index.
    pub fn remove(&mut self, index: usize) {
        if index < self.annotations.len() {
            self.annotations.remove(index);
        }
    }

    /// Number of annotations.
    pub fn len(&self) -> usize {
        self.annotations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.annotations.is_empty()
    }

    /// Find the next annotation after the given time.
    pub fn next_after(&self, time: f64) -> Option<&Annotation> {
        self.annotations.iter().find(|a| a.time > time + 1e-10)
    }

    /// Find the previous annotation before the given time.
    pub fn prev_before(&self, time: f64) -> Option<&Annotation> {
        self.annotations
            .iter()
            .rev()
            .find(|a| a.time < time - 1e-10)
    }

    /// Save to the configured file path.
    pub fn save(&self) -> std::io::Result<()> {
        if let Some(ref path) = self.file_path {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content =
                toml::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
            std::fs::write(path, content)?;
        }
        Ok(())
    }
}
