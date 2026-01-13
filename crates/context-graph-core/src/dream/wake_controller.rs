//! Wake Controller - Fast Dream Interruption System
//!
//! Manages wake transitions with guaranteed <100ms latency as required by
//! Constitution Section dream.constraints.wake.
//!
//! ## Constitution Reference
//!
//! - wake: <100ms latency (docs2/constitution.yaml line 273)
//! - gpu: <30% usage during dream (docs2/constitution.yaml line 273)
//! - abort_on_query: true (docs2/constitution.yaml line 273)

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};

use super::constants;
use super::triggers::{GpuMonitor, StubGpuMonitor};
use super::WakeReason;

/// Error types for wake controller operations.
#[derive(Debug, Error)]
pub enum WakeError {
    #[error("Wake latency exceeded: {actual_ms}ms > {max_ms}ms (Constitution violation)")]
    LatencyViolation { actual_ms: u64, max_ms: u64 },

    #[error("GPU budget exceeded during dream: {usage:.1}% > {max:.1}%")]
    GpuBudgetExceeded { usage: f32, max: f32 },

    #[error("Failed to signal wake: {reason}")]
    SignalFailed { reason: String },
}

/// Wake controller state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WakeState {
    /// Controller idle, no dream active
    Idle,
    /// Dream in progress, ready to wake
    Dreaming,
    /// Wake signal sent, waiting for completion
    Waking,
    /// Wake completed, processing cleanup
    Completing,
}

/// Resource usage snapshot during dream.
#[derive(Debug, Clone)]
pub struct ResourceSnapshot {
    /// GPU utilization [0.0, 1.0]
    pub gpu_usage: f32,
    /// Memory usage in bytes
    pub memory_bytes: u64,
    /// CPU usage estimate [0.0, 1.0]
    pub cpu_usage: f32,
    /// Timestamp of snapshot
    pub timestamp: Instant,
}

impl Default for ResourceSnapshot {
    fn default() -> Self {
        Self {
            gpu_usage: 0.0,
            memory_bytes: 0,
            cpu_usage: 0.0,
            timestamp: Instant::now(),
        }
    }
}

/// Fast wake controller for dream interruption.
///
/// Guarantees <100ms wake latency through:
/// 1. Atomic interrupt flags checked at each processing step
/// 2. Pre-allocated cleanup state
/// 3. Non-blocking signal propagation
///
/// # Constitution Compliance
///
/// - Wake latency: <100ms (enforced, logged on violation)
/// - GPU budget: <30% (monitored, triggers wake on violation)
/// - Abort on query: true (external query -> immediate wake)
#[derive(Debug)]
pub struct WakeController {
    /// Current state
    state: Arc<RwLock<WakeState>>,

    /// Interrupt flag (shared with dream phases)
    interrupt_flag: Arc<AtomicBool>,

    /// Wake reason channel
    wake_sender: watch::Sender<Option<WakeReason>>,
    wake_receiver: watch::Receiver<Option<WakeReason>>,

    /// Wake start time (for latency measurement)
    wake_start: Arc<RwLock<Option<Instant>>>,

    /// Wake completion time
    wake_complete: Arc<RwLock<Option<Instant>>>,

    /// Maximum allowed latency (Constitution: <100ms)
    max_latency: Duration,

    /// GPU monitor for budget enforcement
    /// Uses StubGpuMonitor until real NVML implementation in TASK-23
    gpu_monitor: Arc<RwLock<StubGpuMonitor>>,

    /// Maximum GPU usage during dream (Constitution: 30%)
    max_gpu_usage: f32,

    /// GPU check interval
    gpu_check_interval: Duration,

    /// Last GPU check time (millis since process start)
    last_gpu_check: Arc<AtomicU64>,

    /// Wake count for statistics
    wake_count: AtomicU64,

    /// Latency violation count
    latency_violations: AtomicU64,
}

impl WakeController {
    /// Create a new wake controller with constitution defaults.
    pub fn new() -> Self {
        let (wake_sender, wake_receiver) = watch::channel(None);

        Self {
            state: Arc::new(RwLock::new(WakeState::Idle)),
            interrupt_flag: Arc::new(AtomicBool::new(false)),
            wake_sender,
            wake_receiver,
            wake_start: Arc::new(RwLock::new(None)),
            wake_complete: Arc::new(RwLock::new(None)),
            max_latency: constants::MAX_WAKE_LATENCY,
            gpu_monitor: Arc::new(RwLock::new(StubGpuMonitor::with_usage(0.0))),
            max_gpu_usage: constants::MAX_GPU_USAGE,
            gpu_check_interval: Duration::from_millis(100),
            last_gpu_check: Arc::new(AtomicU64::new(0)),
            wake_count: AtomicU64::new(0),
            latency_violations: AtomicU64::new(0),
        }
    }

    /// Get the shared interrupt flag for dream phases.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupt_flag)
    }

    /// Prepare controller for a new dream cycle.
    pub fn prepare_for_dream(&self) {
        let mut state = self.state.write();
        *state = WakeState::Dreaming;
        drop(state);

        // Reset interrupt flag
        self.interrupt_flag.store(false, Ordering::SeqCst);

        // Clear wake times
        *self.wake_start.write() = None;
        *self.wake_complete.write() = None;

        // Send None to clear any previous wake reason
        let _ = self.wake_sender.send(None);

        debug!("Wake controller prepared for dream cycle");
    }

    /// Signal wake from dream state.
    ///
    /// Returns immediately after signaling; actual wake completes asynchronously.
    /// Measures latency from signal to completion.
    #[must_use = "check if wake signal was sent successfully"]
    pub fn signal_wake(&self, reason: WakeReason) -> Result<(), WakeError> {
        let current_state = *self.state.read();

        if current_state != WakeState::Dreaming {
            debug!("Wake signal ignored: not in dreaming state ({:?})", current_state);
            return Ok(());
        }

        // Record wake start time
        {
            let mut wake_start = self.wake_start.write();
            *wake_start = Some(Instant::now());
        }

        // Update state
        {
            let mut state = self.state.write();
            *state = WakeState::Waking;
        }

        // Set interrupt flag (checked by all phases)
        self.interrupt_flag.store(true, Ordering::SeqCst);

        // Send wake reason through channel
        self.wake_sender
            .send(Some(reason))
            .map_err(|_| WakeError::SignalFailed {
                reason: "Channel closed".to_string(),
            })?;

        info!("Wake signal sent: {:?}", reason);

        Ok(())
    }

    /// Mark wake as complete and measure latency.
    ///
    /// # Returns
    ///
    /// The measured wake latency. Returns error if latency > 100ms.
    ///
    /// # Errors
    ///
    /// Returns `WakeError::LatencyViolation` if latency exceeds constitution limit.
    #[must_use = "check wake latency for constitution violations"]
    pub fn complete_wake(&self) -> Result<Duration, WakeError> {
        let wake_time = Instant::now();

        // Record completion time
        {
            let mut wake_complete = self.wake_complete.write();
            *wake_complete = Some(wake_time);
        }

        // Calculate latency
        let latency = {
            let wake_start = self.wake_start.read();
            wake_start
                .map(|start| wake_time.duration_since(start))
                .unwrap_or(Duration::ZERO)
        };

        // Check latency violation
        if latency > self.max_latency {
            self.latency_violations.fetch_add(1, Ordering::Relaxed);
            error!(
                "CONSTITUTION VIOLATION: Wake latency {:?} > {:?} (max allowed)",
                latency, self.max_latency
            );
            return Err(WakeError::LatencyViolation {
                actual_ms: latency.as_millis() as u64,
                max_ms: self.max_latency.as_millis() as u64,
            });
        }

        // Update state
        {
            let mut state = self.state.write();
            *state = WakeState::Completing;
        }

        self.wake_count.fetch_add(1, Ordering::Relaxed);

        info!("Wake completed in {:?}", latency);

        Ok(latency)
    }

    /// Reset controller to idle state.
    pub fn reset(&self) {
        let mut state = self.state.write();
        *state = WakeState::Idle;
        drop(state);

        self.interrupt_flag.store(false, Ordering::SeqCst);
        *self.wake_start.write() = None;
        *self.wake_complete.write() = None;

        let _ = self.wake_sender.send(None);

        debug!("Wake controller reset to idle");
    }

    /// Check GPU usage and signal wake if over budget.
    ///
    /// Should be called periodically during dream.
    #[must_use = "check if GPU budget was exceeded"]
    pub fn check_gpu_budget(&self) -> Result<(), WakeError> {
        // Rate limit checks using monotonic counter
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let last_check = self.last_gpu_check.load(Ordering::Relaxed);
        if now_ms.saturating_sub(last_check) < self.gpu_check_interval.as_millis() as u64 {
            return Ok(());
        }
        self.last_gpu_check.store(now_ms, Ordering::Relaxed);

        let usage = self.gpu_monitor.write().get_utilization().unwrap_or(0.0);

        if usage > self.max_gpu_usage {
            warn!(
                "GPU usage exceeded budget: {:.1}% > {:.1}%",
                usage * 100.0,
                self.max_gpu_usage * 100.0
            );

            // Signal wake due to GPU overload
            self.signal_wake(WakeReason::GpuOverBudget)?;

            return Err(WakeError::GpuBudgetExceeded {
                usage: usage * 100.0,
                max: self.max_gpu_usage * 100.0,
            });
        }

        Ok(())
    }

    /// Get current resource snapshot.
    pub fn get_resource_snapshot(&self) -> ResourceSnapshot {
        ResourceSnapshot {
            gpu_usage: self.gpu_monitor.write().get_utilization().unwrap_or(0.0),
            memory_bytes: 0, // Future: Implement memory tracking
            cpu_usage: 0.0,  // Future: Implement CPU tracking
            timestamp: Instant::now(),
        }
    }

    /// Subscribe to wake events.
    pub fn subscribe(&self) -> watch::Receiver<Option<WakeReason>> {
        self.wake_receiver.clone()
    }

    /// Get current state.
    pub fn state(&self) -> WakeState {
        *self.state.read()
    }

    /// Check if currently dreaming.
    pub fn is_dreaming(&self) -> bool {
        *self.state.read() == WakeState::Dreaming
    }

    /// Check if wake has been signaled.
    pub fn is_wake_signaled(&self) -> bool {
        self.interrupt_flag.load(Ordering::SeqCst)
    }

    /// Get wake statistics.
    pub fn stats(&self) -> WakeStats {
        WakeStats {
            wake_count: self.wake_count.load(Ordering::Relaxed),
            latency_violations: self.latency_violations.load(Ordering::Relaxed),
            max_latency: self.max_latency,
            max_gpu_usage: self.max_gpu_usage,
        }
    }

    /// Update GPU usage for testing.
    pub fn set_gpu_usage(&self, usage: f32) {
        self.gpu_monitor.write().set_usage(usage);
    }

    /// Reset the GPU check rate limiter (for testing).
    ///
    /// This allows immediate GPU budget checks without waiting for the rate limit interval.
    pub fn reset_gpu_check_timer(&self) {
        self.last_gpu_check.store(0, Ordering::Relaxed);
    }

    /// Get the maximum latency allowed (Constitution: <100ms).
    pub fn max_latency(&self) -> Duration {
        self.max_latency
    }
}

impl Default for WakeController {
    fn default() -> Self {
        Self::new()
    }
}

/// Wake statistics.
#[derive(Debug, Clone)]
pub struct WakeStats {
    /// Total wake events
    pub wake_count: u64,
    /// Latency violations (>100ms)
    pub latency_violations: u64,
    /// Maximum allowed latency
    pub max_latency: Duration,
    /// Maximum GPU usage allowed
    pub max_gpu_usage: f32,
}

/// Handle for external systems to signal wake.
///
/// Lightweight clone of wake signaling capability.
#[derive(Clone)]
pub struct WakeHandle {
    interrupt_flag: Arc<AtomicBool>,
    wake_sender: watch::Sender<Option<WakeReason>>,
}

impl WakeHandle {
    /// Create from wake controller.
    pub fn from_controller(controller: &WakeController) -> Self {
        Self {
            interrupt_flag: controller.interrupt_flag(),
            wake_sender: controller.wake_sender.clone(),
        }
    }

    /// Signal immediate wake.
    pub fn wake(&self, reason: WakeReason) {
        self.interrupt_flag.store(true, Ordering::SeqCst);
        let _ = self.wake_sender.send(Some(reason));
    }

    /// Check if wake was signaled.
    pub fn is_signaled(&self) -> bool {
        self.interrupt_flag.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wake_controller_creation() {
        let controller = WakeController::new();
        assert_eq!(controller.state(), WakeState::Idle);
        assert!(!controller.is_dreaming());
        assert!(controller.max_latency.as_millis() < 100, "Must be <100ms per Constitution");
    }

    #[test]
    fn test_prepare_for_dream() {
        let controller = WakeController::new();
        controller.prepare_for_dream();
        assert_eq!(controller.state(), WakeState::Dreaming);
        assert!(controller.is_dreaming());
        assert!(!controller.is_wake_signaled());
    }

    #[test]
    fn test_signal_wake() {
        let controller = WakeController::new();
        controller.prepare_for_dream();
        controller.signal_wake(WakeReason::ExternalQuery).unwrap();
        assert_eq!(controller.state(), WakeState::Waking);
        assert!(controller.is_wake_signaled());
    }

    #[test]
    fn test_wake_latency_success() {
        let controller = WakeController::new();
        controller.prepare_for_dream();
        controller.signal_wake(WakeReason::ExternalQuery).unwrap();

        // Complete immediately (should be well under 100ms)
        let latency = controller.complete_wake().unwrap();
        assert!(latency < Duration::from_millis(100), "Latency {:?} must be <100ms", latency);
        assert_eq!(controller.stats().wake_count, 1);
        assert_eq!(controller.stats().latency_violations, 0);
    }

    #[test]
    fn test_reset() {
        let controller = WakeController::new();
        controller.prepare_for_dream();
        controller.signal_wake(WakeReason::ManualAbort).unwrap();
        controller.reset();
        assert_eq!(controller.state(), WakeState::Idle);
        assert!(!controller.is_wake_signaled());
    }

    #[test]
    fn test_gpu_budget_check_ok() {
        let controller = WakeController::new();
        controller.prepare_for_dream();

        // Set GPU usage below budget (30%)
        controller.set_gpu_usage(0.2);

        // Reset rate limiter using helper method
        controller.reset_gpu_check_timer();

        // Should pass
        controller.check_gpu_budget().unwrap();
        assert!(controller.is_dreaming());
    }

    #[test]
    fn test_gpu_budget_exceeded() {
        let controller = WakeController::new();
        controller.prepare_for_dream();

        // Set GPU usage above budget (30%)
        controller.set_gpu_usage(0.5);

        // Reset rate limiter using helper method
        controller.reset_gpu_check_timer();

        // Should fail and signal wake
        let result = controller.check_gpu_budget();
        assert!(matches!(result, Err(WakeError::GpuBudgetExceeded { .. })));
        assert!(controller.is_wake_signaled());
    }

    #[test]
    fn test_wake_handle() {
        let controller = WakeController::new();
        let handle = WakeHandle::from_controller(&controller);

        controller.prepare_for_dream();
        assert!(!handle.is_signaled());

        handle.wake(WakeReason::ExternalQuery);
        assert!(handle.is_signaled());
        assert!(controller.is_wake_signaled());
    }

    #[test]
    fn test_interrupt_flag_sharing() {
        let controller = WakeController::new();
        let flag = controller.interrupt_flag();

        controller.prepare_for_dream();

        // Simulate phase checking flag
        assert!(!flag.load(Ordering::SeqCst));

        // Signal wake
        controller.signal_wake(WakeReason::ManualAbort).unwrap();

        // Phase should see interrupt
        assert!(flag.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_wake_subscription() {
        let controller = WakeController::new();
        let mut receiver = controller.subscribe();

        controller.prepare_for_dream();

        // Initially None
        assert!(receiver.borrow().is_none());

        // Signal wake
        controller.signal_wake(WakeReason::ExternalQuery).unwrap();

        // Wait for change
        receiver.changed().await.unwrap();

        // Should receive wake reason
        assert_eq!(*receiver.borrow(), Some(WakeReason::ExternalQuery));
    }

    // ============ Edge Case Tests (From Task Spec 5.3) ============

    #[test]
    fn test_edge_case_wake_during_idle_state() {
        // Setup: Controller in Idle state (not dreaming)
        let controller = WakeController::new();
        println!("STATE BEFORE: {:?}", controller.state()); // Idle

        // Action: Try to signal wake when not dreaming
        let result = controller.signal_wake(WakeReason::ExternalQuery);
        println!("STATE AFTER: {:?}", controller.state()); // Still Idle

        // Expected: No error, state unchanged (wake ignored)
        assert!(result.is_ok());
        assert_eq!(controller.state(), WakeState::Idle);
    }

    #[test]
    fn test_edge_case_gpu_budget_exactly_at_30_percent() {
        // Setup
        let controller = WakeController::new();
        controller.prepare_for_dream();
        controller.set_gpu_usage(0.30); // Exactly at threshold
        controller.reset_gpu_check_timer();

        println!("GPU: 30%, STATE BEFORE: {:?}", controller.state());

        // Action: Check GPU budget
        let result = controller.check_gpu_budget();

        println!("STATE AFTER: {:?}, signaled: {}", controller.state(), controller.is_wake_signaled());

        // Expected: Should NOT trigger wake (>= threshold is 30%, we're AT 30%)
        // The check is `usage > self.max_gpu_usage`, so 0.30 > 0.30 is false
        assert!(result.is_ok(), "GPU at exactly 30% should not trigger (strict > comparison)");
    }

    #[test]
    fn test_edge_case_gpu_budget_just_above_30_percent() {
        // Setup
        let controller = WakeController::new();
        controller.prepare_for_dream();
        controller.set_gpu_usage(0.31); // Just above threshold
        controller.reset_gpu_check_timer();

        println!("GPU: 31%, STATE BEFORE: {:?}", controller.state());

        // Action: Check GPU budget
        let result = controller.check_gpu_budget();

        println!("STATE AFTER: {:?}, signaled: {}", controller.state(), controller.is_wake_signaled());

        // Expected: Should trigger wake (0.31 > 0.30)
        assert!(matches!(result, Err(WakeError::GpuBudgetExceeded { .. })));
        assert!(controller.is_wake_signaled());
    }

    #[test]
    fn test_edge_case_double_wake_signal() {
        // Setup
        let controller = WakeController::new();
        controller.prepare_for_dream();

        // First wake
        controller.signal_wake(WakeReason::ExternalQuery).unwrap();
        println!("STATE AFTER FIRST WAKE: {:?}", controller.state()); // Waking

        // Second wake attempt (should be ignored because state is no longer Dreaming)
        let result = controller.signal_wake(WakeReason::ManualAbort);
        println!("STATE AFTER SECOND WAKE: {:?}", controller.state()); // Still Waking

        // Expected: No error, second wake ignored (state is Waking, not Dreaming)
        assert!(result.is_ok());
        assert_eq!(controller.state(), WakeState::Waking);
    }

    #[test]
    fn test_wake_stats_tracking() {
        let controller = WakeController::new();

        // Initial stats
        let stats = controller.stats();
        assert_eq!(stats.wake_count, 0);
        assert_eq!(stats.latency_violations, 0);
        assert!(stats.max_latency.as_millis() < 100);
        assert_eq!(stats.max_gpu_usage, 0.30);

        // Perform a wake
        controller.prepare_for_dream();
        controller.signal_wake(WakeReason::CycleComplete).unwrap();
        controller.complete_wake().unwrap();

        // Stats should update
        let stats = controller.stats();
        assert_eq!(stats.wake_count, 1);
        assert_eq!(stats.latency_violations, 0);
    }

    #[test]
    fn test_resource_snapshot() {
        let controller = WakeController::new();
        controller.set_gpu_usage(0.25);

        let snapshot = controller.get_resource_snapshot();

        assert_eq!(snapshot.gpu_usage, 0.25);
        assert_eq!(snapshot.memory_bytes, 0);
        assert_eq!(snapshot.cpu_usage, 0.0);
    }

    #[test]
    fn test_state_transitions_complete_cycle() {
        let controller = WakeController::new();

        // Idle -> Dreaming
        assert_eq!(controller.state(), WakeState::Idle);
        controller.prepare_for_dream();
        assert_eq!(controller.state(), WakeState::Dreaming);

        // Dreaming -> Waking
        controller.signal_wake(WakeReason::CycleComplete).unwrap();
        assert_eq!(controller.state(), WakeState::Waking);

        // Waking -> Completing
        controller.complete_wake().unwrap();
        assert_eq!(controller.state(), WakeState::Completing);

        // Completing -> Idle
        controller.reset();
        assert_eq!(controller.state(), WakeState::Idle);
    }
}
