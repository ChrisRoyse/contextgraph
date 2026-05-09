# TASK-EMBED-023 — MCP tools: mejepa_load_learner_state + mejepa_routing_lookup + mejepa_vram_budget_report

```xml
<task_spec id="TASK-EMBED-023" version="1.0">
<metadata>
  <title>Three MCP tools wiring Ensemble + RoutingTable + VramBudget into the cgreality MCP server</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>23</sequence>
  <implements>
    <item ref="REQ-EMBED-11">3 MCP tools under mcp__cgreality__mejepa_*</item>
    <item ref="EC-EMBED-09">file_sha not in corpus → on-demand compute (panel tool — but panel materialization itself is INSTRUMENTS domain)</item>
    <item ref="TECH-EMBEDDERS::api_contracts">3 endpoints</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EMBED-006</task_ref>
    <task_ref>TASK-EMBED-007</task_ref>
    <task_ref>TASK-EMBED-009</task_ref>
    <task_ref>TASK-EMBED-019</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>3 files</estimated_files>
</metadata>

<context>
Surface layer. Wires the EMBEDDERS domain into the running cgreality MCP server (`crates/context-graph-mcp`). Ships 3 typed JSON-RPC tools under the `mcp__cgreality__mejepa_*` namespace lock. The `mejepa_compute_panel` MCP tool referenced in TECH-EMBEDDERS api_contracts is NOT shipped here because panel materialization is owned by INSTRUMENTS domain (downstream Phase 1c task). The `mejepa_audit_pairwise_mi` MCP tool ships in a later phase (out of Phase 1b scope).
</context>

<input_context_files>
  <file purpose="mcp_tool_pattern">crates/context-graph-mcp/src/handlers/tools/</file>
  <file purpose="mcp_tool_namespace_lock">docs/ruvectorfindings/specs/constitution.md (mcp_tool_namespace_lock)</file>
  <file purpose="api_contracts">docs/ruvectorfindings/specs/technical/TECH-EMBEDDERS.md#api_contracts</file>
  <file purpose="ensemble">crates/context-graph-mejepa-embedders/src/ensemble.rs</file>
  <file purpose="routing">crates/context-graph-mejepa-embedders/src/routing.rs</file>
  <file purpose="vram_budget">crates/context-graph-mejepa-embedders/src/vram_budget.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-EMBED-006, 007, 009, 019 complete</check>
  <check>`context-graph-mcp` crate path-deps `context-graph-mejepa-embedders` (added in this task if absent)</check>
  <check>Ensemble singleton or dependency-injection pattern available in MCP server</check>
</prerequisites>

<scope>
  <in_scope>
    - `mcp__cgreality__mejepa_load_learner_state` MCP tool:
      - Request: `{ slot: EmbedderId }` (must be E15..E21)
      - Response 200: `{ loaded: true, embedder_id, vram_resident_bytes, vram_remaining_bytes, weight_sha, version }`
      - Response 412: `MEJEPA_EMBED_VRAM_BUDGET_EXCEEDED { needed_bytes, available_bytes, currently_loaded }`
      - Response 500: `MEJEPA_EMBED_DIGEST_MISMATCH { expected_sha, actual_sha, file_path }`
      - Response 400: `MEJEPA_EMBED_INVALID_INPUT { reason }` (slot not in E15..E21)
    - `mcp__cgreality__mejepa_routing_lookup` MCP tool:
      - Request: `{ entity_type: serde-tagged EntityTypeKey, language: Option<Language> }`
      - Response 200: `{ kind: "embedders" | "handled_by_instrument", embedders?: Vec<EmbedderId>, instrument_slot?: InstrumentSlot }`
      - Response 404: `MEJEPA_EMBED_ROUTING_MISSING { entity_type, language, chunk_sha }`
    - `mcp__cgreality__mejepa_vram_budget_report` MCP tool:
      - Request: `{}` (no args)
      - Response 200: `VramBudgetReport` JSON
    - JSON schemas for each tool (additionalProperties: false; explicit field types)
    - Tool registration in `context-graph-mcp/src/handlers/tools/mejepa_embedders.rs`
    - Wire-up in tools dispatcher / tool-list response so `tools/list` includes all 3
    - Tests:
      - load_learner_state(E17) happy path → loaded=true; subsequent call idempotent
      - load_learner_state(E1) → MEJEPA_EMBED_INVALID_INPUT (E1 is content, not learner)
      - routing_lookup(FunctionBody, None) → ["E7", "E14"]
      - routing_lookup(DiffHunk, None) → handled_by_instrument: "e_diff"
      - routing_lookup(Unknown, None) → MEJEPA_EMBED_ROUTING_MISSING
      - vram_budget_report after load_content_set returns sane JSON (resident_total_bytes > 0; per_embedder_bytes populated)
  </in_scope>
  <out_of_scope>
    - mejepa_compute_panel MCP tool (INSTRUMENTS domain)
    - mejepa_audit_pairwise_mi MCP tool (later phase)
    - mejepa_lora_register / refresh / merge MCP tools (Phase 5 self-healing)
    - mejepa_calibration_register MCP tool (Phase 7)
    - Live MCP integration test against running server (CI uses in-process handler invocation)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa_embedders.rs">
      use std::sync::Arc;
      use serde::{Deserialize, Serialize};
      use serde_json::Value;

      use context_graph_mejepa_embedders::{
          EmbedderId,
          ensemble::Ensemble,
          routing::{route_for_entity_type, EntityTypeKey},
          types::RoutingResult,
          registration::VramBudgetReport,
      };
      use context_graph_core::ast_chunker::Language;

      use crate::error::{McpError, McpResult};

      // ---- mejepa_load_learner_state ----
      #[derive(Debug, Deserialize)]
      pub struct LoadLearnerStateRequest { pub slot: EmbedderId }

      #[derive(Debug, Serialize)]
      pub struct LoadLearnerStateResponse {
          pub loaded: bool,
          pub embedder_id: EmbedderId,
          pub vram_resident_bytes: u64,
          pub vram_remaining_bytes: u64,
          pub weight_sha: String,
          pub version: String,
      }

      pub async fn handle_load_learner_state(
          ensemble: Arc&lt;Ensemble&gt;,
          req: LoadLearnerStateRequest,
      ) -&gt; McpResult&lt;LoadLearnerStateResponse&gt;;

      // ---- mejepa_routing_lookup ----
      #[derive(Debug, Deserialize)]
      pub struct RoutingLookupRequest {
          pub entity_type: EntityTypeKey,
          pub language: Option&lt;Language&gt;,
      }

      #[derive(Debug, Serialize)]
      #[serde(tag = "kind", rename_all = "snake_case")]
      pub enum RoutingLookupResponse {
          Embedders { embedders: Vec&lt;EmbedderId&gt; },
          HandledByInstrument { instrument_slot: String },
      }

      pub async fn handle_routing_lookup(
          req: RoutingLookupRequest,
      ) -&gt; McpResult&lt;RoutingLookupResponse&gt;;

      // ---- mejepa_vram_budget_report ----
      pub async fn handle_vram_budget_report(
          ensemble: Arc&lt;Ensemble&gt;,
      ) -&gt; McpResult&lt;VramBudgetReport&gt;;

      pub fn register_tools(registry: &amp;mut crate::handlers::tools::Registry, ensemble: Arc&lt;Ensemble&gt;);
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa_embedders_schema.rs">
      // JSON Schemas for the 3 tools per MCP `tools/list` response. additionalProperties: false on every schema.
      pub fn load_learner_state_schema() -&gt; serde_json::Value;
      pub fn routing_lookup_schema() -&gt; serde_json::Value;
      pub fn vram_budget_report_schema() -&gt; serde_json::Value;
    </signature>
  </signatures>

  <constraints>
    - All 3 tools under `mcp__cgreality__mejepa_*` namespace lock per constitution `<mcp_tool_namespace_lock>`.
    - JSON schemas have `additionalProperties: false` (per MEMORY.md lesson `additionalProperties:false blocks unlisted params`).
    - Error responses include SCREAMING_SNAKE error code in the `error.code` MCP field; structured fields per failure mode.
    - HTTP status codes from `<api_contracts>`: 400 invalid input, 404 routing missing, 412 VRAM exceeded, 500 digest mismatch.
    - load_learner_state validates slot ∈ E15..E21 BEFORE calling Ensemble::load_learner_state.
    - vram_budget_report sources from `Ensemble::vram_report()` snapshot (no live cuMemGetInfo if Ensemble caches it).
    - `register_tools` wires handlers into the `Registry` used by `handlers/tools/mod.rs` dispatcher.
    - tracing instrumentation per call.
  </constraints>

  <verification>
    - load_learner_state(E17) returns 200 with loaded=true
    - load_learner_state(E1) returns 400 INVALID_INPUT
    - routing_lookup(FunctionBody) returns embedders=[E7, E14]
    - routing_lookup(DiffHunk) returns instrument_slot=e_diff
    - routing_lookup(Unknown) returns 404 ROUTING_MISSING
    - vram_budget_report returns valid VramBudgetReport JSON
    - 3 tools appear in tools/list with correct schemas
    - cargo test in context-graph-mcp passes
    - cargo clippy -- -D warnings clean
  </verification>
</definition_of_done>

<pseudo_code>
mejepa_embedders.rs:
  - 3 handler async fns delegating to context-graph-mejepa-embedders APIs.
  - Error mapping: EmbedError::DigestMismatch → McpError::InternalServerError(500, "MEJEPA_EMBED_DIGEST_MISMATCH", details); etc.
  - register_tools registers each under its namespaced name with the corresponding schema.

mejepa_embedders_schema.rs:
  - serde_json::json! macros producing schemas with additionalProperties:false.
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa_embedders.rs">3 handlers + register_tools</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa_embedders_schema.rs">JSON schemas for the 3 tools</file>
  <file path="crates/context-graph-mcp/tests/mejepa_embedders_integration.rs">In-process handler invocation tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">Add `pub mod mejepa_embedders;` + register_tools call from server bootstrap</file>
  <file path="crates/context-graph-mcp/Cargo.toml">Add path-dep `context-graph-mejepa-embedders = { path = "../context-graph-mejepa-embedders" }` (if absent)</file>
  <file path="crates/context-graph-mcp/src/main.rs (or bootstrap)">Wire Arc&lt;Ensemble&gt; through handler registration on startup</file>
</files_to_modify>

<validation_criteria>
  <criterion>3 tools appear in tools/list with additionalProperties:false schemas</criterion>
  <criterion>load_learner_state happy path + invalid slot covered</criterion>
  <criterion>routing_lookup happy + handled_by_instrument + missing covered</criterion>
  <criterion>vram_budget_report returns valid JSON</criterion>
  <criterion>Error codes are SCREAMING_SNAKE per failure taxonomy</criterion>
  <criterion>cargo test in context-graph-mcp passes</criterion>
  <criterion>cargo clippy -- -D warnings clean for both crates</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mcp mejepa_embedders</command>
  <command>cargo build --release -p context-graph-mcp -p context-graph-mejepa-embedders</command>
  <command>cargo clippy -p context-graph-mcp -p context-graph-mejepa-embedders --all-features -- -D warnings</command>
</test_commands>

</task_spec>
```
