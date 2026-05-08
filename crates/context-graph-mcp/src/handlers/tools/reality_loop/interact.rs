use super::errors::{CCRealityError, Result};
use super::helpers::*;
use super::shift_log::{append_shift, ShiftRecord};
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, Instant};
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
    let runtime_root = optional_str_strict(&args, "runtime_root")?
        .unwrap_or_else(|| "/tmp/cgreality-mcp-smoke".to_string());
    let label = optional_str_strict(&args, "label")?.unwrap_or_else(|| "mcp-smoke".to_string());
    let bin = locate_engine_binary()?;
    let out = Command::new(&bin)
        .arg("smoke")
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--label")
        .arg(&label)
        // If this future is dropped (caller cancels, MCP shutdown, panic in
        // surrounding task), the child engine process gets SIGKILL instead of
        // becoming an orphan reparented to PID 1.
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| spawn_err("reality_run_smoke", &bin, e))?;
    let payload = engine_output("reality_run_smoke", &bin, out)?;
    if payload.get("status").and_then(Value::as_str) == Some("success") {
        let rr = PathBuf::from(&runtime_root);
        let session = session_id(&args)?;
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

/// Spawn a fresh attempt and return as soon as the engine has demonstrably
/// created its run directory. The engine continues running in its own
/// session (setsid) so that `/mcp reconnect`, MCP singleton-takeover, or any
/// SIGTERM/SIGKILL aimed at this MCP server does **not** cascade to the
/// engine.
///
/// Root cause of the prior MCP "Connection closed" pattern (2026-05-07):
/// the previous implementation called `cmd.output().await`, which blocks
/// the JSON-RPC handler for the full 1–2 hour engine runtime. Claude Code's
/// stdio MCP transport closes the session long before then, producing
/// `error -32000: Connection closed`. The engine survived only because the
/// old MCP got SIGKILL'd (its tokio runtime aborted before `kill_on_drop`
/// could fire). That accidental survival was load-bearing but fragile.
///
/// The fix detaches the engine cleanly:
///   - `setsid()` via pre_exec — engine becomes its own session leader, so
///     SIGHUP/SIGTERM aimed at the MCP process group do not reach it.
///   - stdout/stderr → log files under `<runtime_root>/_engine_logs/` so
///     diagnostics survive MCP exit and the operator can inspect failures.
///   - Spawn returns immediately; we then poll `runtime_root` for the new
///     run directory the engine creates at reality-loop.rs:1578 (right after
///     parsing/validating the task manifest).
///   - If the engine exits before creating the run dir, we surface its
///     stdout/stderr tail via `CCREALITY_INTERACT_ENGINE_EXITED_DURING_STARTUP`.
///   - If it stays alive but fails to create the run dir within 30s, we
///     surface its tails via `CCREALITY_INTERACT_ENGINE_NO_RUN_DIR`.
pub async fn reality_run_attempt(args: Value) -> Result<Value> {
    let task_manifest = required_str(&args, "task_manifest")?;
    let runtime_root = optional_str_strict(&args, "runtime_root")?
        .unwrap_or_else(|| "/tmp/contextgraph-rust-reality-loop".to_string());
    let max_attempts = optional_u64_strict(&args, "max_attempts", 1)?.to_string();
    let model = optional_str_strict(&args, "model")?.unwrap_or_else(|| "qwen3.6:27b".to_string());
    let inner_model_path = optional_str_strict(&args, "inner_model_path")?;
    let inner_context_tokens = optional_u64_value_strict(&args, "inner_context_tokens")?;
    let inner_max_new_tokens = optional_u64_value_strict(&args, "inner_max_new_tokens")?;
    let official = optional_bool_strict(&args, "official_eval", true)?;
    let bin = locate_engine_binary()?;

    let rr_path = PathBuf::from(&runtime_root);
    fs::create_dir_all(&rr_path)
        .map_err(|e| fs_error("CCREALITY_RUNTIME_ROOT_CREATE_FAILED", &rr_path, e))?;

    // Snapshot the existing top-level run directories so we can identify the
    // brand-new one created by THIS spawn (engine writes <rr>/<loop_id>/).
    let pre_existing = list_run_dirs(&rr_path)?;

    // Engine logs live under the runtime root so they survive MCP exit and
    // can be tailed by the operator after the fact.
    let logs_dir = rr_path.join("_engine_logs");
    fs::create_dir_all(&logs_dir)
        .map_err(|e| fs_error("CCREALITY_ENGINE_LOG_DIR_CREATE_FAILED", &logs_dir, e))?;
    let log_id = format!("attempt-{}-{}", unix_ns()?, std::process::id());
    let stdout_log = logs_dir.join(format!("{log_id}.stdout.log"));
    let stderr_log = logs_dir.join(format!("{log_id}.stderr.log"));
    let stdout_file = std::fs::File::create(&stdout_log)
        .map_err(|e| fs_error("CCREALITY_ENGINE_STDOUT_LOG_CREATE_FAILED", &stdout_log, e))?;
    let stderr_file = std::fs::File::create(&stderr_log)
        .map_err(|e| fs_error("CCREALITY_ENGINE_STDERR_LOG_CREATE_FAILED", &stderr_log, e))?;

    // Use std::process::Command so we own the lifetime end-to-end and avoid
    // tokio's Child reaper auto-registering the engine. Engine survives
    // independently after this handler returns.
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("attempt")
        .arg("--task-manifest")
        .arg(&task_manifest)
        .arg("--runtime-root")
        .arg(&runtime_root)
        .arg("--max-attempts")
        .arg(max_attempts)
        .arg("--model")
        .arg(&model);
    if let Some(path) = inner_model_path.as_deref() {
        cmd.arg("--inner-model-path").arg(path);
    }
    if let Some(tokens) = inner_context_tokens {
        cmd.arg("--inner-context-tokens").arg(tokens.to_string());
    }
    if let Some(tokens) = inner_max_new_tokens {
        cmd.arg("--inner-max-new-tokens").arg(tokens.to_string());
    }
    if official {
        cmd.arg("--official-eval-each-attempt");
    }
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::from(stdout_file))
        .stderr(std::process::Stdio::from(stderr_file));

    // setsid() makes the engine a session leader so it is detached from this
    // MCP process group. Same pattern as `spawn_daemon_process` in main.rs
    // (INFRA-H1). pre_exec error → spawn error.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| spawn_err("reality_run_attempt", &bin, e))?;
    let pid = child.id();

    // Poll for the engine's new run directory. Engine creates
    // <runtime_root>/<loop_id>/ at reality-loop.rs:1578 (right after parsing
    // and validating the task manifest). 30s deadline is generous —
    // observed startup is well under 5s on this hardware.
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut run_id: Option<String> = None;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stderr_text = std::fs::read_to_string(&stderr_log).unwrap_or_default();
                let stdout_text = std::fs::read_to_string(&stdout_log).unwrap_or_default();
                return Err(CCRealityError::new(
                    "CCREALITY_INTERACT_ENGINE_EXITED_DURING_STARTUP",
                    format!(
                        "engine exited before creating its run directory (exit_code={:?})",
                        status.code()
                    ),
                    "reality_run_attempt.startup",
                    "inspect engine stderr at details.stderr_log; fix the engine and retry",
                    json!({
                        "bin": bin.display().to_string(),
                        "pid": pid,
                        "exit_code": status.code(),
                        "runtime_root": runtime_root,
                        "task_manifest": task_manifest,
                        "stdout_log": stdout_log.display().to_string(),
                        "stderr_log": stderr_log.display().to_string(),
                        "stdout_tail": tail(&stdout_text, 4096),
                        "stderr_tail": tail(&stderr_text, 4096),
                    }),
                    Some(file_sot(&stderr_log)),
                ));
            }
            Ok(None) => {}
            Err(e) => {
                return Err(CCRealityError::new(
                    "CCREALITY_INTERACT_TRY_WAIT_FAILED",
                    format!("failed to query engine child status: {e}"),
                    "reality_run_attempt.try_wait",
                    "inspect host process state; engine pid in details.pid may need manual SIGTERM",
                    json!({
                        "pid": pid,
                        "stdout_log": stdout_log.display().to_string(),
                        "stderr_log": stderr_log.display().to_string(),
                        "error": e.to_string(),
                    }),
                    Some(file_sot(&stderr_log)),
                ));
            }
        }
        let current = list_run_dirs(&rr_path)?;
        if let Some(name) = current.difference(&pre_existing).next() {
            run_id = Some(name.clone());
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let Some(run_id) = run_id else {
        let stderr_text = std::fs::read_to_string(&stderr_log).unwrap_or_default();
        let stdout_text = std::fs::read_to_string(&stdout_log).unwrap_or_default();
        return Err(CCRealityError::new(
            "CCREALITY_INTERACT_ENGINE_NO_RUN_DIR",
            "engine started but did not create its run directory within 30s",
            "reality_run_attempt.startup_timeout",
            "inspect engine stderr at details.stderr_log; engine pid in details.pid may need manual SIGTERM",
            json!({
                "bin": bin.display().to_string(),
                "pid": pid,
                "runtime_root": runtime_root,
                "task_manifest": task_manifest,
                "stdout_log": stdout_log.display().to_string(),
                "stderr_log": stderr_log.display().to_string(),
                "stdout_tail": tail(&stdout_text, 4096),
                "stderr_tail": tail(&stderr_text, 4096),
            }),
            Some(file_sot(&stderr_log)),
        ));
    };

    // std::process::Child Drop neither waits nor kills — engine survives
    // this handler returning, this MCP exiting, /mcp reconnect, etc.
    drop(child);

    let run_dir = rr_path.join(&run_id);
    Ok(json!({
        "status": "spawned",
        "run_id": run_id,
        "runtime_root": runtime_root,
        "pid": pid,
        "model": model,
        "task_manifest": task_manifest,
        "stdout_log": stdout_log.display().to_string(),
        "stderr_log": stderr_log.display().to_string(),
        "source_of_truth": file_sot(&run_dir),
        "guidance": "engine runs in background; poll mcp__cgreality__reality_attempt_summary or reality_official_report for progress"
    }))
}

/// List top-level directories under runtime_root, excluding the metadata
/// dirs that are not engine runs (`_engine_logs`, `failures`, `workspaces`,
/// `cgreality-shift-log`, `cgreality-hook-state`, `claude-code-optimizer`).
fn list_run_dirs(rr: &Path) -> Result<HashSet<String>> {
    let mut out = HashSet::new();
    if !rr.is_dir() {
        return Ok(out);
    }
    for entry in
        fs::read_dir(rr).map_err(|e| fs_error("CCREALITY_RUNTIME_ROOT_READ_FAILED", rr, e))?
    {
        let entry =
            entry.map_err(|e| fs_error("CCREALITY_RUNTIME_ROOT_ENTRY_READ_FAILED", rr, e))?;
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_run_metadata_dir(&name) {
            out.insert(name);
        }
    }
    Ok(out)
}

fn is_run_metadata_dir(name: &str) -> bool {
    matches!(
        name,
        "_engine_logs"
            | "failures"
            | "workspaces"
            | "cgreality-shift-log"
            | "cgreality-hook-state"
            | "claude-code-optimizer"
    )
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
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| spawn_err("reality_run_official_eval", &bin, e))?;
    engine_output("reality_run_official_eval", &bin, out)
}

pub async fn reality_run_targeted_test(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let attempt = required_u64(&args, "attempt")?;
    let test_id = required_str(&args, "test_id")?;
    let timeout_secs = optional_u64_strict(&args, "timeout_secs", 300)?.to_string();
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
        .kill_on_drop(true)
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
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| spawn_err("reality_recompile_reward", &bin, e))?;
    engine_output("reality_recompile_reward", &bin, out)
}

pub async fn reality_query_ledger(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let where_clause = required_str(&args, "where")?;
    let limit = optional_u64_strict(&args, "limit", 50)?.min(500);
    let where_clause = validate_ledger_where(&where_clause)?;
    let runtime_root = require_active_runtime_root().await?;
    let target = read_active_target_instance().await?;
    let ledger = find_ledger_for_run(&runtime_root, &run_id, target.as_deref())?;
    let conn =
        rusqlite::Connection::open_with_flags(&ledger, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| sqlite_err("CCREALITY_LEDGER_OPEN_FAILED", &ledger, e))?;
    let sql = format!(
        "SELECT sequence, record_kind, created_at, payload_json FROM ledger_records WHERE run_id = ?1 AND ({where_clause}) ORDER BY sequence DESC LIMIT ?2"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_PREPARE_FAILED", &ledger, e))?;
    let rows = stmt
        .query_map(rusqlite::params![&run_id, limit as i64], |row| {
            Ok(json!({
                "sequence": row.get::<_, i64>(0)?,
                "record_kind": row.get::<_, String>(1)?,
                "created_at": row.get::<_, String>(2)?,
                "payload_json": row.get::<_, String>(3)?
            }))
        })
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_QUERY_FAILED", &ledger, e))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| sqlite_err("CCREALITY_LEDGER_ROW_READ_FAILED", &ledger, e))?;
    let rows = rows
        .into_iter()
        .map(|mut row| {
            let payload_json =
                row.get("payload_json")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                        CCRealityError::new(
                            "CCREALITY_LEDGER_PAYLOAD_MISSING",
                            "ledger row did not contain payload_json",
                            "ledger.payload_json",
                            "inspect the ledger schema and row projection",
                            json!({"ledger": ledger.display().to_string(), "row": row}),
                            Some(format!("sqlite:{}#ledger_records", ledger.display())),
                        )
                    })?;
            let payload = serde_json::from_str::<Value>(payload_json).map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_LEDGER_PAYLOAD_JSON_INVALID",
                    format!("ledger payload_json is not valid JSON: {e}"),
                    "ledger.payload_json",
                    "repair the ledger row or rebuild the attempt from source artifacts",
                    json!({
                        "ledger": ledger.display().to_string(),
                        "sequence": row.get("sequence").cloned().unwrap_or(Value::Null)
                    }),
                    Some(format!("sqlite:{}#ledger_records", ledger.display())),
                )
            })?;
            if let Value::Object(map) = &mut row {
                map.insert("payload".to_string(), payload);
            }
            Ok(row)
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(json!({
        "rows": rows,
        "row_count": rows.len(),
        "run_id": run_id,
        "ledger_path": ledger.display().to_string(),
        "source_of_truth": format!("sqlite:{}#ledger_records", ledger.display())
    }))
}

fn validate_ledger_where(where_clause: &str) -> Result<String> {
    static WHERE_RE: OnceLock<Regex> = OnceLock::new();
    let re = WHERE_RE.get_or_init(|| {
        let atom = r#"(?:record_kind|created_at|payload_json)\s*(?:<=|>=|<>|!=|=|<|>|LIKE)\s*(?:'(?:[^']|'')*'|[0-9]+)"#;
        Regex::new(&format!(r#"(?i)^\s*{atom}(?:\s+AND\s+{atom})*\s*$"#))
            .expect("ledger WHERE validator regex compiles")
    });
    let trimmed = where_clause.trim();
    if trimmed.is_empty() || !re.is_match(trimmed) {
        return Err(CCRealityError::new(
            "CCREALITY_LEDGER_QUERY_REJECTED",
            "WHERE clause is outside the read-only ledger query contract",
            "reality_query_ledger.where",
            "use AND-joined comparisons on record_kind, created_at, or payload_json with quoted string or numeric literals",
            json!({"where": where_clause}),
            None,
        ));
    }
    Ok(trimmed.to_string())
}

/// Locate the reality-loop CLI binary. Prefers release over debug so that the
/// MCP server picks up production rebuilds without needing path-specific config.
/// Operator override via `CGREALITY_REALITY_LOOP_BIN` always wins.
///
/// Made `pub(super)` so the harness-write tools in `alter.rs` can shell out
/// through the same binary-resolution path as `interact.rs` engine spawns.
pub(super) fn locate_engine_binary() -> Result<PathBuf> {
    if let Ok(env_bin) = std::env::var("CGREALITY_REALITY_LOOP_BIN") {
        let bin = PathBuf::from(env_bin);
        if !bin.is_file() {
            return Err(CCRealityError::new(
                "CCREALITY_ENGINE_BINARY_MISSING",
                "CGREALITY_REALITY_LOOP_BIN points to a missing file",
                "engine.binary",
                "fix or unset CGREALITY_REALITY_LOOP_BIN",
                json!({"path": bin.display().to_string()}),
                Some(file_sot(&bin)),
            ));
        }
        return Ok(bin);
    }
    let root = project_root()?;
    let release = root.join("target/release/reality-loop");
    if release.is_file() {
        return Ok(release);
    }
    let debug = root.join("target/debug/reality-loop");
    if debug.is_file() {
        return Ok(debug);
    }
    Err(CCRealityError::new(
        "CCREALITY_ENGINE_BINARY_MISSING",
        "neither target/release/reality-loop nor target/debug/reality-loop exists",
        "engine.binary",
        "run cargo build --release -p context-graph-cli --bin reality-loop",
        json!({
            "release_path": release.display().to_string(),
            "debug_path": debug.display().to_string()
        }),
        Some(file_sot(&release)),
    ))
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

pub(super) fn find_ledger_for_run(
    root: &Path,
    run_id: &str,
    target_instance: Option<&str>,
) -> Result<PathBuf> {
    let direct = root.join("ledger.sqlite");
    if direct.is_file() {
        return Ok(direct);
    }
    let run_dir = root.join(run_id);
    if !run_dir.is_dir() {
        return Err(CCRealityError::new(
            "CCREALITY_LEDGER_RUN_DIR_MISSING",
            "run_id directory is missing under active runtime root",
            "ledger.run_id",
            "verify run_id with reality_latest_root and rerun the engine if artifacts are missing",
            json!({"runtime_root": root.display().to_string(), "run_id": run_id, "path": run_dir.display().to_string()}),
            Some(file_sot(&run_dir)),
        ));
    }
    let mut found = Vec::new();
    collect_ledgers(&run_dir, &mut found)?;
    found.sort();
    let candidates = match target_instance {
        Some(target) => {
            let target_filtered = found
                .iter()
                .filter(|path| path_has_component(path, target))
                .cloned()
                .collect::<Vec<_>>();
            if target_filtered.is_empty() && !found.is_empty() {
                return Err(CCRealityError::new(
                    "CCREALITY_LEDGER_TARGET_MISMATCH",
                    "no ledger under the requested run_id matches the active target instance",
                    "ledger.target_instance",
                    "set the active target instance to the run's target or inspect the run directory before querying",
                    json!({
                        "runtime_root": root.display().to_string(),
                        "run_id": run_id,
                        "target_instance": target,
                        "candidates": found.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
                    }),
                    Some(file_sot(&run_dir)),
                ));
            }
            target_filtered
        }
        None => found,
    };
    match candidates.as_slice() {
        [ledger] => Ok(ledger.clone()),
        [] => Err(CCRealityError::new(
            "CCREALITY_LEDGER_MISSING",
            "no ledger.sqlite or blackbox.sqlite was found under requested run_id",
            "ledger.path",
            "run an attempt that initializes the BlackBox ledger for this run_id",
            json!({"runtime_root": root.display().to_string(), "run_id": run_id, "target_instance": target_instance}),
            Some(file_sot(&run_dir)),
        )),
        many => Err(CCRealityError::new(
            "CCREALITY_LEDGER_AMBIGUOUS",
            "multiple ledgers match the requested run_id",
            "ledger.path",
            "narrow the active target instance or inspect the run directory before querying",
            json!({
                "runtime_root": root.display().to_string(),
                "run_id": run_id,
                "target_instance": target_instance,
                "candidates": many.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()
            }),
            Some(file_sot(&run_dir)),
        )),
    }
}

fn path_has_component(path: &Path, expected: &str) -> bool {
    path.components()
        .any(|component| component.as_os_str().to_str() == Some(expected))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_where_validator_accepts_allowlisted_comparisons() {
        assert!(validate_ledger_where("record_kind = 'agent_run'").is_ok());
        assert!(validate_ledger_where(
            "record_kind = 'tool_call' AND payload_json LIKE '%optimizer%'"
        )
        .is_ok());
        assert!(validate_ledger_where("created_at >= 1700000000").is_ok());
    }

    #[test]
    fn ledger_where_validator_rejects_sql_control_syntax() {
        for clause in [
            "1=1",
            "record_kind = 'agent_run' OR 1=1",
            "record_kind = 'agent_run'; DROP TABLE ledger_records",
            "record_kind IN ('agent_run')",
            "json_extract(payload_json, '$.status') = 'ok'",
            "record_kind = 'agent_run' --",
        ] {
            assert!(
                validate_ledger_where(clause).is_err(),
                "{clause} should be rejected"
            );
        }
    }

    #[test]
    fn ledger_locator_scopes_to_run_id_and_target_component() {
        let root = tempfile::tempdir().expect("runtime root");
        let run_a_ledger = root
            .path()
            .join("run-a/tasks/target-a/model/blackbox.sqlite");
        let run_b_ledger = root
            .path()
            .join("run-b/tasks/target-b/model/blackbox.sqlite");
        std::fs::create_dir_all(run_a_ledger.parent().expect("parent")).expect("run a parent");
        std::fs::create_dir_all(run_b_ledger.parent().expect("parent")).expect("run b parent");
        std::fs::write(&run_a_ledger, b"sqlite-a").expect("run a ledger");
        std::fs::write(&run_b_ledger, b"sqlite-b").expect("run b ledger");

        let resolved =
            find_ledger_for_run(root.path(), "run-b", Some("target-b")).expect("run b ledger");
        assert_eq!(resolved, run_b_ledger);
    }

    #[test]
    fn ledger_locator_rejects_ambiguous_run_ledgers() {
        let root = tempfile::tempdir().expect("runtime root");
        let ledger_a = root.path().join("run-a/tasks/target-a/blackbox.sqlite");
        let ledger_b = root.path().join("run-a/tasks/target-b/blackbox.sqlite");
        std::fs::create_dir_all(ledger_a.parent().expect("parent")).expect("target a parent");
        std::fs::create_dir_all(ledger_b.parent().expect("parent")).expect("target b parent");
        std::fs::write(&ledger_a, b"sqlite-a").expect("target a ledger");
        std::fs::write(&ledger_b, b"sqlite-b").expect("target b ledger");

        let err = find_ledger_for_run(root.path(), "run-a", None).expect_err("ambiguous");
        assert_eq!(err.error_code, "CCREALITY_LEDGER_AMBIGUOUS");

        let resolved =
            find_ledger_for_run(root.path(), "run-a", Some("target-a")).expect("target filter");
        assert_eq!(resolved, ledger_a);
    }

    #[test]
    fn ledger_locator_rejects_active_target_mismatch() {
        let root = tempfile::tempdir().expect("runtime root");
        let ledger = root.path().join("run-a/tasks/target-a/blackbox.sqlite");
        std::fs::create_dir_all(ledger.parent().expect("parent")).expect("target parent");
        std::fs::write(&ledger, b"sqlite-a").expect("ledger");

        let err = find_ledger_for_run(root.path(), "run-a", Some("target-b"))
            .expect_err("target mismatch");
        assert_eq!(err.error_code, "CCREALITY_LEDGER_TARGET_MISMATCH");
    }
}
