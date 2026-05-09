# TASK-TRAIN-007 — Cross-task transfer (p=0.3 sample from related_tasks(T) where cosine(E_Diff)&gt;0.7)

```xml
<task_spec id="TASK-TRAIN-007" version="1.0">

<metadata>
  <title>Cross-task transfer: extend BatchSampler with related_tasks neighborhood lookup; with p=0.3 each batch slot drawn from {T' : cosine(E_Diff(T), E_Diff(T')) &gt; 0.7}; track cross_task_transfer_indices + fallback_count</title>
  <status>blocked</status>
  <layer>logic</layer>
  <sequence>7</sequence>
  <implements>
    <item ref="REQ-TRAIN-07">Cross-task transfer with patch-similarity graph</item>
    <item ref="AC-TRAIN-07-01..03">p=0.3; cosine&gt;0.7; fallback when neighbors empty</item>
    <item ref="BR-TRAIN-07">BatchSampler composes batch slot with cross-task probability</item>
    <item ref="EC-TRAIN-12">Sparse graph &gt; 30% fallback rate warning</item>
  </implements>
  <depends_on>
    <task_ref>TASK-TRAIN-001</task_ref>
    <task_ref>TASK-TRAIN-005</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_files>1 created, 1 modified</estimated_files>
</metadata>

<context>
Cross-task transfer (doc 09 §3.5.3 + §3.7.5) shares gradient signal across related tasks. Empirical defense of GENERIC-ONLY: a generic primitive helps similar tasks together. The `PatchSimilarityGraph` is constructed offline from E_Diff embeddings of all 2,400 corpus entries (Phase 0 dependency); thresholded at cosine &gt; 0.7. Each batch slot has 0.3 probability of being drawn from `related_tasks(current_task)`; 0.7 probability falls back to default sampling_weight flow.
</context>

<input_context_files>
  <file purpose="functional_spec">docs/ruvectorfindings/specs/functional/SPEC-TRAINING.md (US-TRAIN-07; AC-TRAIN-07-01..03; BR-TRAIN-07; EC-TRAIN-12)</file>
  <file purpose="technical_spec">docs/ruvectorfindings/specs/technical/TECH-TRAINING.md (architecture_diagram XTASK subgraph; trainer.train_one_epoch step 4a)</file>
  <file purpose="design_doc">docs/ruvectorfindings/09_END_GOAL_REVIEW_REPLACEMENT.md §3.5.3 + §3.7.5</file>
  <file purpose="prior_tasks">TASK-TRAIN-001 (PatchSimilarityGraph stub), TASK-TRAIN-005 (BatchSampler::next_batch)</file>
</input_context_files>

<prerequisites>
  <check>TASK-TRAIN-001 + TASK-TRAIN-005 complete</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `src/sampler/cross_task.rs` with:
      - `pub fn build_patch_similarity_graph(e_diff_embeddings: &amp;[Vec&lt;f32&gt;], cosine_threshold: f32) -&gt; PatchSimilarityGraph` (cosine of all pairs; threshold)
      - `pub fn related_tasks_above(graph: &amp;PatchSimilarityGraph, task_idx: usize) -&gt; &amp;[usize]` returns neighbor indices
    - Extend `BatchSampler` with new method:
      - `pub fn next_batch_with_cross_task(&amp;mut self, current_task_idx: usize, batch_size: usize) -&gt; Result&lt;BatchPlan, TrainerError&gt;` — for each remaining slot (after force-includes), with p=cross_task_transfer_probability draw from related_tasks(current_task_idx) ∩ regular_indices; with 1-p draw from regular_indices via weighted sampling
      - Track cross_task_indices (slot indices drawn cross-task) and cross_task_fallback_count (slots where neighbor set ∩ regular was empty → fell back to weighted)
      - Surface warning when fallback_count / batch_size &gt; 0.30 across an epoch (called by epoch loop in TASK-TRAIN-012)
    - Determinism: same RNG state + same graph → same batch sequence
    - Unit tests: dense graph (every task has &gt;=10 neighbors) → ~30% cross_task slots; empty graph → 100% fallback; threshold filtering; cosine math correctness
  </in_scope>
  <out_of_scope>
    - E_Diff embedder (Phase 1 dep — TASK-INSTR-014)
    - Full corpus loading (Phase 0)
    - Epoch-level fallback warning aggregation (TASK-TRAIN-012)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mejepa-train/src/sampler/cross_task.rs">
use crate::error::TrainerError;
use crate::sampler::{PatchSimilarityGraph, BatchSampler, BatchPlan};

pub const DEFAULT_FALLBACK_WARNING_RATIO: f32 = 0.30;

pub fn build_patch_similarity_graph(
    e_diff_embeddings: &amp;[Vec&lt;f32&gt;],
    cosine_threshold: f32,
) -&gt; PatchSimilarityGraph;

pub fn related_tasks_above&lt;'g&gt;(
    graph: &amp;'g PatchSimilarityGraph,
    task_idx: usize,
) -&gt; &amp;'g [usize];

impl BatchSampler {
    pub fn next_batch_with_cross_task(
        &amp;mut self,
        current_task_idx: usize,
        batch_size: usize,
    ) -&gt; Result&lt;BatchPlan, TrainerError&gt;;
}
    </signature>
  </signatures>
  <constraints>
    - cosine_threshold default 0.7 per AC-TRAIN-07-01
    - cross_task_transfer_probability from config (default 0.3)
    - For each non-force slot: rng.next_unit_f32() &lt; p_xtask → try draw from related_tasks(current_task) ∩ regular_indices; if empty → fallback to weighted-sampling-without-replacement from regular_indices and increment fallback_count
    - cross_task_indices: indices of *batch slots* drawn cross-task (not example indices)
    - File &lt; 350 lines
  </constraints>
  <verification>
    - cargo test ≥ 6 tests pass
    - Synthetic 100-task graph with 10 neighbors per task → 1000 batches → ~300 cross-task slots (statistical)
    - Empty graph → 100% fallback per slot
  </verification>
</definition_of_done>

<pseudo_code>
pub fn build_patch_similarity_graph(embs, thr) -&gt; PatchSimilarityGraph {
    let n = embs.len();
    let mut neighbors = vec![Vec::new(); n];
    for i in 0..n {
        for j in 0..n {
            if i == j { continue; }
            if cosine(&amp;embs[i], &amp;embs[j]) &gt; thr { neighbors[i].push(j); }
        }
    }
    PatchSimilarityGraph { neighbors }
}

impl BatchSampler {
    pub fn next_batch_with_cross_task(&amp;mut self, current: usize, batch_size: usize) -&gt; Result&lt;BatchPlan, TrainerError&gt; {
        let mut plan = self.fill_force_only(batch_size)?;
        let need = batch_size - plan.force_count;
        let neighbors = related_tasks_above(&amp;self.patch_similarity_graph, current);
        let regular_set: HashSet&lt;usize&gt; = self.regular_indices.iter().copied().collect();
        for slot_idx in 0..need {
            let r = self.rng.next_unit_f32();
            if r &lt; self.config.cross_task_transfer_probability {
                let mut pool: Vec&lt;usize&gt; = neighbors.iter().filter(|i| regular_set.contains(i)).copied().collect();
                if pool.is_empty() {
                    plan.cross_task_fallback_count += 1;
                    let idx = self.weighted_pick_from_regular()?;
                    plan.indices.push(idx);
                } else {
                    let idx = pool[self.rng.next_usize_below(pool.len())];
                    plan.cross_task_indices.push(plan.indices.len());
                    plan.indices.push(idx);
                }
            } else {
                let idx = self.weighted_pick_from_regular()?;
                plan.indices.push(idx);
            }
            plan.regular_count += 1;
        }
        Ok(plan)
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mejepa-train/src/sampler/cross_task.rs">build_patch_similarity_graph + related_tasks_above + extension impl + tests</file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mejepa-train/src/sampler/mod.rs">pub mod cross_task; re-export</file>
</files_to_modify>

<validation_criteria>
  <criterion>Dense graph → ~30% cross-task slots in 1000 batches (binomial confidence interval)</criterion>
  <criterion>Empty graph → 100% fallback per slot</criterion>
  <criterion>cross_task_indices length matches actual cross-task picks</criterion>
  <criterion>fallback_count tracked correctly</criterion>
  <criterion>Same seed → same sequence</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --release -p context-graph-mejepa-train</command>
  <command>cargo test -p context-graph-mejepa-train --lib cross_task</command>
  <command>cargo clippy -p context-graph-mejepa-train -- -D warnings</command>
</test_commands>

</task_spec>
```
