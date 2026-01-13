# TASK-DREAM-004: Integrate KuramotoStepper with MCP server

```xml
<task_spec id="TASK-DREAM-004" version="1.0">
<metadata>
  <title>Integrate KuramotoStepper with MCP server</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>25</sequence>
  <implements><requirement_ref>REQ-DREAM-004</requirement_ref></implements>
  <depends_on>TASK-12</depends_on>
  <estimated_hours>3</estimated_hours>
</metadata>

<context>
The MCP server must own and manage the KuramotoStepper lifecycle.
Stepper must start when server starts and stop gracefully on shutdown.
Constitution: REQ-GWT-004 (fail on stepper failure)
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs (from TASK-GWT-003)
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/server.rs (existing server)
</input_context_files>

<scope>
<in_scope>
- Add KuramotoStepper field to McpServer
- Initialize stepper in server new()
- Start stepper in server start()
- Stop stepper in server shutdown()
- Expose order_parameter() via server method
- Handle startup failure per REQ-GWT-004
</in_scope>
<out_of_scope>
- KuramotoStepper implementation (TASK-GWT-003)
- IC event emission (TASK-DREAM-005)
- MCP tools for Kuramoto state (TASK-MCP-013)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/server.rs
use crate::handlers::kuramoto_stepper::{KuramotoStepper, KuramotoStepperConfig};

pub struct McpServer {
    // ... existing fields ...
    kuramoto_stepper: KuramotoStepper,
}

impl McpServer {
    /// Create a new MCP server.
    ///
    /// Initializes KuramotoStepper but does not start it.
    pub fn new(config: McpServerConfig) -> Self;

    /// Start the MCP server.
    ///
    /// # Constitution
    /// REQ-GWT-004: MUST fail if KuramotoStepper cannot start.
    pub async fn start(&mut self) -> Result<(), McpServerError>;

    /// Shutdown the MCP server gracefully.
    ///
    /// Stops KuramotoStepper with timeout.
    pub async fn shutdown(&mut self, timeout: Duration) -> Result<(), McpServerError>;

    /// Get current Kuramoto order parameter.
    pub async fn order_parameter(&self) -> f32;
}
```
</signatures>
<constraints>
- Server MUST fail to start if stepper fails (REQ-GWT-004)
- Shutdown MUST stop stepper with timeout
- Stepper MUST be initialized before start()
- order_parameter() MUST be accessible for MCP tools
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp server_stepper_lifecycle
cargo test -p context-graph-mcp test_server_fails_on_stepper_failure
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/server.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp server
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Server Lifecycle Integration

```rust
impl McpServer {
    pub fn new(config: McpServerConfig) -> Self {
        let stepper_config = KuramotoStepperConfig::default();
        let kuramoto_stepper = KuramotoStepper::new(stepper_config);

        Self {
            // ... other fields ...
            kuramoto_stepper,
        }
    }

    pub async fn start(&mut self) -> Result<(), McpServerError> {
        // Start Kuramoto stepper FIRST
        self.kuramoto_stepper.start()
            .map_err(|e| McpServerError::StepperStartFailed(e.to_string()))?;

        // Continue with other startup...
        Ok(())
    }

    pub async fn shutdown(&mut self, timeout: Duration) -> Result<(), McpServerError> {
        // Stop stepper with half the timeout
        let stepper_timeout = timeout / 2;
        if let Err(e) = self.kuramoto_stepper.stop(stepper_timeout).await {
            tracing::warn!("Stepper stop warning: {:?}", e);
            // Don't fail shutdown on timeout warning
        }

        // Continue with other shutdown...
        Ok(())
    }
}
```

### Fail-Fast Startup

Per REQ-GWT-004, the server must not proceed if the stepper fails:
```rust
pub async fn start(&mut self) -> Result<(), McpServerError> {
    self.kuramoto_stepper.start()
        .map_err(|e| {
            tracing::error!("KuramotoStepper failed to start: {:?}", e);
            McpServerError::StepperStartFailed(e.to_string())
        })?;
    // Only continue if stepper started successfully
}
```

### Graceful Shutdown

Shutdown should be best-effort:
- Try to stop stepper gracefully
- Warn on timeout but don't fail
- Continue with other cleanup
