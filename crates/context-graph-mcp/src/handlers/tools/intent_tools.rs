//! Intent-aware tool implementations (search_by_intent, find_contextual_matches).
//!
//! # E10 Query→Document Retrieval (E5-base-v2)
//!
//! These tools leverage the E10 embedder (intfloat/e5-base-v2) for asymmetric
//! query→document retrieval:
//! - User queries → encoded with "query: " prefix → `get_e10_as_intent()`
//! - Stored memories → encoded with "passage: " prefix → `get_e10_as_context()`
//!
//! Both tools use the SAME direction (query→document), as this is how E5-base-v2
//! was trained. E10 ENHANCES E1 semantic search, it doesn't replace it.
//!
//! ## Constitution Compliance
//!
//! - ARCH-12: E1 is the semantic foundation, E10 enhances
//! - ARCH-15: Uses E5-base-v2's query/passage prefix-based asymmetry
//! - E10 ENHANCES E1 semantic search via blendWithSemantic parameter
//! - AP-02: All comparisons within respective spaces (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging

use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::audit::{AuditOperation, AuditRecord};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::intent_dtos::{
    IntentBoostConfig, IntentSearchMetadata, IntentSearchResult, SearchByIntentRequest,
    SearchByIntentResponse, SourceInfo,
};

use super::super::Handlers;

impl Handlers {
    /// search_by_intent tool implementation.
    ///
    /// Finds memories that share similar intent or purpose using E10 (E5-base-v2).
    /// ENHANCES E1 semantic search with intent awareness via blendWithSemantic parameter.
    ///
    /// Supports both query-based and context-based searches:
    /// - `query`: Intent/goal to search for (uses "intent_search" weight profile)
    /// - `context`: Situation/context to find relevant memories (uses "balanced" profile)
    ///
    /// # Algorithm
    ///
    /// 1. Embed the intent query using all 13 embedders
    /// 2. Search using E1 semantic as primary, E10 as enhancement
    /// 3. Compute E10 similarity (query→document direction per E5-base-v2 training)
    /// 4. Blend E1 and E10 scores using blendWithSemantic weight
    /// 5. Filter by minScore and return top-K results
    ///
    /// # Parameters
    ///
    /// - `query`: The intent or goal to search for (one of query/context required)
    /// - `context`: The context or situation to find relevant memories for (alternative to query)
    /// - `topK`: Maximum results to return (1-50, default: 10)
    /// - `minScore`: Minimum blended score threshold (0-1, default: 0.2)
    /// - `blendWithSemantic`: E10 weight in blend (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
    /// - `weightProfile`: Override the default weight profile
    pub(crate) async fn call_search_by_intent(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: SearchByIntentRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "search_by_intent: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_by_intent: Validation failed");
            return self.tool_error(id, &e);
        }

        // Get the effective query text (either query or context)
        let query_text = request.get_query_text().unwrap(); // Safe after validation
        let is_context_based = request.is_context_based();
        let weight_profile = request.get_weight_profile();
        let top_k = request.top_k;
        let min_score = request.min_score;

        // Parse strategy from request - Pipeline enables E13 recall + E12 reranking
        let strategy = request.parse_strategy();
        let enable_rerank = matches!(strategy, SearchStrategy::Pipeline);

        // Use multiplicative boost (ARCH-17) instead of linear blending
        // E10 ENHANCES E1, it doesn't compete with it
        let intent_boost_config = IntentBoostConfig::default();

        info!(
            query_preview = %query_text.chars().take(50).collect::<String>(),
            is_context_based = is_context_based,
            weight_profile = weight_profile,
            top_k = top_k,
            min_score = min_score,
            strategy = ?strategy,
            enable_rerank = enable_rerank,
            boost_mode = "multiplicative",
            "search_by_intent: Starting intent-enhanced search (ARCH-17)"
        );

        // Step 1: Embed the intent query
        let query_embedding = match self.multi_array_provider.embed_all(query_text).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_by_intent: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search for candidates (3x over-fetch for blended reranking)
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        // Resolve custom weights if provided (overrides weight profile per constitution)
        // AP-NAV-01: FAIL FAST on invalid embedder names
        let custom_weight_array: Option<[f32; 13]> = if let Some(ref custom) = request.custom_weights {
            const VALID_NAMES: [&str; 13] = ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"];
            for key in custom.keys() {
                if !VALID_NAMES.contains(&key.as_str()) {
                    error!(invalid_key = %key, "search_by_intent: invalid embedder name in customWeights");
                    return self.tool_error(id, &format!(
                        "Invalid embedder name '{}' in customWeights. Valid names: E1-E13.", key
                    ));
                }
            }
            let mut w = [0.0f32; 13];
            for (i, name) in VALID_NAMES.iter().enumerate() {
                if let Some(&val) = custom.get(*name) {
                    w[i] = val as f32;
                }
            }
            if let Err(e) = context_graph_core::weights::validate_weights(&w) {
                error!(error = %e, "search_by_intent: invalid custom weights");
                return self.tool_error(id, &format!("Invalid custom weights: {}", e));
            }
            Some(w)
        } else {
            None
        };

        // Resolve exclude embedders
        let mut exclude_indices: Vec<usize> = Vec::new();
        for name in &request.exclude_embedders {
            match name.as_str() {
                "E1" => exclude_indices.push(0), "E2" => exclude_indices.push(1),
                "E3" => exclude_indices.push(2), "E4" => exclude_indices.push(3),
                "E5" => exclude_indices.push(4), "E6" => exclude_indices.push(5),
                "E7" => exclude_indices.push(6), "E8" => exclude_indices.push(7),
                "E9" => exclude_indices.push(8), "E10" => exclude_indices.push(9),
                "E11" => exclude_indices.push(10), "E12" => exclude_indices.push(11),
                "E13" => exclude_indices.push(12),
                _ => {
                    error!(invalid = %name, "search_by_intent: invalid embedder in excludeEmbedders");
                    return self.tool_error(id, &format!(
                        "Invalid embedder '{}' in excludeEmbedders. Valid names: E1-E13.", name
                    ));
                }
            }
        }

        // Use the appropriate weight profile based on query type
        let mut options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile(weight_profile)
            .with_min_similarity(0.0) // Get all candidates, filter later
            .with_rerank(enable_rerank); // Auto-enable E12 for pipeline

        if let Some(weights) = custom_weight_array {
            options = options.with_custom_weights(weights);
        }
        if !exclude_indices.is_empty() {
            options = options.with_exclude_embedders(exclude_indices);
        }

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_by_intent: Candidate search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "search_by_intent: Evaluating candidates for blended scoring"
        );

        // Step 3: Compute intent-enhanced scores using multiplicative boost (ARCH-17)
        // E10 ENHANCES E1 - it doesn't compete via linear blending
        let query_e10_intent = query_embedding.get_e10_as_intent();
        let query_e1 = &query_embedding.e1_semantic;

        let mut scored_results: Vec<(Uuid, f32, f32, f32, f32)> = Vec::with_capacity(candidates.len());

        for candidate in &candidates {
            let cand_id = candidate.fingerprint.id;
            let cand_e1 = &candidate.fingerprint.semantic.e1_semantic;
            let cand_e10_context = candidate.fingerprint.semantic.get_e10_as_context();

            // E1 cosine similarity (THE semantic foundation per ARCH-12)
            let e1_sim = cosine_similarity(query_e1, cand_e1);

            // E10 asymmetric similarity (query→document direction per E5-base-v2)
            let e10_sim = cosine_similarity(query_e10_intent, cand_e10_context);

            // Apply multiplicative boost (ARCH-17)
            // E10 ENHANCES E1 based on intent alignment:
            // - E10 > 0.5: Intent aligned → boost E1 up
            // - E10 < 0.5: Intent misaligned → reduce E1
            // - E10 ≈ 0.5: Neutral → no change
            let enhanced_score = intent_boost_config.compute_enhanced_score(e1_sim, e10_sim);

            if enhanced_score >= min_score {
                scored_results.push((cand_id, enhanced_score, e1_sim, e10_sim, e10_sim));
            }
        }

        // Step 4: Sort by blended score and take top-K
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(top_k);

        let filtered_count = candidates_evaluated - scored_results.len();

        // Step 5: Build results with optional content
        let result_ids: Vec<Uuid> = scored_results.iter().map(|r| r.0).collect();

        // Get content if requested - FAIL FAST on error
        let contents: Vec<Option<String>> = if request.include_content && !result_ids.is_empty() {
            match self.teleological_store.get_content_batch(&result_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        result_count = result_ids.len(),
                        "search_by_intent: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} results: {}",
                            result_ids.len(),
                            e
                        ),
                    );
                }
            }
        } else {
            vec![None; result_ids.len()]
        };

        // Get source metadata - FAIL FAST on error
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&result_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    result_count = result_ids.len(),
                    "search_by_intent: Source metadata retrieval FAILED"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} results: {}",
                        result_ids.len(),
                        e
                    ),
                );
            }
        };

        // Build response
        let mut results: Vec<IntentSearchResult> = Vec::with_capacity(scored_results.len());

        for (i, (memory_id, score, e1_sim, e10_sim, _)) in scored_results.into_iter().enumerate() {
            let source = source_metadata.get(i).and_then(|m| {
                m.as_ref().map(|meta| SourceInfo {
                    source_type: format!("{}", meta.source_type),
                    file_path: meta.file_path.clone(),
                    hook_type: meta.hook_type.clone(),
                    tool_name: meta.tool_name.clone(),
                })
            });

            results.push(IntentSearchResult {
                memory_id,
                score,
                e1_similarity: e1_sim,
                e10_similarity: e10_sim,
                content: contents.get(i).and_then(|c| c.clone()),
                source,
            });
        }

        let response = SearchByIntentResponse {
            query: query_text.to_string(),
            results: results.clone(),
            count: results.len(),
            metadata: IntentSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                // Multiplicative boost mode: E10 enhances E1, weights are not applicable
                // Keeping fields for backward compatibility, but values reflect new approach
                blend_weight: 0.0,  // Not used in multiplicative boost mode
                e1_weight: 1.0,     // E1 is THE foundation (ARCH-12)
                direction_modifier: 1.0, // No modifier - E5-base-v2 uses natural prefix asymmetry
            },
            is_context_based,
            weight_profile: weight_profile.to_string(),
        };

        info!(
            results_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            is_context_based = is_context_based,
            "search_by_intent: Completed intent-aware search"
        );

        // Emit SearchPerformed audit record for read-path provenance
        let audit_record = AuditRecord::new(
            AuditOperation::SearchPerformed {
                tool_name: "search_by_intent".to_string(),
                results_returned: response.count,
                weight_profile: Some(weight_profile.to_string()),
                strategy: Some(format!("{:?}", strategy)),
            },
            // Use first result ID as target, or nil if no results
            result_ids.first().copied().unwrap_or(Uuid::nil()),
        )
        .with_operator("search_by_intent")
        .with_parameters(json!({
            "query_preview": query_text.chars().take(100).collect::<String>(),
            "is_context_based": is_context_based,
            "top_k": top_k,
            "min_score": min_score,
            "candidates_evaluated": candidates_evaluated,
            "filtered_by_score": filtered_count,
            "weight_profile": weight_profile,
        }));

        if let Err(e) = self.teleological_store.append_audit_record(&audit_record).await {
            warn!(error = %e, "search_by_intent: Failed to write audit record (non-fatal)");
        }

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }
}

/// Compute cosine similarity between two vectors.
///
/// Returns 0.0 if either vector is empty or has zero norm.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_empty() {
        let a: Vec<f32> = vec![];
        let b = vec![1.0, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn test_cosine_similarity_zero_norm() {
        let a = vec![0.0, 0.0, 0.0];
        let b = vec![1.0, 2.0, 3.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    #[ignore = "Design documentation only - direction enforced by get_e10_as_intent/get_e10_as_context implementations"]
    fn test_e10_uses_query_document_direction() {
        // search_by_intent uses query→document direction (per E5-base-v2 training)
        // whether using the `query` or `context` parameter:
        // - User queries → "query:" prefix → get_e10_as_intent()
        // - Stored memories → "passage:" prefix → get_e10_as_context()
        // No artificial modifiers are applied - E5's prefix asymmetry is sufficient
        //
        // This test documents the design decision; the actual direction is enforced
        // by the function implementations using the correct get_e10_as_* methods.
        //
        // To verify: inspect TeleologicalFingerprint::get_e10_as_intent() and
        // TeleologicalFingerprint::get_e10_as_context() in context-graph-core.
    }
}
