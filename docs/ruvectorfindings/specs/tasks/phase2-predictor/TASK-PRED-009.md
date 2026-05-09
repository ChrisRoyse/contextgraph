# TASK-PRED-009 — Frozen-target gradient detection (CRITICAL bug detector — `MEJEPA_PRED_FROZEN_TARGET_GRAD`)

```xml
<task_spec id="TASK-PRED-009" version="1.0">

<metadata>
  <title>Implement run_gradient_hook + GradientHookReport: detect any gradient flow into frozen instrument tensors and HALT training (CRITICAL severity)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>9</sequence>
  <implements>
    <item ref="REQ-PRED-10">Runtime gradient-hook detects ‖dL/dθ_instrument‖ > 1e-12 → MEJEPA_PRED_FROZEN_TARGET_GRAD CRITICAL</item>
    <item ref="REQ-PRED-09">Defense-in-depth runtime safety net behind the type-level guarantee from TASK-PRED-006</item>
    <item ref="component:GradientHook">grad_hook module + run_gradient_hook + InstrumentGradHandle</item>
  </implements>
  <depends_on>
    <task_ref>TASK-PRED-002</task_ref>
    <task_ref>TASK-PRED-003</task_ref>
    <task_ref>TASK-PRED-006</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified, 1 modified (predictor/mod.rs adds run_gradient_hook method)</estimated_files>
</metadata>

<context>
This is the runtime safety net for constitution `<utml_v1_invariants>` rule 2: `ΔΞ.target_collapse = 0 by construction`.
The type-level guarantee (TASK-PRED-006: `FrozenTargetAdapter::parameters()` returns the static
empty slice; the `Instrument` trait API forbids `&mut self` weight-mutating methods) covers the
common case. This task adds defense-in-depth: after `loss.backward()` and BEFORE
`optimizer.step()`, walk every instrument's reachable tensor handles, query the gradient store,
and assert `‖dL/dθ_instrument‖ < GRAD_NORM_NOISE_FLOOR` (1e-12 per TASK-PRED-001 const).

If any instrument tensor has a non-zero gradient, the hook returns
`PredictorError::FrozenTargetGrad { instrument_id, grad_norm, threshold, fix_at }`. This error
has `is_critical() == true` (per TASK-PRED-003). On detection:
  - The training step is aborted (the training loop is responsible for this; the hook just
    emits the Err).
  - No per-step training cert is written (Phase 3 training-loop concern).
  - A blocker entry is persisted to `memory/blockers/` (Phase 3 training-loop concern).
  - The implicated instrument's trait impl is flagged for trait-level fix.

This task implements ONLY the hook function + the supporting `InstrumentGradHandle` trait
extension on `Instrument`. The training-loop-side aborts and blocker-writing live in Phase 3.

Iron Rule observed: this task does NOT call MeJepaPredictor::forward (already shipped in
TASK-PRED-005), does NOT call vicreg_loss (TASK-PRED-007), does NOT write FSV files
(TASK-PRED-011).

**Implementation Reality**: candle 0.9.x's autograd model uses `GradStore` returned by
`Tensor::backward()`. To enumerate "instrument tensors with non-zero gradients", we need each
instrument to expose the `TensorId`s (or `Var` references) of any internal tensors that COULD
have been wired into the autograd graph. Since instruments are supposed to be fully detached
(per TASK-PRED-006 `encode_target` calls `.detach()`), the expected outcome is that the
GradStore contains NO entries for instrument tensor IDs. The hook iterates instrument-reported
TensorIds and queries `grad_store.get(&id)`; absence == clean (norm = 0); presence with norm
> 1e-12 == violation.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; failure_taxonomy MEJEPA_PRED_FROZEN_TARGET_GRAD; security_requirements SEC-MEJEPA-01; provenance — fix_at points to file:crates/...)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-PREDICTOR.md (REQ-PRED-09, REQ-PRED-10; AC-PRED-03-02; BR-PRED-01; ERR-PRED-04; EC-PRED-04 detach-then-reattach edge case)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-PREDICTOR.md (api_contracts MeJepaPredictor::run_gradient_hook; gradient_isolation_diagram; implementation_notes "frozen-target-architecture")</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-001.md (GRAD_NORM_NOISE_FLOOR const = 1e-12)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-003.md (PredictorError::FrozenTargetGrad variant + is_critical)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase2-predictor/TASK-PRED-006.md (FrozenTargetAdapter struct + parameters() returns &[])</file>
  <file purpose="upstream_trait">crates/context-graph-mejepa-instruments/src/lib.rs (Instrument trait — may need to extend with `fn instrument_tensor_ids(&self) -> Vec<TensorId>` for enumeration)</file>
  <file purpose="prior_files">crates/context-graph-mejepa/src/grad_hook.rs (empty placeholder from TASK-PRED-001)</file>
</input_context_files>

<prerequisites>
  <check>TASK-PRED-002 + TASK-PRED-003 + TASK-PRED-006 complete</check>
  <check>cargo build --release -p context-graph-mejepa succeeds</check>
  <check>candle_core::backprop::GradStore is accessible from this crate (candle 0.9.x)</check>
  <check>The Instrument trait either (a) already exposes a method for enumerating internal TensorIds, OR (b) accepts an additive method via this task's extension trait `InstrumentGradHandle`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `InstrumentGradHandle` extension trait in `grad_hook.rs` that augments `Instrument` with `fn instrument_tensor_ids(&self) -> Vec<TensorId>` (default impl returns empty Vec — instruments that don't wire any tensor into a graph are clean by default; instruments that DO hold candle Tensors override this)
    - Implement `pub fn run_gradient_hook(adapter: &FrozenTargetAdapter, grads: &GradStore) -> Result<GradientHookReport, PredictorError>` that walks every instrument's `instrument_tensor_ids()`, queries `grads.get(&id)`, computes l2 norm, and returns `Err(PredictorError::FrozenTargetGrad{...})` on any norm > GRAD_NORM_NOISE_FLOOR
    - Add `GradientHookReport` struct: `instruments_checked: usize`, `tensors_checked: usize`, `max_grad_norm: f64`, `instrument_with_max_norm: Option<String>` — for FSV evidence
    - Add `MeJepaPredictor::run_gradient_hook(&self, grads: &GradStore) -> Result<GradientHookReport, PredictorError>` that delegates to the free function with `&self.frozen_target_adapter`
    - Add 5 unit tests:
        (a) clean adapter (zero instruments) → Ok(report) with tensors_checked == 0
        (b) clean adapter with instruments that report empty tensor_ids → Ok(report)
        (c) ADVERSARIAL: synthetic instrument that exposes a TensorId for which GradStore has a non-zero gradient → Err(FrozenTargetGrad) with grad_norm > 1e-12
        (d) below-threshold gradient (norm = 1e-15, below 1e-12 noise floor) → Ok(report); record max_grad_norm
        (e) PredictorError::FrozenTargetGrad.is_critical() == true (cross-check from TASK-PRED-003)
  </in_scope>
  <out_of_scope>
    - Phase 3 training-loop integration (calling run_gradient_hook between backward() and step())
    - memory/blockers/ persistence on violation (Phase 3)
    - Operator alerting via the recommendation channel (Phase 3 / Phase 7)
    - The trait extension on the upstream `Instrument` trait — if the upstream crate needs a non-additive change, that's owned by Phase 1c TASK-INSTR-002. This task uses an extension trait pattern (additive, no upstream change) where possible.
    - VRAM measurement (TASK-PRED-010)
    - VICReg loss (TASK-PRED-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/grad_hook.rs">
use candle_core::backprop::GradStore;
use candle_core::TensorId;
use serde::Serialize;

use context_graph_mejepa_instruments::Instrument;

use crate::config::GRAD_NORM_NOISE_FLOOR;
use crate::error::PredictorError;
use crate::frozen_target::FrozenTargetAdapter;

/// Extension trait that lets an Instrument expose the candle TensorIds of any internal
/// tensor that COULD be wired into an autograd graph. Default impl returns an empty Vec —
/// instruments that hold no candle Tensors are clean by default.
pub trait InstrumentGradHandle {
    fn instrument_tensor_ids(&amp;self) -&gt; Vec&lt;TensorId&gt;;

    /// String label used in PredictorError::FrozenTargetGrad.instrument_id and FSV reports.
    fn instrument_label(&amp;self) -&gt; &amp;'static str;
}

/// Default impl: every Instrument has zero candle TensorIds unless overridden.
impl&lt;T: Instrument + ?Sized&gt; InstrumentGradHandle for T {
    default fn instrument_tensor_ids(&amp;self) -&gt; Vec&lt;TensorId&gt; { Vec::new() }
    default fn instrument_label(&amp;self) -&gt; &amp;'static str { "instrument" }
}

#[derive(Debug, Clone, Serialize)]
pub struct GradientHookReport {
    pub instruments_checked: usize,
    pub tensors_checked: usize,
    pub max_grad_norm: f64,
    pub instrument_with_max_norm: Option&lt;String&gt;,
    pub threshold: f64,
}

/// Runs the frozen-target gradient hook. Returns Ok(report) when every instrument tensor
/// has grad_norm &lt; GRAD_NORM_NOISE_FLOOR; returns Err(PredictorError::FrozenTargetGrad) on
/// the FIRST violation (fail-fast — a single violation is a CRITICAL bug; no need to
/// enumerate further violations in a single hook call).
pub fn run_gradient_hook(
    adapter: &amp;FrozenTargetAdapter,
    grads: &amp;GradStore,
) -&gt; Result&lt;GradientHookReport, PredictorError&gt;;

/// Variant that returns ALL violations (instead of fail-fast on first) — for adversarial
/// FSV that wants to enumerate every tap.
pub fn run_gradient_hook_full(
    adapter: &amp;FrozenTargetAdapter,
    grads: &amp;GradStore,
) -&gt; (GradientHookReport, Vec&lt;PredictorError&gt;);
    </signature>
    <signature file="crates/context-graph-mejepa/src/predictor/mod.rs">
use candle_core::backprop::GradStore;

use crate::grad_hook::{run_gradient_hook, GradientHookReport};

impl MeJepaPredictor {
    /// Defense-in-depth gradient hook (REQ-PRED-10). Called by the Phase 3 training loop
    /// AFTER `loss.backward()` and BEFORE `optimizer.step()`. On Err, the training loop
    /// MUST skip optimizer.step() AND skip writing the per-step training cert.
    pub fn run_gradient_hook(&amp;self, grads: &amp;GradStore) -&gt; Result&lt;GradientHookReport, PredictorError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - The extension trait `InstrumentGradHandle` MUST use `default fn` (specialization) OR a non-specialization fallback pattern (manual blanket impl with no upstream-trait method) — pick whichever compiles cleanly on stable Rust 1.84+. Specialization (`feature(specialization)`) is unstable; **prefer the non-specialization fallback**: declare a blanket `impl<T: Instrument + ?Sized> InstrumentGradHandle for T` that returns empty (no `default fn`); instruments that need to override write `impl InstrumentGradHandle for MyConcreteInstrument` overriding the blanket via the standard trait-resolution rules. This sidesteps unstable features.
    - **WAIT** — Rust does not allow non-specialization blanket impl + concrete override (orphan rule + coherence). Alternative pattern: make `InstrumentGradHandle` a NEW trait that callers manually implement on each `Box<dyn Instrument>` wrapper; the FrozenTargetAdapter holds `Vec<Box<dyn InstrumentGradHandle + Send + Sync>>` via a thin wrapper struct, OR we redesign to enumerate via a parallel `Vec<Vec<TensorId>>` registry kept inside the adapter. **Adopt the parallel-registry approach**: `FrozenTargetAdapter` carries an additional field `instrument_grad_handles: Vec<Box<dyn InstrumentGradHandle + Send + Sync>>` that the constructor populates side-by-side with `instruments` (the caller passes both). Default for instruments without grad handles is to pass `Box::new(NoOpGradHandle::with_label("e_oracle"))` etc.
    - To minimize TASK-PRED-006's churn, this task **adds an additive constructor** `FrozenTargetAdapter::with_grad_handles(instruments, grad_handles, provenance)` and a backward-compatible `FrozenTargetAdapter::new(...)` that auto-populates a Vec of `NoOpGradHandle`. Update the data-models constraint section accordingly (a one-line edit to `frozen_target.rs`).
    - **Norm computation**: `let g = grads.get(&id);` returns `Option<&Tensor>`. Compute `g.sqr()?.sum_all()?.to_scalar::<f32>()?.sqrt() as f64` to obtain l2 norm. Cast to f64 for comparison against GRAD_NORM_NOISE_FLOOR (1e-12 f64).
    - **fix_at**: `format!("file:crates/context-graph-mejepa-instruments/src/{}.rs", instrument_label)` — provides operator-actionable file path per constitution `<file_organization>` `Source-of-truth file paths in errors and recommendations always start with file:` rule
    - **Threshold**: `GRAD_NORM_NOISE_FLOOR = 1e-12` (TASK-PRED-001 const)
    - **fail-fast** in `run_gradient_hook`: return on first violation
    - **enumerate** in `run_gradient_hook_full`: collect ALL violations into Vec<PredictorError> (used by FSV adversarial in TASK-PRED-011)
    - GradientHookReport derives Serialize for FSV evidence (TASK-PRED-011 will read it)
    - File length stays &lt; 500 lines
    - No `unwrap()` outside test code
  </constraints>
  <verification>
    - cargo build --release -p context-graph-mejepa succeeds with zero warnings
    - cargo test -p context-graph-mejepa shows >= 56 tests passing (this task adds 5)
    - cargo clippy -p context-graph-mejepa -- -D warnings clean
    - Test (cuda-gated): clean FrozenTargetAdapter (empty_for_test) + empty GradStore → Ok(report) with instruments_checked == 0
    - Test: FrozenTargetAdapter with NoOpGradHandle instruments + empty GradStore → Ok(report) with tensors_checked == 0
    - Test: synthetic InstrumentGradHandle reporting a TensorId for which GradStore has gradient norm 1.0 → Err(PredictorError::FrozenTargetGrad{grad_norm ≈ 1.0, threshold = 1e-12, ..})
    - Test: synthetic gradient norm 1e-15 (below noise floor) → Ok(report) with max_grad_norm ≈ 1e-15
    - Test: PredictorError::FrozenTargetGrad{...}.is_critical() == true (cross-spec contract)
    - Test: run_gradient_hook_full returns Vec of all violations (≥ 1 when synthetic violation present)
  </verification>
</definition_of_done>

<pseudo_code>
grad_hook.rs:
    pub trait InstrumentGradHandle {
        fn instrument_tensor_ids(&self) -&gt; Vec&lt;TensorId&gt;;
        fn instrument_label(&self) -&gt; &amp;'static str;
    }

    pub struct NoOpGradHandle { label: &amp;'static str }
    impl NoOpGradHandle { pub fn with_label(label: &amp;'static str) -&gt; Box&lt;dyn InstrumentGradHandle + Send + Sync&gt; { Box::new(Self { label }) } }
    impl InstrumentGradHandle for NoOpGradHandle {
        fn instrument_tensor_ids(&self) -&gt; Vec&lt;TensorId&gt; { Vec::new() }
        fn instrument_label(&self) -&gt; &amp;'static str { self.label }
    }

    pub struct GradientHookReport { instruments_checked, tensors_checked, max_grad_norm, instrument_with_max_norm, threshold }

    pub fn run_gradient_hook(adapter, grads) -&gt; Result&lt;GradientHookReport, PredictorError&gt; {
        let mut report = GradientHookReport {
            instruments_checked: adapter.instrument_grad_handles().len(),
            tensors_checked: 0,
            max_grad_norm: 0.0,
            instrument_with_max_norm: None,
            threshold: GRAD_NORM_NOISE_FLOOR,
        };
        for h in adapter.instrument_grad_handles().iter() {
            for id in h.instrument_tensor_ids() {
                report.tensors_checked += 1;
                if let Some(g) = grads.get(&id) {
                    let norm_f32 = g.sqr()?.sum_all()?.to_scalar::&lt;f32&gt;()?;
                    let norm = (norm_f32 as f64).sqrt();
                    if norm &gt; report.max_grad_norm {
                        report.max_grad_norm = norm;
                        report.instrument_with_max_norm = Some(h.instrument_label().to_string());
                    }
                    if norm &gt; GRAD_NORM_NOISE_FLOOR {
                        return Err(PredictorError::FrozenTargetGrad {
                            instrument_id: h.instrument_label().to_string(),
                            grad_norm: norm,
                            threshold: GRAD_NORM_NOISE_FLOOR,
                            fix_at: format!("file:crates/context-graph-mejepa-instruments/src/{}.rs", h.instrument_label()),
                        });
                    }
                }
            }
        }
        Ok(report)
    }

    pub fn run_gradient_hook_full(adapter, grads) -&gt; (GradientHookReport, Vec&lt;PredictorError&gt;) {
        // Same enumeration but collect all violations
        let mut violations = Vec::new();
        let mut report = ...;
        for h in adapter.instrument_grad_handles() {
            for id in h.instrument_tensor_ids() {
                report.tensors_checked += 1;
                if let Some(g) = grads.get(&id) {
                    let norm = compute_norm(g)?;  // map ? to logging-and-skip in this enumeration variant
                    if norm &gt; report.max_grad_norm { ... }
                    if norm &gt; GRAD_NORM_NOISE_FLOOR {
                        violations.push(PredictorError::FrozenTargetGrad { ... });
                    }
                }
            }
        }
        (report, violations)
    }

predictor/mod.rs:
    impl MeJepaPredictor::run_gradient_hook(&self, grads) -&gt; Result&lt;GradientHookReport, PredictorError&gt; {
        run_gradient_hook(&self.frozen_target_adapter, grads)
    }

frozen_target.rs (small additive edit):
    add field instrument_grad_handles: Vec&lt;Box&lt;dyn InstrumentGradHandle + Send + Sync&gt;&gt; to FrozenTargetAdapter
    update new() to auto-populate with NoOpGradHandle::with_label per instrument
    add with_grad_handles() constructor that takes both instruments and grad_handles explicitly
    add accessor instrument_grad_handles(&self) -&gt; &[Box&lt;...&gt;]

tests:
    test_run_gradient_hook_clean_empty_adapter
    test_run_gradient_hook_with_noop_handles_empty_gradstore
    test_run_gradient_hook_synthetic_violation_norm_one_returns_err
    test_run_gradient_hook_below_noise_floor_returns_ok
    test_predictor_error_frozen_target_grad_is_critical
    test_run_gradient_hook_full_collects_all_violations
</pseudo_code>

<files_to_create>
  <!-- none; modifying placeholder -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/grad_hook.rs">Replace empty placeholder with InstrumentGradHandle trait + NoOpGradHandle + GradientHookReport + run_gradient_hook + run_gradient_hook_full + 5+ tests</file>
  <file path="crates/context-graph-mejepa/src/frozen_target.rs">Add instrument_grad_handles field + with_grad_handles constructor + accessor; update new() to auto-populate NoOpGradHandle wrappers (additive; existing tests keep passing)</file>
  <file path="crates/context-graph-mejepa/src/predictor/mod.rs">Add MeJepaPredictor::run_gradient_hook delegating to free function</file>
  <file path="crates/context-graph-mejepa/src/lib.rs">`pub use grad_hook::{run_gradient_hook, run_gradient_hook_full, GradientHookReport, InstrumentGradHandle};`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa shows >= 56 tests passing</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings is clean</criterion>
  <criterion>Synthetic instrument with grad norm 1.0 → Err(PredictorError::FrozenTargetGrad{grad_norm ≈ 1.0, threshold == 1e-12, fix_at starts with "file:crates/..."})</criterion>
  <criterion>Below-noise-floor gradient (1e-15) → Ok(report) with max_grad_norm captured</criterion>
  <criterion>Clean FrozenTargetAdapter::empty_for_test() + GradStore::default() → Ok(report) with instruments_checked = tensors_checked = 0</criterion>
  <criterion>PredictorError::FrozenTargetGrad{...}.is_critical() == true (cross-checked via TASK-PRED-003 contract)</criterion>
  <criterion>run_gradient_hook is fail-fast (returns on FIRST violation)</criterion>
  <criterion>run_gradient_hook_full enumerates all violations</criterion>
  <criterion>FrozenTargetAdapter::new still works (additive constructor; existing tests unmodified)</criterion>
  <criterion>fix_at field starts with `file:` prefix per constitution `<file_organization>`</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa grad_hook</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
