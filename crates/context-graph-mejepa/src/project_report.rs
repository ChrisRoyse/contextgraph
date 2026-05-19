use crate::calibration::open_infer_rocksdb;
use crate::cli::write_json_0600;
use crate::error::MejepaInferError;
use crate::project_ingest::{
    is_valid_project_id, project_prediction_created_at_unix_ms, project_prediction_live_key,
    ProjectIngestManifest,
};
use crate::types::{decode_reality_prediction, RealityPrediction, Severity, Verdict};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_PROJECT_REPORTS};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const REPORT_SCHEMA_VERSION: u32 = 1;
const BROKEN_LIMIT: usize = 20;
const EDGE_LIMIT: usize = 10;
const VULNERABILITY_LIMIT: usize = 10;
const WORKS_LIMIT: usize = 20;
const COVERAGE_LIMIT: usize = 100;
const COLD_LIMIT: usize = 50;
const DEPENDENCY_LIMIT: usize = 20;
const PREDICTION_PAGE_LIMIT: usize = 100;

#[derive(Debug, Error)]
pub enum ProjectReportError {
    #[error("MEJEPA_PROJECT_REPORT_INVALID_PROJECT_ID: {project_id}")]
    InvalidProjectId { project_id: String },
    #[error("MEJEPA_PROJECT_REPORT_INGEST_REQUIRED: project_id={project_id}")]
    IngestRequired { project_id: String },
    #[error("MEJEPA_PROJECT_REPORT_MISSING_PREDICTION_ROW: project_id={project_id} file_path={file_path} key={key_hex}")]
    MissingPredictionRow {
        project_id: String,
        file_path: String,
        key_hex: String,
    },
    #[error("MEJEPA_PROJECT_REPORT_MANIFEST_PATH_MISMATCH: project_id={project_id} field={field} expected={expected} actual={actual}")]
    ManifestPathMismatch {
        project_id: String,
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("MEJEPA_PROJECT_REPORT_IO: {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("MEJEPA_PROJECT_REPORT_JSON: {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Path(#[from] context_graph_paths::PathError),
    #[error("MEJEPA_PROJECT_REPORT_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("MEJEPA_PROJECT_REPORT_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
    #[error("{0}")]
    Infer(#[from] MejepaInferError),
}

impl ProjectReportError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidProjectId { .. } => "MEJEPA_PROJECT_REPORT_INVALID_PROJECT_ID",
            Self::IngestRequired { .. } => "MEJEPA_PROJECT_REPORT_INGEST_REQUIRED",
            Self::MissingPredictionRow { .. } => "MEJEPA_PROJECT_REPORT_MISSING_PREDICTION_ROW",
            Self::ManifestPathMismatch { .. } => "MEJEPA_PROJECT_REPORT_MANIFEST_PATH_MISMATCH",
            Self::Io { .. } => "MEJEPA_PROJECT_REPORT_IO",
            Self::Json { .. } => "MEJEPA_PROJECT_REPORT_JSON",
            Self::Path(err) => err.code,
            Self::RocksDb(_) => "MEJEPA_PROJECT_REPORT_ROCKSDB",
            Self::Bincode(_) => "MEJEPA_PROJECT_REPORT_BINCODE",
            Self::Infer(err) => err.code(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReportRequest {
    pub project_id: String,
    #[serde(default)]
    pub section: Option<ProjectReportSection>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProjectReportSection {
    Predictions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReportRun {
    pub schema_version: u32,
    pub project_id: String,
    pub report_version: String,
    pub report_json_path: String,
    pub report_md_path: String,
    pub report_db_path: String,
    pub report_cf: String,
    pub report_key: String,
    pub report: ProjectRealityReport,
    pub prediction_rows_page: Option<ProjectPredictionPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectRealityReport {
    pub schema_version: u32,
    pub project_id: String,
    pub report_version: String,
    pub generated_at_unix_ms: i64,
    pub source_manifest_path: String,
    pub source_predictions_db_path: String,
    pub source_live_prediction_cf: String,
    pub source_report_cf: String,
    pub source_prediction_count: usize,
    pub warning: Option<String>,
    pub verdict_summary: VerdictSummary,
    pub top_likely_broken_areas: Vec<BrokenAreaRow>,
    pub top_edge_cases_not_exercised: Vec<EdgeCaseGapRow>,
    pub top_vulnerability_slices: Vec<VulnerabilitySliceRow>,
    pub dependency_risks: Vec<DependencyRiskRow>,
    pub predicted_works: Vec<PredictedWorksRow>,
    pub coverage_prediction_map: Vec<CoveragePredictionRow>,
    pub cold_cells: Vec<ColdCellRow>,
    pub caps: ProjectReportCaps,
    pub pagination: ProjectReportPagination,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VerdictSummary {
    pub pass: usize,
    pub fail: usize,
    pub abstain: usize,
    pub out_of_distribution: usize,
    pub guard_rejected: usize,
    pub total: usize,
    pub per_file: BTreeMap<String, Verdict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceLink {
    pub prediction_id: String,
    pub live_prediction_key_hex: String,
    pub task_id: String,
    pub file_path: String,
    pub chunk_id: Option<String>,
    pub line_range: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrokenAreaRow {
    pub rank: usize,
    pub file_path: String,
    pub chunk_id: String,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub severity: Severity,
    pub failure_class: String,
    pub root_cause_class: String,
    pub explanation: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EdgeCaseGapRow {
    pub rank: usize,
    pub file_path: String,
    pub gap_kind: String,
    pub chunk_id: String,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub description: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VulnerabilitySliceRow {
    pub rank: usize,
    pub file_path: String,
    pub chunk_id: String,
    pub line_range: (u32, u32),
    pub cvss_estimate: Option<f32>,
    pub concern_class: String,
    pub explanation: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DependencyRiskRow {
    pub rank: usize,
    pub dependency: String,
    pub cve_id: Option<String>,
    pub severity: Option<String>,
    pub explanation: String,
    pub evidence: Option<EvidenceLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PredictedWorksRow {
    pub rank: usize,
    pub file_path: String,
    pub chunk_id: String,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub evidence_strength: f32,
    pub claim: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoveragePredictionRow {
    pub rank: usize,
    pub file_path: String,
    pub chunk_id: String,
    pub line_range: (u32, u32),
    pub defect_probability: f32,
    pub confidence: f32,
    pub path_description: String,
    pub evidence_text: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColdCellRow {
    pub rank: usize,
    pub file_path: String,
    pub verdict: Verdict,
    pub ood_score: f32,
    pub calibrated_confidence: f32,
    pub reason: String,
    pub evidence: EvidenceLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReportCaps {
    pub likely_broken_areas: usize,
    pub edge_cases: usize,
    pub vulnerability_slices: usize,
    pub dependency_risks: usize,
    pub predicted_works: usize,
    pub coverage_map: usize,
    pub cold_cells: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectReportPagination {
    pub prediction_rows_total: usize,
    pub has_more_prediction_rows: bool,
    pub all_predictions_link: Option<String>,
    pub likely_broken_total_before_cap: usize,
    pub edge_cases_total_before_cap: usize,
    pub vulnerability_total_before_cap: usize,
    pub predicted_works_total_before_cap: usize,
    pub coverage_total_before_cap: usize,
    pub cold_cells_total_before_cap: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPredictionPage {
    pub section: ProjectReportSection,
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub has_more: bool,
    pub next_link: Option<String>,
    pub rows: Vec<ProjectPredictionPageRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectPredictionPageRow {
    pub index: usize,
    pub file_path: String,
    pub verdict: Verdict,
    pub prediction_id: String,
    pub live_prediction_key_hex: String,
    pub task_id: String,
    pub chunk_ids: Vec<String>,
}

#[derive(Debug)]
struct LoadedPrediction {
    file_path: String,
    live_key_hex: String,
    prediction: RealityPrediction,
}

pub fn run_project_report(
    request: ProjectReportRequest,
) -> Result<ProjectReportRun, ProjectReportError> {
    let project_id = validate_project_id(&request.project_id)?;
    let project_root = project_root(&project_id)?;
    let manifest_path = project_root.join("manifest.json");
    if !manifest_path.exists() {
        return Err(ProjectReportError::IngestRequired { project_id });
    }
    let manifest = read_manifest(&manifest_path)?;
    if manifest.project_id != project_id {
        return Err(MejepaInferError::InvalidInput {
            field: "project_manifest.project_id".to_string(),
            detail: format!(
                "manifest project_id {} does not match requested {}",
                manifest.project_id, project_id
            ),
        }
        .into());
    }
    let db_path = validate_manifest_paths(&project_id, &manifest)?;
    let db = open_infer_rocksdb(&db_path)?;
    let predictions = load_project_predictions(db.as_ref(), &manifest)?;
    let prediction_rows_page = build_prediction_page(&project_id, &request, &predictions);
    let generated_at_unix_ms = now_ms();
    let report_version = report_version(&manifest, generated_at_unix_ms);
    let report = build_report(
        &manifest,
        &manifest_path,
        &db_path,
        &report_version,
        generated_at_unix_ms,
        &predictions,
    );
    let report_dir = project_root.join("report");
    fs::create_dir_all(&report_dir).map_err(|source| ProjectReportError::Io {
        path: report_dir.display().to_string(),
        source,
    })?;
    let json_path = report_dir.join("report.json");
    let md_path = report_dir.join("report.md");
    write_json_0600(&json_path, &report)?;
    write_text_0600(&md_path, &render_markdown(&report))?;
    let report_key = project_report_key(&project_id, &report_version);
    write_report_cf(db.as_ref(), &report_key, &report)?;
    Ok(ProjectReportRun {
        schema_version: REPORT_SCHEMA_VERSION,
        project_id,
        report_version,
        report_json_path: json_path.display().to_string(),
        report_md_path: md_path.display().to_string(),
        report_db_path: db_path.display().to_string(),
        report_cf: CF_MEJEPA_PROJECT_REPORTS.to_string(),
        report_key,
        report,
        prediction_rows_page,
    })
}

pub fn project_report_key(project_id: &str, report_version: &str) -> String {
    format!("project_report:{project_id}:{report_version}")
}

pub fn count_project_reports(db: &DB, project_id: &str) -> Result<usize, ProjectReportError> {
    let cf = report_cf(db)?;
    let prefix = format!("project_report:{project_id}:");
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, _value) = item?;
        if String::from_utf8_lossy(&key).starts_with(&prefix) {
            count += 1;
        }
    }
    Ok(count)
}

pub fn read_project_report_cf(
    db: &DB,
    project_id: &str,
    report_version: &str,
) -> Result<ProjectRealityReport, ProjectReportError> {
    let key = project_report_key(project_id, report_version);
    let bytes = db.get_cf(report_cf(db)?, key.as_bytes())?.ok_or_else(|| {
        MejepaInferError::InvalidInput {
            field: "project_report.key".to_string(),
            detail: format!("missing project report row {key}"),
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ProjectReportError::Json { path: key, source })
}

pub fn project_report_db_path(project_id: &str) -> Result<PathBuf, ProjectReportError> {
    Ok(project_root(project_id)?.join("predictions/live-predictions.rocksdb"))
}

fn build_report(
    manifest: &ProjectIngestManifest,
    manifest_path: &Path,
    db_path: &Path,
    report_version: &str,
    generated_at_unix_ms: i64,
    predictions: &[LoadedPrediction],
) -> ProjectRealityReport {
    let mut summary = VerdictSummary::default();
    let mut broken = Vec::new();
    let mut edge_cases = Vec::new();
    let mut vulnerabilities = Vec::new();
    let mut predicted_works = Vec::new();
    let mut coverage = Vec::new();
    let mut cold = Vec::new();
    let files_by_chunk = chunk_file_index(predictions);

    for loaded in predictions {
        update_summary(&mut summary, loaded);
        collect_broken_areas(&mut broken, loaded, &files_by_chunk);
        collect_edge_gaps(&mut edge_cases, loaded, &files_by_chunk);
        collect_vulnerabilities(&mut vulnerabilities, loaded, &files_by_chunk);
        collect_predicted_works(&mut predicted_works, loaded, &files_by_chunk);
        collect_coverage(&mut coverage, loaded, &files_by_chunk);
        collect_cold_cells(&mut cold, loaded);
    }

    let likely_broken_total_before_cap = broken.len();
    let edge_cases_total_before_cap = edge_cases.len();
    let vulnerability_total_before_cap = vulnerabilities.len();
    let predicted_works_total_before_cap = predicted_works.len();
    let coverage_total_before_cap = coverage.len();
    let cold_cells_total_before_cap = cold.len();

    sort_and_rank(&mut broken, |row| row.confidence, BROKEN_LIMIT);
    sort_and_rank(&mut edge_cases, |row| row.confidence, EDGE_LIMIT);
    sort_vulnerabilities(&mut vulnerabilities);
    vulnerabilities.truncate(VULNERABILITY_LIMIT);
    for (idx, row) in vulnerabilities.iter_mut().enumerate() {
        row.rank = idx + 1;
    }
    sort_and_rank(
        &mut predicted_works,
        |row| row.confidence * row.evidence_strength,
        WORKS_LIMIT,
    );
    sort_and_rank(
        &mut coverage,
        |row| row.defect_probability * row.confidence,
        COVERAGE_LIMIT,
    );
    sort_and_rank(
        &mut cold,
        |row| row.ood_score.max(1.0 - row.calibrated_confidence),
        COLD_LIMIT,
    );

    let warning = if predictions.is_empty() {
        Some("No project predictions exist yet; run mejepa_project_ingest on source files before relying on this report.".to_string())
    } else {
        None
    };
    ProjectRealityReport {
        schema_version: REPORT_SCHEMA_VERSION,
        project_id: manifest.project_id.clone(),
        report_version: report_version.to_string(),
        generated_at_unix_ms,
        source_manifest_path: manifest_path.display().to_string(),
        source_predictions_db_path: db_path.display().to_string(),
        source_live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        source_report_cf: CF_MEJEPA_PROJECT_REPORTS.to_string(),
        source_prediction_count: predictions.len(),
        warning,
        verdict_summary: summary,
        pagination: ProjectReportPagination {
            prediction_rows_total: predictions.len(),
            has_more_prediction_rows: predictions.len() > PREDICTION_PAGE_LIMIT,
            all_predictions_link: (predictions.len() > PREDICTION_PAGE_LIMIT).then(|| {
                format!(
                    "mcp__cgreality__mejepa_project_report?projectId={}&section=predictions&offset={}",
                    manifest.project_id, PREDICTION_PAGE_LIMIT
                )
            }),
            likely_broken_total_before_cap,
            edge_cases_total_before_cap,
            vulnerability_total_before_cap,
            predicted_works_total_before_cap,
            coverage_total_before_cap,
            cold_cells_total_before_cap,
        },
        top_likely_broken_areas: broken,
        top_edge_cases_not_exercised: edge_cases,
        top_vulnerability_slices: vulnerabilities,
        dependency_risks: Vec::new(),
        predicted_works,
        coverage_prediction_map: coverage,
        cold_cells: cold,
        caps: ProjectReportCaps {
            likely_broken_areas: BROKEN_LIMIT,
            edge_cases: EDGE_LIMIT,
            vulnerability_slices: VULNERABILITY_LIMIT,
            dependency_risks: DEPENDENCY_LIMIT,
            predicted_works: WORKS_LIMIT,
            coverage_map: COVERAGE_LIMIT,
            cold_cells: COLD_LIMIT,
        },
    }
}

fn build_prediction_page(
    project_id: &str,
    request: &ProjectReportRequest,
    predictions: &[LoadedPrediction],
) -> Option<ProjectPredictionPage> {
    if request.section != Some(ProjectReportSection::Predictions) && request.offset.is_none() {
        return None;
    }
    let offset = request.offset.unwrap_or(0).min(predictions.len());
    let end = offset
        .saturating_add(PREDICTION_PAGE_LIMIT)
        .min(predictions.len());
    let has_more = end < predictions.len();
    let rows = predictions[offset..end]
        .iter()
        .enumerate()
        .map(|(idx, loaded)| ProjectPredictionPageRow {
            index: offset + idx,
            file_path: loaded.file_path.clone(),
            verdict: loaded.prediction.verdict,
            prediction_id: hex::encode(loaded.prediction.prediction_id),
            live_prediction_key_hex: loaded.live_key_hex.clone(),
            task_id: loaded.prediction.task_id.0.clone(),
            chunk_ids: loaded
                .prediction
                .covered_chunks
                .iter()
                .map(|chunk| chunk.0.clone())
                .collect(),
        })
        .collect();
    Some(ProjectPredictionPage {
        section: ProjectReportSection::Predictions,
        offset,
        limit: PREDICTION_PAGE_LIMIT,
        total: predictions.len(),
        has_more,
        next_link: has_more.then(|| {
            format!(
                "mcp__cgreality__mejepa_project_report?projectId={project_id}&section=predictions&offset={end}"
            )
        }),
        rows,
    })
}

fn load_project_predictions(
    db: &DB,
    manifest: &ProjectIngestManifest,
) -> Result<Vec<LoadedPrediction>, ProjectReportError> {
    let cf = live_prediction_cf(db)?;
    let mut out = Vec::with_capacity(manifest.predictions.len());
    for row in &manifest.predictions {
        let key =
            project_prediction_live_key(&manifest.project_id, &row.file_path, &row.file_blake3);
        let key_hex = hex::encode(&key);
        if !row.live_prediction_key_hex.is_empty() && row.live_prediction_key_hex != key_hex {
            return Err(MejepaInferError::InvalidInput {
                field: "project_manifest.predictions.live_prediction_key_hex".to_string(),
                detail: format!("{} does not match deterministic key", row.file_path),
            }
            .into());
        }
        let bytes =
            db.get_cf(cf, &key)?
                .ok_or_else(|| ProjectReportError::MissingPredictionRow {
                    project_id: manifest.project_id.clone(),
                    file_path: row.file_path.clone(),
                    key_hex: key_hex.clone(),
                })?;
        let prediction = decode_reality_prediction(&bytes)?;
        if hex::encode(prediction.prediction_id) != row.prediction_id_hex
            || prediction.task_id.0 != row.task_id
            || prediction.created_at_unix_ms
                != project_prediction_created_at_unix_ms(
                    &manifest.project_id,
                    &row.file_path,
                    &row.file_blake3,
                )
        {
            return Err(MejepaInferError::InvalidInput {
                field: "live_predictions.project_report_row".to_string(),
                detail: format!(
                    "prediction row for {} did not match manifest task/id/timestamp",
                    row.file_path
                ),
            }
            .into());
        }
        out.push(LoadedPrediction {
            file_path: row.file_path.clone(),
            live_key_hex: key_hex,
            prediction,
        });
    }
    Ok(out)
}

fn update_summary(summary: &mut VerdictSummary, loaded: &LoadedPrediction) {
    match loaded.prediction.verdict {
        Verdict::Pass => summary.pass += 1,
        Verdict::Fail => summary.fail += 1,
        Verdict::Abstain => summary.abstain += 1,
        Verdict::OutOfDistribution => summary.out_of_distribution += 1,
        Verdict::GuardRejected => summary.guard_rejected += 1,
    }
    summary.total += 1;
    summary
        .per_file
        .insert(loaded.file_path.clone(), loaded.prediction.verdict);
}

fn collect_broken_areas(
    rows: &mut Vec<BrokenAreaRow>,
    loaded: &LoadedPrediction,
    files_by_chunk: &BTreeMap<String, String>,
) {
    for failure in &loaded.prediction.predicted_failure_modes {
        let chunk_id = failure.chunk.0.clone();
        rows.push(BrokenAreaRow {
            rank: 0,
            file_path: file_for_chunk(files_by_chunk, &chunk_id, &loaded.file_path),
            chunk_id: chunk_id.clone(),
            line_range: failure.line_range,
            confidence: failure.confidence,
            severity: failure.severity,
            failure_class: enum_string(&failure.failure_class),
            root_cause_class: enum_string(&failure.root_cause_class),
            explanation: failure.explanation.clone(),
            evidence: evidence_link(loaded, Some(chunk_id), Some(failure.line_range)),
        });
    }
}

fn collect_edge_gaps(
    rows: &mut Vec<EdgeCaseGapRow>,
    loaded: &LoadedPrediction,
    files_by_chunk: &BTreeMap<String, String>,
) {
    for edge in loaded
        .prediction
        .predicted_edge_cases
        .iter()
        .filter(|edge| !edge.covered_by_test)
    {
        let chunk_id = edge.chunk.0.clone();
        rows.push(EdgeCaseGapRow {
            rank: 0,
            file_path: file_for_chunk(files_by_chunk, &chunk_id, &loaded.file_path),
            gap_kind: enum_string(&edge.edge_class),
            chunk_id: chunk_id.clone(),
            line_range: edge.line_range,
            confidence: edge.confidence,
            description: edge.triggering_input_description.clone(),
            evidence: evidence_link(loaded, Some(chunk_id), Some(edge.line_range)),
        });
    }
    for uncovered in &loaded.prediction.predicted_uncovered_paths {
        let chunk_id = uncovered.chunk.0.clone();
        rows.push(EdgeCaseGapRow {
            rank: 0,
            file_path: file_for_chunk(files_by_chunk, &chunk_id, &loaded.file_path),
            gap_kind: "uncovered_path".to_string(),
            chunk_id: chunk_id.clone(),
            line_range: uncovered.line_range,
            confidence: uncovered.defect_probability * uncovered.confidence,
            description: uncovered.path_description.clone(),
            evidence: evidence_link(loaded, Some(chunk_id), Some(uncovered.line_range)),
        });
    }
}

fn collect_vulnerabilities(
    rows: &mut Vec<VulnerabilitySliceRow>,
    loaded: &LoadedPrediction,
    files_by_chunk: &BTreeMap<String, String>,
) {
    for concern in &loaded.prediction.predicted_security_concerns {
        let chunk_id = concern.chunk.0.clone();
        rows.push(VulnerabilitySliceRow {
            rank: 0,
            file_path: file_for_chunk(files_by_chunk, &chunk_id, &loaded.file_path),
            chunk_id: chunk_id.clone(),
            line_range: concern.line_range,
            cvss_estimate: concern.cvss_estimate,
            concern_class: enum_string(&concern.class),
            explanation: concern.explanation.clone(),
            evidence: evidence_link(loaded, Some(chunk_id), Some(concern.line_range)),
        });
    }
}

fn collect_predicted_works(
    rows: &mut Vec<PredictedWorksRow>,
    loaded: &LoadedPrediction,
    files_by_chunk: &BTreeMap<String, String>,
) {
    for predicted in &loaded.prediction.predicted_works {
        let chunk_id = predicted.chunk.0.clone();
        rows.push(PredictedWorksRow {
            rank: 0,
            file_path: file_for_chunk(files_by_chunk, &chunk_id, &loaded.file_path),
            chunk_id: chunk_id.clone(),
            line_range: predicted.line_range,
            confidence: predicted.confidence,
            evidence_strength: predicted.evidence_strength,
            claim: predicted.claim.clone(),
            evidence: evidence_link(loaded, Some(chunk_id), Some(predicted.line_range)),
        });
    }
}

fn collect_coverage(
    rows: &mut Vec<CoveragePredictionRow>,
    loaded: &LoadedPrediction,
    files_by_chunk: &BTreeMap<String, String>,
) {
    for uncovered in &loaded.prediction.predicted_uncovered_paths {
        let chunk_id = uncovered.chunk.0.clone();
        rows.push(CoveragePredictionRow {
            rank: 0,
            file_path: file_for_chunk(files_by_chunk, &chunk_id, &loaded.file_path),
            chunk_id: chunk_id.clone(),
            line_range: uncovered.line_range,
            defect_probability: uncovered.defect_probability,
            confidence: uncovered.confidence,
            path_description: uncovered.path_description.clone(),
            evidence_text: uncovered.evidence.clone(),
            evidence: evidence_link(loaded, Some(chunk_id), Some(uncovered.line_range)),
        });
    }
}

fn collect_cold_cells(rows: &mut Vec<ColdCellRow>, loaded: &LoadedPrediction) {
    if matches!(
        loaded.prediction.verdict,
        Verdict::Abstain | Verdict::OutOfDistribution | Verdict::GuardRejected
    ) {
        rows.push(ColdCellRow {
            rank: 0,
            file_path: loaded.file_path.clone(),
            verdict: loaded.prediction.verdict,
            ood_score: loaded.prediction.ood_score,
            calibrated_confidence: loaded.prediction.calibrated_confidence,
            reason: cold_reason(loaded.prediction.verdict),
            evidence: evidence_link(loaded, None, None),
        });
    }
}

fn sort_and_rank<T, F>(rows: &mut Vec<T>, score: F, limit: usize)
where
    F: Fn(&T) -> f32,
    T: RankedRow,
{
    rows.sort_by(|left, right| {
        score(right)
            .partial_cmp(&score(left))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.sort_key().cmp(&right.sort_key()))
    });
    rows.truncate(limit);
    for (idx, row) in rows.iter_mut().enumerate() {
        row.set_rank(idx + 1);
    }
}

fn sort_vulnerabilities(rows: &mut [VulnerabilitySliceRow]) {
    rows.sort_by(|left, right| {
        right
            .cvss_estimate
            .unwrap_or(0.0)
            .partial_cmp(&left.cvss_estimate.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.line_range.cmp(&left.line_range))
    });
}

trait RankedRow {
    fn set_rank(&mut self, rank: usize);
    fn sort_key(&self) -> (&str, (u32, u32));
}

macro_rules! ranked_row {
    ($ty:ty) => {
        impl RankedRow for $ty {
            fn set_rank(&mut self, rank: usize) {
                self.rank = rank;
            }

            fn sort_key(&self) -> (&str, (u32, u32)) {
                (&self.file_path, self.line_range)
            }
        }
    };
}

ranked_row!(BrokenAreaRow);
ranked_row!(EdgeCaseGapRow);
ranked_row!(PredictedWorksRow);
ranked_row!(CoveragePredictionRow);

impl RankedRow for ColdCellRow {
    fn set_rank(&mut self, rank: usize) {
        self.rank = rank;
    }

    fn sort_key(&self) -> (&str, (u32, u32)) {
        (&self.file_path, (0, 0))
    }
}

fn chunk_file_index(predictions: &[LoadedPrediction]) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    for loaded in predictions {
        for chunk in &loaded.prediction.covered_chunks {
            files.insert(chunk.0.clone(), loaded.file_path.clone());
        }
    }
    files
}

fn evidence_link(
    loaded: &LoadedPrediction,
    chunk_id: Option<String>,
    line_range: Option<(u32, u32)>,
) -> EvidenceLink {
    EvidenceLink {
        prediction_id: hex::encode(loaded.prediction.prediction_id),
        live_prediction_key_hex: loaded.live_key_hex.clone(),
        task_id: loaded.prediction.task_id.0.clone(),
        file_path: loaded.file_path.clone(),
        chunk_id,
        line_range,
    }
}

fn file_for_chunk(
    files_by_chunk: &BTreeMap<String, String>,
    chunk_id: &str,
    fallback: &str,
) -> String {
    files_by_chunk
        .get(chunk_id)
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn write_report_cf(
    db: &DB,
    report_key: &str,
    report: &ProjectRealityReport,
) -> Result<(), ProjectReportError> {
    let bytes = serde_json::to_vec_pretty(report).map_err(|source| ProjectReportError::Json {
        path: report_key.to_string(),
        source,
    })?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let cf = report_cf(db)?;
    db.put_cf_opt(cf, report_key.as_bytes(), &bytes, &opts)?;
    db.flush_cf(cf)?;
    let readback =
        db.get_cf(cf, report_key.as_bytes())?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "project_report.cf_readback".to_string(),
                detail: format!("missing report row {report_key} after put"),
            })?;
    if readback != bytes {
        return Err(MejepaInferError::InvalidInput {
            field: "project_report.cf_readback".to_string(),
            detail: "report CF row readback bytes differed from written bytes".to_string(),
        }
        .into());
    }
    Ok(())
}

fn render_markdown(report: &ProjectRealityReport) -> String {
    let mut out = String::new();
    push_line(
        &mut out,
        format!("# Reality Compile Report - {}", report.project_id),
    );
    push_line(&mut out, "");
    push_line(
        &mut out,
        format!(
            "- Report version: `{}`\n- Predictions: `{}`\n- Source: `{}`",
            report.report_version,
            report.source_prediction_count,
            report.source_predictions_db_path
        ),
    );
    if let Some(warning) = &report.warning {
        push_line(&mut out, format!("\n> {warning}"));
    }
    push_line(&mut out, "\n## Verdict summary");
    push_line(
        &mut out,
        format!(
            "| Pass | Fail | Abstain | OOD | GuardRejected | Total |\n|---:|---:|---:|---:|---:|---:|\n| {} | {} | {} | {} | {} | {} |",
            report.verdict_summary.pass,
            report.verdict_summary.fail,
            report.verdict_summary.abstain,
            report.verdict_summary.out_of_distribution,
            report.verdict_summary.guard_rejected,
            report.verdict_summary.total
        ),
    );
    render_broken(&mut out, &report.top_likely_broken_areas);
    render_edge_cases(&mut out, &report.top_edge_cases_not_exercised);
    render_vulnerabilities(&mut out, &report.top_vulnerability_slices);
    render_dependency_risks(&mut out, &report.dependency_risks);
    render_predicted_works(&mut out, &report.predicted_works);
    render_coverage(&mut out, &report.coverage_prediction_map);
    render_cold(&mut out, &report.cold_cells);
    out
}

fn render_broken(out: &mut String, rows: &[BrokenAreaRow]) {
    push_line(out, "\n## Top-20 likely-broken areas");
    push_line(
        out,
        "| Rank | File | Lines | Confidence | Failure | Evidence |",
    );
    push_line(out, "|---:|---|---:|---:|---|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | {}-{} | {:.3} | `{}` | `{}` |",
                row.rank,
                row.file_path,
                row.line_range.0,
                row.line_range.1,
                row.confidence,
                row.failure_class,
                row.evidence.prediction_id
            ),
        );
    }
}

fn render_edge_cases(out: &mut String, rows: &[EdgeCaseGapRow]) {
    push_line(
        out,
        "\n## Top-10 predicted edge cases not exercised by tests",
    );
    push_line(out, "| Rank | File | Lines | Confidence | Gap | Evidence |");
    push_line(out, "|---:|---|---:|---:|---|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | {}-{} | {:.3} | `{}` | `{}` |",
                row.rank,
                row.file_path,
                row.line_range.0,
                row.line_range.1,
                row.confidence,
                row.gap_kind,
                row.evidence.prediction_id
            ),
        );
    }
}

fn render_vulnerabilities(out: &mut String, rows: &[VulnerabilitySliceRow]) {
    push_line(out, "\n## Top-10 vulnerability slices");
    push_line(out, "| Rank | File | Lines | CVSS | Class | Evidence |");
    push_line(out, "|---:|---|---:|---:|---|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | {}-{} | {} | `{}` | `{}` |",
                row.rank,
                row.file_path,
                row.line_range.0,
                row.line_range.1,
                row.cvss_estimate
                    .map(|value| format!("{value:.1}"))
                    .unwrap_or_else(|| "-".to_string()),
                row.concern_class,
                row.evidence.prediction_id
            ),
        );
    }
}

fn render_dependency_risks(out: &mut String, rows: &[DependencyRiskRow]) {
    push_line(out, "\n## Dependency risks");
    push_line(out, "| Rank | Dependency | CVE | Severity | Evidence |");
    push_line(out, "|---:|---|---|---|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | {} | {} | {} |",
                row.rank,
                row.dependency,
                row.cve_id.as_deref().unwrap_or("-"),
                row.severity.as_deref().unwrap_or("-"),
                row.evidence
                    .as_ref()
                    .map(|value| format!("`{}`", value.prediction_id))
                    .unwrap_or_else(|| "-".to_string())
            ),
        );
    }
}

fn render_predicted_works(out: &mut String, rows: &[PredictedWorksRow]) {
    push_line(out, "\n## Predicted-Works");
    push_line(
        out,
        "| Rank | File | Lines | Confidence | Evidence strength | Evidence |",
    );
    push_line(out, "|---:|---|---:|---:|---:|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | {}-{} | {:.3} | {:.3} | `{}` |",
                row.rank,
                row.file_path,
                row.line_range.0,
                row.line_range.1,
                row.confidence,
                row.evidence_strength,
                row.evidence.prediction_id
            ),
        );
    }
}

fn render_coverage(out: &mut String, rows: &[CoveragePredictionRow]) {
    push_line(out, "\n## Coverage prediction map");
    push_line(
        out,
        "| Rank | File | Lines | Defect probability | Confidence | Evidence |",
    );
    push_line(out, "|---:|---|---:|---:|---:|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | {}-{} | {:.3} | {:.3} | `{}` |",
                row.rank,
                row.file_path,
                row.line_range.0,
                row.line_range.1,
                row.defect_probability,
                row.confidence,
                row.evidence.prediction_id
            ),
        );
    }
}

fn render_cold(out: &mut String, rows: &[ColdCellRow]) {
    push_line(out, "\n## Cold cells");
    push_line(
        out,
        "| Rank | File | Verdict | OOD | Confidence | Evidence |",
    );
    push_line(out, "|---:|---|---|---:|---:|---|");
    if rows.is_empty() {
        push_line(out, "| - | - | - | - | - | - |");
        return;
    }
    for row in rows {
        push_line(
            out,
            format!(
                "| {} | `{}` | `{:?}` | {:.3} | {:.3} | `{}` |",
                row.rank,
                row.file_path,
                row.verdict,
                row.ood_score,
                row.calibrated_confidence,
                row.evidence.prediction_id
            ),
        );
    }
}

fn write_text_0600(path: &Path, text: &str) -> Result<(), ProjectReportError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ProjectReportError::Io {
            path: parent.display().to_string(),
            source,
        })?;
    }
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .map_err(|source| ProjectReportError::Io {
                path: path.display().to_string(),
                source,
            })?
    };
    #[cfg(not(unix))]
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|source| ProjectReportError::Io {
            path: path.display().to_string(),
            source,
        })?;
    file.write_all(text.as_bytes())
        .map_err(|source| ProjectReportError::Io {
            path: path.display().to_string(),
            source,
        })?;
    file.sync_all().map_err(|source| ProjectReportError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

fn read_manifest(path: &Path) -> Result<ProjectIngestManifest, ProjectReportError> {
    let bytes = fs::read(path).map_err(|source| ProjectReportError::Io {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ProjectReportError::Json {
        path: path.display().to_string(),
        source,
    })
}

fn live_prediction_cf(db: &DB) -> Result<&rocksdb::ColumnFamily, ProjectReportError> {
    db.cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "rocksdb.column_family".to_string(),
            detail: format!("missing column family {CF_MEJEPA_LIVE_PREDICTIONS}"),
        })
        .map_err(ProjectReportError::Infer)
}

fn report_cf(db: &DB) -> Result<&rocksdb::ColumnFamily, ProjectReportError> {
    db.cf_handle(CF_MEJEPA_PROJECT_REPORTS)
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "rocksdb.column_family".to_string(),
            detail: format!("missing column family {CF_MEJEPA_PROJECT_REPORTS}"),
        })
        .map_err(ProjectReportError::Infer)
}

fn validate_project_id(value: &str) -> Result<String, ProjectReportError> {
    if !is_valid_project_id(value) {
        return Err(ProjectReportError::InvalidProjectId {
            project_id: value.to_string(),
        });
    }
    Ok(value.to_string())
}

fn project_root(project_id: &str) -> Result<PathBuf, ProjectReportError> {
    Ok(context_graph_paths::production_data_root()?
        .join("projects")
        .join(project_id))
}

fn validate_manifest_paths(
    project_id: &str,
    manifest: &ProjectIngestManifest,
) -> Result<PathBuf, ProjectReportError> {
    let expected_root = project_root(project_id)?;
    let expected_db = project_report_db_path(project_id)?;
    let actual_root = PathBuf::from(&manifest.project_root);
    let actual_db = PathBuf::from(&manifest.predictions_db_path);
    if actual_root != expected_root {
        return Err(ProjectReportError::ManifestPathMismatch {
            project_id: project_id.to_string(),
            field: "project_root",
            expected: expected_root.display().to_string(),
            actual: actual_root.display().to_string(),
        });
    }
    if actual_db != expected_db {
        return Err(ProjectReportError::ManifestPathMismatch {
            project_id: project_id.to_string(),
            field: "predictions_db_path",
            expected: expected_db.display().to_string(),
            actual: actual_db.display().to_string(),
        });
    }
    Ok(expected_db)
}

fn report_version(manifest: &ProjectIngestManifest, generated_at_unix_ms: i64) -> String {
    let merkle = manifest.merkle_root.chars().take(16).collect::<String>();
    format!(
        "v{}-{}-{}-{}",
        REPORT_SCHEMA_VERSION, manifest.last_ingest_unix_ms, merkle, generated_at_unix_ms
    )
}

fn enum_string<T: std::fmt::Debug>(value: &T) -> String {
    format!("{value:?}")
}

fn cold_reason(verdict: Verdict) -> String {
    match verdict {
        Verdict::Abstain => "ME-JEPA abstained for insufficient decisive evidence".to_string(),
        Verdict::OutOfDistribution => {
            "prediction is out of the calibrated project distribution".to_string()
        }
        Verdict::GuardRejected => "Gtau guard rejected the prediction".to_string(),
        Verdict::Pass | Verdict::Fail => "not cold".to_string(),
    }
}

fn push_line(out: &mut String, value: impl AsRef<str>) {
    out.push_str(value.as_ref());
    out.push('\n');
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

#[allow(dead_code)]
fn _assert_report_rows_are_unique(report: &ProjectRealityReport) -> bool {
    let mut seen = BTreeSet::new();
    report
        .top_likely_broken_areas
        .iter()
        .all(|row| seen.insert((row.file_path.clone(), row.chunk_id.clone(), row.line_range)))
}
