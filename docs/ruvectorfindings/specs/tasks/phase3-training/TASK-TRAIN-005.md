# TASK-TRAIN-005 — BatchSampler with sampling_weight gates + deterministic seeded RNG

```xml
<task_spec id="TASK-TRAIN-005" version="1.0">

<metadata>
  <title>BatchSampler implementation: drop&lt;0.05; force&gt;1.0; weighted sampling without replacement; deterministic seeded RNG; force-includes guaranteed in every batch they qualify for; cross-task transfer integration point</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>5</sequence>
  <implements>
    <item ref="REQ-TRAIN-04">BatchSampler with weight = base · L_step · (1+5·override) · 0.995^age</item>
    <item ref="AC-TRAIN-04-01..05">All 5 acceptance criteria (drop, force, override, age, deterministic seed)</item>
    <item ref="BR-TRAIN-04">Sampling rules</item>
    <item ref="EC-TRAIN-07">All weights &lt; 0.05 → epoch vacuous warning</item>
    <item ref="EC-TRAIN-08">force-include count &gt; batch_size/2 warning</item>
    <item ref="component:BatchSampler">crates/context-graph-mejepa-train/src/sampler/mod.rs methods</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-001</task_ref>
    <task_ref>TASK-TRAIN-002</task_ref>
    <task_ref>../utml-learning-formula/TASK-UTML-007</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 modified, 1 created</estimated_files>
</metadata>

<context>
The BatchSampler implements UTML §6 sampling: hard-example mining (high L_step) + gold-standard up-weighting (operator override) + recency-weighted curriculum, replacing doc 09 §3.7.2's older `(1 + 2·residual_error_z_score)` term. Force-includes are guaranteed every batch they qualify for; drops never appear; regular slots fill via weighted-sampling-without-replacement. Determinism requires SplitMix64 seeded from config.random_seed.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-04; AC-TRAIN-04-01..05; BR-TRAIN-04; EC-TRAIN-07..08)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (architecture_diagram BatchSampler subgraph; data_models BatchSampler)</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (BatchSampler struct), TASK-TRAIN-002 (TrainingConfig sampling thresholds), ../utml-learning-formula/TASK-UTML-007 (sampling_weight, should_drop, should_force_include)</file>
  <file purpose="prng_source">crates/context-graph-mejepa-corpus/src/prng.rs (SplitMix64; reused)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-002 + UTML-007 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Implement methods on `BatchSampler` (struct from TASK-TRAIN-001):
      - `pub fn new(config: TrainingConfig, n_examples: usize, l_steps: Vec&lt;f32&gt;, overrides: Vec&lt;bool&gt;, ages_days: Vec&lt;f32&gt;, task_ids: Vec&lt;String&gt;, patch_similarity_graph: PatchSimilarityGraph) -&gt; Result&lt;Self, TrainerError&gt;`
      - `pub fn recompute_weights(&amp;mut self) -&gt; Result&lt;(), TrainerError&gt;` — recomputes weights from current l_steps + overrides + ages; partitions into force/drop/regular index sets
      - `pub fn next_batch(&amp;mut self, current_task_idx: Option&lt;usize&gt;, batch_size: usize) -&gt; Result&lt;BatchPlan, TrainerError&gt;`
      - `pub fn save_rng_state(&amp;self) -&gt; u64` and `pub fn restore_rng_state(&amp;mut self, state: u64)` for checkpoint resume
    - Define `pub struct BatchPlan { pub indices: Vec&lt;usize&gt;, pub force_count: usize, pub regular_count: usize, pub cross_task_indices: Vec&lt;usize&gt;, pub cross_task_fallback_count: usize }` (cross_task_* populated by TASK-TRAIN-007 integration; default empty here)
    - recompute_weights: for i in 0..n: weights[i] = sampling_weight(l_steps[i], overrides[i], ages_days[i], config.sampling_age_decay); partition by config.sampling_drop_threshold (0.05) and config.sampling_force_threshold (1.0)
    - next_batch: start with all force_indices; remaining `batch_size - force_count` slots filled via weighted-sampling-without-replacement from regular_indices; weights proportional to `weights[idx]` clamped at 1.0
    - Weighted sampling without replacement: alias-method or sequential roulette using SplitMix64
    - If force_count &gt; batch_size: emit warning, partition force-includes across multiple batches in epoch loop (next_batch returns first batch_size force-includes; epoch loop calls again)
    - Edge cases: if all weights &lt; 0.05 → return BatchPlan with empty indices + warn; if force_count &gt; batch_size/2 → warn (cert.generic_only_warning surface); if regular_indices empty after dropping → use only force-includes
    - Unit tests:
      - 100 synthetic examples (10 perfect_prediction L_step=0, 30 high L_step=0.6, 50 medium L_step=0.3, 10 override true) → weights computed correctly; force_count=10; drop_count=10; regular_count=80
      - Two runs with same seed produce identical batch sequences
      - Force-includes appear in every batch (verify across 10 batches)
      - High-L_step examples appear more often than medium (χ² test, p&lt;0.01)
      - All-zero weights → empty BatchPlan + warning
  </in_scope>
  <out_of_scope>
    - Cross-task transfer integration (TASK-TRAIN-007 adds it via `next_batch_with_cross_task` extension)
    - Patch similarity graph construction (TASK-TRAIN-007)
    - L_step recomputation (TASK-TRAIN-012 calls trainer-side recompute)
    - HoldoutEvaluator (TASK-TRAIN-011)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/sampler/mod.rs">
use crate::config::TrainingConfig;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::learning_signal::sampling_weight;
use context_graph_mejepa_corpus::prng::SplitMix64;

pub struct PatchSimilarityGraph {
    pub neighbors: Vec&lt;Vec&lt;usize&gt;&gt;,
}

pub struct BatchSampler {
    pub config: TrainingConfig,
    pub weights: Vec&lt;f32&gt;,
    pub overrides: Vec&lt;bool&gt;,
    pub ages_days: Vec&lt;f32&gt;,
    pub l_steps: Vec&lt;f32&gt;,
    pub task_ids: Vec&lt;String&gt;,
    pub patch_similarity_graph: PatchSimilarityGraph,
    pub rng: SplitMix64,
    pub force_indices: Vec&lt;usize&gt;,
    pub drop_indices: Vec&lt;usize&gt;,
    pub regular_indices: Vec&lt;usize&gt;,
    pub force_cursor: usize, // for splitting force-includes across multiple batches when oversupplied
}

#[derive(Debug, Clone)]
pub struct BatchPlan {
    pub indices: Vec&lt;usize&gt;,
    pub force_count: usize,
    pub regular_count: usize,
    pub cross_task_indices: Vec&lt;usize&gt;,
    pub cross_task_fallback_count: usize,
}

#[derive(Debug, Clone)]
pub enum SamplerWarning {
    AllWeightsBelowDropThreshold,
    ForceCountExceedsHalfBatch { force_count: usize, batch_size: usize },
}

impl BatchSampler {
    pub fn new(
        config: TrainingConfig,
        l_steps: Vec&lt;f32&gt;,
        overrides: Vec&lt;bool&gt;,
        ages_days: Vec&lt;f32&gt;,
        task_ids: Vec&lt;String&gt;,
        patch_similarity_graph: PatchSimilarityGraph,
    ) -&gt; Result&lt;Self, TrainerError&gt;;

    pub fn recompute_weights(&amp;mut self) -&gt; Result&lt;Vec&lt;SamplerWarning&gt;, TrainerError&gt;;

    pub fn next_batch(
        &amp;mut self,
        current_task_idx: Option&lt;usize&gt;,
        batch_size: usize,
    ) -&gt; Result&lt;BatchPlan, TrainerError&gt;;

    pub fn save_rng_state(&amp;self) -&gt; u64;
    pub fn restore_rng_state(&amp;mut self, state: u64);
    pub fn n_examples(&amp;self) -&gt; usize;
}
    </signature>
  </signatures>
  <constraints>
    - Validate input vec lengths all equal n_examples; emit TrainerError::new(MejepaTrainConfigInvalid) on mismatch
    - SplitMix64 seeded from config.random_seed
    - Weighted-sampling-without-replacement uses incremental roulette: pick max-weight from remaining; respect determinism
    - force_cursor advances across batches when force_count &gt; batch_size; epoch loop calls next_batch() until all force-includes seen at least once
    - cross_task_indices initialized empty; TASK-TRAIN-007 fills via separate API path
    - File &lt; 500 lines
  </constraints>
  <verification>
    - cargo test ≥ 8 tests pass: weights computed, force/drop/regular partitions correct, determinism, force-include guarantee, all-zero warning, oversupply force-include warning
    - Determinism: two BatchSamplers with same seed + same inputs produce byte-equal BatchPlans across 10 next_batch calls
  </verification>
</definition_of_done>

<pseudo_code>
impl BatchSampler {
    pub fn recompute_weights(&amp;mut self) -&gt; Result&lt;Vec&lt;SamplerWarning&gt;, TrainerError&gt; {
        let n = self.l_steps.len();
        self.weights = (0..n).map(|i| sampling_weight(
            self.l_steps[i], self.overrides[i], self.ages_days[i], self.config.sampling_age_decay
        )).collect();
        self.force_indices.clear(); self.drop_indices.clear(); self.regular_indices.clear();
        for i in 0..n {
            let w = self.weights[i];
            if w &lt; self.config.sampling_drop_threshold { self.drop_indices.push(i); }
            else if w &gt; self.config.sampling_force_threshold { self.force_indices.push(i); }
            else { self.regular_indices.push(i); }
        }
        let mut warns = vec![];
        if self.regular_indices.is_empty() &amp;&amp; self.force_indices.is_empty() {
            warns.push(SamplerWarning::AllWeightsBelowDropThreshold);
        }
        if self.force_indices.len() &gt; self.config.batch_size / 2 {
            warns.push(SamplerWarning::ForceCountExceedsHalfBatch { force_count: self.force_indices.len(), batch_size: self.config.batch_size });
        }
        Ok(warns)
    }

    pub fn next_batch(&amp;mut self, _current_task_idx: Option&lt;usize&gt;, batch_size: usize) -&gt; Result&lt;BatchPlan, TrainerError&gt; {
        let mut plan = BatchPlan { indices: Vec::with_capacity(batch_size), force_count: 0, regular_count: 0, cross_task_indices: vec![], cross_task_fallback_count: 0 };
        // 1. Force-includes (split across batches if oversupplied)
        let take = (self.force_indices.len() - self.force_cursor).min(batch_size);
        plan.indices.extend(&amp;self.force_indices[self.force_cursor..self.force_cursor+take]);
        plan.force_count = take;
        self.force_cursor = (self.force_cursor + take) % self.force_indices.len().max(1);
        // 2. Fill remaining via weighted-sampling-without-replacement
        let need = batch_size - plan.force_count;
        let mut pool: Vec&lt;usize&gt; = self.regular_indices.clone();
        let mut weights: Vec&lt;f32&gt; = pool.iter().map(|&amp;i| self.weights[i]).collect();
        for _ in 0..need.min(pool.len()) {
            let total: f32 = weights.iter().sum();
            if total &lt;= 0.0 { break; }
            let mut acc = 0.0; let r = self.rng.next_unit_f32() * total;
            let mut chosen = 0usize;
            for (k, &amp;w) in weights.iter().enumerate() { acc += w; if r &lt;= acc { chosen = k; break; } }
            plan.indices.push(pool[chosen]);
            pool.swap_remove(chosen); weights.swap_remove(chosen);
            plan.regular_count += 1;
        }
        Ok(plan)
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/sampler/plan.rs">BatchPlan + SamplerWarning enums</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/sampler/mod.rs">add new + recompute_weights + next_batch + save/restore RNG state methods</file>
</files_to_modify>

<validation_criteria>
  <criterion>weights match sampling_weight() per UTML-007 for all examples</criterion>
  <criterion>force_count == count of weight&gt;1.0 examples</criterion>
  <criterion>drop_count == count of weight&lt;0.05 examples</criterion>
  <criterion>two runs with same seed produce identical BatchPlans</criterion>
  <criterion>force-includes appear in every batch when supply &lt;= batch_size</criterion>
  <criterion>oversupplied force-includes split across batches via force_cursor</criterion>
  <criterion>warnings surfaced for empty regular_indices and force&gt;batch_size/2</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib sampler</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
