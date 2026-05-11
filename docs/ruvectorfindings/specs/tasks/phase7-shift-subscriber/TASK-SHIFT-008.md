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

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase + 2026 best practices)

> **Codebase reality table:** see `_index.md` in this directory.

**2026 best-practice refresh (per WebSearch on RocksDB durability):**
- `WriteOptions { sync: true }` flushes the WAL synchronously to disk — sufficient for at-least-once delivery to a single CF. Per-shift latency budget (5 s p50) easily absorbs an fsync (~2 ms on NVMe).
- **NEVER batch watermark writes.** Batched writes would advance past shifts whose persistence is still in flight, violating REQ-SHIFT-02 / BR-SHIFT-02.
- Idempotency is structural: writes to `CF_MEJEPA_LIVE_PREDICTIONS` and `CF_MEJEPA_ORACLE_VERDICTS` overwrite atomically on the same `(session_id, witness_chain_offset)` key, so a SIGKILL between persistence and watermark advance is safe — the in-flight shift is reprocessed exactly once on respawn.

**Confirmed 2026-05-11:**
- `CF_MEJEPA_SHIFT_WATERMARK` already declared in `mejepa-cf/src/lib.rs:14`. Key/value layout owned by sister TASK-SHIFT-004.
- The RocksDB handle is shared with `RocksDbInferStore` — open via `Arc<DB>` rather than per-component opens.

**Decided scope for this task (refined):**
1. `WatermarkWriter::new(rocksdb: Arc<DB>, cf: ColumnFamilyHandle) -> Self`.
2. `write_watermark(record: WatermarkRecord) -> Result<(), WatermarkError>`:
   (a) read existing record at `b"wm:" || hex(session_id)`; if `persisted.last_consumed_shift_id > record.last_consumed_shift_id` (lex compare per ULID monotonicity), return `WatermarkError::Backwards` (maps to `MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS`);
   (b) JSON-serialize record;
   (c) `db.put_cf_opt(&self.cf, key, json_bytes, WriteOptions { sync: true })?`;
   (d) return Ok ONLY after the put returns (sync=true blocks until fsync completes).
3. `read_watermark(session_id) -> Result<Option<WatermarkRecord>, WatermarkError>`: point read; deserialize JSON; None on missing key.
4. `read_all_watermarks() -> Result<Vec<WatermarkRecord>, WatermarkError>`: prefix iter on `b"wm:"`; deserialize each value; used by `run()` startup hydration in TASK-SHIFT-010.
5. NO `bincode`. JSON only. Per memory `bincode + skip_serializing_if = BROKEN`.
6. NEVER `unwrap_or` defaults. Every error path returns a structured `WatermarkError` whose `.code()` returns SCREAMING_SNAKE.

**FSV for this task:** integration test against a tempdir RocksDB:
(a) write 100 watermarks for 10 sessions → `read_all_watermarks` returns all 100 in lexicographic order;
(b) write a backwards shift_id → asserts `WatermarkError::Backwards` returned, persisted state unchanged;
(c) spawn process A writing watermark 50, SIGKILL between put_cf and Ok return, spawn process B → process B reads back watermark 50 (sync=true guarantees the put hit disk before Ok was returned).
Source of Truth: separate `db.get_cf` reads after each scenario; for (c), shutdown + reopen the RocksDB to confirm SST persistence.

---

<task_spec id="TASK-SHIFT-008" version="1.0">
<metadata>
  <title>Logic — WatermarkWriter durable, idempotent, restart-safe advance (CF_MEJEPA_SHIFT_WATERMARK)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-SHIFT-02">Watermark advance only after persistence-and-cert success; durable fsync</item>
    <item ref="REQ-SHIFT-18">Provenance metadata in every watermark record</item>
    <item ref="ERR-SHIFT-04">MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS</item>
    <item ref="EC-SHIFT-06">SIGKILL between persist and watermark advance is recoverable</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[WatermarkWriter]">3 methods</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-003</task_ref>
    <task_ref>TASK-SHIFT-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (watermark.rs body) + tests</estimated_files>
</metadata>

<context>
Implements durable per-session watermark writes. `write_watermark` validates monotonicity, serializes WatermarkRecord to JSON, calls `rocksdb::DB::put_cf_opt` with `WriteOptions { sync: true }`, returns Ok only after fsync. `read_watermark` does point-read on `wm:{session_id}`. `read_all_watermarks` iterates with prefix `wm:` for restart hydration. NEVER batches watermark writes (per implementation_notes.durability rule).
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[WatermarkWriter]</file>
  <file purpose="durability_note">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (durability + ordering rows)</file>
  <file purpose="cf_keys">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-004.md</file>
  <file purpose="watermark_record">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-002, TASK-SHIFT-003, TASK-SHIFT-004 complete</check>
  <check>`rocksdb` workspace dep available</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement `WatermarkWriter::new`, `write_watermark`, `read_watermark`, `read_all_watermarks`
    - WriteOptions with `set_sync(true)` per durability invariant; one watermark write per shift (no batching)
    - Monotonicity validation: read existing record; if `persisted.last_consumed_shift_id >= record.last_consumed_shift_id`, return `WatermarkBackwards` (using ULID lexicographic ordering)
    - JSON serialization via `serde_json::to_vec`; deserialization via `serde_json::from_slice`
    - Tests: write → read round-trip; backwards write rejected; iter returns all written records; SIGKILL drill (write, kill simulation, reopen DB, verify last write is durable)
  </in_scope>
  <out_of_scope>
    - Tier-compression policy (SPEC-HYGIENE)
    - Watermark CF retention pruning (SPEC-HYGIENE)
    - run() startup hydration call (TASK-SHIFT-010)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/watermark.rs">
      use std::sync::Arc;
      use rocksdb::{DB, ColumnFamily, WriteOptions};
      use crate::cf::{CF_MEJEPA_SHIFT_WATERMARK, watermark_key};
      use crate::entry::SessionId;
      use crate::error::SubscriberError;

      pub struct WatermarkWriter {
          rocksdb: Arc&lt;DB&gt;,
          cf_name: String, // resolved to handle on each call (rocksdb crate idiom)
          write_options: WriteOptions,
      }

      impl WatermarkWriter {
          pub fn new(rocksdb: Arc&lt;DB&gt;) -&gt; Self;

          pub fn write_watermark(&amp;self, record: &amp;WatermarkRecord) -&gt; Result&lt;(), SubscriberError&gt;;

          pub fn read_watermark(&amp;self, session_id: &amp;SessionId) -&gt; Result&lt;Option&lt;WatermarkRecord&gt;, SubscriberError&gt;;

          pub fn read_all_watermarks(&amp;self) -&gt; Result&lt;Vec&lt;WatermarkRecord&gt;, SubscriberError&gt;;
      }
    </signature>
  </signatures>

  <constraints>
    - `WriteOptions::set_sync(true)` MUST be set on every write (no batching)
    - Monotonicity check uses ULID lexicographic ordering (`record.last_consumed_shift_id.as_str() &lt; persisted.as_str()` triggers WatermarkBackwards)
    - NO `unwrap()` outside tests
    - Iterator close is implicit via Drop; no leaked handles
    - File length under 500 lines
  </constraints>

  <verification>
    - Round-trip test: write WatermarkRecord, read back, assert byte-identical
    - Monotonicity test: write {shift_id: A}, attempt write {shift_id: A-1} → WatermarkBackwards
    - read_all_watermarks returns all written sessions
    - SIGKILL drill: write watermark, drop+reopen DB instance, read_watermark returns the record (proves fsync semantic)
    - Concurrent test: 8 threads each writing distinct sessions → no conflict; all 8 records readable
  </verification>
</definition_of_done>

<pseudo_code>
write_watermark:
  let cf = self.rocksdb.cf_handle(&self.cf_name).ok_or(...)?;
  let key = watermark_key(&record.session_id);
  if let Some(existing) = self.read_watermark(&record.session_id)? {
    if existing.last_consumed_shift_id.as_str() > record.last_consumed_shift_id.as_str() {
      return Err(SubscriberError::WatermarkBackwards{ persisted: existing.last_consumed_shift_id, observed: record.last_consumed_shift_id.clone(), session_id: record.session_id.clone() });
    }
  }
  let value = serde_json::to_vec(record)?;
  self.rocksdb.put_cf_opt(cf, &key, &value, &self.write_options)
    .map_err(|e| SubscriberError::RocksDbWriteFailed{ cf: CF_MEJEPA_SHIFT_WATERMARK.to_string(), key: key.clone(), retry: 0, source: e })?;
  Ok(())
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/watermark_durability.rs">round-trip + monotonicity + SIGKILL drill</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/watermark.rs">implement 3 methods + ctor</file>
</files_to_modify>

<validation_criteria>
  <criterion>WriteOptions::sync is true on every write</criterion>
  <criterion>Monotonicity violation returns WatermarkBackwards</criterion>
  <criterion>SIGKILL drill survives DB reopen</criterion>
  <criterion>Iterator-based read_all returns all rows</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber watermark::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test watermark_durability</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
