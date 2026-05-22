use crate::synthetic_stress_eval::evaluate_synthetic_stress_case;
use crate::synthetic_stress_store::persist_synthetic_stress_result;
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_SYNTHETIC_STRESS_RESULTS};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

pub use crate::synthetic_stress_corpus::{
    materialize_synthetic_stress_corpus, read_synthetic_stress_corpus,
};
pub use crate::synthetic_stress_store::{
    count_synthetic_stress_results, read_synthetic_stress_result, synthetic_stress_result_key,
};

use crate::error::MejepaInferError;
use crate::project_ingest::ProjectIngestError;
use crate::types::{FailureModeClass, Verdict};

pub const SYNTHETIC_STRESS_ROOT: &str =
    "/zfs/archive/contextgraph/corpus/python-synthetic-stress-v1";
pub const SYNTHETIC_STRESS_RESULTS_DB: &str =
    "/zfs/archive/contextgraph/corpus/python-synthetic-stress-v1-results.rocksdb";
pub const SYNTHETIC_STRESS_CASES: usize = 30;
pub const SYNTHETIC_STRESS_THRESHOLD: f32 = 0.85;
pub const SYNTHETIC_STRESS_SCHEMA_VERSION: u32 = 1;
pub const SYNTHETIC_STRESS_SOURCE: &str = "synthetic_stress_fixture";
pub const SYNTHETIC_STRESS_GATE_PREDICATE: &str = "not_applicable";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntheticStressKind {
    WorksAsClaimed,
    BrokenInObviousWay,
    PassesButShouldFail,
    EdgeCaseTrap,
}

impl SyntheticStressKind {
    pub(crate) fn case_id_prefix(self) -> &'static str {
        match self {
            Self::WorksAsClaimed => "works_as_claimed",
            Self::BrokenInObviousWay => "broken_obvious",
            Self::PassesButShouldFail => "passes_but_should_fail",
            Self::EdgeCaseTrap => "edge_case_trap",
        }
    }

    pub fn as_snake_case(self) -> &'static str {
        match self {
            Self::WorksAsClaimed => "works_as_claimed",
            Self::BrokenInObviousWay => "broken_in_obvious_way",
            Self::PassesButShouldFail => "passes_but_should_fail",
            Self::EdgeCaseTrap => "edge_case_trap",
        }
    }

    pub(crate) fn from_case_id(case_id: &str) -> Option<Self> {
        [
            Self::WorksAsClaimed,
            Self::BrokenInObviousWay,
            Self::PassesButShouldFail,
            Self::EdgeCaseTrap,
        ]
        .into_iter()
        .find(|kind| case_id.starts_with(kind.case_id_prefix()))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimReconciliationExpectation {
    NoAgentClaims,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticExpectedVerdict {
    pub schema_version: u32,
    pub verdict: Verdict,
    pub top_failure_mode: Option<FailureModeClass>,
    pub top_failure_explanation_contains: Option<String>,
    pub top_q4_concerns: Vec<String>,
    pub predicted_works: bool,
    pub claim_reconciliation_expectation: ClaimReconciliationExpectation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticStressCase {
    pub schema_version: u32,
    pub case_id: String,
    pub kind: SyntheticStressKind,
    pub title: String,
    pub case_dir: String,
    pub code_path: String,
    pub test_path: String,
    pub expected_verdict_path: String,
    pub code_sha256: String,
    pub test_sha256: String,
    pub expected_verdict_sha256: String,
    pub expected: SyntheticExpectedVerdict,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticActualPredictionShape {
    pub verdict: Verdict,
    pub top_failure_mode: Option<FailureModeClass>,
    pub top_failure_explanation: Option<String>,
    pub top_q4_concerns: Vec<String>,
    pub predicted_works: bool,
    pub claim_reconciliation_count: usize,
    pub prediction_id_hex: String,
    pub live_prediction_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticStressResult {
    pub schema_version: u32,
    pub case_id: String,
    pub kind: SyntheticStressKind,
    pub project_id: String,
    pub expected: SyntheticExpectedVerdict,
    pub actual: SyntheticActualPredictionShape,
    pub matched: bool,
    pub mismatch_reasons: Vec<String>,
    pub result_key: String,
    pub source_of_truth_cf: String,
    pub live_prediction_cf: String,
    pub predictions_db_path: String,
    pub live_prediction_rows_before: usize,
    pub live_prediction_rows_after: usize,
    pub result_rows_before: usize,
    pub result_rows_after: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SyntheticStressEvalReport {
    pub schema_version: u32,
    pub task_id: String,
    pub corpus_root: String,
    pub results_db_path: String,
    pub result_cf: String,
    pub live_prediction_cf: String,
    pub cases_expected: usize,
    pub cases_total: usize,
    pub cases_passed: usize,
    pub synthetic_stress_correctness: f32,
    pub threshold: f32,
    pub synthetic_stress_passed: bool,
    pub ship_gate_countable: bool,
    pub gate_predicate: String,
    pub source: String,
    pub result_rows_before: usize,
    pub result_rows_after: usize,
    pub case_counts_by_kind: BTreeMap<String, usize>,
    pub results: Vec<SyntheticStressResult>,
}

#[derive(Debug, Clone)]
pub struct SyntheticStressEvalRequest {
    pub corpus_root: PathBuf,
    pub results_db_path: PathBuf,
    pub project_id_prefix: String,
    pub materialize_corpus: bool,
    pub overwrite_corpus: bool,
}

impl Default for SyntheticStressEvalRequest {
    fn default() -> Self {
        Self {
            corpus_root: PathBuf::from(SYNTHETIC_STRESS_ROOT),
            results_db_path: PathBuf::from(SYNTHETIC_STRESS_RESULTS_DB),
            project_id_prefix: format!("task-py-g-071-{}", synthetic_stress_now_ms()),
            materialize_corpus: true,
            overwrite_corpus: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum SyntheticStressError {
    #[error("SYNTHETIC_STRESS_INVALID_INPUT: {field}: {detail}")]
    InvalidInput { field: String, detail: String },
    #[error("SYNTHETIC_STRESS_EMPTY_CORPUS: {path}")]
    EmptyCorpus { path: String },
    #[error("SYNTHETIC_STRESS_MALFORMED_EXPECTATION: {path}: {detail}")]
    MalformedExpectation { path: String, detail: String },
    #[error("SYNTHETIC_STRESS_IO: {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("SYNTHETIC_STRESS_JSON: {path}: {source}")]
    Json {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Ingest(#[from] ProjectIngestError),
    #[error("SYNTHETIC_STRESS_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("SYNTHETIC_STRESS_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
    #[error("{0}")]
    Infer(#[from] MejepaInferError),
}

impl SyntheticStressError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "SYNTHETIC_STRESS_INVALID_INPUT",
            Self::EmptyCorpus { .. } => "SYNTHETIC_STRESS_EMPTY_CORPUS",
            Self::MalformedExpectation { .. } => "SYNTHETIC_STRESS_MALFORMED_EXPECTATION",
            Self::Io { .. } => "SYNTHETIC_STRESS_IO",
            Self::Json { .. } => "SYNTHETIC_STRESS_JSON",
            Self::Ingest(err) => err.code(),
            Self::RocksDb(_) => "SYNTHETIC_STRESS_ROCKSDB",
            Self::Bincode(_) => "SYNTHETIC_STRESS_BINCODE",
            Self::Infer(err) => err.code(),
        }
    }
}

pub fn run_synthetic_stress_eval(
    request: SyntheticStressEvalRequest,
) -> Result<SyntheticStressEvalReport, SyntheticStressError> {
    validate_synthetic_stress_project_prefix(&request.project_id_prefix)?;
    if request.materialize_corpus {
        materialize_synthetic_stress_corpus(&request.corpus_root, request.overwrite_corpus)?;
    }

    let cases = read_synthetic_stress_corpus(&request.corpus_root)?;
    if cases.is_empty() {
        return Err(SyntheticStressError::EmptyCorpus {
            path: request.corpus_root.display().to_string(),
        });
    }

    let db = crate::calibration::open_infer_rocksdb(&request.results_db_path)?;
    let result_rows_before = count_synthetic_stress_results(db.as_ref())?;
    let mut results = Vec::with_capacity(cases.len());
    for case in cases {
        let before = count_synthetic_stress_results(db.as_ref())?;
        let mut result = evaluate_synthetic_stress_case(&request, &case, before)?;
        persist_synthetic_stress_result(db.as_ref(), &result)?;
        result.result_rows_after = count_synthetic_stress_results(db.as_ref())?;
        results.push(result);
    }

    let cases_passed = results.iter().filter(|row| row.matched).count();
    let correctness = cases_passed as f32 / results.len() as f32;
    let mut counts = BTreeMap::new();
    for result in &results {
        *counts
            .entry(result.kind.as_snake_case().to_string())
            .or_insert(0usize) += 1;
    }
    let synthetic_stress_passed =
        results.len() == SYNTHETIC_STRESS_CASES && correctness >= SYNTHETIC_STRESS_THRESHOLD;

    Ok(SyntheticStressEvalReport {
        schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
        task_id: "TASK-PY-G-071".to_string(),
        corpus_root: request.corpus_root.display().to_string(),
        results_db_path: request.results_db_path.display().to_string(),
        result_cf: CF_MEJEPA_SYNTHETIC_STRESS_RESULTS.to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        cases_expected: SYNTHETIC_STRESS_CASES,
        cases_total: results.len(),
        cases_passed,
        synthetic_stress_correctness: correctness,
        threshold: SYNTHETIC_STRESS_THRESHOLD,
        synthetic_stress_passed,
        ship_gate_countable: false,
        gate_predicate: SYNTHETIC_STRESS_GATE_PREDICATE.to_string(),
        source: SYNTHETIC_STRESS_SOURCE.to_string(),
        result_rows_before,
        result_rows_after: count_synthetic_stress_results(db.as_ref())?,
        case_counts_by_kind: counts,
        results,
    })
}

pub(crate) fn synthetic_stress_invalid(
    field: impl Into<String>,
    detail: impl Into<String>,
) -> SyntheticStressError {
    SyntheticStressError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    }
}

pub(crate) fn validate_synthetic_stress_project_prefix(
    value: &str,
) -> Result<(), SyntheticStressError> {
    if value.len() <= 96
        && !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
    {
        Ok(())
    } else {
        Err(synthetic_stress_invalid(
            "project_id_prefix",
            format!("invalid project id prefix {value:?}"),
        ))
    }
}

fn synthetic_stress_now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_stress_report_is_explicitly_not_ship_gate_countable() {
        let report = SyntheticStressEvalReport {
            schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
            task_id: "TASK-PY-G-071".to_string(),
            corpus_root: "/tmp/synthetic-corpus".to_string(),
            results_db_path: "/tmp/synthetic-results.rocksdb".to_string(),
            result_cf: CF_MEJEPA_SYNTHETIC_STRESS_RESULTS.to_string(),
            live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
            cases_expected: SYNTHETIC_STRESS_CASES,
            cases_total: SYNTHETIC_STRESS_CASES,
            cases_passed: SYNTHETIC_STRESS_CASES,
            synthetic_stress_correctness: 1.0,
            threshold: SYNTHETIC_STRESS_THRESHOLD,
            synthetic_stress_passed: true,
            ship_gate_countable: false,
            gate_predicate: SYNTHETIC_STRESS_GATE_PREDICATE.to_string(),
            source: SYNTHETIC_STRESS_SOURCE.to_string(),
            result_rows_before: 0,
            result_rows_after: SYNTHETIC_STRESS_CASES,
            case_counts_by_kind: BTreeMap::new(),
            results: Vec::new(),
        };

        let value = serde_json::to_value(&report).expect("serialize synthetic stress report");
        assert_eq!(value["syntheticStressPassed"], true);
        assert_eq!(value["shipGateCountable"], false);
        assert_eq!(value["gatePredicate"], SYNTHETIC_STRESS_GATE_PREDICATE);
        assert_eq!(value["source"], SYNTHETIC_STRESS_SOURCE);
        let legacy_camel_key = ["ship", "Gate", "Passed"].join("");
        let legacy_snake_key = ["ship", "gate", "passed"].join("_");
        assert!(value.get(&legacy_camel_key).is_none());
        assert!(value.get(&legacy_snake_key).is_none());
    }
}
