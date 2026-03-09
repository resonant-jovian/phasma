use serde::{Deserialize, Serialize};

/// A user-placed annotation on the simulation timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub time: f64,
    pub label: String,
    pub tab: usize,
}

#[derive(Debug, Default)]
pub struct AnnotationStore {
    pub items: Vec<Annotation>,
}

impl AnnotationStore {
    pub fn add(&mut self, time: f64, label: impl Into<String>, tab: usize) {
        self.items.push(Annotation { time, label: label.into(), tab });
        self.items.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    pub fn near(&self, time: f64, tolerance: f64) -> Vec<&Annotation> {
        self.items
            .iter()
            .filter(|a| (a.time - time).abs() <= tolerance)
            .collect()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Save annotations to a TOML file alongside the output directory.
    pub fn save(&self, path: &std::path::Path) {
        #[derive(serde::Serialize)]
        struct Wrapper<'a> {
            annotations: &'a [Annotation],
        }
        if let Ok(text) = toml::to_string_pretty(&Wrapper { annotations: &self.items }) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Load annotations from a TOML file.
    pub fn load(path: &std::path::Path) -> Self {
        #[derive(serde::Deserialize)]
        struct Wrapper {
            annotations: Vec<Annotation>,
        }
        let items = std::fs::read_to_string(path)
            .ok()
            .and_then(|s| toml::from_str::<Wrapper>(&s).ok())
            .map(|w| w.annotations)
            .unwrap_or_default();
        Self { items }
    }
}
