# 15 — Prediction Taxonomy: Everything ME-JEPA Predicts About Code

> **Purpose.** Catalog every prediction ME-JEPA emits for any code change, with the data label source, the head that owns it, the embedders that contribute signal, the failure modes within each category, and the inference-time output schema. **The system makes every prediction it can, every time it predicts at all** — even on `Verdict::Pass` it returns the full set of concerns. This document is the complete enumeration.
>
> Owned by [`SPEC-PREDICT.md`](specs/functional/SPEC-PREDICT.md) (functional) and [`TECH-PREDICT.md`](specs/technical/TECH-PREDICT.md) (technical).

---

## 1. The unified output schema

Every call to `mejepa_verify(patch, context)` or `mejepa_predict_latest(session_id)` returns:

```rust
pub struct RealityPrediction {
    /// stable id; bind feedback events to this
    pub prediction_id: PredictionId,
    /// the parent witness hash for cryptographic provenance
    pub witness_hash: WitnessHash,
    /// monotonic time the prediction was produced
    pub ts: SystemTime,
    /// the session this prediction belongs to (for hook-injection)
    pub session_id: SessionId,
    /// the chunk_ids covered by this prediction (per the AST chunker)
    pub covered_chunks: Vec<ChunkId>,

    /// the headline verdict
    pub verdict: Verdict,
    /// calibrated confidence interval per the conformal predictor
    pub confidence_interval: ConformalInterval,
    /// out-of-distribution score (higher = more OOD)
    pub ood_score: f32,

    /// =========== Q3: WHY (when verdict != Pass) ===========
    /// ranked predicted failure modes with chunk citations
    pub predicted_failure_modes: Vec<PredictedFailureMode>,
    /// per-test predicted flip from PASS→FAIL (or new failure)
    pub predicted_failed_tests: Vec<PredictedTestOutcome>,
    /// TCT Gτ guard violations: which embedders + chunks fell below threshold
    pub guard_violations: Vec<GuardViolation>,
    /// nearest-exemplar retrieval from corpus via patch-similarity graph
    pub closest_exemplars: Vec<ExemplarMatch>,

    /// =========== Q4: WHAT ELSE (always emitted) ===========
    /// edge cases the change exposes that are not covered by tests
    pub predicted_edge_cases: Vec<PredictedEdgeCase>,
    /// latent bugs the change introduces (subtle, may not break tests)
    pub predicted_latent_bugs: Vec<PredictedLatentBug>,
    /// tech debt added (smells, complexity, dead code, redundancy)
    pub predicted_tech_debt_added: Vec<PredictedTechDebt>,
    /// dead code that this change creates or amplifies
    pub predicted_dead_code: Vec<PredictedDeadCode>,
    /// redundancy with existing code
    pub predicted_redundant_code: Vec<PredictedRedundancy>,
    /// performance regressions (cpu, memory, allocations)
    pub predicted_perf_regressions: Vec<PredictedPerfRegression>,
    /// security concerns
    pub predicted_security_concerns: Vec<PredictedSecurityConcern>,
    /// accuracy degradations for ML/data-pipeline code
    pub predicted_accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    /// cost regressions (CI minutes, $/run, token impact downstream)
    pub predicted_cost_regressions: Vec<PredictedCostRegression>,
    /// classification of the agent's accompanying explanation
    pub predicted_reasoning_class: ReasoningClass,

    /// =========== Q1: EXISTENCE ===========
    /// claim graph extracted from agent's response
    pub claim_graph: AgentClaimGraph,
    /// per-claim reconciliation against bytes-on-disk
    pub claim_reconciliation: Vec<ClaimReconciliation>,

    /// =========== Q5: REALITY IMPACT ===========
    /// the predicted-vs-observed delta if shifts have already been applied
    pub reality_impact: Option<RealityImpact>,
}
```

Every field is **always populated**. When a category has no predictions, the vec is empty (not `Option::None`). The agent reads the rendered markdown injection and sees "predicted security concerns: none" rather than the field being elided — this is intentional, so the absence of a prediction is itself a prediction.

---

## 2. Verdict enum (the headline)

```rust
pub enum Verdict {
    /// confidence_interval lower bound ≥ ship_gate_threshold
    /// AND no high-severity Q4 concerns
    /// AND every claim reconciles
    Pass,
    /// at least one predicted_failed_test with confidence ≥ failure_threshold
    Fail,
    /// ood_score ≥ ood_threshold OR conformal interval width > width_threshold
    OutOfDistribution,
    /// confidence_interval too wide to commit; conformal coverage degraded
    Abstain,
    /// TCT Gτ guard rejected: at least one embedder cosine to centroid < τ_m
    GuardRejected,
}
```

`Pass` is the strict case. `Fail / OOD / Abstain / GuardRejected` are all "not Pass" with different remediation paths; the agent should treat them as distinct because the underlying issue is different.

---

## 3. The Q3 surface — WHY it doesn't work

### 3.1 PredictedFailureMode

```rust
pub struct PredictedFailureMode {
    pub failure_class: FailureModeClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub severity: Severity,
    pub explanation: String,                 // human-readable summary
    pub contributing_embedders: Vec<EmbedderId>,
    pub root_cause_class: RootCauseClass,
}

pub enum FailureModeClass {
    WrongAlgorithm,             // wrong logic in branch / wrong condition / inverted operator
    OffByOne,                   // boundary error
    NullDeref,                  // None / null / nullptr access not guarded
    ResourceLeak,               // file / socket / handle not closed
    RaceCondition,              // shared mutable state without synchronization
    DeadlockPotential,          // lock ordering / re-entry violation
    InfiniteLoop,               // termination not guaranteed
    StackOverflow,              // unbounded recursion
    TypeError,                  // type mismatch the static checker would flag
    SignatureMismatch,          // caller / callee signature disagreement
    ImportMissing,              // dependency not declared
    SyntaxError,                // doesn't parse
    SemanticError,              // parses but semantically invalid (e.g., dangling reference)
    AssertionViolation,         // an explicit assert would fail
    ContractViolation,          // pre-/post-condition violation
    ApiMisuse,                  // library called against documented contract
    ConfigDrift,                // env / config schema mismatch
    EncodingError,              // unicode / byte handling wrong
    DateTimeError,              // timezone / DST / leap-second handling wrong
    NumericalInstability,       // overflow / underflow / NaN / division by zero
    SerializationError,         // serde format mismatch
    SchemaMigrationGap,         // database schema not reconciled
    DependencyVersionConflict,  // version mismatch between deps
    PlatformAssumption,         // OS/arch/byte-order assumption violated
    CompilerVersionGap,         // language-feature requires newer toolchain
    PermissionsError,           // file mode / capability / IAM
    WrongTestUpdated,           // test was modified to fit broken code instead of code fixed
    Other(String),              // free text — promoted to a class when seen ≥10 times
}

pub enum RootCauseClass {
    LogicError,
    StateError,
    ResourceError,
    ConcurrencyError,
    InterfaceError,
    EnvironmentError,
    ConfigurationError,
    TestQualityError,
    Other,
}
```

**Label source for the failure-mode head:**
- Per-test failure log from the corpus: when a mutation produces a test failure, the failure trace is parsed (pytest traceback / cargo-test output / jest stack) and classified into `FailureModeClass` by a deterministic rule-based extractor (no LLM). 
- For unclassified traces, the text is assigned `Other("...")`; the operator-side weekly review promotes any `Other` seen ≥ 10 times into a first-class enum variant.
- Patch-similarity graph: when the predictor sees a new patch resembling a known-fail exemplar, it can predict the same failure class even before tests run.

### 3.2 PredictedTestOutcome

```rust
pub struct PredictedTestOutcome {
    pub test_id: TestId,                     // (file, name) or pytest nodeid
    pub current_outcome: TestOutcome,        // last known
    pub predicted_outcome: TestOutcome,
    pub delta_kind: TestDeltaKind,           // PassToFail | FailToPass | NewFailure | NewPass | NowSkipped | NowFlaky
    pub confidence: f32,
    pub why: PredictedFailureMode,           // back-reference
}
```

The Oracle head emits per-test logits already; this struct just binds them to the failure-mode head's reasoning.

### 3.3 GuardViolation

```rust
pub struct GuardViolation {
    pub embedder: EmbedderId,
    pub chunk: ChunkId,
    pub centroid_id: CentroidId,             // which (category × entity × language × embedder) cell
    pub cosine: f32,
    pub threshold_tau_m: f32,
    pub deficit: f32,                        // tau_m - cosine
}
```

This is the TCT-side reasoning: "the predicted panel for this chunk's E_TypeGraph signal is 0.61 cosine to the WrongAlgorithm centroid; threshold is 0.72; deficit 0.11." The agent can interpret this as "ME-JEPA's geometric model says this chunk's predicted post-execution behavior does not look like a known-good algorithm."

### 3.4 ExemplarMatch

```rust
pub struct ExemplarMatch {
    pub exemplar_prediction_id: Option<[u8; 16]>,
    pub task_id: SwebenchTaskId,
    pub mutation_kind: MutationKind,
    pub similarity_score: f32,               // patch-similarity-graph edge weight
    pub diff_summary: String,
    pub oracle_outcome: OracleVerdict,       // what happened to the exemplar
    pub failure_mode_class: Option<FailureModeClass>,
    pub evidence_path: Option<String>,
    pub witness_hash: WitnessHash,
}
```

Produced by TASK-PY-G-042 from the schema-versioned patch-similarity HNSW index. The active index metadata lives in `CF_MEJEPA_PATCH_SIMILARITY_META`; graph/records/meta files live under `/mnt/d/contextgraph/storage/patch-similarity-index-v1/`. The agent sees "the closest known case is `psf__requests-2317`, which failed in the same way," with the exemplar prediction id and evidence path available for replay.

---

## 4. The Q4 surface — what else could go wrong

### 4.1 PredictedEdgeCase

```rust
pub struct PredictedEdgeCase {
    pub edge_class: EdgeCaseClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub triggering_input_description: String,
    pub covered_by_test: bool,               // false if no existing test exercises it
    pub confidence: f32,
}

pub enum EdgeCaseClass {
    EmptyInput,                              // empty list / string / dict / null
    SingleElement,                           // n=1 case
    BoundaryValue,                           // INT_MAX / 0 / -1 / NaN / "" / extremely long string
    UnicodeEdge,                             // non-ASCII / RTL / combining marks / zero-width
    WhitespaceOnly,                          // all-whitespace input
    LeadingTrailingWhitespace,
    DuplicateKeys,                           // dict with collision
    CircularReference,                       // graph / object cycle
    SelfReference,                           // foo == foo identity vs equality
    DeepNesting,                             // recursion to platform stack limit
    LargeInput,                              // memory / time scaling
    UninitializedField,                      // partial construction
    OptionalNotPresent,                      // Option::None / undefined / missing key
    DefaultValueLeakage,                     // mutable default arg in Python; nil receiver in Go
    ConcurrentAccess,                        // read during write
    PartialFailure,                          // half-applied transaction
    CrossPlatform,                           // path separator / line ending / case sensitivity
    TimezoneTransition,
    Locale,                                  // decimal separator / number formatting
    NetworkPartition,                        // dependency unreachable
    DiskFull,
    PermissionDenied,
    Other(String),
}
```

**Label source for the edge-case head:**
- Coverage delta: when the patch adds/modifies code paths not exercised by the test suite, the coverage tool's diff identifies the uncovered lines. A symbolic-execution proxy (deterministic AST walk extracting branch conditions) generates `triggering_input_description` candidates.
- Mutation corpus: the existing 8 mutation operators (KnownGood, SubtleFlip, OffByOne, ...) provide ground-truth edge cases. We extend with a 9th category `MutationKind::EdgeCaseExploit` that generates inputs targeting the predicted edge.
- Pattern catalog: per-language ruleset of well-known edge cases (mutable default args in Python, integer overflow in C, etc.).

### 4.2 PredictedLatentBug

```rust
pub struct PredictedLatentBug {
    pub bug_class: LatentBugClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub confidence: f32,
    pub severity: Severity,
    pub explanation: String,
}

pub enum LatentBugClass {
    ShadowedVariable,
    ShadowedImport,
    InconsistentNullCheck,                   // some paths check, some don't
    InconsistentErrorHandling,               // some paths handle, some swallow
    UnreachableBranch,
    DuplicateCondition,                      // if A; elif A
    SwitchFallthrough,                       // missing break / return
    LooseEquality,                           // == in JS where === intended
    BareException,                           // except Exception with no logging
    LoggerNotConfigured,                     // logs to silent default
    ForgottenAwait,                          // async function called sync
    ForgottenYield,                          // generator returns instead of yielding
    ForgottenReturn,                         // function falls through with implicit None
    ResourceNotReleasedOnError,              // try without finally
    LockHeldAcrossAwait,
    NondeterministicIteration,               // dict iteration order assumed
    HashableMutation,                        // mutating a hash-key object
    Other(String),
}
```

**Label source:** static analysis tool snapshot (clippy + pylint + eslint + bandit + cargo audit) before vs after; the patch contributes a "delta" of warnings; warnings introduced by the patch become labels.

### 4.3 PredictedTechDebt

```rust
pub struct PredictedTechDebt {
    pub debt_class: TechDebtClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub severity: Severity,
    pub explanation: String,
}

pub enum TechDebtClass {
    HighCyclomaticComplexity,
    DeepNesting,
    LongFunction,                            // > N lines
    LongParameterList,                       // > N params
    LongFile,                                // > N lines (per CLAUDE.md: 500)
    DuplicatedCode,                          // copy-paste detector
    DataClumps,                              // 3+ params always passed together
    FeatureEnvy,                             // method uses other class more than its own
    ShotgunSurgery,                          // change required across many files
    GodObject,
    PrematureAbstraction,                    // single-call interface
    DeadFlagBranch,                          // feature flag branch never enabled
    TodoWithoutOwner,                        // TODO/FIXME/XXX without name + date
    CommentedOutCode,                        // dead code as comment
    MagicNumber,
    StringLiteralForEnum,                    // string used as discriminant
    CopyOnWriteAbuse,                        // .clone() in hot path
    SerdeUntagged,                           // expensive untagged enum deserialization
    UnboundedRetry,                          // retry loop without backoff
    UnboundedCache,                          // grows without eviction
    Other(String),
}
```

**Label source:** complexity metrics from `radon` / `lizard` / `clippy::cognitive_complexity`; duplicate detection from `pmd` / `simian`; TODO regex; CLAUDE.md's "files under 500 lines" rule.

### 4.4 PredictedDeadCode

```rust
pub struct PredictedDeadCode {
    pub kind: DeadCodeKind,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub reason: String,
}

pub enum DeadCodeKind {
    UnusedFunction,
    UnusedClass,
    UnusedImport,
    UnusedVariable,
    UnreachableAfterReturn,
    UnreachableInfiniteLoop,
    DeadParameter,                            // never read inside fn
    DeadField,                               // struct field never accessed
    AlwaysFalseBranch,
    AlwaysTrueBranch,
    Other(String),
}
```

**Label source:** static analysis (`vulture` for Python, `cargo +nightly udeps` for Rust unused deps, `eslint` no-unused-vars, `tsc --noUnusedLocals`); reachability graph computed from `E_DataFlow` + `E_CFG`.

### 4.5 PredictedRedundancy

```rust
pub struct PredictedRedundancy {
    pub kind: RedundancyKind,
    pub chunk: ChunkId,
    pub also_at: Vec<ChunkId>,               // sibling chunks doing the same thing
    pub similarity: f32,                     // E7 + E14 cosine
    pub explanation: String,
}

pub enum RedundancyKind {
    DuplicatedFunction,                      // near-clone of an existing function
    DuplicatedTest,                          // test that asserts same property as another
    DuplicatedConstant,
    OverlappingErrorMessage,                 // two error messages with > 0.95 cosine
    ParallelImplementation,                  // implements something already in stdlib / dependency
    ReinventedWheel,                         // own crypto / parser / regex when dep available
    Other(String),
}
```

**Label source:** E7 (Qodo code) + E14 cosine across the working tree; for `ReinventedWheel` we maintain a small per-language registry of standard solutions.

### 4.6 PredictedPerfRegression

```rust
pub struct PredictedPerfRegression {
    pub axis: PerfAxis,
    pub chunk: ChunkId,
    pub predicted_delta_pct: f32,            // can be negative (improvement)
    pub baseline_value: Option<f64>,
    pub confidence: f32,
    pub explanation: String,
}

pub enum PerfAxis {
    CpuMs,
    WallclockMs,
    RssKb,
    HeapAllocs,
    HeapBytes,
    StackBytes,
    BinarySize,
    StartupMs,
    P50LatencyMs,
    P99LatencyMs,
    Throughput,
    NetworkBytesIn,
    NetworkBytesOut,
    DiskReadBytes,
    DiskWriteBytes,
    GpuMemoryMb,
    Other(String),
}
```

**Label source:** `pytest-benchmark` / `cargo bench` / `time` / `valgrind --tool=massif` runs against the patch on the corpus; deltas vs the KnownGood baseline.

### 4.7 PredictedSecurityConcern

```rust
pub struct PredictedSecurityConcern {
    pub class: SecurityConcernClass,
    pub chunk: ChunkId,
    pub line_range: (u32, u32),
    pub cvss_estimate: Option<f32>,
    pub explanation: String,
}

pub enum SecurityConcernClass {
    SqlInjection,
    CommandInjection,
    PathTraversal,
    Xss,
    Csrf,
    Ssrf,
    XmlExternalEntity,
    Deserialization,                         // unsafe pickle / yaml.load / etc.
    InsecureRandom,                          // crypto from non-CSPRNG
    HardcodedSecret,
    LoggingSecret,                           // secret leaked into log line
    OverbroadException,                      // catches and ignores security-relevant exception
    InsecureCryptoAlgo,                      // MD5 / SHA-1 / RC4 / DES
    InsufficientCryptoKeyLength,
    MissingAuth,                             // endpoint without auth check
    BrokenAccessControl,                     // user can access another user's resource
    OpenRedirect,
    ClickjackingMissing,                     // X-Frame-Options missing
    MissingTlsVerify,                        // verify=False or InsecureSkipVerify
    Other(String),
}
```

**Label source:** `bandit` (Python), `cargo audit` + `cargo deny` (Rust), `eslint-plugin-security` (JS), `semgrep` rulesets; CVE database matches via dep-version delta.

### 4.8 PredictedAccuracyDegradation

For ML / data-pipeline code:

```rust
pub struct PredictedAccuracyDegradation {
    pub metric: AccuracyMetric,
    pub chunk: ChunkId,
    pub predicted_delta: f32,
    pub baseline_value: Option<f64>,
    pub confidence: f32,
    pub explanation: String,
}

pub enum AccuracyMetric {
    F1, Precision, Recall, Auc, Accuracy,
    MeanAbsoluteError, MeanSquaredError, R2,
    PerplexitySnapshot,
    CalibrationError, BrierScore,
    EmbeddingSimilarity,
    DownstreamTaskScore(String),
    Other(String),
}
```

**Label source:** test outputs that include accuracy metrics (e.g., `test_model_f1_above_0_9`); CI artifact diffs.

### 4.9 PredictedCostRegression

```rust
pub struct PredictedCostRegression {
    pub axis: CostAxis,
    pub chunk: ChunkId,
    pub predicted_delta: f64,
    pub baseline_value: Option<f64>,
    pub explanation: String,
}

pub enum CostAxis {
    CiMinutes,
    CiCpuSeconds,
    CiWallSeconds,
    EstimatedDollarsPerRun,
    LlmTokensPerCall,
    LlmDollarsPerCall,
    StorageBytes,
    EgressBytes,
    DependencyCount,
    BuildSeconds,
    Other(String),
}
```

**Label source:** CI duration deltas; agent token-usage telemetry; build-script timing; dep-graph counter.

### 4.10 ReasoningClass

For the agent's accompanying explanation (the natural-language part of the agent's response that says *what* it did and *why*):

```rust
pub enum ReasoningClass {
    Correct,                     // agent's reasoning aligns with the change made
    MostlyCorrect,               // minor inaccuracy that doesn't change the verdict
    PlausibleButWrong,           // sounds right; wrong about the change made
    Hallucination,               // claims things that did not happen
    Hedging,                     // "I think this might..." — calibration warning
    Overclaiming,                // "this fixes everything" — calibration warning
    UnderClaiming,               // "I'm not sure this works" but it does — calibration warning
    Mute,                        // agent did not explain
}
```

**Label source:** weak supervision via E_Reasoning agreement with E_Oracle; specifically, when the agent's described action doesn't match the chunks actually changed (E_Reasoning ⟂ E_Diff), classify as `Hallucination`; when E_Reasoning describes outcome that disagrees with E_Oracle's predicted outcome, classify as `PlausibleButWrong`.

---

## 5. Q1 — existence reconciliation

### 5.1 AgentClaimGraph

```rust
pub struct AgentClaimGraph {
    pub raw_response: String,
    pub claims: Vec<AgentClaim>,
}

pub struct AgentClaim {
    pub kind: ClaimKind,
    pub text: String,            // the quoted snippet from the agent's response
    pub references: Vec<ClaimReference>,
}

pub enum ClaimKind {
    Added(SymbolRef),            // "I added function X to file Y"
    Modified(SymbolRef),         // "I changed function X in file Y"
    Removed(SymbolRef),
    Renamed(SymbolRef, String),
    Fixed(SymbolRef),
    Tested(TestId),              // "I added a test for X"
    WillPass(TestId),            // "tests/test_x.py::test_y will pass"
    WillFail(TestId),            // "tests/test_x.py::test_y should fail"
    Verified(String),            // "I verified that X works"
    Refactored(String),
    Documented(SymbolRef),
    Other(String),
}

pub struct ClaimReference {
    pub file: PathBuf,
    pub symbol: Option<String>,
    pub line: Option<u32>,
}
```

**Extraction:** a deterministic rule-based parser over the agent's response (`response.md` produced by Claude Code) extracting noun-phrases that match `(added|modified|removed|renamed|fixed|tested|verified|refactored|documented) <symbol> in <file>` plus `will pass` / `will fail` test claims. No LLM call. Qualified Python test ids such as `tests/test_auth.py::test_login` split into file and symbol references.

### 5.2 ClaimReconciliation

```rust
pub struct ClaimReconciliation {
    pub claim: AgentClaim,
    pub status: ReconciliationStatus,
    pub evidence: Vec<EvidenceRow>,
}

pub enum ReconciliationStatus {
    Confirmed,
    Matched,                     // claim and bytes agree under the active hardening path
    Missing,                     // claim says added X; X is not present
    Modified,                    // claim says modified X to do Y; X exists but doesn't do Y
    ModifiedUnexpectedly,        // symbol exists but not at the claimed line/window
    UnexpectedSideEffect,        // X exists but file Z was also modified — not claimed
    SuperficialMatch,            // symbol exists but signature/body diverges
    Ambiguous,                   // line hint is outside file or target cannot be disambiguated
    AmbiguousRef,                // claim references "the helper function" — multiple matches
    Unverifiable,                // claim is too vague to bind to bytes
}

pub struct EvidenceRow {
    pub file: PathBuf,
    pub before_sha: Sha256,
    pub after_sha: Sha256,
    pub line_range: (u32, u32),
    pub contributing_shift_id: ShiftId,
}
```

The reconciliation reads the shift log + bytes-on-disk + the AST chunker; for `Matched` / `Confirmed`, the agent's claim and the SHA-bound evidence agree. For `Missing` / `ModifiedUnexpectedly` / `Modified` / `UnexpectedSideEffect` / `SuperficialMatch`, the agent's words do not match the bytes and are surfaced to the agent via the next prompt injection. TASK-PY-G-041 persists schema-versioned `ClaimReconciliationRecord` rows to `CF_MEJEPA_CLAIM_RECONCILIATION` keyed by `prediction_id x claim_id`; FSV `/mnt/d/contextgraph/fsv/task-py-g-041-claim-graph-extractor-fsv/run-1779077268576/claim_graph_extractor_fsv.json` sha256 `ff5ade6a7b410e2d46a48eca245b9514239b3eae28de65c7578cf49536c37cb9` proves 50 real response samples, `1.0` Added/Modified match rate on green Python rows, and required fail-closed edges.

---

## 6. Q5 — reality impact (post-shift)

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
    Aligned,                     // observed matches predicted
    UnderPredicted,              // more changes / failures than predicted
    OverPredicted,               // fewer changes / failures than predicted
    DivergentInClass,            // same count but different specific items
    Surprise,                    // catastrophic prediction failure → SurpriseEvent recorded
}
```

This is what closes the inference→reality→re-train loop. Each `RealityImpact` evaluation is itself a training example: the system records its own prediction correctness.

---

## 7. Severity and confidence

```rust
pub enum Severity {
    Info,        // FYI; default when prediction confidence is low
    Low,         // worth knowing; ignore-by-default in agent UI
    Medium,      // surface in agent's prompt
    High,        // block "I'm done" until addressed
    Critical,    // block + escalate to operator alert
}

pub struct ConformalInterval {
    pub lower: f32,
    pub upper: f32,
    pub method: ConformalMethod,             // SplitConformal | Mondrian | Group
    pub coverage_target: f32,                // typically 0.90
    pub empirical_coverage: f32,             // current measured on calibration set
}
```

Severity is computed per-prediction by the head's calibrated logit + a per-class severity-floor mapping. Confidence intervals come from the conformal predictor; the agent UI can render `[lower, upper]` next to each prediction.

---

## 8. Rendering: what the agent actually sees

When `RealityPrediction` is injected into UserPromptSubmit context, it's rendered as compact markdown:

```markdown
### Reality prediction (chunks affected since last prompt: 3)

**Verdict:** Fail (oracle pass confidence 0.07 [0.04, 0.12])

**Predicted failure modes (ranked):**
1. `astropy/modeling/separable.py:62-118` — **WrongAlgorithm**
   - predicted FAIL of `test_separability_compound_models`, `test_nested_compound_separability` (confidence 0.93)
   - root cause: `WrongAlgorithm` (left-vs-right child indexing inverted in nested CompoundModel walk)
   - violating embedders: E_TypeGraph, E_DataFlow
   - closest exemplar: `psf__requests-2317` (same inverted-traversal class)

**Predicted edge cases not covered by tests:**
- `EmptyInput`: empty CompoundModel branch (current code path raises IndexError)
- `DeepNesting`: mutually-recursive CompoundModel (predicted hang)

**Predicted latent bugs:** none above threshold.

**Predicted tech debt added (Medium):** 1 new TODO without owner at `astropy/modeling/separable.py:78`.

**Predicted dead code (Low):** function `_legacy_separable` now unreachable from any test.

**Predicted perf / security / accuracy / cost regressions:** none above threshold.

**Reasoning class:** `MostlyCorrect` (your explanation matches the diff except you said "I removed the recursion limit" but the limit is still present at line 91)

**Claim reconciliation:**
- ✅ "I added function `_walk_compound`" — confirmed at `astropy/modeling/separable.py:142`
- ⚠️ "I removed the recursion limit" — `Modified` (limit at line 91 still present)

**Reality impact:** 1 file changed as predicted; 0 unexpected; 2 tests would flip PASS→FAIL as predicted.

**To respond to a wrong prediction:** call `mejepa_record_agent_feedback` with `prediction_id=pred_01HXYZ...`.
```

The rendering is bounded (≤ 4 KB by default; tightened from the 16 KB initial cap per turn 79 decision in `memory/decisions/agent-79-...md`) and the agent can always fetch the full structured `RealityPrediction` via `mejepa_predict_latest`.

---

## 9. How predictions get learned (per-head label sources)

| Head | Primary label source | Secondary signal | Where in code |
|---|---|---|---|
| Panel | frozen-instrument target on observed panel[t=2] | VICReg variance/cov regularization | shipped (`predictor` + `loss/vicreg.rs`) |
| Oracle | Docker test results from Phase 0 corpus + CI runs from Mode B reality capture | E_Test agreement | shipped (`oracle_head.rs`) |
| Failure-mode | rule-based extraction from pytest/cargo-test/jest tracebacks; patch-similarity-graph nearest exemplar | TCT Gτ guard violation pattern | code_complete Phase B; exemplar hardening remains |
| Edge-case | coverage diff + symbolic-execution proxy on AST + per-language pattern catalog | mutation operator (`MutationKind::EdgeCaseExploit` proposed) | partial/code_complete; coverage/symbolic hardening remains |
| Tech-debt | clippy/pylint/eslint warning-delta + radon/lizard complexity + duplicate detector | E14 (semantic-code) cosine for redundancy | partial/code_complete; analyzer deltas remain |
| Perf | pytest-benchmark / cargo bench / valgrind massif deltas | per-axis regression head | partial/code_complete; benchmark integration remains |
| Security | bandit / cargo-audit / eslint-security / semgrep + CVE-db on dep deltas | per-class severity floor | partial/code_complete; SAST/CVE deltas remain |
| Accuracy | test-output metric extraction (regex on common metric names) | E_Reasoning agreement with E_Oracle when ML-test detected | partial/code_complete; broader metric catalog remains |
| Cost | CI minute counters + agent token telemetry + build-script timing + dep-count delta | per-axis regression head | partial/code_complete; CI/token telemetry remains |
| Reasoning | E_Reasoning ⟂ E_Diff agreement / E_Reasoning predictions vs E_Oracle outcomes | weak-supervised | partial/code_complete; embedder-agreement supervisor remains |

Phase B added the typed prediction surfaces and unified deterministic extractor in `crates/context-graph-mejepa-train/src/labels.rs`. Remaining work is trained quality: per-tool/per-language analyzers, production DDA/neural feature consumption, and head-specific holdout gates per `12_IMPLEMENTATION_PHASES.md`.

---

## 10. Operating principles

1. **Always emit. Never elide.** Every category has a vec, every vec is populated even with `[]`. The agent sees the full surface; the absence of a prediction is itself a prediction.
2. **Confidence-calibrated, not threshold-binarized.** Every prediction has a confidence; the agent UI may filter by severity but the system always serves the full distribution.
3. **Citation-grounded.** Every prediction names the chunk_id (and line range) and the contributing embedders, so the agent can independently inspect the basis.
4. **Falsifiable.** Every prediction is testable: run the test, look at the static-analysis warning, run the benchmark. The Oracle / coverage / static-analysis / benchmark are the labels.
5. **Provenance-bound.** Every prediction is bound to a `witness_hash`; the prediction → feedback loop (Q5 + agent-feedback) writes a bound entry tying the verdict to the eventual outcome.
6. **Schema-stable.** Adding a new prediction class extends an enum; old fields don't change shape. Predictions written months ago are still readable.
7. **Generic across languages.** Every class above is language-agnostic; per-language realizations are config (the per-language pattern catalogs, the per-language analyzer call sites) not core schema.
