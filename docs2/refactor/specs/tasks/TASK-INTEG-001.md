# TASK-INTEG-001: Memory MCP Handlers

```xml
<task_spec id="TASK-INTEG-001" version="1.0">
<metadata>
  <title>Implement Memory MCP Tool Handlers</title>
  <status>todo</status>
  <layer>integration</layer>
  <sequence>21</sequence>
  <implements>
    <requirement_ref>REQ-MCP-MEMORY-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-LOGIC-008</task_ref>
    <task_ref>TASK-CORE-006</task_ref>
  </depends_on>
  <estimated_complexity>high</estimated_complexity>
  <estimated_days>3</estimated_days>
</metadata>

<context>
Memory MCP handlers provide the JSON-RPC interface for memory injection, search, and
comparison operations. All handlers work with teleological arrays as the atomic unit.
</context>

<objective>
Implement MCP handlers for memory/inject, memory/inject_batch, memory/search,
memory/search_multi_perspective, memory/compare, memory/batch_compare, and
memory/similarity_matrix tools.
</objective>

<rationale>
Memory handlers are the primary interface for Claude Code:
1. memory/inject triggers autonomous 13-embedder array creation
2. memory/search uses entry-point discovery
3. memory/compare enables apples-to-apples comparison
4. All return per-embedder breakdowns for interpretability
</rationale>

<input_context_files>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#1-memory-injection-tools</file>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#2-search-tools</file>
  <file purpose="mcp_spec">docs2/refactor/08-MCP-TOOLS.md#3-comparison-tools</file>
  <file purpose="search_engine">crates/context-graph-storage/src/teleological/search/engine.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-LOGIC-008 complete (search pipeline exists)</check>
  <check>TASK-CORE-006 complete (storage trait exists)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Implement memory/inject handler</item>
    <item>Implement memory/inject_batch handler</item>
    <item>Implement memory/search handler with all strategy types</item>
    <item>Implement memory/search_multi_perspective handler</item>
    <item>Implement memory/compare handler</item>
    <item>Implement memory/batch_compare handler</item>
    <item>Implement memory/similarity_matrix handler</item>
    <item>JSON-RPC request/response serialization</item>
    <item>Rate limiting and error handling</item>
  </in_scope>
  <out_of_scope>
    <item>Purpose/goal handlers (TASK-INTEG-002)</item>
    <item>Consciousness handlers (TASK-INTEG-003)</item>
    <item>Hook handlers (TASK-INTEG-004)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-mcp/src/handlers/memory.rs">
      use crate::protocol::{JsonRpcRequest, JsonRpcResponse, MethodParams};
      use context_graph_storage::teleological::store::TeleologicalArrayStore;
      use context_graph_storage::teleological::search::TeleologicalSearchEngine;

      /// Memory MCP handler.
      pub struct MemoryHandler {
          store: Arc<dyn TeleologicalArrayStore>,
          search_engine: Arc<TeleologicalSearchEngine>,
          embedder_pipeline: Arc<EmbedderPipeline>,
      }

      impl MemoryHandler {
          pub fn new(
              store: Arc<dyn TeleologicalArrayStore>,
              search_engine: Arc<TeleologicalSearchEngine>,
              embedder_pipeline: Arc<EmbedderPipeline>,
          ) -> Self;

          /// Handle memory/inject request.
          pub async fn handle_inject(
              &self,
              params: InjectParams,
          ) -> Result<InjectResponse, HandlerError>;

          /// Handle memory/inject_batch request.
          pub async fn handle_inject_batch(
              &self,
              params: InjectBatchParams,
          ) -> Result<InjectBatchResponse, HandlerError>;

          /// Handle memory/search request with all strategy types.
          pub async fn handle_search(
              &self,
              params: SearchParams,
          ) -> Result<SearchResponse, HandlerError>;

          /// Handle memory/search_multi_perspective request.
          pub async fn handle_search_multi_perspective(
              &self,
              params: MultiPerspectiveSearchParams,
          ) -> Result<MultiPerspectiveSearchResponse, HandlerError>;

          /// Handle memory/compare request.
          pub async fn handle_compare(
              &self,
              params: CompareParams,
          ) -> Result<CompareResponse, HandlerError>;

          /// Handle memory/batch_compare request.
          pub async fn handle_batch_compare(
              &self,
              params: BatchCompareParams,
          ) -> Result<BatchCompareResponse, HandlerError>;

          /// Handle memory/similarity_matrix request.
          pub async fn handle_similarity_matrix(
              &self,
              params: SimilarityMatrixParams,
          ) -> Result<SimilarityMatrixResponse, HandlerError>;
      }

      // Request/Response types matching MCP spec
      #[derive(Debug, Deserialize)]
      pub struct InjectParams {
          pub content: String,
          pub memory_type: Option<MemoryType>,
          pub namespace: Option<String>,
          pub metadata: Option<serde_json::Value>,
          pub options: Option<InjectOptions>,
      }

      #[derive(Debug, Serialize)]
      pub struct InjectResponse {
          pub memory_id: Uuid,
          pub teleological_array: TeleologicalArraySummary,
          pub indices_updated: usize,
          pub created_at: DateTime<Utc>,
          pub autonomous_goals_affected: Vec<GoalEffect>,
      }

      #[derive(Debug, Deserialize)]
      pub struct SearchParams {
          pub query: String,
          pub strategy: SearchStrategy,
          pub limit: Option<usize>,
          pub threshold: Option<f32>,
          pub options: Option<SearchOptions>,
      }

      #[derive(Debug, Serialize)]
      pub struct SearchResponse {
          pub memories: Vec<SearchResultItem>,
          pub entry_point_discovery: Option<EntryPointAnalysis>,
          pub query_info: QueryInfo,
      }

      // Additional types for other handlers...
    </signature>
  </signatures>

  <constraints>
    <constraint>All 13 embedders generated atomically on inject</constraint>
    <constraint>Search strategies map to correct search implementation</constraint>
    <constraint>Per-embedder scores returned when requested</constraint>
    <constraint>Rate limits enforced per MCP spec</constraint>
  </constraints>

  <verification>
    <command>cargo test -p context-graph-mcp handlers::memory</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-mcp/src/handlers/memory.rs

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::protocol::{HandlerError, ErrorCode};
use context_graph_core::teleology::array::TeleologicalArray;
use context_graph_core::teleology::comparison::ComparisonType;
use context_graph_storage::teleological::store::TeleologicalArrayStore;
use context_graph_storage::teleological::search::TeleologicalSearchEngine;

pub struct MemoryHandler {
    store: Arc<dyn TeleologicalArrayStore + Send + Sync>,
    search_engine: Arc<TeleologicalSearchEngine>,
    embedder_pipeline: Arc<EmbedderPipeline>,
}

impl MemoryHandler {
    pub fn new(
        store: Arc<dyn TeleologicalArrayStore + Send + Sync>,
        search_engine: Arc<TeleologicalSearchEngine>,
        embedder_pipeline: Arc<EmbedderPipeline>,
    ) -> Self {
        Self {
            store,
            search_engine,
            embedder_pipeline,
        }
    }

    pub async fn handle_inject(
        &self,
        params: InjectParams,
    ) -> Result<InjectResponse, HandlerError> {
        // Generate teleological array from content
        let array = self.embedder_pipeline
            .embed_content(&params.content, params.memory_type)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Store the array
        let memory_id = array.id;
        self.store
            .store(&array, params.namespace.as_deref())
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Build response
        let summary = self.build_array_summary(&array, &params.options);
        let goals_affected = self.check_goal_effects(&array).await;

        Ok(InjectResponse {
            memory_id,
            teleological_array: summary,
            indices_updated: 13,
            created_at: Utc::now(),
            autonomous_goals_affected: goals_affected,
        })
    }

    pub async fn handle_inject_batch(
        &self,
        params: InjectBatchParams,
    ) -> Result<InjectBatchResponse, HandlerError> {
        let mut results = Vec::with_capacity(params.memories.len());
        let mut succeeded = 0;
        let mut failed = 0;

        // Process in parallel if requested
        if params.options.as_ref().map(|o| o.parallel).unwrap_or(true) {
            let futures: Vec<_> = params.memories.iter()
                .map(|m| self.handle_inject(InjectParams {
                    content: m.content.clone(),
                    memory_type: m.memory_type.clone(),
                    namespace: m.namespace.clone(),
                    metadata: m.metadata.clone(),
                    options: None,
                }))
                .collect();

            let batch_results = futures::future::join_all(futures).await;

            for result in batch_results {
                match result {
                    Ok(r) => {
                        results.push(BatchInjectResult {
                            memory_id: r.memory_id,
                            status: "success".to_string(),
                            embedders_generated: 13,
                        });
                        succeeded += 1;
                    }
                    Err(e) => {
                        if params.options.as_ref().map(|o| o.stop_on_error).unwrap_or(false) {
                            return Err(e);
                        }
                        failed += 1;
                    }
                }
            }
        }

        Ok(InjectBatchResponse {
            total: params.memories.len(),
            succeeded,
            failed,
            memories: results,
            batch_time_ms: 0, // TODO: measure
            indices_updated: 13,
        })
    }

    pub async fn handle_search(
        &self,
        params: SearchParams,
    ) -> Result<SearchResponse, HandlerError> {
        // Generate query array
        let query_array = self.embedder_pipeline
            .embed_content(&params.query, None)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Convert strategy to comparison type
        let comparison_type = self.strategy_to_comparison_type(&params.strategy)?;

        // Execute search
        let limit = params.limit.unwrap_or(10);
        let threshold = params.threshold;

        let results = self.search_engine
            .search(&query_array, &comparison_type, limit, threshold)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Build response with entry point analysis if auto_discover
        let entry_point_analysis = if matches!(params.strategy, SearchStrategy::AutoDiscover { .. }) {
            Some(self.analyze_entry_points(&query_array, &results))
        } else {
            None
        };

        let memory_items: Vec<_> = results.iter()
            .map(|r| self.result_to_search_item(r, &params.options))
            .collect();

        Ok(SearchResponse {
            memories: memory_items,
            entry_point_discovery: entry_point_analysis,
            query_info: QueryInfo {
                total_entry_points_used: 13, // Simplified
                fusion_method: "weighted".to_string(),
                candidates_scanned: results.len(),
                search_time_ms: 0, // TODO: measure
            },
        })
    }

    pub async fn handle_search_multi_perspective(
        &self,
        params: MultiPerspectiveSearchParams,
    ) -> Result<MultiPerspectiveSearchResponse, HandlerError> {
        // Generate query array
        let query_array = self.embedder_pipeline
            .embed_content(&params.query, None)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

        // Search from each perspective
        let mut all_results = Vec::new();
        for perspective in &params.perspectives {
            let comparison_type = ComparisonType::SingleEmbedder(perspective.embedder);
            let results = self.search_engine
                .search(&query_array, &comparison_type, params.limit.unwrap_or(10), None)
                .await
                .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?;

            all_results.push((perspective.clone(), results));
        }

        // Fuse results using RRF or specified method
        let fused = self.fuse_perspectives(&all_results, &params.synthesis);

        Ok(MultiPerspectiveSearchResponse {
            memories: fused,
            synthesis_info: SynthesisInfo {
                method: params.synthesis.method.clone(),
                total_candidates_per_perspective: all_results.iter().map(|(_, r)| r.len()).collect(),
                unique_candidates_after_fusion: fused.len(),
                synthesis_time_ms: 0,
            },
        })
    }

    pub async fn handle_compare(
        &self,
        params: CompareParams,
    ) -> Result<CompareResponse, HandlerError> {
        // Get or create arrays
        let array_a = match &params.array_a {
            ArrayRef::Id(id) => {
                self.store.retrieve(id)
                    .await
                    .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
                    .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, "Array A not found"))?
            }
            ArrayRef::Query(q) => {
                self.embedder_pipeline.embed_content(q, None).await
                    .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
            }
        };

        let array_b = self.store.retrieve(&params.array_b)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
            .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, "Array B not found"))?;

        // Compare
        let comparison_type = self.parse_comparison_type(&params.comparison_type)?;
        let result = self.search_engine
            .comparator()
            .compare(&array_a, &array_b, &comparison_type);

        // Build response
        let per_embedder = self.build_per_embedder_comparison(&result);
        let correlation = if params.options.as_ref().map(|o| o.include_correlation_analysis).unwrap_or(false) {
            Some(self.analyze_correlation(&result))
        } else {
            None
        };

        Ok(CompareResponse {
            overall_similarity: result.overall_similarity,
            per_embedder_comparison: per_embedder,
            correlation_analysis: correlation,
            interpretation: self.interpret_comparison(&result),
        })
    }

    pub async fn handle_batch_compare(
        &self,
        params: BatchCompareParams,
    ) -> Result<BatchCompareResponse, HandlerError> {
        let reference = self.store.retrieve(&params.reference)
            .await
            .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
            .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, "Reference not found"))?;

        let comparison_type = self.parse_comparison_type(&params.comparison_type)?;
        let mut comparisons = Vec::with_capacity(params.targets.len());

        for target_id in &params.targets {
            if let Some(target) = self.store.retrieve(target_id).await.ok().flatten() {
                let result = self.search_engine
                    .comparator()
                    .compare(&reference, &target, &comparison_type);

                comparisons.push(BatchComparisonItem {
                    target_id: *target_id,
                    overall_similarity: result.overall_similarity,
                    rank: 0, // Set after sorting
                    per_embedder: if params.options.as_ref().map(|o| o.return_per_embedder).unwrap_or(false) {
                        Some(self.build_per_embedder_simple(&result))
                    } else {
                        None
                    },
                });
            }
        }

        // Sort and assign ranks
        comparisons.sort_by(|a, b| b.overall_similarity.partial_cmp(&a.overall_similarity).unwrap());
        for (i, c) in comparisons.iter_mut().enumerate() {
            c.rank = i + 1;
        }

        // Compute statistics
        let stats = if params.options.as_ref().map(|o| o.include_statistics).unwrap_or(false) {
            Some(self.compute_batch_statistics(&comparisons))
        } else {
            None
        };

        Ok(BatchCompareResponse {
            reference_id: params.reference,
            comparisons,
            statistics: stats,
            processing_time_ms: 0,
        })
    }

    pub async fn handle_similarity_matrix(
        &self,
        params: SimilarityMatrixParams,
    ) -> Result<SimilarityMatrixResponse, HandlerError> {
        let n = params.array_ids.len();
        let mut matrix = vec![vec![0.0f32; n]; n];

        // Load all arrays
        let mut arrays = Vec::with_capacity(n);
        for id in &params.array_ids {
            let array = self.store.retrieve(id)
                .await
                .map_err(|e| HandlerError::new(ErrorCode::InternalError, e.to_string()))?
                .ok_or_else(|| HandlerError::new(ErrorCode::InvalidParams, format!("Array {} not found", id)))?;
            arrays.push(array);
        }

        let comparison_type = self.parse_comparison_type(&params.comparison_type)?;

        // Compute pairwise similarities
        for i in 0..n {
            for j in i..n {
                if i == j {
                    matrix[i][j] = 1.0;
                } else {
                    let result = self.search_engine
                        .comparator()
                        .compare(&arrays[i], &arrays[j], &comparison_type);
                    matrix[i][j] = result.overall_similarity;
                    matrix[j][i] = result.overall_similarity;
                }
            }
        }

        // Clustering suggestion if requested
        let clustering = if params.options.as_ref().map(|o| o.include_clustering_suggestion).unwrap_or(false) {
            Some(self.suggest_clustering(&matrix))
        } else {
            None
        };

        Ok(SimilarityMatrixResponse {
            matrix,
            array_ids: params.array_ids,
            clustering_suggestion: clustering,
            embedder_used: "comparison_type".to_string(),
        })
    }

    // Helper methods
    fn strategy_to_comparison_type(&self, strategy: &SearchStrategy) -> Result<ComparisonType, HandlerError> {
        // Convert search strategy to comparison type
        match strategy {
            SearchStrategy::AutoDiscover { .. } => Ok(ComparisonType::default()),
            SearchStrategy::SingleEmbedder { embedder } => Ok(ComparisonType::SingleEmbedder(*embedder)),
            SearchStrategy::EmbedderGroup { group, .. } => Ok(ComparisonType::EmbedderGroup(*group)),
            SearchStrategy::WeightedFull { weights } => Ok(ComparisonType::WeightedFull(weights.clone())),
            SearchStrategy::MatrixStrategy { matrix } => Ok(ComparisonType::MatrixStrategy(matrix.clone())),
        }
    }

    fn parse_comparison_type(&self, ct: &serde_json::Value) -> Result<ComparisonType, HandlerError> {
        // Parse JSON comparison type specification
        serde_json::from_value(ct.clone())
            .map_err(|e| HandlerError::new(ErrorCode::InvalidParams, e.to_string()))
    }

    // Additional helper methods...
}

// Request/Response types
#[derive(Debug, Deserialize)]
pub struct InjectParams {
    pub content: String,
    pub memory_type: Option<MemoryType>,
    pub namespace: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub options: Option<InjectOptions>,
}

#[derive(Debug, Serialize)]
pub struct InjectResponse {
    pub memory_id: Uuid,
    pub teleological_array: TeleologicalArraySummary,
    pub indices_updated: usize,
    pub created_at: DateTime<Utc>,
    pub autonomous_goals_affected: Vec<GoalEffect>,
}

#[derive(Debug, Deserialize)]
pub struct SearchParams {
    pub query: String,
    pub strategy: SearchStrategy,
    pub limit: Option<usize>,
    pub threshold: Option<f32>,
    pub options: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum SearchStrategy {
    #[serde(rename = "auto_discover")]
    AutoDiscover {
        max_entry_points: Option<usize>,
        min_confidence: Option<f32>,
    },
    #[serde(rename = "single_embedder")]
    SingleEmbedder { embedder: Embedder },
    #[serde(rename = "embedder_group")]
    EmbedderGroup {
        group: EmbedderGroup,
        embedders: Option<Vec<Embedder>>,
    },
    #[serde(rename = "weighted_full")]
    WeightedFull { weights: EmbedderWeights },
    #[serde(rename = "matrix_strategy")]
    MatrixStrategy { matrix: SearchMatrix },
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub memories: Vec<SearchResultItem>,
    pub entry_point_discovery: Option<EntryPointAnalysis>,
    pub query_info: QueryInfo,
}

// Additional types...

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inject_handler() {
        // Test memory injection
    }

    #[tokio::test]
    async fn test_search_auto_discover() {
        // Test auto-discover search
    }

    #[tokio::test]
    async fn test_compare_handler() {
        // Test comparison
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-mcp/src/handlers/memory.rs">
    Memory MCP handler implementation
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-mcp/src/handlers/mod.rs">
    Add: pub mod memory;
  </file>
  <file path="crates/context-graph-mcp/src/handlers/core.rs">
    Add dispatch routes for memory/* tools
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>memory/inject generates all 13 embedders</criterion>
  <criterion>memory/search supports all strategy types</criterion>
  <criterion>memory/compare returns per-embedder breakdown</criterion>
  <criterion>JSON-RPC responses match MCP spec</criterion>
  <criterion>Rate limits enforced</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-mcp handlers::memory -- --nocapture</command>
</test_commands>
</task_spec>
```
