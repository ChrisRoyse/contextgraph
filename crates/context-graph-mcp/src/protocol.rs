//! MCP JSON-RPC protocol types.

use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Box<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Box<JsonRpcError>>,
}

/// JSON-RPC ID (can be string, number, or null per JSON-RPC 2.0 spec).
///
/// The `Null` variant handles `"id": null` in requests, which is a valid
/// (if unusual) request ID per the spec — distinct from absent `"id"` (notification).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Number(i64),
    Null,
}

/// JSON-RPC error object.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// Create a success response.
    pub fn success(id: Option<JsonRpcId>, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(Box::new(result)),
            error: None,
        }
    }

    /// Create an error response.
    pub fn error(id: Option<JsonRpcId>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(Box::new(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            })),
        }
    }
}

/// JSON-RPC error codes.
///
/// Standard JSON-RPC 2.0 codes plus Context Graph specific codes.
pub mod error_codes {
    // Standard JSON-RPC 2.0 error codes
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Context Graph specific error codes (-32001 to -32099)
    #[allow(dead_code)] // Protocol-defined; used in tests, reserved for future handlers
    pub const FEATURE_DISABLED: i32 = -32001;
    pub const NODE_NOT_FOUND: i32 = -32002;
    pub const STORAGE_ERROR: i32 = -32004;
    #[allow(dead_code)] // Protocol-defined; used in tests, reserved for future handlers
    pub const EMBEDDING_ERROR: i32 = -32005;
    pub const TOOL_NOT_FOUND: i32 = -32006;
    pub const LAYER_TIMEOUT: i32 = -32007;

    /// Insufficient memories for topic detection (< min_cluster_size)
    #[allow(dead_code)] // D-L14: used in tests only
    pub const INSUFFICIENT_MEMORIES: i32 = -32021;

    // TCP Transport error codes (-32110 to -32119) - TASK-INTEG-018
    /// TCP bind failed - address/port unavailable or permission denied
    #[allow(dead_code)] // D-L15: protocol-defined, used in tests, pending full TCP transport
    pub const TCP_BIND_FAILED: i32 = -32110;
    /// TCP connection error - stream read/write failed, client disconnected
    #[allow(dead_code)] // D-L15: protocol-defined, used in tests, pending full TCP transport
    pub const TCP_CONNECTION_ERROR: i32 = -32111;
    /// Maximum concurrent TCP connections reached
    #[allow(dead_code)] // D-L15: protocol-defined, used in tests, pending full TCP transport
    pub const TCP_MAX_CONNECTIONS_REACHED: i32 = -32112;
    /// TCP frame error - invalid NDJSON framing, message too large
    #[allow(dead_code)] // D-L15: protocol-defined, used in tests, pending full TCP transport
    pub const TCP_FRAME_ERROR: i32 = -32113;
    /// TCP client timeout - request processing exceeded request_timeout
    pub const TCP_CLIENT_TIMEOUT: i32 = -32114;
}

/// MCP method names.
pub mod methods {
    // MCP lifecycle methods
    pub const INITIALIZE: &str = "initialize";
    pub const SHUTDOWN: &str = "shutdown";

    // MCP tools protocol methods
    pub const TOOLS_LIST: &str = "tools/list";
    pub const TOOLS_CALL: &str = "tools/call";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(JsonRpcId::Number(1)));
    }

    #[test]
    fn test_success_response() {
        let resp = JsonRpcResponse::success(
            Some(JsonRpcId::Number(1)),
            serde_json::json!({"status": "ok"}),
        );
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[test]
    fn test_error_response() {
        let resp = JsonRpcResponse::error(
            Some(JsonRpcId::String("req-123".to_string())),
            error_codes::METHOD_NOT_FOUND,
            "Method not found",
        );
        assert!(resp.result.is_none());
        assert_eq!(
            resp.error.as_ref().unwrap().code,
            error_codes::METHOD_NOT_FOUND
        );
    }
}
