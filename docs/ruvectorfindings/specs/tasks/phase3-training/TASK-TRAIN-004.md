# TASK-TRAIN-004 — Multi-objective L_full assembler (predict + VICReg + 8 aux + L_constellation_centroid + L_step weighting)

```xml
<task_spec id="TASK-TRAIN-004" version="1.0">

<metadata>
  <title>Multi-objective L_full = L_predict + λ_var·L_var + λ_cov·L_cov + λ_inv·L_inv + α_warn·L_warn + α_runtime·L_runtime + α_rss·L_memory + α_trace·L_traceback + α_hunk·L_hunk + α_adjacent·L_collateral + α_reasoning·L_reasoning + α_overrides·L_operator_match + δ·L_constellation_centroid (+ α_counterfactual·L_counterfactual added in TASK-TRAIN-006). Reuses context-graph-embeddings/src/training/loss.rs primitives. VICReg ONLY on predictor latent.</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-TRAIN-02">L_full 13-term composite</item>
    <item ref="AC-TRAIN-02-01..06">Coefficients pinned, VICReg only on predictor side, aux MSE/CE/contrastive, operator override KL, constellation centroid term + δ=0.0 fallback, finite check + skip on NaN</item>
    <item ref="BR-TRAIN-02">All terms gradient through predictor + aux + LoRA but NEVER through Phase 1 instruments</item>
    <item ref="EC-TRAIN-03">δ=0.0 fallback when Phase 4b not yet shipped</item>
    <item ref="EC-TRAIN-04">L_operator_match=0 when no override examples in batch</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-001</task_ref>
    <task_ref>TASK-TRAIN-002</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>5 created</estimated_files>
</metadata>

<context>
L_full is the 13-term composite loss the AdamW optimizer minimizes. L_predict is JEPA-foundational latent prediction. VICReg variance/cov/invariance defends predictor-side ΔΞ (target side is structurally collapse-immune by frozen instruments). 8 aux losses (warnings/runtime/RSS/traceback/hunk/collateral/reasoning/operator_match) calibrate against ground-truth oracle side outputs. δ·L_constellation_centroid bridges into Phase 4b TCT. The implementation re-uses primitives from `crates/context-graph-embeddings/src/training/loss.rs` (InfoNCE, directional, soft-label) where the math matches; new code is the assembler.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants rule "no learnable target encoder"; anti_patterns)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-02; AC-TRAIN-02-01..06; BR-TRAIN-02; EC-TRAIN-03..04; data_requirements LossCoefficients)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (architecture_diagram subgraph "Multi-objective L_full"; component_contracts: trainer.train_one_epoch step 4d)</file>
  <file purpose="shipped_code">crates/context-graph-embeddings/src/training/loss.rs (InfoNCE, directional, soft-label primitives; REUSED)</file>
  <file purpose="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.7.1 (loss decomposition)</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (AuxHeadEnsemble skeleton), TASK-TRAIN-002 (LossCoefficients)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-002 complete</check>
  <check>shipped `context-graph-embeddings::training::loss` accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/loss/mod.rs` with `compose_l_full(predicted_latent, target_latent, aux_outputs, batch_metadata, coefficients) -&gt; Result&lt;LossBreakdown, TrainerError&gt;` returning `pub struct LossBreakdown { total: f32, components: HashMap&lt;String, f32&gt; }`
    - Create `src/loss/predict.rs` with `pub fn l_predict(predicted: &amp;Tensor, target_detached: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;` (MSE; target.detach() to block backprop into instruments)
    - Create `src/loss/vicreg.rs` with `pub fn l_variance(predicted: &amp;Tensor) -&gt; Tensor`, `pub fn l_covariance(predicted: &amp;Tensor) -&gt; Tensor`, `pub fn l_invariance(p1: &amp;Tensor, p2: &amp;Tensor) -&gt; Tensor` (Bardes 2022 reference values)
    - Create `src/loss/aux.rs` with 8 aux loss functions: l_warnings (MSE), l_runtime (MSE log scale), l_memory (MSE), l_traceback (contrastive over stack-trace clusters), l_hunk_attribution (cross-entropy over per-hunk verdicts), l_collateral (binary CE per adjacent-test outcome), l_reasoning (contrastive over CoT embeddings), l_operator_match (KL between ME-JEPA verdict and operator gold; non-override examples contribute 0)
    - Create `src/loss/constellation.rs` with `pub fn l_constellation_centroid(predicted: &amp;Tensor, current_task_centroid: Option&lt;&amp;Tensor&gt;) -&gt; Tensor` (cosine distance to nearest constellation centroid for (category, language, entity_type)); falls back to 0.0 when centroid is None (Phase 4b not yet shipped) AND coefficient.delta defaults to 0.0
    - Define `pub struct AuxOutputs { pub warnings: Tensor, pub runtime_log: Tensor, pub rss: Tensor, pub traceback_cluster_logits: Tensor, pub hunk_verdicts: Tensor, pub adjacent_outcomes: Tensor, pub reasoning_embedding: Tensor, pub operator_match: Tensor }` and `pub struct BatchMetadata { actual_warnings, actual_runtime_log, actual_rss, actual_traceback_cluster_id, actual_hunk_verdicts, actual_adjacent_outcomes, actual_cot_cluster_id, override_flags, override_gold_labels, current_task_centroids: Option&lt;Vec&lt;Tensor&gt;&gt; }`
    - compose_l_full: detach target_latent before L_predict; never feed VICReg target_latent (loss only on predicted side); skip operator_match contribution when override_flags is all-false; gracefully fall back constellation_centroid to 0.0 when None (with cert.loss_components.constellation_centroid = 0.0)
    - Finite-check the total: NaN/Inf → Err(MejepaTrainLossNan) per AC-TRAIN-02-06
    - Unit tests: each loss component on toy tensors; VICReg matches Bardes 2022; total finite check
  </in_scope>
  <out_of_scope>
    - L_counterfactual term (added by TASK-TRAIN-006)
    - Aux head architecture (TASK-TRAIN-001 placeholder; real implementation in Phase 1c neural backends)
    - Constellation centroid construction (Phase 4b TCT)
    - Distillation loss (TASK-TRAIN-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/loss/mod.rs">
use std::collections::HashMap;
use candle_core::Tensor;
use crate::config::LossCoefficients;
use crate::error::TrainerError;

pub struct AuxOutputs {
    pub warnings: Tensor,
    pub runtime_log: Tensor,
    pub rss: Tensor,
    pub traceback_cluster_logits: Tensor,
    pub hunk_verdicts: Tensor,
    pub adjacent_outcomes: Tensor,
    pub reasoning_embedding: Tensor,
    pub operator_match: Tensor,
}

pub struct BatchMetadata {
    pub actual_warnings: Tensor,
    pub actual_runtime_log: Tensor,
    pub actual_rss: Tensor,
    pub actual_traceback_cluster_id: Tensor,
    pub actual_hunk_verdicts: Tensor,
    pub actual_adjacent_outcomes: Tensor,
    pub actual_cot_cluster_id: Tensor,
    pub override_flags: Vec&lt;bool&gt;,
    pub override_gold_labels: Tensor,
    pub current_task_centroids: Option&lt;Vec&lt;Tensor&gt;&gt;,
}

pub struct LossBreakdown {
    pub total: f32,
    pub total_tensor: Tensor,
    pub components: HashMap&lt;String, f32&gt;,
}

pub fn compose_l_full(
    predicted_latent: &amp;Tensor,
    target_latent: &amp;Tensor,
    aux_outputs: &amp;AuxOutputs,
    batch_metadata: &amp;BatchMetadata,
    coefficients: &amp;LossCoefficients,
) -&gt; Result&lt;LossBreakdown, TrainerError&gt;;
    </signature>
  </signatures>
  <constraints>
    - target_latent must be `.detach()`'d before any gradient operation per `<utml_v1_invariants>` "no learnable target encoder"
    - VICReg terms operate ONLY on predicted_latent (NEVER on target_latent)
    - L_predict = MSE(predicted, target.detach())
    - L_variance, L_covariance, L_invariance per Bardes 2022 (reuse existing primitives where signatures match)
    - L_warnings, L_runtime, L_memory = MSE on aux head outputs
    - L_traceback = InfoNCE-style contrastive (reuse loss.rs InfoNCE)
    - L_hunk_attribution = cross_entropy over per-hunk verdicts
    - L_collateral = binary cross-entropy on adjacent-test outcomes
    - L_reasoning = InfoNCE contrastive on CoT embedding clusters
    - L_operator_match: if !override_flags[i] for all i → contribution = 0; else KL(softmax(operator_match[i]) || override_gold[i]) summed
    - L_constellation_centroid: if current_task_centroids.is_none() → return Tensor::zeros and cert component = 0.0; else mean cosine distance
    - components HashMap keys: "predict", "variance", "covariance", "invariance", "warnings", "runtime", "rss", "traceback", "hunk_attribution", "collateral", "reasoning", "operator_match", "constellation_centroid"
    - total_tensor must be the gradient-traceable sum (for backward())
    - File &lt; 500 lines per file; loss/aux.rs largest
  </constraints>
  <verification>
    - cargo test ≥ 14 tests pass (one per component + composite)
    - VICReg variance on Gaussian batch ≈ small known value
    - L_predict zero when predicted == target_detached
    - L_operator_match == 0 when override_flags all false
    - constellation_centroid 0 when None
    - Composite finite_check fails when injected NaN
  </verification>
</definition_of_done>

<pseudo_code>
pub fn compose_l_full(p, t, aux, meta, c) -&gt; Result&lt;LossBreakdown, TrainerError&gt; {
    let t_detached = t.detach()?;
    let l_pred = l_predict(p, &amp;t_detached)?;
    let l_var = l_variance(p);
    let l_cov = l_covariance(p);
    let l_inv = l_invariance(p, p); // per Bardes 2022 self-comparison
    let l_warn = l_warnings(&amp;aux.warnings, &amp;meta.actual_warnings)?;
    // ... 7 more aux terms
    let l_const = match &amp;meta.current_task_centroids {
        Some(centroids) => l_constellation_centroid(p, Some(centroids))?,
        None => Tensor::zeros_like(&amp;l_pred)?,
    };
    let total = (&amp;l_pred
        + (l_var * c.lambda_var)? + (l_cov * c.lambda_cov)? + (l_inv * c.lambda_inv)?
        + (l_warn * c.alpha_warn)? + ... + (l_const * c.delta)?)?;
    let total_scalar = total.to_scalar::&lt;f32&gt;()?;
    if !total_scalar.is_finite() {
        return Err(TrainerError::new(MejepaTrainLossNan, format!("L_full not finite: {total_scalar}")));
    }
    let mut components = HashMap::new();
    components.insert("predict".into(), l_pred.to_scalar::&lt;f32&gt;()?);
    // ... 12 more
    Ok(LossBreakdown { total: total_scalar, total_tensor: total, components })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/loss/mod.rs">compose_l_full + LossBreakdown + AuxOutputs + BatchMetadata + tests</file>
  <file path="crates/context-graph-mejepa-train/src/loss/predict.rs">l_predict (MSE on detached target) + tests</file>
  <file path="crates/context-graph-mejepa-train/src/loss/vicreg.rs">l_variance + l_covariance + l_invariance + tests</file>
  <file path="crates/context-graph-mejepa-train/src/loss/aux.rs">8 aux loss functions + tests</file>
  <file path="crates/context-graph-mejepa-train/src/loss/constellation.rs">l_constellation_centroid + None fallback + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">pub mod loss;</file>
</files_to_modify>

<validation_criteria>
  <criterion>13 component keys present in LossBreakdown.components</criterion>
  <criterion>VICReg ops operate on predicted_latent only</criterion>
  <criterion>Target detach blocks backprop (verified via candle requires_grad check)</criterion>
  <criterion>Operator_match=0 when no overrides; non-zero with synthetic gold</criterion>
  <criterion>Constellation 0 when centroids None</criterion>
  <criterion>NaN detection triggers MejepaTrainLossNan</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib loss</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
