# TASK-LOGIC-006: Weighted Full Search

```xml
<task_spec id="TASK-LOGIC-006" version="1.0">
<metadata>
  <title>Implement Weighted Full Array Search</title>
  <status>todo</status>
  <layer>logic</layer>
  <sequence>16</sequence>
  <implements>
    <requirement_ref>REQ-SEARCH-WEIGHTED-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-004</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
Weighted search queries all 13 embedder indices with custom weights per embedder.
Results from each index are fused using weighted score combination. Supports
parallel execution strategies for performance.
</context>

<objective>
Implement WeightedFullSearch that queries all 13 indices in parallel, applies
custom weights, and fuses results into a single ranked list.
</objective>

<rationale>
Weighted full search enables:
1. Query-specific weight tuning
2. Balance between different semantic spaces
3. Comprehensive retrieval across all dimensions
4. Configurable emphasis on specific embedders
</rationale>

<input_context_files>
  <file purpose="comparator">crates/context-graph-core/src/teleology/comparator.rs</file>
  <file purpose="single_search">crates/context-graph-storage/src/teleological/search/single.rs</file>
  <file purpose="comparison_types">crates/context-graph-core/src/teleology/comparison.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-004 complete (TeleologicalComparator exists)</check>
  <check>TASK-LOGIC-005 complete (SingleEmbedderSearch exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create WeightedFullSearch struct</item>
    <item>Search all 13 indices with custom weights</item>
    <item>Implement ParallelStrategy enum (FullParallel, Staged, Sequential)</item>
    <item>Score fusion with configurable weights</item>
    <item>Re-ranking pass for final results</item>
  </in_scope>
  <out_of_scope>
    <item>Matrix strategy (TASK-LOGIC-007)</item>
    <item>5-stage pipeline (TASK-LOGIC-008)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-storage/src/teleological/search/weighted.rs">
      use crate::teleological::store::SearchResult;
      use crate::teleological::index::EmbedderIndex;
      use crate::teleological::search::single::SingleEmbedderSearch;
      use context_graph_core::teleology::array::TeleologicalArray;
      use context_graph_core::teleology::comparison::EmbedderWeights;

      /// Execution strategy for parallel search.
      #[derive(Debug, Clone, Copy)]
      pub enum ParallelStrategy {
          /// Query all 13 indices in parallel
          FullParallel,
          /// Query in stages, using early results to prune later
          Staged { stage_size: usize },
          /// Query sequentially (for debugging/testing)
          Sequential,
      }

      /// Weighted full array search across all embedders.
      pub struct WeightedFullSearch<I: EmbedderIndex> {
          indices: [Box<dyn EmbedderIndex>; 13],
          strategy: ParallelStrategy,
      }

      impl<I: EmbedderIndex> WeightedFullSearch<I> {
          pub fn new(indices: [Box<dyn EmbedderIndex>; 13]) -> Self;
          pub fn with_strategy(indices: [Box<dyn EmbedderIndex>; 13], strategy: ParallelStrategy) -> Self;

          /// Search with custom weights per embedder.
          pub async fn search(
              &self,
              query: &TeleologicalArray,
              weights: &EmbedderWeights,
              limit: usize,
              threshold: Option<f32>,
          ) -> Result<Vec<SearchResult>, SearchError>;

          /// Fuse results from multiple embedders.
          fn fuse_results(
              &self,
              per_embedder_results: Vec<Vec<SearchResult>>,
              weights: &EmbedderWeights,
          ) -> Vec<SearchResult>;
      }

      /// Score fusion method.
      #[derive(Debug, Clone, Copy)]
      pub enum FusionMethod {
          /// Weighted sum of scores
          WeightedSum,
          /// Reciprocal Rank Fusion
          RRF { k: usize },
          /// Max score across embedders
          MaxScore,
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Zero-weight embedders not queried</constraint>
    <constraint>Results sorted by fused score descending</constraint>
    <constraint>Parallel execution provides speedup</constraint>
    <constraint>Staged search enables early termination</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-storage search::weighted</command>
    <command>cargo bench -p context-graph-storage weighted_search</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-storage/src/teleological/search/weighted.rs

use std::collections::HashMap;
use tokio::task;
use futures::future::join_all;

use crate::teleological::store::SearchResult;
use crate::teleological::index::EmbedderIndex;
use crate::teleological::search::single::SearchError;
use context_graph_core::teleology::array::TeleologicalArray;
use context_graph_core::teleology::comparison::EmbedderWeights;
use context_graph_core::teleology::embedder::Embedder;

#[derive(Debug, Clone, Copy)]
pub enum ParallelStrategy {
    FullParallel,
    Staged { stage_size: usize },
    Sequential,
}

#[derive(Debug, Clone, Copy)]
pub enum FusionMethod {
    WeightedSum,
    RRF { k: usize },
    MaxScore,
}

pub struct WeightedFullSearch {
    indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>,
    strategy: ParallelStrategy,
    fusion: FusionMethod,
}

impl WeightedFullSearch {
    pub fn new(indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>) -> Self {
        Self {
            indices,
            strategy: ParallelStrategy::FullParallel,
            fusion: FusionMethod::WeightedSum,
        }
    }

    pub fn with_strategy(
        indices: Vec<Box<dyn EmbedderIndex + Send + Sync>>,
        strategy: ParallelStrategy,
    ) -> Self {
        Self {
            indices,
            strategy,
            fusion: FusionMethod::WeightedSum,
        }
    }

    pub async fn search(
        &self,
        query: &TeleologicalArray,
        weights: &EmbedderWeights,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SearchResult>, SearchError> {
        // Collect indices with non-zero weights
        let active_indices: Vec<_> = self.indices.iter()
            .enumerate()
            .filter(|(i, _)| weights.get(Embedder::from_index(*i).unwrap()) > 0.0)
            .collect();

        if active_indices.is_empty() {
            return Ok(vec![]);
        }

        // Execute based on strategy
        let per_embedder_results = match self.strategy {
            ParallelStrategy::FullParallel => {
                self.search_parallel(query, &active_indices, limit, threshold).await?
            }
            ParallelStrategy::Staged { stage_size } => {
                self.search_staged(query, &active_indices, stage_size, limit, threshold).await?
            }
            ParallelStrategy::Sequential => {
                self.search_sequential(query, &active_indices, limit, threshold).await?
            }
        };

        // Fuse results
        let fused = self.fuse_results(per_embedder_results, weights);

        // Return top-k
        Ok(fused.into_iter().take(limit).collect())
    }

    async fn search_parallel(
        &self,
        query: &TeleologicalArray,
        active_indices: &[(usize, &Box<dyn EmbedderIndex + Send + Sync>)],
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<Vec<SearchResult>>, SearchError> {
        let futures: Vec<_> = active_indices.iter()
            .map(|(idx, index)| {
                let embedder = Embedder::from_index(*idx).unwrap();
                let query_embedding = query.get(embedder).clone();
                async move {
                    index.search(&query_embedding, limit * 2, threshold).await
                }
            })
            .collect();

        let results = join_all(futures).await;

        // Convert IndexSearchResult to SearchResult
        results.into_iter()
            .map(|r| r.map_err(|e| SearchError::Index(e.to_string())))
            .map(|r| r.map(|items| {
                items.into_iter()
                    .map(|ir| SearchResult {
                        array: TeleologicalArray::new(ir.id),
                        similarity: ir.score,
                        per_embedder_scores: [None; 13],
                    })
                    .collect()
            }))
            .collect()
    }

    fn fuse_results(
        &self,
        per_embedder_results: Vec<Vec<SearchResult>>,
        weights: &EmbedderWeights,
    ) -> Vec<SearchResult> {
        // Group by array ID
        let mut score_map: HashMap<uuid::Uuid, (f32, [Option<f32>; 13])> = HashMap::new();

        for (embedder_idx, results) in per_embedder_results.into_iter().enumerate() {
            let embedder = Embedder::from_index(embedder_idx).unwrap();
            let weight = weights.get(embedder);

            for result in results {
                let entry = score_map.entry(result.array.id)
                    .or_insert((0.0, [None; 13]));

                match self.fusion {
                    FusionMethod::WeightedSum => {
                        entry.0 += result.similarity * weight;
                    }
                    FusionMethod::MaxScore => {
                        if result.similarity * weight > entry.0 {
                            entry.0 = result.similarity * weight;
                        }
                    }
                    FusionMethod::RRF { k } => {
                        // RRF score contribution
                        // (score approximated by rank)
                        entry.0 += weight / (k as f32 + 1.0);
                    }
                }

                entry.1[embedder_idx] = Some(result.similarity);
            }
        }

        // Convert to results and sort
        let mut results: Vec<_> = score_map.into_iter()
            .map(|(id, (score, per_embedder))| SearchResult {
                array: TeleologicalArray::new(id),
                similarity: score,
                per_embedder_scores: per_embedder,
            })
            .collect();

        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_weighted_search_respects_weights() {
        // Test that zero-weight embedders are not queried
    }

    #[tokio::test]
    async fn test_fusion_weighted_sum() {
        // Test weighted sum fusion
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-storage/src/teleological/search/weighted.rs">
    Weighted full array search implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-storage/src/teleological/search/mod.rs">
    Add: pub mod weighted;
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>Zero-weight embedders not queried</criterion>
  <criterion>Results sorted by fused score</criterion>
  <criterion>WeightedSum fusion correct</criterion>
  <criterion>RRF fusion correct</criterion>
  <criterion>Parallel strategy faster than sequential</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-storage search::weighted -- --nocapture</command>
</test_commands>
</task_spec>
```
