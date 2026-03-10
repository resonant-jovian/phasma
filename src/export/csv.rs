use std::io::Write;
use std::path::Path;

use crate::data::live::DiagnosticsStore;

pub fn export_csv(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    stem: &str,
) -> Result<String, String> {
    let path = dir.join(format!("{stem}.csv"));
    let mut f = std::fs::File::create(&path).map_err(|e| format!("create file: {e}"))?;

    writeln!(f, "time,total_energy,kinetic_energy,potential_energy,total_mass,casimir_c2,entropy,virial_ratio")
        .map_err(|e| format!("write header: {e}"))?;

    let energy_data = diagnostics.total_energy.iter_chart_data();
    let kinetic_data = diagnostics.kinetic_energy.iter_chart_data();
    let potential_data = diagnostics.potential_energy.iter_chart_data();
    let mass_data = diagnostics.total_mass.iter_chart_data();
    let c2_data = diagnostics.casimir_c2.iter_chart_data();
    let entropy_data = diagnostics.entropy.iter_chart_data();
    let virial_data = diagnostics.virial_ratio.iter_chart_data();

    for (i, &(t, e)) in energy_data.iter().enumerate() {
        let k = kinetic_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let w = potential_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let m = mass_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let c = c2_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let s = entropy_data.get(i).map(|x| x.1).unwrap_or(0.0);
        let v = virial_data.get(i).map(|x| x.1).unwrap_or(0.0);
        writeln!(f, "{t},{e},{k},{w},{m},{c},{s},{v}").map_err(|e| format!("write row: {e}"))?;
    }

    Ok(path.display().to_string())
}
