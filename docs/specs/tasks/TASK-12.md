# TASK-GWT-003: Implement KuramotoStepper lifecycle

```xml
<task_spec id="TASK-GWT-003" version="1.0">
<metadata>
  <title>Implement KuramotoStepper lifecycle</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>12</sequence>
  <implements><requirement_ref>REQ-GWT-003</requirement_ref></implements>
  <depends_on>TASK-11</depends_on>
  <estimated_hours>4</estimated_hours>
</metadata>

<context>
KuramotoStepper is an async wrapper that runs KuramotoNetwork at 100Hz (10ms intervals).
It manages the background task lifecycle (start/stop) and provides async access to
order parameter. Constitution: GWT-006 requires continuous coherence calculation.
</context>

<input_context_files>
- /home/cabdru/contextgraph/crates/context-graph-core/src/layers/coherence/network.rs (from TASK-GWT-002)
- /home/cabdru/contextgraph/docs/specs/technical/TECH-REMEDIATION-MASTER.md (section 5.1)
</input_context_files>

<scope>
<in_scope>
- Create stepper.rs in coherence module (or MCP handlers)
- Implement KuramotoStepperConfig with step interval
- Implement KuramotoStepper with tokio task management
- Implement start() to spawn background task
- Implement stop(timeout) with graceful shutdown
- Implement order_parameter() async getter
- Implement is_running() status check
- Define KuramotoStepperError enum
</in_scope>
<out_of_scope>
- Integration with MCP server (TASK-DREAM-004)
- IC event emission (TASK-DREAM-005)
</out_of_scope>
</scope>

<definition_of_done>
<signatures>
```rust
// crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs
use context_graph_core::layers::coherence::network::KuramotoNetwork;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// Configuration for Kuramoto stepper.
#[derive(Debug, Clone)]
pub struct KuramotoStepperConfig {
    /// Step interval (default: 10ms = 100Hz)
    pub step_interval: Duration,

    /// Coupling strength
    pub coupling: f32,
}

impl Default for KuramotoStepperConfig {
    fn default() -> Self;
}

/// Kuramoto network stepper for continuous phase updates.
///
/// Constitution: GWT-006
pub struct KuramotoStepper {
    config: KuramotoStepperConfig,
    network: Arc<Mutex<KuramotoNetwork>>,
    running: Arc<AtomicBool>,
    task_handle: Option<JoinHandle<()>>,
}

impl KuramotoStepper {
    /// Create a new Kuramoto stepper.
    pub fn new(config: KuramotoStepperConfig) -> Self;

    /// Start the stepper background task.
    ///
    /// # Returns
    /// `Ok(())` if started, `Err` if already running.
    ///
    /// # Constitution
    /// REQ-GWT-004: System MUST fail to start if stepper cannot start.
    pub fn start(&mut self) -> Result<(), KuramotoStepperError>;

    /// Stop the stepper background task.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for graceful shutdown.
    ///
    /// # Constitution
    /// REQ-GWT-005: Warn on timeout, do not block.
    pub async fn stop(&mut self, timeout: Duration) -> Result<(), KuramotoStepperError>;

    /// Get the current order parameter r(t).
    pub async fn order_parameter(&self) -> f32;

    /// Check if the stepper is running.
    pub fn is_running(&self) -> bool;
}

#[derive(Debug, thiserror::Error)]
pub enum KuramotoStepperError {
    #[error("Stepper is already running")]
    AlreadyRunning,

    #[error("Stepper task panicked")]
    TaskPanicked,

    #[error("Stop operation timed out")]
    StopTimeout,
}
```
</signatures>
<constraints>
- start() MUST fail if already running
- stop() MUST respect timeout and log warning if exceeded
- Task MUST NOT block on network.step()
- Running flag MUST use AtomicBool for thread safety
- Network MUST use tokio::sync::Mutex for async access
</constraints>
<verification>
```bash
cargo test -p context-graph-mcp kuramoto_stepper
cargo test -p context-graph-mcp test_stepper_start_stop
```
</verification>
</definition_of_done>

<files_to_create>
- crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs
</files_to_create>

<files_to_modify>
- crates/context-graph-mcp/src/handlers/mod.rs (add kuramoto_stepper module)
</files_to_modify>

<test_commands>
```bash
cargo test -p context-graph-mcp kuramoto
```
</test_commands>
</task_spec>
```

## Implementation Notes

### Background Task Pattern

```rust
let handle = tokio::spawn(async move {
    let mut ticker = tokio::time::interval(interval);
    let dt = interval.as_secs_f32();

    while running.load(Ordering::SeqCst) {
        ticker.tick().await;
        let mut net = network.lock().await;
        net.step(dt);
    }
});
```

### Graceful Shutdown

```rust
pub async fn stop(&mut self, timeout: Duration) -> Result<(), KuramotoStepperError> {
    self.running.store(false, Ordering::SeqCst);

    if let Some(handle) = self.task_handle.take() {
        match tokio::time::timeout(timeout, handle).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(KuramotoStepperError::TaskPanicked),
            Err(_) => {
                tracing::warn!("Stepper stop timeout after {:?}", timeout);
                Err(KuramotoStepperError::StopTimeout)
            }
        }
    } else {
        Ok(())
    }
}
```

### Thread Safety

- `AtomicBool` for running flag (lock-free)
- `tokio::sync::Mutex` for network (async-safe)
- `Arc` for shared ownership between main code and task
