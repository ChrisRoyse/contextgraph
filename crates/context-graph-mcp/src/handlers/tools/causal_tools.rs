//! Causal reasoning tool implementations (search_causes, get_causal_chain).
//!
//! # E5 Causal Asymmetric Similarity (ARCH-15, AP-77)
//!
//! These tools leverage the E5 (V_causality) embedder's asymmetric encoding:
//! - `search_causes`: Abductive reasoning to find likely causes of observed effects
//! - `get_causal_chain`: Build and visualize transitive causal chains
//!
//! ## Constitution Compliance
//!
//! - ARCH-15: Uses asymmetric E5 with separate cause/effect encodings
//! - AP-77: Direction modifiers: cause→effect=1.2, effect→cause=0.8
//! - AP-02: All comparisons within E5 space (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging

use serde_json::json;
use std::collections::HashSet;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::causal::asymmetric::{
    compute_e5_asymmetric_fingerprint_similarity, CausalDirection,
};
use context_graph_core::causal::chain::rank_causes_by_abduction;
use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::fingerprint::SemanticFingerprint;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::causal_dtos::{
    CausalChainHop, CausalChainMetadata, CauseSearchMetadata, CauseSearchResult,
    GetCausalChainRequest, GetCausalChainResponse, SearchCausesRequest, SearchCausesResponse,
    SourceInfo, ABDUCTIVE_DAMPENING,
};

use super::super::Handlers;

impl Handlers {
    /// search_causes tool implementation.
    ///
    /// Performs abductive reasoning to find likely causes of an observed effect.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the effect query using all 13 embedders
    /// 2. Search for candidates using causal_reasoning weight profile (5x over-fetch)
    /// 3. Apply abductive scoring using asymmetric E5 similarity
    /// 4. Apply 0.8x dampening per AP-77 (effect→cause direction)
    /// 5. Filter by minScore and return top-K ranked causes
    ///
    /// # Parameters
    ///
    /// - `query`: The observed effect to find causes for (required)
    /// - `topK`: Maximum causes to return (1-50, default: 10)
    /// - `minScore`: Minimum abductive score threshold (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
    /// - `filterCausalDirection`: Filter by persisted causal direction
    pub(crate) async fn call_search_causes(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: SearchCausesRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "search_causes: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_causes: Validation failed");
            return self.tool_error(id, &e);
        }

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            "search_causes: Starting abductive search"
        );

        // Step 1: Embed the effect query
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_causes: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search for candidates (5x over-fetch for abductive reranking)
        let fetch_multiplier = 5;
        let fetch_top_k = top_k * fetch_multiplier;

        // Parse strategy from request - Pipeline enables E13 recall + E12 reranking
        let strategy = request.parse_strategy();
        let enable_rerank = matches!(strategy, SearchStrategy::Pipeline);

        info!(
            strategy = ?strategy,
            enable_rerank = enable_rerank,
            rerank_weight = request.rerank_weight,
            "search_causes: Using search strategy"
        );

        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile("causal_reasoning")
            .with_min_similarity(0.0) // Get all candidates, filter later
            .with_causal_direction(CausalDirection::Cause) // Seeking causes
            .with_rerank(enable_rerank) // Auto-enable E12 for pipeline
            .with_rerank_weight(request.rerank_weight); // User-configurable E12 weight

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_causes: Candidate search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "search_causes: Evaluating candidates for abductive scoring"
        );

        // Step 3: Apply abductive scoring using rank_causes_by_abduction
        let candidate_pairs: Vec<(Uuid, SemanticFingerprint)> = candidates
            .iter()
            .map(|r| (r.fingerprint.id, r.fingerprint.semantic.clone()))
            .collect();

        let abduction_results = rank_causes_by_abduction(&query_embedding, &candidate_pairs);

        // Step 4: Filter by minScore and prepare response
        let mut filtered_count = 0;
        let causes: Vec<CauseSearchResult> = abduction_results
            .into_iter()
            .filter_map(|result| {
                // The abduction score already has 0.8x dampening applied
                let score = result.score;

                if score < min_score {
                    filtered_count += 1;
                    return None;
                }

                Some(CauseSearchResult {
                    cause_id: result.cause_id,
                    score,
                    raw_similarity: result.raw_similarity,
                    causal_direction: None, // Will be populated from source metadata
                    content: None,
                    source: None,
                })
            })
            .take(top_k)
            .collect();

        // Step 5: Optionally filter by causal direction and hydrate content
        let cause_ids: Vec<Uuid> = causes.iter().map(|c| c.cause_id).collect();

        // Get source metadata for causal direction and provenance
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&cause_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    error = %e,
                    "search_causes: Source metadata retrieval failed"
                );
                vec![None; cause_ids.len()]
            }
        };

        // Get content if requested
        let contents: Vec<Option<String>> = if request.include_content && !causes.is_empty() {
            match self.teleological_store.get_content_batch(&cause_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "search_causes: Content retrieval failed");
                    vec![None; cause_ids.len()]
                }
            }
        } else {
            vec![None; cause_ids.len()]
        };

        // Populate metadata and content, filter by causal direction if specified
        let filter_direction = request.filter_causal_direction.as_deref();
        let mut final_causes: Vec<CauseSearchResult> = Vec::with_capacity(causes.len());

        for (i, mut cause) in causes.into_iter().enumerate() {
            // Get causal direction from source metadata
            if let Some(Some(ref metadata)) = source_metadata.get(i) {
                cause.causal_direction = metadata.causal_direction.clone();
                cause.source = Some(SourceInfo {
                    source_type: format!("{}", metadata.source_type),
                    file_path: metadata.file_path.clone(),
                    hook_type: metadata.hook_type.clone(),
                    tool_name: metadata.tool_name.clone(),
                });
            }

            // Apply causal direction filter if specified
            if let Some(filter) = filter_direction {
                if let Some(ref dir) = cause.causal_direction {
                    if dir != filter {
                        filtered_count += 1;
                        continue;
                    }
                }
            }

            // Add content if requested
            if request.include_content {
                cause.content = contents.get(i).and_then(|c| c.clone());
            }

            final_causes.push(cause);
        }

        // Truncate to requested top_k after filtering
        final_causes.truncate(top_k);

        let response = SearchCausesResponse {
            query: query.clone(),
            causes: final_causes.clone(),
            count: final_causes.len(),
            metadata: CauseSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                abductive_dampening: ABDUCTIVE_DAMPENING,
            },
        };

        info!(
            causes_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "search_causes: Completed abductive search"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// search_effects tool implementation.
    ///
    /// Performs forward causal reasoning to find likely effects of a given cause.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the cause query using all 13 embedders
    /// 2. Search for candidates using causal_reasoning weight profile (5x over-fetch)
    /// 3. Apply predictive scoring using asymmetric E5 similarity
    /// 4. Apply 1.2x boost per AP-77 (cause→effect direction)
    /// 5. Filter by minScore and return top-K ranked effects
    ///
    /// # Parameters
    ///
    /// - `query`: The cause to find effects for (required)
    /// - `topK`: Maximum effects to return (1-50, default: 10)
    /// - `minScore`: Minimum predictive score threshold (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
    /// - `filterCausalDirection`: Filter by persisted causal direction
    pub(crate) async fn call_search_effects(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        use super::causal_dtos::{
            EffectSearchMetadata, EffectSearchResult, SearchEffectsRequest, SearchEffectsResponse,
            SourceInfo, PREDICTIVE_BOOST,
        };
        use context_graph_core::causal::chain::rank_effects_by_prediction;

        // Parse and validate request
        let request: SearchEffectsRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "search_effects: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_effects: Validation failed");
            return self.tool_error(id, &e);
        }

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            "search_effects: Starting predictive search"
        );

        // Step 1: Embed the cause query
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_effects: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
        };

        // Step 2: Search for candidates (5x over-fetch for predictive reranking)
        let fetch_multiplier = 5;
        let fetch_top_k = top_k * fetch_multiplier;

        let strategy = request.parse_strategy();
        let enable_rerank = matches!(strategy, SearchStrategy::Pipeline);

        info!(
            strategy = ?strategy,
            enable_rerank = enable_rerank,
            "search_effects: Using search strategy"
        );

        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(strategy)
            .with_weight_profile("causal_reasoning")
            .with_min_similarity(0.0)
            .with_causal_direction(CausalDirection::Effect) // Seeking effects
            .with_rerank(enable_rerank)
            .with_rerank_weight(request.rerank_weight);

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_effects: Candidate search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "search_effects: Evaluating candidates for predictive scoring"
        );

        // Step 3: Apply predictive scoring using rank_effects_by_prediction
        let candidate_pairs: Vec<(Uuid, SemanticFingerprint)> = candidates
            .iter()
            .map(|r| (r.fingerprint.id, r.fingerprint.semantic.clone()))
            .collect();

        let prediction_results = rank_effects_by_prediction(&query_embedding, &candidate_pairs);

        // Step 4: Filter by minScore and prepare response
        let mut filtered_count = 0;
        let effects: Vec<EffectSearchResult> = prediction_results
            .into_iter()
            .filter_map(|result| {
                let score = result.score;

                if score < min_score {
                    filtered_count += 1;
                    return None;
                }

                Some(EffectSearchResult {
                    effect_id: result.effect_id,
                    score,
                    raw_similarity: result.raw_similarity,
                    causal_direction: None,
                    content: None,
                    source: None,
                })
            })
            .take(top_k)
            .collect();

        // Step 5: Optionally filter by causal direction and hydrate content
        let effect_ids: Vec<Uuid> = effects.iter().map(|e| e.effect_id).collect();

        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&effect_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    error = %e,
                    "search_effects: Source metadata retrieval failed"
                );
                vec![None; effect_ids.len()]
            }
        };

        let contents: Vec<Option<String>> = if request.include_content && !effects.is_empty() {
            match self.teleological_store.get_content_batch(&effect_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "search_effects: Content retrieval failed");
                    vec![None; effect_ids.len()]
                }
            }
        } else {
            vec![None; effect_ids.len()]
        };

        // Populate metadata and content
        let filter_direction = request.filter_causal_direction.as_deref();
        let mut final_effects: Vec<EffectSearchResult> = Vec::with_capacity(effects.len());

        for (i, mut effect) in effects.into_iter().enumerate() {
            if let Some(Some(ref metadata)) = source_metadata.get(i) {
                effect.causal_direction = metadata.causal_direction.clone();
                effect.source = Some(SourceInfo {
                    source_type: format!("{}", metadata.source_type),
                    file_path: metadata.file_path.clone(),
                    hook_type: metadata.hook_type.clone(),
                    tool_name: metadata.tool_name.clone(),
                });
            }

            if let Some(filter) = filter_direction {
                if let Some(ref dir) = effect.causal_direction {
                    if dir != filter {
                        filtered_count += 1;
                        continue;
                    }
                }
            }

            if request.include_content {
                effect.content = contents.get(i).and_then(|c: &Option<String>| c.clone());
            }

            final_effects.push(effect);
        }

        final_effects.truncate(top_k);

        let response = SearchEffectsResponse {
            query: query.clone(),
            effects: final_effects.clone(),
            count: final_effects.len(),
            metadata: EffectSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                predictive_boost: PREDICTIVE_BOOST,
            },
        };

        info!(
            effects_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "search_effects: Completed predictive search"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// get_causal_chain tool implementation.
    ///
    /// Builds and visualizes transitive causal chains from an anchor point.
    ///
    /// # Algorithm
    ///
    /// 1. Verify anchor memory exists
    /// 2. Iteratively search for next hop using asymmetric E5 similarity
    /// 3. Track visited memories to avoid cycles
    /// 4. Apply hop attenuation (0.9^hop) for chain scoring
    /// 5. Return chain with per-hop and total scores
    ///
    /// # Parameters
    ///
    /// - `anchorId`: UUID of the starting memory (required)
    /// - `direction`: "forward" (cause→effect) or "backward" (effect→cause)
    /// - `maxHops`: Maximum hops to traverse (1-10, default: 5)
    /// - `minSimilarity`: Minimum similarity for each hop (0-1, default: 0.3)
    /// - `includeContent`: Include full content text (default: false)
    pub(crate) async fn call_get_causal_chain(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: GetCausalChainRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_causal_chain: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let anchor_uuid = match request.validate() {
            Ok(uuid) => uuid,
            Err(e) => {
                error!(error = %e, "get_causal_chain: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        let direction = &request.direction;
        let max_hops = request.max_hops;
        let min_similarity = request.min_similarity;
        let is_forward = request.is_forward();

        info!(
            anchor_id = %anchor_uuid,
            direction = %direction,
            max_hops = max_hops,
            min_similarity = min_similarity,
            "get_causal_chain: Starting chain traversal"
        );

        // Step 1: Verify anchor exists and get its fingerprint
        let anchor_fingerprint = match self
            .teleological_store
            .retrieve(anchor_uuid)
            .await
        {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(anchor_id = %anchor_uuid, "get_causal_chain: Anchor not found");
                return self.tool_error(
                    id,
                    &format!("Anchor memory not found: {}", anchor_uuid),
                );
            }
            Err(e) => {
                error!(error = %e, "get_causal_chain: Failed to get anchor");
                return self.tool_error(id, &format!("Failed to get anchor: {}", e));
            }
        };

        // Step 2: Iteratively build the chain
        let mut chain: Vec<CausalChainHop> = Vec::with_capacity(max_hops);
        let mut visited: HashSet<Uuid> = HashSet::new();
        visited.insert(anchor_uuid);

        let mut current_fingerprint = anchor_fingerprint.semantic.clone();
        let mut cumulative_strength = 1.0_f32;
        let mut total_candidates_evaluated = 0;
        let mut truncated = false;

        for hop_index in 0..max_hops {
            // Search for next hop candidates
            let options = TeleologicalSearchOptions::quick(20) // Get top 20 candidates per hop
                .with_strategy(SearchStrategy::MultiSpace)
                .with_weight_profile("causal_reasoning")
                .with_min_similarity(min_similarity);

            let candidates = match self
                .teleological_store
                .search_semantic(&current_fingerprint, options)
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    warn!(error = %e, hop = hop_index, "get_causal_chain: Hop search failed");
                    break;
                }
            };

            total_candidates_evaluated += candidates.len();

            // Find best unvisited candidate with asymmetric E5 similarity
            let mut best_candidate: Option<(Uuid, f32, f32, SemanticFingerprint)> = None;

            for candidate in candidates {
                let cand_id = candidate.fingerprint.id;

                // Skip if already visited (cycle prevention)
                if visited.contains(&cand_id) {
                    continue;
                }

                // Compute asymmetric E5 similarity
                // Forward: query is cause, doc is effect (use 1.2x modifier)
                // Backward: query is effect, doc is cause (use 0.8x modifier)
                let asymmetric_sim = compute_e5_asymmetric_fingerprint_similarity(
                    &current_fingerprint,
                    &candidate.fingerprint.semantic,
                    is_forward, // query_is_cause
                );

                // Apply direction modifier
                let direction_mod = if is_forward { 1.2 } else { 0.8 };
                let adjusted_sim = asymmetric_sim * direction_mod;

                if adjusted_sim < min_similarity {
                    continue;
                }

                // Track best candidate
                if best_candidate.is_none()
                    || adjusted_sim > best_candidate.as_ref().unwrap().2
                {
                    best_candidate = Some((
                        cand_id,
                        candidate.similarity, // base similarity
                        adjusted_sim,         // asymmetric similarity
                        candidate.fingerprint.semantic.clone(),
                    ));
                }
            }

            // If no valid candidate found, chain ends here
            let (next_id, base_sim, asymmetric_sim, next_fingerprint) = match best_candidate {
                Some(c) => c,
                None => {
                    debug!(hop = hop_index, "get_causal_chain: No more candidates found");
                    break;
                }
            };

            // Infer causal direction of next hop
            let hop_direction = infer_causal_direction(&next_fingerprint);

            // Create hop with computed cumulative strength
            let hop = CausalChainHop::new(
                next_id,
                hop_index,
                base_sim,
                asymmetric_sim,
                cumulative_strength,
                hop_direction,
            );

            // Update state for next iteration
            cumulative_strength = hop.cumulative_strength;
            current_fingerprint = next_fingerprint;
            visited.insert(next_id);
            chain.push(hop);

            // Check if we hit max hops
            if hop_index + 1 >= max_hops {
                truncated = true;
            }
        }

        // Step 3: Optionally hydrate content
        if request.include_content && !chain.is_empty() {
            let hop_ids: Vec<Uuid> = chain.iter().map(|h| h.memory_id).collect();
            let contents = match self.teleological_store.get_content_batch(&hop_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "get_causal_chain: Content retrieval failed");
                    vec![None; hop_ids.len()]
                }
            };

            for (i, hop) in chain.iter_mut().enumerate() {
                if let Some(Some(ref content)) = contents.get(i) {
                    hop.content = Some(content.clone());
                }
            }
        }

        // Step 4: Build response
        let total_score = if chain.is_empty() {
            0.0
        } else {
            chain.last().map(|h| h.cumulative_strength).unwrap_or(0.0)
        };

        let response = GetCausalChainResponse {
            anchor_id: anchor_uuid,
            direction: direction.clone(),
            chain: chain.clone(),
            total_chain_score: total_score,
            hop_count: chain.len(),
            truncated,
            metadata: CausalChainMetadata {
                max_hops,
                min_similarity,
                hop_attenuation: 0.9, // HOP_ATTENUATION constant
                total_candidates_evaluated,
            },
        };

        info!(
            anchor_id = %anchor_uuid,
            direction = %direction,
            hops_found = chain.len(),
            total_score = total_score,
            truncated = truncated,
            "get_causal_chain: Completed chain traversal"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }
}

/// Infer causal direction from a semantic fingerprint's E5 embeddings.
///
/// Documents that describe causes tend to have stronger "as_cause" vectors,
/// while documents describing effects have stronger "as_effect" vectors.
fn infer_causal_direction(fingerprint: &SemanticFingerprint) -> CausalDirection {
    let cause_vec = fingerprint.get_e5_as_cause();
    let effect_vec = fingerprint.get_e5_as_effect();

    // Compare vector norms as a proxy for directional strength
    let cause_norm: f32 = cause_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    let effect_norm: f32 = effect_vec.iter().map(|x| x * x).sum::<f32>().sqrt();

    // Require >10% difference to be confident in direction
    let threshold = 0.1;
    let diff_ratio = if effect_norm > f32::EPSILON {
        (cause_norm - effect_norm) / effect_norm
    } else if cause_norm > f32::EPSILON {
        1.0 // All cause, no effect
    } else {
        0.0 // Both zero
    };

    if diff_ratio > threshold {
        CausalDirection::Cause
    } else if diff_ratio < -threshold {
        CausalDirection::Effect
    } else {
        CausalDirection::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_causal_direction_cause() {
        // Create a fingerprint where cause norm > effect norm
        let mut fp = SemanticFingerprint::zeroed();
        // Set e5_as_cause to have higher norm than e5_as_effect
        fp.e5_causal_as_cause = vec![1.0, 0.5, 0.3]; // norm ~= 1.14
        fp.e5_causal_as_effect = vec![0.5, 0.2, 0.1]; // norm ~= 0.55

        let direction = infer_causal_direction(&fp);
        assert_eq!(direction, CausalDirection::Cause);
        println!("[PASS] Correctly inferred Cause direction");
    }

    #[test]
    fn test_infer_causal_direction_effect() {
        // Create a fingerprint where effect norm > cause norm
        let mut fp = SemanticFingerprint::zeroed();
        fp.e5_causal_as_cause = vec![0.5, 0.2, 0.1]; // norm ~= 0.55
        fp.e5_causal_as_effect = vec![1.0, 0.5, 0.3]; // norm ~= 1.14

        let direction = infer_causal_direction(&fp);
        assert_eq!(direction, CausalDirection::Effect);
        println!("[PASS] Correctly inferred Effect direction");
    }

    #[test]
    fn test_infer_causal_direction_unknown() {
        // Create a fingerprint where norms are similar
        let mut fp = SemanticFingerprint::zeroed();
        fp.e5_causal_as_cause = vec![1.0, 0.5, 0.3];
        fp.e5_causal_as_effect = vec![1.0, 0.5, 0.3];

        let direction = infer_causal_direction(&fp);
        assert_eq!(direction, CausalDirection::Unknown);
        println!("[PASS] Correctly inferred Unknown direction for equal norms");
    }

    #[test]
    fn test_infer_causal_direction_empty_vectors() {
        // Create a fingerprint with empty vectors
        let fp = SemanticFingerprint::zeroed();
        let direction = infer_causal_direction(&fp);
        assert_eq!(direction, CausalDirection::Unknown);
        println!("[PASS] Correctly handled empty vectors");
    }
}
