# TASK-HEAL-003 — A/B/C promotion data models (AbcPromoter, ModelHandle, EnsembleHandle, HoldoutEval, PromotionGate)

```xml
<task_spec id="TASK-HEAL-003" version="1.0">

<metadata>
  <title>A/B/C promotion data models in `crates/context-graph-mejepa/src/heal/promote.rs` (struct definitions only; method bodies in TASK-HEAL-009 / TASK-HEAL-010)</title>
  <status>ready</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <item ref="REQ-HEAL-04">AbcPromoter holds mode_a, mode_b, mode_c, holdout, grid_search_step, weight_blobs, witness_chain handles</item>
    <item ref="data_model:AbcPromoter">field shapes per TECH-SELF-HEALING data_models</item>
    <item ref="data_model:ModelHandle">theta_sha + weights_blob_key + trained_corpus_sha + trained_at_step</item>
    <item ref="data_model:EnsembleHandle">w_a + w_b + theta_a_sha + theta_b_sha (w_a + w_b == 1)</item>
    <item ref="data_model:HoldoutEval">coverage + oracle_agreement + ood_distribution_kl + num_samples + evaluation_summary_sha</item>
    <item ref="data_model:PromotionGate">stated_coverage_min + oracle_agreement_floor + ood_kl_ceiling + full_retrain_floor</item>
  </implements>
  <depends_on><!-- foundation; cross-domain shipped --></depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created (src/heal/promote.rs)</estimated_files>
</metadata>

<context>
Third foundation task. Lands the public data model contract for the A/B/C promotion gate: three
parallel modes (A current, B retrain-from-scratch, C grid-search ensemble), holdout-split
evaluation, and a 3-condition gate (coverage ≥ stated, oracle_agreement ≥ A's, OOD KL ≤ A's).
Method bodies (retrain_and_promote, rollback_to, promotion_gate evaluation, witness-chain entry
emission) are deferred to TASK-HEAL-009 / TASK-HEAL-010. This task ships definitions, derives,
validating constructors, and unit tests for invariants (w_a + w_b == 1; coverage/agreement/KL
finite; full_retrain_floor == 0.02).
</context>

<input_context_files>
  <file purpose="canonical_authority">docs/ruvectorfindings/specs/constitution.md (`<failure_taxonomy>` MEJEPA_RETRAIN_HOLDOUT_REGRESSION, MEJEPA_RETRAIN_QUOTA_EXCEEDED, MEJEPA_HEAL_PROMOTION_DEADLOCK)</file>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-SELF-HEALING.md (REQ-HEAL-04, REQ-HEAL-12; AC-HEAL-03-01..05; EC-HEAL-05 NaN mid-retrain, EC-HEAL-06 grid-search w_A=1)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-SELF-HEALING.md (data_models block: AbcPromoter, ModelHandle, EnsembleHandle, HoldoutEval, PromotionGate)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase5-self-healing/TASK-HEAL-001.md (heal/mod.rs scaffold)</file>
  <file purpose="prior_task">docs/ruvectorfindings/specs/tasks/phase5-self-healing/TASK-HEAL-002.md (DriftSeverity for trigger_reason)</file>
</input_context_files>

<prerequisites>
  <check>TASK-HEAL-001 + TASK-HEAL-002 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Fill `crates/context-graph-mejepa/src/heal/promote.rs` with:
      - `pub const DEFAULT_GRID_SEARCH_STEP: f32 = 0.1;`
      - `pub const DEFAULT_STATED_COVERAGE_MIN: f32 = 0.90;`
      - `pub const DEFAULT_FULL_RETRAIN_FLOOR: f32 = 0.02;`
      - `TriggerReason` enum: `{DriftHard, DriftCatastrophic, PeriodicFullRetrain, OperatorTriggered}` with `#[serde(rename_all = "snake_case")]`.
      - `ModelHandle { theta_sha: [u8; 32], weights_blob_key: Vec<u8>, trained_corpus_sha: [u8; 32], trained_at_step: u64 }` with `try_new` validating `weights_blob_key` non-empty.
      - `EnsembleHandle { w_a: f32, w_b: f32, theta_a_sha: [u8; 32], theta_b_sha: [u8; 32] }` with `try_new` validating finite weights ∈ [0,1] and `(w_a + w_b - 1.0).abs() < 1e-3`.
      - `HoldoutEval { coverage: f32, oracle_agreement: f32, ood_distribution_kl: f32, num_samples: usize, evaluation_summary_sha: [u8; 32] }` with `try_new` validating finite coverage ∈ [0,1], finite oracle_agreement ∈ [0,1], finite non-negative ood_distribution_kl, num_samples > 0.
      - `PromotionGate { stated_coverage_min: f32, oracle_agreement_floor: f32, ood_kl_ceiling: f32, full_retrain_floor: f32 }` with `try_new` validating finite + monotonic, and `Default::default()` matching constants.
      - `ModeWinner` enum: `{A, B, C, A_unchanged_no_floor_clearance, A_unchanged_no_winner}` with `#[serde(rename_all = "snake_case")]`. Implement `is_promoted(&self) -> bool` returning true for A, B, C.
      - `AbcPromoter { mode_a: Option<ModelHandle>, mode_b: Option<ModelHandle>, mode_c: Option<EnsembleHandle>, holdout_summary_sha: Option<[u8; 32]>, grid_search_step: f32, gate: PromotionGate, last_trigger_reason: Option<TriggerReason> }` with `try_new(grid_search_step: f32, gate: PromotionGate) -> Result<Self, &'static str>` validating `0 < grid_search_step <= 0.5`. Note: the heavy fields (Arc<RocksDbStore>, Arc<WitnessChainAppender>) are NOT yet in this struct — they're injected at method-body time in TASK-HEAL-009 to avoid forward references; this struct is the "configuration core".
    - Module-level test set in `#[cfg(test)] mod tests`:
      - `model_handle_rejects_empty_blob_key`
      - `ensemble_handle_rejects_w_a_plus_w_b_neq_one`
      - `ensemble_handle_rejects_negative_weight`
      - `holdout_eval_rejects_non_finite_coverage`
      - `holdout_eval_rejects_zero_num_samples`
      - `holdout_eval_rejects_negative_kl`
      - `promotion_gate_default_matches_spec` (coverage=0.90, full_retrain_floor=0.02)
      - `mode_winner_is_promoted_true_for_abc_only`
      - `abc_promoter_rejects_grid_step_zero`
      - `abc_promoter_rejects_grid_step_above_half`
      - `default_constants_match_spec` (0.1, 0.90, 0.02)
      - `serde_round_trip_holdout_eval`
      - `serde_round_trip_ensemble_handle`
      - `serde_snake_case_trigger_reason` (asserts `"drift_hard"` JSON literal)
  </in_scope>
  <out_of_scope>
    - retrain_and_promote / rollback_to method bodies (TASK-HEAL-009 / TASK-HEAL-010)
    - promotion_gate evaluation logic (TASK-HEAL-009)
    - Mode B train_from_scratch invocation (TASK-HEAL-009; needs Phase 3 Training)
    - Mode C grid-search loop (TASK-HEAL-009)
    - Witness-chain ModelPromote / ModelRollback entry emission (TASK-HEAL-009 / TASK-HEAL-010)
    - HealReport (TASK-HEAL-004)
    - HealError variants (TASK-HEAL-004)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa/src/heal/promote.rs">
use serde::{Deserialize, Serialize};

pub const DEFAULT_GRID_SEARCH_STEP: f32 = 0.1;
pub const DEFAULT_STATED_COVERAGE_MIN: f32 = 0.90;
pub const DEFAULT_FULL_RETRAIN_FLOOR: f32 = 0.02;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerReason {
    DriftHard,
    DriftCatastrophic,
    PeriodicFullRetrain,
    OperatorTriggered,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelHandle {
    pub theta_sha: [u8; 32],
    pub weights_blob_key: Vec&lt;u8&gt;,
    pub trained_corpus_sha: [u8; 32],
    pub trained_at_step: u64,
}

impl ModelHandle {
    pub fn try_new(theta_sha: [u8; 32], weights_blob_key: Vec&lt;u8&gt;,
                   trained_corpus_sha: [u8; 32], trained_at_step: u64)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct EnsembleHandle {
    pub w_a: f32, pub w_b: f32,
    pub theta_a_sha: [u8; 32], pub theta_b_sha: [u8; 32],
}

impl EnsembleHandle {
    pub fn try_new(w_a: f32, w_b: f32, theta_a_sha: [u8; 32], theta_b_sha: [u8; 32])
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct HoldoutEval {
    pub coverage: f32,
    pub oracle_agreement: f32,
    pub ood_distribution_kl: f32,
    pub num_samples: usize,
    pub evaluation_summary_sha: [u8; 32],
}

impl HoldoutEval {
    pub fn try_new(coverage: f32, oracle_agreement: f32, ood_distribution_kl: f32,
                   num_samples: usize, evaluation_summary_sha: [u8; 32])
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct PromotionGate {
    pub stated_coverage_min: f32,
    pub oracle_agreement_floor: f32,
    pub ood_kl_ceiling: f32,
    pub full_retrain_floor: f32,
}

impl Default for PromotionGate { fn default() -&gt; Self; }
impl PromotionGate {
    pub fn try_new(stated_coverage_min: f32, oracle_agreement_floor: f32,
                   ood_kl_ceiling: f32, full_retrain_floor: f32)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModeWinner {
    A, B, C,
    A_unchanged_no_floor_clearance,
    A_unchanged_no_winner,
}

impl ModeWinner {
    pub fn is_promoted(&amp;self) -&gt; bool;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AbcPromoter {
    pub mode_a: Option&lt;ModelHandle&gt;,
    pub mode_b: Option&lt;ModelHandle&gt;,
    pub mode_c: Option&lt;EnsembleHandle&gt;,
    pub holdout_summary_sha: Option&lt;[u8; 32]&gt;,
    pub grid_search_step: f32,
    pub gate: PromotionGate,
    pub last_trigger_reason: Option&lt;TriggerReason&gt;,
}

impl AbcPromoter {
    pub fn try_new(grid_search_step: f32, gate: PromotionGate)
        -&gt; Result&lt;Self, &amp;'static str&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - File &lt; 500 lines
    - `(w_a + w_b - 1.0).abs() &lt; 1e-3` (Polyak-style ensemble normalization)
    - `w_a, w_b ∈ [0,1]`, finite
    - `coverage, oracle_agreement ∈ [0,1]`, finite
    - `ood_distribution_kl >= 0`, finite
    - `num_samples > 0` (HoldoutEval)
    - `stated_coverage_min ∈ (0,1]`, finite
    - `oracle_agreement_floor ∈ [0,1]`, finite
    - `ood_kl_ceiling >= 0`, finite
    - `full_retrain_floor >= 0`, finite (default 0.02)
    - `0 < grid_search_step <= 0.5` (AbcPromoter)
    - Constants exactly: 0.1, 0.90, 0.02
    - `weights_blob_key` non-empty (ModelHandle)
    - All public types `Debug + Clone + Serialize + Deserialize + PartialEq` (Copy where possible)
    - File begins with `// A/B/C promotion data shapes; method bodies in TASK-HEAL-009 (retrain_and_promote) and TASK-HEAL-010 (rollback_to).`
  </constraints>
  <verification>
    - `cargo build --release -p context-graph-mejepa` zero warnings
    - `cargo test -p context-graph-mejepa heal::promote` shows ≥ 14 passing tests
    - `cargo clippy -p context-graph-mejepa -- -D warnings` clean
    - PromotionGate::default() == { 0.90, 0.0, f32::INFINITY-or-equivalent, 0.02 } per pseudo
    - ModeWinner::is_promoted true only for A, B, C
    - EnsembleHandle::try_new(w_a=0.6, w_b=0.6, ...) returns Err
  </verification>
</definition_of_done>

<pseudo_code>
impl Default for PromotionGate {
    fn default() -> Self {
        Self {
            stated_coverage_min: DEFAULT_STATED_COVERAGE_MIN, // 0.90
            oracle_agreement_floor: 0.0, // computed dynamically at promotion start; 0 is "no constraint" placeholder
            ood_kl_ceiling: f32::INFINITY, // computed dynamically; INFINITY is "no constraint" placeholder
            full_retrain_floor: DEFAULT_FULL_RETRAIN_FLOOR, // 0.02
        }
    }
}

impl EnsembleHandle {
    pub fn try_new(w_a, w_b, ...) -> Result<Self, &'static str> {
        if !w_a.is_finite() || !w_b.is_finite() { return Err("non-finite ensemble weight"); }
        if !(0.0..=1.0).contains(&w_a) || !(0.0..=1.0).contains(&w_b) { return Err("weight out of [0,1]"); }
        if (w_a + w_b - 1.0).abs() > 1e-3 { return Err("w_a + w_b must equal 1.0 within 1e-3"); }
        Ok(Self { w_a, w_b, theta_a_sha, theta_b_sha })
    }
}

impl HoldoutEval {
    pub fn try_new(coverage, oracle_agreement, ood_distribution_kl, num_samples, summary_sha)
        -> Result<Self, &'static str>
    {
        if !coverage.is_finite() || !(0.0..=1.0).contains(&coverage) { return Err("coverage out of [0,1]"); }
        if !oracle_agreement.is_finite() || !(0.0..=1.0).contains(&oracle_agreement) { return Err("oracle_agreement out of [0,1]"); }
        if !ood_distribution_kl.is_finite() || ood_distribution_kl < 0.0 { return Err("kl negative or non-finite"); }
        if num_samples == 0 { return Err("num_samples must be > 0"); }
        Ok(Self { coverage, oracle_agreement, ood_distribution_kl, num_samples, evaluation_summary_sha: summary_sha })
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa/src/heal/promote.rs">TriggerReason + ModelHandle + EnsembleHandle + HoldoutEval + PromotionGate + ModeWinner + AbcPromoter + DEFAULT_* constants + tests</file>
</files_to_create>

<files_to_modify>
  <!-- heal/mod.rs already declares pub mod promote; from TASK-HEAL-001 -->
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --release -p context-graph-mejepa zero warnings</criterion>
  <criterion>cargo test -p context-graph-mejepa heal::promote shows ≥ 14 passing tests</criterion>
  <criterion>cargo clippy -p context-graph-mejepa -- -D warnings clean</criterion>
  <criterion>EnsembleHandle invariant w_a + w_b == 1.0 within 1e-3 enforced</criterion>
  <criterion>HoldoutEval rejects non-finite coverage, negative KL, num_samples==0</criterion>
  <criterion>PromotionGate::default() has full_retrain_floor == 0.02</criterion>
  <criterion>ModeWinner::is_promoted true exactly for A, B, C</criterion>
  <criterion>AbcPromoter::try_new rejects grid_search_step == 0 or &gt; 0.5</criterion>
  <criterion>TriggerReason serializes as snake_case</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa</command>
  <command>cargo test -p context-graph-mejepa heal::promote</command>
  <command>cargo clippy -p context-graph-mejepa -- -D warnings</command>
</test_commands>

</task_spec>
```
