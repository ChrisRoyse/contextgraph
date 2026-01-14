//! Tests for McpConfig validation.
//!
//! TASK-INTEG-017: Comprehensive tests for TCP transport configuration fields.
//! Tests cover all validation rules including transport type, TCP-specific fields,
//! and edge cases for FAIL FAST behavior.

use crate::config::McpConfig;

// ============================================================================
// TASK-INTEG-017: McpConfig Default Tests
// ============================================================================

#[test]
fn test_mcp_config_default_values() {
    let config = McpConfig::default();

    assert_eq!(config.transport, "stdio", "Default transport must be stdio");
    assert_eq!(
        config.max_payload_size, 10_485_760,
        "Default max_payload_size must be 10MB"
    );
    assert_eq!(config.request_timeout, 30, "Default request_timeout must be 30s");
    assert_eq!(
        config.bind_address, "127.0.0.1",
        "Default bind_address must be 127.0.0.1"
    );
    assert_eq!(config.tcp_port, 3100, "Default tcp_port must be 3100");
    assert_eq!(
        config.max_connections, 32,
        "Default max_connections must be 32"
    );
}

#[test]
fn test_mcp_config_default_validates() {
    let config = McpConfig::default();
    let result = config.validate();
    assert!(
        result.is_ok(),
        "Default McpConfig must validate: {:?}",
        result.err()
    );
}

// ============================================================================
// TASK-INTEG-017: Transport Type Validation Tests
// ============================================================================

#[test]
fn test_mcp_config_validates_stdio_transport() {
    let config = McpConfig {
        transport: "stdio".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_ok(), "stdio transport must validate");
}

#[test]
fn test_mcp_config_validates_tcp_transport() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "0.0.0.0".to_string(),
        tcp_port: 8080,
        max_connections: 64,
        ..Default::default()
    };
    assert!(config.validate().is_ok(), "tcp transport must validate");
}

#[test]
fn test_mcp_config_validates_transport_case_insensitive() {
    // Test uppercase
    let config_upper = McpConfig {
        transport: "STDIO".to_string(),
        ..Default::default()
    };
    assert!(
        config_upper.validate().is_ok(),
        "STDIO (uppercase) must validate"
    );

    // Test mixed case
    let config_mixed = McpConfig {
        transport: "TcP".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 3100,
        max_connections: 1,
        ..Default::default()
    };
    assert!(
        config_mixed.validate().is_ok(),
        "TcP (mixed case) must validate"
    );
}

#[test]
fn test_mcp_config_rejects_invalid_transport() {
    let config = McpConfig {
        transport: "http".to_string(),
        ..Default::default()
    };
    let result = config.validate();
    assert!(
        result.is_err(),
        "Invalid transport 'http' must fail validation"
    );

    let err_msg = result.unwrap_err().to_string();
    // TASK-42: Updated to include 'sse' transport
    assert!(
        err_msg.contains("transport must be 'stdio', 'tcp', or 'sse'"),
        "Error must explain valid transport types, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("http"),
        "Error must include the invalid value, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_rejects_empty_transport() {
    let config = McpConfig {
        transport: "".to_string(),
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err(), "Empty transport must fail validation");
}

#[test]
fn test_mcp_config_rejects_websocket_transport() {
    let config = McpConfig {
        transport: "websocket".to_string(),
        ..Default::default()
    };
    let result = config.validate();
    assert!(
        result.is_err(),
        "Websocket transport (not supported) must fail validation"
    );
}

// ============================================================================
// TASK-INTEG-017: max_payload_size Validation Tests
// ============================================================================

#[test]
fn test_mcp_config_rejects_zero_max_payload_size() {
    let config = McpConfig {
        max_payload_size: 0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err(), "max_payload_size=0 must fail validation");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("max_payload_size must be > 0"),
        "Error must explain constraint, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_accepts_minimum_max_payload_size() {
    let config = McpConfig {
        max_payload_size: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "max_payload_size=1 must validate"
    );
}

#[test]
fn test_mcp_config_accepts_large_max_payload_size() {
    let config = McpConfig {
        max_payload_size: 100 * 1024 * 1024, // 100MB
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "max_payload_size=100MB must validate"
    );
}

// ============================================================================
// TASK-INTEG-017: request_timeout Validation Tests
// ============================================================================

#[test]
fn test_mcp_config_rejects_zero_request_timeout() {
    let config = McpConfig {
        request_timeout: 0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err(), "request_timeout=0 must fail validation");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("request_timeout must be > 0"),
        "Error must explain constraint, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_accepts_minimum_request_timeout() {
    let config = McpConfig {
        request_timeout: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "request_timeout=1 must validate"
    );
}

#[test]
fn test_mcp_config_accepts_long_request_timeout() {
    let config = McpConfig {
        request_timeout: 3600, // 1 hour
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "request_timeout=3600 must validate"
    );
}

// ============================================================================
// TASK-INTEG-017: TCP-Specific Validation Tests (bind_address)
// ============================================================================

#[test]
fn test_mcp_config_tcp_rejects_empty_bind_address() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "".to_string(),
        tcp_port: 3100,
        max_connections: 32,
        ..Default::default()
    };
    let result = config.validate();
    assert!(
        result.is_err(),
        "TCP with empty bind_address must fail validation"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("bind_address must be non-empty"),
        "Error must explain constraint, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_tcp_rejects_whitespace_bind_address() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "   ".to_string(),
        tcp_port: 3100,
        max_connections: 32,
        ..Default::default()
    };
    let result = config.validate();
    assert!(
        result.is_err(),
        "TCP with whitespace-only bind_address must fail validation"
    );
}

#[test]
fn test_mcp_config_tcp_accepts_localhost_bind_address() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 3100,
        max_connections: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "TCP with 127.0.0.1 bind_address must validate"
    );
}

#[test]
fn test_mcp_config_tcp_accepts_any_bind_address() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "0.0.0.0".to_string(),
        tcp_port: 3100,
        max_connections: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "TCP with 0.0.0.0 bind_address must validate"
    );
}

#[test]
fn test_mcp_config_tcp_accepts_ipv6_bind_address() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "::1".to_string(),
        tcp_port: 3100,
        max_connections: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "TCP with ::1 (IPv6) bind_address must validate"
    );
}

#[test]
fn test_mcp_config_stdio_ignores_empty_bind_address() {
    // For stdio transport, bind_address is not validated
    let config = McpConfig {
        transport: "stdio".to_string(),
        bind_address: "".to_string(),
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "stdio transport ignores bind_address validation"
    );
}

// ============================================================================
// TASK-INTEG-017: TCP-Specific Validation Tests (tcp_port)
// ============================================================================

#[test]
fn test_mcp_config_tcp_rejects_zero_port() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 0,
        max_connections: 32,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err(), "TCP with port=0 must fail validation");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("tcp_port must be in range 1-65535"),
        "Error must explain valid range, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_tcp_accepts_minimum_port() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 1,
        max_connections: 1,
        ..Default::default()
    };
    assert!(config.validate().is_ok(), "TCP with port=1 must validate");
}

#[test]
fn test_mcp_config_tcp_accepts_maximum_port() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 65535,
        max_connections: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "TCP with port=65535 must validate"
    );
}

#[test]
fn test_mcp_config_tcp_accepts_common_ports() {
    for port in [80, 443, 3000, 3100, 8080, 8443, 9000] {
        let config = McpConfig {
            transport: "tcp".to_string(),
            bind_address: "127.0.0.1".to_string(),
            tcp_port: port,
            max_connections: 1,
            ..Default::default()
        };
        assert!(
            config.validate().is_ok(),
            "TCP with common port {} must validate",
            port
        );
    }
}

#[test]
fn test_mcp_config_stdio_ignores_zero_port() {
    // For stdio transport, tcp_port is not validated
    let config = McpConfig {
        transport: "stdio".to_string(),
        tcp_port: 0,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "stdio transport ignores tcp_port validation"
    );
}

// ============================================================================
// TASK-INTEG-017: TCP-Specific Validation Tests (max_connections)
// ============================================================================

#[test]
fn test_mcp_config_tcp_rejects_zero_max_connections() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 3100,
        max_connections: 0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(
        result.is_err(),
        "TCP with max_connections=0 must fail validation"
    );

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("max_connections must be > 0"),
        "Error must explain constraint, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_tcp_accepts_minimum_max_connections() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 3100,
        max_connections: 1,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "TCP with max_connections=1 must validate"
    );
}

#[test]
fn test_mcp_config_tcp_accepts_large_max_connections() {
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 3100,
        max_connections: 10000,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "TCP with max_connections=10000 must validate"
    );
}

#[test]
fn test_mcp_config_stdio_ignores_zero_max_connections() {
    // For stdio transport, max_connections is not validated
    let config = McpConfig {
        transport: "stdio".to_string(),
        max_connections: 0,
        ..Default::default()
    };
    assert!(
        config.validate().is_ok(),
        "stdio transport ignores max_connections validation"
    );
}

// ============================================================================
// TASK-INTEG-017: Serde Deserialization Tests
// ============================================================================

#[test]
fn test_mcp_config_serde_default_transport() {
    // Test that missing fields get defaults during deserialization
    let json = r#"{}"#;
    let config: McpConfig = serde_json::from_str(json).expect("Failed to deserialize");

    assert_eq!(config.transport, "stdio", "Missing transport should default to stdio");
    assert_eq!(
        config.max_payload_size, 10_485_760,
        "Missing max_payload_size should default to 10MB"
    );
}

#[test]
fn test_mcp_config_serde_partial_deserialization() {
    // Test partial JSON with only transport specified
    let json = r#"{"transport": "tcp"}"#;
    let config: McpConfig = serde_json::from_str(json).expect("Failed to deserialize");

    assert_eq!(config.transport, "tcp", "Transport should be tcp");
    assert_eq!(
        config.bind_address, "127.0.0.1",
        "Missing bind_address should default"
    );
    assert_eq!(config.tcp_port, 3100, "Missing tcp_port should default");
}

#[test]
fn test_mcp_config_serde_full_deserialization() {
    let json = r#"{
        "transport": "tcp",
        "max_payload_size": 1048576,
        "request_timeout": 60,
        "bind_address": "0.0.0.0",
        "tcp_port": 8080,
        "max_connections": 100
    }"#;
    let config: McpConfig = serde_json::from_str(json).expect("Failed to deserialize");

    assert_eq!(config.transport, "tcp");
    assert_eq!(config.max_payload_size, 1048576);
    assert_eq!(config.request_timeout, 60);
    assert_eq!(config.bind_address, "0.0.0.0");
    assert_eq!(config.tcp_port, 8080);
    assert_eq!(config.max_connections, 100);
}

#[test]
fn test_mcp_config_serde_serialization_roundtrip() {
    let original = McpConfig {
        transport: "tcp".to_string(),
        max_payload_size: 2_097_152,
        request_timeout: 45,
        bind_address: "192.168.1.1".to_string(),
        tcp_port: 9000,
        sse_port: 9001, // TASK-42
        max_connections: 64,
    };

    let json = serde_json::to_string(&original).expect("Failed to serialize");
    let deserialized: McpConfig = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.transport, original.transport);
    assert_eq!(deserialized.max_payload_size, original.max_payload_size);
    assert_eq!(deserialized.request_timeout, original.request_timeout);
    assert_eq!(deserialized.bind_address, original.bind_address);
    assert_eq!(deserialized.tcp_port, original.tcp_port);
    assert_eq!(deserialized.max_connections, original.max_connections);
}

// ============================================================================
// TASK-INTEG-017: TOML Deserialization Tests
// ============================================================================

#[test]
fn test_mcp_config_toml_empty() {
    let toml_str = "";
    // Empty TOML should result in defaults
    let config: McpConfig = toml::from_str(toml_str).expect("Failed to deserialize empty TOML");
    assert_eq!(config.transport, "stdio");
}

#[test]
fn test_mcp_config_toml_tcp_config() {
    let toml_str = r#"
transport = "tcp"
bind_address = "0.0.0.0"
tcp_port = 8080
max_connections = 128
"#;
    let config: McpConfig = toml::from_str(toml_str).expect("Failed to deserialize");
    assert_eq!(config.transport, "tcp");
    assert_eq!(config.bind_address, "0.0.0.0");
    assert_eq!(config.tcp_port, 8080);
    assert_eq!(config.max_connections, 128);
}

// ============================================================================
// TASK-INTEG-017: Edge Cases and Boundary Tests
// ============================================================================

#[test]
fn test_mcp_config_multiple_validation_errors_first_wins() {
    // When multiple fields are invalid, the first check should fail
    let config = McpConfig {
        transport: "invalid".to_string(),
        max_payload_size: 0,
        request_timeout: 0,
        bind_address: "127.0.0.1".to_string(),
        tcp_port: 3100,
        sse_port: 3101, // TASK-42
        max_connections: 32,
    };
    let result = config.validate();
    assert!(result.is_err(), "Multiple invalid fields must fail");

    // Transport is checked first
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("transport"),
        "First error should be about transport, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_tcp_all_fields_must_be_valid() {
    // For TCP, all TCP-specific fields must be valid together
    let config = McpConfig {
        transport: "tcp".to_string(),
        bind_address: "".to_string(), // Invalid
        tcp_port: 0,                  // Invalid
        max_connections: 0,           // Invalid
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err(), "TCP with all invalid fields must fail");

    // bind_address is checked first for TCP
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("bind_address"),
        "First TCP error should be about bind_address, got: {}",
        err_msg
    );
}

#[test]
fn test_mcp_config_clone_and_debug() {
    let config = McpConfig::default();
    let cloned = config.clone();
    assert_eq!(cloned.transport, config.transport);

    // Debug should not panic
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("McpConfig"));
    assert!(debug_str.contains("stdio"));
}
