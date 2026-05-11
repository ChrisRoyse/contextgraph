# SPEC-HEAL — Functional Specification

> **Level 2.** Defines WHAT the autonomous self-healing scheduler must do.
> Owns: REQ-HEAL-FB-01..15.
> Implementation reference: [`09_SELF_HEALING_AUTONOMY.md`](../../09_SELF_HEALING_AUTONOMY.md). Extends Phase 5 SPEC-HEAL (shipped) per `docs/ruvectorfindings/specs/`.

---

## User stories

- **As an operator**, I want ME-JEPA to detect drift and recover automatically without me pulling.
- **As a maintainer**, I want catastrophic-class events surfaced for explicit approval so the system never silently regresses on rare events.
- **As a researcher**, I want per-cell stratified drift so a single language regression doesn't trigger global retraining.

## Requirements

### REQ-HEAL-FB-01 — Continuous self-optimization scheduler

A new tokio task `run_self_optimization_scheduler` SHALL fire 5 tickers per `09 §2`: observe (250ms), drift_check (60s), active_learning (600s), promote (3600s), continual_backprop (1800s), constellation_freshness (3600s).

### REQ-HEAL-FB-02 — Per-cell stratified drift detection

The drift detector SHALL track rolling accuracy per (mutation_category × language) cell separately, in addition to the global rolling window.

### REQ-HEAL-FB-03 — Bayesian small-window posterior

When a per-cell sample count is < 50, the scheduler SHALL use a Beta posterior over accuracy rather than the point estimate, and SHALL not fire on noise (require posterior 95% CI to fall below threshold before queuing intervention).

### REQ-HEAL-FB-04 — Surprise-event-weighted drift

Agent-flagged Surprise events SHALL count as 2× weight in the rolling drift window. Catastrophic surprises count as 4×.

### REQ-HEAL-FB-05 — Dissipation detection

The scheduler SHALL detect the "training falls but holdout doesn't" failure mode (UTML §VII.1):
- mean(L_step) over last 1000 updates > 0.3
- ΔK or ΔΞ silently degrading
- ΔM / mean(L_step) < 0.2 over equivalent eval window

Fires `MEJEPA_DISSIPATION_DETECTED`. Triggers replay-buffer enrichment, EWC re-anchor, LR reset, continual-backprop re-init.

### REQ-HEAL-FB-06 — Per-head dormancy detection

`detect_dormant_units_per_head` SHALL run per-head detection. A head's dormancy > 15% triggers reinit on that head's projection layer only (not the trunk).

### REQ-HEAL-FB-07 — Per-head λ ramp

When a head's holdout metric crosses its ramp gate (per `10 §8`), the scheduler SHALL increment λ_h by 0.05 per week up to the steady-state value.

### REQ-HEAL-FB-08 — `mejepa_promote_approval` MCP tool

A new MCP tool SHALL allow the operator to approve or reject a queued catastrophic A/B/C promotion. Catastrophic promotions block until approved; non-catastrophic auto-promote.

### REQ-HEAL-FB-09 — Constellation refresh trigger

The scheduler's constellation_freshness_tick SHALL refresh constellation cells when:
- Cell age > 90 days, OR
- Cell has > 100 new training examples since last refresh, OR
- Per-embedder threshold τ_m calibration drifts > 3% on rolling validation.

### REQ-HEAL-FB-10 — Emergency eviction tick

In addition to the nightly GC at 03:00, a 10-minute tick SHALL check `quota_status` and trigger emergency eviction when usage > 110 GB.

### REQ-HEAL-FB-11 — Per-cell promotion criterion

A promotion (B or C wins over A) SHALL be accepted only if every (category × language) cell holds or improves — not just the average. A cell-regressing promotion is rejected.

### REQ-HEAL-FB-12 — A/B/C three-way evaluation

Promotion candidates SHALL include A (current), B (fresh retrain), C (ensemble of A and B). All three evaluated on holdout; winner picked by `score = holdout_correlation - 0.05 × inference_latency_multiplier`.

### REQ-HEAL-FB-13 — Rollback retention

Active pointers SHALL retain the previous active for 30 days as a rollback target. `mejepa_rollback_to` (shipped) lets operator force rollback.

### REQ-HEAL-FB-14 — Operator alert stream

Catastrophic-class events SHALL write to `CF_MEJEPA_HEAL_REPORTS` with `severity=Catastrophic` AND emit to a separate operator-visible alert stream (e.g., a top-of-`weekly.md` section).

### REQ-HEAL-FB-15 — Witness-chain integrity check

The scheduler SHALL run witness-chain integrity check every 10000 attempts (event-counted, not time-based) per existing Phase 5; on corruption, attempt auto-repair, then alert + freeze writes if repair fails.

## Acceptance criteria

1. Inject synthetic drift (90% → 84% over 1000 obs); scheduler detects within next 1k obs; queues LoRA refresh; A/B/C runs; winner promoted automatically; per-cell breakdown shows the affected cell improved.
2. Inject catastrophic drift (90% → 75%); scheduler queues full retrain + writes operator alert; promotion blocked until `mejepa_promote_approval` called.
3. Inject dissipation pattern (low ΔΞ, high training-but-no-validation gain); detector fires `MEJEPA_DISSIPATION_DETECTED`; replay-buffer enrichment runs.
4. Inject dormancy in one head's projection (force 20% units to zero pre-activation); per-head detector fires; reinit runs on that head only.
5. λ ramping observed: head F1 crosses ramp gate; next week's λ_h = previous + 0.05.
6. Witness-chain corruption injected (manually mutate a CF_MEJEPA_TRAIN_CERTS row); integrity check fires; auto-repair attempted; if repair fails, writes paused + alert written.

## Edge cases

- **All cells passing ship gate**: scheduler observes idle; no interventions queued; recommendation: `no_change` from any health checks.
- **Drift detected but A and B and C all fail to beat the gate**: log `MEJEPA_PROMOTION_NO_WINNER`; retain A; do not regress.
- **Operator approval times out (no decision in 24h)**: catastrophic promotion stays queued; reminder alert escalated daily; no auto-promotion.
- **Storage at exactly 110 GB**: emergency eviction fires once; subsequent ticks no-op until 1 GB headroom restored.

## Non-functional

- Scheduler memory overhead < 200 MB.
- Drift detection per-tick CPU < 100ms.
- A/B/C full-retrain wall-time bounded by configurable budget (default 4 hours).
- Promotion gate evaluation < 60s (small holdout split).
