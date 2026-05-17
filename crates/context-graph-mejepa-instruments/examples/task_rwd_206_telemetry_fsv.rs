use context_graph_mejepa_instruments::telemetry_signals::{
    extract_telemetry_signals, write_telemetry_raw_output, BigOClass, BigOShift,
    SymbolCallObservation, TelemetrySignalRecord, TelemetrySignalStore, TelemetrySource,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    error_code: Option<String>,
    details: Value,
}

fn main() -> FsvResult<()> {
    let run_id = format!("run-{}", now_millis()?);
    let fsv_root =
        PathBuf::from("/mnt/d/contextgraph/fsv/task-rwd-206-telemetry-fsv").join(&run_id);
    let raw_root = PathBuf::from("/mnt/d/contextgraph/corpus/python-telemetry-signals-v1")
        .join("task-rwd-206-fsv")
        .join(&run_id);
    let db_path = fsv_root.join("telemetry_signals.db");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    fs::set_permissions(&fsv_root, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&raw_root, fs::Permissions::from_mode(0o700))?;

    let evidence_path = fsv_root.join("telemetry_fsv.json");
    let source = happy_source();
    let before_state = json!({
        "db_exists": db_path.exists(),
        "raw_root_exists": raw_root.exists(),
        "raw_file_count": count_files(&raw_root)?,
    });

    let extraction = extract_telemetry_signals(&source)?;
    require(
        "happy extraction emits one label",
        extraction.labels.len() == 1,
    )?;
    require(
        "happy extraction emits symbol profiles",
        extraction.symbols.len() >= 3,
    )?;
    let label = extraction.labels.first().unwrap();
    require("before Big-O is O(n)", label.before_big_o == BigOClass::ON)?;
    require("after Big-O is O(n^2)", label.after_big_o == BigOClass::ON2)?;
    require(
        "Big-O shift regresses",
        label.big_o_shift == BigOShift::Regressed,
    )?;
    require(
        "predicted latency delta is positive",
        label.predicted_latency_delta_ms > 0,
    )?;
    require(
        "predicted VRAM delta is positive",
        label.predicted_vram_delta_bytes > 0,
    )?;
    require(
        "predicted cost delta is positive",
        label.predicted_cost_delta_micro_usd > 0,
    )?;
    require(
        "runtime vector hash is sha256-prefixed",
        label.runtime_vector_sha256.starts_with("sha256:"),
    )?;
    require(
        "hot symbol is score_pair",
        label.hot_symbol.as_deref() == Some("score_pair"),
    )?;

    let score_pair = extraction
        .symbols
        .iter()
        .find(|symbol| symbol.symbol == "score_pair")
        .ok_or("missing score_pair symbol")?;
    require(
        "score_pair profile carries observed calls",
        score_pair.observed_calls_after == 900 && score_pair.frequency_fraction_after > 0.8,
    )?;

    let raw_paths = write_telemetry_raw_output(&raw_root, "task-rwd-206-happy", &extraction)?;
    let raw_readback: Value = serde_json::from_slice(&fs::read(&raw_paths.telemetry_json)?)?;
    require(
        "raw telemetry output readback has one label",
        raw_readback["labels"].as_array().map_or(0, Vec::len) == 1,
    )?;

    let keys = {
        let store = TelemetrySignalStore::open(&db_path)?;
        let keys = store.put_extraction(&extraction)?;
        store.flush()?;
        require(
            "CF key count matches persisted records",
            keys.len() == extraction.persisted_records().len(),
        )?;
        require(
            "count after write matches persisted records",
            store.count_records()? == extraction.persisted_records().len(),
        )?;
        keys
    };
    let reopened_rows = {
        let reopened = TelemetrySignalStore::open(&db_path)?;
        reopened.scan_records()?
    };
    require(
        "reopened CF row count matches persisted records",
        reopened_rows.len() == extraction.persisted_records().len(),
    )?;
    let reopened_label = reopened_rows
        .iter()
        .find_map(|(_, record)| match &record.signal {
            TelemetrySignalRecord::Label(label) => Some(label),
            TelemetrySignalRecord::SymbolProfile(_) => None,
        })
        .ok_or("missing reopened telemetry label")?;
    require(
        "reopened label matches written label",
        reopened_label == label,
    )?;

    let static_only = edge_static_only_persists(&db_path)?;
    let edge_cases = vec![
        edge_invalid_source("empty_before_source", |source| source.before_source.clear()),
        edge_invalid_source("syntax_error", |source| {
            source.after_source = "def broken(:\n    pass\n".to_string()
        }),
        edge_invalid_source("nul_source", |source| {
            source.after_source.push('\0');
        }),
        edge_invalid_source("zero_latency_baseline", |source| {
            source.baseline_latency_ms = 0
        }),
        edge_invalid_source("negative_symbol_duration", |source| {
            source.symbol_observations[0].inclusive_time_ms_after = -1.0
        }),
        static_only,
    ];
    require(
        "all edge cases pass",
        edge_cases.iter().all(|case| case.passed),
    )?;

    let after_state = json!({
        "db_exists": db_path.exists(),
        "raw_telemetry_json": raw_paths.telemetry_json,
        "raw_telemetry_sha256": sha256_file(&raw_paths.telemetry_json)?,
        "cf_keys": keys,
        "reopened_count": reopened_rows.len(),
        "raw_file_count": count_files(&raw_root)?,
    });
    let evidence = json!({
        "schema_version": 1,
        "task_id": "TASK-RWD-206",
        "issue": 315,
        "run_id": run_id,
        "all_passed": true,
        "source_of_truth": {
            "fsv_json": evidence_path,
            "rocksdb": db_path,
            "raw_root": raw_root,
            "cf": "CF_MEJEPA_TELEMETRY_SIGNALS",
        },
        "before_state": before_state,
        "trigger": "extract E_Telemetry prediction features, persist raw output, sync-write CF rows, drop and reopen RocksDB",
        "after_state": after_state,
        "happy_path": {
            "label": label,
            "score_pair_profile": score_pair,
            "reopened_label": reopened_label,
            "raw_readback": raw_readback,
        },
        "edge_cases": edge_cases,
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "FSV evidence readback passes",
        readback["all_passed"] == true,
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn happy_source() -> TelemetrySource {
    TelemetrySource {
        corpus_row_id: "task-rwd-206-happy".to_string(),
        chunk_id: "chunk-telemetry-happy".to_string(),
        language: "python".to_string(),
        path: "pkg/telemetry_happy.py".to_string(),
        before_source: r#"def score(values):
    total = 0
    for value in values:
        total += normalize(value)
    return total
"#
        .to_string(),
        after_source: r#"def score(values):
    total = 0
    ordered = sorted(values)
    for left in ordered:
        for right in ordered:
            total += score_pair(left, right)
    return total
"#
        .to_string(),
        baseline_latency_ms: 120,
        baseline_vram_bytes: 512 * 1024 * 1024,
        baseline_cost_micro_usd: 900,
        symbol_observations: vec![
            SymbolCallObservation {
                symbol: "normalize".to_string(),
                calls_before: 100,
                calls_after: 0,
                inclusive_time_ms_before: 15.0,
                inclusive_time_ms_after: 0.0,
            },
            SymbolCallObservation {
                symbol: "sorted".to_string(),
                calls_before: 0,
                calls_after: 1,
                inclusive_time_ms_before: 0.0,
                inclusive_time_ms_after: 3.0,
            },
            SymbolCallObservation {
                symbol: "score_pair".to_string(),
                calls_before: 0,
                calls_after: 900,
                inclusive_time_ms_before: 0.0,
                inclusive_time_ms_after: 260.0,
            },
        ],
    }
}

fn edge_static_only_persists(db_path: &Path) -> FsvResult<EdgeCase> {
    let mut source = happy_source();
    source.corpus_row_id = "task-rwd-206-static-only".to_string();
    source.chunk_id = "chunk-telemetry-static-only".to_string();
    source.symbol_observations.clear();
    let extraction = extract_telemetry_signals(&source)?;
    let label = extraction
        .labels
        .first()
        .ok_or("missing static-only label")?;
    let store = TelemetrySignalStore::open(db_path)?;
    let before = store.count_records()?;
    let keys = store.put_extraction(&extraction)?;
    store.flush()?;
    let after = store.count_records()?;
    Ok(EdgeCase {
        name: "static_only_call_profile_persists",
        passed: label.total_observed_calls_after == 0
            && !extraction.symbols.is_empty()
            && keys.len() == extraction.persisted_records().len()
            && after == before + keys.len(),
        error_code: None,
        details: json!({
            "before_count": before,
            "after_count": after,
            "label": label,
            "symbol_count": extraction.symbols.len(),
        }),
    })
}

fn edge_invalid_source(name: &'static str, mutate: impl FnOnce(&mut TelemetrySource)) -> EdgeCase {
    let mut source = happy_source();
    mutate(&mut source);
    match extract_telemetry_signals(&source) {
        Ok(value) => EdgeCase {
            name,
            passed: false,
            error_code: None,
            details: json!({ "unexpected_success": value }),
        },
        Err(err) => EdgeCase {
            name,
            passed: err.code() == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
            error_code: Some(err.code().to_string()),
            details: json!({ "error": err.to_string() }),
        },
    }
}

fn require(message: &'static str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}

fn now_millis() -> FsvResult<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json write/readback bytes match", readback == bytes)
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn count_files(path: &Path) -> FsvResult<usize> {
    let mut count = 0;
    if path.exists() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                count += 1;
            }
        }
    }
    Ok(count)
}
