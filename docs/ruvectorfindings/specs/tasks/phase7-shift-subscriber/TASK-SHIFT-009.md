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
