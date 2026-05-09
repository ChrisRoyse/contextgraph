# TASK-UTML-006 — `compute_delta_xi` (CRITICAL `target_collapse == 0.0` assertion)

```xml
<task_spec id="TASK-UTML-006" version="1.0">

<metadata>
  <title>compute_delta_xi (target_collapse, predictor_redundancy, constellation_violation_rate) returning (1−tc)^0.5 · (1−red)^0.3 · (1−viol)^0.2 with hard target_collapse==0.0 assertion (MEJEPA_INSTRUMENT_GRADIENT_LEAK on violation)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-UTML-05">ΔΞ representational non-collapse; target_collapse asserted == 0</item>
    <item ref="AC-UTML-05-01">Geometric exponents 0.5/0.3/0.2; first factor locked at 1.0 always</item>
    <item ref="AC-UTML-05-02">target_collapse&gt;0 → MEJEPA_INSTRUMENT_GRADIENT_LEAK CRITICAL fail-closed</item>
    <item ref="AC-UTML-05-03">predictor_redundancy &gt; 0.5 → trigger VICReg bump (downstream signal)</item>
    <item ref="AC-UTML-05-04">constellation_violation_rate from Phase 4b Gτ; &gt;5% triggers re-fit (downstream signal)</item>
    <item ref="EC-UTML-04">target_collapse&gt;0 is the central architectural integrity check</item>
    <item ref="ERR-UTML-02">MEJEPA_INSTRUMENT_GRADIENT_LEAK fail-closed</item>
    <item ref="component:DeltaXi">crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
ΔΞ is THE central architectural integrity check of ME-JEPA. The frozen target panel is produced by externally-defined instruments with zero trainable parameters (constitution &lt;anti_patterns&gt; "no learnable target encoder"). target_collapse is therefore zero by construction. The assertion `target_collapse == 0.0` is the runtime measurement of that structural property: any nonzero value indicates an instrument has accidentally become trainable (gradient flow through what should be a frozen function), which is a CRITICAL bug requiring immediate halt and operator inspection.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule 2; anti_patterns no learnable target encoder; failure_taxonomy MEJEPA_INSTRUMENT_GRADIENT_LEAK is implicitly added to extend taxonomy)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-05; AC-UTML-05-01..05; EC-UTML-04; ERR-UTML-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component DeltaXi)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §6</file>
  <file purpose="prior_tasks">TASK-UTML-001.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/delta_xi.rs` with `compute_delta_xi(target_collapse, predictor_redundancy, constellation_violation_rate) -&gt; Result&lt;f32, UtmlError&gt;`
    - Validate redundancy in [0, 1] AND violation in [0, 1]; UtmlFactorOutOfRange otherwise
    - HARD CHECK: `if target_collapse != 0.0 { return Err(MejepaInstrumentGradientLeak with full diagnostic) }` (any nonzero is a critical bug; no epsilon tolerance)
    - In debug builds additionally: `debug_assert_eq!(target_collapse, 0.0, "ME-JEPA architectural integrity violation: target_collapse must be 0.0 by construction; an instrument has accidentally become trainable")`
    - first_factor = (1.0 - target_collapse).powf(0.5) — locked at exactly 1.0 always
    - second_factor = (1.0 - redundancy).powf(0.3)
    - third_factor = (1.0 - violation).powf(0.2)
    - delta_xi = clamp(first*second*third, 0.0, 1.0)
    - Unit tests: target_collapse=0, redundancy=0.1, violation=0.05 → ≈ 0.964; target_collapse=0.001 → fail-closed; target_collapse=NaN → fail-closed; redundancy out of [0,1] → fail-closed
    - Provide a `pub const TARGET_COLLAPSE_TOLERANCE: f32 = 0.0;` (DOCUMENT that no epsilon tolerance is allowed)
  </in_scope>
  <out_of_scope>
    - Predictor redundancy computation (TASK-UTML-008 pairwise MI audit)
    - Constellation Gτ rejection rate (Phase 4b TCT)
    - VICReg bump intervention (Phase 5)
    - L_step assembly (TASK-UTML-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs">
use crate::error::{UtmlError, UtmlErrorCode};
use serde_json::json;

/// ME-JEPA architectural invariant: target_collapse MUST be exactly 0.0 by construction.
/// No epsilon tolerance; any nonzero value is a CRITICAL bug indicator.
pub const TARGET_COLLAPSE_TOLERANCE: f32 = 0.0;

pub fn compute_delta_xi(
    target_collapse: f32,
    predictor_redundancy: f32,
    constellation_violation_rate: f32,
) -&gt; Result&lt;f32, UtmlError&gt;;
    </signature>
  </signatures>
  <constraints>
    - target_collapse comparison is EXACT (`!= 0.0`), not `abs() &gt; ε` — by spec; an "instrument has gradient flow" is detected at the first nonzero observation
    - Error context includes `target_collapse`, `predictor_redundancy`, `constellation_violation_rate` as JSON for operator forensics
    - Error message verbatim: "ME-JEPA target_collapse must be 0.0 by construction; instrument has gradient flow"
    - debug_assert_eq! fires only in debug builds (release uses Err return path)
    - On NaN target_collapse: still fails closed (`NaN != 0.0` is true in IEEE 754)
    - File &lt; 200 lines
  </constraints>
  <verification>
    - cargo test shows ≥ 7 tests pass: target_collapse=0 healthy, target_collapse=0.001 critical, target_collapse=NaN critical, target_collapse=-0.0 healthy (since -0.0 == 0.0 in f32 is true), redundancy=1 → ΔΞ=0, violation=1 → ΔΞ=0, exponent table 0.5/0.3/0.2 sums to 1.0
    - Error context includes all three input values
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compute_delta_xi(tc: f32, red: f32, viol: f32) -&gt; Result&lt;f32, UtmlError&gt; {
    if !(0.0..=1.0).contains(&amp;red) { return Err(UtmlError::new(UtmlFactorOutOfRange, format!("redundancy={red} out of [0,1]"))); }
    if !(0.0..=1.0).contains(&amp;viol) { return Err(UtmlError::new(UtmlFactorOutOfRange, format!("violation={viol}"))); }
    if tc != 0.0 {
        debug_assert_eq!(tc, 0.0, "ME-JEPA architectural integrity violation");
        return Err(UtmlError::new(MejepaInstrumentGradientLeak,
                                  "ME-JEPA target_collapse must be 0.0 by construction; instrument has gradient flow")
                          .with_context(json!({"target_collapse": tc, "predictor_redundancy": red, "constellation_violation_rate": viol})));
    }
    let f1 = (1.0 - tc).powf(0.5);   // == 1.0 always
    let f2 = (1.0 - red).powf(0.3);
    let f3 = (1.0 - viol).powf(0.2);
    Ok((f1 * f2 * f3).clamp(0.0, 1.0))
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/delta_xi.rs">compute_delta_xi + tests + provenance comment</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod delta_xi; pub use delta_xi::*;</file>
</files_to_modify>

<validation_criteria>
  <criterion>target_collapse=0 produces first_factor exactly 1.0</criterion>
  <criterion>target_collapse=0.001 returns Err(MejepaInstrumentGradientLeak)</criterion>
  <criterion>target_collapse=NaN returns Err (NaN != 0.0)</criterion>
  <criterion>Error context JSON includes all three inputs</criterion>
  <criterion>Geometric exponents 0.5+0.3+0.2 sum to 1.0</criterion>
  <criterion>cargo build/test/clippy clean</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_xi</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_xi -- --release</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
