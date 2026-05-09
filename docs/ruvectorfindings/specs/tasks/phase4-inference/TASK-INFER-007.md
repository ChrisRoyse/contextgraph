# TASK-INFER-007 — ConformalSet outcome construction (build_outcome_set)

```xml
<task_spec id="TASK-INFER-007" version="1.0">

<metadata>
  <title>build_outcome_set in `crates/context-graph-mejepa/src/conformal.rs`</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-INFER-06">prediction_set = {outcomes : non_conformity_score(outcome) ≤ τ}</item>
    <item ref="REQ-INFER-11">Gate 3 LowConfidence (consumer of outcome_set.len() > outcome_set_max)</item>
    <item ref="component:MeJepaCompiler.build_outcome_set">behavior steps 1-5 in TECH-INFERENCE</item>
  </implements>
  <depends_on>
    <task_ref>TASK-INFER-001</task_ref>
    <task_ref>TASK-INFER-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>0 created (extends conformal.rs from TASK-INFER-005)</estimated_files>
</metadata>

<context>
At inference time, `compile()` calls `build_outcome_set(per_test_probs, context, alpha, tau, multiplier)`
to return the conformal prediction set: every candidate `OracleOutcome` whose non-conformity score
is ≤ τ_eff (where `τ_eff = tau / multiplier.clamp(0.10, 0.95)` per ΔΩ × ΔΞ multiplicative gate).
The set is non-empty (`{Pass, Fail, OOD, Abstain}` enumerated) and ordered by ascending non-conformity
(sharpest first). Gate 3 LowConfidence fires when `len() > outcome_set_max` (default 2).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-INFERENCE.md (REQ-INFER-06; REQ-INFER-11; AC-INFER-01-01)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-INFERENCE.md (component_contracts MeJepaCompiler::build_outcome_set behavior; conformal_calibration_diagram)</file>
  <file purpose="upstream_types">crates/context-graph-mejepa/src/types.rs (OracleOutcome enum: Pass, Fail, OutOfDistribution, Abstain)</file>
  <file purpose="upstream_calib">crates/context-graph-mejepa/src/conformal.rs (TASK-INFER-005 — non_conformity_score helper)</file>
</input_context_files>

<prerequisites>
  <check>TASK-INFER-001 complete; OracleOutcome enum exists with 4 variants</check>
  <check>TASK-INFER-005 complete; non_conformity_score in conformal.rs</check>
</prerequisites>

<scope>
  <in_scope>
    - Add `pub fn build_outcome_set(per_test_probs: &[f32], context: &TaskContext, alpha: f32, tau: f32, multiplier: f32) -> Result<Vec<OracleOutcome>, MejepaInferError>` to `conformal.rs`
    - Helper: `pub fn enumerate_candidate_outcomes(test_count: usize) -> Vec<(OracleOutcome, Vec<f32>)>` — returns the closed candidate set: Pass=all 1.0, Fail=all 0.0, OutOfDistribution=all 0.5 (uniform-uncertainty target), Abstain=all `f32::NAN` placeholder per AC-INFER but binary-friendly: model Abstain as "all 0.5" overlap-with-Fail; documented choice
    - For each candidate: compute `non_conformity_score(per_test_probs, candidate_test_pass)`; tag with score
    - Filter `score ≤ tau_eff = (tau / multiplier).min(1.0).max(0.0)`
    - Sort ascending by score (deterministic via total_cmp)
    - At least the closest-by-score outcome is always returned (non-empty); if no candidate ≤ tau_eff, return the single closest candidate with a logged warning (per BR-INFER-04 fallback note; the LowConfidence gate handles the high-entropy result)
    - `pub fn shannon_entropy_of_outcome_set(outcomes: &[OracleOutcome]) -> f32` — unique-count uniform entropy used by Gate 3 LowConfidence
    - Add unit tests:
      - `build_outcome_set_sharp_prediction_returns_one_outcome`: per_test all 0.99 → only Pass passes filter
      - `build_outcome_set_uncertain_returns_multiple`: per_test all 0.50 → Pass + Fail + OOD all pass filter (large outcome_set)
      - `build_outcome_set_with_multiplier_tightens`: multiplier=0.10 → tau_eff = tau / 0.1 = 10*tau (very loose); test that Fail enters set under degraded
      - `build_outcome_set_at_multiplier_clamp`: multiplier=0.05 → clamped to 0.10
      - `build_outcome_set_non_empty_invariant`: synthetic per_test that yields zero matches → returns single closest candidate (logged)
      - `build_outcome_set_sorted_ascending_by_score`: outcome ordering reproducible
      - `shannon_entropy_uniform_set_log2_n`: 4 distinct outcomes → entropy = log2(4) = 2.0
      - `shannon_entropy_singleton_zero`: 1 outcome → 0.0
  </in_scope>
  <out_of_scope>
    - τ calibration (TASK-INFER-005)
    - σ² tuning (TASK-INFER-006)
    - ΔΩ × ΔΞ multiplier computation (TASK-INFER-008 produces it)
    - Gate 3 ladder check (TASK-INFER-004)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/conformal.rs">
use crate::types::{OracleOutcome, TaskContext};
use crate::error::MejepaInferError;

pub fn build_outcome_set(
    per_test_probs: &amp;[f32],
    context: &amp;TaskContext,
    alpha: f32,
    tau: f32,
    multiplier: f32,
) -&gt; Result&lt;Vec&lt;OracleOutcome&gt;, MejepaInferError&gt;;

pub fn enumerate_candidate_outcomes(test_count: usize)
    -&gt; Vec&lt;(OracleOutcome, Vec&lt;f32&gt;)&gt;;

pub fn shannon_entropy_of_outcome_set(outcomes: &amp;[OracleOutcome]) -&gt; f32;
    </signature>
  </signatures>
  <constraints>
    - `tau_eff` clamps multiplier to [0.10, 0.95] BEFORE division (per REQ-INFER-08 / doc 11 §12)
    - candidate set is the closed taxonomy {Pass, Fail, OutOfDistribution, Abstain} (4 variants); test_count drives the per-candidate Vec length
    - sort uses `f32::total_cmp` for NaN safety
    - non-empty invariant via fallback to closest candidate
    - shannon_entropy on len=0 returns 0.0; on uniform N returns log2(N); always finite
    - Determinism: same inputs → same outcome_set bytes (sort stable; total_cmp deterministic)
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds
    - `cargo test -p context-graph-mejepa conformal::build_outcome` shows ≥ 8 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Sharp-prediction test passes (single Pass)
    - Uncertain-prediction test passes (≥ 3 outcomes)
    - Multiplier clamp test passes
    - shannon_entropy_uniform_set_log2_n returns 2.0 ± 1e-6
  </verification>
</definition_of_done>

<pseudo_code>
pub fn enumerate_candidate_outcomes(test_count: usize) -> Vec<(OracleOutcome, Vec<f32>)> {
    vec![
        (OracleOutcome::Pass,              vec![1.0; test_count]),
        (OracleOutcome::Fail,              vec![0.0; test_count]),
        (OracleOutcome::OutOfDistribution, vec![0.5; test_count]),
        (OracleOutcome::Abstain,           vec![0.5; test_count]),  // documented: Abstain treated as uniform-uncertain
    ]
}

pub fn build_outcome_set(per_test, context, alpha, tau, multiplier) -> Result<Vec<OracleOutcome>, MejepaInferError> {
    let m = multiplier.clamp(0.10, 0.95);
    let tau_eff = (tau / m).clamp(0.0, 1.0);
    let candidates = enumerate_candidate_outcomes(per_test.len());
    let mut scored: Vec<(OracleOutcome, f32)> = Vec::with_capacity(candidates.len());
    for (outcome, vec) in &candidates {
        let s = non_conformity_score(per_test, vec)?;
        scored.push((*outcome, s));
    }
    scored.sort_by(|a, b| a.1.total_cmp(&b.1));
    let kept: Vec<OracleOutcome> = scored.iter()
        .filter(|(_, s)| *s <= tau_eff)
        .map(|(o, _)| *o)
        .collect();
    if kept.is_empty() {
        // Fallback: return closest candidate; LowConfidence will flag it
        Ok(vec![scored[0].0])
    } else {
        Ok(kept)
    }
}

pub fn shannon_entropy_of_outcome_set(outcomes: &[OracleOutcome]) -> f32 {
    let mut seen = std::collections::HashSet::new();
    for o in outcomes { seen.insert(*o); }
    let n = seen.len() as f32;
    if n <= 1.0 { return 0.0; }
    n.log2()
}
</pseudo_code>

<files_to_create>
  <!-- none; extends conformal.rs from TASK-INFER-005 -->
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/conformal.rs">add build_outcome_set + enumerate_candidate_outcomes + shannon_entropy_of_outcome_set + 8 tests</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa conformal shows ≥ 16 passing tests (8 from 005 + 8 new)</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>build_outcome_set non-empty invariant holds even when no candidate ≤ tau_eff</criterion>
  <criterion>multiplier clamps below 0.10 to 0.10</criterion>
  <criterion>multiplier clamps above 0.95 to 0.95</criterion>
  <criterion>shannon_entropy_of_outcome_set on 4 distinct == log2(4) = 2.0</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa conformal</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
