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

## 📋 2026-05-10 STATE AUDIT (agent-211-task-updater)

**Audit pass against current tree on 2026-05-10:**

- ✅ **Crate `context-graph-mejepa-shift-subscriber` does NOT exist yet** — `SubscriberError` lands in the same not-yet-existing crate via this task.
- ⚠ **`crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs` does NOT exist.** `ls` returns "No such file or directory". The "verify or create: 4+ retired handlers all returning CCREALITY_ENGINE_RETIRED with no subprocess" branch is the relevant one — this task CREATES `retired.rs`. The pre-pivot retired-tool fail-closed wiring is currently distributed across other handlers (cf. the 2026-05-09 retirement commits `77cd98c5` / `7a718785` in `git log`); centralizing it into a single `retired.rs` per this task spec is still wanted.
- ✅ Existing retired-MCP fail-closed sentinels already exist as `CCREALITY_ENGINE_RETIRED` markers — confirmed via repo search; new `retired.rs` should match the same sentinel.
- ✅ `thiserror`, `rocksdb`, `serde_json` workspace deps present.
- No drift in `<files_to_create>` for `error.rs`.

<task_spec id="TASK-SHIFT-003" version="1.0">
<metadata>
  <title>Foundation — SubscriberError taxonomy + retired-tool fail-closed wiring (CCREALITY_ENGINE_RETIRED)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-SHIFT-13">Retired tool surfaces fail-closed with CCREALITY_ENGINE_RETIRED</item>
    <item ref="REQ-SHIFT-14">SubscriberError variants with SCREAMING_SNAKE .code() tags</item>
    <item ref="ERR-SHIFT-01..08">8 documented error states</item>
    <item ref="BR-SHIFT-05">Retired surfaces stay retired; no compatibility shims</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#data_models[SubscriberError]">9-variant enum</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[RetiredSurfaceHandler]">reality_run_attempt + reality_alter_*</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>3 files</estimated_files>
</metadata>

<context>
Defines the structured failure taxonomy for the subscriber crate (every fallible path returns `Result<T, SubscriberError>` per constitution `<error_handling>`) AND wires the retired-tool fail-closed pattern into the existing `context-graph-mcp` handler tree. The retired wiring lives in `context-graph-mcp/src/handlers/tools/reality_loop/retired.rs` (existing file; this task either extends or replaces the body to ensure CCREALITY_ENGINE_RETIRED is the only outcome, with NO `target/release/reality-loop` spawn).
</context>

<input_context_files>
  <file purpose="failure_taxonomy_codes">docs/ruvectorfindings/specs/constitution.md#failure_taxonomy</file>
  <file purpose="error_state_specs">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md#error_states</file>
  <file purpose="subscriber_error_variants">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#data_models[SubscriberError]</file>
  <file purpose="existing_retired_handler">crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs (if it exists)</file>
  <file purpose="prior_task_data_models">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-001.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001 complete</check>
  <check>`thiserror` workspace dependency available</check>
  <check>`context-graph-mcp` builds at HEAD</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa-shift-subscriber/src/error.rs` — `SubscriberError` enum with 9 variants; `.code()` returns SCREAMING_SNAKE per constitution `<failure_taxonomy>`
    - Implement `From<...>` for upstream error types (rocksdb::Error, serde_json::Error, std::io::Error)
    - `From<ConfigOutOfBoundsError>` (TASK-SHIFT-001) → `SubscriberError::ConfigOutOfBounds`
    - Wire / verify `crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs` returns `CCREALITY_ENGINE_RETIRED` for: `reality_run_attempt`, `reality_alter_file`, `reality_alter_run`, `reality_alter_apply` (and any other pre-pivot harness-write surfaces present in the dispatcher)
    - Unit tests: every variant's `.code()` returns the documented tag
  </in_scope>
  <out_of_scope>
    - MCP tool registration for `mejepa_*` namespace (TASK-MCP-001)
    - Any new MCP tool body (sister phase)
    - Removing the retired handlers (they STAY; only verify the body fail-closes)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/error.rs">
      use crate::entry::{SessionId, ShiftId};
      use crate::audit::UtmlFactorBundle;
      use thiserror::Error;
      use std::path::PathBuf;

      #[derive(Debug, Error)]
      pub enum SubscriberError {
          #[error("MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT: lag={lag}, sustain_seconds={sustain_seconds}")]
          LagAlert { lag: u64, sustain_seconds: u64 },

          #[error("MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL: session={session_id:?}, byte_offset={byte_offset}, error={parse_error}")]
          LogParseFail { session_id: SessionId, byte_offset: u64, line_preview: String, parse_error: String },

          #[error("MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW: requested={requested_bytes}, current={current_bytes}, capacity={capacity_bytes}")]
          InstrumentCacheOverflow { requested_bytes: u64, current_bytes: u64, capacity_bytes: u64 },

          #[error("MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS: persisted={persisted:?}, observed={observed:?}, session={session_id:?}")]
          WatermarkBackwards { persisted: ShiftId, observed: ShiftId, session_id: SessionId },

          #[error("MEJEPA_OBSERVE_BATCH_NAN: shift={shift_id:?}, factors={factors:?}")]
          ObserveBatchNan { shift_id: ShiftId, factors: UtmlFactorBundle },

          #[error("MEJEPA_OBSERVE_INTEGRITY_VIOLATION: shift={shift_id:?}, offset={witness_chain_offset}")]
          ObserveIntegrityViolation { shift_id: ShiftId, witness_chain_offset: u64, expected_hash: [u8; 32], observed_hash: [u8; 32] },

          #[error("CCREALITY_ENGINE_RETIRED: tool_name={tool_name}")]
          EngineRetired { tool_name: String },

          #[error("MEJEPA_SHIFT_SUBSCRIBER_ROCKSDB_WRITE_FAILED: cf={cf}, retry={retry}")]
          RocksDbWriteFailed { cf: String, key: Vec&lt;u8&gt;, retry: u8, #[source] source: rocksdb::Error },

          #[error("MEJEPA_SHIFT_SUBSCRIBER_CONFIG_OUT_OF_BOUNDS: knob={knob}, value={value}")]
          ConfigOutOfBounds { knob: String, value: String },
      }

      impl SubscriberError {
          pub fn code(&amp;self) -&gt; &amp;'static str {
              match self {
                  Self::LagAlert{..}                  =&gt; "MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT",
                  Self::LogParseFail{..}              =&gt; "MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL",
                  Self::InstrumentCacheOverflow{..}   =&gt; "MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW",
                  Self::WatermarkBackwards{..}        =&gt; "MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS",
                  Self::ObserveBatchNan{..}           =&gt; "MEJEPA_OBSERVE_BATCH_NAN",
                  Self::ObserveIntegrityViolation{..} =&gt; "MEJEPA_OBSERVE_INTEGRITY_VIOLATION",
                  Self::EngineRetired{..}             =&gt; "CCREALITY_ENGINE_RETIRED",
                  Self::RocksDbWriteFailed{..}        =&gt; "MEJEPA_SHIFT_SUBSCRIBER_ROCKSDB_WRITE_FAILED",
                  Self::ConfigOutOfBounds{..}         =&gt; "MEJEPA_SHIFT_SUBSCRIBER_CONFIG_OUT_OF_BOUNDS",
              }
          }
          pub fn as_json_payload(&amp;self) -&gt; serde_json::Value;
      }
    </signature>
    <signature file="crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs">
      use serde_json::{json, Value};
      use crate::error::McpError;

      pub async fn reality_run_attempt(_args: Value) -&gt; Result&lt;Value, McpError&gt;;
      pub async fn reality_alter_file(_args: Value) -&gt; Result&lt;Value, McpError&gt;;
      pub async fn reality_alter_run(_args: Value) -&gt; Result&lt;Value, McpError&gt;;
      pub async fn reality_alter_apply(_args: Value) -&gt; Result&lt;Value, McpError&gt;;

      // Each returns:
      //   Err(McpError {
      //     code: "CCREALITY_ENGINE_RETIRED",
      //     message: "tool retired in 2026-05-09 ME-JEPA pivot; use mcp__cgreality__mejepa_*",
      //     payload: json!({ "attempted_tool": "&lt;name&gt;" }),
      //   })
      // No spawn of `target/release/reality-loop`. No alias.
    </signature>
  </signatures>

  <constraints>
    - Every variant carries SHA / shift_id / session_id / byte_offset / paths context per `<error_handling>` (no prose-only errors)
    - `.code()` returns `&'static str` exactly matching constitution `<failure_taxonomy>` SCREAMING_SNAKE codes
    - `as_json_payload()` produces a JSON object suitable for direct embedding in MCP tool error responses
    - Retired handlers MUST NOT spawn any subprocess; MUST NOT read any file in `target/release/`; MUST NOT alias to any new tool
    - If existing `retired.rs` is missing, this task creates it; if it exists, this task ensures behaviour matches contract (no extra error paths added)
    - File length under 500 lines
  </constraints>

  <verification>
    - Unit test exhaustively constructs each `SubscriberError` variant and asserts `.code()` matches the constitution row
    - JSON serialization of each variant produces a stable shape (round-trip via serde_json)
    - Integration test invokes each retired handler with arbitrary args and asserts `CCREALITY_ENGINE_RETIRED` returned, no subprocess spawned
    - `cargo build --release -p context-graph-mejepa-shift-subscriber -p context-graph-mcp` clean
    - Grep verification: `rg "target/release/reality-loop" crates/context-graph-mcp/src/handlers/tools/reality_loop/` returns zero matches
  </verification>
</definition_of_done>

<pseudo_code>
SubscriberError::as_json_payload(&self) -> serde_json::Value:
  json!({ "error_code": self.code(), ...variant fields... })

reality_run_attempt(_args) -> McpError {
  code: "CCREALITY_ENGINE_RETIRED",
  message: "tool retired in 2026-05-09 ME-JEPA pivot; use mcp__cgreality__mejepa_*",
  payload: json!({ "attempted_tool": "reality_run_attempt" }),
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/error.rs">SubscriberError enum + .code() + as_json_payload + From impls</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod error; pub use error::SubscriberError;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add `rocksdb` dep (workspace) for the From impl + `serde_json`</file>
  <file path="crates/context-graph-mcp/src/handlers/tools/reality_loop/retired.rs">verify or create: 4+ retired handlers all returning CCREALITY_ENGINE_RETIRED with no subprocess</file>
</files_to_modify>

<validation_criteria>
  <criterion>9 SubscriberError variants present with documented .code() tags</criterion>
  <criterion>Retired handlers fail closed with CCREALITY_ENGINE_RETIRED for every retired tool name</criterion>
  <criterion>No `target/release/reality-loop` reference anywhere under `handlers/tools/reality_loop/`</criterion>
  <criterion>Build succeeds for both crates</criterion>
  <criterion>JSON payload shape stable across runs (deterministic test)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber -p context-graph-mcp</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber error::tests</command>
  <command>cargo test -p context-graph-mcp retired_handlers</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
