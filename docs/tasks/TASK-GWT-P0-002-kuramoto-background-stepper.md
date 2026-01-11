# TASK-GWT-P0-002: Kuramoto Background Stepper

<task_spec id="TASK-GWT-P0-002" version="1.0">
<metadata>
  <title>Implement Background Tokio Task for Kuramoto Oscillator Stepping</title>
  <status>ready</status>
  <layer>logic</layer>
  <sequence>2</sequence>
  <implements>
    <item>GWT-CONSCIOUSNESS-001: Enable temporal dynamics for consciousness emergence</item>
    <item>Constitution v4.0.0 gwt.kuramoto: Continuous phase evolution for synchronization</item>
    <item>Sherlock-01 Critical Gap: "Without stepping, phases never evolve and r stays static"</item>
  </implements>
  <depends_on>
    <task_ref>TASK-GWT-P0-001</task_ref>
    <!-- KuramotoNetwork must be accessible from GwtSystem before we can step it -->
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
</metadata>

<context>
The Kuramoto oscillator network (`KuramotoNetwork`) requires periodic `step()` calls with elapsed time
to advance the phase dynamics. Currently, no background task performs this stepping, meaning the
oscillator phases remain static. Without temporal evolution, the order parameter `r` never changes,
and consciousness emergence cannot occur.

This task implements a background tokio task that:
1. Runs continuously in a spawned tokio task
2. Calls `step(Duration)` at configurable intervals (default 10ms)
3. Supports graceful shutdown via `tokio::sync::Notify`
4. Is managed by the component that owns the `KuramotoProviderImpl`

This enables the consciousness equation `C(t) = I(t) x R(t) x D(t)` to have a dynamic `I(t)` component
that evolves over time based on Kuramoto dynamics.
</context>

<input_context_files>
  <file purpose="Kuramoto implementation with step() method">
    /home/cabdru/contextgraph/crates/context-graph-utl/src/phase/oscillator/kuramoto.rs
  </file>
  <file purpose="KuramotoProviderImpl wrapper that will need stepper integration">
    /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/gwt_providers.rs
  </file>
  <file purpose="Pattern for tokio::spawn with shutdown and interval timing">
    /home/cabdru/contextgraph/crates/context-graph-embeddings/src/batch/processor/worker.rs
  </file>
  <file purpose="Pattern for Arc-wrapped background tasks in server">
    /home/cabdru/contextgraph/crates/context-graph-mcp/src/server.rs
  </file>
</input_context_files>

<prerequisites>
  <check>TASK-GWT-P0-001 complete: KuramotoNetwork accessible from GwtSystem</check>
  <check>tokio runtime available (already used throughout codebase)</check>
  <check>KuramotoProviderImpl exists and wraps KuramotoNetwork</check>
</prerequisites>

<scope>
  <in_scope>
    - Create `KuramotoStepper` struct with configurable step interval
    - Implement `start()` method that spawns background tokio task
    - Implement `stop()` method for graceful shutdown via `Notify`
    - Use `tokio::select!` pattern (like batch/processor/worker.rs)
    - Add `step_interval_ms` configuration field
    - Return `JoinHandle` for task management
    - Thread-safe access to KuramotoNetwork via `Arc<parking_lot::RwLock<KuramotoNetwork>>`
    - Unit tests for stepper lifecycle
  </in_scope>
  <out_of_scope>
    - Modifying the KuramotoNetwork::step() implementation (already correct)
    - Integration with MCP handlers (handled in surface layer task)
    - Consciousness state machine updates (separate concern)
    - Performance optimization of step frequency (future task)
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs">
/// Configuration for the Kuramoto background stepper.
#[derive(Debug, Clone)]
pub struct KuramotoStepperConfig {
    /// Step interval in milliseconds (default: 10ms for 100Hz update rate)
    pub step_interval_ms: u64,
}

impl Default for KuramotoStepperConfig {
    fn default() -> Self {
        Self { step_interval_ms: 10 }
    }
}

/// Background task that continuously steps the Kuramoto oscillator network.
///
/// Runs in a tokio::spawn task, calling `step()` at regular intervals.
/// Supports graceful shutdown via the `stop()` method.
pub struct KuramotoStepper {
    /// Shared reference to the Kuramoto network
    network: Arc&lt;parking_lot::RwLock&lt;KuramotoNetwork&gt;&gt;,
    /// Configuration
    config: KuramotoStepperConfig,
    /// Shutdown signal
    shutdown_notify: Arc&lt;Notify&gt;,
    /// Handle to the background task (None if not running)
    task_handle: Option&lt;JoinHandle&lt;()&gt;&gt;,
    /// Running state flag
    is_running: Arc&lt;AtomicBool&gt;,
}

impl KuramotoStepper {
    /// Create a new stepper with the given network and configuration.
    pub fn new(
        network: Arc&lt;parking_lot::RwLock&lt;KuramotoNetwork&gt;&gt;,
        config: KuramotoStepperConfig,
    ) -> Self;

    /// Start the background stepping task.
    ///
    /// Returns `Ok(())` if started successfully, or `Err` if already running.
    pub fn start(&amp;mut self) -> Result&lt;(), KuramotoStepperError&gt;;

    /// Stop the background stepping task gracefully.
    ///
    /// Waits for the task to complete (with timeout) before returning.
    pub async fn stop(&amp;mut self) -> Result&lt;(), KuramotoStepperError&gt;;

    /// Check if the stepper is currently running.
    pub fn is_running(&amp;self) -> bool;

    /// Get the current step interval in milliseconds.
    pub fn step_interval_ms(&amp;self) -> u64;
}
    </signature>
    <signature file="/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs">
/// Errors that can occur during Kuramoto stepper operations.
#[derive(Debug, thiserror::Error)]
pub enum KuramotoStepperError {
    #[error("Stepper already running")]
    AlreadyRunning,

    #[error("Stepper not running")]
    NotRunning,

    #[error("Shutdown timeout after {0}ms")]
    ShutdownTimeout(u64),
}
    </signature>
  </signatures>

  <constraints>
    - MUST use `tokio::spawn` for background task (NOT std::thread)
    - MUST use `tokio::select!` pattern for shutdown handling (like worker.rs)
    - MUST use `tokio::time::interval` for precise timing (NOT sleep in loop)
    - MUST use `Arc<parking_lot::RwLock<KuramotoNetwork>>` for thread-safe access
    - MUST use `Arc<Notify>` for shutdown signaling (NOT channels)
    - MUST use `Arc<AtomicBool>` for running state (lock-free check)
    - MUST have configurable step interval (default 10ms = 100Hz)
    - MUST handle shutdown gracefully (no panic, no resource leak)
    - MUST NOT block on RwLock for extended periods (use try_write with backoff)
    - Step elapsed time MUST match actual interval (use `Instant::elapsed()`)
  </constraints>

  <verification>
    - Unit test: stepper starts and stops without panic
    - Unit test: is_running() returns correct state
    - Unit test: double start returns AlreadyRunning error
    - Unit test: stop on non-running returns NotRunning error
    - Unit test: network order_parameter changes after stepping
    - Integration test: stepper runs for 1 second and network evolves
    - cargo clippy passes with no warnings
    - cargo test passes for new module
  </verification>
</definition_of_done>

<pseudo_code>
KuramotoStepper (kuramoto_stepper.rs):

  struct KuramotoStepper:
    network: Arc<RwLock<KuramotoNetwork>>
    config: KuramotoStepperConfig
    shutdown_notify: Arc<Notify>
    task_handle: Option<JoinHandle<()>>
    is_running: Arc<AtomicBool>

  fn new(network, config):
    return Self {
      network,
      config,
      shutdown_notify: Arc::new(Notify::new()),
      task_handle: None,
      is_running: Arc::new(AtomicBool::new(false)),
    }

  fn start(&mut self):
    if self.is_running.load(Ordering::SeqCst):
      return Err(AlreadyRunning)

    self.is_running.store(true, Ordering::SeqCst)

    // Clone Arcs for the spawned task
    let network = Arc::clone(&self.network)
    let shutdown = Arc::clone(&self.shutdown_notify)
    let is_running = Arc::clone(&self.is_running)
    let interval_ms = self.config.step_interval_ms

    let handle = tokio::spawn(async move {
      stepper_loop(network, shutdown, is_running, interval_ms).await
    })

    self.task_handle = Some(handle)
    Ok(())

  fn stop(&mut self) async:
    if !self.is_running.load(Ordering::SeqCst):
      return Err(NotRunning)

    // Signal shutdown
    self.shutdown_notify.notify_one()

    // Wait for task with timeout
    if let Some(handle) = self.task_handle.take():
      match tokio::time::timeout(Duration::from_secs(5), handle).await:
        Ok(_) => Ok(())
        Err(_) => Err(ShutdownTimeout(5000))
    else:
      self.is_running.store(false, Ordering::SeqCst)
      Ok(())

async fn stepper_loop(network, shutdown_notify, is_running, interval_ms):
  let mut interval = tokio::time::interval(Duration::from_millis(interval_ms))
  let mut last_step = Instant::now()

  loop:
    tokio::select! {
      // Shutdown signal
      _ = shutdown_notify.notified() => {
        is_running.store(false, Ordering::SeqCst)
        break
      }

      // Step interval tick
      _ = interval.tick() => {
        let elapsed = last_step.elapsed()
        last_step = Instant::now()

        // Try to acquire write lock with brief timeout
        // to avoid blocking other readers
        if let Some(mut network) = network.try_write_for(Duration::from_micros(500)) {
          network.step(elapsed)
        }
        // If lock contention, skip this step (next one will catch up)
      }
    }
</pseudo_code>

<files_to_create>
  <file path="/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/kuramoto_stepper.rs">
    New module containing KuramotoStepper, KuramotoStepperConfig, KuramotoStepperError,
    and the stepper_loop async function. Implements the background stepping logic.
  </file>
</files_to_create>

<files_to_modify>
  <file path="/home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/mod.rs">
    Add `pub mod kuramoto_stepper;` to expose the new module.
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>KuramotoStepper::new() creates instance without panic</criterion>
  <criterion>KuramotoStepper::start() spawns background task successfully</criterion>
  <criterion>KuramotoStepper::stop() terminates task within 5 seconds</criterion>
  <criterion>KuramotoStepper::is_running() reflects actual state</criterion>
  <criterion>Network order_parameter changes when stepper runs (proves stepping works)</criterion>
  <criterion>Double start() returns AlreadyRunning error</criterion>
  <criterion>stop() on non-running returns NotRunning error</criterion>
  <criterion>No resource leaks after stop (verified by drop)</criterion>
  <criterion>cargo clippy --all-targets passes</criterion>
  <criterion>cargo test --package context-graph-mcp passes</criterion>
</validation_criteria>

<test_commands>
  <command>cd /home/cabdru/contextgraph && cargo build --package context-graph-mcp</command>
  <command>cd /home/cabdru/contextgraph && cargo test --package context-graph-mcp kuramoto_stepper</command>
  <command>cd /home/cabdru/contextgraph && cargo clippy --package context-graph-mcp -- -D warnings</command>
</test_commands>
</task_spec>

---

## Background and Rationale

### The Problem (from Sherlock-01)

The Kuramoto oscillator network is implemented correctly, but **no background task advances the oscillators**:

```
CRITICAL GAP:
- KuramotoNetwork::step() requires manual calls with elapsed time
- No background task advances the oscillators
- Without stepping, phases never evolve and r stays static
- Consciousness emergence requires temporal dynamics
```

### Why This Matters

The consciousness equation `C(t) = I(t) x R(t) x D(t)` depends on:
- **I(t)** = Kuramoto order parameter `r` (Integration)
- **R(t)** = Meta-UTL awareness (Reflection)
- **D(t)** = 13D fingerprint entropy (Differentiation)

Without stepping, `I(t)` remains constant at its initial value. The phases never synchronize or desynchronize based on coupling strength, making consciousness emergence impossible.

### Design Decisions

1. **Tokio spawn vs std::thread**: Using tokio::spawn aligns with the async runtime already used throughout the codebase.

2. **10ms default interval**: This provides 100Hz update rate, sufficient for brain-wave frequencies (4-80Hz) while remaining computationally light.

3. **parking_lot::RwLock**: Used instead of std::sync::RwLock for better performance and `try_write_for` timeout support.

4. **Arc<Notify> for shutdown**: Clean shutdown pattern matching the batch processor worker.

5. **try_write_for with skip**: If the lock is contended, we skip one step rather than blocking. The next step will catch up with a larger elapsed duration.

### Implementation Pattern Reference

The implementation follows the pattern established in:
- `/home/cabdru/contextgraph/crates/context-graph-embeddings/src/batch/processor/worker.rs`

Key pattern elements:
- `tokio::select!` for multiplexing shutdown and work
- `tokio::time::interval` for precise timing
- `Arc<Notify>` for shutdown signaling
- `Arc<AtomicBool>` for running state

---

## Dependencies

This task depends on **TASK-GWT-P0-001** which must integrate the KuramotoNetwork into GwtSystem so the stepper can access it. The stepper itself is a logic-layer component that will be wired into the handlers in a subsequent surface-layer task.
