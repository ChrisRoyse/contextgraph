# TASK-TRAIN-012 — Trainer::train_one_epoch + Trainer::evaluate_holdout + Trainer::run_full_training orchestration; LoRA wiring

```xml
<task_spec id="TASK-TRAIN-012" version="1.0">

<metadata>
  <title>Trainer state machine: new + train_one_epoch (per-batch forward, L_full compose, finite check, backward, AdamW step, UTML factor compute, cert emit, distillation cycle, holdout eval, checkpoint, witness append) + evaluate_holdout + run_full_training; LoRA adapter wiring on predictor + aux + trainable embedders; --full-finetune escape hatch</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-TRAIN-01..12">orchestration of all 12 requirements end-to-end</item>
    <item ref="AC-TRAIN-01-05..06">walltime budget; NaN escalation 3-in-100</item>
    <item ref="AC-TRAIN-10-01..03">Atomic checkpoint every 2000 steps + resume</item>
    <item ref="AC-TRAIN-11-01..03">LoRA wiring (default rank=32)</item>
    <item ref="EC-TRAIN-14">Resume cert chain integrity</item>
    <item ref="EC-TRAIN-15">VRAM OOM mid-step</item>
    <item ref="component:Trainer">crates/context-graph-mejepa-train/src/trainer/*</item>
    <item ref="component:Checkpointer">crates/context-graph-mejepa-train/src/checkpoint.rs</item>
    <item ref="component:LoraWiring">crates/context-graph-mejepa-train/src/lora_wiring.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-003</task_ref>
    <task_ref>TASK-TRAIN-004</task_ref>
    <task_ref>TASK-TRAIN-005</task_ref>
    <task_ref>TASK-TRAIN-006</task_ref>
    <task_ref>TASK-TRAIN-007</task_ref>
    <task_ref>TASK-TRAIN-008</task_ref>
    <task_ref>TASK-TRAIN-009</task_ref>
    <task_ref>TASK-TRAIN-010</task_ref>
    <task_ref>TASK-TRAIN-011</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>3 created</estimated_files>
</metadata>

<context>
The orchestrator wires everything: per-batch forward → L_full → finite-check → backward → grad-clip → grad-explode-detect → UTML factor compute (consumes UTML-007) → AdamW step (TASK-TRAIN-003) → sampling_weight update → cert emit (TASK-TRAIN-009) → optional distillation cycle (TASK-TRAIN-008) → optional counterfactual (TASK-TRAIN-006) → optional holdout eval (TASK-TRAIN-011) → optional checkpoint → end-of-epoch witness append (TASK-TRAIN-010). LoRA adapters wired by default; --full-finetune disables. NaN-counter escalation: 3 NaNs in 100 steps → MEJEPA_TRAIN_GRAD_EXPLODE halt. VRAM OOM during step → MEJEPA_TRAIN_VRAM_OOM halt with suggestions.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (utml_v1_invariants; performance_budgets vram_steady_state &lt;= 29 GB; failure_taxonomy MEJEPA_TRAIN_VRAM_OOM, MEJEPA_TRAIN_DOD_FAILED)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-01..12 all; complete behavior table)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (component_contracts: Trainer::new, train_one_epoch, evaluate_holdout, run_full_training; checkpoint.rs; lora_wiring.rs)</file>
  <file purpose="prior_tasks">TASK-TRAIN-003..011 (all logic units this task composes)</file>
  <file purpose="shipped_code">crates/context-graph-embeddings/src/training/lora.rs::LoRAAdapter (rank=32, alpha=16, dropout=0.05; REUSED)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-003..011 all complete</check>
  <check>../utml-learning-formula/TASK-UTML-001..009 all complete</check>
  <check>shipped LoRA adapter accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/checkpoint.rs` with:
      - `pub struct Checkpointer { pub output_dir: PathBuf, pub interval_steps: u64 }`
      - `pub struct CheckpointPayload { pub predictor_weights: HashMap&lt;String, Tensor&gt;, pub lora_adapters: Option&lt;HashMap&lt;String, Tensor&gt;&gt;, pub aux_heads: HashMap&lt;String, Tensor&gt;, pub adamw_state: AdamWStateBlob, pub sampler_rng_state: u64, pub step: u64, pub training_mode: String }`
      - `pub fn write_atomic(&amp;self, payload: &amp;CheckpointPayload, kind: CheckpointKind) -&gt; Result&lt;PathBuf, TrainerError&gt;` (write *.tmp, fsync, rename) — kind ∈ {Step, Last, Best}
      - `pub fn load(path: &amp;Path) -&gt; Result&lt;CheckpointPayload, TrainerError&gt;` with structured error on corruption
      - `pub fn verify_resume_chain_integrity(payload: &amp;CheckpointPayload, rocksdb: &amp;DB, cf_name: &amp;str) -&gt; Result&lt;(), TrainerError&gt;` (cert chain ends at payload.step)
    - Create `src/lora_wiring.rs` with:
      - `pub struct LoraState { pub adapters: HashMap&lt;String, LoRAAdapterHandle&gt;, pub rank: usize, pub alpha: usize, pub dropout: f32 }`
      - `pub fn attach_loras(predictor: &amp;mut PredictorTrunkLike, aux: &amp;mut AuxHeadEnsembleLike, trainable_embedders: &amp;mut [Embedder], rank: usize, alpha: usize, dropout: f32) -&gt; Result&lt;LoraState, TrainerError&gt;` (uses shipped LoRAAdapter::new for q/k/v projections; base instrument + frozen embedder weights NEVER trainable)
      - `pub fn detach_loras(state: LoraState) -&gt; Result&lt;(), TrainerError&gt;`
    - Create `src/trainer/methods.rs` (or extend trainer/mod.rs from TASK-TRAIN-001) with:
      - `impl Trainer { pub fn new(config: TrainingConfig, rocksdb: Arc&lt;DB&gt;, device: candle_core::Device) -&gt; Result&lt;Self, TrainerError&gt; }` per TECH-TRAINING component_contracts.Trainer.new behavior 12-step
      - `impl Trainer { pub fn train_one_epoch(&amp;mut self, dataset: &amp;TrainingDataset, epoch: u32) -&gt; Result&lt;EpochReport, TrainerError&gt; }` per TECH-TRAINING train_one_epoch behavior steps 1-7
      - `impl Trainer { pub fn evaluate_holdout(&amp;self, holdout: &amp;HoldoutDataset) -&gt; Result&lt;HoldoutReport, TrainerError&gt; }` (delegates to HoldoutEvaluator)
      - `impl Trainer { pub fn run_full_training(&amp;mut self, dataset: &amp;TrainingDataset) -&gt; Result&lt;TrainingResult, TrainerError&gt; }`
      - `pub struct TrainingDataset { pub train: TrainSplit, pub calibration: CalibrationDataset, pub holdout: HoldoutDataset }`
      - `pub struct TrainingResult { pub final_holdout_agreement: f32, pub total_walltime_seconds: u64, pub total_steps: u64, pub total_certs: u64, pub best_safetensors_path: Option&lt;PathBuf&gt; }`
    - NaN escalation: 3 NaN-skipped steps within sliding 100-step window → MejepaTrainGradExplode halt
    - Holdout eval cadence handled by `should_run_holdout(step, interval) = step &gt; 0 &amp;&amp; step % interval == 0` and final-step trigger
    - Walltime budget surfaced as warning (not auto-halt) when projection &gt; 12h at any 1000-step checkpoint per AC-TRAIN-01-06
    - Phase 3 DoD gate at end of run_full_training: if final_agreement &lt; config.phase3_dod_min_agreement → emit MEJEPA_TRAIN_DOD_FAILED, return Err
    - VRAM OOM handling: catch candle OOM at backward/optimizer step → emit MEJEPA_TRAIN_VRAM_OOM with diagnostic
    - Operator-abort: SIGINT handler sets abort flag; trainer checks at batch boundaries; persists current state via Checkpointer::write_atomic(Last) before exit
    - Smoke test (100-step dry-run on synthetic batch) in `examples/train_smoke_fsv.rs` (full FSV in TASK-TRAIN-014)
  </in_scope>
  <out_of_scope>
    - Real predictor + frozen target loading (Phase 2 dep — placeholder traits used)
    - Real corpus loading (Phase 0 dep — `TrainingDataset` constructed externally)
    - CLI entrypoint (TASK-TRAIN-013)
    - Full FSV harness (TASK-TRAIN-014)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/trainer/mod.rs">
impl Trainer {
    pub fn new(
        config: TrainingConfig,
        rocksdb: Arc&lt;DB&gt;,
        device: candle_core::Device,
    ) -&gt; Result&lt;Self, TrainerError&gt;;

    pub fn train_one_epoch(
        &amp;mut self,
        dataset: &amp;TrainingDataset,
        epoch: u32,
    ) -&gt; Result&lt;EpochReport, TrainerError&gt;;

    pub fn evaluate_holdout(
        &amp;self,
        holdout: &amp;HoldoutDataset,
        is_final_step: bool,
    ) -&gt; Result&lt;HoldoutReport, TrainerError&gt;;

    pub fn run_full_training(
        &amp;mut self,
        dataset: &amp;TrainingDataset,
    ) -&gt; Result&lt;TrainingResult, TrainerError&gt;;

    pub fn current_step(&amp;self) -&gt; u64;
    pub fn request_abort(&amp;mut self);
}

pub struct TrainingDataset {
    pub train: TrainSplit,
    pub calibration: CalibrationDataset,
    pub holdout: HoldoutDataset,
}

pub struct TrainingResult {
    pub final_holdout_agreement: f32,
    pub total_walltime_seconds: u64,
    pub total_steps: u64,
    pub total_certs: u64,
    pub best_safetensors_path: Option&lt;PathBuf&gt;,
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/checkpoint.rs">
use std::path::{Path, PathBuf};
use candle_core::Tensor;
use std::collections::HashMap;
use crate::error::TrainerError;

#[derive(Debug, Clone, Copy)]
pub enum CheckpointKind { Step, Last, Best }

pub struct AdamWStateBlob {
    pub m: HashMap&lt;String, Tensor&gt;,
    pub v: HashMap&lt;String, Tensor&gt;,
    pub step: u64,
    pub lr_schedule_state: serde_json::Value,
}

pub struct CheckpointPayload {
    pub predictor_weights: HashMap&lt;String, Tensor&gt;,
    pub lora_adapters: Option&lt;HashMap&lt;String, Tensor&gt;&gt;,
    pub aux_heads: HashMap&lt;String, Tensor&gt;,
    pub adamw_state: AdamWStateBlob,
    pub sampler_rng_state: u64,
    pub step: u64,
    pub training_mode: String,
}

pub struct Checkpointer {
    pub output_dir: PathBuf,
    pub interval_steps: u64,
}

impl Checkpointer {
    pub fn new(output_dir: PathBuf, interval_steps: u64) -&gt; Self;
    pub fn should_checkpoint(&amp;self, step: u64) -&gt; bool;
    pub fn write_atomic(&amp;self, payload: &amp;CheckpointPayload, kind: CheckpointKind) -&gt; Result&lt;PathBuf, TrainerError&gt;;
    pub fn load(path: &amp;Path) -&gt; Result&lt;CheckpointPayload, TrainerError&gt;;
}

pub fn verify_resume_chain_integrity(
    payload: &amp;CheckpointPayload,
    rocksdb: &amp;rocksdb::DB,
    cf_name: &amp;str,
) -&gt; Result&lt;(), TrainerError&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/lora_wiring.rs">
use crate::error::TrainerError;
use crate::aux::AuxHeadEnsemble;

pub struct LoRAAdapterHandle {
    // wraps shipped context_graph_embeddings::training::lora::LoRAAdapter
}

pub struct LoraState {
    pub adapters: std::collections::HashMap&lt;String, LoRAAdapterHandle&gt;,
    pub rank: usize,
    pub alpha: usize,
    pub dropout: f32,
}

// Trait abstractions over predictor/aux to avoid circular Phase 2 dep
pub trait LoraAttachable {
    fn project_param_names(&amp;self) -&gt; Vec&lt;String&gt;;
    fn attach_lora(&amp;mut self, name: &amp;str, adapter: LoRAAdapterHandle) -&gt; Result&lt;(), TrainerError&gt;;
}

pub fn attach_loras(
    predictor: &amp;mut dyn LoraAttachable,
    aux: &amp;mut dyn LoraAttachable,
    trainable_embedders: &amp;mut [Box&lt;dyn LoraAttachable&gt;],
    rank: usize,
    alpha: usize,
    dropout: f32,
) -&gt; Result&lt;LoraState, TrainerError&gt;;
    </signature>
  </signatures>
  <constraints>
    - run_full_training timing: capture start_instant; total_walltime_seconds = end - start
    - DoD gate strict: agreement &lt; config.phase3_dod_min_agreement → MejepaTrainDodFailed Err
    - VRAM OOM detection: catch candle Error variant ResourceExhausted; recommend gradient checkpointing OR batch_size reduction OR lazy-load E15-E21
    - NaN sliding window: VecDeque of last 100 step booleans (NaN observed); count &gt;= 3 → halt
    - Operator abort: AtomicBool; trainer checks at top of each batch loop iteration
    - Files &lt; 500 lines each (split methods.rs if needed)
    - Smoke FSV emits at /tmp/contextgraph-mejepa-train-fsv/training-smoke-evidence.json
  </constraints>
  <verification>
    - cargo test ≥ 8 trainer-level integration tests with stub predictor/oracle: 100-step smoke run; cert chain integrity at end; NaN escalation; checkpoint+resume parity; DoD failure mode; operator-abort
  </verification>
</definition_of_done>

<pseudo_code>
impl Trainer {
    pub fn run_full_training(&amp;mut self, ds) -&gt; Result&lt;TrainingResult, TrainerError&gt; {
        let start = Instant::now();
        for epoch in 0..self.config.epochs {
            let report = self.train_one_epoch(ds, epoch)?;
            log_epoch_to_journal(&amp;report);
            if epoch == self.config.epochs - 1 {
                if let Some(false) = report.phase3_dod_passed_at_this_epoch {
                    return Err(TrainerError::new(MejepaTrainDodFailed,
                        format!("agreement {} &lt; min {}", report.summary.holdout_agreement, self.config.phase3_dod_min_agreement)));
                }
            }
        }
        // final cert chain audit
        verify_chain(&amp;self.rocksdb, &amp;self.cert_writer.cf_train_certs_name, 0, self.step)?;
        // FSV evidence
        write_fsv_evidence(...)?;
        Ok(TrainingResult { ... })
    }

    pub fn train_one_epoch(&amp;mut self, ds, epoch) -&gt; Result&lt;EpochReport, TrainerError&gt; {
        self.sampler.recompute_weights()?;
        let n_batches = ds.train.examples.len().div_ceil(self.config.batch_size);
        let mut nan_window = VecDeque::with_capacity(100);
        for batch_idx in 0..n_batches {
            if self.abort_flag.load(Ordering::SeqCst) { break; }
            let batch_plan = self.sampler.next_batch_with_cross_task(current_task, self.config.batch_size)?;
            let inputs = ds.train.load_batch(&amp;batch_plan.indices)?;
            let p = self.predictor.forward(&amp;inputs.panel_t01)?;
            let t = self.frozen_target.readback(&amp;inputs.panel_t2)?;
            let aux = self.aux_heads.forward(&amp;p)?;
            let mut loss = compose_l_full(&amp;p, &amp;t, &amp;aux, &amp;inputs.metadata, &amp;self.config.loss_coefficients)?;
            if should_run_counterfactual_cycle(self.step, self.config.counterfactual_interval_steps, self.config.counterfactual_warmup_steps) {
                let cf = compute_counterfactual_loss(&amp;inputs.panel_t01, &amp;self.predictor, &amp;self.frozen_target, self.step, ...)?;
                loss.add_term("counterfactual", cf.loss, self.config.loss_coefficients.alpha_counterfactual);
            }
            // finite check
            if !loss.total.is_finite() {
                nan_window.push_back(true);
                if nan_window.iter().filter(|x| **x).count() &gt;= 3 {
                    return Err(TrainerError::new(MejepaTrainGradExplode, "3 NaN-skipped within 100 steps"));
                }
                self.emit_skip_cert(SkipReason::NanInf)?;
                self.step += 1;
                continue;
            } else { nan_window.push_back(false); }
            if nan_window.len() &gt; 100 { nan_window.pop_front(); }
            // backward
            loss.total_tensor.backward()?;
            let grads = self.optimizer.collect_grads()?;
            nan_inf_finite_check(&amp;grads)?;
            let pre_clip_norm = clip_grads(&amp;mut grads, self.config.max_grad_norm)?;
            self.grad_norm_running_mean = update_grad_norm_running_mean(self.grad_norm_running_mean, pre_clip_norm, 0.99);
            if let Some(err) = detect_grad_explode(pre_clip_norm, self.grad_norm_running_mean, GRAD_EXPLODE_RATIO_THRESHOLD) {
                return Err(err.with_step(self.step));
            }
            // UTML compute
            let signal = self.compute_utml_signal(&amp;p, &amp;t, &amp;grads, ...)?;
            // AdamW step
            let lr = current_lr(self.step, total_steps, effective_warmup_steps(...), self.config.lr);
            self.optimizer.step(lr)?;
            // sampling weight update for batch examples
            for (ex_i, batch_i) in batch_plan.indices.iter().enumerate() {
                self.sampler.l_steps[*batch_i] = signal.l_step;
            }
            // emit cert
            let mut cert = self.build_cert(self.step, &amp;loss, &amp;signal, pre_clip_norm, ...)?;
            self.cert_writer.emit(&amp;mut cert)?;
            // distillation
            if self.distiller.should_run(self.step) {
                let dist = self.distiller.run_cycle(...)?;
                cert.distillation_cycle_id = Some(dist.cycle_id);
                cert.distillation_loss_mean = Some(dist.mean_loss);
                cert.distillation_skipped_count = Some(dist.skipped_count);
            }
            // holdout
            if self.step &gt; 0 &amp;&amp; self.step % self.config.holdout_eval_interval_steps == 0 {
                let report = self.holdout_evaluator.evaluate(...)?;
                if report.prediction_oracle_agreement &gt; self.best_holdout_agreement + self.config.holdout_promotion_threshold {
                    self.checkpointer.write_atomic(&amp;current_payload, CheckpointKind::Best)?;
                    self.best_holdout_agreement = report.prediction_oracle_agreement;
                    cert.holdout_promotion = true;
                }
                if let Some(err) = detect_regression(report.prediction_oracle_agreement, self.best_holdout_agreement, self.config.holdout_regression_threshold) {
                    self.regression_history.push(true);
                    if track_consecutive_regressions(&amp;self.regression_history, CONSECUTIVE_REGRESSION_HALT) { return Err(err); }
                } else { self.regression_history.push(false); }
            }
            // checkpoint
            if self.checkpointer.should_checkpoint(self.step) {
                self.checkpointer.write_atomic(&amp;current_payload, CheckpointKind::Step)?;
                self.checkpointer.write_atomic(&amp;current_payload, CheckpointKind::Last)?;
            }
            self.step += 1;
        }
        // epoch summary + witness append
        let summary = build_epoch_summary(epoch, &amp;self.cert_writer, ...)?;
        let witness = self.epoch_witness.append(summary.clone())?;
        let dod = if epoch == self.config.epochs - 1 {
            Some(summary.holdout_agreement &gt;= self.config.phase3_dod_min_agreement)
        } else { None };
        Ok(EpochReport { epoch, summary, witness_entry: witness, phase3_dod_passed_at_this_epoch: dod })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/checkpoint.rs">Checkpointer + CheckpointPayload + write_atomic + load + verify_resume_chain_integrity + tests</file>
  <file path="crates/context-graph-mejepa-train/src/lora_wiring.rs">LoraState + LoraAttachable trait + attach_loras + detach_loras + tests</file>
  <file path="crates/context-graph-mejepa-train/src/trainer/methods.rs">impl Trainer { new, train_one_epoch, evaluate_holdout, run_full_training } + smoke test</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/trainer/mod.rs">re-export TrainingDataset + TrainingResult; pub mod methods;</file>
  <file path="crates/context-graph-mejepa-train/src/lib.rs">pub mod checkpoint; pub mod lora_wiring;</file>
</files_to_modify>

<validation_criteria>
  <criterion>100-step smoke dry-run with stub predictor produces valid cert chain</criterion>
  <criterion>3-NaN-in-100 escalation triggers MejepaTrainGradExplode</criterion>
  <criterion>Checkpoint write+resume produces identical step N+1 output</criterion>
  <criterion>DoD gate: synthetic agreement&lt;0.75 → MejepaTrainDodFailed</criterion>
  <criterion>Operator abort flag honored at batch boundary</criterion>
  <criterion>VRAM OOM error includes diagnostic suggestions</criterion>
  <criterion>LoRA mode default; --full-finetune disables; cert.training_mode populated</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib trainer</command>
  <command>cargo test -p context-graph-mejepa-train --lib checkpoint</command>
  <command>cargo test -p context-graph-mejepa-train --lib lora_wiring</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
