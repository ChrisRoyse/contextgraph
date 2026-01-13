# TASK-MCP-007: Implement SSE handler with keep-alive

```xml
<task_spec id="TASK-MCP-007" version="1.0">
<metadata>
  <title>Implement SSE handler with keep-alive</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>33</sequence>
  <implements><requirement_ref>REQ-MCP-007</requirement_ref></implements>
  <depends_on>TASK-32</depends_on> <!-- TASK-32 = TASK-MCP-006 (SSE transport types) -->
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
The SSE handler provides real-time streaming endpoint for MCP events.
It uses axum's SSE support with keep-alive pings.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/transport/sse.rs (from TASK-32)
</input_context_files>

<scope>
<in_scope>
- Implement create_sse_router() function
- Implement sse_handler() with event streaming
- Add keep-alive ping at configured interval
- Respect max_connection_duration
- Handle event buffer
</in_scope>
<out_of_scope>
- SSE types (TASK-32)
- Router integration (TASK-42)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/transport/sse.rs
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::Stream;

/// Create SSE router for MCP transport.
pub fn create_sse_router(state: AppState) -> Router<AppState>;

/// SSE endpoint handler.
async fn sse_handler(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>;
```
</signatures>
<constraints>
- Keep-alive MUST send ping every keepalive_interval
- Connection MUST close after max_connection_duration
- Events MUST be JSON serialized
- Stream MUST be infallible (use Result<Event, Infallible>)
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp sse_handler
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/transport/sse.rs (add handler)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp transport
```
</test_commands>
</task_spec>
```
