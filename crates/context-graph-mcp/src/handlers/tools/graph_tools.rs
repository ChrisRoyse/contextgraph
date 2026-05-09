//! Graph reasoning tool implementations.
//!
//! # E8 Graph Asymmetric Similarity (ARCH-15, AP-77)
//!
//! These tools leverage the E8 (V_connectivity) embedder's asymmetric encoding:
//! - `search_connections`: Find memories connected to a given concept
//! - `get_graph_path`: Build and visualize multi-hop graph paths
//!
//! ## Constitution Compliance
//!
//! - ARCH-15: Uses asymmetric E8 with separate source/target encodings
//! - AP-77: Direction modifiers: source→target=1.2, target→source=0.8
//! - AP-02: All comparisons within E8 space (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging

use serde_json::json;
use std::collections::HashSet;
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::graph::asymmetric::{
    compute_e8_asymmetric_fingerprint_similarity, GraphDirection,
};
use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::fingerprint::SemanticFingerprint;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::graph_dtos::{
    ConnectionSearchMetadata, ConnectionSearchResult, GetGraphPathRequest, GetGraphPathResponse,
    GraphPathHop, GraphPathMetadata, GraphSourceInfo, SearchConnectionsRequest,
    SearchConnectionsResponse, HOP_ATTENUATION, SOURCE_DIRECTION_MODIFIER,
    TARGET_DIRECTION_MODIFIER,
};

use super::super::Handlers;
use super::helpers::ToolErrorKind;

impl Handlers {
    /// search_connections tool implementation.
    ///
    /// Finds memories connected to a given concept using asymmetric E8 similarity.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the query using all 14 embedders
    /// 2. Search for candidates using graph_reasoning weight profile (5x over-fetch)
    /// 3. Apply connection scoring using asymmetric E8 similarity
    /// 4. Apply direction modifier per AP-77 (1.2 source→target, 0.8 target→source)
    /// 5. Filter by minScore and return top-K ranked connections
    ///
    /// # Parameters
    ///
    /// - `query`: The concept to find connections for (required)
    /// - `direction`: "source", "target", or "both" (default: "both")
    /// - `topK`: Maximum connections to return (1-50, default: 10)
    /// - `minScore`: Minimum connection score threshold (0-1, default: 0.1)
    /// - `includeContent`: Include full content text (default: false)
    pub(crate) async fn call_search_connections(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        // MCP-6 FIX: includeProvenance is now modeled in DTO — no need for raw args clone
        let request: SearchConnectionsRequest =
            match self.parse_request(id.clone(), args, "search_connections") {
                Ok(req) => req,
                Err(resp) => return resp,
            };

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let is_source_seeking = request.is_source();
        let is_bidirectional = request.is_both();
        let include_provenance = request.include_provenance;

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            direction = %request.direction,
            top_k = top_k,
            min_score = min_score,
            "search_connections: Starting connection search"
        );

        // Step 1: Embed the query
        let query_embedding = match self
            .embed_query(id.clone(), query, "search_connections")
            .await
        {
            Ok(fp) => fp,
            Err(resp) => return resp,
        };

        // Step 2: Search for candidates (5x over-fetch for reranking)
        let fetch_multiplier = 5;
        let fetch_top_k = top_k * fetch_multiplier;

        let options = TeleologicalSearchOptions::quick(fetch_top_k)
            .with_strategy(SearchStrategy::MultiSpace)
            .with_weight_profile("graph_reasoning")
            .with_min_similarity(0.0); // Get all candidates, filter later

        let candidates = match self
            .teleological_store
            .search_semantic(&query_embedding, options)
            .await
        {
            Ok(results) => results,
            Err(e) => {
                error!(error = %e, "search_connections: Candidate search FAILED");
                return self.tool_error(id, &format!("Search failed: {}", e));
            }
        };

        let candidates_evaluated = candidates.len();
        debug!(
            candidates_evaluated = candidates_evaluated,
            "search_connections: Evaluating candidates for connection scoring"
        );

        // Step 3: Apply connection scoring using asymmetric E8 similarity.
        // MCP-7 FIX: For "both", compute max of source-seeking and target-seeking scores.
        let direction_modifier = if is_bidirectional {
            1.0 // No modifier for bidirectional — raw asymmetric scores used
        } else if is_source_seeking {
            SOURCE_DIRECTION_MODIFIER
        } else {
            TARGET_DIRECTION_MODIFIER
        };

        // Score each candidate using asymmetric E8 fingerprint similarity.
        // MCP-1 FIX: Also infer graph_direction from E8 vectors.
        // MCP-2 FIX: Apply direction_modifier to the asymmetric score.
        let mut scored_candidates: Vec<(Uuid, f32, f32, GraphDirection)> = candidates
            .iter()
            .map(|c| {
                let raw_sim = c.similarity;
                let graph_dir = infer_graph_direction(&c.fingerprint.semantic);

                let adjusted_score = if is_bidirectional {
                    // MCP-7 FIX: For "both", compute both directions and take the max
                    let source_score = compute_e8_asymmetric_fingerprint_similarity(
                        &query_embedding,
                        &c.fingerprint.semantic,
                        true,
                    ) * SOURCE_DIRECTION_MODIFIER;
                    let target_score = compute_e8_asymmetric_fingerprint_similarity(
                        &query_embedding,
                        &c.fingerprint.semantic,
                        false,
                    ) * TARGET_DIRECTION_MODIFIER;
                    source_score.max(target_score).clamp(0.0, 1.0)
                } else {
                    let asymmetric_score = compute_e8_asymmetric_fingerprint_similarity(
                        &query_embedding,
                        &c.fingerprint.semantic,
                        is_source_seeking,
                    );
                    // MCP-2 FIX: Apply direction modifier to the score (was cosmetic-only)
                    (asymmetric_score * direction_modifier).clamp(0.0, 1.0)
                };

                (c.fingerprint.id, adjusted_score, raw_sim, graph_dir)
            })
            .collect();

        // Sort by adjusted score descending
        scored_candidates
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Step 4: Filter by minScore and prepare response
        let mut filtered_count = 0;
        let connections: Vec<ConnectionSearchResult> = scored_candidates
            .into_iter()
            .filter_map(|(id, score, raw_sim, graph_dir)| {
                if score < min_score {
                    filtered_count += 1;
                    return None;
                }

                Some(ConnectionSearchResult {
                    connection_id: id,
                    score,
                    raw_similarity: raw_sim,
                    graph_direction: Some(format!("{}", graph_dir)),
                    content: None,
                    source: None,
                })
            })
            .take(top_k)
            .collect();

        // Step 5: Optionally filter by graph direction and hydrate content
        let connection_ids: Vec<Uuid> = connections.iter().map(|c| c.connection_id).collect();

        // Get source metadata for graph direction and provenance - FAIL FAST on error
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&connection_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                error!(
                    error = %e,
                    connection_count = connection_ids.len(),
                    "search_connections: Source metadata retrieval FAILED"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "Failed to retrieve source metadata for {} connections: {}",
                        connection_ids.len(),
                        e
                    ),
                );
            }
        };

        // Get content if requested - FAIL FAST on error
        let contents: Vec<Option<String>> = if request.include_content && !connections.is_empty() {
            match self
                .teleological_store
                .get_content_batch(&connection_ids)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        connection_count = connection_ids.len(),
                        "search_connections: Content retrieval FAILED"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to retrieve content for {} connections: {}",
                            connection_ids.len(),
                            e
                        ),
                    );
                }
            }
        } else {
            vec![None; connection_ids.len()]
        };

        // Populate metadata and content, filter by graph direction if specified
        let filter_direction = request.filter_graph_direction.as_deref();
        let mut final_connections: Vec<ConnectionSearchResult> =
            Vec::with_capacity(connections.len());

        for (i, mut conn) in connections.into_iter().enumerate() {
            // Populate source metadata (graph_direction is inferred, not stored)
            if let Some(Some(ref metadata)) = source_metadata.get(i) {
                conn.source = Some(GraphSourceInfo {
                    source_type: format!("{}", metadata.source_type),
                    file_path: metadata.file_path.clone(),
                    hook_type: metadata.hook_type.clone(),
                    tool_name: metadata.tool_name.clone(),
                });
            }

            // Graph direction filtering: exclude connections that don't match the requested direction.
            // If a direction filter is active and the connection has no direction data,
            // it cannot satisfy the filter — exclude it.
            if let Some(dir_filter) = filter_direction {
                match &conn.graph_direction {
                    None => {
                        debug!(
                            connection_id = %conn.connection_id,
                            filter = %dir_filter,
                            "search_connections: Excluding connection with no direction (filter active)"
                        );
                        continue;
                    }
                    Some(conn_dir) => {
                        let conn_dir_str = conn_dir.to_string();
                        if conn_dir_str.to_lowercase() != dir_filter.to_lowercase() {
                            debug!(
                                connection_id = %conn.connection_id,
                                connection_direction = %conn_dir_str,
                                filter = %dir_filter,
                                "search_connections: Excluding connection with mismatched direction"
                            );
                            continue;
                        }
                    }
                }
            }

            // Add content if requested
            if request.include_content {
                if let Some(content_opt) = contents.get(i) {
                    conn.content = content_opt.clone();
                }
            }

            final_connections.push(conn);
        }

        // Truncate to requested top_k after filtering
        final_connections.truncate(top_k);

        let response = SearchConnectionsResponse {
            query: query.clone(),
            direction: request.direction.clone(),
            connections: final_connections.clone(),
            count: final_connections.len(),
            metadata: ConnectionSearchMetadata {
                candidates_evaluated,
                filtered_by_score: filtered_count,
                direction_modifier,
            },
        };

        info!(
            connections_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "search_connections: Completed connection search"
        );

        // PHASE-2-PROVENANCE: Add retrieval provenance when requested
        let mut response_json = match serde_json::to_value(&response) {
            Ok(v) => v,
            Err(e) => {
                error!(error = %e, "search_connections: Response serialization failed");
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                );
            }
        };
        if include_provenance {
            response_json["retrievalProvenance"] = json!({
                "connectionScoringMethod": "asymmetric_e8_similarity",
                "e8GraphSimilarity": {
                    "direction": request.direction,
                    "isSourceSeeking": is_source_seeking,
                    "directionModifier": direction_modifier,
                    "sourceModifier": SOURCE_DIRECTION_MODIFIER,
                    "targetModifier": TARGET_DIRECTION_MODIFIER
                },
                "candidateSearchProfile": "graph_reasoning",
                "fetchMultiplier": 5,
                "minScoreThreshold": min_score,
                "candidatesEvaluated": candidates_evaluated,
                "filteredByScore": filtered_count
            });
        }

        self.tool_result(id, response_json)
    }

    /// get_graph_path tool implementation.
    ///
    /// Builds and visualizes multi-hop graph paths from an anchor point.
    ///
    /// # Algorithm
    ///
    /// 1. Verify anchor memory exists
    /// 2. Iteratively search for next hop using asymmetric E8 similarity
    /// 3. Track visited memories to avoid cycles
    /// 4. Apply hop attenuation (0.9^hop) for path scoring
    /// 5. Return path with per-hop and total scores
    ///
    /// # Parameters
    ///
    /// - `anchorId`: UUID of the starting memory (required)
    /// - `direction`: "forward" (source→target) or "backward" (target→source)
    /// - `maxHops`: Maximum hops to traverse (1-10, default: 5)
    /// - `minSimilarity`: Minimum similarity for each hop (0-1, default: 0.3)
    /// - `includeContent`: Include full content text (default: false)
    pub(crate) async fn call_get_graph_path(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let (request, anchor_uuid) = match self.parse_request_validated::<GetGraphPathRequest>(
            id.clone(),
            args,
            "get_graph_path",
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
            "get_graph_path: Starting path traversal"
        );

        // Step 1: Verify anchor exists and get its fingerprint
        let anchor_fingerprint = match self.teleological_store.retrieve(anchor_uuid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(anchor_id = %anchor_uuid, "get_graph_path: Anchor not found");
                return self.tool_error(id, &format!("Anchor memory not found: {}", anchor_uuid));
            }
            Err(e) => {
                error!(error = %e, "get_graph_path: Failed to get anchor");
                return self.tool_error(id, &format!("Failed to get anchor: {}", e));
            }
        };

        // Step 2: Iteratively build the path
        let mut path: Vec<GraphPathHop> = Vec::with_capacity(max_hops);
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
                .with_weight_profile("graph_reasoning")
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
                        "get_graph_path: Hop search FAILED - cannot continue path"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "Failed to search for hop {} candidates: {}. Path traversal aborted.",
                            hop_index, e
                        ),
                    );
                }
            };

            total_candidates_evaluated += candidates.len();

            // Find best unvisited candidate with asymmetric E8 similarity
            let mut best_candidate: Option<(Uuid, f32, f32, SemanticFingerprint)> = None;

            for candidate in candidates {
                let cand_id = candidate.fingerprint.id;

                // Skip if already visited (cycle prevention)
                if visited.contains(&cand_id) {
                    continue;
                }

                // Compute asymmetric E8 similarity
                // Forward: query is source, doc is target (use 1.2x modifier)
                // Backward: query is target, doc is source (use 0.8x modifier)
                let asymmetric_sim = compute_e8_asymmetric_fingerprint_similarity(
                    &current_fingerprint,
                    &candidate.fingerprint.semantic,
                    is_forward, // query_is_source
                );

                // Audit-10 MCP-H1 FIX: Filter on RAW similarity before direction modifier.
                // Previously, direction_mod was applied before the threshold check, making
                // the effective threshold direction-dependent (matches causal_tools.rs pattern).
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

            // If no valid candidate found, path ends here
            let (next_id, base_sim, asymmetric_sim, next_fingerprint) = match best_candidate {
                Some(c) => c,
                None => {
                    debug!(hop = hop_index, "get_graph_path: No more candidates found");
                    break;
                }
            };

            // Infer graph direction of next hop
            let hop_direction = infer_graph_direction(&next_fingerprint);

            // Create hop with computed cumulative strength
            let hop = GraphPathHop::new(
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
            path.push(hop);

            // Check if we hit max hops
            if hop_index + 1 >= max_hops {
                truncated = true;
            }
        }

        // Step 3: Optionally hydrate content - FAIL FAST on error
        if request.include_content && !path.is_empty() {
            let hop_ids: Vec<Uuid> = path.iter().map(|h| h.memory_id).collect();
            let contents = match self.teleological_store.get_content_batch(&hop_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        error = %e,
                        hop_count = hop_ids.len(),
                        "get_graph_path: Content retrieval FAILED"
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

            for (i, hop) in path.iter_mut().enumerate() {
                if let Some(Some(ref content)) = contents.get(i) {
                    hop.content = Some(content.clone());
                }
            }
        }

        // Step 4: Build response
        let total_score = if path.is_empty() {
            0.0
        } else {
            path.last().map(|h| h.cumulative_strength).unwrap_or(0.0)
        };

        let response = GetGraphPathResponse {
            anchor_id: anchor_uuid,
            direction: direction.clone(),
            path: path.clone(),
            total_path_score: total_score,
            hop_count: path.len(),
            truncated,
            metadata: GraphPathMetadata {
                max_hops,
                min_similarity,
                hop_attenuation: HOP_ATTENUATION,
                total_candidates_evaluated,
            },
        };

        info!(
            anchor_id = %anchor_uuid,
            direction = %direction,
            hops_found = path.len(),
            total_score = total_score,
            truncated = truncated,
            "get_graph_path: Completed path traversal"
        );

        match serde_json::to_value(&response) {
            Ok(v) => self.tool_result(id, v),
            Err(e) => {
                error!(error = %e, "get_graph_path: Response serialization failed");
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("Response serialization failed: {}", e),
                )
            }
        }
    }
}

/// Infer graph direction from a semantic fingerprint's E8 embeddings.
///
/// Documents that describe sources tend to have stronger "as_source" vectors,
/// while documents describing targets have stronger "as_target" vectors.
/// Audit-10 MCP-H2 FIX: Use component variance instead of L2 norms.
/// L2 norms can't distinguish direction when vectors have similar magnitudes
/// but different activation patterns. Variance captures spread of activations,
/// which is a better signal for directional strength.
fn infer_graph_direction(fingerprint: &SemanticFingerprint) -> GraphDirection {
    use super::helpers::component_variance_f32;

    let source_vec = &fingerprint.e8_graph_as_source;
    let target_vec = &fingerprint.e8_graph_as_target;

    let source_variance = component_variance_f32(source_vec);
    let target_variance = component_variance_f32(target_vec);

    let max_var = source_variance.max(target_variance);
    if max_var < f32::EPSILON {
        return GraphDirection::Unknown; // Both zero vectors
    }

    let diff_ratio = (source_variance - target_variance) / max_var;

    // Require >10% difference to be confident in direction
    if diff_ratio > 0.1 {
        GraphDirection::Source
    } else if diff_ratio < -0.1 {
        GraphDirection::Target
    } else {
        GraphDirection::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_graph_direction() {
        let mut fp = SemanticFingerprint::zeroed();
        fp.e8_graph_as_source = vec![1.0, 0.5, 0.3];
        fp.e8_graph_as_target = vec![0.5, 0.2, 0.1];
        assert_eq!(infer_graph_direction(&fp), GraphDirection::Source);
        fp.e8_graph_as_source = vec![0.5, 0.2, 0.1];
        fp.e8_graph_as_target = vec![1.0, 0.5, 0.3];
        assert_eq!(infer_graph_direction(&fp), GraphDirection::Target);
        fp.e8_graph_as_source = vec![1.0, 0.5, 0.3];
        fp.e8_graph_as_target = vec![1.0, 0.5, 0.3];
        assert_eq!(infer_graph_direction(&fp), GraphDirection::Unknown);
        assert_eq!(
            infer_graph_direction(&SemanticFingerprint::zeroed()),
            GraphDirection::Unknown
        );
    }
}
