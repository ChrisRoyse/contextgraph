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

- ✅ **Crate `context-graph-mejepa-shift-subscriber` does NOT exist yet** — `LagMonitor` lands in the same not-yet-existing crate.
- ✅ `SubscriberMetrics`, `LagAlertState`, `LagAlertStateTag` declarations come from TASK-SHIFT-002 in the same crate.
- ✅ Atomic state transitions via `AtomicU8::compare_exchange_weak` are the documented invariant; no replacement primitive shipped elsewhere.
- ✅ Lag threshold + sustain knobs are part of `MeJepaShiftSubscriberConfig` (TASK-SHIFT-001), with bounds checked at construction.
- No drift in `<files_to_create>` paths.

<task_spec id="TASK-SHIFT-009" version="1.0">
<metadata>
  <title>Logic — Backpressure + lag-alert state machine (>500 / >60s)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-SHIFT-03">Lag alerting at 500 unprocessed shifts sustained > 1 minute</item>
    <item ref="ERR-SHIFT-01">MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT informational</item>
    <item ref="EC-SHIFT-07">Sustained-window prevents alert flapping</item>
    <item ref="BR-SHIFT-07">Continue consuming; never drop shifts</item>
    <item ref="TECH-SHIFT-SUBSCRIBER#lag_alert_state_machine">5-state diagram</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-002</task_ref>
    <task_ref>TASK-SHIFT-007</task_ref>
    <task_ref>TASK-SHIFT-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (lag.rs) + tests</estimated_files>
</metadata>

<context>
Implements the 5-state lag alert state machine: Idle → Watching → Trigger → AlertActive → AlertClearing → Watching. Lag = sum of unprocessed shifts across all sessions, recomputed each poll. Sustained-window timer prevents flapping at the boundary. Alert emits exactly once per Idle→AlertActive transition (atomic compare_exchange on `LagAlertStateTag` u8). Emission is informational only — consumption never pauses.
</context>

<input_context_files>
  <file purpose="state_machine">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#lag_alert_state_machine</file>
  <file purpose="metrics_skeleton">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-002.md</file>
  <file purpose="config_knobs">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-001.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-002 + 007 + 008 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - `crates/context-graph-mejepa-shift-subscriber/src/lag.rs` — `LagMonitor` with `evaluate(current_lag, now)` advancing the state machine
    - `evaluate_lag_alert_state(&mut self)` method on `ShiftSubscriber` calling `LagMonitor`
    - Per-session lag computation: `(producer file size in JSONL lines) - (last consumed offset's line index)` — we approximate by `(last_byte_size - watermark_byte_offset) / avg_line_bytes` where avg_line_bytes is a running estimate
    - Atomic `compare_exchange` on `lag_alert_state_tag` (AtomicU8) for race-free transitions
    - Emit log entry exactly once per Idle→AlertActive transition (idempotent via state machine)
    - Tests: lag rises → Watching; sustains > 60s above threshold → AlertActive once; drops → AlertClearing; sustains below for 60s → Watching/Idle
    - Test: oscillation around threshold for &lt; 60s never triggers AlertActive
  </in_scope>
  <out_of_scope>
    - Producer rate-limit (out of scope; producer is unbounded)
    - Auto-scaling (out of scope; alert is informational; operator decides)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/lag.rs">
      use std::time::{Duration, Instant};
      use crate::metrics::{LagAlertState, LagAlertStateTag, SubscriberMetrics};

      pub struct LagMonitor {
          threshold_shifts: usize,
          sustain_window: Duration,
      }

      impl LagMonitor {
          pub fn new(threshold_shifts: usize, sustain_window: Duration) -&gt; Self;

          // Returns true if a transition into AlertActive happened on THIS evaluation
          // (caller emits MEJEPA_SHIFT_SUBSCRIBER_LAG_ALERT once per true return).
          pub fn evaluate(
              &amp;self,
              metrics: &amp;SubscriberMetrics,
              current_lag: u64,
              now: Instant,
          ) -&gt; bool;
      }
    </signature>
  </signatures>

  <constraints>
    - State machine transitions atomic via `compare_exchange_weak` on `lag_alert_state_tag`
    - The `LagAlertState::Trigger { since: Instant }` and `AlertClearing { since: Instant }` variants store wall time; protected by `metrics.lag_alert_state_full: RwLock<LagAlertState>`
    - Alert emission is the CALLER's responsibility (this module returns bool); caller logs `tracing::warn!` with structured fields
    - Lag computation must NEVER block the main loop (uses atomic snapshots only)
    - File length under 500 lines
  </constraints>

  <verification>
    - Test 1: lag = 600, t = 0..120s → at t=60s `evaluate` returns true exactly once
    - Test 2: lag oscillates 400 → 600 → 400 every 5s → `evaluate` returns false every time (Trigger collapses back to Watching before 60s)
    - Test 3: lag=600 sustained 60s → AlertActive; lag drops to 100 sustained 60s → AlertClearing → Watching
    - Test 4: 16-thread concurrent evaluate stress (random lag values) → no double-emit; state remains consistent
    - `cargo clippy` clean
  </verification>
</definition_of_done>

<pseudo_code>
evaluate(metrics, lag, now):
  let tag = metrics.lag_alert_state_tag.load(Ordering::Acquire);
  if lag > self.threshold_shifts as u64 {
    match tag {
      Idle | Watching => transition_to(Trigger{since: now}); false
      Trigger{since} => {
        if now - since >= self.sustain_window {
          transition_to(AlertActive); true       // alert ONCE
        } else { false }
      }
      AlertActive => false
      AlertClearing{..} => transition_to(AlertActive); false
    }
  } else { /* below threshold */
    match tag {
      Idle | Watching => false
      Trigger{..} => transition_to(Watching); false
      AlertActive => transition_to(AlertClearing{since: now}); false
      AlertClearing{since} => {
        if now - since >= self.sustain_window {
          transition_to(Watching); false
        } else { false }
      }
    }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lag.rs">LagMonitor + state machine</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/lag_drills.rs">3 sustained-vs-flapping scenarios</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod lag; pub use lag::LagMonitor;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>Sustained > 60s triggers AlertActive exactly once per Idle→AlertActive transition</criterion>
  <criterion>Flapping &lt; 60s never triggers AlertActive</criterion>
  <criterion>Drop sustained 60s clears alert (AlertClearing → Watching)</criterion>
  <criterion>Concurrent stress test: no double-emit, no torn state</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber lag::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test lag_drills</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
