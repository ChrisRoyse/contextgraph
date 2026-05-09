# TASK-HEAL-008 — SelfHealingPipeline::observe (UTML cycle ASSESS+REGULATE first; EWC-regularized SGD step; signal_clarity gating)

```xml
<task_spec id="TASK-HEAL-008" version="1.0">

<metadata>
  <title>SelfHealingPipeline orchestrator + observe() body in `crates/context-graph-mejepa/src/heal/pipeline.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-HEAL-01">observe() signature on MeJepaCompiler delegated here</item>
    <item ref="REQ-HEAL-10">UTML regulate-state-first cycle (ASSESS → REGULATE → OBSERVE → COMPUTE → CLASSIFY → INTERVENE → EVALUATE)</item>
    <item ref="REQ-HEAL-13">signal_clarity multiplicative gating</item>
    <item ref="REQ-HEAL-20">CF_MEJEPA_SHIFT_WATERMARK advance AFTER successful step (exactly-once)</item>
    <item ref="BR-HEAL-03">REGULATE substrate FIRST when ΔΩ &lt; 0.4 OR ΔΞ &lt; 0.4</item>
    <item ref="BR-HEAL-05">signal_clarity multiplicative gate</item>
    <item ref="EC-HEAL-11">REGULATE priority: stability_floor → effective_plasticity → constellation_violation → predictor_redundancy → landscape_health</item>
    <item ref="EC-HEAL-15">batched-mode resume from stale watermark</item>
    <item ref="ERR-HEAL-01">MEJEPA_OBSERVE_BATCH_NAN with witness_chain_offset</item>
    <item ref="ERR-HEAL-10">MEJEPA_HEAL_CRITICAL_BUG_TARGET_COLLAPSE_NONZERO</item>
  </implements>
  <depends_on>
    <task_ref>TASK-HEAL-004</task_ref>
    <task_ref>TASK-HEAL-005</task_ref>
    <task_ref>TASK-HEAL-006</task_ref>
    <task_ref>TASK-HEAL-007</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 created (src/heal/pipeline.rs, src/heal/regulate.rs); 1 modified (src/heal/mod.rs)</estimated_files>
</metadata>

<context>
Logic 3 of 10. The orchestrator that wires every prior heal struct into a working observe() body
following the UTML §IX regulate-state-first cycle. Reads last 128 training certs from
CF_MEJEPA_TRAIN_CERTS to compute ΔΩ_mean, ΔΞ_mean (ASSESS), branches to substrate regulation when
either &lt; 0.4 (REGULATE in priority order per EC-HEAL-11), gates the SGD step by signal_clarity
(BR-HEAL-05), takes one EWC-regularized step on L_total = L_predict + λ·EWC_penalty, runs the Fisher
update, checks task boundary, snapshots if needed, pushes to the drift window, runs detect_drift,
spawns A/B/C if Hard/Catastrophic (delegates to TASK-HEAL-009), invokes the distillation step
(TASK-HEAL-015), advances the watermark atomically. The whole call is the entry-point for the
shift-subscriber.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<utml_v1_invariants>` rule 3 coaching order; rule 1 ΔΞ.target_collapse=0; `<failure_taxonomy>`)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-01, 10, 11, 13, 20; AC-HEAL-09-01..03; EC-HEAL-11/12/13/15)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (component_contracts SelfHealingPipeline + observe_sequence_diagram + utml_cycle_diagram)</file>
  <file purpose="prior_task">TASK-HEAL-004 (HealError + HealStatus + StatusChange)</file>
  <file purpose="prior_task">TASK-HEAL-005 (CF descriptors + codecs)</file>
  <file purpose="prior_task">TASK-HEAL-006 (EwcPlusPlus methods + PredictorGradAccessor trait)</file>
  <file purpose="prior_task">TASK-HEAL-007 (DriftDetector methods + DriftStore/DriftHistoryStore/DriftSurface traits)</file>
  <file purpose="cross_domain_train">crates/context-graph-mejepa-train (CF_MEJEPA_TRAIN_CERTS layout; ΔΩ/ΔΞ component breakdown)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-004..007 complete</check>
  <check>Phase 2 Predictor Arc&lt;RwLock&lt;Predictor&gt;&gt; surface available</check>
  <check>Phase 3 training-cert reader available; CF_MEJEPA_TRAIN_CERTS schema stable</check>
  <check>SHIFT-SUBSCRIBER (Phase 7) cross-cutting contract per REQ-HEAL-20 acknowledged</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/heal/pipeline.rs`:
      - `pub struct SelfHealingPipeline { ewc, drift_detector, abc_promoter, calibration_writer, integrity_checker, lora_refresher, distiller, predictor: Arc&lt;RwLock&lt;Predictor&gt;&gt;, storage: Arc&lt;RocksDbStore&gt;, witness_chain: Arc&lt;WitnessChainAppender&gt;, status: Arc&lt;Mutex&lt;HealStatus&gt;&gt;, promotion_lock: Arc&lt;Mutex&lt;PromotionLockState&gt;&gt;, config: MeJepaHealConfig }`
      - `pub fn new(...) -> Result<Self, HealError>` per component_contracts SelfHealingPipeline::new (loads latest EwcPlusPlus from CF_MEJEPA_FISHER_SNAPSHOTS; loads DriftDetector state; loads latest CalibrationRecord; verifies witness chain at startup; aborts with status=paused if broken)
      - `pub fn observe(&mut self, panel: &Panel, oracle_outcome: &OracleOutcome, signal_clarity: f32, witness_chain_offset_in: u64, session_id: &str) -> Result<ObserveOutput, HealError>` per component_contracts SelfHealingPipeline::observe (the 15-step body in the technical spec)
      - `ObserveOutput` enum: `Stepped { theta_sha_after, l_step, delta_p, delta_k, delta_omega, delta_xi, fisher_rank, status_change, witness_chain_offset_after } | Skipped { signal_clarity, reason: String }`
      - Helper: `fn read_train_cert_window(&self) -> Result<TrainCertWindowMeans, HealError>` returning `{ delta_omega_mean: f32, delta_xi_mean: f32, plasticity: f32, landscape_health: f32, stability_floor: f32, predictor_redundancy: f32, target_collapse: f32, constellation_violation_rate: f32, source_step_count: usize }`
      - Helper: `fn assess_substrate(&self, window: &TrainCertWindowMeans) -> SubstrateState` returning `Healthy | NeedsRegulation { breakdown: ComponentBreakdown }`
    - Create `crates/context-graph-mejepa/src/heal/regulate.rs`:
      - `pub fn regulate_substrate(pipeline: &mut SelfHealingPipeline, breakdown: &ComponentBreakdown) -> Result<RegulationReport, HealError>` — dispatches per EC-HEAL-11 priority
      - `pub struct ComponentBreakdown { effective_plasticity: f32, landscape_health: f32, stability_floor: f32, predictor_redundancy: f32, constellation_violation_rate: f32, target_collapse: f32, embedder_id_with_highest_dk_contribution: Option<u32> }`
      - `pub struct RegulationReport { actions_taken: Vec<RegulationAction>, total_duration_ms: u64 }`
      - `pub enum RegulationAction { LoraRefresh { embedder_id: u32, report: LoraRefreshReport } | SamTighten { lr_after: f32 } | VicregBump { coefficient_after: f32 } | ConstellationRefit { centroids_changed: u64 } | HaltAndRollback { last_good_step: u64 } | TargetCollapseHaltCycle { instruments: Vec<u8> } }`
      - Priority dispatch logic: stability_floor==0 → halt+rollback (return CriticalBug); target_collapse>0 → halt cycle (return CriticalBug); effective_plasticity<0.4 → LoRA refresh (TASK-HEAL-014); constellation_violation_rate>0.05 → constellation refit (cross-cutting SPEC-TCT); predictor_redundancy>0.7 → VICReg coefficient bump; landscape_health<0.4 → SAM tighten + LR cosine restart
    - Implement pipeline.rs `observe()` 15-step body per technical spec (ASSESS → signal_clarity gate → forward + NaN check → Fisher update → SGD step → boundary detect/snapshot → drift push → OBSERVE/COMPUTE/CLASSIFY → INTERVENE → drift severity check → distillation update → watermark advance → EVALUATE → return).
    - Atomic write semantics: SGD step writes to a staging file, fsync, atomic-rename; CF_MEJEPA_SHIFT_WATERMARK advance is the COMMIT point; on error before watermark advance, the watermark stays put (subscriber retries).
    - Implement live impls of TASK-HEAL-006 / TASK-HEAL-007 traits (PredictorGradAccessor, HessianTraceProbe, SnapshotStore, DriftStore, DriftHistoryStore, DriftSurface) backed by the live Arc&lt;RwLock&lt;Predictor&gt;&gt; / Arc&lt;RocksDbStore&gt;.
    - status.rs StatusChange stamping: when DriftSeverity transitions to Hard/Catastrophic, set `status.status_change = StatusChange::Retraining` (or Paused for Catastrophic) and stamp every subsequent prediction during the heal cycle (cross-cutting with SPEC-INFERENCE).
    - Tests:
      - `pipeline_new_loads_latest_fisher_snapshots`
      - `pipeline_new_aborts_with_paused_on_broken_chain`
      - `observe_skipped_on_signal_clarity_below_threshold_when_skip_below_true`
      - `observe_weighted_when_signal_clarity_low_and_skip_below_false`
      - `observe_returns_batch_nan_with_witness_chain_offset`
      - `observe_takes_sgd_step_with_ewc_regularization`
      - `observe_advances_watermark_after_success`
      - `observe_does_not_advance_watermark_on_error`
      - `observe_triggers_lora_refresh_when_plasticity_below_zero_point_four`
      - `observe_returns_critical_bug_on_target_collapse_nonzero`
      - `observe_returns_critical_bug_on_stability_floor_zero`
      - `observe_returns_drift_detected_on_hard_severity`
      - `observe_does_not_advance_to_intervene_when_substrate_unhealthy` (BR-HEAL-03 coaching-order)
      - `regulate_substrate_priority_order_stability_first`
      - `regulate_substrate_serializes_actions` (no concurrent regulations per EC-HEAL-11)
      - `pipeline_observe_atomic_on_crash_mid_step` (kill mid-call; on resume same shift re-processed exactly once per EC-HEAL-15)
  </in_scope>
  <out_of_scope>
    - retrain_and_promote spawn (TASK-HEAL-009; observe returns Err(DriftDetected) and the subscriber pauses; the actual A/B/C is invoked by the scheduler after observe returns)
    - rollback_to (TASK-HEAL-010)
    - schedule_full_retrain / continuous_calibration_if_due / integrity_check_if_due (TASK-HEAL-011 / 012 / 013)
    - LoraRefresher::refresh body (TASK-HEAL-014)
    - OnlineDistiller::consume body (TASK-HEAL-015)
    - MCP tools (TASK-HEAL-016, 017)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/pipeline.rs">
use std::sync::{Arc, Mutex, RwLock};
use crate::heal::{ewc::EwcPlusPlus, drift::DriftDetector, promote::AbcPromoter,
                  calibration::CalibrationWriter, integrity::ChainIntegrityChecker,
                  lora_refresh::LoraRefresher, distill::OnlineDistiller,
                  status::{HealStatus, StatusChange, PromotionLockState},
                  errors::HealError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ObserveOutput {
    Stepped {
        theta_sha_after: [u8; 32],
        l_step: f32, delta_p: f32, delta_k: f32, delta_omega: f32, delta_xi: f32,
        fisher_rank: usize,
        status_change: StatusChange,
        witness_chain_offset_after: u64,
    },
    Skipped { signal_clarity: f32, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeJepaHealConfig { /* per data_requirements: initial_lambda, max_lambda, drift_window_size, full_retrain_period, continuous_calibration_period, integrity_check_period, full_retrain_promotion_floor, lora_refresh_rank, lora_refresh_epochs, distillation_ema_tau, signal_clarity_threshold, skip_below_signal_clarity, train_cert_window_steps */ }

pub struct SelfHealingPipeline { /* fields per data_models */ }

impl SelfHealingPipeline {
    pub fn new(predictor: Arc&lt;RwLock&lt;Predictor&gt;&gt;,
               storage: Arc&lt;RocksDbStore&gt;,
               witness_chain: Arc&lt;WitnessChainAppender&gt;,
               lora_handle: Arc&lt;LoraTrainer&gt;,
               distill_handle: Arc&lt;DistillationTrainer&gt;,
               config: MeJepaHealConfig) -&gt; Result&lt;Self, HealError&gt;;

    pub fn observe(&amp;mut self, panel: &amp;Panel, oracle_outcome: &amp;OracleOutcome,
                   signal_clarity: f32, witness_chain_offset_in: u64, session_id: &amp;str)
        -&gt; Result&lt;ObserveOutput, HealError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - File &lt; 500 lines (split via regulate.rs; possibly pipeline_assess.rs / pipeline_observe.rs if the body grows)
    - Coaching-order rule (BR-HEAL-03): NEVER call OBSERVE / COMPUTE / CLASSIFY / INTERVENE-content when substrate unhealthy
    - target_collapse > 0 → return CriticalBug { TargetCollapseNonZero } (constitution rule 1 of utml_v1_invariants is non-negotiable)
    - stability_floor == 0 → return CriticalBug { StabilityFloorZero }
    - signal_clarity &lt; threshold AND skip_below=true → return Ok(ObserveOutput::Skipped); else multiplicatively gate the SGD step
    - Watermark advance is the COMMIT point — never advanced on error
    - Atomic durability: write_text_checked staging + fsync + atomic-rename for predictor weights snapshot
    - p50 &lt; 5s, p99 &lt; 30s per NFR-HEAL-01 (tracked but not asserted in unit tests; asserted in TASK-HEAL-018 drill)
    - No silent fallback paths; no `unwrap_or(0.5)` on ΔΩ/ΔΞ
    - SMAP frontmatter on every memory/journal/ + memory/blockers/ entry per REQ-HEAL-11
  </constraints>
  <verification>
    - `cargo test -p context-graph-mejepa heal::pipeline heal::regulate` shows ≥ 16 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - target_collapse > 0 returns Err(HealError::CriticalBug) with .code() == "MEJEPA_HEAL_CRITICAL_BUG"
    - signal_clarity 0.2 with skip_below=true returns Ok(ObserveOutput::Skipped)
    - NaN gradient injection returns Err(BatchNan) and watermark unchanged
    - Hard drift detection returns Err(DriftDetected) (caller pauses; scheduler handles retrain)
  </verification>
</definition_of_done>

<pseudo_code>
pub fn observe(&mut self, panel, oracle_outcome, signal_clarity, witness_chain_offset_in, session_id) -> Result<ObserveOutput, HealError> {
    // 1. ASSESS
    let win = self.read_train_cert_window()?;
    if win.target_collapse > 0.0 {
        return Err(HealError::CriticalBug { kind: CriticalBugKind::TargetCollapseNonZero { value: win.target_collapse, contributing_instruments: vec![] } });
    }
    let breakdown = ComponentBreakdown::from_window(&win);
    // 2. REGULATE
    if win.delta_omega_mean < 0.4 || win.delta_xi_mean < 0.4 {
        let report = regulate_substrate(self, &breakdown)?;
        // continue to OBSERVE only if regulation succeeded (HaltAndRollback / TargetCollapseHaltCycle return Err)
    }
    // 3. signal_clarity gating (BR-HEAL-05)
    if signal_clarity < self.config.signal_clarity_threshold && self.config.skip_below_signal_clarity {
        return Ok(ObserveOutput::Skipped { signal_clarity, reason: "below threshold".into() });
    }
    // 4. Forward + NaN check
    let (loss, grad) = self.predictor.read().unwrap().forward_with_loss(panel, oracle_outcome)?;
    if !loss.is_finite() || grad.iter().any(|x| !x.is_finite()) {
        tracing::error!(error_code = "MEJEPA_OBSERVE_BATCH_NAN", witness_chain_offset = witness_chain_offset_in);
        return Err(HealError::BatchNan { component: "predictor.forward".into(), witness_chain_offset: witness_chain_offset_in });
    }
    // 5. Fisher update
    self.ewc.update_fisher_online(panel, oracle_outcome, &*self.predictor.read().unwrap())?;
    // 6. SGD step on L_total = L_predict + lambda * ewc_penalty * signal_clarity_multiplier
    let lambda = self.ewc.current_lambda();
    let theta_flat = self.predictor.read().unwrap().parameters_flat();
    let ewc_pen = self.ewc.ewc_penalty(&theta_flat)?;
    let l_total = loss + lambda * ewc_pen * signal_clarity;
    self.predictor.write().unwrap().sgd_step(&grad, l_total)?;
    // 7. Boundary detect + snapshot
    if self.ewc.detect_task_boundary(loss, &*self.predictor.read().unwrap(), panel)? {
        let theta = self.predictor.read().unwrap().parameters_flat();
        self.ewc.snapshot_current_task(&theta, self.corpus_sha(), &self.snapshot_store())?;
    }
    // 8. Drift window update
    let sample = DriftSample::try_new(...)?;
    self.drift_detector.push(sample, &self.drift_store())?;
    // 9. detect_drift
    let severity = self.drift_detector.detect_drift(&self.drift_history_store(), &self.surface())?;
    if severity == DriftSeverity::Hard || severity == DriftSeverity::Catastrophic {
        // status_change=Retraining (Hard) or Paused (Catastrophic)
        // signal scheduler (does not block here)
        return Err(HealError::DriftDetected { severity, empirical_coverage: ... });
    }
    // 10. Distillation update
    self.distiller.consume(panel, oracle_outcome)?; // skips if signal_clarity below quality_gate
    // 11. Watermark advance (COMMIT point)
    self.storage.put(CF_MEJEPA_SHIFT_WATERMARK, session_id.as_bytes(), &witness_chain_offset_in.to_be_bytes())?;
    // 12. Return
    let theta_sha_after = sha256(&self.predictor.read().unwrap().parameters_flat());
    Ok(ObserveOutput::Stepped { theta_sha_after, l_step, delta_p, delta_k, delta_omega: win.delta_omega_mean, delta_xi: win.delta_xi_mean, fisher_rank: self.ewc.fisher_rank(), status_change: StatusChange::Active, witness_chain_offset_after: witness_chain_offset_in })
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/pipeline.rs">SelfHealingPipeline + new + observe + helpers + tests</file>
  <file path="crates/context-graph-mejepa/src/heal/regulate.rs">regulate_substrate dispatch + ComponentBreakdown + RegulationAction + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/heal/mod.rs">add `pub mod regulate;`; expose pipeline + ObserveOutput at crate root</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo test -p context-graph-mejepa heal::pipeline heal::regulate shows ≥ 16 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>NaN gradient returns BatchNan and watermark unchanged</criterion>
  <criterion>signal_clarity below threshold + skip_below=true returns Skipped</criterion>
  <criterion>target_collapse > 0 returns CriticalBug</criterion>
  <criterion>stability_floor == 0 returns CriticalBug</criterion>
  <criterion>Hard drift returns DriftDetected; subscriber pauses</criterion>
  <criterion>Substrate unhealthy ALWAYS routes through REGULATE before INTERVENE-content (BR-HEAL-03)</criterion>
  <criterion>regulate_substrate priority order: stability_floor → effective_plasticity → constellation_violation → predictor_redundancy → landscape_health (EC-HEAL-11)</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::pipeline heal::regulate</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
