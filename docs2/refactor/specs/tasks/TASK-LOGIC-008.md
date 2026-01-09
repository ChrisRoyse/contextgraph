# TASK-LOGIC-008: 5-Stage Pipeline

```xml
<task_spec id="TASK-LOGIC-008" version="1.0">
<metadata>
  <title>Implement 5-Stage Retrieval Pipeline</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>18</sequence>
  <implements>
    <requirement_ref>REQ-SEARCH-PIPELINE-01</requirement_ref>
    <requirement_ref>REQ-LATENCY-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-005</task_ref>
    <task_ref>TASK-LOGIC-006</task_ref>
    <task_ref>TASK-LOGIC-007</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
The 5-stage pipeline optimizes retrieval latency by progressively filtering
candidates through stages of increasing precision but decreasing speed.
Target: <60ms at 1M memories.
</context>

<objective>
Implement the 5-stage retrieval pipeline: SPLADE sparse pre-filter, Matryoshka
fast ANN, multi-space RRF rerank, teleological alignment filter, and late
interaction MaxSim.
</objective>

<rationale>
Multi-stage retrieval enables:
1. Early elimination of non-candidates (Stage 1-2)
2. Progressive precision increase (Stages 3-5)
3. Sub-60ms latency even at scale
4. Stage skipping for specialized queries
5. Configurable precision-speed tradeoff
</rationale>

<input_context_files>
  <file purpose="single_search">crates/context-graph-storage/src/teleological/search/single.rs</file>
  <file purpose="weighted_search">crates/context-graph-storage/src/teleological/search/weighted.rs</file>
  <file purpose="matrix_search">crates/context-graph-storage/src/teleological/search/matrix.rs</file>
  <file purpose="pipeline_spec">docs2/refactor/03-SEARCH.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-005 complete (SingleEmbedderSearch)</check>
  <check>TASK-LOGIC-006 complete (WeightedFullSearch)</check>
  <check>TASK-LOGIC-007 complete (MatrixStrategySearch)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Stage 1: SPLADE sparse pre-filter</item>
    <item>Stage 2: Matryoshka 128D fast ANN</item>
    <item>Stage 3: Multi-space RRF rerank</item>
    <item>Stage 4: Teleological alignment filter</item>
    <item>Stage 5: Late interaction MaxSim</item>
    <item>Configurable stage pipeline</item>
    <item>Latency tracking and optimization</item>
  </in_scope>
  <out_of_scope>
    <item>Goal discovery (TASK-LOGIC-009)</item>
    <item>Drift detection (TASK-LOGIC-010)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/search/pipeline.rs">
      use crate::teleological::store::SearchResult;
      use context_graph_core::teleology::array::TeleologicalArray;

      /// Configuration for a pipeline stage.
      #[derive(Debug, Clone)]
      pub struct StageConfig {
          pub enabled: bool,
          pub candidate_multiplier: f32,
          pub min_score_threshold: f32,
          pub max_latency_ms: u64,
      }

      /// Pipeline stage results with timing.
      #[derive(Debug)]
      pub struct StageResult {
          pub candidates: Vec<SearchResult>,
          pub latency_ms: u64,
          pub candidates_in: usize,
          pub candidates_out: usize,
      }

      /// 5-stage retrieval pipeline.
      pub struct RetrievalPipeline {
          stage_configs: [StageConfig; 5],
          indices: PipelineIndices,
      }

      impl RetrievalPipeline {
          pub fn new(indices: PipelineIndices) -> Self;
          pub fn with_configs(indices: PipelineIndices, configs: [StageConfig; 5]) -> Self;

          /// Execute full pipeline.
          pub async fn execute(
              &self,
              query: &TeleologicalArray,
              limit: usize,
          ) -> Result<PipelineResult, PipelineError>;

          /// Execute with stage selection.
          pub async fn execute_stages(
              &self,
              query: &TeleologicalArray,
              stages: &[PipelineStage],
              limit: usize,
          ) -> Result<PipelineResult, PipelineError>;

          /// Stage 1: SPLADE sparse pre-filter
          async fn stage_splade_filter(
              &self,
              query: &TeleologicalArray,
              config: &StageConfig,
          ) -> Result<StageResult, PipelineError>;

          /// Stage 2: Matryoshka 128D fast ANN
          async fn stage_matryoshka_ann(
              &self,
              query: &TeleologicalArray,
              candidates: Vec<SearchResult>,
              config: &StageConfig,
          ) -> Result<StageResult, PipelineError>;

          /// Stage 3: Multi-space RRF rerank
          async fn stage_rrf_rerank(
              &self,
              query: &TeleologicalArray,
              candidates: Vec<SearchResult>,
              config: &StageConfig,
          ) -> Result<StageResult, PipelineError>;

          /// Stage 4: Teleological alignment filter
          async fn stage_alignment_filter(
              &self,
              query: &TeleologicalArray,
              candidates: Vec<SearchResult>,
              config: &StageConfig,
          ) -> Result<StageResult, PipelineError>;

          /// Stage 5: Late interaction MaxSim
          async fn stage_maxsim_rerank(
              &self,
              query: &TeleologicalArray,
              candidates: Vec<SearchResult>,
              config: &StageConfig,
          ) -> Result<StageResult, PipelineError>;
      }

      #[derive(Debug, Clone, Copy)]
      pub enum PipelineStage {
          SpladeFilter,
          MatryoshkaAnn,
          RrfRerank,
          AlignmentFilter,
          MaxSimRerank,
      }

      #[derive(Debug)]
      pub struct PipelineResult {
          pub results: Vec<SearchResult>,
          pub stage_results: Vec<StageResult>,
          pub total_latency_ms: u64,
          pub stages_executed: Vec<PipelineStage>,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Full pipeline completes in <60ms at 1M memories</constraint>
    <constraint>Each stage filters to fewer candidates</constraint>
    <constraint>Stages can be skipped for specialized queries</constraint>
    <constraint>Latency tracked per stage</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-storage search::pipeline</command>
    <command>cargo bench -p context-graph-storage pipeline_latency</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/search/pipeline.rs

use std::time::Instant;

use crate::teleological::store::SearchResult;
use crate::teleological::index::EmbedderIndex;
use context_graph_core::teleology::array::TeleologicalArray;
use context_graph_core::teleology::embedder::Embedder;
use context_graph_core::teleology::similarity::token_level;

#[derive(Debug, Clone)]
pub struct StageConfig {
    pub enabled: bool,
    pub candidate_multiplier: f32,
    pub min_score_threshold: f32,
    pub max_latency_ms: u64,
}

impl Default for StageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            candidate_multiplier: 3.0,
            min_score_threshold: 0.3,
            max_latency_ms: 20,
        }
    }
}

#[derive(Debug)]
pub struct StageResult {
    pub candidates: Vec<SearchResult>,
    pub latency_ms: u64,
    pub candidates_in: usize,
    pub candidates_out: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum PipelineStage {
    SpladeFilter,
    MatryoshkaAnn,
    RrfRerank,
    AlignmentFilter,
    MaxSimRerank,
}

pub struct PipelineIndices {
    pub splade: Box<dyn EmbedderIndex + Send + Sync>,      // E6 or E13
    pub semantic: Box<dyn EmbedderIndex + Send + Sync>,     // E1 (Matryoshka)
    pub late_interaction: Box<dyn EmbedderIndex + Send + Sync>, // E12
    pub all_indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>,
}

pub struct RetrievalPipeline {
    stage_configs: [StageConfig; 5],
    indices: PipelineIndices,
}

impl RetrievalPipeline {
    pub fn new(indices: PipelineIndices) -> Self {
        Self {
            stage_configs: [
                StageConfig { candidate_multiplier: 10.0, ..Default::default() }, // Stage 1
                StageConfig { candidate_multiplier: 5.0, ..Default::default() },  // Stage 2
                StageConfig { candidate_multiplier: 3.0, ..Default::default() },  // Stage 3
                StageConfig { candidate_multiplier: 2.0, ..Default::default() },  // Stage 4
                StageConfig { candidate_multiplier: 1.0, ..Default::default() },  // Stage 5
            ],
            indices,
        }
    }

    pub async fn execute(
        &self,
        query: &TeleologicalArray,
        limit: usize,
    ) -> Result<PipelineResult, PipelineError> {
        let stages = vec![
            PipelineStage::SpladeFilter,
            PipelineStage::MatryoshkaAnn,
            PipelineStage::RrfRerank,
            PipelineStage::AlignmentFilter,
            PipelineStage::MaxSimRerank,
        ];
        self.execute_stages(query, &stages, limit).await
    }

    pub async fn execute_stages(
        &self,
        query: &TeleologicalArray,
        stages: &[PipelineStage],
        limit: usize,
    ) -> Result<PipelineResult, PipelineError> {
        let start = Instant::now();
        let mut stage_results = Vec::new();
        let mut current_candidates: Vec<SearchResult> = Vec::new();
        let mut executed_stages = Vec::new();

        for (idx, stage) in stages.iter().enumerate() {
            let config = &self.stage_configs[idx];
            if !config.enabled {
                continue;
            }

            let target_count = (limit as f32 * config.candidate_multiplier) as usize;

            let result = match stage {
                PipelineStage::SpladeFilter => {
                    self.stage_splade_filter(query, target_count, config).await?
                }
                PipelineStage::MatryoshkaAnn => {
                    self.stage_matryoshka_ann(query, current_candidates.clone(), target_count, config).await?
                }
                PipelineStage::RrfRerank => {
                    self.stage_rrf_rerank(query, current_candidates.clone(), target_count, config).await?
                }
                PipelineStage::AlignmentFilter => {
                    self.stage_alignment_filter(query, current_candidates.clone(), target_count, config).await?
                }
                PipelineStage::MaxSimRerank => {
                    self.stage_maxsim_rerank(query, current_candidates.clone(), limit, config).await?
                }
            };

            current_candidates = result.candidates.clone();
            stage_results.push(result);
            executed_stages.push(*stage);
        }

        Ok(PipelineResult {
            results: current_candidates,
            stage_results,
            total_latency_ms: start.elapsed().as_millis() as u64,
            stages_executed: executed_stages,
        })
    }

    /// Stage 1: SPLADE sparse pre-filter
    /// Uses E6 (SpladeExpansion) for broad recall with lexical matching.
    async fn stage_splade_filter(
        &self,
        query: &TeleologicalArray,
        target_count: usize,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let start = Instant::now();

        let query_embedding = query.get(Embedder::SpladeExpansion);
        let results = self.indices.splade
            .search(query_embedding, target_count, Some(config.min_score_threshold))
            .await
            .map_err(|e| PipelineError::Stage { stage: "splade", error: e.to_string() })?;

        Ok(StageResult {
            candidates_in: 0, // First stage
            candidates_out: results.len(),
            candidates: results.into_iter()
                .map(|r| SearchResult {
                    array: TeleologicalArray::new(r.id),
                    similarity: r.score,
                    per_embedder_scores: [None; 13],
                })
                .collect(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Stage 2: Matryoshka 128D fast ANN
    /// Uses truncated E1 (Semantic) embedding for fast approximate search.
    async fn stage_matryoshka_ann(
        &self,
        query: &TeleologicalArray,
        candidates: Vec<SearchResult>,
        target_count: usize,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let start = Instant::now();
        let candidates_in = candidates.len();

        // Use Matryoshka truncation (128D) for speed
        let query_embedding = query.get(Embedder::Semantic);

        // If we have candidates, rerank them; otherwise search fresh
        let results = if candidates.is_empty() {
            self.indices.semantic
                .search(query_embedding, target_count, None)
                .await
                .map_err(|e| PipelineError::Stage { stage: "matryoshka", error: e.to_string() })?
        } else {
            // Rerank candidates using semantic similarity
            // In practice, we'd look up the actual arrays and compare
            todo!("Rerank candidates using semantic similarity")
        };

        Ok(StageResult {
            candidates_in,
            candidates_out: results.len().min(target_count),
            candidates: results.into_iter()
                .take(target_count)
                .map(|r| SearchResult {
                    array: TeleologicalArray::new(r.id),
                    similarity: r.score,
                    per_embedder_scores: [None; 13],
                })
                .collect(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Stage 3: Multi-space RRF rerank
    /// Combines scores from multiple embedding spaces using Reciprocal Rank Fusion.
    async fn stage_rrf_rerank(
        &self,
        query: &TeleologicalArray,
        candidates: Vec<SearchResult>,
        target_count: usize,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let start = Instant::now();
        let candidates_in = candidates.len();

        // RRF with k=60
        const RRF_K: f32 = 60.0;

        // Get rankings from multiple spaces
        // ... implementation details

        Ok(StageResult {
            candidates_in,
            candidates_out: target_count.min(candidates.len()),
            candidates: candidates.into_iter().take(target_count).collect(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Stage 4: Teleological alignment filter
    /// Filters based on alignment with discovered goals.
    async fn stage_alignment_filter(
        &self,
        query: &TeleologicalArray,
        candidates: Vec<SearchResult>,
        target_count: usize,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let start = Instant::now();
        let candidates_in = candidates.len();

        // Filter by teleological alignment
        // ... implementation details

        Ok(StageResult {
            candidates_in,
            candidates_out: target_count.min(candidates.len()),
            candidates: candidates.into_iter().take(target_count).collect(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Stage 5: Late interaction MaxSim
    /// Final precision reranking using ColBERT-style token matching.
    async fn stage_maxsim_rerank(
        &self,
        query: &TeleologicalArray,
        candidates: Vec<SearchResult>,
        limit: usize,
        config: &StageConfig,
    ) -> Result<StageResult, PipelineError> {
        let start = Instant::now();
        let candidates_in = candidates.len();

        // Final MaxSim reranking for precision
        // ... implementation using token_level::max_sim

        Ok(StageResult {
            candidates_in,
            candidates_out: limit.min(candidates.len()),
            candidates: candidates.into_iter().take(limit).collect(),
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PipelineError {
    #[error("Stage {stage} error: {error}")]
    Stage { stage: &'static str, error: String },
    #[error("Timeout at stage {stage}")]
    Timeout { stage: &'static str },
}

#[derive(Debug)]
pub struct PipelineResult {
    pub results: Vec<SearchResult>,
    pub stage_results: Vec<StageResult>,
    pub total_latency_ms: u64,
    pub stages_executed: Vec<PipelineStage>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_reduces_candidates() {
        // Each stage should reduce candidate count
    }

    #[tokio::test]
    async fn test_pipeline_latency_target() {
        // Full pipeline should complete in <60ms
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/search/pipeline.rs">
    5-stage retrieval pipeline implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/search/mod.rs">
    Add: pub mod pipeline;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Pipeline completes in <60ms at 1M scale</criterion>
  <criterion>Each stage reduces candidate count</criterion>
  <criterion>Stage skipping works correctly</criterion>
  <criterion>Latency tracked per stage</criterion>
  <criterion>Final results are high quality</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage search::pipeline -- --nocapture</command>
  <command>cargo bench -p context-graph-storage -- pipeline</command>
</test_commands>
</task_spec>
```
