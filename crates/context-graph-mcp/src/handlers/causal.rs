//! Causal Inference MCP Handlers
//!
//! TASK-CAUSAL-001: MCP tool handlers for causal inference system.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See `omni_infer` tool requirements (line 539).
//!
//! ## Tools
//!
//! - omni_infer: Perform omni-directional causal inference

use serde_json::json;
use tracing::{debug, error};
use uuid::Uuid;

use context_graph_core::causal::{InferenceDirection, OmniInfer};

use crate::protocol::{JsonRpcId, JsonRpcResponse};

use super::Handlers;

impl Handlers {
    /// omni_infer tool implementation.
    ///
    /// TASK-CAUSAL-001: Perform omni-directional causal inference.
    /// FAIL FAST on invalid parameters.
    ///
    /// Arguments:
    /// - source: UUID of source node
    /// - target: UUID of target node (optional for bridge/abduction)
    /// - direction: Inference direction (forward, backward, bidirectional, bridge, abduction)
    ///
    /// Returns:
    /// - results: Array of inference results
    /// - direction: Direction used
    /// - source: Source UUID
    /// - target: Target UUID (if provided)
    pub(super) async fn call_omni_infer(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling omni_infer tool call");

        // Parse source UUID (required)
        let source_str = match args.get("source").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return self.tool_error_with_pulse(id, "Missing required 'source' parameter");
            }
        };

        let source = match Uuid::parse_str(source_str) {
            Ok(id) => id,
            Err(e) => {
                return self.tool_error_with_pulse(
                    id,
                    &format!("Invalid UUID format for source: {}", e),
                );
            }
        };

        // Parse target UUID (optional)
        let target = if let Some(target_str) = args.get("target").and_then(|v| v.as_str()) {
            match Uuid::parse_str(target_str) {
                Ok(id) => Some(id),
                Err(e) => {
                    return self.tool_error_with_pulse(
                        id,
                        &format!("Invalid UUID format for target: {}", e),
                    );
                }
            }
        } else {
            None
        };

        // Parse direction (default: forward)
        let direction_str = args
            .get("direction")
            .and_then(|v| v.as_str())
            .unwrap_or("forward");

        let direction = match direction_str.parse::<InferenceDirection>() {
            Ok(d) => d,
            Err(_) => {
                return self.tool_error_with_pulse(
                    id,
                    &format!(
                        "Invalid direction '{}'. Must be one of: forward, backward, bidirectional, bridge, abduction",
                        direction_str
                    ),
                );
            }
        };

        // TASK-CAUSAL-001: Validate target parameter based on direction
        // Per tool schema: target is REQUIRED for forward, backward, bidirectional
        // target is OPTIONAL for bridge, abduction
        match direction {
            InferenceDirection::Forward
            | InferenceDirection::Backward
            | InferenceDirection::Bidirectional => {
                if target.is_none() {
                    return self.tool_error_with_pulse(
                        id,
                        &format!(
                            "Missing required 'target' parameter for direction '{}'. \
                             Target is only optional for bridge and abduction directions.",
                            direction_str
                        ),
                    );
                }
            }
            InferenceDirection::Bridge | InferenceDirection::Abduction => {
                // Target is optional for these directions
            }
        }

        // Create inference engine and perform inference
        let infer = OmniInfer::new();

        match infer.infer(source, target, direction) {
            Ok(results) => {
                // Convert results to JSON
                let results_json: Vec<_> = results
                    .iter()
                    .map(|r| {
                        json!({
                            "direction": r.direction.as_str(),
                            "source": r.source.to_string(),
                            "target": r.target.to_string(),
                            "strength": r.strength,
                            "confidence": r.confidence,
                            "path": r.path.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                            "path_length": r.path_length(),
                            "is_direct": r.is_direct(),
                            "is_high_confidence": r.is_high_confidence(),
                            "is_strong": r.is_strong(),
                            "explanation": r.explanation
                        })
                    })
                    .collect();

                self.tool_result_with_pulse(
                    id,
                    json!({
                        "results": results_json,
                        "result_count": results_json.len(),
                        "direction": direction.as_str(),
                        "direction_description": direction.description(),
                        "source": source.to_string(),
                        "target": target.map(|t| t.to_string()),
                        "inference_config": {
                            "min_confidence": infer.min_confidence,
                            "max_path_length": infer.max_path_length,
                            "include_indirect": infer.include_indirect
                        }
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "omni_infer: Inference failed");
                self.tool_error_with_pulse(id, &format!("Causal inference failed: {}", e))
            }
        }
    }
}
