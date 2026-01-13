# TASK-EMBED-001: Implement Green Contexts auto-enable

```xml
<task_spec id="TASK-EMBED-001" version="1.0">
<metadata>
  <title>Implement Green Contexts auto-enable</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>13</sequence>
  <implements><requirement_ref>REQ-EMBED-001</requirement_ref></implements>
  <depends_on>TASK-04</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
Green Contexts is an RTX 5090+ feature (compute capability 12.0+) that enables
GPU partitioning. Constitution specifies 70% inference / 30% background partitioning.
Auto-enable should detect GPU capability and gracefully degrade on older GPUs.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-cuda/src/safe/device.rs (from TASK-ARCH-004)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 6.4)
</input_context_files>

<scope>
<in_scope>
- Create green_contexts.rs in context-graph-cuda
- Implement GreenContextsConfig with partition settings
- Implement should_enable_green_contexts() capability check
- Implement GreenContexts struct with enable/disable
- Add graceful degradation for older GPUs (not fatal)
</in_scope>
<out_of_scope>
- CUDA FFI (already in TASK-ARCH-002)
- Token pruning (TASK-EMBED-002, TASK-EMBED-003)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-cuda/src/context/green_contexts.rs
use crate::safe::device::GpuDevice;
use crate::error::GpuError;

/// Green Contexts configuration for RTX 5090+.
///
/// Constitution: stack.gpu.target = "RTX 5090"
/// Partition: 70% inference / 30% background
#[derive(Debug, Clone)]
pub struct GreenContextsConfig {
    /// Compute capability required (12.0 for RTX 5090)
    pub min_compute_capability: (u32, u32),

    /// Inference partition percentage (0.70)
    pub inference_partition: f32,

    /// Background partition percentage (0.30)
    pub background_partition: f32,
}

impl Default for GreenContextsConfig {
    fn default() -> Self {
        Self {
            min_compute_capability: (12, 0),
            inference_partition: 0.70,
            background_partition: 0.30,
        }
    }
}

/// Check if Green Contexts should be auto-enabled.
///
/// Returns `true` if compute_capability >= 12.0
/// Gracefully degrades on older GPUs (not fatal).
pub fn should_enable_green_contexts(device: &GpuDevice) -> bool;

/// Green Contexts manager for GPU partitioning.
pub struct GreenContexts {
    config: GreenContextsConfig,
    enabled: bool,
}

impl GreenContexts {
    /// Create Green Contexts manager.
    ///
    /// Auto-enables if GPU supports it, otherwise disabled.
    pub fn new(device: &GpuDevice, config: GreenContextsConfig) -> Self;

    /// Check if Green Contexts is enabled.
    pub fn is_enabled(&self) -> bool;

    /// Get inference partition context (if enabled).
    pub fn inference_context(&self) -> Option<GreenContext>;

    /// Get background partition context (if enabled).
    pub fn background_context(&self) -> Option<GreenContext>;
}

/// Represents a Green Context partition.
pub struct GreenContext {
    partition_id: u32,
    percentage: f32,
}
```
</signatures>
<constraints>
- MUST NOT fail on older GPUs (graceful degradation)
- MUST require compute capability >= 12.0
- Partition percentages MUST sum to <= 1.0
- MUST log info message about enable/disable status
</constraints>
<verification>
```bash
cargo test -p context-graph-cuda green_contexts
cargo test -p context-graph-cuda test_graceful_degradation
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-cuda/src/context/mod.rs
- crates/context-graph-cuda/src/context/green_contexts.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-cuda/src/lib.rs (add context module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-cuda
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Capability Detection

```rust
pub fn should_enable_green_contexts(device: &GpuDevice) -> bool {
    let (major, minor) = device.compute_capability();
    let config = GreenContextsConfig::default();
    let (req_major, req_minor) = config.min_compute_capability;

    if major > req_major || (major == req_major && minor >= req_minor) {
        tracing::info!(
            "Green Contexts enabled: compute {}.{} >= {}.{}",
            major, minor, req_major, req_minor
        );
        true
    } else {
        tracing::info!(
            "Green Contexts not available: compute {}.{} < {}.{}",
            major, minor, req_major, req_minor
        );
        false
    }
}
```

### Graceful Degradation

When Green Contexts is not available:
1. Log informational message
2. Return `is_enabled() = false`
3. `inference_context()` and `background_context()` return `None`
4. Callers should handle `None` and fall back to default GPU behavior

This is NOT an error condition - older GPUs work fine without partitioning.
