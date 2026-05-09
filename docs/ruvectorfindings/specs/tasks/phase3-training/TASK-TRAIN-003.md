# TASK-TRAIN-003 — AdamW wiring with cosine decay + linear warmup (REUSES `context-graph-embeddings/src/training/optimizer.rs`)

```xml
<task_spec id="TASK-TRAIN-003" version="1.0">

<metadata>
  <title>AdamW with linear warmup (500 steps) + cosine decay over total_steps; lr=5e-4, wd=0.04 decoupled, batch=64, β1=0.9, β2=0.999, ε=1e-8, max_grad_norm=1.0, bf16 forward/backward + fp32 master weights + fp32 optimizer state. Reuses shipped AdamW.</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-TRAIN-01">AdamW + warmup + cosine + clipping + decoupled WD + mixed precision</item>
    <item ref="AC-TRAIN-01-01..06">All 6 acceptance criteria</item>
    <item ref="BR-TRAIN-01">LR(t) schedule + decoupled WD on predictor+LoRA+aux only</item>
    <item ref="EC-TRAIN-01">total_steps&lt;500 → warmup hard-clamped to min(500, total_steps/4)</item>
    <item ref="EC-TRAIN-02">NaN observed mid-step → skip + rollback + escalate after 3-in-100</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-001</task_ref>
    <task_ref>TASK-TRAIN-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created, 1 modified</estimated_files>
</metadata>

<context>
The shipped `context-graph-embeddings::training::optimizer::AdamW` already implements warmup + cosine + clipping + decoupled weight decay. This task is wiring: construct AdamW from TrainingConfig, attach trainable parameter groups (predictor + LoRA + aux heads with weight_decay=0.04; LayerNorm+biases+frozen embedder weights with weight_decay=0.0), expose `step()` and `current_lr()` helpers, and add NaN finite-check + rollback semantics.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_TRAIN_LOSS_NAN, MEJEPA_TRAIN_GRAD_EXPLODE)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-01; AC-TRAIN-01-01..06; BR-TRAIN-01; EC-TRAIN-01..02)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (component_contracts: trainer.new step 6: AdamW construction; field optimizer)</file>
  <file purpose="shipped_code">crates/context-graph-embeddings/src/training/optimizer.rs (AdamW + AdamWConfig + ParamGroup; REUSED)</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (Trainer struct), TASK-TRAIN-002 (TrainingConfig)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-002 complete</check>
  <check>shipped `context-graph-embeddings::training::optimizer::AdamW` accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/optim/mod.rs` with:
      - `pub struct OptimizerWiring { adamw: embeddings_training::optimizer::AdamW, total_steps: u64, warmup_steps: u32 }`
      - `pub fn build_adamw(config: &amp;TrainingConfig, total_steps: u64, predictor_params: Vec&lt;Tensor&gt;, lora_params: Vec&lt;Tensor&gt;, aux_params: Vec&lt;Tensor&gt;, layernorm_bias_params: Vec&lt;Tensor&gt;) -&gt; Result&lt;OptimizerWiring, TrainerError&gt;`
      - `pub fn current_lr(t: u64, total_steps: u64, warmup_steps: u32, base_lr: f64) -&gt; f64` — pure function for LR schedule
      - `pub fn nan_inf_finite_check(grads: &amp;[Tensor]) -&gt; Result&lt;(), TrainerError&gt;`
      - `pub fn clip_grads(grads: &amp;mut [Tensor], max_norm: f32) -&gt; Result&lt;f32, TrainerError&gt;` returning pre-clip ‖g‖
      - `pub fn detect_grad_explode(grad_norm: f32, running_mean: f32, ratio_threshold: f32) -&gt; Option&lt;TrainerError&gt;`
    - LR schedule: t&lt;warmup_steps → 5e-4·(t/warmup); else → 5e-4·0.5·(1 + cos(π·(t-warmup)/(total-warmup))); clamp to ≥0
    - warmup_steps hard-clamped to min(config.warmup_steps, total_steps/4) per EC-TRAIN-01
    - Param groups:
      - predictor + LoRA + aux: weight_decay = config.weight_decay (0.04)
      - LayerNorm + biases + frozen embedder weights: weight_decay = 0.0
    - Mixed precision wiring: fp32 master weights stored alongside bf16 forward weights; cast each step before forward
    - finite_check: any !is_finite() → Err(TrainerError::new(MejepaTrainLossNan, ...))
    - clip_grads: compute Σ‖g_i‖² → sqrt → if &gt; max_norm scale by max_norm/‖g‖
    - detect_grad_explode: if grad_norm &gt; 100·running_mean → Some(TrainerError::new(MejepaTrainGradExplode, ...))
    - Property tests: LR=0 at t=0, LR=5e-4 at t=warmup, LR≈0 at t=total; clip math correctness; finite_check rejects NaN/Inf
  </in_scope>
  <out_of_scope>
    - Trainer.train_one_epoch (TASK-TRAIN-012)
    - Loss computation (TASK-TRAIN-004)
    - LoRA construction (TASK-TRAIN-012)
    - 3-in-100 NaN escalation logic (TASK-TRAIN-012; this task ships the per-step gate)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/optim/mod.rs">
use candle_core::Tensor;
use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};

pub const GRAD_EXPLODE_RATIO_THRESHOLD: f32 = 100.0;

pub struct OptimizerWiring {
    // adamw: context_graph_embeddings::training::optimizer::AdamW (REUSED — type re-exported below)
    pub total_steps: u64,
    pub warmup_steps: u32,
    pub base_lr: f64,
}

pub fn build_adamw(
    config: &amp;TrainingConfig,
    total_steps: u64,
    predictor_params: Vec&lt;Tensor&gt;,
    lora_params: Vec&lt;Tensor&gt;,
    aux_params: Vec&lt;Tensor&gt;,
    layernorm_bias_params: Vec&lt;Tensor&gt;,
) -&gt; Result&lt;OptimizerWiring, TrainerError&gt;;

pub fn current_lr(
    t: u64,
    total_steps: u64,
    warmup_steps: u32,
    base_lr: f64,
) -&gt; f64;

pub fn effective_warmup_steps(config_warmup: u32, total_steps: u64) -&gt; u32;

pub fn nan_inf_finite_check(grads: &amp;[Tensor]) -&gt; Result&lt;(), TrainerError&gt;;

pub fn clip_grads(grads: &amp;mut [Tensor], max_norm: f32) -&gt; Result&lt;f32, TrainerError&gt;;

pub fn detect_grad_explode(
    grad_norm: f32,
    running_mean: f32,
    ratio_threshold: f32, // default GRAD_EXPLODE_RATIO_THRESHOLD
) -&gt; Option&lt;TrainerError&gt;;

pub fn update_grad_norm_running_mean(prev: f32, current: f32, ema_decay: f32) -&gt; f32;
    </signature>
  </signatures>
  <constraints>
    - effective_warmup_steps = min(config_warmup, (total_steps/4) as u32); if total_steps &lt; 4, returns 1
    - current_lr at t=0 returns 0.0; at t=warmup_steps returns base_lr; at t=total_steps returns ≈ 0.0 (cosine end)
    - clip_grads computes ‖g‖_2 = sqrt(Σ ‖g_i‖²); scales in-place; returns pre-clip norm
    - finite_check iterates grads; first NaN/Inf → Err with diagnostic context (which tensor, which value)
    - detect_grad_explode returns Some only if running_mean &gt; 0 AND grad_norm &gt; ratio·running_mean
    - update_grad_norm_running_mean: decay·prev + (1-decay)·current; default decay 0.99
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test ≥ 12 tests pass: LR boundary points, warmup clamp, clip math, finite check rejects NaN, grad explode threshold, EMA update
    - Property test: forall t ∈ [0, total_steps], current_lr ∈ [0, base_lr]
  </verification>
</definition_of_done>

<pseudo_code>
pub fn current_lr(t, total, warm, base) -&gt; f64 {
    let warm = warm as u64;
    if t &lt; warm { return base * (t as f64 / warm.max(1) as f64); }
    if t &gt;= total { return 0.0; }
    let progress = (t - warm) as f64 / (total - warm) as f64;
    let decayed = base * 0.5 * (1.0 + (std::f64::consts::PI * progress).cos());
    decayed.max(0.0)
}

pub fn effective_warmup_steps(cfg_warm, total) -&gt; u32 {
    let cap = (total / 4).max(1) as u32;
    cfg_warm.min(cap).min(500)
}

pub fn nan_inf_finite_check(grads) -&gt; Result&lt;(), TrainerError&gt; {
    for (i, g) in grads.iter().enumerate() {
        if !is_tensor_finite(g)? {
            return Err(TrainerError::new(MejepaTrainLossNan,
                format!("non-finite gradient in tensor index {i}"))
                .with_context(json!({"tensor_idx": i})));
        }
    }
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/optim/mod.rs">build_adamw + current_lr + effective_warmup_steps + nan_inf_finite_check + clip_grads + detect_grad_explode + update_grad_norm_running_mean + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">pub mod optim;</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add context-graph-embeddings path dep</file>
</files_to_modify>

<validation_criteria>
  <criterion>current_lr boundary points correct</criterion>
  <criterion>effective_warmup_steps clamps for total_steps&lt;500 case (EC-TRAIN-01)</criterion>
  <criterion>NaN gradient → MejepaTrainLossNan with tensor index</criterion>
  <criterion>grad_norm &gt; 100·running_mean → MejepaTrainGradExplode</criterion>
  <criterion>clip_grads returns pre-clip norm; in-place scaling correct</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib optim</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
