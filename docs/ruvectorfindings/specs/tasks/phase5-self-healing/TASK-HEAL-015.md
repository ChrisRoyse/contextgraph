# TASK-HEAL-015 — OnlineDistiller::consume (InfoNCE + EMA τ=0.996; signal_clarity quality gate)

```xml
<task_spec id="TASK-HEAL-015" version="1.0">

<metadata>
  <title>Online distillation consumer in `crates/context-graph-mejepa/src/heal/distill.rs` (delegates to shipped context-graph-embeddings)</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>15</sequence>
  <implements>
    <item ref="REQ-HEAL-09">Distillation-based observe loop: InfoNCE + EMA τ=0.996; reuses crates/context-graph-embeddings/src/training/distillation.rs</item>
    <item ref="REQ-HEAL-13">signal_clarity multiplicative gating (skip below threshold OR weight proportionally)</item>
    <item ref="BR-HEAL-05">signal_clarity gating per quality_gate=true</item>
    <item ref="EC-HEAL-10">No oracle ground truth available → signal_clarity=0 → skip example; do not fall back to pseudo-teacher</item>
    <item ref="ERR-HEAL-01">MEJEPA_OBSERVE_BATCH_NAN (NaN mid-distill_step)</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-008</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/distill.rs); 1 modified (src/heal/mod.rs)</estimated_files>
</metadata>

<context>
Logic 10 of 10. Implements `OnlineDistiller::consume()` — the embedder-consolidation step invoked
from observe() (TASK-HEAL-008 step 12) when `signal_clarity >= threshold`. Per `12_EMBEDDER_*` §8
verbatim: for each active embedder, compute `teacher_score = oracle.score(panel)` (ground truth),
compute `student_score = embedder.score(panel)`, compute `loss = info_nce_loss(student, teacher,
temperature=0.07)`, backprop through student, EMA-update student weights with τ=0.996. Examples
with `signal_clarity < signal_clarity_threshold` (default 0.5) are skipped per `quality_gate=true`.
This task is `ready` (not blocked) because the distillation training infrastructure is already
shipped per `_index.md` Reuse note (`crates/context-graph-embeddings/src/training/distillation.rs`).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_OBSERVE_BATCH_NAN; `<anti_patterns>` no `unwrap_or(0.5)` fallback)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-09, REQ-HEAL-13; BR-HEAL-05; AC-HEAL-10-01..03; EC-HEAL-10; ERR-HEAL-01; TC-HEAL-10)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts OnlineDistiller::consume 4-step body; data_models OnlineDistiller fields)</file>
  <file purpose="design_doc">docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md §6 (LoRA + distillation as ΔK/ΔΞ/ΔΩ instrumentation), §8 (distillation_loop with EMA τ=0.996), §11 item 16 (distillation-based online observe wiring)</file>
  <file purpose="design_doc">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md §D (signal_clarity gating locked verbatim)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError, OnlineDistiller struct, DistillStep enum)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF_MEJEPA_HEAL_REPORTS for DistillStep persistence; CF_MEJEPA_PANELS read path)</file>
  <file purpose="prior_task">TASK-HEAL-008 (SelfHealingPipeline::observe step 12 invokes distiller.consume)</file>
  <file purpose="shipped_distill">crates/context-graph-embeddings/src/training/distillation.rs (DistillationTrainer::distill_step shipped; InfoNCE + EMA + quality_gate signatures)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-004..005 + TASK-HEAL-008 complete</check>
  <check>Shipped: `context_graph_embeddings::training::distillation::DistillationTrainer::distill_step(student: &mut EmbedderHandle, teacher: &OracleHandle, panel: &Panel, oracle_outcome: &OracleOutcome, config: DistillConfig) -> Result<DistillStepReport, EmbeddingError>`</check>
  <check>Shipped: `context_graph_embeddings::training::distillation::DistillConfig { temperature: f32, ema_tau: f32, quality_gate: bool, signal_clarity_threshold: f32 }`</check>
  <check>Shipped: `info_nce_loss(student_score: f32, teacher_score: f32, temperature: f32) -> f32` per `12_EMBEDDER_*` §8</check>
  <check>OracleOutcome carries enough info to derive signal_clarity (or it is supplied externally per observe() signature)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/distill.rs`:
      - `pub fn consume(distiller: &mut OnlineDistiller, panel: &Panel, oracle_outcome: &OracleOutcome, signal_clarity: f32, active_embedders: &mut [EmbedderHandle], oracle_handle: &OracleHandle, storage: Arc<RocksDbStore>) -> Result<DistillStep, HealError>` per component_contracts (4-step body):
        1. **Quality gate** (BR-HEAL-05 + REQ-HEAL-13):
           - if `distiller.quality_gate AND signal_clarity < distiller.signal_clarity_threshold`: return `Ok(DistillStep::Skipped { signal_clarity, reason: "below quality_gate threshold".into() })`
           - signal_clarity == 0.0 (EC-HEAL-10: no oracle ground truth): always skip; do NOT fall back to pseudo-teacher
        2. **Per-embedder distill** (for each EmbedderHandle in active_embedders):
           a. `teacher_score = oracle_handle.score(panel, oracle_outcome)` (ground-truth signal from oracle)
           b. `student_score = embedder.score(panel)` (current embedder's prediction)
           c. `loss = info_nce_loss(student_score, teacher_score, temperature=0.07)` (shipped helper)
           d. NaN check: if `!loss.is_finite()` return Err(BatchNan { component: format!("distill_{:?}", embedder.id()), witness_chain_offset: panel.witness_chain_offset() })
           e. `embedder.backward(loss * signal_clarity)` (signal_clarity multiplicatively gates the gradient strength)
           f. **EMA update** (τ=0.996): `w_student ← τ * w_student + (1 − τ) * w_student_after_grad` per shipped distillation infrastructure
        3. **Persist DistillStep** with provenance: `DistillStep::Stepped { num_embedders_updated, total_loss, signal_clarity, ema_tau, frozen_at, witness_chain_offset }` to CF_MEJEPA_HEAL_REPORTS keyed by frozen_at (i64 big-endian)
        4. Return Ok(DistillStep::Stepped { ... })
      - Helper: `pub fn info_nce_loss(student: f32, teacher: f32, temperature: f32) -> f32` — wraps shipped `context_graph_embeddings::training::distillation::info_nce_loss`
      - Helper: `pub fn ema_update_in_place(w_student: &mut [f32], w_after_grad: &[f32], tau: f32) -> Result<(), HealError>` — applies `w_student[i] = tau * w_student[i] + (1 - tau) * w_after_grad[i]`; rejects NaN/Inf in either input
    - DistillStep enum (re-export from TASK-HEAL-004 if defined there; else define here):
      - `DistillStep::Stepped { num_embedders_updated: u32, total_loss: f32, signal_clarity: f32, ema_tau: f32, frozen_at: i64, witness_chain_offset: u64 }`
      - `DistillStep::Skipped { signal_clarity: f32, reason: String }`
    - Tests:
      - `consume_skipped_when_signal_clarity_below_threshold_and_quality_gate_true` (signal_clarity=0.3, threshold=0.5, quality_gate=true)
      - `consume_skipped_when_signal_clarity_zero_per_ec_heal_10` (no oracle ground truth)
      - `consume_proceeds_when_signal_clarity_at_or_above_threshold` (0.5 → process; 0.6 → process)
      - `consume_proceeds_when_quality_gate_false_even_below_threshold` (override: quality_gate=false → process anything)
      - `consume_returns_batch_nan_on_nan_loss` (inject NaN via mock teacher_score)
      - `consume_per_embedder_loss_nce_form` (verified against textbook InfoNCE for known student/teacher pair)
      - `consume_signal_clarity_multiplicatively_gates_gradient` (signal_clarity=0.9 → larger weight delta than signal_clarity=0.6 on identical observation)
      - `consume_ema_update_form_w_eq_tau_w_plus_one_minus_tau_w_after_grad` (synthetic τ=0.5; verify w_after = 0.5*w_before + 0.5*w_after_grad EXACTLY)
      - `consume_ema_update_at_production_tau_0_996` (τ=0.996; verify rate of weight movement)
      - `consume_persists_distillstep_to_heal_reports`
      - `consume_skipped_does_not_persist_distillstep` (Skipped is logged but not persisted)
      - `consume_handles_zero_active_embedders_as_no_op` (empty slice → Skipped { reason: "no active embedders" })
      - `info_nce_loss_temperature_0_07_textbook_value`
      - `ema_update_in_place_rejects_nan_in_either_input`
      - End-to-end: 100 observations with signal_clarity range [0.2, 0.95]; verify <0.5 skipped, >=0.5 processed; student loss decreases over the run (TC-HEAL-10)
    - FSV evidence emission for TASK-HEAL-019: `DistillEvidence { num_observations_processed, num_skipped_below_threshold, num_skipped_zero_clarity, total_loss_initial, total_loss_final, ema_tau, signal_clarity_threshold, average_loss_decrease_per_step }`
  </in_scope>
  <out_of_scope>
    - DistillationTrainer::distill_step internals (SHIPPED in crates/context-graph-embeddings/src/training/distillation.rs)
    - LoRA refresh (TASK-HEAL-014)
    - signal_clarity computation formula (UTML §VIII; this task only RECEIVES signal_clarity from observe())
    - The orchestrator dispatch (TASK-HEAL-008 observe() step 12; this is the implementation called)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/distill.rs">
use std::sync::Arc;
use crate::heal::{errors::HealError, data_models::{OnlineDistiller, DistillStep}};
use context_graph_embeddings::training::distillation::{DistillationTrainer, DistillConfig};
use context_graph_mejepa_instruments::{Panel, OracleOutcome, EmbedderHandle, OracleHandle};

pub fn consume(
    distiller: &amp;mut OnlineDistiller,
    panel: &amp;Panel,
    oracle_outcome: &amp;OracleOutcome,
    signal_clarity: f32,
    active_embedders: &amp;mut [EmbedderHandle],
    oracle_handle: &amp;OracleHandle,
    storage: Arc&lt;RocksDbStore&gt;,
) -&gt; Result&lt;DistillStep, HealError&gt;;

pub fn info_nce_loss(student: f32, teacher: f32, temperature: f32) -&gt; f32;

pub fn ema_update_in_place(
    w_student: &amp;mut [f32],
    w_after_grad: &amp;[f32],
    tau: f32,
) -&gt; Result&lt;(), HealError&gt;;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DistillEvidence {
    pub num_observations_processed: u64,
    pub num_skipped_below_threshold: u64,
    pub num_skipped_zero_clarity: u64,
    pub total_loss_initial: f32,
    pub total_loss_final: f32,
    pub ema_tau: f32,
    pub signal_clarity_threshold: f32,
    pub average_loss_decrease_per_step: f32,
}
    </signature>
  </signatures>
  <constraints>
    - DELEGATE to shipped `context_graph_embeddings::training::distillation::DistillationTrainer::distill_step` (NEVER reinvent — already shipped per `_index.md` Reuse note + `12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md` §8 verbatim)
    - signal_clarity gating is MANDATORY per BR-HEAL-05; never silently bypass even when quality_gate=false (gate=false means "proceed but still apply multiplicative gradient weighting"; gate=true means "skip below threshold")
    - signal_clarity multiplicatively gates the gradient: `embedder.backward(loss * signal_clarity)` — NOT a binary skip-or-process; the magnitude of the update scales with clarity
    - EMA τ=0.996 (production); the form is `w ← τ * w + (1 - τ) * w_after_grad` exactly (verified at synthetic τ=0.5)
    - InfoNCE temperature=0.07 (per shipped distillation config and `12_EMBEDDER_*` §8 verbatim)
    - NaN in loss → BatchNan with the EMBEDDER ID in the component string for diagnostic clarity
    - EC-HEAL-10: signal_clarity == 0.0 → ALWAYS skip; never fall back to a pseudo-teacher (per ERR-HEAL-10 spirit and constitution `<anti_patterns>` no-fallback rule)
    - No `unwrap_or(0.5)` on signal_clarity per constitution `<anti_patterns>`
    - File &lt; 500 lines
    - SCREAMING_SNAKE error codes: MEJEPA_OBSERVE_BATCH_NAN
    - tracing::info! on Stepped (with embedder count + loss); tracing::debug! on Skipped (with reason)
    - No mock data in tests; real DistillationTrainer (small synthetic embedder + synthetic teacher) + real Panel/OracleOutcome fixtures
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::distill` shows ≥ 15 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - signal_clarity 0.3 + quality_gate=true → Skipped (no embedder mutation)
    - signal_clarity 0.0 → ALWAYS Skipped per EC-HEAL-10
    - signal_clarity 0.6 → Stepped (embedder weights mutate)
    - EMA form `w ← τ·w + (1−τ)·w_after_grad` byte-exact at synthetic τ=0.5
    - Loss decreases over a 100-observation run with signal_clarity ≥ 0.5 (TC-HEAL-10)
    - NaN injection → BatchNan with component string identifying the failing embedder
    - 100-obs synthetic run: half skipped (signal_clarity &lt; 0.5), half processed; loss decreases across processed
  </verification>
</definition_of_done>

<pseudo_code>
pub fn consume(distiller, panel, oracle_outcome, signal_clarity, active_embedders, oracle_handle, storage) -> Result<DistillStep, HealError> {
    // Step 1: Quality gate
    if signal_clarity == 0.0 {
        // EC-HEAL-10: no oracle ground truth; always skip
        return Ok(DistillStep::Skipped { signal_clarity, reason: "no oracle ground truth (EC-HEAL-10)".into() });
    }
    if distiller.quality_gate && signal_clarity < distiller.signal_clarity_threshold {
        return Ok(DistillStep::Skipped { signal_clarity, reason: format!("below quality_gate threshold {}", distiller.signal_clarity_threshold) });
    }
    if active_embedders.is_empty() {
        return Ok(DistillStep::Skipped { signal_clarity, reason: "no active embedders".into() });
    }
    // Step 2: Per-embedder distill
    let mut total_loss = 0.0_f32;
    let mut num_updated = 0_u32;
    let temperature = 0.07_f32;
    for embedder in active_embedders.iter_mut() {
        let teacher_score = oracle_handle.score(panel, oracle_outcome)?;
        let student_score = embedder.score(panel)?;
        let loss = info_nce_loss(student_score, teacher_score, temperature);
        if !loss.is_finite() {
            tracing::error!(error_code = "MEJEPA_OBSERVE_BATCH_NAN", embedder = ?embedder.id());
            return Err(HealError::BatchNan { component: format!("distill_{:?}", embedder.id()), witness_chain_offset: panel.witness_chain_offset() });
        }
        let scaled_loss = loss * signal_clarity; // signal_clarity gradient gating
        let w_before = embedder.weights_flat();
        embedder.backward(scaled_loss)?;
        let w_after_grad = embedder.weights_flat();
        // EMA update: w ← τ·w + (1 − τ)·w_after_grad
        let mut w_student = w_before;
        ema_update_in_place(&mut w_student, &w_after_grad, distiller.ema_tau)?;
        embedder.load_weights_flat(&w_student)?;
        total_loss += loss;
        num_updated += 1;
    }
    // Step 3: Persist
    let step = DistillStep::Stepped {
        num_embedders_updated: num_updated,
        total_loss,
        signal_clarity,
        ema_tau: distiller.ema_tau,
        frozen_at: now_secs(),
        witness_chain_offset: panel.witness_chain_offset(),
    };
    storage.put(CF_MEJEPA_HEAL_REPORTS, &now_secs().to_be_bytes(), &bincode::serialize(&step)?)?;
    Ok(step)
}

pub fn ema_update_in_place(w_student: &mut [f32], w_after_grad: &[f32], tau: f32) -> Result<(), HealError> {
    if w_student.len() != w_after_grad.len() {
        return Err(HealError::BatchNan { component: "ema_update length mismatch".into(), witness_chain_offset: 0 });
    }
    if !tau.is_finite() || !(0.0..=1.0).contains(&tau) {
        return Err(HealError::BatchNan { component: format!("ema_tau out of range: {}", tau), witness_chain_offset: 0 });
    }
    for (i, w) in w_student.iter_mut().enumerate() {
        let after = w_after_grad[i];
        if !w.is_finite() || !after.is_finite() {
            return Err(HealError::BatchNan { component: format!("ema input nan at {}", i), witness_chain_offset: 0 });
        }
        *w = tau * *w + (1.0 - tau) * after;
    }
    Ok(())
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/distill.rs">consume + info_nce_loss + ema_update_in_place + DistillEvidence + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod distill;`; re-export consume + DistillStep</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::distill shows ≥ 15 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>signal_clarity 0.0 → ALWAYS Skipped per EC-HEAL-10 (no pseudo-teacher fallback)</criterion>
  <criterion>signal_clarity below threshold + quality_gate=true → Skipped (no embedder mutation)</criterion>
  <criterion>signal_clarity ≥ threshold → Stepped (embedder weights mutate; EMA τ=0.996 applied)</criterion>
  <criterion>EMA form w ← τ·w + (1−τ)·w_after_grad byte-exact at synthetic τ=0.5</criterion>
  <criterion>InfoNCE loss with temperature=0.07 matches textbook value</criterion>
  <criterion>NaN loss → BatchNan with embedder ID in component string</criterion>
  <criterion>signal_clarity multiplicatively scales gradient magnitude (verified via two-observation comparison)</criterion>
  <criterion>100-obs synthetic run: loss decreases across processed examples (TC-HEAL-10)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::distill</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
