# TECH-HEAL — Technical Specification

> **Level 3.** Defines HOW the autonomous self-healing scheduler is built. Implements [`SPEC-HEAL.md`](../functional/SPEC-HEAL.md). Extends shipped Phase 5 `heal/*` modules.

**Status snapshot (2026-05-13):** Phase E is code/FSV shipped. The implementation is centered on `heal::scheduler_state`, deterministic heal policy modules, the MCP `mejepa_promote_approval` surface, and a durable policy ledger in `CF_MEJEPA_MODEL_PROMOTIONS`. Source-of-truth verification evidence is `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`.

**Research alignment:** The policy follows production ML best practices from Google Cloud's guidance on continuous model monitoring, ground-truth joins, rollout/rollback, and efficiency metrics; Azure ML's model-monitoring guidance on production/reference data, drift thresholds, alerts, and objective performance signals; and AWS Prescriptive Guidance on automated quality gates, A/B/canary rollout, automatic rollback, security gates, and human approval for high-impact promotion. References: <https://docs.cloud.google.com/architecture/guidelines-for-developing-high-quality-ml-solutions>, <https://learn.microsoft.com/en-us/azure/machine-learning/concept-model-monitoring?view=azureml-api-2>, <https://docs.aws.amazon.com/prescriptive-guidance/latest/gen-ai-lifecycle-operational-excellence/prod-monitoring-advanced-operations.html>.

---

## Module layout

```
crates/context-graph-mejepa/
  src/
    heal/
      scheduler.rs                 # shipped: run_self_optimization_scheduler (REQ-HEAL-FB-01)
      scheduler_state.rs           # shipped: seven ticker policy implementation
      drift_per_cell.rs            # shipped: per-(category × language) stratification (REQ-HEAL-FB-02)
      drift_bayesian.rs            # shipped: Jeffreys/Beta posterior small-window (REQ-HEAL-FB-03)
      drift_surprise_weighted.rs   # shipped: agent-flagged 2× / catastrophic 4× (REQ-HEAL-FB-04)
      dissipation.rs               # shipped: UTML §VII.1 detection (REQ-HEAL-FB-05)
      dormancy.rs                  # shipped: per-head dormancy and targeted reinit (REQ-HEAL-FB-06)
      fisher.rs                    # shipped: per-head Fisher snapshots
      lambda_ramp.rs               # shipped: per-head λ ramping (REQ-HEAL-FB-07)
      policy.rs                    # shipped: readback-verified policy ledger
      promote_approval.rs          # shipped: two-operator approval queue (REQ-HEAL-FB-08)
      drift_attribution.rs         # shipped: drift attribution and alerts (REQ-HEAL-FB-14)
      emergency_eviction.rs        # shipped: 10-minute quota tick (REQ-HEAL-FB-10)
      per_cell_promotion.rs        # shipped: per-cell promotion criterion (REQ-HEAL-FB-11)
      
      # existing modules (consumed)
      drift.rs, ewc.rs, promote.rs, full_retrain.rs, lora_refresh.rs,
      calibration.rs, distill.rs, regulate.rs, integrity.rs,
      pipeline.rs, drill.rs, store.rs, cf.rs, fsv.rs

crates/context-graph-mejepa-train/
  src/
    learning_signal/
      continual_backprop.rs        # existing; extended for per-head detection (REQ-HEAL-FB-06)

crates/context-graph-mcp/
  src/
    handlers/tools/
      mejepa_tools.rs              # extended: mejepa_promote_approval (REQ-HEAL-FB-08)
```

---

## Source-of-truth persistence

Phase E policy records are not trusted from function return values. Mutating paths sync-write and read back their source-of-truth row:

| Concern | Source of truth |
|---|---|
| Drift windows | `CF_MEJEPA_DRIFT_WINDOW` |
| Intervention reports | `CF_MEJEPA_HEAL_REPORTS` |
| Promotion approvals, alerts, refresh policies, lambda/dissipation/Fisher records | `CF_MEJEPA_MODEL_PROMOTIONS` |
| Active model pointer | `CF_MEJEPA_ACTIVE_POINTERS` |
| Dormancy reinit weights | `CF_MEJEPA_WEIGHT_BLOBS` |
| Scheduler health | `self_optimization_status.json` |

`heal::store::put_cf_readback` performs sync write + exact byte readback for critical writes. Heterogeneous policy records are stored in `CF_MEJEPA_MODEL_PROMOTIONS` instead of overloading `CF_MEJEPA_HEAL_REPORTS`, so heal-report decoders remain type-safe.

---

## Scheduler implementation shape

```rust
// crates/context-graph-mejepa/src/heal/scheduler.rs + scheduler_state.rs (shipped)
pub async fn run_self_optimization_scheduler(
    storage: Storage,
    config: SelfOptimConfig,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let mut observe_tick = tokio::time::interval(config.observe_period);             // 250ms
    let mut drift_tick = tokio::time::interval(config.drift_period);                 // 60s
    let mut active_learning_tick = tokio::time::interval(config.active_learning_period); // 600s
    let mut promote_tick = tokio::time::interval(config.promote_period);             // 3600s
    let mut continual_backprop_tick = tokio::time::interval(config.continual_backprop_period); // 1800s
    let mut constellation_tick = tokio::time::interval(config.constellation_period); // 3600s
    let mut emergency_eviction_tick = tokio::time::interval(config.emergency_eviction_period); // 600s
    
    let mut state = SchedulerState::open(&storage)?;
    
    loop {
        tokio::select! {
            _ = observe_tick.tick() => state.tick_observe().await?,
            _ = drift_tick.tick() => state.tick_drift().await?,
            _ = active_learning_tick.tick() => state.tick_active_learning().await?,
            _ = promote_tick.tick() => state.tick_promote().await?,
            _ = continual_backprop_tick.tick() => state.tick_continual_backprop().await?,
            _ = constellation_tick.tick() => state.tick_constellation_freshness().await?,
            _ = emergency_eviction_tick.tick() => state.tick_emergency_eviction().await?,
            res = shutdown.changed() => {
                res?;
                if *shutdown.borrow() { break; }
            }
        }
    }
    Ok(())
}

impl SchedulerState {
    async fn tick_observe(&mut self) -> Result<()> {
        // Drain pending shifts AND pending oracle outcomes
        let pending = self.drain_pending_shifts(deadline_ms = 200).await?;
        let oracle = self.drain_pending_oracle_outcomes().await?;
        for shift in pending {
            self.compiler.observe_shift(shift).await?;
        }
        for outcome in oracle {
            self.compiler.bind_oracle(outcome).await?;
        }
        Ok(())
    }
    
    async fn tick_drift(&mut self) -> Result<()> {
        let global = drift::DriftWindow::snapshot(&self.storage)?;
        let per_cell = drift_per_cell::snapshot_per_cell(&self.storage)?;
        let with_surprise = drift_surprise_weighted::reweight(&global, &self.recent_surprises);
        
        // Catastrophic global drift takes precedence
        match drift::classify(&with_surprise) {
            DriftLevel::None => {}
            DriftLevel::Soft => self.queue_lora_refresh(global.most_violating_head()).await?,
            DriftLevel::Hard => {
                self.queue_calibration_recalibrate().await?;
                self.queue_lora_refresh(global.most_violating_head()).await?;
            }
            DriftLevel::Catastrophic => {
                self.queue_full_retrain_alert(&global).await?;
                write_operator_alert(&self.storage, AlertSeverity::Catastrophic, format!("Catastrophic drift: {}", global)).await?;
            }
        }
        
        // Per-cell drift may also queue per-cell LoRA refresh
        for (cell, drift_level) in per_cell.iter_significant(&drift_bayesian::posterior_threshold) {
            if matches!(drift_level, DriftLevel::Soft | DriftLevel::Hard) {
                self.queue_per_cell_lora_refresh(cell.clone()).await?;
            }
        }
        
        // Dissipation
        if dissipation::detect(&self.storage, &self.recent_certs)? {
            write_operator_alert(&self.storage, AlertSeverity::Warning, "MEJEPA_DISSIPATION_DETECTED").await?;
            self.queue_replay_buffer_enrichment().await?;
            self.queue_lr_warmup_reset().await?;
        }
        Ok(())
    }
    
    async fn tick_active_learning(&mut self) -> Result<()> {
        let queue = eval::queue::ActiveLearningQueueState::open(&self.storage)?;
        for entry in queue.high_priority(top_n = 10) {
            if entry.kind == ActiveLearningKind::AgentSurprise {
                self.fast_track_observe(entry).await?;
            }
        }
        Ok(())
    }
    
    async fn tick_promote(&mut self) -> Result<()> {
        if let Some(refresh) = self.pending_lora_refresh.take() {
            let result = heal::lora_refresh::run(&self.storage, refresh).await?;
            if per_cell_promotion::all_cells_hold_or_improve(&self.storage, &result.holdout_eval)? {
                self.promote_lora_refresh(result).await?;
            } else {
                tracing::info!("LoRA refresh failed per-cell promotion gate; rolling back");
            }
        }
        if self.should_run_full_retrain() {
            let result = heal::full_retrain::run_periodic(&self.storage).await?;
            if !per_cell_promotion::all_cells_hold_or_improve(&self.storage, &result.holdout_eval)? {
                tracing::info!("Full retrain failed per-cell gate; not promoting");
            } else if result.is_catastrophic_class() {
                self.queue_for_operator_approval(result).await?;
            } else {
                self.promote_full_retrain(result).await?;
            }
        }
        // Lambda ramping
        for head in heads::ALL {
            lambda_ramp::tick(&self.storage, head)?;
        }
        Ok(())
    }
    
    async fn tick_continual_backprop(&mut self) -> Result<()> {
        let per_head_dorm = continual_backprop::detect_dormant_units_per_head(&self.storage)?;
        for (head, report) in per_head_dorm {
            if report.dormant_fraction > self.config.dormancy_threshold {
                continual_backprop::reinit_dormant_units_per_head(&self.storage, head, report)?;
            }
        }
        Ok(())
    }
    
    async fn tick_constellation_freshness(&mut self) -> Result<()> {
        let stale = mejepa_tct::freshness::find_stale_cells(&self.storage, max_age_days = 90)?;
        for cell in stale {
            mejepa_tct::builder::refit_cell(&self.storage, cell).await?;
        }
        Ok(())
    }
    
    async fn tick_emergency_eviction(&mut self) -> Result<()> {
        let status = hygiene::quota::quota_status(&self.storage)?;
        if status.used_gb > 110.0 {
            hygiene::quota::quota_check_and_evict(&self.storage)?;
        }
        Ok(())
    }
}
```

---

## Per-cell drift detection

```rust
// crates/context-graph-mejepa/src/heal/drift_per_cell.rs (shipped)
pub fn snapshot_per_cell(storage: &Storage) -> Result<PerCellDriftSnapshot> {
    let mut by_cell = HashMap::new();
    for (key, train_cert) in storage.iter_cf(CF_MEJEPA_TRAIN_CERTS)? {
        let cell = (train_cert.mutation_category, train_cert.language);
        let acc = compute_accuracy_from_cert(&train_cert);
        by_cell.entry(cell).or_insert_with(Vec::new).push(acc);
    }
    let snapshots: HashMap<_, _> = by_cell.into_iter()
        .map(|(cell, accs)| (cell, RollingAccuracySnapshot::from_window(&accs, window=1000)))
        .collect();
    Ok(PerCellDriftSnapshot { snapshots })
}
```

---

## Bayesian small-window posterior

```rust
// crates/context-graph-mejepa/src/heal/drift_bayesian.rs (shipped)
pub fn posterior_below_threshold(snapshot: &RollingAccuracySnapshot, threshold: f32, ci: f32) -> bool {
    let n = snapshot.n() as f32;
    let k = snapshot.successes() as f32;
    // Beta(α, β) posterior with Jeffreys prior (α=0.5, β=0.5)
    let alpha_post = k + 0.5;
    let beta_post = n - k + 0.5;
    let upper_quantile = beta_dist_quantile(alpha_post, beta_post, ci);
    upper_quantile < threshold
}
```

If the upper bound of the 95% Beta CI is below the drift threshold, fire. Otherwise treat as noise.

---

## Dissipation detection

```rust
// crates/context-graph-mejepa/src/heal/dissipation.rs (shipped)
pub fn detect(storage: &Storage, recent_certs: &[TrainingCertificate]) -> Result<bool> {
    let mean_l_step = recent_certs.iter().map(|c| c.l_step).sum::<f32>() / recent_certs.len() as f32;
    if mean_l_step <= 0.3 { return Ok(false); }
    
    // Check ΔK or ΔΞ silently degrading: rolling EMA of each
    let delta_k_trend = compute_ema_slope(recent_certs.iter().map(|c| c.delta_k_global));
    let delta_xi_trend = compute_ema_slope(recent_certs.iter().map(|c| c.delta_xi_global));
    if delta_k_trend > 0.0 && delta_xi_trend > 0.0 { return Ok(false); }
    
    // Validation generalization gap widening
    let val_gap = compute_val_gap(storage)?;
    if val_gap < 0.05 { return Ok(false); }
    
    // ΔM / mean(L_step) < 0.2
    let delta_m = compute_delta_m(storage)?;
    let ratio = delta_m / mean_l_step;
    Ok(ratio < 0.2)
}
```

---

## Per-head λ ramping

```rust
// crates/context-graph-mejepa/src/heal/lambda_ramp.rs (shipped)
pub fn tick(storage: &Storage, head: HeadId) -> Result<()> {
    let metric = read_head_holdout_metric(storage, head)?;
    let gate = head_ramp_gate(head);
    let cur_lambda = read_lambda(storage, head)?;
    let target = head_steady_state_lambda(head);
    
    if metric > gate && cur_lambda < target {
        let new_lambda = (cur_lambda + 0.05).min(target);
        write_lambda(storage, head, new_lambda)?;
        tracing::info!(?head, ?cur_lambda, ?new_lambda, "ramped λ");
    }
    Ok(())
}

fn head_ramp_gate(head: HeadId) -> f32 {
    match head {
        HeadId::FailureMode => 0.6,    // F1
        HeadId::EdgeCase => 0.5,        // recall
        HeadId::TechDebt => 0.5,        // PR-comment alignment
        HeadId::Perf => 0.10,           // 1 - MAE (i.e., MAE < 0.10 = 90% of relative scale right)
        HeadId::Security => 0.7,        // analyzer agreement
        HeadId::Accuracy => 0.05,       // 1 - MAE
        HeadId::Cost => 0.20,           // 1 - MAE
        HeadId::Reasoning => 0.6,       // class agreement
        _ => 1.0,                        // panel/oracle: not ramped
    }
}

fn head_steady_state_lambda(head: HeadId) -> f32 {
    match head {
        HeadId::FailureMode => 0.30,
        HeadId::EdgeCase => 0.20,
        HeadId::TechDebt => 0.10,
        HeadId::Perf => 0.15,
        HeadId::Security => 0.20,
        HeadId::Accuracy => 0.10,
        HeadId::Cost => 0.05,
        HeadId::Reasoning => 0.10,
        _ => 1.0,
    }
}
```

---

## `mejepa_promote_approval` MCP tool

```rust
// crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs (shipped extension)
pub async fn handle_promote_approval(storage: &Storage, args: PromoteApprovalArgs) -> Result<PromoteApprovalResponse> {
    let promotion = read_pending_promotion(storage, &args.promotion_id)?
        .ok_or(HealError::PromotionNotFound)?;
    
    if !promotion.is_catastrophic_class() {
        return Err(HealError::PromotionNotPendingApproval);
    }
    
    if args.approve {
        promote_approved(&storage, &promotion).await?;
        Ok(PromoteApprovalResponse { promoted: true, witness_hash: promotion.witness_hash })
    } else {
        reject_pending(&storage, &promotion, args.reason).await?;
        Ok(PromoteApprovalResponse { promoted: false, witness_hash: promotion.witness_hash })
    }
}
```

The live tool accepts `promotionId`, `operatorId`, `action=approve|reject`, `operatorReason`, and `twoPersonRule`. Catastrophic promotions require two distinct operator IDs; duplicate approvals, invalid IDs, empty reasons, and attempts to lower the required approval count fail closed with a heal error and leave the pending promotion row unchanged.

---

## FSV strategy

| FSV | What it proves | Output |
|---|---|---|
| `phase_e_self_optimization_fsv` | all Phase E policy paths execute against durable RocksDB/status-file state with before/after readback | `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json` |
| `soft_drift_860_of_1000` | persisted soft drift triggers LoRA/A/B/C and heal-report rows | same artifact |
| `catastrophic_drift_750_of_1000` | catastrophic drift queues approval, records alert, and promotes only after two distinct approvals | same artifact |
| `per_cell_small_window_compile_error_rust` | Jeffreys small-window posterior fires on catastrophic per-cell evidence | same artifact |
| `per_cell_small_window_7_of_10_noise` | noisy 7/10 small window does not fire | same artifact |
| `dissipation_high_l_step_degrading_xi` | per-head dissipation persists replay/EWC/LR interventions | same artifact |
| `lambda_ramp_and_per_head_fisher` | lambda ramp and Fisher snapshots persist/read back for every head | same artifact |
| `per_head_dormancy_targeted_reinit` | only detected head units are reinitialized | same artifact |
| `emergency_eviction_exact_110gb` | exact 110 GB threshold fires once and duplicate tick suppresses | same artifact |
| `per_cell_promotion_rejects_regression` | A/B/C score winner is rejected when any cell regresses | same artifact |
| `invalid_approval_operator_id` | invalid approval fails closed and leaves pending-promotion contents unchanged | same artifact |
