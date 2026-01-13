//! Meta-learning tool handlers.
//!
//! TASK-MCP-P0-001: Wire Meta-UTL tools to MCP dispatch.

use tracing::{debug, error};

use crate::handlers::core::meta_utl_service::MetaLearningService;
use crate::handlers::meta_learning::{
    handle_get_meta_learning_log, handle_get_meta_learning_status,
    handle_trigger_lambda_recalibration, GetMetaLearningLogInput, GetMetaLearningStatusInput,
    TriggerRecalibrationInput,
};
use crate::handlers::Handlers;
use crate::protocol::{error_codes, JsonRpcId, JsonRpcResponse};

impl Handlers {
    /// call_get_meta_learning_status tool implementation.
    ///
    /// TASK-MCP-P0-001: Get current Meta-UTL self-correction status.
    pub(super) async fn call_get_meta_learning_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_meta_learning_status tool call");

        // Parse input - default to empty object if no args
        let input: GetMetaLearningStatusInput = match serde_json::from_value(args) {
            Ok(i) => i,
            Err(e) => {
                error!("get_meta_learning_status: Invalid arguments: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid arguments: {}", e),
                );
            }
        };

        // Create MetaLearningService on-demand
        let service = MetaLearningService::with_defaults();

        match handle_get_meta_learning_status(input, &service) {
            Ok(output) => match serde_json::to_value(&output) {
                Ok(value) => self.tool_result_with_pulse(id, value),
                Err(e) => {
                    error!("Failed to serialize meta_learning_status output: {}", e);
                    JsonRpcResponse::error(
                        id,
                        error_codes::INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    )
                }
            },
            Err(e) => {
                error!("get_meta_learning_status failed: {:?}", e);
                JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Meta-learning status error: {:?}", e),
                )
            }
        }
    }

    /// call_trigger_lambda_recalibration tool implementation.
    ///
    /// TASK-MCP-P0-001: Manually trigger lambda weight recalibration.
    pub(super) async fn call_trigger_lambda_recalibration(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling trigger_lambda_recalibration tool call");

        // Parse input
        let input: TriggerRecalibrationInput = match serde_json::from_value(args) {
            Ok(i) => i,
            Err(e) => {
                error!("trigger_lambda_recalibration: Invalid arguments: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid arguments: {}", e),
                );
            }
        };

        // Create mutable service
        let mut service = MetaLearningService::with_defaults();

        match handle_trigger_lambda_recalibration(input, &mut service) {
            Ok(output) => match serde_json::to_value(&output) {
                Ok(value) => self.tool_result_with_pulse(id, value),
                Err(e) => {
                    error!("Failed to serialize recalibration output: {}", e);
                    JsonRpcResponse::error(
                        id,
                        error_codes::INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    )
                }
            },
            Err(e) => {
                error!("trigger_lambda_recalibration failed: {:?}", e);
                JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Recalibration error: {:?}", e),
                )
            }
        }
    }

    /// call_get_meta_learning_log tool implementation.
    ///
    /// TASK-MCP-P0-001: Query meta-learning event log.
    pub(super) async fn call_get_meta_learning_log(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        debug!("Handling get_meta_learning_log tool call");

        // Parse input
        let input: GetMetaLearningLogInput = match serde_json::from_value(args) {
            Ok(i) => i,
            Err(e) => {
                error!("get_meta_learning_log: Invalid arguments: {}", e);
                return JsonRpcResponse::error(
                    id,
                    error_codes::INVALID_PARAMS,
                    format!("Invalid arguments: {}", e),
                );
            }
        };

        let service = MetaLearningService::with_defaults();

        match handle_get_meta_learning_log(input, &service) {
            Ok(output) => match serde_json::to_value(&output) {
                Ok(value) => self.tool_result_with_pulse(id, value),
                Err(e) => {
                    error!("Failed to serialize meta_learning_log output: {}", e);
                    JsonRpcResponse::error(
                        id,
                        error_codes::INTERNAL_ERROR,
                        format!("Serialization error: {}", e),
                    )
                }
            },
            Err(e) => {
                error!("get_meta_learning_log failed: {:?}", e);
                JsonRpcResponse::error(
                    id,
                    error_codes::INTERNAL_ERROR,
                    format!("Event log query error: {:?}", e),
                )
            }
        }
    }
}
