# 12 — Self-Healing Scheduler

Source files covered: `crates/context-graph-mejepa/src/heal/{drift,ewc,promote,full_retrain,integrity,lora_refresh,calibration,distill,regulate,pipeline,drill,store,cf,fsv,scheduler,scheduler_state}.rs`, `crates/context-graph-mejepa-train/src/learning_signal/{continual_backprop,state_transfer}.rs`, `crates/context-graph-mcp/src/daemon.rs`, `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md`.

> **Status**: All 18 `heal/*` modules **shipped** (Phase 5). Phase A scheduler skeleton with 7 tickers shipped. Phase E full autonomous policy is futurebuild.

---

## 1. The 7 supervised tickers

Source: `heal::scheduler::run_self_optimization_scheduler`.

| Ticker | Period | Action | Source |
|---|---|---|---|
| `observe_tick` | 250 ms | Drain pending shifts → `observe()` with EWC++; deadline 200ms | `compiler::observe` |
| `drift_tick` | 60 s | Snapshot drift window → classify Soft/Hard/Catastrophic | `heal::drift` |
| `active_learning_tick` | 600 s | Drain top-N high-priority queue entries (`AgentSurprise` fast-tracks observe) | `eval::queue` |
| `promote_tick` | 3600 s | Run pending LoRA refresh + (if scheduled) full A/B/C retrain | `heal::promote` + `heal::full_retrain` |
| `continual_backprop_tick` | 1800 s | Detect dormant units; reinit if `dormant_fraction > 0.15` | `learning_signal::continual_backprop` |
| `constellation_freshness_tick` | 3600 s | Find stale cells (> 90 days); force-refit | `mejepa_tct::freshness` |
| `emergency_eviction_tick` | 600 s | Inspect quota; invoke incremental eviction if > threshold | `hygiene::quota` |

---

## 2. Drift classes

Source: `heal::drift::classify`.

| Class | Range | Intervention | Promotion gate | Operator role |
|---|---|---|---|---|
| `None` | ≥ 88% | none | n/a | none |
| `Soft` | 88–85% | LoRA refresh on most-violating head | A/B/C on holdout | none |
| `Hard` | 85–80% | Calibration recalibrate + LoRA refresh | A/B/C stricter | none |
| `Catastrophic` | < 80% | Full A/B/C retrain on rolling corpus | A/B/C multi-window stability | **alert + approve before promote** |

Rolling window: 1,000 attempts. Per-cell stratification is futurebuild Phase E (`TASK-HEAL-002`).

---

## 3. EWC++ regularization

Source: `heal::ewc.rs`. Online Fisher matrix with task-boundary detection.

```rust
pub struct FisherSnapshot {
    pub trunk_fisher: Tensor,                      // shared across heads
    pub per_head_fisher: HashMap<HeadId, Tensor>,  // head-specific (futurebuild Phase B)
    pub witness_hash: WitnessHash,
    pub ts: SystemTime,
    pub source_examples_window: (StepId, StepId),
}
```

EWC++ adds a regularization term to the loss:

```
L_ewc = Σ_i Fisher_i × (θ_i − θ_anchor_i)²
```

When a head fast-tracks (LoRA refresh), only its per-head Fisher is consulted; the trunk Fisher protects the shared body.

---

## 4. A/B/C promotion gate

Source: `heal::promote::run` + `heal::full_retrain::run_periodic`.

- **A** = current active model (the "champion")
- **B** = freshly-retrained model on the rolling corpus + most-recent shifts
- **C** = ensemble of A and B (weighted average)

Each evaluated on holdout:

```
score = holdout_correlation - 0.05 · inference_latency_multiplier
```

Example: `C` wins by 0.94 − 0.05·1.4 = 0.87 vs `B`'s 0.93 − 0.05·1.8 = 0.84 vs `A`'s 0.91 − 0.05·1.0 = 0.86 → `C` promoted.

Active pointer (`CF_MEJEPA_ACTIVE_POINTERS`) atomically updated. Previous active retained as rollback target for 30 days. `mejepa_rollback_to` MCP tool for operator-forced rollback.

**Catastrophic class requires `mejepa_promote_approval` (futurebuild Phase E)** before replacing A; non-Catastrophic promotions are automatic.

---

## 5. Continual backprop

Source: `learning_signal::continual_backprop::{detect_dormant_units, reinit_dormant_units}`. Detector identifies units (ReLU pre-activations, attention heads) near-zero on > 99% of recent batches. Re-init applies a small Gaussian to incoming weights and resets output bias.

| Step | Action |
|---|---|
| 1 | Snapshot recent batch activations |
| 2 | Per unit: compute zero-fraction over recent batches |
| 3 | Flag units with zero_frac > 0.99 |
| 4 | If `dormant_fraction > 0.15`, queue reinit |
| 5 | Reinit + write new weights to `CF_MEJEPA_WEIGHT_BLOBS` |
| 6 | A/B/C evaluates whether reinit helped |

Operational closure of UTML §V.1 effective plasticity (REQ-UTML-07, shipped).

---

## 6. Calibration recalibration

Source: `heal::calibration::recalibrate`. Re-fits the conformal predictor:

```
1. Pull last 1000 (prediction, oracle_outcome) tuples
2. Compute non-conformity scores per SplitConformal
3. Fit new threshold so empirical coverage = stated target (0.90)
4. Persist new ConformalCalibration (versioned)
5. Update active pointer
```

Triggered when ECE > 3% sustained over 10k attempts. Runs in milliseconds.

---

## 7. LoRA refresh

Source: `heal::lora_refresh::run`. Per-head only (the most-violating head at a time):

```
1. Sample N examples weighted by sampling_weight (favors hard recent)
2. Init LoRA adapter (rank r=16; per-head)
3. Fine-tune 1 epoch with EWC++
4. Eval on holdout vs no-LoRA baseline
5. If improved > 1%, promote; else discard
```

---

## 8. Full retrain

Source: `heal::full_retrain::run_periodic`. Triggers:

- Catastrophic drift detected, OR
- 10k attempts since last full retrain (configurable), OR
- Operator: `mejepa_eval_run --force-full-retrain`

```
1. Snapshot current active weights → C variant
2. Train fresh weights from scratch on (Phase 0 corpus + rolling-window + agent-feedback gold)
3. EWC++-regularized fine-tune to anchor on prior consolidated knowledge
4. A/B/C evaluate; promote winner (Catastrophic requires operator approval)
```

---

## 9. Self-healing storage

Source: `crates/context-graph-mejepa-hygiene/`. See [`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md).

| Pathology | Detection | Intervention |
|---|---|---|
| Storage > 110 GB | quota_status @ 600s tick | Emergency eviction |
| Storage > 115 GB hard | quota emergency | Aggressive cold-tier down |
| Witness chain corruption | integrity @ 10k attempts | Auto-repair attempt; if fail, freeze writes + operator alert |
| Constellation stale > 90d | freshness @ 3600s tick | Force-refit |
| Subscriber lag > 500/min | subscriber_status @ 60s tick | Auto-restart with backoff |
| Scheduler task panic | JoinSet supervisor | Restart with exp. backoff; > 5/5min → `MEJEPA_SCHEDULER_FLAPPING` |
| Pairwise MI > 0.85 (redundancy) | weekly MI audit | Flag slot; operator decides |

---

## 10. Operator-facing tools

| Tool | Purpose | Status |
|---|---|---|
| `mejepa_heal_status` | drift level + queued interventions + last A/B/C result | shipped |
| `mejepa_rollback_to` | force rollback to prior model | shipped |
| `mejepa_promote_approval` | approve Catastrophic A/B/C | futurebuild Phase E |
| `mejepa_active_learning_queue` | view + label queue entries | shipped |
| `mejepa_eval_run` | force holdout eval | shipped |

---

## 11. Dissipation detection (UTML §VII.1)

Per `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md §10` + `TASK-HEAL-003`:

```
Dissipating ≡ ( mean(L_step) > 0.3 )
              AND ( ΔK or ΔΞ silently degrading; validation generalization gap widening )
              AND ( ΔM / mean(L_step) < 0.2 )
```

Detection: rolling training loss trend vs rolling holdout-correlation trend. If training loss falls > 5%/1k steps but holdout flat or decreasing → `MEJEPA_DISSIPATION_DETECTED`.

Intervention: replay-buffer enrichment + EWC re-anchor + LR annealing/warmup reset + continual backprop reinit + flywheel data injection.

---

## 12. Operator's narrowed role

After scheduler ships fully autonomous:

| Operator action | Frequency | Purpose |
|---|---|---|
| Approve Catastrophic promotions | as triggered (rare) | guard against regression |
| Review weekly eval report | weekly | per-cell ship-gate, drift, agent-surprise summary |
| Triage active-learning top-N | weekly | provide gold labels |
| Inspect pairwise MI when flagged | quarterly | retire / recalibrate / accept |
| Tune scheduler tick periods, thresholds | as needed | policy |
| Respond to high-severity alerts | as triggered | catastrophic drift, witness corruption, flapping |
| `mejepa_rollback_to` if promotion turns out badly | rare | safety override |

**The operator does NOT manually trigger**: LoRA refresh, calibration, constellation refit, continual backprop, nightly GC, weekly eval, non-catastrophic promotions.

---

## 13. Falsifiers

Per `docs/futurebuild/09_SELF_HEALING_AUTONOMY.md §14`:

| Falsifier | Diagnosis |
|---|---|
| Self-healing detects soft drift but A/B/C never produces a winner | LoRA / calibration insufficient; need richer intervention |
| Operator forced to manually trigger A/B/C non-catastrophically | scheduler broken; promotion gate too strict |
| Correlation regresses post-promotion despite passing gate | promotion gate metric wrong; revisit |
| Same drift recurs within 24h of promotion | data distribution shift > model adaptation rate; need shorter τ_decay or higher k_replay |
| Continual backprop reinits cause measurable degradation | dormancy threshold too aggressive; tune up |

---

## 14. FSV evidence

| FSV | Output |
|---|---|
| `mejepa-heal-drill` binary | synthetic drift drill detects within 1,000 obs + promotes mode-winner |
| `phase5_self_healing_fsv` | hard/soft/catastrophic drills; evidence files mode 0600 |
| Invalid max-observations edge | exits with `MEJEPA_HEAL_INVALID_STATE`; no output root |

---

## 15. Cross-references

[`10_training_loop_utml.md`](10_training_loop_utml.md), [`13_data_flywheel.md`](13_data_flywheel.md), [`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md), [`22_operator_runbook.md`](22_operator_runbook.md).
