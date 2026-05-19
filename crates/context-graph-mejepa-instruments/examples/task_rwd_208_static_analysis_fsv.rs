use context_graph_mejepa_instruments::{
    extract_static_analysis_labels, run_static_analysis_tools_with_timeout,
    write_static_analysis_raw_outputs, EStaticAnalysisInstrument, Instrument,
    StaticAnalysisDetector, StaticAnalysisRawOutputs, StaticAnalysisSignalRecord,
    StaticAnalysisSignalStore, StaticAnalysisSource, StaticAnalysisToolOutput,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Serialize)]
struct EdgeCase {
    name: &'static str,
    passed: bool,
    detail: Value,
}

fn main() -> FsvResult<()> {
    let run_id = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .to_string();
    let fsv_root = PathBuf::from("/zfs/archive/contextgraph/fsv/task-py-g-074-static-analysis-fsv")
        .join(format!("run-{run_id}"));
    fs::create_dir_all(&fsv_root)?;
    let raw_root = PathBuf::from("/zfs/archive/contextgraph/corpus/python-static-analysis-v1");
    let row_id = format!("task-rwd-208-{run_id}");
    let rocksdb_path = fsv_root.join("static-analysis-rocksdb");
    let evidence_path = fsv_root.join("static_analysis_fsv.json");

    let sample_path = fsv_root.join("sample.py");
    let source = happy_source_with_path(sample_path.display().to_string());
    fs::write(&sample_path, source.source_text.as_bytes())?;
    let old_path = install_static_analysis_shims(&fsv_root)?;
    let outputs = run_static_analysis_tools_with_timeout(&sample_path, Duration::from_secs(2))?;
    restore_path(old_path);
    require(
        "mypy shim executed through runner",
        outputs.mypy.status_code == Some(1) && !outputs.mypy.toolchain_missing,
    )?;
    require(
        "pyright shim executed through runner",
        outputs.pyright.status_code == Some(1) && !outputs.pyright.toolchain_missing,
    )?;
    require(
        "ruff shim executed through runner",
        outputs.ruff.status_code == Some(1) && !outputs.ruff.toolchain_missing,
    )?;
    let raw_paths = write_static_analysis_raw_outputs(&raw_root, &row_id, &outputs)?;
    require("mypy raw output exists", raw_paths.mypy_json.exists())?;
    require("pyright raw output exists", raw_paths.pyright_json.exists())?;
    require("ruff raw output exists", raw_paths.ruff_json.exists())?;
    let pyright_raw_readback: StaticAnalysisToolOutput =
        serde_json::from_slice(&fs::read(&raw_paths.pyright_json)?)?;
    require(
        "pyright raw output readback preserves detector",
        pyright_raw_readback.detector == StaticAnalysisDetector::Pyright,
    )?;

    let extraction = extract_static_analysis_labels(&source, &outputs)?;
    require("happy path produces 3 labels", extraction.labels.len() == 3)?;
    require(
        "happy path has no quarantines",
        extraction.quarantines.is_empty(),
    )?;
    let vector = EStaticAnalysisInstrument.encode(&extraction.static_analysis_input)?;
    require("E_StaticAnalysis vector len is 256", vector.len() == 256)?;
    require(
        "E_StaticAnalysis vector nonzero",
        vector.iter().any(|value| *value != 0.0),
    )?;

    let store = StaticAnalysisSignalStore::open(&rocksdb_path)?;
    let before_count = store.count_records()?;
    let keys = store.put_extraction(&extraction)?;
    store.flush()?;
    let after_count = store.count_records()?;
    require(
        "store count moved by label count",
        after_count == before_count + 3,
    )?;
    drop(store);
    let reopened = StaticAnalysisSignalStore::open(&rocksdb_path)?;
    let reopened_rows = reopened.scan_records()?;
    require("reopened RocksDB sees 3 rows", reopened_rows.len() == 3)?;
    require(
        "reopened rows are all labels",
        reopened_rows
            .iter()
            .all(|(_, row)| matches!(row.signal, StaticAnalysisSignalRecord::Label(_))),
    )?;
    drop(reopened);

    let edge_cases = vec![
        edge_type_ignore_respected()?,
        edge_pyright_ignore_respected()?,
        edge_tool_disagreement_preserved()?,
        edge_runtime_exceeded_quarantined(&rocksdb_path)?,
        edge_malformed_pyright_fails_closed()?,
    ];
    if !edge_cases.iter().all(|case| case.passed) {
        return Err(format!(
            "static-analysis edge cases failed: {}",
            serde_json::to_string_pretty(&edge_cases)?
        )
        .into());
    }

    let evidence = json!({
        "task": "TASK-RWD-208",
        "issue": 293,
        "all_passed": true,
        "source_of_truth": {
            "raw_output_root": raw_root,
            "raw_output_paths": raw_paths,
            "rocksdb_path": rocksdb_path,
            "cf": context_graph_mejepa_cf::CF_MEJEPA_STATIC_ANALYSIS_SIGNALS,
            "evidence_path": evidence_path,
        },
        "happy_path": {
            "before_state": {"cf_record_count": before_count},
            "trigger": "Run mypy, pyright, and ruff through deterministic PATH shims; normalize raw outputs into StaticAnalysisLabel records; persist to CF; reopen RocksDB; encode E_StaticAnalysis.",
            "expected": {
                "labels": 3,
                "quarantines": 0,
                "cf_record_delta": 3,
                "raw_files": ["mypy.json", "pyright.json", "ruff.json"]
            },
            "after_state": {
                "labels": extraction.labels,
                "quarantines": extraction.quarantines,
                "cf_record_count": after_count,
                "reopened_record_count": reopened_rows.len(),
                "keys": keys,
                "vector_len": vector.len(),
                "vector_non_zero": vector.iter().filter(|value| **value != 0.0).count(),
                "vector_sha256": sha256_json(&vector)?,
            }
        },
        "edge_cases": edge_cases,
    });
    write_json_checked(&evidence_path, &evidence)?;
    let readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback all_passed",
        readback["all_passed"] == true,
    )?;
    println!("{}", serde_json::to_string_pretty(&readback)?);
    Ok(())
}

fn happy_source() -> StaticAnalysisSource {
    happy_source_with_path("sample.py")
}

fn happy_source_with_path(path: impl Into<String>) -> StaticAnalysisSource {
    StaticAnalysisSource {
        chunk_id: "chunk-static-analysis-001".into(),
        path: path.into(),
        source_text: "from typing import Any\n\n\ndef f(x: int) -> str:\n    return x\n".into(),
    }
}

fn happy_outputs() -> StaticAnalysisRawOutputs {
    StaticAnalysisRawOutputs {
        mypy: StaticAnalysisToolOutput {
            detector: StaticAnalysisDetector::Mypy,
            command: vec![
                "mypy".into(),
                "--strict".into(),
                "--no-error-summary".into(),
                "sample.py".into(),
            ],
            status_code: Some(1),
            stdout: "sample.py:5: error: Incompatible return value type (got \"int\", expected \"str\")  [return-value]\n".into(),
            stderr: String::new(),
            runtime_exceeded: false,
            toolchain_missing: false,
        },
        pyright: StaticAnalysisToolOutput {
            detector: StaticAnalysisDetector::Pyright,
            command: vec!["pyright".into(), "--outputjson".into(), "sample.py".into()],
            status_code: Some(1),
            stdout: r#"{"generalDiagnostics":[{"file":"sample.py","severity":"error","message":"Type \"int\" is not assignable to return type \"str\"","range":{"start":{"line":4,"character":11},"end":{"line":4,"character":12}},"rule":"reportReturnType"}]}"#.into(),
            stderr: String::new(),
            runtime_exceeded: false,
            toolchain_missing: false,
        },
        ruff: StaticAnalysisToolOutput {
            detector: StaticAnalysisDetector::Ruff,
            command: vec![
                "ruff".into(),
                "check".into(),
                "--output-format".into(),
                "json".into(),
                "sample.py".into(),
            ],
            status_code: Some(1),
            stdout: r#"[{"cell":null,"code":"F401","end_location":{"column":29,"row":1},"filename":"sample.py","fix":null,"location":{"column":20,"row":1},"message":"`typing.Any` imported but unused","noqa_row":1,"url":"https://docs.astral.sh/ruff/rules/unused-import/"}]"#.into(),
            stderr: String::new(),
            runtime_exceeded: false,
            toolchain_missing: false,
        },
    }
}

fn edge_type_ignore_respected() -> FsvResult<EdgeCase> {
    let mut source = happy_source();
    source.source_text =
        "from typing import Any\n\n\ndef f(x: int) -> str:\n    return x  # type: ignore[return-value]\n"
            .into();
    let extraction = extract_static_analysis_labels(&source, &happy_outputs())?;
    let mypy_labels = extraction
        .labels
        .iter()
        .filter(|label| label.detector == StaticAnalysisDetector::Mypy)
        .count();
    Ok(EdgeCase {
        name: "type_ignore_suppresses_mypy_label",
        passed: mypy_labels == 0,
        detail: json!({"mypy_label_count": mypy_labels, "labels": extraction.labels}),
    })
}

fn edge_pyright_ignore_respected() -> FsvResult<EdgeCase> {
    let mut source = happy_source();
    source.source_text =
        "from typing import Any\n\n\ndef f(x: int) -> str:\n    return x  # pyright: ignore[reportReturnType]\n"
            .into();
    let extraction = extract_static_analysis_labels(&source, &happy_outputs())?;
    let pyright_labels = extraction
        .labels
        .iter()
        .filter(|label| label.detector == StaticAnalysisDetector::Pyright)
        .count();
    Ok(EdgeCase {
        name: "pyright_ignore_suppresses_pyright_label",
        passed: pyright_labels == 0,
        detail: json!({"pyright_label_count": pyright_labels, "labels": extraction.labels}),
    })
}

fn edge_tool_disagreement_preserved() -> FsvResult<EdgeCase> {
    let extraction = extract_static_analysis_labels(&happy_source(), &happy_outputs())?;
    let has_pyright = extraction
        .labels
        .iter()
        .any(|label| label.detector == StaticAnalysisDetector::Pyright);
    let has_ruff = extraction
        .labels
        .iter()
        .any(|label| label.detector == StaticAnalysisDetector::Ruff);
    Ok(EdgeCase {
        name: "ruff_pyright_disagreement_persists_both",
        passed: has_pyright && has_ruff,
        detail: json!({"has_pyright": has_pyright, "has_ruff": has_ruff}),
    })
}

fn edge_runtime_exceeded_quarantined(rocksdb_path: &Path) -> FsvResult<EdgeCase> {
    let mut outputs = happy_outputs();
    outputs.mypy.runtime_exceeded = true;
    outputs.mypy.stdout.clear();
    let extraction = extract_static_analysis_labels(&happy_source(), &outputs)?;
    let store = StaticAnalysisSignalStore::open(rocksdb_path)?;
    let before = store.count_records()?;
    store.put_extraction(&extraction)?;
    store.flush()?;
    let after = store.count_records()?;
    let quarantines = extraction
        .quarantines
        .iter()
        .filter(|q| q.reason_code == "STATIC_ANALYSIS_RUNTIME_EXCEEDED")
        .count();
    let expected_delta = extraction.labels.len() + quarantines;
    Ok(EdgeCase {
        name: "runtime_exceeded_quarantines_chunk",
        passed: quarantines == 1 && after == before + expected_delta,
        detail: json!({
            "before_cf_records": before,
            "after_cf_records": after,
            "expected_cf_delta": expected_delta,
            "labels": extraction.labels.len(),
            "runtime_exceeded_quarantines": quarantines,
            "quarantines": extraction.quarantines
        }),
    })
}

fn edge_malformed_pyright_fails_closed() -> FsvResult<EdgeCase> {
    let mut outputs = happy_outputs();
    outputs.pyright.stdout = "{not-json".into();
    let error_code = extract_static_analysis_labels(&happy_source(), &outputs)
        .unwrap_err()
        .code()
        .to_string();
    Ok(EdgeCase {
        name: "malformed_pyright_json_fails_closed",
        passed: error_code == "MEJEPA_INSTRUMENTS_INVALID_INPUT",
        detail: json!({"error_code": error_code}),
    })
}

fn install_static_analysis_shims(fsv_root: &Path) -> FsvResult<OsString> {
    let shim_dir = fsv_root.join("shims");
    fs::create_dir_all(&shim_dir)?;
    write_executable(
        &shim_dir.join("mypy"),
        r#"#!/bin/sh
printf '%s\n' 'sample.py:5: error: Incompatible return value type (got "int", expected "str")  [return-value]'
exit 1
"#,
    )?;
    write_executable(
        &shim_dir.join("pyright"),
        r#"#!/bin/sh
printf '%s\n' '{"generalDiagnostics":[{"file":"sample.py","severity":"error","message":"Type \"int\" is not assignable to return type \"str\"","range":{"start":{"line":4,"character":11},"end":{"line":4,"character":12}},"rule":"reportReturnType"}]}'
exit 1
"#,
    )?;
    write_executable(
        &shim_dir.join("ruff"),
        r#"#!/bin/sh
printf '%s\n' '[{"cell":null,"code":"F401","end_location":{"column":29,"row":1},"filename":"sample.py","fix":null,"location":{"column":20,"row":1},"message":"`typing.Any` imported but unused","noqa_row":1,"url":"https://docs.astral.sh/ruff/rules/unused-import/"}]'
exit 1
"#,
    )?;
    let old_path = env::var_os("PATH").unwrap_or_default();
    let new_path = if old_path.is_empty() {
        shim_dir.as_os_str().to_os_string()
    } else {
        OsString::from(format!(
            "{}:{}",
            shim_dir.display(),
            old_path.to_string_lossy()
        ))
    };
    env::set_var("PATH", new_path);
    Ok(old_path)
}

fn restore_path(old_path: OsString) {
    env::set_var("PATH", old_path);
}

fn write_executable(path: &Path, body: &str) -> FsvResult<()> {
    fs::write(path, body.as_bytes())?;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

fn write_json_checked(path: &Path, value: &Value) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, &bytes)?;
    let readback = fs::read(path)?;
    require("json write/readback bytes equal", readback == bytes)?;
    Ok(())
}

fn sha256_json<T: Serialize>(value: &T) -> FsvResult<String> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_bytes(&bytes))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn require(message: &'static str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
