use super::{non_empty, validate_finite, validate_unit, UtmlError, UtmlErrorCode};
use context_graph_mejepa_instruments::{InstrumentSlot, Panel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MI_BINS: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseMiMatrix {
    pub slots: Vec<String>,
    pub values: Vec<Vec<f32>>,
}

impl PairwiseMiMatrix {
    pub fn validate(&self) -> Result<(), UtmlError> {
        non_empty("pairwise_mi.slots", &self.slots)?;
        if self.values.len() != self.slots.len() {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "pairwise_mi matrix row count {} does not match slot count {}",
                    self.values.len(),
                    self.slots.len()
                ),
            ));
        }
        for (row_idx, row) in self.values.iter().enumerate() {
            if row.len() != self.slots.len() {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    format!(
                        "pairwise_mi row {row_idx} has {} columns; expected {}",
                        row.len(),
                        self.slots.len()
                    ),
                ));
            }
            for (col_idx, value) in row.iter().enumerate() {
                validate_unit(&format!("pairwise_mi.values[{row_idx}][{col_idx}]"), *value)?;
            }
        }
        Ok(())
    }

    pub fn max_off_diagonal(&self) -> f32 {
        let mut max_value = 0.0;
        for (row_idx, row) in self.values.iter().enumerate() {
            for (col_idx, value) in row.iter().enumerate() {
                if row_idx != col_idx && *value > max_value {
                    max_value = *value;
                }
            }
        }
        max_value
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseMiSummary {
    pub step: u64,
    pub sample_count: usize,
    pub path: PathBuf,
    pub readback_path: PathBuf,
    pub matrix: PairwiseMiMatrix,
    pub max_off_diagonal: f32,
    pub mean_off_diagonal: f32,
}

#[derive(Debug, Clone)]
pub struct PairwiseMiAuditor {
    pub period_steps: u64,
    pub output_dir: PathBuf,
}

impl PairwiseMiAuditor {
    pub fn new(period_steps: u64, output_dir: impl Into<PathBuf>) -> Result<Self, UtmlError> {
        if period_steps == 0 {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                "period_steps must be greater than zero",
            ));
        }
        Ok(Self {
            period_steps,
            output_dir: output_dir.into(),
        })
    }

    pub fn should_run(&self, step: u64) -> bool {
        step.is_multiple_of(self.period_steps)
    }

    pub fn run_from_panels(
        &self,
        panels: &[Panel],
        step: u64,
    ) -> Result<PairwiseMiSummary, UtmlError> {
        non_empty("panels", panels)?;
        let mut series_by_slot = BTreeMap::new();
        for slot in InstrumentSlot::all() {
            let mut series = Vec::with_capacity(panels.len());
            for (panel_idx, panel) in panels.iter().enumerate() {
                if !panel.is_filled(slot) {
                    return Err(UtmlError::new(
                        UtmlErrorCode::MissingSourceOfTruth,
                        format!(
                            "panel {panel_idx} is missing filled source-of-truth slot {}",
                            slot.slug()
                        ),
                    ));
                }
                series.push(mean_slot(panel.slot(slot), slot.slug())?);
            }
            series_by_slot.insert(slot.slug().to_string(), series);
        }
        self.run_from_slot_series(&series_by_slot, step)
    }

    pub fn run_from_slot_series(
        &self,
        series_by_slot: &BTreeMap<String, Vec<f32>>,
        step: u64,
    ) -> Result<PairwiseMiSummary, UtmlError> {
        let matrix = compute_pairwise_mi_matrix(series_by_slot)?;
        fs::create_dir_all(&self.output_dir).map_err(|err| {
            UtmlError::new(
                UtmlErrorCode::Io,
                format!(
                    "failed to create pairwise MI output dir {}: {err}",
                    self.output_dir.display()
                ),
            )
        })?;
        let path = self
            .output_dir
            .join(format!("utml-pairwise-mi-matrix-{step:08}.csv"));
        write_matrix_csv_0600(&path, &matrix)?;
        let readback = load_pairwise_mi_matrix_csv(&path)?;
        if readback.slots != matrix.slots || !matrix_values_close(&readback.values, &matrix.values)
        {
            return Err(UtmlError::new(
                UtmlErrorCode::ReadbackMismatch,
                format!("pairwise MI CSV readback mismatch at {}", path.display()),
            ));
        }
        let (mean_off_diagonal, max_off_diagonal) = off_diagonal_stats(&matrix);
        Ok(PairwiseMiSummary {
            step,
            sample_count: series_by_slot.values().next().map_or(0, Vec::len),
            path: path.clone(),
            readback_path: path,
            matrix,
            max_off_diagonal,
            mean_off_diagonal,
        })
    }
}

pub fn compute_pairwise_mi_matrix(
    series_by_slot: &BTreeMap<String, Vec<f32>>,
) -> Result<PairwiseMiMatrix, UtmlError> {
    non_empty("series_by_slot", &series_by_slot.keys().collect::<Vec<_>>())?;
    let slots = series_by_slot.keys().cloned().collect::<Vec<_>>();
    let sample_count = series_by_slot
        .values()
        .next()
        .ok_or_else(|| UtmlError::new(UtmlErrorCode::EmptyInput, "series_by_slot is empty"))?
        .len();
    if sample_count < 2 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "pairwise MI matrix requires at least two samples",
        ));
    }
    for (slot, series) in series_by_slot {
        if series.len() != sample_count {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "slot {slot} has {} samples; expected {sample_count}",
                    series.len()
                ),
            ));
        }
        for (idx, value) in series.iter().enumerate() {
            validate_finite(&format!("{slot}[{idx}]"), *value)?;
        }
    }
    let mut values = vec![vec![0.0; slots.len()]; slots.len()];
    for row in 0..slots.len() {
        values[row][row] = 1.0;
        for col in (row + 1)..slots.len() {
            let left = series_by_slot.get(&slots[row]).ok_or_else(|| {
                UtmlError::new(
                    UtmlErrorCode::MissingSourceOfTruth,
                    format!("pairwise MI missing slot {}", slots[row]),
                )
            })?;
            let right = series_by_slot.get(&slots[col]).ok_or_else(|| {
                UtmlError::new(
                    UtmlErrorCode::MissingSourceOfTruth,
                    format!("pairwise MI missing slot {}", slots[col]),
                )
            })?;
            let mi = normalized_mutual_information(left, right)?;
            values[row][col] = mi;
            values[col][row] = mi;
        }
    }
    let matrix = PairwiseMiMatrix { slots, values };
    matrix.validate()?;
    Ok(matrix)
}

pub fn load_pairwise_mi_matrix_csv(path: impl AsRef<Path>) -> Result<PairwiseMiMatrix, UtmlError> {
    let text = fs::read_to_string(path.as_ref()).map_err(|err| {
        UtmlError::new(
            UtmlErrorCode::Io,
            format!(
                "failed to read pairwise MI CSV {}: {err}",
                path.as_ref().display()
            ),
        )
    })?;
    let mut lines = text.lines();
    let header = lines.next().ok_or_else(|| {
        UtmlError::new(
            UtmlErrorCode::MissingSourceOfTruth,
            "pairwise MI CSV is empty",
        )
    })?;
    let header_cols = header.split(',').collect::<Vec<_>>();
    if header_cols.len() < 2 || header_cols[0] != "slot" {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "pairwise MI CSV header must start with slot",
        ));
    }
    let slots = header_cols[1..]
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    let mut row_slots = Vec::new();
    let mut values = Vec::new();
    for (line_idx, line) in lines.enumerate() {
        let cols = line.split(',').collect::<Vec<_>>();
        if cols.len() != slots.len() + 1 {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "pairwise MI CSV line {} has {} columns; expected {}",
                    line_idx + 2,
                    cols.len(),
                    slots.len() + 1
                ),
            ));
        }
        row_slots.push(cols[0].to_string());
        let mut row = Vec::with_capacity(slots.len());
        for raw in &cols[1..] {
            let parsed = raw.parse::<f32>().map_err(|err| {
                UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    format!("invalid pairwise MI value {raw}: {err}"),
                )
            })?;
            row.push(parsed);
        }
        values.push(row);
    }
    if row_slots != slots {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "pairwise MI CSV row labels must match header slots",
        ));
    }
    let matrix = PairwiseMiMatrix { slots, values };
    matrix.validate()?;
    Ok(matrix)
}

fn write_matrix_csv_0600(path: &Path, matrix: &PairwiseMiMatrix) -> Result<(), UtmlError> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|err| {
            UtmlError::new(
                UtmlErrorCode::Io,
                format!("failed to open pairwise MI CSV {}: {err}", path.display()),
            )
        })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|err| {
                UtmlError::new(
                    UtmlErrorCode::Io,
                    format!("failed to chmod 0600 {}: {err}", path.display()),
                )
            })?;
    }
    writeln!(file, "slot,{}", matrix.slots.join(",")).map_err(csv_write_err)?;
    for (slot, row) in matrix.slots.iter().zip(&matrix.values) {
        let cells = row
            .iter()
            .map(|value| format!("{value:.9}"))
            .collect::<Vec<_>>()
            .join(",");
        writeln!(file, "{slot},{cells}").map_err(csv_write_err)?;
    }
    file.sync_all().map_err(|err| {
        UtmlError::new(
            UtmlErrorCode::Io,
            format!("failed to fsync pairwise MI CSV {}: {err}", path.display()),
        )
    })?;
    Ok(())
}

fn normalized_mutual_information(a: &[f32], b: &[f32]) -> Result<f32, UtmlError> {
    if a.len() != b.len() || a.len() < 2 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            "normalized MI requires same length vectors with at least two samples",
        ));
    }
    let a_bins = bin_series(a)?;
    let b_bins = bin_series(b)?;
    let mut joint = [[0usize; MI_BINS]; MI_BINS];
    let mut a_counts = [0usize; MI_BINS];
    let mut b_counts = [0usize; MI_BINS];
    for (&ai, &bi) in a_bins.iter().zip(&b_bins) {
        joint[ai][bi] += 1;
        a_counts[ai] += 1;
        b_counts[bi] += 1;
    }
    let total = a.len() as f32;
    let mut mi = 0.0f32;
    for ai in 0..MI_BINS {
        for (bi, _) in b_counts.iter().enumerate().take(MI_BINS) {
            let joint_count = joint[ai][bi];
            if joint_count == 0 {
                continue;
            }
            let pxy = joint_count as f32 / total;
            let px = a_counts[ai] as f32 / total;
            let py = b_counts[bi] as f32 / total;
            mi += pxy * (pxy / (px * py)).ln();
        }
    }
    let ha = entropy_nats(&a_counts, total);
    let hb = entropy_nats(&b_counts, total);
    let denom = ha.min(hb);
    if denom <= f32::EPSILON {
        return Ok(0.0);
    }
    Ok((mi / denom).clamp(0.0, 1.0))
}

fn bin_series(values: &[f32]) -> Result<Vec<usize>, UtmlError> {
    for (idx, value) in values.iter().enumerate() {
        validate_finite(&format!("mi_input[{idx}]"), *value)?;
    }
    let min = values.iter().copied().fold(f32::INFINITY, f32::min);
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if (max - min).abs() <= f32::EPSILON {
        return Ok(vec![0; values.len()]);
    }
    Ok(values
        .iter()
        .map(|value| {
            let scaled = ((*value - min) / (max - min)).clamp(0.0, 1.0);
            ((scaled * (MI_BINS as f32 - 1.0)).round() as usize).min(MI_BINS - 1)
        })
        .collect())
}

fn entropy_nats(counts: &[usize; MI_BINS], total: f32) -> f32 {
    counts
        .iter()
        .filter(|&&count| count > 0)
        .map(|&count| {
            let p = count as f32 / total;
            -p * p.ln()
        })
        .sum()
}

fn mean_slot(values: &[f32], slot: &str) -> Result<f32, UtmlError> {
    non_empty(slot, values)?;
    let mut sum = 0.0f32;
    for (idx, value) in values.iter().enumerate() {
        validate_finite(&format!("{slot}[{idx}]"), *value)?;
        sum += value;
    }
    Ok(sum / values.len() as f32)
}

fn off_diagonal_stats(matrix: &PairwiseMiMatrix) -> (f32, f32) {
    let mut sum = 0.0f32;
    let mut max_value = 0.0f32;
    let mut count = 0usize;
    for (row_idx, row) in matrix.values.iter().enumerate() {
        for (col_idx, value) in row.iter().enumerate() {
            if row_idx != col_idx {
                sum += *value;
                max_value = max_value.max(*value);
                count += 1;
            }
        }
    }
    (if count == 0 { 0.0 } else { sum / count as f32 }, max_value)
}

fn matrix_values_close(left: &[Vec<f32>], right: &[Vec<f32>]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter().zip(right).all(|(left_row, right_row)| {
        left_row.len() == right_row.len()
            && left_row
                .iter()
                .zip(right_row)
                .all(|(left, right)| (*left - *right).abs() <= 1.0e-6)
    })
}

fn csv_write_err(err: std::io::Error) -> UtmlError {
    UtmlError::new(
        UtmlErrorCode::Io,
        format!("failed to write pairwise MI CSV: {err}"),
    )
}
