# TASK-EVAL-007 — Per-language Gτ pass rate (read-only consumption of TCT calibrated thresholds)

```xml
<task_spec id="TASK-EVAL-007" version="1.0">

<metadata>
  <title>Implement per-language Gτ pass-rate computation in `crates/context-graph-mejepa/src/eval/gtau_health.rs` (read-only consumer of SPEC-TCT)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-EVAL-02">gtau_pass_rate field per language</item>
    <item ref="REQ-EVAL-10">Per-language stratification of Gτ thresholds (cross-cutting with TCT)</item>
    <item ref="component:EvalRunner.run_holdout_eval step 5c">gtau_pass_rate[l] = fraction Approve</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-004</task_ref>
    <task_ref>TASK-TCT-***</task_ref><!-- ConstellationGuard::gtau_test available -->
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_files>1 created (gtau_health.rs); 1 modified (runner.rs)</estimated_files>
</metadata>

<context>
This task lands the per-language Gτ pass-rate metric — fraction of holdout panels for which
`ConstellationGuard::gtau_test(panel)` returns Approve. **Cross-cutting boundary:** TCT spec hosts
the actual Gτ threshold calibration AND the per-language stratification of those thresholds
(per-cell math: 8 mutation × 13 entity × 11 languages × 21 embedders); this task only READS the
result and rolls it up per language. When SPEC-TCT not yet shipped, `ConstellationGuard` is a
mocked-always-Approve (per phase4-inference _index.md note); this task's pass-rate will be 1.0
across all languages and surface a `provenance_warnings: ["constellation_unavailable"]` flag in
EvalReport (added in TASK-EVAL-015 weekly report writer).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (tct_invariants rule 3 "Gτ test is deterministic O(N · d_max), no learned model"; rule 5 per-cell math)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-02 gtau_pass_rate; REQ-EVAL-10 per-language stratification)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (run_holdout_eval step 5c; ConstellationGuard::gtau_test)</file>
  <file purpose="prior_task">TASK-EVAL-001..004</file>
  <file purpose="cross_domain_authority">docs/ruvectorfindings/specs/functional/SPEC-TCT.md (Gτ threshold calibration owner)</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-004 shipped</check>
  <check>SPEC-TCT (or its Phase 4b mock) exposes `ConstellationGuard::gtau_test(&Panel) -> Result<GtauVerdict, _>` where `GtauVerdict = enum { Approve, Reject{violating_embedders, violating_chunks} }`</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/gtau_health.rs` with:
      - `pub fn compute_gtau_pass_rate_per_language(panels: &[Panel], gtau_checker: &ConstellationGuard, config: &EvalConfig) -> Result<BTreeMap<Language, f32>, EvalError>`
        - For each language `l`:
          1. Filter panels to language `l`
          2. If N < `config.min_samples_per_language`, skip (entry absent from result map)
          3. Count Approve verdicts via `gtau_checker.gtau_test(panel)`
          4. `pass_rate = approve_count / N`; clamp [0, 1]
          5. Insert into result map
        - Return `BTreeMap<Language, f32>`
      - `pub fn gtau_constellation_unavailable_warning(gtau_checker: &ConstellationGuard) -> Option<&'static str>` — returns `Some("constellation_unavailable")` when checker is the MockedAlwaysApprove placeholder; else None. Used by TASK-EVAL-015 to populate `provenance_warnings`.
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 5c of `run_holdout_eval` to call `compute_gtau_pass_rate_per_language` and store in `EvalReportBuilder.gtau_pass_rate`
    - Add unit tests in `gtau_health.rs` `#[cfg(test)] mod tests` for:
      - `gtau_pass_rate_all_approve_returns_1` (mock checker always Approve)
      - `gtau_pass_rate_half_reject_returns_0_5`
      - `gtau_pass_rate_under_min_samples_skips_language`
      - `gtau_pass_rate_returns_btree_with_deterministic_iter`
      - `gtau_constellation_unavailable_warning_returns_some_when_mocked`
  </in_scope>
  <out_of_scope>
    - Gτ threshold calibration (SPEC-TCT owns)
    - Per-cell Gτ math (8 × 13 × 11 × 21 = 24,024 cells per constitution; SPEC-TCT owns)
    - Bayesian shrinkage of Gτ centroids when N < 50 (SPEC-TCT owns)
    - check_ship_gate consumption (TASK-EVAL-008)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/gtau_health.rs">
use std::collections::BTreeMap;
use crate::types::Language;
use super::config::EvalConfig;
use super::error::EvalError;

pub fn compute_gtau_pass_rate_per_language(
    panels: &amp;[Panel],
    gtau_checker: &amp;ConstellationGuard,
    config: &amp;EvalConfig,
) -&gt; Result&lt;BTreeMap&lt;Language, f32&gt;, EvalError&gt;;

pub fn gtau_constellation_unavailable_warning(gtau_checker: &amp;ConstellationGuard) -&gt; Option&lt;&amp;'static str&gt;;
    </signature>
  </signatures>
  <constraints>
    - Strictly read-only (does not mutate constellation, does not write to CF_MEJEPA_CONSTELLATION)
    - Languages with N < min_samples_per_language are absent from result map (NOT 0.0); ship gate later (TASK-EVAL-008) skips absent entries
    - All Pearson-style errors propagate via `.log_context("eval::gtau_health")`
    - File length budget: < 200 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 5 passing tests
    - run_holdout_eval populates EvalReport.gtau_pass_rate
  </verification>
</definition_of_done>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/gtau_health.rs">compute_gtau_pass_rate_per_language + gtau_constellation_unavailable_warning + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 5c</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod gtau_health;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>compute_gtau_pass_rate_per_language returns BTreeMap with deterministic iteration</criterion>
  <criterion>Pass rate clamped [0, 1]</criterion>
  <criterion>Languages under min_samples absent from map</criterion>
  <criterion>gtau_constellation_unavailable_warning returns Some when mock; None when real</criterion>
  <criterion>run_holdout_eval populates EvalReport.gtau_pass_rate</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::gtau_health</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
