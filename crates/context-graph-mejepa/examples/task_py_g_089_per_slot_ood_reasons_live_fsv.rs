use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    count_cf, open_infer_rocksdb, replay_prediction_from_db, write_json_0600, AgentClaimGraph,
    ChunkId, ConformalInterval, ConformalMethod, ConformalSet, EmbedderId, Language, MejepaStore,
    OracleOutcome, PerSlotOodReason, PerSlotOodReasonKind, PredictionId, PredictionProvenance,
    RealityPrediction, ReasoningClass, RocksDbInferStore, TaskId, Verdict, WitnessHash,
};
use context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const DEFAULT_FSV_ROOT: &str =
    "/zfs/archive/contextgraph/fsv/task-py-g-089-per-slot-ood-reasons-live-fsv";
const MAX_EMBEDDER_PANEL_REASONS_PLUS_ONE: usize = 257;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BoundaryCase {
    case: String,
    expected_error_code: String,
    observed_error_code: String,
    row_count_before: u64,
    row_count_after: u64,
    pass: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskPyG089LiveFsvReport {
    task_id: String,
    issue: String,
    status: String,
    all_passed: bool,
    fsv_root: PathBuf,
    db_path: PathBuf,
    artifact_path: PathBuf,
    source_of_truth: Value,
    happy_path: Value,
    boundary_cases: Vec<BoundaryCase>,
}

fn main() -> Result<()> {
    let run_id = chrono::Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(
        std::env::var("CG_MEJEPA_TASK_PY_G_089_LIVE_FSV_ROOT")
            .unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string()),
    );
    let run_root = fsv_root.join(format!("run-{run_id}"));
    let db_path = run_root.join("rocksdb");
    let artifact_path = run_root.join("task_py_g_089_per_slot_ood_reasons_live_fsv.json");

    let happy_path = run_happy_path(&db_path).context("happy path")?;
    let boundary_cases = run_boundary_cases(&db_path).context("boundary cases")?;
    let all_passed =
        happy_path["pass"].as_bool() == Some(true) && boundary_cases.iter().all(|case| case.pass);
    ensure!(all_passed, "one or more FSV cases failed");

    let report = TaskPyG089LiveFsvReport {
        task_id: "TASK-PY-G-089".to_string(),
        issue: "https://github.com/cabdru/contextgraph/issues/389".to_string(),
        status: "pass".to_string(),
        all_passed,
        fsv_root,
        db_path: db_path.clone(),
        artifact_path: artifact_path.clone(),
        source_of_truth: json!({
            "durableRoot": "/zfs/archive/contextgraph",
            "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
            "readbackMode": "write row, drop DB handle, reopen RocksDB, replay row bytes"
        }),
        happy_path,
        boundary_cases,
    };

    write_json_0600(&artifact_path, &report).context("write FSV artifact")?;
    let artifact_bytes = std::fs::read(&artifact_path).context("read FSV artifact")?;
    let readback: TaskPyG089LiveFsvReport =
        serde_json::from_slice(&artifact_bytes).context("decode FSV artifact")?;
    ensure!(
        readback.all_passed && readback.status == "pass",
        "FSV artifact readback did not preserve pass status"
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": hex::encode(Sha256::digest(&artifact_bytes)),
            "all_passed": true,
            "live_prediction_cf": CF_MEJEPA_LIVE_PREDICTIONS
        }))?
    );
    Ok(())
}

fn run_happy_path(db_path: &Path) -> Result<Value> {
    let prediction_id = [0x89; 16];
    let session_id = [0x91; 16];
    let prediction = base_prediction(
        prediction_id,
        session_id,
        1_779_100_000_001,
        vec![gtau_reason("E7")],
    )
    .context("build happy prediction")?;

    let row_count_before;
    {
        let db = open_infer_rocksdb(db_path).context("open RocksDB")?;
        let store = RocksDbInferStore::new(db.clone());
        row_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        store
            .write_live_prediction(&prediction)
            .context("write live prediction")?;
        let loaded = store
            .read_live_predictions(session_id, 1)
            .context("read live predictions")?;
        ensure!(
            loaded == vec![prediction.clone()],
            "typed readback mismatch"
        );
        let row_count_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        ensure!(
            row_count_after == row_count_before + 1,
            "expected exactly one persisted live prediction row"
        );
    }

    let replay = replay_prediction_from_db(db_path, PredictionId(prediction_id))
        .context("replay persisted prediction")?;
    let replay_reason = replay
        .prediction
        .per_slot_ood_reasons
        .first()
        .context("missing replayed per-slot OOD reason")?;
    ensure!(
        replay_reason.embedder.0 == "E7"
            && replay_reason.reason == PerSlotOodReasonKind::GtauViolation
            && replay.byte_for_byte_equal
            && replay.key_payload_equal,
        "replayed row did not preserve per-slot OOD reason evidence"
    );

    Ok(json!({
        "case": "happy_path_persists_and_replays_per_slot_ood_reason",
        "predictionId": hex::encode(prediction_id),
        "rowCountBefore": row_count_before,
        "rowCountAfter": replay.live_prediction_count,
        "replayValueSha256": replay.stored_value_sha256,
        "replayValueBytes": replay.source_of_truth.value_bytes,
        "reasonCount": replay.prediction.per_slot_ood_reasons.len(),
        "reason": replay_reason,
        "pass": true
    }))
}

fn run_boundary_cases(db_path: &Path) -> Result<Vec<BoundaryCase>> {
    let cases = vec![
        (
            "empty_embedder_rejected",
            "MEJEPA_INFER_INVALID_INPUT",
            vec![PerSlotOodReason {
                embedder: EmbedderId(String::new()),
                ..gtau_reason("E7")
            }],
        ),
        (
            "missing_calibration_version_rejected",
            "MEJEPA_INFER_INVALID_INPUT",
            vec![PerSlotOodReason {
                calibration_version: String::new(),
                ..gtau_reason("E8")
            }],
        ),
        (
            "non_finite_score_rejected",
            "MEJEPA_INFER_NAN_DETECTED",
            vec![PerSlotOodReason {
                observed_score: f32::NAN,
                ..gtau_reason("E9")
            }],
        ),
        (
            "too_many_reasons_rejected",
            "MEJEPA_INFER_DIM_MISMATCH",
            (0..MAX_EMBEDDER_PANEL_REASONS_PLUS_ONE)
                .map(|idx| gtau_reason(&format!("E{idx}")))
                .collect(),
        ),
    ];

    let db = open_infer_rocksdb(db_path).context("open RocksDB for boundary cases")?;
    let store = RocksDbInferStore::new(db.clone());
    let mut out = Vec::with_capacity(cases.len());
    for (idx, (case_name, expected_code, reasons)) in cases.into_iter().enumerate() {
        let row_count_before = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        let result = base_prediction(
            [0xa0 + idx as u8; 16],
            [0xb0; 16],
            1_779_100_000_010 + idx as i64,
            reasons,
        )
        .and_then(|prediction| store.write_live_prediction(&prediction));
        let observed_error_code = match result {
            Ok(()) => "unexpected_ok".to_string(),
            Err(err) => err.code().to_string(),
        };
        let row_count_after = count_cf(db.as_ref(), CF_MEJEPA_LIVE_PREDICTIONS)?;
        out.push(BoundaryCase {
            case: case_name.to_string(),
            expected_error_code: expected_code.to_string(),
            observed_error_code: observed_error_code.clone(),
            row_count_before,
            row_count_after,
            pass: observed_error_code == expected_code && row_count_after == row_count_before,
        });
    }
    Ok(out)
}

fn gtau_reason(embedder: &str) -> PerSlotOodReason {
    PerSlotOodReason {
        embedder: EmbedderId(embedder.to_string()),
        chunk: Some(ChunkId("src/lib.py#fn#demo".to_string())),
        reason: PerSlotOodReasonKind::GtauViolation,
        observed_score: 0.71,
        threshold: 0.85,
        margin: 0.14,
        calibration_version: "per-slot-v1".to_string(),
        evidence: format!(
            "Gtau slot violation: embedder={embedder} chunk=src/lib.py#fn#demo centroid=known-good-python cosine=0.71 threshold_tau_m=0.85 deficit=0.14"
        ),
    }
}

fn base_prediction(
    prediction_id: [u8; 16],
    session_id: [u8; 16],
    created_at_unix_ms: i64,
    per_slot_ood_reasons: Vec<PerSlotOodReason>,
) -> std::result::Result<RealityPrediction, context_graph_mejepa::MejepaInferError> {
    RealityPrediction::try_new(RealityPrediction {
        prediction_id,
        witness_hash: WitnessHash([0x44; 32]),
        task_id: TaskId("task-py-g-089-per-slot-ood-live-fsv".to_string()),
        session_id,
        language: Language::Python,
        covered_chunks: vec![ChunkId("src/lib.py#fn#demo".to_string())],
        verdict: Verdict::OutOfDistribution,
        confidence_interval: ConformalInterval {
            lower: 0.31,
            upper: 0.69,
            method: ConformalMethod::SplitConformal,
            coverage_target: 0.90,
            empirical_coverage: 0.89,
        },
        predicted_oracle_pass: 0.42,
        predicted_test_pass: vec![0.42],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.83,
        outcome_set: ConformalSet::try_new(vec![OracleOutcome::OutOfDistribution], 0.1, 0.2)?,
        calibrated_confidence: 0.57,
        degraded_status: true,
        granger_attestations: std::collections::BTreeMap::from([(
            "task-py-g-089:per-slot-ood".to_string(),
            0.91,
        )]),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: Vec::new(),
        predicted_works: Vec::new(),
        predicted_uncovered_paths: Vec::new(),
        predicted_flaky_tests: Vec::new(),
        guard_violations: Vec::new(),
        per_slot_ood_reasons,
        closest_exemplars: Vec::new(),
        predicted_edge_cases: Vec::new(),
        predicted_latent_bugs: Vec::new(),
        predicted_tech_debt_added: Vec::new(),
        predicted_dead_code: Vec::new(),
        predicted_redundant_code: Vec::new(),
        predicted_perf_regressions: Vec::new(),
        predicted_security_concerns: Vec::new(),
        predicted_accuracy_degradations: Vec::new(),
        predicted_cost_regressions: Vec::new(),
        predicted_reasoning_class: ReasoningClass::MostlyCorrect,
        agent_claim_graph: AgentClaimGraph::default(),
        claim_reconciliation: Vec::new(),
        reality_impact: None,
        provenance: PredictionProvenance {
            predictor_version: "task-py-g-089-live-fsv".to_string(),
            constellation_version: "slot-preserving-teleological-constellation-v1".to_string(),
            calibration_version: "per-slot-v1".to_string(),
            active_pointer: hex::encode(prediction_id),
        },
        source_panel_sha: [0x55; 32],
        calibration_version: "per-slot-v1".to_string(),
        created_at_unix_ms,
        matched_fingerprint: None,
        unknown_candidate_id: None,
    })
}
