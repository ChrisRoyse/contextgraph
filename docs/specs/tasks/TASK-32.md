# TASK-MCP-006: Add SSE transport types

```xml
<task_spec id="TASK-MCP-006" version="1.0">
<metadata>
  <title>Add SSE transport types</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>32</sequence>
  <implements><requirement_ref>REQ-MCP-006</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
SSE (Server-Sent Events) transport enables real-time streaming of MCP responses.
This task defines the types and configuration for SSE transport.
PRD Section 5.1: "JSON-RPC 2.0, stdio/SSE"
</context>

<input_context_files>
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 4.4)
</input_context_files>

<scope>
<in_scope>
- Create sse.rs in transport module
- Define SseConfig struct (keepalive, max_duration, buffer_size)
- Define McpSseEvent enum (Response, Error, Notification, Ping)
- Define JsonRpcError struct
</in_scope>
<out_of_scope>
- SSE handler implementation (TASK-MCP-007)
- Router integration (TASK-MCP-016)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/transport/sse.rs
use std::time::Duration;

/// SSE transport configuration.
#[derive(Debug, Clone)]
pub struct SseConfig {
    /// Keep-alive interval (default: 15s)
    pub keepalive_interval: Duration,

    /// Maximum connection duration (default: 1 hour)
    pub max_connection_duration: Duration,

    /// Event buffer size (default: 100)
    pub buffer_size: usize,
}

impl Default for SseConfig {
    fn default() -> Self {
        Self {
            keepalive_interval: Duration::from_secs(15),
            max_connection_duration: Duration::from_secs(3600),
            buffer_size: 100,
        }
    }
}

/// SSE event types for MCP communication.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum McpSseEvent {
    /// JSON-RPC response
    Response { id: serde_json::Value, result: serde_json::Value },

    /// JSON-RPC error
    Error { id: serde_json::Value, error: JsonRpcError },

    /// Notification (no id)
    Notification { method: String, params: serde_json::Value },

    /// Keep-alive ping
    Ping { timestamp: u64 },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<serde_json::Value>,
}
```
</signatures>
<constraints>
- keepalive_interval MUST default to 15 seconds
- Event types MUST match JSON-RPC 2.0 spec
- Ping MUST include Unix timestamp
</constraints>
<verification>
```bash
cargo check -p context-graph-mcp
cargo test -p context-graph-mcp sse_types
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/transport/sse.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/transport/mod.rs (add sse module)
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-mcp
```
</test_commands>
</task_spec>
```
