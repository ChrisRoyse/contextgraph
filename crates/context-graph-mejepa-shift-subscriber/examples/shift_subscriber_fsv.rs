// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use context_graph_mejepa::{
    open_infer_rocksdb, CalibrationExample, CalibrationStore, EmbedderId, MejepaStore, PanelId,
    RealityPrediction, RocksDbInferStore, TrainCertSummary,
};
use context_graph_mejepa_shift_subscriber::{MeJepaShiftSubscriberConfig, ShiftSubscriber};
use context_graph_mejepa_train::dda::{count_persisted_dda_signals, read_dda_signals};
use rocksdb::IteratorMode;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const ROOT: &str = "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(ROOT);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;
    let infer_db = root.join("infer-db");
    let embedder_cache = infer_db.with_file_name("mejepa-embedder-cache");
    let panel_db = root.join("panel-db");
    let repo = root.join("repo");
    let log_dir = root.join("cgreality-shift-log");
    std::fs::create_dir_all(&repo)?;
    std::fs::create_dir_all(&log_dir)?;
    seed_infer_db(&infer_db)?;

    let session = "22222222222222222222222222222222";
    let fixture = write_shift_fixture(&root, &repo, &log_dir, session)?;

    let before = read_state(&infer_db, session)?;
    let subscriber = ShiftSubscriber::open(MeJepaShiftSubscriberConfig {
        infer_db_path: infer_db.clone(),
        panel_db_path: panel_db.clone(),
        repo_root: repo,
        shift_log_dir: log_dir.clone(),
        l_step_observe_threshold: 0.05,
        max_concurrent_shifts: 1,
        lag_alert_threshold_shifts: 500,
        lag_alert_sustain_seconds: 60,
        tail_poll_interval_ms: 10,
    })?;
    let summary = subscriber.run_until_idle().await?;
    let watermark = subscriber.read_watermarks()?;
    drop(subscriber);
    let after = read_state(&infer_db, session)?;

    if after["prediction_count"] != json!(100) {
        return Err("prediction_count readback != 100".into());
    }
    if after["dda_signal_count"] != json!(100) {
        return Err("dda_signal_count readback != 100".into());
    }
    if after["oracle_verdict_count"] != json!(20) {
        return Err("oracle_verdict_count readback != 20".into());
    }
    if after["phase7_train_cert_count"] != json!(20) {
        return Err("phase7_train_cert_count readback != 20".into());
    }
    if watermark[session].last_consumed_shift_id != fixture.final_shift_id {
        return Err("watermark did not advance to expected shift_id".into());
    }
    if watermark[session].last_consumed_byte_offset != fixture.total_bytes {
        return Err("watermark byte offset did not match JSONL line length".into());
    }
    if summary.observed != 15 {
        return Err("observed_count readback != 15".into());
    }
    if summary.dropped_l_step_below_threshold != 5 {
        return Err("dropped_l_step_below_threshold readback != 5".into());
    }
    if summary.latency_ms.p50 >= 5_000 || summary.latency_ms.p99 >= 30_000 {
        return Err("latency budget failed for 100-shift FSV stream".into());
    }

    let source_of_truth = json!({
        "infer_db": file_sot(&infer_db),
        "embedder_cache": file_sot(&embedder_cache),
        "panel_db": file_sot(&panel_db),
        "shift_log": file_sot(&log_dir.join(format!("{session}.jsonl"))),
    });
    let watermark_evidence = json!({
        "source_of_truth": source_of_truth,
        "before": before,
        "after": after,
        "watermark_readback": watermark,
    });
    let observe_evidence = json!({
        "source_of_truth": source_of_truth,
        "summary": summary,
        "expected": {
            "processed": 100,
            "observed": 15,
            "dropped_l_step_below_threshold": 5,
            "watermark_shift_id": fixture.final_shift_id,
        }
    });
    let latency_evidence = json!({
        "source_of_truth": source_of_truth,
        "processed_shifts": 100,
        "latency_ms": summary.latency_ms,
        "p50_budget_ms": 5000.0,
        "p99_budget_ms": 30000.0,
        "passes": summary.latency_ms.p50 < 5000 && summary.latency_ms.p99 < 30000,
    });
    let edge_case_evidence = json!({
        "source_of_truth": {
            "edge_case_roots": file_sot(&root.join("edge-cases")),
        },
        "edge_cases": run_edge_cases(&root).await?,
    });
    let mcp_evidence = json!({
        "source_of_truth": source_of_truth,
        "sister_fsv": "cargo test -p context-graph-mcp phase7_mcp_fsv_happy_path_and_edges -- --nocapture",
        "phase7_mcp_tools": [
            "mcp__cgreality__mejepa_observe_shift",
            "mcp__cgreality__mejepa_subscriber_status",
            "mcp__cgreality__mejepa_capture_audit"
        ]
    });

    let mut files = BTreeMap::new();
    for (name, value) in [
        ("watermark.json", watermark_evidence),
        ("observe-evidence.json", observe_evidence),
        ("predict-latency-evidence.json", latency_evidence),
        ("edge-cases.json", edge_case_evidence),
        ("mcp-tool-roundtrip-evidence.json", mcp_evidence),
    ] {
        let path = root.join(name);
        write_json_0600(&path, &value)?;
        files.insert(name, sha256_file(&path)?);
    }
    let manifest = json!({
        "phase": "phase7-shift-subscriber",
        "source_of_truth": source_of_truth,
        "fixture": fixture,
        "evidence_files_sha256": files,
    });
    write_json_0600(&root.join("manifest.json"), &manifest)?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}

fn seed_infer_db(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let db = open_infer_rocksdb(path)?;
    let calibration = CalibrationStore::new(db.clone(), 30)?;
    let examples = (0..40)
        .map(|idx| CalibrationExample {
            language: context_graph_mejepa::Language::Python,
            predicted_test_pass: vec![if idx % 10 == 0 { 0.2 } else { 0.95 }],
            actual_test_pass: vec![if idx % 10 == 0 { 0.0 } else { 1.0 }],
        })
        .collect::<Vec<_>>();
    let norms = vec![0.01; examples.len()];
    calibration.calibrate(
        &examples,
        &norms,
        0.10,
        30,
        0.30,
        [7; 32],
        BTreeMap::<EmbedderId, String>::new(),
    )?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS)
        .ok_or("missing train cert CF")?;
    let cert = TrainCertSummary {
        step: 1,
        delta_omega: 0.8,
        delta_xi: 0.8,
        witness_offset: 44,
    };
    db.put_cf(
        cf,
        b"cert:shift-subscriber-fsv:0001",
        bincode::serialize(&cert)?,
    )?;
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct FixtureSummary {
    shift_count: usize,
    oracle_attached: usize,
    expected_observed: usize,
    expected_dropped_l_step: usize,
    final_shift_id: String,
    total_bytes: u64,
    shift_log_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
struct EdgeCaseEvidence {
    case: String,
    trigger: String,
    source_of_truth: Value,
    before: Value,
    outcome: Value,
    after: Value,
    watermark_readback: Value,
    expected: Value,
}

async fn run_edge_cases(root: &Path) -> Result<Vec<EdgeCaseEvidence>, Box<dyn std::error::Error>> {
    let edge_root = root.join("edge-cases");
    std::fs::create_dir_all(&edge_root)?;
    Ok(vec![
        run_single_line_edge_case(
            &edge_root,
            "non_code_shift_advances_watermark_without_prediction_or_dda",
            "33333333333333333333333333333333",
            non_code_shift_line("33333333333333333333333333333333")?,
            json!({
                "processed": 1,
                "prediction_count": 0,
                "dda_signal_count": 0,
                "watermark_shift_id": "01J00000000000000000101",
            }),
        )
        .await?,
        run_single_line_edge_case(
            &edge_root,
            "empty_post_shift_chunk_fails_closed_before_state_write",
            "44444444444444444444444444444444",
            code_shift_line(
                &edge_root,
                "empty_post_shift_chunk_fails_closed_before_state_write",
                "44444444444444444444444444444444",
                "01J00000000000000000102",
                "def removed():\n    return 1\n",
                "",
                json!({ "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment()) }),
            )?,
            json!({
                "error_code": "MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT",
                "prediction_count": 0,
                "dda_signal_count": 0,
                "watermark_count": 0,
            }),
        )
        .await?,
        run_single_line_edge_case(
            &edge_root,
            "low_l_step_persists_truth_but_does_not_observe",
            "55555555555555555555555555555555",
            code_shift_line(
                &edge_root,
                "low_l_step_persists_truth_but_does_not_observe",
                "55555555555555555555555555555555",
                "01J00000000000000000103",
                "def answer():\n    return 1\n",
                "def answer():\n    return 2\n",
                json!({
                    "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment()),
                    "l_step": 0.01,
                    "delta_p": 0.9,
                    "delta_k": 0.8,
                    "delta_omega": 0.7,
                    "delta_xi": 0.6,
                    "actual_test_pass": [1.0]
                }),
            )?,
            json!({
                "processed": 1,
                "observed": 0,
                "dropped_l_step_below_threshold": 1,
                "prediction_count": 1,
                "dda_signal_count": 1,
                "oracle_verdict_count": 1,
                "phase7_train_cert_count": 1,
            }),
        )
        .await?,
    ])
}

async fn run_single_line_edge_case(
    edge_root: &Path,
    case: &str,
    session: &str,
    line: String,
    expected: Value,
) -> Result<EdgeCaseEvidence, Box<dyn std::error::Error>> {
    let case_root = edge_root.join(case);
    let infer_db = case_root.join("infer-db");
    let panel_db = case_root.join("panel-db");
    let repo = case_root.join("repo");
    let log_dir = case_root.join("cgreality-shift-log");
    std::fs::create_dir_all(&repo)?;
    std::fs::create_dir_all(&log_dir)?;
    seed_infer_db(&infer_db)?;
    let log_path = log_dir.join(format!("{session}.jsonl"));
    std::fs::write(&log_path, format!("{line}\n"))?;

    let before = read_state(&infer_db, session)?;
    println!(
        "EDGE BEFORE {case}: {}",
        serde_json::to_string_pretty(&before)?
    );
    let subscriber = ShiftSubscriber::open(MeJepaShiftSubscriberConfig {
        infer_db_path: infer_db.clone(),
        panel_db_path: panel_db.clone(),
        repo_root: repo,
        shift_log_dir: log_dir.clone(),
        l_step_observe_threshold: 0.05,
        max_concurrent_shifts: 1,
        lag_alert_threshold_shifts: 500,
        lag_alert_sustain_seconds: 60,
        tail_poll_interval_ms: 10,
    })?;
    let outcome = match subscriber.run_until_idle().await {
        Ok(summary) => json!({ "ok": true, "summary": summary }),
        Err(err) => json!({
            "ok": false,
            "error_code": err.code(),
            "error": err.to_string(),
        }),
    };
    let watermarks = subscriber.read_watermarks()?;
    drop(subscriber);
    let after = read_state(&infer_db, session)?;
    println!(
        "EDGE AFTER {case}: {}",
        serde_json::to_string_pretty(&after)?
    );
    println!(
        "EDGE OUTCOME {case}: {}",
        serde_json::to_string_pretty(&outcome)?
    );
    verify_edge_expectations(case, &outcome, &after, &watermarks)?;
    Ok(EdgeCaseEvidence {
        case: case.to_string(),
        trigger: line,
        source_of_truth: json!({
            "infer_db": file_sot(&infer_db),
            "embedder_cache": file_sot(&infer_db.with_file_name("mejepa-embedder-cache")),
            "panel_db": file_sot(&panel_db),
            "shift_log": file_sot(&log_path),
        }),
        before,
        outcome,
        after,
        watermark_readback: json!(watermarks),
        expected,
    })
}

fn verify_edge_expectations(
    case: &str,
    outcome: &Value,
    after: &Value,
    watermarks: &BTreeMap<String, context_graph_mejepa_shift_subscriber::WatermarkRecord>,
) -> Result<(), Box<dyn std::error::Error>> {
    match case {
        "non_code_shift_advances_watermark_without_prediction_or_dda" => {
            require_json_u64(outcome, "/summary/processed", 1)?;
            require_json_u64(after, "/prediction_count", 0)?;
            require_json_u64(after, "/dda_signal_count", 0)?;
            if watermarks.len() != 1 {
                return Err(format!("edge {case}: watermark count != 1").into());
            }
        }
        "empty_post_shift_chunk_fails_closed_before_state_write" => {
            if outcome.pointer("/ok").and_then(Value::as_bool) != Some(false) {
                return Err(format!("edge {case}: expected failure outcome").into());
            }
            if outcome.pointer("/error_code").and_then(Value::as_str)
                != Some("MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT")
            {
                return Err(format!("edge {case}: unexpected error code").into());
            }
            require_json_u64(after, "/prediction_count", 0)?;
            require_json_u64(after, "/dda_signal_count", 0)?;
            if !watermarks.is_empty() {
                return Err(format!("edge {case}: watermark advanced on failure").into());
            }
        }
        "low_l_step_persists_truth_but_does_not_observe" => {
            require_json_u64(outcome, "/summary/processed", 1)?;
            require_json_u64(outcome, "/summary/observed", 0)?;
            require_json_u64(outcome, "/summary/dropped_l_step_below_threshold", 1)?;
            require_json_u64(after, "/prediction_count", 1)?;
            require_json_u64(after, "/dda_signal_count", 1)?;
            require_json_u64(after, "/oracle_verdict_count", 1)?;
            require_json_u64(after, "/phase7_train_cert_count", 1)?;
        }
        other => return Err(format!("unknown edge case {other}").into()),
    }
    Ok(())
}

fn require_json_u64(value: &Value, pointer: &str, expected: u64) -> Result<(), String> {
    let actual = value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{pointer} missing or not u64"))?;
    if actual != expected {
        return Err(format!("{pointer} expected {expected}, got {actual}"));
    }
    Ok(())
}

fn non_code_shift_line(session: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(serde_json::to_string(&json!({
        "shift_id": "01J00000000000000000101",
        "timestamp_unix_ns": 1_772_000_000_000_000_000u128,
        "tool_name": "optimizer_record_decision",
        "tool_use_id": null,
        "session_id": session,
        "subject": {"type": "decision", "policy": "synthetic-fsv"},
        "before": {},
        "after": {},
        "delta_summary": {"summary": "non-code shift should advance only the watermark"},
        "verification": {},
        "harness_transition_path": null
    }))?)
}

fn code_shift_line(
    edge_root: &Path,
    case: &str,
    session: &str,
    shift_id: &str,
    before: &str,
    after: &str,
    verification: Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let case_root = edge_root.join(case);
    let before_path = case_root.join("before.py");
    let after_path = case_root.join("repo").join("edge.py");
    let parent = after_path.parent().ok_or("after path has no parent")?;
    std::fs::create_dir_all(parent)?;
    std::fs::write(&before_path, before)?;
    std::fs::write(&after_path, after)?;
    Ok(serde_json::to_string(&json!({
        "shift_id": shift_id,
        "timestamp_unix_ns": 1_772_000_000_000_000_000u128,
        "tool_name": "harness_apply_line_window_edit",
        "tool_use_id": format!("toolu-edge-{case}"),
        "session_id": session,
        "subject": {
            "task_id": format!("edge_case_{case}"),
            "path": "edge.py",
            "language": "python",
            "tests": ["test_edge"],
            "problem_statement": "edge case source-of-truth verification",
            "os": std::env::consts::OS
        },
        "before": {
            "text_source_of_truth": file_sot(&before_path),
            "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(before.as_bytes())))
        },
        "after": {
            "source_of_truth": file_sot(&after_path),
            "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(after.as_bytes())))
        },
        "delta_summary": {"commit_message": format!("edge case {case}")},
        "verification": verification,
        "harness_transition_path": null
    }))?)
}

fn write_shift_fixture(
    root: &Path,
    repo: &Path,
    log_dir: &Path,
    session: &str,
) -> Result<FixtureSummary, Box<dyn std::error::Error>> {
    let before_dir = root.join("preedit-before-text");
    std::fs::create_dir_all(&before_dir)?;
    let mut payload = String::new();
    let mut final_shift_id = String::new();
    for idx in 1..=100u64 {
        let shift_id = format!("01J{idx:020X}");
        final_shift_id = shift_id.clone();
        let before = format!("def answer():\n    return {}\n", idx - 1);
        let after = format!("def answer():\n    return {idx}\n");
        let before_path = before_dir.join(format!("answer_{idx:03}.before.py"));
        let after_path = repo.join(format!("answer_{idx:03}.py"));
        std::fs::write(&before_path, &before)?;
        std::fs::write(&after_path, &after)?;
        let mut verification = json!({
            "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment())
        });
        if idx <= 20 {
            let l_step = if idx <= 15 { 0.8 } else { 0.01 };
            verification["l_step"] = json!(l_step);
            verification["delta_p"] = json!(0.9);
            verification["delta_k"] = json!(0.8);
            verification["delta_omega"] = json!(0.7);
            verification["delta_xi"] = json!(0.6);
            verification["actual_test_pass"] = json!([1.0]);
        }
        let line = serde_json::to_string(&json!({
            "shift_id": shift_id,
            "timestamp_unix_ns": 1_772_000_000_000_000_000u128 + idx as u128 * 1_000_000u128,
            "tool_name": "harness_apply_line_window_edit",
            "tool_use_id": format!("toolu-shift-subscriber-fsv-{idx:03}"),
            "session_id": session,
            "subject": {
                "task_id": format!("shift_subscriber_fsv_attempt_{idx:03}"),
                "path": format!("answer_{idx:03}.py"),
                "language": "python",
                "tests": ["test_answer"],
                "problem_statement": "answer returns the indexed value",
                "os": std::env::consts::OS
            },
            "before": {
                "text_source_of_truth": file_sot(&before_path),
                "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(before.as_bytes())))
            },
            "after": {
                "source_of_truth": file_sot(&after_path),
                "sha256": format!("sha256:{}", hex::encode(context_graph_mejepa::sha256_bytes(after.as_bytes())))
            },
            "delta_summary": {"commit_message": format!("phase7 shift subscriber fsv {idx}")},
            "verification": verification,
            "harness_transition_path": null
        }))?;
        payload.push_str(&line);
        payload.push('\n');
    }
    let path = log_dir.join(format!("{session}.jsonl"));
    std::fs::write(&path, &payload)?;
    Ok(FixtureSummary {
        shift_count: 100,
        oracle_attached: 20,
        expected_observed: 15,
        expected_dropped_l_step: 5,
        final_shift_id,
        total_bytes: payload.len() as u64,
        shift_log_sha256: sha256_file(&path)?,
    })
}

fn read_state(infer_db: &Path, session: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let db = open_infer_rocksdb(infer_db)?;
    let store = RocksDbInferStore::new(db);
    let session = context_graph_mejepa_shift_subscriber::decode_session_hex32(session)?;
    let predictions = store.read_live_predictions(session, 1000)?;
    let db = store.db();
    let oracle_verdict_count = count_cf(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_ORACLE_VERDICTS,
        None,
    )?;
    let dda_signal_count = count_persisted_dda_signals(db.as_ref())?;
    let phase7_train_cert_count = count_cf(
        db.as_ref(),
        context_graph_mejepa_cf::CF_MEJEPA_TRAIN_CERTS,
        Some(b"cert:phase7:"),
    )?;
    Ok(json!({
        "prediction_count": predictions.len(),
        "oracle_verdict_count": oracle_verdict_count,
        "dda_signal_count": dda_signal_count,
        "phase7_train_cert_count": phase7_train_cert_count,
        "dda_signal_samples": dda_signal_samples(db.as_ref(), &predictions)?,
        "prediction_ids": predictions.iter().map(|row| hex::encode(row.prediction_id)).collect::<Vec<_>>(),
    }))
}

fn dda_signal_samples(
    db: &rocksdb::DB,
    predictions: &[RealityPrediction],
) -> Result<Value, Box<dyn std::error::Error>> {
    let mut out = Vec::new();
    for (label, prediction) in [
        ("newest", predictions.first()),
        ("oldest", predictions.last()),
    ] {
        let Some(prediction) = prediction else {
            continue;
        };
        let Some(chunk_id) = prediction.covered_chunks.first() else {
            continue;
        };
        let panel_id = PanelId(prediction.source_panel_sha);
        let signals = read_dda_signals(db, &panel_id, chunk_id)?
            .ok_or_else(|| format!("missing DDA row for {label} prediction"))?;
        out.push(json!({
            "label": label,
            "prediction_id": hex::encode(prediction.prediction_id),
            "panel_id": hex::encode(prediction.source_panel_sha),
            "chunk_id": chunk_id.0,
            "embedder_count": signals.embedder_count(),
            "per_embedder_cosine": signals.per_embedder_cosine,
            "pairwise_cosine_upper": signals.pairwise_cosine_upper,
            "blind_spot_z_scores": signals.blind_spot_z_scores,
        }));
    }
    Ok(Value::Array(out))
}

fn count_cf(
    db: &rocksdb::DB,
    cf_name: &str,
    key_prefix: Option<&[u8]>,
) -> Result<usize, Box<dyn std::error::Error>> {
    let cf = db
        .cf_handle(cf_name)
        .ok_or_else(|| format!("missing CF {cf_name}"))?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, _value) = item?;
        if key_prefix
            .map(|prefix| key.starts_with(prefix))
            .unwrap_or(true)
        {
            count += 1;
        }
    }
    Ok(count)
}

fn write_json_0600<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = serde_json::to_vec_pretty(value)?;
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let readback = std::fs::read(path)?;
    if readback != bytes {
        return Err(format!("{} readback bytes differed", path.display()).into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(format!("{} mode {mode:o} != 600", path.display()).into());
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn file_sot(path: &Path) -> String {
    format!("file:{}", path.display())
}
