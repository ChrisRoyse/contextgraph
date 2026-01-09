//! MCP tool call handlers.
//!
//! TASK-S001: Updated to use TeleologicalMemoryStore and MultiArrayEmbeddingProvider.
//! NO BACKWARDS COMPATIBILITY with legacy MemoryStore.
//!
//! All tool responses include `_cognitive_pulse` with live UTL metrics.
//! This provides real-time cognitive state in every MCP response.
//!
//! # Constitution Reference
//!
//! Johari quadrant to action mapping (constitution.yaml:159-163):
//! - Open: delta_s < 0.5, delta_c > 0.5 -> DirectRecall
//! - Blind: delta_s > 0.5, delta_c < 0.5 -> TriggerDream
//! - Hidden: delta_s < 0.5, delta_c < 0.5 -> GetNeighborhood
//! - Unknown: delta_s > 0.5, delta_c > 0.5 -> EpistemicAction

use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, error, warn};

use context_graph_core::gwt::state_machine::ConsciousnessState;
use context_graph_core::purpose::{DefaultPurposeComputer, PurposeComputeConfig, PurposeVectorComputer};
use context_graph_core::traits::TeleologicalSearchOptions;
use context_graph_core::types::UtlContext;
use context_graph_core::types::fingerprint::{
    JohariFingerprint, PurposeVector, TeleologicalFingerprint, NUM_EMBEDDERS,
};

use crate::middleware::CognitivePulse;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::Handlers;

impl Handlers {
    /// Handle tools/list request.
    ///
    /// Returns all available MCP tools with their schemas.
    pub(super) async fn handle_tools_list(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling tools/list request");

        let tools = get_tool_definitions();
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    /// Handle tools/call request.
    ///
    /// Dispatches to the appropriate tool handler and returns MCP-compliant result.
    pub(super) async fn handle_tools_call(
        &self,
        id: Option<JsonRpcId>,
        params: Option<serde_json::Value>,
    ) -> JsonRpcResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing params for tools/call",
                );
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'name' parameter in tools/call",
                );
            }
        };

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        debug!(
            "Calling tool: {} with arguments: {:?}",
            tool_name, arguments
        );

        match tool_name {
            tool_names::INJECT_CONTEXT => self.call_inject_context(id, arguments).await,
            tool_names::STORE_MEMORY => self.call_store_memory(id, arguments).await,
            tool_names::GET_MEMETIC_STATUS => self.call_get_memetic_status(id).await,
            tool_names::GET_GRAPH_MANIFEST => self.call_get_graph_manifest(id).await,
            tool_names::SEARCH_GRAPH => self.call_search_graph(id, arguments).await,
            tool_names::UTL_STATUS => self.call_utl_status(id).await,
            tool_names::GET_CONSCIOUSNESS_STATE => self.call_get_consciousness_state(id).await,
            tool_names::GET_KURAMOTO_SYNC => self.call_get_kuramoto_sync(id).await,
            tool_names::GET_WORKSPACE_STATUS => self.call_get_workspace_status(id).await,
            tool_names::GET_EGO_STATE => self.call_get_ego_state(id).await,
            tool_names::TRIGGER_WORKSPACE_BROADCAST => {
                self.call_trigger_workspace_broadcast(id, arguments).await
            }
            tool_names::ADJUST_COUPLING => self.call_adjust_coupling(id, arguments).await,
            // TASK-ATC-001: Adaptive Threshold Calibration tools
            tool_names::GET_THRESHOLD_STATUS => {
                self.handle_get_threshold_status(id, Some(arguments)).await
            }
            tool_names::GET_CALIBRATION_METRICS => {
                self.handle_get_calibration_metrics(id, Some(arguments)).await
            }
            tool_names::TRIGGER_RECALIBRATION => {
                self.handle_trigger_recalibration(id, Some(arguments)).await
            }
            // TASK-DREAM-MCP: Dream consolidation tools
            tool_names::TRIGGER_DREAM => self.call_trigger_dream(id, arguments).await,
            tool_names::GET_DREAM_STATUS => self.call_get_dream_status(id).await,
            tool_names::ABORT_DREAM => self.call_abort_dream(id, arguments).await,
            tool_names::GET_AMORTIZED_SHORTCUTS => {
                self.call_get_amortized_shortcuts(id, arguments).await
            }
            // TASK-NEUROMOD-MCP: Neuromodulation tools
            tool_names::GET_NEUROMODULATION_STATE => {
                self.call_get_neuromodulation_state(id).await
            }
            tool_names::ADJUST_NEUROMODULATOR => {
                self.call_adjust_neuromodulator(id, arguments).await
            }
            // TASK-STEERING-001: Steering tools
            tool_names::GET_STEERING_FEEDBACK => {
                self.call_get_steering_feedback(id).await
            }
            // TASK-CAUSAL-001: Causal inference tools
            tool_names::OMNI_INFER => {
                self.call_omni_infer(id, arguments).await
            }
            // NOTE: Manual North Star tools REMOVED (set_north_star, get_north_star,
            // update_north_star, delete_north_star, init_north_star_from_documents,
            // get_goal_hierarchy) - they created single 1024D embeddings that cannot
            // be meaningfully compared to 13-embedder teleological arrays.
            // Use the autonomous system below which works with proper teleological embeddings.
            // TELEO-H1 to TELEO-H5: Teleological tools
            tool_names::SEARCH_TELEOLOGICAL => {
                self.call_search_teleological(id, arguments).await
            }
            tool_names::COMPUTE_TELEOLOGICAL_VECTOR => {
                self.call_compute_teleological_vector(id, arguments).await
            }
            tool_names::FUSE_EMBEDDINGS => {
                self.call_fuse_embeddings(id, arguments).await
            }
            tool_names::UPDATE_SYNERGY_MATRIX => {
                self.call_update_synergy_matrix(id, arguments).await
            }
            tool_names::MANAGE_TELEOLOGICAL_PROFILE => {
                self.call_manage_teleological_profile(id, arguments).await
            }
            // TASK-AUTONOMOUS-MCP: Autonomous North Star tools
            tool_names::AUTO_BOOTSTRAP_NORTH_STAR => {
                self.call_auto_bootstrap_north_star(id, arguments).await
            }
            tool_names::GET_ALIGNMENT_DRIFT => {
                self.call_get_alignment_drift(id, arguments).await
            }
            tool_names::TRIGGER_DRIFT_CORRECTION => {
                self.call_trigger_drift_correction(id, arguments).await
            }
            tool_names::GET_PRUNING_CANDIDATES => {
                self.call_get_pruning_candidates(id, arguments).await
            }
            tool_names::TRIGGER_CONSOLIDATION => {
                self.call_trigger_consolidation(id, arguments).await
            }
            tool_names::DISCOVER_SUB_GOALS => {
                self.call_discover_sub_goals(id, arguments).await
            }
            tool_names::GET_AUTONOMOUS_STATUS => {
                self.call_get_autonomous_status(id, arguments).await
            }
            _ => JsonRpcResponse::error(
                id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", tool_name),
            ),
        }
    }

    // ========== Tool Call Implementations ==========

    /// MCP-compliant tool result helper WITH CognitivePulse injection.
    ///
    /// Wraps tool output in the required MCP format with live UTL metrics:
    /// ```json
    /// {
    ///   "content": [{"type": "text", "text": "..."}],
    ///   "isError": false,
    ///   "_cognitive_pulse": {
    ///     "entropy": 0.42,
    ///     "coherence": 0.78,
    ///     "learning_score": 0.55,
    ///     "quadrant": "Open",
    ///     "suggested_action": "DirectRecall"
    ///   }
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// CognitivePulse computation targets < 1ms. Warning logged if exceeded.
    ///
    /// # Error Handling
    ///
    /// FAIL FAST: If CognitivePulse computation fails, the ENTIRE tool call
    /// fails with a detailed error. NO fallbacks, NO default values.
    pub(super) fn tool_result_with_pulse(
        &self,
        id: Option<JsonRpcId>,
        data: serde_json::Value,
    ) -> JsonRpcResponse {
        // Compute CognitivePulse - FAIL FAST if unavailable
        let pulse = match CognitivePulse::from_processor(self.utl_processor.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                // FAIL FAST - no fallbacks
                error!(
                    error = %e,
                    "CognitivePulse computation FAILED - tool call rejected"
                );
                return JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": format!("UTL pulse computation failed: {}", e)
                        }],
                        "isError": true
                    }),
                );
            }
        };

        JsonRpcResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_string())
                }],
                "isError": false,
                "_cognitive_pulse": pulse
            }),
        )
    }

    /// MCP-compliant tool error helper WITH CognitivePulse injection.
    ///
    /// Even error responses include the cognitive pulse to maintain
    /// consistent system state visibility.
    ///
    /// # Error Handling
    ///
    /// If pulse computation fails during error response, logs warning
    /// but still returns the original error (pulse failure is secondary).
    pub(super) fn tool_error_with_pulse(&self, id: Option<JsonRpcId>, message: &str) -> JsonRpcResponse {
        // Try to compute pulse, but don't fail the error response if it fails
        let pulse_result = CognitivePulse::from_processor(self.utl_processor.as_ref());

        match pulse_result {
            Ok(pulse) => JsonRpcResponse::success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": message
                    }],
                    "isError": true,
                    "_cognitive_pulse": pulse
                }),
            ),
            Err(e) => {
                warn!(
                    error = %e,
                    original_error = message,
                    "CognitivePulse computation failed for error response"
                );
                // Still return the original error, just without pulse
                JsonRpcResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": format!("{} (pulse unavailable: {})", message, e)
                        }],
                        "isError": true
                    }),
                )
            }
        }
    }

    /// inject_context tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore with 13-embedding fingerprint.
    ///
    /// Injects context into the memory graph with UTL metrics computation.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(super) async fn call_inject_context(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => return self.tool_error_with_pulse(id, "Content cannot be empty"),
            None => return self.tool_error_with_pulse(id, "Missing 'content' parameter"),
        };

        let rationale = args.get("rationale").and_then(|v| v.as_str()).unwrap_or("");
        let _importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        // Get goal_vector from North Star for UTL alignment computation (Issue 3 fix)
        // Per constitution.yaml: alignment = cos(content_embedding, goal_vector)
        // Without goal_vector, alignment always returns 1.0 (useless)
        // Per TASK-CORE-005: Use E1 semantic embedding from TeleologicalArray for UTL alignment
        let goal_vector = {
            let hierarchy = self.goal_hierarchy.read();
            hierarchy.north_star().map(|ns| ns.teleological_array.e1_semantic.clone())
        };

        // Compute UTL metrics for the content
        let context = UtlContext {
            goal_vector,
            ..Default::default()
        };
        let metrics = match self.utl_processor.compute_metrics(&content, &context).await {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "inject_context: UTL processing FAILED");
                return self.tool_error_with_pulse(id, &format!("UTL processing failed: {}", e));
            }
        };

        // Generate all 13 embeddings using MultiArrayEmbeddingProvider
        let embedding_output = match self.multi_array_provider.embed_all(&content).await {
            Ok(output) => output,
            Err(e) => {
                error!(error = %e, "inject_context: Multi-array embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Embedding failed: {}", e));
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // AUTONOMOUS OPERATION: Compute purpose vector if North Star exists,
        // otherwise use default (neutral) alignment.
        //
        // From contextprd.md: "The array [of 13 embeddings] IS the teleological vector"
        // Purpose alignment is SECONDARY metadata - the 13-embedding fingerprint is primary.
        // This allows autonomous operation without manual North Star configuration.
        //
        // When North Star exists: PV = [A(E1,V), A(E2,V), ..., A(E13,V)]
        // where A(Ei, V) = cos(θ) between embedder i and North Star goal V
        //
        // When no North Star: PV = [0.0; 13] (neutral alignment)
        // Memories can be stored immediately; purpose can be recomputed later.
        let purpose_vector = {
            let hierarchy = self.goal_hierarchy.read().clone();

            // If no North Star goal is defined, use default purpose vector
            // This enables AUTONOMOUS operation - no manual configuration required
            if hierarchy.north_star().is_none() {
                debug!(
                    "inject_context: No North Star configured. Using default purpose vector. \
                     Memory will be stored with neutral alignment (can be recomputed later)."
                );
                PurposeVector::default()
            } else {
                // Compute purpose vector using DefaultPurposeComputer
                // This computes alignment for each of 13 embedding spaces
                let config = PurposeComputeConfig::with_hierarchy(hierarchy);

                match DefaultPurposeComputer::new()
                    .compute_purpose(&embedding_output.fingerprint, &config)
                    .await
                {
                    Ok(pv) => {
                        debug!(
                            aggregate_alignment = pv.aggregate_alignment(),
                            dominant_embedder = pv.dominant_embedder,
                            coherence = pv.coherence,
                            "inject_context: Purpose vector computed for semantic fingerprint"
                        );
                        pv
                    }
                    Err(e) => {
                        // If North Star exists but computation fails, THAT is an error
                        error!(
                            error = %e,
                            "inject_context: Failed to compute purpose vector. \
                             Cannot store memory without alignment metadata."
                        );
                        return self.tool_error_with_pulse(
                            id,
                            &format!("Purpose vector computation failed: {}", e),
                        );
                    }
                }
            }
        };

        // Create TeleologicalFingerprint with REAL computed purpose vector
        let fingerprint = TeleologicalFingerprint::new(
            embedding_output.fingerprint,
            purpose_vector,
            JohariFingerprint::zeroed(),
            content_hash,
        );
        let fingerprint_id = fingerprint.id;

        // Store in TeleologicalMemoryStore
        if let Err(e) = self.teleological_store.store(fingerprint).await {
            error!(error = %e, "inject_context: Storage FAILED");
            return self.tool_error_with_pulse(id, &format!("Storage failed: {}", e));
        }

        self.tool_result_with_pulse(
            id,
            json!({
                "fingerprintId": fingerprint_id.to_string(),
                "rationale": rationale,
                "embedderCount": NUM_EMBEDDERS,
                "embeddingLatencyMs": embedding_output.total_latency.as_millis(),
                "utl": {
                    "learningScore": metrics.learning_score,
                    "entropy": metrics.entropy,
                    "coherence": metrics.coherence,
                    "surprise": metrics.surprise
                }
            }),
        )
    }

    /// store_memory tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore with 13-embedding fingerprint.
    ///
    /// Stores content in the memory graph.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(super) async fn call_store_memory(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.is_empty() => c.to_string(),
            Some(_) => return self.tool_error_with_pulse(id, "Content cannot be empty"),
            None => return self.tool_error_with_pulse(id, "Missing 'content' parameter"),
        };

        let _importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        // Generate all 13 embeddings using MultiArrayEmbeddingProvider
        let embedding_output = match self.multi_array_provider.embed_all(&content).await {
            Ok(output) => output,
            Err(e) => {
                error!(error = %e, "store_memory: Multi-array embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Embedding failed: {}", e));
            }
        };

        // Compute content hash
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let content_hash: [u8; 32] = hasher.finalize().into();

        // AP-007: Compute purpose vector from semantic fingerprint and goal hierarchy
        // Purpose vectors MUST be computed at storage time, not deferred (NO DEFAULT)
        // From constitution.yaml: PV = [A(E1,V), A(E2,V), ..., A(E13,V)]
        let purpose_vector = {
            let hierarchy = self.goal_hierarchy.read().clone();

            // If no North Star goal is defined, FAIL FAST (AP-007)
            if hierarchy.north_star().is_none() {
                error!(
                    "store_memory: Goal hierarchy missing North Star goal. \
                     Cannot compute purpose vector. \
                     CONFIGURATION ERROR: Use auto_bootstrap_north_star tool for autonomous goal discovery."
                );
                return self.tool_error_with_pulse(
                    id,
                    "Goal hierarchy not configured. Cannot compute purpose vector. \
                     Use auto_bootstrap_north_star tool for autonomous goal discovery.",
                );
            }

            // Compute purpose vector using DefaultPurposeComputer
            let config = PurposeComputeConfig::with_hierarchy(hierarchy);

            match DefaultPurposeComputer::new()
                .compute_purpose(&embedding_output.fingerprint, &config)
                .await
            {
                Ok(pv) => {
                    debug!(
                        aggregate_alignment = pv.aggregate_alignment(),
                        dominant_embedder = pv.dominant_embedder,
                        coherence = pv.coherence,
                        "store_memory: Purpose vector computed for semantic fingerprint"
                    );
                    pv
                }
                Err(e) => {
                    error!(
                        error = %e,
                        "store_memory: Failed to compute purpose vector. \
                         Cannot store memory without alignment metadata."
                    );
                    return self.tool_error_with_pulse(
                        id,
                        &format!("Purpose vector computation failed: {}", e),
                    );
                }
            }
        };

        // Create TeleologicalFingerprint with REAL computed purpose vector
        let fingerprint = TeleologicalFingerprint::new(
            embedding_output.fingerprint,
            purpose_vector,
            JohariFingerprint::zeroed(),
            content_hash,
        );
        let fingerprint_id = fingerprint.id;

        match self.teleological_store.store(fingerprint).await {
            Ok(_) => self.tool_result_with_pulse(
                id,
                json!({
                    "fingerprintId": fingerprint_id.to_string(),
                    "embedderCount": NUM_EMBEDDERS,
                    "embeddingLatencyMs": embedding_output.total_latency.as_millis()
                }),
            ),
            Err(e) => {
                error!(error = %e, "store_memory: Storage FAILED");
                self.tool_error_with_pulse(id, &format!("Storage failed: {}", e))
            }
        }
    }

    /// get_memetic_status tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore count.
    ///
    /// Returns comprehensive system status including:
    /// - Fingerprint count from TeleologicalMemoryStore
    /// - Live UTL metrics from UtlProcessor (NOT hardcoded)
    /// - 5-layer bio-nervous system status
    /// - `_cognitive_pulse` with live system state
    ///
    /// # Constitution References
    /// - UTL formula: constitution.yaml:152
    /// - Johari quadrant actions: constitution.yaml:159-163
    pub(super) async fn call_get_memetic_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        let fingerprint_count = match self.teleological_store.count().await {
            Ok(count) => count,
            Err(e) => {
                error!(error = %e, "get_memetic_status: TeleologicalStore.count() FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to get fingerprint count: {}", e),
                );
            }
        };

        // Get LIVE UTL status from the processor
        let utl_status = self.utl_processor.get_status();

        // FAIL-FAST: UTL processor MUST return all required fields.
        // Per constitution AP-007: No stubs or fallbacks in production code paths.
        // If the UTL processor doesn't have these fields, the system is broken.
        let lifecycle_phase = match utl_status.get("lifecycle_phase").and_then(|v| v.as_str()) {
            Some(phase) => phase,
            None => {
                error!("get_memetic_status: UTL processor missing 'lifecycle_phase' field - system is broken");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    "UTL processor returned incomplete status: missing 'lifecycle_phase'. \
                     This indicates a broken UTL system that must be fixed.".to_string(),
                );
            }
        };

        let entropy = match utl_status.get("entropy").and_then(|v| v.as_f64()) {
            Some(v) => v as f32,
            None => {
                error!("get_memetic_status: UTL processor missing 'entropy' field - system is broken");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    "UTL processor returned incomplete status: missing 'entropy'. \
                     This indicates a broken UTL system that must be fixed.".to_string(),
                );
            }
        };

        let coherence = match utl_status.get("coherence").and_then(|v| v.as_f64()) {
            Some(v) => v as f32,
            None => {
                error!("get_memetic_status: UTL processor missing 'coherence' field - system is broken");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    "UTL processor returned incomplete status: missing 'coherence'. \
                     This indicates a broken UTL system that must be fixed.".to_string(),
                );
            }
        };

        let learning_score = match utl_status.get("learning_score").and_then(|v| v.as_f64()) {
            Some(v) => v as f32,
            None => {
                error!("get_memetic_status: UTL processor missing 'learning_score' field - system is broken");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    "UTL processor returned incomplete status: missing 'learning_score'. \
                     This indicates a broken UTL system that must be fixed.".to_string(),
                );
            }
        };

        let johari_quadrant = match utl_status.get("johari_quadrant").and_then(|v| v.as_str()) {
            Some(q) => q,
            None => {
                error!("get_memetic_status: UTL processor missing 'johari_quadrant' field - system is broken");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    "UTL processor returned incomplete status: missing 'johari_quadrant'. \
                     This indicates a broken UTL system that must be fixed.".to_string(),
                );
            }
        };

        let consolidation_phase = match utl_status.get("consolidation_phase").and_then(|v| v.as_str()) {
            Some(phase) => phase,
            None => {
                error!("get_memetic_status: UTL processor missing 'consolidation_phase' field - system is broken");
                return JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    "UTL processor returned incomplete status: missing 'consolidation_phase'. \
                     This indicates a broken UTL system that must be fixed.".to_string(),
                );
            }
        };

        // Map Johari quadrant to suggested action per constitution.yaml:159-163
        let suggested_action = match johari_quadrant {
            "Open" => "direct_recall",
            "Blind" => "trigger_dream",
            "Hidden" => "get_neighborhood",
            "Unknown" => "epistemic_action",
            _ => "continue",
        };

        // Get quadrant counts from teleological store
        let quadrant_counts = match self.teleological_store.count_by_quadrant().await {
            Ok(counts) => counts,
            Err(e) => {
                error!(error = %e, "get_memetic_status: count_by_quadrant() FAILED");
                return JsonRpcResponse::error(
                    id,
                    error_codes::STORAGE_ERROR,
                    format!("Failed to get quadrant counts: {}", e),
                );
            }
        };

        // TASK-EMB-024: Get REAL layer statuses from LayerStatusProvider
        let perception_status = self.layer_status_provider.perception_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: perception_status FAILED");
                "error".to_string()
            });
        let memory_status = self.layer_status_provider.memory_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: memory_status FAILED");
                "error".to_string()
            });
        let reasoning_status = self.layer_status_provider.reasoning_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: reasoning_status FAILED");
                "error".to_string()
            });
        let action_status = self.layer_status_provider.action_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: action_status FAILED");
                "error".to_string()
            });
        let meta_status = self.layer_status_provider.meta_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_memetic_status: meta_status FAILED");
                "error".to_string()
            });

        self.tool_result_with_pulse(
            id,
            json!({
                "phase": lifecycle_phase,
                "fingerprintCount": fingerprint_count,
                "embedderCount": NUM_EMBEDDERS,
                "storageBackend": self.teleological_store.backend_type().to_string(),
                "storageSizeBytes": self.teleological_store.storage_size_bytes(),
                "quadrantCounts": {
                    "open": quadrant_counts[0],
                    "hidden": quadrant_counts[1],
                    "blind": quadrant_counts[2],
                    "unknown": quadrant_counts[3]
                },
                "utl": {
                    "entropy": entropy,
                    "coherence": coherence,
                    "learningScore": learning_score,
                    "johariQuadrant": johari_quadrant,
                    "consolidationPhase": consolidation_phase,
                    "suggestedAction": suggested_action
                },
                "layers": {
                    "perception": perception_status,
                    "memory": memory_status,
                    "reasoning": reasoning_status,
                    "action": action_status,
                    "meta": meta_status
                }
            }),
        )
    }

    /// get_graph_manifest tool implementation.
    ///
    /// Returns the 5-layer bio-nervous architecture manifest.
    /// Response includes `_cognitive_pulse` with live system state.
    ///
    /// TASK-EMB-024: Layer statuses now come from LayerStatusProvider.
    pub(super) async fn call_get_graph_manifest(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        // TASK-EMB-024: Get REAL layer statuses from LayerStatusProvider
        let perception_status = self.layer_status_provider.perception_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_graph_manifest: perception_status FAILED");
                "error".to_string()
            });
        let memory_status = self.layer_status_provider.memory_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_graph_manifest: memory_status FAILED");
                "error".to_string()
            });
        let reasoning_status = self.layer_status_provider.reasoning_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_graph_manifest: reasoning_status FAILED");
                "error".to_string()
            });
        let action_status = self.layer_status_provider.action_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_graph_manifest: action_status FAILED");
                "error".to_string()
            });
        let meta_status = self.layer_status_provider.meta_status().await
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|e| {
                error!(error = %e, "get_graph_manifest: meta_status FAILED");
                "error".to_string()
            });

        self.tool_result_with_pulse(
            id,
            json!({
                "architecture": "5-layer-bio-nervous",
                "fingerprintType": "TeleologicalFingerprint",
                "embedderCount": NUM_EMBEDDERS,
                "layers": [
                    {
                        "name": "Perception",
                        "description": "Sensory input processing and feature extraction",
                        "status": perception_status
                    },
                    {
                        "name": "Memory",
                        "description": "Teleological memory with 13-embedding semantic fingerprints",
                        "status": memory_status
                    },
                    {
                        "name": "Reasoning",
                        "description": "Inference, planning, and decision making",
                        "status": reasoning_status
                    },
                    {
                        "name": "Action",
                        "description": "Response generation and motor control",
                        "status": action_status
                    },
                    {
                        "name": "Meta",
                        "description": "Self-monitoring, learning rate control, and system optimization",
                        "status": meta_status
                    }
                ],
                "utl": {
                    "description": "Universal Transfer Learning - measures learning potential",
                    "formula": "L(x) = H(P) - H(P|x) + alpha * C(x)"
                },
                "teleological": {
                    "description": "Purpose-aware retrieval with North Star alignment",
                    "purposeVectorDimension": NUM_EMBEDDERS,
                    "johariQuadrants": ["Open", "Hidden", "Blind", "Unknown"]
                }
            }),
        )
    }

    /// search_graph tool implementation.
    ///
    /// TASK-S001: Updated to use TeleologicalMemoryStore search_semantic.
    ///
    /// Searches the memory graph for matching content.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(super) async fn call_search_graph(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let query = match args.get("query").and_then(|v| v.as_str()) {
            Some(q) if !q.is_empty() => q,
            Some(_) => return self.tool_error_with_pulse(id, "Query cannot be empty"),
            None => return self.tool_error_with_pulse(id, "Missing 'query' parameter"),
        };

        let top_k = args.get("topK").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let options = TeleologicalSearchOptions::quick(top_k);

        // Generate query embedding
        let query_embedding = match self.multi_array_provider.embed_all(query).await {
            Ok(output) => output.fingerprint,
            Err(e) => {
                error!(error = %e, "search_graph: Query embedding FAILED");
                return self.tool_error_with_pulse(id, &format!("Query embedding failed: {}", e));
            }
        };

        match self.teleological_store.search_semantic(&query_embedding, options).await {
            Ok(results) => {
                let results_json: Vec<_> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "fingerprintId": r.fingerprint.id.to_string(),
                            "similarity": r.similarity,
                            "purposeAlignment": r.purpose_alignment,
                            "dominantEmbedder": r.dominant_embedder(),
                            "thetaToNorthStar": r.fingerprint.theta_to_north_star
                        })
                    })
                    .collect();

                self.tool_result_with_pulse(
                    id,
                    json!({ "results": results_json, "count": results_json.len() }),
                )
            }
            Err(e) => {
                error!(error = %e, "search_graph: Search FAILED");
                self.tool_error_with_pulse(id, &format!("Search failed: {}", e))
            }
        }
    }

    /// utl_status tool implementation.
    ///
    /// Returns current UTL system state including lifecycle phase, entropy,
    /// coherence, learning score, Johari quadrant, and consolidation phase.
    /// Response includes `_cognitive_pulse` with live system state.
    pub(super) async fn call_utl_status(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling utl_status tool call");

        // Get status from UTL processor (returns serde_json::Value)
        let status = self.utl_processor.get_status();

        self.tool_result_with_pulse(id, status)
    }

    /// get_consciousness_state tool implementation.
    ///
    /// TASK-GWT-001: Returns complete consciousness state from GWT/Kuramoto system.
    /// FAIL FAST on missing GWT components - no stubs or fallbacks.
    ///
    /// Returns:
    /// - C: Consciousness level C(t) = I(t) x R(t) x D(t)
    /// - r: Kuramoto order parameter (synchronization)
    /// - psi: Kuramoto mean phase
    /// - meta_score: Meta-cognitive accuracy
    /// - differentiation: Purpose vector entropy
    /// - state: CONSCIOUS/EMERGING/FRAGMENTED
    /// - workspace: Active memory and broadcast status
    /// - identity: Coherence and purpose vector
    /// - component_analysis: Which factor limits consciousness
    pub(super) async fn call_get_consciousness_state(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_consciousness_state tool call");

        // FAIL FAST: Check all required GWT providers
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("get_consciousness_state: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        let gwt_system = match &self.gwt_system {
            Some(g) => g,
            None => {
                error!("get_consciousness_state: GWT system not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "GWT system not initialized - use with_gwt() constructor",
                );
            }
        };

        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("get_consciousness_state: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        let meta_cognitive = match &self.meta_cognitive {
            Some(m) => m,
            None => {
                error!("get_consciousness_state: Meta-cognitive provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Meta-cognitive provider not initialized - use with_gwt() constructor",
                );
            }
        };

        let self_ego = match &self.self_ego {
            Some(s) => s,
            None => {
                error!("get_consciousness_state: Self-ego provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Self-ego provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // Get Kuramoto order parameter (r, psi)
        // parking_lot::RwLock::read() doesn't return Result, it blocks until lock acquired
        let (r, psi) = {
            let kuramoto_guard = kuramoto.read();
            kuramoto_guard.order_parameter()
        };

        // Get purpose vector from self-ego
        let purpose_vector = self_ego.read().await.purpose_vector();

        // Get meta-cognitive accuracy
        let meta_accuracy = {
            let meta = meta_cognitive.read().await;
            // Use acetylcholine level as a proxy for meta-cognitive accuracy
            // Higher Ach means better learning/attention
            meta.acetylcholine()
        };

        // Compute consciousness metrics from GWT system
        let metrics = match gwt_system.compute_metrics(r as f32, meta_accuracy, &purpose_vector) {
            Ok(m) => m,
            Err(e) => {
                error!(error = %e, "get_consciousness_state: Consciousness computation failed");
                return JsonRpcResponse::error(
                    id,
                    error_codes::CONSCIOUSNESS_COMPUTATION_FAILED,
                    format!("Consciousness computation failed: {}", e),
                );
            }
        };

        // Get workspace status
        let workspace_guard = workspace.read().await;
        let active_memory = workspace_guard.get_active_memory();
        let is_broadcasting = workspace_guard.is_broadcasting();
        let has_conflict = workspace_guard.has_conflict();
        let coherence_threshold = workspace_guard.coherence_threshold();

        // Get identity coherence from self-ego
        let identity_coherence = self_ego.read().await.identity_coherence();
        let identity_status = self_ego.read().await.identity_status();
        let trajectory_length = self_ego.read().await.trajectory_length();

        // Determine consciousness state string per constitution.yaml lines 394-408
        // Uses the canonical ConsciousnessState::from_level() which implements all 5 states:
        // DORMANT (r < 0.3), FRAGMENTED (0.3 <= r < 0.5), EMERGING (0.5 <= r < 0.8),
        // CONSCIOUS (0.8 <= r <= 0.95), HYPERSYNC (r > 0.95)
        let state = ConsciousnessState::from_level(r as f32).name();

        // Get GWT current state
        let gwt_state = gwt_system.current_state();
        let time_in_state = gwt_system.time_in_state();

        self.tool_result_with_pulse(
            id,
            json!({
                "C": metrics.consciousness,
                "r": r,
                "psi": psi,
                "meta_score": meta_accuracy,
                "differentiation": metrics.differentiation,
                "integration": metrics.integration,
                "reflection": metrics.reflection,
                "state": state,
                "gwt_state": format!("{:?}", gwt_state),
                "time_in_state_ms": time_in_state.as_millis(),
                "workspace": {
                    "active_memory": active_memory.map(|id| id.to_string()),
                    "is_broadcasting": is_broadcasting,
                    "has_conflict": has_conflict,
                    "coherence_threshold": coherence_threshold
                },
                "identity": {
                    "coherence": identity_coherence,
                    "status": format!("{:?}", identity_status),
                    "trajectory_length": trajectory_length,
                    "purpose_vector": purpose_vector.to_vec()
                },
                "component_analysis": {
                    "integration_sufficient": metrics.component_analysis.integration_sufficient,
                    "reflection_sufficient": metrics.component_analysis.reflection_sufficient,
                    "differentiation_sufficient": metrics.component_analysis.differentiation_sufficient,
                    "limiting_factor": format!("{:?}", metrics.component_analysis.limiting_factor)
                }
            }),
        )
    }

    /// get_kuramoto_sync tool implementation.
    ///
    /// TASK-GWT-001: Returns Kuramoto oscillator network state.
    /// Provides detailed synchronization metrics for 13-embedding phase coupling.
    ///
    /// FAIL FAST on missing Kuramoto network - no stubs or fallbacks.
    ///
    /// Returns:
    /// - r: Order parameter (synchronization level) in [0, 1]
    /// - psi: Mean phase in [0, 2π]
    /// - synchronization: Same as r (for convenience)
    /// - state: CONSCIOUS/EMERGING/FRAGMENTED/HYPERSYNC
    /// - phases: All 13 oscillator phases
    /// - natural_freqs: All 13 natural frequencies (Hz)
    /// - coupling: Coupling strength K
    /// - elapsed_seconds: Time since creation/reset
    /// - embedding_labels: Names of the 13 embedding spaces
    /// - thresholds: State transition thresholds
    pub(super) async fn call_get_kuramoto_sync(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_kuramoto_sync tool call");

        // FAIL FAST: Check kuramoto provider
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("get_kuramoto_sync: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // Acquire read lock (parking_lot RwLock blocks until available, no Result)
        let network = kuramoto.read();

        // Get order parameter (r, psi)
        let (r, psi) = network.order_parameter();

        // Get all 13 oscillator phases
        let phases = network.phases();

        // Get all 13 natural frequencies
        let natural_freqs = network.natural_frequencies();

        // Get coupling strength K
        let coupling = network.coupling_strength();

        // Get synchronization (same as r but as f64)
        let sync = network.synchronization();

        // Classify state based on r using canonical state machine (constitution.yaml lines 394-408)
        // This ensures consistency with get_consciousness_state and all 5 states are handled:
        // DORMANT (r < 0.3), FRAGMENTED (0.3 <= r < 0.5), EMERGING (0.5 <= r < 0.8),
        // CONSCIOUS (0.8 <= r <= 0.95), HYPERSYNC (r > 0.95)
        let state = ConsciousnessState::from_level(sync as f32).name();

        // Get elapsed time
        let elapsed = network.elapsed_total();

        // Return complete Kuramoto state
        self.tool_result_with_pulse(id, json!({
            "r": r,
            "psi": psi,
            "synchronization": sync,
            "state": state,
            "phases": phases.to_vec(),
            "natural_freqs": natural_freqs.to_vec(),
            "coupling": coupling,
            "elapsed_seconds": elapsed.as_secs_f64(),
            "embedding_labels": [
                "E1_semantic", "E2_temporal_recent", "E3_temporal_periodic",
                "E4_temporal_positional", "E5_causal", "E6_sparse",
                "E7_code", "E8_graph", "E9_hdc", "E10_multimodal",
                "E11_entity", "E12_late_interaction", "E13_splade"
            ],
            "thresholds": {
                "conscious": 0.8,
                "fragmented": 0.5,
                "hypersync": 0.95
            }
        }))
    }

    /// get_workspace_status tool implementation.
    ///
    /// TASK-GWT-001: Returns Global Workspace status including active memory,
    /// competing candidates, broadcast state, and coherence threshold.
    ///
    /// FAIL FAST on missing workspace provider - no stubs or fallbacks.
    ///
    /// Returns:
    /// - active_memory: UUID of currently active (conscious) memory, or null
    /// - is_broadcasting: Whether broadcast window is active
    /// - has_conflict: Whether multiple memories compete (r > 0.8)
    /// - coherence_threshold: Threshold for workspace entry (default 0.8)
    /// - conflict_memories: List of conflicting memory UUIDs if has_conflict
    pub(super) async fn call_get_workspace_status(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_workspace_status tool call");

        // FAIL FAST: Check workspace provider
        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("get_workspace_status: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // Acquire read lock (tokio RwLock)
        let ws = workspace.read().await;

        // Get active memory if broadcasting
        let active_memory = ws.get_active_memory();

        // Check broadcast state
        let is_broadcasting = ws.is_broadcasting();

        // Check for conflict
        let has_conflict = ws.has_conflict();

        // Get coherence threshold
        let coherence_threshold = ws.coherence_threshold();

        // Get conflict details if present
        let conflict_memories = ws.get_conflict_details();

        self.tool_result_with_pulse(
            id,
            json!({
                "active_memory": active_memory.map(|id| id.to_string()),
                "is_broadcasting": is_broadcasting,
                "has_conflict": has_conflict,
                "coherence_threshold": coherence_threshold,
                "conflict_memories": conflict_memories.map(|ids|
                    ids.iter().map(|id| id.to_string()).collect::<Vec<_>>()
                ),
                "broadcast_duration_ms": 100 // Constitution default
            }),
        )
    }

    /// get_ego_state tool implementation.
    ///
    /// TASK-GWT-001: Returns Self-Ego Node state including purpose vector,
    /// identity continuity, coherence with actions, and trajectory length.
    ///
    /// FAIL FAST on missing self-ego provider - no stubs or fallbacks.
    ///
    /// Returns:
    /// - purpose_vector: 13D purpose alignment vector
    /// - identity_coherence: IC = cos(PV_t, PV_{t-1}) x r(t)
    /// - coherence_with_actions: Alignment between actions and purpose
    /// - identity_status: Healthy/Warning/Degraded/Critical
    /// - trajectory_length: Number of purpose snapshots stored
    pub(super) async fn call_get_ego_state(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        debug!("Handling get_ego_state tool call");

        // FAIL FAST: Check self-ego provider
        let self_ego = match &self.self_ego {
            Some(s) => s,
            None => {
                error!("get_ego_state: Self-ego provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Self-ego provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // Acquire read lock (tokio RwLock)
        let ego = self_ego.read().await;

        // Get purpose vector
        let purpose_vector = ego.purpose_vector();

        // Get identity coherence
        let identity_coherence = ego.identity_coherence();

        // Get coherence with actions
        let coherence_with_actions = ego.coherence_with_actions();

        // Get identity status
        let identity_status = ego.identity_status();

        // Get trajectory length
        let trajectory_length = ego.trajectory_length();

        self.tool_result_with_pulse(
            id,
            json!({
                "purpose_vector": purpose_vector.to_vec(),
                "identity_coherence": identity_coherence,
                "coherence_with_actions": coherence_with_actions,
                "identity_status": format!("{:?}", identity_status),
                "trajectory_length": trajectory_length,
                "thresholds": {
                    "healthy": 0.9,
                    "warning": 0.7,
                    "degraded": 0.5,
                    "critical": 0.0
                }
            }),
        )
    }

    /// trigger_workspace_broadcast tool implementation.
    ///
    /// TASK-GWT-001: Triggers winner-take-all selection with a specific memory.
    /// Forces memory into workspace competition. Requires write lock on workspace.
    ///
    /// FAIL FAST on missing providers - no stubs or fallbacks.
    ///
    /// Arguments:
    /// - memory_id: UUID of memory to broadcast
    /// - importance: Importance score [0,1] (default 0.8)
    /// - alignment: North star alignment [0,1] (default 0.8)
    /// - force: Force broadcast even if below coherence threshold
    ///
    /// Returns:
    /// - success: Whether broadcast was successful
    /// - memory_id: UUID of the memory
    /// - new_r: Current Kuramoto order parameter
    /// - was_selected: Whether this memory won WTA selection
    pub(super) async fn call_trigger_workspace_broadcast(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling trigger_workspace_broadcast tool call");

        // FAIL FAST: Check workspace provider
        let workspace = match &self.workspace_provider {
            Some(w) => w,
            None => {
                error!("trigger_workspace_broadcast: Workspace provider not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Workspace provider not initialized - use with_gwt() constructor",
                );
            }
        };

        // FAIL FAST: Check kuramoto provider (needed for order parameter)
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("trigger_workspace_broadcast: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // Parse memory_id (required)
        let memory_id_str = match args.get("memory_id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'memory_id' parameter");
            }
        };

        let memory_id = match uuid::Uuid::parse_str(memory_id_str) {
            Ok(id) => id,
            Err(e) => {
                return self.tool_error_with_pulse(
                    id,
                    &format!("Invalid UUID format for memory_id: {}", e),
                );
            }
        };

        // Parse optional parameters
        let importance = args
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8) as f32;
        let alignment = args
            .get("alignment")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8) as f32;
        let force = args
            .get("force")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get current order parameter from Kuramoto network
        let r = {
            let kuramoto_guard = kuramoto.read();
            kuramoto_guard.synchronization() as f32
        };

        // Check if memory qualifies for workspace (r >= 0.8 unless forced)
        let coherence_threshold = 0.8;
        if r < coherence_threshold && !force {
            return self.tool_result_with_pulse(
                id,
                json!({
                    "success": false,
                    "memory_id": memory_id.to_string(),
                    "new_r": r,
                    "was_selected": false,
                    "reason": format!(
                        "Order parameter r={:.3} below coherence threshold {}. Use force=true to override.",
                        r, coherence_threshold
                    )
                }),
            );
        }

        // Acquire write lock and trigger selection
        let ws = workspace.write().await;

        // Create candidate and trigger WTA selection
        let candidates = vec![(memory_id, r, importance, alignment)];
        let winner = match ws.select_winning_memory(candidates).await {
            Ok(w) => w,
            Err(e) => {
                error!(error = %e, "trigger_workspace_broadcast: WTA selection failed");
                return JsonRpcResponse::error(
                    id,
                    error_codes::WORKSPACE_ERROR,
                    format!("Workspace selection failed: {}", e),
                );
            }
        };

        let was_selected = winner == Some(memory_id);

        // GAP-1 FIX: Wire workspace events to neuromodulation
        // When a memory enters workspace (was_selected), increase dopamine
        let dopamine_triggered = if was_selected {
            if let Some(neuromod) = &self.neuromod_manager {
                let mut manager = neuromod.write();
                manager.on_workspace_entry();
                let new_dopamine = manager.get_hopfield_beta();
                debug!(
                    memory_id = %memory_id,
                    dopamine = new_dopamine,
                    "Workspace entry triggered dopamine increase"
                );
                Some(new_dopamine)
            } else {
                None
            }
        } else {
            None
        };

        self.tool_result_with_pulse(
            id,
            json!({
                "success": true,
                "memory_id": memory_id.to_string(),
                "new_r": r,
                "was_selected": was_selected,
                "is_broadcasting": ws.is_broadcasting(),
                "dopamine_triggered": dopamine_triggered
            }),
        )
    }

    /// adjust_coupling tool implementation.
    ///
    /// TASK-GWT-001: Adjusts Kuramoto oscillator network coupling strength K.
    /// Higher K leads to faster synchronization. K is clamped to [0, 10].
    ///
    /// FAIL FAST on missing kuramoto provider - no stubs or fallbacks.
    ///
    /// Arguments:
    /// - new_K: New coupling strength (clamped to [0, 10])
    ///
    /// Returns:
    /// - old_K: Previous coupling strength
    /// - new_K: New coupling strength (after clamping)
    /// - predicted_r: Predicted order parameter after adjustment
    pub(super) async fn call_adjust_coupling(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling adjust_coupling tool call");

        // FAIL FAST: Check kuramoto provider
        let kuramoto = match &self.kuramoto_network {
            Some(k) => k,
            None => {
                error!("adjust_coupling: Kuramoto network not initialized");
                return JsonRpcResponse::error(
                    id,
                    error_codes::GWT_NOT_INITIALIZED,
                    "Kuramoto network not initialized - use with_gwt() constructor",
                );
            }
        };

        // Parse new_K (required)
        let new_k = match args.get("new_K").and_then(|v| v.as_f64()) {
            Some(k) => k,
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'new_K' parameter");
            }
        };

        // Acquire write lock (parking_lot RwLock)
        let mut kuramoto_guard = kuramoto.write();

        // Get old coupling strength
        let old_k = kuramoto_guard.coupling_strength();

        // Set new coupling strength (will be clamped internally to [0, 10])
        kuramoto_guard.set_coupling_strength(new_k);

        // Get the actual new K (after clamping)
        let actual_new_k = kuramoto_guard.coupling_strength();

        // Get current synchronization for prediction
        let current_r = kuramoto_guard.synchronization();

        // Simple prediction: higher K tends to increase r
        // This is a rough approximation based on Kuramoto dynamics
        let predicted_r = if actual_new_k > old_k {
            // Increasing K tends to increase r
            (current_r + 0.1 * (actual_new_k - old_k)).min(1.0)
        } else {
            // Decreasing K may decrease r
            (current_r - 0.05 * (old_k - actual_new_k)).max(0.0)
        };

        self.tool_result_with_pulse(
            id,
            json!({
                "old_K": old_k,
                "new_K": actual_new_k,
                "predicted_r": predicted_r,
                "current_r": current_r,
                "K_clamped": new_k != actual_new_k
            }),
        )
    }

    // ========== NORTH STAR TOOLS ==========
    // Real implementations are in handlers/north_star.rs (TASK-NORTHSTAR-001)
}
