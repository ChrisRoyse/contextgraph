# utml-learning-formula — UTML v1.0 Task Index

> **📊 2026-05-10 STATUS — partial.** `compute_delta_p`, `compute_delta_k`, `compute_delta_omega`, `compute_delta_xi`, `compute_l_step` shipped in `crates/context-graph-mejepa-train/src/learning_signal.rs`. `BatchSampler` with `sampling_weight` reweighting + EMA running gradient mean shipped. Per-step training certificate with all 4 UTML factors shipped. Pairwise MI audit (REQ-UTML-06), continual-backprop dormant-unit re-init (REQ-UTML-07), and state-transfer T detector (REQ-UTML-08) pending. Authoritative status: ../../implementation-plan.md Phase 3+.

> **Level 4.** Atomic task specifications for the **UTML domain** (cross-cutting Phase 3 sub-track: four-factor learning signal `L_step = ΔP × ΔK × ΔΩ × ΔΞ`, sampling weight, training certificate, cycle classifier, consolidation integral M(t), state-transfer T). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-UTML.md` (Level 2) > `../../technical/TECH-UTML.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 12 functional `REQ-UTML-NN` requirements (cross-linked in `../_traceability.md` under `### UTML`) and all 11 component contracts in `../../technical/TECH-UTML.md`. Foundation -> Logic -> Surface ordering enforced; Iron Rule applies.

**Anchor invariants from `../../constitution.md` `<utml_v1_invariants>`:**

- Multiplicative gating (`L_step = ΔP × ΔK × ΔΩ × ΔΞ`) is non-negotiable.
- All four factors clamped to `[0, 1]`.
- `ΔΞ.target_collapse == 0.0` for ME-JEPA by construction; nonzero observation = `MEJEPA_INSTRUMENT_GRADIENT_LEAK` CRITICAL bug.
- Coaching order: regulate ΔΩ + ΔΞ FIRST, then ΔP/ΔK.
- Pruning rule: prune curriculum item only if `L_step < 0.1` for N steps AND `ΔΩ > 0.4` throughout AND `ΔΞ > 0.4` throughout.
- Per-step training certificate with hash-chain (`parent_witness_hash → self_hash`) is the physical M(t) consolidation tensor (per UTML §XII).

---

## Overview

- **Total tasks:** 10
- **Foundation layer:** 2 tasks (TASK-UTML-001..002)
- **Logic layer:** 7 tasks (TASK-UTML-003..009)
- **Surface layer:** 1 task (TASK-UTML-010)
- **Crate touched:** `crates/context-graph-mejepa-train` (NEW; under `src/learning_signal/` + `src/cert/`)
- **Estimated walltime:** ~5 days end-to-end
- **All UTML compute functions return `Result<f32, UtmlError>`; clamped output in `[0, 1]`.**
- **TRAINING tasks depend on these (cross-phase dependency).**

---

## Dependency DAG

```mermaid
graph TD
    subgraph Foundation [Foundation Layer]
        T001[TASK-UTML-001<br/>Error+Signal+Cert+Components data models]
        T002[TASK-UTML-002<br/>SkipReason + UtmlState + ClassifierVerdict enums]
    end

    subgraph Logic [Logic Layer]
        T003[TASK-UTML-003<br/>compute_delta_p + per-chunk aggregator]
        T004[TASK-UTML-004<br/>compute_delta_k + EMA grad update]
        T005[TASK-UTML-005<br/>compute_delta_omega + sub-functions]
        T006[TASK-UTML-006<br/>compute_delta_xi (target_collapse assert)]
        T007[TASK-UTML-007<br/>compute_l_step + sampling_weight]
        T008[TASK-UTML-008<br/>EmbedderCoherence + pairwise MI audit]
        T009[TASK-UTML-009<br/>UtmlClassifier + prune_decision + Consolidation M(t) + StateTransfer T + continual-backprop dormant re-init]
    end

    subgraph Surface [Surface Layer]
        T010[TASK-UTML-010<br/>FSV: 100 synthetic examples + L_step envelope assertions]
    end

    T001 --> T002
    T001 --> T003
    T001 --> T004
    T001 --> T005
    T001 --> T006
    T002 --> T009
    T003 --> T007
    T004 --> T007
    T005 --> T007
    T006 --> T007
    T007 --> T008
    T007 --> T009
    T008 --> T009
    T009 --> T010
    T007 --> T010
```

---

## Execution order

| # | Task ID | Title | Layer | Depends on | Status |
|---|---------|-------|-------|------------|--------|
| 1 | TASK-UTML-001 | Data models: errors, signal, cert, ΔP/ΔK/ΔΩ/ΔΞ component structs | foundation | — | ready |
| 2 | TASK-UTML-002 | Enums: SkipReason + UtmlState + ClassifierVerdict + DeltaPAggregator | foundation | TASK-UTML-001 | ready |
| 3 | TASK-UTML-003 | `compute_delta_p` + per-chunk aggregation | logic | TASK-UTML-001 | blocked |
| 4 | TASK-UTML-004 | `compute_delta_k` + `update_grad_ema` | logic | TASK-UTML-001 | blocked |
| 5 | TASK-UTML-005 | `compute_delta_omega` + sub-functions (plasticity / landscape / stability / agent_state) | logic | TASK-UTML-001 | blocked |
| 6 | TASK-UTML-006 | `compute_delta_xi` (CRITICAL `target_collapse == 0.0` assert) | logic | TASK-UTML-001 | blocked |
| 7 | TASK-UTML-007 | `compute_l_step` + `sampling_weight` + drop/force gates | logic | TASK-UTML-003..006 | blocked |
| 8 | TASK-UTML-008 | EmbedderCoherence + pairwise-MI audit job | logic | TASK-UTML-007 | blocked |
| 9 | TASK-UTML-009 | UtmlClassifier (7 states) + prune_decision + Consolidation M(t) + StateTransfer T + continual-backprop dormant-unit re-init | logic | TASK-UTML-007, TASK-UTML-008, TASK-UTML-002 | blocked |
| 10 | TASK-UTML-010 | FSV harness: 100 synthetic examples spanning corner cases; L_step envelope + target_collapse always 0 + sampling histogram | surface | TASK-UTML-007, TASK-UTML-009 | blocked |

---

## Status legend

- ready — Can be started immediately (all dependencies complete)
- blocked — Has unmet dependencies
- in_progress — Currently being executed
- complete — Shipped + FSV passed
- failed — Needs revision

---

## Critical path

`TASK-UTML-001 → TASK-UTML-006 → TASK-UTML-007 → TASK-UTML-009 → TASK-UTML-010`

ΔΞ's `target_collapse == 0.0` assertion is the architectural integrity check of ME-JEPA; it must ship before `compute_l_step` so the multiplication is gated by the architectural property at every step.

---

## Parallel batches

- **Batch 1 (after TASK-UTML-001 ships):** TASK-UTML-002, TASK-UTML-003, TASK-UTML-004, TASK-UTML-005, TASK-UTML-006 (independent compute functions; share data models only).
- **Batch 2 (after Batch 1 ships):** TASK-UTML-007 alone (consumes all four ΔX).
- **Batch 3 (after TASK-UTML-007 ships):** TASK-UTML-008 alone.
- **Batch 4 (after TASK-UTML-008 ships):** TASK-UTML-009 alone.
- **Batch 5 (after TASK-UTML-009 ships):** TASK-UTML-010 (FSV).

---

## Cross-phase consumers

`crates/context-graph-mejepa-train::trainer` (TRAINING domain) consumes every UTML compute function per step. The TRAINING phase tasks have explicit `<depends_on>` references back into this phase.

---

## FSV evidence locations

Per `../../constitution.md` `<testing_requirements>` and `<fsv_evidence_paths>`:

- `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json` — 100 synthetic examples spanning corner cases (TASK-UTML-010)
- `/tmp/contextgraph-mejepa-train-fsv/utml-classifier-evidence.json` — 5 synthetic regimes (TASK-UTML-009)
- `/tmp/contextgraph-mejepa-train-fsv/utml-cert-chain-evidence.json` — 100,000-step chain replay smoke (TASK-UTML-007 emits cert; TRAINING-009 owns full integration)
- `/tmp/contextgraph-mejepa-train-fsv/utml-pairwise-mi-matrix.csv` — pairwise MI audit (TASK-UTML-008)
