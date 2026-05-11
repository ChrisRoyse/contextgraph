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

> **Codebase reality table:** see `_index.md` in this directory.

**Confirmed 2026-05-11:**
- The MCP server already manages a singleton flock at `~/.cache/cgreality/reality-loop-stdio.lock` (per CLAUDE.md §11.12, commits `71404040`, `36e01578`); only one MCP server runs at a time, so only one `ShiftSubscriber::run` task runs at a time.
- The MCP server lifecycle is in `crates/context-graph-mcp/src/main.rs`; the subscriber Arc must be constructed during startup AFTER `RocksDbInferStore` is opened, BEFORE the JSON-RPC dispatcher starts accepting requests.

**Decided scope for this task (refined):**
1. `pub async fn run(mut self) -> Result<(), SubscriberError>`:
   (a) `metrics.task_alive_since = Some(now())`;
   (b) hydrate per-session resume points via `watermark_writer.read_all_watermarks()`; for each session call `tail.open_or_create_reader(session_id, last_consumed_byte_offset)`;
   (c) main loop:
       - poll all session readers via `tail.poll_all().await` (returns `Vec<(SessionId, ShiftEntry)>`)
       - for each ShiftEntry: `self.process_shift(entry).await?`
       - on Ok: `watermark_writer.write_watermark(record)?`
       - update lag metrics
       - `evaluate_lag_alert_state()` (TASK-SHIFT-009)
       - `tokio::select!` on shutdown signal: drain in-flight, set `task_alive_since = None`, return Ok
   (d) panic supervision: wrap CPU-bound calls in `tokio::task::spawn_blocking`; the outer Tokio task uses `tokio::task::JoinSet` (or a manual `JoinHandle::is_panicked()` check) to catch instrument-forward panics; on panic populate `metrics.last_panic = Some(PanicInfo { ... })`; respawn-or-die per EC-SHIFT-10 (3 panics on the same shift_id → high-severity blocker).
2. MCP integration: in `context-graph-mcp/src/main.rs`, after RocksDB open, construct the `ShiftSubscriber`, get an `Arc<MeJepaSubscriber>` (or split into `Arc<MeJepaCompiler>` + `Arc<SubscriberMetrics>` so individual handlers don't pull the whole subscriber), spawn the `run()` task via `tokio::spawn`, store the `JoinHandle` for graceful shutdown.
3. The 4 NEW MCP handlers from sister `phase7-mcp-tools/` get `Arc<SubscriberMetrics>` + `Arc<MejepaStore>` injected through the existing `Handlers` context (extend the struct in `crates/context-graph-mcp/src/handlers/mod.rs` as needed).

**Cross-phase deps:** sister TASK-SHIFT-{007, 008, 009} provide the helpers; sister `phase7-mcp-tools/TASK-MCP-{004,005,006}` consume the subscriber Arc.

**FSV for this task:** integration test (a) starts the MCP server with a tempdir RocksDB + a synthetic JSONL fixture → invokes `mejepa_subscriber_status` → asserts `task_alive_since` populated and growing; (b) injects a panic via test-only hook → asserts `last_panic` populated and the supervisor respawns once; (c) sends SIGTERM → asserts graceful drain (in-flight shift completes before exit). Source of Truth: status endpoint readback + shift-log progression + RocksDB watermark CF.

---

<task_spec id="TASK-SHIFT-010" version="1.0">
<metadata>
  <title>Logic — ShiftSubscriber::run Tokio task entry point (1 dedicated CPU + 512 MB RSS)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>10</sequence>
  <implements>
    <item ref="REQ-SHIFT-01">Single Tokio task; 1 CPU + 512 MB RSS budget</item>
    <item ref="REQ-SHIFT-02">Restart hydration from CF_MEJEPA_SHIFT_WATERMARK</item>
    <item ref="REQ-SHIFT-03">Lag-alert evaluation each loop iteration</item>
    <item ref="EC-SHIFT-10">Supervisor catches panics; respawn-or-die per retry count</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#component_contracts[ShiftSubscriber][run]">behaviour</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#restart_recovery_diagram">restart sequence</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-005</task_ref>
    <task_ref>TASK-SHIFT-007</task_ref>
    <task_ref>TASK-SHIFT-008</task_ref>
    <task_ref>TASK-SHIFT-009</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (run.rs) + integration test</estimated_files>
</metadata>

<context>
The Tokio task entry point. Hydrates per-session resume points from `CF_MEJEPA_SHIFT_WATERMARK`; opens tail readers; enters main loop: poll_all → process_shift → write_watermark → evaluate_lag_alert → repeat. Listens for shutdown signal via `tokio::select!`. Supervisor (JoinSet) catches panics inside spawn_blocking encode calls per EC-SHIFT-10.
</context>

<input_context_files>
  <file purpose="component_contract">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#component_contracts[ShiftSubscriber][run]</file>
  <file purpose="restart_diagram">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#restart_recovery_diagram</file>
  <file purpose="implementation_notes">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (concurrency)</file>
  <file purpose="prior_logic">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-005..009.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-005..009 complete</check>
  <check>`tokio::select!` and `tokio::task::JoinSet` available</check>
</prerequisites>

<scope>
  <in_scope>
    - `pub async fn run(mut self, shutdown: tokio::sync::watch::Receiver<bool>) -> Result<(), SubscriberError>`
    - Startup: validate config, verify all 5 CFs exist, capture code_version, set `task_alive_since`, hydrate watermarks, open readers
    - Main loop: poll_all() → for each parsed entry, await process_shift; on Ok, write_watermark; on Err, log + skip-or-halt (LogParseFail → skip + advance past offset; WatermarkBackwards → halt; transient RocksDb → 3-retry; ObserveBatchNan → still advance watermark per ERR-SHIFT-06 recovery_action: persistence already happened)
    - Per-iteration: recompute lag, call `LagMonitor::evaluate`, on true return emit `tracing::warn!` for the alert
    - Shutdown handling via `tokio::select!`: on shutdown signal, finish in-flight shift, set task_alive_since = None, return Ok
    - Panic supervisor: wrap each `process_shift` call in `tokio::task::JoinSet::spawn`; catch JoinError::is_panic → populate `metrics.last_panic`; respawn up to 3 times on same shift; after 3rd panic on same shift → write `memory/blockers/` entry + return Err
    - Per-session retry counter (HashMap<ShiftId, u8>) cleared on successful process_shift
    - Integration test: spawn run() with 100-shift fixture, watch all consumed, watermark per-session populated, no double-process
    - Integration test: SIGKILL drill (drop the task mid-loop, restart, verify exactly-once on CF rows + at-least-once delivery)
  </in_scope>
  <out_of_scope>
    - MCP server spawning the task (lives in MCP startup wiring, sister phase)
    - Latency export (TASK-SHIFT-012)
    - FSV harness (TASK-SHIFT-013)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/run.rs">
      use tokio::sync::watch;
      use std::time::Duration;

      impl ShiftSubscriber {
          pub async fn run(
              mut self,
              shutdown: watch::Receiver&lt;bool&gt;,
          ) -&gt; Result&lt;(), SubscriberError&gt;;

          fn record_panic(&amp;self, info: PanicInfo);
          fn current_lag(&amp;self) -&gt; u64;
      }
    </signature>
  </signatures>

  <constraints>
    - Single Tokio task: process_shift is the only async fn awaited per iteration; no `tokio::spawn` of additional sibling tasks
    - Inner-shift parallelism (15-slot buffer_unordered) is fine; outer-shift serialization is the invariant
    - `max_concurrent_shifts: 1` enforced (Q-SHIFT-04 lock; per REQ-SHIFT-17)
    - Watermark advance is the LAST step of a successful shift (after process_shift returns Ok)
    - On `LogParseFail`, advance watermark past the bad byte_offset (so a fixed log can be re-tailed); on `WatermarkBackwards`, halt with the structured error and write a `memory/blockers/` entry
    - Provenance bundle assembled once at startup; embedded in every WatermarkRecord written
    - File length under 500 lines
  </constraints>

  <verification>
    - 100-shift integration test: all 100 consumed in order; final per-session watermarks correct; no double-process
    - SIGKILL drill (test simulates panic mid-process_shift, verifies on respawn the in-flight shift is reprocessed; final CF rows == 100)
    - Panic supervisor test: inject panic in encoder mock, verify last_panic populated, retry count increments, hard-fail after 3
    - Shutdown: signal shutdown → run() returns Ok within 1 in-flight shift's worth of time
  </verification>
</definition_of_done>

<pseudo_code>
run:
  self.config.validate()?;
  // hydrate
  let records = self.watermark_writer.read_all_watermarks()?;
  for rec in &records { self.tail.open_or_create_reader(&rec.session_id, rec.last_consumed_byte_offset).await?; }
  // discover new sessions not yet in watermark CF
  for sid in self.tail.discover_sessions().await? {
    if !records.iter().any(|r| r.session_id == sid) {
      self.tail.open_or_create_reader(&sid, 0).await?;
    }
  }
  *self.metrics.task_alive_since.write() = Some(now_unix_seconds());
  let mut retry_counts: HashMap<ShiftId, u8> = HashMap::new();
  loop {
    tokio::select! {
      biased;
      _ = shutdown.changed() => break,
      results = self.tail.poll_all() => {
        for r in results {
          match r {
            Ok((sid, entry)) => {
              match self.process_shift(entry.clone()).await {
                Ok(_outcome) => {
                  retry_counts.remove(&entry.shift_id);
                  let rec = WatermarkRecord{ session_id: sid, last_consumed_shift_id: entry.shift_id, last_consumed_byte_offset: entry.byte_offset + line_len(&entry), last_advanced_at_unix_seconds: now_unix_seconds(), provenance_corpus_sha: self.provenance.corpus_sha, provenance_code_version: self.provenance.code_version.clone() };
                  self.watermark_writer.write_watermark(&rec)?;
                }
                Err(SubscriberError::WatermarkBackwards{..}) as e => return Err(e),
                Err(SubscriberError::ObserveBatchNan{..}) as e => {
                  tracing::warn!(error_code = e.code(), "observe skipped; persistence already happened");
                  // advance watermark per ERR-SHIFT-06
                  // (same write_watermark as Ok branch)
                }
                Err(e) if let SubscriberError::RocksDbWriteFailed{retry, ..} = &e if *retry < 3 => {
                  // retry next iteration; do NOT advance watermark
                  tracing::warn!(error_code = e.code(), retry = *retry, "transient rocksdb error, retrying");
                }
                Err(e) => return Err(e),
              }
            }
            Err(SubscriberError::LogParseFail{ byte_offset, session_id, .. }) as e => {
              tracing::warn!(error_code = e.code(), "advancing past malformed line");
              // advance to byte_offset + 1 so next poll skips
              // (write watermark with byte_offset advanced; shift_id unchanged from prior good)
            }
            Err(e) => return Err(e),
          }
        }
        // lag eval
        let now = Instant::now();
        if self.lag_monitor.evaluate(&self.metrics, self.current_lag(), now) {
          tracing::warn!(error_code = "MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT", lag = self.current_lag(), "lag alert active");
        }
      }
    }
  }
  *self.metrics.task_alive_since.write() = None;
  Ok(())
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/run.rs">run() entry point + supervisor logic</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/run_integration.rs">100-shift consumption + SIGKILL drill + panic supervisor</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod run;`</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/subscriber.rs">add `pub(crate) lag_monitor: LagMonitor` + `pub(crate) tail: ShiftLogTail` + `pub(crate) watermark_writer: WatermarkWriter` fields</file>
</files_to_modify>

<validation_criteria>
  <criterion>100-shift fixture consumed exactly once</criterion>
  <criterion>SIGKILL drill: 1,000 shifts with 10 random kills → final CF rows == 1,000 (re-processed shifts overwrite atomically)</criterion>
  <criterion>Panic supervisor: 3rd panic on same shift surfaces structured blocker</criterion>
  <criterion>Shutdown signal cleanly terminates run() within one shift's budget</criterion>
  <criterion>WatermarkBackwards halts run()</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber run::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test run_integration</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
