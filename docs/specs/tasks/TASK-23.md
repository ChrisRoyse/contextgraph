# TASK-DREAM-002: Implement NvmlGpuMonitor with thresholds

```xml
<task_spec id="TASK-DREAM-002" version="1.0">
<metadata>
  <title>Implement NvmlGpuMonitor with thresholds</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>23</sequence>
  <implements><requirement_ref>REQ-DREAM-002</requirement_ref></implements>
  <depends_on>TASK-22</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
NvmlGpuMonitor is the real NVML-based implementation of GpuMonitor.
It queries GPU utilization via NVML library and implements caching
to reduce syscall overhead. Constitution: AP-26 (no silent failures)
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/dream/gpu_monitor.rs (from TASK-DREAM-001)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.3)
</input_context_files>

<scope>
<in_scope>
- Implement NvmlGpuMonitor struct with nvml-wrapper
- Implement new() with NVML init and device discovery
- Implement get_utilization() with caching
- Implement GpuMonitor trait for NvmlGpuMonitor
- Handle multi-GPU systems (return MAX utilization)
- Implement utilization caching (100ms validity)
</in_scope>
<out_of_scope>
- GPU resource allocation
- Green Contexts (TASK-EMBED-001)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/dream/gpu_monitor.rs
use nvml_wrapper::Nvml;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Real GPU monitor using NVML.
///
/// # Fail-Fast Behavior (AP-26)
/// - Returns `Err(GpuMonitorError)` on any failure
/// - Does NOT return 0.0 as fallback
/// - Does NOT silently degrade
pub struct NvmlGpuMonitor {
    nvml: Arc<Nvml>,
    device_count: u32,
    /// Cached utilization (valid for 100ms)
    cached_utilization: Option<(f32, Instant)>,
    cache_duration: Duration,
}

impl NvmlGpuMonitor {
    /// Create a new GPU monitor with NVML backend.
    ///
    /// # Errors
    /// Returns error if NVML cannot be initialized or no GPUs found.
    /// Does NOT fall back to stub mode (AP-26 compliance).
    pub fn new() -> Result<Self, GpuMonitorError>;

    /// Get current GPU utilization as a fraction [0.0, 1.0].
    ///
    /// For multi-GPU systems, returns the MAXIMUM utilization.
    ///
    /// # Errors
    /// Returns error if utilization cannot be queried.
    /// Does NOT return 0.0 on failure (AP-26).
    pub fn get_utilization(&mut self) -> Result<f32, GpuMonitorError>;

    /// Check if GPU is eligible to start a dream (< 80% utilization).
    pub fn is_eligible_for_dream(&mut self) -> Result<bool, GpuMonitorError>;

    /// Check if dream should abort due to GPU budget exceeded (> 30%).
    pub fn should_abort_dream(&mut self) -> Result<bool, GpuMonitorError>;
}

impl GpuMonitor for NvmlGpuMonitor {
    fn get_utilization(&mut self) -> Result<f32, GpuMonitorError> {
        NvmlGpuMonitor::get_utilization(self)
    }

    fn is_eligible_for_dream(&mut self) -> Result<bool, GpuMonitorError> {
        NvmlGpuMonitor::is_eligible_for_dream(self)
    }

    fn should_abort_dream(&mut self) -> Result<bool, GpuMonitorError> {
        NvmlGpuMonitor::should_abort_dream(self)
    }
}
```
</signatures>
<constraints>
- MUST use nvml-wrapper crate (not raw FFI)
- Cache MUST expire after 100ms
- Multi-GPU MUST return MAX utilization
- All errors MUST propagate (no silent fallback)
- Utilization MUST be normalized to [0.0, 1.0]
</constraints>
<verification>
```bash
cargo test -p context-graph-core nvml_gpu_monitor
cargo test -p context-graph-core test_utilization_caching -- --ignored
```
</verification>
</definition_of_done>

<files_to_create>
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/dream/gpu_monitor.rs (add NvmlGpuMonitor)
- crates/context-graph-core/Cargo.toml (add nvml-wrapper dependency)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-core gpu_monitor
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Caching Logic

```rust
pub fn get_utilization(&mut self) -> Result<f32, GpuMonitorError> {
    // Check cache
    if let Some((cached, timestamp)) = &self.cached_utilization {
        if timestamp.elapsed() < self.cache_duration {
            return Ok(*cached);
        }
    }

    // Query all devices, take max
    let mut max_utilization: f32 = 0.0;
    for i in 0..self.device_count {
        let device = self.nvml.device_by_index(i)?;
        let utilization = device.utilization_rates()?;
        let gpu_util = utilization.gpu as f32 / 100.0;
        max_utilization = max_utilization.max(gpu_util);
    }

    // Update cache
    self.cached_utilization = Some((max_utilization, Instant::now()));
    Ok(max_utilization)
}
```

### Multi-GPU Rationale

Taking MAX utilization ensures:
- We don't start dreams when ANY GPU is busy
- We abort dreams when ANY GPU becomes busy
- Conservative approach prevents resource contention

### nvml-wrapper Dependency

```toml
[dependencies]
nvml-wrapper = "0.10"
```

This provides safe Rust bindings to NVML without manual FFI.
