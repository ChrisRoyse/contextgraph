# TASK-UTML-004 — `compute_delta_k` + `update_grad_ema`

```xml
<task_spec id="TASK-UTML-004" version="1.0">

<metadata>
  <title>compute_delta_k (grad, ema_grad, fisher, ECE, embedder_coherence) returning weighted sum 0.5·cos_align + 0.3·(1-fisher_violation) + 0.15·(1-ECE) + 0.05·embedder_coherence; update_grad_ema in-place EMA update</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-UTML-03">ΔK four-term weighted sum + cos_align + EWC++ + ECE + embedder_coherence</item>
    <item ref="AC-UTML-03-01">Weight (0.5, 0.3, 0.15, 0.05) sum to 1.0; embedder_coherence default 0.5 for first 1,000 corpus entries</item>
    <item ref="AC-UTML-03-02">cos_align &lt; 0 → ΔK collapses; UTML_CATASTROPHIC_FORGETTING; rollback</item>
    <item ref="ERR-UTML-07">Catastrophic forgetting structured error</item>
    <item ref="component:DeltaK">crates/context-graph-mejepa-train/src/learning_signal/delta_k.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-UTML-001</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (FisherMatrix stub may be co-located)</estimated_files>
</metadata>

<context>
ΔK measures gradient alignment with the prior EMA gradient (NTK alignment, Atanasov 2021), Fisher-violation against EWC++ protected weights (Kirkpatrick 2017), calibration health (ECE early-Goodhart indicator, Gao 2022), and cross-embedder coherence (doc 12 §5.1). Catastrophic-forgetting detection emits structured error when cos_align &lt; threshold. The full Fisher matrix construction lives in Phase 5 self-healing; this task accepts a `FisherMatrix` trait/struct producing `compute_violation(grad)` and consumes it.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy: UTML_CATASTROPHIC_FORGETTING)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-UTML.md (US-UTML-03; AC-UTML-03-01..03; ERR-UTML-07; EC-UTML-02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-UTML.md (component DeltaK — compute_delta_k + update_grad_ema)</file>
  <file purpose="design_doc">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §4</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §5.1 (embedder_coherence)</file>
  <file purpose="prior_tasks">TASK-UTML-001.md (UtmlError shipped)</file>
</input_context_files>

<prerequisites>
  <check>TASK-UTML-001 complete</check>
  <check>candle-core dep available (added in TASK-UTML-003)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/learning_signal/delta_k.rs` with `compute_delta_k`, `update_grad_ema`, and a minimal `FisherMatrix` trait (`compute_violation(grad: &amp;Tensor) -&gt; f32`)
    - Default `catastrophic_forgetting_threshold = 0.0` (config-overridable)
    - Map cos_align from [-1, 1] to [0, 1] via (cos_align + 1.0)/2.0 before weighted sum (per TECH-UTML behavior step 7)
    - Catastrophic forgetting: emit `UtmlCatastrophicForgetting` error AND return Ok(0.0) per AC-UTML-03-02 (the function may return either Err OR Ok(0.0) — TECH-UTML behavior step 4 says "return Ok(0.0) signaling step abort"; choose Ok(0.0) AND set the cert.skip_reason=CatastrophicForgetting at the upstream caller via a side-channel — but for THIS function, return Err so upstream sets skip_reason)
    - Decision: return `Err(UtmlError{code: UtmlCatastrophicForgetting})` so upstream (TASK-UTML-007 / Trainer) treats it as fail-closed; trainer captures error and sets cert.skip_reason. Document this clearly in source.
    - Validate ECE in [0, 1] AND embedder_coherence in [0, 1]; UtmlFactorOutOfRange otherwise
    - Validate finite(grad) AND finite(ema_grad); MejepaObserveBatchNan on NaN/Inf
    - Output clamped to [0, 1]
    - Unit tests: weight-sum=1.0; cos_align=1 + low fisher → high ΔK; cos_align=-0.1 → CatastrophicForgetting err; ECE rising → ΔK shrinks; embedder_coherence=0.5 (bootstrap) verified
  </in_scope>
  <out_of_scope>
    - Full Fisher matrix construction (Phase 5)
    - ECE computation (Phase 4 conformal calibration)
    - embedder_coherence implementation (TASK-UTML-008)
    - L_step assembly (TASK-UTML-007)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/learning_signal/delta_k.rs">
use candle_core::Tensor;
use crate::error::{UtmlError, UtmlErrorCode};

pub const DEFAULT_CATASTROPHIC_FORGETTING_THRESHOLD: f32 = 0.0;
pub const DEFAULT_EMA_DECAY: f32 = 0.99;

pub trait FisherMatrix {
    fn compute_violation(&amp;self, grad: &amp;Tensor) -&gt; Result&lt;f32, UtmlError&gt;;
}

pub fn compute_delta_k(
    grad: &amp;Tensor,
    ema_grad: &amp;Tensor,
    fisher: &amp;dyn FisherMatrix,
    ece: f32,
    embedder_coherence: f32,
    catastrophic_forgetting_threshold: f32, // default DEFAULT_CATASTROPHIC_FORGETTING_THRESHOLD
) -&gt; Result&lt;f32, UtmlError&gt;;

pub fn update_grad_ema(
    ema_grad: &amp;mut Tensor,
    new_grad: &amp;Tensor,
    decay: f32, // default DEFAULT_EMA_DECAY
) -&gt; Result&lt;(), UtmlError&gt;;

#[cfg(test)]
mod tests {
    pub struct StubFisher { pub violation: f32 }
    impl FisherMatrix for StubFisher {
        fn compute_violation(&amp;self, _: &amp;Tensor) -&gt; Result&lt;f32, UtmlError&gt; { Ok(self.violation) }
    }
    /* unit tests using StubFisher */
}
    </signature>
  </signatures>
  <constraints>
    - cos_align computed as cosine_similarity over flattened gradient vectors (1D treatment); range [-1, 1]; NOT clamped
    - if cos_align &lt; threshold → return Err(UtmlError::new(UtmlCatastrophicForgetting, ...).with_context(json!({"cos_align": cos_align})))
    - cos_align_normalized = (cos_align + 1.0) / 2.0 → [0, 1]
    - delta_k_raw = 0.5*cos_align_norm + 0.3*(1-fisher_viol) + 0.15*(1-ece) + 0.05*embedder_coherence
    - delta_k = clamp(delta_k_raw, 0.0, 1.0)
    - update_grad_ema: validate decay ∈ (0, 1); fail UtmlFactorOutOfRange otherwise; in-place ema = decay*ema + (1-decay)*new
    - All input validations fail-closed; no silent fallback
    - File &lt; 350 lines
  </constraints>
  <verification>
    - cargo test shows ≥ 8 tests pass (cos_align=+1, =-0.1 (catastrophic), low ece, fisher_violation=0/1, embedder_coherence=0.5, weight-sum, decay range, EMA update math)
    - Weights 0.5+0.3+0.15+0.05 verified to sum exactly 1.0 in a constant test
  </verification>
</definition_of_done>

<pseudo_code>
fn compute_delta_k(grad, ema_grad, fisher, ece, embedder_coherence, threshold) -&gt; Result&lt;f32, UtmlError&gt; {
    if !(0.0..=1.0).contains(&amp;ece) { return Err(UtmlError::new(UtmlFactorOutOfRange, ...)); }
    if !(0.0..=1.0).contains(&amp;embedder_coherence) { return Err(...); }
    if !is_finite(grad) || !is_finite(ema_grad) { return Err(UtmlError::new(MejepaObserveBatchNan, ...)); }
    let cos_align = cosine_flatten(grad, ema_grad)?;
    if cos_align &lt; threshold {
        return Err(UtmlError::new(UtmlCatastrophicForgetting, format!("cos_align={cos_align}"))
                          .with_context(json!({"cos_align": cos_align})));
    }
    let fv = fisher.compute_violation(grad)?.clamp(0.0, 1.0);
    let cos_norm = (cos_align + 1.0) / 2.0;
    let raw = 0.5*cos_norm + 0.3*(1.0 - fv) + 0.15*(1.0 - ece) + 0.05*embedder_coherence;
    Ok(raw.clamp(0.0, 1.0))
}

fn update_grad_ema(ema_grad, new_grad, decay) -&gt; Result&lt;(), UtmlError&gt; {
    if !(0.0..1.0).contains(&amp;decay) || decay == 0.0 { return Err(...); }
    if !is_finite(new_grad) { return Err(...); }
    *ema_grad = (&amp;*ema_grad * decay)? + (new_grad * (1.0 - decay))?;
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/delta_k.rs">compute_delta_k + update_grad_ema + FisherMatrix trait + tests with StubFisher</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/learning_signal/mod.rs">pub mod delta_k; pub use delta_k::*;</file>
</files_to_modify>

<validation_criteria>
  <criterion>Weight sum 0.5+0.3+0.15+0.05 == 1.0 verified in unit test</criterion>
  <criterion>Catastrophic forgetting (cos_align &lt; 0) returns UtmlCatastrophicForgetting error</criterion>
  <criterion>NaN/Inf in grad triggers MejepaObserveBatchNan</criterion>
  <criterion>ECE/embedder_coherence out of [0,1] triggers UtmlFactorOutOfRange</criterion>
  <criterion>EMA update is in-place and produces expected output for known input</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib delta_k</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
