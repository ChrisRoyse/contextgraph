//! Tool dispatch logic for MCP tool calls.

use serde_json::json;
use tracing::debug;

use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};
use crate::tools::{get_tool_definitions, tool_names};

use super::super::Handlers;

impl Handlers {
    /// Handle tools/list request.
    ///
    /// Returns all available MCP tools with their schemas.
    pub(crate) async fn handle_tools_list(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        debug!("Handling tools/list request");

        let tools = get_tool_definitions();
        JsonRpcResponse::success(id, json!({ "tools": tools }))
    }

    /// Handle tools/call request.
    ///
    /// Dispatches to the appropriate tool handler and returns MCP-compliant result.
    pub(crate) async fn handle_tools_call(
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

        let raw_tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(n) => n,
            None => {
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    "Missing 'name' parameter in tools/call",
                );
            }
        };

        // TASK-MCP-P1-001: Resolve alias to canonical name
        let tool_name = crate::tools::aliases::resolve_alias(raw_tool_name);

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        debug!(
            "Calling tool: {} with arguments: {:?}{}",
            tool_name,
            arguments,
            if raw_tool_name != tool_name {
                format!(" (resolved from alias '{}')", raw_tool_name)
            } else {
                String::new()
            }
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
            // TASK-UTL-P1-001: UTL delta S/C computation
            tool_names::COMPUTE_DELTA_SC => {
                self.handle_gwt_compute_delta_sc(id, Some(arguments)).await
            }
            // TASK-ATC-001: Adaptive Threshold Calibration tools
            tool_names::GET_THRESHOLD_STATUS => {
                self.handle_get_threshold_status(id, Some(arguments)).await
            }
            tool_names::GET_CALIBRATION_METRICS => {
                self.handle_get_calibration_metrics(id, Some(arguments))
                    .await
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
            tool_names::GET_NEUROMODULATION_STATE => self.call_get_neuromodulation_state(id).await,
            tool_names::ADJUST_NEUROMODULATOR => {
                self.call_adjust_neuromodulator(id, arguments).await
            }
            // TASK-STEERING-001: Steering tools
            tool_names::GET_STEERING_FEEDBACK => self.call_get_steering_feedback(id).await,
            // TASK-CAUSAL-001: Causal inference tools
            tool_names::OMNI_INFER => self.call_omni_infer(id, arguments).await,
            // TELEO-H1 to TELEO-H5: Teleological tools
            tool_names::SEARCH_TELEOLOGICAL => self.call_search_teleological(id, arguments).await,
            tool_names::COMPUTE_TELEOLOGICAL_VECTOR => {
                self.call_compute_teleological_vector(id, arguments).await
            }
            tool_names::FUSE_EMBEDDINGS => self.call_fuse_embeddings(id, arguments).await,
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
            tool_names::GET_ALIGNMENT_DRIFT => self.call_get_alignment_drift(id, arguments).await,
            tool_names::TRIGGER_DRIFT_CORRECTION => {
                self.call_trigger_drift_correction(id, arguments).await
            }
            tool_names::GET_PRUNING_CANDIDATES => {
                self.call_get_pruning_candidates(id, arguments).await
            }
            tool_names::TRIGGER_CONSOLIDATION => {
                self.call_trigger_consolidation(id, arguments).await
            }
            tool_names::DISCOVER_SUB_GOALS => self.call_discover_sub_goals(id, arguments).await,
            tool_names::GET_AUTONOMOUS_STATUS => {
                self.call_get_autonomous_status(id, arguments).await
            }
            // TASK-MCP-P0-001: Meta-learning self-correction tools
            tool_names::GET_META_LEARNING_STATUS => {
                self.call_get_meta_learning_status(id, arguments).await
            }
            tool_names::TRIGGER_LAMBDA_RECALIBRATION => {
                self.call_trigger_lambda_recalibration(id, arguments).await
            }
            tool_names::GET_META_LEARNING_LOG => {
                self.call_get_meta_learning_log(id, arguments).await
            }
            _ => JsonRpcResponse::error(
                id,
                error_codes::TOOL_NOT_FOUND,
                format!("Unknown tool: {}", tool_name),
            ),
        }
    }
}
