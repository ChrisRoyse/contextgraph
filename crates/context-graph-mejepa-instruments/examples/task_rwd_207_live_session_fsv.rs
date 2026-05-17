use context_graph_mejepa_instruments::live_session_signals::{
    extract_live_session_signals, write_live_session_raw_output, LiveSessionSignalRecord,
    LiveSessionSignalStore, LiveSessionSource, OperatorOverrideKind, OperatorOverrideObservation,
    SessionVerdictKind, SessionVerdictObservation, UserPromptEvent,
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
        PathBuf::from("/mnt/d/contextgraph/fsv/task-rwd-207-live-session-fsv").join(&run_id);
    let raw_root = PathBuf::from("/mnt/d/contextgraph/corpus/python-live-session-signals-v1")
        .join("task-rwd-207-fsv")
        .join(&run_id);
    let db_path = fsv_root.join("live_session_signals.db");
    fs::create_dir_all(&fsv_root)?;
    fs::create_dir_all(&raw_root)?;
    fs::set_permissions(&fsv_root, fs::Permissions::from_mode(0o700))?;
    fs::set_permissions(&raw_root, fs::Permissions::from_mode(0o700))?;

    let evidence_path = fsv_root.join("live_session_fsv.json");
    let source = happy_source();
    let before_state = json!({
        "db_exists": db_path.exists(),
        "raw_root_exists": raw_root.exists(),
        "raw_file_count": count_files(&raw_root)?,
    });

    let extraction = extract_live_session_signals(&source)?;
    require(
        "happy extraction emits one label",
        extraction.labels.len() == 1,
    )?;
    require(
        "happy extraction emits one goal",
        extraction.goals.len() == 1,
    )?;
    require(
        "happy extraction emits four verdicts",
        extraction.verdicts.len() == 4,
    )?;
    require(
        "happy extraction emits three overrides",
        extraction.overrides.len() == 3,
    )?;
    let label = extraction.labels.first().unwrap();
    require("prompt count is two", label.prompt_event_count == 2)?;
    require(
        "prior verdict count is four",
        label.prior_verdict_count == 4,
    )?;
    require("prior fail count is one", label.prior_fail_count == 1)?;
    require("prior OOD count is one", label.prior_ood_count == 1)?;
    require(
        "override count is three",
        label.operator_override_count == 3,
    )?;
    require(
        "refuted override count is one",
        label.refuted_override_count == 1,
    )?;
    require(
        "refuted fraction is one third",
        (label.refuted_override_fraction - (1.0 / 3.0)).abs() < 0.0001,
    )?;
    require("goal embedding dim is 384", label.goal_embedding_dim == 384)?;
    require(
        "goal embedding hash is sha256-prefixed",
        label.goal_embedding_sha256.starts_with("sha256:"),
    )?;
    require(
        "dominant goal terms include reward",
        label
            .dominant_goal_terms
            .iter()
            .any(|term| term == "reward"),
    )?;

    let raw_paths = write_live_session_raw_output(&raw_root, "task-rwd-207-happy", &extraction)?;
    let raw_readback: Value = serde_json::from_slice(&fs::read(&raw_paths.live_session_json)?)?;
    require(
        "raw live-session output readback has one label",
        raw_readback["labels"].as_array().map_or(0, Vec::len) == 1,
    )?;

    let expected_records = extraction.persisted_records().len();
    let keys = {
        let store = LiveSessionSignalStore::open(&db_path)?;
        let keys = store.put_extraction(&extraction)?;
        store.flush()?;
        require(
            "CF key count matches records",
            keys.len() == expected_records,
        )?;
        require(
            "count after write matches records",
            store.count_records()? == expected_records,
        )?;
        keys
    };
    let reopened_rows = {
        let reopened = LiveSessionSignalStore::open(&db_path)?;
        reopened.scan_records()?
    };
    require(
        "reopened CF count matches records",
        reopened_rows.len() == expected_records,
    )?;
    let reopened_label = reopened_rows
        .iter()
        .find_map(|(_, record)| match &record.signal {
            LiveSessionSignalRecord::Label(label) => Some(label),
            _ => None,
        })
        .ok_or("missing reopened live-session label")?;
    require(
        "reopened label matches written label",
        reopened_label == label,
    )?;

    let minimal = edge_minimal_prompt_only_persists(&db_path)?;
    let edge_cases = vec![
        edge_invalid_source("empty_prompt_history", |source| {
            source.prompt_history.clear()
        }),
        edge_invalid_source("duplicate_prompt_event", |source| {
            source.prompt_history[1].event_id = source.prompt_history[0].event_id.clone()
        }),
        edge_invalid_source("nul_prompt", |source| {
            source.prompt_history[0].prompt.push('\0')
        }),
        edge_invalid_source("empty_prediction_id", |source| {
            source.prior_verdicts[0].prediction_id.clear()
        }),
        edge_invalid_source("negative_override_timestamp", |source| {
            source.operator_overrides[0].unix_secs = -1
        }),
        edge_invalid_source("empty_override_reason", |source| {
            source.operator_overrides[0].reason.clear()
        }),
        minimal,
    ];
    require(
        "all edge cases pass",
        edge_cases.iter().all(|case| case.passed),
    )?;

    let after_state = json!({
        "db_exists": db_path.exists(),
        "raw_live_session_json": raw_paths.live_session_json,
        "raw_live_session_sha256": sha256_file(&raw_paths.live_session_json)?,
        "cf_keys": keys,
        "reopened_count": reopened_rows.len(),
        "raw_file_count": count_files(&raw_root)?,
    });
    let evidence = json!({
        "schema_version": 1,
        "task_id": "TASK-RWD-207",
        "issue": 315,
        "run_id": run_id,
        "all_passed": true,
        "source_of_truth": {
            "fsv_json": evidence_path,
            "rocksdb": db_path,
            "raw_root": raw_root,
            "cf": "CF_MEJEPA_LIVE_SESSION_SIGNALS",
        },
        "before_state": before_state,
        "trigger": "extract E_LiveSession goal/session evidence, persist raw output, sync-write CF rows, drop and reopen RocksDB",
        "after_state": after_state,
        "happy_path": {
            "label": label,
            "goal": extraction.goals.first(),
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

fn happy_source() -> LiveSessionSource {
    LiveSessionSource {
        session_id: "session-rwd-207".to_string(),
        turn_id: "turn-0002".to_string(),
        prompt_history: vec![
            UserPromptEvent {
                event_id: "prompt-1".to_string(),
                unix_secs: 1_779_055_000,
                prompt: "Read the GitHub issues and capture all reward signals for ME-JEPA."
                    .to_string(),
            },
            UserPromptEvent {
                event_id: "prompt-2".to_string(),
                unix_secs: 1_779_055_060,
                prompt: "Finish the live-session reward signal with operator goal evidence."
                    .to_string(),
            },
        ],
        prior_verdicts: vec![
            verdict("pred-pass", SessionVerdictKind::Pass),
            verdict("pred-fail", SessionVerdictKind::Fail),
            verdict("pred-ood", SessionVerdictKind::Ood),
            verdict("pred-abstain", SessionVerdictKind::Abstain),
        ],
        operator_overrides: vec![
            override_row("pred-pass", OperatorOverrideKind::Confirmed),
            override_row("pred-fail", OperatorOverrideKind::Refuted),
            override_row("pred-ood", OperatorOverrideKind::Inconclusive),
        ],
    }
}

fn edge_minimal_prompt_only_persists(db_path: &Path) -> FsvResult<EdgeCase> {
    let mut source = happy_source();
    source.session_id = "session-rwd-207-minimal".to_string();
    source.turn_id = "turn-minimal".to_string();
    source.prior_verdicts.clear();
    source.operator_overrides.clear();
    let extraction = extract_live_session_signals(&source)?;
    let label = extraction.labels.first().ok_or("missing minimal label")?;
    let store = LiveSessionSignalStore::open(db_path)?;
    let before = store.count_records()?;
    let keys = store.put_extraction(&extraction)?;
    store.flush()?;
    let after = store.count_records()?;
    Ok(EdgeCase {
        name: "minimal_prompt_only_persists",
        passed: label.prior_verdict_count == 0
            && label.operator_override_count == 0
            && keys.len() == extraction.persisted_records().len()
            && after == before + keys.len(),
        error_code: None,
        details: json!({
            "before_count": before,
            "after_count": after,
            "label": label,
            "keys": keys,
        }),
    })
}

fn edge_invalid_source(
    name: &'static str,
    mutate: impl FnOnce(&mut LiveSessionSource),
) -> EdgeCase {
    let mut source = happy_source();
    mutate(&mut source);
    match extract_live_session_signals(&source) {
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

fn verdict(prediction_id: &str, verdict: SessionVerdictKind) -> SessionVerdictObservation {
    SessionVerdictObservation {
        prediction_id: prediction_id.to_string(),
        verdict,
        fingerprint_id: Some(format!("fp-{prediction_id}")),
    }
}

fn override_row(prediction_id: &str, outcome: OperatorOverrideKind) -> OperatorOverrideObservation {
    OperatorOverrideObservation {
        prediction_id: prediction_id.to_string(),
        outcome,
        unix_secs: 1_779_055_120,
        reason: format!("{prediction_id} operator feedback is durable reality"),
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
