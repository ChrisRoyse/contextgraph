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
- Q1 (source-SHA + chain replay) primitive: `verify_chain_bytes` SHIPPED in `crates/context-graph-witness/`.
- Q2 (overall correlation) is the existing `EvalReport.overall_correlation` field — no new compute.
- Q3 (F1 over predicted vs observed side-effect set) needs Phase 7 `CF_MEJEPA_LIVE_PREDICTIONS` — degrades to None when Phase 7 not shipped.
- Gates 5/6 (predictor quality + ladder) SHIPPED in `crates/context-graph-mejepa/src/gates.rs`.

**Decided scope for this task:**
1. New file `crates/context-graph-mejepa/src/eval/q_rollup.rs`.
2. `compute_q1_pass_rate(certs: &[TrainingCertificate]) -> Result<f32, EvalError>`: for each cert, replay the witness-chain bytes via `verify_chain_bytes`; pass-rate = passing / total. Bound `[0, 1]`; ship gate `>= 0.99`.
3. `compute_q2_correlation(report: &EvalReport) -> f32`: alias for `report.overall_correlation`. (Trivial — exists for symmetry of API.)
4. `compute_q3_side_effect_agreement(predictions: &[Prediction], observed: Option<&[ObservedSideEffect]>) -> Result<Option<f32>, EvalError>`:
   - When `observed.is_none()` (Phase 7 not shipped) → return `Ok(None)` + `tracing::warn!`.
   - Else → compute F1 score over predicted side-effect set vs observed; bound `[0, 1]`; ship gate `>= 0.90`.
5. `detect_regression(prior: &EvalReport, current: &EvalReport) -> RegressionCheck`:
   - If `prior.corpus_sha != current.corpus_sha` → `RegressionCheck::CorpusShaChanged{..}` (cannot compare across corpus changes).
   - Else: compute per-cell delta `delta_pct = (current[cell] - prior[cell]) / prior[cell] * 100`; flag `is_regression: true` when `delta_pct < -2.0` (week-over-week regression > 2 % drop).
6. `apply_bayesian_shrinkage(per_cell: &mut BTreeMap<(MutationCategory, Language), (f32, usize)>, prior_strength: f32)` — for cells with N < 50, shrink toward the global mean per `<bayesian_shrinkage>` formula in TECH-EVALUATION.

**FSV for this task:** integration test (a) feeds synthetic certs with KNOWN Q1 outcomes → asserts pass-rate; (b) feeds Phase-7-absent state → asserts Q3=None + warning; (c) feeds prior + current reports with KNOWN regression → asserts `CellDelta` entries; (d) verifies Bayesian shrinkage formula on synthetic small-sample cells. Source of Truth: returned struct + log capture.

---

# TASK-EVAL-012 — Q1/Q2/Q3 rollup + week-over-week regression detection + Bayesian shrinkage entry-point

```xml
<task_spec id="TASK-EVAL-012" version="1.0">

<metadata>
  <title>Implement Q1/Q2/Q3 rollup + week-over-week regression detection + Bayesian shrinkage in `crates/context-graph-mejepa/src/eval/q123.rs` + `regression.rs`</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements>
    <item ref="REQ-EVAL-11">Q1/Q2/Q3 rollup (Q1 source-SHA + chain replay; Q2 alias overall_correlation; Q3 F1 over predicted vs observed side-effect set)</item>
    <item ref="REQ-EVAL-04">Week-over-week regression detection — populates RegressionCheck on EvalReport (ship gate consumes in TASK-EVAL-008)</item>
    <item ref="REQ-EVAL-20">Bayesian shrinkage entry-point — flagged-cell rollup for `shrinkage_applied` Vec</item>
    <item ref="component:EvalRunner.compute_q1_pass_rate">Gate 5 (witness chain) + Gate 6 (source SHA drift) per panel</item>
    <item ref="component:EvalRunner.compute_q3_f1">F1 over predicted_set vs observed_set per panel; macro-mean across panels</item>
    <item ref="EC-EVAL-03">When prior corpus_sha differs, RegressionCheck::CorpusShaChanged (don't compare cells)</item>
    <item ref="EC-EVAL-07">Bayesian shrinkage applied + flagged when N &lt; min_samples_per_cell</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-001</task_ref><!-- RegressionCheck, CellDelta, ResearchQuestionId -->
    <task_ref>TASK-EVAL-002</task_ref><!-- EvalError, EvalConfig -->
    <task_ref>TASK-EVAL-010</task_ref><!-- PatchGraph for cross-task transfer rollup (optional) -->
    <task_ref>TASK-EVAL-011</task_ref><!-- aux-head + state-transfer for AC-EVAL-09 -->
    <task_ref>TASK-INFER-***</task_ref><!-- Gate 5 (WitnessChainBroken) + Gate 6 (SourceShaDrift) callable per panel -->
    <task_ref>TASK-WITNESS-***</task_ref><!-- verify_chain_bytes for Q1 chain replay -->
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>2 created (q123.rs, regression.rs); 1 modified (runner.rs to wire steps 8 + 9 + 10)</estimated_files>
</metadata>

<context>
This task lands the **three operator questions** rollup and the **week-over-week regression detection**:

- **Q1 (compilation success criterion 1)**: "Did every accepted-Approve patch leave a valid witness chain whose source SHA matches what was perceived?" → for each holdout panel, run Gate 5 (witness chain not broken) AND Gate 6 (source SHA drift) per SPEC-INFERENCE; `q1_pass_rate = (passing_both / total)`.
- **Q2 (compilation success criterion 2)**: "How well does prediction track the oracle?" → `q2_correlation = overall_correlation` (alias from TASK-EVAL-004).
- **Q3 (compilation success criterion 3)**: "Did the verifier predict the right side-effects?" → for each holdout panel where Phase 7 has captured an observed side-effect set, compute `f1(predicted_side_effects, observed_side_effects)`; macro-mean across panels = `q3_side_effect_agreement`. When CF_MEJEPA_LIVE_PREDICTIONS contains zero observed side-effect sets (Phase 7 not shipped), `q3_side_effect_agreement = None`.

**Week-over-week regression detection**: compute `RegressionCheck` (NoPriorReport / Compared{deltas} / CorpusShaChanged) by reading the prior week's `EvalReport` from CF_MEJEPA_EVAL_REPORTS via prefix scan. The actual blocker generation is in TASK-EVAL-008 ship gate; this task only populates the `RegressionCheck` field on `EvalReport`. The CellDelta entries are constructed here (cell, prior, current, delta_pct, is_regression).

**Bayesian shrinkage entry-point**: TASK-EVAL-004 already shipped `compute_bayesian_shrinkage`; this task adds the orchestration that walks `per_cell` entries, applies shrinkage to under-sampled cells, and populates `shrinkage_applied: Vec<String>` for visibility. TASK-EVAL-008 ship gate then SKIPS those cells when checking the per_cell threshold.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (failure_taxonomy MEJEPA_EVAL_SHIP_GATE_REGRESSION; tct_invariants rule 5 Bayesian shrinkage parallel pattern)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-04, REQ-EVAL-11, REQ-EVAL-20; AC-EVAL-03-* regression scenarios; EC-EVAL-03 corpus_sha changed; EC-EVAL-07 shrinkage applied)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (compute_q1_pass_rate, compute_q3_f1, compute_regression_check, apply_bayesian_shrinkage_to_cells)</file>
  <file purpose="design_authority">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §1.3 (the three operator questions)</file>
  <file purpose="prior_task">TASK-EVAL-001..004, TASK-EVAL-008 (regression payload format), TASK-EVAL-011</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-001..004 shipped: EvalReport, EvalConfig, runner with overall_correlation</check>
  <check>TASK-INFER-*** shipped: Gate 5 (`fn gate_witness_chain_broken(&Panel) -> Result<bool, _>`) + Gate 6 (`fn gate_source_sha_drift(&Panel) -> Result<bool, _>`) callable</check>
  <check>TASK-WITNESS-*** shipped: `verify_chain_bytes(&[u8]) -> Result<(), WitnessError>` (already shipped per memory)</check>
  <check>CF_MEJEPA_EVAL_REPORTS open (TASK-EVAL-003)</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/q123.rs` with:
      - `pub fn compute_q1_pass_rate(panels: &[HoldoutPanel], compiler: &MeJepaCompiler) -> Result<f32, EvalError>`:
        1. For each panel, run `let r = compiler.compile(&panel.patch, &panel.context)?`
        2. Run Gate 5: `let g5 = compiler.gate_witness_chain_broken(&r)?`; pass = `!g5.broken`
        3. Run Gate 6: `let g6 = compiler.gate_source_sha_drift(&r, &panel.expected_source_sha)?`; pass = `!g6.drifted`
        4. Increment `pass_count` only when BOTH pass
        5. Return `pass_count as f32 / panels.len() as f32`; reject if panels empty.
      - `pub fn compute_q3_f1(predicted_side_effects: &[SideEffect], observed_side_effects: &[SideEffect]) -> f32`:
        1. Convert both to `BTreeSet<SideEffect>` (deterministic; SideEffect is `Eq + Ord` per SPEC-INSTRUMENTS)
        2. `tp = predicted ∩ observed`; `fp = predicted - observed`; `fn = observed - predicted`
        3. `precision = tp / (tp + fp)`; `recall = tp / (tp + fn)`
        4. `f1 = 2 * precision * recall / (precision + recall)` (return 0.0 when both precision and recall are 0; NOT NaN)
        5. Clamp `[0, 1]`
      - `pub fn compute_q3_macro_mean(panel_pairs: &[(Vec<SideEffect>, Vec<SideEffect>)]) -> Option<f32>`:
        1. If `panel_pairs.is_empty()` → `None` (Phase 7 not shipped)
        2. Compute F1 per panel; clamp finite; macro-average; clamp `[0, 1]`
        3. Return `Some(mean)`
      - **SideEffect**: forward-declare a minimal value-type for tests; in production this re-exports SPEC-INSTRUMENTS / SPEC-CORPUS's canonical type (`#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)] pub struct SideEffect { pub kind: String, pub target: String }` — the canonical shape per doc 09 §1.3 example "test_x failed", "file_y modified")
    - Create `crates/context-graph-mejepa/src/eval/regression.rs` with:
      - `pub fn compute_regression_check(current: &EvalReport, prior: Option<&EvalReport>, weekly_regression_pct: f32) -> RegressionCheck`:
        1. If `prior.is_none()` → `RegressionCheck::NoPriorReport`
        2. If `prior.corpus_sha != current.corpus_sha` → `RegressionCheck::CorpusShaChanged { prior_corpus_sha: prior.corpus_sha, current_corpus_sha: current.corpus_sha }`
        3. Else: walk `current.per_cell` and `prior.per_cell` (both Some); for each cell present in both with Some-Some values, compute `delta_pct = (current_v - prior_v) * 100`; build `CellDelta { cell, prior, current, delta_pct, is_regression: delta_pct &lt; -weekly_regression_pct }`. Sort by `delta_pct` ascending (most-regressed first) for visibility.
        4. Return `RegressionCheck::Compared { prior_date: prior.report_date, deltas }`
      - `pub fn apply_bayesian_shrinkage_to_cells(per_cell: &mut BTreeMap<(MutationCategory, Language), Option<f32>>, language_aggregates: &BTreeMap<Language, f32>, cell_sample_counts: &BTreeMap<(MutationCategory, Language), usize>, language_sample_counts: &BTreeMap<Language, usize>, config: &EvalConfig) -> Vec<String>`:
        1. For each cell with `Some(v)` AND `cell_n &lt; config.min_samples_per_cell` AND `config.enable_bayesian_shrinkage`:
           - Look up `language_aggregate = language_aggregates[&l]` (skip if missing)
           - Look up `language_n = language_sample_counts[&l]` (skip if missing)
           - Compute `shrunk = compute_bayesian_shrinkage(v, cell_n, language_aggregate, language_n)` from `eval::correlation`
           - Replace `per_cell[(c, l)]` with `Some(shrunk)`
           - Push human-readable note to `shrinkage_applied: Vec<String>` like `format!("({:?},{:?}): N={} &lt; {}; shrunk to language-aggregate", c, l, cell_n, config.min_samples_per_cell)`
        2. Return the `shrinkage_applied` vec
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 8 (Q1/Q2/Q3): `report.q1_pass_rate = compute_q1_pass_rate(...)`; `report.q2_correlation = report.overall_correlation`; `report.q3_side_effect_agreement = compute_q3_macro_mean(observed_pairs)?`
      - Wire step 9 (regression): `report.regression_check = compute_regression_check(&report, prior_report.as_ref(), config.thresholds.weekly_regression_pct)`
      - Wire step 10 (Bayesian shrinkage): if `include_per_cell && config.enable_bayesian_shrinkage`, call `apply_bayesian_shrinkage_to_cells(&mut per_cell, ..., &mut shrinkage_applied)` BEFORE `EvalReport::try_new(builder)` so the shrunk per_cell + shrinkage_applied land in the final report
      - All wrapped with `.map_err(|e| { e.log_context("eval::runner.q123"); e })?`
    - Add unit + integration tests in `q123.rs` `#[cfg(test)] mod tests`:
      - `q1_all_pass_returns_1`
      - `q1_half_chain_broken_returns_0_5`
      - `q1_handles_empty_panels_via_error`
      - `q3_f1_perfect_match_returns_1`
      - `q3_f1_no_overlap_returns_0`
      - `q3_f1_partial_overlap_correct_score` (predicted={A,B,C}, observed={B,C,D} → tp=2, fp=1, fn=1 → P=0.667, R=0.667, F1=0.667)
      - `q3_f1_both_empty_returns_zero` (NOT NaN — explicit 0.0 fallback)
      - `q3_macro_returns_none_when_no_pairs`
      - `q3_macro_returns_some_correct_when_pairs_present`
    - Add unit + integration tests in `regression.rs` `#[cfg(test)] mod tests`:
      - `regression_check_no_prior_returns_no_prior_report`
      - `regression_check_corpus_sha_changed_returns_corpus_sha_changed`
      - `regression_check_compared_with_3pct_drop_marks_is_regression_true`
      - `regression_check_compared_with_1pct_drop_marks_is_regression_false`
      - `regression_check_skips_cells_missing_in_one_side`
      - `regression_check_sorts_deltas_ascending` (most regressed first)
      - `bayesian_shrinkage_applied_to_low_n_cell` (cell_n=10, lang_n=80 → weight=0.111 → shrunk towards lang aggregate)
      - `bayesian_shrinkage_skipped_when_disabled` (config.enable_bayesian_shrinkage=false)
      - `bayesian_shrinkage_skipped_when_n_above_min` (cell_n=50 → not shrunk)
      - `bayesian_shrinkage_writes_human_readable_note`
  </in_scope>
  <out_of_scope>
    - SideEffect canonical type body (SPEC-INSTRUMENTS / SPEC-CORPUS owns; this task forward-declares minimal shape)
    - Gate 5/6 implementation (SPEC-INFERENCE owns)
    - check_ship_gate (TASK-EVAL-008 — consumes RegressionCheck this task populates)
    - Weekly report writer (TASK-EVAL-015)
    - MCP handlers (TASK-EVAL-013, TASK-EVAL-014)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/q123.rs">
use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use crate::compiler::MeJepaCompiler;
use super::error::EvalError;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SideEffect {
    pub kind: String,
    pub target: String,
}

pub fn compute_q1_pass_rate(
    panels: &amp;[HoldoutPanel],
    compiler: &amp;MeJepaCompiler,
) -&gt; Result&lt;f32, EvalError&gt;;

pub fn compute_q3_f1(
    predicted_side_effects: &amp;[SideEffect],
    observed_side_effects: &amp;[SideEffect],
) -&gt; f32;

pub fn compute_q3_macro_mean(
    panel_pairs: &amp;[(Vec&lt;SideEffect&gt;, Vec&lt;SideEffect&gt;)],
) -&gt; Option&lt;f32&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa/src/eval/regression.rs">
use std::collections::BTreeMap;
use crate::types::Language;
use super::config::EvalConfig;
use super::report::EvalReport;
use super::types::{CellDelta, MutationCategory, RegressionCheck};

pub fn compute_regression_check(
    current: &amp;EvalReport,
    prior: Option&lt;&amp;EvalReport&gt;,
    weekly_regression_pct: f32,
) -&gt; RegressionCheck;

pub fn apply_bayesian_shrinkage_to_cells(
    per_cell: &amp;mut BTreeMap&lt;(MutationCategory, Language), Option&lt;f32&gt;&gt;,
    language_aggregates: &amp;BTreeMap&lt;Language, f32&gt;,
    cell_sample_counts: &amp;BTreeMap&lt;(MutationCategory, Language), usize&gt;,
    language_sample_counts: &amp;BTreeMap&lt;Language, usize&gt;,
    config: &amp;EvalConfig,
) -&gt; Vec&lt;String&gt;;
    </signature>
  </signatures>
  <constraints>
    - `compute_q3_f1` returns 0.0 (NOT NaN) when both precision and recall are 0 — explicit fallback per doc 09 §1.3 (no side effects predicted AND none observed = trivially correct = 0 F1 score by convention)
    - `compute_q3_macro_mean` returns `None` only when `panel_pairs.is_empty()` (Phase 7 unavailable); never NaN
    - `RegressionCheck::Compared` deltas sorted by `delta_pct` ascending (most-regressed first); deterministic across runs
    - Bayesian shrinkage formula re-uses `compute_bayesian_shrinkage` from TASK-EVAL-004 (no formula divergence)
    - `apply_bayesian_shrinkage_to_cells` MUTATES `per_cell` in-place — caller (runner) keeps the shrunk values in the final report
    - `shrinkage_applied` notes are human-readable strings; format MUST match constraint pattern `({:?},{:?}): N=&lt;n&gt; &lt; &lt;min&gt;; shrunk to language-aggregate` (consumer-stable per AC-EVAL-08-* test assertions)
    - All Err returns wrapped with `.log_context("eval::q123")` or `("eval::regression")`
    - File length budget: each &lt; 500 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - q123: ≥ 9 passing tests
    - regression: ≥ 10 passing tests
    - Q1, Q3 deterministic across runs
    - Regression sorted ascending by delta_pct
    - Shrinkage formula verified to 3 decimal places against Beta-Binomial conjugate analytic ground truth
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/q123.rs:
  pub fn compute_q3_f1(predicted, observed) -&gt; f32 {
      let p_set: BTreeSet&lt;_&gt; = predicted.iter().cloned().collect();
      let o_set: BTreeSet&lt;_&gt; = observed.iter().cloned().collect();
      let tp = p_set.intersection(&o_set).count();
      let fp = p_set.difference(&o_set).count();
      let fn_ = o_set.difference(&p_set).count();
      if tp == 0 && fp == 0 && fn_ == 0 { return 0.0; }
      let precision = if tp + fp == 0 { 0.0 } else { tp as f32 / (tp + fp) as f32 };
      let recall = if tp + fn_ == 0 { 0.0 } else { tp as f32 / (tp + fn_) as f32 };
      if precision == 0.0 && recall == 0.0 { return 0.0; }
      (2.0 * precision * recall / (precision + recall)).clamp(0.0, 1.0)
  }

crates/context-graph-mejepa/src/eval/regression.rs:
  pub fn compute_regression_check(current, prior, weekly_regression_pct) -&gt; RegressionCheck {
      let prior = match prior { Some(p) =&gt; p, None =&gt; return RegressionCheck::NoPriorReport };
      if prior.corpus_sha != current.corpus_sha {
          return RegressionCheck::CorpusShaChanged {
              prior_corpus_sha: prior.corpus_sha,
              current_corpus_sha: current.corpus_sha,
          };
      }
      let mut deltas = Vec::new();
      if let (Some(curr_cells), Some(prior_cells)) = (&current.per_cell, &prior.per_cell) {
          for ((c, l), curr_opt) in curr_cells {
              if let (Some(curr_v), Some(Some(prior_v))) = (curr_opt, prior_cells.get(&(*c, *l))) {
                  let delta_pct = (curr_v - prior_v) * 100.0;
                  deltas.push(CellDelta {
                      cell: (*c, *l),
                      prior: *prior_v,
                      current: *curr_v,
                      delta_pct,
                      is_regression: delta_pct &lt; -weekly_regression_pct,
                  });
              }
          }
      }
      deltas.sort_by(|a, b| a.delta_pct.partial_cmp(&b.delta_pct).unwrap_or(Ordering::Equal));
      RegressionCheck::Compared { prior_date: prior.report_date, deltas }
  }

  pub fn apply_bayesian_shrinkage_to_cells(per_cell, lang_aggs, cell_n, lang_n, config) -&gt; Vec&lt;String&gt; {
      let mut applied = Vec::new();
      if !config.enable_bayesian_shrinkage { return applied; }
      let mut updates = Vec::new();
      for ((c, l), opt) in per_cell.iter() {
          if let Some(v) = opt {
              let cn = match cell_n.get(&(*c, *l)) { Some(n) =&gt; *n, None =&gt; continue };
              if cn &gt;= config.min_samples_per_cell { continue; }
              let lang_agg = match lang_aggs.get(l) { Some(a) =&gt; *a, None =&gt; continue };
              let lan = match lang_n.get(l) { Some(n) =&gt; *n, None =&gt; continue };
              let shrunk = compute_bayesian_shrinkage(*v, cn, lang_agg, lan);
              updates.push((*c, *l, shrunk));
              applied.push(format!("({:?},{:?}): N={} &lt; {}; shrunk to language-aggregate", c, l, cn, config.min_samples_per_cell));
          }
      }
      for (c, l, v) in updates { per_cell.insert((c, l), Some(v)); }
      applied
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/q123.rs">compute_q1_pass_rate + compute_q3_f1 + compute_q3_macro_mean + SideEffect + tests</file>
  <file path="crates/context-graph-mejepa/src/eval/regression.rs">compute_regression_check + apply_bayesian_shrinkage_to_cells + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 8 (Q1/Q2/Q3), step 9 (regression_check), step 10 (Bayesian shrinkage) of run_holdout_eval</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod q123; pub mod regression;` + re-export `SideEffect`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>compute_q1_pass_rate counts panels passing BOTH Gate 5 and Gate 6</criterion>
  <criterion>compute_q3_f1 returns 0.0 (not NaN) when both predicted and observed are empty</criterion>
  <criterion>compute_q3_macro_mean returns None when panel_pairs empty</criterion>
  <criterion>compute_regression_check returns NoPriorReport when prior is None</criterion>
  <criterion>compute_regression_check returns CorpusShaChanged on mismatch</criterion>
  <criterion>compute_regression_check.Compared.deltas sorted ascending by delta_pct</criterion>
  <criterion>CellDelta.is_regression true when delta_pct &lt; -weekly_regression_pct</criterion>
  <criterion>apply_bayesian_shrinkage_to_cells skipped when disabled</criterion>
  <criterion>apply_bayesian_shrinkage_to_cells skipped when cell_n ≥ min_samples_per_cell</criterion>
  <criterion>apply_bayesian_shrinkage_to_cells writes human-readable notes</criterion>
  <criterion>apply_bayesian_shrinkage_to_cells mutates per_cell in-place to shrunk values</criterion>
  <criterion>run_holdout_eval populates EvalReport.q1_pass_rate, .q2_correlation, .q3_side_effect_agreement, .regression_check, .shrinkage_applied</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::q123</command>
  <command>cargo test -p context-graph-mejepa eval::regression</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
