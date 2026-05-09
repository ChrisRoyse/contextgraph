# Phase 8 — EVALUATION Task Index

> **Level 4.** Atomic task specifications for the **EVALUATION domain** (Phase 8: holdout evaluation, ≥0.95 ship gate, per-language stratification, active learning, cross-task transfer, weekly report). Authority order: `../../constitution.md` (Level 1) > `../../functional/SPEC-EVALUATION.md` (Level 2) > `../../technical/TECH-EVALUATION.md` (Level 3) > this file (Level 4). Each task is one conceptual change shippable end-to-end with FSV evidence per `docs2/prdtospec.md` §4.

## Phase scope

Implements all 20 functional REQ-EVAL-NN requirements (cross-linked in `../_traceability.md` under `### EVALUATION`) and all 5 component contracts (`EvalRunner`, `EvalReport`, `ActiveLearningQueueState`, `EvalError`, `EvalConfig`/`ShipGateThresholds`) in `../../technical/TECH-EVALUATION.md`. Foundation -> Logic -> Surface ordering enforced; Iron Rule applies (no task references files created by a later task).

**Cross-cutting note:** Phase 8 is **ongoing** — there is no terminal DoD; instead a continuous ship-gate metric (per constitution `<performance_budgets>` phase8_ship_gate). Phase 8 reads from Phase 0 (CF_MEJEPA_ORACLE_VERDICTS), Phase 1 (panels for E_Diff cosines), Phase 2 (`Predictor`), Phase 3 (CF_MEJEPA_TRAIN_CERTS for ΔΩ × ΔΞ), Phase 4a (`MeJepaCompiler::compile`, `OodDetector`, `CalibrationStore`), Phase 4b (`ConstellationGuard::gtau_test`), Phase 7 (CF_MEJEPA_LIVE_PREDICTIONS for state-transfer T + Q3). When Phase 7 is not yet shipped, `state_transfer_diagnostic` and `q3_side_effect_agreement` degrade to `None` and the ship gate skips the q3 condition (per phase7_dependency in TECH-EVALUATION).

**Cross-domain handoff boundaries:**
- TCT spec hosts Gτ threshold calibration; this spec only **reads** the Gτ pass rate per language.
- SELF-HEALING (Phase 5) hosts drift detection + A/B/C retrain; this spec only **writes** `ship_gate_failures` consumed by Phase 5 read-only.
- TRAINING (Phase 3) hosts the mini-batch sampler; this spec only **writes** CF_MEJEPA_TASK_GRAPH consumed by Phase 3 read-only.
- INFERENCE (Phase 4a) hosts `compile()`/`verify()`; this spec **invokes** Gates 5-6 per holdout panel and reads ConformalSet construction.

---

## Overview

- **Total tasks:** 17
- **Foundation layer:** 3 tasks (TASK-EVAL-001..003)
- **Logic layer:** 9 tasks (TASK-EVAL-004..012)
- **Surface layer:** 5 tasks (TASK-EVAL-013..017)
- **Estimated walltime:** ~5-7 days end-to-end once Phases 0-4 (corpus + instruments + predictor + training + inference) ship; ongoing thereafter
- **Crate touched:** `crates/context-graph-mejepa/` (new `src/eval/` module sub-tree)
- **Crate touched (handler wrapper):** `crates/context-graph-mcp/`

---

## Dependency DAG

```mermaid
graph TD
    subgraph Foundation [Foundation Layer]
        T001[TASK-EVAL-001<br/>Public data models<br/>EvalReport, ShipGateStatus,<br/>PerCellAccuracy via per_cell map,<br/>ConformalHealthEntry,<br/>ActiveLearningSummary,<br/>StateTransferDiagnostic,<br/>RegressionCheck, CellDelta,<br/>OpenResearchQuestionStatus]
        T002[TASK-EVAL-002<br/>Internal types + error<br/>ActiveLearningQueueState,<br/>ActiveLearningQueueEntry,<br/>ActiveLearningLabel, LabelMethod,<br/>PatchGraph, PatchGraphEdge,<br/>EvalError + .code(),<br/>QueueError, GraphError,<br/>EvalConfig + ShipGateThresholds]
        T003[TASK-EVAL-003<br/>RocksDB CF descriptors<br/>CF_MEJEPA_EVAL_REPORTS,<br/>CF_MEJEPA_TASK_GRAPH(+_HISTORY),<br/>CF_MEJEPA_ACTIVE_LEARNING_QUEUE,<br/>CF_MEJEPA_ACTIVE_LEARNING_LABELS,<br/>CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS,<br/>CF_MEJEPA_OOD_ESCALATIONS,<br/>CF_MEJEPA_RESEARCH_QUESTIONS,<br/>CF_MEJEPA_MODEL_PROMOTIONS]
    end

    subgraph Logic [Logic Layer]
        T004[TASK-EVAL-004<br/>EvalRunner::run_holdout_eval<br/>leakage check + per-panel forward<br/>+ Pearson correlations<br/>per_category, per_language, per_cell]
        T005[TASK-EVAL-005<br/>Per-language stratified<br/>conformal coverage health<br/>±2% band check]
        T006[TASK-EVAL-006<br/>Per-language OOD AUC<br/>+ 4-week drift detection<br/>+ stratification of ConformalSet]
        T007[TASK-EVAL-007<br/>Per-language Gtau pass rate<br/>cross-cut TCT read-only]
        T008[TASK-EVAL-008<br/>check_ship_gate<br/>8-condition AND<br/>+ week-over-week regression]
        T009[TASK-EVAL-009<br/>Active learning loop<br/>enqueue_uncertain<br/>+ apply_active_learning_label<br/>+ OOD escalation routing]
        T010[TASK-EVAL-010<br/>Cross-task transfer<br/>build_patch_similarity_graph<br/>E_Diff cosines + top-k]
        T011[TASK-EVAL-011<br/>Aux-head distillation summary<br/>+ State-transfer T diagnostic<br/>Wasserstein-1 + M(t)]
        T012[TASK-EVAL-012<br/>Q1/Q2/Q3 rollup<br/>+ regression detection<br/>+ Bayesian shrinkage]
    end

    subgraph Surface [Surface Layer]
        T013[TASK-EVAL-013<br/>mcp__cgreality__mejepa_eval_run<br/>+ mejepa_eval_ship_gate_status<br/>MCP tool handlers]
        T014[TASK-EVAL-014<br/>mcp__cgreality__<br/>mejepa_active_learning_queue<br/>+ mejepa_active_learning_label]
        T015[TASK-EVAL-015<br/>Weekly report writer<br/>data/mejepa-eval/<br/>mejepa-weekly-report-YYYY-MM-DD.json<br/>+ open research questions seed]
        T016[TASK-EVAL-016<br/>CLI subcommands<br/>eval-run + eval-build-graph<br/>+ Phase 8 DoD command]
        T017[TASK-EVAL-017<br/>FSV harness<br/>16 evidence files at<br/>/tmp/contextgraph-mejepa-evaluation-fsv/]
    end

    %% Foundation -> Logic
    T001 --> T004
    T001 --> T005
    T001 --> T006
    T001 --> T007
    T001 --> T008
    T001 --> T011
    T001 --> T012
    T002 --> T004
    T002 --> T008
    T002 --> T009
    T002 --> T010
    T002 --> T011
    T003 --> T004
    T003 --> T009
    T003 --> T010
    T003 --> T011

    %% Logic chain
    T004 --> T005
    T004 --> T006
    T004 --> T007
    T005 --> T008
    T006 --> T008
    T007 --> T008
    T012 --> T008
    T010 --> T012
    T011 --> T012

    %% Logic -> Surface
    T008 --> T013
    T009 --> T014
    T008 --> T015
    T011 --> T015
    T012 --> T015
    T015 --> T016
    T010 --> T016
    T013 --> T017
    T014 --> T017
    T016 --> T017
```

---

## Execution order (sequential)

| # | Task ID | Title | Layer | Depends on | Status |
|---|---|---|---|---|---|
| 1 | TASK-EVAL-001 | Public eval data models (`EvalReport`, `ShipGateStatus`, `ConformalHealthEntry`, `ActiveLearningSummary`, `StateTransferDiagnostic`, `RegressionCheck`, `CellDelta`, `OpenResearchQuestionStatus`, `MutationCategory`, `DriftSeverity`, `ModelMode`, `SplitMode`, `ResearchQuestionId`, `MitigationState`, `AuxHeadDistillationSummary`, `ModelPromotionEntry`) | foundation | TASK-INFER-001 (`Language`, `OracleOutcome`), TASK-CORPUS-003 (`MutationCategory`) | ready |
| 2 | TASK-EVAL-002 | Internal types + error taxonomy (`ActiveLearningQueueState`, `ActiveLearningQueueEntry`, `ActiveLearningLabel`, `LabelMethod`, `PatchGraph`, `PatchGraphEdge`, `EvalError` + `.code()` + `.log_context()`, `QueueError`, `GraphError`, `EvalConfig`, `ShipGateThresholds`) | foundation | 001 | ready |
| 3 | TASK-EVAL-003 | RocksDB CF descriptors (CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_TASK_GRAPH(+_HISTORY), CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_ACTIVE_LEARNING_LABELS, CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS, CF_MEJEPA_OOD_ESCALATIONS, CF_MEJEPA_RESEARCH_QUESTIONS, CF_MEJEPA_MODEL_PROMOTIONS) | foundation | 002 | ready |
| 4 | TASK-EVAL-004 | `EvalRunner::run_holdout_eval` core (leakage check + per-panel `compile()` + Pearson correlations per_category / per_language / per_cell + `compute_pearson_correlation` + `compute_bayesian_shrinkage`) | logic | 001, 002, 003 + Phase 4a (`MeJepaCompiler::compile`) + Phase 0 (holdout split + `assert_no_test_patch_leakage`) | blocked |
| 5 | TASK-EVAL-005 | Per-language stratified conformal coverage health (`compute_conformal_coverage_per_language`; ±2 % band; `MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT` flag) | logic | 004 + Phase 4a (`CalibrationStore`) | blocked |
| 6 | TASK-EVAL-006 | Per-language OOD calibration health + 4-week drift detection + per-language stratification of OOD calibration (Python-trained → Rust evaluation surface) | logic | 004 + Phase 4a (`OodDetector`) + Phase 0 (synthetic OOD panels per language) | blocked |
| 7 | TASK-EVAL-007 | Per-language Gτ pass rate (cross-cut TCT read-only); per-language stratification of Gτ thresholds | logic | 004 + Phase 4b (`ConstellationGuard`) | blocked |
| 8 | TASK-EVAL-008 | `check_ship_gate` (8-condition AND of overall ≥ 0.95, per_category ≥ 0.95, per_language ≥ 0.95, per_cell ≥ 0.90, OOD AUC ≥ 0.85, conformal ±2 %, Q1 ≥ 0.99, Q3 ≥ 0.90 + week-over-week regression < 2 %) | logic | 005, 006, 007, 012 | blocked |
| 9 | TASK-EVAL-009 | Active learning loop (`enqueue_uncertain` + `apply_active_learning_label` + OOD escalation routing + signal_value heap + capacity eviction; `outcome_set > 3` AND `ood_score < 0.5` filter; ~1-2 % corpus per cycle target) | logic | 002, 003 + Phase 4a (`ConformalSet`, `OodDetector`) | blocked |
| 10 | TASK-EVAL-010 | Cross-task transfer (`build_patch_similarity_graph` E_Diff cosine over corpus; threshold 0.7; top-k 32; CF_MEJEPA_TASK_GRAPH persist; idempotent on corpus_sha) | logic | 002, 003 + Phase 1c (E_Diff slot) | blocked |
| 11 | TASK-EVAL-011 | Aux-head distillation summary (consumer side; reads CF_MEJEPA_TRAIN_CERTS distillation steps every 1,000 steps) + State-transfer T diagnostic (Wasserstein-1 sampled + M(t) geometric mean from CF_MEJEPA_TRAIN_CERTS + attribution string) | logic | 001, 002, 003 + Phase 3 (CF_MEJEPA_TRAIN_CERTS) + Phase 7 (CF_MEJEPA_LIVE_PREDICTIONS, optional) | blocked |
| 12 | TASK-EVAL-012 | Q1/Q2/Q3 rollup (Q1 source-SHA + chain replay; Q2 alias overall_correlation; Q3 F1 over predicted vs observed side-effect set) + week-over-week regression detection (CellDelta) + Bayesian shrinkage entry-point | logic | 001, 002, 010, 011 + Phase 4a (Gates 5/6) + Phase 7 (shift-log read, optional) | blocked |
| 13 | TASK-EVAL-013 | `mcp__cgreality__mejepa_eval_run` + `mcp__cgreality__mejepa_eval_ship_gate_status` MCP tool handlers (`additionalProperties: false`; long-running tool with `report_path: file:...`) | surface | 008 | blocked |
| 14 | TASK-EVAL-014 | `mcp__cgreality__mejepa_active_learning_queue` + `mcp__cgreality__mejepa_active_learning_label` MCP tool handlers (top-N by signal_value descending; panel_sha existence check) | surface | 009 | blocked |
| 15 | TASK-EVAL-015 | Weekly report writer (`data/mejepa-eval/mejepa-weekly-report-YYYY-MM-DD.json`; mode 0o600; open-research-questions seed; drift_severity_history; model_promotion_history; persist-fail fallback to /tmp/) | surface | 008, 011, 012 | blocked |
| 16 | TASK-EVAL-016 | CLI subcommands `eval-run` (Phase 8 DoD command; exit 0/1/2) + `eval-build-graph` | surface | 010, 015 | blocked |
| 17 | TASK-EVAL-017 | FSV harness (16 evidence files at /tmp/contextgraph-mejepa-evaluation-fsv/: holdout-eval, weekly-report, active-learning-queue, ship-gate, patch-similarity-graph, regression, ood, q1q2q3, state-transfer, aux-head-distillation, open-research-questions, leakage-check, bayesian-shrinkage, provenance, latency, mcp-roundtrip) | surface | 013, 014, 016 | blocked |

**Status legend:** `ready` (foundation; can start when Phase 4a `Language`/`OracleOutcome` types land); `blocked` (waits on a prior task or upstream Phase 0/1/2/3/4/7 deliverable). Phase 8 is fully blocked until Phase 4a (inference) ships at minimum; this index is the implementation plan once those land. Until then, foundation tasks 001-003 can be staged; logic tasks 004-012 begin once `MeJepaCompiler::compile` is callable end-to-end on at least a synthetic holdout subset.

---

## Parallel-execution batches

- **Batch F.1** (foundation, parallel-safe): TASK-EVAL-001 (data-model file, depends only on shipped Phase 4a types).
- **Batch F.2** (foundation, depends on F.1): TASK-EVAL-002 (internal types + error + queue state) — parallel-safe with TASK-EVAL-003 once 001 lands.
- **Batch F.3** (foundation, depends on F.2): TASK-EVAL-003 (RocksDB CFs).
- **Batch L.1** (logic, depends on F.3 + Phase 4a inference): TASK-EVAL-004 (run_holdout_eval core).
- **Batch L.2** (logic, depends on L.1): TASK-EVAL-005, TASK-EVAL-006, TASK-EVAL-007 — three independent per-language stratification helpers (parallel-safe).
- **Batch L.3** (logic, depends on F.3 + Phase 1c E_Diff + Phase 3 train certs + optionally Phase 7): TASK-EVAL-009, TASK-EVAL-010, TASK-EVAL-011 — active learning + graph + state-transfer (parallel-safe).
- **Batch L.4** (logic, depends on L.1 + L.3): TASK-EVAL-012 (Q1/Q2/Q3 rollup + regression + shrinkage).
- **Batch L.5** (logic, depends on L.2 + L.4): TASK-EVAL-008 (check_ship_gate — gates everything).
- **Batch S.1** (surface, depends on L.5): TASK-EVAL-013 (eval-run + ship-gate-status MCP).
- **Batch S.2** (surface, depends on L.3): TASK-EVAL-014 (active-learning MCP).
- **Batch S.3** (surface, depends on L.5 + L.4): TASK-EVAL-015 (weekly report writer).
- **Batch S.4** (surface, depends on L.3 + S.3): TASK-EVAL-016 (CLI).
- **Batch S.5** (surface, depends on S.1 + S.2 + S.4): TASK-EVAL-017 (FSV).

---

## Critical path

```
TASK-EVAL-001 -> TASK-EVAL-002 -> TASK-EVAL-003 -> TASK-EVAL-004 -> TASK-EVAL-012 -> TASK-EVAL-008 -> TASK-EVAL-015 -> TASK-EVAL-016 -> TASK-EVAL-017
```

The `EvalReport` data model (001) gates the runner (004) which gates the rollup (012) which gates the ship gate (008) which gates the weekly report writer (015) which gates the Phase 8 DoD CLI command (016) which gates FSV (017). Anchor scheduling on TASK-EVAL-004 once Phase 4a `MeJepaCompiler::compile` is callable on synthetic holdout subset.

---

## Cross-domain dependencies

- **CORPUS (Phase 0)**: provides the holdout split (`HoldoutDataset`, `assert_no_test_patch_leakage`), the calibration split (`CalibrationDataset`), `MutationCategory` enum (8 variants), `OracleOutcome`, `OracleVerdict`, `Corpus` for graph build, and synthetic OOD panels per language. Required for TASK-EVAL-001, TASK-EVAL-004, TASK-EVAL-006, TASK-EVAL-010.
- **AST-CHUNKING (Phase 1a)**: provides `ChunkId` + 11-language coverage; consumed transitively via E_Diff embeddings used in TASK-EVAL-010.
- **EMBEDDERS / INSTRUMENTS (Phase 1b/1c)**: provides `Panel`, `InstrumentSlot`, `EmbedderId`, E_Diff slot encoder, E_Oracle slot encoder. Consumed by TASK-EVAL-004 (panel materialization) and TASK-EVAL-010 (E_Diff for graph).
- **PREDICTOR (Phase 2)**: provides `Arc<Predictor>` and `Arc<FrozenTarget>` consumed by `EvalRunner::new()` (TASK-EVAL-002 reserves the field shapes).
- **TRAINING (Phase 3)**: produces per-step training certificates in `CF_MEJEPA_TRAIN_CERTS` consumed by TASK-EVAL-011 for ΔΩ × ΔΞ M(t) estimate AND aux-head distillation summary. Reads CF_MEJEPA_TASK_GRAPH (TASK-EVAL-010 output) for cross-task transfer mini-batch sampler — Phase 8 is the writer, Phase 3 is the reader; no direct call coupling.
- **INFERENCE (Phase 4a)**: provides `MeJepaCompiler::compile`, `OodDetector`, `CalibrationStore`, `ConformalSet`, Gates 5/6 logic. Consumed by TASK-EVAL-004 (per-panel forward), TASK-EVAL-005, TASK-EVAL-006, TASK-EVAL-009, TASK-EVAL-012 (Q1).
- **TCT (Phase 4b)**: provides `ConstellationGuard::gtau_test`. Consumed read-only by TASK-EVAL-007. Until shipped, returns mocked Approve + provenance warning.
- **SELF-HEALING (Phase 5)**: consumes `eval_report.ship_gate_failures` to trigger A/B/C retrain on regressed cells. No call coupling — Phase 5 reads CF_MEJEPA_EVAL_REPORTS (TASK-EVAL-015 output).
- **HYGIENE (Phase 6)**: tier-compresses old EvalReports per retention policy. No call coupling — TASK-EVAL-003 declares CFs, Phase 6 owns lifecycle.
- **SHIFT-SUBSCRIBER (Phase 7)**: writes CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_ORACLE_VERDICTS. Consumed by TASK-EVAL-011 (state-transfer T) and TASK-EVAL-012 (Q3 F1 over observed side-effect sets). When Phase 7 not shipped, both degrade to None — ship gate skips q3 condition.
- **WITNESS (shipped)**: provides `verify_chain_bytes` consumed by TASK-EVAL-012 Q1 (chain replay).

---

## Coverage roll-up

Every functional REQ-EVAL-NN (20 total) and every component contract (5 in TECH-EVALUATION) maps to at least one task in this index. Coverage tracking is in `../_traceability.md`.

| REQ | Title | Task ID(s) |
|---|---|---|
| REQ-EVAL-01 | EvalRunner impl block (run_holdout_eval, check_ship_gate, enqueue_uncertain, build_patch_similarity_graph) | TASK-EVAL-002, TASK-EVAL-004, TASK-EVAL-008, TASK-EVAL-009, TASK-EVAL-010 |
| REQ-EVAL-02 | EvalReport struct shape + invariants | TASK-EVAL-001 |
| REQ-EVAL-03 | 8-condition ship-gate AND | TASK-EVAL-008 |
| REQ-EVAL-04 | Week-over-week regression detection | TASK-EVAL-012 |
| REQ-EVAL-05 | ActiveLearningQueue + label methods | TASK-EVAL-002, TASK-EVAL-009, TASK-EVAL-014 |
| REQ-EVAL-06 | Active-learning corpus growth ≥ 1 % target | TASK-EVAL-009, TASK-EVAL-015 |
| REQ-EVAL-07 | build_patch_similarity_graph + idempotency | TASK-EVAL-010 |
| REQ-EVAL-08 | MCP tool surface (4 tools) | TASK-EVAL-013, TASK-EVAL-014 |
| REQ-EVAL-09 | State-transfer T diagnostic | TASK-EVAL-011 |
| REQ-EVAL-10 | Per-language OOD AUC + 4-week drift + conformal coverage drift | TASK-EVAL-005, TASK-EVAL-006 |
| REQ-EVAL-11 | Q1/Q2/Q3 rollup | TASK-EVAL-012 |
| REQ-EVAL-12 | Aux-head distillation summary | TASK-EVAL-011 |
| REQ-EVAL-13 | Fail-closed error taxonomy + .code() | TASK-EVAL-002 |
| REQ-EVAL-14 | Holdout split leakage check | TASK-EVAL-004 |
| REQ-EVAL-15 | Phase 8 DoD CLI command | TASK-EVAL-016 |
| REQ-EVAL-16 | Performance budgets (< 30 min p50) | TASK-EVAL-016, TASK-EVAL-017 |
| REQ-EVAL-17 | Determinism (byte-equal modulo report_date) | TASK-EVAL-001, TASK-EVAL-017 |
| REQ-EVAL-18 | Provenance binding | TASK-EVAL-001, TASK-EVAL-015 |
| REQ-EVAL-19 | Open-research-question tracking | TASK-EVAL-001, TASK-EVAL-015 |
| REQ-EVAL-20 | Bayesian shrinkage for under-sampled cells | TASK-EVAL-012 |

| Tech component | Layer | Task ID |
|---|---|---|
| EvalRunner | logic | TASK-EVAL-004, TASK-EVAL-008, TASK-EVAL-009, TASK-EVAL-010, TASK-EVAL-011, TASK-EVAL-012 |
| EvalReport (try_new + serialize_canonical + hash_for_determinism) | foundation | TASK-EVAL-001 |
| ActiveLearningQueueState | foundation | TASK-EVAL-002 |
| EvalError | foundation | TASK-EVAL-002 |
| EvalConfig + ShipGateThresholds | foundation | TASK-EVAL-002 |

---

## Authority cross-reference

- Functional spec: `../../functional/SPEC-EVALUATION.md`
- Technical spec: `../../technical/TECH-EVALUATION.md`
- Constitution: `../../constitution.md` (`<performance_budgets>` phase8_ship_gate)
- Master traceability: `../_traceability.md`
- Predecessor phases: `../phase0-corpus/`, `../phase1-instruments/`, `../phase2-predictor/`, `../phase3-training/`, `../phase4-inference/`, `../phase4-tct-constellation/`
- Cross-cutting: `../phase5-self-healing/_index.md` (consumes ship_gate_failures), `../phase7-shift-subscriber/` (provides live predictions for Q3 + state-transfer)
- Architectural sources: `docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md` §1.3 (Q1/Q2/Q3), §3.4 (holdout split), §3.7.4 (active learning), §3.7.5 (cross-task transfer), §3.7.6 (aux-head distillation), §7 Phase 8, §8 (8 open research questions); `docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md` §C (cross-language mandate); `docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md` §8 (state-transfer T)
- Memory: `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`, `memory/feedback_no_cost_caps.md`
