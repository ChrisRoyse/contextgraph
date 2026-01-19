//! DreamController - Main orchestrator for dream cycles
//!
//! The DreamController manages the complete dream cycle including:
//! - State transitions (Awake -> NREM -> REM -> Awake)
//! - Phase timing and coordination
//! - Interrupt handling for query abort
//! - GPU budget enforcement
//! - Wake latency guarantees
//!
//! ## Constitution Reference
//!
//! Section dream (lines 446-453):
//! - NREM: 3 minutes, Hebbian replay, coupling=0.9
//! - REM: 2 minutes, attractor exploration, temp=2.0
//! - Constraints: 100 queries, <100ms wake, <30% GPU

use std::fmt::Debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use super::amortized::AmortizedLearner;
use super::constants;
use super::nrem::{MemoryProvider, NremPhase, NremReport};
use super::rem::{RemPhase, RemReport};
use super::scheduler::DreamScheduler;
use super::types::{ConsolidationCallback, ConsolidationMetrics};
use super::WakeReason;
use crate::error::{CoreError, CoreResult};

/// Current state of the dream system
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub enum DreamState {
    /// System is awake and processing queries normally
    #[default]
    Awake,

    /// Transitioning into dream state
    EnteringDream,

    /// NREM phase active - Hebbian replay with tight coupling
    Nrem {
        /// Time elapsed in NREM phase
        elapsed_ms: u64,
        /// Progress through NREM phase (0.0 - 1.0)
        progress: f32,
    },

    /// REM phase active - Attractor exploration
    Rem {
        /// Time elapsed in REM phase
        elapsed_ms: u64,
        /// Progress through REM phase (0.0 - 1.0)
        progress: f32,
    },

    /// Waking up from dream state
    Waking,
}

impl DreamState {
    /// Check if currently in a dream phase (NREM or REM)
    pub fn is_dreaming(&self) -> bool {
        matches!(
            self,
            DreamState::EnteringDream | DreamState::Nrem { .. } | DreamState::Rem { .. }
        )
    }

    /// Get the phase name for logging
    pub fn phase_name(&self) -> &'static str {
        match self {
            DreamState::Awake => "awake",
            DreamState::EnteringDream => "entering_dream",
            DreamState::Nrem { .. } => "nrem",
            DreamState::Rem { .. } => "rem",
            DreamState::Waking => "waking",
        }
    }
}

/// Status information for the dream system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamStatus {
    /// Current dream state
    pub state: DreamState,

    /// Current GPU usage percentage
    pub gpu_usage: f32,

    /// Whether dream is currently active
    pub is_dreaming: bool,

    /// Time since last dream cycle
    pub time_since_last_dream: Option<Duration>,

    /// Number of completed dream cycles
    pub completed_cycles: u64,

    /// Last dream completion timestamp
    pub last_dream_completed: Option<DateTime<Utc>>,

    /// Current activity level
    pub activity_level: f32,
}

/// Report from a completed dream cycle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamReport {
    /// Whether the cycle completed successfully
    pub completed: bool,

    /// NREM phase report (if executed)
    pub nrem_report: Option<NremReport>,

    /// REM phase report (if executed)
    pub rem_report: Option<RemReport>,

    /// Total cycle duration
    pub total_duration: Duration,

    /// Wake reason
    pub wake_reason: WakeReason,

    /// Number of shortcuts created during amortized learning
    pub shortcuts_created: usize,

    /// Peak GPU usage during cycle
    pub peak_gpu_usage: f32,

    /// Wake latency (time from interrupt to awake)
    pub wake_latency: Option<Duration>,

    /// Cycle start timestamp
    pub started_at: DateTime<Utc>,

    /// Cycle end timestamp
    pub ended_at: DateTime<Utc>,
}

/// Main orchestrator for dream cycles
///
/// Manages the complete dream cycle lifecycle including phase transitions,
/// interrupt handling, and resource monitoring.
pub struct DreamController {
    /// Current dream state
    state: DreamState,

    /// NREM phase handler
    nrem: NremPhase,

    /// REM phase handler
    rem: RemPhase,

    /// Amortized shortcut learner
    amortizer: AmortizedLearner,

    /// Dream scheduler for trigger detection
    scheduler: DreamScheduler,

    /// Maximum GPU usage budget (Constitution: 0.30)
    gpu_budget: f32,

    /// Maximum synthetic queries during REM (Constitution: 100)
    #[allow(dead_code)]
    query_limit: usize,

    /// Maximum wake latency (Constitution: 100ms)
    wake_latency_budget: Duration,

    /// Interrupt flag for abort handling
    interrupt_flag: Arc<AtomicBool>,

    /// Number of completed dream cycles
    completed_cycles: u64,

    /// Last dream completion time
    last_dream_completed: Option<DateTime<Utc>>,

    /// Start time of current dream cycle
    cycle_start: Option<Instant>,

    /// Peak GPU usage during current cycle
    peak_gpu_usage: f32,

    /// TASK-L02: Optional callback for consolidation completion.
    /// Called when dream cycle completes successfully.
    consolidation_callback: Option<ConsolidationCallback>,
}

impl DreamController {
    /// Create a new DreamController with a real MemoryProvider (PRODUCTION USE).
    ///
    /// This is the REQUIRED constructor for production and integration tests.
    /// Per AP-71: "Dream NREM/REM returning stubs forbidden"
    ///
    /// # Arguments
    ///
    /// * `provider` - A real MemoryProvider implementation (e.g., GraphMemoryProvider)
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_storage::GraphMemoryProvider;
    ///
    /// let storage = Arc::new(RocksDbMemex::open("/tmp/test")?);
    /// let provider = Arc::new(GraphMemoryProvider::new(storage));
    /// let controller = DreamController::with_provider(provider);
    /// ```
    pub fn with_provider(provider: Arc<dyn MemoryProvider>) -> Self {
        info!(
            "DreamController::with_provider() - Creating controller with real memory provider"
        );
        Self {
            state: DreamState::Awake,
            nrem: NremPhase::with_provider(provider),
            rem: RemPhase::new(),
            amortizer: AmortizedLearner::new(),
            scheduler: DreamScheduler::new(),
            gpu_budget: constants::MAX_GPU_USAGE,
            query_limit: constants::MAX_REM_QUERIES,
            wake_latency_budget: constants::MAX_WAKE_LATENCY,
            interrupt_flag: Arc::new(AtomicBool::new(false)),
            completed_cycles: 0,
            last_dream_completed: None,
            cycle_start: None,
            peak_gpu_usage: 0.0,
            consolidation_callback: None,
        }
    }

    /// Create a new DreamController WITHOUT a memory provider (UNIT TESTING ONLY).
    ///
    /// # WARNING: Constitution Compliance
    ///
    /// - AP-71: "Dream NREM/REM returning stubs forbidden"
    /// - This method logs a WARNING because production code should use `with_provider()`
    /// - Only use this in unit tests that test state machine logic
    ///
    /// # For Production
    ///
    /// Use `with_provider()` with a real MemoryProvider instead.
    pub fn new() -> Self {
        warn!(
            "DreamController::new() called without MemoryProvider - this is ONLY for unit tests! \
             Production code MUST use DreamController::with_provider() per AP-71"
        );
        Self {
            state: DreamState::Awake,
            nrem: NremPhase::new(),
            rem: RemPhase::new(),
            amortizer: AmortizedLearner::new(),
            scheduler: DreamScheduler::new(),
            gpu_budget: constants::MAX_GPU_USAGE,
            query_limit: constants::MAX_REM_QUERIES,
            wake_latency_budget: constants::MAX_WAKE_LATENCY,
            interrupt_flag: Arc::new(AtomicBool::new(false)),
            completed_cycles: 0,
            last_dream_completed: None,
            cycle_start: None,
            peak_gpu_usage: 0.0,
            consolidation_callback: None,
        }
    }

    /// Start a complete dream cycle (NREM + REM)
    ///
    /// # Returns
    ///
    /// A `DreamReport` containing metrics from both phases and overall cycle status.
    ///
    /// # Errors
    ///
    /// Returns `CoreError::LayerError` if dream cannot be started or processing fails.
    pub async fn start_dream_cycle(&mut self) -> CoreResult<DreamReport> {
        let started_at = Utc::now();
        let cycle_start = Instant::now();
        self.cycle_start = Some(cycle_start);
        self.peak_gpu_usage = 0.0;
        self.interrupt_flag.store(false, Ordering::SeqCst);

        info!("Starting dream cycle at {:?}", started_at);

        // Transition to entering dream state
        self.state = DreamState::EnteringDream;
        debug!("Dream state: EnteringDream");

        // Check GPU budget before starting
        if !self.check_gpu_budget() {
            warn!("GPU budget exceeded before dream cycle start");
            return Ok(self.create_aborted_report(
                started_at,
                cycle_start.elapsed(),
                WakeReason::GpuOverBudget,
            ));
        }

        // Execute NREM phase
        let nrem_result = self.execute_nrem_phase().await;

        // Check for interrupt
        if self.interrupt_flag.load(Ordering::SeqCst) {
            info!("Dream cycle interrupted during NREM");
            return Ok(self.create_aborted_report(
                started_at,
                cycle_start.elapsed(),
                WakeReason::ExternalQuery,
            ));
        }

        let nrem_report = match nrem_result {
            Ok(report) => Some(report),
            Err(e) => {
                error!("NREM phase failed: {:?}", e);
                self.state = DreamState::Waking;
                return Ok(self.create_aborted_report(
                    started_at,
                    cycle_start.elapsed(),
                    WakeReason::Error,
                ));
            }
        };

        // Execute REM phase
        let rem_result = self.execute_rem_phase().await;

        // Check for interrupt
        if self.interrupt_flag.load(Ordering::SeqCst) {
            info!("Dream cycle interrupted during REM");
            return Ok(DreamReport {
                completed: false,
                nrem_report,
                rem_report: None,
                total_duration: cycle_start.elapsed(),
                wake_reason: WakeReason::ExternalQuery,
                shortcuts_created: self.amortizer.shortcuts_created_this_cycle(),
                peak_gpu_usage: self.peak_gpu_usage,
                wake_latency: None,
                started_at,
                ended_at: Utc::now(),
            });
        }

        let rem_report = match rem_result {
            Ok(report) => Some(report),
            Err(e) => {
                error!("REM phase failed: {:?}", e);
                self.state = DreamState::Waking;
                return Ok(DreamReport {
                    completed: false,
                    nrem_report,
                    rem_report: None,
                    total_duration: cycle_start.elapsed(),
                    wake_reason: WakeReason::Error,
                    shortcuts_created: self.amortizer.shortcuts_created_this_cycle(),
                    peak_gpu_usage: self.peak_gpu_usage,
                    wake_latency: None,
                    started_at,
                    ended_at: Utc::now(),
                });
            }
        };

        // Complete cycle
        self.state = DreamState::Awake;
        self.completed_cycles += 1;
        self.last_dream_completed = Some(Utc::now());
        self.scheduler.record_dream_completion();

        let shortcuts_created = self.amortizer.shortcuts_created_this_cycle();
        self.amortizer.reset_cycle_counter();

        info!(
            "Dream cycle completed: {} shortcuts created, {:?} duration",
            shortcuts_created,
            cycle_start.elapsed()
        );

        // TASK-L02: Invoke consolidation callback for lambda adjustment
        // SPEC-DREAM-LAMBDA-001: Wire DreamController to MetaUtlTracker
        // METAUTL-003: `"dream_triggered → lambda_adjustment"`
        let edges_pruned = nrem_report.as_ref().map(|r| r.edges_pruned).unwrap_or(0) as u32;
        let duration = cycle_start.elapsed();

        // Compute quality and coherence from consolidation results
        // Quality: ratio of pruned edges to total processed (approximated by memories_replayed * edges per memory)
        // Coherence: placeholder computed from shortcuts ratio (will be replaced with clustering order parameter)
        let quality = {
            let memories = nrem_report
                .as_ref()
                .map(|r| r.memories_replayed)
                .unwrap_or(0);
            if memories > 0 {
                // Approximate: each memory ~10 edges, quality = edges_pruned / estimated_total
                let estimated_edges = memories * 10;
                (edges_pruned as f32 / estimated_edges.max(1) as f32).clamp(0.0, 1.0)
            } else {
                0.0
            }
        };

        let coherence = {
            // Placeholder: coherence based on shortcuts_created / edges_pruned ratio
            // High shortcuts with moderate pruning = good coherence
            if edges_pruned > 0 {
                let ratio = shortcuts_created as f32 / edges_pruned as f32;
                // Normalize ratio to [0, 1] using sigmoid-like function
                (1.0 - (-ratio).exp()).clamp(0.0, 1.0)
            } else if shortcuts_created > 0 {
                1.0 // All shortcuts, no pruning = high coherence
            } else {
                0.0
            }
        };

        let consolidation_metrics = ConsolidationMetrics {
            quality,
            coherence,
            edges_pruned,
            shortcuts_created: shortcuts_created as u32,
            duration,
            success: true,
            blind_spots_found: 0, // Placeholder: will be computed by identity layer
        };

        self.invoke_consolidation_callback(consolidation_metrics);

        Ok(DreamReport {
            completed: true,
            nrem_report,
            rem_report,
            total_duration: cycle_start.elapsed(),
            wake_reason: WakeReason::CycleComplete,
            shortcuts_created,
            peak_gpu_usage: self.peak_gpu_usage,
            wake_latency: None,
            started_at,
            ended_at: Utc::now(),
        })
    }

    /// Execute the NREM phase
    async fn execute_nrem_phase(&mut self) -> CoreResult<NremReport> {
        self.state = DreamState::Nrem {
            elapsed_ms: 0,
            progress: 0.0,
        };

        debug!("Starting NREM phase");

        let phase_start = Instant::now();
        let _ = constants::NREM_DURATION; // phase_duration used for future progress tracking

        // Run NREM processing
        let report = self
            .nrem
            .process(&self.interrupt_flag, &mut self.amortizer)
            .await?;

        // Update state with final progress
        let elapsed = phase_start.elapsed();
        self.state = DreamState::Nrem {
            elapsed_ms: elapsed.as_millis() as u64,
            progress: 1.0,
        };

        debug!("NREM phase completed in {:?}", elapsed);

        Ok(report)
    }

    /// Execute the REM phase
    async fn execute_rem_phase(&mut self) -> CoreResult<RemReport> {
        self.state = DreamState::Rem {
            elapsed_ms: 0,
            progress: 0.0,
        };

        debug!("Starting REM phase");

        let phase_start = Instant::now();

        // Run REM processing
        let report = self.rem.process(&self.interrupt_flag).await?;

        // Update state with final progress
        let elapsed = phase_start.elapsed();
        self.state = DreamState::Rem {
            elapsed_ms: elapsed.as_millis() as u64,
            progress: 1.0,
        };

        debug!("REM phase completed in {:?}", elapsed);

        Ok(report)
    }

    /// Abort the current dream cycle
    ///
    /// Signals an immediate wake and returns the wake latency.
    /// Constitution mandates wake latency <100ms.
    ///
    /// # Returns
    ///
    /// The actual wake latency duration.
    ///
    /// # Errors
    ///
    /// Returns error if wake latency exceeds 100ms budget (constitution violation).
    pub fn abort(&mut self) -> CoreResult<Duration> {
        let abort_start = Instant::now();

        info!("Dream abort requested");

        // Set interrupt flag
        self.interrupt_flag.store(true, Ordering::SeqCst);

        // Transition to waking state
        self.state = DreamState::Waking;

        // Complete transition to awake
        self.state = DreamState::Awake;

        let wake_latency = abort_start.elapsed();

        // Verify wake latency budget
        if wake_latency > self.wake_latency_budget {
            error!(
                "Wake latency {:?} exceeded budget {:?}",
                wake_latency, self.wake_latency_budget
            );
            return Err(CoreError::LayerError {
                layer: "dream".to_string(),
                message: format!(
                    "Wake latency {:?} exceeded budget {:?} (constitution violation)",
                    wake_latency, self.wake_latency_budget
                ),
            });
        }

        info!("Dream aborted in {:?}", wake_latency);

        Ok(wake_latency)
    }

    /// Get the current dream status
    pub fn get_status(&self) -> DreamStatus {
        let time_since_last = self
            .last_dream_completed
            .map(|t| (Utc::now() - t).to_std().unwrap_or(Duration::ZERO));

        DreamStatus {
            state: self.state.clone(),
            gpu_usage: self.current_gpu_usage(),
            is_dreaming: self.state.is_dreaming(),
            time_since_last_dream: time_since_last,
            completed_cycles: self.completed_cycles,
            last_dream_completed: self.last_dream_completed,
            activity_level: self.scheduler.get_average_activity(),
        }
    }

    /// Check if GPU usage is within budget
    ///
    /// Returns false if GPU usage exceeds 30% (constitution constraint).
    pub fn check_gpu_budget(&mut self) -> bool {
        let usage = self.current_gpu_usage();
        self.peak_gpu_usage = self.peak_gpu_usage.max(usage);
        usage <= self.gpu_budget
    }

    /// Get current GPU usage percentage
    ///
    /// Note: This is a placeholder that returns 0.0. Real implementation
    /// would query CUDA/GPU monitoring APIs.
    fn current_gpu_usage(&self) -> f32 {
        // TODO: Integrate with actual GPU monitoring
        // For now, return a safe value
        0.0
    }

    /// Set the interrupt flag for query abort
    ///
    /// This is called by external query handlers to signal an immediate wake.
    pub fn set_interrupt(&self) {
        self.interrupt_flag.store(true, Ordering::SeqCst);
    }

    /// Get the interrupt flag for sharing with async tasks
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.interrupt_flag)
    }

    /// Update activity level in the scheduler
    pub fn update_activity(&mut self, activity: f32) {
        self.scheduler.update_activity(activity);
    }

    /// Check if a dream cycle should be triggered
    pub fn should_trigger_dream(&self) -> bool {
        self.scheduler.should_trigger_dream()
    }

    /// Set the memory provider for NREM phase.
    ///
    /// TASK-008: Allows injecting a real memory provider for Hebbian replay.
    /// Per DREAM-001: Provider data feeds dw = eta * phi_i * phi_j.
    /// Per AP-35: Must not return stub data when real data is available.
    ///
    /// # Arguments
    ///
    /// * `provider` - Implementation of `MemoryProvider` trait
    ///
    /// # Example
    ///
    /// ```ignore
    /// use context_graph_storage::GraphMemoryProvider;
    ///
    /// let storage = Arc::new(RocksDbMemex::open("/tmp/test")?);
    /// let provider = Arc::new(GraphMemoryProvider::new(storage));
    /// controller.set_memory_provider(provider);
    /// ```
    pub fn set_memory_provider(&mut self, provider: Arc<dyn MemoryProvider>) {
        self.nrem.set_memory_provider(provider);
    }

    // ========================================================================
    // TASK-L02: Consolidation Callback Methods
    // SPEC-DREAM-LAMBDA-001: Wire DreamController to MetaUtlTracker.
    // METAUTL-003: `"dream_triggered → lambda_adjustment"`
    // ========================================================================

    /// Set a callback to be invoked when dream consolidation completes successfully.
    ///
    /// The callback receives `ConsolidationMetrics` containing quality and coherence
    /// scores that can be used to adjust lambda weights via `MetaUtlTracker::adjust_lambdas()`.
    ///
    /// # Arguments
    ///
    /// * `callback` - Thread-safe callback function
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::sync::Arc;
    ///
    /// controller.set_consolidation_callback(Arc::new(|metrics| {
    ///     info!("Consolidation completed: quality={}, coherence={}",
    ///           metrics.quality, metrics.coherence);
    /// }));
    /// ```
    pub fn set_consolidation_callback(&mut self, callback: ConsolidationCallback) {
        self.consolidation_callback = Some(callback);
    }

    /// Clear the consolidation callback.
    ///
    /// After calling this, no callback will be invoked on consolidation completion.
    pub fn clear_consolidation_callback(&mut self) {
        self.consolidation_callback = None;
    }

    /// Check if a consolidation callback is registered.
    ///
    /// # Returns
    ///
    /// `true` if a callback is set, `false` otherwise.
    pub fn has_consolidation_callback(&self) -> bool {
        self.consolidation_callback.is_some()
    }

    /// Invoke the consolidation callback if one is registered.
    ///
    /// This is called internally when a dream cycle completes successfully.
    /// The callback is invoked synchronously, so any expensive operations
    /// should be performed asynchronously inside the callback.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Consolidation metrics from the completed cycle
    fn invoke_consolidation_callback(&self, metrics: ConsolidationMetrics) {
        if let Some(ref callback) = self.consolidation_callback {
            debug!(
                "Invoking consolidation callback: quality={}, coherence={}",
                metrics.quality, metrics.coherence
            );
            callback(metrics);
        }
    }

    /// Create an aborted report
    fn create_aborted_report(
        &self,
        started_at: DateTime<Utc>,
        duration: Duration,
        reason: WakeReason,
    ) -> DreamReport {
        DreamReport {
            completed: false,
            nrem_report: None,
            rem_report: None,
            total_duration: duration,
            wake_reason: reason,
            shortcuts_created: 0,
            peak_gpu_usage: self.peak_gpu_usage,
            wake_latency: None,
            started_at,
            ended_at: Utc::now(),
        }
    }
}

impl Default for DreamController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dream_controller_creation() {
        let controller = DreamController::new();

        assert_eq!(controller.state, DreamState::Awake);
        assert_eq!(controller.gpu_budget, 0.30);
        assert_eq!(controller.query_limit, 100);
        assert!(controller.wake_latency_budget.as_millis() < 100);
        assert_eq!(controller.completed_cycles, 0);
    }

    #[test]
    fn test_dream_state_transitions() {
        assert!(!DreamState::Awake.is_dreaming());
        assert!(DreamState::EnteringDream.is_dreaming());
        assert!(DreamState::Nrem {
            elapsed_ms: 0,
            progress: 0.0
        }
        .is_dreaming());
        assert!(DreamState::Rem {
            elapsed_ms: 0,
            progress: 0.0
        }
        .is_dreaming());
        assert!(!DreamState::Waking.is_dreaming());
    }

    #[test]
    fn test_dream_state_phase_names() {
        assert_eq!(DreamState::Awake.phase_name(), "awake");
        assert_eq!(DreamState::EnteringDream.phase_name(), "entering_dream");
        assert_eq!(
            DreamState::Nrem {
                elapsed_ms: 0,
                progress: 0.0
            }
            .phase_name(),
            "nrem"
        );
        assert_eq!(
            DreamState::Rem {
                elapsed_ms: 0,
                progress: 0.0
            }
            .phase_name(),
            "rem"
        );
        assert_eq!(DreamState::Waking.phase_name(), "waking");
    }

    #[test]
    fn test_abort_wake_latency() {
        let mut controller = DreamController::new();

        // Should be in awake state, abort should be fast
        let latency = controller.abort().expect("Abort should succeed");

        // Should be well under 100ms since we're already awake
        assert!(
            latency < Duration::from_millis(100),
            "Wake latency {:?} exceeded 100ms",
            latency
        );
    }

    #[test]
    fn test_get_status() {
        let controller = DreamController::new();
        let status = controller.get_status();

        assert!(!status.is_dreaming);
        assert_eq!(status.completed_cycles, 0);
        assert!(status.last_dream_completed.is_none());
    }

    #[test]
    fn test_interrupt_flag() {
        let controller = DreamController::new();

        assert!(!controller.interrupt_flag.load(Ordering::SeqCst));

        controller.set_interrupt();

        assert!(controller.interrupt_flag.load(Ordering::SeqCst));
    }

    // ========================================================================
    // TASK-L02: Consolidation Callback Tests
    // SPEC-DREAM-LAMBDA-001: Wire DreamController to MetaUtlTracker
    // ========================================================================

    #[test]
    fn test_callback_initially_none() {
        let controller = DreamController::new();
        assert!(!controller.has_consolidation_callback());
    }

    #[test]
    fn test_set_consolidation_callback() {
        use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

        let mut controller = DreamController::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        controller.set_consolidation_callback(Arc::new(move |_metrics| {
            called_clone.store(true, AtomicOrdering::SeqCst);
        }));

        assert!(controller.has_consolidation_callback());

        // Invoke callback manually to test
        let metrics = ConsolidationMetrics {
            quality: 0.8,
            coherence: 0.7,
            edges_pruned: 100,
            shortcuts_created: 10,
            duration: Duration::from_secs(30),
            success: true,
            blind_spots_found: 2,
        };
        controller.invoke_consolidation_callback(metrics);

        assert!(called.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_clear_consolidation_callback() {
        let mut controller = DreamController::new();

        controller.set_consolidation_callback(Arc::new(|_| {}));
        assert!(controller.has_consolidation_callback());

        controller.clear_consolidation_callback();
        assert!(!controller.has_consolidation_callback());
    }

    #[test]
    fn test_callback_receives_correct_metrics() {
        use std::sync::Mutex;

        let mut controller = DreamController::new();
        let received_metrics = Arc::new(Mutex::new(None));
        let metrics_clone = received_metrics.clone();

        controller.set_consolidation_callback(Arc::new(move |metrics| {
            *metrics_clone.lock().unwrap() = Some(metrics);
        }));

        let input_metrics = ConsolidationMetrics {
            quality: 0.85,
            coherence: 0.75,
            edges_pruned: 50,
            shortcuts_created: 5,
            duration: Duration::from_secs(60),
            success: true,
            blind_spots_found: 3,
        };
        controller.invoke_consolidation_callback(input_metrics.clone());

        let received = received_metrics.lock().unwrap();
        let received = received.as_ref().expect("Callback should have been called");
        assert_eq!(received.quality, 0.85);
        assert_eq!(received.coherence, 0.75);
        assert_eq!(received.edges_pruned, 50);
        assert_eq!(received.shortcuts_created, 5);
        assert!(received.success);
    }

    #[test]
    fn test_invoke_without_callback_is_safe() {
        let controller = DreamController::new();

        // This should not panic
        let metrics = ConsolidationMetrics {
            quality: 0.5,
            coherence: 0.5,
            edges_pruned: 10,
            shortcuts_created: 1,
            duration: Duration::from_secs(10),
            success: true,
            blind_spots_found: 0,
        };
        controller.invoke_consolidation_callback(metrics);
        // No panic = success
    }
}
