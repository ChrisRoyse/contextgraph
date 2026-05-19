// TASK-PY-G-012: Q4 confidence calibration and fail-closed thresholds.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use context_graph_mejepa_cf::CF_MEJEPA_HEAD_CALIBRATIONS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{InstrumentError, InstrumentResult};

pub const Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION: u32 = 1;
pub const Q4_CONFIDENCE_MIN_LABELS: usize = 100;
pub const Q4_CONFIDENCE_HOLDOUT_FRACTION: f64 = 0.15;
pub const Q4_CONFIDENCE_TARGET_ECE: f64 = 0.05;
pub const Q4_CONFIDENCE_TARGET_PRECISION: f64 = 0.80;
pub const Q4_CONFIDENCE_TARGET_COVERAGE: f64 = 0.90;
pub const Q4_CONFIDENCE_COVERAGE_LOW: f64 = 0.88;
pub const Q4_CONFIDENCE_COVERAGE_HIGH: f64 = 0.92;
pub const Q4_CONFIDENCE_IMBALANCE_RATE: f64 = 0.05;
const MAX_EXAMPLES: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4CalibrationHead {
    EdgeCase,
    LatentBug,
    TechDebt,
    DeadCode,
    Redundancy,
    Perf,
    Security,
    Accuracy,
    Cost,
    Reasoning,
    NonTrivialOnPass,
}

impl Q4CalibrationHead {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EdgeCase => "edge_case",
            Self::LatentBug => "latent_bug",
            Self::TechDebt => "tech_debt",
            Self::DeadCode => "dead_code",
            Self::Redundancy => "redundancy",
            Self::Perf => "perf",
            Self::Security => "security",
            Self::Accuracy => "accuracy",
            Self::Cost => "cost",
            Self::Reasoning => "reasoning",
            Self::NonTrivialOnPass => "non_trivial_on_pass",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4CalibrationStatus {
    Calibrated,
    TemperatureFallback,
    InsufficientLabelsForCalibration,
    StaleLabels,
    BelowQualityGate,
}

impl Q4CalibrationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Calibrated => "calibrated",
            Self::TemperatureFallback => "temperature_fallback",
            Self::InsufficientLabelsForCalibration => "insufficient_labels_for_calibration",
            Self::StaleLabels => "q4_calibration_stale_labels",
            Self::BelowQualityGate => "below_quality_gate",
        }
    }

    pub fn is_supported(self) -> bool {
        matches!(self, Self::Calibrated | Self::TemperatureFallback)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4CalibrationMethod {
    Raw,
    Platt,
    TemperatureScaling,
    Isotonic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CalibrationExample {
    pub row_id: String,
    pub cell: String,
    pub head: Q4CalibrationHead,
    pub class_name: String,
    pub raw_confidence: f64,
    pub actual: bool,
    pub label_schema_version: u32,
    pub source_artifact_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4CalibrationCellMetric {
    pub cell: String,
    pub label_count: usize,
    pub positive_rate: f64,
    pub brier: f64,
    pub ece: f64,
    pub empirical_coverage: f64,
    pub ece_retrain_flag: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4HeadCalibrationReport {
    pub schema_version: u32,
    pub head: Q4CalibrationHead,
    pub class_name: String,
    pub status: Q4CalibrationStatus,
    pub label_count: usize,
    pub train_count: usize,
    pub holdout_count: usize,
    pub positive_rate: f64,
    pub severe_class_imbalance: bool,
    pub selected_method: Option<Q4CalibrationMethod>,
    pub overfit_rejected_method: Option<Q4CalibrationMethod>,
    pub brier: Option<f64>,
    pub ece: Option<f64>,
    pub train_ece: Option<f64>,
    pub empirical_coverage: Option<f64>,
    pub conformal_radius: Option<f64>,
    pub tau: Option<f64>,
    pub precision_at_tau: Option<f64>,
    pub target_ece: f64,
    pub target_precision: f64,
    pub target_coverage: f64,
    pub per_cell: Vec<Q4CalibrationCellMetric>,
    pub fail_closed_reason: Option<String>,
    pub source_artifact_sha256: String,
}

impl Q4HeadCalibrationReport {
    pub fn trust_supported(&self) -> bool {
        self.status.is_supported()
            && self.ece.is_some_and(|ece| ece <= Q4_CONFIDENCE_TARGET_ECE)
            && self
                .precision_at_tau
                .is_some_and(|p| p >= Q4_CONFIDENCE_TARGET_PRECISION)
            && self.empirical_coverage.is_some_and(|coverage| {
                (Q4_CONFIDENCE_COVERAGE_LOW..=Q4_CONFIDENCE_COVERAGE_HIGH).contains(&coverage)
            })
            && self.per_cell.iter().all(|cell| !cell.ece_retrain_flag)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4HeadCalibration {
    pub schema_version: u32,
    pub report: Q4HeadCalibrationReport,
}

pub fn calibrate_q4_binary_class(
    head: Q4CalibrationHead,
    class_name: &str,
    examples: &[Q4CalibrationExample],
    expected_label_schema_version: u32,
) -> InstrumentResult<Q4HeadCalibrationReport> {
    validate_class_name(class_name)?;
    if examples.len() > MAX_EXAMPLES {
        return invalid(
            "q4_calibration.examples",
            format!("{} examples exceeds max {MAX_EXAMPLES}", examples.len()),
            "shard Q4 confidence calibration inputs",
        );
    }
    for example in examples {
        validate_example(example)?;
        if example.head != head || example.class_name != class_name {
            return invalid(
                "q4_calibration.identity",
                "example head/class does not match requested calibration",
                "partition Q4 calibration examples by head and class before fitting",
            );
        }
    }

    let source_artifact_sha256 = source_hash(examples);
    let label_count = examples.len();
    let positive_rate = ratio(
        examples.iter().filter(|example| example.actual).count(),
        label_count,
    );
    let severe_class_imbalance = label_count > 0
        && !(Q4_CONFIDENCE_IMBALANCE_RATE..=1.0 - Q4_CONFIDENCE_IMBALANCE_RATE)
            .contains(&positive_rate);

    if examples
        .iter()
        .any(|example| example.label_schema_version != expected_label_schema_version)
    {
        return base_report(
            head,
            class_name,
            examples,
            Q4CalibrationStatus::StaleLabels,
            Some(format!(
                "Q4_CALIBRATION_STALE_LABELS: expected schema {expected_label_schema_version}"
            )),
            source_artifact_sha256,
            positive_rate,
            severe_class_imbalance,
        );
    }

    if label_count < Q4_CONFIDENCE_MIN_LABELS {
        return base_report(
            head,
            class_name,
            examples,
            Q4CalibrationStatus::InsufficientLabelsForCalibration,
            Some(format!(
                "INSUFFICIENT_LABELS_FOR_CALIBRATION: rows {label_count} < {Q4_CONFIDENCE_MIN_LABELS}"
            )),
            source_artifact_sha256,
            positive_rate,
            severe_class_imbalance,
        );
    }

    let (train, holdout) = split_train_holdout(examples);
    let candidates = fit_candidates(&train);
    let mut selected = candidates
        .iter()
        .min_by(|left, right| {
            metrics_for(left, &holdout)
                .brier
                .total_cmp(&metrics_for(right, &holdout).brier)
        })
        .ok_or_else(|| {
            InstrumentError::invalid(
                "q4_calibration.candidates",
                "no calibration candidates were generated",
                "provide non-empty Q4 calibration labels",
            )
        })?
        .clone();

    let mut selected_train = metrics_for(&selected, &train);
    let mut selected_holdout = metrics_for(&selected, &holdout);
    let mut overfit_rejected_method = None;
    if selected.method != Q4CalibrationMethod::TemperatureScaling
        && selected_holdout.ece > (selected_train.ece * 2.0).max(0.02)
    {
        overfit_rejected_method = Some(selected.method);
        selected = candidates
            .iter()
            .filter(|candidate| candidate.method == Q4CalibrationMethod::TemperatureScaling)
            .min_by(|left, right| {
                metrics_for(left, &holdout)
                    .brier
                    .total_cmp(&metrics_for(right, &holdout).brier)
            })
            .cloned()
            .ok_or_else(|| {
                InstrumentError::invalid(
                    "q4_calibration.temperature_candidates",
                    "temperature fallback candidate missing",
                    "keep temperature scaling in the Q4 calibration candidate set",
                )
            })?;
        selected_train = metrics_for(&selected, &train);
        selected_holdout = metrics_for(&selected, &holdout);
    }

    let tau = choose_tau(&selected, &holdout).unwrap_or(TauSelection {
        tau: 1.0,
        precision: 0.0,
    });
    let conformal_radius = conformal_radius(&selected, &holdout);
    let coverage = nominal_empirical_coverage(holdout.len());
    let per_cell = per_cell_metrics(&selected, &holdout);
    let quality_ok = selected_holdout.ece <= Q4_CONFIDENCE_TARGET_ECE
        && tau.precision >= Q4_CONFIDENCE_TARGET_PRECISION
        && (Q4_CONFIDENCE_COVERAGE_LOW..=Q4_CONFIDENCE_COVERAGE_HIGH).contains(&coverage)
        && per_cell.iter().all(|cell| !cell.ece_retrain_flag);
    let status = if !quality_ok {
        Q4CalibrationStatus::BelowQualityGate
    } else if overfit_rejected_method.is_some() {
        Q4CalibrationStatus::TemperatureFallback
    } else {
        Q4CalibrationStatus::Calibrated
    };
    let fail_closed_reason = if quality_ok {
        None
    } else {
        Some("Q4 calibration quality gate failed; head must emit Unknown".to_string())
    };

    let report = Q4HeadCalibrationReport {
        schema_version: Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION,
        head,
        class_name: class_name.to_string(),
        status,
        label_count,
        train_count: train.len(),
        holdout_count: holdout.len(),
        positive_rate,
        severe_class_imbalance,
        selected_method: Some(selected.method),
        overfit_rejected_method,
        brier: Some(selected_holdout.brier),
        ece: Some(selected_holdout.ece),
        train_ece: Some(selected_train.ece),
        empirical_coverage: Some(coverage),
        conformal_radius: Some(conformal_radius),
        tau: Some(tau.tau),
        precision_at_tau: Some(tau.precision),
        target_ece: Q4_CONFIDENCE_TARGET_ECE,
        target_precision: Q4_CONFIDENCE_TARGET_PRECISION,
        target_coverage: Q4_CONFIDENCE_TARGET_COVERAGE,
        per_cell,
        fail_closed_reason,
        source_artifact_sha256,
    };
    validate_report(&report)?;
    Ok(report)
}

pub fn q4_head_calibration_key(head: Q4CalibrationHead, class_name: &str) -> String {
    format!("q4cal::{}::{}", head.as_str(), normalized_key(class_name))
}

pub struct Q4HeadCalibrationStore {
    db: DB,
}

impl Q4HeadCalibrationStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_HEAD_CALIBRATIONS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        Ok(Self { db })
    }

    pub fn put_reports(
        &self,
        reports: &[Q4HeadCalibrationReport],
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for report in reports {
            validate_report(report)?;
            let record = PersistedQ4HeadCalibration {
                schema_version: Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION,
                report: report.clone(),
            };
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_reports(&self) -> InstrumentResult<Vec<(String, PersistedQ4HeadCalibration)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_HEAD_CALIBRATIONS,
                    err.to_string(),
                    "inspect iterator state for Q4 head calibrations",
                )
            })?;
            let key = decode_key(&key)?;
            if !key.starts_with("q4cal::") {
                continue;
            }
            rows.push((
                key,
                serde_json::from_slice(&value).map_err(|err| {
                    InstrumentError::store(
                        "deserialize",
                        CF_MEJEPA_HEAD_CALIBRATIONS,
                        err.to_string(),
                        "only mutate Q4 head calibration rows through Q4HeadCalibrationStore",
                    )
                })?,
            ));
        }
        Ok(rows)
    }

    pub fn get_report(
        &self,
        head: Q4CalibrationHead,
        class_name: &str,
    ) -> InstrumentResult<Option<PersistedQ4HeadCalibration>> {
        validate_class_name(class_name)?;
        let key = q4_head_calibration_key(head, class_name);
        let Some(value) = self.db.get_cf(self.cf()?, key.as_bytes()).map_err(|err| {
            InstrumentError::store(
                "get_report",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "read Q4 head calibration by canonical head/class key",
            )
        })?
        else {
            return Ok(None);
        };
        serde_json::from_slice(&value).map(Some).map_err(|err| {
            InstrumentError::store(
                "deserialize_report",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "only mutate Q4 head calibration rows through Q4HeadCalibrationStore",
            )
        })
    }

    pub fn count_reports(&self) -> InstrumentResult<usize> {
        Ok(self.scan_reports()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedQ4HeadCalibration) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "ensure Q4 head calibration rows remain JSON-serializable",
            )
        })?;
        let key = q4_head_calibration_key(record.report.head, &record.report.class_name);
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_HEAD_CALIBRATIONS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self.db.get_cf(self.cf()?, key.as_bytes()).map_err(|err| {
            InstrumentError::store(
                "get_cf",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                err.to_string(),
                "inspect RocksDB read permissions and column-family health",
            )
        })?;
        if readback.as_deref() != Some(value.as_slice()) {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_HEAD_CALIBRATIONS,
                "Q4 head calibration row missing or changed after put_cf",
                "do not advance Q4 calibration until the row is readable",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_HEAD_CALIBRATIONS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_HEAD_CALIBRATIONS,
                    "column-family handle not found",
                    "open the store through Q4HeadCalibrationStore::open",
                )
            })
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    method: Q4CalibrationMethod,
    platt_a: f64,
    platt_b: f64,
    temperature: f64,
    isotonic_bins: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct CandidateMetrics {
    brier: f64,
    ece: f64,
}

#[derive(Debug, Clone, Copy)]
struct TauSelection {
    tau: f64,
    precision: f64,
}

fn base_report(
    head: Q4CalibrationHead,
    class_name: &str,
    examples: &[Q4CalibrationExample],
    status: Q4CalibrationStatus,
    fail_closed_reason: Option<String>,
    source_artifact_sha256: String,
    positive_rate: f64,
    severe_class_imbalance: bool,
) -> InstrumentResult<Q4HeadCalibrationReport> {
    let report = Q4HeadCalibrationReport {
        schema_version: Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION,
        head,
        class_name: class_name.to_string(),
        status,
        label_count: examples.len(),
        train_count: 0,
        holdout_count: 0,
        positive_rate,
        severe_class_imbalance,
        selected_method: None,
        overfit_rejected_method: None,
        brier: None,
        ece: None,
        train_ece: None,
        empirical_coverage: None,
        conformal_radius: None,
        tau: None,
        precision_at_tau: None,
        target_ece: Q4_CONFIDENCE_TARGET_ECE,
        target_precision: Q4_CONFIDENCE_TARGET_PRECISION,
        target_coverage: Q4_CONFIDENCE_TARGET_COVERAGE,
        per_cell: Vec::new(),
        fail_closed_reason,
        source_artifact_sha256,
    };
    validate_report(&report)?;
    Ok(report)
}

fn split_train_holdout(
    examples: &[Q4CalibrationExample],
) -> (Vec<Q4CalibrationExample>, Vec<Q4CalibrationExample>) {
    let holdout_count = ((examples.len() as f64) * Q4_CONFIDENCE_HOLDOUT_FRACTION)
        .ceil()
        .max(1.0) as usize;
    let train_count = examples.len().saturating_sub(holdout_count).max(1);
    let split = train_count.min(examples.len());
    (examples[..split].to_vec(), examples[split..].to_vec())
}

fn fit_candidates(train: &[Q4CalibrationExample]) -> Vec<Candidate> {
    let mut candidates = vec![
        Candidate {
            method: Q4CalibrationMethod::Raw,
            platt_a: 1.0,
            platt_b: 0.0,
            temperature: 1.0,
            isotonic_bins: Vec::new(),
        },
        Candidate {
            method: Q4CalibrationMethod::Isotonic,
            platt_a: 1.0,
            platt_b: 0.0,
            temperature: 1.0,
            isotonic_bins: fit_isotonic_bins(train),
        },
    ];

    let platt = [0.5, 0.75, 1.0, 1.5, 2.0, 3.0]
        .into_iter()
        .flat_map(|a| [-1.0, -0.5, 0.0, 0.5, 1.0].into_iter().map(move |b| (a, b)))
        .map(|(a, b)| Candidate {
            method: Q4CalibrationMethod::Platt,
            platt_a: a,
            platt_b: b,
            temperature: 1.0,
            isotonic_bins: Vec::new(),
        })
        .min_by(|left, right| {
            metrics_for(left, train)
                .brier
                .total_cmp(&metrics_for(right, train).brier)
        })
        .expect("platt grid is non-empty");
    candidates.push(platt);

    for temperature in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
        candidates.push(Candidate {
            method: Q4CalibrationMethod::TemperatureScaling,
            platt_a: 1.0,
            platt_b: 0.0,
            temperature,
            isotonic_bins: Vec::new(),
        });
    }
    candidates
}

fn fit_isotonic_bins(train: &[Q4CalibrationExample]) -> Vec<f64> {
    let mut positives = [0.0f64; 10];
    let mut counts = [0.0f64; 10];
    for example in train {
        let bin = confidence_bin(example.raw_confidence);
        counts[bin] += 1.0;
        if example.actual {
            positives[bin] += 1.0;
        }
    }
    positives
        .into_iter()
        .zip(counts)
        .enumerate()
        .map(|(idx, (positive, count))| {
            if count > 0.0 {
                (positive + 1.0) / (count + 2.0)
            } else {
                (idx as f64 + 0.5) / 10.0
            }
        })
        .collect()
}

fn metrics_for(candidate: &Candidate, examples: &[Q4CalibrationExample]) -> CandidateMetrics {
    CandidateMetrics {
        brier: brier(candidate, examples),
        ece: ece(candidate, examples),
    }
}

fn brier(candidate: &Candidate, examples: &[Q4CalibrationExample]) -> f64 {
    if examples.is_empty() {
        return 0.0;
    }
    examples
        .iter()
        .map(|example| {
            let p = calibrate(candidate, example.raw_confidence);
            let y = if example.actual { 1.0 } else { 0.0 };
            let delta = p - y;
            delta * delta
        })
        .sum::<f64>()
        / examples.len() as f64
}

fn ece(candidate: &Candidate, examples: &[Q4CalibrationExample]) -> f64 {
    if examples.is_empty() {
        return 0.0;
    }
    let mut conf_sum = [0.0f64; 10];
    let mut actual_sum = [0.0f64; 10];
    let mut counts = [0.0f64; 10];
    for example in examples {
        let p = calibrate(candidate, example.raw_confidence);
        let bin = confidence_bin(p);
        conf_sum[bin] += p;
        actual_sum[bin] += if example.actual { 1.0 } else { 0.0 };
        counts[bin] += 1.0;
    }
    counts
        .into_iter()
        .enumerate()
        .filter(|(_, count)| *count > 0.0)
        .map(|(idx, count)| {
            let avg_conf = conf_sum[idx] / count;
            let avg_actual = actual_sum[idx] / count;
            (count / examples.len() as f64) * (avg_conf - avg_actual).abs()
        })
        .sum()
}

fn choose_tau(candidate: &Candidate, holdout: &[Q4CalibrationExample]) -> Option<TauSelection> {
    let mut thresholds = holdout
        .iter()
        .map(|example| calibrate(candidate, example.raw_confidence))
        .collect::<Vec<_>>();
    thresholds.sort_by(f64::total_cmp);
    thresholds.dedup_by(|left, right| (*left - *right).abs() < 1e-12);
    for tau in thresholds {
        let mut selected = 0.0;
        let mut true_positive = 0.0;
        for example in holdout {
            if calibrate(candidate, example.raw_confidence) >= tau {
                selected += 1.0;
                if example.actual {
                    true_positive += 1.0;
                }
            }
        }
        if selected > 0.0 {
            let precision = true_positive / selected;
            if precision >= Q4_CONFIDENCE_TARGET_PRECISION {
                return Some(TauSelection { tau, precision });
            }
        }
    }
    None
}

fn conformal_radius(candidate: &Candidate, holdout: &[Q4CalibrationExample]) -> f64 {
    if holdout.is_empty() {
        return 0.0;
    }
    let mut residuals = holdout
        .iter()
        .map(|example| {
            let y = if example.actual { 1.0 } else { 0.0 };
            (y - calibrate(candidate, example.raw_confidence)).abs()
        })
        .collect::<Vec<_>>();
    residuals.sort_by(f64::total_cmp);
    let idx = ((residuals.len() as f64) * Q4_CONFIDENCE_TARGET_COVERAGE)
        .ceil()
        .max(1.0) as usize
        - 1;
    residuals[idx.min(residuals.len() - 1)]
}

fn nominal_empirical_coverage(holdout_len: usize) -> f64 {
    if holdout_len == 0 {
        return 0.0;
    }
    let rank = (((holdout_len + 1) as f64) * Q4_CONFIDENCE_TARGET_COVERAGE)
        .ceil()
        .min(holdout_len as f64);
    rank / ((holdout_len + 1) as f64)
}

fn per_cell_metrics(
    candidate: &Candidate,
    holdout: &[Q4CalibrationExample],
) -> Vec<Q4CalibrationCellMetric> {
    let mut by_cell: BTreeMap<String, Vec<Q4CalibrationExample>> = BTreeMap::new();
    for example in holdout {
        by_cell
            .entry(example.cell.clone())
            .or_default()
            .push(example.clone());
    }
    by_cell
        .into_iter()
        .map(|(cell, examples)| {
            let ece = ece(candidate, &examples);
            Q4CalibrationCellMetric {
                cell,
                label_count: examples.len(),
                positive_rate: ratio(
                    examples.iter().filter(|example| example.actual).count(),
                    examples.len(),
                ),
                brier: brier(candidate, &examples),
                ece,
                empirical_coverage: nominal_empirical_coverage(examples.len()),
                ece_retrain_flag: ece > Q4_CONFIDENCE_TARGET_ECE,
            }
        })
        .collect()
}

fn calibrate(candidate: &Candidate, raw: f64) -> f64 {
    let raw = raw.clamp(0.000001, 0.999999);
    match candidate.method {
        Q4CalibrationMethod::Raw => raw,
        Q4CalibrationMethod::Platt => sigmoid(candidate.platt_a * logit(raw) + candidate.platt_b),
        Q4CalibrationMethod::TemperatureScaling => sigmoid(logit(raw) / candidate.temperature),
        Q4CalibrationMethod::Isotonic => candidate
            .isotonic_bins
            .get(confidence_bin(raw))
            .copied()
            .unwrap_or(raw),
    }
}

fn confidence_bin(value: f64) -> usize {
    ((value.clamp(0.0, 0.999999) * 10.0).floor() as usize).min(9)
}

fn logit(value: f64) -> f64 {
    (value / (1.0 - value)).ln()
}

fn sigmoid(value: f64) -> f64 {
    1.0 / (1.0 + (-value).exp())
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn source_hash(examples: &[Q4CalibrationExample]) -> String {
    let mut values = examples
        .iter()
        .map(|example| example.source_artifact_sha256.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    values.sort();
    if values.is_empty() {
        "none".to_string()
    } else {
        format!("{:x}", Sha256::digest(values.join("|").as_bytes()))
    }
}

fn validate_report(report: &Q4HeadCalibrationReport) -> InstrumentResult<()> {
    if report.schema_version != Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION {
        return invalid(
            "q4_calibration.schema_version",
            format!(
                "expected {}, got {}",
                Q4_CONFIDENCE_CALIBRATION_SCHEMA_VERSION, report.schema_version
            ),
            "read Q4 calibration records with the matching schema",
        );
    }
    validate_class_name(&report.class_name)?;
    validate_unit("q4_calibration.positive_rate", report.positive_rate)?;
    if let Some(value) = report.brier {
        validate_unit("q4_calibration.brier", value)?;
    }
    if let Some(value) = report.ece {
        validate_unit("q4_calibration.ece", value)?;
    }
    if let Some(value) = report.train_ece {
        validate_unit("q4_calibration.train_ece", value)?;
    }
    if let Some(value) = report.empirical_coverage {
        validate_unit("q4_calibration.empirical_coverage", value)?;
    }
    if let Some(value) = report.conformal_radius {
        validate_unit("q4_calibration.conformal_radius", value)?;
    }
    if let Some(value) = report.tau {
        validate_unit("q4_calibration.tau", value)?;
    }
    if let Some(value) = report.precision_at_tau {
        validate_unit("q4_calibration.precision_at_tau", value)?;
    }
    validate_non_empty_single_line(
        "q4_calibration.source_artifact_sha256",
        &report.source_artifact_sha256,
    )?;
    for cell in &report.per_cell {
        validate_non_empty_single_line("q4_calibration.cell", &cell.cell)?;
        validate_unit("q4_calibration.cell.positive_rate", cell.positive_rate)?;
        validate_unit("q4_calibration.cell.brier", cell.brier)?;
        validate_unit("q4_calibration.cell.ece", cell.ece)?;
        validate_unit(
            "q4_calibration.cell.empirical_coverage",
            cell.empirical_coverage,
        )?;
    }
    Ok(())
}

fn validate_example(example: &Q4CalibrationExample) -> InstrumentResult<()> {
    validate_non_empty_single_line("q4_calibration.row_id", &example.row_id)?;
    validate_non_empty_single_line("q4_calibration.cell", &example.cell)?;
    validate_class_name(&example.class_name)?;
    validate_unit("q4_calibration.raw_confidence", example.raw_confidence)?;
    validate_non_empty_single_line(
        "q4_calibration.source_artifact_sha256",
        &example.source_artifact_sha256,
    )
}

fn validate_class_name(value: &str) -> InstrumentResult<()> {
    validate_non_empty_single_line("q4_calibration.class_name", value)?;
    if value.contains('/') || value.contains('\\') || value.contains("..") {
        return invalid(
            "q4_calibration.class_name",
            "class name cannot contain filesystem metacharacters",
            "normalize Q4 class names before calibration",
        );
    }
    Ok(())
}

fn validate_unit(field: &'static str, value: f64) -> InstrumentResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(
            field,
            format!("expected finite unit interval value, got {value}"),
            "calibrate Q4 metrics before persisting report",
        );
    }
    Ok(())
}

fn validate_non_empty_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() || value.contains('\n') || value.contains('\r') {
        return invalid(
            field,
            "value must be non-empty and single-line",
            "normalize Q4 calibration text before persistence",
        );
    }
    Ok(())
}

fn normalized_key(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn cf_options() -> Options {
    let mut opts = Options::default();
    opts.set_paranoid_checks(true);
    opts
}

fn decode_key(bytes: &[u8]) -> InstrumentResult<String> {
    String::from_utf8(bytes.to_vec()).map_err(|err| {
        InstrumentError::store(
            "decode_key",
            CF_MEJEPA_HEAD_CALIBRATIONS,
            err.to_string(),
            "Q4 head calibration keys must be UTF-8",
        )
    })
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

#[cfg(test)]
#[path = "q4_confidence_calibration_tests.rs"]
mod q4_confidence_calibration_tests;
