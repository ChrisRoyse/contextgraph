# 09 — Self-Healing Autonomy

> **Purpose.** Specify the autonomous learning loop that lets ME-JEPA improve itself the more it is used, without operator pull. Builds on the shipped Phase 5 self-healing primitives (`heal/drift`, `heal/ewc`, `heal/promote`, `heal/full_retrain`, `heal/calibration`, `heal/lora_refresh`, `heal/distill`, `heal/regulate`, `heal/integrity`, `heal/pipeline`, `heal/drill`, `heal/store`, `heal/cf`, `heal/fsv`) and the UTML continual-backprop carrier shipped in `learning_signal/continual_backprop.rs`.
>
> **Operator directive:** *"the me jepa system needs to continually learn and evolve and improve itself the more it is used, it continually self optimizes."* This document specifies how.
>
> Owned by [`SPEC-HEAL.md`](specs/functional/SPEC-HEAL.md) and [`TECH-HEAL.md`](specs/technical/TECH-HEAL.md).

**Status snapshot (2026-05-12):** The daemon/scheduler skeleton and seven tickers are code/FSV-shipped under Phase A. Phase B added `ActiveLearningKind::AgentSurprise` consumption and core per-head UTML carriers. The full autonomous policy remains Phase E: per-cell drift, Bayesian small-window drift, full dissipation handling, per-head Fisher/dormancy/lambda ramp, constellation refresh policy, catastrophic promotion approval, emergency eviction tick, and drift attribution.

---

## 1. Self-healing scope (what gets healed without humans)

| Pathology | Detection | Intervention | Promotion gate | Operator role |
|---|---|---|---|---|
| Soft drift (88-85% rolling accuracy) | drift detector at 60s tick | LoRA refresh on most-violating head | A/B/C on holdout split; pick winner | none |
| Hard drift (85-80%) | drift detector | Calibration recalibration + LoRA refresh | A/B/C with stricter holdout | none |
| Catastrophic drift (< 80%) | drift detector | Full A/B/C retrain on rolling corpus | A/B/C with multi-window stability check | **alert + approve before promote** |
| Plasticity loss (> 15% dormant units) | continual_backprop detector at 1800s tick | Re-init dormant units | per-head metric must improve | none |
| Calibration drift (ECE > 3%) | weekly eval + per-prediction-class running avg | conformal recalibrate | empirical coverage within 88-92% | none |
| Constellation staleness (> 90 days OR > 100 new examples) | freshness check at 3600s tick | Refit affected centroids | per-cell shrinkage stability | none |
| Pairwise MI redundancy (slot MI > 0.85 ceiling) | weekly MI audit | flag slot for retire/recalibration | operator decision | **review** |
| Storage approaching quota (> 100 GB used / 115 GB cap) | quota_status tick (3600s) | tier_demote(promote) on cool slabs | none | none |
| Storage at hard quota | quota emergency eviction | evict oldest cold slabs | none | **alert** |
| Witness-chain corruption | integrity check (10000s tick) | auto-repair attempt; if fail, freeze writes | none | **alert** |
| Subscriber lag > 500 unprocessed shifts/min | subscriber metrics at 60s tick | alert + auto-restart subscriber | none | escalate if > 5 restarts/5min |
| Self-optim scheduler tokio task panic | JoinSet supervisor | restart with exponential backoff | none | escalate if flapping |
| Agent records Catastrophic surprise | CF_MEJEPA_AGENT_FEEDBACK write | fast-track observe + optionally fast-track A/B/C if drift threshold crossed | per-prediction-class metric | none unless drift triggered |

**Target behavior:** only Catastrophic-class events surface to the operator. Everything else should be automatic, measured, and reversible once Phase E policy work is complete.

---

## 2. The continuous self-optimization scheduler (the daemon's heart)

```rust
// crates/context-graph-mejepa/src/heal/scheduler.rs (skeleton shipped; full policy Phase E)
pub async fn run_self_optimization_scheduler(
    storage: Storage,
    config: SelfOptimConfig,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> Result<()> {
    let mut observe_tick = tokio::time::interval(Duration::from_millis(250));
    let mut drift_tick = tokio::time::interval(Duration::from_secs(60));
    let mut active_learning_tick = tokio::time::interval(Duration::from_secs(600));
    let mut promote_tick = tokio::time::interval(Duration::from_secs(3600));
    let mut continual_backprop_tick = tokio::time::interval(Duration::from_secs(1800));
    let mut constellation_freshness_tick = tokio::time::interval(Duration::from_secs(3600));
    
    let mut state = SchedulerState::open(&storage)?;
    
    loop {
        tokio::select! {
            _ = observe_tick.tick() => {
                state.drain_pending_shifts(deadline_ms = 200).await?;
            }
            _ = drift_tick.tick() => {
                let snapshot = drift::DriftWindow::snapshot(&storage)?;
                match drift::classify(&snapshot) {
                    DriftLevel::None => {}
                    DriftLevel::Soft => state.queue_lora_refresh(snapshot.most_violating_head()).await?,
                    DriftLevel::Hard => {
                        state.queue_calibration_recalibrate().await?;
                        state.queue_lora_refresh(snapshot.most_violating_head()).await?;
                    }
                    DriftLevel::Catastrophic => {
                        state.queue_full_retrain_alert(&snapshot).await?;
                        write_operator_alert(&storage, AlertSeverity::Catastrophic, snapshot)?;
                    }
                }
            }
            _ = active_learning_tick.tick() => {
                let queue = eval::queue::ActiveLearningQueueState::open(&storage)?;
                for entry in queue.high_priority(top_n = 10) {
                    if entry.kind == ActiveLearningKind::AgentSurprise {
                        state.fast_track_observe(entry).await?;
                    }
                }
            }
            _ = promote_tick.tick() => {
                if state.has_pending_lora_refresh() {
                    let result = heal::lora_refresh::run(&storage, state.pending_lora_refresh_head()).await?;
                    if result.passed_promotion_gate() {
                        state.promote_lora_refresh(result).await?;
                    } else {
                        tracing::info!("LoRA refresh failed promotion gate; rolling back");
                    }
                }
                if state.should_run_full_retrain() {
                    let result = heal::full_retrain::run_periodic(&storage).await?;
                    if result.passed_promotion_gate() {
                        if result.is_catastrophic_class() {
                            // Catastrophic: promote only after operator approval
                            state.queue_for_operator_approval(result).await?;
                        } else {
                            state.promote_full_retrain(result).await?;
                        }
                    }
                }
            }
            _ = continual_backprop_tick.tick() => {
                let plasticity = learning_signal::continual_backprop::detect_dormant_units(&storage)?;
                if plasticity.dormant_fraction > config.dormancy_threshold {
                    learning_signal::continual_backprop::reinit_dormant_units(plasticity, &storage)?;
                }
            }
            _ = constellation_freshness_tick.tick() => {
                let stale = mejepa_tct::freshness::find_stale_cells(&storage, max_age_days=90)?;
                for cell in stale {
                    mejepa_tct::builder::refit_cell(&storage, cell).await?;
                }
            }
            _ = shutdown.changed() => {
                tracing::info!("self-optimization scheduler received shutdown");
                break;
            }
        }
    }
    Ok(())
}
```

Implementation status:
- All `heal::*` modules exist (shipped Phase 5).
- `learning_signal::continual_backprop::*` exists (shipped REQ-UTML-07).
- `mejepa_tct::freshness::*` exists (shipped Phase 4b).
- `eval::queue::ActiveLearningQueueState::*` exists (shipped Phase 8).
- The scheduler `run_self_optimization_scheduler` itself is **the new code**. It's pure orchestration over shipped APIs.

---

## 3. The three-mode promotion gate (A/B/C, shipped)

Per `docs/ruvectorfindings/specs/_README.md` Phase 5 (REQ-HEAL-04), shipped:

- **A** = current active model (the "champion")
- **B** = freshly-retrained model on the rolling corpus + most-recent shifts
- **C** = ensemble of A and B (weighted average)

For every potential promotion, all three are evaluated on the holdout split:

```
A: holdout correlation 0.91 per (mutation_category × language)
B: holdout correlation 0.93 with 1.8x slower inference
C: holdout correlation 0.94 with 1.4x slower inference
```

The promotion gate picks the winner by `score = holdout_correlation - 0.05 * inference_latency_multiplier`. C wins by 0.94 - 0.05·1.4 = 0.87 vs B's 0.93 - 0.05·1.8 = 0.84 vs A's 0.91 - 0.05·1.0 = 0.86. C is promoted.

Active pointer (`CF_MEJEPA_ACTIVE_POINTERS`) is updated atomically; the previous active is retained as a rollback target for 30 days. `mejepa_rollback_to` MCP tool (shipped) lets the operator force a rollback at any time.

**The promotion gate is the safety mechanism for unsupervised self-healing.** It guarantees no degradation ever reaches production, because B/C have to *win* on the holdout before they replace A.

---

## 4. Drift detection details (shipped, with operational tuning)

`heal::drift::DriftWindow` is a rolling-1000-attempt window. Per `REQ-HEAL-03` thresholds (shipped):

```
Soft           : 88% > rolling_accuracy ≥ 85%
Hard           : 85% > rolling_accuracy ≥ 80%
Catastrophic   : rolling_accuracy < 80%
```

Operational tuning the futurebuild plan adds (`TASK-HEAL-002`):
- **Per-(category × language) stratification.** Today the rolling window is global; we add per-cell tracking so soft drift in one language can trigger a per-cell intervention without retraining the whole model.
- **Bayesian posterior on small windows.** When per-cell sample count < 50, use a Bayesian posterior (Beta(α, β)) over accuracy rather than the point estimate, so we don't fire on noise.
- **Surprise-event-weighted drift.** Agent-flagged surprise events count as 2× weight in the rolling window — they're stronger signal than passive missed predictions.

---

## 5. Continual backprop (shipped, with scheduler integration)

`learning_signal::continual_backprop::{detect_dormant_units, reinit_dormant_units}` (shipped). Detector identifies units (ReLU pre-activations, attention heads) that are near-zero on > 99% of recent batches. Re-init applies a small magnitude Gaussian to the dormant unit's incoming weights and resets the corresponding output bias.

Scheduler integration (skeleton shipped; full policy in §2 above):
- Detector runs every 1800s (30 minutes).
- If `dormant_fraction > 0.15`, re-init runs.
- Re-init writes to `CF_MEJEPA_WEIGHT_BLOBS` as a new variant; A/B/C promotion gate evaluates whether the re-init helped (A = pre-re-init; B = post-re-init).

This is the operational closure of UTML §V.1 (effective plasticity) per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §4.1`.

---

## 6. Calibration recalibration (shipped, with scheduler integration)

`heal::calibration::recalibrate` (shipped). Re-fits the conformal predictor on a fresh calibration split:

```
1. Pull last 1000 (prediction, oracle_outcome) tuples from CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_ORACLE_VERDICTS
2. Compute non-conformity scores per the SplitConformal algorithm
3. Fit new threshold so empirical coverage = stated coverage_target (default 0.90)
4. Persist new ConformalCalibration to CF (versioned)
5. Update active pointer
```

Scheduler integration: triggered by drift detection when ECE > 3% sustained over 10k attempts. Runs in milliseconds; effectively free.

---

## 7. LoRA refresh (shipped, with scheduler integration)

`heal::lora_refresh::run` (shipped). Trains a small LoRA adapter on the rolling-window data, layered on top of the active weights:

```
1. Sample N training examples weighted by sampling_weight (favors hard recent examples)
2. Initialize LoRA adapter (rank r=16 by default; per-head)
3. Fine-tune for 1 epoch with EWC++ regularization
4. Evaluate on holdout split → score the LoRA against the no-LoRA baseline
5. If LoRA improves the head's metric by > 1%, promote; else discard
```

Scheduler integration: triggered by Soft or Hard drift detection. Per-head; only the most-violating head's LoRA is refreshed at a time (avoid simultaneous unrelated updates that hide regressions).

---

## 8. Full retrain (shipped, with scheduler integration)

`heal::full_retrain::run_periodic` (shipped). Triggers when:
- Catastrophic drift detected, OR
- 10k attempts since last full retrain (configurable), OR
- Operator manually invokes `mejepa_eval_run --force-full-retrain`

Process:
```
1. Snapshot current active weights → C variant in A/B/C
2. Train fresh weights from scratch on the union of (Phase 0 corpus + rolling-window corpus + agent-feedback gold-standard)
3. Run 1 epoch of EWC++-regularized fine-tune to anchor on prior consolidated knowledge
4. A = current active, B = fresh, C = ensemble; evaluate all three on holdout
5. If A wins: discard B, C; log "no improvement; current is best"
6. If B or C wins: promote (after operator approval if Catastrophic class)
```

Catastrophic-class promotion gate requires the operator's `mejepa_promote_approval` (NEW, small task) MCP tool call before A is replaced; non-Catastrophic promotions are automatic.

---

## 9. Continual learning theory (UTML §VII consolidation)

Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §7`, the consolidation integral is:

```
M(t) = ∫₀ᵗ L_step(τ) · ΔΞ(τ) · k_replay(τ) · e^(−(t−τ)/τ_decay(τ)) dτ
```

In ME-JEPA:
- **L_step(τ)** is the per-step learning efficiency, computed from ΔP × ΔK × ΔΩ × ΔΞ.
- **ΔΞ(τ)** = 1 (target collapse = 0 by construction) × predictor health × constellation violation rate.
- **k_replay(τ)** = 1 normally; 3-4 during periodic full-retrain phases.
- **τ_decay(τ)** = 7 days for un-replayed weights; 365 days for high-Fisher consolidated examples.

The product `L_step · ΔΞ` is the "did this step actually learn anything generalizable" signal. The scheduler ensures this product stays high by:
- **Soft drift** → LoRA refresh (raises L_step for the violating head)
- **Plasticity loss** → continual backprop re-init (raises ΔΩ.effective_plasticity)
- **Calibration drift** → recalibrate (raises ΔK.ECE component)
- **Constellation staleness** → refit (lowers ΔΞ.constellation_violation_rate)

These four interventions are sufficient to keep `M(t)` monotone-increasing under normal operation. The Catastrophic intervention (full A/B/C retrain) is the bigger hammer for when the smaller ones don't suffice.

---

## 10. The "dissipation" failure mode (UTML §VII.1)

Per `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §7.1`:

```
Dissipating_ML ≡ ( mean(L_step) over last N updates > 0.3 )
                AND
                ( ΔK or ΔΞ has been silently degrading; validation generalization gap widening )
                AND
                ( ΔM over equivalent eval window / mean(L_step) < 0.2 )
```

This is the failure mode where training loss falls but generalization doesn't. Detection:
- Compare rolling training loss trend vs rolling holdout-correlation trend.
- If training loss is decreasing > 5% over 1000 steps but holdout correlation is flat or decreasing, fire `MEJEPA_DISSIPATION_DETECTED`.

Intervention (per UTML §VII.1):
- Replay-buffer enrichment (weight high-L_step examples even more)
- EWC re-anchoring on protected gold-standard examples
- Learning-rate annealing or warmup reset
- Continual-backprop dormant-unit reinit
- Fresh real-data injection (anti-Shumailov; the flywheel handles this naturally)

The scheduler's drift_check_tick is extended to detect dissipation as a fourth class beyond Soft/Hard/Catastrophic (`TASK-HEAL-003-dissipation-detection.md`).

---

## 11. Self-healing storage (already mostly automated)

Per `06 HYGIENE` (shipped):
- Nightly GC at 03:00: demote + compact + evict + prune + verify
- Tier compression: hot 8-bit / warm 7-bit / cool 5-bit / cold 3-bit
- Quota enforcement: 115 GB hard cap; emergency eviction
- Witness chain Merkle compression for chains > 90 days

Scheduler integration now has:
- An in-process nightly GC scheduler in the MCP daemon that runs `gc_run_nightly` at 03:00 local time.
- An `emergency_eviction_tick` in the self-optimization scheduler that runs every 10 minutes to check quota and invokes incremental eviction when the threshold is exceeded.

Still missing: production threshold validation, operator-visible dashboard/runbook surfaces, and the Phase E policy that decides when quota pressure should also pause writes or alert.

---

## 12. Witness-chain integrity (shipped)

`heal::integrity` (shipped). Checks the witness chain hash links every 10000 attempts. If a corruption is detected:
1. Attempt auto-repair via `optimizer_witness_chain_repair_legacy` (shipped).
2. If repair fails, freeze further panel writes and alert operator.
3. Read-side operations continue (predictions still serve from existing weights).

The scheduler invokes integrity check via a separate ticker (every 10000 observations, not time-based).

---

## 13. The operator's narrowed role

After the self-healing scheduler ships, the operator's role narrows from "monitor and intervene" to "set policy and respond to alerts." Specifically:

| Operator action | Frequency | Purpose |
|---|---|---|
| Approve Catastrophic-class A/B/C promotions | as triggered (rare) | guard against regression on rare events |
| Review weekly evaluation report | weekly | confirm ship-gate metrics, per-(category × language) trends |
| Triage active-learning queue (top-N labels) | weekly or as queue grows | provide gold-standard labels for hard examples |
| Inspect pairwise MI matrix when audit flags slot | quarterly or as flagged | decide whether to retire / recalibrate / accept |
| Adjust scheduler tick periods, drift thresholds, severity bumps | as needed | policy tuning |
| Respond to high-severity alerts (catastrophic drift, witness corruption, subscriber flapping) | as triggered | manual intervention |
| Run `mejepa_rollback_to` if a recent promotion turns out badly | rare | safety override |

**The operator does not need to:**
- Manually trigger LoRA refreshes
- Manually trigger calibration recalibrations
- Manually trigger constellation refits
- Manually trigger continual backprop
- Manually run nightly GC
- Manually run weekly eval (unless ad-hoc)
- Manually approve non-catastrophic promotions

---

## 14. Verifying self-healing works (the operator's reasonable doubt)

Per `01_VISION_TRUTH_SYSTEM.md §8` falsifiability commitments, the test for whether self-healing actually works:

| Falsifier | What it'd mean |
|---|---|
| Self-healing detects soft drift but A/B/C never produces a winner | LoRA / calibration interventions are insufficient for the drift class; needs richer intervention |
| Operator must manually trigger A/B/C in any non-catastrophic case | scheduler is broken; promotion gate too strict |
| `prediction_oracle_correlation` regresses post-promotion despite passing the gate | promotion gate metric is wrong; revisit the score function |
| Same drift recurs within 24h of a promotion | data distribution shift > model adaptation rate; need shorter τ_decay or higher k_replay |
| Continual backprop re-inits cause measurable degradation | dormancy threshold too aggressive; tune up |

Each of these is detectable from the weekly evaluation report; if any fires sustainably, the heal config is wrong (not the architecture).

---

## 15. What this self-healing spec commits us to

1. `TASK-HEAL-001-self-optimization-scheduler.md` — the orchestrator over shipped heal modules
2. `TASK-HEAL-002-per-cell-stratified-drift.md` — extend drift detection to per-(category × language)
3. `TASK-HEAL-003-dissipation-detection.md` — UTML §VII.1 detection
4. `TASK-HEAL-004-bayesian-small-window-drift.md` — Beta posterior on small N
5. `TASK-HEAL-005-constellation-refresh-scheduler.md` — wire into scheduler
6. `TASK-HEAL-006-promote-approval-tool.md` — `mejepa_promote_approval` MCP tool
7. `TASK-HEAL-007-emergency-eviction-tick.md` — short-period quota check beyond nightly
8. `TASK-FLYWHEEL-001-mcp-daemon-mode.md` — the supervised process that hosts the scheduler

All listed in [`13_TASK_INDEX.md`](13_TASK_INDEX.md).
