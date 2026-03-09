use std::path::Path;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

pub fn export_json(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
) -> Result<String, String> {
    let path = dir.join("diagnostics.json");

    let energy = diagnostics.total_energy.iter_chart_data();
    let mass = diagnostics.total_mass.iter_chart_data();
    let c2 = diagnostics.casimir_c2.iter_chart_data();

    let mut doc = serde_json::Map::new();
    doc.insert("energy".to_string(), serde_json::to_value(&energy).unwrap_or_default());
    doc.insert("mass".to_string(), serde_json::to_value(&mass).unwrap_or_default());
    doc.insert("casimir_c2".to_string(), serde_json::to_value(&c2).unwrap_or_default());

    if let Some(s) = state {
        doc.insert("final_state".to_string(), serde_json::to_value(s).unwrap_or_default());
    }

    let json = serde_json::to_string_pretty(&doc).map_err(|e| format!("json: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("write: {e}"))?;

    Ok(path.display().to_string())
}
