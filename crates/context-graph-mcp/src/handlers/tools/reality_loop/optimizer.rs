use super::errors::Result;
use super::helpers::*;
use super::schema_validator::RecommendationValidator;
use super::shift_log::{append_shift, ShiftRecord};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::process::Command;

impl Handlers {
    pub(crate) async fn call_optimizer_record_decision(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_record_decision(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_record_recommendation(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_record_recommendation(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_optimizer_record_harness_transition(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match optimizer_record_harness_transition(args, true).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn optimizer_record_decision(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let session = required_str(&args, "claude_session_id").or_else(|_| Ok(session_id(&args)))?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let attempt_dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let optimizer_dir = attempt_dir.join("claude-code-optimizer");
    fs::create_dir_all(&optimizer_dir)
        .map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_CREATE_FAILED", &optimizer_dir, e))?;
    let turn = next_turn(&optimizer_dir, "decision-turn-")?;
    let path = optimizer_dir.join(format!("decision-turn-{turn:02}.json"));
    let trigger_path = attempt_dir.join("trigger-decision.json");
    let payload = json!({
        "record_kind": "ccreality_optimizer_decision",
        "schema_version": 1,
        "run_id": run_id,
        "attempt": attempt,
        "turn_number": turn,
        "policy": args.get("policy"),
        "should_run": args.get("should_run"),
        "reasons": args.get("reasons").cloned().unwrap_or_else(|| json!([])),
        "claude_session_id": session,
        "claude_model": args.get("claude_model"),
        "created_at_unix": unix_secs()?,
        "source_of_truth": file_sot(&path)
    });
    write_json_checked(&path, &payload)?;
    write_json_checked(&trigger_path, &payload)?;
    let ledger_append = append_optimizer_ledger(
        &runtime_root,
        &optimizer_dir,
        "optimizer_record_decision",
        &payload,
    )
    .await?;

    let mut shift = ShiftRecord::new(
        "optimizer_record_decision",
        payload
            .get("claude_session_id")
            .and_then(Value::as_str)
            .unwrap_or("cgreality-stdio-session"),
        json!({"type": "decision", "policy": payload.get("policy"), "should_run": payload.get("should_run")}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary = json!({"artifact": file_sot(&path)});
    append_shift(&runtime_root, &shift.session_id, &shift)?;

    Ok(json!({
        "decision_path": file_sot(&path),
        "trigger_decision_path": file_sot(&trigger_path),
        "sha256": sha256_file(&path)?,
        "ledger_record_id": ledger_append.get("record_id").cloned().unwrap_or(Value::Null),
        "ledger_append": ledger_append,
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn optimizer_record_recommendation(args: Value) -> Result<Value> {
    // Phase 13 T13.2: tolerate JSON-stringified arguments from JSON-RPC clients that
    // collapse typed values (integers, arrays, booleans) into strings before transport.
    let args = coerce_stringified_args(args);
    let validator = RecommendationValidator::load()?;
    validator.validate(&args)?;
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let session = required_str(&args, "claude_session_id")?;
    let turn = required_u64(&args, "turn_number")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let attempt_dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let optimizer_dir = attempt_dir.join("claude-code-optimizer");
    fs::create_dir_all(&optimizer_dir)
        .map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_CREATE_FAILED", &optimizer_dir, e))?;
    let path = optimizer_dir.join(format!("recommendation-turn-{turn:02}.json"));
    let mut payload = args;
    if let Value::Object(map) = &mut payload {
        map.insert(
            "source_of_truth".to_string(),
            Value::String(file_sot(&path)),
        );
        map.insert("created_at_unix".to_string(), json!(unix_secs()?));
    }
    validator.validate(&payload)?;
    write_json_checked(&path, &payload)?;
    let ledger_append = append_optimizer_ledger(
        &runtime_root,
        &optimizer_dir,
        "optimizer_record_recommendation",
        &payload,
    )
    .await?;

    let mut shift = ShiftRecord::new(
        "optimizer_record_recommendation",
        &session,
        json!({"type": "recommendation", "status": payload.get("status")}),
    )?;
    shift.after = json!({"sha256": sha256_file(&path)?});
    shift.delta_summary = json!({"artifact": file_sot(&path)});
    append_shift(&runtime_root, &session, &shift)?;

    Ok(json!({
        "recommendation_path": file_sot(&path),
        "sha256": sha256_file(&path)?,
        "ledger_record_id": ledger_append.get("record_id").cloned().unwrap_or(Value::Null),
        "ledger_append": ledger_append,
        "shift_id": shift.shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub async fn optimizer_record_harness_transition(
    args: Value,
    append_external_shift: bool,
) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let dir = attempt_dir(&runtime_root, &run_id, &task, attempt)?;
    let path = write_harness_transition(&dir, &args)?;
    let round = read_json(&path)?
        .get("round")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let session = session_id(&args);
    let mut shift_id = Value::Null;
    if append_external_shift {
        let mut shift = ShiftRecord::new(
            "optimizer_record_harness_transition",
            &session,
            json!({"type": "harness_transition", "path": file_sot(&path)}),
        )?;
        shift.after = json!({"sha256": sha256_file(&path)?});
        shift.harness_transition_path = Some(file_sot(&path));
        append_shift(&runtime_root, &session, &shift)?;
        shift_id = json!(shift.shift_id);
    }
    Ok(json!({
        "transition_path": file_sot(&path),
        "round": round,
        "shift_id": shift_id,
        "source_of_truth": file_sot(&path)
    }))
}

pub fn write_harness_transition(attempt_dir: &Path, payload: &Value) -> Result<PathBuf> {
    let dir = attempt_dir.join("harness-transitions");
    fs::create_dir_all(&dir)
        .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_DIR_CREATE_FAILED", &dir, e))?;
    let round = next_round(&dir)?;
    let path = dir.join(format!("round-{round:02}.json"));
    let mut body = payload.clone();
    if let Value::Object(map) = &mut body {
        map.insert(
            "record_kind".to_string(),
            json!("ccreality_harness_transition"),
        );
        map.insert("schema_version".to_string(), json!(1));
        map.insert("round".to_string(), json!(round));
        map.insert("created_at_unix".to_string(), json!(unix_secs()?));
        map.insert("source_of_truth".to_string(), json!(file_sot(&path)));
        map.entry("outcome".to_string())
            .or_insert(json!("recorded"));
    }
    write_json_checked(&path, &body)?;
    Ok(path)
}

fn next_turn(dir: &Path, prefix: &str) -> Result<u64> {
    let mut max_turn = 0;
    if dir.is_dir() {
        for entry in fs::read_dir(dir)
            .map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_READ_FAILED", dir, e))?
        {
            let entry =
                entry.map_err(|e| fs_error("CCREALITY_OPTIMIZER_DIR_ENTRY_FAILED", dir, e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(num) = name
                .strip_prefix(prefix)
                .and_then(|s| s.strip_suffix(".json"))
                .and_then(|s| s.parse::<u64>().ok())
            {
                max_turn = max_turn.max(num);
            }
        }
    }
    Ok(max_turn + 1)
}

fn next_round(dir: &Path) -> Result<u64> {
    let mut max_round = 0;
    if dir.is_dir() {
        for entry in fs::read_dir(dir)
            .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_DIR_READ_FAILED", dir, e))?
        {
            let entry = entry
                .map_err(|e| fs_error("CCREALITY_HARNESS_TRANSITION_DIR_ENTRY_FAILED", dir, e))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(num) = name
                .strip_prefix("round-")
                .and_then(|s| s.strip_suffix(".json"))
                .and_then(|s| s.parse::<u64>().ok())
            {
                max_round = max_round.max(num);
            }
        }
    }
    Ok(max_round + 1)
}

async fn append_optimizer_ledger(
    runtime_root: &Path,
    optimizer_dir: &Path,
    tool_name: &str,
    payload: &Value,
) -> Result<Value> {
    let ledger = find_optimizer_ledger(runtime_root)?;
    let blackbox_run_id = latest_blackbox_run_id(&ledger)?;
    let input_path =
        optimizer_dir.join(format!("ledger-input-{}-{}.json", tool_name, unix_secs()?));
    write_json_checked(&input_path, payload)?;
    let bin = project_root()?.join("target/debug/context-graph");
    if !bin.is_file() {
        return Err(super::errors::CCRealityError::new(
            "CCREALITY_OPTIMIZER_BLACKBOX_BINARY_MISSING",
            "target/debug/context-graph is missing",
            "optimizer.ledger.binary",
            "run cargo build -p context-graph-cli --bin context-graph",
            json!({"path": bin.display().to_string()}),
            Some(file_sot(&bin)),
        ));
    }
    let mut cmd = Command::new(&bin);
    cmd.arg("blackbox")
        .arg("record")
        .arg("tool-call")
        .arg("--ledger")
        .arg(&ledger)
        .arg("--run-id")
        .arg(&blackbox_run_id)
        .arg("--tool")
        .arg(tool_name)
        .arg("--input")
        .arg(&input_path)
        .arg("--agent")
        .arg("cgreality_optimizer")
        .arg("--capability")
        .arg("ccreality.optimizer")
        .arg("--status")
        .arg("ok");
    if let Some(model) = payload.get("claude_model").and_then(Value::as_str) {
        cmd.arg("--model").arg(model);
    }
    let output = cmd.output().await.map_err(|e| {
        super::errors::CCRealityError::new(
            "CCREALITY_OPTIMIZER_LEDGER_APPEND_SPAWN_FAILED",
            format!("failed to spawn BlackBox ledger append: {e}"),
            "optimizer.ledger.spawn",
            "ensure target/debug/context-graph is executable",
            json!({"bin": bin.display().to_string(), "ledger": ledger.display().to_string()}),
            Some(file_sot(&bin)),
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        return Err(super::errors::CCRealityError::new(
            "CCREALITY_OPTIMIZER_LEDGER_APPEND_FAILED",
            "BlackBox ledger append failed",
            "optimizer.ledger.append",
            "inspect the ledger, run id, and ledger input artifact",
            json!({
                "exit_code": output.status.code(),
                "stdout": stdout,
                "stderr": stderr,
                "ledger_input": file_sot(&input_path)
            }),
            Some(format!("sqlite:{}#ledger_records", ledger.display())),
        ));
    }
    let mut parsed: Value = serde_json::from_str(&stdout).map_err(|e| {
        super::errors::CCRealityError::new(
            "CCREALITY_OPTIMIZER_LEDGER_APPEND_JSON_INVALID",
            format!("BlackBox ledger append stdout was not JSON: {e}"),
            "optimizer.ledger.stdout",
            "inspect BlackBox stdout/stderr",
            json!({"stdout": stdout, "stderr": stderr}),
            Some(format!("sqlite:{}#ledger_records", ledger.display())),
        )
    })?;
    if let Value::Object(map) = &mut parsed {
        map.insert("ledger_input".to_string(), json!(file_sot(&input_path)));
    }
    Ok(parsed)
}

fn find_optimizer_ledger(root: &Path) -> Result<PathBuf> {
    let direct = root.join("ledger.sqlite");
    if direct.is_file() {
        return Ok(direct);
    }
    let mut found = Vec::new();
    collect_optimizer_ledgers(root, &mut found)?;
    found.into_iter().next().ok_or_else(|| {
        super::errors::CCRealityError::new(
            "CCREALITY_OPTIMIZER_LEDGER_MISSING",
            "no ledger.sqlite or blackbox.sqlite was found under active runtime root",
            "optimizer.ledger.path",
            "initialize a real BlackBox ledger before recording optimizer artifacts",
            json!({"runtime_root": root.display().to_string()}),
            Some(file_sot(root)),
        )
    })
}

fn collect_optimizer_ledgers(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)
        .map_err(|e| fs_error("CCREALITY_OPTIMIZER_LEDGER_WALK_READ_FAILED", root, e))?
    {
        let entry =
            entry.map_err(|e| fs_error("CCREALITY_OPTIMIZER_LEDGER_WALK_ENTRY_FAILED", root, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_optimizer_ledgers(&path, out)?;
        } else if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n == "ledger.sqlite" || n == "blackbox.sqlite")
        {
            out.push(path);
        }
    }
    Ok(())
}

fn latest_blackbox_run_id(ledger: &Path) -> Result<String> {
    let conn =
        rusqlite::Connection::open_with_flags(ledger, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| {
            super::errors::CCRealityError::new(
                "CCREALITY_OPTIMIZER_LEDGER_OPEN_FAILED",
                format!("failed to open ledger: {e}"),
                "optimizer.ledger.open",
                "inspect the ledger path and SQLite permissions",
                json!({"path": ledger.display().to_string(), "error": e.to_string()}),
                Some(format!("sqlite:{}#ledger_records", ledger.display())),
            )
        })?;
    conn.query_row(
        "SELECT run_id FROM ledger_records WHERE record_kind = 'agent_run' AND run_id IS NOT NULL ORDER BY sequence DESC LIMIT 1",
        [],
        |row| row.get::<_, String>(0),
    )
    .map_err(|e| {
        super::errors::CCRealityError::new(
            "CCREALITY_OPTIMIZER_LEDGER_RUN_MISSING",
            format!("could not read a BlackBox agent_run row: {e}"),
            "optimizer.ledger.run_id",
            "record a real BlackBox agent_run before optimizer artifacts",
            json!({"path": ledger.display().to_string(), "error": e.to_string()}),
            Some(format!("sqlite:{}#ledger_records", ledger.display())),
        )
    })
}
