use std::path::Path;
use std::sync::Arc;

use arrow::array::Float64Array;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;

use crate::data::live::DiagnosticsStore;
use crate::sim::SimState;

pub fn export_parquet(
    dir: &Path,
    diagnostics: &DiagnosticsStore,
    _state: Option<&SimState>,
) -> Result<String, String> {
    let path = dir.join("diagnostics.parquet");

    let energy = diagnostics.total_energy.iter_chart_data();
    let kinetic = diagnostics.kinetic_energy.iter_chart_data();
    let potential = diagnostics.potential_energy.iter_chart_data();
    let mass = diagnostics.total_mass.iter_chart_data();
    let c2 = diagnostics.casimir_c2.iter_chart_data();
    let entropy = diagnostics.entropy.iter_chart_data();
    let virial = diagnostics.virial_ratio.iter_chart_data();

    let n = energy.len();
    if n == 0 {
        return Err("no diagnostics data to export".to_string());
    }

    let time_arr: Vec<f64> = energy.iter().map(|(t, _)| *t).collect();
    let energy_arr: Vec<f64> = energy.iter().map(|(_, v)| *v).collect();
    let kinetic_arr: Vec<f64> = (0..n)
        .map(|i| kinetic.get(i).map(|x| x.1).unwrap_or(0.0))
        .collect();
    let potential_arr: Vec<f64> = (0..n)
        .map(|i| potential.get(i).map(|x| x.1).unwrap_or(0.0))
        .collect();
    let mass_arr: Vec<f64> = (0..n)
        .map(|i| mass.get(i).map(|x| x.1).unwrap_or(0.0))
        .collect();
    let c2_arr: Vec<f64> = (0..n)
        .map(|i| c2.get(i).map(|x| x.1).unwrap_or(0.0))
        .collect();
    let entropy_arr: Vec<f64> = (0..n)
        .map(|i| entropy.get(i).map(|x| x.1).unwrap_or(0.0))
        .collect();
    let virial_arr: Vec<f64> = (0..n)
        .map(|i| virial.get(i).map(|x| x.1).unwrap_or(0.0))
        .collect();

    let schema = Arc::new(Schema::new(vec![
        Field::new("time", DataType::Float64, false),
        Field::new("total_energy", DataType::Float64, false),
        Field::new("kinetic_energy", DataType::Float64, false),
        Field::new("potential_energy", DataType::Float64, false),
        Field::new("total_mass", DataType::Float64, false),
        Field::new("casimir_c2", DataType::Float64, false),
        Field::new("entropy", DataType::Float64, false),
        Field::new("virial_ratio", DataType::Float64, false),
    ]));

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(Float64Array::from(time_arr)),
            Arc::new(Float64Array::from(energy_arr)),
            Arc::new(Float64Array::from(kinetic_arr)),
            Arc::new(Float64Array::from(potential_arr)),
            Arc::new(Float64Array::from(mass_arr)),
            Arc::new(Float64Array::from(c2_arr)),
            Arc::new(Float64Array::from(entropy_arr)),
            Arc::new(Float64Array::from(virial_arr)),
        ],
    )
    .map_err(|e| format!("record batch: {e}"))?;

    let file = std::fs::File::create(&path).map_err(|e| format!("create file: {e}"))?;
    let mut writer =
        ArrowWriter::try_new(file, schema, None).map_err(|e| format!("parquet writer: {e}"))?;

    writer
        .write(&batch)
        .map_err(|e| format!("write batch: {e}"))?;
    writer.close().map_err(|e| format!("close parquet: {e}"))?;

    Ok(path.display().to_string())
}
