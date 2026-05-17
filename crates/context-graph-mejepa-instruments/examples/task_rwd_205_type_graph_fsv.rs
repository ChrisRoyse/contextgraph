use context_graph_mejepa_instruments::type_graph_signals::{
    extract_type_graph_signals, write_type_graph_raw_output, TypeGraphSignalRecord,
    TypeGraphSignalStore, TypeGraphSource,
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
        PathBuf::from("/mnt/d/contextgraph/fsv/task-rwd-205-type-graph-fsv").join(&run_id);
    let raw_root = PathBuf::from("/mnt/d/contextgraph/corpus/python-type-graph-signals-v1")
        .join("task-rwd-205-fsv")
        .join(&run_id);
    let db_path = fsv_root.join("type_graph_signals.db");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    fs::set_permissions(&fsv_root, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&raw_root, fs::Permissions::from_mode(0o700))?;

    let evidence_path = fsv_root.join("type_graph_fsv.json");
    let source = TypeGraphSource {
        corpus_row_id: "task-rwd-205-happy".to_string(),
        chunk_id: "chunk-type-graph-happy".to_string(),
        path: "pkg/type_graph_happy.py".to_string(),
        source_text: happy_source().to_string(),
    };

    let before_state = json!({
        "db_exists": db_path.exists(),
        "raw_root_exists": raw_root.exists(),
        "raw_file_count": count_files(&raw_root)?,
    });
    let extraction = extract_type_graph_signals(&source)?;
    require(
        "happy extraction emits one label",
        extraction.labels.len() == 1,
    )?;
    let label = extraction.labels.first().unwrap();
    require("signature count is populated", label.signature_count >= 3)?;
    require("call-site count is populated", label.call_site_count >= 5)?;
    require(
        "known call-site count is populated",
        label.known_call_site_count >= 4,
    )?;
    require(
        "unknown call-site count is populated",
        label.unknown_call_site_count >= 1,
    )?;
    require(
        "typed argument count is populated",
        label.typed_argument_count >= 3,
    )?;
    require(
        "untyped argument count is populated",
        label.untyped_argument_count >= 1,
    )?;
    require(
        "return edge count is populated",
        label.return_edge_count >= 2,
    )?;
    require(
        "Any propagation count is populated",
        label.any_call_site_count >= 1,
    )?;
    require(
        "arity mismatch count is populated",
        label.arity_mismatch_count >= 1,
    )?;
    require(
        "type graph vector hash is sha256-prefixed",
        label.type_graph_vector_sha256.starts_with("sha256:"),
    )?;

    let raw_paths = write_type_graph_raw_output(&raw_root, "task-rwd-205-happy", &extraction)?;
    let raw_readback: Value = serde_json::from_slice(&fs::read(&raw_paths.type_graph_json)?)?;
    require(
        "raw type-graph output readback has one label",
        raw_readback["labels"].as_array().map_or(0, Vec::len) == 1,
    )?;

    let keys = {
        let store = TypeGraphSignalStore::open(&db_path)?;
        let keys = store.put_extraction(&extraction)?;
        store.flush()?;
        require("one CF key written", keys.len() == 1)?;
        require("count after write is one", store.count_records()? == 1)?;
        keys
    };
    let reopened_rows = {
        let reopened = TypeGraphSignalStore::open(&db_path)?;
        reopened.scan_records()?
    };
    require("reopened CF row count is one", reopened_rows.len() == 1)?;
    let TypeGraphSignalRecord::Label(reopened_label) = &reopened_rows[0].1.signal;
    require(
        "reopened label matches written label",
        reopened_label == label,
    )?;

    let no_call = no_call_edge(&db_path)?;
    let edge_cases = vec![
        edge_invalid_source("empty_source", "", "MEJEPA_INSTRUMENTS_INVALID_INPUT"),
        edge_invalid_source(
            "syntax_error",
            "def broken(:\n    pass\n",
            "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        ),
        edge_invalid_source(
            "nul_source",
            "def ok():\n    return '\0'\n",
            "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        ),
        no_call,
    ];
    require(
        "all edge cases pass",
        edge_cases.iter().all(|case| case.passed),
    )?;

    let after_state = json!({
        "db_exists": db_path.exists(),
        "raw_type_graph_json": raw_paths.type_graph_json,
        "raw_type_graph_sha256": sha256_file(&raw_paths.type_graph_json)?,
        "cf_keys": keys,
        "reopened_count": reopened_rows.len(),
        "raw_file_count": count_files(&raw_root)?,
    });
    let evidence = json!({
        "schema_version": 1,
        "task_id": "TASK-RWD-205",
        "issue": 315,
        "run_id": run_id,
        "all_passed": true,
        "source_of_truth": {
            "fsv_json": evidence_path,
            "rocksdb": db_path,
            "raw_root": raw_root,
            "cf": "CF_MEJEPA_TYPE_GRAPH_SIGNALS",
        },
        "before_state": before_state,
        "trigger": "extract E_TypeGraph call-site type evidence, persist raw output, sync-write CF row, drop and reopen RocksDB",
        "after_state": after_state,
        "happy_path": {
            "label": label,
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

fn happy_source() -> &'static str {
    r#"from typing import Any

def parse_number(value: str, default: int) -> int:
    try:
        return int(value)
    except ValueError:
        return default

def passthrough(value: Any) -> Any:
    return value

def loose(value):
    return value

result: int = parse_number("42", 0)
bad = parse_number("42")
wrapped = passthrough(result)
unknown = loose(wrapped)
"#
}

fn no_call_edge(db_path: &Path) -> FsvResult<EdgeCase> {
    let source = TypeGraphSource {
        corpus_row_id: "task-rwd-205-no-call".to_string(),
        chunk_id: "chunk-type-graph-no-call".to_string(),
        path: "pkg/type_graph_no_call.py".to_string(),
        source_text: "value: int = 1\n".to_string(),
    };
    let extraction = extract_type_graph_signals(&source)?;
    let label = extraction.labels.first().ok_or("missing no-call label")?;
    let store = TypeGraphSignalStore::open(db_path)?;
    let before = store.count_records()?;
    let keys = store.put_extraction(&extraction)?;
    store.flush()?;
    let after = store.count_records()?;
    Ok(EdgeCase {
        name: "no_call_site_persists_zero_count_label",
        passed: label.call_site_count == 0 && keys.len() == 1 && after == before + 1,
        error_code: None,
        details: json!({
            "before_count": before,
            "after_count": after,
            "label": label,
        }),
    })
}

fn edge_invalid_source(
    name: &'static str,
    source_text: &str,
    expected_code: &'static str,
) -> EdgeCase {
    let source = TypeGraphSource {
        corpus_row_id: format!("task-rwd-205-{name}"),
        chunk_id: format!("chunk-{name}"),
        path: format!("pkg/{name}.py"),
        source_text: source_text.to_string(),
    };
    match extract_type_graph_signals(&source) {
        Ok(_) => EdgeCase {
            name,
            passed: false,
            error_code: None,
            details: json!({"unexpected": "extraction succeeded"}),
        },
        Err(err) => EdgeCase {
            name,
            passed: err.code() == expected_code,
            error_code: Some(err.code().to_string()),
            details: json!({"message": err.to_string()}),
        },
    }
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json readback equals written bytes", readback == bytes)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn count_files(path: &Path) -> FsvResult<usize> {
    if !path.exists() {
        return Ok(0);
    }
    let mut count = 0usize;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            count += 1;
        } else if entry.file_type()?.is_dir() {
            count += count_files(&entry.path())?;
        }
    }
    Ok(count)
}

fn sha256_file(path: &Path) -> FsvResult<String> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn now_millis() -> FsvResult<u128> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis())
}

fn require(message: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.to_string().into())
    }
}
