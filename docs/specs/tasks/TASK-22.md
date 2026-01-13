# TASK-DREAM-001: Implement GpuMonitor trait and error types

```xml
<task_spec id="TASK-DREAM-001" version="1.0">
<metadata>
  <title>Implement GpuMonitor trait and error types</title>
  <status>ready</status>
  <layer>integration</layer>
  <sequence>22</sequence>
  <implements><requirement_ref>REQ-DREAM-001</requirement_ref></implements>
  <depends_on></depends_on>
  <estimated_hours>2</estimated_hours>
</metadata>

<context>
GpuMonitor trait abstracts GPU utilization monitoring for dream triggering.
This allows mocking in tests while enabling real NVML integration in production.
Constitution: dream.trigger.gpu, dream.constraints.gpu
</context>

<input_context_files>
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 3.3)
</input_context_files>

<scope>
<in_scope>
- Create gpu_monitor.rs in dream module
- Define GpuMonitorError enum with all error variants
- Define GPU utilization thresholds as constants
- Define GpuMonitor trait with methods
- Implement trait for future NvmlGpuMonitor
</in_scope>
<out_of_scope>
- NvmlGpuMonitor implementation (TASK-DREAM-002)
- TriggerManager integration (TASK-IDENTITY-003)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-core/src/dream/gpu_monitor.rs

/// GPU monitoring error types.
/// Constitution: AP-26 (fail-fast, no silent failures)
#[derive(Debug, thiserror::Error)]
pub enum GpuMonitorError {
    #[error("NVML initialization failed: {0}")]
    NvmlInitFailed(String),

    #[error("No GPU devices found")]
    NoDevices,

    #[error("Failed to get device {index}: {message}")]
    DeviceAccessFailed { index: u32, message: String },

    #[error("Failed to query GPU utilization: {0}")]
    UtilizationQueryFailed(String),

    #[error("NVML not available (drivers not installed)")]
    NvmlNotAvailable,

    #[error("GPU monitoring disabled")]
    Disabled,
}

/// GPU utilization thresholds per Constitution.
pub mod thresholds {
    /// Dream ELIGIBILITY threshold - dreams can START when GPU < 80%
    /// Constitution: dream.trigger.gpu = "<80%"
    pub const GPU_ELIGIBILITY_THRESHOLD: f32 = 0.80;

    /// Dream BUDGET threshold - dreams must ABORT if GPU > 30%
    /// Constitution: dream.constraints.gpu = "<30%"
    pub const GPU_BUDGET_THRESHOLD: f32 = 0.30;
}

/// Trait for GPU monitoring (allows mocking in tests).
pub trait GpuMonitor: Send + Sync {
    /// Get current GPU utilization as fraction [0.0, 1.0].
    fn get_utilization(&mut self) -> Result<f32, GpuMonitorError>;

    /// Check if GPU is eligible to start a dream (< 80%).
    fn is_eligible_for_dream(&mut self) -> Result<bool, GpuMonitorError>;

    /// Check if dream should abort due to GPU budget exceeded (> 30%).
    fn should_abort_dream(&mut self) -> Result<bool, GpuMonitorError>;
}
```
</signatures>
<constraints>
- All methods MUST return Result (no silent failures)
- Trait MUST be Send + Sync for thread safety
- Thresholds MUST match constitution values exactly
- Error messages MUST be descriptive
</constraints>
<verification>
```bash
cargo check -p context-graph-core
cargo test -p context-graph-core gpu_monitor
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-core/src/dream/gpu_monitor.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-core/src/dream/mod.rs (add gpu_monitor module)
</files_to_modify>

<test_commands>
```bash
cargo check -p context-graph-core
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Two Thresholds Explained

**ELIGIBILITY (80%)**: When to START a dream
- GPU utilization < 80% = "idle enough" to begin dream
- This ensures dreams don't compete with active workloads

**BUDGET (30%)**: When to ABORT a dream
- GPU utilization > 30% = "too busy" to continue dream
- This ensures dreams yield to incoming workloads

### Mock Implementation for Tests

```rust
pub struct MockGpuMonitor {
    utilization: f32,
}

impl GpuMonitor for MockGpuMonitor {
    fn get_utilization(&mut self) -> Result<f32, GpuMonitorError> {
        Ok(self.utilization)
    }

    fn is_eligible_for_dream(&mut self) -> Result<bool, GpuMonitorError> {
        Ok(self.utilization < thresholds::GPU_ELIGIBILITY_THRESHOLD)
    }

    fn should_abort_dream(&mut self) -> Result<bool, GpuMonitorError> {
        Ok(self.utilization > thresholds::GPU_BUDGET_THRESHOLD)
    }
}
```

### Error Granularity

Different error variants enable different handling:
- `NvmlNotAvailable`: Can fall back to stub mode
- `NoDevices`: Fatal in GPU-required deployments
- `UtilizationQueryFailed`: Transient, can retry
