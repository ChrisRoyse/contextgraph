use crate::calibration::open_infer_rocksdb;
use crate::project_ingest::{
    run_project_ingest, ProjectIngestMode, ProjectIngestRequest, ProjectIngestScope,
    ProjectPredictionManifestRow,
};
use crate::synthetic_stress::{
    synthetic_stress_invalid, ClaimReconciliationExpectation, SyntheticActualPredictionShape,
    SyntheticExpectedVerdict, SyntheticStressCase, SyntheticStressError,
    SyntheticStressEvalRequest, SyntheticStressResult, SYNTHETIC_STRESS_SCHEMA_VERSION,
};
use crate::synthetic_stress_store::synthetic_stress_result_key;
use crate::types::{decode_reality_prediction, RealityPrediction};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_SYNTHETIC_STRESS_RESULTS};
use std::path::{Path, PathBuf};

pub(crate) fn evaluate_synthetic_stress_case(
    request: &SyntheticStressEvalRequest,
    case: &SyntheticStressCase,
    result_rows_before: usize,
) -> Result<SyntheticStressResult, SyntheticStressError> {
    let project_id = format!("{}-{}", request.project_id_prefix, case.case_id);
    let ingest = run_project_ingest(ProjectIngestRequest {
        repo_path: PathBuf::from(&case.case_dir),
        project_id: Some(project_id.clone()),
        mode: ProjectIngestMode::Full,
        scope: ProjectIngestScope::SourceOnly,
        overwrite: true,
        changed_paths: Vec::new(),
    })?;
    let row = code_prediction_row(&ingest.manifest.predictions)?;
    let prediction = read_live_prediction(&ingest.predictions_db_path, row)?;
    let actual = actual_shape(row, &prediction);
    let mismatch_reasons = compare_expected(&case.expected, &actual);

    Ok(SyntheticStressResult {
        schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
        case_id: case.case_id.clone(),
        kind: case.kind,
        project_id,
        expected: case.expected.clone(),
        actual,
        matched: mismatch_reasons.is_empty(),
        mismatch_reasons,
        result_key: String::from_utf8_lossy(&synthetic_stress_result_key(&case.case_id))
            .to_string(),
        source_of_truth_cf: CF_MEJEPA_SYNTHETIC_STRESS_RESULTS.to_string(),
        live_prediction_cf: CF_MEJEPA_LIVE_PREDICTIONS.to_string(),
        predictions_db_path: ingest.predictions_db_path,
        live_prediction_rows_before: ingest.project_prediction_rows_before,
        live_prediction_rows_after: ingest.project_prediction_rows_after,
        result_rows_before,
        result_rows_after: result_rows_before,
    })
}

fn code_prediction_row(
    rows: &[ProjectPredictionManifestRow],
) -> Result<&ProjectPredictionManifestRow, SyntheticStressError> {
    rows.iter()
        .find(|row| row.file_path == "code.py")
        .ok_or_else(|| {
            synthetic_stress_invalid(
                "synthetic_stress.code_prediction",
                "missing code.py prediction",
            )
        })
}

fn read_live_prediction(
    db_path: &str,
    row: &ProjectPredictionManifestRow,
) -> Result<RealityPrediction, SyntheticStressError> {
    let key = hex::decode(&row.live_prediction_key_hex)
        .map_err(|err| synthetic_stress_invalid("live_prediction_key_hex", err.to_string()))?;
    let db = open_infer_rocksdb(Path::new(db_path))?;
    let cf = db.cf_handle(CF_MEJEPA_LIVE_PREDICTIONS).ok_or_else(|| {
        synthetic_stress_invalid("rocksdb.column_family", CF_MEJEPA_LIVE_PREDICTIONS)
    })?;
    let bytes = db.get_cf(cf, &key)?.ok_or_else(|| {
        synthetic_stress_invalid("live_prediction.readback", "missing live prediction row")
    })?;
    Ok(decode_reality_prediction(&bytes)?)
}

fn actual_shape(
    row: &ProjectPredictionManifestRow,
    prediction: &RealityPrediction,
) -> SyntheticActualPredictionShape {
    let top_failure = prediction.predicted_failure_modes.first();
    let mut q4 = Vec::new();
    q4.extend(
        prediction
            .predicted_security_concerns
            .iter()
            .map(|concern| concern.explanation.clone()),
    );
    q4.extend(
        prediction
            .predicted_edge_cases
            .iter()
            .map(|edge| edge.triggering_input_description.clone()),
    );
    q4.extend(
        prediction
            .predicted_latent_bugs
            .iter()
            .map(|bug| bug.explanation.clone()),
    );
    SyntheticActualPredictionShape {
        verdict: prediction.verdict,
        top_failure_mode: top_failure.map(|failure| failure.failure_class),
        top_failure_explanation: top_failure.map(|failure| failure.explanation.clone()),
        top_q4_concerns: q4,
        predicted_works: !prediction.predicted_works.is_empty(),
        claim_reconciliation_count: prediction.claim_reconciliation.len(),
        prediction_id_hex: row.prediction_id_hex.clone(),
        live_prediction_key_hex: row.live_prediction_key_hex.clone(),
    }
}

fn compare_expected(
    expected: &SyntheticExpectedVerdict,
    actual: &SyntheticActualPredictionShape,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if actual.verdict != expected.verdict {
        reasons.push(format!(
            "verdict expected {:?} got {:?}",
            expected.verdict, actual.verdict
        ));
    }
    if actual.top_failure_mode != expected.top_failure_mode {
        reasons.push(format!(
            "top_failure_mode expected {:?} got {:?}",
            expected.top_failure_mode, actual.top_failure_mode
        ));
    }
    if let Some(needle) = &expected.top_failure_explanation_contains {
        let matched = actual
            .top_failure_explanation
            .as_ref()
            .is_some_and(|value| value.contains(needle));
        if !matched {
            reasons.push(format!("top_failure_explanation missing {needle:?}"));
        }
    }
    for concern in &expected.top_q4_concerns {
        if !actual
            .top_q4_concerns
            .iter()
            .any(|value| value.contains(concern))
        {
            reasons.push(format!("q4 concern missing {concern:?}"));
        }
    }
    if actual.predicted_works != expected.predicted_works {
        reasons.push(format!(
            "predicted_works expected {} got {}",
            expected.predicted_works, actual.predicted_works
        ));
    }
    if expected.claim_reconciliation_expectation == ClaimReconciliationExpectation::NoAgentClaims
        && actual.claim_reconciliation_count != 0
    {
        reasons.push(format!(
            "claim_reconciliation_count expected 0 got {}",
            actual.claim_reconciliation_count
        ));
    }
    reasons
}
