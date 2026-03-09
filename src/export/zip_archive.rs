use std::io::Write;
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

pub fn export_zip(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    state: Option<&SimState>,
) -> Result<String, String> {
    let path = dir.join("phasma_export.zip");
    let file = std::fs::File::create(&path).map_err(|e| format!("create zip: {e}"))?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();

    // Add CSV diagnostics
    let csv_content = build_csv(diagnostics);
    zip.start_file("diagnostics.csv", options)
        .map_err(|e| format!("zip entry: {e}"))?;
    zip.write_all(csv_content.as_bytes())
        .map_err(|e| format!("write csv: {e}"))?;

    // Add JSON final state
    if let Some(s) = state {
        let json = serde_json::to_string_pretty(s).unwrap_or_default();
        zip.start_file("final_state.json", options)
            .map_err(|e| format!("zip entry: {e}"))?;
        zip.write_all(json.as_bytes())
            .map_err(|e| format!("write json: {e}"))?;
    }

    // Add Markdown report
    let report = build_report(diagnostics, state);
    zip.start_file("report.md", options)
        .map_err(|e| format!("zip entry: {e}"))?;
    zip.write_all(report.as_bytes())
        .map_err(|e| format!("write report: {e}"))?;

    // Add performance CSV
    let perf_csv = build_performance_csv(diagnostics);
    zip.start_file("performance.csv", options)
        .map_err(|e| format!("zip entry: {e}"))?;
    zip.write_all(perf_csv.as_bytes())
        .map_err(|e| format!("write perf: {e}"))?;

    // Add conservation CSV (energy/mass/casimir drift series)
    let cons_csv = build_conservation_csv(diagnostics);
    zip.start_file("conservation.csv", options)
        .map_err(|e| format!("zip entry: {e}"))?;
    zip.write_all(cons_csv.as_bytes())
        .map_err(|e| format!("write conservation: {e}"))?;

    zip.finish().map_err(|e| format!("zip finish: {e}"))?;
    Ok(path.display().to_string())
}

fn build_csv(diagnostics: &DiagnosticsStore) -> String {
    let mut out = String::from("time,total_energy,kinetic_energy,potential_energy,total_mass,casimir_c2,entropy,virial_ratio\n");
    let energy = diagnostics.total_energy.iter_chart_data();
    let kinetic = diagnostics.kinetic_energy.iter_chart_data();
    let potential = diagnostics.potential_energy.iter_chart_data();
    let mass = diagnostics.total_mass.iter_chart_data();
    let c2 = diagnostics.casimir_c2.iter_chart_data();
    let entropy = diagnostics.entropy.iter_chart_data();
    let virial = diagnostics.virial_ratio.iter_chart_data();

    for i in 0..energy.len() {
        let t = energy[i].0;
        let e = energy[i].1;
        let k = kinetic.get(i).map(|x| x.1).unwrap_or(0.0);
        let w = potential.get(i).map(|x| x.1).unwrap_or(0.0);
        let m = mass.get(i).map(|x| x.1).unwrap_or(0.0);
        let c = c2.get(i).map(|x| x.1).unwrap_or(0.0);
        let s = entropy.get(i).map(|x| x.1).unwrap_or(0.0);
        let v = virial.get(i).map(|x| x.1).unwrap_or(0.0);
        out.push_str(&format!("{t},{e},{k},{w},{m},{c},{s},{v}\n"));
    }
    out
}

fn build_report(diagnostics: &DiagnosticsStore, state: Option<&SimState>) -> String {
    let mut out = String::from("# Phasma Simulation Report\n\n");

    if let Some(s) = state {
        out.push_str(&format!("## Final State\n\n"));
        out.push_str(&format!("- **Time**: t = {:.4} / {:.1}\n", s.t, s.t_final));
        out.push_str(&format!("- **Step**: {}\n", s.step));
        out.push_str(&format!("- **Total energy**: {:.6e}\n", s.total_energy));
        out.push_str(&format!("- **Energy drift**: {:.2e}\n", s.energy_drift()));
        out.push_str(&format!("- **Total mass**: {:.6e}\n", s.total_mass));
        out.push_str(&format!("- **Virial ratio**: {:.4}\n", s.virial_ratio));
        out.push_str(&format!("- **Casimir C₂**: {:.6e}\n", s.casimir_c2));
        out.push_str(&format!("- **Entropy**: {:.6e}\n", s.entropy));
        out.push_str(&format!("- **Max density**: {:.6e}\n", s.max_density));
        out.push_str(&format!("- **Grid**: {}×{}\n", s.density_nx, s.density_ny));
        if let Some(reason) = s.exit_reason {
            out.push_str(&format!("- **Exit reason**: {reason}\n"));
        }
    }

    out.push_str(&format!("\n## Data Points\n\n"));
    out.push_str(&format!("- Energy series: {} points\n", diagnostics.total_energy.len()));
    out.push_str(&format!("- Mass series: {} points\n", diagnostics.total_mass.len()));

    out
}

fn build_performance_csv(diagnostics: &DiagnosticsStore) -> String {
    let mut out = String::from("time,virial_ratio\n");
    let virial = diagnostics.virial_ratio.iter_chart_data();
    for (t, v) in &virial {
        out.push_str(&format!("{t},{v}\n"));
    }
    out
}

fn build_conservation_csv(diagnostics: &DiagnosticsStore) -> String {
    let mut out = String::from("time,energy_drift,mass_drift,casimir_drift\n");
    let e_drift = diagnostics.energy_drift_series();
    let m_drift = diagnostics.mass_drift_series();
    let c_drift = diagnostics.c2_drift_series();

    for i in 0..e_drift.len() {
        let t = e_drift[i].0;
        let e = e_drift[i].1;
        let m = m_drift.get(i).map(|x| x.1).unwrap_or(0.0);
        let c = c_drift.get(i).map(|x| x.1).unwrap_or(0.0);
        out.push_str(&format!("{t},{e},{m},{c}\n"));
    }
    out
}
