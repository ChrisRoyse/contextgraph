<task_spec id="TASK-SHIFT-011" version="1.0">
<metadata>
  <title>Logic — EWC++-regularized observe() call site (idempotent against duplicate / out-of-order shifts)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-SHIFT-06">observe() call site (cross-cutting with SPEC-SELF-HEALING)</item>
    <item ref="REQ-SHIFT-11">Idempotent shift replay; replay flag in train cert</item>
    <item ref="BR-SHIFT-08">Replay does NOT roll watermark backward; observe side-effects de-duplicated by EWC++ Fisher accumulation</item>
    <item ref="EC-SHIFT-12">Concurrent replay serialized by max_concurrent_shifts: 1</item>
    <item ref="implementation_notes[idempotency]">EWC++ tolerates duplicate observe</item>
  </implements>
  <depends_on>
    <task_ref>TASK-SHIFT-007</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 file (observe.rs) + tests</estimated_files>
</metadata>

<context>
Wires the call site for `predictor.observe()` with the discipline that out-of-order or duplicate shifts MUST NOT cause catastrophic forgetting. The actual EWC++ regularization body lives in SPEC-SELF-HEALING; this task ensures (1) the cert chain records `replay: true` on replay, (2) the call is serialized via `max_concurrent_shifts: 1` per REQ-SHIFT-17, (3) duplicate observe of the same `(panel_t1, panel_t2, oracle)` is harmless because Fisher information accumulation is monotone. Adds a `ReplayContext` parameter so `mejepa_observe_shift` (sister phase) can opt into replay mode without rolling the watermark.
</context>

<input_context_files>
  <file purpose="cross_cut_self_healing">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (cross_cutting_self_healing row)</file>
  <file purpose="idempotency_note">docs/ruvectorfindings/specs/technical/TECH-SHIFT-SUBSCRIBER.md#implementation_notes (idempotency row)</file>
  <file purpose="replay_contract">docs/ruvectorfindings/specs/functional/SPEC-SHIFT-SUBSCRIBER.md (REQ-SHIFT-11, BR-SHIFT-08)</file>
  <file purpose="process_shift_body">docs/ruvectorfindings/specs/tasks/phase7-shift-subscriber/TASK-SHIFT-007.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-SHIFT-007 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Add `ReplayContext { pub replay: bool, pub original_shift_id: Option<ShiftId> }` carrier
    - Refactor `process_shift` to accept `Option<ReplayContext>` (default None = first-time)
    - When `replay: true`:
      - watermark advance is SUPPRESSED (caller's policy: TASK-SHIFT-010 only advances on `replay: None`)
      - train cert sampling_decision becomes `ReplayObserved` instead of `Observed`
      - `last_panic.last_in_flight_shift` field updated with `original_shift_id` for audit on subsequent panic
    - Document the EWC++ idempotency invariant in module doc comments: "duplicate observe is structurally safe because EWC++ Fisher accumulation is monotone — re-observing the same (panel_t1, panel_t2, oracle) tuple cannot cause catastrophic forgetting; the worst case is a slightly over-weighted Fisher diagonal entry"
    - Tests:
      - 1× observe vs 5× observe of same panel triple → predictor.observe called 5×; live_prediction CF row overwritten 5× (atomic; final state byte-equal)
      - Out-of-order test: shift A (oracle present) at t=10s, shift B (no oracle) at t=20s, then `mejepa_observe_shift(A.shift_id)` at t=30s → final CF state has B's prediction at B's offset and A's replayed prediction at A's offset (both keys distinct); train cert chain has 1× Observed for A, 1× ReplayObserved for A; watermark unchanged (still at B's shift_id)
  </in_scope>
  <out_of_scope>
    - The actual EWC++ Fisher math (SPEC-SELF-HEALING)
    - The `mejepa_observe_shift` MCP handler (TASK-MCP-004)
    - Drift detection (SPEC-SELF-HEALING / SPEC-EVALUATION)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/observe.rs">
      use crate::entry::ShiftId;
      use crate::traits::SamplingDecision;

      #[derive(Debug, Clone)]
      pub struct ReplayContext {
          pub replay: bool,
          pub original_shift_id: Option&lt;ShiftId&gt;,
      }
      impl Default for ReplayContext { fn default() -&gt; Self { Self{ replay: false, original_shift_id: None } } }

      impl ReplayContext {
          pub fn for_new_shift() -&gt; Self;
          pub fn for_replay(original_shift_id: ShiftId) -&gt; Self;
          pub fn sampling_decision_when_observed(&amp;self) -&gt; SamplingDecision; // Observed | ReplayObserved
      }
    </signature>
    <signature file="crates/context-graph-mejepa-shift-subscriber/src/process.rs (modified)">
      impl ShiftSubscriber {
          // Replaces TASK-SHIFT-007's signature
          pub async fn process_shift_with_context(
              &amp;mut self,
              shift: ShiftEntry,
              replay_ctx: ReplayContext,
          ) -&gt; Result&lt;ShiftOutcome, SubscriberError&gt;;

          // Convenience wrapper (preserves the TASK-SHIFT-007 entry point)
          pub async fn process_shift(
              &amp;mut self,
              shift: ShiftEntry,
          ) -&gt; Result&lt;ShiftOutcome, SubscriberError&gt; {
              self.process_shift_with_context(shift, ReplayContext::default()).await
          }
      }
    </signature>
  </signatures>

  <constraints>
    - `process_shift` (no replay arg) MUST behave bit-identically to TASK-SHIFT-007 — the refactor adds a parameter without changing default semantics
    - Replay's watermark suppression is enforced in TASK-SHIFT-010's `run()` loop (which only writes watermark on `replay: false` paths); no change here besides surfacing the flag
    - `ReplayObserved` sampling_decision is a new SamplingDecision variant added in TASK-SHIFT-007's traits.rs (or extend here)
    - Module doc-comment cites the EWC++ Fisher monotonicity property with reference to doc 11 §5.2 / §11
    - File length under 500 lines
  </constraints>

  <verification>
    - 5× replay test: predictor.observe called 5×; live_prediction CF has exactly 1 row (5 atomic overwrites); train cert chain has 5 ReplayObserved entries
    - Out-of-order test as in scope
    - Default-context test: `process_shift(entry)` behaves identically to TASK-SHIFT-007 (regression sentinel)
    - `cargo clippy` clean
  </verification>
</definition_of_done>

<pseudo_code>
process_shift_with_context(shift, replay_ctx):
  // Same body as TASK-SHIFT-007 EXCEPT:
  // - When constructing TrainCert: replay = replay_ctx.replay; sampling_decision = if observe_invoked { replay_ctx.sampling_decision_when_observed() } else { DroppedLStepBelowThreshold }
  // - On panic, populate last_panic.last_in_flight_shift with replay_ctx.original_shift_id.or(shift.shift_id.clone())

ReplayContext::sampling_decision_when_observed:
  if self.replay { SamplingDecision::ReplayObserved } else { SamplingDecision::Observed }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/observe.rs">ReplayContext type</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/tests/replay_drills.rs">5× replay + out-of-order tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/process.rs">refactor process_shift to delegate to process_shift_with_context</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/traits.rs">add ReplayObserved variant to SamplingDecision (if not already present)</file>
  <file path="crates/context-graph-mejepa-shift-subscriber/src/lib.rs">`pub mod observe; pub use observe::ReplayContext;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>5× replay: 5 cert entries, 1 final CF row</criterion>
  <criterion>Default `process_shift(entry)` regression-equivalent to TASK-SHIFT-007</criterion>
  <criterion>Out-of-order test: replay watermark NEVER rolled back</criterion>
  <criterion>EWC++ idempotency property documented in module-level doc comment</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-shift-subscriber</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber observe::</command>
  <command>cargo test -p context-graph-mejepa-shift-subscriber --test replay_drills</command>
  <command>cargo clippy -p context-graph-mejepa-shift-subscriber -- -D warnings</command>
</test_commands>
</task_spec>
