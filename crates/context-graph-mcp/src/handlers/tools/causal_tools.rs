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
#![allow(dead_code)] // Retired E5 tool surface; dispatch fails closed before these handlers run.

use std::collections::HashSet;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::causal::asymmetric::{
    apply_causal_gate, compute_e5_asymmetric_fingerprint_similarity, CausalDirection,
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
use super::helpers::ToolErrorKind;

impl Handlers {
    /// search_causes tool implementation.
    ///
    /// Performs abductive reasoning to find likely causes of an observed effect.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the effect query using all 14 embedders
    /// 2. Search for candidates using semantic_search weight profile (5x over-fetch)
    /// 3. Apply abductive scoring using 80% E1 + 20% E5 blend
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
        let request: SearchCausesRequest =
            match self.parse_request(id.clone(), args, "search_causes") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;

        let search_scope = &request.search_scope;

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            search_scope = %search_scope,
            "search_causes: Starting abductive search"
        );

        // Step 1: Embed the query
        // For "relationships" scope, we only need E5 dual vectors (much faster).
        // For "memories" or "all", we need full 14-embedder fingerprint.
        let search_memories = search_scope == "memories" || search_scope == "all";
        let search_relationships = search_scope == "relationships" || search_scope == "all";

        let query_embedding = if search_memories {
            match self.embed_query(id.clone(), query, "search_causes").await {
                Ok(fp) => Some(fp),
                Err(resp) => return resp,
            }
        } else {
            None
        };

        // Get E5 effect vector for relationship search
        let e5_effect_vec = if search_relationships {
            if let Some(ref fp) = query_embedding {
                // Already have full fingerprint — extract E5 effect vector
                Some(fp.e5_causal_as_effect.clone())
            } else {
                // Relationships-only: use efficient E5-only path
                match self.multi_array_provider.embed_e5_dual(query).await {
                    Ok((_cause_vec, effect_vec)) => Some(effect_vec),
                    Err(e) => {
                        error!(error = %e, "search_causes: E5 dual embedding FAILED");
                        return self.tool_error(id, &format!("E5 embedding failed: {}", e));
                    }
                }
            }
        } else {
            None
        };

        let mut filtered_count = 0;
        let mut candidates_evaluated = 0usize;
        let mut all_causes: Vec<CauseSearchResult> = Vec::new();

        // ---- Path A: Search fingerprint HNSW (memories) ----
        if search_memories {
            let fp = query_embedding.as_ref().unwrap();

            let fetch_multiplier = 5;
            let fetch_top_k = top_k * fetch_multiplier;

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
                .with_weight_profile("semantic_search")
                .with_min_similarity(0.0)
                .with_causal_direction(CausalDirection::Cause)
                .with_rerank(enable_rerank)
                .with_rerank_weight(request.rerank_weight);

            let candidates = match self.teleological_store.search_semantic(fp, options).await {
                Ok(results) => results,
                Err(e) => {
                    error!(error = %e, "search_causes: Candidate search FAILED");
                    return self.tool_error(id, &format!("Search failed: {}", e));
                }
            };

            candidates_evaluated += candidates.len();

            let candidate_pairs: Vec<(Uuid, SemanticFingerprint)> = candidates
                .iter()
                .map(|r| (r.fingerprint.id, r.fingerprint.semantic.clone()))
                .collect();

            let mut abduction_results = rank_causes_by_abduction(fp, &candidate_pairs);

            // Apply causal gate AFTER abduction ranking (Gap 2 fix).
            // rank_causes_by_abduction computes its own 80%E1+20%E5 scores,
            // so the gate must be applied to the abduction scores, not the
            // pre-ranking similarity (which would be discarded).
            // NOTE: fp is the EFFECT fingerprint (user has effect, wants causes).
            // query_is_cause=false because the query IS the effect, not the cause.
            // This matches chain.rs rank_causes_by_abduction which also uses false.
            for result in abduction_results.iter_mut() {
                if let Some((_, stored_fp)) = candidate_pairs
                    .iter()
                    .find(|(id, _)| *id == result.cause_id)
                {
                    let e5_sim = compute_e5_asymmetric_fingerprint_similarity(fp, stored_fp, false);
                    result.score = apply_causal_gate(result.score, e5_sim, true);
                }
            }
            abduction_results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let memory_causes: Vec<CauseSearchResult> = abduction_results
                .into_iter()
                .filter_map(|result| {
                    let score = result.score;
                    if score < min_score {
                        filtered_count += 1;
                        return None;
                    }
                    Some(CauseSearchResult {
                        source_memory_id: result.cause_id,
                        score,
                        raw_similarity: result.raw_similarity,
                        causal_direction: None,
                        content: None,
                        source: None,
                        result_source: if search_scope == "all" {
                            Some("fingerprint".to_string())
                        } else {
                            None
                        },
                    })
                })
                .collect();

            all_causes.extend(memory_causes);
        }

        // ---- Path B: Search CF_CAUSAL_RELATIONSHIPS (relationships) ----
        if search_relationships {
            let effect_vec = e5_effect_vec.as_ref().unwrap();
            let rel_top_k = if search_memories {
                top_k * 2
            } else {
                top_k * 5
            };

            let rel_results = match self
                .teleological_store
                .search_causal_e5(effect_vec, true, rel_top_k)
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    error!(error = %e, "search_causes: Relationship search FAILED");
                    return self.tool_error(id, &format!("Relationship search failed: {}", e));
                }
            };

            candidates_evaluated += rel_results.len();

            // Fetch full CausalRelationships to get source_fingerprint_id
            for (rel_id, similarity) in &rel_results {
                let score = similarity * ABDUCTIVE_DAMPENING; // Apply same dampening
                if score < min_score {
                    filtered_count += 1;
                    continue;
                }

                // Retrieve the relationship to get source_fingerprint_id
                let rel = match self
                    .teleological_store
                    .get_causal_relationship(*rel_id)
                    .await
                {
                    Ok(Some(rel)) => rel,
                    Ok(None) => {
                        warn!(rel_id = %rel_id, "search_causes: Causal relationship not found, skipping");
                        continue;
                    }
                    Err(e) => {
                        error!(rel_id = %rel_id, error = %e, "search_causes: Failed to retrieve causal relationship");
                        return self.tool_error_typed(
                            id,
                            ToolErrorKind::Storage,
                            &format!("Failed to retrieve causal relationship {}: {}", rel_id, e),
                        );
                    }
                };
                all_causes.push(CauseSearchResult {
                    source_memory_id: rel.source_fingerprint_id,
                    score,
                    raw_similarity: *similarity,
                    causal_direction: Some(rel.mechanism_type.clone()),
                    content: None,
                    source: None,
                    result_source: if search_scope == "all" {
                        Some("causal_relationship".to_string())
                    } else {
                        None
                    },
                });
            }
        }

        // ---- Merge and deduplicate ----
        // Sort by score descending
        all_causes.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by source_memory_id (keep highest scoring)
        let mut seen_ids = HashSet::new();
        let causes: Vec<CauseSearchResult> = all_causes
            .into_iter()
            .filter(|c| seen_ids.insert(c.source_memory_id))
            .take(top_k)
            .collect();

        // Step 5: Optionally filter by causal direction and hydrate content
        let cause_ids: Vec<Uuid> = causes.iter().map(|c| c.source_memory_id).collect();

        // Get source metadata for causal direction and provenance
        // FAIL FAST: If metadata retrieval fails, the entire operation fails
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&cause_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    cause_count = cause_ids.len(),
                    "search_causes: Source metadata retrieval FAILED - cannot proceed without metadata"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} causes: {}. \
                         This indicates a storage layer issue that must be debugged.",
                        cause_ids.len(),
                        e
                    ),
                );
            }
        };

        // Get content if requested
        // FAIL FAST: If content retrieval fails when requested, the entire operation fails
        let contents: Vec<Option<String>> = if request.include_content && !causes.is_empty() {
            match self.teleological_store.get_content_batch(&cause_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        cause_count = cause_ids.len(),
                        "search_causes: Content retrieval FAILED - cannot proceed"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} causes: {}. \
                             This indicates a storage layer issue that must be debugged.",
                            cause_ids.len(),
                            e
                        ),
                    );
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
                    start_line: metadata.start_line,
                    end_line: metadata.end_line,
                    chunk_index: metadata.chunk_index,
                    total_chunks: metadata.total_chunks,
                    hook_type: metadata.hook_type.clone(),
                    tool_name: metadata.tool_name.clone(),
                    display_string: Some(metadata.display_string()),
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

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_causes: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }

    /// search_effects tool implementation.
    ///
    /// Performs forward causal reasoning to find likely effects of a given cause.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the cause query using all 14 embedders
    /// 2. Search for candidates using semantic_search weight profile (5x over-fetch)
    /// 3. Apply predictive scoring using 80% E1 + 20% E5 blend
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
        let request: SearchEffectsRequest =
            match self.parse_request(id.clone(), args, "search_effects") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let search_scope = &request.search_scope;

        let search_memories = search_scope == "memories" || search_scope == "all";
        let search_relationships = search_scope == "relationships" || search_scope == "all";

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            top_k = top_k,
            min_score = min_score,
            search_scope = %search_scope,
            "search_effects: Starting predictive search"
        );

        // Step 1: Embed the cause query
        let query_embedding = if search_memories {
            match self.embed_query(id.clone(), query, "search_effects").await {
                Ok(fp) => Some(fp),
                Err(resp) => return resp,
            }
        } else {
            None
        };

        // Get E5 cause vector for relationship search
        let e5_cause_vec = if search_relationships {
            if let Some(ref fp) = query_embedding {
                Some(fp.e5_causal_as_cause.clone())
            } else {
                match self.multi_array_provider.embed_e5_dual(query).await {
                    Ok((cause_vec, _effect_vec)) => Some(cause_vec),
                    Err(e) => {
                        error!(error = %e, "search_effects: E5 dual embedding FAILED");
                        return self.tool_error(id, &format!("E5 embedding failed: {}", e));
                    }
                }
            }
        } else {
            None
        };

        let mut filtered_count = 0;
        let mut candidates_evaluated = 0usize;
        let mut all_effects: Vec<EffectSearchResult> = Vec::new();

        // ---- Path A: Search fingerprint HNSW (memories) ----
        if search_memories {
            let fp = query_embedding.as_ref().unwrap();

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
                .with_weight_profile("semantic_search")
                .with_min_similarity(0.0)
                .with_causal_direction(CausalDirection::Effect)
                .with_rerank(enable_rerank)
                .with_rerank_weight(request.rerank_weight);

            let candidates = match self.teleological_store.search_semantic(fp, options).await {
                Ok(results) => results,
                Err(e) => {
                    error!(error = %e, "search_effects: Candidate search FAILED");
                    return self.tool_error(id, &format!("Search failed: {}", e));
                }
            };

            candidates_evaluated += candidates.len();

            let candidate_pairs: Vec<(Uuid, SemanticFingerprint)> = candidates
                .iter()
                .map(|r| (r.fingerprint.id, r.fingerprint.semantic.clone()))
                .collect();

            let mut prediction_results = rank_effects_by_prediction(fp, &candidate_pairs);

            // Apply causal gate AFTER prediction ranking (Gap 2 fix).
            // rank_effects_by_prediction computes its own 80%E1+20%E5 scores,
            // so the gate must be applied to the prediction scores, not the
            // pre-ranking similarity (which would be discarded).
            for result in prediction_results.iter_mut() {
                if let Some((_, stored_fp)) = candidate_pairs
                    .iter()
                    .find(|(id, _)| *id == result.effect_id)
                {
                    let e5_sim = compute_e5_asymmetric_fingerprint_similarity(fp, stored_fp, true);
                    result.score = apply_causal_gate(result.score, e5_sim, true);
                }
            }
            prediction_results.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let memory_effects: Vec<EffectSearchResult> = prediction_results
                .into_iter()
                .filter_map(|result| {
                    let score = result.score;
                    if score < min_score {
                        filtered_count += 1;
                        return None;
                    }
                    Some(EffectSearchResult {
                        source_memory_id: result.effect_id,
                        score,
                        raw_similarity: result.raw_similarity,
                        causal_direction: None,
                        content: None,
                        source: None,
                        result_source: if search_scope == "all" {
                            Some("fingerprint".to_string())
                        } else {
                            None
                        },
                    })
                })
                .collect();

            all_effects.extend(memory_effects);
        }

        // ---- Path B: Search CF_CAUSAL_RELATIONSHIPS (relationships) ----
        if search_relationships {
            let cause_vec = e5_cause_vec.as_ref().unwrap();
            let rel_top_k = if search_memories {
                top_k * 2
            } else {
                top_k * 5
            };

            let rel_results = match self
                .teleological_store
                .search_causal_e5(cause_vec, false, rel_top_k) // false = search effect vectors
                .await
            {
                Ok(results) => results,
                Err(e) => {
                    error!(error = %e, "search_effects: Relationship search FAILED");
                    return self.tool_error(id, &format!("Relationship search failed: {}", e));
                }
            };

            candidates_evaluated += rel_results.len();

            for (rel_id, similarity) in &rel_results {
                let score = similarity * PREDICTIVE_BOOST; // Apply predictive boost
                if score < min_score {
                    filtered_count += 1;
                    continue;
                }

                let rel = match self
                    .teleological_store
                    .get_causal_relationship(*rel_id)
                    .await
                {
                    Ok(Some(rel)) => rel,
                    Ok(None) => {
                        warn!(rel_id = %rel_id, "search_effects: Causal relationship not found, skipping");
                        continue;
                    }
                    Err(e) => {
                        error!(rel_id = %rel_id, error = %e, "search_effects: Failed to retrieve causal relationship");
                        return self.tool_error_typed(
                            id,
                            ToolErrorKind::Storage,
                            &format!("Failed to retrieve causal relationship {}: {}", rel_id, e),
                        );
                    }
                };
                all_effects.push(EffectSearchResult {
                    source_memory_id: rel.source_fingerprint_id,
                    score,
                    raw_similarity: *similarity,
                    causal_direction: Some(rel.mechanism_type.clone()),
                    content: None,
                    source: None,
                    result_source: if search_scope == "all" {
                        Some("causal_relationship".to_string())
                    } else {
                        None
                    },
                });
            }
        }

        // ---- Merge and deduplicate ----
        all_effects.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut seen_ids = HashSet::new();
        let effects: Vec<EffectSearchResult> = all_effects
            .into_iter()
            .filter(|e| seen_ids.insert(e.source_memory_id))
            .take(top_k)
            .collect();

        let effect_ids: Vec<Uuid> = effects.iter().map(|e| e.source_memory_id).collect();

        // Get source metadata - FAIL FAST on error
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&effect_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    effect_count = effect_ids.len(),
                    "search_effects: Source metadata retrieval FAILED"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} effects: {}",
                        effect_ids.len(),
                        e
                    ),
                );
            }
        };

        // Get content if requested - FAIL FAST on error
        let contents: Vec<Option<String>> = if request.include_content && !effects.is_empty() {
            match self.teleological_store.get_content_batch(&effect_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        effect_count = effect_ids.len(),
                        "search_effects: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} effects: {}",
                            effect_ids.len(),
                            e
                        ),
                    );
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
                    start_line: metadata.start_line,
                    end_line: metadata.end_line,
                    chunk_index: metadata.chunk_index,
                    total_chunks: metadata.total_chunks,
                    hook_type: metadata.hook_type.clone(),
                    tool_name: metadata.tool_name.clone(),
                    display_string: Some(metadata.display_string()),
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

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "search_effects: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
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
        let (request, anchor_uuid) = match self.parse_request_validated::<GetCausalChainRequest>(
            id.clone(),
            args,
            "get_causal_chain",
        ) {
            Ok(pair) => pair,
            Err(resp) => return resp,
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
        let anchor_fingerprint = match self.teleological_store.retrieve(anchor_uuid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(anchor_id = %anchor_uuid, "get_causal_chain: Anchor not found");
                return self.tool_error(id, &format!("Anchor memory not found: {}", anchor_uuid));
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

        // M6 FIX: Move instead of clone — anchor_fingerprint is not used after this point.
        let mut current_fingerprint = anchor_fingerprint.semantic;
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
                    error!(
                        error = %e,
                        hop = hop_index,
                        "get_causal_chain: Hop search FAILED - cannot continue chain"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to search for hop {} candidates: {}. Chain traversal aborted.",
                            hop_index, e
                        ),
                    );
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

                // Audit-7 MCP-M3 FIX: Filter on RAW similarity before direction modifier.
                // Previously, direction_mod was applied before the threshold check, making
                // the effective threshold direction-dependent (0.375 backward vs 0.25 forward
                // for min_similarity=0.3). Now the user's threshold is respected uniformly.
                if asymmetric_sim < min_similarity {
                    continue;
                }

                // Apply direction modifier for RANKING only (not threshold filtering)
                let direction_mod = if is_forward { 1.2 } else { 0.8 };
                let adjusted_sim = asymmetric_sim * direction_mod;

                // Track best candidate
                if best_candidate.is_none() || adjusted_sim > best_candidate.as_ref().unwrap().2 {
                    // M6 FIX: Move instead of clone — `for candidate in candidates` gives
                    // ownership, so the SemanticFingerprint can be moved without copying
                    // all 14 embedding vectors (~50KB).
                    best_candidate = Some((
                        cand_id,
                        candidate.similarity, // base similarity (f32 is Copy)
                        adjusted_sim,         // asymmetric similarity
                        candidate.fingerprint.semantic,
                    ));
                }
            }

            // If no valid candidate found, chain ends here
            let (next_id, base_sim, asymmetric_sim, next_fingerprint) = match best_candidate {
                Some(c) => c,
                None => {
                    debug!(
                        hop = hop_index,
                        "get_causal_chain: No more candidates found"
                    );
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

        // Step 3: Optionally hydrate content - FAIL FAST on error
        if request.include_content && !chain.is_empty() {
            let hop_ids: Vec<Uuid> = chain.iter().map(|h| h.memory_id).collect();
            let contents = match self.teleological_store.get_content_batch(&hop_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        hop_count = hop_ids.len(),
                        "get_causal_chain: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} hops: {}",
                            hop_ids.len(),
                            e
                        ),
                    );
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

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "get_causal_chain: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }
}

/// Infer causal direction from a semantic fingerprint's E5 embeddings.
///
/// MCP-L2 FIX: Uses component variance instead of L2 norms. L2 norms of E5 dual
/// vectors are nearly identical regardless of direction (both are unit-normalized
/// by the model). Component variance captures distributional differences: causal
/// content activates different dimensions than effect content.
///
/// # Returns
/// - `Cause` if cause vector has significantly higher variance (>10% difference)
/// - `Effect` if effect vector has significantly higher variance
/// - `Unknown` if variances are similar or both are near zero
fn infer_causal_direction(fingerprint: &SemanticFingerprint) -> CausalDirection {
    let cause_vec = fingerprint.get_e5_as_cause();
    let effect_vec = fingerprint.get_e5_as_effect();

    let cause_variance = component_variance_f32(cause_vec);
    let effect_variance = component_variance_f32(effect_vec);

    let max_var = cause_variance.max(effect_variance);
    if max_var < f32::EPSILON {
        return CausalDirection::Unknown; // Both zero vectors
    }

    let diff_ratio = (cause_variance - effect_variance) / max_var;

    // Require >10% difference to be confident in direction
    if diff_ratio > 0.1 {
        CausalDirection::Cause
    } else if diff_ratio < -0.1 {
        CausalDirection::Effect
    } else {
        CausalDirection::Unknown
    }
}

use super::helpers::component_variance_f32;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_causal_direction() {
        let mut fp = SemanticFingerprint::zeroed();
        fp.e5_causal_as_cause = vec![2.0, -1.0, 0.5, -0.3];
        fp.e5_causal_as_effect = vec![0.5, 0.5, 0.5, 0.5];
        assert_eq!(infer_causal_direction(&fp), CausalDirection::Cause);
        fp.e5_causal_as_cause = vec![0.5, 0.5, 0.5, 0.5];
        fp.e5_causal_as_effect = vec![2.0, -1.0, 0.5, -0.3];
        assert_eq!(infer_causal_direction(&fp), CausalDirection::Effect);
        assert_eq!(
            infer_causal_direction(&SemanticFingerprint::zeroed()),
            CausalDirection::Unknown
        );
    }

    #[test]
    fn test_component_variance_f32() {
        assert!((component_variance_f32(&[1.0, 3.0]) - 1.0).abs() < 1e-6);
        assert!((component_variance_f32(&[5.0, 5.0, 5.0]) - 0.0).abs() < 1e-6);
        assert_eq!(component_variance_f32(&[]), 0.0);
    }
}
