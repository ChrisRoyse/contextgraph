# utml-learning-formula — UTML v1.0 Task Index

> **📊 2026-05-11 STATUS — mostly shipped / 3 items remaining.** UTML 001-007 are SHIPPED + ΔΞ leak gate hardened. UTML 008 (pairwise MI audit JOB) and UTML 009 (UtmlClassifier + M(t) + StateTransfer T + continual-backprop dormant re-init) and UTML 010 (FSV envelope harness) are the only PENDING items.
>
> **Shipped (verified by `grep` on `crates/context-graph-mejepa-train/src/learning_signal.rs` 2026-05-11):**
> - `compute_delta_p(c: DeltaPComponents) -> Result<f32, UtmlError>` — line 164
> - `compute_delta_k(c: DeltaKComponents) -> Result<f32, UtmlError>` — line 171
> - `compute_delta_omega(c: DeltaOmegaComponents) -> Result<f32, UtmlError>` — line 182
> - `compute_delta_xi(c: DeltaXiComponents) -> Result<f32, UtmlError>` — line 189; rejects non-finite + non-zero `target_collapse` with `MEJEPA_INSTRUMENT_GRADIENT_LEAK` (lines 190-205); healthy formula `(1-target_collapse)^0.5 * (1-predictor_collapse)^0.3 * (1-redundancy_mi)^0.2`
> - `compute_l_step(...)` — line 232
> - `sampling_weight(l_step, operator_override, age_days, age_decay) -> f32` — line 259
> - `pairwise_mi_audit(probabilities) -> Result<f32, UtmlError>` — line 275 (the COMPUTE function; the periodic AUDIT JOB persisting `pairwise_mi_matrix.csv` is still pending — TASK-UTML-008)
> - `BatchSampler` (`sampler/mod.rs:34`) integrates `sampling_weight` (line 103) — sampler reweighting SHIPPED; cross-task transfer wiring `sampler/cross_task.rs` (102 lines) SHIPPED
> - `TrainingCertificate` + chain helpers (`cert/{mod,chain,writer,invariant_scan}.rs`) SHIPPED with hash-chained body / Merkle / SHA-256 self+parent
> - `train_edge_cases_fsv` example proves `target_collapse=0.001` → leak rejected before cert write
>
> **Pending:**
> - **TASK-UTML-008**: a SCHEDULED audit job that periodically computes the pairwise MI MATRIX over all instrument pairs and persists `/tmp/contextgraph-mejepa-train-fsv/utml-pairwise-mi-matrix.csv`. The pairwise MI compute primitive ships but no scheduling layer wraps it.
> - **TASK-UTML-009**: `UtmlClassifier` (7-state classifier of training regime), `prune_decision`, **Consolidation M(t)** (the temporal integral of L_step over training windows), **StateTransfer T detector** (Wasserstein-1-based train→deploy distribution shift), and **continual-backprop dormant-unit re-init** (intervention triggered by `ΔΩ.effective_plasticity` collapse). NONE exist in code today; this is the largest remaining UTML item.
> - **TASK-UTML-010**: 100-synthetic-example FSV envelope harness writing `utml-l-step-evidence.json` + `utml-classifier-evidence.json`. Adjacent FSV (`train_edge_cases_fsv`, `train_holdout_eval_fsv`) ships individual cases but not the full envelope sweep.
>
> Authoritative status: `../../implementation-plan.md` Phase 3+ row.

> **Level 4.** Atomic task specifications for the **UTML domain** (cross-cutting Phase 3 sub-track: four-factor learning signal `L_step = ΔP × ΔK × ΔΩ × ΔΞ`, sampling weight, training certificate, cycle classifier, consolidation integral M(t), state-transfer T). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-UTML.md` (Level 2) > `../../technical/TECH-UTML.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 12 functional `REQ-UTML-NN` requirements (cross-linked in `../_traceability.md` under `### UTML`) and all 11 component contracts in `../../technical/TECH-UTML.md`. Foundation -> Logic -> Surface ordering enforced; Iron Rule applies.

**Anchor invariants from `../../constitution.md` `<utml_v1_invariants>`:**

- Multiplicative gating (`L_step = ΔP × ΔK × ΔΩ × ΔΞ`) is non-negotiable.
- All four factors clamped to `[0, 1]`.
- `ΔΞ.target_collapse == 0.0` for ME-JEPA by construction; nonzero observation = `MEJEPA_INSTRUMENT_GRADIENT_LEAK` CRITICAL bug. **Hardened 2026-05-10:** the assertion is now in code at `learning_signal.rs:189`.
- Coaching order: regulate ΔΩ + ΔΞ FIRST, then ΔP/ΔK.
- Pruning rule: prune curriculum item only if `L_step < 0.1` for N steps AND `ΔΩ > 0.4` throughout AND `ΔΞ > 0.4` throughout.
- Per-step training certificate with hash-chain (`parent_witness_hash → self_hash`) is the physical M(t) consolidation tensor (per UTML §XII). **Cert chain SHIPPED;** the M(t) consumer that integrates over the chain is part of pending TASK-UTML-009.

---

## Codebase reality (verified 2026-05-11 by reading source)

| Question | Answer |
|---|---|
| Does `crates/context-graph-mejepa-train/` exist? | **Yes.** Public crate with: `learning_signal.rs` (388 lines), `cert/{mod,chain,writer,invariant_scan}.rs`, `sampler/{mod,cross_task}.rs`, `eval/{mod,holdout,slicing,regression,epoch_witness}.rs`, `optim/`, `loss/`, `distill/`, `aux/`, `lora_wiring.rs`, `trainer/`, `bin/`, `cli/`, `error.rs`, `config.rs`, `checkpoint.rs`. Plus 5 FSV examples. |
| Where do the `compute_delta_*` functions live? | `crates/context-graph-mejepa-train/src/learning_signal.rs`. ALL FOUR are SHIPPED + tested. The `UtmlError`/`UtmlErrorCode` enum is at the top of the same file; `UtmlErrorCode::MejepaInstrumentGradientLeak` maps to wire string `"MEJEPA_INSTRUMENT_GRADIENT_LEAK"`. |
| Where is `compute_l_step`? | Same file, line 232. Already wired into `BatchSampler` via `sampling_weight` (line 103 of `sampler/mod.rs`). |
| Is the pairwise MI audit JOB shipped? | The COMPUTE function `pairwise_mi_audit(probabilities) -> Result<f32, UtmlError>` exists at line 275 (returns a SCALAR — for one pair). The MATRIX-emitting periodic job that walks every instrument pair and writes `utml-pairwise-mi-matrix.csv` does NOT exist. TASK-UTML-008 lands the matrix iterator + scheduler + CSV writer. |
| Is the `UtmlClassifier` shipped? | **No.** No `UtmlClassifier`, no `prune_decision`, no `ConsolidationMt`, no `StateTransferT`, no continual-backprop dormant-unit-reinit anywhere in the workspace. TASK-UTML-009 is the consolidation home for all of these. |
| Is the training certificate writer shipped? | **Yes.** `cert/writer.rs::TrainCertWriter::{new, resume_from_rocksdb, emit, last_self_hash}` SHIPPED. `cert/chain.rs::{compute_self_hash, compute_parent_hash, compute_merkle_root, verify_chain, last_cert_hash}` SHIPPED. `TrainingCertificate::validate_phase3` SHIPPED. The chain replay smoke is exercised by `train_cert_chain_fsv` example. |
| Is `BatchSampler` shipped? | **Yes** (`sampler/mod.rs:34`). Constructor at line 49; calls `sampling_weight` at line 103. Cross-task graph wiring in `sampler/cross_task.rs` (102 lines) SHIPPED. |
| What FSV examples exist? | `train_cert_chain_fsv`, `train_edge_cases_fsv`, `train_full_fsv`, `train_holdout_eval_fsv`, `train_smoke_fsv` — all in `crates/context-graph-mejepa-train/examples/`. The full 100-synthetic-example UTML envelope harness (TASK-UTML-010) is not yet a separate example. |

---

## Overview

- **Total tasks:** 10
- **Foundation layer:** 2 tasks (TASK-UTML-001..002)
- **Logic layer:** 7 tasks (TASK-UTML-003..009)
- **Surface layer:** 1 task (TASK-UTML-010)
- **Crate touched:** `crates/context-graph-mejepa-train` (SHIPPED; under `src/learning_signal.rs` + `src/cert/` + `src/sampler/`)
- **Estimated walltime to complete remaining 008/009/010:** ~3-4 days (008 ~0.5 day, 009 ~2-3 days, 010 ~1 day)
- **All UTML compute functions return `Result<f32, UtmlError>`; clamped output in `[0, 1]`.**
- **TRAINING tasks depend on these (cross-phase dependency).**

---

## Dependency DAG

```mermaid
graph TD
    subgraph Foundation [Foundation Layer — SHIPPED]
        T001[TASK-UTML-001<br/>Error+Signal+Cert+Components data models<br/>SHIPPED]
        T002[TASK-UTML-002<br/>SkipReason + UtmlState + ClassifierVerdict enums<br/>partial — SkipReason in eval slot, UtmlState/ClassifierVerdict pending until TASK-UTML-009]
    end

    subgraph Logic [Logic Layer]
        T003[TASK-UTML-003<br/>compute_delta_p + per-chunk aggregator<br/>SHIPPED]
        T004[TASK-UTML-004<br/>compute_delta_k + EMA grad update<br/>SHIPPED]
        T005[TASK-UTML-005<br/>compute_delta_omega + sub-functions<br/>SHIPPED]
        T006[TASK-UTML-006<br/>compute_delta_xi target_collapse assert<br/>SHIPPED + HARDENED]
        T007[TASK-UTML-007<br/>compute_l_step + sampling_weight + drop/force gates<br/>SHIPPED]
        T008[TASK-UTML-008<br/>Periodic pairwise MI matrix audit JOB<br/>+ utml-pairwise-mi-matrix.csv writer<br/>PENDING]
        T009[TASK-UTML-009<br/>UtmlClassifier 7 states + prune_decision<br/>+ ConsolidationMt integral over cert chain<br/>+ StateTransferT Wasserstein-1<br/>+ continual-backprop dormant re-init<br/>PENDING]
    end

    subgraph Surface [Surface Layer]
        T010[TASK-UTML-010<br/>FSV: 100 synthetic L_step examples<br/>+ classifier 5 regimes<br/>+ envelope assertions<br/>PENDING]
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

| # | Task ID | Title | Layer | Depends on | Status (2026-05-11) |
|---|---------|-------|-------|------------|--------|
| 1 | TASK-UTML-001 | Data models: errors, signal, cert, ΔP/ΔK/ΔΩ/ΔΞ component structs | foundation | — | **complete** |
| 2 | TASK-UTML-002 | Enums: SkipReason + UtmlState + ClassifierVerdict + DeltaPAggregator | foundation | TASK-UTML-001 | partial — SkipReason needed by sampler is in `learning_signal.rs`; UtmlState/ClassifierVerdict block TASK-UTML-009 |
| 3 | TASK-UTML-003 | `compute_delta_p` + per-chunk aggregation | logic | TASK-UTML-001 | **complete** |
| 4 | TASK-UTML-004 | `compute_delta_k` + `update_grad_ema` | logic | TASK-UTML-001 | **complete** |
| 5 | TASK-UTML-005 | `compute_delta_omega` + sub-functions (plasticity / landscape / stability / agent_state) | logic | TASK-UTML-001 | **complete** |
| 6 | TASK-UTML-006 | `compute_delta_xi` (CRITICAL `target_collapse == 0.0` assert) | logic | TASK-UTML-001 | **complete** + hardened 2026-05-10 |
| 7 | TASK-UTML-007 | `compute_l_step` + `sampling_weight` + drop/force gates | logic | TASK-UTML-003..006 | **complete** |
| 8 | TASK-UTML-008 | EmbedderCoherence + pairwise-MI audit JOB (matrix-emitting periodic scheduler + CSV writer) | logic | TASK-UTML-007 | **ready** (compute primitive shipped; need scheduling + CSV writer + integration test) |
| 9 | TASK-UTML-009 | UtmlClassifier (7 states) + prune_decision + Consolidation M(t) integral over cert chain + StateTransfer T (Wasserstein-1) + continual-backprop dormant-unit re-init | logic | TASK-UTML-007, TASK-UTML-008, TASK-UTML-002 | **ready** (largest remaining item; consolidation home for 4 distinct primitives) |
| 10 | TASK-UTML-010 | FSV harness: 100 synthetic examples spanning corner cases; L_step envelope + target_collapse always 0 + sampling histogram | surface | TASK-UTML-007, TASK-UTML-009 | **ready** (build on existing `train_edge_cases_fsv` pattern; expand to 100-example sweep) |

---

## Status legend

- ready — Can be started immediately (all dependencies complete)
- blocked — Has unmet dependencies
- in_progress — Currently being executed
- complete — Shipped + FSV passed
- failed — Needs revision

---

## Critical path (remaining work only)

`TASK-UTML-008 → TASK-UTML-009 → TASK-UTML-010`

All upstream items (001..007) shipped. The remaining critical path is small. TASK-UTML-009 is the largest item and gates TASK-UTML-010.

---

## Parallel batches (remaining work)

- **Batch 1:** TASK-UTML-008 alone (independent of 009 — build the audit job + CSV writer; 0.5 day).
- **Batch 2 (after Batch 1):** TASK-UTML-009 alone (~2-3 days). Internally splittable into 4 independent sub-modules: `utml_classifier.rs`, `consolidation_mt.rs`, `state_transfer_t.rs`, `continual_backprop.rs` — could be 2 parallel sub-agents working on 2 modules each.
- **Batch 3 (after Batch 2):** TASK-UTML-010 (FSV — 1 day).

---

## Cross-phase consumers

`crates/context-graph-mejepa-train::trainer` (TRAINING domain) consumes every UTML compute function per step — already wired (BatchSampler integrates `sampling_weight`; per-step cert writer integrates `compute_l_step` factor decomposition). Phase 3 TRAINING tasks have explicit `<depends_on>` references back into this phase; those references are SATISFIED for 001-007.

`crates/context-graph-mejepa-shift-subscriber` (Phase 7, NEW) will consume `compute_l_step` per shift to gate `observe()` — see `phase7-shift-subscriber/TASK-SHIFT-007.md`.

`crates/context-graph-mejepa::eval` (Phase 8, NEW) will consume `pairwise_mi_audit` matrix from TASK-UTML-008 for the open-research-question Q5/Q6 entries — see `phase8-evaluation/TASK-EVAL-011.md`.

---

## FSV evidence locations

Per `../../constitution.md` `<testing_requirements>` and `<fsv_evidence_paths>`:

- `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json` — 100 synthetic examples spanning corner cases (TASK-UTML-010, pending)
- `/tmp/contextgraph-mejepa-train-fsv/utml-classifier-evidence.json` — 5 synthetic regimes (TASK-UTML-009 + 010, pending)
- `/tmp/contextgraph-mejepa-train-fsv/utml-cert-chain-evidence.json` — chain replay smoke (TASK-UTML-007 emits cert; **already exercised** by `train_cert_chain_fsv` example)
- `/tmp/contextgraph-mejepa-train-fsv/utml-pairwise-mi-matrix.csv` — pairwise MI audit (TASK-UTML-008, pending)
- `/tmp/contextgraph-mejepa-train-fsv/edge-case-evidence.json` — `target_collapse=0.001` leak proof (**already exercised** by `train_edge_cases_fsv`)

---

## Hard rules (apply to every task in this directory)

1. **No backwards compatibility.** No migration shims; no compat aliases for retired UTML primitives. The four `compute_delta_*` signatures are LOCKED at v1.0.
2. **No mock data in tests.** Use real `DeltaPComponents` / `DeltaKComponents` / `DeltaOmegaComponents` / `DeltaXiComponents` (sister TASK-UTML-001 SHIPPED). The synthetic envelope test harness in TASK-UTML-010 must use REAL component structs with carefully-chosen field values, not stubs.
3. **`target_collapse == 0.0` is non-negotiable.** Any code path that constructs a `DeltaXiComponents` with non-zero `target_collapse` MUST result in a `MEJEPA_INSTRUMENT_GRADIENT_LEAK` error. Tests verifying this are in `learning_signal.rs:368-386`.
4. **Multiplicative gating.** `L_step = ΔP × ΔK × ΔΩ × ΔΞ` — any factor at zero zeros the product. Tests must include the 4 zero-corner cases.
5. **`compute_l_step` returns `Result<f32, UtmlError>` clamped to `[0, 1]`.** NaN/Inf in any component MUST surface a structured error, never silently propagate.
6. **Per-step training cert is the M(t) physical tensor.** Consolidation M(t) (TASK-UTML-009) integrates over the chain; do NOT recompute from raw losses.
7. **Files under 500 lines.** `learning_signal.rs` is at 388 lines today; if TASK-UTML-008 + 009 push it past 500, split into `learning_signal/{mod,deltas,sampling,classifier,consolidation,state_transfer}.rs`.
8. **Full State Verification (FSV) is mandatory** — every task ships FSV evidence under `/tmp/contextgraph-mejepa-train-fsv/`; readback verifies the persisted matrix CSV / classifier verdict / cert chain bytes, not the function return value alone.
