use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::shift_log::{append_shift, ShiftRecord};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::process::Command;

impl Handlers {
    pub(crate) async fn call_reality_run_smoke(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_run_smoke(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_run_attempt(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_run_attempt(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_run_official_eval(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_run_official_eval(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_run_targeted_test(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_run_targeted_test(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_recompile_reward(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_recompile_reward(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }

    pub(crate) async fn call_reality_query_ledger(
        &self,
        id: Option<JsonRpcId>,
        args: Value,
    ) -> JsonRpcResponse {
        match reality_query_ledger(args).await {
            Ok(v) => super::ok(self, id, v),
            Err(e) => super::err(id, e),
        }
    }
}

pub async fn reality_run_smoke(args: Value) -> Result<Value> {
    let runtime_root = optional_str(&args, "runtime_root")
        .unwrap_or_else(|| "/tmp/cgreality-mcp-smoke".to_string());
    let label = optional_str(&args, "label").unwrap_or_else(|| "mcp-smoke".to_string());
    let bin = locate_engine_binary()?;
    let out = Command::new(&bin)
        .arg("smoke")
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--label")
        .arg(&label)
        .output()
        .await
        .map_err(|e| spawn_err("reality_run_smoke", &bin, e))?;
    let payload = engine_output("reality_run_smoke", &bin, out)?;
    if payload.get("status").and_then(Value::as_str) == Some("success") {
        let rr = PathBuf::from(&runtime_root);
        let session = session_id(&args);
        let mut shift = ShiftRecord::new(
            "reality_run_smoke",
            &session,
            json!({"type": "engine_run", "subcommand": "smoke", "runtime_root": runtime_root}),
        )?;
        shift.after = json!({"stdout": payload.get("stdout")});
        shift.delta_summary = json!({"exit_code": payload.get("exit_code")});
        append_shift(&rr, &session, &shift)?;
    }
    Ok(payload)
}

pub async fn reality_run_attempt(args: Value) -> Result<Value> {
    let task_manifest = required_str(&args, "task_manifest")?;
    let runtime_root = optional_str(&args, "runtime_root")
        .unwrap_or_else(|| "/tmp/contextgraph-rust-reality-loop".to_string());
    let max_attempts = optional_u64(&args, "max_attempts", 1).to_string();
    let model = optional_str(&args, "model").unwrap_or_else(|| "gemma4:e4b".to_string());
    let official = optional_bool(&args, "official_eval", true);
    let bin = locate_engine_binary()?;
    let mut cmd = Command::new(&bin);
    cmd.arg("attempt")
        .arg("--task-manifest")
        .arg(task_manifest)
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--max-attempts")
        .arg(max_attempts)
        .arg("--model")
        .arg(model);
    if official {
        cmd.arg("--official-eval-each-attempt");
    }
    let out = cmd
        .output()
        .await
        .map_err(|e| spawn_err("reality_run_attempt", &bin, e))?;
    engine_output("reality_run_attempt", &bin, out)
}

pub async fn reality_run_official_eval(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let target = require_active_target_instance().await?;
    let bin = locate_engine_binary()?;
    let out = Command::new(&bin)
        .arg("official-eval")
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--run-id")
        .arg(&run_id)
        .arg("--target-instance")
        .arg(&target)
        .arg("--attempt")
        .arg(attempt.to_string())
        .output()
        .await
        .map_err(|e| spawn_err("reality_run_official_eval", &bin, e))?;
    engine_output("reality_run_official_eval", &bin, out)
}

pub async fn reality_run_targeted_test(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let test_id = required_str(&args, "test_id")?;
    let timeout_secs = optional_u64(&args, "timeout_secs", 300).to_string();
    let runtime_root = require_active_runtime_root().await?;
    let target = require_active_target_instance().await?;
    let bin = locate_engine_binary()?;
    let out = Command::new(&bin)
        .arg("targeted-test")
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--run-id")
        .arg(&run_id)
        .arg("--target-instance")
        .arg(&target)
        .arg("--attempt")
        .arg(attempt.to_string())
        .arg("--test-id")
        .arg(&test_id)
        .arg("--timeout-secs")
        .arg(timeout_secs)
        .output()
        .await
        .map_err(|e| spawn_err("reality_run_targeted_test", &bin, e))?;
    engine_output("reality_run_targeted_test", &bin, out)
}

pub async fn reality_recompile_reward(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let runtime_root = require_active_runtime_root().await?;
    let task = require_active_target_instance().await?;
    let summary = attempt_dir(&runtime_root, &run_id, &task, attempt)?.join("attempt-summary.json");
    let bin = locate_engine_binary()?;
    let out = Command::new(&bin)
        .arg("replay")
        .arg("--attempt-summary-path")
        .arg(&summary)
        .arg("--label")
        .arg("recompile-reward")
        .output()
        .await
        .map_err(|e| spawn_err("reality_recompile_reward", &bin, e))?;
    engine_output("reality_recompile_reward", &bin, out)
}

pub async fn reality_query_ledger(args: Value) -> Result<Value> {
    let where_clause = required_str(&args, "where")?;
    let limit = optional_u64(&args, "limit", 50).min(500);
    let lower = where_clause.to_ascii_lowercase();
    for denied in [
        "select", "insert", "update", "delete", "drop", "alter", ";", "--",
    ] {
        if lower.contains(denied) {
            return Err(CCRealityError::new(
                "CCREALITY_LEDGER_QUERY_REJECTED",
                format!("WHERE clause may not contain '{denied}'"),
                "reality_query_ledger.where",
                "use only column comparisons in the WHERE clause",
                json!({"where": where_clause}),
                None,
            ));
        }
    }
    let runtime_root = require_active_runtime_root().await?;
    let ledger = find_ledger(&runtime_root)?;
    let conn =
        rusqlite::Connection::open_with_flags(&ledger, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| sqlite_err("CCREALITY_LEDGER_OPEN_FAILED", &ledger, e))?;
    let sql = format!(
        "SELECT record_kind, created_at, payload_json FROM ledger_records WHERE {where_clause} ORDER BY created_at DESC LIMIT {limit}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_PREPARE_FAILED", &ledger, e))?;
    let rows = stmt
        .query_map([], |row| {
            Ok(json!({
                "record_kind": row.get::<_, String>(0)?,
                "created_at": row.get::<_, String>(1)?,
                "payload_json": row.get::<_, String>(2)?
            }))
        })
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_QUERY_FAILED", &ledger, e))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_ROW_READ_FAILED", &ledger, e))?;
    Ok(json!({
        "rows": rows,
        "row_count": rows.len(),
        "source_of_truth": format!("sqlite:{}#ledger_records", ledger.display())
    }))
}

fn locate_engine_binary() -> Result<PathBuf> {
    let root = project_root()?;
    let bin = root.join("target/debug/reality-loop");
    if !bin.is_file() {
        return Err(CCRealityError::new(
            "CCREALITY_ENGINE_BINARY_MISSING",
            "target/debug/reality-loop is missing",
            "engine.binary",
            "run cargo build -p context-graph-cli --bin reality-loop",
            json!({"path": bin.display().to_string()}),
            Some(file_sot(&bin)),
        ));
    }
    Ok(bin)
}

fn engine_output(tool: &str, bin: &Path, output: std::process::Output) -> Result<Value> {
    let stdout_text = String::from_utf8(output.stdout).map_err(|e| {
        CCRealityError::new(
            "CCREALITY_INTERACT_STDOUT_UTF8_INVALID",
            format!("engine stdout was not valid UTF-8: {e}"),
            format!("{tool}.stdout"),
            "inspect engine stderr and fix the engine output",
            json!({"bin": bin.display().to_string()}),
            None,
        )
    })?;
    let stderr_text = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout_json = if stdout_text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&stdout_text).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_INTERACT_STDOUT_JSON_INVALID",
                format!("engine stdout was not JSON: {e}"),
                format!("{tool}.stdout"),
                "inspect engine stderr and fix the engine output",
                json!({"stdout": stdout_text, "stderr": stderr_text}),
                None,
            )
        })?
    };
    if !output.status.success() {
        let stderr_json = serde_json::from_str::<Value>(&stderr_text).ok();
        return Err(CCRealityError::new(
            "CCREALITY_INTERACT_ENGINE_FAILED",
            format!("engine command failed for {tool}"),
            tool,
            "inspect engine stderr, source_of_truth, and failure artifacts before retrying",
            json!({
                "bin": bin.display().to_string(),
                "exit_code": output.status.code(),
                "stdout": stdout_json,
                "stderr_tail": tail(&stderr_text, 4096),
                "stderr_json": stderr_json
            }),
            stdout_json
                .get("source_of_truth")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
        ));
    }
    Ok(json!({
        "status": "success",
        "exit_code": output.status.code(),
        "stdout": stdout_json,
        "stderr_tail": tail(&stderr_text, 4096),
        "source_of_truth": stdout_json.get("source_of_truth").cloned().unwrap_or(Value::Null)
    }))
}

fn spawn_err(tool: &str, bin: &Path, err: std::io::Error) -> CCRealityError {
    CCRealityError::new(
        "CCREALITY_INTERACT_SPAWN_FAILED",
        format!("failed to spawn engine: {err}"),
        format!("{tool}.spawn"),
        "ensure target/debug/reality-loop is built and executable",
        json!({"bin": bin.display().to_string()}),
        Some(file_sot(bin)),
    )
}

fn tail(text: &str, limit: usize) -> String {
    let len = text.chars().count();
    if len <= limit {
        text.to_string()
    } else {
        text.chars().skip(len - limit).collect()
    }
}

fn find_ledger(root: &Path) -> Result<PathBuf> {
    let direct = root.join("ledger.sqlite");
    if direct.is_file() {
        return Ok(direct);
    }
    let mut found = Vec::new();
    collect_ledgers(root, &mut found)?;
    found.into_iter().next().ok_or_else(|| {
        CCRealityError::new(
            "CCREALITY_LEDGER_MISSING",
            "no ledger.sqlite or blackbox.sqlite was found under active runtime root",
            "ledger.path",
            "run an attempt that initializes the BlackBox ledger",
            json!({"runtime_root": root.display().to_string()}),
            Some(file_sot(root)),
        )
    })
}

fn collect_ledgers(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(root)
        .map_err(|e| fs_error("CCREALITY_LEDGER_WALK_READ_FAILED", root, e))?
    {
        let entry = entry.map_err(|e| fs_error("CCREALITY_LEDGER_WALK_ENTRY_FAILED", root, e))?;
        let path = entry.path();
        if path.is_dir() {
            collect_ledgers(&path, out)?;
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

fn sqlite_err(code: &str, path: &Path, err: rusqlite::Error) -> CCRealityError {
    CCRealityError::new(
        code,
        format!("SQLite operation failed: {err}"),
        "ledger.sqlite",
        "inspect ledger schema and query shape",
        json!({"path": path.display().to_string(), "error": err.to_string()}),
        Some(format!("sqlite:{}#ledger_records", path.display())),
    )
}
