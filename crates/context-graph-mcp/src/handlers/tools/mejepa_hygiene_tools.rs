//! ME-JEPA Phase 6 hygiene MCP handlers.

use context_graph_mejepa_hygiene::{
    mcp_gc_run, mcp_quota_status, mcp_witness_compress, HygieneMcpRequest,
    WitnessCompressMcpRequest,
};

use crate::handlers::tools::helpers::ToolErrorKind;
use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};
use crate::tools::names as tool_names;

impl Handlers {
    pub(crate) async fn call_mejepa_gc_run(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: HygieneMcpRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_GC_RUN
                    ),
                );
            }
        };
        match mcp_gc_run(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_quota_status(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: HygieneMcpRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_QUOTA_STATUS
                    ),
                );
            }
        };
        match mcp_quota_status(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code),
                )
            }
        }
    }

    pub(crate) async fn call_mejepa_witness_compress(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let request: WitnessCompressMcpRequest = match serde_json::from_value(args) {
            Ok(value) => value,
            Err(err) => {
                return self.tool_error_typed(
                    id,
                    ToolErrorKind::Validation,
                    &format!(
                        "{} schema validation failed: {err}",
                        tool_names::MEJEPA_WITNESS_COMPRESS
                    ),
                );
            }
        };
        match mcp_witness_compress(request) {
            Ok(value) => self.tool_result(id, value),
            Err(err) => {
                err.log_context(file!());
                self.tool_error_typed(
                    id,
                    ToolErrorKind::Execution,
                    &format!("{}: {err}", err.code),
                )
            }
        }
    }
}
