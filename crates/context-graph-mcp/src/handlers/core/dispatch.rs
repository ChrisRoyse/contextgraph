//! Request dispatch logic for MCP handlers.
//!
//! Per PRD v6: MCP tools are accessed via tools/list and tools/call.
//! Direct method calls are NOT supported.

use tracing::debug;

use crate::protocol::{error_codes, methods, JsonRpcRequest, JsonRpcResponse};

use super::handlers::Handlers;

impl Handlers {
    /// Dispatch a request to the appropriate handler.
    ///
    /// Per PRD v6 Section 10, all tool access is via:
    /// - tools/list: List available tools
    /// - tools/call: Call a specific tool
    ///
    /// Direct method calls (memory/store, etc.) are NOT supported.
    pub async fn dispatch(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        debug!("Dispatching method: {}", request.method);

        match request.method.as_str() {
            // MCP lifecycle methods
            methods::INITIALIZE => self.handle_initialize(request.id).await,
            "notifications/initialized" => self.handle_initialized_notification(),
            methods::SHUTDOWN => self.handle_shutdown(request.id).await,

            // MCP tools protocol (PRD v6 Section 10)
            methods::TOOLS_LIST => self.handle_tools_list(request.id).await,
            methods::TOOLS_CALL => self.handle_tools_call(request.id, request.params).await,

            // Unknown method
            _ => JsonRpcResponse::error(
                request.id,
                error_codes::METHOD_NOT_FOUND,
                format!(
                    "Method not found: {}. Use tools/call for tool access.",
                    request.method
                ),
            ),
        }
    }
}
