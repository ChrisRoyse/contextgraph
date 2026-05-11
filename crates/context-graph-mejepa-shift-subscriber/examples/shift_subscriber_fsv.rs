// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use context_graph_mejepa::{
    open_infer_rocksdb, CalibrationExample, CalibrationStore, EmbedderId, MejepaStore,
    RocksDbInferStore, TrainCertSummary,
};
use context_graph_mejepa_shift_subscriber::{MeJepaShiftSubscriberConfig, ShiftSubscriber};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const ROOT: &str = "/tmp/contextgraph-mejepa-shift-subscriber-fsv";

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(ROOT);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root)?;
    let infer_db = root.join("infer-db");
    let panel_db = root.join("panel-db");
    let repo = root.join("repo");
    let log_dir = root.join("cgreality-shift-log");
    std::fs::create_dir_all(&repo)?;
    std::fs::create_dir_all(&log_dir)?;
    seed_infer_db(&infer_db)?;

    let session = "22222222222222222222222222222222";
    let shift_id = "01J0123456789ABCDEF4567";
    let line = write_shift_fixture(&log_dir, session, shift_id)?;

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
    let started = Instant::now();
    let summary = subscriber.run_until_idle().await?;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    let watermark = subscriber.read_watermarks()?;
    drop(subscriber);
    let after = read_state(&infer_db, session)?;

    if after["prediction_count"] != json!(1) {
        return Err("prediction_count readback != 1".into());
    }
    if watermark[session].last_consumed_shift_id != shift_id {
        return Err("watermark did not advance to expected shift_id".into());
    }
    if watermark[session].last_consumed_byte_offset != line.len() as u64 + 1 {
        return Err("watermark byte offset did not match JSONL line length".into());
    }
    if summary.observed != 1 {
        return Err("oracle-attached shift was not observed".into());
    }

    let source_of_truth = json!({
        "infer_db": file_sot(&infer_db),
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
            "processed": 1,
            "observed": 1,
            "watermark_shift_id": shift_id,
        }
    });
    let latency_evidence = json!({
        "source_of_truth": source_of_truth,
        "processed_shifts": 1,
        "wall_latency_ms": latency_ms,
        "p50_budget_ms": 5000.0,
        "p99_budget_ms": 30000.0,
        "passes": latency_ms < 5000.0,
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
        ("mcp-tool-roundtrip-evidence.json", mcp_evidence),
    ] {
        let path = root.join(name);
        write_json_0600(&path, &value)?;
        files.insert(name, sha256_file(&path)?);
    }
    let manifest = json!({
        "phase": "phase7-shift-subscriber",
        "source_of_truth": source_of_truth,
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

fn write_shift_fixture(
    log_dir: &Path,
    session: &str,
    shift_id: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let before = "def answer():\n    return 3\n";
    let after = "def answer():\n    return 4\n";
    let line = serde_json::to_string(&json!({
        "shift_id": shift_id,
        "timestamp_unix_ns": 1_772_000_000_000_000_000u128,
        "tool_name": "harness_apply_line_window_edit",
        "tool_use_id": "toolu-shift-subscriber-fsv",
        "session_id": session,
        "subject": {
            "task_id": "shift_subscriber_fsv_attempt",
            "path": "answer.py",
            "language": "python",
            "tests": ["test_answer"],
            "problem_statement": "answer returns four",
            "os": std::env::consts::OS
        },
        "before": {"text": before},
        "after": {"text": after},
        "delta_summary": {"commit_message": "phase7 shift subscriber fsv"},
        "verification": {
            "witness_chain_segment_hex": hex::encode(context_graph_mejepa::valid_witness_segment()),
            "l_step": 0.8,
            "delta_p": 0.9,
            "delta_k": 0.8,
            "delta_omega": 0.7,
            "delta_xi": 0.6,
            "actual_test_pass": [1.0]
        },
        "harness_transition_path": null
    }))?;
    std::fs::write(
        log_dir.join(format!("{session}.jsonl")),
        format!("{line}\n"),
    )?;
    Ok(line)
}

fn read_state(infer_db: &Path, session: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let db = open_infer_rocksdb(infer_db)?;
    let store = RocksDbInferStore::new(db);
    let session = context_graph_mejepa_shift_subscriber::decode_session_hex32(session)?;
    let predictions = store.read_live_predictions(session, 1000)?;
    Ok(json!({
        "prediction_count": predictions.len(),
        "prediction_ids": predictions.iter().map(|row| hex::encode(row.prediction_id)).collect::<Vec<_>>(),
    }))
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
