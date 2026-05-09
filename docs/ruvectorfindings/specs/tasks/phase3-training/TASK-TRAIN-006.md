# TASK-TRAIN-006 — Counterfactual training (K=4 perturbations, ε ∈ {0.01, 0.05, 0.1, 0.2}) + smoothness anomaly detector

```xml
<task_spec id="TASK-TRAIN-006" version="1.0">

<metadata>
  <title>Counterfactual training: every config.counterfactual_interval_steps (default 5) after warmup (default 1000), generate K=4 perturbed panels (panel + ε·v with v unit, ε ∈ {0.01,0.05,0.1,0.2}, seed=step·K+ε_idx); supervise predictions against frozen-target readback; add α_counterfactual·L_counterfactual to L_full; emit cert.counterfactual_smoothness + smoothness_anomaly flag</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>6</sequence>
  <implements>
    <item ref="REQ-TRAIN-06">Counterfactual training derivative-of-prediction signal</item>
    <item ref="AC-TRAIN-06-01..03">K=4 perturbations deterministic; L_counterfactual MSE; smoothness signal</item>
    <item ref="BR-TRAIN-06">Counterfactual cycle every 5th step after warmup</item>
    <item ref="EC-TRAIN-11">Smoothness ceiling exceeded → cert.counterfactual_smoothness_anomaly</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
Counterfactual training (doc 09 §3.7.3) builds a derivative-of-prediction signal so the predictor learns local geometry, not just point predictions. K=4 perturbations at 4 ε scales catch degenerate local minima. Deterministic given (step, ε_idx) so audits can replay. local_smoothness signal exposes anomalies (predictor's response curvature exceeding configured ceiling).
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-06; AC-TRAIN-06-01..03; BR-TRAIN-06; EC-TRAIN-11)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (loss/counterfactual.rs; trainer.train_one_epoch step 4e)</file>
  <file purpose="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.7.3</file>
  <file purpose="prior_tasks">TASK-TRAIN-004 (LossBreakdown; α_counterfactual coefficient already in LossCoefficients)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-004 complete (LossBreakdown + LossCoefficients shipped)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/loss/counterfactual.rs` with:
      - `pub const K_PERTURBATIONS: usize = 4;`
      - `pub const EPSILON_VALUES: [f32; 4] = [0.01, 0.05, 0.1, 0.2];`
      - `pub fn sample_unit_vector(dim: usize, seed: u64) -&gt; Tensor` deterministic via SplitMix64 + L2 normalize
      - `pub fn perturb_panel(panel_t1: &amp;Tensor, eps: f32, v: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;`
      - `pub fn compute_counterfactual_loss(panel_t1: &amp;Tensor, predictor: &amp;dyn PredictorFn, frozen_target: &amp;dyn FrozenTargetFn, step: u64, smoothness_anomaly_ceiling: f32) -&gt; Result&lt;CounterfactualOutput, TrainerError&gt;`
      - `pub struct CounterfactualOutput { pub loss: Tensor, pub local_smoothness: f32, pub smoothness_anomaly: bool }`
      - `pub trait PredictorFn { fn forward(&amp;self, panel: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;; }`
      - `pub trait FrozenTargetFn { fn readback(&amp;self, panel: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;; }`
    - Determinism: seed = step * K_PERTURBATIONS as u64 + eps_idx as u64
    - Loss = mean over K of MSE(predictor.forward(panel + ε·v), frozen_target.readback(panel + ε·v))
    - local_smoothness = mean over K of (1 - cosine(predictor(panel + ε·v), predictor(panel))) / ε
    - smoothness_anomaly = local_smoothness &gt; smoothness_anomaly_ceiling (default 10.0)
    - Helper `pub fn should_run_counterfactual_cycle(step: u64, interval: u64, warmup: u64) -&gt; bool`: step &gt;= warmup AND step % interval == 0
    - Unit tests with stub predictor + stub frozen target: K=4 perturbations applied; same step → same v; smoothness signal correct on linear stub (≈ 1); anomaly flag triggers above ceiling
  </in_scope>
  <out_of_scope>
    - Real predictor + frozen target (Phase 2 dep)
    - Trainer integration (TASK-TRAIN-012 calls compute_counterfactual_loss conditionally)
    - α_counterfactual coefficient (defined in TASK-TRAIN-002 LossCoefficients)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/loss/counterfactual.rs">
use candle_core::Tensor;
use crate::error::TrainerError;

pub const K_PERTURBATIONS: usize = 4;
pub const EPSILON_VALUES: [f32; 4] = [0.01, 0.05, 0.1, 0.2];
pub const DEFAULT_SMOOTHNESS_ANOMALY_CEILING: f32 = 10.0;

pub trait PredictorFn {
    fn forward(&amp;self, panel: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;;
}

pub trait FrozenTargetFn {
    fn readback(&amp;self, panel: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;;
}

pub struct CounterfactualOutput {
    pub loss: Tensor,
    pub local_smoothness: f32,
    pub smoothness_anomaly: bool,
}

pub fn sample_unit_vector(dim: usize, seed: u64) -&gt; Tensor;
pub fn perturb_panel(panel_t1: &amp;Tensor, eps: f32, v: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;;

pub fn compute_counterfactual_loss(
    panel_t1: &amp;Tensor,
    predictor: &amp;dyn PredictorFn,
    frozen_target: &amp;dyn FrozenTargetFn,
    step: u64,
    smoothness_anomaly_ceiling: f32,
) -&gt; Result&lt;CounterfactualOutput, TrainerError&gt;;

pub fn should_run_counterfactual_cycle(step: u64, interval: u64, warmup: u64) -&gt; bool;
    </signature>
  </signatures>
  <constraints>
    - Determinism asserted: same step produces byte-equal v across runs (test via SplitMix64 seed verification)
    - K_PERTURBATIONS = 4 hard-coded per doc 09 (config flag could change in future; for now const)
    - EPSILON_VALUES = [0.01, 0.05, 0.1, 0.2] fixed
    - sample_unit_vector: SplitMix64(seed) → fill Vec&lt;f32&gt; → L2 normalize → Tensor::from_vec
    - perturb_panel: panel + ε·v elementwise
    - smoothness_anomaly default ceiling 10.0; configurable
    - File &lt; 350 lines
  </constraints>
  <verification>
    - cargo test ≥ 6 tests pass: K_PERTURBATIONS=4; EPSILON_VALUES exact; same step → same v; smoothness on linear stub ≈ 1.0; anomaly flag triggers above ceiling; should_run logic
  </verification>
</definition_of_done>

<pseudo_code>
pub fn sample_unit_vector(dim: usize, seed: u64) -&gt; Tensor {
    let mut rng = SplitMix64::seeded(seed);
    let v: Vec&lt;f32&gt; = (0..dim).map(|_| rng.next_f32_normal()).collect();
    let norm: f32 = v.iter().map(|x| x*x).sum::&lt;f32&gt;().sqrt().max(1e-8);
    let v: Vec&lt;f32&gt; = v.iter().map(|x| x / norm).collect();
    Tensor::from_vec(v, &amp;[dim], &amp;Device::Cpu).unwrap() // device handled by caller
}

pub fn compute_counterfactual_loss(panel, pred, target, step, ceiling) -&gt; Result&lt;CounterfactualOutput, TrainerError&gt; {
    let dim = panel.dims()[panel.dims().len()-1];
    let mut sum_mse = Tensor::zeros_like(panel)?.sum_all()?; // scalar zero
    let mut smooth_acc = 0f32;
    let p_orig = pred.forward(panel)?;
    for (eps_idx, &amp;eps) in EPSILON_VALUES.iter().enumerate() {
        let seed = step * K_PERTURBATIONS as u64 + eps_idx as u64;
        let v = sample_unit_vector(dim, seed);
        let perturbed = perturb_panel(panel, eps, &amp;v)?;
        let pred_pert = pred.forward(&amp;perturbed)?;
        let target_pert = target.readback(&amp;perturbed)?;
        let mse = (&amp;pred_pert - &amp;target_pert)?.sqr()?.mean_all()?;
        sum_mse = (&amp;sum_mse + &amp;mse)?;
        let cos = cosine_scalar(&amp;pred_pert, &amp;p_orig)?;
        smooth_acc += (1.0 - cos) / eps;
    }
    let loss = (sum_mse / K_PERTURBATIONS as f64)?;
    let local_smoothness = smooth_acc / K_PERTURBATIONS as f32;
    let anomaly = local_smoothness &gt; ceiling;
    Ok(CounterfactualOutput { loss, local_smoothness, smoothness_anomaly: anomaly })
}

pub fn should_run_counterfactual_cycle(step, interval, warmup) -&gt; bool {
    step &gt;= warmup &amp;&amp; step % interval == 0
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/loss/counterfactual.rs">all the above + tests with stub predictor</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/loss/mod.rs">pub mod counterfactual;</file>
</files_to_modify>

<validation_criteria>
  <criterion>K=4 enforced via const</criterion>
  <criterion>Same (step, eps_idx) produces same v</criterion>
  <criterion>local_smoothness ≈ 1.0 on linear-identity stub predictor</criterion>
  <criterion>smoothness_anomaly fires when ceiling exceeded</criterion>
  <criterion>should_run_counterfactual_cycle(1500, 5, 1000) true; (500, 5, 1000) false</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib counterfactual</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
