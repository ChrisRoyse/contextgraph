use chrono::DateTime;
use context_graph_mejepa::eval::{LatencyTelemetry, ProductionTelemetryWindow, RocksDbEvalStore};
use context_graph_mejepa::open_infer_rocksdb;
use context_graph_mejepa::system_cost::HealTickerTelemetrySnapshot;
use rocksdb::{IteratorMode, DB};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

const SHIP_GATE_FSV: &str =
    "/zfs/archive/contextgraph/fsv/swebench-300x8-ship-gate-fsv/ship-gate-fsv.json";
const CONFORMAL_FSV: &str =
    "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-infer-fsv/conformal-calibration-evidence.json";
const OOD_FSV: &str =
    "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-infer-fsv/ood-evidence.json";
const SHIFT_LATENCY_FSV: &str =
    "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-shift-subscriber-fsv/predict-latency-evidence.json";
const VRAM_FSV: &str =
    "/zfs/archive/contextgraph/fsv/contextgraph-mejepa-vram-wsl-fsv/vram-wsl-telemetry-evidence.json";
const PYTHON_CORPUS_INDEX: &str =
    "/zfs/archive/contextgraph/corpus/swebench-lite-python-300x8-v1-20260511/index.json";

fn main() -> FsvResult<()> {
    let root =
        PathBuf::from("/zfs/archive/contextgraph/fsv/contextgraph-mejepa-production-telemetry-fsv");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let db_path = root.join("rocksdb");
    let db = open_infer_rocksdb(&db_path)?;
    let store = RocksDbEvalStore::new(db.clone())?;
    let (windows, telemetry_sources) = production_windows_from_real_evidence()?;
    require(
        "telemetry evidence includes final and rolling windows",
        windows.len() >= 4,
    )?;
    let primary_window = windows
        .first()
        .ok_or("production telemetry produced no windows")?;

    let before = physical_state(db.as_ref(), &store, &primary_window.window_id)?;
    let mut loaded_windows = Vec::new();
    for window in &windows {
        store.persist_production_telemetry(window)?;
        let loaded = store
            .load_production_telemetry(&window.window_id)?
            .ok_or("production telemetry readback missing")?;
        require(
            "loaded telemetry matches persisted value",
            loaded == *window,
        )?;
        loaded_windows.push(loaded);
    }
    let after = physical_state(db.as_ref(), &store, &primary_window.window_id)?;
    require(
        "telemetry CF count increments by persisted window count",
        after["cf_count"].as_u64()
            == before["cf_count"]
                .as_u64()
                .map(|count| count + windows.len() as u64),
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
                ..primary_window.clone()
            },
            "MEJEPA_EVAL_INVALID_INPUT",
        )?,
        edge_case(
            db.as_ref(),
            &store,
            "edge_zero_samples",
            ProductionTelemetryWindow {
                sample_count: 0,
                ..with_window_id(primary_window, "edge-zero-samples")
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
                ..with_window_id(primary_window, "edge-non-monotonic-latency")
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
                ..with_window_id(primary_window, "edge-ship-gate-contradiction")
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
                ..with_window_id(primary_window, "edge-vram-free-exceeds-total")
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
    let mut reopened_loaded_windows = Vec::new();
    for window in &windows {
        let reopened_loaded = reopened_store
            .load_production_telemetry(&window.window_id)?
            .ok_or("production telemetry readback missing after reopen")?;
        require(
            "reopened telemetry equals persisted value",
            reopened_loaded == *window,
        )?;
        reopened_loaded_windows.push(reopened_loaded);
    }
    let reopened_state = physical_state(
        reopened_db.as_ref(),
        &reopened_store,
        &primary_window.window_id,
    )?;

    let evidence = json!({
        "all_passed": true,
        "passes": true,
        "source_of_truth": {
            "rocksdb_path": db_path,
            "column_family": context_graph_mejepa_cf::CF_MEJEPA_PRODUCTION_TELEMETRY,
            "primary_window_key": primary_window.window_id,
            "window_keys": windows.iter().map(|window| window.window_id.clone()).collect::<Vec<_>>(),
            "evidence_json": root.join("production-telemetry-evidence.json"),
            "input_sources": telemetry_sources,
        },
        "happy_path": {
            "before_state": before,
            "trigger": "RocksDbEvalStore::persist_production_telemetry for final + rolling windows",
            "after_state": after,
            "loaded_readbacks": loaded_windows,
            "reopened_state": reopened_state,
            "reopened_loaded_readbacks": reopened_loaded_windows,
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

fn production_windows_from_real_evidence() -> FsvResult<(Vec<ProductionTelemetryWindow>, Value)> {
    let ship_gate = read_json(Path::new(SHIP_GATE_FSV))?;
    let conformal = read_json(Path::new(CONFORMAL_FSV))?;
    let ood = read_json(Path::new(OOD_FSV))?;
    let latency = read_json(Path::new(SHIFT_LATENCY_FSV))?;
    let vram = read_json(Path::new(VRAM_FSV))?;
    let final_report = &ship_gate["execute_and_inspect"]["final_report"];
    let final_readback = &ship_gate["execute_and_inspect"]["final_report_readback"];
    require(
        "ship-gate final report equals independent readback",
        final_report == final_readback,
    )?;

    let timestamp = string_at(final_report, "timestamp_iso8601")?;
    let captured_at_unix_ms = DateTime::parse_from_rfc3339(timestamp)?.timestamp_millis();
    let split_counts = &ship_gate["execute_and_inspect"]["loaded_split_counts"];
    let sample_count = usize_at(split_counts, "train")?
        + usize_at(split_counts, "calibration")?
        + usize_at(split_counts, "holdout")?;
    require(
        "production telemetry sample_count comes from full corpus",
        sample_count == 2400,
    )?;

    let expected_coverage = 1.0 - f32_at(&conformal, "alpha")?;
    let empirical_coverage = f32_at(&conformal, "empirical_coverage_holdout")?;
    let conformal_ece = (empirical_coverage - expected_coverage).abs();
    let prediction_oracle_agreement = f32_at(final_report, "prediction_oracle_agreement")?;
    let ship_gate_target = f32_at(&ship_gate["execute_and_inspect"], "ship_gate_target")?;
    let ship_gate_passed = bool_at(&ship_gate["execute_and_inspect"], "ship_gate_passed")?;
    let vram_report = &vram["happy_path"]["after"]["report"];
    let latency_ms = &latency["latency_ms"];
    let corpus_sha = sha256_file(Path::new(PYTHON_CORPUS_INDEX))?;
    let latency = LatencyTelemetry {
        p50_ms: f32_at(latency_ms, "p50")?,
        p95_ms: f32_at(latency_ms, "p95")?,
        p99_ms: f32_at(latency_ms, "p99")?,
        max_ms: f32_at(latency_ms, "max")?,
    };
    let vram_free_bytes = u64_at(vram_report, "free_bytes")?;
    let vram_total_bytes = u64_at(vram_report, "total_bytes")?;
    let vram_required_bytes = u64_at(vram_report, "required_bytes")?;
    let gtau_pass_rate = f32_at(final_report, "gtau_pass_rate")?;
    let ood_auc = f32_at(&ood, "auc")?;

    let final_window = ProductionTelemetryWindow {
        window_id: format!(
            "prod-window-swebench-300x8-{}",
            timestamp.replace([':', '.'], "-")
        ),
        captured_at_unix_ms,
        corpus_sha: corpus_sha.clone(),
        sample_count,
        conformal_ece,
        ood_auc,
        gtau_pass_rate,
        prediction_oracle_agreement,
        ship_gate_target,
        ship_gate_passed,
        latency: latency.clone(),
        vram_free_bytes,
        vram_total_bytes,
        vram_required_bytes,
        heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
        training_holdout_distribution_drift: None,
        source: "real_fsv_sources:swebench_300x8+conformal+ood+shift_latency+cuda_vram".to_string(),
    };
    let rolling_reports = ship_gate["execute_and_inspect"]["rolling_reports"]
        .as_array()
        .ok_or("missing rolling_reports array")?;
    let mut windows = vec![final_window];
    for report in rolling_reports {
        let window_index = u64_at(report, "window_index")?;
        let step = u64_at(report, "step")?;
        let after = &report["after"];
        let agreement = f32_at(after, "agreement")?;
        let rolling_sample_count = usize_at(after, "sample_count")?;
        let captured_offset = i64::try_from(window_index + 1)?;
        windows.push(ProductionTelemetryWindow {
            window_id: format!("prod-window-swebench-300x8-rolling-{window_index}-step-{step}"),
            captured_at_unix_ms: captured_at_unix_ms + captured_offset,
            corpus_sha: corpus_sha.clone(),
            sample_count: rolling_sample_count,
            conformal_ece,
            ood_auc,
            gtau_pass_rate,
            prediction_oracle_agreement: agreement,
            ship_gate_target,
            ship_gate_passed: agreement >= ship_gate_target,
            latency: latency.clone(),
            vram_free_bytes,
            vram_total_bytes,
            vram_required_bytes,
            heal_ticker_telemetry_total: HealTickerTelemetrySnapshot::default(),
            training_holdout_distribution_drift: None,
            source: "real_fsv_sources:rolling_swebench_300x8+conformal+ood+shift_latency+cuda_vram"
                .to_string(),
        });
    }
    let sources = json!({
        "ship_gate_fsv": SHIP_GATE_FSV,
        "conformal_fsv": CONFORMAL_FSV,
        "ood_fsv": OOD_FSV,
        "shift_latency_fsv": SHIFT_LATENCY_FSV,
        "vram_fsv": VRAM_FSV,
        "python_corpus_index": PYTHON_CORPUS_INDEX,
        "expected_coverage": expected_coverage,
        "empirical_coverage": empirical_coverage,
        "ship_gate_final_report_readback_matched": true,
        "rolling_window_count": rolling_reports.len(),
    });
    Ok((windows, sources))
}

fn with_window_id(base: &ProductionTelemetryWindow, window_id: &str) -> ProductionTelemetryWindow {
    let mut window = base.clone();
    window.window_id = window_id.to_string();
    window
}

fn read_json(path: &Path) -> FsvResult<Value> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let mut hasher = Sha256::new();
    hasher.update(fs::read(path)?);
    Ok(hex::encode(hasher.finalize()))
}

fn string_at<'a>(value: &'a Value, key: &str) -> FsvResult<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing string field {key}").into())
}

fn bool_at(value: &Value, key: &str) -> FsvResult<bool> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("missing bool field {key}").into())
}

fn f32_at(value: &Value, key: &str) -> FsvResult<f32> {
    let number = value
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("missing number field {key}"))?;
    if !number.is_finite() {
        return Err(format!("field {key} is not finite").into());
    }
    Ok(number as f32)
}

fn usize_at(value: &Value, key: &str) -> FsvResult<usize> {
    let number = u64_at(value, key)?;
    usize::try_from(number).map_err(|_| format!("field {key} exceeds usize").into())
}

fn u64_at(value: &Value, key: &str) -> FsvResult<u64> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing u64 field {key}").into())
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
