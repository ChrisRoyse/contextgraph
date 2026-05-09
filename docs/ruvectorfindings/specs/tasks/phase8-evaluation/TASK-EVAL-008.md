# TASK-EVAL-008 — `check_ship_gate` (8-condition AND + week-over-week regression < 2 %)

```xml
<task_spec id="TASK-EVAL-008" version="1.0">

<metadata>
  <title>Implement `EvalRunner::check_ship_gate` in `crates/context-graph-mejepa/src/eval/ship_gate.rs` (8-condition AND + week-over-week regression detection)</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>8</sequence>
  <implements>
    <item ref="REQ-EVAL-03">Ship gate: 8-condition AND of overall ≥ 0.95, per_category ≥ 0.95, per_language ≥ 0.95, per_cell ≥ 0.90, OOD AUC ≥ 0.85, conformal coverage ±2 %, Q1 ≥ 0.99, Q3 ≥ 0.90</item>
    <item ref="REQ-EVAL-04">Week-over-week regression detection (any cell drops &gt; 2 % triggers MEJEPA_EVAL_SHIP_GATE_REGRESSION)</item>
    <item ref="component:EvalRunner.check_ship_gate">8 conditions short-circuit-AND with structured blocker payload</item>
    <item ref="constitution#performance_budgets">phase8_ship_gate constitution-pinned thresholds</item>
    <item ref="BR-EVAL-02">Failure is visibility, not write-blocking; report is still persisted</item>
    <item ref="BR-EVAL-05">Week-over-week comparison reads last week's report from CF_MEJEPA_EVAL_REPORTS</item>
  </implements>
  <depends_on>
    <task_ref>TASK-EVAL-005</task_ref><!-- conformal_coverage_health populated -->
    <task_ref>TASK-EVAL-006</task_ref><!-- ood_calibration_health populated -->
    <task_ref>TASK-EVAL-007</task_ref><!-- gtau_pass_rate populated -->
    <task_ref>TASK-EVAL-012</task_ref><!-- q1_pass_rate, q2_correlation, q3_side_effect_agreement populated + RegressionCheck construction -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (ship_gate.rs); 1 modified (runner.rs to wire step 15)</estimated_files>
</metadata>

<context>
This task lands the **ship gate** — the canonical Boolean predicate that gates ME-JEPA model promotion. Eight conditions chained by AND with short-circuit semantics; each failure produces a SCREAMING_SNAKE-tagged blocker entry in `ship_gate_failures`. Week-over-week regression detection (REQ-EVAL-04) is implemented in the same module: this week's report is compared cell-by-cell to last week's persisted report (read from CF_MEJEPA_EVAL_REPORTS via prefix scan over the last 7 days of `report_date_iso8601` keys); any cell whose Pearson dropped &gt; 2 % triggers `MEJEPA_EVAL_SHIP_GATE_REGRESSION` with structured payload `{cell, prior, current, delta_pct}`. The Q3 condition is **conditionally skipped** when `q3_side_effect_agreement: None` (Phase 7 not shipped) per TECH-EVALUATION step 9 — documented constitution-pinned exception, NOT a silent fallback. Per BR-EVAL-02, ship-gate failure does NOT prevent report persistence — failures surface to the operator as visibility.
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (performance_budgets phase8_ship_gate verbatim 8 conditions; failure_taxonomy MEJEPA_EVAL_SHIP_GATE_REGRESSION)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-EVALUATION.md (REQ-EVAL-03; REQ-EVAL-04; AC-EVAL-02-* eight failure scenarios; AC-EVAL-03-01 regression scenario)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-EVALUATION.md (check_ship_gate 8-step behavior; step 9 q3 conditional skip; week-over-week comparison via CF_MEJEPA_EVAL_REPORTS prefix scan)</file>
  <file purpose="prior_task">TASK-EVAL-001..007, TASK-EVAL-012</file>
</input_context_files>

<prerequisites>
  <check>TASK-EVAL-005, 006, 007 shipped: report carries `conformal_coverage_health`, `ood_calibration_health`, `gtau_pass_rate`</check>
  <check>TASK-EVAL-012 shipped: report carries `q1_pass_rate`, `q2_correlation`, `q3_side_effect_agreement`, `regression_check`</check>
  <check>CF_MEJEPA_EVAL_REPORTS open via TASK-EVAL-003 storage</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `crates/context-graph-mejepa/src/eval/ship_gate.rs` with:
      - `pub fn check_ship_gate(report: &EvalReport, prior_report: Option<&EvalReport>, thresholds: &ShipGateThresholds) -> ShipGateStatus` per TECH-EVALUATION 8-step behavior:
        1. **Condition 1 — overall**: `report.overall_correlation >= thresholds.overall_min` (default 0.95). If fail, push `MEJEPA_EVAL_SHIP_GATE_OVERALL: 0.93 < 0.95` to blockers.
        2. **Condition 2 — per_category**: every entry of `report.per_category` `>= thresholds.per_category_min` (default 0.95). If any fail, push `MEJEPA_EVAL_SHIP_GATE_PER_CATEGORY: subtle_flip 0.92 < 0.95` per failing cell.
        3. **Condition 3 — per_language**: every Some-entry of `report.per_language` `>= thresholds.per_language_min` (default 0.95). None entries SKIP (under-sampled, not failing); push `MEJEPA_EVAL_SHIP_GATE_PER_LANGUAGE: rust 0.85 < 0.95` per failing language.
        4. **Condition 4 — per_cell**: every Some-entry of `report.per_cell` `>= thresholds.per_cell_min` (default 0.90). Cells whose name appears in `report.shrinkage_applied` are SKIPPED per REQ-EVAL-20 (don't penalize shrunk cells); None entries SKIP. Push `MEJEPA_EVAL_SHIP_GATE_PER_CELL: (over_engineer, java) 0.87 < 0.90` per failing cell.
        5. **Condition 5 — OOD AUC**: every entry of `report.ood_calibration_health` `>= thresholds.ood_auc_min` (default 0.85). Push `MEJEPA_EVAL_SHIP_GATE_OOD_AUC: c 0.78 < 0.85` per failing language.
        6. **Condition 6 — conformal coverage**: every entry of `report.conformal_coverage_health` has `passes_band == true`. Push `MEJEPA_EVAL_SHIP_GATE_CONFORMAL_COVERAGE: ruby 0.84 outside [0.88, 0.92]` per failing language.
        7. **Condition 7 — Q1**: `report.q1_pass_rate >= thresholds.q1_min` (default 0.99). Push `MEJEPA_EVAL_SHIP_GATE_Q1: 0.97 < 0.99` if fail.
        8. **Condition 8 — Q3** (conditional): if `report.q3_side_effect_agreement` is `Some(v)`, require `v >= thresholds.q3_min` (default 0.90). Push `MEJEPA_EVAL_SHIP_GATE_Q3: 0.87 < 0.90` if fail. If `None`, log `INFO: Q3 condition skipped (Phase 7 not shipped)` and do NOT add a blocker.
        9. **Week-over-week regression** (orthogonal to the 8-AND; produces additional blockers): when `prior_report.is_some()` AND `prior_report.corpus_sha == report.corpus_sha` (graphs are comparable only when corpus didn't change — TECH-EVALUATION rationale), iterate over each cell in `report.per_cell`; for each cell present in BOTH this and prior with Some-correlations, compute `delta_pct = (current - prior) * 100`; if `delta_pct < -thresholds.weekly_regression_pct` (default -2.0), push `MEJEPA_EVAL_SHIP_GATE_REGRESSION: (subtle_flip, typescript) 0.94 → 0.91 (delta -3.0%)`. When `prior_report.corpus_sha != report.corpus_sha`, do NOT compare cells — the corpus changed, comparison is meaningless; surface this via `report.regression_check = RegressionCheck::CorpusShaChanged` (already populated by TASK-EVAL-012); ship gate ignores this case (not a regression failure, just an informational note).
        10. **Aggregate verdict**: `passed = blockers.is_empty()` (all 8 conditions met AND no week-over-week regression).
      - Construct `ShipGateStatus { passed, per_category: report.per_category.clone(), per_language: report.per_language.clone(), blockers, report_date: report.report_date, overall_correlation: report.overall_correlation }` and return.
    - Update `crates/context-graph-mejepa/src/eval/runner.rs`:
      - Wire step 15 of `run_holdout_eval` to load prior report via `load_prior_report(&self.storage, report.report_date - chrono::Duration::days(7))?` (helper added here, not in TASK-EVAL-006/012)
      - Call `check_ship_gate(&report, prior_report.as_ref(), &self.config.thresholds)` and store the resulting `(passed, blockers)` into `EvalReportBuilder.ship_gate_passed` and `.ship_gate_failures`
      - The mutable `EvalReport` is then re-wrapped via `EvalReport::try_new(builder)` — this is allowed because `ship_gate_passed` and `ship_gate_failures` are part of the builder.
    - Add `pub fn load_prior_report(storage: &RocksDbStore, target_date: NaiveDate) -> Result<Option<EvalReport>, EvalError>` to `ship_gate.rs` (or `storage.rs` — keep here since it's ship-gate-specific). Prefix-scan CF_MEJEPA_EVAL_REPORTS by date; return the LATEST report whose `report_date <= target_date`. None when no eligible prior exists.
    - Add unit + integration tests in `ship_gate.rs` `#[cfg(test)] mod tests` for:
      - `ship_gate_all_pass_returns_passed_true`
      - `ship_gate_overall_below_threshold_blocks` (push 1 blocker, passed=false)
      - `ship_gate_per_category_below_threshold_blocks_per_failing_category`
      - `ship_gate_per_language_below_threshold_blocks_per_failing_language` (covers AC-EVAL-02-03 rust 0.85)
      - `ship_gate_per_cell_below_threshold_blocks_per_failing_cell` (covers AC-EVAL-02-06 (over_engineer, java) 0.87)
      - `ship_gate_per_cell_skips_shrunk_cells` (cell in `shrinkage_applied` does NOT block even when value is 0.85)
      - `ship_gate_ood_auc_below_threshold_blocks` (covers AC-EVAL-02-04 c 0.78)
      - `ship_gate_conformal_band_fails_blocks` (covers AC-EVAL-02-05 ruby 0.84)
      - `ship_gate_q1_below_threshold_blocks`
      - `ship_gate_q3_none_skips_condition` (q3_side_effect_agreement=None; passed=true if other 7 pass)
      - `ship_gate_q3_below_threshold_blocks_when_some` (q3=0.87 < 0.90 → blocker)
      - `regression_within_2pct_does_not_block` (delta -1.5%)
      - `regression_over_2pct_blocks_with_payload` (covers AC-EVAL-03-01: typescript subtle_flip 0.94 → 0.91 = -3.0% → blocker)
      - `regression_skipped_when_corpus_sha_changed`
      - `load_prior_report_returns_latest_within_window`
      - `load_prior_report_returns_none_when_no_history`
  </in_scope>
  <out_of_scope>
    - Q1/Q2/Q3 computation (TASK-EVAL-012)
    - Regression-payload `CellDelta` data model (TASK-EVAL-001 already shipped)
    - SELF-HEALING consumption of `ship_gate_failures` (Phase 5)
    - MCP tool surface (TASK-EVAL-013)
    - Weekly report writer (TASK-EVAL-015)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/eval/ship_gate.rs">
use chrono::NaiveDate;
use crate::types::Language;
use super::error::EvalError;
use super::report::EvalReport;
use super::types::{ShipGateStatus, ShipGateThresholds};

pub fn check_ship_gate(
    report: &amp;EvalReport,
    prior_report: Option&lt;&amp;EvalReport&gt;,
    thresholds: &amp;ShipGateThresholds,
) -&gt; ShipGateStatus;

pub fn load_prior_report(
    storage: &amp;crate::eval::storage::RocksDbStore,
    target_date: NaiveDate,
) -&gt; Result&lt;Option&lt;EvalReport&gt;, EvalError&gt;;
    </signature>
  </signatures>
  <constraints>
    - Short-circuit AND is enforced ONLY for the aggregate verdict; ALL eight conditions are evaluated to populate the full blocker list (operator wants every failure visible at once, not just the first)
    - Q3 conditional skip MUST log at `INFO` (not `WARN`) — informational, not a regression
    - Week-over-week regression compares `report.per_cell[(c,l)]` SOME values only; None on either side skips
    - `prior_report.corpus_sha != report.corpus_sha` → no regression check; ship gate's 8 conditions still run normally
    - Threshold comparisons use `&gt;=` (boundary inclusive) per constitution; e.g., 0.95 EXACT passes, 0.949999 fails
    - `f32` comparisons MUST handle NaN: any `is_nan()` value in a checked field → push `MEJEPA_EVAL_SHIP_GATE_NAN_DETECTED:&lt;field&gt;` blocker (defensive — TASK-EVAL-001 try_new should already reject NaN, but ship gate is the last line)
    - All blocker strings use SCREAMING_SNAKE prefix `MEJEPA_EVAL_SHIP_GATE_*` per `&lt;mcp_tool_namespace_lock&gt;` derived patterns
    - File length budget: &lt; 500 lines
  </constraints>
  <verification>
    - `cargo build/test/clippy` clean
    - ≥ 16 passing tests
    - `passed=true` on all-good fixture
    - Each of the 8 conditions independently testable
    - Regression detection produces structured payload matching AC-EVAL-03-01 verbatim
  </verification>
</definition_of_done>

<pseudo_code>
crates/context-graph-mejepa/src/eval/ship_gate.rs:
  pub fn check_ship_gate(report, prior, thresholds) -&gt; ShipGateStatus {
      let mut blockers = Vec::new();

      // 1. overall
      if !(report.overall_correlation &gt;= thresholds.overall_min) {
          blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_OVERALL: {} &lt; {}",
              report.overall_correlation, thresholds.overall_min));
      }
      // 2. per_category
      for (c, v) in &report.per_category {
          if !(v &gt;= &amp;thresholds.per_category_min) {
              blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_PER_CATEGORY: {:?} {} &lt; {}",
                  c, v, thresholds.per_category_min));
          }
      }
      // 3. per_language (skip None)
      for (l, opt) in &report.per_language {
          if let Some(v) = opt {
              if !(v &gt;= &amp;thresholds.per_language_min) {
                  blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_PER_LANGUAGE: {:?} {} &lt; {}",
                      l, v, thresholds.per_language_min));
              }
          }
      }
      // 4. per_cell (skip shrunk + None)
      if let Some(cell_map) = &report.per_cell {
          for ((c, l), opt) in cell_map {
              if let Some(v) = opt {
                  let cell_str = format!("({:?},{:?})", c, l);
                  if report.shrinkage_applied.iter().any(|s| s.contains(&cell_str)) { continue; }
                  if !(v &gt;= &amp;thresholds.per_cell_min) {
                      blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_PER_CELL: {} {} &lt; {}",
                          cell_str, v, thresholds.per_cell_min));
                  }
              }
          }
      }
      // 5. OOD AUC
      for (l, v) in &report.ood_calibration_health {
          if !(v &gt;= &amp;thresholds.ood_auc_min) {
              blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_OOD_AUC: {:?} {} &lt; {}",
                  l, v, thresholds.ood_auc_min));
          }
      }
      // 6. conformal coverage band
      for (l, entry) in &report.conformal_coverage_health {
          if !entry.passes_band {
              blockers.push(format!(
                  "MEJEPA_EVAL_SHIP_GATE_CONFORMAL_COVERAGE: {:?} {} delta_pct={}",
                  l, entry.empirical, entry.delta_pct));
          }
      }
      // 7. Q1
      if !(report.q1_pass_rate &gt;= thresholds.q1_min) {
          blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_Q1: {} &lt; {}",
              report.q1_pass_rate, thresholds.q1_min));
      }
      // 8. Q3 (conditional)
      if let Some(q3) = report.q3_side_effect_agreement {
          if !(q3 &gt;= thresholds.q3_min) {
              blockers.push(format!("MEJEPA_EVAL_SHIP_GATE_Q3: {} &lt; {}", q3, thresholds.q3_min));
          }
      } else {
          tracing::info!(target: "eval::ship_gate", "Q3 condition skipped (Phase 7 not shipped)");
      }
      // 9. week-over-week regression
      if let Some(prior) = prior_report {
          if prior.corpus_sha == report.corpus_sha {
              if let (Some(curr_cells), Some(prior_cells)) = (&report.per_cell, &prior.per_cell) {
                  for ((c, l), curr_opt) in curr_cells {
                      if let (Some(curr), Some(Some(prior_v))) = (curr_opt, prior_cells.get(&(*c, *l))) {
                          let delta_pct = (curr - prior_v) * 100.0;
                          if delta_pct &lt; -thresholds.weekly_regression_pct {
                              blockers.push(format!(
                                  "MEJEPA_EVAL_SHIP_GATE_REGRESSION: ({:?},{:?}) {} → {} (delta {:.1}%)",
                                  c, l, prior_v, curr, delta_pct));
                          }
                      }
                  }
              }
          }
      }

      ShipGateStatus {
          passed: blockers.is_empty(),
          per_category: report.per_category.clone(),
          per_language: report.per_language.clone(),
          blockers,
          report_date: report.report_date,
          overall_correlation: report.overall_correlation,
      }
  }
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/eval/ship_gate.rs">check_ship_gate + load_prior_report + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa/src/eval/runner.rs">wire step 15 of run_holdout_eval; load prior report via load_prior_report; populate ship_gate_passed + ship_gate_failures into builder</file>
  <file path="crates/context-graph-mejepa/src/eval/mod.rs">declare `pub mod ship_gate;`</file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build / test / clippy clean</criterion>
  <criterion>ship_gate_all_pass_returns_passed_true</criterion>
  <criterion>Each of 8 conditions independently fails on synthesized input → expected SCREAMING_SNAKE blocker</criterion>
  <criterion>Q3 None skips condition; Q3 Some(0.87) blocks</criterion>
  <criterion>Cells in shrinkage_applied are NOT counted as ship-gate failures</criterion>
  <criterion>Regression detects -3.0% delta and pushes MEJEPA_EVAL_SHIP_GATE_REGRESSION</criterion>
  <criterion>Regression skipped when corpus_sha differs</criterion>
  <criterion>load_prior_report returns latest within 7-day window from CF_MEJEPA_EVAL_REPORTS</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa eval::ship_gate</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
