# TASK-TRAIN-008 — Aux-head distillation back into the trunk every 1000 steps (REUSES `embeddings/training/distillation.rs` with `quality_gate=true`)

```xml
<task_spec id="TASK-TRAIN-008" version="1.0">

<metadata>
  <title>AuxHeadDistiller: snapshot aux heads as frozen teachers; for ~50 batches: KL(teacher_logits || student_logits) gradient flows back through predictor's hidden state ONLY (aux heads themselves frozen during distillation cycle); reuses shipped distillation.rs with quality_gate=true; emits cert.distillation_cycle_id + distillation_loss_mean + distillation_skipped_count</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-TRAIN-08">Aux-head distillation infrastructure</item>
    <item ref="AC-TRAIN-08-01..03">Cycle every 1000 steps; quality_gate filter; cert fields populated</item>
    <item ref="BR-TRAIN-08">Aux frozen during cycle; gradients through predictor hidden state only</item>
    <item ref="EC-TRAIN-13">quality_gate filters &gt; 50% → warn</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created</estimated_files>
</metadata>

<context>
Aux-head distillation (doc 09 §3.7.6) prevents aux heads from becoming detached "specialists" — gradients from KL(teacher_logits || student_logits) flow back through the predictor's hidden state ONLY (aux heads are frozen during the cycle). quality_gate=true filters low-confidence teacher signals (UTML signal_clarity per doc 12 §6). The shipped `crates/context-graph-embeddings/src/training/distillation.rs` already implements this with quality_gate; this task wires it.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-08; AC-TRAIN-08-01..03; BR-TRAIN-08; EC-TRAIN-13)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (architecture_diagram LDIST; trainer.train_one_epoch step 4p)</file>
  <file purpose="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.7.6</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION.md §6 (signal_clarity)</file>
  <file purpose="shipped_code">crates/context-graph-embeddings/src/training/distillation.rs (REUSED; quality_gate semantics)</file>
  <file purpose="prior_tasks">TASK-TRAIN-004 (AuxHeadEnsemble forward path)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-004 complete</check>
  <check>shipped distillation.rs accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/distill/mod.rs` with:
      - `pub struct AuxHeadDistiller { pub interval_steps: u64, pub batches_per_cycle: usize, pub quality_gate: bool, pub teacher_confidence_threshold: f32, pub cycle_id: u64 }`
      - `impl AuxHeadDistiller { pub fn new(interval_steps: u64, batches_per_cycle: usize) -&gt; Self; pub fn should_run(&amp;self, step: u64) -&gt; bool; pub fn run_cycle(&amp;mut self, predictor: &amp;mut dyn PredictorTrunk, aux_heads_frozen_snapshot: &amp;AuxHeadEnsemble, aux_heads_trainable: &amp;mut AuxHeadEnsemble, batches: &amp;[BatchInput]) -&gt; Result&lt;DistillationResult, TrainerError&gt;; }`
      - `pub struct DistillationResult { pub cycle_id: u64, pub mean_loss: f32, pub skipped_count: u64, pub processed_count: u64, pub excessive_skips: bool }`
      - `pub trait PredictorTrunk { fn hidden_forward(&amp;self, panel: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;; fn backward_through_hidden(&amp;mut self, hidden_grad: &amp;Tensor) -&gt; Result&lt;(), TrainerError&gt;; }`
    - During cycle: snapshot aux heads (clone) as frozen teachers; iterate batches_per_cycle batches; for each: compute teacher_logits = frozen_aux(predictor.hidden_forward(panel)); student_logits = trainable_aux(predictor.hidden_forward(panel)); if quality_gate: skip when teacher_max_confidence &lt; threshold; else compute KL divergence sum across heads; backward only through predictor's hidden state (aux heads frozen during cycle by detaching their parameters from the autograd graph)
    - excessive_skips = skipped_count / processed_count &gt; 0.5
    - Cycle id monotonic across run
    - Reuse `crates/context-graph-embeddings/src/training/distillation.rs::OnlineDistiller` if its API matches; otherwise re-implement KL helper using its primitives
    - Unit tests with stub predictor + stub aux heads: cycle runs at step % 1000 == 0; skipped_count populated under low-confidence; non-quality-gate path bypasses skip; KL math correctness on toy logits
  </in_scope>
  <out_of_scope>
    - Real predictor architecture (Phase 2 dep)
    - LoRA adapter wiring during distillation (TASK-TRAIN-012)
    - Trainer-side cert.distillation_* field assembly (TASK-TRAIN-009/012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/distill/mod.rs">
use candle_core::Tensor;
use crate::aux::AuxHeadEnsemble;
use crate::error::TrainerError;

pub const DEFAULT_BATCHES_PER_CYCLE: usize = 50;
pub const DEFAULT_TEACHER_CONFIDENCE_THRESHOLD: f32 = 0.5;
pub const EXCESSIVE_SKIP_THRESHOLD: f32 = 0.5;

pub struct BatchInput {
    pub panel_t01: Tensor,
}

pub struct DistillationResult {
    pub cycle_id: u64,
    pub mean_loss: f32,
    pub skipped_count: u64,
    pub processed_count: u64,
    pub excessive_skips: bool,
}

pub trait PredictorTrunk {
    fn hidden_forward(&amp;self, panel: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;;
    fn backward_through_hidden(&amp;mut self, hidden_grad: &amp;Tensor) -&gt; Result&lt;(), TrainerError&gt;;
}

pub struct AuxHeadDistiller {
    pub interval_steps: u64,
    pub batches_per_cycle: usize,
    pub quality_gate: bool,
    pub teacher_confidence_threshold: f32,
    pub cycle_id: u64,
}

impl AuxHeadDistiller {
    pub fn new(interval_steps: u64, batches_per_cycle: usize) -&gt; Self;
    pub fn should_run(&amp;self, step: u64) -&gt; bool;
    pub fn run_cycle(
        &amp;mut self,
        predictor: &amp;mut dyn PredictorTrunk,
        aux_heads_frozen_snapshot: &amp;AuxHeadEnsemble,
        aux_heads_trainable: &amp;mut AuxHeadEnsemble,
        batches: &amp;[BatchInput],
    ) -&gt; Result&lt;DistillationResult, TrainerError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - interval_steps default 1000; batches_per_cycle default 50; quality_gate=true default
    - should_run: step &gt; 0 AND step % interval_steps == 0
    - excessive_skips threshold 0.5; populated in result when skipped/processed exceeds
    - Cycle increments cycle_id monotonically
    - File &lt; 400 lines
  </constraints>
  <verification>
    - cargo test ≥ 6 tests pass: should_run boundary; skip count under low confidence; non-quality-gate path; KL math; excessive_skips threshold; cycle_id monotonic
  </verification>
</definition_of_done>

<pseudo_code>
impl AuxHeadDistiller {
    pub fn run_cycle(&amp;mut self, pred, frozen, trainable, batches) -&gt; Result&lt;DistillationResult, TrainerError&gt; {
        self.cycle_id += 1;
        let mut total_loss = 0.0; let mut processed = 0u64; let mut skipped = 0u64;
        for batch in batches {
            let hidden = pred.hidden_forward(&amp;batch.panel_t01)?;
            let teacher = frozen.forward(&amp;hidden)?; // .detach() to freeze
            let student = trainable.forward(&amp;hidden)?;
            if self.quality_gate {
                let max_conf = teacher.softmax(-1)?.max_all()?.to_scalar::&lt;f32&gt;()?;
                if max_conf &lt; self.teacher_confidence_threshold { skipped += 1; continue; }
            }
            let kl = kl_divergence(&amp;teacher, &amp;student)?;
            total_loss += kl.to_scalar::&lt;f32&gt;()?;
            // gradient flows only through predictor's hidden state (aux heads frozen)
            let hidden_grad = compute_hidden_grad_from_kl(&amp;kl)?;
            pred.backward_through_hidden(&amp;hidden_grad)?;
            processed += 1;
        }
        let mean_loss = if processed &gt; 0 { total_loss / processed as f32 } else { 0.0 };
        let excessive = (skipped as f32 / (processed + skipped).max(1) as f32) &gt; EXCESSIVE_SKIP_THRESHOLD;
        Ok(DistillationResult { cycle_id: self.cycle_id, mean_loss, skipped_count: skipped, processed_count: processed, excessive_skips: excessive })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/distill/mod.rs">AuxHeadDistiller + run_cycle + tests with stub PredictorTrunk</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">pub mod distill;</file>
</files_to_modify>

<validation_criteria>
  <criterion>should_run(1000) true; should_run(999) false; should_run(0) false</criterion>
  <criterion>quality_gate=true skips low-confidence; quality_gate=false processes all</criterion>
  <criterion>excessive_skips=true when &gt; 50%</criterion>
  <criterion>cycle_id monotonic</criterion>
  <criterion>KL math matches reference on toy logits</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib distill</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
