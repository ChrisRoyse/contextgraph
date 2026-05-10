# SPEC-EVALUATION — ME-JEPA-Code Phase 8 Holdout Evaluation, Ship Gate, Active Learning

> **📊 2026-05-10 STATUS:** PARTIAL — see ../implementation-plan.md for the full Phase 8 status.

> **Level 2 — Functional Specification.** Defines WHAT the evaluation domain (Phase 8) must do, not HOW. Implementation details live in `TECH-EVALUATION.md`. Conflicts with the constitution resolve to the constitution.

```xml
<functional_spec id="SPEC-EVALUATION" version="1.0">

<metadata>
  <title>ME-JEPA-Code Phase 8 Holdout Evaluation, ≥0.95 Ship Gate, Active Learning, Cross-Task Transfer, Weekly Report</title>
  <status>partial</status>
  <owner>Phase 8 ME-JEPA implementation team (Claude Code outer optimizer)</owner>
  <created_date>2026-05-09</created_date>
  <last_updated>2026-05-10</last_updated>
  <related_specs>
    <spec_ref>SPEC-CORPUS</spec_ref>            <!-- Phase 0 holdout split + per-language stratification -->
    <spec_ref>SPEC-INSTRUMENTS</spec_ref>       <!-- Phase 1 panel substrate (E_Oracle ground truth) -->
    <spec_ref>SPEC-AST-CHUNKING</spec_ref>      <!-- 11-language chunker -->
    <spec_ref>SPEC-EMBEDDERS</spec_ref>         <!-- 21-embedder substrate -->
    <spec_ref>SPEC-PREDICTOR</spec_ref>         <!-- Phase 2 predictor (compile() under evaluation) -->
    <spec_ref>SPEC-TRAINING</spec_ref>          <!-- Phase 3 training loop produces checkpoints -->
    <spec_ref>SPEC-INFERENCE</spec_ref>         <!-- Phase 4a compile()/verify() — eval consumes both -->
    <spec_ref>SPEC-TCT</spec_ref>               <!-- Phase 4b Gτ guard — eval reads pass-rate per-language -->
    <spec_ref>SPEC-SELF-HEALING</spec_ref>      <!-- Phase 5 drift / Mode B/C — eval surfaces promotion history -->
    <spec_ref>SPEC-HYGIENE</spec_ref>           <!-- Phase 6 — eval reports persist via tier compression -->
    <spec_ref>SPEC-SHIFT-SUBSCRIBER</spec_ref>  <!-- Phase 7 §3.8 — eval consumes live (predicted, actual) pairs -->
    <spec_ref>SPEC-UTML</spec_ref>              <!-- ΔΩ × ΔΞ degraded_status accounting -->
    <spec_ref>SPEC-WITNESS</spec_ref>           <!-- chain offset pinning per evaluation report -->
  </related_specs>
  <source_documents>
    <source type="design">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §1.3 (compilation success criteria — three operator questions), §3.4 (holdout split — Stoer-Wagner mincut, no shared-task leakage), §3.7.4 (active learning loop — uncertain + in-distribution surface), §3.7.5 (cross-task transfer — patch-similarity graph), §3.7.6 (aux-head distillation), §7 Phase 8 (verbatim end-goal metric: prediction_oracle_correlation), §8 (open research questions 1-8)</source>
    <source type="design">docs/ruvectorfindings/10_MEJEPA_PIVOT_2026_05_09.md Section C (cross-language mandate — per-language stratification of conformal + OOD + Gτ), Section F (priorities — cross-language calibration + ship gate first-class)</source>
    <source type="design">docs/ruvectorfindings/11_MEJEPA_LEARNING_FORMULA.md §8 (state-transfer T(train, deploy) = exp(−d(P_train, P_deploy) / λ); performance_deploy = M(t) · T(train, deploy))</source>
    <source type="design">docs/ruvectorfindings/13_TELEOLOGICAL_CONSTELLATION_TRAINING_MEJEPA.md §3 (Gτ pass rate per (mutation_category × entity_type × language) cell — surfaced in eval report)</source>
    <source type="constitution">docs/ruvectorfindings/specs/constitution.md (performance_budgets phase8_ship_gate: prediction-oracle correlation ≥ 0.95 stably across all 8 mutation categories AND all supported languages on rolling-window holdout; utml_v1_invariants: degraded_status feeds into evaluation report; failure_taxonomy MEJEPA_RETRAIN_HOLDOUT_REGRESSION; mission §three operator questions)</source>
    <source type="memory">memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md (clean-room reimplementation; weekly report writer ships in our crates)</source>
    <source type="memory">memory/feedback_no_cost_caps.md (no cost / budget caps in evaluation; latency budget enforcement only)</source>
  </source_documents>
</metadata>

<overview>
The evaluation domain is the answer to "are we done?" Phase 8 is **ongoing**: it runs continuously
once Phase 7 (shift-subscriber) ships durable `(predicted, actual)` pairs. It produces a single
canonical artifact each week — `mejepa-weekly-report-YYYY-MM-DD.json` — that surfaces to the
operator the prediction-oracle correlation cell-by-cell across **all 8 mutation categories** and
**all supported languages** on the holdout split, plus a parallel slice on **live reality-loop
attempts** captured by SPEC-SHIFT-SUBSCRIBER. The end-goal ship gate per constitution
`<performance_budgets>` is explicit: correlation **stably ≥ 0.95 across every cell** AND **across
all supported languages** AND **on live reality-loop attempts**. When that gate stably passes,
human code review of AI-generated patches becomes statistically unjustified — that is the
contractual definition of "done" per doc 09 §1.3 and §7 Phase 8.

Three statistical machineries underlie the evaluation. (1) **Holdout split eval** computes
correlation per (category × language) cell on the 10 % holdout split that SPEC-CORPUS produces via
Stoer-Wagner mincut (no shared-task leakage between repos). (2) **Per-language stratified
calibration health** verifies that the conformal threshold τ produced by SPEC-INFERENCE has
empirical coverage within ±2 % of stated 1−α **per language**, that OOD score distributions
remain well-separated **per language**, and that Gτ pass rates from SPEC-TCT stay above their
language-specific calibrated thresholds. (3) **State-transfer T** per doc 11 §8 — `T(train,
deploy) = exp(−d(P_train, P_deploy) / λ)`, `performance_deploy = M(t) · T(train, deploy)` — is
computed as a diagnostic so a low ship-gate metric can be attributed: training problem (M(t) low)
versus distribution-shift problem (T low).

The **active learning loop** (per doc 09 §3.7.4) uses evaluation-derived uncertainty signals to
pick high-value training examples. When ME-JEPA's `confidence.outcome_set.len() > 3` AND
`out_of_distribution_score < 0.5` (uncertain BUT in-distribution — exactly the cells where extra
supervision moves the needle), the example is enqueued for one of three labelling paths:
(a) operator manual label, (b) Docker oracle re-run with mutation variants, (c) adjacent-test
re-runs. Per-week budget: ≥ 1 % of corpus added as gold supervision. **Cross-task transfer**
(per doc 09 §3.7.5) builds a patch-similarity graph (AST-diff cosine over corpus); the mini-batch
sampler in SPEC-TRAINING reads this graph to upweight similar tasks during fine-tuning. This spec
specifies the graph construction + the active-learning queue management; SPEC-TRAINING owns the
sampler integration.

Beneficiaries: (1) the operator reads `mejepa-weekly-report-YYYY-MM-DD.json` to make ship/no-ship
decisions; (2) the Phase 5 self-healing loop reads `eval_report.ship_gate_failures` to trigger
A/B/C retrain when a specific cell regresses; (3) Phase 1+ implementers use the `per_cell` matrix
to identify language/category combinations that need additional instrument coverage; (4) external
agents read `mcp__cgreality__mejepa_eval_ship_gate_status()` to decide whether to call
`mejepa_verify` (gate passed) or escalate immediately to human (gate failed). The report is the
**single source of truth** for "is ME-JEPA-Code done?" — every claim about predictive reliability
must reduce to a row in this report.

The three-operator-question check (per constitution mission and per doc 09 §1.3) is integrated.
**Q1 ("does what the AI claimed it did EXIST")** is answered by the SPEC-INFERENCE source-SHA-drift
gate + SPEC-WITNESS chain integrity, surfaced in the eval report as `q1_pass_rate`. **Q2 ("does
it WORK")** is the primary phase-8 metric: `prediction_oracle_correlation`. **Q3 ("how is it
actually IMPACTING REALITY")** is answered by the predicted side-effect set ↔ observed shifts
agreement, surfaced as `q3_side_effect_agreement`. All three roll up into the ship-gate decision.
</overview>

<user_types>

  <user_type id="UT-01" name="Operator (human reviewer + ship-gate signer)">
    <description>The human running the system. Reads the weekly evaluation report, makes ship/no-ship decisions for ME-JEPA model promotions, signs off on ≥0.95 ship-gate transitions, decides when active-learning queue items go for manual labeling vs Docker re-run vs adjacent-test re-run.</description>
    <permissions>READ all eval-report fields, per-cell metrics, ship-gate status, active-learning queue; APPROVE ship-gate transitions (e.g., 0.94 → 0.95 → "review obsolete" production status); LABEL queued active-learning items via `mcp__cgreality__mejepa_active_learning_label(panel_id, oracle_outcome)`; cannot silently mutate the holdout split or the cell-correlation thresholds.</permissions>
    <goals>Confidently sign off on the ship-gate transition. Receive cell-level visibility into where the model fails so the next training cycle is targeted. Per doc 09 §1.3: when correlation stably exceeds 0.95 across ALL cells, declare ME-JEPA-Code production-grade.</goals>
  </user_type>

  <user_type id="UT-02" name="Phase 5 Self-Healing Loop">
    <description>The drift-detection + A/B/C retrain machinery. Reads weekly eval reports (in addition to the 1,000-attempt rolling window) to detect cell-level regression and trigger retrain modes.</description>
    <permissions>READ `eval_report.ship_gate_failures`, `per_cell`, `drift_severity_history`, `model_promotion_history`; cannot directly write to the report (writes go through `EvalRunner::run_holdout_eval`).</permissions>
    <goals>When a specific cell regresses > 2 % week-over-week (`MEJEPA_EVAL_SHIP_GATE_REGRESSION`), trigger Mode B / Mode C retrain on that cell's training stratum.</goals>
  </user_type>

  <user_type id="UT-03" name="External AI agent (calibration consumer)">
    <description>Any AI agent (Claude Code in the outer reality loop, an external coding agent over MCP) that wants to know whether the model is currently shippable before relying on `mejepa_verify`.</description>
    <permissions>READ-only over MCP: `mcp__cgreality__mejepa_eval_ship_gate_status() -> ShipGateStatus { passed: bool, per_category, per_language, blockers: [...] }`; cannot trigger eval runs.</permissions>
    <goals>Decide whether to call `mejepa_verify` and trust an Approve verdict (gate passed for the input's language + likely-mutation-category) or pre-escalate to human (gate failed for the relevant cell).</goals>
  </user_type>

  <user_type id="UT-04" name="Phase 7 mejepa-shift-subscriber">
    <description>The Tokio task that materializes panels from shift-log entries and writes `RealityPrediction` records to CF_MEJEPA_LIVE_PREDICTIONS. Phase 8 reads from this CF to compute live-attempt correlation as a parallel slice to the holdout.</description>
    <permissions>WRITE `RealityPrediction` to CF_MEJEPA_LIVE_PREDICTIONS (own surface); WRITE corresponding `OracleOutcome` to CF_MEJEPA_ORACLE_VERDICTS once tests run; eval consumes these read-only.</permissions>
    <goals>Provide a steady stream of live (predicted, actual) pairs so the weekly report has a live slice in addition to the static holdout.</goals>
  </user_type>

  <user_type id="UT-05" name="Phase 3 SPEC-TRAINING (mini-batch sampler)">
    <description>The training loop's mini-batch sampler reads the patch-similarity graph constructed here to upweight similar tasks (per doc 09 §3.7.5 cross-task transfer).</description>
    <permissions>READ `mejepa-task-graph` edges from `CF_MEJEPA_TASK_GRAPH`; cannot mutate graph (graph rebuilds are an evaluation-domain operation).</permissions>
    <goals>When training on task T, with prob 0.3 sample from tasks within cosine > 0.7 of T's patch (per doc 09 §3.7.5).</goals>
  </user_type>

  <user_type id="UT-06" name="Phase 1+ instrument implementers (Claude Code, future contributors)">
    <description>Anyone shipping new instruments, embedder upgrades, or AST chunker extensions reads the per-cell matrix to identify cells that need additional support.</description>
    <permissions>READ `eval_report.per_cell` and `per_language` matrices; READ the active-learning queue's `most_uncertain_per_language` slice to identify high-leverage targets.</permissions>
    <goals>Direct effort to the cells with the lowest correlation — e.g., if `(compile_error, c)` is at 0.82 while `(known_good, python)` is at 0.97, the next instrument-extension turn targets C-language compile-error coverage.</goals>
  </user_type>

</user_types>

<user_stories>

  <story id="US-EVAL-01" priority="must-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>Read a weekly `mejepa-weekly-report-YYYY-MM-DD.json` that reports per-(category × language) cell correlation, ship-gate pass/fail, OOD calibration health per language, conformal coverage health per language, Gτ pass rate, active-learning queue depth, drift severity history, and model promotion history</i_want_to>
      <so_that>I can make ship/no-ship decisions cell-by-cell and identify exactly where additional training data or instrument coverage is needed</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-01-01">
        <given>Phase 7 shift-subscriber has written ≥ 100 `RealityPrediction` records to CF_MEJEPA_LIVE_PREDICTIONS during the rolling window AND the holdout split (10 % of Phase 0 corpus, repo-atomic split per SPEC-CORPUS) is materialized on disk</given>
        <when>The weekly scheduler invokes `EvalRunner::run_holdout_eval(window_days: 7, include_per_cell: true)`</when>
        <then>A single `EvalReport` JSON is written to `data/mejepa-eval/mejepa-weekly-report-YYYY-MM-DD.json` carrying: `report_date`, `rolling_window_size`, `overall_correlation`, `per_category` (8 entries), `per_language` (9 supported entries; 2 future-expansion language entries marked `null` until corpus catches up), `per_cell` (8 × 9 = 72 entries), `ship_gate_passed: bool`, `ship_gate_failures: Vec<String>`, `ood_calibration_health` per language, `conformal_coverage_health` per language (stated vs empirical), `gtau_pass_rate` per language, `active_learning_queue` (depth), `drift_severity_history` (last 4 weeks), `model_promotion_history` (rolling 12 months); FSV evidence at /tmp/contextgraph-mejepa-evaluation-fsv/{holdout-eval-evidence.json, weekly-report.json}</then>
      </criterion>
      <criterion id="AC-EVAL-01-02">
        <given>The current week's `overall_correlation = 0.92` AND `per_cell[(compile_error, c)] = 0.78`</given>
        <when>The eval runner evaluates the ship gate</when>
        <then>`ship_gate_passed: false`; `ship_gate_failures: ["c language correlation 0.78 < 0.90 cell threshold", "compile_error category overall 0.84 < 0.95 ship threshold"]`; per `<failure_taxonomy>` the fail-closed code is `MEJEPA_EVAL_SHIP_GATE_REGRESSION` if the cell dropped > 2 % week-over-week; the report is still persisted (failures are visibility, not write-blocking)</then>
      </criterion>
      <criterion id="AC-EVAL-01-03">
        <given>The current week's `overall_correlation = 0.96` AND every cell ≥ 0.95 AND ood_calibration_health each language ≥ 0.85 AUC AND conformal coverage each language within ±2 % of stated</given>
        <when>The eval runner evaluates the ship gate</when>
        <then>`ship_gate_passed: true`; `ship_gate_failures: []`; the report carries an explicit `q1_pass_rate, q2_pass_rate, q3_side_effect_agreement` rollup of the three-operator-question metrics; the operator may call `mcp__cgreality__mejepa_eval_ship_gate_status()` and receive `passed: true`</then>
      </criterion>
      <criterion id="AC-EVAL-01-04">
        <given>The holdout split has fewer than the minimum required samples per (category × language) cell (default 30 per cell)</given>
        <when>The eval runner evaluates that cell</when>
        <then>The cell's `per_cell` entry is `null` with a parallel `per_cell_warnings: ["(compile_error, ruby): MEJEPA_EVAL_INSUFFICIENT_SAMPLES required=30 observed=12"]`; ship-gate ignores `null` cells but the report still reports them; do not silently substitute language-aggregate (substitution is allowed only when explicitly enabled per AC-EVAL-08-02)</then>
      </criterion>
      <criterion id="AC-EVAL-01-05">
        <given>The same eval is run twice within the same week</given>
        <when>Both runs complete</when>
        <then>Both reports are persisted (with distinct timestamps); the most recent report is the canonical `mejepa-weekly-report-YYYY-MM-DD.json` the operator reads; older within-week reports are durable for replay but do not appear in the public path</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-02" priority="must-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>The ship-gate metric to enforce **all** of: per-category ≥ 0.95 (8 categories), per-language ≥ 0.95 (across all 9 supported languages), per-cell ≥ 0.90, OOD calibration health AUC ≥ 0.85 per language, conformal empirical coverage within ±2 % of stated per language</i_want_to>
      <so_that>The "≥0.95 ship gate" is unambiguous and cannot be silently relaxed by a single failing language masquerading behind a passing aggregate</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-02-01">
        <given>`per_language[python] = 0.97` AND `per_language[rust] = 0.85` AND every other per-category ≥ 0.95</given>
        <when>The ship gate is evaluated</when>
        <then>`ship_gate_passed: false`; `ship_gate_failures: ["rust language correlation 0.85 < 0.95 ship threshold"]`; the per-category aggregate could not save the per-language fail (Python-bias mitigation per doc 10 §C)</then>
      </criterion>
      <criterion id="AC-EVAL-02-02">
        <given>`per_category[compile_error] = 0.91` AND every other per-category ≥ 0.95 AND every per-language ≥ 0.95</given>
        <when>The ship gate is evaluated</when>
        <then>`ship_gate_passed: false`; `ship_gate_failures: ["compile_error category correlation 0.91 < 0.95 ship threshold"]`</then>
      </criterion>
      <criterion id="AC-EVAL-02-03">
        <given>Every per-category, per-language correlation ≥ 0.95 BUT `per_cell[(over_engineer, java)] = 0.87`</given>
        <when>The ship gate is evaluated</when>
        <then>`ship_gate_passed: false`; `ship_gate_failures: ["(over_engineer, java) cell correlation 0.87 < 0.90 cell threshold"]`; the per-cell threshold is 0.90 (looser than the 0.95 aggregate threshold) because cells have smaller sample size and higher variance</then>
      </criterion>
      <criterion id="AC-EVAL-02-04">
        <given>Per-language and per-category and per-cell all pass BUT `ood_calibration_health[c] = 0.78 AUC`</given>
        <when>The ship gate is evaluated</when>
        <then>`ship_gate_passed: false`; `ship_gate_failures: ["c language ood_calibration_health 0.78 AUC < 0.85 ship threshold"]`</then>
      </criterion>
      <criterion id="AC-EVAL-02-05">
        <given>All correlations + OOD pass BUT `conformal_coverage_health[ruby]` reports `stated 0.90, empirical 0.84` (off by 6 %)</given>
        <when>The ship gate is evaluated</when>
        <then>`ship_gate_passed: false`; `ship_gate_failures: ["ruby language conformal coverage 0.84 outside [0.88, 0.92] ±2% band"]`</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-03" priority="must-have">
    <narrative>
      <as_a>Phase 5 self-healing loop (UT-02)</as_a>
      <i_want_to>Receive `MEJEPA_EVAL_SHIP_GATE_REGRESSION` when a specific cell drops > 2 % week-over-week so I can trigger Mode B / Mode C retrain targeted at that cell's training stratum</i_want_to>
      <so_that>Cell-level regressions trigger surgical fixes rather than blanket retrains</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-03-01">
        <given>Last week's `per_cell[(subtle_flip, typescript)] = 0.94` AND this week's `per_cell[(subtle_flip, typescript)] = 0.91`</given>
        <when>The ship gate is evaluated this week</when>
        <then>`ship_gate_failures` includes `MEJEPA_EVAL_SHIP_GATE_REGRESSION: (subtle_flip, typescript) dropped 0.94 → 0.91 (delta -3.0%) > 2% week-over-week threshold`; SPEC-SELF-HEALING reads this and queues a targeted Mode B retrain on the typescript subtle_flip stratum</then>
      </criterion>
      <criterion id="AC-EVAL-03-02">
        <given>This week's report contains a cell newly above 0.95 for the first time</given>
        <when>The eval completes</when>
        <then>`model_promotion_history` records `{ date: ..., cell: "(subtle_flip, typescript)", crossed_to_above_0.95: true, weights_sha: ..., calibration_version: ... }`; this is the audit trail of "ME-JEPA crossed the bar for this cell"</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-04" priority="must-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>Surface uncertain-but-in-distribution panels (`outcome_set.len() > 3` AND `out_of_distribution_score < 0.5`) into an active-learning queue I can label manually, send to Docker oracle re-run, or send to adjacent-test re-runs</i_want_to>
      <so_that>≥ 1 % of the corpus per cycle gets fed back as gold supervision per doc 09 §3.7.4</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-04-01">
        <given>A `RealityPrediction` written by SPEC-SHIFT-SUBSCRIBER with `confidence.outcome_set.len() = 4` AND `out_of_distribution_score = 0.32`</given>
        <when>`ActiveLearningQueue::enqueue_uncertain(panel, conformal_set, ood_score)` runs</when>
        <then>An entry is added to CF_MEJEPA_ACTIVE_LEARNING_QUEUE keyed by (panel_sha, prediction_id); `signal_value` score computed per `signal_value = (outcome_set.len() / outcome_set_max_observed) × (1 - ood_score)` (uncertain × in-distribution); the queue is ranked by `signal_value` descending; the operator can read top-N via `mcp__cgreality__mejepa_active_learning_queue(limit: u32)`</then>
      </criterion>
      <criterion id="AC-EVAL-04-02">
        <given>A `RealityPrediction` with `outcome_set.len() = 4` AND `out_of_distribution_score = 0.65` (uncertain BUT out-of-distribution)</given>
        <when>The active-learning trigger evaluates the panel</when>
        <then>The panel is NOT enqueued for active learning (OOD inputs always escalate to human per doc 09 §8.5; corpus expansion not active learning); instead, the panel is logged to `CF_MEJEPA_OOD_ESCALATIONS` with reason `OOD_REQUIRES_CORPUS_EXPANSION`</then>
      </criterion>
      <criterion id="AC-EVAL-04-03">
        <given>The operator calls `mcp__cgreality__mejepa_active_learning_label(panel_sha, oracle_outcome: OracleOutcome)`</given>
        <when>The label is recorded</when>
        <then>The (panel, oracle_outcome) pair is appended to CF_MEJEPA_ACTIVE_LEARNING_LABELS with provenance metadata (operator_id, label_method: "manual" | "docker_rerun" | "adjacent_test"); the entry is removed from the queue; the next training cycle (per SPEC-TRAINING) consumes the label as a gold-tier example with `α_overrides = 5.0` weight per doc 09 §3.7.1</then>
      </criterion>
      <criterion id="AC-EVAL-04-04">
        <given>The active-learning queue has been live for 7 days</given>
        <when>The weekly report runs</when>
        <then>The report's `active_learning` section reports `queue_depth`, `labels_added_this_week`, `corpus_growth_pct = labels_added / total_corpus`; per Phase 8 DoD `corpus_growth_pct ≥ 1.0`; the report flags `active_learning_below_target: true` if below 1 %</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-05" priority="must-have">
    <narrative>
      <as_a>Phase 3 SPEC-TRAINING mini-batch sampler (UT-05)</as_a>
      <i_want_to>Read the patch-similarity graph `mejepa-task-graph` edges (cosine > 0.7 of T's patch AST diff) so I can sample similar-task examples with prob 0.3 during fine-tuning per doc 09 §3.7.5</i_want_to>
      <so_that>Cross-task transfer accelerates learning on under-represented cells</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-05-01">
        <given>The Phase 0 corpus is materialized (2,400 entries: 8 categories × 300 SWE-bench Lite tasks)</given>
        <when>`build_patch_similarity_graph(corpus: &Corpus)` runs (offline, idempotent, run on corpus_sha change)</when>
        <then>For every task pair (T_i, T_j), an `AstDiffCosine(T_i, T_j) ∈ [-1, 1]` edge is computed via E_Diff slot cosine; only edges with `cosine > 0.7` are persisted to CF_MEJEPA_TASK_GRAPH; total edge count bounded by 2,400 × top-k per node (default k = 32) ≈ 76,800 edges max; FSV at /tmp/contextgraph-mejepa-evaluation-fsv/patch-similarity-graph-evidence.json</then>
      </criterion>
      <criterion id="AC-EVAL-05-02">
        <given>The graph is built and SPEC-TRAINING is sampling task T</given>
        <when>The training sampler queries `task_graph.similar_tasks(T, cosine_threshold: 0.7, limit: 32)`</when>
        <then>The query returns Vec<(TaskId, f32)> ordered by cosine descending; SPEC-TRAINING uses with prob 0.3 to substitute a similar-task example</then>
      </criterion>
      <criterion id="AC-EVAL-05-03">
        <given>The corpus_sha changes (operator added new tasks via active learning)</given>
        <when>The graph rebuild scheduler detects corpus_sha mismatch</when>
        <then>The graph is rebuilt; the previous graph version is preserved in CF_MEJEPA_TASK_GRAPH_HISTORY for replay; the rebuild emits `MEJEPA_EVAL_TASK_GRAPH_REBUILT` info-level structured log</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-06" priority="must-have">
    <narrative>
      <as_a>External AI agent (UT-03)</as_a>
      <i_want_to>Call `mcp__cgreality__mejepa_eval_ship_gate_status()` over MCP and receive a `ShipGateStatus { passed, per_category, per_language, blockers }` so I can decide whether to trust `mejepa_verify` Approve verdicts</i_want_to>
      <so_that>I do not consume `Approve` from a model that is currently failing the ship gate for the relevant language / category</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-06-01">
        <given>The most recent eval report has `ship_gate_passed: true` AND `per_language[python] = 0.97`</given>
        <when>The agent invokes `mcp__cgreality__mejepa_eval_ship_gate_status()`</when>
        <then>The response is `ShipGateStatus { passed: true, per_category: {...}, per_language: {python: 0.97, ...}, blockers: [] }`; agent may proceed with `mejepa_verify`</then>
      </criterion>
      <criterion id="AC-EVAL-06-02">
        <given>The most recent eval report has `ship_gate_passed: false` due to `(compile_error, c) = 0.78`</given>
        <when>An agent submits a C-language patch likely classified as `compile_error` and invokes `mejepa_verify`</when>
        <then>SPEC-INFERENCE's `verify()` may still return Approve if all 7 gates pass for THIS specific patch; however, the agent SHOULD pre-check `mejepa_eval_ship_gate_status()` and pre-escalate to human when the relevant cell is blocked; this is an advisory contract, not a hard-block (per the doc 09 §1.3 "no human review needed" criterion is statistical, not per-patch deterministic)</then>
      </criterion>
      <criterion id="AC-EVAL-06-03">
        <given>No eval report has been produced yet (Phase 8 just started)</given>
        <when>The agent invokes `mcp__cgreality__mejepa_eval_ship_gate_status()`</when>
        <then>Returns `Err(MejepaEvalError::NoReportAvailable)` with `.code() == "MEJEPA_EVAL_NO_REPORT_AVAILABLE"`; no silent default; agent must escalate to human</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-07" priority="should-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>The eval report to surface the state-transfer T diagnostic (per doc 11 §8) so I can attribute low ship-gate scores to either training-quality (M(t) low) or distribution-shift (T low) problems</i_want_to>
      <so_that>I know whether to invest in more training compute or more diverse corpus expansion</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-07-01">
        <given>Phase 0 corpus distribution P_train and live-attempt distribution P_deploy from CF_MEJEPA_LIVE_PREDICTIONS</given>
        <when>The eval runner computes the state-transfer diagnostic</when>
        <then>Report carries `state_transfer_diagnostic: { d_train_deploy: f32, lambda: f32, T: f32, M_t_estimate: f32 }` with `T = exp(-d / lambda)` per doc 11 §8; `d` is computed via Wasserstein-1 distance between holdout panel embeddings and live panel embeddings; `M_t_estimate` is computed from training certs ΔΩ × ΔΞ over the last 1,000 steps; `performance_deploy_predicted = M_t_estimate · T`</then>
      </criterion>
      <criterion id="AC-EVAL-07-02">
        <given>`overall_correlation = 0.85` AND `M_t_estimate = 0.92` AND `T = 0.92`</given>
        <when>The diagnostic is computed</when>
        <then>`performance_deploy_predicted = 0.85` matches `overall_correlation = 0.85`; the report flags `attribution: "balanced (M and T both contributing)"`</then>
      </criterion>
      <criterion id="AC-EVAL-07-03">
        <given>`overall_correlation = 0.80` AND `M_t_estimate = 0.95` AND `T = 0.84`</given>
        <when>The diagnostic is computed</when>
        <then>`attribution: "distribution_shift_dominated (T = 0.84 << M = 0.95)"`; the report recommends "expand corpus to cover the live-attempt distribution" rather than "retrain harder"</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-08" priority="should-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>The OOD score and Gτ pass rate to be tracked **per language** in the eval report, with month-over-month drift surfaced when a language's score drifts > 0.1</i_want_to>
      <so_that>Per-language cross-language regressions are caught early (e.g., a Rust embedder upgrade silently regresses C++ OOD discrimination)</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-08-01">
        <given>4 weeks of eval reports with `ood_calibration_health[c]` at 0.92, 0.91, 0.88, 0.79</given>
        <when>This week's report runs</when>
        <then>`ship_gate_failures` includes `MEJEPA_EVAL_LANGUAGE_OOD_DRIFT: c language OOD AUC drifted from 0.92 to 0.79 (delta -0.13) over 4 weeks > 0.10 monthly threshold`; the report's `ood_calibration_health_history` carries the 4-point trend</then>
      </criterion>
      <criterion id="AC-EVAL-08-02">
        <given>The (over_engineer, ruby) cell has < 30 samples this week</given>
        <when>`include_per_cell: true` AND `enable_bayesian_shrinkage: true`</when>
        <then>The cell's per_cell entry is computed via Bayesian shrinkage to the language-aggregate or category-aggregate centroid (per constitution `<tct_invariants>` rule 5 parallel pattern); the report flags `shrinkage_applied: ["(over_engineer, ruby): N=12 < 30; shrunk to ruby-aggregate"]`; ship-gate ignores shrunk cells when computing per-cell threshold (avoids penalizing under-sampled cells)</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-09" priority="should-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>The eval report to surface answers to the three operator questions (Q1: claim exists, Q2: it works, Q3: how it impacts reality) per doc 09 §1.3 + the constitution mission</i_want_to>
      <so_that>The "no human review needed" decision binds to all three questions, not just predicted oracle correlation alone</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-09-01">
        <given>The holdout split contains panels with `PatchBundle.witness_chain_segment` and `pre_patch_sha / post_patch_sha`</given>
        <when>The eval runner runs Q1 evaluation (via SPEC-INFERENCE source-SHA-drift gate + SPEC-WITNESS chain integrity)</when>
        <then>`q1_pass_rate` is the fraction of holdout patches that passed both the source-SHA-drift gate AND the witness-chain-integrity gate; for healthy data q1_pass_rate ≥ 0.99 (rare to fail on holdout); report flags `q1_pass_rate_below_threshold: true` when < 0.99</then>
      </criterion>
      <criterion id="AC-EVAL-09-02">
        <given>Standard holdout eval</given>
        <when>The eval runner runs Q2 evaluation</when>
        <then>`q2_correlation = overall_correlation` (the primary phase-8 metric per doc 09 §7); ship-gate gates on q2 ≥ 0.95</then>
      </criterion>
      <criterion id="AC-EVAL-09-03">
        <given>Holdout panels with `RealityPrediction.predicted_side_effect_set` AND corresponding observed shift-log entries</given>
        <when>The eval runner runs Q3 evaluation</when>
        <then>`q3_side_effect_agreement` is the F1 score between predicted_side_effect_set and observed_side_effect_set; report flags `q3_side_effect_agreement_below_threshold: true` when < 0.90</then>
      </criterion>
    </acceptance_criteria>
  </story>

  <story id="US-EVAL-10" priority="should-have">
    <narrative>
      <as_a>Operator (UT-01)</as_a>
      <i_want_to>The aux-head distillation cycle (per doc 09 §3.7.6) to be tracked in the eval report as a separate aux_head_distillation_health metric</i_want_to>
      <so_that>I can detect when aux heads become detached specialists (their predictions don't move the predictor's hidden state)</so_that>
    </narrative>
    <acceptance_criteria>
      <criterion id="AC-EVAL-10-01">
        <given>SPEC-TRAINING distills aux-head predictions into the predictor's hidden state every 1,000 steps per doc 09 §3.7.6</given>
        <when>The eval runner inspects the most recent distillation cycle</when>
        <then>Report carries `aux_head_distillation: { last_distillation_step: u64, distillation_loss: f32, aux_heads_in_consensus: Vec<String>, aux_heads_diverged: Vec<String> }`; `aux_heads_diverged` lists aux heads whose predictions differ from the trunk by more than threshold (per-aux-head calibrated; default 0.20)</then>
      </criterion>
    </acceptance_criteria>
  </story>

</user_stories>

<requirements>

  <requirement id="REQ-EVAL-01" story_ref="US-EVAL-01" priority="must">
    <description>The `EvalRunner` impl block exposes the public methods: `run_holdout_eval(&self, window_days: u32, include_per_cell: bool) -> Result<EvalReport, EvalError>`, `check_ship_gate(&self, report: &EvalReport) -> ShipGateStatus`, `enqueue_uncertain(&mut self, panel: &Panel, conformal_set: &ConformalSet, ood_score: f32) -> Result<(), QueueError>`, `build_patch_similarity_graph(corpus: &Corpus) -> Result<PatchGraph, GraphError>`. The `EvalRunner` itself is constructed with `holdout_dataset, calibration_set, predictor, conformal_calibrator, ood_detector, gtau_checker, storage, witness_chain` per the component-contracts constraint.</description>
    <rationale>Per doc 09 §7 Phase 8 plus the operator's spec-development brief; the surface is fixed.</rationale>
    <constraints>`run_holdout_eval` is `&self` (pure-read; no model mutation); `enqueue_uncertain` is `&mut self` (queue mutation). All return `Result<_, EvalError>` whose variants have `.code()` SCREAMING_SNAKE tags per constitution `<coding_standards>`.</constraints>
    <verification_method>Unit tests assert signature stability via static `fn _assert_signatures()`; integration test calls all four through MCP shim and asserts response shapes.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-02" story_ref="US-EVAL-01" priority="must">
    <description>The `EvalReport` struct carries exactly these fields: `report_date: NaiveDate`, `corpus_sha: [u8; 32]`, `model_weights_sha: [u8; 32]`, `calibration_version: String`, `constellation_version: String`, `code_version: String`, `rolling_window_size: usize`, `overall_correlation: f32`, `per_category: HashMap<MutationCategory, f32>` (8 entries: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError), `per_language: HashMap<Language, Option<f32>>` (11 entries: rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php — Go and PHP marked Option::None until corpus catches up), `per_cell: Option<HashMap<(MutationCategory, Language), Option<f32>>>` (88 entries when `include_per_cell: true`; None entries on insufficient samples), `ship_gate_passed: bool`, `ship_gate_failures: Vec<String>`, `ood_calibration_health: HashMap<Language, f32>`, `conformal_coverage_health: HashMap<Language, ConformalHealthEntry>`, `gtau_pass_rate: HashMap<Language, f32>`, `q1_pass_rate: f32`, `q2_correlation: f32` (== overall_correlation), `q3_side_effect_agreement: f32`, `active_learning: ActiveLearningSummary`, `drift_severity_history: Vec<DriftSeverity>` (last 4 weeks), `model_promotion_history: Vec<ModelPromotion>` (rolling 12 months), `state_transfer_diagnostic: Option<StateTransferDiagnostic>` (None when M(t) cannot be estimated), `aux_head_distillation: Option<AuxHeadDistillationSummary>`, `per_cell_warnings: Vec<String>`, `shrinkage_applied: Vec<String>`.</description>
    <rationale>Per doc 09 §7 Phase 8 verbatim plus doc 10 §C cross-language mandate plus doc 11 §8 state-transfer T plus doc 09 §1.3 three operator questions. The 11-language matrix matches constitution `<tech_stack>` (rust, python, javascript, typescript, go, java, c, cpp, csharp, ruby, php). Phase-8 supports 9 today (Go, PHP marked `Option::None` until corpus + AST chunker per language ship).</rationale>
    <constraints>All correlation values must be finite (`!is_nan() && !is_infinite()`) and in [-1, 1]; all f32 values clamped at construction; `MutationCategory` is one of the 8 doc-09 §3.1 categories; `Language` is one of the 11 supported languages; `EvalReport::try_new(builder)` validates invariants and returns `Err(EvalError::NanDetected)` on violation.</constraints>
    <verification_method>Unit test passes `f32::NAN` into each f32 field and asserts `try_new` returns `Err(NanDetected)`; FSV harness writes one healthy + one degraded report and verifies all fields populated correctly.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-03" story_ref="US-EVAL-02" priority="must">
    <description>The ship-gate decision is the conjunction (AND) of: (i) `overall_correlation ≥ 0.95`; (ii) `per_category[c] ≥ 0.95` for every c in 8 categories; (iii) `per_language[l] ≥ 0.95` for every l in supported (non-null) languages; (iv) `per_cell[(c, l)] ≥ 0.90` for every non-null cell (looser threshold given smaller per-cell sample); (v) `ood_calibration_health[l] ≥ 0.85` AUC for every l; (vi) `conformal_coverage_health[l].empirical` within ±2 % of stated 1−α for every l; (vii) `q1_pass_rate ≥ 0.99`; (viii) `q3_side_effect_agreement ≥ 0.90`. Any single violation yields `ship_gate_passed: false`.</description>
    <rationale>Per constitution `<performance_budgets>` phase8_ship_gate verbatim ("≥ 0.95 stably across all 8 mutation categories AND all supported languages on rolling-window holdout") plus doc 10 §C ("conformal calibration set MUST stratify by language so threshold not Python-biased"). The per-cell 0.90 threshold (looser than per-category 0.95) reflects that per-cell estimators have higher variance.</rationale>
    <constraints>The thresholds (0.95 aggregate, 0.90 per-cell, 0.85 OOD AUC, ±2 % coverage) are constitution-pinned; cannot be loosened without explicit operator approval AND a constitution `<spec_version>` bump per `<spec_authority_order>`. Tightening (e.g., 0.97 ship gate) is allowed via configuration but recorded in `EvalReport.ship_gate_thresholds` for audit.</constraints>
    <verification_method>Unit test exercises every passing/failing combination of the 8 conditions; integration test injects synthetic regression in each cell and asserts `ship_gate_failures` carries the expected SCREAMING_SNAKE-tagged blocker.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-04" story_ref="US-EVAL-03" priority="must">
    <description>Week-over-week regression detection: when comparing this week's `EvalReport` to last week's, any cell whose correlation drops > 2 % yields `ship_gate_failures` entry tagged `MEJEPA_EVAL_SHIP_GATE_REGRESSION` with payload `{ cell: (category, language), prior: f32, current: f32, delta_pct: f32 }`. Comparison uses last week's persisted report from CF_MEJEPA_EVAL_REPORTS.</description>
    <rationale>Per failure-modes doc requirement; per doc 09 §5.2 the rolling-window drift detection trigger; per doc 09 §7 Phase 8 ship-gate stability requirement.</rationale>
    <constraints>The 2 % threshold is per-cell; whole-language drops > 1 % also flag (`per_language` regression check). When no prior week's report exists, regression check is skipped (no false positive on first run); FSV harness records this case.</constraints>
    <verification_method>Integration test seeds last week's report at 0.94 cell + this week's at 0.91; asserts the regression failure is reported; FSV harness writes `regression-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-05" story_ref="US-EVAL-04" priority="must">
    <description>The `ActiveLearningQueue` accepts entries via `enqueue_uncertain(panel, conformal_set, ood_score)`. An entry is added only when `conformal_set.outcome_set.len() > 3` AND `ood_score < 0.5`. Each entry carries `signal_value = (outcome_set.len() / max_observed_outcome_set_len) × (1 - ood_score)` for ranking. The queue is bounded (default 10,000 entries; older ranked-low entries evicted on overflow). Operator labels via `mcp__cgreality__mejepa_active_learning_label(panel_sha, oracle_outcome, label_method)` where `label_method ∈ {Manual, DockerRerun, AdjacentTestRerun}`.</description>
    <rationale>Per doc 09 §3.7.4: "When ME-JEPA's conformal set has size > 3 (uncertain) AND OOD score < 0.5 (in-distribution), surface for either (a) operator label, (b) Docker oracle re-run with mutation variants, or (c) adjacent-test re-runs."</rationale>
    <constraints>OOD inputs (`ood_score >= 0.5`) MUST NOT be enqueued for active learning per doc 09 §8.5 ("OOD inputs ALWAYS escalate to human until the corpus catches up"); they go to CF_MEJEPA_OOD_ESCALATIONS instead. The queue's eviction is signal_value-priority not FIFO (eviction lowest-signal-value first).</constraints>
    <verification_method>Unit test asserts both the accept and reject paths; FSV harness writes `active-learning-queue.json` with one in-distribution-uncertain (enqueued) and one OOD-uncertain (rejected).</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-06" story_ref="US-EVAL-04" priority="must">
    <description>Active-learning corpus growth target: weekly report's `active_learning.corpus_growth_pct ≥ 1.0 %`. When < 1 %, report flags `active_learning_below_target: true` and surfaces in `ship_gate_failures` as `MEJEPA_EVAL_ACTIVE_LEARNING_BELOW_TARGET` (info-level; does not block ship gate but recommends operator action).</description>
    <rationale>Per Phase 8 DoD verbatim: "Active learning loop adds ≥ 1% corpus per week as gold supervision."</rationale>
    <constraints>`corpus_growth_pct = labels_added_this_week / total_corpus_size_at_start_of_week × 100`; total_corpus includes Phase 0 corpus (2,400) + previous active-learning labels.</constraints>
    <verification_method>Integration test seeds 50 active-learning labels in a week with 5,000-entry corpus; asserts growth_pct = 1.0 %; FSV writes weekly report capturing the metric.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-07" story_ref="US-EVAL-05" priority="must">
    <description>The `build_patch_similarity_graph(corpus)` function constructs the patch-similarity graph used by SPEC-TRAINING for cross-task transfer. For every pair (T_i, T_j) in corpus, compute `cosine(E_Diff(T_i.patch_diff), E_Diff(T_j.patch_diff))`; persist edges with `cosine > 0.7`; cap top-k per source node at 32 (default; configurable). Persist to CF_MEJEPA_TASK_GRAPH keyed by `(source_task_sha | target_task_sha)` value `cosine_f32_be`. Idempotent: rebuild only on corpus_sha change. Previous graphs preserved in CF_MEJEPA_TASK_GRAPH_HISTORY.</description>
    <rationale>Per doc 09 §3.7.5 verbatim: "Use the patch-similarity graph (§3.5.3) to upweight examples from similar tasks during fine-tuning. Implementation: in mini-batch sampler, when training on task T, with probability 0.3 sample from tasks within cosine > 0.7 of T's patch."</rationale>
    <constraints>Total edge budget bounded by O(N × k_top) ≈ 76,800 for 2,400-task corpus; computation O(N² × d_diff) but parallelizable; FSV harness must show it completes in < 30 minutes on a 2,400-task corpus on RTX 5090. Cosine computed on AST-perfect E_Diff slot embeddings (per SPEC-INSTRUMENTS), not raw text.</constraints>
    <verification_method>Integration test on a 50-task subset asserts edge count, cosine threshold, top-k cap; FSV harness `patch-similarity-graph-evidence.json` writes histogram of edge weights + node degree distribution + total edge count.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-08" story_ref="US-EVAL-06" priority="must">
    <description>The MCP tool `mcp__cgreality__mejepa_eval_run(window_days: u32, include_per_cell: bool)` triggers a fresh holdout eval and returns `EvalReport`. The tool `mcp__cgreality__mejepa_eval_ship_gate_status()` returns the most recent `ShipGateStatus` from CF_MEJEPA_EVAL_REPORTS without re-running. The tool `mcp__cgreality__mejepa_active_learning_queue(limit: u32)` returns the top-`limit` queue entries ranked by `signal_value` descending. The tool `mcp__cgreality__mejepa_active_learning_label(panel_sha, oracle_outcome, label_method)` records a label.</description>
    <rationale>Per doc 09 §7 Phase 8 plus the operator's spec-dev brief listing the MCP tool surface. Tool names follow `<mcp_tool_namespace_lock>` per constitution.</rationale>
    <constraints>JSON schemas enforce `additionalProperties: false` per the audit-9 lesson. `mejepa_eval_run` is a long-running tool (may take minutes on a full holdout); response includes `report_path: file:...` for retrieval. The retired `mcp__cgreality__reality_run_attempt` MUST remain fail-closed with `CCREALITY_ENGINE_RETIRED`.</constraints>
    <verification_method>Integration test invokes each MCP tool over JSON-RPC; asserts schema compliance + round-trip equivalence with direct in-process calls; FSV harness writes `mcp-roundtrip-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-09" story_ref="US-EVAL-07" priority="should">
    <description>The state-transfer diagnostic per doc 11 §8: `T(train, deploy) = exp(-d / λ)` where `d` is Wasserstein-1 distance between holdout panel embeddings (proxy for P_train) and live-attempt panel embeddings from CF_MEJEPA_LIVE_PREDICTIONS (proxy for P_deploy); λ is a configurable transfer-bandwidth constant (default 1.0). `M_t_estimate` is the geometric mean of ΔΩ × ΔΞ over the last 1,000 training certs from CF_MEJEPA_TRAIN_CERTS. `performance_deploy_predicted = M_t_estimate · T`. The report carries an `attribution` string: "balanced" | "training_quality_dominated (M low)" | "distribution_shift_dominated (T low)" | "both low" computed via thresholds (M < 0.85 OR T < 0.85 trigger non-balanced labels).</description>
    <rationale>Per doc 11 §8 verbatim: "performance_deploy = M(t) · T(train, deploy)". The diagnostic lets operators attribute low ship-gate scores to the right cause.</rationale>
    <constraints>Wasserstein-1 distance computed via samples (not exact; default 5,000 samples per side). When CF_MEJEPA_LIVE_PREDICTIONS has < 100 entries, T is `Option::None` and attribution is "insufficient_live_data"; the report still ships. λ is constitution-pinned; cannot be altered without operator approval.</constraints>
    <verification_method>Unit test on synthetic distributions with known Wasserstein distance; FSV writes `state-transfer-evidence.json` with d, λ, T, M_t_estimate, attribution.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-10" story_ref="US-EVAL-08" priority="must">
    <description>Per-language OOD calibration health is computed via ROC AUC between in-distribution holdout panels for that language and synthetic OOD panels for that language (random AST diffs over unrelated repos in the same language). When AUC drifts > 0.10 over 4-week rolling window, report flags `MEJEPA_EVAL_LANGUAGE_OOD_DRIFT`. Per-language conformal coverage health computes empirical coverage = (count of holdout examples whose actual oracle outcome is in `outcome_set`) / total. When |empirical − stated| > 2 %, flags `MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT`.</description>
    <rationale>Per doc 10 §C cross-language mandate ("OOD detection respects language boundaries... cross-language inputs escalate to human until the language is folded into the training set"); per constitution `<performance_budgets>` ood_separation_target AUC > 0.85 + conformal_coverage_target ±2 %.</rationale>
    <constraints>Synthetic OOD panels are generated by SPEC-CORPUS's mutation engine on a held-out repo per language; they MUST be physical, never mock. When a language has < 50 holdout samples, OOD AUC is `Option::None` and reports `MEJEPA_EVAL_INSUFFICIENT_SAMPLES`.</constraints>
    <verification_method>FSV harness `ood-evidence.json` writes per-language ROC curve + AUC + 4-week trend + drift flag; integration test seeds 4 weeks of trending data.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-11" story_ref="US-EVAL-09" priority="must">
    <description>The three-operator-question rollup. Q1 (`q1_pass_rate`): fraction of holdout patches whose `pre_patch_sha` and `post_patch_sha` match disk AND whose `witness_chain_segment` replays clean. Q2 (`q2_correlation`): identical to `overall_correlation`. Q3 (`q3_side_effect_agreement`): F1 score (precision × recall × 2 / (precision + recall)) between `RealityPrediction.predicted_side_effect_set` and `observed_side_effect_set` derived from the corresponding shift-log subscriber entry. All three are surfaced in the eval report and gate the ship gate per REQ-EVAL-03 (vii) and (viii).</description>
    <rationale>Per constitution mission verbatim and per doc 09 §1.3 "ME-JEPA-Code answers three operator questions deterministically." The ship gate must bind on all three, not just Q2.</rationale>
    <constraints>Q1 requires SPEC-INFERENCE Gate 5 + Gate 6 logic to be invoked per holdout panel; Q3 requires SPEC-SHIFT-SUBSCRIBER's observed-shift extraction to be available — when not available (Phase 7 not shipped), Q3 is `Option::None` and the ship gate skips the q3 condition.</constraints>
    <verification_method>FSV harness writes `q1q2q3-evidence.json` with one example per question + the rolled-up metrics; integration test against synthetic shift-log entries.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-12" story_ref="US-EVAL-10" priority="should">
    <description>Aux-head distillation health (per doc 09 §3.7.6) is tracked. Report carries `aux_head_distillation: { last_distillation_step, distillation_loss, aux_heads_in_consensus: Vec<String>, aux_heads_diverged: Vec<String> }`. Aux-head divergence is detected when an aux-head's prediction differs from the trunk's hidden-state-derived prediction by more than a per-aux-head calibrated threshold (default 0.20).</description>
    <rationale>Per doc 09 §3.7.6 verbatim: "Every 1,000 steps, distill aux heads' predictions into the predictor's hidden state via a frozen-target distillation loss. Prevents aux heads becoming detached 'specialists' — keeps trunk representation rich."</rationale>
    <constraints>When SPEC-TRAINING has not run distillation yet (no distillation step recorded in CF_MEJEPA_TRAIN_CERTS), `aux_head_distillation: Option::None` and report does not flag.</constraints>
    <verification_method>Integration test on synthetic aux-head outputs with known divergence; FSV `aux-head-distillation-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-13" story_ref="US-EVAL-01" priority="must">
    <description>The fail-closed error taxonomy includes (per constitution `<failure_taxonomy>` extended): `MEJEPA_EVAL_HOLDOUT_LEAKAGE`, `MEJEPA_EVAL_INSUFFICIENT_SAMPLES`, `MEJEPA_EVAL_SHIP_GATE_REGRESSION`, `MEJEPA_EVAL_LANGUAGE_OOD_DRIFT`, `MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT`, `MEJEPA_EVAL_REPORT_PERSIST_FAIL`, `MEJEPA_EVAL_NO_REPORT_AVAILABLE`, `MEJEPA_EVAL_TASK_GRAPH_REBUILD_FAIL`, `MEJEPA_EVAL_ACTIVE_LEARNING_BELOW_TARGET` (info-level). Each carries a structured payload (cell, language, delta_pct, etc.) and is logged with input shapes + report_date before returning per `<error_handling>`.</description>
    <rationale>Per constitution `<error_handling>` "robust error logging" and `<failure_taxonomy>` extension; per doc 10 §D ("MUST NOT mean silent fallback").</rationale>
    <verification_method>Unit test for each variant asserts `.code()` returns the expected SCREAMING_SNAKE tag; FSV harness writes one example of each error case to `error-taxonomy-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-14" story_ref="US-EVAL-01" priority="must">
    <description>Holdout split leakage check: at every `run_holdout_eval` invocation, validate that no task_id appears in BOTH the train split AND the holdout split. Use `assert_no_test_patch_leakage` helper from SPEC-CORPUS. Fail-closed with `MEJEPA_EVAL_HOLDOUT_LEAKAGE` on any leakage.</description>
    <rationale>Per doc 09 §3.4 holdout split protocol verbatim ("no shared-task leakage between repos"); per business-rule guard against silent corruption of the eval signal.</rationale>
    <constraints>Leakage detection is REPO-ATOMIC by default (per SPEC-CORPUS SplitMode::RepoAtomic) — no two tasks from the same repo can split across train/holdout boundary; `SplitMode::InstanceAtomic` permitted only when explicitly configured AND recorded in `EvalReport.split_mode`.</constraints>
    <verification_method>Integration test injects a deliberate leakage and asserts `MEJEPA_EVAL_HOLDOUT_LEAKAGE`; FSV harness writes `leakage-check-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-15" story_ref="US-EVAL-01" priority="must">
    <description>Phase 8 DoD verification command: `cargo run --release -p context-graph-mejepa -- eval-run --window-days 7 --include-per-cell --output ./data/mejepa-eval/` reports the EvalReport JSON to stdout AND writes the persistent file path; exit code 0 when ship gate passes, 1 when gate fails (cell visibility), 2 on hard error (leakage, persistence failure, etc.).</description>
    <rationale>Per doc 09 §7 Phase 8 ongoing operational requirement and the operator's spec-dev brief.</rationale>
    <verification_method>The command is itself the verification; FSV evidence at /tmp/contextgraph-mejepa-evaluation-fsv/ captures stdout + persisted JSON + exit code.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-16" story_ref="US-EVAL-01" priority="must">
    <description>Performance budgets: `run_holdout_eval` on a 240-task holdout split (10 % of 2,400-task Phase 0 corpus) completes in `< 30 minutes` p50 wall-clock on RTX 5090 with cached instruments; `< 2 hours` p99 cold (full instrument re-eval); `build_patch_similarity_graph` on a 2,400-task corpus completes in `< 30 minutes` (computed once per corpus_sha change). All EvalReport JSONs ≤ 1 MiB; persistence to CF_MEJEPA_EVAL_REPORTS uses bincode for efficiency.</description>
    <rationale>The eval pipeline must complete weekly without crowding out training time on the same hardware.</rationale>
    <verification_method>FSV harness runs both end-to-end and reports wall-clock to `latency-evidence.json`; integration test asserts upper bounds.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-17" story_ref="US-EVAL-01" priority="must">
    <description>Determinism: same `(corpus_sha, model_weights_sha, calibration_version, constellation_version, code_version, holdout split)` produces byte-equal `EvalReport` modulo `report_date` field. Stochastic operations (e.g., Wasserstein sampling for state-transfer T) are seeded from `code_version` per constitution `<anti_patterns>` "Determinism" rule.</description>
    <rationale>Per constitution `<anti_patterns>` Determinism rule verbatim; per doc 09 §1.3 reproducibility is part of the verifier-as-deliverable guarantee.</rationale>
    <verification_method>FSV harness runs `run_holdout_eval` 3× on the same fixtures; asserts SHA-256 of the serialized report (excluding report_date field) is byte-equal.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-18" story_ref="US-EVAL-01" priority="must">
    <description>Provenance binding: every persisted `EvalReport` carries `corpus_sha, model_weights_sha, calibration_version, constellation_version, code_version, holdout_split_sha` per constitution `<provenance>` rule. Reports are written to CF_MEJEPA_EVAL_REPORTS keyed by `(report_date_iso8601 | report_ulid)` with the JSON value bincode-serialized.</description>
    <rationale>Per constitution `<provenance>` rule verbatim ("Every persistent artifact... carries provenance metadata"). Allows replay verification: given the report, the reader can reconstruct exactly which model + corpus + calibration produced the metrics.</rationale>
    <verification_method>Integration test writes a report and reads it back; asserts every provenance field matches the running configuration; FSV harness writes `provenance-evidence.json`.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-19" story_ref="US-EVAL-04" priority="must">
    <description>Open-research-question tracking. The `EvalReport` carries an `open_research_questions` list per doc 09 §8 (1-8): each entry has `question_id` (Q1..Q8), `mitigation_state: enum {Active, Resolved, Deferred}`, `evidence_pointers: Vec<String>` (e.g., paths to FSV evidence). When the operator marks a question Resolved, the resolution and the supporting evidence are recorded; the report carries the snapshot.</description>
    <rationale>Per doc 09 §8 verbatim 8 open research questions ("Will JEPA generalize... Are 5,120-d panels expressive enough... Per-language ME-JEPAs or unified... Mutation corpus diverse... Trust on novel APIs... right λ_ewc... Non-SWE-bench code... ME-JEPA disagreeing with itself").</rationale>
    <constraints>The list is not error-flag generating; it is informational state for the operator to track over time. When all 8 are Resolved, the operator may issue a "ME-JEPA-Code complete" signal.</constraints>
    <verification_method>FSV harness writes `open-research-questions-evidence.json` with the 8 questions + initial mitigation states.</verification_method>
  </requirement>

  <requirement id="REQ-EVAL-20" story_ref="US-EVAL-08" priority="should">
    <description>Bayesian shrinkage for under-sampled cells: when a (category × language) cell has `N < 30` samples in the holdout, optionally shrink to either language-aggregate or category-aggregate centroid per constitution `<tct_invariants>` rule 5 parallel pattern. Shrinkage is opt-in via `enable_bayesian_shrinkage: bool` config (default true). Shrunk cells are excluded from per-cell ship-gate threshold check (avoids penalizing under-sampled cells); the report flags them in `shrinkage_applied`.</description>
    <rationale>Per constitution `<tct_invariants>` rule 5 verbatim ("Bayesian shrinkage collapses cells with N < 50 supporting examples to language-aggregate or entity-aggregate centroid"); the same parallel pattern applies to evaluation cells.</rationale>
    <constraints>Min-N for non-shrinkage is 30 (looser than TCT's 50 because evaluation does not need to support fine-grained centroid construction). When operator disables shrinkage, under-sampled cells appear as `null` and are flagged with MEJEPA_EVAL_INSUFFICIENT_SAMPLES.</constraints>
    <verification_method>Unit test exercises both shrinkage-on and shrinkage-off paths on a synthetic 5-sample cell; FSV harness writes `bayesian-shrinkage-evidence.json`.</verification_method>
  </requirement>

</requirements>

<business_rules>

  <rule id="BR-EVAL-01" req_ref="REQ-EVAL-01">
    <condition>Any caller invokes `run_holdout_eval` from outside the workspace</condition>
    <action>The call MUST go through `mcp__cgreality__mejepa_eval_run`; direct in-process FFI is reserved for the workspace's own crates.</action>
    <exception>None. Network surface is exclusively the MCP server per CLAUDE.md §11.9.</exception>
  </rule>

  <rule id="BR-EVAL-02" req_ref="REQ-EVAL-03">
    <condition>The ship gate evaluates with any condition (i)–(viii) failing</condition>
    <action>`ship_gate_passed: false`; the failing condition(s) appear in `ship_gate_failures` with SCREAMING_SNAKE error tag and structured payload; report is still persisted (failure is visibility, not write-blocking).</action>
    <exception>None. The ship gate cannot be silently skipped or muted; thresholds cannot be loosened without a constitution `<spec_version>` bump per `<spec_authority_order>`.</exception>
  </rule>

  <rule id="BR-EVAL-03" req_ref="REQ-EVAL-05">
    <condition>An OOD-flagged panel (`ood_score >= 0.5`) attempts to enqueue for active learning</condition>
    <action>Reject the enqueue; route the panel to CF_MEJEPA_OOD_ESCALATIONS with reason `OOD_REQUIRES_CORPUS_EXPANSION`; surface to operator via the next eval report's `ood_escalations_pending` field.</action>
    <exception>None. Per doc 09 §8.5 "OOD inputs ALWAYS escalate to human until the corpus catches up." Active learning is for in-distribution-uncertain only.</exception>
  </rule>

  <rule id="BR-EVAL-04" req_ref="REQ-EVAL-14">
    <condition>The holdout split contains a task that also appears in the train split (leakage)</condition>
    <action>`run_holdout_eval` returns `Err(MejepaEvalError::HoldoutLeakage { task_id, found_in: ["train", "holdout"] })`; do not produce a report; surface to operator as a CRITICAL blocker per `memory/blockers/`.</action>
    <exception>None. Leakage corrupts the evaluation signal silently; fail-closed at boundary.</exception>
  </rule>

  <rule id="BR-EVAL-05" req_ref="REQ-EVAL-04">
    <condition>This week's eval report shows `MEJEPA_EVAL_SHIP_GATE_REGRESSION` for cell (c, l) with delta > 2 %</condition>
    <action>SPEC-SELF-HEALING reads the failure and queues a Mode B retrain on (c, l) stratum; the regression is logged to `model_promotion_history` with `regression_triggered: true`.</action>
    <exception>When SPEC-SELF-HEALING is not yet shipped, the regression flag stays in the report; operator runs targeted retrain manually.</exception>
  </rule>

  <rule id="BR-EVAL-06" req_ref="REQ-EVAL-08">
    <condition>An MCP caller invokes `mcp__cgreality__mejepa_eval_ship_gate_status()` with no eval report yet persisted (Phase 8 just started)</condition>
    <action>Return `Err(MejepaEvalError::NoReportAvailable)` with `.code() == "MEJEPA_EVAL_NO_REPORT_AVAILABLE"`; agents must escalate to human.</action>
    <exception>None. No silent default; the contract is "report exists or no report exists."</exception>
  </rule>

  <rule id="BR-EVAL-07" req_ref="REQ-EVAL-07">
    <condition>The corpus_sha changes (operator added active-learning labels OR new tasks)</condition>
    <action>Trigger `build_patch_similarity_graph` rebuild; preserve previous graph in CF_MEJEPA_TASK_GRAPH_HISTORY; emit `MEJEPA_EVAL_TASK_GRAPH_REBUILT` info-level structured log.</action>
    <exception>The rebuild may be deferred to next nightly window (configurable `task_graph_rebuild_schedule: enum {Immediate, Nightly}`); during defer, training samplers continue using the prior graph.</exception>
  </rule>

  <rule id="BR-EVAL-08" req_ref="REQ-EVAL-17">
    <condition>The same eval inputs are submitted twice within a turn</condition>
    <action>Both runs produce identical `EvalReport` modulo `report_date`; SHA-256 of serialized report (excluding report_date) byte-equal across runs.</action>
    <exception>None. Determinism is constitution-pinned per `<anti_patterns>` Determinism rule.</exception>
  </rule>

</business_rules>

<edge_cases>

  <edge_case id="EC-EVAL-01" req_ref="REQ-EVAL-01" priority="critical">
    <scenario>The holdout split is empty (Phase 0 corpus has not generated any holdout entries; SPEC-CORPUS not run yet).</scenario>
    <expected_behavior>`run_holdout_eval` returns `Err(MejepaEvalError::EmptyHoldout)` with `.code() == "MEJEPA_EVAL_EMPTY_HOLDOUT"`; recommend operator run `mejepa-corpus generate --split` first.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-02" req_ref="REQ-EVAL-02" priority="critical">
    <scenario>The predictor returns a NaN or Infinity in any per-cell correlation.</scenario>
    <expected_behavior>`EvalReport::try_new` returns `Err(MejepaEvalError::NanDetected { field, cell })`; the partial computation is logged with input shapes per `<error_handling>`; no truncation, no clamp-to-zero fallback.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-03" req_ref="REQ-EVAL-04" priority="high">
    <scenario>The very first eval run (no prior week's report exists).</scenario>
    <expected_behavior>Regression check is skipped; report carries `regression_check: "no_prior_report"`; ship gate evaluates only the absolute thresholds (i)–(viii).</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-04" req_ref="REQ-EVAL-05" priority="high">
    <scenario>The active-learning queue is at capacity (10,000 entries) when a new high-signal-value entry arrives.</scenario>
    <expected_behavior>Evict the lowest-signal-value existing entry (NOT FIFO); insert the new entry; emit `MEJEPA_EVAL_ACTIVE_LEARNING_QUEUE_EVICT` debug-level structured log; the eviction is recorded in CF_MEJEPA_ACTIVE_LEARNING_EVICTIONS for replay.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-05" req_ref="REQ-EVAL-09" priority="medium">
    <scenario>CF_MEJEPA_LIVE_PREDICTIONS has < 100 entries (Phase 7 just shipped or is offline).</scenario>
    <expected_behavior>State-transfer diagnostic returns `Option::None` for T and the report carries `state_transfer_diagnostic: { attribution: "insufficient_live_data", ... }`; the rest of the report is unaffected.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-06" req_ref="REQ-EVAL-10" priority="high">
    <scenario>A language has no holdout examples (e.g., go-language tasks not in the Phase 0 corpus).</scenario>
    <expected_behavior>`per_language[go] = Option::None`; `ood_calibration_health[go]` is omitted from the HashMap; `gtau_pass_rate[go]` is omitted; ship gate ignores the missing language; the report flags `unsupported_languages: ["go"]` for clarity.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-07" req_ref="REQ-EVAL-11" priority="high">
    <scenario>Q3 evaluation: a holdout panel has no corresponding shift-log entry (the patch was submitted via test fixture, not via the live reality loop).</scenario>
    <expected_behavior>That panel contributes only to Q1 and Q2; Q3 F1 is computed only over panels with both predicted and observed side-effect sets; report carries `q3_eligible_count` separately from total holdout count.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-08" req_ref="REQ-EVAL-12" priority="medium">
    <scenario>SPEC-TRAINING has not yet performed any aux-head distillation (training has run < 1,000 steps).</scenario>
    <expected_behavior>`aux_head_distillation: Option::None`; report does not flag; ship gate does not bind on aux-head divergence.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-09" req_ref="REQ-EVAL-07" priority="high">
    <scenario>The 2,400-task corpus has a single task with no parseable AST diff (corrupted patch).</scenario>
    <expected_behavior>`build_patch_similarity_graph` skips that task with `MEJEPA_EVAL_TASK_GRAPH_SKIP { task_id, reason }`; remaining 2,399 tasks build the graph; FSV writes a `skipped_tasks` list.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-10" req_ref="REQ-EVAL-03" priority="medium">
    <scenario>The (mutation_category, language) cell `(known_good, python)` has perfect 1.0 correlation across 200 samples.</scenario>
    <expected_behavior>`per_cell[(known_good, python)] = 1.0`; ship gate passes for this cell; FSV `per-cell-evidence.json` records the perfect-cell case as a positive control.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-11" req_ref="REQ-EVAL-04" priority="medium">
    <scenario>The corpus_sha changed between last week's report and this week's; cell-level comparison is moot (the cell's underlying data changed).</scenario>
    <expected_behavior>Regression check runs but flags `regression_check_warning: "corpus_sha changed; comparison cells may not be commensurate"`; the regression flag is still emitted but operator interprets with the warning context.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-12" req_ref="REQ-EVAL-16" priority="low">
    <scenario>The eval run wall-clock exceeds 30 minutes p50 budget on an unusually large corpus expansion (> 5,000 tasks).</scenario>
    <expected_behavior>Eval still completes (budget is informational, not write-blocking); the report flags `latency_budget_exceeded: true` with actual wall-clock recorded; operator may sample-scale or shard the holdout for efficiency.</expected_behavior>
  </edge_case>

  <edge_case id="EC-EVAL-13" req_ref="REQ-EVAL-18" priority="critical">
    <scenario>Persistence to CF_MEJEPA_EVAL_REPORTS fails (RocksDB write error, disk full).</scenario>
    <expected_behavior>Return `Err(MejepaEvalError::ReportPersistFail { backend_error })` with `.code() == "MEJEPA_EVAL_REPORT_PERSIST_FAIL"`; the in-memory report is also written to a fallback path (`/tmp/contextgraph-mejepa-evaluation-fsv/last-eval-report.json`) so it is not lost; operator alerted via `memory/blockers/`.</expected_behavior>
  </edge_case>

</edge_cases>

<error_states>

  <error id="ERR-EVAL-01" http_code="409" req_ref="REQ-EVAL-14">
    <condition>Task appears in both train and holdout splits.</condition>
    <user_message>Holdout split is corrupted: task {task_id} found in both train and holdout. Eval cannot run until split is repaired.</user_message>
    <internal_message>MEJEPA_EVAL_HOLDOUT_LEAKAGE: task_id={...}, found_in=["train", "holdout"]</internal_message>
    <recovery_action>Operator: regenerate the holdout split via `mejepa-corpus split --mode repo-atomic`; verify with `assert_no_test_patch_leakage` helper.</recovery_action>
  </error>

  <error id="ERR-EVAL-02" http_code="422" req_ref="REQ-EVAL-01">
    <condition>A (category × language) cell has < N (default 30) holdout samples.</condition>
    <user_message>Per-cell evaluation insufficient samples for ({category}, {language}); cell omitted from per-cell metrics.</user_message>
    <internal_message>MEJEPA_EVAL_INSUFFICIENT_SAMPLES: cell=({category}, {language}), required={30}, observed={N}</internal_message>
    <recovery_action>Operator: expand corpus for under-represented cell via active learning OR enable Bayesian shrinkage (`enable_bayesian_shrinkage: true`).</recovery_action>
  </error>

  <error id="ERR-EVAL-03" http_code="422" req_ref="REQ-EVAL-04">
    <condition>This week's cell correlation dropped > 2 % from last week's.</condition>
    <user_message>Ship-gate regression: cell ({category}, {language}) dropped from {prior:.2} to {current:.2} ({delta_pct:.1}%).</user_message>
    <internal_message>MEJEPA_EVAL_SHIP_GATE_REGRESSION: cell=({category}, {language}), prior={prior}, current={current}, delta_pct={delta_pct}</internal_message>
    <recovery_action>SPEC-SELF-HEALING auto-triggers Mode B retrain on the regressed stratum; or operator runs `mejepa-train --target-cell ({category}, {language})`.</recovery_action>
  </error>

  <error id="ERR-EVAL-04" http_code="422" req_ref="REQ-EVAL-10">
    <condition>Per-language OOD AUC drifted > 0.10 over 4-week rolling window.</condition>
    <user_message>OOD calibration drift detected for language {l}: AUC dropped from {prior:.2} to {current:.2} over 4 weeks.</user_message>
    <internal_message>MEJEPA_EVAL_LANGUAGE_OOD_DRIFT: language={l}, prior_auc={prior}, current_auc={current}, delta={delta}, window_weeks=4</internal_message>
    <recovery_action>Operator: investigate recent embedder upgrades / corpus changes affecting language {l}; SPEC-SELF-HEALING may auto-trigger Mode B retrain.</recovery_action>
  </error>

  <error id="ERR-EVAL-05" http_code="422" req_ref="REQ-EVAL-10">
    <condition>Per-language conformal coverage drifted outside ±2 % of stated 1−α.</condition>
    <user_message>Conformal coverage drift for language {l}: stated {stated:.2} but empirical {empirical:.2} (off by {delta_pct:.1}%).</user_message>
    <internal_message>MEJEPA_EVAL_LANGUAGE_CONFORMAL_DRIFT: language={l}, stated={stated}, empirical={empirical}, delta_pct={delta_pct}</internal_message>
    <recovery_action>Operator: re-run conformal calibration via SPEC-INFERENCE `mejepa infer-test --calibration ...`; verify per-language stratum has min_samples.</recovery_action>
  </error>

  <error id="ERR-EVAL-06" http_code="500" req_ref="REQ-EVAL-18">
    <condition>RocksDB write of EvalReport failed (disk full, IO error).</condition>
    <user_message>Failed to persist eval report; report saved to fallback path /tmp/contextgraph-mejepa-evaluation-fsv/last-eval-report.json. Operator action required.</user_message>
    <internal_message>MEJEPA_EVAL_REPORT_PERSIST_FAIL: cf={CF_MEJEPA_EVAL_REPORTS}, backend_error={...}</internal_message>
    <recovery_action>Operator: investigate disk usage / RocksDB health; report is durable at fallback path until persist succeeds.</recovery_action>
  </error>

  <error id="ERR-EVAL-07" http_code="404" req_ref="REQ-EVAL-08">
    <condition>`mcp__cgreality__mejepa_eval_ship_gate_status()` invoked when no eval report has been persisted.</condition>
    <user_message>No eval report available yet. Run an eval first via `mcp__cgreality__mejepa_eval_run`.</user_message>
    <internal_message>MEJEPA_EVAL_NO_REPORT_AVAILABLE: cf={CF_MEJEPA_EVAL_REPORTS}, count=0</internal_message>
    <recovery_action>Operator: run `mejepa eval-run --window-days 7`; or wait for nightly scheduler.</recovery_action>
  </error>

  <error id="ERR-EVAL-08" http_code="500" req_ref="REQ-EVAL-07">
    <condition>`build_patch_similarity_graph` failed mid-build (e.g., E_Diff embedder unavailable).</condition>
    <user_message>Patch similarity graph build failed; previous graph version remains active for training samplers.</user_message>
    <internal_message>MEJEPA_EVAL_TASK_GRAPH_REBUILD_FAIL: corpus_sha={...}, failed_at_task={...}, reason={...}</internal_message>
    <recovery_action>Operator: investigate embedder health; re-run `mejepa eval-build-graph --force`.</recovery_action>
  </error>

  <error id="ERR-EVAL-09" http_code="200" req_ref="REQ-EVAL-06">
    <condition>(Info-level) Active learning corpus growth this week below 1 % target.</condition>
    <user_message>Active learning loop added only {growth:.2}% corpus this week (target 1.0%); consider pushing more queue items for labeling.</user_message>
    <internal_message>MEJEPA_EVAL_ACTIVE_LEARNING_BELOW_TARGET: growth_pct={growth}, target_pct={1.0}, queue_depth={depth}</internal_message>
    <recovery_action>Operator: label top-N entries from active-learning queue via `mcp__cgreality__mejepa_active_learning_label`.</recovery_action>
  </error>

  <error id="ERR-EVAL-10" http_code="422" req_ref="REQ-EVAL-01">
    <condition>Holdout split is empty (SPEC-CORPUS not run yet).</condition>
    <user_message>Holdout split is empty; cannot run eval. Run `mejepa-corpus generate --split` first.</user_message>
    <internal_message>MEJEPA_EVAL_EMPTY_HOLDOUT: holdout_dir={...}, count=0</internal_message>
    <recovery_action>Operator: run Phase 0 corpus generation per SPEC-CORPUS.</recovery_action>
  </error>

</error_states>

<data_requirements>

  <entity name="EvalReport">
    <field name="report_date" type="NaiveDate" required="true">
      <description>Report's date in ISO 8601 (YYYY-MM-DD); the canonical filename suffix.</description>
    </field>
    <field name="corpus_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the active corpus when the eval ran (per `<provenance>` rule).</description>
    </field>
    <field name="model_weights_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the active predictor weights at eval time.</description>
    </field>
    <field name="calibration_version" type="String" required="true">
      <description>Active conformal calibration record id (per SPEC-INFERENCE CF_MEJEPA_CALIBRATION_HISTORY).</description>
    </field>
    <field name="constellation_version" type="String" required="true">
      <description>Active TCT constellation id (per SPEC-TCT CF_MEJEPA_CONSTELLATION).</description>
    </field>
    <field name="code_version" type="String" required="true">
      <description>Repo SHA + workspace tag (`git describe --always --dirty`).</description>
    </field>
    <field name="holdout_split_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the holdout task list at eval time; pins the split.</description>
    </field>
    <field name="rolling_window_size" type="usize" required="true">
      <description>Number of (predicted, actual) pairs evaluated in the report.</description>
    </field>
    <field name="overall_correlation" type="f32" required="true">
      <description>Pearson correlation between `predicted_oracle_pass` and `actual_docker_oracle_pass` over the rolling-window holdout.</description>
      <constraints>Finite; in [-1, 1].</constraints>
    </field>
    <field name="per_category" type="HashMap&lt;MutationCategory, f32&gt;" required="true">
      <description>Correlation per mutation category. Eight entries: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError.</description>
    </field>
    <field name="per_language" type="HashMap&lt;Language, Option&lt;f32&gt;&gt;" required="true">
      <description>Correlation per language. Eleven entries; Go and PHP marked Option::None until corpus catches up.</description>
    </field>
    <field name="per_cell" type="Option&lt;HashMap&lt;(MutationCategory, Language), Option&lt;f32&gt;&gt;&gt;" required="false">
      <description>Per-cell correlation when `include_per_cell: true`; up to 88 entries (8 × 11). None entries on insufficient samples.</description>
    </field>
    <field name="ship_gate_passed" type="bool" required="true">
      <description>True iff all 8 ship-gate conditions (REQ-EVAL-03 i–viii) pass.</description>
    </field>
    <field name="ship_gate_failures" type="Vec&lt;String&gt;" required="true">
      <description>SCREAMING_SNAKE-tagged failure descriptions (e.g., "MEJEPA_EVAL_SHIP_GATE_REGRESSION (compile_error, c) ...").</description>
    </field>
    <field name="ood_calibration_health" type="HashMap&lt;Language, f32&gt;" required="true">
      <description>OOD ROC AUC per language; ≥ 0.85 required for ship gate.</description>
    </field>
    <field name="conformal_coverage_health" type="HashMap&lt;Language, ConformalHealthEntry&gt;" required="true">
      <description>Per-language stated vs empirical coverage; within ±2 % required.</description>
    </field>
    <field name="gtau_pass_rate" type="HashMap&lt;Language, f32&gt;" required="true">
      <description>Per-language fraction of holdout panels accepted by SPEC-TCT Gτ guard.</description>
    </field>
    <field name="q1_pass_rate" type="f32" required="true">
      <description>Q1: fraction of holdout patches passing source-SHA-drift + witness-chain integrity.</description>
    </field>
    <field name="q2_correlation" type="f32" required="true">
      <description>Q2: identical to overall_correlation (the primary phase-8 metric).</description>
    </field>
    <field name="q3_side_effect_agreement" type="Option&lt;f32&gt;" required="false">
      <description>Q3: F1 score between predicted and observed side-effect sets; None when SPEC-SHIFT-SUBSCRIBER not yet shipped.</description>
    </field>
    <field name="active_learning" type="ActiveLearningSummary" required="true">
      <description>Queue depth, labels added this week, corpus growth %.</description>
    </field>
    <field name="drift_severity_history" type="Vec&lt;DriftSeverity&gt;" required="true">
      <description>Last 4 weeks of drift severity (healthy | soft | hard | critical).</description>
    </field>
    <field name="model_promotion_history" type="Vec&lt;ModelPromotion&gt;" required="true">
      <description>Rolling 12 months of model promotions (Mode A/B/C winners) and their weights_sha.</description>
    </field>
    <field name="state_transfer_diagnostic" type="Option&lt;StateTransferDiagnostic&gt;" required="false">
      <description>d_train_deploy, λ, T, M_t_estimate, attribution. Per doc 11 §8. None when CF_MEJEPA_LIVE_PREDICTIONS has &lt; 100 entries.</description>
    </field>
    <field name="aux_head_distillation" type="Option&lt;AuxHeadDistillationSummary&gt;" required="false">
      <description>Per doc 09 §3.7.6 distillation health; None when SPEC-TRAINING has not yet distilled.</description>
    </field>
    <field name="open_research_questions" type="Vec&lt;OpenResearchQuestionStatus&gt;" required="true">
      <description>Per doc 09 §8 questions 1–8 with mitigation_state and evidence_pointers.</description>
    </field>
    <field name="per_cell_warnings" type="Vec&lt;String&gt;" required="true">
      <description>Cells with insufficient samples or shrinkage applied.</description>
    </field>
    <field name="shrinkage_applied" type="Vec&lt;String&gt;" required="true">
      <description>Cells where Bayesian shrinkage was applied (per REQ-EVAL-20).</description>
    </field>
    <field name="split_mode" type="SplitMode" required="true">
      <description>RepoAtomic (default) or InstanceAtomic per SPEC-CORPUS.</description>
    </field>
    <field name="latency_budget_exceeded" type="bool" required="true">
      <description>True when wall-clock exceeded 30-minute p50 budget per REQ-EVAL-16.</description>
    </field>
    <field name="ship_gate_thresholds" type="ShipGateThresholds" required="true">
      <description>Audit record of the active thresholds (overall 0.95, per-cell 0.90, OOD AUC 0.85, conformal ±2 %).</description>
    </field>
  </entity>

  <entity name="ConformalHealthEntry">
    <field name="stated" type="f32" required="true">
      <description>Stated coverage = 1 − α; default 0.90.</description>
    </field>
    <field name="empirical" type="f32" required="true">
      <description>Empirical coverage measured on holdout for this language.</description>
    </field>
    <field name="delta_pct" type="f32" required="true">
      <description>(empirical − stated) × 100; ±2 % tolerated.</description>
    </field>
    <field name="num_samples" type="usize" required="true">
      <description>Number of holdout samples in this language stratum.</description>
    </field>
  </entity>

  <entity name="ActiveLearningSummary">
    <field name="queue_depth" type="usize" required="true">
      <description>Current count of pending uncertain-but-in-distribution panels.</description>
    </field>
    <field name="labels_added_this_week" type="usize" required="true">
      <description>Count of (panel, oracle_outcome) gold pairs added in the rolling 7-day window.</description>
    </field>
    <field name="corpus_growth_pct" type="f32" required="true">
      <description>labels_added / total_corpus × 100; ≥ 1.0 % is the Phase 8 DoD target.</description>
    </field>
    <field name="below_target" type="bool" required="true">
      <description>True iff corpus_growth_pct &lt; 1.0.</description>
    </field>
    <field name="ood_escalations_pending" type="usize" required="true">
      <description>OOD-flagged panels routed to operator (not active learning) per BR-EVAL-03.</description>
    </field>
    <field name="label_method_distribution" type="HashMap&lt;LabelMethod, usize&gt;" required="true">
      <description>Counts per label method: Manual / DockerRerun / AdjacentTestRerun.</description>
    </field>
  </entity>

  <entity name="DriftSeverity">
    <field name="week" type="NaiveDate" required="true">
      <description>Week's report date.</description>
    </field>
    <field name="severity" type="enum" required="true">
      <description>One of {Healthy, Soft, Hard, Critical} per SPEC-SELF-HEALING drift levels.</description>
    </field>
  </entity>

  <entity name="ModelPromotion">
    <field name="date" type="NaiveDate" required="true"/>
    <field name="mode_winner" type="enum" required="true">
      <description>One of {A (no retrain), B (targeted retrain), C (ensemble)}.</description>
    </field>
    <field name="weights_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the promoted weights.</description>
    </field>
    <field name="calibration_version" type="String" required="true"/>
    <field name="overall_correlation" type="f32" required="true">
      <description>Correlation at promotion time.</description>
    </field>
    <field name="regression_triggered" type="bool" required="true">
      <description>True when promotion was triggered by a cell regression (BR-EVAL-05).</description>
    </field>
  </entity>

  <entity name="StateTransferDiagnostic">
    <field name="d_train_deploy" type="f32" required="true">
      <description>Wasserstein-1 distance between holdout panel embeddings and live-attempt panel embeddings.</description>
    </field>
    <field name="lambda" type="f32" required="true">
      <description>Transfer-bandwidth constant; default 1.0.</description>
    </field>
    <field name="t_value" type="f32" required="true">
      <description>T = exp(-d / lambda) per doc 11 §8.</description>
    </field>
    <field name="m_t_estimate" type="f32" required="true">
      <description>Geometric mean of ΔΩ × ΔΞ over last 1,000 training certs.</description>
    </field>
    <field name="performance_deploy_predicted" type="f32" required="true">
      <description>M(t) · T per doc 11 §8.</description>
    </field>
    <field name="attribution" type="String" required="true">
      <description>One of "balanced" | "training_quality_dominated (M low)" | "distribution_shift_dominated (T low)" | "both low" | "insufficient_live_data".</description>
    </field>
  </entity>

  <entity name="AuxHeadDistillationSummary">
    <field name="last_distillation_step" type="u64" required="true"/>
    <field name="distillation_loss" type="f32" required="true"/>
    <field name="aux_heads_in_consensus" type="Vec&lt;String&gt;" required="true">
      <description>Aux heads whose predictions agree with trunk within threshold.</description>
    </field>
    <field name="aux_heads_diverged" type="Vec&lt;String&gt;" required="true">
      <description>Aux heads whose predictions diverge from trunk by more than threshold (default 0.20).</description>
    </field>
  </entity>

  <entity name="OpenResearchQuestionStatus">
    <field name="question_id" type="enum" required="true">
      <description>One of Q1..Q8 per doc 09 §8.</description>
    </field>
    <field name="question_text" type="String" required="true"/>
    <field name="mitigation_state" type="enum" required="true">
      <description>One of {Active, Resolved, Deferred}.</description>
    </field>
    <field name="evidence_pointers" type="Vec&lt;String&gt;" required="true">
      <description>Paths to FSV evidence supporting the resolution / mitigation.</description>
    </field>
  </entity>

  <entity name="ShipGateStatus">
    <field name="passed" type="bool" required="true"/>
    <field name="per_category" type="HashMap&lt;MutationCategory, f32&gt;" required="true"/>
    <field name="per_language" type="HashMap&lt;Language, Option&lt;f32&gt;&gt;" required="true"/>
    <field name="blockers" type="Vec&lt;String&gt;" required="true">
      <description>Same as `EvalReport.ship_gate_failures` (subset surface for the MCP tool).</description>
    </field>
    <field name="report_date" type="NaiveDate" required="true">
      <description>The eval report this status is read from.</description>
    </field>
  </entity>

  <entity name="ShipGateThresholds">
    <field name="overall_min" type="f32" required="true" /> <!-- 0.95 -->
    <field name="per_category_min" type="f32" required="true" /> <!-- 0.95 -->
    <field name="per_language_min" type="f32" required="true" /> <!-- 0.95 -->
    <field name="per_cell_min" type="f32" required="true" /> <!-- 0.90 -->
    <field name="ood_auc_min" type="f32" required="true" /> <!-- 0.85 -->
    <field name="conformal_band_pct" type="f32" required="true" /> <!-- 2.0 -->
    <field name="q1_min" type="f32" required="true" /> <!-- 0.99 -->
    <field name="q3_min" type="f32" required="true" /> <!-- 0.90 -->
    <field name="weekly_regression_pct" type="f32" required="true" /> <!-- 2.0 -->
  </entity>

  <entity name="ActiveLearningQueueEntry">
    <field name="panel_sha" type="[u8; 32]" required="true">
      <description>SHA-256 of the materialized panel.</description>
    </field>
    <field name="prediction_id" type="u64" required="true">
      <description>Witness chain offset of the source RealityPrediction.</description>
    </field>
    <field name="conformal_set_size" type="usize" required="true"/>
    <field name="ood_score" type="f32" required="true"/>
    <field name="signal_value" type="f32" required="true">
      <description>signal_value = (outcome_set.len() / max_observed) × (1 − ood_score). Higher = more leverage.</description>
    </field>
    <field name="enqueued_at" type="i64" required="true">
      <description>Unix timestamp seconds.</description>
    </field>
    <field name="language" type="Language" required="true"/>
    <field name="suggested_label_method" type="LabelMethod" required="true">
      <description>Heuristic recommendation: Manual | DockerRerun | AdjacentTestRerun.</description>
    </field>
  </entity>

  <entity name="LabelMethod">
    <field name="variant" type="enum" required="true">
      <description>One of {Manual, DockerRerun, AdjacentTestRerun}.</description>
    </field>
  </entity>

  <entity name="PatchGraphEdge">
    <field name="source_task_sha" type="[u8; 32]" required="true"/>
    <field name="target_task_sha" type="[u8; 32]" required="true"/>
    <field name="cosine" type="f32" required="true">
      <description>Cosine of E_Diff embeddings; only persisted when &gt; 0.7.</description>
    </field>
  </entity>

  <entity name="MutationCategory">
    <field name="variant" type="enum" required="true">
      <description>One of {KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError} per doc 09 §3.1.</description>
    </field>
  </entity>

  <entity name="EvalError">
    <field name="variant" type="enum" required="true">
      <description>One of {EmptyHoldout, HoldoutLeakage{task_id, found_in}, InsufficientSamples{cell, required, observed}, ShipGateRegression{cell, prior, current, delta_pct}, LanguageOodDrift{language, prior_auc, current_auc, delta}, LanguageConformalDrift{language, stated, empirical, delta_pct}, ReportPersistFail{cf, backend_error}, NoReportAvailable, TaskGraphRebuildFail{corpus_sha, failed_at_task, reason}, ActiveLearningBelowTarget{growth_pct, target_pct}, NanDetected{field, cell}}.</description>
    </field>
    <field name="code_method" type="fn" required="true">
      <description>.code() returns SCREAMING_SNAKE per constitution `<failure_taxonomy>` extension.</description>
    </field>
  </entity>

</data_requirements>

<non_functional_requirements>

  <nfr id="NFR-EVAL-01" category="performance">
    <description>`run_holdout_eval` on 240-task holdout split completes in &lt; 30 minutes p50 wall-clock with cached instruments.</description>
    <metric>p50 wall-clock on FSV stress test (3 runs) &lt; 1,800 s.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-02" category="performance">
    <description>`build_patch_similarity_graph` on 2,400-task corpus completes in &lt; 30 minutes.</description>
    <metric>Wall-clock on FSV harness &lt; 1,800 s on RTX 5090.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-03" category="reliability">
    <description>Determinism: identical inputs produce byte-equal `EvalReport` modulo `report_date`.</description>
    <metric>3× repeated `run_holdout_eval` on same fixtures produce SHA-256-equal serialized reports (excluding report_date field).</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-04" category="reliability">
    <description>Holdout split leakage zero tolerance: `MEJEPA_EVAL_HOLDOUT_LEAKAGE` fail-closed always.</description>
    <metric>Integration test injects deliberate leakage; assert eval refuses to run.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-05" category="security">
    <description>FSV evidence files at /tmp/contextgraph-mejepa-evaluation-fsv/ are mode 0o600 per SEC-MEJEPA-07.</description>
    <metric>`stat -c '%a'` on every FSV file == 600.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-06" category="security">
    <description>MCP tool input validation rejects malformed paths, control characters in `mcp__cgreality__mejepa_active_learning_label`.</description>
    <metric>100 % of malformed inputs in fuzz suite rejected with structured errors per SEC-MEJEPA-01.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-07" category="usability">
    <description>Every error variant carries a structured payload (cell, language, delta_pct) plus a SCREAMING_SNAKE `.code()` tag; logged with input shapes before returning per `<error_handling>`.</description>
    <metric>Code review confirms no silent fallback; no `unwrap_or_default()` on cell metrics.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-08" category="reliability">
    <description>Per-language stratification non-degeneracy: when a language has &lt; 30 samples, all language-conditioned metrics (ood_calibration_health, conformal_coverage_health, gtau_pass_rate) for that language MUST be Option::None, never silently substituted from the aggregate.</description>
    <metric>FSV harness fixtures with one language at N=10 verify all three metrics are None.</metric>
    <priority>must</priority>
  </nfr>

  <nfr id="NFR-EVAL-09" category="storage">
    <description>EvalReport JSON ≤ 1 MiB per report; bincode-serialized to CF_MEJEPA_EVAL_REPORTS for storage efficiency.</description>
    <metric>FSV harness writes a fully-populated 11-language report; assert ≤ 1 MiB.</metric>
    <priority>should</priority>
  </nfr>

  <nfr id="NFR-EVAL-10" category="reliability">
    <description>Provenance binding: every persisted report includes corpus_sha, model_weights_sha, calibration_version, constellation_version, code_version, holdout_split_sha. Reports without complete provenance refused at write boundary.</description>
    <metric>Integration test seeds a missing provenance field; assert write fails with `MEJEPA_EVAL_REPORT_PERSIST_FAIL`.</metric>
    <priority>must</priority>
  </nfr>

</non_functional_requirements>

<dependencies>

  <dependency type="internal">
    <name>SPEC-CORPUS / TECH-CORPUS (Phase 0)</name>
    <description>The 10 % holdout split (repo-atomic via Stoer-Wagner mincut) AND `assert_no_test_patch_leakage` helper.</description>
    <impact>Without holdout split, eval cannot run (`MEJEPA_EVAL_EMPTY_HOLDOUT`).</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-INFERENCE / TECH-INFERENCE (Phase 4a)</name>
    <description>`compile()` and `verify()` plus the conformal calibrator + OOD detector. Eval consumes both for Q1/Q2/Q3 evaluation.</description>
    <impact>Hard dependency. Phase 8 cannot start before Phase 4a ships.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-PREDICTOR / TECH-PREDICTOR (Phase 2)</name>
    <description>The trained predictor whose correlation we evaluate.</description>
    <impact>Hard dependency.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-TRAINING / TECH-TRAINING (Phase 3)</name>
    <description>Per-step training certs (CF_MEJEPA_TRAIN_CERTS) for ΔΩ × ΔΞ in M(t) estimate; plus mini-batch sampler that consumes the patch-similarity graph.</description>
    <impact>State-transfer diagnostic returns Option::None when CF_MEJEPA_TRAIN_CERTS is empty; cross-task transfer requires SPEC-TRAINING to read `mejepa-task-graph` edges.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-TCT / TECH-TCT (Phase 4b)</name>
    <description>Gτ pass rate per language. Eval reads pass rates from CF_MEJEPA_CONSTELLATION_VERDICTS.</description>
    <impact>When SPEC-TCT not shipped, gtau_pass_rate is omitted; ship gate skips that condition.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-SELF-HEALING / TECH-SELF-HEALING (Phase 5)</name>
    <description>Drift severity history + Mode A/B/C promotion records.</description>
    <impact>When SPEC-SELF-HEALING not shipped, drift_severity_history and model_promotion_history are empty arrays; ship gate still works on absolute thresholds.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-SHIFT-SUBSCRIBER (Phase 7)</name>
    <description>Live (predicted, actual) pairs in CF_MEJEPA_LIVE_PREDICTIONS + CF_MEJEPA_ORACLE_VERDICTS; observed side-effect sets for Q3 evaluation.</description>
    <impact>Hard dependency for the live-attempts slice + state-transfer diagnostic + Q3. Without Phase 7, only the static holdout slice is evaluable; live correlation is Option::None.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-WITNESS</name>
    <description>Witness chain integrity replay for Q1 (claim graph ↔ source-of-truth SHA chain).</description>
    <impact>Hard dependency for Q1 metric.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-INSTRUMENTS / TECH-INSTRUMENTS (Phase 1)</name>
    <description>E_Diff slot for patch-similarity graph cosines; E_Oracle slot for actual oracle outcomes.</description>
    <impact>Hard dependency for cross-task transfer + correlation computation.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-EMBEDDERS / TECH-EMBEDDERS (Phase 1)</name>
    <description>21-embedder ensemble; embedder_versions captured in EvalReport provenance.</description>
    <impact>Hard dependency.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-AST-CHUNKING (Phase 1)</name>
    <description>Multi-language AST chunker; per-language eval requires the chunker for each supported language.</description>
    <impact>When a language's chunker is not yet shipped, that language's per_language entry is Option::None.</impact>
  </dependency>

  <dependency type="internal">
    <name>SPEC-HYGIENE (Phase 6)</name>
    <description>Tier compression for retained EvalReports older than 30 days.</description>
    <impact>Phase 6 owns retention; Phase 8 produces.</impact>
  </dependency>

  <dependency type="external">
    <name>swebench/harness/grading.py</name>
    <description>OracleOutcome semantics (FAIL_TO_PASS / PASS_TO_PASS) per upstream evaluator. Eval reads actual oracle verdicts persisted by SPEC-CORPUS Docker bridge.</description>
    <impact>Read-only reference; never modified per `<anti_patterns>` reality-boundary rule.</impact>
  </dependency>

  <dependency type="external">
    <name>candle 0.9.x</name>
    <description>Inference forward pass during per-cell correlation computation; Wasserstein sampling for state-transfer T.</description>
    <impact>Without GPU, fall back to CPU (latency budget breaks; flag latency_budget_exceeded).</impact>
  </dependency>

</dependencies>

<out_of_scope>
  <item>Body of `MeJepaCompiler::observe()` (online learning + EWC++ + drift detection) — handled by SPEC-SELF-HEALING (Phase 5). This spec only consumes the resulting drift_severity_history and model_promotion_history.</item>
  <item>Per-step training certificate generation (`L_step = ΔP × ΔK × ΔΩ × ΔΞ`) — handled by SPEC-TRAINING (Phase 3). This spec only reads ΔΩ × ΔΞ from CF_MEJEPA_TRAIN_CERTS for the M(t) estimate.</item>
  <item>TCT constellation centroid construction + per-cell Gτ logic — handled by SPEC-TCT (Phase 4b). This spec only reads gtau_pass_rate per language.</item>
  <item>Conformal threshold τ + bandwidth σ tuning — handled by SPEC-INFERENCE (Phase 4a). This spec only reads conformal_coverage_health.</item>
  <item>Tier compression of stored EvalReports — handled by SPEC-HYGIENE (Phase 6) per CF_MEJEPA_EVAL_REPORTS retention policy.</item>
  <item>The MCP shift-log subscriber that produces live predictions — handled by SPEC-SHIFT-SUBSCRIBER (Phase 7 §3.8). This spec only consumes CF_MEJEPA_LIVE_PREDICTIONS.</item>
  <item>Mini-batch sampling logic that consumes the patch-similarity graph — handled by SPEC-TRAINING (Phase 3). This spec only constructs the graph.</item>
  <item>Witness-chain entry creation — handled by SPEC-WITNESS; this spec only consumes (replays) chain entries for Q1 evaluation.</item>
  <item>The `MeJepaCompiler::verify()` ladder logic — handled by SPEC-INFERENCE. This spec only reports verify-derived metrics in aggregate.</item>
  <item>Mutation operator generation — handled by SPEC-CORPUS Phase 0. This spec only consumes mutation outcomes.</item>
</out_of_scope>

<test_plan>

  <test_case id="TC-EVAL-01" type="unit" req_ref="REQ-EVAL-01">
    <description>API signature stability: `run_holdout_eval`, `check_ship_gate`, `enqueue_uncertain`, `build_patch_similarity_graph` exist with the documented signatures.</description>
    <preconditions>Crate context-graph-mejepa-eval (or context-graph-mejepa eval module) builds.</preconditions>
    <inputs>None (signature check via type system).</inputs>
    <expected_result>Compile-time assertion via static `fn _assert_signatures()` confirms the impl block.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-02" type="unit" req_ref="REQ-EVAL-02">
    <description>EvalReport NaN / Inf rejection at construction.</description>
    <preconditions>None.</preconditions>
    <inputs>overall_correlation = NaN | Inf | -1.5 | 1.5 | finite valid</inputs>
    <expected_result>Invalid: returns `Err(EvalError::NanDetected { field })`. Valid: returns `Ok(_)`.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-03" type="integration" req_ref="REQ-EVAL-03">
    <description>Ship-gate decision exhaustively exercises all 8 conditions.</description>
    <preconditions>Synthetic EvalReport fixtures: one passing, one failing per condition.</preconditions>
    <inputs>9 fixtures total.</inputs>
    <expected_result>Each fixture produces the expected `ship_gate_passed` value and `ship_gate_failures` content; FSV `ship-gate-evidence.json` writes all 9.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-04" type="integration" req_ref="REQ-EVAL-04">
    <description>Week-over-week regression detection on synthetic prior + current reports.</description>
    <preconditions>Last week's report seeded at cell (subtle_flip, typescript) = 0.94.</preconditions>
    <inputs>This week's eval at same cell = 0.91.</inputs>
    <expected_result>`ship_gate_failures` includes `MEJEPA_EVAL_SHIP_GATE_REGRESSION` with delta_pct=-3.0; FSV `regression-evidence.json` written.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-05" type="integration" req_ref="REQ-EVAL-05">
    <description>Active learning queue accept/reject paths.</description>
    <preconditions>Empty queue.</preconditions>
    <inputs>(panel_A, outcome_set.len=4, ood=0.32) → enqueue; (panel_B, outcome_set.len=4, ood=0.65) → reject; (panel_C, outcome_set.len=2, ood=0.32) → reject (not uncertain).</inputs>
    <expected_result>Only panel_A is in the queue; panel_B routed to OOD escalations; panel_C neither.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-06" type="integration" req_ref="REQ-EVAL-06">
    <description>Active learning corpus growth tracking over a 7-day window.</description>
    <preconditions>5,000-entry corpus; 50 manual labels added during week.</preconditions>
    <inputs>One eval run.</inputs>
    <expected_result>`active_learning.corpus_growth_pct` ≈ 1.0; `below_target: false`.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-EVAL-07" type="integration" req_ref="REQ-EVAL-07">
    <description>Patch-similarity graph build on 50-task subset.</description>
    <preconditions>50-task corpus with computed E_Diff embeddings.</preconditions>
    <inputs>`build_patch_similarity_graph(corpus)`.</inputs>
    <expected_result>Edge count > 0; all edges have cosine > 0.7; top-k per node ≤ 32; FSV `patch-similarity-graph-evidence.json` written.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-08" type="integration" req_ref="REQ-EVAL-08">
    <description>MCP tool round-trip: `mcp__cgreality__mejepa_eval_run` parity with direct in-process call.</description>
    <preconditions>MCP server running; holdout split fixtures.</preconditions>
    <inputs>Same call invoked both ways.</inputs>
    <expected_result>Byte-equal `EvalReport` (modulo report_date).</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-09" type="integration" req_ref="REQ-EVAL-09">
    <description>State-transfer diagnostic on synthetic distributions with known Wasserstein distance.</description>
    <preconditions>Synthetic P_train and P_deploy (Gaussians with known means) loaded into CF_MEJEPA_LIVE_PREDICTIONS.</preconditions>
    <inputs>One eval run.</inputs>
    <expected_result>d_train_deploy matches analytic Wasserstein-1; T = exp(-d / λ); attribution string matches manually-computed regime.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-EVAL-10" type="integration" req_ref="REQ-EVAL-10">
    <description>Per-language OOD AUC drift detection over 4-week trend.</description>
    <preconditions>Seeded prior 4 weeks of reports with `ood_calibration_health[c]` at 0.92, 0.91, 0.88, 0.79.</preconditions>
    <inputs>This week's eval.</inputs>
    <expected_result>`ship_gate_failures` includes `MEJEPA_EVAL_LANGUAGE_OOD_DRIFT`; FSV `ood-evidence.json` written.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-11" type="integration" req_ref="REQ-EVAL-11">
    <description>Three-operator-question rollup on a single in-distribution holdout panel with valid SHAs + witness chain + observed shift-log entry.</description>
    <preconditions>One synthetic panel + corresponding shift-log entry.</preconditions>
    <inputs>Eval runs Q1, Q2, Q3 on the panel.</inputs>
    <expected_result>q1_pass_rate = 1.0 (this panel passes); q3_side_effect_agreement = 1.0; FSV `q1q2q3-evidence.json` written.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-12" type="unit" req_ref="REQ-EVAL-13 ERR-EVAL-01..ERR-EVAL-10">
    <description>Each EvalError variant carries SCREAMING_SNAKE `.code()` and structured payload.</description>
    <preconditions>None.</preconditions>
    <inputs>One instance per error variant.</inputs>
    <expected_result>`.code()` matches the canonical tag; payload populated; logged with input shape.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-13" type="integration" req_ref="REQ-EVAL-14">
    <description>Holdout leakage detection: deliberately inject a duplicate task into both train and holdout.</description>
    <preconditions>Modified split fixture with leakage.</preconditions>
    <inputs>`run_holdout_eval`.</inputs>
    <expected_result>Returns `Err(MejepaEvalError::HoldoutLeakage)`; no report produced; CRITICAL blocker logged.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-14" type="e2e" req_ref="REQ-EVAL-15">
    <description>Phase 8 DoD verification command end-to-end.</description>
    <preconditions>Full Phase 0 corpus + Phase 3 trained checkpoint + Phase 4a calibration.</preconditions>
    <inputs>`cargo run --release -p context-graph-mejepa -- eval-run --window-days 7 --include-per-cell --output ./data/mejepa-eval/`.</inputs>
    <expected_result>Exit 0 on ship gate pass; exit 1 on gate fail; report JSON persisted at expected path; FSV evidence at /tmp/contextgraph-mejepa-evaluation-fsv/ captures stdout + JSON + exit code.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-15" type="integration" req_ref="REQ-EVAL-17">
    <description>Determinism: 3 sequential `run_holdout_eval` calls on same fixtures return byte-equal serialized reports modulo report_date.</description>
    <preconditions>Frozen weights; pinned calibration / constellation versions; pinned holdout split.</preconditions>
    <inputs>3× same configuration.</inputs>
    <expected_result>SHA-256 of serialized report (excluding report_date field) equal across all 3.</expected_result>
    <priority>critical</priority>
  </test_case>

  <test_case id="TC-EVAL-16" type="manual" req_ref="NFR-EVAL-05">
    <description>FSV evidence files mode 0o600.</description>
    <preconditions>Run FSV harness.</preconditions>
    <inputs>None.</inputs>
    <expected_result>`stat -c '%a'` returns 600 for every file in /tmp/contextgraph-mejepa-evaluation-fsv/.</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-EVAL-17" type="integration" req_ref="REQ-EVAL-20">
    <description>Bayesian shrinkage for under-sampled cells with N=12.</description>
    <preconditions>One cell at N=12; `enable_bayesian_shrinkage: true`.</preconditions>
    <inputs>Eval run.</inputs>
    <expected_result>Cell value computed via shrinkage to language-aggregate; `shrinkage_applied` lists the cell; ship gate ignores the shrunk cell when computing per-cell threshold.</expected_result>
    <priority>high</priority>
  </test_case>

  <test_case id="TC-EVAL-18" type="integration" req_ref="REQ-EVAL-19">
    <description>Open research question tracking through eval reports.</description>
    <preconditions>Initial 8 questions seeded in mitigation_state=Active.</preconditions>
    <inputs>Operator marks Q1 Resolved with evidence pointer.</inputs>
    <expected_result>Next eval report carries Q1 as Resolved with the evidence pointer; FSV `open-research-questions-evidence.json` written.</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-EVAL-19" type="integration" req_ref="REQ-EVAL-12">
    <description>Aux-head distillation health on synthetic aux outputs with known divergence.</description>
    <preconditions>SPEC-TRAINING distillation stub with synthetic aux outputs.</preconditions>
    <inputs>Eval run.</inputs>
    <expected_result>`aux_head_distillation.aux_heads_diverged` lists the synthetically-divergent aux; FSV `aux-head-distillation-evidence.json` written.</expected_result>
    <priority>medium</priority>
  </test_case>

  <test_case id="TC-EVAL-20" type="integration" req_ref="NFR-EVAL-08">
    <description>Per-language stratification non-degeneracy: language with N=10 yields all-None metrics.</description>
    <preconditions>Synthetic split with go-language at N=10.</preconditions>
    <inputs>Eval run.</inputs>
    <expected_result>`per_language[go] = None`; `ood_calibration_health` omits go; `gtau_pass_rate` omits go; report flags `unsupported_languages: ["go"]`.</expected_result>
    <priority>critical</priority>
  </test_case>

</test_plan>

<open_questions>

  <question id="Q-EVAL-01" status="open" assignee="operator">
    <text>Should the ship gate's per-cell threshold be 0.90 (looser than per-aggregate 0.95) or stricter (0.92)?</text>
    <context>Per-cell estimators have higher variance with smaller sample size (typically 30 samples per cell). 0.90 may be too loose to be meaningful; 0.92 may penalize cells that are statistically indistinguishable from 0.95.</context>
    <resolution>Default 0.90 with operator override via `ShipGateThresholds.per_cell_min`; recorded in EvalReport for audit. Tighten only after Phase 8 has run for ≥ 4 weeks and per-cell variance has been measured empirically.</resolution>
  </question>

  <question id="Q-EVAL-02" status="open" assignee="operator">
    <text>Should the active-learning queue auto-route low-confidence panels to Docker oracle re-runs (option b in doc 09 §3.7.4) without operator intervention, or always require operator approval?</text>
    <context>Auto-routing accelerates corpus growth but consumes Docker resources without operator visibility. Manual gating is safer but slower.</context>
    <resolution>Default: queue items get a `suggested_label_method` heuristic; operator manually selects via `mcp__cgreality__mejepa_active_learning_label`. Auto-route is opt-in via `auto_route_docker_rerun: bool` config; when enabled, only the top-N by signal_value (default 5/day) auto-route.</resolution>
  </question>

  <question id="Q-EVAL-03" status="open" assignee="operator">
    <text>What is the right transfer-bandwidth λ for state-transfer T per doc 11 §8?</text>
    <context>doc 11 §8 specifies `T = exp(-d / λ)` but does not pin λ. Too small → T saturates near 0 quickly; too large → T saturates near 1 quickly.</context>
    <resolution>Default λ = 1.0; recorded in EvalReport.state_transfer_diagnostic.lambda for audit. Tune empirically once Phase 7 produces ≥ 1,000 live predictions and the relationship between observed performance_deploy and predicted M(t) · T can be regressed.</resolution>
  </question>

  <question id="Q-EVAL-04" status="open" assignee="operator">
    <text>Should the 8 mutation categories ever be expanded (per doc 09 §8 question 4) and how does the eval report handle the addition?</text>
    <context>Adding categories iteratively is the recommended mitigation for "is mutation corpus diverse enough?". Each new category extends `MutationCategory` enum and adds a row to `per_category` in the report.</context>
    <resolution>When a new category ships, it appears in subsequent reports as a new entry with `mitigation_state: Active`; the ship gate threshold (0.95) applies once the category has ≥ 30 samples; before that it is `None`.</resolution>
  </question>

  <question id="Q-EVAL-05" status="open" assignee="operator">
    <text>Should the weekly report include cross-task transfer effectiveness (doc 09 §3.7.5) as a separate metric?</text>
    <context>Cross-task transfer is mostly a SPEC-TRAINING concern (the sampler), but its effectiveness can be measured by comparing per-cell correlation on cells with high vs low task-graph density.</context>
    <resolution>Phase 8 ships without explicit cross-task transfer effectiveness metric; if a cell with high task-graph density consistently underperforms a similarly-sampled low-density cell, the operator can investigate manually. Future work: add `cross_task_transfer_effectiveness: HashMap<MutationCategory, f32>` to the report.</resolution>
  </question>

  <question id="Q-EVAL-06" status="open" assignee="operator">
    <text>Should `mcp__cgreality__mejepa_eval_ship_gate_status()` return per-(language, category) cell or only per-language and per-category aggregates?</text>
    <context>Cell granularity is detailed but blows up the response size (88 entries vs 19). Aggregates suffice for ship/no-ship decisions but obscure the cell-level diagnosis.</context>
    <resolution>Default: aggregates only (per_category + per_language + blockers). Operator may pass `include_per_cell: true` to receive the full cell matrix; cell matrix is paged at 32 cells per response if needed.</resolution>
  </question>

</open_questions>

<glossary>
  <term name="EvalReport">The canonical weekly artifact: `mejepa-weekly-report-YYYY-MM-DD.json`. Per REQ-EVAL-02 it carries the entire Phase 8 metric surface.</term>
  <term name="ShipGateStatus">A subset of EvalReport returned by `mcp__cgreality__mejepa_eval_ship_gate_status()` for agents who need only pass/fail + per-aggregate detail.</term>
  <term name="prediction_oracle_correlation">Pearson correlation between `predicted_oracle_pass` (from SPEC-INFERENCE compile()) and `actual_docker_oracle_pass` (from SPEC-CORPUS Docker oracle bridge). The primary phase-8 metric per doc 09 §7.</term>
  <term name="Per-cell">A (mutation_category × language) cell: 8 × 11 = 88 cells (9 supported today). Each cell holds a correlation. Gates at 0.90 (looser than aggregates' 0.95).</term>
  <term name="Holdout split">10 % of Phase 0 corpus reserved per SPEC-CORPUS's repo-atomic Stoer-Wagner mincut. No tasks shared with train split.</term>
  <term name="Calibration split">10 % of Phase 0 corpus, used by SPEC-INFERENCE for conformal threshold τ + OOD bandwidth σ². Distinct from the holdout split.</term>
  <term name="Train split">80 % of Phase 0 corpus.</term>
  <term name="Mutation category">One of 8 doc-09 §3.1 categories: KnownGood, SubtleFlip, OffByOne, SwapVariable, DeleteTestCall, WrongFile, OverEngineer, CompileError.</term>
  <term name="Active learning">The doc 09 §3.7.4 loop: surface uncertain-but-in-distribution panels to operator (a) manual label, (b) Docker oracle re-run, (c) adjacent-test re-run. Adds ≥ 1 % corpus per week.</term>
  <term name="signal_value">Active-learning queue ranking score: `(outcome_set.len() / max_observed) × (1 − ood_score)`. Higher = more leverage from labeling.</term>
  <term name="Cross-task transfer">Per doc 09 §3.7.5: in mini-batch sampler, with prob 0.3 sample from tasks within cosine > 0.7 of T's patch. Implemented via `mejepa-task-graph` edges.</term>
  <term name="Patch-similarity graph">AST-diff cosine over corpus, persisted to CF_MEJEPA_TASK_GRAPH. Built once per corpus_sha.</term>
  <term name="Aux-head distillation">Per doc 09 §3.7.6: every 1,000 steps, distill aux-head predictions into the predictor's hidden state. Prevents detached specialists.</term>
  <term name="State-transfer T">Per doc 11 §8: `T(train, deploy) = exp(-d / λ)`. The deployment-distribution-shift factor that, multiplied by M(t), predicts deploy performance.</term>
  <term name="M(t)">Per doc 11: the consolidation tensor's training-quality factor. Geometric mean of ΔΩ × ΔΞ over recent training certs.</term>
  <term name="d_train_deploy">Wasserstein-1 distance between holdout panel embeddings and live-attempt panel embeddings. Per doc 11 §8.</term>
  <term name="Q1 / Q2 / Q3">Three operator questions per constitution mission and doc 09 §1.3: Q1 = does what the AI claimed exist (source-SHA + witness-chain integrity); Q2 = does it work (predicted ↔ actual oracle correlation); Q3 = how does it impact reality (predicted side-effect set ↔ observed shifts F1).</term>
  <term name="Bayesian shrinkage">Per constitution `<tct_invariants>` rule 5: under-sampled cells (N < 30) shrink to language-aggregate or category-aggregate centroid.</term>
  <term name="degraded_status">Per SPEC-INFERENCE, set true on RealityPrediction when ΔΩ × ΔΞ multiplicative gating triggers. Eval surfaces fraction of degraded predictions in the rolling window.</term>
  <term name="Drift severity">Per SPEC-SELF-HEALING: enum {Healthy, Soft, Hard, Critical}. Eval report carries last 4 weeks for trend visualization.</term>
  <term name="ModelPromotion">Per SPEC-SELF-HEALING: a record of A/B/C retrain mode promotion + weights_sha + calibration version. Eval report carries rolling 12 months for audit.</term>
  <term name="Mode A/B/C">Per SPEC-SELF-HEALING + doc 09 §5.3: Mode A = no retrain (current model); Mode B = targeted retrain; Mode C = ensemble of A + B.</term>
  <term name="Stoer-Wagner mincut">The graph algorithm SPEC-CORPUS uses for repo-atomic split. Shipped clean-room in context-graph-mincut.</term>
  <term name="Open research questions">Doc 09 §8: 8 numbered uncertainties (JEPA generalization, panel dim, per-language vs unified, corpus diversity, novel APIs, λ_ewc, non-SWE-bench, self-disagreement). Tracked in EvalReport.open_research_questions over time.</term>
</glossary>

</functional_spec>
```
