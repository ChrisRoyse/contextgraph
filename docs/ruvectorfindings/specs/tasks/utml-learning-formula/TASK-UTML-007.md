# TASK-UTML-007 — `compute_l_step` (multiplicative ΔP × ΔK × ΔΩ × ΔΞ) + sampling_weight + drop/force gates

```xml
<task_spec id="TASK-UTML-007" version="1.0">

<metadata>
  <title>compute_l_step (multiplicative product clamped [0,1]) + sampling_weight (base · L · (1+5·override) · 0.995^age) + should_drop/should_force_include thresholds</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-UTML-01">L_step = ΔP × ΔK × ΔΩ × ΔΞ multiplicative; clamped [0,1]</item>
    <item ref="REQ-UTML-06">sampling_weight = base · L_step · (1+5·override) · decay^age; drop&lt;0.05; force&gt;1.0</item>
    <item ref="AC-UTML-01-01">Multiplicative product; deterministic byte-equal under same code_version</item>
    <item ref="AC-UTML-01-02">Any factor==0 → L_step==0 exactly</item>
    <item ref="AC-UTML-06-01..03">Sampling weight gates</item>
    <item ref="component:LStep">crates/context-graph-mejepa-train/src/learning_signal/l_step.rs</item>
    <item ref="component:SamplingWeight">crates/context-graph-mejepa-train/src/learning_signal/sampling_weight.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-003</task_ref>
    <task_ref>TASK-UTML-004</task_ref>
    <task_ref>TASK-UTML-005</task_ref>
    <task_ref>TASK-UTML-006</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>2 created</estimated_files>
</metadata>

<context>
The hot-path L_step product and the sampling_weight that consumes it. Per UTML §I multiplication is non-negotiable: any single factor at zero zeros the entire learning signal. Per UTML §6 the sampling_weight replaces doc 09 §3.7.2's older `(1 + 2·residual_error_z_score)` term — L_step already captures hard-example mining via ΔP AND enforces ΔΞ-safety + ΔΩ-safety + ΔK-coherence which residual-z-score did not.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 1)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-01, US-UTML-06; AC-UTML-01-01..03, AC-UTML-06-01..03; EC-UTML-01..06)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (components LStep + SamplingWeight)</file>
  <file purpose="prior_tasks">TASK-UTML-003.md, TASK-UTML-004.md, TASK-UTML-005.md, TASK-UTML-006.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-003..006 all complete (compute_delta_p/k/omega/xi all shipped)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/l_step.rs` with `compute_l_step(delta_p, delta_k, delta_omega, delta_xi) -&gt; f32`
    - Create `src/learning_signal/sampling_weight.rs` with `sampling_weight(l_step, override, age_days, decay) -&gt; f32`, `should_drop(weight) -&gt; bool`, `should_force_include(weight) -&gt; bool`, plus `DROP_THRESHOLD = 0.05`, `FORCE_THRESHOLD = 1.0`, `DEFAULT_DECAY = 0.995`, `DEFAULT_BASE_WEIGHT = 1.0`, `OVERRIDE_MULTIPLIER = 6.0` constants
    - compute_l_step is hot-path: validation in debug only; release fast-path; clamp post-product to [0,1] for f32 epsilon
    - sampling_weight: validate in debug; release fast-path; non-negative output; not clamped to [0,1] (override pushes above)
    - Property tests via proptest crate: 10,000 random (ΔP, ΔK, ΔΩ, ΔΞ) tuples in [0,1]^4 → output ∈ [0,1] equals product
    - Unit tests: each factor=0 → 0; all factors=1 → 1; deterministic byte-equal across two runs with same input; sampling_weight=0.04 → drop; weight=0.95 → keep; weight=1.05 → force; operator_override=true with L_step=0.167 + age=0 → weight=1.0+ → force; age decay 140-day half-life sanity check
  </in_scope>
  <out_of_scope>
    - BatchSampler (TRAINING phase TASK-TRAIN-005)
    - Cert chain hashing (TASK-UTML-007 stops at producing the LearningSignal struct; cert chain hashing happens in TASK-TRAIN-009)
    - Classifier (TASK-UTML-009)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/l_step.rs">
/// L_step = ΔP × ΔK × ΔΩ × ΔΞ; multiplicative gating per constitution &lt;utml_v1_invariants&gt; rule 1.
/// Hot-path: validation in debug only; release uses upstream gates (compute_delta_* return Result).
pub fn compute_l_step(delta_p: f32, delta_k: f32, delta_omega: f32, delta_xi: f32) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/sampling_weight.rs">
pub const DROP_THRESHOLD: f32 = 0.05;
pub const FORCE_THRESHOLD: f32 = 1.0;
pub const DEFAULT_BASE_WEIGHT: f32 = 1.0;
pub const DEFAULT_DECAY: f32 = 0.995;
pub const OVERRIDE_MULTIPLIER: f32 = 6.0; // (1 + 5·1)

pub fn sampling_weight(
    l_step: f32,
    operator_override: bool,
    age_days: f32,
    decay: f32, // default DEFAULT_DECAY
) -&gt; f32;

pub fn should_drop(weight: f32) -&gt; bool;
pub fn should_force_include(weight: f32) -&gt; bool;
    </signature>
  </signatures>
  <constraints>
    - compute_l_step debug_asserts each input ∈ [0, 1]; release does not assert (upstream `compute_delta_*` already returns Err on out-of-range)
    - Output clamped to [0, 1] post-product for f32 rounding (1.0 + 1e-7 → 1.0)
    - sampling_weight debug_asserts l_step ∈ [0,1], age_days &gt;= 0, decay ∈ (0,1)
    - sampling_weight = base * l_step * (override ? OVERRIDE_MULTIPLIER : 1.0) * decay.powf(age_days)
    - Output non-negative; NOT clamped to [0,1]
    - should_drop: weight &lt; 0.05 (strict)
    - should_force_include: weight &gt; 1.0 (strict)
    - Both files &lt; 250 lines
    - Property test must pass: forall (a,b,c,d) ∈ [0,1]^4, compute_l_step(a,b,c,d) ∈ [0,1] AND equals a*b*c*d after clamp
    - Determinism test: same inputs produce byte-equal outputs (call twice, compare bits)
  </constraints>
  <verification>
    - cargo test shows ≥ 12 tests pass across both files
    - proptest with 10,000 cases in [0,1]^4 passes
    - Determinism: f32::to_bits comparison stable across two runs
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_l_step(p: f32, k: f32, o: f32, x: f32) -&gt; f32 {
    debug_assert!((0.0..=1.0).contains(&amp;p), "delta_p={p} out of [0,1]");
    debug_assert!((0.0..=1.0).contains(&amp;k));
    debug_assert!((0.0..=1.0).contains(&amp;o));
    debug_assert!((0.0..=1.0).contains(&amp;x));
    (p * k * o * x).clamp(0.0, 1.0)
}

pub fn sampling_weight(l: f32, ovr: bool, age: f32, decay: f32) -&gt; f32 {
    debug_assert!((0.0..=1.0).contains(&amp;l));
    debug_assert!(age &gt;= 0.0);
    debug_assert!((0.0..1.0).contains(&amp;decay) &amp;&amp; decay &gt; 0.0);
    let mult = if ovr { OVERRIDE_MULTIPLIER } else { 1.0 };
    let recency = decay.powf(age);
    DEFAULT_BASE_WEIGHT * l * mult * recency
}

pub fn should_drop(w: f32) -&gt; bool { w &lt; DROP_THRESHOLD }
pub fn should_force_include(w: f32) -&gt; bool { w &gt; FORCE_THRESHOLD }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/l_step.rs">compute_l_step + tests</file>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/sampling_weight.rs">sampling_weight + should_drop + should_force_include + constants + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod l_step + pub mod sampling_weight + re-exports</file>
</files_to_modify>

<validation_criteria>
  <criterion>compute_l_step(0, 1, 1, 1) == 0 exactly</criterion>
  <criterion>compute_l_step(1, 1, 1, 1) == 1 exactly</criterion>
  <criterion>Property test 10,000 cases: l_step ∈ [0,1] forall inputs in [0,1]^4</criterion>
  <criterion>Determinism: two calls with same f32 bits produce same f32 bits</criterion>
  <criterion>sampling_weight(0, true, 0, 0.995) == 0; sampling_weight(0.5, true, 0, 0.995) == 3.0</criterion>
  <criterion>should_drop(0.04) true; should_drop(0.05) false; should_force_include(1.001) true; should_force_include(1.0) false</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib l_step</command>
  <command>cargo test -p context-graph-mejepa-train --lib sampling_weight</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
