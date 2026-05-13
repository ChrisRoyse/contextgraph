# Heal domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

**Status snapshot (2026-05-13):** Phase E is code/FSV shipped. The scheduler policy layer now consumes persisted drift windows, per-cell observations, train certs, eval reports, active-learning entries, production telemetry, and quota state; it writes durable policy/evidence rows to `CF_MEJEPA_MODEL_PROMOTIONS`, `CF_MEJEPA_HEAL_REPORTS`, `CF_MEJEPA_ACTIVE_POINTERS`, `CF_MEJEPA_WEIGHT_BLOBS`, and `self_optimization_status.json`. Evidence: `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`.

## TASK-HEAL-001 — Self-optimization scheduler (full implementation)

- **Phase:** E · **Effort:** 1.5d · **Blocked by:** TASK-FLYWHEEL-002 · **Spec:** TECH-HEAL §3 + SPEC-HEAL REQ-HEAL-FB-01
- **Goal:** Replace the FLYWHEEL-002 skeleton with full implementation that calls all the heal/* modules per `09_SELF_HEALING_AUTONOMY.md §2` + `TECH-HEAL §3`.
- **Outputs:** `crates/context-graph-mejepa/src/heal/scheduler.rs` (full); `scheduler_state.rs` extensions.
- **Status:** complete/code+FSV.
- **DoD:** all 7 tickers call expected heal modules; FSV verifies by independent readback of RocksDB CFs and scheduler status, not return values.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-phase-e-fsv/phase_e_self_optimization_fsv.json`

## TASK-HEAL-002 — Per-cell stratified drift detection

- **Phase:** E · **Effort:** 1d · **Blocked by:** TASK-HEAL-001 · **Spec:** SPEC-HEAL REQ-HEAL-FB-02 + TECH-HEAL §4
- **Goal:** Track drift per (mutation_category × language) cell separately. Per-cell soft drift triggers per-cell LoRA refresh.
- **Outputs:** `heal/drift_per_cell.rs` (NEW).
- **Status:** complete/code+FSV.
- **DoD:** persisted per-cell drift observations fire per-cell intervention; noisy 7/10 small-window evidence does not fire; global state is inspected before/after.

## TASK-HEAL-003 — Dissipation detection

- **Phase:** E · **Effort:** 1d · **Blocked by:** TASK-HEAL-001, TASK-UTML-008
- **Goal:** UTML §VII.1 detection per `TECH-HEAL §6`. mean(L_step) > 0.3 + ΔK or ΔΞ degrading + ΔM/mean(L_step) < 0.2 → fire.
- **Outputs:** `heal/dissipation.rs` (NEW).
- **Status:** complete/code+FSV.
- **DoD:** controlled train-cert/eval evidence fires within expected window; replay/EWC/LR interventions are persisted and read back.

## TASK-HEAL-004 — Bayesian small-window drift posterior

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-002 · **Spec:** SPEC-HEAL REQ-HEAL-FB-03 + TECH-HEAL §5
- **Goal:** When per-cell N < 50, use Beta posterior. Don't fire on noise.
- **Outputs:** `heal/drift_bayesian.rs` (NEW).
- **Status:** complete/code+FSV.
- **DoD:** N=10 with 7/10 PASS → no fire; N=10 with 0/10 PASS → fire (catastrophic posterior).

## TASK-HEAL-005 — Constellation refresh trigger wired into scheduler

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-001
- **Goal:** Per `09 §11`. Refresh stale cells > 90 days OR > 100 new examples since last refresh OR τ_m drift > 3%.
- **Outputs:** `heal/scheduler_state.rs::tick_constellation_freshness` extension.
- **Status:** complete/code+FSV.
- **DoD:** stale cells, train-cert count thresholds, and τ drift persist refresh policy records with readback.

## TASK-HEAL-006 — `mejepa_promote_approval` MCP tool

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-001 · **Spec:** SPEC-HEAL REQ-HEAL-FB-08
- **Goal:** Operator-callable tool to approve / reject queued catastrophic A/B/C promotions.
- **Outputs:** `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs::handle_promote_approval`.
- **Status:** complete/code+FSV.
- **DoD:** queued catastrophic promotion blocks until two distinct approvals; duplicate operators and invalid operator IDs fail closed; rejection path persists with reason.

## TASK-HEAL-007 — Emergency eviction tick

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** none
- **Goal:** Per `SPEC-HEAL REQ-HEAL-FB-10`. 10-minute tick checks quota; if > 110 GB, emergency eviction.
- **Outputs:** `heal/emergency_eviction.rs` (NEW); scheduler tick wiring.
- **Status:** complete/code+FSV.
- **DoD:** persisted exact-threshold 110 GB usage fires once; repeated same usage suppresses duplicate action until usage changes.

## TASK-HEAL-008 — Drift attribution (D4)

- **Phase:** E · **Effort:** 0.5d · **Blocked by:** TASK-HEAL-002
- **Goal:** When drift fires, classify root cause: data shift / model staleness / spec change / per-cell distribution shift.
- **Outputs:** `heal/drift_attribution.rs` (NEW).
- **Status:** complete/code+FSV.
- **DoD:** controlled drift events classify to data shift / model staleness / spec change / per-cell distribution shift; alerts persist attribution evidence to the policy ledger.
