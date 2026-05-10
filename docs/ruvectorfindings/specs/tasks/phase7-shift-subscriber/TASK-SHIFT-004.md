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

- ✅ **Crate `context-graph-mejepa-shift-subscriber` does NOT exist yet** — CF descriptor module lands in the same not-yet-existing crate created by TASK-SHIFT-001.
- ⚠ **CF naming overlap audit:** the 5 CFs declared here (`CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS`) DO match the doc 11 / CLAUDE.md §11.13 artifact layout. Sibling CFs `CF_MEJEPA_ACTIVE_POINTERS` and `CF_MEJEPA_HEAL_REPORTS` are already used by `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` against a separate RocksDB instance — naming convention consistent.
- ✅ `rocksdb` workspace dep present; `sha2` workspace dep present (used by `context-graph-witness` already).
- ✅ Constitution `<naming_conventions>` row "rocksdb_cfs SCREAMING_SNAKE with CF_ prefix" still in force.
- No drift in `<files_to_create>` paths.

<task_spec id="TASK-SHIFT-004" version="1.0">
<metadata>
  <title>Foundation — RocksDB CF_MEJEPA_SHIFT_WATERMARK schema + per-shift cert key layout</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-SHIFT-02">CF_MEJEPA_SHIFT_WATERMARK persistence + restart-safe resume</item>
    <item ref="REQ-SHIFT-18">Provenance metadata embedded in each persisted record</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_SHIFT_WATERMARK]">key/value/write_options</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_LIVE_PREDICTIONS]">key layout (10-byte composite)</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#rocksdb_schema[CF_MEJEPA_TRAIN_CERTS]">per-shift cert key layout (step number BE u64)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 files</estimated_files>
</metadata>

<context>
Adds RocksDB column-family descriptors and key-builder helpers consumed by TASK-SHIFT-008 (WatermarkWriter), TASK-SHIFT-007 (process_shift), and the sister phase7-mcp-tools handlers. No I/O happens in this task — only deterministic key-builder functions + CF descriptor declarations + key-format unit tests.
</context>

<input_context_files>
  <file purpose="cf_schema">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#rocksdb_schema</file>
  <file purpose="cf_naming_convention">docs/ruvectorfindings/specs/constitution.md#coding_standards (rocksdb_cfs row)</file>
  <file purpose="prior_storage_pattern">crates/context-graph-storage/ (existing CF descriptor patterns)</file>
  <file purpose="watermark_record_struct">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-001 complete</check>
  <check>TASK-SHIFT-002 complete (WatermarkRecord exists)</check>
  <check>`rocksdb` crate is a workspace dependency</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa-shift-subscriber/src/cf.rs` declaring CF name constants (`CF_MEJEPA_SHIFT_WATERMARK`, `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_ORACLE_VERDICTS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS`)
    - Key-builder helpers (pure functions, no I/O):
      - `watermark_key(session_id) -> Vec<u8>` (= `b"wm:" || session_id` UTF-8)
      - `live_prediction_key(session_id_hash6, witness_chain_offset_be8) -> [u8; 10]`
      - `oracle_verdict_key(task_id, mutation_category) -> Vec<u8>`
      - `train_cert_key(step_be_u64) -> [u8; 8]`
    - CF descriptor function: `pub fn shift_subscriber_cf_descriptors() -> Vec<rocksdb::ColumnFamilyDescriptor>` returning the 5 CFs with appropriate per-CF Options
    - Unit tests: each key-builder is deterministic, length-stable, prefix-scannable
  </in_scope>
  <out_of_scope>
    - Any RocksDB open/write code (TASK-SHIFT-008)
    - Tier-compression or hygiene policy (SPEC-HYGIENE)
    - Secondary index `SubscriberIndex` (lives in TASK-SHIFT-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/cf.rs">
      pub const CF_MEJEPA_SHIFT_WATERMARK: &amp;str = "CF_MEJEPA_SHIFT_WATERMARK";
      pub const CF_MEJEPA_LIVE_PREDICTIONS: &amp;str = "CF_MEJEPA_LIVE_PREDICTIONS";
      pub const CF_MEJEPA_ORACLE_VERDICTS: &amp;str = "CF_MEJEPA_ORACLE_VERDICTS";
      pub const CF_MEJEPA_TRAIN_CERTS: &amp;str = "CF_MEJEPA_TRAIN_CERTS";
      pub const CF_MEJEPA_PANELS: &amp;str = "CF_MEJEPA_PANELS";

      pub fn watermark_key(session_id: &amp;crate::entry::SessionId) -&gt; Vec&lt;u8&gt;;
      pub fn live_prediction_key(session_id_hash6: [u8; 6], witness_chain_offset: u64) -&gt; [u8; 10];
      pub fn live_prediction_prefix(session_id_hash6: [u8; 6]) -&gt; [u8; 6];
      pub fn oracle_verdict_key(task_id: &amp;str, mutation_category: &amp;str) -&gt; Vec&lt;u8&gt;;
      pub fn train_cert_key(step: u64) -&gt; [u8; 8];

      pub fn session_id_hash6(session_id: &amp;crate::entry::SessionId) -&gt; [u8; 6];
        // SHA-256(session_id.as_str())[..6]; deterministic; collision-tolerant for prefix scan
        // (collisions resolve via subsequent (offset_be8) tail or a follow-up lookup)

      pub fn shift_subscriber_cf_descriptors() -&gt; Vec&lt;rocksdb::ColumnFamilyDescriptor&gt;;
    </signature>
  </signatures>

  <constraints>
    - All CF names exactly as specified (SCREAMING_SNAKE with `CF_` prefix per constitution `<naming_conventions>`)
    - `live_prediction_key` MUST be 10 bytes total (6-byte hashed session + 8-byte big-endian offset) for prefix-scannability per `<rocksdb_schema>`
    - `train_cert_key` MUST be 8-byte big-endian u64 (lexicographic == numerical order)
    - `oracle_verdict_key` is `task_id || ":" || mutation_category` UTF-8; reject task/category strings containing `:` (must be Docker-name-safe per SEC-MEJEPA-05)
    - Per-CF `rocksdb::Options`:
      - All 5 CFs: `set_compression_type(rocksdb::DBCompressionType::Lz4)` (default)
      - `CF_MEJEPA_SHIFT_WATERMARK`: small CF; `set_max_write_buffer_number(2)`
      - `CF_MEJEPA_LIVE_PREDICTIONS`, `CF_MEJEPA_TRAIN_CERTS`, `CF_MEJEPA_PANELS`: prefix extractor on `[..6]` to enable bloom filters on prefix scans
    - NO `unwrap()` outside tests
  </constraints>

  <verification>
    - Unit test asserts `watermark_key(SessionId("abc"))` == `b"wm:abc"`
    - Unit test asserts `live_prediction_key(_, 0)` ends in `0x00..0x00` (8 zero bytes); `live_prediction_key(_, u64::MAX)` ends in `0xFF..0xFF`
    - Unit test asserts `train_cert_key(1)` == `[0,0,0,0,0,0,0,1]`
    - Unit test asserts `live_prediction_prefix(h)` is a prefix of `live_prediction_key(h, _)`
    - Unit test asserts `oracle_verdict_key("t", "c")` == `b"t:c"`
    - Unit test rejects `oracle_verdict_key("a:b", "c")` with structured error
    - 5 CF descriptors returned by `shift_subscriber_cf_descriptors()`; names match constants
  </verification>
</definition_of_done>

<pseudo_code>
session_id_hash6(sid):
  let mut hasher = sha2::Sha256::new();
  hasher.update(sid.as_str().as_bytes());
  let digest = hasher.finalize();
  digest[..6].try_into().unwrap()  -- safe: SHA-256 always 32 bytes

live_prediction_key(h6, offset):
  let mut k = [0u8; 10];
  k[..6].copy_from_slice(&h6);
  k[6..].copy_from_slice(&offset.to_be_bytes());
  k
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/cf.rs">CF name constants + key builders + CF descriptors</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod cf; pub use cf::*;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/Cargo.toml">add `sha2` workspace dep + ensure `rocksdb` enabled</file>
</files_to_modify>

<validation_criteria>
  <criterion>5 CF descriptors returned; names match constants byte-for-byte</criterion>
  <criterion>Key-builders deterministic across runs (test asserts identity over 100 invocations)</criterion>
  <criterion>Length invariants: watermark_key &lt;= 67 bytes (3 + 64); live_prediction_key == 10; train_cert_key == 8</criterion>
  <criterion>oracle_verdict_key rejects `:` in task_id or mutation_category</criterion>
  <criterion>Build clean; no warnings</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber cf::tests</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
