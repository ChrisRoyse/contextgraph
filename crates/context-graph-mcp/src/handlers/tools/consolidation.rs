//! Consolidation tool handler.
//!
//! PRD v6 Section 10.1: trigger_consolidation is a Core tool.

use serde::Deserialize;
use serde_json::json;
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::traits::TeleologicalSearchOptions;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

// ============================================================================
// LOCAL TYPES FOR CONSOLIDATION
// ============================================================================

/// Memory identifier wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct MemoryId(Uuid);

/// Memory content for consolidation analysis.
#[derive(Debug, Clone)]
struct MemoryContent {
    id: MemoryId,
    embedding: Vec<f32>,
    #[allow(dead_code)]
    text: String,
    alignment: f32,
    access_count: u32,
}

impl MemoryContent {
    fn new(id: MemoryId, embedding: Vec<f32>, text: String, alignment: f32) -> Self {
        Self {
            id,
            embedding,
            text,
            alignment,
            access_count: 0,
        }
    }

    fn with_access_count(mut self, count: u32) -> Self {
        self.access_count = count;
        self
    }
}

/// A pair of memories to potentially consolidate.
#[derive(Debug, Clone)]
struct MemoryPair {
    first: MemoryContent,
    second: MemoryContent,
}

impl MemoryPair {
    fn new(first: MemoryContent, second: MemoryContent) -> Self {
        Self { first, second }
    }
}

/// A consolidation candidate - memories that should be merged.
#[derive(Debug)]
struct ConsolidationCandidate {
    source_ids: Vec<MemoryId>,
    target_id: MemoryId,
    similarity: f32,
    combined_alignment: f32,
}

/// Configuration for consolidation service.
#[derive(Debug, Clone)]
struct ConsolidationConfig {
    enabled: bool,
    similarity_threshold: f32,
    #[allow(dead_code)]
    max_daily_merges: usize,
    theta_diff_threshold: f32,
}

/// Service that finds consolidation candidates.
struct ConsolidationService {
    config: ConsolidationConfig,
}

impl ConsolidationService {
    fn with_config(config: ConsolidationConfig) -> Self {
        Self { config }
    }

    fn find_consolidation_candidates(&self, pairs: &[MemoryPair]) -> Vec<ConsolidationCandidate> {
        if !self.config.enabled {
            return Vec::new();
        }

        pairs
            .iter()
            .filter_map(|pair| {
                // Compute similarity
                let sim: f32 = pair
                    .first
                    .embedding
                    .iter()
                    .zip(pair.second.embedding.iter())
                    .map(|(a, b)| a * b)
                    .sum();

                // Compute alignment difference
                let alignment_diff = (pair.first.alignment - pair.second.alignment).abs();

                // Accept if high similarity and small alignment difference
                if sim >= self.config.similarity_threshold
                    && alignment_diff <= self.config.theta_diff_threshold
                {
                    Some(ConsolidationCandidate {
                        source_ids: vec![pair.first.id, pair.second.id],
                        target_id: pair.first.id, // Keep the first as target
                        similarity: sim,
                        combined_alignment: (pair.first.alignment + pair.second.alignment) / 2.0,
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

// ============================================================================
// HANDLER IMPLEMENTATION
// ============================================================================

/// Parameters for trigger_consolidation tool.
#[derive(Debug, Deserialize)]
pub struct TriggerConsolidationParams {
    /// Maximum memories to process in one batch (default: 100)
    #[serde(default = "default_max_memories")]
    pub max_memories: usize,

    /// Consolidation strategy: "similarity", "temporal", "semantic" (default: "similarity")
    #[serde(default = "default_consolidation_strategy")]
    pub strategy: String,

    /// Minimum similarity for consolidation (default: 0.85)
    #[serde(default = "default_consolidation_similarity")]
    pub min_similarity: f32,
}

fn default_max_memories() -> usize {
    100
}

fn default_consolidation_strategy() -> String {
    "similarity".to_string()
}

fn default_consolidation_similarity() -> f32 {
    0.85
}

impl Handlers {
    /// trigger_consolidation tool implementation.
    ///
    /// PRD v6 Section 10.1: Trigger memory consolidation.
    /// Uses ConsolidationService to merge similar memories.
    ///
    /// Arguments:
    /// - max_memories (optional): Maximum to process (default: 100)
    /// - strategy (optional): "similarity", "temporal", "semantic" (default: "similarity")
    /// - min_similarity (optional): Minimum similarity for merge (default: 0.85)
    ///
    /// Returns:
    /// - consolidation_result: Pairs merged and outcome
    /// - statistics: Consolidation metrics
    pub(crate) async fn call_trigger_consolidation(
        &self,
        id: Option<JsonRpcId>,
        arguments: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling trigger_consolidation tool call");

        // Parse parameters
        let params: TriggerConsolidationParams = match serde_json::from_value(arguments) {
            Ok(p) => p,
            Err(e) => {
                error!(error = %e, "trigger_consolidation: Failed to parse parameters");
                return self.tool_error_with_pulse(id, &format!("Invalid parameters: {}", e));
            }
        };

        // Validate strategy
        let valid_strategies = ["similarity", "temporal", "semantic"];
        if !valid_strategies.contains(&params.strategy.as_str()) {
            error!(
                strategy = %params.strategy,
                "trigger_consolidation: Invalid strategy"
            );
            return self.tool_error_with_pulse(
                id,
                &format!(
                    "Invalid strategy '{}'. Valid strategies: similarity, temporal, semantic",
                    params.strategy
                ),
            );
        }

        debug!(
            max_memories = params.max_memories,
            strategy = %params.strategy,
            min_similarity = params.min_similarity,
            "trigger_consolidation: Parsed parameters"
        );

        // Get fingerprints from store using semantic search with a broad query.
        // This retrieves up to max_memories fingerprints for consolidation analysis.
        //
        // NOTE: Using semantic search with a generic query because:
        // 1. search_text("") returns empty (empty strings produce no embeddings)
        // 2. The store trait does not expose a list_all() or sample() method
        //
        // LIMITATION: The query "context memory patterns" may bias retrieval toward
        // semantically similar memories. A future improvement would be to add a
        // store.list_recent(limit) method for unbiased sampling.
        let search_options = TeleologicalSearchOptions::quick(params.max_memories);

        let broad_query = match self
            .multi_array_provider
            .embed_all("context memory patterns")
            .await
        {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "trigger_consolidation: Failed to generate broad query embedding");
                return self.tool_error_with_pulse(
                    id,
                    &format!("Embedding error: Failed to generate query: {}", e),
                );
            }
        };

        let search_results = match self
            .teleological_store
            .search_semantic(&broad_query, search_options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(
                    error = %e,
                    max_memories = params.max_memories,
                    "trigger_consolidation: Store access failed"
                );
                return self.tool_error_with_pulse(
                    id,
                    &format!("Store error: Failed to list fingerprints: {}", e),
                );
            }
        };

        debug!(
            fingerprint_count = search_results.len(),
            "trigger_consolidation: Retrieved fingerprints from store"
        );

        // Convert TeleologicalFingerprint to MemoryContent
        let mut memory_contents: Vec<MemoryContent> = Vec::with_capacity(search_results.len());
        let mut fingerprints: Vec<(Uuid, chrono::DateTime<chrono::Utc>)> = Vec::new();

        for result in search_results.iter() {
            let fp = &result.fingerprint;
            // Use E1 (semantic 1024D) embedding for comparison
            let embedding = fp.semantic.e1_semantic.clone();

            // Use result similarity as alignment proxy
            let alignment = result.similarity;

            let content = MemoryContent::new(MemoryId(fp.id), embedding, String::new(), alignment)
                .with_access_count(fp.access_count as u32);

            memory_contents.push(content);
            fingerprints.push((fp.id, fp.created_at));
        }

        // Build pairs based on strategy
        let pairs: Vec<MemoryPair> = match params.strategy.as_str() {
            "similarity" => {
                let mut pairs = Vec::new();
                let threshold = params.min_similarity * 0.9;

                for i in 0..memory_contents.len() {
                    for j in (i + 1)..memory_contents.len() {
                        let sim: f32 = memory_contents[i]
                            .embedding
                            .iter()
                            .zip(memory_contents[j].embedding.iter())
                            .map(|(a, b)| a * b)
                            .sum();

                        if sim >= threshold {
                            pairs.push(MemoryPair::new(
                                memory_contents[i].clone(),
                                memory_contents[j].clone(),
                            ));
                        }
                    }
                }
                pairs
            }
            "temporal" => {
                let window_secs = 24 * 60 * 60;
                let mut pairs = Vec::new();

                for i in 0..memory_contents.len() {
                    for j in (i + 1)..memory_contents.len() {
                        if i < fingerprints.len() && j < fingerprints.len() {
                            let diff = (fingerprints[i].1 - fingerprints[j].1).num_seconds().abs();
                            if diff < window_secs {
                                pairs.push(MemoryPair::new(
                                    memory_contents[i].clone(),
                                    memory_contents[j].clone(),
                                ));
                            }
                        }
                    }
                }
                pairs
            }
            "semantic" => {
                let mut pairs = Vec::new();
                let alignment_threshold = 0.5;

                for i in 0..memory_contents.len() {
                    for j in (i + 1)..memory_contents.len() {
                        if memory_contents[i].alignment >= alignment_threshold
                            && memory_contents[j].alignment >= alignment_threshold
                        {
                            pairs.push(MemoryPair::new(
                                memory_contents[i].clone(),
                                memory_contents[j].clone(),
                            ));
                        }
                    }
                }
                pairs
            }
            _ => Vec::new(),
        };

        // Create consolidation service
        let config = ConsolidationConfig {
            enabled: true,
            similarity_threshold: params.min_similarity,
            max_daily_merges: 50,
            theta_diff_threshold: 0.05,
        };
        let consolidation_service = ConsolidationService::with_config(config);

        // Find consolidation candidates
        let candidates = consolidation_service.find_consolidation_candidates(&pairs);

        let statistics = json!({
            "pairs_evaluated": pairs.len(),
            "pairs_consolidated": candidates.len(),
            "strategy": params.strategy,
            "similarity_threshold": params.min_similarity,
            "max_memories_limit": params.max_memories,
            "fingerprints_analyzed": memory_contents.len()
        });

        let consolidation_result = json!({
            "status": if candidates.is_empty() { "no_candidates" } else { "candidates_found" },
            "candidate_count": candidates.len(),
            "action_required": !candidates.is_empty()
        });

        let candidates_sample: Vec<serde_json::Value> = candidates
            .iter()
            .take(10)
            .map(|c| {
                json!({
                    "source_ids": c.source_ids.iter().map(|id| id.0.to_string()).collect::<Vec<_>>(),
                    "target_id": c.target_id.0.to_string(),
                    "similarity": c.similarity,
                    "combined_alignment": c.combined_alignment
                })
            })
            .collect();

        info!(
            candidate_count = candidates.len(),
            pairs_evaluated = pairs.len(),
            strategy = %params.strategy,
            "trigger_consolidation: Analysis complete"
        );

        self.tool_result_with_pulse(
            id,
            json!({
                "consolidation_result": consolidation_result,
                "statistics": statistics,
                "candidates_sample": candidates_sample
            }),
        )
    }
}
