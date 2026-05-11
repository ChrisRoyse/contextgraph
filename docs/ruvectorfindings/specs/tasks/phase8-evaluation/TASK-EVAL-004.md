## 🔍 AI-Agent Implementation Mandate (read FIRST — fresh-context onboarding)

> **You are an AI coding assistant inheriting this task with no prior context. This block gives you everything you need.**

### 1. Project state pointers — read these before touching code

- **Live shipping status table:** `docs/ruvectorfindings/specs/_README.md` Domain map (one row per phase, current as of 2026-05-10).
- **Per-phase deliverable status:** `docs/ruvectorfindings/specs/implementation-plan.md` "Per-phase deliverable table".
- **Constitution (immutable rules):** `docs/ruvectorfindings/specs/constitution.md`.
- **This phase's status snapshot:** the `_index.md` in this same directory.
- **Recent activity:** `git log --oneline -30` shows the last 30 commits.
- **Architecture intent:** `docs/ruvectorfindings/{09,10,11,12,13,14}*.md` (read 09 master plan + 10 pivot first).
- **Project rules:** `CLAUDE.md` at workspace root.

### 2. ⚠ ASSUME THIS TASK IS WRONG — audit before you implement

This task body was authored 2026-05-09. Before implementing, audit it against the current codebase:

1. Do the file paths it references still exist? `find` / `ls` them.
2. Do the type signatures it specifies still match what's in tree? `grep` for the type names in `crates/`.
3. Has the crate it tells you to "create" already been created? Check `crates/`. If it exists, STOP — read what's there, reconcile against the task, and update the task itself before implementing more.
4. Does the spec it cites (e.g., `TECH-<DOMAIN>.md`) still describe the same data shapes? Re-read those specs.

If you find drift, document it in your handoff (`memory/handoff/`) and either (a) update the task to match reality before continuing, or (b) escalate to the operator if scope changed materially.

### 3. Hard rules — ZERO tolerance, fail-closed

1. **No backwards compatibility.** Schema changes wipe + recreate; never write migration shims. Synthetic data only.
2. **No mock data in tests.** Use real types, real corpus rows, real RocksDB CFs, real file SHAs. Tests must FAIL when the system is broken — never write a test that passes against a broken state.
3. **Fail closed with structured error codes** (e.g., `MEJEPA_*`, `UTML_*`). No `unwrap_or(default)` on confidence / quota / correlation / signature / chain-integrity paths. No silent fallback. Every error has `.code()` returning `SCREAMING_SNAKE_CASE`.
4. **Robust error logging.** Every fail-path emits `tracing::error!` with the structured code + actionable context (which file, which CF, which byte offset, which step). When something breaks, the log must say exactly what failed and how to fix it.
5. **No comments unless they encode non-obvious WHY.** Don't explain WHAT — well-named identifiers do that.
6. **Files under 500 lines** (split if exceeded).
7. **Provenance comment** at top of every new `.rs` file: `// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).` Per `memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md`.

### 4. Mandatory Full State Verification (FSV) — every task ships with it

Do not rely on return values alone. After the logic runs, you MUST:

1. **Define the Source of Truth.** State explicitly where the final result lives: a RocksDB column family, a `/tmp/contextgraph-*-fsv/*.json` file, a witness-chain segment, a tier-compressed panel blob, a CF watermark row, a tensor allocator's `live_tensors` map, etc.
2. **Execute & Inspect.** Run the logic, then perform a SEPARATE READ on the Source of Truth to verify the data was processed correctly. Cite the readback command (e.g., `cargo run --release -p <crate> --example <fsv_example>` or a direct CF dump).
3. **Boundary & Edge Case Audit.** Manually simulate AT LEAST 3 edge cases — empty inputs, maximum limits, invalid formats, NaN/Inf in floats, control chars in strings, byte offsets at file boundary, etc. For each, print the system state BEFORE and AFTER the action.
4. **Evidence of Success.** Provide a log showing the actual data residing in the system after execution. Cite the log path in the task's `<verification>` block.

### 5. Trigger → outcome verification (mandatory database / artifact check)

> Every triggered event exists to produce a specific intended outcome. The trigger is observable; the outcome is queryable. If your task writes anything to a persistent surface, you MUST query that surface AFTER running and confirm the expected data is there. Do not trust return values alone.

Synthetic protocol: pick an input where you KNOW the expected output. Run it. Manually check the source of truth for the expected output (e.g., open RocksDB and dump the CF, read the FSV JSON, replay the witness chain). Document the trigger → outcome chain in the FSV evidence. Examples:

- Trigger: `compute_l_step({delta_p:0, delta_k:1, delta_omega:1, delta_xi:1})` → Outcome: returns `0.0` exactly (multiplicative gate; any factor at zero zeros the product). FSV: dump training cert, confirm `l_step == 0.0`.
- Trigger: insert `ShiftEntry { shift_id: "01J0123456789ABCDEFGHIJK", ... }` → Outcome: row in `CF_MEJEPA_SHIFT_WATERMARK` with that exact key. FSV: open RocksDB, `cf.get(b"01J0123456789ABCDEFGHIJK")` returns Some.

### 6. Occam's razor

The simplest implementation that satisfies the contract is the correct one. Do NOT add features, abstractions, or fallbacks beyond the task scope. A bug fix doesn't need surrounding cleanup. Three similar lines beats a premature abstraction. If the task says "create struct X with 5 fields", create exactly that — not a builder, not a derive macro family, not a generic over a placeholder.

### 7. Subagent / verification expectations

After implementation:
- Run `cargo build --release -p <crate>` AND `cargo test -p <crate>` AND `cargo clippy -p <crate> -- -D warnings`.
- Spawn the `code-simplifier` subagent (or equivalent) on your changes; apply BLOCKER fixes.
- Write a final SMAP report at `memory/reports/agent-{N}-{role}--final-report.md`.
- Touch the corresponding row in `tasks/_traceability.md` if the REQ→TASK mapping changed.

---

## 📋 2026-05-11 STATE AUDIT (sharpened against verified codebase)

> **Codebase reality table:** see `_index.md` in this directory.

**Confirmed 2026-05-11:**
- `crates/context-graph-mejepa/src/compiler.rs::MeJepaCompiler::compile` SHIPPED (line ~199 vicinity) with `compile(patch, context) -> Result<RealityPrediction, MejepaInferError>`.
- `crates/context-graph-mejepa-train/src/eval/holdout.rs::split_holdout_via_mincut` SHIPPED + `assert_no_test_patch_leakage` available.
- The train-internal `HoldoutEvaluator` in `eval/mod.rs:121` is per-epoch + train-side. Phase 8 `EvalRunner` is operator-facing — DO NOT delegate to `HoldoutEvaluator`; build a fresh `EvalRunner` that takes `Arc<MeJepaCompiler>` + a holdout dataset and produces a public `EvalReport`.

**Decided scope for this task:**
1. New file `crates/context-graph-mejepa/src/eval/runner.rs` (created by sister TASK-EVAL-001's mod tree).
2. `pub struct EvalRunner { compiler: Arc<MeJepaCompiler>, store: Arc<dyn EvalStore>, config: EvalConfig }`.
3. `pub async fn run_holdout_eval(&self, holdout: HoldoutDataset, report_date: chrono::NaiveDate) -> Result<EvalReport, EvalError>`:
   - **Leakage check** FIRST: invoke `assert_no_test_patch_leakage(holdout)?` — if any panel SHA appears in train OR calibration splits, return `EvalError::LeakageDetected`.
   - For each panel in holdout: dispatch `MeJepaCompiler::compile(patch, context).await?` via `tokio::task::spawn(...)` with `holdout_max_concurrent_panels` parallelism (default 8).
   - Collect predictions; group by `(MutationCategory, Language)` cell.
   - `compute_pearson_correlation(predictions, oracle_outcomes) -> Result<f32, EvalError>` returns clamped `[-1, 1]`.
   - Aggregate `per_category: BTreeMap<MutationCategory, f32>`, `per_language: BTreeMap<Language, f32>`, `per_cell: BTreeMap<(MutationCategory, Language), f32>`.
   - `compute_bayesian_shrinkage(per_cell)` for cells with N < 50 samples (cross-cuts TCT shrinkage; the formula is documented in TECH-TCT and reused here).
   - Build `EvalReport` via `EvalReport::try_new(builder)`; persist to `CF_MEJEPA_EVAL_REPORTS` keyed by `report_date.iso8601()`.
4. NaN/Inf in any predicted-outcome value → return `EvalError::PanelMaterializationFailed`; never silently skip.
5. NO mock data. The synthetic holdout used in tests is REAL panels constructed via `mejepa::fixtures::synthetic_holdout(8 categories × 11 languages)`.

**FSV for this task:** integration test (a) constructs synthetic holdout with KNOWN oracle outcomes per cell → asserts `per_category[KnownGood] == 1.0` exactly + `per_category[CompileError] < 1.0` + every cell finite; (b) inserts a leakage point into both train and holdout → asserts `EvalError::LeakageDetected` returned (Source of Truth: error code; CF UNCHANGED — separate read of `CF_MEJEPA_EVAL_REPORTS` confirms no row written); (c) measures wall time for the 1,000-panel synthetic holdout → asserts < 30 min p50.

---

# TASK-EVAL-004 — EvalRunner::run_holdout_eval core (leakage check + per-panel forward + Pearson per_category/per_language/per_cell)

```xml
<task_spec id="TASK-EVAL-004" version="1.0">

<metadata>
  <title>Implement `EvalRunner::new` + `run_holdout_eval` core (leakage check, per-panel `compile()`, Pearson correlations, Bayesian shrinkage entry-point) in `crates/context-graph-mejepa/src/eval/runner.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>4</sequence>
  <implements>
    <item ref="REQ-EVAL-01">EvalRunner::new + run_holdout_eval signatures</item>
    <item ref="REQ-EVAL-02">EvalReport population (overall_correlation, per_category, per_language, per_cell)</item>
    <item ref="REQ-EVAL-14">assert_no_test_patch_leakage on every run</item>
    <item ref="REQ-EVAL-16">Performance budget: < 30 min p50 on 240-task holdout, RTX 5090, cached instruments</item>
    <item ref="component:EvalRunner.new">verify holdout non-empty + config sane bounds + load active learning queue</item>
    <item ref="component:EvalRunner.run_holdout_eval">leakage → forward → accumulate → Pearson per cell</item>
    <item ref="component:EvalRunner.compute_pearson_correlation">Pearson r = cov / (σ_P · σ_A); clamp [-1, 1]</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-001</task_ref><!-- EvalReport, EvalReportBuilder, MutationCategory -->
    <task_ref>TASK-EVAL-002</task_ref><!-- EvalError, EvalConfig, ActiveLearningQueueState -->
    <task_ref>TASK-EVAL-003</task_ref><!-- CF constants + serializers -->
    <task_ref>TASK-INFER-003</task_ref><!-- MeJepaCompiler::compile available -->
    <task_ref>TASK-CORPUS-017</task_ref><!-- assert_no_test_patch_leakage helper -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 created (runner.rs, correlation.rs)</estimated_files>
</metadata>

<context>
This task lands the heart of Phase 8: `run_holdout_eval`. It runs the predictor's `compile()` on
every holdout panel, accumulates `(predicted_oracle_pass, actual_oracle_pass, language,
mutation_category)` tuples, and computes Pearson correlation per cell. Per-language stratification
(005), OOD AUC (006), Gτ pass rate (007), Q1/Q2/Q3 (012), state-transfer (011), regression
detection (012), and ship-gate (008) all assemble onto the report this task produces. Bayesian
shrinkage entry-point lives here; the gate-skip logic for shrunk cells is in TASK-EVAL-008.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets phase8_ship_gate; tct_invariants rule 5 Bayesian shrinkage parallel pattern)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-01, REQ-EVAL-02, REQ-EVAL-14, REQ-EVAL-16, REQ-EVAL-20; AC-EVAL-01-01..05; EC-EVAL-01 empty holdout; EC-EVAL-06 missing language)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (EvalRunner.new behavior; run_holdout_eval 19-step behavior; compute_pearson_correlation; compute_bayesian_shrinkage)</file>
  <file purpose="prior_task">TASK-EVAL-001..003 (data models, error, CFs)</file>
  <file purpose="cross_domain">crates/context-graph-mejepa/src/compiler.rs (TASK-INFER-003 MeJepaCompiler::compile)</file>
  <file purpose="cross_domain">crates/context-graph-mejepa-corpus/src/split.rs (assert_no_test_patch_leakage helper)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-001..003 shipped</check>
  <check>TASK-INFER-003 shipped: `MeJepaCompiler::compile(&PatchBundle, &TaskContext) -> Result<RealityPrediction, _>`</check>
  <check>TASK-CORPUS-017 shipped: `assert_no_test_patch_leakage(train_split, holdout_split) -> Result<(), LeakageReport>`</check>
  <check>Phase 0 holdout split materialized at `./data/mejepa-corpus/holdout/`</check>
  <check>CF_MEJEPA_ORACLE_VERDICTS populated with actual oracle outcomes</check>
  <check>`rayon` available for parallel batched compile</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/correlation.rs` with:
      - `compute_pearson_correlation(predicted: &[f32], actual: &[f32]) -> Result<f32, EvalError>` per TECH-EVALUATION method behavior
      - `compute_bayesian_shrinkage(cell_correlation: f32, cell_n: usize, language_aggregate_correlation: f32, language_n: usize) -> f32` per REQ-EVAL-20
      - Helpers: `mean(&[f32])`, `variance(&[f32])`, `clamp_corr(f32) -> f32` (clamp to [-1, 1])
    - Create `crates/context-graph-mejepa/src/eval/runner.rs` with:
      - `pub struct EvalRunner { holdout_dataset, calibration_set, predictor, frozen_target, conformal_calibrator, ood_detector, gtau_checker, storage, witness_chain, active_learning_queue: Arc<parking_lot::Mutex<ActiveLearningQueueState>>, config: EvalConfig }`
      - `pub fn EvalRunner::new(...) -> Result<Self, EvalError>` per TECH-EVALUATION:
        1. Verify holdout dataset is non-empty → `EvalError::EmptyHoldout`
        2. Verify config sane via `config.validate()`
        3. Verify calibration store has at least one record (propagates `CalibrationStale` from inference if any)
        4. Initialize ActiveLearningQueueState; load existing entries from CF_MEJEPA_ACTIVE_LEARNING_QUEUE via prefix scan
      - `pub fn EvalRunner::run_holdout_eval(&self, window_days: u32, include_per_cell: bool) -> Result<EvalReport, EvalError>` implementing steps 1-4 of TECH-EVALUATION's 19-step behavior:
        1. Capture wall-clock start; set deterministic seed from `code_version` (sha256 → u64)
        2. Leakage check via `assert_no_test_patch_leakage(train_split, holdout_split)`; on duplicate, return `Err(HoldoutLeakage{task_id, found_in})`
        3. Per-panel forward pass (parallel via `rayon::par_iter` with 4-way thread pool for GPU memory headroom):
            a. For each holdout panel, call `predictor.compile(&panel.patch, &panel.context)` → `RealityPrediction`
            b. Read actual oracle outcome from CF_MEJEPA_ORACLE_VERDICTS keyed by `(panel.task_id, panel.mutation_category)`
            c. Append `(predicted_oracle_pass, actual_pass_as_f32, language, mutation_category)` to `accumulators` vector
        4. Compute correlations:
            a. `overall_correlation = compute_pearson_correlation(all_predicted, all_actual)?`
            b. `per_category[c] = ...` for each of 8 categories filtered from accumulators
            c. `per_language[l] = ...` for each of 11 languages; Option::None when N < min_samples_per_language
            d. `per_cell[(c, l)] = ...` per cell when `include_per_cell == true`; apply Bayesian shrinkage when N < min_samples_per_cell AND `enable_bayesian_shrinkage == true`; record cell name in `shrinkage_applied` vector
            e. Skip categories with 0 samples (return cell as None) AND log `EvalError::InsufficientSamples` to `per_cell_warnings`
      - Stub the remaining steps 5-19 (per-language stratification + Q1/Q2/Q3 + state-transfer + regression + ship-gate + persist) with `// TODO: TASK-EVAL-005..015` comments — those land in subsequent tasks but the function signature must accept the full surface so call sites compile. For this task, populate the report with:
          - `overall_correlation`, `per_category`, `per_language`, `per_cell` (computed)
          - `ood_calibration_health`, `conformal_coverage_health`, `gtau_pass_rate`, `q3_side_effect_agreement` = empty / None placeholders
          - `q1_pass_rate = 0.0`, `q2_correlation = overall_correlation` (alias)
          - `ship_gate_passed = false`, `ship_gate_failures = vec!["MEJEPA_EVAL_NOT_FULLY_GATED_YET"]` (placeholder; replaced when TASK-EVAL-008 lands)
          - `regression_check = RegressionCheck::NoPriorReport`
          - `active_learning = ActiveLearningSummary { queue_depth: queue.lock().len(), ... }` (depth + stub fields)
          - `state_transfer_diagnostic = None`, `aux_head_distillation = None`
          - `open_research_questions = seed_initial_research_questions()`
          - `provenance fields`: corpus_sha + model_weights_sha + calibration_version + constellation_version + code_version + holdout_split_sha all read from environment / config / git
          - Build via `EvalReport::try_new(builder)` (returns `Err(NanDetected)` on violation)
          - Capture wall-clock end; set `latency_budget_exceeded = (wall_clock_seconds > 1800.0)`
      - Add unit + integration tests in `runner.rs` `#[cfg(test)] mod tests` for:
        - `run_holdout_eval_returns_empty_holdout_when_split_empty`
        - `run_holdout_eval_returns_holdout_leakage_when_dup_task` (synthesize duplicate task across train+holdout)
        - `run_holdout_eval_populates_per_category_8_keys`
        - `run_holdout_eval_populates_per_language_11_keys` (some Option::None for under-sampled)
        - `run_holdout_eval_populates_per_cell_only_when_include_flag_true`
        - `compute_pearson_correlation_perfect_match_returns_1` (predicted == actual)
        - `compute_pearson_correlation_zero_variance_returns_0` (degenerate but well-defined)
        - `compute_pearson_correlation_handles_nan_input_via_error` (NaN in input → `EvalError::NanDetected`)
        - `compute_bayesian_shrinkage_weight_correctness` (small N → language aggregate weighted heavier)
        - `latency_budget_exceeded_flagged_when_over_1800s` (mock clock)
    - Update `crates/context-graph-mejepa/src/eval/mod.rs` to declare `pub mod runner; pub mod correlation;` and re-export `EvalRunner`
  </in_scope>
  <out_of_scope>
    - Per-language stratified conformal coverage health (TASK-EVAL-005)
    - Per-language OOD AUC + 4-week drift (TASK-EVAL-006)
    - Per-language Gτ pass rate (TASK-EVAL-007)
    - check_ship_gate (TASK-EVAL-008)
    - enqueue_uncertain + apply_active_learning_label (TASK-EVAL-009)
    - build_patch_similarity_graph (TASK-EVAL-010)
    - Aux-head distillation summary + state-transfer T diagnostic (TASK-EVAL-011)
    - Q1/Q2/Q3 rollup + regression detection (TASK-EVAL-012)
    - MCP handlers (TASK-EVAL-013, TASK-EVAL-014)
    - Weekly report writer (TASK-EVAL-015)
    - CLI subcommand (TASK-EVAL-016)
    - FSV harness (TASK-EVAL-017)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/correlation.rs">
use super::error::EvalError;

pub fn compute_pearson_correlation(predicted: &amp;[f32], actual: &amp;[f32]) -&gt; Result&lt;f32, EvalError&gt;;
pub fn compute_bayesian_shrinkage(
    cell_correlation: f32, cell_n: usize,
    language_aggregate_correlation: f32, language_n: usize,
) -&gt; f32;
fn mean(xs: &amp;[f32]) -&gt; f32;
fn variance(xs: &amp;[f32], mean: f32) -&gt; f32;
fn clamp_corr(r: f32) -&gt; f32;
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/runner.rs">
use std::sync::Arc;
use parking_lot::Mutex;
use crate::compiler::{MeJepaCompiler, Predictor, FrozenTarget};
use crate::types::Language;
use super::error::EvalError;
use super::config::EvalConfig;
use super::queue::ActiveLearningQueueState;
use super::report::EvalReport;
use super::types::ShipGateStatus;

pub struct EvalRunner {
    holdout_dataset: Arc&lt;HoldoutDataset&gt;,
    calibration_set: Arc&lt;CalibrationDataset&gt;,
    predictor: Arc&lt;dyn Predictor + Send + Sync&gt;,
    frozen_target: Arc&lt;dyn FrozenTarget + Send + Sync&gt;,
    conformal_calibrator: Arc&lt;CalibrationStore&gt;,
    ood_detector: Arc&lt;OodDetector&gt;,
    gtau_checker: Arc&lt;ConstellationGuard&gt;,
    storage: Arc&lt;RocksDbStore&gt;,
    witness_chain: Arc&lt;WitnessChainReader&gt;,
    active_learning_queue: Arc&lt;Mutex&lt;ActiveLearningQueueState&gt;&gt;,
    config: EvalConfig,
}

impl EvalRunner {
    pub fn new(
        holdout_dataset: Arc&lt;HoldoutDataset&gt;,
        calibration_set: Arc&lt;CalibrationDataset&gt;,
        predictor: Arc&lt;dyn Predictor + Send + Sync&gt;,
        frozen_target: Arc&lt;dyn FrozenTarget + Send + Sync&gt;,
        conformal_calibrator: Arc&lt;CalibrationStore&gt;,
        ood_detector: Arc&lt;OodDetector&gt;,
        gtau_checker: Arc&lt;ConstellationGuard&gt;,
        storage: Arc&lt;RocksDbStore&gt;,
        witness_chain: Arc&lt;WitnessChainReader&gt;,
        config: EvalConfig,
    ) -&gt; Result&lt;Self, EvalError&gt;;

    pub fn run_holdout_eval(&amp;self, window_days: u32, include_per_cell: bool) -&gt; Result&lt;EvalReport, EvalError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - `EvalRunner::new` returns `Err(EvalError::EmptyHoldout)` when holdout has 0 entries
    - `EvalRunner::new` calls `config.validate()?` before storing config
    - `run_holdout_eval` invokes `assert_no_test_patch_leakage` on EVERY call (NOT cached) per TECH-EVALUATION implementation_notes "leakage check is invoked on every run"
    - Determinism: `code_version` hash seeds the rayon iteration order via stable sort; no `rand::thread_rng()` anywhere
    - `compute_pearson_correlation` rejects:
      - empty slices → `EvalError::NanDetected{field: "pearson_input_empty", cell: None}`
      - mismatched lengths → `EvalError::NanDetected{field: "pearson_length_mismatch", cell: None}`
      - any NaN/Inf in input → `EvalError::NanDetected{field, cell}`
    - `compute_pearson_correlation` returns 0.0 (NOT error) when zero variance (TECH-EVALUATION says "degenerate but well-defined") — this is the explicit exception to the "no silent fallback" rule because the correlation is mathematically undefined and 0.0 is the convention
    - `compute_pearson_correlation` clamps result to [-1, 1] for numerical safety per TECH-EVALUATION
    - `compute_bayesian_shrinkage` weight `w = cell_n / (cell_n + language_n)` per REQ-EVAL-20
    - Per-cell Pearson MUST skip cells with N == 0 (return None entry, log warning); cells with `0 < N < min_samples_per_cell` get shrinkage when enabled, else None
    - Per-language Pearson MUST return Option::None when N < `config.min_samples_per_language`; record in `unsupported_languages` if N == 0
    - All `EvalError::*` returns wrap with `.map_err(|e| { e.log_context("eval::runner"); e })?` per `<error_handling>` "robust error logging"
    - rayon parallelism: 4-way for GPU memory headroom (per TECH-EVALUATION implementation_notes/performance); MUST use `par_iter` with stable iteration order (`.collect::<Vec<_>>()` then deterministic sort by panel_sha before correlation accumulation)
    - No `unwrap_or` on confidence-related fields
    - File length budget: `runner.rs` < 500 lines (per CLAUDE.md §4); split helpers into separate files if needed
    - Rust 2021; Rust 1.84+
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` succeeds with zero warnings
    - `cargo test -p context-graph-mejepa eval::correlation` shows ≥ 4 passing tests
    - `cargo test -p context-graph-mejepa eval::runner` shows ≥ 8 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - Test `run_holdout_eval_returns_empty_holdout_when_split_empty` passes
    - Test `run_holdout_eval_returns_holdout_leakage_when_dup_task` passes (synthesize duplicate)
    - Test `run_holdout_eval_populates_per_category_8_keys` passes
    - Test `run_holdout_eval_populates_per_language_11_keys` passes
    - Test `run_holdout_eval_populates_per_cell_only_when_include_flag_true` passes
    - Test `compute_pearson_correlation_perfect_match_returns_1` passes
    - Test `compute_pearson_correlation_zero_variance_returns_0` passes (graceful)
    - Test `compute_pearson_correlation_rejects_nan_input` passes
    - Test `compute_bayesian_shrinkage_weight_correctness` passes
    - Test `latency_budget_exceeded_flagged_when_over_1800s` passes (mock clock)
    - Determinism test: 3 invocations of run_holdout_eval on identical fixture produce identical `overall_correlation` byte-equal
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/correlation.rs:
  pub fn compute_pearson_correlation(predicted: &[f32], actual: &[f32]) -> Result<f32, EvalError> {
      if predicted.is_empty() || actual.is_empty() {
          return Err(EvalError::NanDetected { field: "pearson_input_empty".into(), cell: None });
      }
      if predicted.len() != actual.len() {
          return Err(EvalError::NanDetected { field: "pearson_length_mismatch".into(), cell: None });
      }
      if predicted.iter().chain(actual.iter()).any(|x| !x.is_finite()) {
          return Err(EvalError::NanDetected { field: "pearson_non_finite".into(), cell: None });
      }
      let mp = mean(predicted);
      let ma = mean(actual);
      let var_p = variance(predicted, mp);
      let var_a = variance(actual, ma);
      if var_p == 0.0 || var_a == 0.0 {
          return Ok(0.0); // degenerate
      }
      let cov: f32 = predicted.iter().zip(actual.iter())
          .map(|(p, a)| (p - mp) * (a - ma)).sum::<f32>() / predicted.len() as f32;
      let r = cov / (var_p.sqrt() * var_a.sqrt());
      if !r.is_finite() {
          return Err(EvalError::NanDetected { field: "pearson_result_non_finite".into(), cell: None });
      }
      Ok(clamp_corr(r))
  }

  pub fn compute_bayesian_shrinkage(
      cell_correlation: f32, cell_n: usize,
      language_aggregate_correlation: f32, language_n: usize,
  ) -> f32 {
      let w = cell_n as f32 / (cell_n + language_n) as f32;
      w * cell_correlation + (1.0 - w) * language_aggregate_correlation
  }

crates/context-graph-mejepa/src/eval/runner.rs:
  impl EvalRunner {
      pub fn new(/* ... */) -> Result<Self, EvalError> {
          if holdout_dataset.is_empty() { return Err(EvalError::EmptyHoldout); }
          config.validate()?;
          // load existing AL queue from CF_MEJEPA_ACTIVE_LEARNING_QUEUE prefix scan
          let queue_state = load_queue_from_storage(&storage, config.active_learning_queue_capacity)?;
          Ok(Self { /* fields */ active_learning_queue: Arc::new(Mutex::new(queue_state)), /* ... */ })
      }

      pub fn run_holdout_eval(&self, window_days: u32, include_per_cell: bool) -> Result<EvalReport, EvalError> {
          let t0 = std::time::Instant::now();
          let seed = sha256_to_u64(&self.config_code_version());

          // step 2: leakage check
          assert_no_test_patch_leakage(&self.train_split(), &self.holdout_dataset)
              .map_err(|leak| EvalError::HoldoutLeakage { task_id: leak.task_id, found_in: leak.found_in })?;

          // step 3: per-panel forward
          let panels = self.holdout_dataset.panels();
          let pairs: Vec<(f32, f32, Language, MutationCategory)> = panels.par_iter()
              .map(|panel| {
                  let pred = self.predictor.compile(&panel.patch, &panel.context).map_err(|e| EvalError::NanDetected {
                      field: "compile_failed".into(), cell: Some(format!("{:?}", panel.task_id))
                  })?;
                  let actual = self.read_oracle_verdict(panel.task_id, panel.mutation_category)?;
                  Ok((pred.predicted_oracle_pass, actual.as_f32(), panel.language, panel.mutation_category))
              })
              .collect::<Result<Vec<_>, EvalError>>()?;

          // sort deterministically
          let mut pairs = pairs;
          pairs.sort_by_key(|t| (t.2 as u8, t.3 as u8));

          // step 4a: overall
          let predicted: Vec<f32> = pairs.iter().map(|t| t.0).collect();
          let actual: Vec<f32> = pairs.iter().map(|t| t.1).collect();
          let overall = compute_pearson_correlation(&predicted, &actual)?;

          // step 4b-d: per_category, per_language, per_cell
          let per_category = self.compute_per_category(&pairs)?;
          let per_language = self.compute_per_language(&pairs)?;
          let per_cell = if include_per_cell { Some(self.compute_per_cell(&pairs)?) } else { None };

          // build report (steps 5-19 stubbed for now)
          let builder = EvalReportBuilder {
              overall_correlation: overall,
              per_category, per_language, per_cell,
              q2_correlation: overall,
              ship_gate_passed: false,
              ship_gate_failures: vec!["MEJEPA_EVAL_NOT_FULLY_GATED_YET".into()],
              wall_clock_seconds: t0.elapsed().as_secs_f64(),
              latency_budget_exceeded: t0.elapsed().as_secs_f64() > 1800.0,
              regression_check: RegressionCheck::NoPriorReport,
              open_research_questions: seed_initial_research_questions(),
              /* other fields stubbed */
              ..EvalReportBuilder::default_stubs()
          };
          EvalReport::try_new(builder)
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/correlation.rs">compute_pearson_correlation + compute_bayesian_shrinkage + helpers + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">EvalRunner struct + ::new + ::run_holdout_eval core (steps 1-4) + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">add `pub mod runner; pub mod correlation;` and re-export EvalRunner</file>
  <file path="crates/context-graph-mejepa/Cargo.toml">add `rayon = { workspace = true }` if not present</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa succeeds with zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa eval::correlation shows ≥ 4 passing tests</criterion>
  <criterion>cargo test -p context-graph-mejepa eval::runner shows ≥ 8 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>EvalRunner::new returns Err(EmptyHoldout) when holdout is empty</criterion>
  <criterion>EvalRunner::new calls config.validate() and propagates errors</criterion>
  <criterion>run_holdout_eval returns Err(HoldoutLeakage) when train+holdout share a task_id</criterion>
  <criterion>run_holdout_eval populates per_category with exactly 8 keys (8 mutation categories)</criterion>
  <criterion>run_holdout_eval populates per_language with exactly 11 keys (11 languages)</criterion>
  <criterion>run_holdout_eval omits per_cell when include_per_cell == false; populates 88 entries when true</criterion>
  <criterion>compute_pearson_correlation returns 1.0 on perfect-match input, 0.0 on zero-variance input, Err on NaN input</criterion>
  <criterion>compute_bayesian_shrinkage weight w = cell_n / (cell_n + language_n) verified on 3 fixture cases</criterion>
  <criterion>3 identical-fixture invocations produce byte-equal overall_correlation</criterion>
  <criterion>runner.rs file < 500 lines per CLAUDE.md §4</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::correlation</command>
  <command>cargo test -p context-graph-mejepa eval::runner</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
