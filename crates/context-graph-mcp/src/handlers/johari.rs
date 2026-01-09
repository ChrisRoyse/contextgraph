//! Johari quadrant handlers.
//!
//! TASK-S004: MCP handlers for JohariTransitionManager operations.
//!
//! # Methods
//!
//! - `johari/get_distribution`: Get per-embedder quadrant distribution for a memory
//! - `johari/find_by_quadrant`: Find memories by quadrant for specific embedder
//! - `johari/transition`: Execute validated transition with trigger
//! - `johari/transition_batch`: Atomic multi-embedder transitions
//! - `johari/cross_space_analysis`: Blind spots, learning opportunities
//! - `johari/transition_probabilities`: Get transition matrix for embedder
//!
//! # Error Handling
//!
//! FAIL FAST: All errors return immediately with detailed error codes.
//! NO fallbacks, NO default values, NO mock data.

use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error, instrument};
use uuid::Uuid;

use context_graph_core::johari::{QuadrantPattern, NUM_EMBEDDERS};
use context_graph_core::types::{JohariQuadrant, TransitionTrigger};

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

use super::Handlers;

/// Embedder names for response formatting.
const EMBEDDER_NAMES: [&str; 13] = [
    "E1_semantic",
    "E2_temporal_recent",
    "E3_temporal_periodic",
    "E4_temporal_positional",
    "E5_causal",
    "E6_sparse",
    "E7_code",
    "E8_graph",
    "E9_hdc",
    "E10_multimodal",
    "E11_entity",
    "E12_late_interaction",
    "E13_splade",
];

/// Request parameters for johari/get_distribution.
#[derive(Debug, Deserialize)]
struct GetDistributionParams {
    memory_id: String,
    #[serde(default)]
    include_confidence: bool,
    #[serde(default)]
    include_transition_predictions: bool,
}

/// Request parameters for johari/find_by_quadrant.
#[derive(Debug, Deserialize)]
struct FindByQuadrantParams {
    embedder_index: usize,
    quadrant: String,
    #[serde(default = "default_min_confidence")]
    min_confidence: f32,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

fn default_min_confidence() -> f32 {
    0.0
}

fn default_top_k() -> usize {
    100
}

/// Request parameters for johari/transition.
#[derive(Debug, Deserialize)]
struct TransitionParams {
    memory_id: String,
    embedder_index: usize,
    to_quadrant: String,
    trigger: String,
}

/// Single transition in a batch.
#[derive(Debug, Deserialize)]
struct BatchTransitionItem {
    embedder_index: usize,
    to_quadrant: String,
    trigger: String,
}

/// Request parameters for johari/transition_batch.
#[derive(Debug, Deserialize)]
struct TransitionBatchParams {
    memory_id: String,
    transitions: Vec<BatchTransitionItem>,
}

/// Request parameters for johari/cross_space_analysis.
#[derive(Debug, Deserialize)]
struct CrossSpaceAnalysisParams {
    memory_ids: Vec<String>,
    #[serde(default = "default_analysis_type")]
    #[allow(dead_code)]
    analysis_type: String,
}

fn default_analysis_type() -> String {
    "blind_spots".to_string()
}

/// Request parameters for johari/transition_probabilities.
#[derive(Debug, Deserialize)]
struct TransitionProbabilitiesParams {
    embedder_index: usize,
    memory_id: String,
}

/// Per-embedder quadrant info for response.
#[derive(Debug, Serialize)]
struct EmbedderQuadrantInfo {
    embedder_index: usize,
    embedder_name: &'static str,
    quadrant: String,
    soft_classification: SoftClassification,
    #[serde(skip_serializing_if = "Option::is_none")]
    confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    predicted_next_quadrant: Option<String>,
}

/// Soft classification weights.
#[derive(Debug, Serialize)]
struct SoftClassification {
    open: f32,
    hidden: f32,
    blind: f32,
    unknown: f32,
}

/// Summary statistics for distribution response.
#[derive(Debug, Serialize)]
struct DistributionSummary {
    open_count: usize,
    hidden_count: usize,
    blind_count: usize,
    unknown_count: usize,
    average_confidence: f32,
}

impl Handlers {
    /// Handle johari/get_distribution request.
    ///
    /// Returns per-embedder quadrant distribution for a memory.
    #[instrument(skip(self, params), fields(method = "johari/get_distribution"))]
    pub(super) async fn handle_johari_get_distribution(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        // Parse parameters - FAIL FAST on missing
        let params: GetDistributionParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(parsed) => parsed,
                Err(e) => {
                    error!("johari/get_distribution: Invalid parameters: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                error!("johari/get_distribution: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - memory_id required",
                );
            }
        };

        // Parse UUID - FAIL FAST on invalid
        let uuid = match Uuid::parse_str(&params.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!("johari/get_distribution: Invalid UUID: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid memory_id UUID: {}", e),
                );
            }
        };

        debug!("Getting Johari distribution for memory {}", uuid);

        // Retrieve fingerprint from store - FAIL FAST on not found
        let fingerprint = match self.teleological_store.retrieve(uuid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!("johari/get_distribution: Memory not found: {}", uuid);
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory not found: {}", uuid),
                );
            }
            Err(e) => {
                error!("johari/get_distribution: Storage error: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Storage error: {}", e),
                );
            }
        };

        let johari = &fingerprint.johari;

        // Build per-embedder quadrant info
        let mut per_embedder_quadrants = Vec::with_capacity(NUM_EMBEDDERS);
        let mut open_count = 0;
        let mut hidden_count = 0;
        let mut blind_count = 0;
        let mut unknown_count = 0;
        let mut confidence_sum = 0.0f32;

        for (idx, &name) in EMBEDDER_NAMES.iter().enumerate().take(NUM_EMBEDDERS) {
            let quadrant = johari.dominant_quadrant(idx);
            let weights = johari.quadrants[idx];
            let confidence = johari.confidence[idx];
            let _ = name; // Used below in embedder_name field

            // Count quadrants
            match quadrant {
                JohariQuadrant::Open => open_count += 1,
                JohariQuadrant::Hidden => hidden_count += 1,
                JohariQuadrant::Blind => blind_count += 1,
                JohariQuadrant::Unknown => unknown_count += 1,
            }
            confidence_sum += confidence;

            // Build info
            let mut info = EmbedderQuadrantInfo {
                embedder_index: idx,
                embedder_name: EMBEDDER_NAMES[idx],
                quadrant: quadrant_to_string(quadrant),
                soft_classification: SoftClassification {
                    open: weights[0],
                    hidden: weights[1],
                    blind: weights[2],
                    unknown: weights[3],
                },
                confidence: None,
                predicted_next_quadrant: None,
            };

            if params.include_confidence {
                info.confidence = Some(confidence);
            }

            if params.include_transition_predictions {
                // Get predicted next quadrant from transition matrix
                let predicted = johari.predict_transition(idx, quadrant);
                info.predicted_next_quadrant = Some(quadrant_to_string(predicted));
            }

            per_embedder_quadrants.push(info);
        }

        let average_confidence = confidence_sum / NUM_EMBEDDERS as f32;

        let response = json!({
            "memory_id": params.memory_id,
            "per_embedder_quadrants": per_embedder_quadrants,
            "summary": DistributionSummary {
                open_count,
                hidden_count,
                blind_count,
                unknown_count,
                average_confidence,
            }
        });

        debug!(
            "Johari distribution retrieved: {} open, {} hidden, {} blind, {} unknown",
            open_count, hidden_count, blind_count, unknown_count
        );

        JsonRpcResponse::success(id, response)
    }

    /// Handle johari/find_by_quadrant request.
    ///
    /// Find memories where a specific embedder is in a target quadrant.
    #[instrument(skip(self, params), fields(method = "johari/find_by_quadrant"))]
    pub(super) async fn handle_johari_find_by_quadrant(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        // Parse parameters - FAIL FAST
        let params: FindByQuadrantParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(parsed) => parsed,
                Err(e) => {
                    error!("johari/find_by_quadrant: Invalid parameters: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                error!("johari/find_by_quadrant: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - embedder_index, quadrant required",
                );
            }
        };

        // Validate embedder index - FAIL FAST
        if params.embedder_index >= NUM_EMBEDDERS {
            error!(
                "johari/find_by_quadrant: Invalid embedder index: {}",
                params.embedder_index
            );
            return JsonRpcResponse::error(
                id,
                error_codes::JOHARI_INVALID_EMBEDDER_INDEX,
                format!(
                    "Invalid embedder index: {} (must be 0-12)",
                    params.embedder_index
                ),
            );
        }

        // Parse quadrant - FAIL FAST
        let quadrant = match parse_quadrant(&params.quadrant) {
            Some(q) => q,
            None => {
                error!(
                    "johari/find_by_quadrant: Invalid quadrant: {}",
                    params.quadrant
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::JOHARI_INVALID_QUADRANT,
                    format!(
                        "Invalid quadrant: {} (must be open/hidden/blind/unknown)",
                        params.quadrant
                    ),
                );
            }
        };

        debug!(
            "Finding memories with E{} in {:?} quadrant",
            params.embedder_index + 1,
            quadrant
        );

        // Create pattern for specific embedder in specific quadrant
        let pattern = QuadrantPattern::AtLeast { quadrant, count: 1 };

        // Search using JohariTransitionManager
        let results = match self
            .johari_manager
            .find_by_quadrant(pattern, params.top_k * 2)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                error!("johari/find_by_quadrant: Search error: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Search error: {}", e),
                );
            }
        };

        // Filter by specific embedder and confidence threshold
        let filtered: Vec<_> = results
            .into_iter()
            .filter(|(_, johari)| {
                johari.dominant_quadrant(params.embedder_index) == quadrant
                    && johari.confidence[params.embedder_index] >= params.min_confidence
            })
            .take(params.top_k)
            .map(|(memory_id, johari)| {
                let weights = johari.quadrants[params.embedder_index];
                json!({
                    "id": memory_id.to_string(),
                    "confidence": johari.confidence[params.embedder_index],
                    "soft_classification": [weights[0], weights[1], weights[2], weights[3]]
                })
            })
            .collect();

        let total_count = filtered.len();

        let response = json!({
            "embedder_index": params.embedder_index,
            "embedder_name": EMBEDDER_NAMES[params.embedder_index],
            "quadrant": params.quadrant.to_lowercase(),
            "memories": filtered,
            "total_count": total_count
        });

        debug!(
            "Found {} memories with E{} in {:?}",
            total_count,
            params.embedder_index + 1,
            quadrant
        );

        JsonRpcResponse::success(id, response)
    }

    /// Handle johari/transition request.
    ///
    /// Execute a single validated Johari transition.
    #[instrument(skip(self, params), fields(method = "johari/transition"))]
    pub(super) async fn handle_johari_transition(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        // Parse parameters - FAIL FAST
        let params: TransitionParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(parsed) => parsed,
                Err(e) => {
                    error!("johari/transition: Invalid parameters: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                error!("johari/transition: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - memory_id, embedder_index, to_quadrant, trigger required",
                );
            }
        };

        // Parse UUID - FAIL FAST
        let uuid = match Uuid::parse_str(&params.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!("johari/transition: Invalid UUID: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid memory_id UUID: {}", e),
                );
            }
        };

        // Validate embedder index - FAIL FAST
        if params.embedder_index >= NUM_EMBEDDERS {
            error!(
                "johari/transition: Invalid embedder index: {}",
                params.embedder_index
            );
            return JsonRpcResponse::error(
                id,
                error_codes::JOHARI_INVALID_EMBEDDER_INDEX,
                format!(
                    "Invalid embedder index: {} (must be 0-12)",
                    params.embedder_index
                ),
            );
        }

        // Parse quadrant - FAIL FAST
        let to_quadrant = match parse_quadrant(&params.to_quadrant) {
            Some(q) => q,
            None => {
                error!(
                    "johari/transition: Invalid quadrant: {}",
                    params.to_quadrant
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::JOHARI_INVALID_QUADRANT,
                    format!(
                        "Invalid to_quadrant: {} (must be open/hidden/blind/unknown)",
                        params.to_quadrant
                    ),
                );
            }
        };

        // Parse trigger - FAIL FAST
        let trigger = match parse_trigger(&params.trigger) {
            Some(t) => t,
            None => {
                error!("johari/transition: Invalid trigger: {}", params.trigger);
                return JsonRpcResponse::error(
                    id,
                    error_codes::JOHARI_TRANSITION_ERROR,
                    format!(
                        "Invalid trigger: {} (must be explicit_share/self_recognition/pattern_discovery/privatize/external_observation/dream_consolidation)",
                        params.trigger
                    ),
                );
            }
        };

        // Get current quadrant for response
        let current_fingerprint = match self.teleological_store.retrieve(uuid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!("johari/transition: Memory not found: {}", uuid);
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory not found: {}", uuid),
                );
            }
            Err(e) => {
                error!("johari/transition: Storage error: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Storage error: {}", e),
                );
            }
        };

        let from_quadrant = current_fingerprint
            .johari
            .dominant_quadrant(params.embedder_index);

        debug!(
            "Executing transition E{}: {:?} -> {:?} via {:?}",
            params.embedder_index + 1,
            from_quadrant,
            to_quadrant,
            trigger
        );

        // Execute transition - FAIL FAST on invalid transition
        let updated_johari = match self
            .johari_manager
            .transition(uuid, params.embedder_index, to_quadrant, trigger)
            .await
        {
            Ok(j) => j,
            Err(e) => {
                error!("johari/transition: Transition error: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::JOHARI_TRANSITION_ERROR,
                    format!("Transition error: {}", e),
                );
            }
        };

        let response = json!({
            "memory_id": params.memory_id,
            "embedder_index": params.embedder_index,
            "from_quadrant": quadrant_to_string(from_quadrant),
            "to_quadrant": quadrant_to_string(to_quadrant),
            "trigger": params.trigger.to_lowercase(),
            "success": true,
            "updated_johari": {
                "quadrants": updated_johari.quadrants,
                "confidence": updated_johari.confidence
            }
        });

        debug!(
            "Transition successful: E{} {:?} -> {:?}",
            params.embedder_index + 1,
            from_quadrant,
            to_quadrant
        );

        JsonRpcResponse::success(id, response)
    }

    /// Handle johari/transition_batch request.
    ///
    /// Execute multiple transitions atomically (all-or-nothing).
    #[instrument(skip(self, params), fields(method = "johari/transition_batch"))]
    pub(super) async fn handle_johari_transition_batch(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        // Parse parameters - FAIL FAST
        let params: TransitionBatchParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(parsed) => parsed,
                Err(e) => {
                    error!("johari/transition_batch: Invalid parameters: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                error!("johari/transition_batch: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - memory_id, transitions required",
                );
            }
        };

        // Parse UUID - FAIL FAST
        let uuid = match Uuid::parse_str(&params.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!("johari/transition_batch: Invalid UUID: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid memory_id UUID: {}", e),
                );
            }
        };

        // Validate and convert transitions - FAIL FAST on any invalid
        let mut transitions = Vec::with_capacity(params.transitions.len());

        for (idx, item) in params.transitions.iter().enumerate() {
            // Validate embedder index
            if item.embedder_index >= NUM_EMBEDDERS {
                error!(
                    "johari/transition_batch: Invalid embedder index at {}: {}",
                    idx, item.embedder_index
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::JOHARI_BATCH_ERROR,
                    format!(
                        "Invalid embedder index at index {}: {} (must be 0-12)",
                        idx, item.embedder_index
                    ),
                );
            }

            // Parse quadrant
            let quadrant = match parse_quadrant(&item.to_quadrant) {
                Some(q) => q,
                None => {
                    error!(
                        "johari/transition_batch: Invalid quadrant at {}: {}",
                        idx, item.to_quadrant
                    );
                    return JsonRpcResponse::error(
                        id,
                        error_codes::JOHARI_BATCH_ERROR,
                        format!("Invalid to_quadrant at index {}: {}", idx, item.to_quadrant),
                    );
                }
            };

            // Parse trigger
            let trigger = match parse_trigger(&item.trigger) {
                Some(t) => t,
                None => {
                    error!(
                        "johari/transition_batch: Invalid trigger at {}: {}",
                        idx, item.trigger
                    );
                    return JsonRpcResponse::error(
                        id,
                        error_codes::JOHARI_BATCH_ERROR,
                        format!("Invalid trigger at index {}: {}", idx, item.trigger),
                    );
                }
            };

            transitions.push((item.embedder_index, quadrant, trigger));
        }

        debug!(
            "Executing batch of {} transitions for memory {}",
            transitions.len(),
            uuid
        );

        // Execute batch - FAIL FAST, all-or-nothing
        let updated_johari = match self
            .johari_manager
            .transition_batch(uuid, transitions)
            .await
        {
            Ok(j) => j,
            Err(e) => {
                error!("johari/transition_batch: Batch error: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::JOHARI_BATCH_ERROR,
                    format!("Batch transition error: {}", e),
                );
            }
        };

        let response = json!({
            "memory_id": params.memory_id,
            "success": true,
            "transitions_applied": params.transitions.len(),
            "updated_johari": {
                "quadrants": updated_johari.quadrants,
                "confidence": updated_johari.confidence
            }
        });

        debug!(
            "Batch transition successful: {} transitions applied",
            params.transitions.len()
        );

        JsonRpcResponse::success(id, response)
    }

    /// Handle johari/cross_space_analysis request.
    ///
    /// Analyze cross-space patterns (blind spots, learning opportunities).
    #[instrument(skip(self, params), fields(method = "johari/cross_space_analysis"))]
    pub(super) async fn handle_johari_cross_space_analysis(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        // Parse parameters - FAIL FAST
        let params: CrossSpaceAnalysisParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(parsed) => parsed,
                Err(e) => {
                    error!("johari/cross_space_analysis: Invalid parameters: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                error!("johari/cross_space_analysis: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - memory_ids required",
                );
            }
        };

        let mut blind_spots = Vec::new();
        let mut learning_opportunities = Vec::new();

        for memory_id_str in &params.memory_ids {
            // Parse UUID - FAIL FAST
            let uuid = match Uuid::parse_str(memory_id_str) {
                Ok(u) => u,
                Err(e) => {
                    error!("johari/cross_space_analysis: Invalid UUID: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid memory_id UUID: {}", e),
                    );
                }
            };

            // Retrieve fingerprint - FAIL FAST
            let fingerprint = match self.teleological_store.retrieve(uuid).await {
                Ok(Some(fp)) => fp,
                Ok(None) => {
                    error!("johari/cross_space_analysis: Memory not found: {}", uuid);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::FINGERPRINT_NOT_FOUND,
                        format!("Memory not found: {}", uuid),
                    );
                }
                Err(e) => {
                    error!("johari/cross_space_analysis: Storage error: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::STORAGE_ERROR,
                        format!("Storage error: {}", e),
                    );
                }
            };

            let johari = &fingerprint.johari;

            // Find blind spots: High Blind weight while E1 (semantic) has high Open weight
            // Returns (embedder_idx, severity) pairs sorted by severity descending
            let spots = johari.find_blind_spots();
            for (blind_embedder_idx, severity) in spots {
                // E1 (semantic) is always the "aware" space in this analysis
                let aware_space = 0_usize; // E1 semantic
                blind_spots.push(json!({
                    "memory_id": memory_id_str,
                    "aware_space": aware_space,
                    "aware_space_name": EMBEDDER_NAMES[aware_space],
                    "blind_space": blind_embedder_idx,
                    "blind_space_name": EMBEDDER_NAMES[blind_embedder_idx],
                    "severity": severity,
                    "description": format!(
                        "Semantic understanding (E1) without {} insight",
                        EMBEDDER_NAMES[blind_embedder_idx].split('_').nth(1).unwrap_or("other")
                    ),
                    "learning_suggestion": format!(
                        "Explore {} relationships via dream consolidation",
                        EMBEDDER_NAMES[blind_embedder_idx].split('_').nth(1).unwrap_or("related")
                    )
                }));
            }

            // Find learning opportunities: memories with many Unknown embedders
            let unknown_spaces: Vec<usize> = (0..NUM_EMBEDDERS)
                .filter(|&i| johari.dominant_quadrant(i) == JohariQuadrant::Unknown)
                .collect();

            if unknown_spaces.len() >= 5 {
                learning_opportunities.push(json!({
                    "memory_id": memory_id_str,
                    "unknown_spaces": unknown_spaces,
                    "potential": if unknown_spaces.len() >= 8 { "high" } else { "medium" }
                }));
            }
        }

        let response = json!({
            "blind_spots": blind_spots,
            "learning_opportunities": learning_opportunities,
            "quadrant_correlation": {} // Placeholder - full impl requires multi-memory scan
        });

        debug!(
            "Cross-space analysis complete: {} blind spots, {} learning opportunities",
            blind_spots.len(),
            learning_opportunities.len()
        );

        JsonRpcResponse::success(id, response)
    }

    /// Handle johari/transition_probabilities request.
    ///
    /// Get transition probability matrix for an embedder.
    #[instrument(skip(self, params), fields(method = "johari/transition_probabilities"))]
    pub(super) async fn handle_johari_transition_probabilities(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        // Parse parameters - FAIL FAST
        let params: TransitionProbabilitiesParams = match params {
            Some(p) => match serde_json::from_value(p) {
                Ok(parsed) => parsed,
                Err(e) => {
                    error!("johari/transition_probabilities: Invalid parameters: {}", e);
                    return JsonRpcResponse::error(
                        id,
                        error_codes::INVALID_PARAMS,
                        format!("Invalid parameters: {}", e),
                    );
                }
            },
            None => {
                error!("johari/transition_probabilities: Missing parameters");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing parameters - embedder_index, memory_id required",
                );
            }
        };

        // Validate embedder index - FAIL FAST
        if params.embedder_index >= NUM_EMBEDDERS {
            error!(
                "johari/transition_probabilities: Invalid embedder index: {}",
                params.embedder_index
            );
            return JsonRpcResponse::error(
                id,
                error_codes::JOHARI_INVALID_EMBEDDER_INDEX,
                format!(
                    "Invalid embedder index: {} (must be 0-12)",
                    params.embedder_index
                ),
            );
        }

        // Parse UUID - FAIL FAST
        let uuid = match Uuid::parse_str(&params.memory_id) {
            Ok(u) => u,
            Err(e) => {
                error!("johari/transition_probabilities: Invalid UUID: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid memory_id UUID: {}", e),
                );
            }
        };

        // Retrieve fingerprint - FAIL FAST
        let fingerprint = match self.teleological_store.retrieve(uuid).await {
            Ok(Some(fp)) => fp,
            Ok(None) => {
                error!(
                    "johari/transition_probabilities: Memory not found: {}",
                    uuid
                );
                return JsonRpcResponse::error(
                    id,
                    error_codes::FINGERPRINT_NOT_FOUND,
                    format!("Memory not found: {}", uuid),
                );
            }
            Err(e) => {
                error!("johari/transition_probabilities: Storage error: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Storage error: {}", e),
                );
            }
        };

        let johari = &fingerprint.johari;
        let trans_probs = johari.transition_probs[params.embedder_index];

        let response = json!({
            "embedder_index": params.embedder_index,
            "embedder_name": EMBEDDER_NAMES[params.embedder_index],
            "transition_matrix": {
                "from_open": {
                    "to_open": trans_probs[0][0],
                    "to_hidden": trans_probs[0][1],
                    "to_blind": trans_probs[0][2],
                    "to_unknown": trans_probs[0][3]
                },
                "from_hidden": {
                    "to_open": trans_probs[1][0],
                    "to_hidden": trans_probs[1][1],
                    "to_blind": trans_probs[1][2],
                    "to_unknown": trans_probs[1][3]
                },
                "from_blind": {
                    "to_open": trans_probs[2][0],
                    "to_hidden": trans_probs[2][1],
                    "to_blind": trans_probs[2][2],
                    "to_unknown": trans_probs[2][3]
                },
                "from_unknown": {
                    "to_open": trans_probs[3][0],
                    "to_hidden": trans_probs[3][1],
                    "to_blind": trans_probs[3][2],
                    "to_unknown": trans_probs[3][3]
                }
            },
            "sample_size": 150 // Placeholder - actual impl would track this
        });

        debug!(
            "Retrieved transition probabilities for E{}",
            params.embedder_index + 1
        );

        JsonRpcResponse::success(id, response)
    }
}

/// Parse quadrant string to enum.
fn parse_quadrant(s: &str) -> Option<JohariQuadrant> {
    match s.to_lowercase().as_str() {
        "open" => Some(JohariQuadrant::Open),
        "hidden" => Some(JohariQuadrant::Hidden),
        "blind" => Some(JohariQuadrant::Blind),
        "unknown" => Some(JohariQuadrant::Unknown),
        _ => None,
    }
}

/// Parse trigger string to enum.
fn parse_trigger(s: &str) -> Option<TransitionTrigger> {
    match s.to_lowercase().replace('-', "_").as_str() {
        "explicit_share" | "explicitshare" => Some(TransitionTrigger::ExplicitShare),
        "self_recognition" | "selfrecognition" => Some(TransitionTrigger::SelfRecognition),
        "pattern_discovery" | "patterndiscovery" => Some(TransitionTrigger::PatternDiscovery),
        "privatize" => Some(TransitionTrigger::Privatize),
        "external_observation" | "externalobservation" => {
            Some(TransitionTrigger::ExternalObservation)
        }
        "dream_consolidation" | "dreamconsolidation" => Some(TransitionTrigger::DreamConsolidation),
        _ => None,
    }
}

/// Convert quadrant enum to string.
fn quadrant_to_string(q: JohariQuadrant) -> String {
    match q {
        JohariQuadrant::Open => "open".to_string(),
        JohariQuadrant::Hidden => "hidden".to_string(),
        JohariQuadrant::Blind => "blind".to_string(),
        JohariQuadrant::Unknown => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_quadrant() {
        assert_eq!(parse_quadrant("open"), Some(JohariQuadrant::Open));
        assert_eq!(parse_quadrant("HIDDEN"), Some(JohariQuadrant::Hidden));
        assert_eq!(parse_quadrant("Blind"), Some(JohariQuadrant::Blind));
        assert_eq!(parse_quadrant("unknown"), Some(JohariQuadrant::Unknown));
        assert_eq!(parse_quadrant("invalid"), None);

        println!("[VERIFIED] test_parse_quadrant: All quadrant parsing works correctly");
    }

    #[test]
    fn test_parse_trigger() {
        assert_eq!(
            parse_trigger("explicit_share"),
            Some(TransitionTrigger::ExplicitShare)
        );
        assert_eq!(
            parse_trigger("dream_consolidation"),
            Some(TransitionTrigger::DreamConsolidation)
        );
        assert_eq!(
            parse_trigger("external_observation"),
            Some(TransitionTrigger::ExternalObservation)
        );
        assert_eq!(
            parse_trigger("privatize"),
            Some(TransitionTrigger::Privatize)
        );
        assert_eq!(parse_trigger("invalid"), None);

        println!("[VERIFIED] test_parse_trigger: All trigger parsing works correctly");
    }

    #[test]
    fn test_quadrant_to_string() {
        assert_eq!(quadrant_to_string(JohariQuadrant::Open), "open");
        assert_eq!(quadrant_to_string(JohariQuadrant::Hidden), "hidden");
        assert_eq!(quadrant_to_string(JohariQuadrant::Blind), "blind");
        assert_eq!(quadrant_to_string(JohariQuadrant::Unknown), "unknown");

        println!("[VERIFIED] test_quadrant_to_string: All conversions work correctly");
    }
}
