use anyhow::{ensure, Context, Result};
use context_graph_mejepa::cli::write_json_0600;
use context_graph_mejepa::{
    count_cf, open_infer_rocksdb, read_reality_impact_record, replay_and_persist_reality_impact,
    AgentClaimGraph, ChunkId, ConformalSet, EmbedderId, FailureModeClass, Language, MejepaStore,
    OracleOutcome, PredictedFailureMode, PredictedTestOutcome, PredictionCorrectness,
    PredictionProvenance, RealityImpact, RealityImpactClassification, RealityPrediction,
    ReasoningClass, RocksDbInferStore, RootCauseClass, Severity, TaskId, TestDeltaKind, TestId,
    TestOutcome, Verdict, WitnessHash, DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
};
use context_graph_mejepa_cf::{CF_MEJEPA_LIVE_PREDICTIONS, CF_MEJEPA_REALITY_IMPACT};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const FSV_SUBDIR: &str = "task-py-g-045-shift-log-replay-engine-fsv";
const REPORT_NAME: &str = "shift_log_replay_engine_fsv.json";
const READBACK_NAME: &str = "independent_rocksdb_readback.json";

fn main() -> Result<()> {
    let root = fsv_root()?;
    if root.exists() {
        fs::remove_dir_all(&root).context("clear previous FSV root")?;
    }
    fs::create_dir_all(&root).context("create FSV root")?;
    let db_path = root.join("reality-impact.rocksdb");
    let runtime_root = root.join("runtime");

    let prediction_ids = {
        let db = open_infer_rocksdb(&db_path).context("open FSV RocksDB")?;
        let store = RocksDbInferStore::new(db.clone());
        let before = source_counts(db.as_ref())?;

        let happy_id = id16("happy");
        let happy = prediction(
            happy_id,
            id16("happy-session"),
            1_000,
            vec!["src/lib.py#0"],
            vec!["tests/test_lib.py::test_handles_edges"],
        );
        store.write_live_prediction(&happy)?;
        write_shift(
            &runtime_root,
            happy.session_id,
            r#"{"shift_id":"happy-shift","timestamp_unix_ms":1100,"test_id":"tests/test_lib.py::test_handles_edges","outcome":"failed"}"#,
        )?;
        let happy_record = replay_and_persist_reality_impact(
            db.as_ref(),
            happy_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_000,
        )?;
        let happy_pass = happy_record.matched.len() == 1
            && happy_record.missed.is_empty()
            && happy_record.not_yet_observed.is_empty()
            && happy_record.surprises.is_empty();

        let empty_id = id16("empty-window");
        let empty = prediction(
            empty_id,
            id16("empty-session"),
            1_000,
            vec!["src/empty.py#0"],
            vec!["tests/test_empty.py::test_later"],
        );
        store.write_live_prediction(&empty)?;
        let empty_record = replay_and_persist_reality_impact(
            db.as_ref(),
            empty_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_100,
        )?;
        let empty_pass = empty_record.matched.is_empty()
            && empty_record.missed.is_empty()
            && empty_record.not_yet_observed.len() == 1;

        let broken_id = id16("broken-chain");
        let broken = prediction_with_predicted_files(
            broken_id,
            id16("broken-session"),
            1_000,
            vec!["src/broken.py#0"],
            Vec::new(),
            vec!["src/broken.py"],
        );
        store.write_live_prediction(&broken)?;
        write_shift(
            &runtime_root,
            broken.session_id,
            r#"{"shift_id":"broken-1","timestamp_unix_ms":1100,"subject":{"path":"src/broken.py"},"after":{"sha256":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
        )?;
        write_shift(
            &runtime_root,
            broken.session_id,
            r#"{"shift_id":"broken-2","timestamp_unix_ms":1200,"subject":{"path":"src/broken.py"},"before":{"sha256":"sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"},"after":{"sha256":"sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}}"#,
        )?;
        let broken_error = replay_and_persist_reality_impact(
            db.as_ref(),
            broken_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_200,
        )
        .unwrap_err()
        .to_string();
        let broken_pass = broken_error.contains("SHIFT_LOG_CHAIN_BROKEN")
            && read_reality_impact_record(db.as_ref(), broken_id)?.is_none();

        let missed_id = id16("missed-test");
        let missed = prediction(
            missed_id,
            id16("missed-session"),
            1_000,
            vec!["src/missed.py#0"],
            vec!["tests/test_missed.py::test_never_ran"],
        );
        store.write_live_prediction(&missed)?;
        write_shift(
            &runtime_root,
            missed.session_id,
            r#"{"shift_id":"missed-file","timestamp_unix_ms":1100,"subject":{"path":"src/missed.py"},"after":{"sha256":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
        )?;
        let missed_record = replay_and_persist_reality_impact(
            db.as_ref(),
            missed_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_300,
        )?;
        let missed_pass = missed_record.matched.is_empty()
            && missed_record
                .missed
                .iter()
                .any(|m| m.reason == "TEST_NEVER_RAN")
            && missed_record
                .surprises
                .iter()
                .any(|s| s.reason == "UNPREDICTED_FILE_SHIFT");

        let surprise_id = id16("surprise");
        let surprise = prediction(
            surprise_id,
            id16("surprise-session"),
            1_000,
            vec!["src/expected.py#0"],
            Vec::new(),
        );
        store.write_live_prediction(&surprise)?;
        write_shift(
            &runtime_root,
            surprise.session_id,
            r#"{"shift_id":"surprise-file","timestamp_unix_ms":1100,"subject":{"path":"src/unexpected.py"},"after":{"sha256":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
        )?;
        let surprise_record = replay_and_persist_reality_impact(
            db.as_ref(),
            surprise_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_400,
        )?;
        let surprise_pass = surprise_record.missed.is_empty()
            && surprise_record.surprises.len() == 1
            && surprise_record.surprises[0].classification == RealityImpactClassification::Surprise;

        let overlap_session = id16("overlap-session");
        write_shift(
            &runtime_root,
            overlap_session,
            r#"{"shift_id":"overlap-file","timestamp_unix_ms":1100,"subject":{"path":"src/shared.py"},"after":{"sha256":"sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}"#,
        )?;
        let overlap_a_id = id16("overlap-a");
        let overlap_b_id = id16("overlap-b");
        let overlap_a = prediction_with_predicted_files(
            overlap_a_id,
            overlap_session,
            1_000,
            vec!["src/shared.py#0"],
            Vec::new(),
            vec!["src/shared.py"],
        );
        let overlap_b = prediction_with_predicted_files(
            overlap_b_id,
            overlap_session,
            1_000,
            vec!["src/shared.py#1"],
            Vec::new(),
            vec!["src/shared.py"],
        );
        store.write_live_prediction(&overlap_a)?;
        store.write_live_prediction(&overlap_b)?;
        let overlap_a_record = replay_and_persist_reality_impact(
            db.as_ref(),
            overlap_a_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_500,
        )?;
        let overlap_b_record = replay_and_persist_reality_impact(
            db.as_ref(),
            overlap_b_id,
            &runtime_root,
            DEFAULT_REALITY_IMPACT_REPLAY_WINDOW_MS,
            2_600,
        )?;
        let overlap_pass = overlap_a_record.matched.len() == 1
            && overlap_b_record.matched.len() == 1
            && overlap_a_record.prediction_id != overlap_b_record.prediction_id;

        let after = source_counts(db.as_ref())?;
        let report = json!({
            "task": "TASK-PY-G-045",
            "status": "pass",
            "sourceOfTruth": {
                "dbPath": db_path.display().to_string(),
                "runtimeRoot": runtime_root.display().to_string(),
                "livePredictionCf": CF_MEJEPA_LIVE_PREDICTIONS,
                "realityImpactCf": CF_MEJEPA_REALITY_IMPACT
            },
            "before": before,
            "happyPath": {
                "pass": happy_pass,
                "matched": happy_record.matched.len(),
                "missed": happy_record.missed.len(),
                "surprises": happy_record.surprises.len()
            },
            "edgeCases": [
                {"name": "window_empty_not_yet_observed", "pass": empty_pass, "notYetObserved": empty_record.not_yet_observed.len()},
                {"name": "sha_chain_break_fail_closed", "pass": broken_pass, "error": broken_error},
                {"name": "prediction_references_nonexistent_test", "pass": missed_pass, "missed": missed_record.missed},
                {"name": "surprise_shift", "pass": surprise_pass, "surprises": surprise_record.surprises},
                {"name": "overlapping_predictions_independent", "pass": overlap_pass, "records": [hex::encode(overlap_a_id), hex::encode(overlap_b_id)]}
            ],
            "after": after
        });
        let all_passed =
            happy_pass && empty_pass && broken_pass && missed_pass && surprise_pass && overlap_pass;
        ensure!(all_passed, "one or more TASK-PY-G-045 FSV checks failed");
        write_json_0600(&root.join(REPORT_NAME), &report)?;
        vec![
            happy_id,
            empty_id,
            missed_id,
            surprise_id,
            overlap_a_id,
            overlap_b_id,
        ]
    };

    let reopened = open_infer_rocksdb(&db_path).context("reopen FSV RocksDB for readback")?;
    let readback_rows = prediction_ids
        .iter()
        .map(|prediction_id| {
            let row = read_reality_impact_record(reopened.as_ref(), *prediction_id)?
                .context("missing reopened reality-impact record")?;
            Ok(json!({
                "predictionId": hex::encode(prediction_id),
                "matched": row.matched.len(),
                "missed": row.missed.len(),
                "notYetObserved": row.not_yet_observed.len(),
                "surprises": row.surprises.len(),
                "shiftCount": row.shift_count
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let independent = json!({
        "status": "pass",
        "dbPath": db_path.display().to_string(),
        "sourceOfTruthCf": CF_MEJEPA_REALITY_IMPACT,
        "rowCount": count_cf(reopened.as_ref(), CF_MEJEPA_REALITY_IMPACT)?,
        "rows": readback_rows
    });
    write_json_0600(&root.join(READBACK_NAME), &independent)?;
    println!("{}", serde_json::to_string_pretty(&independent)?);
    Ok(())
}

fn source_counts(db: &rocksdb::DB) -> Result<Value> {
    Ok(json!({
        CF_MEJEPA_LIVE_PREDICTIONS: count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
        CF_MEJEPA_REALITY_IMPACT: count_cf(db, CF_MEJEPA_REALITY_IMPACT)?
    }))
}

fn fsv_root() -> Result<PathBuf> {
    if let Ok(root) = std::env::var("CG_MEJEPA_TASK_PY_G_045_FSV_ROOT") {
        return Ok(PathBuf::from(root));
    }
    Ok(PathBuf::from("/tmp/contextgraph").join(FSV_SUBDIR))
}

fn prediction(
    prediction_id: [u8; 16],
    session_id: [u8; 16],
    created_at_unix_ms: i64,
    covered_chunks: Vec<&str>,
    failed_tests: Vec<&str>,
) -> RealityPrediction {
    prediction_with_predicted_files(
        prediction_id,
        session_id,
        created_at_unix_ms,
        covered_chunks,
        failed_tests,
        Vec::new(),
    )
}

fn prediction_with_predicted_files(
    prediction_id: [u8; 16],
    session_id: [u8; 16],
    created_at_unix_ms: i64,
    covered_chunks: Vec<&str>,
    failed_tests: Vec<&str>,
    predicted_files: Vec<&str>,
) -> RealityPrediction {
    let failure_mode = PredictedFailureMode {
        failure_class: FailureModeClass::AssertionMismatch,
        chunk: ChunkId("src/lib.py#0".to_string()),
        line_range: (1, 3),
        confidence: 0.9,
        severity: Severity::High,
        explanation: "synthetic known failure".to_string(),
        contributing_embedders: vec![EmbedderId("e1".to_string())],
        root_cause_class: RootCauseClass::LogicError,
    };
    RealityPrediction::try_new(RealityPrediction {
        prediction_id,
        witness_hash: WitnessHash([2; 32]),
        task_id: TaskId("task-py-g-045".to_string()),
        session_id,
        language: Language::Python,
        covered_chunks: covered_chunks
            .into_iter()
            .map(|chunk| ChunkId(chunk.to_string()))
            .collect(),
        verdict: Verdict::Pass,
        confidence_interval: Default::default(),
        predicted_oracle_pass: 0.9,
        predicted_test_pass: vec![0.9],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.1,
        outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        calibrated_confidence: 0.8,
        degraded_status: false,
        granger_attestations: BTreeMap::new(),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: failed_tests
            .into_iter()
            .map(|test| PredictedTestOutcome {
                test_id: TestId(test.to_string()),
                current_outcome: TestOutcome::Pass,
                predicted_outcome: TestOutcome::Fail,
                delta_kind: TestDeltaKind::PassToFail,
                confidence: 0.9,
                why: failure_mode.clone(),
            })
            .collect(),
        predicted_works: Vec::new(),
        predicted_uncovered_paths: Vec::new(),
        predicted_flaky_tests: Vec::new(),
        guard_violations: Vec::new(),
        per_slot_ood_reasons: Vec::new(),
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
        predicted_reasoning_class: ReasoningClass::Mute,
        agent_claim_graph: AgentClaimGraph::default(),
        claim_reconciliation: Vec::new(),
        reality_impact: if predicted_files.is_empty() {
            None
        } else {
            Some(RealityImpact {
                observed_shifts: Vec::new(),
                predicted_files_changed: predicted_files.into_iter().map(PathBuf::from).collect(),
                observed_files_changed: Vec::new(),
                unexpected_files_changed: Vec::new(),
                predicted_test_outcomes: Vec::new(),
                observed_test_outcomes: Vec::new(),
                prediction_correctness: PredictionCorrectness::Aligned,
            })
        },
        provenance: PredictionProvenance::default(),
        source_panel_sha: [4; 32],
        calibration_version: "task-py-g-045".to_string(),
        created_at_unix_ms,
        matched_fingerprint: None,
        unknown_candidate_id: None,
    })
    .unwrap()
}

fn write_shift(root: &Path, session_id: [u8; 16], line: &str) -> Result<()> {
    let dir = root.join("cgreality-shift-log");
    fs::create_dir_all(&dir).context("create shift log dir")?;
    let path = dir.join(format!("{}.jsonl", hex::encode(session_id)));
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .context("open shift log")?;
    writeln!(file, "{line}").context("write shift log line")?;
    Ok(())
}

fn id16(seed: &str) -> [u8; 16] {
    let digest = Sha256::digest(seed.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}
