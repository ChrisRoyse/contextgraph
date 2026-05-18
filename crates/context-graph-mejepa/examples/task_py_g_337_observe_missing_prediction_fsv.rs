//! TASK-PY-G-337 FSV: observe ticker quarantines test outcomes without a live prediction.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::heal::{read_status_snapshot, SchedulerState, SelfOptimConfig, TickName};
use context_graph_mejepa::{
    AgentClaimGraph, ConformalInterval, ConformalSet, Language, MejepaStore, OracleOutcome,
    PredictionProvenance, RealityPrediction, ReasoningClass, RocksDbInferStore, TaskId, Verdict,
    WitnessHash,
};
use rocksdb::{ColumnFamilyDescriptor, IteratorMode, Options, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-337-observe-missing-prediction-fsv";

fn main() -> Result<()> {
    let run_root =
        PathBuf::from(FSV_ROOT).join(format!("run-{}", chrono::Utc::now().timestamp_millis()));
    fs::create_dir_all(&run_root)?;

    let healthy_case = run_healthy_mixed_case(&run_root.join("mixed"))?;
    let malformed_case = run_malformed_fail_closed_case(&run_root.join("malformed"))?;
    let cases = vec![healthy_case, malformed_case];
    let all_passed = cases
        .iter()
        .all(|case| case["pass"].as_bool().unwrap_or(false));
    let report = json!({
        "task": "TASK-PY-G-337",
        "issue": 337,
        "all_passed": all_passed,
        "source_of_truth": {
            "verification_cf": context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
            "oracle_cf": context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
            "signal_drop_cf": context_graph_mejepa_cf::CF_MEJEPA_SIGNAL_DROP_LOG,
            "status_file": "self_optimization_status.json"
        },
        "cases": cases
    });
    ensure!(all_passed, "one or more FSV cases failed: {report}");

    let report_path = run_root.join("observe_missing_prediction_fsv.json");
    let bytes = serde_json::to_vec_pretty(&report)?;
    fs::write(&report_path, &bytes)?;
    let readback = fs::read(&report_path)?;
    ensure!(readback == bytes, "FSV report readback differs");
    let sha256 = hex::encode(Sha256::digest(&readback));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "report_path": report_path,
            "sha256": sha256
        }))?
    );
    Ok(())
}

fn run_healthy_mixed_case(root: &Path) -> Result<Value> {
    fs::create_dir_all(root)?;
    let db_path = root.join("db");
    let db = open_test_db(&db_path)?;
    let config = test_config(root);
    fs::create_dir_all(&config.test_outcome_root)?;

    let matched_session = [0x31; 16];
    let missing_session = [0x32; 16];
    let skipped_session = [0x33; 16];
    let matched_session_hex = hex::encode(matched_session);
    let missing_session_hex = hex::encode(missing_session);
    let skipped_session_hex = hex::encode(skipped_session);
    let prediction = test_prediction(matched_session);
    RocksDbInferStore::new(db.clone()).write_live_prediction(&prediction)?;

    let outcome_path = config.test_outcome_root.join("mixed-events.jsonl");
    let matched_event_id = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let missing_event_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let skipped_event_id = "cccccccccccccccccccccccccccccccc";
    let events = [
        test_event(&matched_session_hex, matched_event_id, "fail"),
        test_event(&missing_session_hex, missing_event_id, "pass"),
        test_event(&skipped_session_hex, skipped_event_id, "skipped"),
    ];
    let mut jsonl = String::new();
    for event in events {
        jsonl.push_str(&serde_json::to_string(&event)?);
        jsonl.push('\n');
    }
    fs::write(&outcome_path, jsonl)?;

    let mut state = SchedulerState::open(db.clone(), config.clone())?;
    state.run_tick(TickName::Observe)?;
    let status = read_status_snapshot(state.status_path())?;
    let observe = status
        .last_success
        .get("observe")
        .context("missing observe success")?;
    let bindings = observe
        .details
        .get("test_outcome_bindings")
        .context("missing binding report")?;

    let prediction_id = hex::encode([0x46; 16]);
    let verification_key = format!("{matched_session_hex}:{matched_event_id}:{prediction_id}");
    let verification_row = read_json_cf_row(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
        verification_key.as_bytes(),
    )?;
    let signal_drops = context_graph_mejepa::load_signal_drop_log_entries(db.as_ref(), 10)?;

    let before_reopen_counts = cf_counts(db.as_ref())?;
    drop(state);
    drop(db);

    let reopened = open_test_db(&db_path)?;
    let reopened_counts = cf_counts(reopened.as_ref())?;
    let reopened_verification_row = read_json_cf_row(
        reopened.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS,
        verification_key.as_bytes(),
    )?;
    let reopened_signal_drops =
        context_graph_mejepa::load_signal_drop_log_entries(reopened.as_ref(), 10)?;

    let pass = !status.last_error.contains_key("observe")
        && bindings["events_scanned"] == json!(3)
        && bindings["events_bound"] == json!(1)
        && bindings["events_missing_prediction"] == json!(1)
        && bindings["events_skipped_non_hard_label"] == json!(1)
        && before_reopen_counts == reopened_counts
        && reopened_counts["oracle"] == json!(1)
        && reopened_counts["verification"] == json!(1)
        && reopened_counts["signal_drop"] == json!(1)
        && verification_row == reopened_verification_row
        && verification_row["agreement"] == json!("refuted")
        && signal_drops.len() == 1
        && reopened_signal_drops.len() == 1
        && reopened_signal_drops[0].error_code == "MEJEPA_TEST_OUTCOME_NO_PREDICTION"
        && reopened_signal_drops[0].context.get("session_id") == Some(&missing_session_hex)
        && reopened_signal_drops[0].context.get("event_id") == Some(&missing_event_id.to_string());

    Ok(json!({
        "case": "matched_missing_and_skipped_events",
        "trigger": "observe tick over one matched, one missing-prediction, and one skipped test outcome",
        "expected": {
            "events_bound": 1,
            "events_missing_prediction": 1,
            "events_skipped_non_hard_label": 1,
            "last_error_observe": null,
            "oracle_rows": 1,
            "verification_rows": 1,
            "signal_drop_rows": 1
        },
        "actual": {
            "status_last_error_observe": status.last_error.get("observe"),
            "binding_report": bindings,
            "before_reopen_counts": before_reopen_counts,
            "reopened_counts": reopened_counts,
            "verification_key": verification_key,
            "verification_row": verification_row,
            "signal_drop_error_codes": reopened_signal_drops.iter().map(|entry| entry.error_code.clone()).collect::<Vec<_>>()
        },
        "pass": pass
    }))
}

fn run_malformed_fail_closed_case(root: &Path) -> Result<Value> {
    fs::create_dir_all(root)?;
    let db = open_test_db(&root.join("db"))?;
    let config = test_config(root);
    fs::create_dir_all(&config.test_outcome_root)?;
    let outcome_path = config.test_outcome_root.join("bad-events.jsonl");
    let event = test_event(
        "not-hex-session",
        "dddddddddddddddddddddddddddddddd",
        "pass",
    );
    fs::write(
        &outcome_path,
        format!("{}\n", serde_json::to_string(&event)?),
    )?;

    let mut state = SchedulerState::open(db.clone(), config)?;
    state.run_tick(TickName::Observe)?;
    let status = read_status_snapshot(state.status_path())?;
    let observe_error = status.last_error.get("observe");
    let signal_drop_count = count_cf(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_SIGNAL_DROP_LOG,
    )?;
    let pass = observe_error.and_then(|record| record.error_code.as_deref())
        == Some("MEJEPA_HEAL_INVALID_STATE")
        && signal_drop_count == 0;

    Ok(json!({
        "case": "malformed_session_id_still_fails_closed",
        "trigger": "observe tick over invalid test-outcome session id",
        "expected": {
            "last_error_observe": "MEJEPA_HEAL_INVALID_STATE",
            "signal_drop_rows": 0
        },
        "actual": {
            "status_last_error_observe": observe_error,
            "signal_drop_rows": signal_drop_count
        },
        "pass": pass
    }))
}

fn open_test_db(path: &Path) -> Result<Arc<DB>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.create_missing_column_families(true);
    let descriptors = context_graph_mejepa_cf::all_hygiene_referenced_cfs()
        .into_iter()
        .map(|cf| ColumnFamilyDescriptor::new(cf, Options::default()))
        .collect::<Vec<_>>();
    Ok(Arc::new(DB::open_cf_descriptors(&opts, path, descriptors)?))
}

fn test_config(root: &Path) -> SelfOptimConfig {
    SelfOptimConfig {
        status_path: root.join("self_optimization_status.json"),
        hygiene_archive_root: root.join("archive"),
        witness_chain_path: root.join("witness-chain.bin"),
        test_outcome_root: root.join("test-outcomes"),
        ..SelfOptimConfig::default().with_all_periods(Duration::from_millis(20))
    }
}

fn test_prediction(session_id: [u8; 16]) -> RealityPrediction {
    RealityPrediction::try_new(RealityPrediction {
        prediction_id: [0x46; 16],
        witness_hash: WitnessHash([0x47; 32]),
        task_id: TaskId("task-py-g-337-fsv".to_string()),
        session_id,
        language: Language::Python,
        covered_chunks: Vec::new(),
        verdict: Verdict::Pass,
        confidence_interval: ConformalInterval {
            lower: 0.6,
            upper: 0.8,
            ..ConformalInterval::default()
        },
        predicted_oracle_pass: 0.95,
        predicted_test_pass: vec![0.9, 0.8],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.1,
        outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2).unwrap(),
        calibrated_confidence: 0.7,
        degraded_status: false,
        granger_attestations: BTreeMap::from([("task-py-g-337:fsv".to_string(), 0.9)]),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: Vec::new(),
        predicted_works: Vec::new(),
        predicted_uncovered_paths: Vec::new(),
        predicted_flaky_tests: Vec::new(),
        guard_violations: Vec::new(),
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
        reality_impact: None,
        provenance: PredictionProvenance {
            predictor_version: "task-py-g-337-fsv".to_string(),
            constellation_version: "task-py-g-337-fsv".to_string(),
            calibration_version: "task-py-g-337-fsv".to_string(),
            active_pointer: hex::encode([0x46; 16]),
        },
        source_panel_sha: [0x5b; 32],
        calibration_version: "task-py-g-337-fsv".to_string(),
        created_at_unix_ms: 1_778_000_000_000,
        matched_fingerprint: None,
        unknown_candidate_id: None,
    })
    .unwrap()
}

fn test_event(session_id: &str, event_id: &str, outcome: &str) -> Value {
    json!({
        "event_id": event_id,
        "ts": 1_778_000_000_010i64,
        "session_id": session_id,
        "tool_use_id": format!("tool-{event_id}"),
        "command": "pytest tests/test_math.py",
        "framework": "pytest",
        "test_id": "tests/test_math.py::test_add",
        "outcome": outcome,
        "duration_ms": 10,
        "error_log": "",
        "source": "task-py-g-337-fsv",
        "sequence": 0,
        "line_no": 1
    })
}

fn cf_counts(db: &DB) -> Result<Value> {
    Ok(json!({
        "oracle": count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS)?,
        "verification": count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_PREDICTION_VERIFICATIONS)?,
        "signal_drop": count_cf(db, context_graph_mejepa_cf::CF_MEJEPA_SIGNAL_DROP_LOG)?,
    }))
}

fn count_cf(db: &DB, name: &str) -> Result<usize> {
    let cf = db.cf_handle(name).context("missing cf")?;
    Ok(db.iterator_cf(cf, IteratorMode::Start).count())
}

fn read_json_cf_row(db: &DB, cf_name: &str, key: &[u8]) -> Result<Value> {
    let cf = db.cf_handle(cf_name).context("missing cf")?;
    let bytes = db
        .get_cf(cf, key)?
        .with_context(|| format!("missing row {}", String::from_utf8_lossy(key)))?;
    Ok(serde_json::from_slice(&bytes)?)
}
