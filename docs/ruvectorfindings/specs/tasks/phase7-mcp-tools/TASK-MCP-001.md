## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory for the full audit table (ground truth as of 2026-05-11). What follows is THIS task's scope-specific drift.

**Original assumption (2026-05-09 author):** create a `crates/context-graph-mcp/src/handlers/tools/mejepa/` sub-module tree with `mod.rs`, `schemas.rs`, and 5 sibling handler files; register 5 tool names.

**What actually exists (verified by `ls`/`grep` 2026-05-11):**
- The sub-module tree was NEVER created. MCP tool handlers live in flat files: `mejepa_tools.rs` (838 lines, 5 handlers), `mejepa_hygiene_tools.rs` (106 lines, 3 Phase 6 handlers), `embedder_tools.rs` (3 more handlers ~1753-1840).
- Tool name constants in `crates/context-graph-mcp/src/tools/names.rs:96-114` hold the **fully-namespaced wire string** (e.g. `MEJEPA_PREDICT_LATEST = "mcp__cgreality__mejepa_predict_latest"`) — there is NO separate dispatcher prefix added at runtime.
- Tool DEFINITIONS (JSON-Schema) live in `crates/context-graph-mcp/src/tools/definitions/mejepa.rs` (149 lines).
- 8 mejepa MCP tools already SHIPPED (5 in `mejepa_tools.rs` + 3 in `embedder_tools.rs`); 3 Phase 6 hygiene tools shipped in `mejepa_hygiene_tools.rs`.
- `crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs` does NOT exist; `CCREALITY_ENGINE_RETIRED` literal absent everywhere in the MCP source. Retired-name calls today fall through to the dispatcher's unknown-tool branch — a regression against constitution `<mcp_tool_namespace_lock>`.

**Decided scope for this task (Occam's-razor, operator-approvable):**
1. Add 3 NEW const tool names to `crates/context-graph-mcp/src/tools/names.rs`: `MEJEPA_OBSERVE_SHIFT`, `MEJEPA_SUBSCRIBER_STATUS`, `MEJEPA_CAPTURE_AUDIT`. Each constant value is the FULL wire name (e.g. `"mcp__cgreality__mejepa_observe_shift"`). Match the established pattern at lines 96-114.
2. Add 3 NEW schema builder functions to `crates/context-graph-mcp/src/tools/definitions/mejepa.rs`. Every schema MUST include `"additionalProperties": false` recursively on all nested objects. Wire each into the existing `mejepa_tool_definitions()` enumerator.
3. Add 3 stub dispatcher arms to `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` (after line 218): `tool_names::MEJEPA_OBSERVE_SHIFT => call_mejepa_observe_shift(arguments),` etc. Each `call_*` is implemented as a handler function in `mejepa_tools.rs` that returns a structured `MEJEPA_TOOL_STUB_NOT_WIRED` error pending TASK-MCP-{004,005,006}.
4. Add a retired-name intercept BEFORE the unknown-tool fall-through in `dispatch.rs`. Match the set: `mcp__cgreality__reality_run_attempt`, `mcp__cgreality__reality_alter_file`, `mcp__cgreality__reality_alter_run`, `mcp__cgreality__reality_alter_apply`, `mcp__cgreality__reality_drip_attempt`. Emit `CCREALITY_ENGINE_RETIRED` with payload `{"attempted_tool": "<name>", "message": "tool retired in 2026-05-09 ME-JEPA pivot; use mcp__cgreality__mejepa_*"}`. NEVER spawn `target/release/reality-loop`. NEVER alias to a new name.
5. Args DTOs (`ObserveShiftArgs`, `SubscriberStatusArgs`, `CaptureAuditArgs`) declared inline at top of `mejepa_tools.rs` with `#[serde(rename_all = "camelCase", deny_unknown_fields)]` matching the existing pattern at line 32-69.
6. SessionId contract: any new tool consuming `session_id` MUST use the hex-32 → `[u8; 16]` decoder pattern from `mejepa_tools.rs:169-185` (validate length=32 + all-hex + decode_to_slice). The SPEC's "up to 64 chars UTF-8" is stale; trust the code.

**Out-of-scope (explicit):** the actual handler bodies (TASK-MCP-{002,003,004,005,006} own those); FSV roundtrip evidence (TASK-MCP-007); refactoring the 8 SHIPPED handlers into a sub-module tree (would touch ~838 lines and risk regression — defer until operator-approved).

**Files this task touches:**
- `crates/context-graph-mcp/src/tools/names.rs` (3 new const lines)
- `crates/context-graph-mcp/src/tools/definitions/mejepa.rs` (3 new schema builders + enumerator extension)
- `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` (3 new arms + retired-name intercept)
- `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` (3 new stub `call_*` functions returning `MEJEPA_TOOL_STUB_NOT_WIRED`; if file >1500 lines after this addition, split in a follow-up task)

**FSV for this task:** unit tests covering (a) `tools/list` enumerates the 3 new names with their schemas; (b) every new schema has `additionalProperties: false` recursively; (c) every retired name in the intercept set returns `CCREALITY_ENGINE_RETIRED`; (d) every stub handler returns `MEJEPA_TOOL_STUB_NOT_WIRED` (no panic).

---

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
