use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, ensure, Context, Result};
use chrono::Utc;
use context_graph_cli::commands::mejepa_replay::{replay_prediction_cli, ReplayArgs};
use context_graph_mejepa::{
    count_cf, open_infer_rocksdb, parse_prediction_id_hex, replay_prediction_by_id,
    replay_prediction_from_db, AgentClaimGraph, ConformalInterval, ConformalSet, Language,
    MejepaStore, OracleOutcome, PredictionId, PredictionProvenance, RealityPrediction,
    ReasoningClass, RocksDbInferStore, TaskId, Verdict, WitnessHash,
};
use rocksdb::{WriteOptions, DB};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-prediction-replay-fsv";
const CF_MEJEPA_LIVE_PREDICTIONS: &str = "CF_MEJEPA_LIVE_PREDICTIONS";

#[tokio::main]
async fn main() -> Result<()> {
    let started_at_unix_ms = Utc::now().timestamp_millis();
    let fsv_root = PathBuf::from(FSV_ROOT);
    fs::create_dir_all(&fsv_root).context("create FSV root")?;
    let run_root = fsv_root.join(format!("run-{}-{}", started_at_unix_ms, std::process::id()));
    fs::create_dir_all(&run_root).context("create run root")?;
    let db_path = run_root.join("rocksdb");
    let build_release_sha = git_head_short()?;

    let db = open_infer_rocksdb(&db_path).context("open replay FSV DB")?;
    let cf_counts_before = cf_counts(db.as_ref())?;
    let store = RocksDbInferStore::new(db.clone());
    let predictions = (0..10).map(prediction).collect::<Result<Vec<_>>>()?;
    for prediction in &predictions {
        store
            .write_live_prediction(prediction)
            .context("write historical prediction")?;
    }
    db.flush_cf(live_prediction_cf(db.as_ref())?)
        .context("flush live prediction CF")?;
    drop(store);
    drop(db);

    let mut replay_reports = Vec::new();
    for prediction in &predictions {
        let report = replay_prediction_from_db(&db_path, PredictionId(prediction.prediction_id))
            .context("replay persisted historical prediction")?;
        ensure!(report.byte_equal, "replay report must be byte-equal");
        ensure!(
            report.semantic_equal,
            "replay report must be semantically equal"
        );
        ensure!(
            report.persisted_value_sha256 == report.replayed_value_sha256,
            "stored/replayed hashes must match"
        );
        replay_reports.push(report);
    }

    let cli_output = replay_prediction_cli(ReplayArgs {
        db_path: db_path.clone(),
        prediction_id: hex::encode(predictions[0].prediction_id),
    })
    .await
    .context("run CLI replay command handler")?;
    ensure!(
        cli_output.replay.byte_equal,
        "CLI replay must be byte-equal"
    );

    let boundary_cases = run_boundary_cases(&run_root).context("run replay boundary cases")?;
    ensure!(
        boundary_cases
            .iter()
            .all(|case| case["pass"].as_bool() == Some(true)),
        "all boundary cases must pass: {}",
        serde_json::to_string_pretty(&boundary_cases)?
    );

    let reopened = open_infer_rocksdb(&db_path).context("reopen replay FSV DB")?;
    let cf_counts_after = cf_counts(reopened.as_ref())?;
    let reopened_report = replay_prediction_by_id(
        reopened.as_ref(),
        Some(&db_path),
        PredictionId(predictions[9].prediction_id),
    )
    .context("replay after reopen")?;
    let readback_equal = reopened_report.byte_equal && reopened_report.semantic_equal;
    ensure!(readback_equal, "reopened replay readback mismatch");
    drop(reopened);

    let physical_artifacts = physical_artifacts(&db_path)?;
    ensure!(
        physical_artifacts["sst_file_count"].as_u64().unwrap_or(0) > 0,
        "expected at least one RocksDB SST file after flush"
    );

    let artifact_path = fsv_root.join("prediction_replay_fsv.json");
    let artifact = json!({
        "fsv_root": fsv_root,
        "task_id": "TASK-OBS-011",
        "issue": 46,
        "started_at_unix_ms": started_at_unix_ms,
        "build_release_sha": build_release_sha,
        "trigger": "cargo run --release -p context-graph-cli --example phase_f_prediction_replay_fsv",
        "db_path": db_path,
        "happy_path": [{
            "case": "replay_10_historical_predictions_byte_equal",
            "sot": CF_MEJEPA_LIVE_PREDICTIONS,
            "before": cf_counts_before,
            "after": cf_counts_after,
            "expected": {
                "prediction_count": 10,
                "byte_equal": true,
                "semantic_equal": true
            },
            "actual": {
                "replay_count": replay_reports.len(),
                "all_byte_equal": replay_reports.iter().all(|report| report.byte_equal),
                "all_semantic_equal": replay_reports.iter().all(|report| report.semantic_equal),
                "cli_output": cli_output
            },
            "pass": true,
            "evidence_path": artifact_path,
        }],
        "boundary_cases": boundary_cases,
        "all_passed": true,
        "cf_counts_before": cf_counts_before,
        "cf_counts_after": cf_counts_after,
        "readback_equal": readback_equal,
        "physical_artifacts": physical_artifacts,
    });
    context_graph_mejepa::eval::report::write_json_0600(&artifact_path, &artifact)
        .context("write FSV artifact")?;
    let readback: Value =
        serde_json::from_slice(&fs::read(&artifact_path).context("read FSV artifact")?)
            .context("decode FSV artifact")?;
    ensure!(readback == artifact, "FSV artifact readback mismatch");
    let artifact_sha256 = format!("{:x}", Sha256::digest(fs::read(&artifact_path)?));
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "artifact_path": artifact_path,
            "artifact_sha256": artifact_sha256,
            "all_passed": true,
            "readback_equal": readback_equal,
        }))?
    );
    Ok(())
}

fn prediction(id: u8) -> Result<RealityPrediction> {
    Ok(RealityPrediction::try_new(RealityPrediction {
        prediction_id: [id; 16],
        witness_hash: WitnessHash([id.wrapping_add(1); 32]),
        task_id: TaskId(format!("phase-f-replay-{id}")),
        session_id: [0x46; 16],
        language: Language::Rust,
        covered_chunks: Vec::new(),
        verdict: Verdict::Pass,
        confidence_interval: ConformalInterval {
            lower: 0.72,
            upper: 0.94,
            ..ConformalInterval::default()
        },
        predicted_oracle_pass: 0.89,
        predicted_test_pass: vec![0.86],
        predicted_runtime_trace: [0.0; 32],
        ood_score: 0.04,
        outcome_set: ConformalSet::try_new(vec![OracleOutcome::Pass], 0.1, 0.2)?,
        calibrated_confidence: 0.83,
        degraded_status: false,
        granger_attestations: BTreeMap::from([("phase-f-replay".to_string(), 1.0)]),
        predicted_failure_modes: Vec::new(),
        predicted_failed_tests: Vec::new(),
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
        predicted_reasoning_class: ReasoningClass::MostlyCorrect,
        agent_claim_graph: AgentClaimGraph::default(),
        claim_reconciliation: Vec::new(),
        reality_impact: None,
        provenance: PredictionProvenance {
            predictor_version: "phase-f-replay-predictor".to_string(),
            constellation_version: "phase-f-replay-constellation".to_string(),
            calibration_version: "phase-f-replay-calibration".to_string(),
            active_pointer: hex::encode([id; 16]),
        },
        source_panel_sha: [id.wrapping_add(2); 32],
        calibration_version: "phase-f-replay-calibration".to_string(),
        created_at_unix_ms: 1_778_660_000_000 + i64::from(id),
        matched_fingerprint: None,
        unknown_candidate_id: None,
    })?)
}

fn run_boundary_cases(run_root: &Path) -> Result<Vec<Value>> {
    let malformed_id = parse_prediction_id_hex("not-hex").expect_err("malformed id must fail");

    let missing_db = run_root.join("missing-rocksdb");
    {
        let db = open_infer_rocksdb(&missing_db).context("open missing boundary DB")?;
        let store = RocksDbInferStore::new(db);
        store
            .write_live_prediction(&prediction(0x21)?)
            .context("write missing-boundary row")?;
    }
    let missing_prediction = replay_prediction_from_db(&missing_db, PredictionId([0x22; 16]))
        .expect_err("missing prediction must fail");

    let corrupt_db = run_root.join("corrupt-rocksdb");
    {
        let db = open_infer_rocksdb(&corrupt_db).context("open corrupt boundary DB")?;
        let cf = live_prediction_cf(db.as_ref())?;
        let key = live_key([0x33; 16], 1_778_660_000_033, [0x33; 16]);
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        db.put_cf_opt(cf, key, b"not-bincode", &opts)
            .context("write corrupt live prediction row")?;
        db.flush_cf(cf).context("flush corrupt row")?;
    }
    let corrupt_row = replay_prediction_from_db(&corrupt_db, PredictionId([0x33; 16]))
        .expect_err("corrupt row must fail");

    let mismatch_db = run_root.join("mismatch-rocksdb");
    {
        let db = open_infer_rocksdb(&mismatch_db).context("open mismatch boundary DB")?;
        let cf = live_prediction_cf(db.as_ref())?;
        let payload = prediction(0x41)?;
        let key = live_key(payload.session_id, payload.created_at_unix_ms, [0x42; 16]);
        let value = bincode::serialize(&payload).context("serialize mismatch payload")?;
        let mut opts = WriteOptions::default();
        opts.set_sync(true);
        db.put_cf_opt(cf, key, value, &opts)
            .context("write mismatched live prediction row")?;
        db.flush_cf(cf).context("flush mismatched row")?;
    }
    let key_mismatch = replay_prediction_from_db(&mismatch_db, PredictionId([0x42; 16]))
        .expect_err("key mismatch must fail");

    Ok(vec![
        boundary_case(
            "malformed_prediction_id",
            "MEJEPA_PREDICTION_REPLAY_ID_INVALID",
            malformed_id,
        ),
        boundary_case(
            "missing_prediction_id",
            "MEJEPA_PREDICTION_REPLAY_NOT_FOUND",
            missing_prediction,
        ),
        boundary_case(
            "corrupt_prediction_row",
            "MEJEPA_INFER_BINCODE",
            corrupt_row,
        ),
        boundary_case(
            "key_payload_prediction_id_mismatch",
            "MEJEPA_PREDICTION_REPLAY_KEY_MISMATCH",
            key_mismatch,
        ),
    ])
}

fn boundary_case(
    name: &str,
    expected_substring: &str,
    err: context_graph_mejepa::MejepaInferError,
) -> Value {
    let actual = err.to_string();
    json!({
        "case": name,
        "expected": expected_substring,
        "actual": actual,
        "pass": actual.contains(expected_substring)
    })
}

fn live_key(session_id: [u8; 16], created_at_unix_ms: i64, prediction_id: [u8; 16]) -> Vec<u8> {
    let mut key = Vec::with_capacity(40);
    key.extend_from_slice(&session_id);
    key.extend_from_slice(&created_at_unix_ms.to_be_bytes());
    key.extend_from_slice(&prediction_id);
    key
}

fn live_prediction_cf(db: &DB) -> Result<&rocksdb::ColumnFamily> {
    db.cf_handle(CF_MEJEPA_LIVE_PREDICTIONS)
        .ok_or_else(|| anyhow!("missing column family {CF_MEJEPA_LIVE_PREDICTIONS}"))
}

fn cf_counts(db: &DB) -> Result<Value> {
    Ok(json!({
        CF_MEJEPA_LIVE_PREDICTIONS: count_cf(db, CF_MEJEPA_LIVE_PREDICTIONS)?,
    }))
}

fn physical_artifacts(db_path: &Path) -> Result<Value> {
    let mut total_bytes = 0u64;
    let mut sst_file_count = 0u64;
    let mut sst_files = Vec::new();
    collect_physical_artifacts(
        db_path,
        &mut total_bytes,
        &mut sst_file_count,
        &mut sst_files,
    )?;
    Ok(json!({
        "db_path": db_path,
        "total_file_bytes": total_bytes,
        "sst_file_count": sst_file_count,
        "sst_files": sst_files,
    }))
}

fn collect_physical_artifacts(
    path: &Path,
    total_bytes: &mut u64,
    sst_file_count: &mut u64,
    sst_files: &mut Vec<Value>,
) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read_dir {}", path.display()))? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let entry_path = entry.path();
        if metadata.is_dir() {
            collect_physical_artifacts(&entry_path, total_bytes, sst_file_count, sst_files)?;
            continue;
        }
        *total_bytes += metadata.len();
        if entry_path.extension().and_then(|ext| ext.to_str()) == Some("sst") {
            *sst_file_count += 1;
            sst_files.push(json!({
                "path": entry_path,
                "bytes": metadata.len(),
            }));
        }
    }
    Ok(())
}

fn git_head_short() -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .context("run git rev-parse")?;
    ensure!(output.status.success(), "git rev-parse failed");
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
