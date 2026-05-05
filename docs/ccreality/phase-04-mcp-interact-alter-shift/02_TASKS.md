# Phase 4 — Tasks

Twenty ordered tasks. Group ordering: foundation (4.1–4.4) → ALTER (4.5–4.10) → INTERACT (4.11–4.16) → optimizer (4.17–4.18) → verify (4.19–4.20).

## Foundation

### Task 4.1 — Add dependencies and create new module files

```bash
cd /home/cabdru/contextgraph
# Edit crates/context-graph-mcp/Cargo.toml to add jsonschema, fs2, uuid (see 01_PRECONDITIONS.md)
# Verify the workspace still builds
cargo build -p context-graph-mcp --quiet

# Create the new module files (empty for now)
cd crates/context-graph-mcp/src/handlers/tools/reality_loop
touch alter.rs interact.rs shift_log.rs optimizer.rs lock.rs schema_validator.rs
```

Update `mod.rs` to declare the new modules:
```rust
pub mod alter;
pub mod interact;
pub mod shift_log;
pub mod optimizer;
pub mod lock;
pub mod schema_validator;
```

Update `register()` to call `alter::register(server)`, `interact::register(server)`, `shift_log::register(server)`, `optimizer::register(server)` (as their bodies materialize).

### Task 4.2 — Implement `lock.rs` (single-writer enforcement)

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) §"lock.rs". Key API:
```rust
pub struct EditLock { ... }
impl EditLock {
    pub fn try_acquire(session_id: &str) -> Result<Self> { ... }
    pub fn current_holder() -> Option<(String, u64)> { ... }
}
impl Drop for EditLock {
    fn drop(&mut self) { /* unlink lock file */ }
}
```

Storage: `~/.cache/cgreality/edit_lock` containing `session_id:unix_ts`. Acquired with `O_EXCL` so concurrent acquisitions fail.

Test:
```bash
cargo test -p context-graph-mcp --lib reality_loop::lock --quiet
```

### Task 4.3 — Implement `shift_log.rs`

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) §"shift_log.rs". API:
```rust
pub struct ShiftRecord { /* fields per 13_STRUCTURED_OUTPUT_SCHEMA */ }
pub fn append_shift(runtime_root: &Path, session_id: &str, record: &ShiftRecord) -> Result<()>;
pub async fn reality_shift_log(args: Value) -> Result<Value>;       // tool
pub async fn reality_shift_compare_to_my_view(args: Value) -> Result<Value>;  // tool
pub fn register(server: &mut McpServer);
```

`append_shift` uses `OpenOptions::new().create(true).append(true)` then `write` then `sync_all` then `sync_data` on the parent dir.

Test:
```bash
cargo test -p context-graph-mcp --lib reality_loop::shift_log --quiet
```

### Task 4.4 — Implement `schema_validator.rs`

Per [03_FILE_SPECS.md](03_FILE_SPECS.md) §"schema_validator.rs". API:
```rust
pub struct RecommendationValidator { compiled: jsonschema::JSONSchema }
impl RecommendationValidator {
    pub fn load() -> Result<Self> { /* load config/ccreality_recommendation_schema.json, compile */ }
    pub fn validate(&self, value: &Value) -> Result<()> { /* return CCREALITY_RECOMMENDATION_SCHEMA_INVALID with full details */ }
}
```

Test with both a valid and invalid recommendation payload.

## ALTER family

### Task 4.5 — Implement `harness_open_window` and `harness_apply_line_window_edit`

These wrap `governed_edit::project_file_window` and `governed_edit::project_apply_line_window_edits` from Phase 1.

For `harness_apply_line_window_edit`:
1. Acquire `EditLock` with the session_id from the MCP context.
2. Call `validate_governed_path` from Phase 3's `path_policy.rs`.
3. Call `governed_edit::project_apply_line_window_edits`.
4. Compute `before_sha`/`after_sha`/`lines_added`/`lines_removed`/`git_diff_stat`.
5. Construct a `ShiftRecord` with all fields.
6. Call `shift_log::append_shift`.
7. Call `optimizer::record_harness_transition` (writes the harness-transitions/round-NN.json artifact).
8. Drop `EditLock`.
9. Return `{ status: "ok", path, before_sha256, after_sha256, lines_added, lines_removed, git_diff_path, harness_transition_path, shift_id, source_of_truth }`.

If any step fails after step 1, drop the lock and return error. No partial-success states.

### Task 4.6 — Implement `harness_run_command`

Wraps `governed_edit::run_governed_command` from Phase 1.

The allowlist is enforced inside `governed_edit::run_governed_command`. The MCP wrapper:
1. Validates command against the allowlist (re-checks defensively).
2. Runs.
3. Persists the output as a JSON artifact under `<runtime_root>/<run>/<task>/<attempt>/claude-code-optimizer/commands/<unix>-<safe_command_id>.json`.
4. Appends a shift record (subject: `{ type: "command", command_hash: sha256 }`).

### Task 4.7 — Implement `harness_git_diff` and `harness_git_status`

Lightweight wrappers. Don't append shift records (they're read-only).

### Task 4.8 — Implement `harness_verify_state`

Calls `governed_edit::verify_project_state` with the requested scope. Read-only; no shift record.

### Task 4.9 — Re-export memory mutators with namespace injection

For each of `store_memory`, `boost_importance`, `forget_concept`, `merge_concepts`:
1. Look up the existing handler in `crates/context-graph-mcp/src/handlers/tools/memory_tools.rs`.
2. Wrap it with a thin function that injects `namespace: "ccreality"` into the args before calling.
3. After the underlying call succeeds, append a shift record (subject: `{ type: "memory", uuid: ... }`).
4. Register under the same name (so Claude Code calling `mcp__cgreality__store_memory` is routed here, but a hypothetical `mcp__memory__store_memory` still goes to the original).

### Task 4.10 — Implement `optimizer_record_harness_transition`

Called by the PostToolUse Edit hook (Phase 6) AND internally by `harness_apply_line_window_edit`. Writes a `harness-transitions/round-NN.json` artifact.

Args:
```json
{ "run_id", "attempt", "tool_use_id", "file_path", "before_sha256", "after_sha256", "lines_added", "lines_removed", "cargo_check": { "exit_code", "stderr_tail" }, "git_diff_stat" }
```

Returns: `{ "transition_path": "file:...", "round": integer }`.

The round number is `max(existing) + 1` for the attempt's harness-transitions/ subdir.

Append a shift record only if called externally (not from `harness_apply_line_window_edit`, which already appends via 4.5).

## INTERACT family

### Task 4.11 — Implement `reality_run_smoke`

Spawns `target/debug/reality-loop smoke --runtime-root <runtime_root> --label <label>`.

```rust
pub async fn reality_run_smoke(args: Value) -> Result<Value> {
    let runtime_root = std::path::PathBuf::from(required_str(&args, "runtime_root")?);
    let label = required_str(&args, "label")?;
    let bin = locate_engine_binary()?;
    let output = tokio::process::Command::new(&bin)
        .args(["smoke", "--runtime-root", &runtime_root.display().to_string(), "--label", label])
        .output().await
        .map_err(|e| CCRealityError::new(
            "CCREALITY_INTERACT_SPAWN_FAILED",
            format!("failed to spawn engine: {e}"),
            "reality_run_smoke.spawn",
            "ensure target/debug/reality-loop is built",
            json!({"bin": bin.display().to_string()}),
            None,
        ))?;
    let stdout_text = std::str::from_utf8(&output.stdout).map_err(|e| CCRealityError::new(
        "CCREALITY_INTERACT_STDOUT_UTF8_INVALID",
        format!("engine stdout was not valid UTF-8: {e}"),
        "reality_run_smoke.stdout",
        "inspect engine stderr and fix the engine output",
        json!({"bin": bin.display().to_string()}),
        None,
    ))?;
    let stdout_tail: Value = serde_json::from_str(stdout_text).map_err(|e| CCRealityError::new(
        "CCREALITY_INTERACT_STDOUT_JSON_INVALID",
        format!("engine stdout was not JSON: {e}"),
        "reality_run_smoke.stdout",
        "inspect engine stderr and fix the engine output",
        json!({"stdout": stdout_text}),
        None,
    ))?;
    let stderr_tail = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(json!({
        "status": if output.status.success() { "success" } else { "failed" },
        "exit_code": output.status.code(),
        "stdout": stdout_tail,
        "stderr_tail": stderr_tail.chars().rev().take(4096).collect::<String>().chars().rev().collect::<String>(),
        "source_of_truth": stdout_tail.pointer("/source_of_truth").cloned()
    }))
}
```

Append a shift record (subject: `{ type: "engine_run", subcommand: "smoke" }`) on success.

### Task 4.12 — Implement `reality_run_attempt`

Same shape as 4.11 but for `attempt` subcommand. Args include `task_manifest`, `runtime_root`, `max_attempts`, `official_eval`, `model`. Default timeout 1800s.

### Task 4.13 — Implement `reality_run_official_eval` and `reality_run_targeted_test`

Both shell out to engine subcommands (or directly to the official Docker SDK runner if more direct).

### Task 4.14 — Implement `reality_recompile_reward`

Shells `reality-loop replay --attempt-summary-path ... --label recompile-reward`. The replay subcommand from Phase 2 already does the round-trip; if a true reward-recompute is needed, extend Phase 2's `run_replay` to optionally re-run the reward feature extraction over the loaded reality-signal.

### Task 4.15 — Implement `reality_query_ledger`

```rust
pub async fn reality_query_ledger(args: Value) -> Result<Value> {
    let run_id = required_str(&args, "run_id")?;
    let where_clause = args.get("where").and_then(Value::as_str).unwrap_or("1=1");
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(50).min(500);

    // Reject anything that isn't strictly a WHERE clause: no SELECT, INSERT, UPDATE, DELETE, DROP.
    let where_lower = where_clause.to_ascii_lowercase();
    for kw in &["select", "insert", "update", "delete", "drop", "alter", ";", "--"] {
        if where_lower.contains(kw) {
            return Err(CCRealityError::new(
                "CCREALITY_LEDGER_QUERY_REJECTED",
                format!("WHERE clause may not contain '{}'", kw),
                "reality_query_ledger.where",
                "use only column comparisons in the WHERE clause",
                json!({"where": where_clause}), None,
            ));
        }
    }

    let runtime_root = require_active_runtime_root().await?;
    let ledger = runtime_root.join("ledger.sqlite");
    let conn = rusqlite::Connection::open_with_flags(&ledger,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let sql = format!("SELECT record_kind, created_at, payload FROM ledger_records WHERE {} ORDER BY created_at DESC LIMIT {}",
                       where_clause, limit);
    let mut stmt = conn.prepare(&sql)?;
    let rows: Vec<Value> = stmt.query_map([], |row| {
        Ok(json!({
            "record_kind": row.get::<_, String>(0)?,
            "created_at": row.get::<_, i64>(1)?,
            "payload": row.get::<_, String>(2)?
        }))
    })?.filter_map(|r| r.ok()).collect();
    Ok(json!({
        "rows": rows,
        "row_count": rows.len(),
        "source_of_truth": format!("sqlite:{}", ledger.display())
    }))
}
```

### Task 4.16 — Register all INTERACT tools

Add to `interact::register(server)`. Verify via tools/list that all 6 INTERACT tools appear.

## Optimizer family

### Task 4.17 — Implement `optimizer_record_decision`

Args:
```json
{ "run_id": "...", "attempt": N, "policy": "...", "should_run": bool, "reasons": [...], "claude_session_id": "...", "claude_model": "..." }
```

Writes `<attempt-dir>/trigger-decision.json` AND `<attempt-dir>/claude-code-optimizer/decision-turn-NN.json`.

Append a shift record (subject: `{ type: "decision", policy: "...", should_run: ... }`).

Append to BlackBox ledger (`record_kind: "ccreality_optimizer_decision"`).

### Task 4.18 — Implement `optimizer_record_recommendation`

Validates input via `RecommendationValidator::validate` (Task 4.4). On failure, returns `CCREALITY_RECOMMENDATION_SCHEMA_INVALID` with the JSON Schema error path.

Writes `<attempt-dir>/claude-code-optimizer/recommendation-turn-NN.json`.

Appends shift record (subject: `{ type: "recommendation", status: "..." }`).

Appends to BlackBox ledger (`record_kind: "ccreality_optimizer_recommendation"`).

This is the artifact the Stop hook (Phase 6) waits for.

## Verify

### Task 4.19 — Build, lint, full unit test

```bash
cargo fmt -p context-graph-mcp
cargo build -p context-graph-mcp --quiet
cargo test -p context-graph-mcp --lib reality_loop --quiet
```

### Task 4.20 — Run full verification

[04_VERIFICATION.md](04_VERIFICATION.md). All 14 clauses pass.
