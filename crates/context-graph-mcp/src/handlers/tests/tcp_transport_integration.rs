#![allow(clippy::field_reassign_with_default)]

//! TCP Transport Unit Tests
//!
//! TASK-INTEG-020: Unit tests for TCP transport configuration and protocol.
//!
//! These tests verify:
//! 1. Error code values and ranges
//! 2. JSON-RPC serialization for TCP
//! 3. NDJSON framing format
//! 4. Transport configuration validation

use serde_json::json;

use crate::protocol::{error_codes, JsonRpcRequest, JsonRpcResponse};

// ============================================================================
// TCP Transport Unit Tests
// ============================================================================

/// Test error code values are correct.
#[test]
fn test_tcp_error_codes_values() {
    assert_eq!(error_codes::TCP_BIND_FAILED, -32110);
    assert_eq!(error_codes::TCP_CONNECTION_ERROR, -32111);
    assert_eq!(error_codes::TCP_MAX_CONNECTIONS_REACHED, -32112);
    assert_eq!(error_codes::TCP_FRAME_ERROR, -32113);
    assert_eq!(error_codes::TCP_CLIENT_TIMEOUT, -32114);
}

/// Test error codes are in the reserved range.
#[test]
fn test_tcp_error_codes_in_range() {
    let codes = [
        error_codes::TCP_BIND_FAILED,
        error_codes::TCP_CONNECTION_ERROR,
        error_codes::TCP_MAX_CONNECTIONS_REACHED,
        error_codes::TCP_FRAME_ERROR,
        error_codes::TCP_CLIENT_TIMEOUT,
    ];

    for code in codes {
        assert!(
            (-32119..=-32110).contains(&code),
            "Error code {} should be in range -32119 to -32110",
            code
        );
    }
}

/// Test JsonRpcRequest serialization for TCP.
#[test]
fn test_jsonrpc_request_serialization() {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(crate::protocol::JsonRpcId::Number(1)),
        method: "initialize".to_string(),
        params: Some(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {}
        })),
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"method\":\"initialize\""));
}

/// Test JsonRpcResponse error construction.
#[test]
fn test_jsonrpc_error_response() {
    use crate::protocol::JsonRpcId;

    let response = JsonRpcResponse::error(
        Some(JsonRpcId::Number(1)),
        error_codes::TCP_CONNECTION_ERROR,
        "Connection lost",
    );

    assert!(response.error.is_some());
    let error = response.error.unwrap();
    assert_eq!(error.code, error_codes::TCP_CONNECTION_ERROR);
    assert_eq!(error.message, "Connection lost");
}

/// Test NDJSON format (newline-delimited JSON).
#[test]
fn test_ndjson_format() {
    let request1 = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(crate::protocol::JsonRpcId::Number(1)),
        method: "tools/list".to_string(),
        params: None,
    };

    let request2 = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(crate::protocol::JsonRpcId::Number(2)),
        method: "memory/search".to_string(),
        params: Some(json!({"query": "test"})),
    };

    // NDJSON: each message on its own line
    let json1 = serde_json::to_string(&request1).unwrap();
    let json2 = serde_json::to_string(&request2).unwrap();
    let ndjson = format!("{}\n{}\n", json1, json2);

    // Verify each line is valid JSON
    for line in ndjson.lines() {
        let parsed: JsonRpcRequest = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.jsonrpc, "2.0");
    }
}

/// Test transport mode configuration defaults.
#[test]
fn test_transport_config_defaults() {
    use context_graph_core::config::Config;

    let config = Config::default_config();

    assert_eq!(config.mcp.transport, "stdio");
    assert_eq!(config.mcp.bind_address, "127.0.0.1");
    assert_eq!(config.mcp.tcp_port, 3100);
    assert_eq!(config.mcp.max_connections, 32);
}

/// Test transport mode validation.
#[test]
fn test_transport_validation() {
    use context_graph_core::config::McpConfig;

    // Valid stdio
    let mut config = McpConfig::default();
    config.transport = "stdio".to_string();
    assert!(config.validate().is_ok());

    // Valid tcp
    config.transport = "tcp".to_string();
    assert!(config.validate().is_ok());

    // Invalid transport
    config.transport = "websocket".to_string();
    assert!(config.validate().is_err());
}

/// Test TCP port validation.
#[test]
fn test_tcp_port_validation() {
    use context_graph_core::config::McpConfig;

    let mut config = McpConfig::default();
    config.transport = "tcp".to_string();

    // Valid port
    config.tcp_port = 3100;
    assert!(config.validate().is_ok());

    // Port 0 is invalid for TCP
    config.tcp_port = 0;
    assert!(config.validate().is_err());

    // Maximum port
    config.tcp_port = 65535;
    assert!(config.validate().is_ok());
}

/// Test max_connections validation.
#[test]
fn test_max_connections_validation() {
    use context_graph_core::config::McpConfig;

    let mut config = McpConfig::default();
    config.transport = "tcp".to_string();

    // Valid max_connections
    config.max_connections = 32;
    assert!(config.validate().is_ok());

    // Zero is invalid for TCP
    config.max_connections = 0;
    assert!(config.validate().is_err());

    // Large value is valid
    config.max_connections = 1000;
    assert!(config.validate().is_ok());
}

/// Test bind address validation.
#[test]
fn test_bind_address_validation() {
    use context_graph_core::config::McpConfig;

    let mut config = McpConfig::default();
    config.transport = "tcp".to_string();

    // Valid addresses
    config.bind_address = "127.0.0.1".to_string();
    assert!(config.validate().is_ok());

    config.bind_address = "0.0.0.0".to_string();
    assert!(config.validate().is_ok());

    // Empty address is invalid for TCP
    config.bind_address = "".to_string();
    assert!(config.validate().is_err());

    // Whitespace-only is invalid
    config.bind_address = "   ".to_string();
    assert!(config.validate().is_err());
}
