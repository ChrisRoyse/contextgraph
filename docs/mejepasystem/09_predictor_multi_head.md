# 09 — Predictor + Multi-Head Stack

Source files covered: `crates/context-graph-mejepa/src/{predictor/,compiler,frozen_target,oracle_head,panel_source,vram,calibration,conformal,ood,types}.rs`, `crates/context-graph-mejepa-train/src/{loss/,aux/,trainer/,sampler/,cert/}*`, `docs/futurebuild/03_ARCHITECTURE_BLUEPRINT.md §3`, `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`, `docs/ruvectorfindings/specs/_README.md`.

> **Status**: Trunk + Panel head + Oracle head **shipped**. The 8 auxiliary heads (failure-mode, edge-case, tech-debt, perf, security, accuracy, cost, reasoning) are **futurebuild Phase B** per `TASK-PREDICT-005`.

---

## 1. Trunk architecture

Source: `crates/context-graph-mejepa/src/predictor/`.

| Property | Value |
|---|---|
| Type | Transformer encoder |
| Layers | 6 |
| Hidden dim | 1,024 |
| Attention heads | 8 |
| FFN expansion | 4× (standard) |
| Activation | GeLU |
| Residual + LayerNorm | yes |
| Input shape | concat of `(panel_t-1, panel_t)` = 10,240-d (futurebuild adds action context = 8,192-d) |
| Output shape | 5,120-d (predicted next-panel latent) |
| Precision | bf16 |
| Hardware | CUDA-only sm_120 (RTX 5090 Blackwell) |
| Framework | candle 0.9.x + cudarc |
| VRAM steady-state | ≤ 8 GB at batch=64 |
| Forward latency p50 | < 50 ms / batch (batch=64) |

**CPU construction is rejected**, not silently down-graded. Production predictor is CUDA-only.

---

## 2. The 10 prediction heads

Each head is a **thin Linear projection on top of the shared trunk output**. Adding a head costs ~Linear<5120, output_dim> parameters — essentially zero relative to the trunk's ~50M.

| # | Head | Output | Loss | Label source | Status |
|---|---|---|---|---|---|
| 1 | Panel | predicted_panel[t=2] in 5,120-d latent | MSE + cosine + VICReg | frozen-instrument target on observed panel[t=2] | shipped |
| 2 | Oracle | per-test pass/fail logits + count | BCE + count loss | Docker oracle outcome | shipped |
| 3 | Failure-mode | logits over FailureModeClass per chunk | softmax × focal | rule-based extractor over pytest/cargo-test traceback | futurebuild Phase B |
| 4 | Edge-case | logits over EdgeCaseClass per chunk × `not_covered_by_tests` | sigmoid multilabel | coverage Δ + symbolic-execution proxy on AST | futurebuild Phase B |
| 5 | Tech-debt | logits over DebtClass per chunk | sigmoid multilabel | clippy/pylint/eslint warning delta + complexity metrics | futurebuild Phase B |
| 6 | Perf | (cpu_us_delta, rss_kb_delta, alloc_count_delta) per chunk | Huber regression | pytest-benchmark / cargo bench / time / massif | futurebuild Phase B |
| 7 | Security | logits over SecurityConcernClass per chunk | sigmoid multilabel | bandit / cargo-audit / eslint-security / semgrep | futurebuild Phase B |
| 8 | Accuracy | (accuracy_metric_delta, calibration_delta) | regression | ML-test metric regex extractor | futurebuild Phase B |
| 9 | Cost | (CI_minutes_delta, token_cost_delta) | regression | CI duration + token counter + dep counter | futurebuild Phase B |
| 10 | Reasoning | logits over ReasoningClass for agent's prose | softmax | weak supervision via E_Reasoning ⟂ E_Diff / E_Oracle | futurebuild Phase B |

---

## 3. Head taxonomy details

### 3.1 Panel head (shipped)

Linear<5120, 5120> + VICReg-targeted regularization. Output is the predicted next panel in 5,120-d latent space. **Frozen target** = observed `panel[t=2]` from `CF_MEJEPA_PANELS` via `FrozenTargetAdapter`. Loss = MSE + cosine similarity + VICReg (variance + invariance + covariance per `crates/context-graph-mejepa-train/src/loss/vicreg.rs`).

### 3.2 Oracle head (shipped)

`OracleHead`: `Linear<5120, num_tests + 1>` per `crates/context-graph-mejepa/src/oracle_head.rs`. Output: per-test pass/fail logits + a `+1` count slot for total tests. Loss: BCE on logits + count regression.

### 3.3 Failure-mode head (futurebuild Phase B)

Logits over the `FailureModeClass` enum (`WrongAlgorithm`, `OffByOne`, `NullDeref`, `ResourceLeak`, `RaceCondition`, `DeadlockPotential`, `InfiniteLoop`, `StackOverflow`, `TypeError`, `SignatureMismatch`, `ImportMissing`, `SyntaxError`, `SemanticError`, `AssertionViolation`, `ContractViolation`, `ApiMisuse`, `ConfigDrift`, `EncodingError`, `DateTimeError`, `NumericalInstability`, `SerializationError`, `SchemaMigrationGap`, `DependencyVersionConflict`, `PlatformAssumption`, `CompilerVersionGap`, `PermissionsError`, `WrongTestUpdated`, `Other(String)`). Per-chunk; softmax + focal loss.

Label source: deterministic rule-based extractor over `pytest` / `cargo-test` / `jest` tracebacks (no LLM). Patch-similarity-graph retrieval for predict-before-tests-run path.

### 3.4 Edge-case head (futurebuild Phase B)

Logits over `EdgeCaseClass` (`EmptyInput`, `SingleElement`, `BoundaryValue`, `UnicodeEdge`, `WhitespaceOnly`, `LeadingTrailingWhitespace`, `DuplicateKeys`, `CircularReference`, `SelfReference`, `DeepNesting`, `LargeInput`, `UninitializedField`, `OptionalNotPresent`, `DefaultValueLeakage`, `ConcurrentAccess`, `PartialFailure`, `CrossPlatform`, `TimezoneTransition`, `Locale`, `NetworkPartition`, `DiskFull`, `PermissionDenied`, `Other(String)`) × `not_covered_by_test` boolean. Sigmoid multilabel.

Label source: coverage delta + symbolic-execution proxy on AST + per-language pattern catalog. A 9th mutation category `MutationKind::EdgeCaseExploit` (proposed) would generate inputs targeting the predicted edge.

### 3.5 Tech-debt head (futurebuild Phase B)

Logits over `TechDebtClass` (`HighCyclomaticComplexity`, `DeepNesting`, `LongFunction`, `LongParameterList`, `LongFile`, `DuplicatedCode`, `DataClumps`, `FeatureEnvy`, `ShotgunSurgery`, `GodObject`, `PrematureAbstraction`, `DeadFlagBranch`, `TodoWithoutOwner`, `CommentedOutCode`, `MagicNumber`, `StringLiteralForEnum`, `CopyOnWriteAbuse`, `SerdeUntagged`, `UnboundedRetry`, `UnboundedCache`, `Other`). Per-chunk; sigmoid multilabel.

Label source: complexity metrics (radon / lizard / clippy::cognitive_complexity) + duplicate detection (pmd / simian) + TODO regex.

### 3.6 Perf head (futurebuild Phase B)

Regression over `PerfAxis` (`CpuMs`, `WallclockMs`, `RssKb`, `HeapAllocs`, `HeapBytes`, `StackBytes`, `BinarySize`, `StartupMs`, `P50LatencyMs`, `P99LatencyMs`, `Throughput`, `NetworkBytesIn/Out`, `DiskRead/WriteBytes`, `GpuMemoryMb`, `Other`). Predicts `(axis, delta_pct, baseline_value)` per chunk. Huber loss.

Label source: pytest-benchmark / cargo bench / time / valgrind massif deltas vs KnownGood baseline.

### 3.7 Security head (futurebuild Phase B)

Logits over `SecurityConcernClass` (`SqlInjection`, `CommandInjection`, `PathTraversal`, `Xss`, `Csrf`, `Ssrf`, `XmlExternalEntity`, `Deserialization`, `InsecureRandom`, `HardcodedSecret`, `LoggingSecret`, `OverbroadException`, `InsecureCryptoAlgo`, `InsufficientCryptoKeyLength`, `MissingAuth`, `BrokenAccessControl`, `OpenRedirect`, `ClickjackingMissing`, `MissingTlsVerify`, `Other`). Per-chunk; sigmoid multilabel; optional CVSS estimate.

Label source: bandit / cargo audit + cargo deny / eslint-plugin-security / semgrep + CVE database via dep-version delta.

### 3.8 Accuracy head (futurebuild Phase B)

Regression over `AccuracyMetric` (`F1`, `Precision`, `Recall`, `Auc`, `Accuracy`, `MeanAbsoluteError`, `MeanSquaredError`, `R2`, `PerplexitySnapshot`, `CalibrationError`, `BrierScore`, `EmbeddingSimilarity`, `DownstreamTaskScore`, `Other`). For ML / data-pipeline code.

Label source: test outputs that include accuracy metrics (regex extractor on common metric names).

### 3.9 Cost head (futurebuild Phase B)

Regression over `CostAxis` (`CiMinutes`, `CiCpuSeconds`, `CiWallSeconds`, `EstimatedDollarsPerRun`, `LlmTokensPerCall`, `LlmDollarsPerCall`, `StorageBytes`, `EgressBytes`, `DependencyCount`, `BuildSeconds`, `Other`).

Label source: CI duration deltas + agent token telemetry + build-script timing + dep-graph counter.

### 3.10 Reasoning head (futurebuild Phase B)

Logits over `ReasoningClass` (`Correct`, `MostlyCorrect`, `PlausibleButWrong`, `Hallucination`, `Hedging`, `Overclaiming`, `UnderClaiming`, `Mute`). For the agent's accompanying prose. Softmax.

Label source: weak supervision via E_Reasoning agreement with E_Oracle + E_Diff. When E_Reasoning ⟂ E_Diff → `Hallucination`; when E_Reasoning describes outcome that disagrees with E_Oracle's predicted outcome → `PlausibleButWrong`.

---

## 4. Loss aggregation

Per `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md §8`:

```
L_total = α·L_panel + β·L_pairwise + γ·L_action
        + δ·L_constellation + ε·L_anti-pole + ζ·L_uncertainty + η·L_invariant
        + λ_oracle·L_oracle
        + λ_failure_mode·L_failure_mode
        + λ_edge_case·L_edge_case
        + λ_tech_debt·L_tech_debt
        + λ_perf·L_perf
        + λ_security·L_security
        + λ_accuracy·L_accuracy
        + λ_cost·L_cost
        + λ_reasoning·L_reasoning
```

**Defaults** (per §8 same doc):

| Coefficient | Default | Ramped to | Ramp trigger |
|---|---|---|---|
| α (panel) | 1.0 | fixed | n/a |
| β (pairwise) | 0.3 | fixed | n/a |
| γ (action) | 0.4 | fixed | n/a |
| δ (constellation) | 0.2 | fixed | n/a |
| ε (anti-pole VICReg) | 0.5 | fixed | n/a |
| ζ (uncertainty) | 0.1 | fixed | n/a |
| η (invariant) | 0.1 | fixed | n/a |
| λ_oracle | 0.5 | fixed once shipped | n/a |
| λ_failure_mode | 0.05 → 0.30 | F1 > 0.6 |
| λ_edge_case | 0.05 → 0.20 | uncovered-branch recall > 0.5 |
| λ_tech_debt | 0.05 → 0.10 | PR-comment alignment > 0.5 |
| λ_perf | 0.05 → 0.15 | benchmark MAE < 10% |
| λ_security | 0.05 → 0.20 | static-analyzer agreement > 0.7 |
| λ_accuracy | 0.05 → 0.10 | ML-test metric MAE < 5% |
| λ_cost | 0.02 → 0.05 | CI-time MAE < 20% |
| λ_reasoning | 0.05 → 0.10 | reasoning-class agreement > 0.6 |

**Ramp policy**: `+0.05 / week` once trigger crosses, until steady-state. The heal scheduler manages ramping automatically.

---

## 5. Per-head deltas (UTML extension)

Per `docs/futurebuild/10_LEARNING_FORMULA_MEJEPA_TRUTH.md`:

```
For each head h:
  ΔP_h(t)        = clamp_[0,1]( loss_h(predicted_h, target_h) / SNR_h(t) )
  ΔK_h(t)        = clamp_[0,1]( 0.5·cos_align_h + 0.3·(1−fisher_violation_h) + 0.2·(1−ECE_h) )
  ΔΞ_h(t)        = (1 − target_collapse_h)^0.5 · (1 − predictor_redundancy_h)^0.3 · (1 − constellation_violation_rate_h)^0.2
                   target_collapse_h ≡ 0 (frozen instruments)

Globally:
  ΔP_global(t)   = Σ_h(λ_h · ΔP_h(t)) / Σ_h(λ_h)
  ΔK_global(t)   = Σ_h(λ_h · ΔK_h(t)) / Σ_h(λ_h)
  ΔΩ(t)          = (effective_plasticity)^0.4 · (landscape_health)^0.4 · (stability_floor)^0.2
  ΔΞ_global(t)   = min_h(ΔΞ_h(t))                                   // any-head-collapse hard-gates global
  L_step(ex)     = ΔP_global · ΔK_global · ΔΩ · ΔΞ_global
```

**`ΔΞ_global = min_h(ΔΞ_h)`** — the MINIMUM, not the mean. Collapse in any single head halts global learning until the heal scheduler intervenes.

---

## 6. Per-head Fisher matrices

Source: `crates/context-graph-mejepa/src/heal/ewc.rs`. Extended for multi-head:

```rust
pub struct FisherSnapshot {
    pub trunk_fisher: Tensor,                          // shared across heads
    pub per_head_fisher: HashMap<HeadId, Tensor>,      // head-specific projections
    pub witness_hash: WitnessHash,
    pub ts: SystemTime,
    pub source_examples_window: (StepId, StepId),
}
```

When a head's LoRA refresh fires, **only its per-head Fisher** is consulted for EWC++ regularization; the trunk Fisher protects the shared body.

---

## 7. Forward path

```rust
pub fn forward(panel_t_minus_1: &Panel, panel_t: &Panel) -> Result<HeadOutputs, PredictorError> {
    // 1. Concatenate inputs
    let input = concat(panel_t_minus_1.data, panel_t.data);  // 10,240-d
    
    // 2. Project to trunk hidden
    let h = input_projection(input);  // hidden 1024
    
    // 3. Transformer trunk forward
    let h = trunk.forward(h);  // 6 layers
    
    // 4. Trunk output (shared)
    let trunk_out = trunk_output_projection(h);  // 5,120-d
    
    // 5. Per-head projections (parallel)
    HeadOutputs {
        panel: panel_head(trunk_out),
        oracle: oracle_head(trunk_out),
        failure_mode: failure_head(trunk_out),     // futurebuild
        edge_case: edge_head(trunk_out),           // futurebuild
        tech_debt: debt_head(trunk_out),           // futurebuild
        perf: perf_head(trunk_out),                // futurebuild
        security: sec_head(trunk_out),             // futurebuild
        accuracy: acc_head(trunk_out),             // futurebuild
        cost: cost_head(trunk_out),                // futurebuild
        reasoning: reason_head(trunk_out),         // futurebuild
    }
}
```

---

## 8. Per-head saliency / explain path (futurebuild)

`TASK-PREDICT-004 mejepa_explain_prediction` exposes:

- **Contributing signals**: per-embedder + per-chunk cosines, weights
- **Saliency map**: attention weights from the head's projection layer
- **Closest exemplars**: from `CF_MEJEPA_FAILURE_EXEMPLARS` keyed by (failure_mode_class, embedder_violation_signature)
- **Counterfactual minimum edit**: smallest change that would flip the concern

---

## 9. Performance budgets

Per `docs/ruvectorfindings/specs/constitution.md §performance_budgets`:

| Metric | Budget |
|---|---|
| Panel materialization p50 | < 100 ms (cached instruments) |
| Panel materialization p99 | < 500 ms cold (full AST + CFG + DataFlow + TypeGraph rebuild) |
| Predictor forward p50 | < 50 ms / batch=64 (RTX 5090, bf16) |
| Single-prediction latency p50 | < 100 ms end-to-end |
| VRAM steady-state | ≤ 29 GB (21 embedders + predictor + LoRA + activations + KV-cache) |

---

## 10. Cross-references

- [`07_instruments_panel.md`](07_instruments_panel.md) — Panel composition + frozen target
- [`08_dda_tct_enrichment.md`](08_dda_tct_enrichment.md) — DDA signals consumed by heads
- [`10_training_loop_utml.md`](10_training_loop_utml.md) — training loop + UTML formula
- [`11_inference_verify_lifecycle.md`](11_inference_verify_lifecycle.md) — inference path
- [`15_prediction_taxonomy.md`](15_prediction_taxonomy.md) → see [`17_prediction_taxonomy_reference.md`](17_prediction_taxonomy_reference.md) for the full enum reference
