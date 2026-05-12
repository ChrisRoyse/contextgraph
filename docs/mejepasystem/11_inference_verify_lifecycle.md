# 11 — Inference + verify() Lifecycle

Source files covered: `crates/context-graph-mejepa/src/{compiler,conformal,ood,calibration,verify_cli,gates,types,oracle_head}.rs`, `crates/context-graph-mejepa/src/eval/`, `crates/context-graph-mejepa-tct/src/{gtau,verdict}.rs`, `crates/context-graph-mcp/src/handlers/tools/mejepa_tools.rs`, `docs/futurebuild/08_LIVE_PREDICTION_AGENT_API.md`, `docs/futurebuild/15_PREDICTION_TAXONOMY.md`.

---

## 1. `MeJepaCompiler` API

Source: `crates/context-graph-mejepa/src/compiler.rs`.

| Method | Signature | Purpose |
|---|---|---|
| `compile` | `compile(patch, context) -> RealityPrediction` | Forward path only (no oracle binding) |
| `verify` | `verify(patch, context) -> Verdict` | Full verdict assembly |
| `observe` | `observe(panel, action, panel_next, oracle_outcome) -> ObserveOutcome` | Online SGD with EWC++ + cert chain |
| `compile_with_panel` | `compile_with_panel(panel) -> RealityPrediction` | Skip chunker; use existing panel |

---

## 2. The 5 Verdict variants

Source: `crates/context-graph-mejepa/src/types.rs`.

```rust
pub enum Verdict {
    Pass,                // confidence ≥ ship_gate_threshold AND no high-severity Q4 AND every claim reconciles
    Fail,                // ≥1 predicted_failed_test at confidence ≥ failure_threshold
    OutOfDistribution,   // ood_score ≥ ood_threshold OR conformal interval width > width_threshold
    Abstain,             // conformal interval too wide to commit
    GuardRejected,       // ≥1 embedder cosine to centroid < τ_m (TCT Gτ)
}
```

---

## 3. The unified `RealityPrediction` schema

Source: `crates/context-graph-mejepa/src/types.rs` + `docs/futurebuild/15_PREDICTION_TAXONOMY.md §1`.

```rust
pub struct RealityPrediction {
    // Identity
    pub prediction_id: PredictionId,
    pub witness_hash: WitnessHash,
    pub ts: SystemTime,
    pub session_id: SessionId,
    pub covered_chunks: Vec<ChunkId>,
    
    // Headline
    pub verdict: Verdict,
    pub confidence_interval: ConformalInterval,
    pub ood_score: f32,
    
    // Q3: WHY
    pub predicted_failure_modes: Vec<PredictedFailureMode>,
    pub predicted_failed_tests: Vec<PredictedTestOutcome>,
    pub guard_violations: Vec<GuardViolation>,
    pub closest_exemplars: Vec<ExemplarMatch>,
    
    // Q4: WHAT ELSE — always emitted (empty vec if none above threshold)
    pub predicted_edge_cases: Vec<PredictedEdgeCase>,
    pub predicted_latent_bugs: Vec<PredictedLatentBug>,
    pub predicted_tech_debt_added: Vec<PredictedTechDebt>,
    pub predicted_dead_code: Vec<PredictedDeadCode>,
    pub predicted_redundant_code: Vec<PredictedRedundancy>,
    pub predicted_perf_regressions: Vec<PredictedPerfRegression>,
    pub predicted_security_concerns: Vec<PredictedSecurityConcern>,
    pub predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    pub predicted_cost_regressions: Vec<PredictedCostRegression>,
    pub predicted_reasoning_class: ReasoningClass,
    
    // Q1: EXISTENCE
    pub claim_graph: AgentClaimGraph,
    pub claim_reconciliation: Vec<ClaimReconciliation>,
    
    // Q5: REALITY IMPACT
    pub reality_impact: Option<RealityImpact>,
    
    // Provenance
    pub provenance: PredictionProvenance,
}

pub struct PredictionProvenance {
    pub predictor_version: String,        // e.g., "v0.4.2-bf16-cuda13.2"
    pub constellation_version: String,
    pub calibration_version: String,
    pub active_pointer: String,           // weights SHA
}
```

**Every Q4 field is always populated** — empty vec when no concerns above threshold. The absence of a prediction is itself a prediction.

---

## 4. Forward path (full lifecycle)

```
verify(patch, context):
  1. Parse patch (unified diff or file_changes)
  2. For each affected file:
     a. memory::ast::chunk_with_options() → Vec<Chunk>
     b. For each chunk × routed embedders: EmbedderForward::forward()
     c. Compute DDA signals → CF_MEJEPA_DDA_SIGNALS
  3. PanelBuilder → Panel(5,120-d)
  4. predictor.forward(panel_t-1, panel_t) → HeadOutputs (10 heads)
  5. ConformalPredictor::wrap(output, calibration) → ConformalInterval
  6. ood_score = OodDetector::score(panel, predictor)
  7. mejepa_tct::gtau::gtau_check_chunks(predicted_panel, constellation) → Vec<GuardViolation>
  8. Resolve verdict:
       if guard_violations.len() > 0       → GuardRejected
       else if ood_score > ood_threshold   → OutOfDistribution
       else if conformal_width > width_t   → Abstain
       else if predicted_failed_tests has high-conf entry → Fail
       else if conformal_lower >= ship_gate_threshold AND no high-severity Q4 AND every claim reconciles
                                            → Pass
       else                                 → Fail or Abstain by edge logic
  9. Assemble RealityPrediction with all Q1-Q5 fields
  10. Persist to CF_MEJEPA_LIVE_PREDICTIONS keyed by session_id (latest + history)
  11. Bind witness_hash to chain
  12. Return Verdict (and full RealityPrediction via include_provenance=true)
```

---

## 5. Conformal predictor (SplitConformal)

Source: `crates/context-graph-mejepa/src/conformal.rs`.

```rust
pub struct ConformalInterval {
    pub lower: f32,
    pub upper: f32,
    pub method: ConformalMethod,             // SplitConformal | Mondrian | Group
    pub coverage_target: f32,                // typically 0.90
    pub empirical_coverage: f32,             // measured on calibration set
}
```

Algorithm:
1. Hold out 10% of training as calibration split.
2. Compute non-conformity score: `score(x) = max_test |predicted - actual|` per `REQ-INFER-03`.
3. Sort non-conformity scores ascending; pick quantile at `1 - α` (default α=0.10 → 90% coverage).
4. Threshold = the picked quantile.
5. At inference: `confidence_interval.lower = prediction - threshold`, `upper = prediction + threshold`.
6. Empirical coverage on next 1000 inferences must stay within `0.88 - 0.92` (per `conformal.coverage_tolerance`).

Per-language stratification: separate calibration split per language. Recalibrate when `ECE > 0.03` sustained over 10k attempts (`heal::calibration::recalibrate`).

---

## 6. OOD detector

Source: `crates/context-graph-mejepa/src/ood.rs`.

- **Score**: predictor reconstruction error normalized by σ². Higher = more OOD.
- **Soft threshold** (`ood.score_threshold = 0.5`): `Verdict::OutOfDistribution`.
- **Hard threshold** (`ood.hard_threshold = 0.85`): `MEJEPA_VERIFY_OUT_OF_DISTRIBUTION_HARD` — verdict still returned but flagged as untrustworthy.
- **Target AUC**: ≥ 0.85 between in-distribution and synthetic OOD inputs (per `REQ-INFER-08`).

---

## 7. Gτ guard (TCT)

See [`08_dda_tct_enrichment.md §6`](08_dda_tct_enrichment.md). One-line: for each chunk × active embedder, check `cos(predicted_emb_m, centroid_m) ≥ τ_m`. Violation → `GuardRejected` with `violating_embedders` + `violating_chunks`.

---

## 8. Claim graph + reconciliation (Q1)

Source: futurebuild `TASK-PREDICT-007-agent-claim-graph-extractor`. Deterministic rule-based parser over the agent's natural-language response.

```rust
pub enum ClaimKind {
    Added(SymbolRef),
    Modified(SymbolRef),
    Removed(SymbolRef),
    Renamed(SymbolRef, String),
    Tested(TestId),
    Verified(String),
    Refactored(String),
    Documented(SymbolRef),
    Other(String),
}

pub enum ReconciliationStatus {
    Confirmed,              // claim matches bytes
    Missing,                // claim says added X; X not present
    Modified,               // claim says modified X to do Y; X exists but doesn't do Y
    UnexpectedSideEffect,   // X exists but file Z was also modified — not claimed
    SuperficialMatch,       // symbol exists but signature/body diverges
    AmbiguousRef,           // claim references "the helper function" — multiple matches
    Unverifiable,           // claim too vague to bind to bytes
}
```

Extraction: regex/parser over `(added|modified|removed|renamed|tested|verified|refactored|documented) <symbol> in <file>`. **No LLM call.** Free-text claims become `ClaimKind::Other`.

---

## 9. Reality impact reconciler (Q5)

Source: futurebuild `TASK-PREDICT-008-reality-impact-reconciler`.

```rust
pub struct RealityImpact {
    pub observed_shifts: Vec<ShiftEntry>,
    pub predicted_files_changed: Vec<PathBuf>,
    pub observed_files_changed: Vec<PathBuf>,
    pub unexpected_files_changed: Vec<PathBuf>,
    pub predicted_test_outcomes: Vec<PredictedTestOutcome>,
    pub observed_test_outcomes: Vec<ObservedTestOutcome>,
    pub prediction_correctness: PredictionCorrectness,
}

pub enum PredictionCorrectness {
    Aligned, UnderPredicted, OverPredicted, DivergentInClass, Surprise,
}
```

**`Surprise` triggers a SurpriseEvent + active-learning queue entry.**

---

## 10. The MCP read-side surface

| Tool | Purpose | Status |
|---|---|---|
| `mejepa_verify` | Synchronous patch verification | shipped |
| `mejepa_predict_latest` | Latest prediction for session from `CF_MEJEPA_LIVE_PREDICTIONS` | shipped |
| `mejepa_predict_what_if` | Hypothetical without applying the edit | futurebuild Phase B |
| `mejepa_explain_prediction` | Per-prediction saliency + contributing signals + counterfactual | futurebuild Phase B |
| `mejepa_constellation_inspect` | TCT cell lookup + force refit | shipped |
| `mejepa_record_agent_feedback` | Agent records `SurpriseEvent` | futurebuild Phase A |

---

## 11. Agent-side example interaction

```
[Agent: Edit src/foo.py]
  → PreToolUse hook records pre_sha
  → Edit returns
  → PostToolUse hook records after_sha + appends shift-log JSONL

~250ms later, daemon's shift subscriber consumes the line:
  → chunks src/foo.py
  → runs neural forwards
  → computes DDA signals
  → builds Panel
  → calls verify(panel, action, context)
  → RealityPrediction lands in CF_MEJEPA_LIVE_PREDICTIONS

[Next prompt fires UserPromptSubmit hook]
  → Hook reads CF_MEJEPA_LIVE_PREDICTIONS
  → Renders markdown injection (≤ 4 KB):

  ### Reality prediction
  **Verdict**: Fail (oracle pass confidence 0.07 [0.04, 0.12])
  **Predicted failure modes**:
    1. src/foo.py:62-118 — WrongAlgorithm — predicted FAIL of test_X (0.93)
       violating embedders: E_TypeGraph, E_DataFlow
       closest exemplar: psf__requests-2317
  **Predicted edge cases not covered**:
    - EmptyInput at src/foo.py:12
  **Predicted tech debt**: 1 new TODO without owner
  **Reasoning class**: MostlyCorrect
  **To respond to wrong prediction**: call mejepa_record_agent_feedback with prediction_id=pred_xyz

[Agent reads, decides ME-JEPA is wrong about test_X — test doesn't exist]
[Agent calls mejepa_record_agent_feedback]
  → SurpriseEvent persisted to CF_MEJEPA_AGENT_FEEDBACK
  → Active-learning queue entry created (high priority)
  → Sampling weight for src/foo.py chunk_xyz bumped 1.6×

[Agent runs pytest]
  → Bash hook captures test results
  → TestOutcomeEvent persisted to CF_MEJEPA_ORACLE_VERDICTS

[Scheduler's next 250ms tick]
  → observe(panel, action, panel_after_test, oracle=Pass)
  → Training cert written; ΔP/ΔK/ΔΩ/ΔΞ logged
```

---

## 12. Error codes

Per `docs/ruvectorfindings/specs/constitution.md §failure_taxonomy`:

| Code | Trigger |
|---|---|
| `MEJEPA_VERIFY_INVALID_PATCH` | unified diff could not be parsed |
| `MEJEPA_VERIFY_REPO_UNAVAILABLE` | `repo_root` not a git repo |
| `MEJEPA_VERIFY_OUT_OF_DISTRIBUTION_HARD` | OOD ≥ hard threshold |
| `MEJEPA_VERIFY_GUARD_REJECTED_HARD` | every chunk fails Gτ |
| `MEJEPA_INFER_OOD_REFUSE` | OOD ≥ soft threshold |
| `MEJEPA_INFER_DIM_MISMATCH` | panel dim ≠ 5,120 |
| `MEJEPA_INFER_INSTRUMENT_MISSING` | required instrument unavailable |
| `MEJEPA_INFER_CONSTELLATION_VIOLATION` | Gτ rejected predicted panel |
| `MEJEPA_INFER_GTAU_STALE_CONSTELLATION` | constellation > max_age |
| `MEJEPA_CALIBRATION_INSUFFICIENT_SAMPLES` | < N samples in calibration window |

---

## 13. Performance budgets

| Metric | Budget | Status |
|---|---|---|
| Single-prediction latency p50 | < 100 ms | shipped (live measurement pending) |
| Predictor forward p50 | < 50 ms / batch=64 | shipped |
| Conformal coverage on calibration | 88–92% (stated 90% ± 2%) | shipped (live measurement pending) |
| OOD AUC | ≥ 0.85 | shipped |
| Shift subscriber p50 | < 5 s | shipped (live measurement pending) |
| Shift subscriber p99 | < 30 s | shipped |

---

## 14. FSV evidence

| FSV | Output |
|---|---|
| `verify()` end-to-end | `cargo run --example inference_fsv` → `/tmp/contextgraph-mejepa-inference-fsv/` |
| Conformal coverage | `calibration-evidence.json` |
| OOD AUC | `ood-auc-evidence.json` |

---

## 15. Cross-references

- [`07_instruments_panel.md`](07_instruments_panel.md), [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md), [`09_predictor_multi_head.md`](09_predictor_multi_head.md), [`13_data_flywheel.md`](13_data_flywheel.md), [`17_prediction_taxonomy_reference.md`](17_prediction_taxonomy_reference.md), [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md).
