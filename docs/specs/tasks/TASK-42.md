# TASK-MCP-016: SSE integration with MCP router

```xml
<task_spec id="TASK-MCP-016" version="1.0">
<metadata>
  <title>SSE integration with MCP router</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>42</sequence>
  <implements><requirement_ref>REQ-MCP-016</requirement_ref></implements>
  <depends_on>TASK-33</depends_on> <!-- TASK-33 = TASK-MCP-007 (SSE handler with keep-alive) -->
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
The SSE transport must be integrated into the main MCP router alongside stdio.
This enables clients to choose their preferred transport.
</context>

<scope>
<in_scope>
- Add SSE routes to main MCP router
- Configure AppState with SSE event channel
- Enable transport selection via config
- Add /mcp/sse endpoint
</in_scope>
<out_of_scope>
- SSE handler implementation (TASK-33)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/router.rs
use crate::transport::sse::{create_sse_router, SseConfig};

/// Create the main MCP router with all transports.
pub fn create_mcp_router(config: McpRouterConfig) -> Router {
    let mut router = Router::new();

    // Add SSE routes if enabled
    if config.sse_enabled {
        let sse_router = create_sse_router(config.state.clone());
        router = router.merge(sse_router);
    }

    // Add health check
    router = router.route("/health", get(health_handler));

    router.with_state(config.state)
}

#[derive(Debug, Clone)]
pub struct McpRouterConfig {
    pub state: AppState,
    pub sse_enabled: bool,
    pub sse_config: SseConfig,
}
```
</signatures>
<constraints>
- SSE MUST be optional (configurable)
- /mcp/sse MUST be the SSE endpoint
- Router MUST include health check
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp router
cargo test -p context-graph-mcp test_sse_endpoint_available
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/router.rs (add SSE integration)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp router
```
</test_commands>
</task_spec>
```
