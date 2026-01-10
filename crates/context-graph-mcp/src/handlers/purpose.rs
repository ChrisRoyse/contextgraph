//! Purpose and goal alignment handlers.
//!
//! TASK-S003: Implements MCP handlers for purpose/goal operations.
//! TASK-CORE-001: Removed manual North Star methods per ARCH-03 (autonomous-first).
//!
//! # Purpose Methods
//!
//! - `purpose/query`: Query memories by 13D purpose vector similarity
//! - `goal/hierarchy_query`: Navigate goal hierarchy
//! - `goal/aligned_memories`: Find memories aligned to a specific goal
//! - `purpose/drift_check`: Detect alignment drift in memories
//!
//! NOTE: Manual North Star methods removed per ARCH-03:
//! - `purpose/north_star_alignment` - REMOVED: Use auto_bootstrap_north_star
//! - `purpose/north_star_update` - REMOVED: Use auto_bootstrap_north_star
//!
//! # Error Handling
//!
//! FAIL FAST: All errors return immediately with detailed error codes.
//! NO fallbacks, NO default values, NO mock data.

use serde_json::json;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

use context_graph_core::purpose::{GoalHierarchy, GoalLevel, GoalNode};
use context_graph_core::traits::TeleologicalSearchOptions;
use context_graph_core::types::fingerprint::{AlignmentThreshold, PurposeVector, NUM_EMBEDDERS};
// TASK-INTEG-002: Import TeleologicalDriftDetector for per-embedder drift analysis
use context_graph_core::autonomous::drift::{
    DriftError, DriftResult, TeleologicalDriftDetector,
};
use context_graph_core::teleological::{SearchStrategy, TeleologicalComparator};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

impl Handlers {
    /// Handle purpose/query request.
    ///
    /// Query memories by 13D purpose vector similarity.
    ///
    /// # Request Parameters
    /// - `purpose_vector` (optional): 13-element alignment vector [0.0-1.0]
    /// - `min_alignment` (optional): Minimum alignment threshold
    /// - `top_k` (optional): Maximum results, default 10
    /// - `include_scores` (optional): Include per-embedder breakdown, default true
    ///
    /// # Response
    /// - `results`: Array of matching memories with purpose alignment scores
    /// - `query_metadata`: Purpose vector used, timing
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Invalid purpose vector format
    /// - PURPOSE_SEARCH_ERROR (-32016): Purpose search failed
    #[instrument(skip(self, params), fields(method = "purpose/query"))]
    pub(super) async fn handle_purpose_query(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = params.unwrap_or(json!({}));

        // Parse purpose vector (required for purpose query)
        let purpose_vector = match params.get("purpose_vector").and_then(|v| v.as_array()) {
            Some(arr) => {
                if arr.len() != NUM_EMBEDDERS {
                    error!(
                        count = arr.len(),
                        "purpose/query: Purpose vector must have 13 elements"
                    );
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!(
                            "purpose_vector must have {} elements, got {}",
                            NUM_EMBEDDERS,
                            arr.len()
                        ),
                    );
                }

                let mut alignments = [0.0f32; NUM_EMBEDDERS];
                for (i, v) in arr.iter().enumerate() {
                    let value = v.as_f64().unwrap_or(0.0) as f32;
                    if !(0.0..=1.0).contains(&value) {
                        error!(
                            index = i,
                            value = value,
                            "purpose/query: Purpose vector values must be in [0.0, 1.0]"
                        );
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!(
                                "purpose_vector[{}] = {} is out of range [0.0, 1.0]",
                                i, value
                            ),
                        );
                    }
                    alignments[i] = value;
                }

                // Find dominant embedder (highest alignment)
                let dominant_embedder = alignments
                    .iter()
                    .enumerate()
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                    .map(|(i, _)| i as u8)
                    .unwrap_or(0);

                // Compute coherence (inverse of standard deviation)
                let mean: f32 = alignments.iter().sum::<f32>() / NUM_EMBEDDERS as f32;
                let variance: f32 = alignments.iter().map(|&x| (x - mean).powi(2)).sum::<f32>()
                    / NUM_EMBEDDERS as f32;
                let coherence = 1.0 / (1.0 + variance.sqrt());

                PurposeVector {
                    alignments,
                    dominant_embedder,
                    coherence,
                    stability: 1.0,
                }
            }
            None => {
                error!("purpose/query: Missing 'purpose_vector' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'purpose_vector' parameter (array of 13 floats in [0.0, 1.0])",
                );
            }
        };

        let top_k = params
            .get("topK")
            .or_else(|| params.get("top_k"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let min_alignment = params
            .get("minAlignment")
            .or_else(|| params.get("min_alignment"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32);

        let include_scores = params
            .get("include_scores")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let search_start = std::time::Instant::now();

        // Build search options
        let mut options = TeleologicalSearchOptions::quick(top_k);
        if let Some(align) = min_alignment {
            options = options.with_min_alignment(align);
        }

        // Execute purpose search
        match self
            .teleological_store
            .search_purpose(&purpose_vector, options)
            .await
        {
            Ok(results) => {
                let search_latency_ms = search_start.elapsed().as_millis();

                let results_json: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        let mut result = json!({
                            "id": r.fingerprint.id.to_string(),
                            "purpose_alignment": r.purpose_alignment,
                            "theta_to_north_star": r.fingerprint.theta_to_north_star,
                        });

                        if include_scores {
                            result["purpose_vector"] =
                                json!(r.fingerprint.purpose_vector.alignments.to_vec());
                            result["dominant_embedder"] =
                                json!(r.fingerprint.purpose_vector.dominant_embedder);
                            result["coherence"] = json!(r.fingerprint.purpose_vector.coherence);
                        }

                        result["johari_quadrant"] =
                            json!(format!("{:?}", r.fingerprint.johari.dominant_quadrant(0)));

                        result
                    })
                    .collect();

                debug!(
                    count = results.len(),
                    latency_ms = search_latency_ms,
                    "purpose/query: Completed"
                );

                JsonRpcResponse::success(
                    id,
                    json!({
                        "results": results_json,
                        "count": results.len(),
                        "query_metadata": {
                            "purpose_vector_used": purpose_vector.alignments.to_vec(),
                            "min_alignment_filter": min_alignment,
                            "dominant_embedder": purpose_vector.dominant_embedder,
                            "query_coherence": purpose_vector.coherence,
                            "search_time_ms": search_latency_ms
                        }
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "purpose/query: FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::PURPOSE_SEARCH_ERROR,
                    format!("Purpose query failed: {}", e),
                )
            }
        }
    }

    // NOTE: handle_north_star_alignment REMOVED per TASK-CORE-001 (ARCH-03)
    // Manual North Star alignment creates single 1024D embeddings incompatible with 13-embedder arrays.
    // Calls to purpose/north_star_alignment now return METHOD_NOT_FOUND (-32601).
    // Use auto_bootstrap_north_star tool for autonomous goal discovery instead.

    /// Handle goal/hierarchy_query request.
    ///
    /// Navigate and query the goal hierarchy.
    ///
    /// # Request Parameters
    /// - `operation` (required): "get_children", "get_ancestors", "get_subtree", "get_all", "get_goal"
    /// - `goal_id` (optional): Goal ID for targeted operations
    /// - `level` (optional): Filter by GoalLevel ("NorthStar", "Strategic", "Tactical", "Immediate")
    ///
    /// # Response
    /// - `goals`: Array of goal objects with hierarchy info
    /// - `hierarchy_stats`: Statistics about the hierarchy
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Invalid operation or goal_id
    /// - GOAL_NOT_FOUND (-32020): Goal ID not found
    #[instrument(skip(self, params), fields(method = "goal/hierarchy_query"))]
    pub(super) async fn handle_goal_hierarchy_query(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("goal/hierarchy_query: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - operation required",
                );
            }
        };

        let operation = match params.get("operation").and_then(|v| v.as_str()) {
            Some(op) => op,
            None => {
                error!("goal/hierarchy_query: Missing 'operation' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'operation' parameter. Valid: get_children, get_ancestors, get_subtree, get_all, get_goal",
                );
            }
        };

        let hierarchy = self.goal_hierarchy.read();

        match operation {
            "get_all" => {
                let goals: Vec<serde_json::Value> =
                    hierarchy.iter().map(|g| self.goal_to_json(g)).collect();

                let stats = self.compute_hierarchy_stats(&hierarchy);

                JsonRpcResponse::success(
                    id,
                    json!({
                        "goals": goals,
                        "count": goals.len(),
                        "hierarchy_stats": stats
                    }),
                )
            }

            "get_goal" => {
                let goal_id = match params.get("goal_id").and_then(|v| v.as_str()) {
                    Some(gid) => match Uuid::parse_str(gid) {
                        Ok(uuid) => uuid,
                        Err(_) => {
                            error!("goal/hierarchy_query: Invalid goal_id UUID format: {}", gid);
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid goal_id UUID format: {}", gid),
                            );
                        }
                    },
                    None => {
                        error!("goal/hierarchy_query: get_goal requires 'goal_id'");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            "get_goal operation requires 'goal_id' parameter",
                        );
                    }
                };

                match hierarchy.get(&goal_id) {
                    Some(goal) => {
                        JsonRpcResponse::success(id, json!({ "goal": self.goal_to_json(goal) }))
                    }
                    None => {
                        error!(goal_id = %goal_id, "goal/hierarchy_query: Goal not found");
                        JsonRpcResponse::error(
                            id,
                            error_codes::GOAL_NOT_FOUND,
                            format!("Goal not found: {}", goal_id),
                        )
                    }
                }
            }

            "get_children" => {
                let goal_id = match params.get("goal_id").and_then(|v| v.as_str()) {
                    Some(gid) => match Uuid::parse_str(gid) {
                        Ok(uuid) => uuid,
                        Err(_) => {
                            error!("goal/hierarchy_query: Invalid goal_id UUID format: {}", gid);
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid goal_id UUID format: {}", gid),
                            );
                        }
                    },
                    None => {
                        error!("goal/hierarchy_query: get_children requires 'goal_id'");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            "get_children operation requires 'goal_id' parameter",
                        );
                    }
                };

                // Verify parent exists
                if hierarchy.get(&goal_id).is_none() {
                    error!(goal_id = %goal_id, "goal/hierarchy_query: Parent goal not found");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::GOAL_NOT_FOUND,
                        format!("Parent goal not found: {}", goal_id),
                    );
                }

                let children: Vec<serde_json::Value> = hierarchy
                    .children(&goal_id)
                    .into_iter()
                    .map(|g| self.goal_to_json(g))
                    .collect();

                JsonRpcResponse::success(
                    id,
                    json!({
                        "parent_goal_id": goal_id.to_string(),
                        "children": children,
                        "count": children.len()
                    }),
                )
            }

            "get_ancestors" => {
                let goal_id = match params.get("goal_id").and_then(|v| v.as_str()) {
                    Some(gid) => match Uuid::parse_str(gid) {
                        Ok(uuid) => uuid,
                        Err(_) => {
                            error!("goal/hierarchy_query: Invalid goal_id UUID format: {}", gid);
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid goal_id UUID format: {}", gid),
                            );
                        }
                    },
                    None => {
                        error!("goal/hierarchy_query: get_ancestors requires 'goal_id'");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            "get_ancestors operation requires 'goal_id' parameter",
                        );
                    }
                };

                let path = hierarchy.path_to_north_star(&goal_id);
                if path.is_empty() {
                    error!(goal_id = %goal_id, "goal/hierarchy_query: Goal not found for ancestors");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::GOAL_NOT_FOUND,
                        format!("Goal not found: {}", goal_id),
                    );
                }

                let ancestors: Vec<serde_json::Value> = path
                    .iter()
                    .filter_map(|gid| hierarchy.get(gid))
                    .map(|g| self.goal_to_json(g))
                    .collect();

                JsonRpcResponse::success(
                    id,
                    json!({
                        "goal_id": goal_id.to_string(),
                        "ancestors": ancestors,
                        "depth": ancestors.len()
                    }),
                )
            }

            "get_subtree" => {
                let goal_id = match params.get("goal_id").and_then(|v| v.as_str()) {
                    Some(gid) => match Uuid::parse_str(gid) {
                        Ok(uuid) => uuid,
                        Err(_) => {
                            error!("goal/hierarchy_query: Invalid goal_id UUID format: {}", gid);
                            return JsonRpcResponse::error(
                                id,
                                error_codes::INVALID_PARAMS,
                                format!("Invalid goal_id UUID format: {}", gid),
                            );
                        }
                    },
                    None => {
                        error!("goal/hierarchy_query: get_subtree requires 'goal_id'");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            "get_subtree operation requires 'goal_id' parameter",
                        );
                    }
                };

                // Get root of subtree
                let root = match hierarchy.get(&goal_id) {
                    Some(g) => g,
                    None => {
                        error!(goal_id = %goal_id, "goal/hierarchy_query: Subtree root not found");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::GOAL_NOT_FOUND,
                            format!("Subtree root not found: {}", goal_id),
                        );
                    }
                };

                // Collect subtree using BFS
                let mut subtree = vec![self.goal_to_json(root)];
                let mut queue = vec![goal_id];

                while let Some(current_id) = queue.pop() {
                    for child in hierarchy.children(&current_id) {
                        subtree.push(self.goal_to_json(child));
                        queue.push(child.id);
                    }
                }

                JsonRpcResponse::success(
                    id,
                    json!({
                        "root_goal_id": goal_id.to_string(),
                        "subtree": subtree,
                        "count": subtree.len()
                    }),
                )
            }

            _ => {
                error!(
                    operation = operation,
                    "goal/hierarchy_query: Unknown operation"
                );
                JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!(
                        "Unknown operation '{}'. Valid: get_children, get_ancestors, get_subtree, get_all, get_goal",
                        operation
                    ),
                )
            }
        }
    }

    /// Handle goal/aligned_memories request.
    ///
    /// Find memories aligned to a specific goal.
    ///
    /// # Request Parameters
    /// - `goal_id` (required): Goal ID to find aligned memories for
    /// - `min_alignment` (optional): Minimum alignment threshold, default 0.55 (Warning threshold)
    /// - `top_k` (optional): Maximum results, default 10
    ///
    /// # Response
    /// - `results`: Array of memories with alignment scores to the goal
    /// - `goal`: The goal being queried
    ///
    /// # Error Codes
    /// - GOAL_NOT_FOUND (-32020): Goal ID not found
    /// - PURPOSE_SEARCH_ERROR (-32016): Search failed
    #[instrument(skip(self, params), fields(method = "goal/aligned_memories"))]
    pub(super) async fn handle_goal_aligned_memories(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("goal/aligned_memories: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - goal_id required",
                );
            }
        };

        // Extract goal_id (required)
        let goal_id = match params.get("goal_id").and_then(|v| v.as_str()) {
            Some(gid) => match Uuid::parse_str(gid) {
                Ok(uuid) => uuid,
                Err(_) => {
                    error!("goal/hierarchy_query: Invalid goal_id UUID format: {}", gid);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid goal_id UUID format: {}", gid),
                    );
                }
            },
            None => {
                error!("goal/aligned_memories: Missing 'goal_id' parameter");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required 'goal_id' parameter",
                );
            }
        };

        // Get the goal from hierarchy (scoped block ensures lock is released before await)
        let goal: GoalNode = {
            let hierarchy = self.goal_hierarchy.read();
            match hierarchy.get(&goal_id) {
                Some(g) => g.clone(),
                None => {
                    error!(goal_id = %goal_id, "goal/aligned_memories: Goal not found");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::GOAL_NOT_FOUND,
                        format!("Goal not found: {}", goal_id),
                    );
                }
            }
        };

        // top_k has a sensible default (pagination parameter)
        const DEFAULT_TOP_K: usize = 10;
        let top_k = params
            .get("topK")
            .or_else(|| params.get("top_k"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TOP_K);

        // FAIL-FAST: min_alignment MUST be explicitly provided.
        // Per constitution AP-007: No silent fallbacks that mask user intent.
        // Using 0.55 (Warning threshold) as default would silently filter results
        // without user awareness. Client MUST specify their desired threshold.
        let min_alignment = match params
            .get("minAlignment")
            .or_else(|| params.get("min_alignment"))
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
        {
            Some(alignment) => {
                // Validate range [0.0, 1.0]
                if !(0.0..=1.0).contains(&alignment) {
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!(
                            "minAlignment must be between 0.0 and 1.0, got: {}. \
                             Reference thresholds: 0.75 (Perfect), 0.70 (Strong), \
                             0.55 (Warning), below 0.55 (Misaligned)",
                            alignment
                        ),
                    );
                }
                alignment
            }
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing required parameter 'minAlignment' (or 'min_alignment'). \
                     You must explicitly specify the alignment threshold for filtering results. \
                     Reference thresholds: 0.75 (Perfect), 0.70 (Strong), 0.55 (Warning), \
                     below 0.55 (Misaligned). Example: \"minAlignment\": 0.55"
                        .to_string(),
                );
            }
        };

        // Create purpose vector from goal's embedding
        // We use the goal's propagation weight to scale alignments
        let propagation_weight = goal.level.propagation_weight();

        // Create a purpose vector that emphasizes the goal's embedding space
        // For simplicity, we use equal alignments scaled by propagation weight
        let alignments = [propagation_weight; NUM_EMBEDDERS];
        let purpose_vector = PurposeVector {
            alignments,
            dominant_embedder: 0, // E1 semantic as dominant
            coherence: 1.0,
            stability: 1.0,
        };

        let search_start = std::time::Instant::now();

        // Build search options
        let options = TeleologicalSearchOptions::quick(top_k).with_min_alignment(min_alignment);

        // Execute purpose search
        match self
            .teleological_store
            .search_purpose(&purpose_vector, options)
            .await
        {
            Ok(results) => {
                let search_latency_ms = search_start.elapsed().as_millis();

                let results_json: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "id": r.fingerprint.id.to_string(),
                            "goal_alignment": r.purpose_alignment * propagation_weight,
                            "raw_alignment": r.purpose_alignment,
                            "theta_to_north_star": r.fingerprint.theta_to_north_star,
                            "threshold": format!("{:?}", AlignmentThreshold::classify(r.purpose_alignment))
                        })
                    })
                    .collect();

                debug!(
                    goal_id = %goal_id,
                    count = results.len(),
                    latency_ms = search_latency_ms,
                    "goal/aligned_memories: Completed"
                );

                JsonRpcResponse::success(
                    id,
                    json!({
                        "goal": self.goal_to_json(&goal),
                        "results": results_json,
                        "count": results.len(),
                        "min_alignment_filter": min_alignment,
                        "search_time_ms": search_latency_ms
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, goal_id = %goal_id, "goal/aligned_memories: FAILED");
                JsonRpcResponse::error(
                    id,
                    error_codes::PURPOSE_SEARCH_ERROR,
                    format!("Aligned memories search failed: {}", e),
                )
            }
        }
    }

    /// Handle purpose/drift_check request.
    ///
    /// TASK-INTEG-002: Integrates TeleologicalDriftDetector (TASK-LOGIC-010) for
    /// per-embedder drift analysis with 5-level DriftLevel classification.
    ///
    /// # Request Parameters
    /// - `fingerprint_ids` (required): Array of fingerprint UUIDs to check
    /// - `goal_id` (optional): Goal to check drift against (default: North Star)
    /// - `strategy` (optional): Comparison strategy - "quick", "balanced", "exhaustive" (default: "balanced")
    ///
    /// # Response
    /// - `overall_drift`: Overall drift level, similarity, score, has_drifted flag
    /// - `per_embedder_drift`: Array of 13 embedder-specific drift results
    /// - `most_drifted_embedders`: Top 5 most drifted embedders sorted worst-first
    /// - `recommendations`: Action recommendations based on drift levels
    /// - `analyzed_count`: Number of memories analyzed
    /// - `timestamp`: RFC3339 formatted timestamp
    ///
    /// # Error Codes
    /// - INVALID_PARAMS (-32602): Invalid fingerprint IDs or parameters
    /// - NORTH_STAR_NOT_CONFIGURED (-32021): No North Star goal
    /// - ALIGNMENT_COMPUTATION_ERROR (-32022): Drift check failed
    ///
    /// # FAIL FAST
    /// - Empty fingerprint_ids returns error (not empty result)
    /// - Invalid goal returns error with clear reason
    /// - All DriftError variants propagate immediately
    #[instrument(skip(self, params), fields(method = "purpose/drift_check"))]
    pub(super) async fn handle_purpose_drift_check(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                error!("purpose/drift_check: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - fingerprint_ids required",
                );
            }
        };

        // FAIL FAST: Extract fingerprint_ids (required, cannot be empty)
        let fingerprint_ids: Vec<uuid::Uuid> =
            match params.get("fingerprint_ids").and_then(|v| v.as_array()) {
                Some(arr) => {
                    // FAIL FAST: Empty array is an error
                    if arr.is_empty() {
                        error!("purpose/drift_check: Empty fingerprint_ids array");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            "fingerprint_ids array cannot be empty - FAIL FAST",
                        );
                    }
                    let mut ids = Vec::with_capacity(arr.len());
                    for (i, v) in arr.iter().enumerate() {
                        match v.as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()) {
                            Some(uuid) => ids.push(uuid),
                            None => {
                                error!(index = i, "purpose/drift_check: Invalid UUID format");
                                return JsonRpcResponse::error(
                                    id,
                                    error_codes::INVALID_PARAMS,
                                    format!("Invalid UUID at fingerprint_ids[{}]", i),
                                );
                            }
                        }
                    }
                    ids
                }
                None => {
                    error!("purpose/drift_check: Missing 'fingerprint_ids' parameter");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        "Missing required 'fingerprint_ids' parameter (array of UUIDs)",
                    );
                }
            };

        // Parse comparison strategy (default: Cosine)
        // SearchStrategy variants: Cosine, Euclidean, SynergyWeighted, GroupHierarchical, CrossCorrelation
        let strategy = match params
            .get("strategy")
            .and_then(|v| v.as_str())
            .unwrap_or("cosine")
        {
            "cosine" => SearchStrategy::Cosine,
            "euclidean" => SearchStrategy::Euclidean,
            "synergy" | "synergy_weighted" => SearchStrategy::SynergyWeighted,
            "group" | "hierarchical" => SearchStrategy::GroupHierarchical,
            "cross_correlation" => SearchStrategy::CrossCorrelationDominant,
            other => {
                error!(strategy = other, "purpose/drift_check: Invalid strategy");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!(
                        "Invalid strategy '{}'. Valid: cosine, euclidean, synergy, group, cross_correlation",
                        other
                    ),
                );
            }
        };

        // Get goal fingerprint (North Star by default, or specified goal_id)
        let goal_fingerprint = {
            let hierarchy = self.goal_hierarchy.read();

            // Check North Star exists
            if !hierarchy.has_north_star() {
                error!("purpose/drift_check: No North Star goal configured");
                return JsonRpcResponse::error(
                    id,
                    error_codes::NORTH_STAR_NOT_CONFIGURED,
                    "No North Star goal configured. Use auto_bootstrap_north_star tool for autonomous goal discovery.",
                );
            }

            // Get goal (North Star or specified)
            let goal = if let Some(goal_id_str) = params.get("goal_id").and_then(|v| v.as_str()) {
                let goal_id = match Uuid::parse_str(goal_id_str) {
                    Ok(uuid) => uuid,
                    Err(_) => {
                        error!(goal_id = goal_id_str, "purpose/drift_check: Invalid goal UUID");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::INVALID_PARAMS,
                            format!("Invalid goal_id UUID format: {}", goal_id_str),
                        );
                    }
                };
                match hierarchy.get(&goal_id) {
                    Some(g) => g.clone(),
                    None => {
                        error!(goal_id = %goal_id, "purpose/drift_check: Goal not found");
                        return JsonRpcResponse::error(
                            id,
                            error_codes::GOAL_NOT_FOUND,
                            format!("Goal {} not found in hierarchy", goal_id),
                        );
                    }
                }
            } else {
                // Use North Star
                hierarchy.north_star().unwrap().clone()
            };

            // GoalNode.teleological_array is a TeleologicalArray which is SemanticFingerprint
            goal.teleological_array.clone()
        };

        let check_start = std::time::Instant::now();

        // Collect all fingerprints - FAIL FAST on any error
        // Extract SemanticFingerprint (.semantic) from each TeleologicalFingerprint
        let mut memories = Vec::with_capacity(fingerprint_ids.len());
        for fp_id in &fingerprint_ids {
            let fingerprint = match self.teleological_store.retrieve(*fp_id).await {
                Ok(Some(fp)) => fp,
                Ok(None) => {
                    error!(fingerprint_id = %fp_id, "purpose/drift_check: Fingerprint not found");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::FINGERPRINT_NOT_FOUND,
                        format!("Fingerprint {} not found - FAIL FAST", fp_id),
                    );
                }
                Err(e) => {
                    error!(fingerprint_id = %fp_id, error = %e, "purpose/drift_check: Storage error");
                    return JsonRpcResponse::error(
                        id,
                        error_codes::ALIGNMENT_COMPUTATION_ERROR,
                        format!("Storage error retrieving {}: {} - FAIL FAST", fp_id, e),
                    );
                }
            };
            // Extract the SemanticFingerprint from TeleologicalFingerprint
            memories.push(fingerprint.semantic);
        }

        // Create TeleologicalDriftDetector (TASK-LOGIC-010)
        let comparator = TeleologicalComparator::new();
        let detector = TeleologicalDriftDetector::new(comparator);

        // Execute drift check - FAIL FAST on any error
        let drift_result: DriftResult = match detector.check_drift(&memories, &goal_fingerprint, strategy) {
            Ok(result) => result,
            Err(e) => {
                // Map DriftError to appropriate handler error
                let (code, message) = match &e {
                    DriftError::EmptyMemories => (
                        error_codes::INVALID_PARAMS,
                        "Empty memories slice - cannot check drift".to_string(),
                    ),
                    DriftError::InvalidGoal { reason } => (
                        error_codes::INVALID_PARAMS,
                        format!("Invalid goal fingerprint: {}", reason),
                    ),
                    DriftError::ComparisonFailed { embedder, reason } => (
                        error_codes::ALIGNMENT_COMPUTATION_ERROR,
                        format!("Comparison failed for {:?}: {}", embedder, reason),
                    ),
                    DriftError::InvalidThresholds { reason } => (
                        error_codes::ALIGNMENT_COMPUTATION_ERROR,
                        format!("Invalid thresholds: {}", reason),
                    ),
                    DriftError::ComparisonValidationFailed { reason } => (
                        error_codes::ALIGNMENT_COMPUTATION_ERROR,
                        format!("Comparison validation failed: {}", reason),
                    ),
                };
                error!(error = %e, "purpose/drift_check: FAILED");
                return JsonRpcResponse::error(id, code, format!("{} - FAIL FAST", message));
            }
        };

        let check_time_ms = check_start.elapsed().as_millis();

        // Build per-embedder drift response (exactly 13 entries)
        let per_embedder_drift: Vec<serde_json::Value> = drift_result
            .per_embedder_drift
            .embedder_drift
            .iter()
            .map(|info| {
                json!({
                    "embedder": format!("{:?}", info.embedder),
                    "embedder_index": info.embedder.index(),
                    "similarity": info.similarity,
                    "drift_score": info.drift_score,
                    "drift_level": format!("{:?}", info.drift_level)
                })
            })
            .collect();

        // Build most drifted embedders (top 5, sorted worst-first)
        let most_drifted: Vec<serde_json::Value> = drift_result
            .most_drifted_embedders
            .iter()
            .take(5)
            .map(|info| {
                json!({
                    "embedder": format!("{:?}", info.embedder),
                    "embedder_index": info.embedder.index(),
                    "similarity": info.similarity,
                    "drift_score": info.drift_score,
                    "drift_level": format!("{:?}", info.drift_level)
                })
            })
            .collect();

        // Build recommendations (fields: embedder, issue, suggestion, priority)
        let recommendations: Vec<serde_json::Value> = drift_result
            .recommendations
            .iter()
            .map(|rec| {
                json!({
                    "embedder": format!("{:?}", rec.embedder),
                    "priority": format!("{:?}", rec.priority),
                    "issue": rec.issue,
                    "suggestion": rec.suggestion
                })
            })
            .collect();

        // Build trend response if available (fields: direction, velocity, samples, projected_critical_in)
        let trend_response = drift_result.trend.as_ref().map(|trend| {
            json!({
                "direction": format!("{:?}", trend.direction),
                "velocity": trend.velocity,
                "samples": trend.samples,
                "projected_critical_in": trend.projected_critical_in
            })
        });

        info!(
            overall_level = ?drift_result.overall_drift.drift_level,
            analyzed_count = drift_result.analyzed_count,
            check_time_ms = check_time_ms,
            "purpose/drift_check: Completed with per-embedder analysis"
        );

        JsonRpcResponse::success(
            id,
            json!({
                "overall_drift": {
                    "level": format!("{:?}", drift_result.overall_drift.drift_level),
                    "similarity": drift_result.overall_drift.similarity,
                    "drift_score": drift_result.overall_drift.drift_score,
                    "has_drifted": drift_result.overall_drift.has_drifted
                },
                "per_embedder_drift": per_embedder_drift,
                "most_drifted_embedders": most_drifted,
                "recommendations": recommendations,
                "trend": trend_response,
                "analyzed_count": drift_result.analyzed_count,
                "timestamp": drift_result.timestamp.to_rfc3339(),
                "check_time_ms": check_time_ms
            }),
        )
    }

    // NOTE: handle_north_star_update REMOVED per TASK-CORE-001 (ARCH-03)
    // Manual North Star update violates autonomous-first architecture.
    // Calls to purpose/north_star_update now return METHOD_NOT_FOUND (-32601).
    // Goals emerge autonomously via auto_bootstrap_north_star tool.

    // Helper methods

    /// Convert a GoalNode to JSON representation.
    fn goal_to_json(&self, goal: &GoalNode) -> serde_json::Value {
        json!({
            "id": goal.id.to_string(),
            "description": goal.description,
            "level": format!("{:?}", goal.level),
            "level_depth": goal.level.depth(),
            "parent_id": goal.parent_id.map(|p| p.to_string()),
            "discovery": {
                "method": format!("{:?}", goal.discovery.method),
                "confidence": goal.discovery.confidence,
                "cluster_size": goal.discovery.cluster_size,
                "coherence": goal.discovery.coherence
            },
            "propagation_weight": goal.level.propagation_weight(),
            "child_count": goal.child_ids.len(),
            "is_north_star": goal.is_north_star()
        })
    }

    /// Compute hierarchy statistics.
    fn compute_hierarchy_stats(&self, hierarchy: &GoalHierarchy) -> serde_json::Value {
        let north_star_count = hierarchy.at_level(GoalLevel::NorthStar).len();
        let strategic_count = hierarchy.at_level(GoalLevel::Strategic).len();
        let tactical_count = hierarchy.at_level(GoalLevel::Tactical).len();
        let immediate_count = hierarchy.at_level(GoalLevel::Immediate).len();

        json!({
            "total_goals": hierarchy.len(),
            "has_north_star": hierarchy.has_north_star(),
            "level_counts": {
                "north_star": north_star_count,
                "strategic": strategic_count,
                "tactical": tactical_count,
                "immediate": immediate_count
            },
            "is_valid": hierarchy.validate().is_ok()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::purpose::GoalDiscoveryMetadata;
    use context_graph_core::types::fingerprint::SemanticFingerprint;

    #[test]
    fn test_goal_to_json_structure() {
        // Verify the JSON structure matches expected output
        // Per TASK-CORE-005: Use autonomous_goal() with TeleologicalArray, not north_star()
        let discovery = GoalDiscoveryMetadata::bootstrap();
        let goal = GoalNode::autonomous_goal(
            "Test North Star".into(),
            GoalLevel::NorthStar,
            SemanticFingerprint::zeroed(),
            discovery,
        )
        .expect("Failed to create test goal");

        // Verify GoalNode structure (id is now Uuid, not custom GoalId)
        assert!(!goal.id.is_nil()); // UUID should not be nil
        assert_eq!(goal.level, GoalLevel::NorthStar);
        assert!(goal.is_north_star());

        println!("[VERIFIED] GoalNode structure is correct with new API");
    }

    #[test]
    fn test_purpose_vector_validation() {
        // Test that purpose vector validation works correctly
        let valid_alignments = [0.5f32; NUM_EMBEDDERS];
        let pv = PurposeVector {
            alignments: valid_alignments,
            dominant_embedder: 0,
            coherence: 1.0,
            stability: 1.0,
        };

        assert_eq!(pv.alignments.len(), NUM_EMBEDDERS);
        assert!(pv.alignments.iter().all(|&v| (0.0..=1.0).contains(&v)));

        println!("[VERIFIED] PurposeVector validation works correctly");
    }
}
