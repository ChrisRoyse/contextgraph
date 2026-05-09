# TASK-TRAIN-011 — HoldoutEvaluator (every 1000 steps): agreement + conformal coverage + per-category + per-language + predictor_redundancy_pairwise_mi + gtau_pass_rate + best.safetensors promotion + regression detector

```xml
<task_spec id="TASK-TRAIN-011" version="1.0">

<metadata>
  <title>HoldoutEvaluator::evaluate runs the 240-example holdout split (Stoer-Wagner mincut), computes 7-field HoldoutReport, persists to CF_MEJEPA_HOLDOUT_REPORTS, promotes best.safetensors when agreement improves by ≥ 0.005, emits MEJEPA_TRAIN_HOLDOUT_REGRESSION when agreement drops by &gt; 0.02; surfaces generic_only_warning on per-category/per-language gap &gt; 0.10</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>11</sequence>
  <implements>
    <item ref="REQ-TRAIN-05">Holdout evaluation cadence + report schema</item>
    <item ref="AC-TRAIN-05-01..06">240-example split, 7 fields, ≥0.005 promotion, &gt;0.02 regression, ≥0.75 DoD, &gt;0.10 slice gap warning</item>
    <item ref="BR-TRAIN-05">Holdout cadence + best.safetensors atomic promotion</item>
    <item ref="EC-TRAIN-09">Per-language gap warning</item>
    <item ref="EC-TRAIN-10">Final agreement &lt; 0.75 → Phase 3 fail</item>
    <item ref="ERR-TRAIN-06">MEJEPA_TRAIN_HOLDOUT_REGRESSION</item>
    <item ref="component:HoldoutEvaluator">crates/context-graph-mejepa-train/src/eval/holdout.rs</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-001</task_ref>
    <task_ref>TASK-TRAIN-002</task_ref>
    <task_ref>../utml-learning-formula/TASK-UTML-008</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_files>3 created</estimated_files>
</metadata>

<context>
HoldoutEvaluator runs every 1,000 training steps on the 10% holdout split (Stoer-Wagner mincut from `crates/context-graph-mincut`). It produces a 7-field HoldoutReport (agreement, conformal_coverage_calibration, per_mutation_category_accuracy, per_language_accuracy, predictor_redundancy_pairwise_mi, gtau_pass_rate, generic_only_warning) persisted to CF_MEJEPA_HOLDOUT_REPORTS. The promotion logic atomically replaces best.safetensors when agreement improves ≥ 0.005; regression by &gt; 0.02 emits MEJEPA_TRAIN_HOLDOUT_REGRESSION. Per-category or per-language gap &gt; 0.10 surfaces a GENERIC-ONLY warning. Final-step report sets phase3_dod_passed = (agreement ≥ 0.75).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (anti_patterns "no per-task overfit"; performance_budgets phase3_ship_gate; failure_taxonomy MEJEPA_TRAIN_HOLDOUT_REGRESSION)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-05; AC-TRAIN-05-01..06; BR-TRAIN-05; EC-TRAIN-09..10; ERR-TRAIN-06; data_requirements HoldoutReport)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (architecture_diagram eval_block; component_contracts trainer.evaluate_holdout step 1-7)</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (HoldoutEvaluator + HoldoutReport + MutationCategory + Lang); TASK-TRAIN-002 (TrainerError); ../utml-learning-formula/TASK-UTML-008 (pairwise_mi_audit for predictor_redundancy)</file>
  <file purpose="shipped_code">crates/context-graph-mincut (Stoer-Wagner); crates/context-graph-embeddings/src/training/evaluation.rs (MRR/AUC/F1 reused if needed)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-002 + UTML-008 complete</check>
  <check>shipped Stoer-Wagner mincut accessible via context-graph-mincut</check>
  <check>shipped evaluation.rs primitives accessible</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/eval/holdout.rs` with:
      - `pub trait OracleHead { fn predict(&amp;self, predicted_latent: &amp;Tensor) -&gt; Result&lt;OraclePrediction, TrainerError&gt;; }`
      - `pub struct OraclePrediction { pub pass_to_pass_resolved: bool, pub conformal_set: Vec&lt;OracleClass&gt;, pub class_logits: Vec&lt;f32&gt; }`
      - `pub struct HoldoutExample { pub task_id: String, pub category: MutationCategory, pub language: Lang, pub panel_t01: Tensor, pub panel_t2: Tensor, pub actual_oracle_pass: bool }`
      - `pub struct HoldoutDataset { pub examples: Vec&lt;HoldoutExample&gt; }`
      - `pub struct CalibrationDataset { pub examples: Vec&lt;HoldoutExample&gt; }`
      - `pub fn split_holdout_via_mincut(corpus_examples: Vec&lt;HoldoutExample&gt;) -&gt; Result&lt;(TrainSplit, CalibrationDataset, HoldoutDataset), TrainerError&gt;` — wraps context-graph-mincut Stoer-Wagner per SPEC-CORPUS
    - Create `src/eval/slicing.rs` with:
      - `pub fn per_mutation_category_accuracy(predictions: &amp;[(MutationCategory, bool, bool)]) -&gt; HashMap&lt;MutationCategory, f32&gt;` (predicted vs actual pairs)
      - `pub fn per_language_accuracy(predictions: &amp;[(Lang, bool, bool)]) -&gt; HashMap&lt;Lang, f32&gt;`
      - `pub fn detect_generic_only_warning(per_cat: &amp;HashMap&lt;MutationCategory, f32&gt;, per_lang: &amp;HashMap&lt;Lang, f32&gt;, gap_threshold: f32) -&gt; Option&lt;String&gt;` (returns "category:KnownGood-vs-CompileError gap=0.12" etc. if max-min &gt; 0.10)
    - Create `src/eval/regression.rs` with:
      - `pub fn promote_best(checkpoint_dir: &amp;Path, current_step: u64, current_agreement: f32, best_so_far: f32, promotion_threshold: f32) -&gt; Result&lt;Option&lt;PathBuf&gt;, TrainerError&gt;` (atomic write-tmp + rename of best.safetensors)
      - `pub fn detect_regression(current_agreement: f32, best_so_far: f32, regression_threshold: f32) -&gt; Option&lt;TrainerError&gt;` (returns Some(MejepaTrainHoldoutRegression) if drop &gt; threshold)
      - `pub fn track_consecutive_regressions(history: &amp;[bool], window: usize) -&gt; bool` (true if last `window` entries all regressed → halt trigger)
    - Implement `HoldoutEvaluator::evaluate(predictor, aux_oracle_head, dataset, calibration, constellation, current_step) -&gt; Result&lt;HoldoutReport, TrainerError&gt;`:
      1. Iterate dataset; for each: predicted_latent = predictor.forward(panel_t01); predicted_oracle_pass = oracle_head.predict(predicted_latent).pass_to_pass_resolved
      2. Track agreement, per_category, per_language buckets, conformal hits on calibration set
      3. Compute predictor_redundancy via pairwise_mi_audit on holdout panels (REUSE TASK-UTML-008)
      4. gtau_pass_rate = if constellation.is_some() { Phase 4b call } else { 1.0 }
      5. detect generic_only_warning
      6. Construct HoldoutReport
      7. Persist to CF_MEJEPA_HOLDOUT_REPORTS keyed by current_step.to_be_bytes()
    - Implement `HoldoutEvaluator::check_promotion_and_regression(...) -&gt; PromotionDecision`
    - Unit tests with synthetic 240-example dataset: agreement computed correctly; per-category buckets exhaustive; per-language buckets cover all 11 languages or empty when absent; gap &gt; 0.10 produces warning string; promotion threshold; regression threshold; 3-consecutive-regression halt trigger
  </in_scope>
  <out_of_scope>
    - Real predictor + aux oracle head (Phase 2 dep)
    - Real Stoer-Wagner integration (uses shipped crate; this task wraps it)
    - Phase 5 self-healing intervention orchestration
    - Conformal calibration α-threshold computation (Phase 4)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/eval/holdout.rs">
use std::sync::Arc;
use std::path::Path;
use std::collections::HashMap;
use rocksdb::DB;
use candle_core::Tensor;
use crate::eval::{HoldoutReport, MutationCategory, Lang, HoldoutEvaluator};
use crate::error::TrainerError;
use crate::sampler::PatchSimilarityGraph;
// pub use crate::utml::pairwise_mi_audit; // re-export for convenience

pub trait PredictorForward {
    fn forward(&amp;self, panel_t01: &amp;Tensor) -&gt; Result&lt;Tensor, TrainerError&gt;;
}

pub struct OraclePrediction {
    pub pass_to_pass_resolved: bool,
    pub conformal_set: Vec&lt;OracleClass&gt;,
    pub class_logits: Vec&lt;f32&gt;,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleClass { Pass, Fail }

pub trait OracleHead {
    fn predict(&amp;self, predicted_latent: &amp;Tensor) -&gt; Result&lt;OraclePrediction, TrainerError&gt;;
}

pub struct HoldoutExample {
    pub task_id: String,
    pub category: MutationCategory,
    pub language: Lang,
    pub panel_t01: Tensor,
    pub panel_t2: Tensor,
    pub actual_oracle_pass: bool,
}

pub struct HoldoutDataset { pub examples: Vec&lt;HoldoutExample&gt; }
pub struct CalibrationDataset { pub examples: Vec&lt;HoldoutExample&gt; }
pub struct TrainSplit { pub examples: Vec&lt;HoldoutExample&gt; }

pub fn split_holdout_via_mincut(
    corpus_examples: Vec&lt;HoldoutExample&gt;,
) -&gt; Result&lt;(TrainSplit, CalibrationDataset, HoldoutDataset), TrainerError&gt;;

impl HoldoutEvaluator {
    pub fn new(rocksdb: Arc&lt;DB&gt;, cf_name: String) -&gt; Self;
    pub fn evaluate(
        &amp;self,
        predictor: &amp;dyn PredictorForward,
        oracle_head: &amp;dyn OracleHead,
        dataset: &amp;HoldoutDataset,
        calibration: &amp;CalibrationDataset,
        current_step: u64,
        is_final_step: bool,
        phase3_dod_min: f32,
    ) -&gt; Result&lt;HoldoutReport, TrainerError&gt;;
}
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/eval/slicing.rs">
use std::collections::HashMap;
use crate::eval::{MutationCategory, Lang};

pub const GENERIC_ONLY_GAP_THRESHOLD: f32 = 0.10;

pub fn per_mutation_category_accuracy(
    predictions: &amp;[(MutationCategory, bool, bool)],
) -&gt; HashMap&lt;MutationCategory, f32&gt;;

pub fn per_language_accuracy(
    predictions: &amp;[(Lang, bool, bool)],
) -&gt; HashMap&lt;Lang, f32&gt;;

pub fn detect_generic_only_warning(
    per_cat: &amp;HashMap&lt;MutationCategory, f32&gt;,
    per_lang: &amp;HashMap&lt;Lang, f32&gt;,
    gap_threshold: f32,
) -&gt; Option&lt;String&gt;;
    </signature>
    <signature file="crates/context-graph-mejepa-train/src/eval/regression.rs">
use std::path::{Path, PathBuf};
use crate::error::{TrainerError, TrainerErrorCode};

pub const PROMOTION_EPSILON: f32 = 0.005;
pub const REGRESSION_THRESHOLD: f32 = 0.02;
pub const CONSECUTIVE_REGRESSION_HALT: usize = 3;

pub fn promote_best(
    checkpoint_dir: &amp;Path,
    current_step: u64,
    current_agreement: f32,
    best_so_far: f32,
    promotion_threshold: f32,
) -&gt; Result&lt;Option&lt;PathBuf&gt;, TrainerError&gt;;

pub fn detect_regression(
    current_agreement: f32,
    best_so_far: f32,
    regression_threshold: f32,
) -&gt; Option&lt;TrainerError&gt;;

pub fn track_consecutive_regressions(
    history: &amp;[bool],
    window: usize,
) -&gt; bool;

#[derive(Debug, Clone)]
pub struct PromotionDecision {
    pub promoted: bool,
    pub regression_emitted: bool,
    pub halt_triggered: bool,
}
    </signature>
  </signatures>
  <constraints>
    - Holdout split: 80/10/10 train/calibration/holdout per SPEC-CORPUS Stoer-Wagner mincut
    - per_category_accuracy: 8 keys; missing categories absent (don't synthesize zero)
    - per_language_accuracy: 11 keys; same
    - generic_only_warning: max - min &gt; 0.10 across present buckets only
    - promote_best: atomic write-tmp (with .tmp suffix) + fsync + rename
    - detect_regression: only if best_so_far &gt; 0 (skip first eval); drop &gt; threshold → return Some
    - 3-consecutive-regression halt: last 3 booleans all true → halt
    - HoldoutReport.phase3_dod_passed populated only on final step (Some(agreement &gt;= 0.75))
    - Files &lt; 500 lines each
  </constraints>
  <verification>
    - cargo test ≥ 14 tests pass
    - Synthetic 240-example dataset: agreement = correct/total ratio; per-cat/per-lang buckets correct; gap detection
    - Promotion atomic: best.safetensors.tmp not present after success
    - Regression detection at threshold boundary
    - 3-consecutive halt trigger
  </verification>
</definition_of_done>

<pseudo_code>
impl HoldoutEvaluator {
    pub fn evaluate(&amp;self, pred, oh, ds, cal, step, final_step, dod_min) -&gt; Result&lt;HoldoutReport, TrainerError&gt; {
        let mut correct = 0u64; let mut cat_predictions = vec![]; let mut lang_predictions = vec![];
        for ex in &amp;ds.examples {
            let p = pred.forward(&amp;ex.panel_t01)?;
            let o = oh.predict(&amp;p)?;
            cat_predictions.push((ex.category, o.pass_to_pass_resolved, ex.actual_oracle_pass));
            lang_predictions.push((ex.language, o.pass_to_pass_resolved, ex.actual_oracle_pass));
            if o.pass_to_pass_resolved == ex.actual_oracle_pass { correct += 1; }
        }
        let agreement = correct as f32 / ds.examples.len() as f32;
        let per_cat = per_mutation_category_accuracy(&amp;cat_predictions);
        let per_lang = per_language_accuracy(&amp;lang_predictions);
        let generic_warning = detect_generic_only_warning(&amp;per_cat, &amp;per_lang, GENERIC_ONLY_GAP_THRESHOLD);
        let conformal = compute_conformal_coverage(cal, oh, 0.10)?;
        let red_mi = pairwise_mi_for_holdout(&amp;ds.examples)?;
        let gtau = 1.0; // Phase 4b not yet shipped
        let dod = if final_step { Some(agreement &gt;= dod_min) } else { None };
        let report = HoldoutReport { step, prediction_oracle_agreement: agreement,
            conformal_coverage_calibration: conformal, per_mutation_category_accuracy: per_cat,
            per_language_accuracy: per_lang, predictor_redundancy_pairwise_mi: red_mi,
            gtau_pass_rate: gtau, generic_only_warning: generic_warning,
            phase3_dod_passed: dod, timestamp_iso8601: now_iso() };
        let cf = self.rocksdb.cf_handle(&amp;self.cf_holdout_reports_name).ok_or(...)?;
        self.rocksdb.put_cf(&amp;cf, step.to_be_bytes(), serde_json::to_vec(&amp;report)?)?;
        Ok(report)
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/eval/holdout.rs">HoldoutEvaluator impl + traits + dataset structs + tests</file>
  <file path="crates/context-graph-mejepa-train/src/eval/slicing.rs">per-category + per-language accuracy + generic_only_warning + tests</file>
  <file path="crates/context-graph-mejepa-train/src/eval/regression.rs">promote_best + detect_regression + track_consecutive_regressions + PromotionDecision + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/eval/mod.rs">re-export new modules</file>
  <file path="crates/context-graph-mejepa-train/Cargo.toml">add context-graph-mincut path dep</file>
</files_to_modify>

<validation_criteria>
  <criterion>240-example synthetic dataset agreement matches manual calc</criterion>
  <criterion>8 mutation categories exhaustive in per_cat output</criterion>
  <criterion>11 languages exhaustive in per_lang output (when populated)</criterion>
  <criterion>generic_only_warning fires at gap = 0.11; not at 0.10</criterion>
  <criterion>promote_best atomic (tmp + rename)</criterion>
  <criterion>detect_regression returns Some at drop = 0.021</criterion>
  <criterion>3 consecutive regressions trigger halt</criterion>
  <criterion>phase3_dod_passed Some only on final step</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib eval</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
