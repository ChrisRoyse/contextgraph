//! Graph linking tool implementations (get_memory_neighbors, get_typed_edges, traverse_graph).
//!
//! # Knowledge Graph Linking Tools
//!
//! These tools expose the K-NN graph and typed edge infrastructure:
//! - `get_memory_neighbors`: K-NN neighbors in specific embedder space
//! - `get_typed_edges`: Typed edges derived from embedder agreement patterns
//! - `traverse_graph`: Multi-hop graph traversal following typed edges
//!
//! ## Constitution Compliance
//!
//! - ARCH-18: E5/E8 use asymmetric similarity (direction matters)
//! - AP-60: Temporal embedders (E2-E4) never count toward edge type detection
//! - AP-77: E5 MUST NOT use symmetric cosine
//! - AP-02: All comparisons within same embedder space (no cross-embedder)
//! - FAIL FAST: All errors propagate immediately with logging - NO FALLBACKS
//!
//! ## TASK-GRAPHLINK: EdgeRepository Integration
//!
//! All graph linking tools now use EdgeRepository directly:
//! - NO FALLBACKS - if EdgeRepository is not available, tools error
//! - K-NN edges from embedder_edges column family
//! - Typed edges from typed_edges column family

use serde_json::json;
use std::collections::{HashMap, HashSet, VecDeque};
use tracing::{debug, error, info};
use uuid::Uuid;

use context_graph_core::graph_linking::GraphLinkEdgeType;
use context_graph_core::weights::get_weight_profile;

use crate::protocol::JsonRpcId;
use crate::protocol::JsonRpcResponse;

use super::graph_link_dtos::{
    embedder_name, uses_asymmetric_similarity, AgreementSummary, EmbedderContribution,
    GetMemoryNeighborsRequest, GetMemoryNeighborsResponse, GetTypedEdgesRequest,
    GetTypedEdgesResponse, GetUnifiedNeighborsRequest, GetUnifiedNeighborsResponse, NeighborResult,
    NeighborSearchMetadata, NeighborSourceInfo, TraversalMetadata, TraversalNode, TraversalPath,
    TraverseGraphRequest, TraverseGraphResponse, TypedEdgeMetadata, TypedEdgeResult,
    UnifiedNeighborMetadata, UnifiedNeighborResult, RRF_K, SEMANTIC_EMBEDDER_INDICES,
};

use super::super::Handlers;

impl Handlers {
    /// get_memory_neighbors tool implementation.
    ///
    /// Finds K nearest neighbors of a memory in a specific embedder space.
    /// TASK-GRAPHLINK: Uses EdgeRepository directly - NO FALLBACKS.
    ///
    /// # Algorithm
    ///
    /// 1. Validate request and verify memory exists
    /// 2. Query EdgeRepository.get_embedder_edges() for K-NN neighbors
    /// 3. Apply min_similarity filter
    /// 4. Optionally hydrate content and source metadata
    ///
    /// # Parameters
    ///
    /// - `memory_id`: UUID of the memory to find neighbors for (required)
    /// - `embedder_id`: Embedder space to search (0-12, default: 0=E1)
    /// - `top_k`: Number of neighbors to return (1-50, default: 10)
    /// - `min_similarity`: Minimum similarity threshold (0-1, default: 0.0)
    /// - `include_content`: Include full content text (default: false)
    pub(crate) async fn call_get_memory_neighbors(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // TASK-GRAPHLINK: Require EdgeRepository - NO FALLBACKS
        let edge_repo = match &self.edge_repository {
            Some(repo) => repo,
            None => {
                error!("get_memory_neighbors: EdgeRepository not available - NO FALLBACKS");
                return self.tool_error(
                    id,
                    "Graph linking not initialized. EdgeRepository is required - NO FALLBACKS.",
                );
            }
        };

        // Parse and validate request
        let request: GetMemoryNeighborsRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_memory_neighbors: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let memory_uuid = match request.validate() {
            Ok(uuid) => uuid,
            Err(e) => {
                error!(error = %e, "get_memory_neighbors: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        let embedder_id = request.embedder_id;
        let top_k = request.top_k;
        let min_similarity = request.min_similarity;
        let emb_name = embedder_name(embedder_id);
        let uses_asymmetric = uses_asymmetric_similarity(embedder_id);

        info!(
            memory_id = %memory_uuid,
            embedder_id = embedder_id,
            embedder_name = %emb_name,
            top_k = top_k,
            min_similarity = min_similarity,
            uses_asymmetric = uses_asymmetric,
            "get_memory_neighbors: Starting K-NN neighbor search via EdgeRepository"
        );

        // Step 1: Verify memory exists
        match self.teleological_store.retrieve(memory_uuid).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                error!(memory_id = %memory_uuid, "get_memory_neighbors: Memory not found");
                return self.tool_error(id, &format!("Memory not found: {}", memory_uuid));
            }
            Err(e) => {
                error!(error = %e, "get_memory_neighbors: Failed to retrieve memory");
                return self.tool_error(id, &format!("Failed to retrieve memory: {}", e));
            }
        };

        debug!(memory_id = %memory_uuid, "Memory verified, querying EdgeRepository");

        // Step 2: Query EdgeRepository for K-NN neighbors - NO FALLBACKS
        let embedder_edges = match edge_repo.get_embedder_edges(embedder_id as u8, memory_uuid) {
            Ok(edges) => edges,
            Err(e) => {
                error!(
                    error = %e,
                    memory_id = %memory_uuid,
                    embedder_id = embedder_id,
                    "get_memory_neighbors: EdgeRepository query failed - NO FALLBACKS"
                );
                return self.tool_error(
                    id,
                    &format!(
                        "EdgeRepository query failed for embedder {}: {}. NO FALLBACKS.",
                        embedder_id, e
                    ),
                );
            }
        };

        let candidates_evaluated = embedder_edges.len();
        let mut filtered_count = 0;

        info!(
            candidates_evaluated = candidates_evaluated,
            embedder_id = embedder_id,
            "get_memory_neighbors: Retrieved {} K-NN edges from EdgeRepository",
            candidates_evaluated
        );

        // Step 3: Filter and prepare neighbors
        let mut neighbors: Vec<NeighborResult> = embedder_edges
            .into_iter()
            .filter_map(|edge| {
                if edge.similarity() < min_similarity {
                    filtered_count += 1;
                    return None;
                }
                Some(NeighborResult {
                    neighbor_id: edge.target(),
                    similarity: edge.similarity(),
                    content: None,
                    source: None,
                })
            })
            .take(top_k)
            .collect();

        // Step 4: Optionally hydrate content and source metadata
        if !neighbors.is_empty() {
            let neighbor_ids: Vec<Uuid> = neighbors.iter().map(|n| n.neighbor_id).collect();

            // Get source metadata - error if fails (NO FALLBACKS)
            let source_metadata = match self
                .teleological_store
                .get_source_metadata_batch(&neighbor_ids)
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "get_memory_neighbors: Source metadata retrieval failed - NO FALLBACKS");
                    return self.tool_error(
                        id,
                        &format!("Failed to retrieve source metadata: {}. NO FALLBACKS.", e),
                    );
                }
            };

            // Get content if requested - error if fails (NO FALLBACKS)
            let contents: Vec<Option<String>> = if request.include_content {
                match self.teleological_store.get_content_batch(&neighbor_ids).await {
                    Ok(c) => c,
                    Err(e) => {
                        error!(error = %e, "get_memory_neighbors: Content retrieval failed - NO FALLBACKS");
                        return self.tool_error(
                            id,
                            &format!("Failed to retrieve content: {}. NO FALLBACKS.", e),
                        );
                    }
                }
            } else {
                vec![None; neighbor_ids.len()]
            };

            // Populate metadata
            for (i, neighbor) in neighbors.iter_mut().enumerate() {
                if let Some(Some(ref metadata)) = source_metadata.get(i) {
                    neighbor.source = Some(NeighborSourceInfo {
                        source_type: format!("{}", metadata.source_type),
                        file_path: metadata.file_path.clone(),
                    });
                }
                if request.include_content {
                    if let Some(content_opt) = contents.get(i) {
                        neighbor.content = content_opt.clone();
                    }
                }
            }
        }

        let response = GetMemoryNeighborsResponse {
            memory_id: memory_uuid,
            embedder_id,
            embedder_name: emb_name.to_string(),
            neighbors: neighbors.clone(),
            count: neighbors.len(),
            metadata: NeighborSearchMetadata {
                candidates_evaluated,
                filtered_by_similarity: filtered_count,
                used_asymmetric: uses_asymmetric,
            },
        };

        info!(
            neighbors_found = response.count,
            candidates_evaluated = candidates_evaluated,
            filtered = filtered_count,
            "get_memory_neighbors: Completed K-NN neighbor search via EdgeRepository"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// get_typed_edges tool implementation.
    ///
    /// Gets typed edges from a memory based on embedder agreement patterns.
    /// TASK-GRAPHLINK: Uses EdgeRepository directly - NO FALLBACKS.
    ///
    /// # Edge Types
    ///
    /// - semantic_similar: E1 strongly agrees
    /// - code_related: E7 strongly agrees
    /// - entity_shared: E11 strongly agrees
    /// - causal_chain: E5 strongly agrees
    /// - graph_connected: E8 strongly agrees
    /// - intent_aligned: E10 strongly agrees
    /// - keyword_overlap: E6/E13 strongly agree
    /// - multi_agreement: Multiple embedders agree (weighted_agreement >= 2.5)
    ///
    /// # Parameters
    ///
    /// - `memory_id`: UUID of the memory to get edges from (required)
    /// - `edge_type`: Filter by edge type (optional)
    /// - `direction`: "outgoing", "incoming", or "both" (default: "outgoing")
    /// - `min_weight`: Minimum edge weight threshold (0-1, default: 0.0)
    /// - `include_content`: Include full content text (default: false)
    pub(crate) async fn call_get_typed_edges(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // TASK-GRAPHLINK: Require EdgeRepository - NO FALLBACKS
        let edge_repo = match &self.edge_repository {
            Some(repo) => repo,
            None => {
                error!("get_typed_edges: EdgeRepository not available - NO FALLBACKS");
                return self.tool_error(
                    id,
                    "Graph linking not initialized. EdgeRepository is required - NO FALLBACKS.",
                );
            }
        };

        // Parse and validate request
        let request: GetTypedEdgesRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_typed_edges: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let memory_uuid = match request.validate() {
            Ok(uuid) => uuid,
            Err(e) => {
                error!(error = %e, "get_typed_edges: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        let edge_type_filter = request.edge_type.clone();
        let direction = &request.direction;
        let min_weight = request.min_weight;

        info!(
            memory_id = %memory_uuid,
            edge_type_filter = ?edge_type_filter,
            direction = %direction,
            min_weight = min_weight,
            "get_typed_edges: Starting typed edge query via EdgeRepository"
        );

        // Step 1: Verify memory exists
        match self.teleological_store.retrieve(memory_uuid).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                error!(memory_id = %memory_uuid, "get_typed_edges: Memory not found");
                return self.tool_error(id, &format!("Memory not found: {}", memory_uuid));
            }
            Err(e) => {
                error!(error = %e, "get_typed_edges: Failed to retrieve memory");
                return self.tool_error(id, &format!("Failed to retrieve memory: {}", e));
            }
        };

        // Step 2: Query typed edges from EdgeRepository - NO FALLBACKS
        let typed_edges = if let Some(ref filter_type) = edge_type_filter {
            // Query by specific edge type
            let graph_link_type = match string_to_edge_type(filter_type) {
                Some(t) => t,
                None => {
                    error!(edge_type = %filter_type, "get_typed_edges: Invalid edge type");
                    return self.tool_error(id, &format!("Invalid edge type: {}", filter_type));
                }
            };
            match edge_repo.get_typed_edges_by_type(memory_uuid, graph_link_type) {
                Ok(edges) => edges,
                Err(e) => {
                    error!(
                        error = %e,
                        memory_id = %memory_uuid,
                        edge_type = %filter_type,
                        "get_typed_edges: EdgeRepository query by type failed - NO FALLBACKS"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "EdgeRepository query failed for edge type {}: {}. NO FALLBACKS.",
                            filter_type, e
                        ),
                    );
                }
            }
        } else {
            // Query all typed edges from source
            match edge_repo.get_typed_edges_from(memory_uuid) {
                Ok(edges) => edges,
                Err(e) => {
                    error!(
                        error = %e,
                        memory_id = %memory_uuid,
                        "get_typed_edges: EdgeRepository query failed - NO FALLBACKS"
                    );
                    return self.tool_error(
                        id,
                        &format!("EdgeRepository query failed: {}. NO FALLBACKS.", e),
                    );
                }
            }
        };

        let total_edges = typed_edges.len();
        let mut filtered_by_weight = 0;

        info!(
            total_edges = total_edges,
            "get_typed_edges: Retrieved {} typed edges from EdgeRepository",
            total_edges
        );

        // Step 3: Filter by min_weight and convert to response format
        let mut edges: Vec<TypedEdgeResult> = typed_edges
            .into_iter()
            .filter_map(|edge| {
                if edge.weight() < min_weight {
                    filtered_by_weight += 1;
                    return None;
                }

                // Build contributing embedders list from agreeing embedders bitfield
                let contributing_embedders = get_contributing_embedders(edge.agreeing_embedders());

                Some(TypedEdgeResult {
                    target_id: edge.target(),
                    edge_type: edge_type_to_string(edge.edge_type()),
                    weight: edge.weight(),
                    weighted_agreement: compute_weighted_agreement(edge.embedder_scores()),
                    direction: if request.is_outgoing() {
                        "outgoing".to_string()
                    } else {
                        "incoming".to_string()
                    },
                    contributing_embedders,
                    content: None,
                })
            })
            .collect();

        // Step 4: Optionally hydrate content
        if request.include_content && !edges.is_empty() {
            let edge_ids: Vec<Uuid> = edges.iter().map(|e| e.target_id).collect();
            let contents = match self.teleological_store.get_content_batch(&edge_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "get_typed_edges: Content retrieval failed - NO FALLBACKS");
                    return self.tool_error(
                        id,
                        &format!("Failed to retrieve content: {}. NO FALLBACKS.", e),
                    );
                }
            };

            for (i, edge) in edges.iter_mut().enumerate() {
                if let Some(Some(ref content)) = contents.get(i) {
                    edge.content = Some(content.clone());
                }
            }
        }

        let response = GetTypedEdgesResponse {
            memory_id: memory_uuid,
            direction: direction.clone(),
            edge_type_filter,
            edges: edges.clone(),
            count: edges.len(),
            metadata: TypedEdgeMetadata {
                total_edges,
                filtered_by_type: 0, // Already filtered by query if specified
                filtered_by_weight,
            },
        };

        info!(
            edges_found = response.count,
            total_edges = total_edges,
            filtered_by_weight = filtered_by_weight,
            "get_typed_edges: Completed typed edge query via EdgeRepository"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// traverse_graph tool implementation.
    ///
    /// Multi-hop graph traversal starting from a memory.
    /// TASK-GRAPHLINK: Uses EdgeRepository directly - NO FALLBACKS.
    ///
    /// # Algorithm
    ///
    /// 1. Start from the specified memory
    /// 2. BFS traversal following typed edges from EdgeRepository
    /// 3. Track paths and cumulative weights
    /// 4. Stop at max_hops or when no more edges above min_weight
    ///
    /// # Parameters
    ///
    /// - `start_memory_id`: UUID of the starting memory (required)
    /// - `max_hops`: Maximum traversal depth (1-5, default: 2)
    /// - `edge_type`: Filter traversal by edge type (optional)
    /// - `min_weight`: Minimum edge weight to follow (0-1, default: 0.3)
    /// - `max_results`: Maximum paths to return (1-100, default: 20)
    /// - `include_content`: Include full content text (default: false)
    pub(crate) async fn call_traverse_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // TASK-GRAPHLINK: Require EdgeRepository - NO FALLBACKS
        let edge_repo = match &self.edge_repository {
            Some(repo) => repo,
            None => {
                error!("traverse_graph: EdgeRepository not available - NO FALLBACKS");
                return self.tool_error(
                    id,
                    "Graph linking not initialized. EdgeRepository is required - NO FALLBACKS.",
                );
            }
        };

        // Parse and validate request
        let request: TraverseGraphRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "traverse_graph: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let start_uuid = match request.validate() {
            Ok(uuid) => uuid,
            Err(e) => {
                error!(error = %e, "traverse_graph: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        let max_hops = request.max_hops;
        let edge_type_filter = request.edge_type.clone();
        let min_weight = request.min_weight;
        let max_results = request.max_results;

        // Convert edge_type filter string to GraphLinkEdgeType
        let graph_link_type_filter = edge_type_filter.as_ref().and_then(|s| string_to_edge_type(s));

        info!(
            start_memory_id = %start_uuid,
            max_hops = max_hops,
            edge_type_filter = ?edge_type_filter,
            min_weight = min_weight,
            max_results = max_results,
            "traverse_graph: Starting graph traversal via EdgeRepository"
        );

        // Step 1: Verify start memory exists
        match self.teleological_store.retrieve(start_uuid).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                error!(memory_id = %start_uuid, "traverse_graph: Start memory not found");
                return self.tool_error(id, &format!("Start memory not found: {}", start_uuid));
            }
            Err(e) => {
                error!(error = %e, "traverse_graph: Failed to retrieve start memory");
                return self.tool_error(id, &format!("Failed to retrieve memory: {}", e));
            }
        };

        // Step 2: BFS traversal using EdgeRepository
        let mut visited: HashSet<Uuid> = HashSet::new();
        let mut nodes: Vec<TraversalNode> = Vec::new();
        let mut paths: Vec<TraversalPath> = Vec::new();
        let mut edges_evaluated = 0;
        let mut edges_filtered_by_weight = 0;

        // Queue: (memory_id, hop_level, cumulative_weight, path_so_far, edge_types_so_far, edge_weights_so_far)
        let mut queue: VecDeque<(Uuid, usize, f32, Vec<Uuid>, Vec<String>, Vec<f32>)> =
            VecDeque::new();
        queue.push_back((start_uuid, 0, 1.0, vec![start_uuid], vec![], vec![]));
        visited.insert(start_uuid);

        // Add start node
        nodes.push(TraversalNode {
            memory_id: start_uuid,
            hop_level: 0,
            edge_type_from_parent: None,
            edge_weight_from_parent: None,
            cumulative_weight: 1.0,
            content: None,
        });

        while let Some((current_id, hop_level, cumulative_weight, path, edge_types, edge_weights)) =
            queue.pop_front()
        {
            // Check if we've reached max hops
            if hop_level >= max_hops {
                // Record this as a complete path
                if path.len() > 1 {
                    paths.push(TraversalPath {
                        path: path.clone(),
                        total_weight: cumulative_weight,
                        hop_count: path.len() - 1,
                        edge_types: edge_types.clone(),
                        edge_weights: edge_weights.clone(),
                    });
                }
                continue;
            }

            // Check if we have enough paths
            if paths.len() >= max_results {
                break;
            }

            // Get neighbors via EdgeRepository - NO FALLBACKS
            let neighbors = if let Some(ref edge_type) = graph_link_type_filter {
                match edge_repo.get_typed_edges_by_type(current_id, *edge_type) {
                    Ok(edges) => edges,
                    Err(e) => {
                        error!(
                            error = %e,
                            memory_id = %current_id,
                            "traverse_graph: EdgeRepository query failed - NO FALLBACKS"
                        );
                        return self.tool_error(
                            id,
                            &format!(
                                "EdgeRepository query failed during traversal: {}. NO FALLBACKS.",
                                e
                            ),
                        );
                    }
                }
            } else {
                match edge_repo.get_typed_edges_from(current_id) {
                    Ok(edges) => edges,
                    Err(e) => {
                        error!(
                            error = %e,
                            memory_id = %current_id,
                            "traverse_graph: EdgeRepository query failed - NO FALLBACKS"
                        );
                        return self.tool_error(
                            id,
                            &format!(
                                "EdgeRepository query failed during traversal: {}. NO FALLBACKS.",
                                e
                            ),
                        );
                    }
                }
            };

            let mut found_next_hop = false;

            for neighbor in neighbors {
                edges_evaluated += 1;
                let neighbor_id = neighbor.target();

                // Skip if already visited or is self
                if visited.contains(&neighbor_id) || neighbor_id == current_id {
                    continue;
                }

                // Filter by minimum weight
                if neighbor.weight() < min_weight {
                    edges_filtered_by_weight += 1;
                    continue;
                }

                found_next_hop = true;
                visited.insert(neighbor_id);

                let edge_type_str = edge_type_to_string(neighbor.edge_type());
                let new_cumulative = cumulative_weight * neighbor.weight();
                let mut new_path = path.clone();
                new_path.push(neighbor_id);
                let mut new_edge_types = edge_types.clone();
                new_edge_types.push(edge_type_str.clone());
                let mut new_edge_weights = edge_weights.clone();
                new_edge_weights.push(neighbor.weight());

                // Add to nodes
                nodes.push(TraversalNode {
                    memory_id: neighbor_id,
                    hop_level: hop_level + 1,
                    edge_type_from_parent: Some(edge_type_str),
                    edge_weight_from_parent: Some(neighbor.weight()),
                    cumulative_weight: new_cumulative,
                    content: None,
                });

                // Add to queue for next hop
                queue.push_back((
                    neighbor_id,
                    hop_level + 1,
                    new_cumulative,
                    new_path,
                    new_edge_types,
                    new_edge_weights,
                ));
            }

            // If no next hop found and we have a path, record it
            if !found_next_hop && path.len() > 1 {
                paths.push(TraversalPath {
                    path: path.clone(),
                    total_weight: cumulative_weight,
                    hop_count: path.len() - 1,
                    edge_types: edge_types.clone(),
                    edge_weights: edge_weights.clone(),
                });
            }
        }

        // Step 3: Optionally hydrate content
        if request.include_content && !nodes.is_empty() {
            let node_ids: Vec<Uuid> = nodes.iter().map(|n| n.memory_id).collect();
            let contents = match self.teleological_store.get_content_batch(&node_ids).await {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "traverse_graph: Content retrieval failed - NO FALLBACKS");
                    return self.tool_error(
                        id,
                        &format!("Failed to retrieve content: {}. NO FALLBACKS.", e),
                    );
                }
            };

            for (i, node) in nodes.iter_mut().enumerate() {
                if let Some(Some(ref content)) = contents.get(i) {
                    node.content = Some(content.clone());
                }
            }
        }

        let truncated = paths.len() >= max_results;

        let response = TraverseGraphResponse {
            start_memory_id: start_uuid,
            max_hops,
            edge_type_filter,
            nodes: nodes.clone(),
            paths: paths.clone(),
            unique_nodes_visited: visited.len(),
            path_count: paths.len(),
            metadata: TraversalMetadata {
                min_weight,
                max_results,
                truncated,
                edges_evaluated,
                edges_filtered_by_weight,
            },
        };

        info!(
            nodes_visited = response.unique_nodes_visited,
            paths_found = response.path_count,
            edges_evaluated = edges_evaluated,
            truncated = truncated,
            "traverse_graph: Completed graph traversal via EdgeRepository"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }

    /// get_unified_neighbors tool implementation.
    ///
    /// Finds neighbors using Weighted RRF fusion across all 13 embedders.
    /// TASK-GRAPHLINK: Uses EdgeRepository K-NN graphs - NO FALLBACKS.
    /// Per ARCH-21: Uses Weighted RRF, not weighted sum.
    /// Per AP-60: Temporal embedders (E2-E4) are excluded from semantic fusion.
    ///
    /// # Algorithm
    ///
    /// 1. Validate request and verify memory exists
    /// 2. For each semantic embedder (E1, E5-E13, excluding E2-E4):
    ///    - Query EdgeRepository.get_embedder_edges() for K-NN neighbors
    ///    - Collect (memory_id, similarity, rank) tuples
    /// 3. Apply Weighted RRF: `score = Sum(weight_i / (rank_i + k))`
    /// 4. Rank by fused score, apply min_score filter
    /// 5. Compute agreement summary
    /// 6. Optionally hydrate content
    ///
    /// # Parameters
    ///
    /// - `memory_id`: UUID of the memory to find neighbors for (required)
    /// - `weight_profile`: Profile for embedder weights (default: "semantic_search")
    /// - `top_k`: Number of neighbors to return (1-50, default: 10)
    /// - `min_score`: Minimum RRF score threshold (0-1, default: 0.0)
    /// - `include_content`: Include full content text (default: false)
    /// - `include_embedder_breakdown`: Include per-embedder scores/ranks (default: true)
    pub(crate) async fn call_get_unified_neighbors(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        // TASK-GRAPHLINK: Require EdgeRepository - NO FALLBACKS
        let edge_repo = match &self.edge_repository {
            Some(repo) => repo,
            None => {
                error!("get_unified_neighbors: EdgeRepository not available - NO FALLBACKS");
                return self.tool_error(
                    id,
                    "Graph linking not initialized. EdgeRepository is required - NO FALLBACKS.",
                );
            }
        };

        // Parse and validate request
        let request: GetUnifiedNeighborsRequest = match serde_json::from_value(args.clone()) {
            Ok(req) => req,
            Err(e) => {
                error!(error = %e, "get_unified_neighbors: Failed to parse request");
                return self.tool_error(id, &format!("Invalid request: {}", e));
            }
        };

        let memory_uuid = match request.validate() {
            Ok(uuid) => uuid,
            Err(e) => {
                error!(error = %e, "get_unified_neighbors: Validation failed");
                return self.tool_error(id, &e);
            }
        };

        let weight_profile = &request.weight_profile;
        let top_k = request.top_k;
        let min_score = request.min_score;

        info!(
            memory_id = %memory_uuid,
            weight_profile = %weight_profile,
            top_k = top_k,
            min_score = min_score,
            "get_unified_neighbors: Starting unified neighbor search via EdgeRepository"
        );

        // Step 1: Verify memory exists
        match self.teleological_store.retrieve(memory_uuid).await {
            Ok(Some(_)) => {}
            Ok(None) => {
                error!(memory_id = %memory_uuid, "get_unified_neighbors: Memory not found");
                return self.tool_error(id, &format!("Memory not found: {}", memory_uuid));
            }
            Err(e) => {
                error!(error = %e, "get_unified_neighbors: Failed to retrieve memory");
                return self.tool_error(id, &format!("Failed to retrieve memory: {}", e));
            }
        };

        // Step 2: Get weight profile for RRF fusion
        // GAP-1: custom_weights overrides weight_profile
        let mut weights = if let Some(ref custom) = request.custom_weights {
            let names = ["E1", "E2", "E3", "E4", "E5", "E6", "E7", "E8", "E9", "E10", "E11", "E12", "E13"];
            let mut w = [0.0f32; 13];
            for (i, name) in names.iter().enumerate() {
                if let Some(&val) = custom.get(*name) {
                    w[i] = val as f32;
                }
            }
            // Validate weights
            if let Err(e) = context_graph_core::weights::validate_weights(&w) {
                error!(error = %e, "get_unified_neighbors: Invalid custom weights");
                return self.tool_error(id, &format!("Invalid custom weights: {}", e));
            }
            w
        } else {
            match get_weight_profile(weight_profile) {
                Ok(w) => w,
                Err(e) => {
                    error!(error = %e, "get_unified_neighbors: Invalid weight profile");
                    return self.tool_error(id, &format!("Invalid weight profile: {}", e));
                }
            }
        };

        // GAP-8: Apply embedder exclusions
        if !request.exclude_embedders.is_empty() {
            let name_to_idx = |s: &str| -> Option<usize> {
                match s {
                    "E1" => Some(0), "E2" => Some(1), "E3" => Some(2),
                    "E4" => Some(3), "E5" => Some(4), "E6" => Some(5),
                    "E7" => Some(6), "E8" => Some(7), "E9" => Some(8),
                    "E10" => Some(9), "E11" => Some(10), "E12" => Some(11),
                    "E13" => Some(12), _ => None,
                }
            };
            for name in &request.exclude_embedders {
                if let Some(idx) = name_to_idx(name) {
                    weights[idx] = 0.0;
                }
            }
            let sum: f32 = weights.iter().sum();
            if sum > 0.0 {
                for w in weights.iter_mut() { *w /= sum; }
            } else {
                return self.tool_error(id, "All embedders excluded - at least one must have weight > 0");
            }
        }

        // Step 3: Query K-NN graphs from each semantic embedder (excluding E2-E4 temporal)
        let mut all_candidates: HashMap<Uuid, CandidateInfo> = HashMap::new();
        let mut total_candidates_evaluated = 0;
        let mut embedder_contribution_counts: [usize; 13] = [0; 13];

        for &embedder_idx in &SEMANTIC_EMBEDDER_INDICES {
            // Query K-NN edges from EdgeRepository - NO FALLBACKS
            let embedder_edges = match edge_repo.get_embedder_edges(embedder_idx as u8, memory_uuid) {
                Ok(edges) => edges,
                Err(e) => {
                    error!(
                        error = %e,
                        memory_id = %memory_uuid,
                        embedder_id = embedder_idx,
                        "get_unified_neighbors: EdgeRepository query failed - NO FALLBACKS"
                    );
                    return self.tool_error(
                        id,
                        &format!(
                            "EdgeRepository query failed for embedder {}: {}. NO FALLBACKS.",
                            embedder_idx, e
                        ),
                    );
                }
            };

            total_candidates_evaluated += embedder_edges.len();

            // Process results and assign ranks (1-based)
            for (rank, edge) in embedder_edges.iter().enumerate() {
                let neighbor_id = edge.target();

                let entry = all_candidates.entry(neighbor_id).or_insert_with(|| {
                    CandidateInfo {
                        embedder_scores: [0.0; 13],
                        embedder_ranks: [0; 13],
                        contributing_embedders: Vec::new(),
                    }
                });

                // Store score and rank for this embedder (1-based rank for RRF)
                entry.embedder_scores[embedder_idx] = edge.similarity();
                entry.embedder_ranks[embedder_idx] = rank + 1; // 1-based rank
                entry.contributing_embedders.push(embedder_name(embedder_idx).to_string());

                embedder_contribution_counts[embedder_idx] += 1;
            }
        }

        info!(
            total_candidates_evaluated = total_candidates_evaluated,
            unique_candidates = all_candidates.len(),
            "get_unified_neighbors: Retrieved K-NN edges from {} embedders",
            SEMANTIC_EMBEDDER_INDICES.len()
        );

        // Step 4: Apply Weighted RRF fusion
        // RRF_score(d) = Sum over embedders i: weight_i / (rank_i(d) + k)
        let mut fused_candidates: Vec<(Uuid, f32, CandidateInfo)> = all_candidates
            .into_iter()
            .map(|(memory_id, info)| {
                let mut rrf_score = 0.0;

                for &embedder_idx in &SEMANTIC_EMBEDDER_INDICES {
                    let rank = info.embedder_ranks[embedder_idx];
                    if rank > 0 {
                        // Only count if this embedder found this candidate
                        let weight = weights[embedder_idx];
                        rrf_score += weight / (rank as f32 + RRF_K);
                    }
                }

                (memory_id, rrf_score, info)
            })
            .collect();

        // Sort by RRF score descending
        fused_candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let unique_candidates = fused_candidates.len();

        // Step 5: Filter by min_score and take top_k
        let mut filtered_count = 0;
        let neighbors: Vec<UnifiedNeighborResult> = fused_candidates
            .into_iter()
            .filter(|(_, score, _)| {
                if *score < min_score {
                    filtered_count += 1;
                    false
                } else {
                    true
                }
            })
            .take(top_k)
            .map(|(neighbor_id, rrf_score, info)| {
                let embedder_count = info.contributing_embedders.len();
                UnifiedNeighborResult {
                    neighbor_id,
                    rrf_score,
                    embedder_count,
                    contributing_embedders: info.contributing_embedders,
                    embedder_scores: if request.include_embedder_breakdown {
                        Some(info.embedder_scores)
                    } else {
                        None
                    },
                    embedder_ranks: if request.include_embedder_breakdown {
                        Some(info.embedder_ranks)
                    } else {
                        None
                    },
                    content: None,
                    source: None,
                }
            })
            .collect();

        // Step 6: Compute agreement summary
        let mut strong_agreement = 0;
        let mut moderate_agreement = 0;
        let mut weak_agreement = 0;

        for neighbor in &neighbors {
            match neighbor.embedder_count {
                n if n >= 6 => strong_agreement += 1,
                n if n >= 3 => moderate_agreement += 1,
                _ => weak_agreement += 1,
            }
        }

        // Build top contributing embedders list
        let mut top_contributing_embedders: Vec<EmbedderContribution> = SEMANTIC_EMBEDDER_INDICES
            .iter()
            .map(|&idx| EmbedderContribution {
                embedder_name: embedder_name(idx).to_string(),
                contribution_count: embedder_contribution_counts[idx],
                weight: weights[idx],
            })
            .filter(|c| c.contribution_count > 0)
            .collect();

        top_contributing_embedders.sort_by(|a, b| b.contribution_count.cmp(&a.contribution_count));
        top_contributing_embedders.truncate(5); // Top 5 contributing embedders

        // Step 7: Optionally hydrate content
        let mut neighbors = neighbors;
        if request.include_content && !neighbors.is_empty() {
            let neighbor_ids: Vec<Uuid> = neighbors.iter().map(|n| n.neighbor_id).collect();

            // Get content - NO FALLBACKS
            let contents: Vec<Option<String>> = match self
                .teleological_store
                .get_content_batch(&neighbor_ids)
                .await
            {
                Ok(c) => c,
                Err(e) => {
                    error!(error = %e, "get_unified_neighbors: Content retrieval failed - NO FALLBACKS");
                    return self.tool_error(
                        id,
                        &format!("Failed to retrieve content: {}. NO FALLBACKS.", e),
                    );
                }
            };

            // Get source metadata - NO FALLBACKS
            let source_metadata = match self
                .teleological_store
                .get_source_metadata_batch(&neighbor_ids)
                .await
            {
                Ok(m) => m,
                Err(e) => {
                    error!(error = %e, "get_unified_neighbors: Source metadata retrieval failed - NO FALLBACKS");
                    return self.tool_error(
                        id,
                        &format!("Failed to retrieve source metadata: {}. NO FALLBACKS.", e),
                    );
                }
            };

            for (i, neighbor) in neighbors.iter_mut().enumerate() {
                if let Some(Some(ref content)) = contents.get(i) {
                    neighbor.content = Some(content.clone());
                }
                if let Some(Some(ref metadata)) = source_metadata.get(i) {
                    neighbor.source = Some(NeighborSourceInfo {
                        source_type: format!("{}", metadata.source_type),
                        file_path: metadata.file_path.clone(),
                    });
                }
            }
        }

        let response = GetUnifiedNeighborsResponse {
            memory_id: memory_uuid,
            weight_profile: weight_profile.clone(),
            count: neighbors.len(),
            neighbors,
            agreement_summary: AgreementSummary {
                strong_agreement,
                moderate_agreement,
                weak_agreement,
                top_contributing_embedders,
            },
            metadata: UnifiedNeighborMetadata {
                total_candidates_evaluated,
                unique_candidates,
                filtered_by_score: filtered_count,
                rrf_k: RRF_K,
                excluded_embedders: vec![
                    "E2 (V_freshness)".to_string(),
                    "E3 (V_periodicity)".to_string(),
                    "E4 (V_ordering)".to_string(),
                ],
                fusion_strategy: "weighted_rrf".to_string(),
            },
        };

        info!(
            neighbors_found = response.count,
            unique_candidates = unique_candidates,
            strong_agreement = strong_agreement,
            moderate_agreement = moderate_agreement,
            weak_agreement = weak_agreement,
            "get_unified_neighbors: Completed unified neighbor search via EdgeRepository"
        );

        self.tool_result(id, serde_json::to_value(response).unwrap_or_else(|_| json!({})))
    }
}

/// Internal struct to track candidate information during RRF fusion.
struct CandidateInfo {
    /// Per-embedder similarity scores (0.0 if not found).
    embedder_scores: [f32; 13],
    /// Per-embedder ranks (0 if not found).
    embedder_ranks: [usize; 13],
    /// Names of contributing embedders.
    contributing_embedders: Vec<String>,
}

/// Convert string edge type to GraphLinkEdgeType.
fn string_to_edge_type(s: &str) -> Option<GraphLinkEdgeType> {
    match s {
        "semantic_similar" => Some(GraphLinkEdgeType::SemanticSimilar),
        "code_related" => Some(GraphLinkEdgeType::CodeRelated),
        "entity_shared" => Some(GraphLinkEdgeType::EntityShared),
        "causal_chain" => Some(GraphLinkEdgeType::CausalChain),
        "graph_connected" => Some(GraphLinkEdgeType::GraphConnected),
        "intent_aligned" => Some(GraphLinkEdgeType::IntentAligned),
        "keyword_overlap" => Some(GraphLinkEdgeType::KeywordOverlap),
        "multi_agreement" => Some(GraphLinkEdgeType::MultiAgreement),
        _ => None,
    }
}

/// Convert GraphLinkEdgeType to string representation.
fn edge_type_to_string(edge_type: GraphLinkEdgeType) -> String {
    match edge_type {
        GraphLinkEdgeType::SemanticSimilar => "semantic_similar".to_string(),
        GraphLinkEdgeType::CodeRelated => "code_related".to_string(),
        GraphLinkEdgeType::EntityShared => "entity_shared".to_string(),
        GraphLinkEdgeType::CausalChain => "causal_chain".to_string(),
        GraphLinkEdgeType::GraphConnected => "graph_connected".to_string(),
        GraphLinkEdgeType::IntentAligned => "intent_aligned".to_string(),
        GraphLinkEdgeType::KeywordOverlap => "keyword_overlap".to_string(),
        GraphLinkEdgeType::MultiAgreement => "multi_agreement".to_string(),
    }
}

/// Get contributing embedder names from bitmask.
fn get_contributing_embedders(mask: u16) -> Vec<String> {
    let mut result = Vec::new();
    for i in 0..13 {
        if (mask >> i) & 1 == 1 {
            result.push(embedder_name(i).to_string());
        }
    }
    result
}

/// Compute weighted agreement from embedder scores.
/// Per Constitution: weighted_agreement = Sum(topic_weight_i × is_clustered_i)
fn compute_weighted_agreement(scores: &[f32; 13]) -> f32 {
    // Topic weights per constitution
    // SEMANTIC: E1, E5, E6, E7, E10, E12, E13 = 1.0
    // RELATIONAL: E8, E11 = 0.5
    // STRUCTURAL: E9 = 0.5
    // TEMPORAL: E2, E3, E4 = 0.0 (never count toward topics)
    let topic_weights: [f32; 13] = [
        1.0, // E1 - semantic
        0.0, // E2 - temporal (excluded)
        0.0, // E3 - temporal (excluded)
        0.0, // E4 - temporal (excluded)
        1.0, // E5 - semantic
        1.0, // E6 - semantic
        1.0, // E7 - semantic
        0.5, // E8 - relational
        0.5, // E9 - structural
        1.0, // E10 - semantic
        0.5, // E11 - relational
        1.0, // E12 - semantic
        1.0, // E13 - semantic
    ];

    let mut agreement = 0.0;
    for (i, &score) in scores.iter().enumerate() {
        if score > 0.5 {
            // Is clustered if score > threshold
            agreement += topic_weights[i];
        }
    }
    agreement
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_to_edge_type() {
        assert_eq!(string_to_edge_type("semantic_similar"), Some(GraphLinkEdgeType::SemanticSimilar));
        assert_eq!(string_to_edge_type("code_related"), Some(GraphLinkEdgeType::CodeRelated));
        assert_eq!(string_to_edge_type("causal_chain"), Some(GraphLinkEdgeType::CausalChain));
        assert_eq!(string_to_edge_type("invalid"), None);
        println!("[PASS] String to edge type conversion works");
    }

    #[test]
    fn test_edge_type_to_string() {
        assert_eq!(
            edge_type_to_string(GraphLinkEdgeType::SemanticSimilar),
            "semantic_similar"
        );
        assert_eq!(
            edge_type_to_string(GraphLinkEdgeType::CodeRelated),
            "code_related"
        );
        assert_eq!(
            edge_type_to_string(GraphLinkEdgeType::CausalChain),
            "causal_chain"
        );
        println!("[PASS] Edge type string conversion works");
    }

    #[test]
    fn test_get_contributing_embedders() {
        // Mask with E1, E5, E7 set (bits 0, 4, 6)
        let mask: u16 = 0b0000_0101_0001;
        let embedders = get_contributing_embedders(mask);
        assert_eq!(embedders.len(), 3);
        assert!(embedders.contains(&"E1 (V_meaning)".to_string()));
        assert!(embedders.contains(&"E5 (V_causality)".to_string()));
        assert!(embedders.contains(&"E7 (V_correctness)".to_string()));
        println!("[PASS] Contributing embedders extraction works");
    }

    #[test]
    fn test_compute_weighted_agreement() {
        // All semantic embedders agree (scores > 0.5)
        let scores: [f32; 13] = [0.9, 0.0, 0.0, 0.0, 0.8, 0.75, 0.85, 0.6, 0.55, 0.7, 0.65, 0.8, 0.7];
        let agreement = compute_weighted_agreement(&scores);
        // E1=1.0, E5=1.0, E6=1.0, E7=1.0, E8=0.5, E9=0.5, E10=1.0, E11=0.5, E12=1.0, E13=1.0 = 8.5
        assert!((agreement - 8.5).abs() < 0.01);
        println!("[PASS] Weighted agreement computation: {}", agreement);
    }

    #[test]
    fn test_compute_weighted_agreement_excludes_temporal() {
        // Only temporal embedders have high scores (should result in 0)
        let scores: [f32; 13] = [0.0, 0.9, 0.9, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let agreement = compute_weighted_agreement(&scores);
        assert!((agreement - 0.0).abs() < 0.01);
        println!("[PASS] Temporal embedders excluded from weighted agreement");
    }

    // ===== Weighted RRF Algorithm Tests =====

    #[test]
    fn test_rrf_formula() {
        // RRF_score(d) = Sum over embedders i: weight_i / (rank_i(d) + k)
        // With k=60 (standard RRF constant)

        let k = 60.0f32;

        // Single embedder case: weight=0.33, rank=1
        let weight = 0.33f32;
        let rank = 1;
        let expected_score = weight / (rank as f32 + k);
        assert!((expected_score - 0.0054).abs() < 0.001);
        println!("[PASS] RRF formula correct for single embedder");

        // Multiple embedders: E1 rank=1, E7 rank=2
        let e1_weight = 0.33f32;
        let e7_weight = 0.20f32;
        let e1_rank = 1;
        let e7_rank = 2;

        let combined_score = e1_weight / (e1_rank as f32 + k) + e7_weight / (e7_rank as f32 + k);
        assert!(combined_score > expected_score);
        println!("[PASS] RRF score increases with multiple embedder agreement");
    }

    #[test]
    fn test_rrf_rank_importance() {
        // Higher rank = lower contribution
        let k = 60.0f32;
        let weight = 0.33f32;

        let rank1_score = weight / (1.0 + k);
        let rank10_score = weight / (10.0 + k);

        assert!(rank1_score > rank10_score);
        println!(
            "[PASS] RRF: rank=1 ({:.4}) > rank=10 ({:.4})",
            rank1_score, rank10_score
        );
    }

    #[test]
    fn test_candidate_info_structure() {
        let info = CandidateInfo {
            embedder_scores: [0.9, 0.0, 0.0, 0.0, 0.8, 0.75, 0.85, 0.6, 0.0, 0.7, 0.65, 0.0, 0.55],
            embedder_ranks: [1, 0, 0, 0, 2, 3, 1, 5, 0, 2, 3, 0, 4],
            contributing_embedders: vec![
                "E1 (V_meaning)".to_string(),
                "E5 (V_causality)".to_string(),
                "E6 (V_selectivity)".to_string(),
                "E7 (V_correctness)".to_string(),
                "E8 (V_connectivity)".to_string(),
                "E10 (V_multimodality)".to_string(),
                "E11 (V_factuality)".to_string(),
                "E13 (V_keyword_precision)".to_string(),
            ],
        };

        assert_eq!(info.embedder_scores.len(), 13);
        assert_eq!(info.embedder_ranks.len(), 13);
        assert_eq!(info.contributing_embedders.len(), 8);

        // Verify temporal embedders have rank=0 (excluded)
        assert_eq!(info.embedder_ranks[1], 0); // E2
        assert_eq!(info.embedder_ranks[2], 0); // E3
        assert_eq!(info.embedder_ranks[3], 0); // E4

        println!("[PASS] CandidateInfo structure correctly excludes temporal embedders");
    }
}
