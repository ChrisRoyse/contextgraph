# Predict domain — all atomic tasks

> **Verification contract.** Every task below inherits from [`../../specs/FSV-PROTOCOL.md`](../../specs/FSV-PROTOCOL.md). Read it before starting. SoT readback, ≥3 boundary cases, NO mocks, NO backwards compat, fail-closed errors, Occam's-razor justification, and the §6 evidence artifacts are mandatory. **`ASSUME THE TASK IS WRONG`** — audit file paths, symbol names, and CF references against the live tree before implementing.

Phase A PREDICT-001/002 have detailed individual task files. This file covers PREDICT-003..012 and PREDICT-LABEL-001..011 compactly.

---

## TASK-PREDICT-003 — `mejepa_predict_what_if` MCP tool

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005 · **Spec:** SPEC-PREDICT REQ-PREDICT-03
- **Goal:** Add `mejepa_predict_what_if(hypothetical_changes, context)` MCP tool. Runs forward pass without applying edits to disk; returns `RealityPrediction` + `comparison_to_current` block.
- **Outputs:** `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs` extension; `crates/context-graph-mejepa/src/compiler.rs` `compile_with_hypothetical` API.
- **DoD:** call returns prediction without modifying any file; before/after disk SHAs unchanged; comparison block populated correctly.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/what_if.json`

## TASK-PREDICT-004 — `mejepa_explain_prediction` MCP tool

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005 · **Spec:** SPEC-PREDICT REQ-PREDICT-04
- **Goal:** `mejepa_explain_prediction(prediction_id, head, concern_index)` returns reasoning trace per `08 §1.4`: contributing signals (embedder + chunk + cosine), saliency map, closest exemplars, counterfactual minimum edit.
- **Outputs:** new MCP handler; `compiler::explain` helper.
- **DoD:** explanation references correct chunks + embedders; saliency sums to 1.0; counterfactual is plausible.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/explain.json`

## TASK-PREDICT-005 — Multi-head stack (8 new heads)

- **Phase:** B · **Effort:** 3d · **Blocked by:** TASK-FLYWHEEL-006 · **Spec:** TECH-PREDICT §3 + 03_ARCHITECTURE_BLUEPRINT.md §3
- **Goal:** Add 8 new heads (failure_mode, edge_case, tech_debt, perf, security, accuracy, cost, reasoning) to `MeJepaPredictor`. Each is a thin Linear projection on the shared trunk + per-head feature projection consuming DDA signals.
- **Outputs:** 8 new modules in `crates/context-graph-mejepa/src/predictor/heads/`; extension to `predictor/mod.rs::MeJepaPredictor` + `forward()`.
- **DoD:** forward pass returns all 10 head outputs (including existing panel + oracle); each head emits a structured output; all heads have FSV.
- **FSV:** per-head FSV files + aggregate `multi_head.json`

## TASK-PREDICT-006 — Rich `RealityPrediction` schema

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** Extend `RealityPrediction` per `15_PREDICTION_TAXONOMY.md §1` with all Q1-Q5 fields. Backwards-compatible (additive only).
- **Outputs:** `crates/context-graph-mejepa/src/types.rs` extension; round-trip serde test.
- **DoD:** old serialized predictions still parse; new predictions include all 10 head outputs + claim graph + reality impact.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/schema_round_trip.json`

## TASK-PREDICT-007 — Agent claim graph extractor (deterministic)

- **Phase:** B · **Effort:** 1d · **Blocked by:** none · **Spec:** SPEC-PREDICT REQ-PREDICT-17
- **Goal:** Implement `extract_claim_graph(agent_response: &str) -> AgentClaimGraph` per `TECH-PREDICT §7`. Rule-based; NO LLM. Pattern-matches "I (added|modified|removed|...) <symbol> in <file>" etc.
- **Outputs:** `crates/context-graph-mejepa/src/claim_graph.rs` (NEW).
- **DoD:** 50 sample agent responses produce expected `AgentClaim`s; unrecognized prose becomes `ClaimKind::Other`.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/claim_graph.json`

## TASK-PREDICT-008 — Reality-impact reconciler (Q5)

- **Phase:** B · **Effort:** 1d · **Blocked by:** TASK-PREDICT-007 · **Spec:** SPEC-PREDICT REQ-PREDICT-19
- **Goal:** `reconcile_reality_impact(prediction, observed_shifts, observed_test_outcomes) -> RealityImpact` per `15 §6`.
- **Outputs:** `crates/context-graph-mejepa/src/reality_impact.rs` (NEW).
- **DoD:** for a known prediction + known shifts, reconciler produces correct `prediction_correctness` classification; mismatch enumerated.
- **FSV:** `/mnt/d/contextgraph/fsv/mejepa-predict-fsv/reality_impact.json`

## TASK-PREDICT-009 — `ActiveLearningKind::AgentSurprise` consumer

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

## TASK-PREDICT-011 — Per-head graceful degradation (D9)

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** When a head fails (panic, NaN, OOM), the verdict-assembly continues with that head marked `unavailable: true`; the other heads still produce output.
- **Outputs:** `verdict_assembly.rs` extension; per-head error wrapping.
- **DoD:** force a head to panic → other heads still emit; final verdict reflects partial information; alert written.

## TASK-PREDICT-012 — Operator override prediction tool (D13)

- **Phase:** F · **Effort:** 0.5d · **Blocked by:** TASK-PREDICT-005
- **Goal:** `mejepa_operator_override_prediction(prediction_id, override_verdict, reason)` lets an operator override a specific prediction; the override propagates as a gold label to the corpus.
- **Outputs:** new MCP tool; CF_MEJEPA_OPERATOR_OVERRIDES.
- **DoD:** override → next batch sampling weight 6× on affected chunks; report shows override count.

---

## TASK-PREDICT-LABEL-001 — Failure-mode label source

- **Phase:** B · **Effort:** 0.5d · **Blocked by:** TASK-CORPUS-002 · **Spec:** SPEC-PREDICT REQ-PREDICT-LABEL-001 + 15_PREDICTION_TAXONOMY.md §3.1
- **Goal:** Deterministic rule-based extractor that parses pytest / cargo-test / jest tracebacks and classifies into `FailureModeClass`.
- **Outputs:** `crates/context-graph-mejepa-train/src/labels/failure_mode_extractor.rs` (NEW); per-language traceback parsers.
- **DoD:** 100 corpus examples produce expected FailureModeClass; unclassified → `Other(name)`.

## TASK-PREDICT-LABEL-002 — Edge-case label source

- **Phase:** B · **Effort:** 1d · **Blocked by:** TASK-PREDICT-LABEL-001
- **Goal:** Coverage delta + symbolic-execution proxy on AST + per-language pattern catalog produces `PredictedEdgeCase` labels.
- **Outputs:** `labels/edge_case_extractor.rs` (NEW); per-language pattern catalogs in `data/edge_cases/`.
- **DoD:** uncovered branch → `EdgeCaseClass::EmptyInput` etc.; classification correct on 30 fixture cases.

## TASK-PREDICT-LABEL-003 — Tech-debt label source

- **Phase:** B · **Effort:** 0.5d
- **Goal:** clippy / pylint / eslint warning-delta + radon / lizard complexity → `PredictedTechDebt` labels.
- **Outputs:** `labels/tech_debt_extractor.rs`; analyzer-output JSON parsers.
- **DoD:** before/after analyzer diff produces correct DebtClass labels.

## TASK-PREDICT-LABEL-004 — Dead-code label source

- **Phase:** B · **Effort:** 0.5d
- **Goal:** vulture / `cargo +nightly udeps` / eslint no-unused-vars + reachability graph from E_DataFlow + E_CFG → `PredictedDeadCode` labels.
- **Outputs:** `labels/dead_code_extractor.rs`.
- **DoD:** 20 fixture cases classify correctly across 5 languages.

## TASK-PREDICT-LABEL-005 — Perf label source

- **Phase:** B · **Effort:** 1d
- **Goal:** pytest-benchmark / cargo bench / `time` / valgrind massif delta extractors → `PredictedPerfRegression` labels.
- **Outputs:** `labels/perf_extractor.rs`; per-tool output parsers.
- **DoD:** synthetic 10% slowdown produces `PerfAxis::CpuMs` label with `predicted_delta_pct ≈ 10`.

## TASK-PREDICT-LABEL-006 — Security label source

- **Phase:** B · **Effort:** 0.5d
- **Goal:** bandit / cargo-audit / eslint-security / semgrep + CVE-db on dep deltas → `PredictedSecurityConcern` labels.
- **Outputs:** `labels/security_extractor.rs`.
- **DoD:** introduces hardcoded secret → `SecurityConcernClass::HardcodedSecret`.

## TASK-PREDICT-LABEL-007 — Accuracy label source

- **Phase:** B · **Effort:** 0.5d
- **Goal:** test-output metric extraction (regex on common metric names: f1, accuracy, mae, mse, etc.) → `PredictedAccuracyDegradation`.
- **Outputs:** `labels/accuracy_extractor.rs`.
- **DoD:** test outputs `f1: 0.85` → `AccuracyMetric::F1` label captured.

## TASK-PREDICT-LABEL-008 — Cost label source

- **Phase:** B · **Effort:** 0.5d
- **Goal:** CI minute counters + agent token telemetry + build-script timing + dep-count delta → `PredictedCostRegression`.
- **Outputs:** `labels/cost_extractor.rs`.
- **DoD:** CI duration before vs after produces `CostAxis::CiMinutes` label.

## TASK-PREDICT-LABEL-009 — Reasoning-class weak supervisor

- **Phase:** B · **Effort:** 0.5d
- **Goal:** Compute pairwise cosine `[E_Reasoning][E_Diff]` and `[E_Reasoning][E_Oracle]`; classify reasoning per `15 §4.10`.
- **Outputs:** `labels/reasoning_extractor.rs`.
- **DoD:** synthetic agent response that mismatches the diff → `ReasoningClass::Hallucination`.

## TASK-PREDICT-LABEL-010 — Toolchain fallback (D2)

- **Phase:** B · **Effort:** 0.5d · **Spec:** SPEC-TESTING REQ-TEST-09
- **Goal:** Each label-source extractor's per-language toolchain availability is detected at startup; missing toolchain → label fields `unavailable: true` rather than crash.
- **Outputs:** `labels/toolchain_detect.rs` (NEW); each extractor gated.
- **DoD:** `pyright` not in PATH → E_TypeGraph fields marked unavailable; daemon still runs.

## TASK-PREDICT-LABEL-011 — Cross-language label gating (D10)

- **Phase:** B · **Effort:** 0.5d · **Spec:** SPEC-SECURITY REQ-SEC-04
- **Goal:** `label_can_transfer` per `TECH-SECURITY §4`. Cross-language label transfer requires same RootCauseClass + similarity > threshold + per-language correlation.
- **Outputs:** `cross_language_label_gating.rs` (NEW).
- **DoD:** Python → Rust label with low similarity → not transferred; high similarity + same root cause → transferred at 0.5× weight.
