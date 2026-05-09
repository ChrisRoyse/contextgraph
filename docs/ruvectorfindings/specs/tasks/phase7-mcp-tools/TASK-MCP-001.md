<task_spec id="TASK-MCP-001" version="1.0">
<metadata>
  <title>Foundation — Register 5 mejepa_* tool stubs with the MCP handler dispatcher (handler tree skeleton)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>1</sequence>
  <implements>
    <item ref="REQ-SHIFT-07">Five new MCP tools under locked `mcp__cgreality__mejepa_*` namespace</item>
    <item ref="REQ-SHIFT-13">Retired surfaces fail-closed; new tools must NOT alias or shim retired tools</item>
    <item ref="constitution#mcp_tool_namespace_lock">Namespace lock: `mcp__cgreality__mejepa_*` only</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#api_contracts[mejepa_predict_latest|mejepa_observe_shift|mejepa_subscriber_status|mejepa_capture_audit|mejepa_verify]">handler_path skeleton</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#implementation_notes[mcp_integration]">tool registration + `additionalProperties: false` enforcement</item>
  </implements>
  <depends_on>
    <!-- Cross-phase: TASK-SHIFT-003 ships SubscriberError + retired-surface fail-closed contract -->
    <task_ref>TASK-SHIFT-003</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>6 files</estimated_files>
</metadata>

<context>
This is the skeleton task that creates the new `crates/context-graph-mcp/src/handlers/tools/mejepa/` module tree, registers 5 tool definitions in the existing `tools/list` registry, and stands up handler stubs returning `unimplemented!`-equivalent structured errors. Subsequent logic tasks (TASK-MCP-002..006) replace each stub body. By doing the full registration in Task 1 we guarantee the namespace is reserved and that no later task accidentally adds a sibling tool under a different prefix.
</context>

<input_context_files>
  <file purpose="namespace_lock">docs/ruvectorfindings/specs/constitution.md#mcp_tool_namespace_lock</file>
  <file purpose="api_contracts">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#api_contracts</file>
  <file purpose="implementation_notes">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (mcp_integration row)</file>
  <file purpose="retired_contract">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-003.md</file>
  <file purpose="existing_dispatcher_pattern">crates/context-graph-mcp/src/handlers/tools/reality_loop/mod.rs</file>
  <file purpose="existing_schema_validator">crates/context-graph-mcp/src/handlers/tools/reality_loop/schema_validator.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-003 complete: `SubscriberError` taxonomy + `CCREALITY_ENGINE_RETIRED` contract shipped</check>
  <check>`crates/context-graph-mcp/src/handlers/tools/reality_loop/schema_validator.rs` exists and is exported (the SHIPPED schema validator we route through)</check>
  <check>The MCP server crate compiles cleanly on `main`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create new module tree at `crates/context-graph-mcp/src/handlers/tools/mejepa/` with `mod.rs` declaring 5 sub-modules: `verify`, `predict_latest`, `observe_shift`, `subscriber_status`, `capture_audit`
    - Each sub-module ships a `handle(args: serde_json::Value) -> Result<serde_json::Value, McpError>` async fn that returns a `MEJEPA_TOOL_STUB_NOT_WIRED` structured error (replaced in Tasks 002-006)
    - Each sub-module declares its `Args` DTO with `#[serde(deny_unknown_fields)]` per the audit-9 lesson (additionalProperties: false at the wire)
    - JSON-Schema `serde_json::Value` for each tool, served via `tools/list` per the existing dispatcher pattern; constants live in `mejepa/schemas.rs`
    - Wire the 5 tool names into the main MCP dispatcher's match arm (existing pattern in `handlers/tools/mod.rs` or the equivalent registration site for this codebase)
    - Tool names EXACTLY: `mejepa_verify`, `mejepa_predict_latest`, `mejepa_observe_shift`, `mejepa_subscriber_status`, `mejepa_capture_audit` (all under the `mcp__cgreality__` server namespace; the wire prefix is added by the dispatcher)
    - Re-route every Args DTO's `try_from(serde_json::Value)` through the SHIPPED `reality_loop::schema_validator` helper (or its public re-export) so unknown-property rejection is centralized, not per-handler
    - Unit tests: 5 tests asserting `tools/list` enumerates all 5 names; 5 tests asserting unknown-property payloads are rejected by each schema
  </in_scope>
  <out_of_scope>
    - The actual handler bodies (TASK-MCP-002..006)
    - FSV roundtrip evidence (TASK-MCP-007)
    - Modifying the retired-surface shims (owned by sister TASK-SHIFT-003; this task asserts they remain untouched)
    - Subscriber wiring at MCP startup (sister phase phase7-shift-subscriber owns task spawning)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs">
      pub mod verify;
      pub mod predict_latest;
      pub mod observe_shift;
      pub mod subscriber_status;
      pub mod capture_audit;
      pub mod schemas;

      /// All 5 tool names registered under `mcp__cgreality__mejepa_*`.
      pub const MEJEPA_TOOL_NAMES: &amp;[&amp;str] = &amp;[
          "mejepa_verify",
          "mejepa_predict_latest",
          "mejepa_observe_shift",
          "mejepa_subscriber_status",
          "mejepa_capture_audit",
      ];

      pub async fn dispatch(
          tool_name: &amp;str,
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/schemas.rs">
      pub fn verify_schema() -&gt; serde_json::Value;
      pub fn predict_latest_schema() -&gt; serde_json::Value;
      pub fn observe_shift_schema() -&gt; serde_json::Value;
      pub fn subscriber_status_schema() -&gt; serde_json::Value;
      pub fn capture_audit_schema() -&gt; serde_json::Value;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/verify.rs">
      #[derive(serde::Deserialize, Debug)]
      #[serde(deny_unknown_fields)]
      pub struct VerifyArgs {
          pub patch: serde_json::Value,
          pub context: serde_json::Value,
          #[serde(default)]
          pub include_provenance: bool,
      }

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/predict_latest.rs">
      #[derive(serde::Deserialize, Debug)]
      #[serde(deny_unknown_fields)]
      pub struct PredictLatestArgs {
          pub session_id: String,
          #[serde(default = "default_limit")]
          pub limit: u32,
      }
      fn default_limit() -&gt; u32 { 1 }

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/observe_shift.rs">
      #[derive(serde::Deserialize, Debug)]
      #[serde(deny_unknown_fields)]
      pub struct ObserveShiftArgs {
          pub shift_id: String,
      }

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/subscriber_status.rs">
      #[derive(serde::Deserialize, Debug, Default)]
      #[serde(deny_unknown_fields)]
      pub struct SubscriberStatusArgs {}

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs">
      #[derive(serde::Deserialize, Debug)]
      #[serde(deny_unknown_fields)]
      pub struct CaptureAuditArgs {
          pub attempt_id: String,
          #[serde(default)]
          pub page: u32,
      }

      pub async fn handle(
          args: serde_json::Value,
          ctx: &amp;crate::handlers::McpContext,
      ) -&gt; Result&lt;serde_json::Value, crate::error::McpError&gt;;
    </signature>
  </signatures>

  <constraints>
    - Tool names EXACTLY as documented (snake_case, no aliasing); registered under the locked `mcp__cgreality__mejepa_*` prefix
    - Every Args DTO MUST carry `#[serde(deny_unknown_fields)]`; every JSON-Schema MUST declare `"additionalProperties": false`; both layers are required (audit-9 / NFR-SHIFT-09)
    - `additionalProperties: false` MUST be present on every nested object schema (recursive), not just the top level
    - All input parsing routed through `reality_loop::schema_validator` (or its public re-export); do NOT roll a per-handler validator
    - Stub body: each `handle()` returns `McpError::structured("MEJEPA_TOOL_STUB_NOT_WIRED", "tool {name} skeleton present; body wired in TASK-MCP-{002..006}")` — never `unimplemented!()` (which would panic the server)
    - The 5 tool names MUST appear in the existing `tools/list` response with their schemas; integration test `mcp_list_tools_includes_mejepa` asserts presence
    - This task MUST NOT touch `crates/context-graph-mcp/src/handlers/tools/reality_loop/` (retired-surface shims are sister TASK-SHIFT-003's domain)
    - File length under 500 lines for every new file
    - Per `<error_handling>`: structured `McpError` only; no `unwrap_or` defaults; no silent fallback
  </constraints>

  <verification>
    - Test 1: `cargo test -p context-graph-mcp mejepa::test_tools_list_enumerates_all_5` — passes; `tools/list` response contains each of the 5 tool names
    - Test 2: `cargo test -p context-graph-mcp mejepa::test_unknown_property_rejected_per_tool` — passes for all 5 tools (5 sub-cases)
    - Test 3: `cargo test -p context-graph-mcp mejepa::test_each_handler_returns_stub_error` — passes for all 5 tools; error code is `MEJEPA_TOOL_STUB_NOT_WIRED`
    - Test 4: `cargo test -p context-graph-mcp mejepa::test_schemas_declare_additional_properties_false` — passes; recursive walk of each schema confirms every object node has `additionalProperties: false`
    - Test 5: `cargo test -p context-graph-mcp mejepa::test_retired_surfaces_untouched` — assert that `reality_loop::retired::reality_run_attempt` still returns `CCREALITY_ENGINE_RETIRED` (regression guard against accidental aliasing)
    - `cargo build --release -p context-graph-mcp` — clean
    - `cargo clippy -p context-graph-mcp -- -D warnings` — clean
  </verification>
</definition_of_done>

<pseudo_code>
mod.rs:
  declare 5 sub-modules + schemas + dispatch
  MEJEPA_TOOL_NAMES = const slice of 5 names
  fn dispatch(tool_name, args, ctx):
    match tool_name:
      "mejepa_verify"            => verify::handle(args, ctx).await
      "mejepa_predict_latest"    => predict_latest::handle(args, ctx).await
      "mejepa_observe_shift"     => observe_shift::handle(args, ctx).await
      "mejepa_subscriber_status" => subscriber_status::handle(args, ctx).await
      "mejepa_capture_audit"     => capture_audit::handle(args, ctx).await
      _ => Err(McpError::unknown_tool(tool_name))

schemas.rs:
  for each tool: build serde_json!({"type": "object", "properties": {...}, "required": [...], "additionalProperties": false})
  recursive helper assert_no_open_objects(schema) used by tests

verify.rs (and 4 siblings):
  parse_args::<VerifyArgs>(args)?  // routed through reality_loop::schema_validator
  return McpError::structured("MEJEPA_TOOL_STUB_NOT_WIRED", "tool body wired in TASK-MCP-002")

dispatcher_registration:
  in handlers/tools/mod.rs (or the equivalent existing registry):
    add: pub mod mejepa; (one line)
    in tool list: extend with mejepa::MEJEPA_TOOL_NAMES.iter().map(|n| ToolDef { name: format!("{MCP_NS_PREFIX}{n}"), schema: mejepa::schemas::<n>_schema() })
    in dispatcher match: route by stripped name to mejepa::dispatch(...)
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/mod.rs">module declarations + dispatch fn + MEJEPA_TOOL_NAMES const</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/schemas.rs">5 schema builders (additionalProperties: false recursive)</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/verify.rs">VerifyArgs DTO + stub handle()</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/predict_latest.rs">PredictLatestArgs DTO + stub handle()</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/observe_shift.rs">ObserveShiftArgs DTO + stub handle()</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/subscriber_status.rs">SubscriberStatusArgs DTO + stub handle()</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/mejepa/capture_audit.rs">CaptureAuditArgs DTO + stub handle()</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/tools/mod.rs">add `pub mod mejepa;` and route the 5 names in the existing tool-name match arm</file>
  <file path="crates/context-graph-mcp/src/handlers/mod.rs">re-export `tools::mejepa::MEJEPA_TOOL_NAMES` if the existing `tools/list` builder requires it</file>
</files_to_modify>

<validation_criteria>
  <criterion>All 5 tool names enumerated in `tools/list`</criterion>
  <criterion>Every tool's JSON-Schema has `additionalProperties: false` recursively</criterion>
  <criterion>Every Args DTO has `#[serde(deny_unknown_fields)]`</criterion>
  <criterion>Every stub handler returns `MEJEPA_TOOL_STUB_NOT_WIRED` (no panic, no `unimplemented!`)</criterion>
  <criterion>Retired-surface shims regression test still returns `CCREALITY_ENGINE_RETIRED`</criterion>
  <criterion>No file exceeds 500 lines</criterion>
  <criterion>Clean release build + zero clippy warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mcp mejepa::</command>
  <command>cargo clippy -p context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>
