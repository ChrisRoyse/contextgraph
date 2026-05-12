# Predict domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

Phase A PREDICT-001/002 have detailed individual task files. This file covers PREDICT-003..012 and PREDICT-LABEL-001..011 compactly.

**Status snapshot (2026-05-12):**
- PREDICT-003/004/006/007/008/009: `complete` with Phase B FSV evidence at `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`.
- PREDICT-005: `code_complete`; all Phase B prediction surfaces are emitted and persisted, but production DDA/neural feature consumption and trained holdout gates remain.
- PREDICT-LABEL-001..009: `code_complete/partial`; unified deterministic extraction lives in `crates/context-graph-mejepa-train/src/labels.rs`. Full per-tool/per-language analyzers remain open.
- PREDICT-011 and PREDICT-LABEL-010 were re-scoped by the operator's fail-closed/no-fallback directive. Missing or failing heads/toolchains must error loudly with durable diagnostics, not degrade silently.

---

## TASK-PREDICT-003 — `mejepa_predict_what_if` MCP tool

**Status:** complete (2026-05-12). Implemented in `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs`; request shape is the live `PatchBundle` as `patch`, not `hypothetical_changes`.

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005 · **Spec:** SPEC-PREDICT REQ-PREDICT-03
- **Goal:** Add `mejepa_predict_what_if(patch, context, compare_to_prediction_id?)` MCP tool. Runs forward pass without applying edits to disk or persisting a live prediction; returns `RealityPrediction` + optional `compareToCurrent` block.
- **Outputs:** `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` extension using `MeJepaCompiler::compile`.
- **DoD:** call returns prediction without modifying any file; before/after disk SHAs unchanged; `sourceOfTruth.persistedPrediction=false`; comparison block populated when a prior prediction id is supplied.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`

## TASK-PREDICT-004 — `mejepa_explain_prediction` MCP tool

**Status:** complete (2026-05-12) for persisted-prediction explanation/readback. Full nearest-exemplar and counterfactual-minimum-edit enrichment remains future hardening once exemplar storage is production-populated.

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005 · **Spec:** SPEC-PREDICT REQ-PREDICT-04
- **Goal:** `mejepa_explain_prediction(prediction_id)` returns persisted prediction explanation data: saliency-by-chunk, top head surfaces, guard violations, claim graph, reality impact, provenance, and SoT CF/db metadata.
- **Outputs:** new MCP handler; `compiler::explain` helper.
- **DoD:** explanation references the correct persisted prediction; saliency sums to 1.0; SoT reports `CF_MEJEPA_LIVE_PREDICTIONS`.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`

## TASK-PREDICT-005 — Multi-head stack (8 new heads)

**Status:** code_complete (2026-05-12). Implemented as typed Phase B surfaces in `crates/context-graph-mejepa/src/types.rs`, `prediction_surfaces.rs`, and `predictor/heads.rs`; persisted through `RealityPrediction`. Production DDA/neural feature projections and trained holdout quality gates remain.

- **Phase:** B · **Effort:** 3d · **Blocked by:** TASK-FLYWHEEL-006 · **Spec:** TECH-PREDICT §3 + 03_ARCHITECTURE_BLUEPRINT.md §3
- **Goal:** Add 8 new heads (failure_mode, edge_case, tech_debt, perf, security, accuracy, cost, reasoning) to `MeJepaPredictor`. Each is a thin Linear projection on the shared trunk + per-head feature projection consuming DDA signals.
- **Outputs:** 8 new modules in `crates/context-graph-mejepa/src/predictor/heads/`; extension to `predictor/mod.rs::MeJepaPredictor` + `forward()`.
- **DoD:** compile path returns all Q1-Q5 structured outputs; each auxiliary surface validates; FSV proves persisted readback.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`

## TASK-PREDICT-006 — Rich `RealityPrediction` schema

**Status:** complete (2026-05-12). Implemented in `crates/context-graph-mejepa/src/types.rs` and stored/read through `RocksDbInferStore`.

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** Extend `RealityPrediction` per `15_PREDICTION_TAXONOMY.md §1` with all Q1-Q5 fields. Per the FSV protocol, schema changes fail closed; no silent backwards-compat defaulting is accepted.
- **Outputs:** `crates/context-graph-mejepa/src/types.rs` extension; round-trip serde test.
- **DoD:** new predictions include all 10 head outputs + claim graph + reality impact surface; persisted RocksDB readback matches.
- **FSV:** `/mnt/d/contextgraph/fsv/contextgraph-phase-b-fsv/phase-b-prediction-fsv.json`

## TASK-PREDICT-007 — Agent claim graph extractor (deterministic)

**Status:** complete (2026-05-12). Implemented in `crates/context-graph-mejepa/src/claim_graph.rs`.

- **Phase:** B · **Effort:** 1d · **Blocked by:** none · **Spec:** SPEC-PREDICT REQ-PREDICT-17
- **Goal:** Implement `extract_claim_graph(agent_response: &str) -> AgentClaimGraph` per `TECH-PREDICT §7`. Rule-based; NO LLM. Pattern-matches "I (added|modified|removed|...) <symbol> in <file>" etc.
- **Outputs:** `crates/context-graph-mejepa/src/claim_graph.rs`.
- **DoD:** deterministic sample responses produce expected `AgentClaim`s; unrecognized prose becomes `ClaimKind::Other`.
- **FSV:** focused unit tests plus Phase B persisted prediction FSV.

## TASK-PREDICT-008 — Reality-impact reconciler (Q5)

**Status:** complete (2026-05-12). Implemented in `crates/context-graph-mejepa/src/reality_impact.rs`.

- **Phase:** B · **Effort:** 1d · **Blocked by:** TASK-PREDICT-007 · **Spec:** SPEC-PREDICT REQ-PREDICT-19
- **Goal:** `reconcile_reality_impact(prediction, observed_shifts, observed_test_outcomes) -> RealityImpact` per `15 §6`.
- **Outputs:** `crates/context-graph-mejepa/src/reality_impact.rs`.
- **DoD:** for a known prediction + known shifts, reconciler produces correct `prediction_correctness` classification; mismatch enumerated.
- **FSV:** focused unit tests plus Phase B persisted prediction FSV.

## TASK-PREDICT-009 — `ActiveLearningKind::AgentSurprise` consumer

**Status:** complete (2026-05-11/12). `ActiveLearningKind::AgentSurprise` is persisted, prioritized, and consumed by `tick_active_learning`.

- **Phase:** B · **Effort:** 0.25d · **Blocked by:** TASK-FLYWHEEL-005
- **Goal:** Wire scheduler's tick_active_learning to drain AgentSurprise entries → call observe() with bumped sampling weight on the affected chunks.
- **Outputs:** `heal/scheduler_state.rs::tick_active_learning` extension.
- **DoD:** AgentSurprise queue entry → observe() called within next active_learning_tick; cert reflects bump.

## TASK-PREDICT-010 — Secret redaction pre-embed (D3)

- **Phase:** A · **Effort:** 0.5d · **Blocked by:** none · **Spec:** SPEC-SECURITY REQ-SEC-01
- **Goal:** Per `TECH-SECURITY §1`. Apply `redact_secrets` in chunk pipeline before embedder forwards; no secrets in any CF.
- **Outputs:** `crates/context-graph-mejepa-instruments/src/redact.rs` (NEW); pipeline integration.
- **DoD:** chunks containing AWS keys / JWT / PEM → embedder receives `[REDACTED]`; verified by storage inspection.
- **FSV:** `/mnt/d/contextgraph/fsv/security-fsv/redact.json`

## TASK-PREDICT-011 — Per-head fail-closed diagnostics (D9)

**Status:** open / re-scoped. The earlier "graceful degradation" design conflicts with the operator directive for no fallbacks or covered-up failures.

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** If a head fails (panic, NaN, OOM, invalid shape), the prediction request fails closed with a precise `MEJEPA_HEAD_*` error, durable diagnostic context, affected head id, input prediction/panel identifiers, and remediation hint. No partial verdict is emitted as if it were trustworthy.
- **Outputs:** `verdict_assembly.rs` / predictor-head error wrapping; structured error logging to the existing heal/report or audit stream.
- **DoD:** force a head to panic → no `RealityPrediction` is persisted; error identifies the exact head and failure class; source-of-truth readback proves prediction row count did not increase.

## TASK-PREDICT-012 — Operator override prediction tool (D13)

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** `mejepa_operator_override_prediction(prediction_id, override_verdict, reason)` lets an operator override a specific prediction; the override propagates as a gold label to the corpus.
- **Outputs:** new MCP tool; CF_MEJEPA_OPERATOR_OVERRIDES.
- **DoD:** override → next batch sampling weight 6× on affected chunks; report shows override count.

---

## TASK-PREDICT-LABEL-001 — Failure-mode label source

**Status:** code_complete (2026-05-12) in unified deterministic extractor `crates/context-graph-mejepa-train/src/labels.rs`; corpus-scale per-language traceback parser hardening remains.

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-CORPUS-002 · **Spec:** SPEC-PREDICT REQ-PREDICT-LABEL-001 + 15_PREDICTION_TAXONOMY.md §3.1
- **Goal:** Deterministic rule-based extractor that parses pytest / cargo-test / jest tracebacks and classifies into `FailureModeClass`.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; future split-out per-language traceback parsers if the unified module becomes too dense.
- **DoD:** 100 corpus examples produce expected FailureModeClass; unclassified → `Other(name)`.

## TASK-PREDICT-LABEL-002 — Edge-case label source

**Status:** partial/code_complete (2026-05-12). Deterministic edge-case signal extraction exists; coverage-delta and symbolic-execution proxy remain open.

- **Phase:** B · **Effort:** 1d · **Blocked by:** TASK-PREDICT-LABEL-001
- **Goal:** Coverage delta + symbolic-execution proxy on AST + per-language pattern catalog produces `PredictedEdgeCase` labels.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; future per-language pattern catalogs in `data/edge_cases/`.
- **DoD:** uncovered branch → `EdgeCaseClass::EmptyInput` etc.; classification correct on 30 fixture cases.

## TASK-PREDICT-LABEL-003 — Tech-debt label source

**Status:** partial/code_complete (2026-05-12). Deterministic text/static-output extraction exists; real clippy/pylint/eslint warning-delta parsers remain open.

- **Phase:** B · **Effort:** 0.5d
- **Goal:** clippy / pylint / eslint warning-delta + radon / lizard complexity → `PredictedTechDebt` labels.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; analyzer-output JSON parsers remain future hardening.
- **DoD:** before/after analyzer diff produces correct DebtClass labels.

## TASK-PREDICT-LABEL-004 — Dead-code label source

**Status:** partial/code_complete (2026-05-12). Schema/taxonomy path exists; dedicated reachability/analyzer deltas remain open.

- **Phase:** B · **Effort:** 0.5d
- **Goal:** vulture / `cargo +nightly udeps` / eslint no-unused-vars + reachability graph from E_DataFlow + E_CFG → `PredictedDeadCode` labels.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; reachability/analyzer integration remains future hardening.
- **DoD:** 20 fixture cases classify correctly across 5 languages.

## TASK-PREDICT-LABEL-005 — Perf label source

**Status:** partial/code_complete (2026-05-12). Deterministic perf-regression extraction exists; benchmark/massif parsers remain open.

- **Phase:** B · **Effort:** 1d
- **Goal:** pytest-benchmark / cargo bench / `time` / valgrind massif delta extractors → `PredictedPerfRegression` labels.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; per-tool output parsers remain future hardening.
- **DoD:** synthetic 10% slowdown produces `PerfAxis::CpuMs` label with `predicted_delta_pct ≈ 10`.

## TASK-PREDICT-LABEL-006 — Security label source

**Status:** partial/code_complete (2026-05-12). Hardcoded-secret/unsafe-crypto style deterministic extraction exists; SAST/CVE delta parsers remain open.

- **Phase:** B · **Effort:** 0.5d
- **Goal:** bandit / cargo-audit / eslint-security / semgrep + CVE-db on dep deltas → `PredictedSecurityConcern` labels.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; SAST/CVE delta parsers remain future hardening.
- **DoD:** introduces hardcoded secret → `SecurityConcernClass::HardcodedSecret`.

## TASK-PREDICT-LABEL-007 — Accuracy label source

**Status:** partial/code_complete (2026-05-12). Common metric extraction exists; broader metric catalog and before/after diffing remain open.

- **Phase:** B · **Effort:** 0.5d
- **Goal:** test-output metric extraction (regex on common metric names: f1, accuracy, mae, mse, etc.) → `PredictedAccuracyDegradation`.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`.
- **DoD:** test outputs `f1: 0.85` → `AccuracyMetric::F1` label captured.

## TASK-PREDICT-LABEL-008 — Cost label source

**Status:** partial/code_complete (2026-05-12). Dependency-count/patch-surface cost signal exists; CI-minute and token telemetry integration remains open.

- **Phase:** B · **Effort:** 0.5d
- **Goal:** CI minute counters + agent token telemetry + build-script timing + dep-count delta → `PredictedCostRegression`.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; CI/token telemetry readers remain future hardening.
- **DoD:** CI duration before vs after produces `CostAxis::CiMinutes` label.

## TASK-PREDICT-LABEL-009 — Reasoning-class weak supervisor

**Status:** partial/code_complete (2026-05-12). Reasoning-class schema and deterministic field extraction exist; E_Reasoning/E_Diff/E_Oracle weak-supervision logic remains open.

- **Phase:** B · **Effort:** 0.5d
- **Goal:** Compute pairwise cosine `[E_Reasoning][E_Diff]` and `[E_Reasoning][E_Oracle]`; classify reasoning per `15 §4.10`.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels.rs`; embedder-agreement supervisor remains future hardening.
- **DoD:** synthetic agent response that mismatches the diff → `ReasoningClass::Hallucination`.

## TASK-PREDICT-LABEL-010 — Toolchain availability fail-closed audit (D2)

**Status:** open / re-scoped. The earlier fallback design conflicts with the fail-closed directive.

- **Phase:** B · **Effort:** 0.5d · **Spec:** SPEC-TESTING REQ-TEST-09
- **Goal:** Each label-source extractor's per-language toolchain availability is detected at startup; missing required toolchain for an enabled extractor fails closed with `MEJEPA_LABEL_TOOLCHAIN_MISSING`, exact binary/config path, affected extractor, and remediation hint.
- **Outputs:** `labels/toolchain_detect.rs` (NEW); startup validation for every enabled extractor.
- **DoD:** `pyright` not in PATH while E_TypeGraph/Python extraction is enabled → daemon refuses to start; durable diagnostics identify the missing binary; no partial `unavailable: true` labels are persisted.

## TASK-PREDICT-LABEL-011 — Cross-language label gating (D10)

- **Phase:** B · **Effort:** 0.5d · **Spec:** SPEC-SECURITY REQ-SEC-04
- **Goal:** `label_can_transfer` per `TECH-SECURITY §4`. Cross-language label transfer requires same RootCauseClass + similarity > threshold + per-language correlation.
- **Outputs:** `cross_language_label_gating.rs` (NEW).
- **DoD:** Python → Rust label with low similarity → not transferred; high similarity + same root cause → transferred at 0.5× weight.
