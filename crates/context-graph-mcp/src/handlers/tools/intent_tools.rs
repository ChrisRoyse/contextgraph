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

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::intent_dtos::{
    FindContextualMatchesRequest, FindContextualMatchesResponse, IntentSearchMetadata,
    IntentSearchResult, SearchByIntentRequest, SearchByIntentResponse, SourceInfo,
};

use super::super::Handlers;

impl Handlers {
    /// search_by_intent tool implementation.
    ///
    /// Finds memories that share similar intent or purpose using E10 (E5-base-v2).
    /// ENHANCES E1 semantic search with intent awareness via blendWithSemantic parameter.
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
    /// - `query`: The intent or goal to search for (required)
    /// - `topK`: Maximum results to return (1-50, default: 10)
    /// - `minScore`: Minimum blended score threshold (0-1, default: 0.2)
    /// - `blendWithSemantic`: E10 weight in blend (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
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
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_by_intent: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let blend_weight = request.blend_with_semantic;
        let e1_weight = 1.0 - blend_weight;

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            blend_weight = blend_weight,
            "search_by_intent: Starting intent-aware search"
        );

        // Step 1: Embed the intent query
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_by_intent: Query embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search for candidates (3x over-fetch for blended reranking)
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        // Use intent_search weight profile for E10-enhanced retrieval
        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(SearchStrategy::MultiSpace)
            .with_weight_profile("intent_search") // E10=0.25, E1=0.40 per E10 Upgrade
            .with_min_similarity(0.0); // Get all candidates, filter later

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_by_intent: Candidate search FAILED");
                return self.tool_error_with_pulse(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "search_by_intent: Evaluating candidates for blended scoring"
        );

        // Step 3: Compute blended scores
        let query_e10_intent = query_embedding.get_e10_as_intent();
        let query_e1 = &query_embedding.e1_semantic;

        let mut scored_results: Vec<(Uuid, f32, f32, f32, f32)> = Vec::with_capacity(candidates.len());

        for candidate in &candidates {
            let cand_id = candidate.fingerprint.id;
            let cand_e1 = &candidate.fingerprint.semantic.e1_semantic;
            let cand_e10_context = candidate.fingerprint.semantic.get_e10_as_context();

            // E1 cosine similarity
            let e1_sim = cosine_similarity(query_e1, cand_e1);

            // E10 asymmetric similarity (query→document direction per E5-base-v2)
            // No artificial modifier - E5's prefix-based training provides natural asymmetry
            let e10_sim = cosine_similarity(query_e10_intent, cand_e10_context);

            // Blend scores
            let blended_score = e1_weight * e1_sim + blend_weight * e10_sim;

            if blended_score >= min_score {
                scored_results.push((cand_id, blended_score, e1_sim, e10_sim, e10_sim));
            }
        }

        // Step 4: Sort by blended score and take top-K
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(top_k);

        let filtered_count = candidates_evaluated - scored_results.len();

        // Step 5: Build results with optional content
        let result_ids: Vec<Uuid> = scored_results.iter().map(|r| r.0).collect();

        // Get content if requested
        let contents: Vec<Option<String>> = if request.include_content && !result_ids.is_empty() {
            match self.teleological_store.get_content_batch(&result_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "search_by_intent: Content retrieval failed");
                    vec![None; result_ids.len()]
                }
            }
        } else {
            vec![None; result_ids.len()]
        };

        // Get source metadata
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&result_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "search_by_intent: Source metadata retrieval failed");
                vec![None; result_ids.len()]
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
            query: query.clone(),
            results: results.clone(),
            count: results.len(),
            metadata: IntentSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                blend_weight,
                e1_weight,
                direction_modifier: 1.0, // No modifier - E5-base-v2 uses natural prefix asymmetry
            },
        };

        info!(
            results_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "search_by_intent: Completed intent-aware search"
        );

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// find_contextual_matches tool implementation.
    ///
    /// Finds memories relevant to a given context or situation using E10 (E5-base-v2).
    /// ENHANCES E1 semantic search with contextual awareness.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the context query using all 13 embedders
    /// 2. Search using E1 semantic as primary, E10 as enhancement
    /// 3. Compute E10 similarity (query→document direction per E5-base-v2 training)
    /// 4. Blend E1 and E10 scores using blendWithSemantic weight
    /// 5. Filter by minScore and return top-K results
    ///
    /// # Note
    ///
    /// Uses the SAME direction as search_by_intent (query→document) because E5-base-v2
    /// was trained for query→passage retrieval. The user's context description is treated
    /// as a "query" to find relevant stored "passages" (memories).
    ///
    /// # Parameters
    ///
    /// - `context`: The context or situation to find relevant memories for (required)
    /// - `topK`: Maximum results to return (1-50, default: 10)
    /// - `minScore`: Minimum blended score threshold (0-1, default: 0.2)
    /// - `blendWithSemantic`: E10 weight in blend (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
    pub(crate) async fn call_find_contextual_matches(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: FindContextualMatchesRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "find_contextual_matches: Failed to parse request");
                return self.tool_error_with_pulse(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "find_contextual_matches: Validation failed");
            return self.tool_error_with_pulse(id, &e);
        }

        let context = &request.context;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let blend_weight = request.blend_with_semantic;
        let e1_weight = 1.0 - blend_weight;

        info!(
            context_preview = %context.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            blend_weight = blend_weight,
            "find_contextual_matches: Starting context-aware search"
        );

        // Step 1: Embed the context query
        let query_embedding = match self.multi_array_provider.embed_all(context).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "find_contextual_matches: Query embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search for candidates
        let fetch_multiplier = 3;
        let fetch_top_k = top_k * fetch_multiplier;

        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(SearchStrategy::MultiSpace)
            .with_weight_profile("balanced")
            .with_min_similarity(0.0);

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "find_contextual_matches: Candidate search FAILED");
                return self.tool_error_with_pulse(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "find_contextual_matches: Evaluating candidates for blended scoring"
        );

        // Step 3: Compute blended scores (query→document direction per E5-base-v2)
        // FIXED: Use same direction as search_by_intent - user context is a "query"
        // to find relevant stored "passages" (memories)
        let query_e10_intent = query_embedding.get_e10_as_intent();
        let query_e1 = &query_embedding.e1_semantic;

        let mut scored_results: Vec<(Uuid, f32, f32, f32, f32)> = Vec::with_capacity(candidates.len());

        for candidate in &candidates {
            let cand_id = candidate.fingerprint.id;
            let cand_e1 = &candidate.fingerprint.semantic.e1_semantic;
            let cand_e10_context = candidate.fingerprint.semantic.get_e10_as_context();

            // E1 cosine similarity
            let e1_sim = cosine_similarity(query_e1, cand_e1);

            // E10 asymmetric similarity (query→document direction per E5-base-v2)
            // No artificial modifier - E5's prefix-based training provides natural asymmetry
            let e10_sim = cosine_similarity(query_e10_intent, cand_e10_context);

            // Blend scores
            let blended_score = e1_weight * e1_sim + blend_weight * e10_sim;

            if blended_score >= min_score {
                scored_results.push((cand_id, blended_score, e1_sim, e10_sim, e10_sim));
            }
        }

        // Step 4: Sort and truncate
        scored_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored_results.truncate(top_k);

        let filtered_count = candidates_evaluated - scored_results.len();

        // Step 5: Build results
        let result_ids: Vec<Uuid> = scored_results.iter().map(|r| r.0).collect();

        let contents: Vec<Option<String>> = if request.include_content && !result_ids.is_empty() {
            match self.teleological_store.get_content_batch(&result_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "find_contextual_matches: Content retrieval failed");
                    vec![None; result_ids.len()]
                }
            }
        } else {
            vec![None; result_ids.len()]
        };

        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&result_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                warn!(error = %e, "find_contextual_matches: Source metadata retrieval failed");
                vec![None; result_ids.len()]
            }
        };

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

        let response = FindContextualMatchesResponse {
            context: context.clone(),
            results: results.clone(),
            count: results.len(),
            metadata: IntentSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                blend_weight,
                e1_weight,
                direction_modifier: 1.0, // No modifier - E5-base-v2 uses natural prefix asymmetry
            },
        };

        info!(
            results_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "find_contextual_matches: Completed context-aware search"
        );

        self.tool_result_with_pulse(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
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
    fn test_e10_uses_query_document_direction() {
        // Both search_by_intent and find_contextual_matches use the same direction:
        // query→document (per E5-base-v2 training)
        // - User queries → "query:" prefix → get_e10_as_intent()
        // - Stored memories → "passage:" prefix → get_e10_as_context()
        // No artificial modifiers are applied - E5's prefix asymmetry is sufficient
        //
        // This test documents the design decision; the actual direction is enforced
        // by the function implementations using the correct get_e10_as_* methods.
    }
}
