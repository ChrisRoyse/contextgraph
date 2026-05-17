//! Compression-progress readback over the ME-JEPA training certificate chain.
//!
//! The source of truth is `CF_MEJEPA_TRAIN_CERTS`.  Each new
//! `TrainingCertificate` row records the conditional description length, in
//! bits, of the frozen target panel under the predictor.  Compression progress
//! is the per-step reduction of that value.

use crate::cert::{TrainingCertificate, CF_MEJEPA_TRAIN_CERTS, MEJEPA_TRAIN_CFS};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_COMPRESSION_PROGRESS_WINDOW: u64 = 1_000;
pub const DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS: f64 = 1e-9;
const PROBABILITY_FLOOR: f64 = 1e-12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionProgressState {
    Empty,
    SingleCertificate,
    Ready,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompressionProgressMonotonicity {
    Empty,
    Indeterminate,
    NonDecreasing,
    Decreasing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompressionProgressCertEntry {
    pub step: u64,
    pub conditional_description_length_bits: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompressionProgressPoint {
    pub previous_step: u64,
    pub step: u64,
    pub previous_conditional_description_length_bits: f64,
    pub conditional_description_length_bits: f64,
    pub cp_phi_bits: f64,
    pub running_mean_cp_phi_bits: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompressionProgressRange {
    pub first_step: u64,
    pub last_step: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompressionProgressReport {
    pub schema_version: u32,
    pub window_requested: u64,
    pub epsilon_bits: f64,
    pub source_cf: String,
    pub state: CompressionProgressState,
    pub certificate_count: usize,
    pub certificate_id_range: Option<CompressionProgressRange>,
    pub entries: Vec<CompressionProgressCertEntry>,
    pub cp_phi_points: Vec<CompressionProgressPoint>,
    pub rolling_mean_cp_phi_bits: Option<f64>,
    pub latest_cp_phi_bits: Option<f64>,
    pub cumulative_description_length_delta_bits: Option<f64>,
    pub monotonicity: CompressionProgressMonotonicity,
    pub monotonicity_passed: Option<bool>,
    pub negative_cp_point_count: usize,
    pub running_mean_decrease_count: usize,
    pub ascii_sparkline: String,
    pub status_reason: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum CompressionProgressError {
    #[error("MEJEPA_CP_WINDOW_INVALID: window must be >= 1")]
    WindowInvalid,
    #[error("MEJEPA_CP_EPSILON_INVALID: epsilon_bits must be finite and non-negative")]
    EpsilonInvalid,
    #[error("MEJEPA_CP_DB_OPEN_FAILED: {path}: {message}")]
    DbOpenFailed { path: PathBuf, message: String },
    #[error("MEJEPA_CP_CF_MISSING: missing {cf}")]
    MissingCf { cf: &'static str },
    #[error("MEJEPA_CP_ITERATOR_FAILED: {0}")]
    IteratorFailed(String),
    #[error("MEJEPA_CP_CERT_DECODE_FAILED: step={step:?}: {message}")]
    CertDecodeFailed { step: Option<u64>, message: String },
    #[error("MEJEPA_CP_CERT_MISSING_BITS: step={step} lacks conditional_description_length_bits")]
    MissingBits { step: u64 },
    #[error("MEJEPA_CP_NON_FINITE: step={step} conditional_description_length_bits={bits}")]
    NonFiniteBits { step: u64, bits: f64 },
}

impl CompressionProgressError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::WindowInvalid => "MEJEPA_CP_WINDOW_INVALID",
            Self::EpsilonInvalid => "MEJEPA_CP_EPSILON_INVALID",
            Self::DbOpenFailed { .. } => "MEJEPA_CP_DB_OPEN_FAILED",
            Self::MissingCf { .. } => "MEJEPA_CP_CF_MISSING",
            Self::IteratorFailed(_) => "MEJEPA_CP_ITERATOR_FAILED",
            Self::CertDecodeFailed { .. } => "MEJEPA_CP_CERT_DECODE_FAILED",
            Self::MissingBits { .. } => "MEJEPA_CP_CERT_MISSING_BITS",
            Self::NonFiniteBits { .. } => "MEJEPA_CP_NON_FINITE",
        }
    }
}

pub fn conditional_description_length_bits_from_probability(probability: f32) -> f64 {
    let probability = f64::from(probability).clamp(PROBABILITY_FLOOR, 1.0);
    -probability.log2()
}

pub fn compression_progress_report_from_path(
    db_path: impl AsRef<Path>,
    window: u64,
    epsilon_bits: f64,
) -> Result<CompressionProgressReport, CompressionProgressError> {
    let db_path = db_path.as_ref();
    let db = open_train_rocksdb_read_only(db_path)?;
    compression_progress_report(&db, window, epsilon_bits)
}

pub fn compression_progress_report(
    db: &DB,
    window: u64,
    epsilon_bits: f64,
) -> Result<CompressionProgressReport, CompressionProgressError> {
    validate_request(window, epsilon_bits)?;
    let cf = db
        .cf_handle(CF_MEJEPA_TRAIN_CERTS)
        .ok_or(CompressionProgressError::MissingCf {
            cf: CF_MEJEPA_TRAIN_CERTS,
        })?;
    let mut entries = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::End) {
        let (key, value) =
            item.map_err(|err| CompressionProgressError::IteratorFailed(err.to_string()))?;
        let step_from_key = decode_step_key(&key);
        let cert: TrainingCertificate = serde_json::from_slice(&value).map_err(|err| {
            CompressionProgressError::CertDecodeFailed {
                step: step_from_key,
                message: err.to_string(),
            }
        })?;
        let bits = cert
            .conditional_description_length_bits
            .ok_or(CompressionProgressError::MissingBits { step: cert.step })?;
        if !bits.is_finite() || bits < 0.0 {
            return Err(CompressionProgressError::NonFiniteBits {
                step: cert.step,
                bits,
            });
        }
        entries.push(CompressionProgressCertEntry {
            step: cert.step,
            conditional_description_length_bits: bits,
        });
        if entries.len() == window as usize {
            break;
        }
    }
    entries.reverse();
    Ok(report_from_entries(window, epsilon_bits, entries))
}

pub fn render_compression_progress_weekly_section(report: &CompressionProgressReport) -> String {
    let badge = match report.monotonicity {
        CompressionProgressMonotonicity::NonDecreasing => "monotone",
        CompressionProgressMonotonicity::Decreasing => "regressing",
        CompressionProgressMonotonicity::Indeterminate => "indeterminate",
        CompressionProgressMonotonicity::Empty => "empty",
    };
    let rolling = report
        .rolling_mean_cp_phi_bits
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| "n/a".to_string());
    let latest = report
        .latest_cp_phi_bits
        .map(|value| format!("{value:.6}"))
        .unwrap_or_else(|| "n/a".to_string());
    let range = report
        .certificate_id_range
        .as_ref()
        .map(|range| format!("{}..{}", range.first_step, range.last_step))
        .unwrap_or_else(|| "n/a".to_string());
    let rows = if report.cp_phi_points.is_empty() {
        "| n/a | n/a | n/a | n/a |\n".to_string()
    } else {
        report
            .cp_phi_points
            .iter()
            .rev()
            .take(8)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|point| {
                format!(
                    "| {} | {:.6} | {:.6} | {:.6} |\n",
                    point.step,
                    point.conditional_description_length_bits,
                    point.cp_phi_bits,
                    point.running_mean_cp_phi_bits
                )
            })
            .collect::<String>()
    };
    format!(
        "## Compression Progress\n\n\
         - source_of_truth: {cf}\n\
         - certificate_range: {range}\n\
         - certificate_count: {count}\n\
         - rolling_mean_cp_phi_bits: {rolling}\n\
         - latest_cp_phi_bits: {latest}\n\
         - monotonicity_badge: {badge}\n\
         - sparkline: {sparkline}\n\n\
         | step | conditional_description_length_bits | cp_phi_bits | running_mean_cp_phi_bits |\n\
         |---:|---:|---:|---:|\n\
         {rows}",
        cf = report.source_cf,
        count = report.certificate_count,
        sparkline = if report.ascii_sparkline.is_empty() {
            "n/a"
        } else {
            &report.ascii_sparkline
        },
    )
}

fn open_train_rocksdb_read_only(path: &Path) -> Result<DB, CompressionProgressError> {
    let mut opts = Options::default();
    opts.create_if_missing(false);
    opts.create_missing_column_families(false);
    opts.set_paranoid_checks(true);
    let descriptors = MEJEPA_TRAIN_CFS
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()))
        .collect::<Vec<_>>();
    DB::open_cf_descriptors_read_only(&opts, path, descriptors, false).map_err(|err| {
        CompressionProgressError::DbOpenFailed {
            path: path.to_path_buf(),
            message: err.to_string(),
        }
    })
}

fn validate_request(window: u64, epsilon_bits: f64) -> Result<(), CompressionProgressError> {
    if window == 0 {
        return Err(CompressionProgressError::WindowInvalid);
    }
    if !epsilon_bits.is_finite() || epsilon_bits < 0.0 {
        return Err(CompressionProgressError::EpsilonInvalid);
    }
    Ok(())
}

fn decode_step_key(key: &[u8]) -> Option<u64> {
    let bytes: [u8; 8] = key.try_into().ok()?;
    Some(u64::from_be_bytes(bytes))
}

fn report_from_entries(
    window: u64,
    epsilon_bits: f64,
    entries: Vec<CompressionProgressCertEntry>,
) -> CompressionProgressReport {
    let certificate_count = entries.len();
    let certificate_id_range =
        entries
            .first()
            .zip(entries.last())
            .map(|(first, last)| CompressionProgressRange {
                first_step: first.step,
                last_step: last.step,
            });
    let mut cp_phi_points = Vec::new();
    let mut running_sum = 0.0;
    let mut previous_running_mean: Option<f64> = None;
    let mut negative_cp_point_count = 0usize;
    let mut running_mean_decrease_count = 0usize;
    for pair in entries.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        let cp_phi_bits = previous.conditional_description_length_bits
            - current.conditional_description_length_bits;
        if cp_phi_bits < -epsilon_bits {
            negative_cp_point_count += 1;
        }
        running_sum += cp_phi_bits;
        let running_mean = running_sum / (cp_phi_points.len() as f64 + 1.0);
        if let Some(previous_mean) = previous_running_mean {
            if running_mean + epsilon_bits < previous_mean {
                running_mean_decrease_count += 1;
            }
        }
        previous_running_mean = Some(running_mean);
        cp_phi_points.push(CompressionProgressPoint {
            previous_step: previous.step,
            step: current.step,
            previous_conditional_description_length_bits: previous
                .conditional_description_length_bits,
            conditional_description_length_bits: current.conditional_description_length_bits,
            cp_phi_bits,
            running_mean_cp_phi_bits: running_mean,
        });
    }
    let rolling_mean_cp_phi_bits = if cp_phi_points.is_empty() {
        None
    } else {
        Some(running_sum / cp_phi_points.len() as f64)
    };
    let latest_cp_phi_bits = cp_phi_points.last().map(|point| point.cp_phi_bits);
    let cumulative_description_length_delta_bits =
        entries
            .first()
            .zip(entries.last())
            .and_then(|(first, last)| {
                if first.step == last.step {
                    None
                } else {
                    Some(
                        first.conditional_description_length_bits
                            - last.conditional_description_length_bits,
                    )
                }
            });
    let state = match certificate_count {
        0 => CompressionProgressState::Empty,
        1 => CompressionProgressState::SingleCertificate,
        _ => CompressionProgressState::Ready,
    };
    let monotonicity = match state {
        CompressionProgressState::Empty => CompressionProgressMonotonicity::Empty,
        CompressionProgressState::SingleCertificate => {
            CompressionProgressMonotonicity::Indeterminate
        }
        CompressionProgressState::Ready => {
            if negative_cp_point_count == 0 && running_mean_decrease_count == 0 {
                CompressionProgressMonotonicity::NonDecreasing
            } else {
                CompressionProgressMonotonicity::Decreasing
            }
        }
    };
    let monotonicity_passed = match monotonicity {
        CompressionProgressMonotonicity::NonDecreasing => Some(true),
        CompressionProgressMonotonicity::Decreasing => Some(false),
        CompressionProgressMonotonicity::Empty | CompressionProgressMonotonicity::Indeterminate => {
            None
        }
    };
    let status_reason = match state {
        CompressionProgressState::Empty => Some("MEJEPA_NO_CERT_HISTORY".to_string()),
        CompressionProgressState::SingleCertificate => {
            Some("MEJEPA_CP_SINGLE_CERTIFICATE_WINDOW".to_string())
        }
        CompressionProgressState::Ready => None,
    };
    let ascii_sparkline = ascii_sparkline(
        &cp_phi_points
            .iter()
            .map(|point| point.cp_phi_bits)
            .collect::<Vec<_>>(),
    );
    CompressionProgressReport {
        schema_version: 1,
        window_requested: window,
        epsilon_bits,
        source_cf: CF_MEJEPA_TRAIN_CERTS.to_string(),
        state,
        certificate_count,
        certificate_id_range,
        entries,
        cp_phi_points,
        rolling_mean_cp_phi_bits,
        latest_cp_phi_bits,
        cumulative_description_length_delta_bits,
        monotonicity,
        monotonicity_passed,
        negative_cp_point_count,
        running_mean_decrease_count,
        ascii_sparkline,
        status_reason,
    }
}

fn ascii_sparkline(values: &[f64]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let levels = ['.', ':', '-', '=', '+', '*', '#', '@'];
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() <= f64::EPSILON {
        return "=".repeat(values.len());
    }
    values
        .iter()
        .map(|value| {
            let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
            let idx = (normalized * (levels.len() as f64 - 1.0)).round() as usize;
            levels[idx]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_marks_monotone_progress() {
        let report = report_from_entries(
            4,
            DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
            vec![entry(0, 8.0), entry(1, 6.0), entry(2, 4.0), entry(3, 2.0)],
        );
        assert_eq!(
            report.monotonicity,
            CompressionProgressMonotonicity::NonDecreasing
        );
        assert_eq!(report.rolling_mean_cp_phi_bits, Some(2.0));
        assert_eq!(report.monotonicity_passed, Some(true));
    }

    #[test]
    fn report_marks_regression() {
        let report = report_from_entries(
            3,
            DEFAULT_COMPRESSION_PROGRESS_EPSILON_BITS,
            vec![entry(0, 4.0), entry(1, 5.0), entry(2, 6.0)],
        );
        assert_eq!(
            report.monotonicity,
            CompressionProgressMonotonicity::Decreasing
        );
        assert_eq!(report.negative_cp_point_count, 2);
        assert_eq!(report.monotonicity_passed, Some(false));
    }

    fn entry(step: u64, bits: f64) -> CompressionProgressCertEntry {
        CompressionProgressCertEntry {
            step,
            conditional_description_length_bits: bits,
        }
    }
}
