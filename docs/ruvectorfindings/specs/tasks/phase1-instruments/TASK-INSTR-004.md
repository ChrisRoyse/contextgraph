# TASK-INSTR-004 — FrozenGuard primitive (MEJEPA_INSTR_FROZEN_VIOLATION)

```xml
<task_spec id="TASK-INSTR-004" version="1.0">

<metadata>
  <title>FrozenGuard primitive — backward-hook bug detector for ΔΞ.target_collapse > 0</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-INSTR-18">Zero trainable parameters; gradient detected = MEJEPA_INSTR_FROZEN_VIOLATION (CRITICAL)</item>
    <item ref="ERR-INSTR-05">Gradient through frozen parameters → halt training</item>
    <item ref="EC-INSTR-10">Maintenance PR exposes requires_grad=true → detected at runtime</item>
    <item ref="NFR-INSTR-06">Phase 3 hook reports zero ∂L/∂θ for every instrument across 1,000 training steps</item>
    <item ref="constitution:utml_v1_invariants rule 2">ΔΞ.target_collapse = 0 by construction</item>
    <item ref="constitution:anti_patterns frozen-instrument">No learnable target encoder</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INSTR-001</task_ref>
    <task_ref>TASK-INSTR-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>2 files created, 1 file modified</estimated_files>
</metadata>

<context>
Per constitution `<utml_v1_invariants>` rule 2 + `<anti_patterns>` frozen-instrument rule, every
instrument's underlying weights MUST be loaded with `requires_grad = false`. ANY code path that
produces ΔΞ.target_collapse > 0 = CRITICAL bug — an instrument has accidentally become trainable.

This task ships the **detection primitive** (the Phase 3 training loop will register it via the
`mejepa-train` crate when that lands). The `FrozenGuard` itself lives in
`context-graph-mejepa-instruments` so EVERY instrument can opt-in at registration time without a
training-time-only crate dependency. The Phase 3 training loop will consume the same primitive.

The primitive is a **structural assertion**, not a per-step check: it is registered once at the
start of training, and any non-zero gradient detected on any instrument's parameter tensor halts
the step + raises `MEJEPA_INSTR_FROZEN_VIOLATION`.

For the deterministic encoders shipped today (no candle tensors yet), the FrozenGuard is a
**stub** that simply records the registered instrument set + asserts no `Var` (candle's parameter
tensor type) is associated with any instrument. When neural-backed instruments land in
TASK-INSTR-008..018, those tasks register their `Var`s with the guard and the guard's
`assert_no_gradient_after_backward()` becomes load-bearing.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; anti_patterns frozen-instrument rule; failure_taxonomy)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INSTRUMENTS.md (REQ-INSTR-18, ERR-INSTR-05, EC-INSTR-10, NFR-INSTR-06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INSTRUMENTS.md (component_contracts FrozenGradientHook, frozen_invariant note, frozen_invariant_diagram)</file>
  <file purpose="shipped_trait">crates/context-graph-mejepa-instruments/src/lib.rs (Instrument trait)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §5.2 §11.2 (ΔΞ edge-case audit)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INSTR-001 + 002 verified</check>
  <check>candle-core 0.9.x available in workspace (already in Cargo.lock)</check>
</prerequisites>

<scope>
  <in_scope>
    - New module `crates/context-graph-mejepa-instruments/src/frozen_hook.rs`
    - `pub struct FrozenGuard { ... }` (registry + violation detector)
    - `pub fn register_instrument(&amp;mut self, slot: InstrumentSlot, params: &amp;[ParameterRef])`
    - `pub fn assert_zero_gradient_after_backward(&amp;self) -&gt; InstrumentResult&lt;()&gt;`
    - `pub fn frozen_violation(slot: InstrumentSlot, grad_norm: f32, param_id: &amp;str) -&gt; InstrumentError`
    - New error variant or sentinel: `MEJEPA_INSTR_FROZEN_VIOLATION`
    - Wire `FrozenGuard` into the `pub use` re-exports in `lib.rs`
    - Unit tests: register a fake-frozen instrument; call `assert_zero_gradient_after_backward()`; pass when no gradient; fail when gradient injected
    - Stub Tensor/ParameterRef abstraction: a thin wrapper that wraps either a candle `Var` (when neural backends are loaded) or a "no-op" placeholder for the deterministic encoders shipped today (so we can register all 15 instruments at startup with no panic)
  </in_scope>
  <out_of_scope>
    - Phase 3 training loop wiring (separate crate, future task)
    - Backward-pass execution (FrozenGuard is the assertion primitive; training loop calls it)
    - Tensor allocation (candle is a peer dependency; FrozenGuard does not allocate Vars)
    - Per-instrument neural integration (TASK-INSTR-008..018 register their Vars with the guard)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-instruments/src/frozen_hook.rs">
use crate::{Instrument, InstrumentError, InstrumentResult, InstrumentSlot};

/// A reference to a single parameter tensor that an instrument owns.
/// Wraps either a candle `Var` ID or a "no-op" placeholder for deterministic encoders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterRef {
    pub instrument_slot: InstrumentSlot,
    pub parameter_id: String,
    pub requires_grad: bool,  // MUST be false at registration; rechecked on assert
}

/// Registry of every instrument's parameter tensors. Lives for the entire
/// training run. Phase 3 training loop registers a single `FrozenGuard` and
/// invokes `assert_zero_gradient_after_backward` after every backward pass.
#[derive(Debug, Default)]
pub struct FrozenGuard {
    registered: Vec&lt;ParameterRef&gt;,
}

impl FrozenGuard {
    pub fn new() -&gt; Self;
    /// Register an instrument's parameter tensors. Returns InvalidInput if any
    /// parameter has requires_grad == true at registration time.
    pub fn register_instrument(
        &amp;mut self,
        slot: InstrumentSlot,
        params: &amp;[ParameterRef],
    ) -&gt; InstrumentResult&lt;()&gt;;
    /// Inspect every registered parameter; for each, look up the most recent
    /// gradient norm via `gradient_norm_lookup(param_id)`; if any is non-zero,
    /// return MEJEPA_INSTR_FROZEN_VIOLATION.
    pub fn assert_zero_gradient_after_backward&lt;F&gt;(
        &amp;self,
        gradient_norm_lookup: F,
    ) -&gt; InstrumentResult&lt;()&gt;
    where
        F: Fn(&amp;str) -&gt; f32;
    /// Number of registered parameters (for Phase 3 logging).
    pub fn registered_count(&amp;self) -&gt; usize;
}

/// Construct a CRITICAL `MEJEPA_INSTR_FROZEN_VIOLATION` error.
pub fn frozen_violation(slot: InstrumentSlot, grad_norm: f32, param_id: &amp;str) -&gt; InstrumentError;

pub const MEJEPA_INSTR_FROZEN_VIOLATION: &amp;str = "MEJEPA_INSTR_FROZEN_VIOLATION";
    </signature>
    <signature file="crates/context-graph-mejepa-instruments/src/lib.rs">
// Add: pub mod frozen_hook;
// Add re-export:
pub use frozen_hook::{
    FrozenGuard, ParameterRef, frozen_violation, MEJEPA_INSTR_FROZEN_VIOLATION,
};
    </signature>
  </signatures>
  <constraints>
    - File &lt; 500 lines (per constitution coding_standards file_organization)
    - `frozen_violation()` builds an `InstrumentError::InvalidInput` with field = "parameters", message containing the slot + grad_norm + param_id, remediation pointing to `requires_grad = false`
    - error code retrieved via `.code()` returns "MEJEPA_INSTRUMENTS_INVALID_INPUT" (existing code path); the SCREAMING_SNAKE `MEJEPA_INSTR_FROZEN_VIOLATION` is exposed as a separate `pub const` and embedded in the error message body so log scrapers can detect it
    - register_instrument refuses any ParameterRef with requires_grad == true → returns `InstrumentError::invalid("requires_grad", ..., "set requires_grad = false on every parameter before registration")`
    - assert_zero_gradient_after_backward iterates registered params; if any grad_norm > 0.0 (with finite check), surfaces `frozen_violation`
    - Deterministic: same registration order + same lookup function → same outcome
    - All existing 27+ tests continue to pass
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa-instruments`
    - `cargo test -p context-graph-mejepa-instruments frozen_guard`
    - `cargo clippy -p context-graph-mejepa-instruments -- -D warnings` clean
  </verification>
</definition_of_done>

<pseudo_code>
frozen_hook.rs:

  pub const MEJEPA_INSTR_FROZEN_VIOLATION: &amp;str = "MEJEPA_INSTR_FROZEN_VIOLATION";

  pub fn frozen_violation(slot: InstrumentSlot, grad_norm: f32, param_id: &amp;str) -&gt; InstrumentError {
      InstrumentError::invalid(
          "parameters",
          format!(
              "{}: instrument {:?} parameter {} has nonzero gradient {:.6}; this is a CRITICAL bug per constitution utml_v1_invariants rule 2",
              MEJEPA_INSTR_FROZEN_VIOLATION, slot, param_id, grad_norm,
          ),
          "halt training; set requires_grad = false; revert PR that exposed parameters as trainable",
      )
  }

  pub struct FrozenGuard { registered: Vec&lt;ParameterRef&gt; }

  impl FrozenGuard {
      pub fn register_instrument(&amp;mut self, slot: InstrumentSlot, params: &amp;[ParameterRef]) -&gt; InstrumentResult&lt;()&gt; {
          for p in params {
              if p.requires_grad {
                  return Err(InstrumentError::invalid("requires_grad",
                      format!("parameter {} on instrument {:?} has requires_grad = true at registration", p.parameter_id, slot),
                      "set requires_grad = false BEFORE registering with FrozenGuard"));
              }
              if p.instrument_slot != slot {
                  return Err(InstrumentError::invalid("instrument_slot",
                      format!("parameter {} declares slot {:?} but registered under {:?}", p.parameter_id, p.instrument_slot, slot),
                      "ensure ParameterRef.instrument_slot matches registration slot"));
              }
              self.registered.push(p.clone());
          }
          Ok(())
      }

      pub fn assert_zero_gradient_after_backward&lt;F&gt;(&amp;self, lookup: F) -&gt; InstrumentResult&lt;()&gt;
      where F: Fn(&amp;str) -&gt; f32 {
          for p in &amp;self.registered {
              let grad = lookup(&amp;p.parameter_id);
              if !grad.is_finite() {
                  return Err(InstrumentError::invariant("gradient_norm",
                      format!("parameter {} grad norm non-finite: {}", p.parameter_id, grad),
                      "investigate NaN/Inf in backward pass"));
              }
              if grad &gt; 0.0 {
                  return Err(frozen_violation(p.instrument_slot, grad, &amp;p.parameter_id));
              }
          }
          Ok(())
      }
  }

tests in frozen_hook.rs (#[cfg(test)] mod tests):
  - register_with_requires_grad_true_rejected
  - register_slot_mismatch_rejected
  - assert_zero_gradient_passes_when_all_zero
  - assert_zero_gradient_fails_with_violation_code_in_message
  - assert_zero_gradient_rejects_nan_norm
  - frozen_violation_message_includes_slot_grad_norm_param_id
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-instruments/src/frozen_hook.rs">FrozenGuard, ParameterRef, frozen_violation(), MEJEPA_INSTR_FROZEN_VIOLATION const, 6 unit tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-instruments/src/lib.rs">Add `pub mod frozen_hook;` and re-export `pub use frozen_hook::{FrozenGuard, ParameterRef, frozen_violation, MEJEPA_INSTR_FROZEN_VIOLATION};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>FrozenGuard rejects registration of any ParameterRef with requires_grad = true</criterion>
  <criterion>FrozenGuard rejects mismatched instrument_slot at registration</criterion>
  <criterion>assert_zero_gradient_after_backward returns Ok when all grads are 0.0</criterion>
  <criterion>assert_zero_gradient_after_backward returns InstrumentError whose Display contains "MEJEPA_INSTR_FROZEN_VIOLATION" when any grad > 0</criterion>
  <criterion>NaN/Inf in gradient lookup → NumericalInvariant</criterion>
  <criterion>cargo build --release -p context-graph-mejepa-instruments clean</criterion>
  <criterion>cargo clippy -p context-graph-mejepa-instruments -- -D warnings clean</criterion>
  <criterion>All shipped 27+ tests still pass</criterion>
  <criterion>frozen_hook.rs &lt; 500 lines</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-instruments</command>
  <command>cargo test -p context-graph-mejepa-instruments frozen_</command>
  <command>cargo clippy -p context-graph-mejepa-instruments -- -D warnings</command>
</test_commands>

</task_spec>
```
