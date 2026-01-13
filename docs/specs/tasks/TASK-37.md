# TASK-MCP-011: Implement get_gpu_status tool

```xml
<task_spec id="TASK-MCP-011" version="1.0">
<metadata>
  <title>Implement get_gpu_status tool</title>
  <status>ready</status>
  <layer>surface</layer>
  <sequence>37</sequence>
  <implements><requirement_ref>REQ-MCP-011</requirement_ref></implements>
  <depends_on>TASK-23</depends_on> <!-- TASK-23 = TASK-DREAM-002 (NvmlGpuMonitor with thresholds) -->
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
The get_gpu_status tool exposes GPU utilization and dream eligibility status.
Uses NvmlGpuMonitor for real metrics.
</context>

<scope>
<in_scope>
- Define input/output schemas
- Implement handler querying GpuMonitor
- Return utilization, eligibility, budget status
</in_scope>
<out_of_scope>
- GpuMonitor implementation (TASK-23)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/tools/schemas/gpu.rs
#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct GetGpuStatusInput {}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct GetGpuStatusOutput {
    pub utilization: f32,
    pub eligible_for_dream: bool,
    pub budget_exceeded: bool,
    pub device_count: u32,
}

pub async fn handle_get_gpu_status(
    gpu_monitor: &mut impl GpuMonitor,
) -> Result<GetGpuStatusOutput, McpError>;
```
</signatures>
<constraints>
- utilization MUST be real from NVML
- MUST handle GPU errors gracefully
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp gpu_tool
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/tools/schemas/gpu.rs
- crates/context-graph-mcp/src/tools/handlers/gpu.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/tools/schemas/mod.rs
- crates/context-graph-mcp/src/tools/handlers/mod.rs
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp gpu
```
</test_commands>
</task_spec>
```
