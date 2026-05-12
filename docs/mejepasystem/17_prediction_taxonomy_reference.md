# 17 — Prediction Taxonomy Reference

Source files covered: `crates/context-graph-mejepa/src/types.rs`, `docs/futurebuild/15_PREDICTION_TAXONOMY.md`.

> **Catalog of every prediction ME-JEPA emits.** The system always emits every category, every time — empty vecs when nothing is above threshold. Absence is itself a prediction.

---

## 1. The unified output schema

See [`11_inference_verify_lifecycle.md §3`](11_inference_verify_lifecycle.md) for the full `RealityPrediction` struct.

## 2. The 5 verdicts

| Verdict | Trigger |
|---|---|
| `Pass` | confidence ≥ ship-gate AND no high-severity Q4 AND every claim reconciles |
| `Fail` | ≥1 predicted_failed_test at confidence ≥ failure_threshold |
| `OutOfDistribution` | ood_score ≥ ood_threshold OR conformal width > width_threshold |
| `Abstain` | conformal interval too wide |
| `GuardRejected` | ≥1 embedder cosine to centroid < τ_m (TCT) |

---

## 3. Q3 — WHY (when verdict != Pass)

### 3.1 `FailureModeClass` (27 variants + `Other`)

```rust
WrongAlgorithm, OffByOne, NullDeref, ResourceLeak, RaceCondition,
DeadlockPotential, InfiniteLoop, StackOverflow,
TypeError, SignatureMismatch, ImportMissing, SyntaxError, SemanticError,
AssertionViolation, ContractViolation, ApiMisuse, ConfigDrift,
EncodingError, DateTimeError, NumericalInstability, SerializationError,
SchemaMigrationGap, DependencyVersionConflict, PlatformAssumption,
CompilerVersionGap, PermissionsError, WrongTestUpdated,
Other(String),  // promoted to first-class variant when seen ≥10 times
```

### 3.2 `RootCauseClass` (9 variants)

```rust
LogicError, StateError, ResourceError, ConcurrencyError, InterfaceError,
EnvironmentError, ConfigurationError, TestQualityError, Other,
```

### 3.3 `PredictedFailureMode` struct

```rust
pub struct PredictedFailureMode {
    pub failure_class: FailureModeClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub severity: Severity,
    pub explanation: String,
    pub contributing_embedders: Vec<EmbedderId>,
    pub root_cause_class: RootCauseClass,
}
```

### 3.4 `PredictedTestOutcome`

```rust
pub struct PredictedTestOutcome {
    pub test_id: TestId,
    pub current_outcome: TestOutcome,
    pub predicted_outcome: TestOutcome,
    pub delta_kind: TestDeltaKind,           // PassToFail | FailToPass | NewFailure | NewPass | NowSkipped | NowFlaky
    pub confidence: f32,
    pub why: PredictedFailureMode,           // back-reference
}
```

### 3.5 `GuardViolation`

```rust
pub struct GuardViolation {
    pub embedder: EmbedderId,
    pub chunk: ChunkId,
    pub centroid_id: CentroidId,
    pub cosine: f32,
    pub threshold_tau_m: f32,
    pub deficit: f32,
}
```

### 3.6 `ExemplarMatch`

```rust
pub struct ExemplarMatch {
    pub task_id: SwebenchTaskId,
    pub mutation_kind: MutationKind,
    pub similarity_score: f32,
    pub diff_summary: String,
    pub oracle_outcome: OracleVerdict,
    pub witness_hash: WitnessHash,
}
```

Retrieved from `CF_MEJEPA_FAILURE_EXEMPLARS` keyed by `(failure_mode_class, embedder_violation_signature)`.

---

## 4. Q4 — WHAT ELSE (always emitted)

### 4.1 `EdgeCaseClass` (23 variants + `Other`)

```rust
EmptyInput, SingleElement, BoundaryValue, UnicodeEdge,
WhitespaceOnly, LeadingTrailingWhitespace,
DuplicateKeys, CircularReference, SelfReference,
DeepNesting, LargeInput, UninitializedField, OptionalNotPresent,
DefaultValueLeakage, ConcurrentAccess, PartialFailure,
CrossPlatform, TimezoneTransition, Locale,
NetworkPartition, DiskFull, PermissionDenied,
Other(String),
```

### 4.2 `LatentBugClass` (17 variants)

```rust
ShadowedVariable, ShadowedImport,
InconsistentNullCheck, InconsistentErrorHandling,
UnreachableBranch, DuplicateCondition,
SwitchFallthrough, LooseEquality, BareException, LoggerNotConfigured,
ForgottenAwait, ForgottenYield, ForgottenReturn,
ResourceNotReleasedOnError, LockHeldAcrossAwait,
NondeterministicIteration, HashableMutation,
Other(String),
```

### 4.3 `TechDebtClass` (20 variants)

```rust
HighCyclomaticComplexity, DeepNesting,
LongFunction, LongParameterList, LongFile,
DuplicatedCode, DataClumps, FeatureEnvy, ShotgunSurgery, GodObject,
PrematureAbstraction, DeadFlagBranch,
TodoWithoutOwner, CommentedOutCode,
MagicNumber, StringLiteralForEnum,
CopyOnWriteAbuse, SerdeUntagged, UnboundedRetry, UnboundedCache,
Other(String),
```

### 4.4 `DeadCodeKind` (10 variants)

```rust
UnusedFunction, UnusedClass, UnusedImport, UnusedVariable,
UnreachableAfterReturn, UnreachableInfiniteLoop,
DeadParameter, DeadField,
AlwaysFalseBranch, AlwaysTrueBranch,
Other(String),
```

### 4.5 `RedundancyKind` (6 variants)

```rust
DuplicatedFunction, DuplicatedTest, DuplicatedConstant,
OverlappingErrorMessage, ParallelImplementation, ReinventedWheel,
Other(String),
```

### 4.6 `PerfAxis` (16 variants)

```rust
CpuMs, WallclockMs, RssKb,
HeapAllocs, HeapBytes, StackBytes,
BinarySize, StartupMs,
P50LatencyMs, P99LatencyMs, Throughput,
NetworkBytesIn, NetworkBytesOut,
DiskReadBytes, DiskWriteBytes,
GpuMemoryMb,
Other(String),
```

### 4.7 `SecurityConcernClass` (19 variants)

```rust
SqlInjection, CommandInjection, PathTraversal, Xss, Csrf, Ssrf,
XmlExternalEntity, Deserialization,
InsecureRandom, HardcodedSecret, LoggingSecret, OverbroadException,
InsecureCryptoAlgo, InsufficientCryptoKeyLength,
MissingAuth, BrokenAccessControl, OpenRedirect, ClickjackingMissing,
MissingTlsVerify,
Other(String),
```

### 4.8 `AccuracyMetric` (13 variants)

```rust
F1, Precision, Recall, Auc, Accuracy,
MeanAbsoluteError, MeanSquaredError, R2,
PerplexitySnapshot, CalibrationError, BrierScore,
EmbeddingSimilarity, DownstreamTaskScore(String),
Other(String),
```

### 4.9 `CostAxis` (10 variants)

```rust
CiMinutes, CiCpuSeconds, CiWallSeconds, EstimatedDollarsPerRun,
LlmTokensPerCall, LlmDollarsPerCall,
StorageBytes, EgressBytes,
DependencyCount, BuildSeconds,
Other(String),
```

### 4.10 `ReasoningClass` (8 variants)

```rust
Correct, MostlyCorrect, PlausibleButWrong, Hallucination,
Hedging, Overclaiming, UnderClaiming, Mute,
```

---

## 5. Q1 — Existence reconciliation

### 5.1 `ClaimKind` (9 variants)

```rust
Added(SymbolRef), Modified(SymbolRef), Removed(SymbolRef),
Renamed(SymbolRef, String), Tested(TestId), Verified(String),
Refactored(String), Documented(SymbolRef),
Other(String),
```

### 5.2 `ReconciliationStatus` (7 variants)

```rust
Confirmed, Missing, Modified, UnexpectedSideEffect,
SuperficialMatch, AmbiguousRef, Unverifiable,
```

---

## 6. Q5 — Reality impact

### 6.1 `PredictionCorrectness` (5 variants)

```rust
Aligned, UnderPredicted, OverPredicted, DivergentInClass, Surprise,
```

`Surprise` triggers a `SurpriseEvent` + active-learning queue entry.

---

## 7. Severity scale

```rust
pub enum Severity {
    Info,      // FYI; default at low confidence
    Low,       // worth knowing; ignore-by-default in agent UI
    Medium,    // surface in agent's prompt
    High,      // block "I'm done" until addressed
    Critical,  // block + escalate to operator alert
}
```

---

## 8. `ConformalInterval`

```rust
pub struct ConformalInterval {
    pub lower: f32,
    pub upper: f32,
    pub method: ConformalMethod,             // SplitConformal | Mondrian | Group
    pub coverage_target: f32,                // typically 0.90
    pub empirical_coverage: f32,             // current measured on calibration set
}
```

---

## 9. Per-head label sources

| Head | Primary label source | Secondary signal |
|---|---|---|
| Panel | frozen-instrument target on observed panel[t=2] | VICReg regularization |
| Oracle | Docker test results + CI runs | E_Test agreement |
| Failure-mode | rule-based extraction from pytest/cargo-test/jest tracebacks | patch-similarity nearest exemplar |
| Edge-case | coverage diff + symbolic-execution proxy on AST | per-language pattern catalog |
| Tech-debt | clippy / pylint / eslint warning delta + radon / lizard complexity | duplicate detector |
| Perf | pytest-benchmark / cargo bench / valgrind massif deltas | per-axis regression head |
| Security | bandit / cargo-audit / eslint-security / semgrep + CVE-db on dep deltas | per-class severity floor |
| Accuracy | test-output metric regex extraction | E_Reasoning ↔ E_Oracle agreement |
| Cost | CI minute counters + agent token telemetry + build-script timing + dep counter | per-axis regression head |
| Reasoning | E_Reasoning ⟂ E_Diff agreement / E_Reasoning vs E_Oracle | weak-supervised |

---

## 10. Rendering (markdown injection)

When `RealityPrediction` is injected into UserPromptSubmit context, rendered as compact markdown (≤ 4 KB cap):

```markdown
### Reality prediction (chunks affected since last prompt: 3)

**Verdict:** Fail (oracle pass confidence 0.07 [0.04, 0.12])

**Predicted failure modes (ranked):**
1. `astropy/modeling/separable.py:62-118` — **WrongAlgorithm**
   - predicted FAIL of `test_separability_compound_models` (confidence 0.93)
   - root cause: WrongAlgorithm (left-vs-right child indexing inverted)
   - violating embedders: E_TypeGraph, E_DataFlow
   - closest exemplar: psf__requests-2317 (similar inverted-traversal class)

**Predicted edge cases not covered by tests:**
- `EmptyInput`: empty CompoundModel branch (current code path raises IndexError)
- `DeepNesting`: mutually-recursive CompoundModel (predicted hang)

**Predicted latent bugs:** none above threshold.

**Predicted tech debt added (Medium):** 1 new TODO without owner at line 78.

**Predicted dead code (Low):** function `_legacy_separable` unreachable from any test.

**Predicted perf / security / accuracy / cost regressions:** none above threshold.

**Reasoning class:** MostlyCorrect (explanation matches diff except "I removed the recursion limit" — limit at line 91 still present)

**Claim reconciliation:**
- ✅ "I added function `_walk_compound`" — confirmed at line 142
- ⚠️ "I removed the recursion limit" — Modified (limit at line 91 still present)

**Reality impact:** 1 file changed as predicted; 0 unexpected; 2 tests would flip PASS→FAIL as predicted.

**To respond to a wrong prediction:** call `mejepa_record_agent_feedback` with `prediction_id=pred_01HXYZ...`.
```

---

## 11. Operating principles

1. **Always emit. Never elide.** Empty vec = "no concerns above threshold" — that's a prediction.
2. **Confidence-calibrated, not threshold-binarized.** Every prediction has a confidence; UI may filter by severity.
3. **Citation-grounded.** Every prediction names chunk_id + line_range + contributing embedders.
4. **Falsifiable.** Every prediction testable via oracle / coverage / static-analysis / benchmark.
5. **Provenance-bound.** Every prediction binds `witness_hash`; feedback events bind back.
6. **Schema-stable.** Adding new prediction class extends an enum; old fields don't change shape.
7. **Generic across languages.** Per-language realizations are config, not schema.

---

## 12. Cross-references

[`09_predictor_multi_head.md`](09_predictor_multi_head.md), [`11_inference_verify_lifecycle.md`](11_inference_verify_lifecycle.md), [`18_mcp_tool_reference.md`](18_mcp_tool_reference.md).
