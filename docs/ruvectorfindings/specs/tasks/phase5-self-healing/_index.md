# Phase 5 — SELF-HEALING Task Index

> **Level 4.** Atomic task specifications for the **SELF-HEALING domain** (Phase 5: online observe with EWC++, drift detection, A/B/C retrain promotion, periodic full retrain, continuous calibration, witness-chain integrity check, LoRA adapter refresh, distillation-based online observe). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-SELF-HEALING.md` (Level 2) > `../../technical/TECH-SELF-HEALING.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 20 functional REQ-HEAL-NN requirements (cross-linked in `../_traceability.md` under `### SELF-HEALING`) and all 7 component contracts (`SelfHealingPipeline`, `EwcPlusPlus`, `DriftDetector`, `AbcPromoter`, `LoraRefresher`, `OnlineDistiller`, `ChainIntegrityChecker`) in `../../technical/TECH-SELF-HEALING.md`. Foundation → Logic → Surface ordering enforced; Iron Rule applies (Task N cannot reference files created by a later task).

**Cross-cutting integration:** SELF-HEALING is fed by Phase 7 SHIFT-SUBSCRIBER (the sole `observe()` caller in normal operation), consumes Phase 4a INFERENCE's RealityPrediction outputs, references Phase 4b TCT for constellation re-fit, references SPEC-UTML's per-step training certs for ΔΩ/ΔΞ ASSESS, and is GC-bounded by Phase 6 HYGIENE (one promotion-cycle retention).

**Reuse:** The shipped `crates/context-graph-embeddings/src/training/lora.rs` and `crates/context-graph-embeddings/src/training/distillation.rs` infrastructure is wrapped (NOT reinvented) by `LoraRefresher` and `OnlineDistiller`. The shipped `crates/context-graph-witness` machinery (verify_chain_integrity, read_witness_chain, write_text_checked, witness_chain_legacy::reconcile, witness_chain_repair) is wrapped by `ChainIntegrityChecker`.

**Crate created:** `crates/context-graph-mejepa/` (NEW for Phase 5; `src/heal/` sub-module). The `bin/mejepa-heal-drill.rs` binary is the canonical FSV harness.

---

## Overview

- **Total tasks:** 19
- **Foundation layer:** 5 tasks (TASK-HEAL-001..005)
- **Logic layer:** 10 tasks (TASK-HEAL-006..015)
- **Surface layer:** 4 tasks (TASK-HEAL-016..019)
- **Estimated walltime:** ~5–7 days end-to-end (per master plan Phase 5 budget)
- **Sub-module:** `crates/context-graph-mejepa/src/heal/`

---

## Dependency DAG

```mermaid
graph TD
    subgraph Foundation [Foundation Layer]
        T001[TASK-HEAL-001<br/>EWC++ data models<br/>EwcPlusPlus, FisherMatrix,<br/>TaskSnapshot, BoundaryDetectorState]
        T002[TASK-HEAL-002<br/>Drift data models<br/>DriftDetector, DriftSample,<br/>SeverityTable, DriftSeverity]
        T003[TASK-HEAL-003<br/>A/B/C promotion data models<br/>AbcPromoter, ModelHandle,<br/>EnsembleHandle, HoldoutEval, PromotionGate]
        T004[TASK-HEAL-004<br/>HealReport, HealError, IntegrityViolationKind,<br/>CriticalBugKind, LoraRefresher, OnlineDistiller,<br/>ChainIntegrityChecker, CalibrationWriter,<br/>PromotionLockState, HealStatus]
        T005[TASK-HEAL-005<br/>RocksDB CFs<br/>FISHER_SNAPSHOTS, DRIFT_WINDOW, DRIFT_HISTORY,<br/>HEAL_REPORTS, ACTIVE_POINTERS,<br/>CALIBRATION_HISTORY]
    end

    subgraph Logic [Logic Layer]
        T006[TASK-HEAL-006<br/>EwcPlusPlus methods<br/>update_fisher_online, detect_task_boundary,<br/>snapshot_current_task, current_lambda, ewc_penalty]
        T007[TASK-HEAL-007<br/>DriftDetector methods<br/>push, detect_drift,<br/>3-window hysteresis]
        T008[TASK-HEAL-008<br/>SelfHealingPipeline::observe<br/>UTML cycle ASSESS+REGULATE first<br/>+ EWC-regularized SGD step]
        T009[TASK-HEAL-009<br/>AbcPromoter::retrain_and_promote<br/>3-mode parallel + holdout gate<br/>+ witness-chain entry]
        T010[TASK-HEAL-010<br/>AbcPromoter::rollback_to<br/>operator-only<br/>+ ModelRollback witness entry]
        T011[TASK-HEAL-011<br/>schedule_full_retrain<br/>every 10,000 obs<br/>+0.02 floor]
        T012[TASK-HEAL-012<br/>continuous_calibration_if_due<br/>sliding 1,000 examples<br/>τ + σ² recompute]
        T013[TASK-HEAL-013<br/>ChainIntegrityChecker::verify<br/>chain.len() %% 73 + replay<br/>halt + restore on break]
        T014[TASK-HEAL-014<br/>LoraRefresher::refresh<br/>rank=32 epochs=10<br/>frozen-base byte-equal invariant]
        T015[TASK-HEAL-015<br/>OnlineDistiller::consume<br/>InfoNCE + EMA τ=0.996<br/>signal_clarity quality gate]
    end

    subgraph Surface [Surface Layer]
        T016[TASK-HEAL-016<br/>mcp__cgreality__<br/>mejepa_heal_status MCP tool]
        T017[TASK-HEAL-017<br/>mcp__cgreality__<br/>mejepa_rollback_to MCP tool<br/>operator-only]
        T018[TASK-HEAL-018<br/>CLI heal-drill binary<br/>--inject-drift soft|hard|catastrophic]
        T019[TASK-HEAL-019<br/>FSV harness<br/>5 evidence files mode 0o600]
    end

    %% Foundation → Logic
    T001 --> T006
    T002 --> T007
    T003 --> T009
    T003 --> T010
    T004 --> T006
    T004 --> T007
    T004 --> T008
    T004 --> T009
    T004 --> T010
    T004 --> T011
    T004 --> T012
    T004 --> T013
    T004 --> T014
    T004 --> T015
    T005 --> T006
    T005 --> T007
    T005 --> T008
    T005 --> T009
    T005 --> T010
    T005 --> T011
    T005 --> T012

    %% Logic chain (observe orchestrates EWC + Drift)
    T006 --> T008
    T007 --> T008
    T008 --> T009
    T009 --> T010
    T009 --> T011
    T008 --> T013
    T008 --> T014
    T008 --> T015
    T012 --> T008

    %% Logic → Surface
    T008 --> T016
    T009 --> T016
    T010 --> T017
    T009 --> T018
    T013 --> T018
    T014 --> T018
    T006 --> T018
    T018 --> T019
    T013 --> T019
    T014 --> T019
    T009 --> T019
    T006 --> T019
```

---

## Execution order (sequential)

| # | Task ID | Title | Layer | Depends on | Status |
|---|---|---|---|---|---|
| 1 | TASK-HEAL-001 | EWC++ data models (EwcPlusPlus, FisherMatrix, TaskSnapshot, BoundaryDetectorState) | foundation | (cross-domain shipped) | ready |
| 2 | TASK-HEAL-002 | Drift data models (DriftDetector, DriftSample, SeverityTable, DriftSeverity enum) | foundation | (cross-domain shipped) | ready |
| 3 | TASK-HEAL-003 | A/B/C promotion data models (AbcPromoter, ModelHandle, EnsembleHandle, HoldoutEval, PromotionGate) | foundation | (cross-domain shipped) | ready |
| 4 | TASK-HEAL-004 | Pipeline-level data models + errors (HealReport, HealError, IntegrityViolationKind, CriticalBugKind, LoraRefresher, OnlineDistiller, ChainIntegrityChecker, CalibrationWriter, PromotionLockState, HealStatus) | foundation | 001, 002, 003 | ready |
| 5 | TASK-HEAL-005 | RocksDB column families (CF_MEJEPA_FISHER_SNAPSHOTS, DRIFT_WINDOW, DRIFT_HISTORY, HEAL_REPORTS, ACTIVE_POINTERS, CALIBRATION_HISTORY) | foundation | 001, 002, 003, 004 | ready |
| 6 | TASK-HEAL-006 | EwcPlusPlus methods (update_fisher_online, detect_task_boundary, snapshot_current_task, current_lambda, ewc_penalty; defaults λ_initial=2000, λ_max=15000) | logic | 001, 004, 005 | blocked (needs Phase 2 Predictor) |
| 7 | TASK-HEAL-007 | DriftDetector methods (push, detect_drift; rolling 1,000-attempt window; severity table; 3-window hysteresis) | logic | 002, 004, 005 | blocked (needs Phase 4a RealityPrediction) |
| 8 | TASK-HEAL-008 | SelfHealingPipeline::observe (UTML cycle ASSESS+REGULATE first; EWC-regularized SGD step; signal_clarity gating; drift window update) | logic | 004, 005, 006, 007 | blocked |
| 9 | TASK-HEAL-009 | AbcPromoter::retrain_and_promote (3-mode parallel; holdout gate coverage+agreement+OOD; ModelPromote witness entry) | logic | 003, 004, 005, 008 | blocked (needs Phase 3 Training loop) |
| 10 | TASK-HEAL-010 | AbcPromoter::rollback_to (operator-only; ModelRollback witness entry; CF_MEJEPA_WEIGHT_BLOBS lookup) | logic | 003, 004, 005, 009 | blocked |
| 11 | TASK-HEAL-011 | schedule_full_retrain (every 10,000 obs; +0.02 floor; immutable corpus snapshot) | logic | 004, 005, 009 | blocked |
| 12 | TASK-HEAL-012 | continuous_calibration_if_due (every 1,000 obs; sliding window τ + σ² recompute; persist to CF_MEJEPA_CALIBRATION_HISTORY) | logic | 004, 005 | blocked (needs Phase 4a CalibrationRecord) |
| 13 | TASK-HEAL-013 | ChainIntegrityChecker::verify (every 10,000 obs or on drift; chain.len() % 73 + prev_hash replay; halt + restore on break) | logic | 004, 005, 008 | ready (witness-chain shipped) |
| 14 | TASK-HEAL-014 | LoraRefresher::refresh (rank=32 epochs=10; reuses crates/context-graph-embeddings/src/training/lora.rs; frozen-base byte-equal invariant) | logic | 004, 005, 008 | ready (LoRA shipped) |
| 15 | TASK-HEAL-015 | OnlineDistiller::consume (reuses crates/context-graph-embeddings/src/training/distillation.rs; InfoNCE; EMA τ=0.996; signal_clarity gate) | logic | 004, 005, 008 | ready (distillation shipped) |
| 16 | TASK-HEAL-016 | mcp__cgreality__mejepa_heal_status MCP tool (read-only HealStatus snapshot) | surface | 008, 009 | blocked |
| 17 | TASK-HEAL-017 | mcp__cgreality__mejepa_rollback_to MCP tool (operator-only; target_witness_chain_offset) | surface | 010 | blocked |
| 18 | TASK-HEAL-018 | CLI mejepa-heal-drill binary (--inject-drift {soft, hard, catastrophic}) | surface | 006, 009, 013, 014 | blocked |
| 19 | TASK-HEAL-019 | FSV harness (5 evidence files at /tmp/contextgraph-mejepa-self-healing-fsv/, mode 0o600 per SEC-MEJEPA-07) | surface | 006, 009, 013, 014, 018 | blocked |

**Status legend:** `ready` (no Phase-5-internal blockers); `blocked` (waits on a prior task in this phase OR a cross-phase dependency such as Phase 2 Predictor / Phase 3 Training loop / Phase 4a RealityPrediction / CalibrationRecord).

---

## Parallel-execution batches

- **Batch F.1** (foundation, parallel-safe): TASK-HEAL-001, TASK-HEAL-002, TASK-HEAL-003 — three independent data-model files.
- **Batch F.2** (foundation, depends on F.1): TASK-HEAL-004 — collects all error/pipeline-level types.
- **Batch F.3** (foundation, depends on F.2): TASK-HEAL-005 — RocksDB CF descriptors keyed by foundation types.
- **Batch L.1** (logic, depends on F.3 + Phase 2 Predictor): TASK-HEAL-006 (EWC++ methods) and TASK-HEAL-007 (Drift methods) and TASK-HEAL-012 (continuous calibration) — three parallel.
- **Batch L.2** (logic, depends on L.1): TASK-HEAL-008 (observe orchestrator).
- **Batch L.3** (logic, depends on L.2 + Phase 3 Training): TASK-HEAL-009 (A/B/C promote), TASK-HEAL-013 (integrity checker), TASK-HEAL-014 (LoRA refresh), TASK-HEAL-015 (distiller) — four parallel.
- **Batch L.4** (logic, depends on L.3): TASK-HEAL-010 (rollback), TASK-HEAL-011 (full retrain).
- **Batch S.1** (surface, depends on L.4): TASK-HEAL-016, TASK-HEAL-017, TASK-HEAL-018 — three parallel.
- **Batch S.2** (surface, depends on S.1): TASK-HEAL-019 (FSV harness consumes drill output).

---

## Critical path

```
TASK-HEAL-001 → TASK-HEAL-004 → TASK-HEAL-005 → TASK-HEAL-006 → TASK-HEAL-008 → TASK-HEAL-009 → TASK-HEAL-018 → TASK-HEAL-019
```

The longest single-task is TASK-HEAL-009 (A/B/C promotion: full Mode B train_from_scratch on a corpus snapshot is the wallclock long-pole; <60min budget on RTX 5090 per Phase 5 DoD). Anchor scheduling on Phase 3 Training loop availability + Phase 4a RealityPrediction surface.

---

## Cross-domain dependencies

- **PREDICTOR (Phase 2)**: provides the `Predictor` struct + forward/backward + Arc<RwLock<Predictor>> consumed by TASK-HEAL-006 (EWC penalty backprop), TASK-HEAL-008 (observe SGD step), TASK-HEAL-009 (Mode A/B/C model handles), TASK-HEAL-014 (LoRA-refresh re-eval), TASK-HEAL-015 (distillation backprop).
- **TRAINING (Phase 3)**: provides the `train_from_scratch(snapshot)` entry point + per-step training certs (ΔP, ΔK, ΔΩ, ΔΞ) consumed by TASK-HEAL-008 (ASSESS step reads last 128 certs), TASK-HEAL-009 (Mode B retrain), TASK-HEAL-011 (periodic full retrain).
- **INFERENCE (Phase 4a)**: provides `RealityPrediction { outcome_set, ood_score, signal_clarity, status_change }` consumed by TASK-HEAL-007 (drift window samples) and `CalibrationRecord` consumed by TASK-HEAL-012 (calibration sliding window).
- **TCT (Phase 4b)**: provides `refit_centroids(fresh_walk_data)` consumed by TASK-HEAL-008 REGULATE step on `constellation_violation_rate` high.
- **CORPUS (Phase 0)**: provides `CF_MEJEPA_MUTATION_CORPUS` snapshot + holdout split consumed by TASK-HEAL-009 (Mode B train data + holdout eval).
- **WITNESS (shipped)**: provides `verify_chain_integrity`, `read_witness_chain`, `write_text_checked`, `witness_chain_legacy::reconcile`, `witness_chain_repair`, ed25519 segment signing consumed by TASK-HEAL-009 (ModelPromote entry), TASK-HEAL-010 (ModelRollback entry), TASK-HEAL-013 (chain replay).
- **EMBEDDINGS (shipped)**: provides `LoraTrainer::fine_tune` consumed by TASK-HEAL-014; `DistillationTrainer::distill_step` consumed by TASK-HEAL-015 — both REUSED, not reinvented.
- **STORAGE (shipped)**: provides RocksDB CF wrappers consumed by TASK-HEAL-005 (CF descriptors) and every logic task that persists.
- **HYGIENE (Phase 6)**: GC-bounds CF_MEJEPA_WEIGHT_BLOBS (active + 1 prior promotion-cycle); cold-tier eviction trigger from TASK-HEAL-009 quota check.
- **SHIFT-SUBSCRIBER (Phase 7)**: SOLE external caller of `observe()` in normal operation; owns CF_MEJEPA_SHIFT_WATERMARK; this domain owns the body of `observe()`.

---

## Coverage roll-up

Every functional REQ-HEAL-NN (20 total) and every component contract (7 in TECH-SELF-HEALING) maps to at least one task in this index. Coverage tracking is in `../_traceability.md` under `### SELF-HEALING`.

| REQ | Title | Task ID(s) |
|---|---|---|
| REQ-HEAL-01 | observe() signature + EWC-regularized SGD | TASK-HEAL-001, TASK-HEAL-006, TASK-HEAL-008 |
| REQ-HEAL-02 | EwcPlusPlus methods (update_fisher_online, detect_task_boundary, snapshot, current_lambda) | TASK-HEAL-001, TASK-HEAL-006 |
| REQ-HEAL-03 | DriftDetector + rolling 1,000-window + severity table | TASK-HEAL-002, TASK-HEAL-007 |
| REQ-HEAL-04 | AbcPromoter retrain_and_promote (3-mode + holdout gate) | TASK-HEAL-003, TASK-HEAL-009 |
| REQ-HEAL-05 | schedule_full_retrain every 10,000 obs + 0.02 floor | TASK-HEAL-011 |
| REQ-HEAL-06 | continuous_calibration_if_due sliding window τ + σ² | TASK-HEAL-012 |
| REQ-HEAL-07 | Witness chain integrity check (chain.len() % 73 + replay) | TASK-HEAL-013 |
| REQ-HEAL-08 | LoRA refresh on plasticity collapse (rank=32 epochs=10) | TASK-HEAL-014 |
| REQ-HEAL-09 | Distillation-based observe (InfoNCE + EMA τ=0.996) | TASK-HEAL-015 |
| REQ-HEAL-10 | UTML regulate-state-first cycle | TASK-HEAL-008 |
| REQ-HEAL-11 | Drift events surface (journal + blocker + status_change + recommendation) | TASK-HEAL-007, TASK-HEAL-008 |
| REQ-HEAL-12 | Promotion witness-chain entry (ModelPromote) | TASK-HEAL-009 |
| REQ-HEAL-13 | signal_clarity multiplicative gating | TASK-HEAL-008, TASK-HEAL-015 |
| REQ-HEAL-14 | HealError taxonomy (12 variants + .code() SCREAMING_SNAKE) | TASK-HEAL-004 |
| REQ-HEAL-15 | mejepa-heal-drill CLI + 5 FSV evidence files | TASK-HEAL-018, TASK-HEAL-019 |
| REQ-HEAL-16 | Real-data tests only (no mocks) | TASK-HEAL-008, TASK-HEAL-009, TASK-HEAL-018 |
| REQ-HEAL-17 | mcp__cgreality__mejepa_heal_status MCP tool | TASK-HEAL-016 |
| REQ-HEAL-18 | mcp__cgreality__mejepa_rollback_to MCP tool | TASK-HEAL-010, TASK-HEAL-017 |
| REQ-HEAL-19 | Performance budgets (observe p50<5s, A/B/C <60min) | TASK-HEAL-008, TASK-HEAL-009, TASK-HEAL-018 |
| REQ-HEAL-20 | Cross-cutting with SHIFT-SUBSCRIBER (watermark) | TASK-HEAL-008 |

| Tech component | Layer | Task ID |
|---|---|---|
| SelfHealingPipeline | foundation/logic | TASK-HEAL-004, TASK-HEAL-008, TASK-HEAL-011, TASK-HEAL-012, TASK-HEAL-013 |
| EwcPlusPlus | foundation/logic | TASK-HEAL-001, TASK-HEAL-006 |
| DriftDetector | foundation/logic | TASK-HEAL-002, TASK-HEAL-007 |
| AbcPromoter | foundation/logic | TASK-HEAL-003, TASK-HEAL-009, TASK-HEAL-010 |
| LoraRefresher | foundation/logic | TASK-HEAL-004, TASK-HEAL-014 |
| OnlineDistiller | foundation/logic | TASK-HEAL-004, TASK-HEAL-015 |
| ChainIntegrityChecker | foundation/logic | TASK-HEAL-004, TASK-HEAL-013 |
| CalibrationWriter | foundation/logic | TASK-HEAL-004, TASK-HEAL-012 |
| HealDrillCli | surface | TASK-HEAL-018, TASK-HEAL-019 |
| McpHealStatusTool | surface | TASK-HEAL-016 |
| McpRollbackTool | surface | TASK-HEAL-017 |

---

## Phase 5 DoD (per `09_END_GOAL_REVIEW_REPLACEMENT.md` §7 + REQ-HEAL-15 + REQ-HEAL-19)

- [ ] Drift detected within 1,000 observations (≥700-obs detection latency on synthetic --inject-drift hard).
- [ ] A/B/C ends with mode_winner ∈ {B, C}, never A unmodified.
- [ ] EWC++ Fisher matrix non-degenerate (full rank) after 10 task boundaries.
- [ ] Promotion latency <60min on RTX 5090.
- [ ] All 5 FSV evidence files present at /tmp/contextgraph-mejepa-self-healing-fsv/ with mode 0o600.
- [ ] Witness chain integrity check halts inference (status_change=paused) and restores from last verified checkpoint on break.
- [ ] LoRA refresh restores ΔΩ.effective_plasticity > 0.4 without mutating frozen-base embedder weights (byte-equal SHA invariant).
- [ ] Distillation EMA update form `w ← τ·w + (1−τ)·w_after_grad` verified at τ=0.5 synthetic; production τ=0.996.
- [ ] Every drift event journaled + blocker (severity ≥ Hard) + `status_change` field on every prediction during heal cycle + recommendation `remaining_risks`.

---

## Authority cross-reference

- Functional spec: `../../functional/SPEC-SELF-HEALING.md`
- Technical spec: `../../technical/TECH-SELF-HEALING.md`
- Constitution: `../../constitution.md` (`<utml_v1_invariants>`, `<failure_taxonomy>`, `<performance_budgets>`)
- Master traceability: `../_traceability.md`
- Cross-cutting consumer: `../phase4-inference/_index.md` (RealityPrediction.confidence consumer); `../phase4-tct-constellation/_index.md` (refit trigger); `../utml-learning-formula/_index.md` (per-step training cert window producer); `../phase7-shift-subscriber/` (sole observe() caller); `../phase6-hygiene/` (CF_MEJEPA_WEIGHT_BLOBS GC)
- Architectural sources: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §5; `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` Section D; `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` §9, §10; `docs/ruvectorfindings/12_EMBEDDER_INTEGRATION_INTO_MEJEPA.md` §6, §8, §11
