// TASK-PY-G-051: Q4 non-trivial-on-Pass annotations and metric gate.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use context_graph_mejepa_cf::CF_MEJEPA_Q4_PASS_ANNOTATIONS;
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{InstrumentError, InstrumentResult};

pub const Q4_PASS_SCHEMA_VERSION: u32 = 1;
pub const Q4_PASS_MIN_ANNOTATIONS: usize = 100;
pub const Q4_PASS_TARGET_NONTRIVIAL_RATE: f64 = 0.60;
pub const Q4_PASS_CONFIDENCE_THRESHOLD: f64 = 0.60;
pub const Q4_PASS_REGRESSION_DROP_THRESHOLD: f64 = 0.10;
const MAX_ROWS: usize = 100_000;
const MAX_TEXT_LEN: usize = 2_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PassHeadKind {
    EdgeCase,
    TechDebt,
    DeadCode,
    Redundancy,
    Perf,
    Security,
    Accuracy,
    Cost,
    Reasoning,
}

impl Q4PassHeadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EdgeCase => "edge_case",
            Self::TechDebt => "tech_debt",
            Self::DeadCode => "dead_code",
            Self::Redundancy => "redundancy",
            Self::Perf => "perf",
            Self::Security => "security",
            Self::Accuracy => "accuracy",
            Self::Cost => "cost",
            Self::Reasoning => "reasoning",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PassSeverity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PassAnnotationKind {
    Concern,
    KnownGoodNoConcern,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4PassMetricStatus {
    Pass,
    BelowThreshold,
    InsufficientPassAnnotations,
}

impl Q4PassMetricStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::BelowThreshold => "below_threshold",
            Self::InsufficientPassAnnotations => "insufficient_pass_annotations",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PassAnnotation {
    pub row_id: String,
    pub cell: String,
    pub head_kind: Q4PassHeadKind,
    pub concern: String,
    pub severity: Q4PassSeverity,
    pub justification: String,
    pub annotation_kind: Q4PassAnnotationKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PassPredictedConcern {
    pub row_id: String,
    pub cell: String,
    pub head_kind: Q4PassHeadKind,
    pub concern: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PassCellMetric {
    pub cell: String,
    pub denominator_rows: usize,
    pub matched_rows: usize,
    pub known_good_excluded_rows: usize,
    pub operator_novel_count: usize,
    pub q4_pass_nontrivial_rate: f64,
    pub passed_threshold: bool,
    pub status: Q4PassMetricStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Q4PassEvaluationReport {
    pub schema_version: u32,
    pub annotation_count: usize,
    pub denominator_rows: usize,
    pub matched_rows: usize,
    pub known_good_excluded_rows: usize,
    pub q4_pass_nontrivial_rate: f64,
    pub target_rate: f64,
    pub confidence_threshold: f64,
    pub status: Q4PassMetricStatus,
    pub per_cell: Vec<Q4PassCellMetric>,
    pub operator_novel_concerns: Vec<Q4PassPredictedConcern>,
    pub regression_alert: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PersistedQ4PassAnnotation {
    pub schema_version: u32,
    pub annotation: Q4PassAnnotation,
}

pub fn evaluate_q4_pass_nontrivial(
    annotations: &[Q4PassAnnotation],
    predictions: &[Q4PassPredictedConcern],
    previous_rate: Option<f64>,
) -> InstrumentResult<Q4PassEvaluationReport> {
    if annotations.len() > MAX_ROWS || predictions.len() > MAX_ROWS {
        return invalid(
            "q4_pass.rows",
            format!(
                "annotation/prediction rows exceed {MAX_ROWS}: annotations={}, predictions={}",
                annotations.len(),
                predictions.len()
            ),
            "shard Q4 pass annotations before evaluation",
        );
    }
    for annotation in annotations {
        validate_annotation(annotation)?;
    }
    for prediction in predictions {
        validate_prediction(prediction)?;
    }

    let sufficient_annotations = annotations.len() >= Q4_PASS_MIN_ANNOTATIONS;
    let annotated_rows = annotated_concern_rows(annotations);
    let known_good_rows = known_good_rows(annotations);
    let prediction_index = matched_prediction_index(predictions);
    let mut matched_rows = BTreeSet::new();

    for (row_id, concerns) in &annotated_rows {
        if concerns
            .iter()
            .any(|concern| prediction_index.contains(&(row_id.clone(), concern.clone())))
        {
            matched_rows.insert(row_id.clone());
        }
    }

    let operator_novel_concerns = operator_novel_concerns(annotations, predictions);
    let denominator_rows = annotated_rows.len();
    let rate = ratio(matched_rows.len(), denominator_rows);
    let per_cell = per_cell_metrics(
        annotations,
        &annotated_rows,
        &matched_rows,
        &known_good_rows,
        &operator_novel_concerns,
        sufficient_annotations,
    );

    let passed_all_cells = sufficient_annotations
        && denominator_rows > 0
        && rate >= Q4_PASS_TARGET_NONTRIVIAL_RATE
        && per_cell.iter().all(|cell| cell.passed_threshold);
    let status = if !sufficient_annotations {
        Q4PassMetricStatus::InsufficientPassAnnotations
    } else if passed_all_cells {
        Q4PassMetricStatus::Pass
    } else {
        Q4PassMetricStatus::BelowThreshold
    };
    let regression_alert = previous_rate.and_then(|previous| {
        if previous.is_finite()
            && previous > rate
            && previous - rate > Q4_PASS_REGRESSION_DROP_THRESHOLD
        {
            Some(format!(
                "Q4_PASS_NONTRIVIAL_REGRESSION: previous={previous:.6} current={rate:.6} drop={:.6} threshold={Q4_PASS_REGRESSION_DROP_THRESHOLD:.6}",
                previous - rate
            ))
        } else {
            None
        }
    });

    let report = Q4PassEvaluationReport {
        schema_version: Q4_PASS_SCHEMA_VERSION,
        annotation_count: annotations.len(),
        denominator_rows,
        matched_rows: matched_rows.len(),
        known_good_excluded_rows: known_good_rows.len(),
        q4_pass_nontrivial_rate: rate,
        target_rate: Q4_PASS_TARGET_NONTRIVIAL_RATE,
        confidence_threshold: Q4_PASS_CONFIDENCE_THRESHOLD,
        status,
        per_cell,
        operator_novel_concerns,
        regression_alert,
    };
    validate_report(&report)?;
    Ok(report)
}

pub fn write_q4_pass_weekly_markdown(
    path: impl AsRef<Path>,
    report: &Q4PassEvaluationReport,
) -> InstrumentResult<()> {
    validate_report(report)?;
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            InstrumentError::store(
                "create_dir_all",
                "q4_pass_weekly_report",
                err.to_string(),
                "ensure the aiwonder Q4 pass report directory is writable",
            )
        })?;
    }
    let per_cell = report
        .per_cell
        .iter()
        .map(|cell| {
            format!(
                "| {} | {} | {} | {} | {:.6} | {} | {} |",
                cell.cell,
                cell.denominator_rows,
                cell.matched_rows,
                cell.known_good_excluded_rows,
                cell.q4_pass_nontrivial_rate,
                cell.passed_threshold,
                cell.status.as_str()
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let novel = if report.operator_novel_concerns.is_empty() {
        "- none".to_string()
    } else {
        report
            .operator_novel_concerns
            .iter()
            .take(25)
            .map(|concern| {
                format!(
                    "- {} {} {} confidence={:.3}",
                    concern.row_id,
                    concern.cell,
                    concern.head_kind.as_str(),
                    concern.confidence
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let bytes = format!(
        "# Q4 Non-Trivial-on-Pass\n\n\
         - source_of_truth: {CF_MEJEPA_Q4_PASS_ANNOTATIONS}\n\
         - status: {}\n\
         - annotation_count: {}\n\
         - denominator_rows: {}\n\
         - matched_rows: {}\n\
         - known_good_excluded_rows: {}\n\
         - q4_pass_nontrivial_rate: {:.6}\n\
         - target_rate: {:.6}\n\
         - confidence_threshold: {:.6}\n\
         - regression_alert: {}\n\n\
         ## Per-Cell Breakdown\n\n\
         | cell | denominator | matched | known_good_excluded | rate | passed | status |\n\
         |---|---:|---:|---:|---:|---|---|\n\
         {}\n\n\
         ## Operator-Novel Concerns\n\n\
         {}\n",
        report.status.as_str(),
        report.annotation_count,
        report.denominator_rows,
        report.matched_rows,
        report.known_good_excluded_rows,
        report.q4_pass_nontrivial_rate,
        report.target_rate,
        report.confidence_threshold,
        report.regression_alert.as_deref().unwrap_or("none"),
        per_cell,
        novel
    )
    .into_bytes();
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|err| {
            InstrumentError::store(
                "open",
                "q4_pass_weekly_report",
                err.to_string(),
                "ensure the Q4 pass weekly markdown path is writable",
            )
        })?;
    file.write_all(&bytes).map_err(|err| {
        InstrumentError::store(
            "write_all",
            "q4_pass_weekly_report",
            err.to_string(),
            "retry after checking aiwonder filesystem health",
        )
    })?;
    file.sync_all().map_err(|err| {
        InstrumentError::store(
            "sync_all",
            "q4_pass_weekly_report",
            err.to_string(),
            "do not claim weekly report completion until fsync succeeds",
        )
    })?;
    let readback = fs::read(path).map_err(|err| {
        InstrumentError::store(
            "readback",
            "q4_pass_weekly_report",
            err.to_string(),
            "verify the weekly markdown path after write",
        )
    })?;
    if readback != bytes {
        return Err(InstrumentError::store(
            "read_after_write",
            "q4_pass_weekly_report",
            "weekly markdown bytes changed after write",
            "do not advance Q4 pass metric until readback matches",
        ));
    }
    Ok(())
}

pub fn q4_pass_annotation_key(row_id: &str, head_kind: Q4PassHeadKind, concern: &str) -> String {
    format!(
        "{}:{}:{}",
        row_id,
        head_kind.as_str(),
        sha256_text(&normalize_concern(concern))
    )
}

pub struct Q4PassAnnotationStore {
    db: DB,
}

impl Q4PassAnnotationStore {
    pub fn open(path: impl AsRef<Path>) -> InstrumentResult<Self> {
        let mut db_opts = Options::default();
        db_opts.create_if_missing(true);
        db_opts.create_missing_column_families(true);
        db_opts.set_paranoid_checks(true);
        let descriptors = vec![
            ColumnFamilyDescriptor::new("default", Options::default()),
            ColumnFamilyDescriptor::new(CF_MEJEPA_Q4_PASS_ANNOTATIONS, cf_options()),
        ];
        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), descriptors).map_err(|err| {
            InstrumentError::store(
                "open",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                err.to_string(),
                "inspect the RocksDB path, lock ownership, and column-family metadata",
            )
        })?;
        Ok(Self { db })
    }

    pub fn put_annotations(
        &self,
        annotations: &[Q4PassAnnotation],
    ) -> InstrumentResult<Vec<String>> {
        let mut keys = Vec::new();
        for annotation in annotations {
            validate_annotation(annotation)?;
            let record = PersistedQ4PassAnnotation {
                schema_version: Q4_PASS_SCHEMA_VERSION,
                annotation: annotation.clone(),
            };
            keys.push(self.put_record(&record)?);
        }
        Ok(keys)
    }

    pub fn scan_records(&self) -> InstrumentResult<Vec<(String, PersistedQ4PassAnnotation)>> {
        let cf = self.cf()?;
        let mut rows = Vec::new();
        for item in self.db.iterator_cf(cf, IteratorMode::Start) {
            let (key, value) = item.map_err(|err| {
                InstrumentError::store(
                    "iterate",
                    CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                    err.to_string(),
                    "inspect RocksDB iterator state and Q4 pass annotation CF health",
                )
            })?;
            rows.push((
                decode_key(&key)?,
                serde_json::from_slice(&value).map_err(|err| {
                    InstrumentError::store(
                        "deserialize",
                        CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                        err.to_string(),
                        "only mutate Q4 pass annotations through Q4PassAnnotationStore",
                    )
                })?,
            ));
        }
        Ok(rows)
    }

    pub fn get_annotation_record(
        &self,
        row_id: &str,
        head_kind: Q4PassHeadKind,
        concern: &str,
    ) -> InstrumentResult<Option<PersistedQ4PassAnnotation>> {
        validate_path_component("row_id", row_id)?;
        validate_non_empty_single_line("concern", concern)?;
        let key = q4_pass_annotation_key(row_id, head_kind, concern);
        let Some(value) = self.db.get_cf(self.cf()?, key.as_bytes()).map_err(|err| {
            InstrumentError::store(
                "get_annotation_record",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                err.to_string(),
                "read Q4 pass annotation evidence by canonical row/head/concern key",
            )
        })?
        else {
            return Ok(None);
        };
        serde_json::from_slice(&value).map(Some).map_err(|err| {
            InstrumentError::store(
                "deserialize_annotation_record",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                err.to_string(),
                "only mutate Q4 pass annotations through Q4PassAnnotationStore",
            )
        })
    }

    pub fn count_records(&self) -> InstrumentResult<usize> {
        Ok(self.scan_records()?.len())
    }

    pub fn flush(&self) -> InstrumentResult<()> {
        self.db.flush_cf(self.cf()?).map_err(|err| {
            InstrumentError::store(
                "flush",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                err.to_string(),
                "inspect RocksDB WAL and filesystem state",
            )
        })
    }

    fn put_record(&self, record: &PersistedQ4PassAnnotation) -> InstrumentResult<String> {
        let value = serde_json::to_vec(record).map_err(|err| {
            InstrumentError::store(
                "serialize",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                err.to_string(),
                "ensure Q4 pass annotation records remain JSON-serializable",
            )
        })?;
        let key = q4_pass_annotation_key(
            &record.annotation.row_id,
            record.annotation.head_kind,
            &record.annotation.concern,
        );
        let mut write_opts = WriteOptions::default();
        write_opts.set_sync(true);
        self.db
            .put_cf_opt(self.cf()?, key.as_bytes(), &value, &write_opts)
            .map_err(|err| {
                InstrumentError::store(
                    "put_cf",
                    CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                    err.to_string(),
                    "inspect RocksDB write permissions, WAL state, and disk capacity",
                )
            })?;
        let readback = self.db.get_cf(self.cf()?, key.as_bytes()).map_err(|err| {
            InstrumentError::store(
                "get_cf",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                err.to_string(),
                "inspect RocksDB read permissions and column-family health",
            )
        })?;
        if readback.as_deref() != Some(value.as_slice()) {
            return Err(InstrumentError::store(
                "read_after_write",
                CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                "Q4 pass annotation row missing or changed after put_cf",
                "do not advance Q4 pass annotation checkpoints until the CF row is readable",
            ));
        }
        Ok(key)
    }

    fn cf(&self) -> InstrumentResult<&rocksdb::ColumnFamily> {
        self.db
            .cf_handle(CF_MEJEPA_Q4_PASS_ANNOTATIONS)
            .ok_or_else(|| {
                InstrumentError::store(
                    "cf_handle",
                    CF_MEJEPA_Q4_PASS_ANNOTATIONS,
                    "column-family handle not found",
                    "open the store through Q4PassAnnotationStore::open",
                )
            })
    }
}

fn per_cell_metrics(
    annotations: &[Q4PassAnnotation],
    annotated_rows: &BTreeMap<String, BTreeSet<ConcernKey>>,
    matched_rows: &BTreeSet<String>,
    known_good_rows: &BTreeSet<String>,
    operator_novel: &[Q4PassPredictedConcern],
    sufficient_annotations: bool,
) -> Vec<Q4PassCellMetric> {
    let mut row_to_cell = BTreeMap::new();
    for annotation in annotations {
        row_to_cell.insert(annotation.row_id.clone(), annotation.cell.clone());
    }
    let mut denominator_by_cell: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row_id in annotated_rows.keys() {
        if let Some(cell) = row_to_cell.get(row_id) {
            denominator_by_cell
                .entry(cell.clone())
                .or_default()
                .insert(row_id.clone());
        }
    }
    let mut known_good_by_cell: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row_id in known_good_rows {
        if let Some(cell) = row_to_cell.get(row_id) {
            known_good_by_cell
                .entry(cell.clone())
                .or_default()
                .insert(row_id.clone());
        }
    }
    let mut novel_by_cell: BTreeMap<String, usize> = BTreeMap::new();
    for concern in operator_novel {
        *novel_by_cell.entry(concern.cell.clone()).or_default() += 1;
    }

    denominator_by_cell
        .into_iter()
        .map(|(cell, rows)| {
            let matched = rows
                .iter()
                .filter(|row| matched_rows.contains(*row))
                .count();
            let denominator = rows.len();
            let rate = ratio(matched, denominator);
            let passed =
                sufficient_annotations && denominator > 0 && rate >= Q4_PASS_TARGET_NONTRIVIAL_RATE;
            let status = if !sufficient_annotations {
                Q4PassMetricStatus::InsufficientPassAnnotations
            } else if passed {
                Q4PassMetricStatus::Pass
            } else {
                Q4PassMetricStatus::BelowThreshold
            };
            Q4PassCellMetric {
                cell: cell.clone(),
                denominator_rows: denominator,
                matched_rows: matched,
                known_good_excluded_rows: known_good_by_cell
                    .get(&cell)
                    .map(BTreeSet::len)
                    .unwrap_or_default(),
                operator_novel_count: novel_by_cell.get(&cell).copied().unwrap_or_default(),
                q4_pass_nontrivial_rate: rate,
                passed_threshold: passed,
                status,
            }
        })
        .collect()
}

fn annotated_concern_rows(
    annotations: &[Q4PassAnnotation],
) -> BTreeMap<String, BTreeSet<ConcernKey>> {
    let mut rows: BTreeMap<String, BTreeSet<ConcernKey>> = BTreeMap::new();
    for annotation in annotations {
        if annotation.annotation_kind == Q4PassAnnotationKind::Concern {
            rows.entry(annotation.row_id.clone())
                .or_default()
                .insert(ConcernKey::from_annotation(annotation));
        }
    }
    rows
}

fn known_good_rows(annotations: &[Q4PassAnnotation]) -> BTreeSet<String> {
    annotations
        .iter()
        .filter(|annotation| annotation.annotation_kind == Q4PassAnnotationKind::KnownGoodNoConcern)
        .map(|annotation| annotation.row_id.clone())
        .collect()
}

fn matched_prediction_index(
    predictions: &[Q4PassPredictedConcern],
) -> BTreeSet<(String, ConcernKey)> {
    predictions
        .iter()
        .filter(|prediction| prediction.confidence >= Q4_PASS_CONFIDENCE_THRESHOLD)
        .map(|prediction| {
            (
                prediction.row_id.clone(),
                ConcernKey {
                    head_kind: prediction.head_kind,
                    concern: normalize_concern(&prediction.concern),
                },
            )
        })
        .collect()
}

fn operator_novel_concerns(
    annotations: &[Q4PassAnnotation],
    predictions: &[Q4PassPredictedConcern],
) -> Vec<Q4PassPredictedConcern> {
    let known = annotations
        .iter()
        .map(ConcernKey::from_annotation)
        .collect::<BTreeSet<_>>();
    predictions
        .iter()
        .filter(|prediction| prediction.confidence >= Q4_PASS_CONFIDENCE_THRESHOLD)
        .filter(|prediction| {
            !known.contains(&ConcernKey {
                head_kind: prediction.head_kind,
                concern: normalize_concern(&prediction.concern),
            })
        })
        .take(1_000)
        .cloned()
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ConcernKey {
    head_kind: Q4PassHeadKind,
    concern: String,
}

impl ConcernKey {
    fn from_annotation(annotation: &Q4PassAnnotation) -> Self {
        Self {
            head_kind: annotation.head_kind,
            concern: normalize_concern(&annotation.concern),
        }
    }
}

fn validate_report(report: &Q4PassEvaluationReport) -> InstrumentResult<()> {
    if report.schema_version != Q4_PASS_SCHEMA_VERSION {
        return invalid(
            "q4_pass_report.schema_version",
            format!(
                "expected {}, got {}",
                Q4_PASS_SCHEMA_VERSION, report.schema_version
            ),
            "read reports with the matching TASK-PY-G-051 schema",
        );
    }
    validate_unit(
        "q4_pass_report.q4_pass_nontrivial_rate",
        report.q4_pass_nontrivial_rate,
    )?;
    validate_unit("q4_pass_report.target_rate", report.target_rate)?;
    validate_unit(
        "q4_pass_report.confidence_threshold",
        report.confidence_threshold,
    )?;
    if report.denominator_rows < report.matched_rows {
        return invalid(
            "q4_pass_report.matched_rows",
            "matched rows cannot exceed denominator rows",
            "recompute Q4 pass metrics from canonical annotations",
        );
    }
    for cell in &report.per_cell {
        validate_non_empty_single_line("q4_pass_cell.cell", &cell.cell)?;
        validate_unit(
            "q4_pass_cell.q4_pass_nontrivial_rate",
            cell.q4_pass_nontrivial_rate,
        )?;
        if cell.denominator_rows < cell.matched_rows {
            return invalid(
                "q4_pass_cell.matched_rows",
                "matched rows cannot exceed denominator rows",
                "recompute Q4 pass per-cell metrics from canonical annotations",
            );
        }
    }
    Ok(())
}

fn validate_annotation(annotation: &Q4PassAnnotation) -> InstrumentResult<()> {
    validate_path_component("q4_pass_annotation.row_id", &annotation.row_id)?;
    validate_non_empty_single_line("q4_pass_annotation.cell", &annotation.cell)?;
    validate_non_empty_single_line("q4_pass_annotation.concern", &annotation.concern)?;
    validate_non_empty_single_line(
        "q4_pass_annotation.justification",
        &annotation.justification,
    )?;
    if annotation.concern.len() > MAX_TEXT_LEN || annotation.justification.len() > MAX_TEXT_LEN {
        return invalid(
            "q4_pass_annotation.text",
            format!("concern or justification exceeds {MAX_TEXT_LEN} bytes"),
            "store long operator notes as external artifacts and keep annotations bounded",
        );
    }
    Ok(())
}

fn validate_prediction(prediction: &Q4PassPredictedConcern) -> InstrumentResult<()> {
    validate_path_component("q4_pass_prediction.row_id", &prediction.row_id)?;
    validate_non_empty_single_line("q4_pass_prediction.cell", &prediction.cell)?;
    validate_non_empty_single_line("q4_pass_prediction.concern", &prediction.concern)?;
    validate_unit("q4_pass_prediction.confidence", prediction.confidence)?;
    Ok(())
}

fn validate_unit(field: &'static str, value: f64) -> InstrumentResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(
            field,
            format!("expected finite unit interval value, got {value}"),
            "calibrate Q4 pass metrics before persisting the report",
        );
    }
    Ok(())
}

fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

fn normalize_concern(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn validate_path_component(field: &'static str, value: &str) -> InstrumentResult<()> {
    validate_non_empty_single_line(field, value)?;
    if value.contains('/') || value.contains('\\') || value.contains("..") {
        return invalid(
            field,
            "path component contains a separator or parent reference",
            "use stable row ids without filesystem metacharacters",
        );
    }
    Ok(())
}

fn validate_non_empty_single_line(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() || value.contains('\n') || value.contains('\r') {
        return invalid(
            field,
            "value must be non-empty and single-line",
            "normalize Q4 pass annotation text before persistence",
        );
    }
    Ok(())
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
            CF_MEJEPA_Q4_PASS_ANNOTATIONS,
            err.to_string(),
            "Q4 pass annotation keys must be UTF-8",
        )
    })
}

fn sha256_text(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn invalid<T>(
    field: &'static str,
    message: impl Into<String>,
    remediation: &'static str,
) -> InstrumentResult<T> {
    Err(InstrumentError::invalid(field, message, remediation))
}

#[cfg(test)]
#[path = "q4_pass_annotations_tests.rs"]
mod q4_pass_annotations_tests;
