//! Save/load/list named presets in ~/.config/phasma/presets/.

use std::path::PathBuf;

fn presets_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "phasma", "phasma")
        .map(|d| d.config_local_dir().join("presets"))
}

/// Save a PhasmaConfig as a named preset.
pub fn save_preset(name: &str, config_path: &str) -> anyhow::Result<()> {
    let cfg = crate::config::load(config_path)?;
    let dir = presets_dir().ok_or_else(|| anyhow::anyhow!("cannot determine config directory"))?;
    std::fs::create_dir_all(&dir)?;

    let toml_str = toml::to_string_pretty(&cfg)?;
    let path = dir.join(format!("{name}.toml"));
    std::fs::write(&path, toml_str)?;

    eprintln!("phasma: preset '{name}' saved to {}", path.display());
    Ok(())
}
