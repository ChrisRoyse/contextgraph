# TASK-EVAL-014 — `mcp__cgreality__mejepa_active_learning_queue` + `mcp__cgreality__mejepa_active_learning_label` MCP tool handlers

```xml
<task_spec id="TASK-EVAL-014" version="1.0">

<metadata>
  <title>Implement `mcp__cgreality__mejepa_active_learning_queue` + `mcp__cgreality__mejepa_active_learning_label` MCP handlers in `crates/context-graph-mcp/src/handlers/tools/mejepa_active_learning.rs`</title>
  <status>blocked</status>
  <layer>surface</layer>
  <sequence>14</sequence>
  <implements>
    <item ref="REQ-EVAL-08">MCP tool surface (2 of 4 tools — active_learning_queue + active_learning_label; first 2 are TASK-EVAL-013)</item>
    <item ref="REQ-EVAL-05">Operator labeling endpoint over MCP</item>
    <item ref="component:mcp__cgreality__mejepa_active_learning_queue">Top-N by signal_value descending; optional language filter</item>
    <item ref="component:mcp__cgreality__mejepa_active_learning_label">panel_sha existence check; oracle_outcome validation; operator_id non-empty</item>
    <item ref="constitution#mcp_tool_namespace_lock">All tool names `mcp__cgreality__mejepa_*`</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-009</task_ref><!-- enqueue_uncertain + apply_active_learning_label callable -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (mejepa_active_learning.rs in MCP crate); 1 modified (handlers/tools/mod.rs)</estimated_files>
</metadata>

<context>
This task lands the **operator-facing active-learning MCP surface** — the two endpoints the operator uses to (a) read the top-N most-uncertain panels awaiting label, (b) submit a label for a chosen panel. The handlers are thin wrappers around TASK-EVAL-009's `EvalRunner::apply_active_learning_label` and the queue's `top_n` read; both validate inputs at the MCP boundary; both return SCREAMING_SNAKE-tagged errors mapping `EvalError.code()` / `QueueError.code()`.

Per AC-EVAL-04-* the queue tool returns top-N entries sorted by `signal_value` descending. Per BR-EVAL-08 the label tool MUST validate `panel_sha` exists in the queue before accepting; otherwise return `MEJEPA_EVAL_PANEL_NOT_IN_QUEUE`. Per SEC-MEJEPA-01 `operator_id` MUST reject control characters and empty strings.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (mcp_tool_namespace_lock; mcp_input_validation rules)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-08; AC-EVAL-04-* active-learning scenarios; BR-EVAL-08 panel-existence check)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (endpoint mejepa_active_learning_queue; endpoint mejepa_active_learning_label)</file>
  <file purpose="prior_task">TASK-EVAL-009 (active learning impl); TASK-EVAL-013 (MCP handler pattern)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-009 shipped: `apply_active_learning_label` and `ActiveLearningQueueState::top_n` callable</check>
  <check>TASK-EVAL-013 shipped: McpContext + McpError + handler registration pattern established</check>
  <check>`schemars` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mcp/src/handlers/tools/mejepa_active_learning.rs` with:
      - `pub async fn handle_mejepa_active_learning_queue(req: MejepaActiveLearningQueueReq, ctx: &McpContext) -> Result<MejepaActiveLearningQueueResp, McpError>`:
        1. Validate `limit: u32` in `[1, 1_000]`; reject otherwise with `MEJEPA_EVAL_INVALID_LIMIT`
        2. Validate `language_filter: Option<Language>` is well-formed (deserializer rejects unknown values via `#[serde(deny_unknown_fields)]` + Language enum's `try_from`)
        3. Borrow the singleton `EvalRunner` from `ctx`
        4. Call `runner.active_learning_queue.lock().top_n(limit as usize, language_filter)` — read-only
        5. Return `MejepaActiveLearningQueueResp { entries: Vec<ActiveLearningQueueEntry>, queue_depth: usize, max_observed_outcome_set_len: usize }`
        6. On RocksDB error: map to `McpError { code: "MEJEPA_EVAL_REPORT_PERSIST_FAIL", ... }`
      - `pub async fn handle_mejepa_active_learning_label(req: MejepaActiveLearningLabelReq, ctx: &McpContext) -> Result<MejepaActiveLearningLabelResp, McpError>`:
        1. Validate `panel_sha` is exactly 32 bytes when hex-decoded; reject `MEJEPA_EVAL_INVALID_PANEL_SHA` otherwise
        2. Validate `operator_id` non-empty AND no byte `&lt; 0x20`; reject `MEJEPA_EVAL_INVALID_OPERATOR_ID` otherwise
        3. Validate `oracle_outcome` deserializes to a known `OracleOutcome` variant
        4. Validate `label_method` is one of {Manual, DockerRerun, AdjacentTestRerun}
        5. Borrow runner; call `runner.apply_active_learning_label(&panel_sha, &oracle_outcome, label_method, &operator_id, provenance, &storage, corpus_appender.as_ref())`
        6. On `Err(EvalError::PanelNotInQueue)`: map to `McpError { code: "MEJEPA_EVAL_PANEL_NOT_IN_QUEUE", ... }` (HTTP 404 equivalent per TECH-EVALUATION endpoint table)
        7. On other `EvalError`: map via `e.code()` round-trip
        8. Return `MejepaActiveLearningLabelResp { label_id: ulid_to_string, queue_depth_after: ..., corpus_size_after: ... }`
      - **Request/response types** with `#[serde(deny_unknown_fields)]` and `#[derive(JsonSchema)]`:
        - `MejepaActiveLearningQueueReq { limit: u32, language_filter: Option<Language> }`
        - `MejepaActiveLearningQueueResp { entries: Vec<ActiveLearningQueueEntry>, queue_depth: usize, max_observed_outcome_set_len: usize }`
        - `MejepaActiveLearningLabelReq { panel_sha: String, oracle_outcome: OracleOutcome, label_method: LabelMethod, operator_id: String }` (panel_sha is hex-encoded for JSON portability)
        - `MejepaActiveLearningLabelResp { label_id: String, queue_depth_after: usize, corpus_size_after: u64 }`
      - **Hex decoding helper**: `fn hex_to_panel_sha(s: &str) -> Result<[u8; 32], McpError>` — validates length 64, parses, returns array; reject on parse error.
      - **Provenance construction**: build `LabelProvenance` from `ctx` (corpus_sha from current corpus singleton, code_version from build-time env, calibration_version from inference singleton).
      - **Tool registration**: register both in the MCP server's tool manifest with full JSON schema.
    - Update `crates/context-graph-mcp/src/handlers/tools/mod.rs`:
      - Add `pub mod mejepa_active_learning;`
      - Register both tools
    - Add unit + integration tests in `mejepa_active_learning.rs` `#[cfg(test)] mod tests`:
      - `queue_rejects_limit_zero`
      - `queue_rejects_limit_above_1000`
      - `queue_returns_top_n_descending_by_signal_value`
      - `queue_with_language_filter_returns_only_matching_language`
      - `queue_with_no_filter_returns_all_languages`
      - `label_rejects_panel_sha_wrong_length`
      - `label_rejects_panel_sha_invalid_hex`
      - `label_rejects_empty_operator_id`
      - `label_rejects_operator_id_with_control_chars`
      - `label_panel_not_in_queue_returns_404_equivalent`
      - `label_succeeds_returns_ulid` (mock corpus appender returns Ok)
      - `label_corpus_appender_failure_returns_persist_fail` (rollback verified)
      - `additional_properties_in_request_rejected`
      - `eval_error_code_mapped_to_mcp_error_code`
  </in_scope>
  <out_of_scope>
    - The active-learning logic body (TASK-EVAL-009 owns)
    - Auto-route Docker rerun (TASK-EVAL-009 covers; this MCP surface does NOT expose auto-route — it's a CLI knob in TASK-EVAL-016)
    - Weekly report writer (TASK-EVAL-015)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa_active_learning.rs">
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::handlers::McpContext;
use crate::error::McpError;
use context_graph_mejepa::eval::{ActiveLearningQueueEntry, LabelMethod, LabelProvenance};
use context_graph_mejepa::types::{Language, OracleOutcome};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaActiveLearningQueueReq {
    pub limit: u32,
    #[serde(default)]
    pub language_filter: Option&lt;Language&gt;,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaActiveLearningQueueResp {
    pub entries: Vec&lt;ActiveLearningQueueEntry&gt;,
    pub queue_depth: usize,
    pub max_observed_outcome_set_len: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaActiveLearningLabelReq {
    /// Hex-encoded 32-byte SHA (64 hex chars)
    pub panel_sha: String,
    pub oracle_outcome: OracleOutcome,
    pub label_method: LabelMethod,
    pub operator_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MejepaActiveLearningLabelResp {
    pub label_id: String,
    pub queue_depth_after: usize,
    pub corpus_size_after: u64,
}

pub async fn handle_mejepa_active_learning_queue(
    req: MejepaActiveLearningQueueReq,
    ctx: &amp;McpContext,
) -&gt; Result&lt;MejepaActiveLearningQueueResp, McpError&gt;;

pub async fn handle_mejepa_active_learning_label(
    req: MejepaActiveLearningLabelReq,
    ctx: &amp;McpContext,
) -&gt; Result&lt;MejepaActiveLearningLabelResp, McpError&gt;;

fn hex_to_panel_sha(s: &amp;str) -&gt; Result&lt;[u8; 32], McpError&gt;;
    </signature>
  </signatures>
  <constraints>
    - `additionalProperties: false` enforced via `#[serde(deny_unknown_fields)]`
    - `panel_sha` field is hex-encoded for JSON (32 bytes → 64 hex chars); validate before passing to internal API
    - `operator_id` validation: non-empty + no byte `&lt; 0x20` (control chars) + length `&lt;= 256` per SEC-MEJEPA-01
    - `limit` strictly in `[1, 1_000]`
    - `language_filter` deserializes to known `Language` variants only; unknown values → deserialize error → 400 equivalent
    - Errors mapped via `EvalError.code()` / `QueueError.code()` → MCP error code
    - Tool name + description in MCP manifest match: `mcp__cgreality__mejepa_active_learning_queue` description "Read top-N most-uncertain panels awaiting label, sorted by signal_value descending"; `mcp__cgreality__mejepa_active_learning_label` description "Submit an oracle label for a queued panel"
    - File length budget: &lt; 500 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 14 passing tests
    - Both tools discoverable via JSON-RPC `tools/list`
    - panel_sha hex round-trips to internal `[u8; 32]`
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mcp/src/handlers/tools/mejepa_active_learning.rs:
  pub async fn handle_mejepa_active_learning_queue(req, ctx) -&gt; Result&lt;MejepaActiveLearningQueueResp, McpError&gt; {
      if !(1..=1_000).contains(&req.limit) {
          return Err(McpError::new("MEJEPA_EVAL_INVALID_LIMIT",
              format!("limit must be in [1, 1000]; got {}", req.limit)));
      }
      let runner = ctx.eval_runner_singleton().await?;
      let queue = runner.active_learning_queue.lock();
      let entries = queue.top_n(req.limit as usize, req.language_filter);
      Ok(MejepaActiveLearningQueueResp {
          entries,
          queue_depth: queue.len(),
          max_observed_outcome_set_len: queue.max_observed_outcome_set_len,
      })
  }

  pub async fn handle_mejepa_active_learning_label(req, ctx) -&gt; Result&lt;MejepaActiveLearningLabelResp, McpError&gt; {
      let panel_sha = hex_to_panel_sha(&req.panel_sha)?;
      if req.operator_id.is_empty() || req.operator_id.bytes().any(|b| b &lt; 0x20) {
          return Err(McpError::new("MEJEPA_EVAL_INVALID_OPERATOR_ID",
              "operator_id must be non-empty and contain no control chars"));
      }
      if req.operator_id.len() &gt; 256 {
          return Err(McpError::new("MEJEPA_EVAL_INVALID_OPERATOR_ID",
              "operator_id length exceeds 256"));
      }
      let runner = ctx.eval_runner_singleton().await?;
      let storage = ctx.rocksdb().await?;
      let appender = ctx.corpus_appender();
      let provenance = LabelProvenance {
          corpus_sha: ctx.current_corpus_sha(),
          code_version: env!("CARGO_PKG_VERSION").to_string(),
          calibration_version: ctx.current_calibration_version().await?,
      };

      let mut queue = runner.active_learning_queue.lock();
      let ulid = apply_active_learning_label(
          &mut queue, &panel_sha, &req.oracle_outcome,
          req.label_method, &req.operator_id, provenance,
          &storage, appender.as_ref(),
      ).map_err(|e| McpError::new(e.code(), e.to_string()))?;

      let queue_depth_after = queue.len();
      drop(queue);
      let corpus_size_after = appender.current_count().map_err(|e| McpError::new("MEJEPA_EVAL_REPORT_PERSIST_FAIL", e))?;

      Ok(MejepaActiveLearningLabelResp {
          label_id: ulid.to_string(),
          queue_depth_after,
          corpus_size_after,
      })
  }

  fn hex_to_panel_sha(s: &str) -&gt; Result&lt;[u8; 32], McpError&gt; {
      if s.len() != 64 {
          return Err(McpError::new("MEJEPA_EVAL_INVALID_PANEL_SHA",
              format!("panel_sha must be exactly 64 hex chars; got {}", s.len())));
      }
      let mut out = [0u8; 32];
      for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
          let hex = std::str::from_utf8(chunk).map_err(|_| McpError::new("MEJEPA_EVAL_INVALID_PANEL_SHA", "non-utf8"))?;
          out[i] = u8::from_str_radix(hex, 16).map_err(|_| McpError::new("MEJEPA_EVAL_INVALID_PANEL_SHA", "invalid hex"))?;
      }
      Ok(out)
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa_active_learning.rs">2 MCP tool handlers + request/response structs + hex helper + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">declare `pub mod mejepa_active_learning;` + register both tools</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>limit outside [1, 1000] rejected</criterion>
  <criterion>queue returns top-N descending by signal_value</criterion>
  <criterion>queue language_filter restricts results</criterion>
  <criterion>panel_sha wrong length rejected</criterion>
  <criterion>panel_sha invalid hex rejected</criterion>
  <criterion>operator_id empty rejected</criterion>
  <criterion>operator_id with control chars rejected</criterion>
  <criterion>operator_id over 256 chars rejected</criterion>
  <criterion>panel-not-in-queue returns MEJEPA_EVAL_PANEL_NOT_IN_QUEUE error</criterion>
  <criterion>Successful label returns ulid_str + queue_depth_after + corpus_size_after</criterion>
  <criterion>Corpus appender failure rolls back and returns persist_fail</criterion>
  <criterion>additionalProperties: false enforced via deny_unknown_fields</criterion>
  <criterion>Both tools discoverable via tools/list</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp handlers::tools::mejepa_active_learning</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>

</task_spec>
```
