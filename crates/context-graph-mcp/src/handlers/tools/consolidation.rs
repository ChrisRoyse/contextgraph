//! Consolidation tool handler.
//!
//! PRD v6 Section 10.1: trigger_consolidation is a Core tool.

use serde::Deserialize;
use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::causal::asymmetric::{
    infer_direction_from_fingerprint, CausalDirection,
};
use context_graph_core::types::audit::{AuditOperation, AuditRecord};

use crate::handlers::Handlers;
use crate::handlers::tools::helpers::cosine_similarity;
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
    #[allow(dead_code)] // Populated from storage; used by future text-based consolidation strategies
    text: String,
    access_count: u32,
    /// Gap 5: E5 causal direction for this memory (Cause, Effect, or Unknown)
    causal_direction: CausalDirection,
}

impl MemoryContent {
    fn new(id: MemoryId, embedding: Vec<f32>, text: String, causal_direction: CausalDirection) -> Self {
        Self {
            id,
            embedding,
            text,
            access_count: 0,
            causal_direction,
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
}

/// Configuration for consolidation service.
#[derive(Debug, Clone)]
struct ConsolidationConfig {
    enabled: bool,
    similarity_threshold: f32,
    max_daily_merges: usize,
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
                // Gap 5: Reject pairs with opposing E5 causal directions.
                // Merging cause-oriented with effect-oriented memories destroys
                // the directional signal used by search_causes/search_effects.
                let dir_a = pair.first.causal_direction;
                let dir_b = pair.second.causal_direction;
                if (dir_a == CausalDirection::Cause && dir_b == CausalDirection::Effect)
                    || (dir_a == CausalDirection::Effect && dir_b == CausalDirection::Cause)
                {
                    debug!(
                        first = %pair.first.id.0,
                        second = %pair.second.id.0,
                        dir_a = ?dir_a,
                        dir_b = ?dir_b,
                        "consolidation: Skipping pair with opposing causal directions"
                    );
                    return None;
                }

                // MCP-05 FIX: Use cosine similarity instead of raw dot product.
                // Dot product can exceed 1.0 for non-normalized embeddings.
                let sim = cosine_similarity(&pair.first.embedding, &pair.second.embedding);

                // Accept if high similarity
                if sim >= self.config.similarity_threshold {
                    Some(ConsolidationCandidate {
                        source_ids: vec![pair.first.id, pair.second.id],
                        target_id: pair.first.id, // Keep the first as target
                        similarity: sim,
                    })
                } else {
                    None
                }
            })
            .take(self.config.max_daily_merges)
            .collect()
    }
}

// LOW-15: cosine_similarity moved to crate::handlers::tools::helpers (shared across 4 tool modules).

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
                return self.tool_error(id, &format!("Invalid parameters: {}", e));
            }
        };

        // Validate bounds for max_memories and min_similarity
        if params.max_memories == 0 || params.max_memories > 10000 {
            return self.tool_error(
                id,
                &format!(
                    "max_memories must be between 1 and 10000, got {}",
                    params.max_memories
                ),
            );
        }
        if params.min_similarity < 0.0 || params.min_similarity > 1.0 {
            return self.tool_error(
                id,
                &format!(
                    "min_similarity must be between 0.0 and 1.0, got {}",
                    params.min_similarity
                ),
            );
        }

        // Validate strategy
        let valid_strategies = ["similarity", "temporal", "semantic"];
        if !valid_strategies.contains(&params.strategy.as_str()) {
            error!(
                strategy = %params.strategy,
                "trigger_consolidation: Invalid strategy"
            );
            return self.tool_error(
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

        // MED-13 FIX: Use unbiased fingerprint scan instead of semantic search
        // with hardcoded "context memory patterns" query (which biased retrieval).
        let unbiased_fingerprints = match self
            .teleological_store
            .list_fingerprints_unbiased(params.max_memories)
            .await
        {
            Ok(fps) => fps,
            Err(e) => {
                error!(
                    error = %e,
                    max_memories = params.max_memories,
                    "trigger_consolidation: Unbiased fingerprint scan failed"
                );
                return self.tool_error(
                    id,
                    &format!("Store error: Failed to list fingerprints: {}", e),
                );
            }
        };

        debug!(
            fingerprint_count = unbiased_fingerprints.len(),
            "trigger_consolidation: Retrieved fingerprints (unbiased scan)"
        );

        // Batch-fetch content text for consolidation analysis (MED-04 fix)
        let fp_ids: Vec<Uuid> = unbiased_fingerprints.iter().map(|fp| fp.id).collect();
        // ERR-3 FIX: FAIL FAST on content fetch failure instead of substituting empty
        // strings. Consolidation on empty content silently merges unrelated memories.
        let content_texts = match self.teleological_store.get_content_batch(&fp_ids).await {
            Ok(texts) => texts,
            Err(e) => {
                error!(
                    error = %e,
                    fingerprint_count = fp_ids.len(),
                    "trigger_consolidation: Content batch fetch FAILED — aborting consolidation"
                );
                return self.tool_error(
                    id,
                    &format!("Content fetch failed — cannot consolidate without content: {}", e),
                );
            }
        };

        // Convert TeleologicalFingerprint to MemoryContent
        let mut memory_contents: Vec<MemoryContent> = Vec::with_capacity(unbiased_fingerprints.len());
        let mut fingerprints: Vec<(Uuid, chrono::DateTime<chrono::Utc>)> = Vec::new();

        for (idx, fp) in unbiased_fingerprints.iter().enumerate() {
            // MED-9 FIX: Skip memories with missing content instead of using empty string.
            // Cosine similarity between an empty-string embedding and real content is meaningless
            // and would produce false consolidation candidates.
            let text = match content_texts.get(idx).and_then(|c| c.clone()) {
                Some(t) if !t.is_empty() => t,
                _ => {
                    warn!(
                        memory_id = %fp.id,
                        "trigger_consolidation: Skipping memory with missing/empty content — \
                         cannot compute meaningful similarity"
                    );
                    continue;
                }
            };

            // Use E1 (semantic 1024D) embedding for comparison
            let embedding = fp.semantic.e1_semantic.clone();

            // Gap 5: Infer E5 causal direction for consolidation direction-check
            let direction = infer_direction_from_fingerprint(&fp.semantic);

            let content = MemoryContent::new(MemoryId(fp.id), embedding, text, direction)
                .with_access_count(fp.access_count as u32);

            memory_contents.push(content);
            fingerprints.push((fp.id, fp.created_at));
        }

        // Build pairs based on strategy.
        //
        // MCP-3 NOTE: Strategies differ in PAIR SELECTION, not scoring.
        // All pairs are scored by cosine similarity in find_consolidation_candidates().
        //  - "similarity": Pre-filters by cosine >= threshold * 0.9 (most selective)
        //  - "temporal": 24h time window only — temporally close but potentially dissimilar
        //  - "semantic": No pre-filter — all O(n²) pairs sent for cosine scoring
        // The consolidation service makes the final merge/skip decision via cosine threshold.
        let pairs: Vec<MemoryPair> = match params.strategy.as_str() {
            "similarity" => {
                let mut pairs = Vec::new();
                let threshold = params.min_similarity * 0.9;

                for i in 0..memory_contents.len() {
                    for j in (i + 1)..memory_contents.len() {
                        // MCP-05 FIX: Use cosine similarity instead of raw dot product.
                        let sim = cosine_similarity(
                            &memory_contents[i].embedding,
                            &memory_contents[j].embedding,
                        );

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
                // Semantic strategy: pair all memories
                let mut pairs = Vec::new();
                for i in 0..memory_contents.len() {
                    for j in (i + 1)..memory_contents.len() {
                        pairs.push(MemoryPair::new(
                            memory_contents[i].clone(),
                            memory_contents[j].clone(),
                        ));
                    }
                }
                pairs
            }
            other => {
                error!(strategy = %other, "trigger_consolidation: Unknown strategy passed validation");
                return self.tool_error(id, &format!("Unknown strategy '{}'", other));
            }
        };

        // Create consolidation service
        let config = ConsolidationConfig {
            enabled: true,
            similarity_threshold: params.min_similarity,
            max_daily_merges: 50,
        };
        let consolidation_service = ConsolidationService::with_config(config);

        // Find consolidation candidates
        let candidates = consolidation_service.find_consolidation_candidates(&pairs);

        // P0: Emit ConsolidationAnalyzed audit record (was dead code - now wired)
        {
            let audit_record = AuditRecord::new(
                AuditOperation::ConsolidationAnalyzed {
                    candidates_found: candidates.len(),
                },
                Uuid::new_v4(), // Consolidation affects the store as a whole
            )
            .with_rationale(format!(
                "Consolidation analysis: {} pairs evaluated, {} candidates found (strategy: {})",
                pairs.len(),
                candidates.len(),
                params.strategy,
            ))
            .with_parameters(json!({
                "strategy": params.strategy,
                "similarity_threshold": params.min_similarity,
                "max_memories": params.max_memories,
                "fingerprints_analyzed": memory_contents.len(),
                "pairs_evaluated": pairs.len(),
            }));

            if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
                error!(error = %e, "trigger_consolidation: Failed to write ConsolidationAnalyzed audit record");
                // Non-fatal - continue with response
            }
        }

        let statistics = json!({
            "pairs_evaluated": pairs.len(),
            "candidates_found": candidates.len(),
            "strategy": params.strategy,
            "similarity_threshold": params.min_similarity,
            "max_memories_limit": params.max_memories,
            "fingerprints_analyzed": memory_contents.len()
        });

        let consolidation_result = json!({
            "status": if candidates.is_empty() { "no_candidates" } else { "candidates_found" },
            "candidate_count": candidates.len(),
            "note": "This tool identifies consolidation candidates but does not merge them. Use merge_concepts to execute merges."
        });

        let candidates_sample: Vec<serde_json::Value> = candidates
            .iter()
            .take(10)
            .map(|c| {
                json!({
                    "source_ids": c.source_ids.iter().map(|id| id.0.to_string()).collect::<Vec<_>>(),
                    "target_id": c.target_id.0.to_string(),
                    "similarity": c.similarity
                })
            })
            .collect();

        info!(
            candidate_count = candidates.len(),
            pairs_evaluated = pairs.len(),
            strategy = %params.strategy,
            "trigger_consolidation: Analysis complete"
        );

        self.tool_result(
            id,
            json!({
                "consolidation_result": consolidation_result,
                "statistics": statistics,
                "candidates_sample": candidates_sample
            }),
        )
    }
}
