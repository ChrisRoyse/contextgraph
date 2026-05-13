# SPEC-HEAL — Functional Specification

> **Level 2.** Defines WHAT the autonomous self-healing scheduler must do.
> Owns: REQ-HEAL-FB-01..15.
> Implementation reference: [`09_SELF_HEALING_AUTONOMY.md`](../../09_SELF_HEALING_AUTONOMY.md). Extends Phase 5 SPEC-HEAL (shipped) per `docs/ruvectorfindings/specs/`.

**Status snapshot (2026-05-13):** REQ-HEAL-FB-01..15 are code/FSV shipped for Phase E. The durable source of truth for policy decisions is `CF_MEJEPA_MODEL_PROMOTIONS`; intervention reports land in `CF_MEJEPA_HEAL_REPORTS`; drift windows are read from `CF_MEJEPA_DRIFT_WINDOW`; scheduler execution state is read from `self_optimization_status.json`. Evidence: `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`.

---

## User stories

- **As an operator**, I want ME-JEPA to detect drift and recover automatically without me pulling.
- **As a maintainer**, I want catastrophic-class events surfaced for explicit approval so the system never silently regresses on rare events.
- **As a researcher**, I want per-cell stratified drift so a single language regression doesn't trigger global retraining.

## Requirements

### REQ-HEAL-FB-01 — Continuous self-optimization scheduler

A tokio task `run_self_optimization_scheduler` SHALL fire 7 tickers per `09 §2`: observe (250ms), drift_check (60s), active_learning (600s), promote (3600s), continual_backprop (1800s), constellation_freshness (3600s), emergency_eviction (600s).

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

A new MCP tool SHALL allow the operator to approve or reject a queued catastrophic A/B/C promotion. Catastrophic promotions block until two distinct operator approvals are durably recorded; non-catastrophic promotions auto-promote through the A/B/C gate. Duplicate operator approvals, invalid operator IDs, empty reasons, and downgrade attempts on the required approval count fail closed.

### REQ-HEAL-FB-09 — Constellation refresh trigger

The scheduler's constellation_freshness_tick SHALL refresh constellation cells when:
- Cell age > 90 days, OR
- Cell has > 100 new training examples since last refresh, OR
- Per-embedder threshold τ_m calibration drifts > 3% on rolling validation.

### REQ-HEAL-FB-10 — Emergency eviction tick

In addition to the nightly GC at 03:00, a 10-minute tick SHALL check `quota_status` and trigger emergency eviction when usage is at or above the 110 GiB threshold.

### REQ-HEAL-FB-11 — Per-cell promotion criterion

A promotion (B or C wins over A) SHALL be accepted only if every (category × language) cell holds or improves — not just the average. A cell-regressing promotion is rejected.

### REQ-HEAL-FB-12 — A/B/C three-way evaluation

Promotion candidates SHALL include A (current), B (fresh retrain), C (ensemble of A and B). All three evaluated on holdout; winner picked by `score = holdout_correlation - 0.05 × inference_latency_multiplier`.

### REQ-HEAL-FB-13 — Rollback retention

Active pointers SHALL retain the previous active for 30 days as a rollback target. `mejepa_rollback_to` (shipped) lets operator force rollback.

### REQ-HEAL-FB-14 — Operator alert stream

Catastrophic-class events SHALL write intervention evidence to `CF_MEJEPA_HEAL_REPORTS` and emit a separate operator-visible alert record to the policy ledger under `CF_MEJEPA_MODEL_PROMOTIONS`. Alert records include severity, title/body, attribution, creation time, and the source-of-truth CF that persisted them.

### REQ-HEAL-FB-15 — Witness-chain integrity check

The scheduler SHALL run witness-chain integrity check every 10000 attempts (event-counted, not time-based) per existing Phase 5; on corruption, attempt auto-repair, then alert + freeze writes if repair fails.

## Acceptance criteria

1. Persist a controlled soft-drift window (`860/1000`) to `CF_MEJEPA_DRIFT_WINDOW`; scheduler detects within the next tick, queues LoRA refresh, runs A/B/C, persists the winner, and readback shows `CF_MEJEPA_HEAL_REPORTS` plus policy rows.
2. Persist a controlled catastrophic-drift window (`750/1000`); scheduler queues full retrain, writes an operator alert, and promotion remains blocked until two distinct `mejepa_promote_approval` approvals are read back.
3. Persist controlled dissipation evidence (high L_step with degrading ΔΞ/holdout); detector fires `MEJEPA_DISSIPATION_DETECTED`; replay-buffer/EWC/LR interventions are persisted.
4. Persist dormancy evidence in one head's projection; per-head detector fires and reinit touches that head only.
5. λ ramping is observed by reading back the persisted lambda-ramp state; a head crossing its gate increments λ_h by 0.05 up to the configured ceiling.
6. Witness-chain integrity runs every 10000 observations; corruption attempts trigger auto-repair and, on repair failure, write-pause plus alert.

## Edge cases

- **All cells passing ship gate**: scheduler observes idle; no interventions queued; recommendation: `no_change` from any health checks.
- **Drift detected but A and B and C all fail to beat the gate**: log `MEJEPA_PROMOTION_NO_WINNER`; retain A; do not regress.
- **Operator approval times out (no decision in 24h)**: catastrophic promotion stays queued; reminder alert escalated daily; no auto-promotion.
- **Invalid approval request**: invalid operator ID, empty reason, duplicate operator, and approval-count downgrade attempts return a structured heal error and leave the persisted pending-promotion row unchanged.
- **Storage at exactly 110 GB**: emergency eviction fires once; subsequent ticks no-op until 1 GB headroom restored.

## Non-functional

- Scheduler memory overhead < 200 MB.
- Drift detection per-tick CPU < 100ms.
- A/B/C full-retrain wall-time bounded by configurable budget (default 4 hours).
- Promotion gate evaluation < 60s (small holdout split).
