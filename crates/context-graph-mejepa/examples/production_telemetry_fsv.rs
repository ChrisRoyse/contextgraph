use context_graph_mejepa::eval::{LatencyTelemetry, ProductionTelemetryWindow, RocksDbEvalStore};
use context_graph_mejepa::open_infer_rocksdb;
use rocksdb::{IteratorMode, DB};
use serde::Serialize;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let root =
        PathBuf::from("/mnt/d/contextgraph/fsv/contextgraph-mejepa-production-telemetry-fsv");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let db_path = root.join("rocksdb");
    let db = open_infer_rocksdb(&db_path)?;
    let store = RocksDbEvalStore::new(db.clone())?;
    let window = happy_window("prod-window-2026-05-11T00");

    let before = physical_state(db.as_ref(), &store, &window.window_id)?;
    store.persist_production_telemetry(&window)?;
    let loaded = store
        .load_production_telemetry(&window.window_id)?
        .ok_or("production telemetry readback missing")?;
    require("loaded telemetry matches persisted value", loaded == window)?;
    let after = physical_state(db.as_ref(), &store, &window.window_id)?;
    require(
        "telemetry CF count increments by one",
        after["cf_count"].as_u64() == before["cf_count"].as_u64().map(|count| count + 1),
    )?;
    require(
        "raw row is present",
        after["raw_row"]["exists"] == Value::Bool(true),
    )?;

    let edge_cases = vec![
        edge_case(
            db.as_ref(),
            &store,
            "edge_empty_window_id",
            ProductionTelemetryWindow {
                window_id: String::new(),
                ..happy_window("unused")
            },
            "MEJEPA_EVAL_INVALID_INPUT",
        )?,
        edge_case(
            db.as_ref(),
            &store,
            "edge_zero_samples",
            ProductionTelemetryWindow {
                sample_count: 0,
                ..happy_window("edge-zero-samples")
            },
            "MEJEPA_EVAL_INVALID_INPUT",
        )?,
        edge_case(
            db.as_ref(),
            &store,
            "edge_non_monotonic_latency",
            ProductionTelemetryWindow {
                window_id: "edge-non-monotonic-latency".to_string(),
                latency: LatencyTelemetry {
                    p50_ms: 10.0,
                    p95_ms: 9.0,
                    p99_ms: 11.0,
                    max_ms: 12.0,
                },
                ..happy_window("edge-non-monotonic-latency")
            },
            "MEJEPA_EVAL_INVALID_INPUT",
        )?,
        edge_case(
            db.as_ref(),
            &store,
            "edge_ship_gate_flag_contradiction",
            ProductionTelemetryWindow {
                window_id: "edge-ship-gate-contradiction".to_string(),
                prediction_oracle_agreement: 0.80,
                ship_gate_target: 0.95,
                ship_gate_passed: true,
                ..happy_window("edge-ship-gate-contradiction")
            },
            "MEJEPA_EVAL_INVALID_INPUT",
        )?,
        edge_case(
            db.as_ref(),
            &store,
            "edge_vram_free_exceeds_total",
            ProductionTelemetryWindow {
                window_id: "edge-vram-free-exceeds-total".to_string(),
                vram_free_bytes: 35_000_000_000,
                vram_total_bytes: 34_190_458_880,
                ..happy_window("edge-vram-free-exceeds-total")
            },
            "MEJEPA_EVAL_INVALID_INPUT",
        )?,
    ];
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY)
        .ok_or("missing production telemetry column family")?;
    db.flush_cf(cf)?;
    drop(store);
    drop(db);

    let reopened_db = open_infer_rocksdb(&db_path)?;
    let reopened_store = RocksDbEvalStore::new(reopened_db.clone())?;
    let reopened_loaded = reopened_store
        .load_production_telemetry(&window.window_id)?
        .ok_or("production telemetry readback missing after reopen")?;
    require(
        "reopened telemetry equals persisted value",
        reopened_loaded == window,
    )?;
    let reopened_state = physical_state(reopened_db.as_ref(), &reopened_store, &window.window_id)?;

    let evidence = json!({
        "all_passed": true,
        "passes": true,
        "source_of_truth": {
            "rocksdb_path": db_path,
            "column_family": context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
            "window_key": window.window_id,
            "evidence_json": root.join("production-telemetry-evidence.json"),
        },
        "happy_path": {
            "before_state": before,
            "trigger": "RocksDbEvalStore::persist_production_telemetry",
            "after_state": after,
            "loaded_readback": loaded,
            "reopened_state": reopened_state,
            "reopened_loaded_readback": reopened_loaded,
            "passes": true,
        },
        "edge_cases": edge_cases,
    });
    let evidence_path = root.join("production-telemetry-evidence.json");
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require("evidence readback equality", readback == evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn happy_window(window_id: &str) -> ProductionTelemetryWindow {
    ProductionTelemetryWindow {
        window_id: window_id.to_string(),
        captured_at_unix_ms: 1_746_921_600_000,
        corpus_sha: "full-swebench-lite-300x8-fsv-sha".to_string(),
        sample_count: 2400,
        conformal_ece: 0.021,
        ood_auc: 0.932,
        gtau_pass_rate: 0.997,
        prediction_oracle_agreement: 0.966,
        ship_gate_target: 0.95,
        ship_gate_passed: true,
        latency: LatencyTelemetry {
            p50_ms: 8.4,
            p95_ms: 18.2,
            p99_ms: 31.7,
            max_ms: 44.0,
        },
        vram_free_bytes: 32_429_309_952,
        vram_total_bytes: 34_190_458_880,
        vram_required_bytes: 19_327_352_832,
        source: "production_eval_window".to_string(),
    }
}

fn edge_case(
    db: &DB,
    store: &RocksDbEvalStore,
    name: &str,
    window: ProductionTelemetryWindow,
    expected_code: &str,
) -> FsvResult<Value> {
    let before = physical_state(db, store, &window.window_id)?;
    let err = store
        .persist_production_telemetry(&window)
        .expect_err("invalid telemetry window must fail closed");
    let after = physical_state(db, store, &window.window_id)?;
    require(
        "edge error code matches expected",
        err.code() == expected_code,
    )?;
    require(
        "edge case does not change telemetry CF count",
        before["cf_count"] == after["cf_count"],
    )?;
    Ok(json!({
        "case": name,
        "before_state": before,
        "trigger": "persist invalid ProductionTelemetryWindow",
        "after_state": after,
        "error_code": err.code(),
        "error": err.to_string(),
        "passes": true,
    }))
}

fn physical_state(db: &DB, store: &RocksDbEvalStore, window_id: &str) -> FsvResult<Value> {
    let count = store.count_production_telemetry()?;
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY)
        .ok_or("missing production telemetry column family")?;
    let raw = if window_id.is_empty() {
        None
    } else {
        db.get_cf(cf, window_id.as_bytes())?
    };
    let mut iter_count = 0u64;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let _ = item?;
        iter_count += 1;
    }
    require(
        "store count matches direct RocksDB iteration count",
        count as u64 == iter_count,
    )?;
    Ok(json!({
        "cf_count": count,
        "direct_iter_count": iter_count,
        "raw_row": {
            "key": window_id,
            "exists": raw.is_some(),
            "byte_len": raw.as_ref().map(|bytes| bytes.len()).unwrap_or_default(),
        }
    }))
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    require("JSON byte readback equality", fs::read(path)? == bytes)?;
    Ok(())
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("production telemetry FSV assertion failed: {label}").into())
    }
}
