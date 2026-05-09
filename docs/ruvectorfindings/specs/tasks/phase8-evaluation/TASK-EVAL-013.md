# TASK-EVAL-013 — `mcp__cgreality__mejepa_eval_run` + `mcp__cgreality__mejepa_eval_ship_gate_status` MCP tool handlers

```xml
<task_spec id="TASK-EVAL-013" version="1.0">

<metadata>
  <title>Implement `mcp__cgreality__mejepa_eval_run` + `mcp__cgreality__mejepa_eval_ship_gate_status` MCP handlers in `crates/context-graph-mcp/src/handlers/tools/mejepa_eval.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>13</sequence>
  <implements>
    <item ref="REQ-EVAL-08">MCP tool surface (2 of 4 tools — eval_run + ship_gate_status; the other 2 are TASK-EVAL-014)</item>
    <item ref="component:mcp__cgreality__mejepa_eval_run">Long-running tool; returns `report_path: file:...` for retrieval</item>
    <item ref="component:mcp__cgreality__mejepa_eval_ship_gate_status">Read-only; returns ShipGateStatus from latest persisted EvalReport</item>
    <item ref="constitution#mcp_tool_namespace_lock">All tool names `mcp__cgreality__mejepa_*`</item>
    <item ref="BR-EVAL-01">Eval run callable only via MCP, no direct in-process FFI from external callers</item>
    <item ref="BR-EVAL-06">ship_gate_status with no persisted report returns `passed: false, blockers: ["MEJEPA_EVAL_NO_REPORT_AVAILABLE"]`</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-008</task_ref><!-- check_ship_gate ready -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (mejepa_eval.rs in MCP crate); 1 modified (handlers/tools/mod.rs to register)</estimated_files>
</metadata>

<context>
This task lands the **operator-facing MCP surface** for evaluation. Two thin wrappers around `EvalRunner` calls; both `additionalProperties: false` per audit-9 lesson; both validate inputs at the MCP boundary (path canonicalization, type ranges, no control characters); both return SCREAMING_SNAKE-tagged errors mapping `EvalError.code()` from TASK-EVAL-002.

`mcp__cgreality__mejepa_eval_run` is **long-running** — a full holdout eval may take up to 30 min per NFR-EVAL-02; the MCP response includes `report_path: file:&lt;data/mejepa-eval/mejepa-weekly-report-YYYY-MM-DD.json&gt;` so the caller can retrieve the report from disk after the long call returns. Per CLAUDE.md §11.5 reality-shift principle, the path is bytes-on-disk reality the agent re-reads.

`mcp__cgreality__mejepa_eval_ship_gate_status` is **read-only and fast** (&lt; 100 ms) — reads the latest persisted `EvalReport` from CF_MEJEPA_EVAL_REPORTS via prefix scan, deserializes, runs `check_ship_gate` over a clone, returns `ShipGateStatus`. No new eval is triggered. When no report has been persisted yet (Phase 8 just started), returns `passed: false, blockers: ["MEJEPA_EVAL_NO_REPORT_AVAILABLE"]` per BR-EVAL-06.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (mcp_tool_namespace_lock; mcp_input_validation rules: additionalProperties:false, path canonicalization, no control chars in operator_id)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-08; AC-EVAL-06-* MCP scenarios; BR-EVAL-01; BR-EVAL-06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (endpoint mejepa_eval_run; endpoint mejepa_eval_ship_gate_status; long-running tool pattern)</file>
  <file purpose="prior_task">TASK-EVAL-008 (check_ship_gate)</file>
  <file purpose="cross_domain_pattern">crates/context-graph-mcp/src/handlers/tools/ (existing MCP tool handler patterns)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-008 shipped: `check_ship_gate` callable</check>
  <check>`crates/context-graph-mcp` has a tool registration mechanism (`MCP_TOOLS_REGISTRY` or similar); existing tools like `mcp__cgreality__optimizer_record_recommendation` follow a documented pattern</check>
  <check>`schemars` workspace dep available for JSON schema generation</check>
  <check>`tokio` async runtime available (eval_run is long-running; spawned on a separate Tokio task with structured cancellation)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mcp/src/handlers/tools/mejepa_eval.rs` with:
      - `pub async fn handle_mejepa_eval_run(req: MejepaEvalRunReq, ctx: &McpContext) -> Result<MejepaEvalRunResp, McpError>`:
        1. Validate inputs:
           - `window_days: u32` in `[1, 30]`; reject otherwise with `MEJEPA_EVAL_INVALID_WINDOW_DAYS`
           - `include_per_cell: bool` (no validation needed)
           - `output_path: Option<PathBuf>` — when Some, canonicalize via `Path::canonicalize`; reject `..` traversal; reject paths outside repo root; default to `./data/mejepa-eval/`
           - `additionalProperties: false` enforced via JSON schema (deny_unknown_fields on req struct)
        2. Borrow the singleton `EvalRunner` from `ctx` (constructed lazily on first call from RocksDB CFs + cached predictor weights)
        3. Spawn `eval_runner.run_holdout_eval(window_days, include_per_cell)` on a Tokio task; await with timeout 30 min (NFR-EVAL-02)
        4. On success: write the EvalReport JSON to `{output_path}/mejepa-weekly-report-{report.report_date}.json` with mode `0o600`; if persist fails, fall back to `/tmp/contextgraph-mejepa-evaluation-fsv/` per TECH-EVALUATION fallback pattern
        5. Return `MejepaEvalRunResp { report_path: format!("file:{}", canonical_path), report_date: report.report_date, ship_gate_passed: report.ship_gate_passed, blockers: report.ship_gate_failures.clone(), wall_clock_seconds: report.wall_clock_seconds }`
        6. On `EvalError`: map to `McpError { code: e.code().to_string(), message: e.to_string() }`; preserve structured payload via `data: serde_json::to_value(&e).ok()`
      - `pub async fn handle_mejepa_eval_ship_gate_status(req: MejepaEvalShipGateStatusReq, ctx: &McpContext) -> Result<ShipGateStatus, McpError>`:
        1. No request fields beyond an optional `report_date: Option<NaiveDate>` for time-travel queries; default to "latest"
        2. Validate `report_date` is sane: NOT `&lt; 2026-05-09`, NOT `&gt; today + 1 day`
        3. Read latest report from CF_MEJEPA_EVAL_REPORTS via `load_prior_report(&storage, report_date.unwrap_or(chrono::Utc::now().date_naive()))`
        4. If `None`: return `Ok(ShipGateStatus { passed: false, per_category: BTreeMap::new(), per_language: BTreeMap::new(), blockers: vec!["MEJEPA_EVAL_NO_REPORT_AVAILABLE".into()], report_date: chrono::Utc::now().date_naive(), overall_correlation: 0.0 })` per BR-EVAL-06
        5. Else: run `check_ship_gate(&report, prior_report_2_weeks_ago.as_ref(), &EvalConfig::default().thresholds)` and return the result
        6. On RocksDB error: map to `McpError { code: "MEJEPA_EVAL_REPORT_PERSIST_FAIL", message: ... }`
      - **Request/response types** with `#[serde(deny_unknown_fields)]` and `#[derive(JsonSchema)]`:
        - `MejepaEvalRunReq { window_days: u32, include_per_cell: bool, output_path: Option<PathBuf> }`
        - `MejepaEvalRunResp { report_path: String, report_date: NaiveDate, ship_gate_passed: bool, blockers: Vec<String>, wall_clock_seconds: f64 }`
        - `MejepaEvalShipGateStatusReq { report_date: Option<NaiveDate> }`
        - `ShipGateStatus` is re-exported from TASK-EVAL-001
      - **Tool registration**: register both in the MCP server's tool manifest with full JSON schema + tool description per existing pattern (search `crates/context-graph-mcp/src/handlers/tools/mod.rs` for the `register_tool!` macro or equivalent)
    - Update `crates/context-graph-mcp/src/handlers/tools/mod.rs`:
      - Add `pub mod mejepa_eval;`
      - Register the two tools in the registry constructor
    - Add unit + integration tests in `mejepa_eval.rs` `#[cfg(test)] mod tests`:
      - `eval_run_rejects_window_days_zero`
      - `eval_run_rejects_window_days_above_30`
      - `eval_run_rejects_path_traversal_in_output_path` (`../../etc/passwd`)
      - `eval_run_rejects_path_outside_repo_root`
      - `eval_run_with_test_runner_returns_report_path` (mocked EvalRunner returning fixed report; assert response shape)
      - `eval_run_long_running_path_writes_report_to_disk_with_0o600_mode` (Unix only)
      - `ship_gate_status_no_persisted_report_returns_no_report_available`
      - `ship_gate_status_with_persisted_report_returns_check_ship_gate_result`
      - `ship_gate_status_rejects_future_date_more_than_1_day_ahead`
      - `ship_gate_status_rejects_pre_phase8_date` (before 2026-05-09)
      - `additional_properties_in_request_rejected_via_serde_deny_unknown_fields` (synthesize `{"window_days": 7, "include_per_cell": true, "rogue_field": "x"}` → expect deserialize error)
      - `eval_error_code_mapped_to_mcp_error_code_round_trip`
  </in_scope>
  <out_of_scope>
    - `mcp__cgreality__mejepa_active_learning_queue` and `mcp__cgreality__mejepa_active_learning_label` (TASK-EVAL-014)
    - Weekly report writer logic (TASK-EVAL-015 — this task only writes the JSON path; the weekly-report content+metadata orchestration is TASK-EVAL-015)
    - CLI subcommand (TASK-EVAL-016)
    - FSV harness (TASK-EVAL-017)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa_eval.rs">
use std::path::PathBuf;
use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::handlers::McpContext;
use crate::error::McpError;
use context_graph_mejepa::eval::{ShipGateStatus, EvalReport};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaEvalRunReq {
    pub window_days: u32,
    pub include_per_cell: bool,
    #[serde(default)]
    pub output_path: Option&lt;PathBuf&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaEvalRunResp {
    pub report_path: String,
    pub report_date: NaiveDate,
    pub ship_gate_passed: bool,
    pub blockers: Vec&lt;String&gt;,
    pub wall_clock_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaEvalShipGateStatusReq {
    #[serde(default)]
    pub report_date: Option&lt;NaiveDate&gt;,
}

pub async fn handle_mejepa_eval_run(
    req: MejepaEvalRunReq,
    ctx: &amp;McpContext,
) -&gt; Result&lt;MejepaEvalRunResp, McpError&gt;;

pub async fn handle_mejepa_eval_ship_gate_status(
    req: MejepaEvalShipGateStatusReq,
    ctx: &amp;McpContext,
) -&gt; Result&lt;ShipGateStatus, McpError&gt;;
    </signature>
  </signatures>
  <constraints>
    - `additionalProperties: false` enforced via `#[serde(deny_unknown_fields)]` on every request struct (audit-9 lesson)
    - All path inputs canonicalized via `std::fs::canonicalize` after `..` rejection
    - All paths must be under repo root (computed from `ctx.workspace_root`); reject with `MEJEPA_VERIFY_PATCH_PATH_INVALID`-style error
    - `window_days` strictly in `[1, 30]`
    - `report_date` strictly in `[2026-05-09, today + 1 day]`
    - Errors mapped via `EvalError.code()` → MCP error code (preserves SCREAMING_SNAKE namespace)
    - Long-running `eval_run` MUST set a 30-min timeout via `tokio::time::timeout`; on timeout, cancel the inner task and return `MEJEPA_EVAL_TIMEOUT`
    - File mode 0o600 on the persisted report (Unix only — graceful fallback on non-Unix with stderr warning)
    - Tool name + description in the MCP manifest match: `mcp__cgreality__mejepa_eval_run` description "Trigger a holdout evaluation run; returns report_path for retrieval"; `mcp__cgreality__mejepa_eval_ship_gate_status` description "Return current ship-gate pass/fail without triggering a new eval"
    - File length budget: &lt; 500 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean for both crates
    - ≥ 12 passing tests
    - Tool registration discoverable via JSON-RPC `tools/list`
    - eval_run timeout enforced
    - ship_gate_status returns correct response on no-prior-report path
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mcp/src/handlers/tools/mejepa_eval.rs:
  pub async fn handle_mejepa_eval_run(req, ctx) -&gt; Result&lt;MejepaEvalRunResp, McpError&gt; {
      // 1. validate
      if !(1..=30).contains(&req.window_days) {
          return Err(McpError::new("MEJEPA_EVAL_INVALID_WINDOW_DAYS",
              format!("window_days must be in [1, 30]; got {}", req.window_days)));
      }
      let output = match req.output_path.as_ref() {
          Some(p) =&gt; canonicalize_under_root(p, &ctx.workspace_root)?,
          None =&gt; ctx.workspace_root.join("data/mejepa-eval/"),
      };

      // 2. delegate
      let runner = ctx.eval_runner_singleton().await?;
      let report = tokio::time::timeout(
          std::time::Duration::from_secs(30 * 60),
          tokio::task::spawn_blocking({
              let runner = runner.clone();
              let include = req.include_per_cell;
              move || runner.run_holdout_eval(req.window_days, include)
          })
      ).await
       .map_err(|_| McpError::new("MEJEPA_EVAL_TIMEOUT", "eval exceeded 30 min budget"))?
       .map_err(|e| McpError::new("MEJEPA_EVAL_JOIN_ERROR", e.to_string()))?
       .map_err(|e| McpError::new(e.code(), e.to_string()))?;

      // 3. persist
      let path = output.join(format!("mejepa-weekly-report-{}.json", report.report_date));
      let bytes = report.serialize_canonical().map_err(|e| McpError::new(e.code(), e.to_string()))?;
      write_with_mode_0o600(&path, &bytes).or_else(|_| {
          let fallback = std::path::Path::new("/tmp/contextgraph-mejepa-evaluation-fsv/").join(path.file_name().unwrap());
          write_with_mode_0o600(&fallback, &bytes)
      })?;

      Ok(MejepaEvalRunResp {
          report_path: format!("file:{}", path.display()),
          report_date: report.report_date,
          ship_gate_passed: report.ship_gate_passed,
          blockers: report.ship_gate_failures.clone(),
          wall_clock_seconds: report.wall_clock_seconds,
      })
  }

  pub async fn handle_mejepa_eval_ship_gate_status(req, ctx) -&gt; Result&lt;ShipGateStatus, McpError&gt; {
      let target_date = req.report_date.unwrap_or_else(|| chrono::Utc::now().date_naive());
      let phase8_start = NaiveDate::from_ymd_opt(2026, 5, 9).unwrap();
      if target_date &lt; phase8_start {
          return Err(McpError::new("MEJEPA_EVAL_REPORT_DATE_TOO_OLD",
              "report_date is before Phase 8 start (2026-05-09)"));
      }
      let tomorrow = chrono::Utc::now().date_naive() + chrono::Duration::days(1);
      if target_date &gt; tomorrow {
          return Err(McpError::new("MEJEPA_EVAL_REPORT_DATE_TOO_FAR",
              "report_date is more than 1 day in the future"));
      }
      let storage = ctx.rocksdb().await?;
      let prior = load_prior_report(&storage, target_date)
          .map_err(|e| McpError::new(e.code(), e.to_string()))?;
      let report = match prior {
          Some(r) =&gt; r,
          None =&gt; return Ok(ShipGateStatus {
              passed: false,
              per_category: Default::default(),
              per_language: Default::default(),
              blockers: vec!["MEJEPA_EVAL_NO_REPORT_AVAILABLE".into()],
              report_date: target_date,
              overall_correlation: 0.0,
          }),
      };
      let prior_2w = load_prior_report(&storage, target_date - chrono::Duration::days(7)).ok().flatten();
      Ok(check_ship_gate(&report, prior_2w.as_ref(), &EvalConfig::default().thresholds))
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa_eval.rs">2 MCP tool handlers + request/response structs + registration + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">declare `pub mod mejepa_eval;` + register both tools in the registry constructor</file>
  <file path="crates/context-graph-mcp/Cargo.toml">add `context-graph-mejepa = { path = "../context-graph-mejepa" }` if not already present; `schemars = { workspace = true }`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean for both context-graph-mcp and context-graph-mejepa</criterion>
  <criterion>JSON schema for MejepaEvalRunReq has additionalProperties: false</criterion>
  <criterion>window_days outside [1, 30] rejected with MEJEPA_EVAL_INVALID_WINDOW_DAYS</criterion>
  <criterion>output_path with `..` rejected</criterion>
  <criterion>output_path outside workspace root rejected</criterion>
  <criterion>eval_run produces file with 0o600 mode on Unix</criterion>
  <criterion>eval_run respects 30-min timeout via tokio::time::timeout</criterion>
  <criterion>ship_gate_status with no prior report returns NO_REPORT_AVAILABLE blocker</criterion>
  <criterion>ship_gate_status rejects pre-Phase 8 dates and futures &gt; 1 day</criterion>
  <criterion>EvalError.code() round-trips into McpError.code</criterion>
  <criterion>Tool registry returns both tools via JSON-RPC tools/list</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp handlers::tools::mejepa_eval</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```
