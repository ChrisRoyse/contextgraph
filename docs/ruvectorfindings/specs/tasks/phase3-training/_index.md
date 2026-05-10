# phase3-training — Phase 3 Training Loop Task Index

> **📊 2026-05-10 STATUS — SHIPPED.** Full `Trainer` + AdamW + cosine LR + `BatchSampler` + `TrainingCertificate` hash chain + holdout eval + `LoRAWiring` + distillation in `crates/context-graph-mejepa-train/`. 5 FSV examples (`train_smoke_fsv`, `train_full_fsv`, `train_holdout_eval_fsv`, `train_cert_chain_fsv`, `train_edge_cases_fsv`). Authoritative status: ../../implementation-plan.md Phase 3.

> **Level 4.** Atomic task specifications for the **TRAINING domain** (Phase 3: AdamW + multi-objective loss + per-step training certificate + BatchSampler + holdout evaluation + counterfactual + cross-task transfer + aux distillation + per-epoch witness + checkpoint + LoRA wiring + CLI + FSV). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-TRAINING.md` (Level 2) > `../../technical/TECH-TRAINING.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 12 functional `REQ-TRAIN-NN` requirements (cross-linked in `../_traceability.md` under `### TRAINING`) and all 7 component contracts in `../../technical/TECH-TRAINING.md`. Foundation -> Logic -> Surface ordering enforced; Iron Rule applies (no task references files created by a later task). Cross-phase dependency: every TRAINING task imports types and functions from `../utml-learning-formula/`.

**Anchor invariants from `../../constitution.md` `<utml_v1_invariants>` and `<failure_taxonomy>`:**

- Every per-step training cert MUST have `delta_xi_components.target_collapse == 0.0`. Any nonzero observation = `MEJEPA_INSTRUMENT_GRADIENT_LEAK` CRITICAL.
- Coaching order: regulate ΔΩ + ΔΞ FIRST, then ΔP/ΔK.
- Pruning rule: prune curriculum item ONLY if `L_step < 0.1` for N steps AND `ΔΩ > 0.4` throughout AND `ΔΞ > 0.4` throughout.
- Per-step cert chain (`parent_witness_hash → self_hash`) is the physical M(t) consolidation tensor.
- Phase 3 DoD: holdout `predicted_oracle_pass`-vs-actual agreement ≥ 75% on final epoch; 10 epochs in &lt; 12 hours wall-clock on RTX 5090; cert chain integrity verifiable end-to-end on 100,000-step run.
- Reuse SHIPPED `crates/context-graph-embeddings/src/training/{trainer,optimizer,loss,evaluation,pipeline,distillation,lora,multitask}.rs` rather than reimplementing.

---

## Overview

- **Total tasks:** 13
- **Foundation layer:** 2 tasks (TASK-TRAIN-001..002)
- **Logic layer:** 9 tasks (TASK-TRAIN-003..011)
- **Surface layer:** 2 tasks (TASK-TRAIN-012..013)
- **Crate touched:** `crates/context-graph-mejepa-train` (NEW; shared with utml-learning-formula)
- **Estimated walltime:** ~7 days end-to-end (per master plan Phase 3 budget)
- **Cross-phase dependencies:** `../utml-learning-formula/TASK-UTML-001..009` MUST complete first
- **Reused crates:** `context-graph-embeddings` (training/*.rs), `context-graph-mincut`, `context-graph-witness`, `context-graph-mejepa-corpus`, `context-graph-mejepa-instruments`, `context-graph-mejepa` (Phase 2 predictor)

---

## Dependency DAG

```mermaid
graph TD
    subgraph CrossPhase [Cross-Phase Prerequisite]
        UTML[utml-learning-formula/<br/>TASK-UTML-001..009<br/>compute_delta_*, l_step,<br/>sampling_weight, classifier]
    end

    subgraph Foundation [Foundation Layer]
        T001[TASK-TRAIN-001<br/>Trainer + BatchSampler + TrainCertWriter +<br/>HoldoutEvaluator + EpochReport +<br/>HoldoutReport + EpochSummary data models]
        T002[TASK-TRAIN-002<br/>TrainerError + TrainerErrorCode +<br/>LossCoefficients + TrainingConfig +<br/>SkipReason extension + MutationCategory + Lang]
    end

    subgraph Logic [Logic Layer]
        T003[TASK-TRAIN-003<br/>AdamW wiring with cosine decay +<br/>linear warmup; reuse embeddings/optimizer.rs]
        T004[TASK-TRAIN-004<br/>Multi-objective L_full assembler<br/>(predict + VICReg + 8 aux + L_constellation +<br/>L_counterfactual + L_step weighting)]
        T005[TASK-TRAIN-005<br/>BatchSampler with sampling_weight gates<br/>+ deterministic seeded RNG]
        T006[TASK-TRAIN-006<br/>Counterfactual training (K=4 perturbations,<br/>ε ∈ {0.01, 0.05, 0.1, 0.2})]
        T007[TASK-TRAIN-007<br/>Cross-task transfer (p=0.3 within<br/>cosine&gt;0.7 patch-similarity neighborhood)]
        T008[TASK-TRAIN-008<br/>Aux-head distillation cycle every 1000 steps]
        T009[TASK-TRAIN-009<br/>Per-step training cert emission +<br/>parent_hash → self_hash chain to<br/>CF_MEJEPA_TRAIN_CERTS]
        T010[TASK-TRAIN-010<br/>Per-epoch SHAKE-256-73 witness chain entry<br/>+ ed25519 signing]
        T011[TASK-TRAIN-011<br/>HoldoutEvaluator (every 1000 steps) +<br/>per-category + per-language slicing +<br/>conformal coverage + gtau_pass_rate]
        T012[TASK-TRAIN-012<br/>Trainer::train_one_epoch +<br/>Trainer::evaluate_holdout +<br/>run_full_training orchestration]
    end

    subgraph Surface [Surface Layer]
        T013[TASK-TRAIN-013<br/>CLI binary mejepa-train + verify-chain +<br/>eval-holdout subcommands]
        T014[TASK-TRAIN-014<br/>FSV: training-cert-chain.json +<br/>utml-l-step-evidence.json +<br/>holdout-eval-evidence.json]
    end

    UTML --> T001
    UTML --> T002
    T001 --> T003
    T002 --> T003
    T001 --> T004
    T002 --> T004
    T001 --> T005
    T002 --> T005
    T004 --> T006
    T001 --> T007
    T004 --> T008
    T001 --> T009
    T002 --> T009
    T009 --> T010
    T001 --> T011
    T002 --> T011
    T003 --> T012
    T004 --> T012
    T005 --> T012
    T006 --> T012
    T007 --> T012
    T008 --> T012
    T009 --> T012
    T010 --> T012
    T011 --> T012
    T012 --> T013
    T012 --> T014
    T013 --> T014
```

---

## Execution order

| # | Task ID | Title | Layer | Depends on | Status |
|---|---------|-------|-------|------------|--------|
| 1 | TASK-TRAIN-001 | Foundation data models: Trainer, BatchSampler, TrainCertWriter, HoldoutEvaluator, EpochReport, HoldoutReport, EpochSummary, EpochWitnessEntry | foundation | UTML-001..009 | blocked |
| 2 | TASK-TRAIN-002 | Foundation enums + config: TrainerError + TrainerErrorCode (10 variants) + LossCoefficients + TrainingConfig + MutationCategory (8) + Lang (11) + extended SkipReason (8 variants) | foundation | UTML-001, UTML-002 | blocked |
| 3 | TASK-TRAIN-003 | AdamW wiring (lr=5e-4, wd=0.04, batch=64, warmup=500, cosine decay, max_grad_norm=1.0, bf16/fp32 mixed precision) | logic | TASK-TRAIN-001, 002 | blocked |
| 4 | TASK-TRAIN-004 | Multi-objective L_full assembler (predict + VICReg + 8 aux heads + L_constellation_centroid + L_counterfactual + α_overrides·L_operator_match + L_step weighting) | logic | TASK-TRAIN-001, 002 | blocked |
| 5 | TASK-TRAIN-005 | BatchSampler with weight = base · L_step · (1+5·override) · 0.995^age; drop&lt;0.05; force&gt;1.0; deterministic seeded RNG | logic | TASK-TRAIN-001, 002, UTML-007 | blocked |
| 6 | TASK-TRAIN-006 | Counterfactual training (K=4, ε∈{0.01,0.05,0.1,0.2}; warmup 1000; α_counterfactual=0.2) + smoothness anomaly detector | logic | TASK-TRAIN-004 | blocked |
| 7 | TASK-TRAIN-007 | Cross-task transfer (p=0.3 sample from related_tasks(T) where cosine(E_Diff)&gt;0.7); fallback counter | logic | TASK-TRAIN-001 | blocked |
| 8 | TASK-TRAIN-008 | Aux-head distillation back into trunk every 1000 steps via shipped distillation.rs (quality_gate=true) | logic | TASK-TRAIN-004 | blocked |
| 9 | TASK-TRAIN-009 | Per-step TrainingCertificate emission + parent_witness_hash → self_hash chain + Merkle root + RocksDB CF_MEJEPA_TRAIN_CERTS write + fsync every 100 + replay verifier | logic | TASK-TRAIN-001, 002, UTML-007 | blocked |
| 10 | TASK-TRAIN-010 | Per-epoch SHAKE-256-73 witness chain entry + ed25519 signing + EpochWitnessChain | logic | TASK-TRAIN-009 | blocked |
| 11 | TASK-TRAIN-011 | HoldoutEvaluator (240-example split per Stoer-Wagner mincut; every 1000 steps; tracks agreement + conformal_coverage + per_category + per_language + predictor_redundancy_pairwise_mi + gtau_pass_rate) + promotion + regression detector | logic | TASK-TRAIN-001, 002, UTML-008 | blocked |
| 12 | TASK-TRAIN-012 | Trainer::train_one_epoch + Trainer::evaluate_holdout + Trainer::run_full_training orchestration; LoRA wiring | logic | TASK-TRAIN-003..011 | blocked |
| 13 | TASK-TRAIN-013 | CLI binary `mejepa-train` + `verify-chain` + `eval-holdout` subcommands | surface | TASK-TRAIN-012 | blocked |
| 14 | TASK-TRAIN-014 | FSV: training-cert-chain.json + utml-l-step-evidence.json + holdout-eval-evidence.json + cert-chain replay verification | surface | TASK-TRAIN-012, 013 | blocked |

> Note: Task numbering goes 001–014 (14 tasks total in this phase) — the original target of "13 tasks" expands to 14 once the FSV harness is split out as its own surface task per `<fsv_protocol>`. The plan's "13" referred to in the source brief includes 12 logic+foundation tasks plus the CLI; FSV is here split as 14 because the constitution requires per-shipped-primitive FSV.

---

## Status legend

- ready — Can be started immediately
- blocked — Has unmet dependencies
- in_progress — Currently being executed
- complete — Shipped + FSV passed
- failed — Needs revision

---

## Critical path

`UTML-001 → UTML-006 → UTML-007 → TASK-TRAIN-001 → TASK-TRAIN-009 → TASK-TRAIN-012 → TASK-TRAIN-013 → TASK-TRAIN-014`

Cert emission (TASK-TRAIN-009) is the most load-bearing primitive: every gradient update must produce an auditable cert with `target_collapse == 0.0` and a verifiable hash chain. The orchestrator (TASK-TRAIN-012) cannot ship until both cert emission AND every contributing UTML factor are operational.

---

## Parallel batches

- **Batch 1 (after UTML-009 ships):** TASK-TRAIN-001, TASK-TRAIN-002 (foundation only).
- **Batch 2 (after Batch 1 ships):** TASK-TRAIN-003, TASK-TRAIN-004, TASK-TRAIN-005, TASK-TRAIN-007, TASK-TRAIN-009, TASK-TRAIN-011 (independent logic units).
- **Batch 3 (after Batch 2 ships):** TASK-TRAIN-006 (depends on 004), TASK-TRAIN-008 (depends on 004), TASK-TRAIN-010 (depends on 009).
- **Batch 4 (after Batch 3 ships):** TASK-TRAIN-012 (orchestrator).
- **Batch 5 (after TASK-TRAIN-012 ships):** TASK-TRAIN-013, TASK-TRAIN-014 (CLI + FSV).

---

## Reused shipped infrastructure

Per `../../technical/TECH-TRAINING.md` `<crate_layout>` reuse list:

- `crates/context-graph-embeddings/src/training/optimizer.rs::AdamW` — REUSED in TASK-TRAIN-003
- `crates/context-graph-embeddings/src/training/loss.rs` (InfoNCE + directional + soft-label) — REUSED in TASK-TRAIN-004
- `crates/context-graph-embeddings/src/training/evaluation.rs` (MRR, AUC, F1) — REUSED in TASK-TRAIN-011
- `crates/context-graph-embeddings/src/training/distillation.rs` (quality_gate path) — REUSED in TASK-TRAIN-008
- `crates/context-graph-embeddings/src/training/lora.rs::LoRAAdapter` — REUSED in TASK-TRAIN-012
- `crates/context-graph-embeddings/src/training/multitask.rs` — REUSED in aux head infra
- `crates/context-graph-mincut` — REUSED in TASK-TRAIN-011 holdout split
- `crates/context-graph-witness` (SHAKE-256-73 + ed25519 + Merkle) — REUSED in TASK-TRAIN-009, 010

---

## FSV evidence locations

Per `../../constitution.md` `<fsv_evidence_paths>`:

- `/tmp/contextgraph-mejepa-train-fsv/training-cert-chain.json` — 100,000-step chain replay smoke (TASK-TRAIN-014)
- `/tmp/contextgraph-mejepa-train-fsv/utml-l-step-evidence.json` — produced by `utml-learning-formula/TASK-UTML-010` (cross-phase)
- `/tmp/contextgraph-mejepa-train-fsv/holdout-eval-evidence.json` — synthetic 240-example holdout (TASK-TRAIN-014)
- `/tmp/contextgraph-mejepa-train-fsv/training-smoke-evidence.json` — 100-step dry-train smoke (TASK-TRAIN-014)

All files mode 0o600 per SEC-MEJEPA-07.
