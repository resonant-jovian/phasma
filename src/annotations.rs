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
        self.annotations.sort_by(|a, b| {
            a.time
                .partial_cmp(&b.time)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_empty() {
        let store = AnnotationStore::new();
        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }

    #[test]
    fn add_single() {
        let mut store = AnnotationStore::new();
        store.add(1.0, "test".to_string());
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());
    }

    #[test]
    fn add_maintains_sorted() {
        let mut store = AnnotationStore::new();
        store.add(5.0, "five".to_string());
        store.add(1.0, "one".to_string());
        store.add(3.0, "three".to_string());
        assert_eq!(store.annotations[0].time, 1.0);
        assert_eq!(store.annotations[1].time, 3.0);
        assert_eq!(store.annotations[2].time, 5.0);
    }

    #[test]
    fn remove_valid() {
        let mut store = AnnotationStore::new();
        store.add(1.0, "a".to_string());
        store.add(2.0, "b".to_string());
        store.add(3.0, "c".to_string());
        store.remove(1); // remove middle
        assert_eq!(store.len(), 2);
        assert_eq!(store.annotations[0].label, "a");
        assert_eq!(store.annotations[1].label, "c");
    }

    #[test]
    fn remove_out_of_bounds() {
        let mut store = AnnotationStore::new();
        store.add(1.0, "a".to_string());
        store.remove(5); // should not panic
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn next_after() {
        let mut store = AnnotationStore::new();
        store.add(1.0, "a".to_string());
        store.add(3.0, "b".to_string());
        store.add(5.0, "c".to_string());
        let next = store.next_after(2.0).unwrap();
        assert_eq!(next.time, 3.0);
        assert_eq!(next.label, "b");
    }

    #[test]
    fn next_after_none() {
        let mut store = AnnotationStore::new();
        store.add(1.0, "a".to_string());
        store.add(3.0, "b".to_string());
        assert!(store.next_after(5.0).is_none());
    }

    #[test]
    fn next_after_empty() {
        let store = AnnotationStore::new();
        assert!(store.next_after(0.0).is_none());
    }

    #[test]
    fn prev_before() {
        let mut store = AnnotationStore::new();
        store.add(1.0, "a".to_string());
        store.add(3.0, "b".to_string());
        store.add(5.0, "c".to_string());
        let prev = store.prev_before(4.0).unwrap();
        assert_eq!(prev.time, 3.0);
        assert_eq!(prev.label, "b");
    }

    #[test]
    fn prev_before_none() {
        let mut store = AnnotationStore::new();
        store.add(3.0, "a".to_string());
        store.add(5.0, "b".to_string());
        assert!(store.prev_before(1.0).is_none());
    }

    #[test]
    fn toml_round_trip() {
        let mut store = AnnotationStore::new();
        store.add(1.5, "first".to_string());
        store.add(3.7, "second".to_string());
        let toml_str = toml::to_string_pretty(&store).unwrap();
        let loaded: AnnotationStore = toml::from_str(&toml_str).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded.annotations[0].time, 1.5);
        assert_eq!(loaded.annotations[0].label, "first");
        assert_eq!(loaded.annotations[1].time, 3.7);
    }

    #[test]
    fn file_save_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("annotations.toml");

        let mut store = AnnotationStore::new();
        store.set_path(path.clone());
        store.add(2.0, "bookmark".to_string());
        store.save().unwrap();

        let loaded = AnnotationStore::load(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.annotations[0].time, 2.0);
        assert_eq!(loaded.annotations[0].label, "bookmark");
    }
}
