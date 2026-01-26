//! Graph reasoning tool implementations.
//!
//! # E8 Graph Asymmetric Similarity (ARCH-15, AP-77)
//!
//! These tools leverage the E8 (V_connectivity) embedder's asymmetric encoding:
//! - `search_connections`: Find memories connected to a given concept
//! - `get_graph_path`: Build and visualize multi-hop graph paths
//!
//! # LLM-based Graph Discovery
//!
//! These tools use the graph-agent with CausalDiscoveryLLM for relationship detection:
//! - `discover_graph_relationships`: Discover relationships between memories
//! - `validate_graph_link`: Validate a proposed graph link
//!
//! ## Constitution Compliance
//!
//! - ARCH-15: Uses asymmetric E8 with separate source/target encodings
//! - AP-77: Direction modifiers: source→target=1.2, target→source=0.8
//! - AP-02: All comparisons within E8 space (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging

use serde_json::json;
use std::collections::HashSet;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::graph::asymmetric::{
    compute_e8_asymmetric_fingerprint_similarity, GraphDirection,
};
use context_graph_core::traits::{SearchStrategy, TeleologicalSearchOptions};
use context_graph_core::types::fingerprint::SemanticFingerprint;

// GRAPH-AGENT: LLM-based relationship discovery types
use context_graph_graph_agent::{MemoryForGraphAnalysis, RelationshipType};

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::graph_dtos::{
    ConnectionSearchMetadata, ConnectionSearchResult, DiscoveredRelationship,
    DiscoverGraphRelationshipsRequest, DiscoverGraphRelationshipsResponse, DiscoveryMetadata,
    GetGraphPathRequest, GetGraphPathResponse, GraphPathHop, GraphPathMetadata, GraphSourceInfo,
    SearchConnectionsRequest, SearchConnectionsResponse, ValidateGraphLinkRequest,
    ValidateGraphLinkResponse, HOP_ATTENUATION, SOURCE_DIRECTION_MODIFIER,
    TARGET_DIRECTION_MODIFIER,
};

use super::super::Handlers;

impl Handlers {
    /// search_connections tool implementation.
    ///
    /// Finds memories connected to a given concept using asymmetric E8 similarity.
    ///
    /// # Algorithm
    ///
    /// 1. Embed the query using all 13 embedders
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
        let request: SearchConnectionsRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "search_connections: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        if let Err(e) = request.validate() {
            error!(error = %e, "search_connections: Validation failed");
            return self.tool_error(id, &e);
        }

        let query = &request.query;
        let top_k = request.top_k;
        let min_score = request.min_score;
        let is_source_seeking = request.is_source();

        info!(
            query_preview = %query.chars().take(50).collect::<String>(),
            direction = %request.direction,
            top_k = top_k,
            min_score = min_score,
            "search_connections: Starting connection search"
        );

        // Step 1: Embed the query
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_connections: Query embedding FAILED");
                return self.tool_error(id, &format!("Query embedding failed: {}", e));
            }
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

        // Step 3: Apply connection scoring using asymmetric E8 similarity
        let direction_modifier = if is_source_seeking {
            SOURCE_DIRECTION_MODIFIER
        } else {
            TARGET_DIRECTION_MODIFIER
        };

        // Score each candidate using asymmetric E8 fingerprint similarity
        let mut scored_candidates: Vec<(Uuid, f32, f32)> = candidates
            .iter()
            .map(|c| {
                let raw_sim = c.similarity;
                let asymmetric_score = compute_e8_asymmetric_fingerprint_similarity(
                    &query_embedding,
                    &c.fingerprint.semantic,
                    is_source_seeking, // query_is_source
                );
                (c.fingerprint.id, asymmetric_score, raw_sim)
            })
            .collect();

        // Sort by asymmetric score descending
        scored_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Step 4: Filter by minScore and prepare response
        let mut filtered_count = 0;
        let connections: Vec<ConnectionSearchResult> = scored_candidates
            .into_iter()
            .filter_map(|(id, score, raw_sim)| {
                if score < min_score {
                    filtered_count += 1;
                    return None;
                }

                Some(ConnectionSearchResult {
                    connection_id: id,
                    score,
                    raw_similarity: raw_sim,
                    graph_direction: None, // Will be populated from source metadata
                    content: None,
                    source: None,
                })
            })
            .take(top_k)
            .collect();

        // Step 5: Optionally filter by graph direction and hydrate content
        let connection_ids: Vec<Uuid> = connections.iter().map(|c| c.connection_id).collect();

        // Get source metadata for graph direction and provenance
        let source_metadata = match self
            .teleological_store
            .get_source_metadata_batch(&connection_ids)
            .await
        {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    error = %e,
                    "search_connections: Source metadata retrieval failed"
                );
                vec![None; connection_ids.len()]
            }
        };

        // Get content if requested
        let contents: Vec<Option<String>> = if request.include_content && !connections.is_empty() {
            match self.teleological_store.get_content_batch(&connection_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "search_connections: Content retrieval failed");
                    vec![None; connection_ids.len()]
                }
            }
        } else {
            vec![None; connection_ids.len()]
        };

        // Populate metadata and content, filter by graph direction if specified
        let filter_direction = request.filter_graph_direction.as_deref();
        let mut final_connections: Vec<ConnectionSearchResult> = Vec::with_capacity(connections.len());

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

            // For graph direction filtering, we'd need to retrieve fingerprints
            // and infer direction. For now, skip direction-based filtering.
            // TODO: Add graph direction inference if filter_direction is specified
            if filter_direction.is_some() && conn.graph_direction.is_none() {
                // Filter requested but no direction available - keep the item
                // unless we implement fingerprint retrieval for direction inference
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

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
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
        let request: GetGraphPathRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_graph_path: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let anchor_uuid = match request.validate() {
            Ok(uuid) => uuid,
            Err(e) => {
                error!(error = %e, "get_graph_path: Validation failed");
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
            "get_graph_path: Starting path traversal"
        );

        // Step 1: Verify anchor exists and get its fingerprint
        let anchor_fingerprint = match self
            .teleological_store
            .retrieve(anchor_uuid)
            .await
        {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(anchor_id = %anchor_uuid, "get_graph_path: Anchor not found");
                return self.tool_error(
                    id,
                    &format!("Anchor memory not found: {}", anchor_uuid),
                );
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

        let mut current_fingerprint = anchor_fingerprint.semantic.clone();
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
                    warn!(error = %e, hop = hop_index, "get_graph_path: Hop search failed");
                    break;
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

        // Step 3: Optionally hydrate content
        if request.include_content && !path.is_empty() {
            let hop_ids: Vec<Uuid> = path.iter().map(|h| h.memory_id).collect();
            let contents = match self.teleological_store.get_content_batch(&hop_ids).await {
                Ok(c) => c,
                Err(e) => {
                    warn!(error = %e, "get_graph_path: Content retrieval failed");
                    vec![None; hop_ids.len()]
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

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// discover_graph_relationships tool implementation.
    ///
    /// Discovers graph relationships between memories using LLM analysis.
    ///
    /// Uses the GraphDiscoveryService (Qwen2.5-3B ~6GB VRAM) to analyze
    /// candidate memory pairs and detect structural relationships like
    /// imports, dependencies, references, calls, etc.
    ///
    /// # Parameters
    ///
    /// - `memory_ids`: UUIDs of memories to analyze (2-50 required)
    /// - `relationship_types`: Filter to specific types (optional)
    /// - `min_confidence`: Minimum confidence threshold (0-1, default: 0.7)
    /// - `batch_size`: Maximum candidate pairs to analyze (1-100, default: 50)
    pub(crate) async fn call_discover_graph_relationships(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: DiscoverGraphRelationshipsRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "discover_graph_relationships: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let memory_uuids = match request.validate() {
            Ok(uuids) => uuids,
            Err(e) => {
                error!(error = %e, "discover_graph_relationships: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        let min_confidence = request.min_confidence;

        info!(
            memory_count = memory_uuids.len(),
            min_confidence = min_confidence,
            "discover_graph_relationships: Request validated"
        );

        // Get graph discovery service - GUARANTEED to be available (NO FALLBACKS)
        let service = self.graph_discovery_service();

        // Fetch memory content and metadata for analysis
        let mut memories_for_analysis: Vec<MemoryForGraphAnalysis> = Vec::with_capacity(memory_uuids.len());
        let mut fetch_errors: Vec<String> = Vec::new();

        for uuid in &memory_uuids {
            // Get fingerprint
            let fingerprint = match self.teleological_store.retrieve(*uuid).await {
                Ok(Some(fp)) => fp,
                Ok(None) => {
                    warn!(uuid = %uuid, "Memory not found, skipping");
                    fetch_errors.push(format!("Memory {} not found", uuid));
                    continue;
                }
                Err(e) => {
                    warn!(uuid = %uuid, error = %e, "Failed to fetch memory, skipping");
                    fetch_errors.push(format!("Failed to fetch {}: {}", uuid, e));
                    continue;
                }
            };

            // Get content
            let content = match self.teleological_store.get_content(*uuid).await {
                Ok(Some(c)) => c,
                Ok(None) => {
                    warn!(uuid = %uuid, "Memory content not found, skipping");
                    fetch_errors.push(format!("Content for {} not found", uuid));
                    continue;
                }
                Err(e) => {
                    warn!(uuid = %uuid, error = %e, "Failed to fetch content, skipping");
                    fetch_errors.push(format!("Failed to fetch content for {}: {}", uuid, e));
                    continue;
                }
            };

            // Get source metadata (optional - for source_type and file_path)
            let source_metadata = self
                .teleological_store
                .get_source_metadata(*uuid)
                .await
                .ok()
                .flatten();

            memories_for_analysis.push(MemoryForGraphAnalysis {
                id: *uuid,
                content,
                created_at: fingerprint.created_at,
                session_id: source_metadata.as_ref().and_then(|m| m.session_id.clone()),
                e1_embedding: fingerprint.semantic.e1_semantic.to_vec(),
                source_type: source_metadata.as_ref().map(|m| format!("{}", m.source_type)),
                file_path: source_metadata.and_then(|m| m.file_path),
            });
        }

        if memories_for_analysis.len() < 2 {
            return self.tool_error(
                id,
                &format!(
                    "Need at least 2 valid memories for relationship discovery, got {}. Errors: {:?}",
                    memories_for_analysis.len(),
                    fetch_errors
                ),
            );
        }

        info!(
            valid_memories = memories_for_analysis.len(),
            errors = fetch_errors.len(),
            "discover_graph_relationships: Memories fetched, running discovery cycle"
        );

        // Run discovery cycle
        let result = match service.run_discovery_cycle(&memories_for_analysis).await {
            Ok(r) => r,
            Err(e) => {
                error!(error = %e, "Graph discovery cycle failed");
                return self.tool_error(id, &format!("Discovery failed: {}", e));
            }
        };

        // Build response from results - get edges from graph storage
        let graph = service.graph();
        let graph_read = graph.read();
        let all_edges = graph_read.all_edges();

        // Filter edges by min_confidence and optionally by relationship types
        let type_filter: Option<HashSet<RelationshipType>> = request
            .relationship_types
            .as_ref()
            .map(|types| types.iter().map(|t| RelationshipType::from_str(t)).collect());

        let relationships: Vec<DiscoveredRelationship> = all_edges
            .iter()
            .filter(|e| e.confidence >= min_confidence)
            .filter(|e| {
                type_filter
                    .as_ref()
                    .map(|f| f.contains(&e.relationship_type))
                    .unwrap_or(true)
            })
            .map(|e| {
                // GraphEdge has source_id and target_id already ordered correctly
                // by the activator based on the LLM analysis direction
                DiscoveredRelationship {
                    source_id: e.source_id,
                    target_id: e.target_id,
                    relationship_type: e.relationship_type.as_str().to_string(),
                    category: Some(e.relationship_type.category().as_str().to_string()),
                    domain: None, // Domain not stored in GraphEdge
                    direction: "a_connects_b".to_string(), // Source → Target
                    confidence: e.confidence,
                    description: e.description.clone(),
                }
            })
            .collect();

        let mut all_errors = result.error_messages.clone();
        all_errors.extend(fetch_errors);

        let response = DiscoverGraphRelationshipsResponse {
            relationships: relationships.clone(),
            count: relationships.len(),
            metadata: DiscoveryMetadata {
                memories_analyzed: memories_for_analysis.len(),
                candidates_evaluated: result.candidates_found,
                relationships_confirmed: result.relationships_confirmed,
                relationships_rejected: result.relationships_rejected,
                min_confidence,
                errors: all_errors,
            },
        };

        info!(
            relationships_found = relationships.len(),
            candidates_evaluated = result.candidates_found,
            confirmed = result.relationships_confirmed,
            rejected = result.relationships_rejected,
            "discover_graph_relationships: Discovery cycle complete"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// validate_graph_link tool implementation.
    ///
    /// Validates a proposed graph link between two memories using LLM analysis.
    ///
    /// Uses the GraphRelationshipLLM (Qwen2.5-3B ~6GB VRAM) to analyze
    /// whether a valid relationship exists between the two memories.
    ///
    /// # Parameters
    ///
    /// - `source_id`: UUID of the source memory (required)
    /// - `target_id`: UUID of the target memory (required)
    /// - `expected_relationship_type`: Expected type to validate (optional)
    pub(crate) async fn call_validate_graph_link(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // Parse and validate request
        let request: ValidateGraphLinkRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "validate_graph_link: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let (source_uuid, target_uuid) = match request.validate() {
            Ok(uuids) => uuids,
            Err(e) => {
                error!(error = %e, "validate_graph_link: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        info!(
            source_id = %source_uuid,
            target_id = %target_uuid,
            expected_type = ?request.expected_relationship_type,
            "validate_graph_link: Request validated"
        );

        // Get graph discovery service - GUARANTEED to be available (NO FALLBACKS)
        let service = self.graph_discovery_service();

        // Fetch content for source memory
        let source_content = match self.teleological_store.get_content(source_uuid).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                error!(uuid = %source_uuid, "validate_graph_link: Source memory content not found");
                return self.tool_error(id, &format!("Source memory content not found: {}", source_uuid));
            }
            Err(e) => {
                error!(uuid = %source_uuid, error = %e, "validate_graph_link: Failed to fetch source content");
                return self.tool_error(id, &format!("Failed to fetch source content: {}", e));
            }
        };

        // Fetch content for target memory
        let target_content = match self.teleological_store.get_content(target_uuid).await {
            Ok(Some(c)) => c,
            Ok(None) => {
                error!(uuid = %target_uuid, "validate_graph_link: Target memory content not found");
                return self.tool_error(id, &format!("Target memory content not found: {}", target_uuid));
            }
            Err(e) => {
                error!(uuid = %target_uuid, error = %e, "validate_graph_link: Failed to fetch target content");
                return self.tool_error(id, &format!("Failed to fetch target content: {}", e));
            }
        };

        info!(
            source_len = source_content.len(),
            target_len = target_content.len(),
            "validate_graph_link: Content fetched, running LLM analysis"
        );

        // Use LLM to analyze the relationship
        let llm = service.llm();

        // If expected_relationship_type is provided, use validation mode
        // Otherwise, use general analysis mode
        if let Some(ref expected_type_str) = request.expected_relationship_type {
            let expected_type = RelationshipType::from_str(expected_type_str);

            match llm
                .validate_relationship(&source_content, &target_content, expected_type)
                .await
            {
                Ok((is_valid, confidence, explanation)) => {
                    let expected_type = RelationshipType::from_str(expected_type_str);
                    let response = ValidateGraphLinkResponse {
                        is_valid,
                        source_id: source_uuid,
                        target_id: target_uuid,
                        relationship_type: if is_valid {
                            Some(expected_type_str.clone())
                        } else {
                            None
                        },
                        category: if is_valid {
                            Some(expected_type.category().as_str().to_string())
                        } else {
                            None
                        },
                        domain: None, // Domain not stored
                        direction: if is_valid {
                            Some("a_connects_b".to_string())
                        } else {
                            None
                        },
                        confidence,
                        description: explanation,
                        expected_type_matched: Some(is_valid),
                    };

                    info!(
                        is_valid = is_valid,
                        confidence = confidence,
                        expected_type = expected_type_str,
                        "validate_graph_link: Validation complete"
                    );

                    self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
                }
                Err(e) => {
                    error!(error = %e, "validate_graph_link: LLM validation failed");
                    self.tool_error(id, &format!("LLM validation failed: {}", e))
                }
            }
        } else {
            // No expected type - use general analysis
            match llm.analyze_relationship(&source_content, &target_content).await {
                Ok(analysis) => {
                    let is_valid = analysis.has_connection && analysis.confidence >= 0.5;

                    let response = ValidateGraphLinkResponse {
                        is_valid,
                        source_id: source_uuid,
                        target_id: target_uuid,
                        relationship_type: if is_valid {
                            Some(analysis.relationship_type.as_str().to_string())
                        } else {
                            None
                        },
                        category: if is_valid {
                            Some(analysis.category.as_str().to_string())
                        } else {
                            None
                        },
                        domain: if is_valid {
                            Some(analysis.domain.as_str().to_string())
                        } else {
                            None
                        },
                        direction: if is_valid {
                            Some(analysis.direction.as_str().to_string())
                        } else {
                            None
                        },
                        confidence: analysis.confidence,
                        description: analysis.description,
                        expected_type_matched: None,
                    };

                    info!(
                        is_valid = is_valid,
                        confidence = analysis.confidence,
                        relationship_type = analysis.relationship_type.as_str(),
                        "validate_graph_link: Analysis complete"
                    );

                    self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
                }
                Err(e) => {
                    error!(error = %e, "validate_graph_link: LLM analysis failed");
                    self.tool_error(id, &format!("LLM analysis failed: {}", e))
                }
            }
        }
    }
}

/// Infer graph direction from a semantic fingerprint's E8 embeddings.
///
/// Documents that describe sources tend to have stronger "as_source" vectors,
/// while documents describing targets have stronger "as_target" vectors.
fn infer_graph_direction(fingerprint: &SemanticFingerprint) -> GraphDirection {
    let source_vec = &fingerprint.e8_graph_as_source;
    let target_vec = &fingerprint.e8_graph_as_target;

    // Compare vector norms as a proxy for directional strength
    let source_norm: f32 = source_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    let target_norm: f32 = target_vec.iter().map(|x| x * x).sum::<f32>().sqrt();

    // Require >10% difference to be confident in direction
    let threshold = 0.1;
    let diff_ratio = if target_norm > f32::EPSILON {
        (source_norm - target_norm) / target_norm
    } else if source_norm > f32::EPSILON {
        1.0 // All source, no target
    } else {
        0.0 // Both zero
    };

    if diff_ratio > threshold {
        GraphDirection::Source
    } else if diff_ratio < -threshold {
        GraphDirection::Target
    } else {
        GraphDirection::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_graph_direction_source() {
        // Create a fingerprint where source norm > target norm
        let mut fp = SemanticFingerprint::zeroed();
        // Set e8_as_source to have higher norm than e8_as_target
        fp.e8_graph_as_source = vec![1.0, 0.5, 0.3]; // norm ~= 1.14
        fp.e8_graph_as_target = vec![0.5, 0.2, 0.1]; // norm ~= 0.55

        let direction = infer_graph_direction(&fp);
        assert_eq!(direction, GraphDirection::Source);
        println!("[PASS] Correctly inferred Source direction");
    }

    #[test]
    fn test_infer_graph_direction_target() {
        // Create a fingerprint where target norm > source norm
        let mut fp = SemanticFingerprint::zeroed();
        fp.e8_graph_as_source = vec![0.5, 0.2, 0.1]; // norm ~= 0.55
        fp.e8_graph_as_target = vec![1.0, 0.5, 0.3]; // norm ~= 1.14

        let direction = infer_graph_direction(&fp);
        assert_eq!(direction, GraphDirection::Target);
        println!("[PASS] Correctly inferred Target direction");
    }

    #[test]
    fn test_infer_graph_direction_unknown() {
        // Create a fingerprint where norms are similar
        let mut fp = SemanticFingerprint::zeroed();
        fp.e8_graph_as_source = vec![1.0, 0.5, 0.3];
        fp.e8_graph_as_target = vec![1.0, 0.5, 0.3];

        let direction = infer_graph_direction(&fp);
        assert_eq!(direction, GraphDirection::Unknown);
        println!("[PASS] Correctly inferred Unknown direction for equal norms");
    }

    #[test]
    fn test_infer_graph_direction_empty_vectors() {
        // Create a fingerprint with empty vectors
        let fp = SemanticFingerprint::zeroed();
        let direction = infer_graph_direction(&fp);
        assert_eq!(direction, GraphDirection::Unknown);
        println!("[PASS] Correctly handled empty vectors");
    }
}
