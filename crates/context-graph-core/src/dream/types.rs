//! Dream Layer Types
//!
//! Defines types for Hebbian learning, hyperbolic walks, and dream triggers.
//!
//! # Constitution Compliance
//!
//! All default values are derived from `docs2/constitution.yaml` lines 390-394.
//! Any deviation from constitution values is a BUG.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uuid::Uuid;

// ============================================================================
// HEBBIAN LEARNING TYPES
// ============================================================================

/// Configuration for Hebbian learning in NREM phase.
///
/// Constitution Reference: `docs2/constitution.yaml` lines 391-393
///
/// # Panics
///
/// Methods panic if values violate constitution bounds.
///
/// # Example
///
/// ```
/// use context_graph_core::dream::HebbianConfig;
///
/// let config = HebbianConfig::default();
/// assert_eq!(config.learning_rate, 0.01);
/// assert_eq!(config.coupling_strength, 0.9); // Constitution mandated
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HebbianConfig {
    /// Learning rate (eta) for weight updates.
    /// Formula: dw_ij = eta * phi_i * phi_j
    /// Constitution default: 0.01
    pub learning_rate: f32,

    /// Weight decay factor applied per NREM cycle.
    /// Constitution: 0.001
    pub weight_decay: f32,

    /// Minimum weight before edge is marked for pruning.
    /// Constitution: 0.05
    pub weight_floor: f32,

    /// Maximum weight cap to prevent runaway strengthening.
    /// Constitution: 1.0
    pub weight_cap: f32,

    /// Kuramoto coupling strength for neural synchronization.
    /// Constitution: 0.9 during NREM (NOT 10.0)
    /// Reference: docs2/constitution.yaml line 393
    pub coupling_strength: f32,
}

impl Default for HebbianConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            weight_decay: 0.001,
            weight_floor: 0.05,
            weight_cap: 1.0,
            coupling_strength: 0.9, // CORRECTED: Constitution says 0.9, not 10.0
        }
    }
}

impl HebbianConfig {
    /// Validate configuration against constitution bounds.
    ///
    /// # Panics
    ///
    /// Panics with detailed error if any value is out of bounds.
    #[track_caller]
    pub fn validate(&self) {
        assert!(
            self.learning_rate > 0.0 && self.learning_rate <= 1.0,
            "HebbianConfig: learning_rate={} must be in (0.0, 1.0], constitution default=0.01",
            self.learning_rate
        );
        assert!(
            self.weight_decay >= 0.0 && self.weight_decay < 1.0,
            "HebbianConfig: weight_decay={} must be in [0.0, 1.0), constitution default=0.001",
            self.weight_decay
        );
        assert!(
            self.weight_floor >= 0.0,
            "HebbianConfig: weight_floor={} must be >= 0.0, constitution default=0.05",
            self.weight_floor
        );
        assert!(
            self.weight_cap > self.weight_floor,
            "HebbianConfig: weight_cap={} must be > weight_floor={}, constitution defaults: 1.0 > 0.05",
            self.weight_cap, self.weight_floor
        );
        assert!(
            self.coupling_strength > 0.0 && self.coupling_strength <= 10.0,
            "HebbianConfig: coupling_strength={} must be in (0.0, 10.0], constitution default=0.9",
            self.coupling_strength
        );
    }

    /// Create a validated config, panicking if invalid.
    #[track_caller]
    pub fn validated(self) -> Self {
        self.validate();
        self
    }
}

/// Activation (phi) value for a node during replay.
///
/// Represents the "firing" level of a node during NREM replay.
/// Used in Hebbian update formula: dw_ij = eta * phi_i * phi_j
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeActivation {
    /// Node identifier (must be valid UUID)
    pub node_id: Uuid,

    /// Activation level [0.0, 1.0]
    /// 0.0 = not firing, 1.0 = maximum firing
    pub phi: f32,

    /// Timestamp when activation was recorded
    #[serde(skip)]
    pub timestamp: Option<Instant>,
}

impl NodeActivation {
    /// Create a new node activation.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The node's UUID (must be valid, not nil)
    /// * `phi` - Activation level (clamped to [0.0, 1.0])
    ///
    /// # Panics
    ///
    /// Panics if `node_id` is nil UUID.
    #[track_caller]
    pub fn new(node_id: Uuid, phi: f32) -> Self {
        assert!(
            !node_id.is_nil(),
            "NodeActivation: node_id cannot be nil UUID"
        );
        Self {
            node_id,
            phi: phi.clamp(0.0, 1.0),
            timestamp: Some(Instant::now()),
        }
    }

    /// Check if this is a significant activation (> 0.1)
    pub fn is_significant(&self) -> bool {
        self.phi > 0.1
    }
}

// ============================================================================
// HYPERBOLIC WALK TYPES
// ============================================================================

/// Configuration for hyperbolic random walk in REM phase.
///
/// Constitution Reference: `docs2/constitution.yaml` lines 394-396
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HyperbolicWalkConfig {
    /// Step size in Poincare ball (Euclidean distance before Mobius add)
    /// Default: 0.1
    pub step_size: f32,

    /// Maximum steps per walk before termination
    /// Default: 50
    pub max_steps: usize,

    /// Exploration temperature for direction sampling
    /// Constitution: 2.0 (docs2/constitution.yaml line 395)
    pub temperature: f32,

    /// Minimum distance from nearest memory to consider a blind spot
    /// Constitution: semantic_leap >= 0.7 (docs2/constitution.yaml line 397)
    pub min_blind_spot_distance: f32,

    /// Number of random direction samples per step
    /// Default: 8
    pub direction_samples: usize,
}

impl Default for HyperbolicWalkConfig {
    fn default() -> Self {
        Self {
            step_size: 0.1,
            max_steps: 50,
            temperature: 2.0,              // Constitution mandated
            min_blind_spot_distance: 0.7,  // Constitution: semantic_leap >= 0.7
            direction_samples: 8,
        }
    }
}

impl HyperbolicWalkConfig {
    /// Validate configuration against constitution bounds.
    #[track_caller]
    pub fn validate(&self) {
        assert!(
            self.step_size > 0.0 && self.step_size < 1.0,
            "HyperbolicWalkConfig: step_size={} must be in (0.0, 1.0) to stay in Poincare ball",
            self.step_size
        );
        assert!(
            self.max_steps > 0,
            "HyperbolicWalkConfig: max_steps={} must be > 0",
            self.max_steps
        );
        assert!(
            self.temperature > 0.0,
            "HyperbolicWalkConfig: temperature={} must be > 0.0, constitution default=2.0",
            self.temperature
        );
        assert!(
            self.min_blind_spot_distance >= 0.7,
            "HyperbolicWalkConfig: min_blind_spot_distance={} must be >= 0.7 per constitution semantic_leap",
            self.min_blind_spot_distance
        );
        assert!(
            self.direction_samples >= 1,
            "HyperbolicWalkConfig: direction_samples={} must be >= 1",
            self.direction_samples
        );
    }
}

/// A single step in the hyperbolic random walk.
///
/// Records position, direction, and distance for trajectory analysis.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalkStep {
    /// Current position in Poincare ball (64D)
    /// INVARIANT: norm(position) < 1.0
    #[serde_as(as = "[_; 64]")]
    pub position: [f32; 64],

    /// Direction of the step taken (unit vector before scaling)
    #[serde_as(as = "[_; 64]")]
    pub step_direction: [f32; 64],

    /// Geodesic distance from walk start position
    pub distance_from_start: f32,

    /// Step index in the walk (0-based)
    pub step_index: usize,

    /// Whether a blind spot was detected at this position
    pub blind_spot_detected: bool,
}

impl WalkStep {
    /// Create a new walk step.
    ///
    /// # Panics
    ///
    /// Panics if position is outside Poincare ball (norm >= 1.0).
    #[track_caller]
    pub fn new(
        position: [f32; 64],
        direction: [f32; 64],
        distance: f32,
        index: usize,
    ) -> Self {
        let norm_sq: f32 = position.iter().map(|x| x * x).sum();
        assert!(
            norm_sq < 1.0,
            "WalkStep: position norm={:.6} must be < 1.0 (inside Poincare ball)",
            norm_sq.sqrt()
        );

        Self {
            position,
            step_direction: direction,
            distance_from_start: distance,
            step_index: index,
            blind_spot_detected: false,
        }
    }

    /// Mark this step as containing a blind spot.
    pub fn mark_blind_spot(&mut self) {
        self.blind_spot_detected = true;
    }

    /// Get the Euclidean norm of the position.
    pub fn position_norm(&self) -> f32 {
        self.position.iter().map(|x| x * x).sum::<f32>().sqrt()
    }
}

// ============================================================================
// TRIGGER TYPES
// ============================================================================

/// Entropy tracking window for dream trigger.
///
/// Monitors system entropy over a sliding window to detect
/// sustained high entropy (> 0.7 for 5 minutes).
///
/// Constitution Reference: Implicit from dream trigger requirements
#[derive(Debug, Clone)]
pub struct EntropyWindow {
    /// Entropy samples with timestamps
    samples: VecDeque<(Instant, f32)>,

    /// Duration of the sliding window
    /// Constitution: 5 minutes
    pub window_duration: Duration,

    /// Entropy threshold for trigger
    /// Constitution: 0.7
    pub threshold: f32,

    /// When continuous high entropy started (None if below threshold)
    high_entropy_since: Option<Instant>,
}

impl EntropyWindow {
    /// Create a new entropy window with constitution defaults.
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(300), // 5 min at 1 sample/sec
            window_duration: Duration::from_secs(300), // 5 minutes
            threshold: 0.7,
            high_entropy_since: None,
        }
    }

    /// Create with custom parameters (for testing with REAL time).
    ///
    /// # Note
    ///
    /// Tests MUST use real durations, not mocked time.
    pub fn with_params(window_duration: Duration, threshold: f32) -> Self {
        assert!((0.0..=1.0).contains(&threshold), "threshold must be in [0.0, 1.0]");
        let capacity = (window_duration.as_secs() + 1) as usize;
        Self {
            samples: VecDeque::with_capacity(capacity),
            window_duration,
            threshold,
            high_entropy_since: None,
        }
    }

    /// Add an entropy sample.
    pub fn push(&mut self, entropy: f32) {
        let now = Instant::now();
        let clamped_entropy = entropy.clamp(0.0, 1.0);
        self.samples.push_back((now, clamped_entropy));
        self.prune_old_samples(now);

        // Track continuous high entropy period
        if clamped_entropy > self.threshold {
            // Above threshold - start tracking if not already
            if self.high_entropy_since.is_none() {
                self.high_entropy_since = Some(now);
            }
        } else {
            // Below threshold - reset tracking
            self.high_entropy_since = None;
        }
    }

    /// Check if entropy trigger condition is met.
    ///
    /// Returns true if entropy has been continuously above threshold
    /// for at least window_duration.
    pub fn should_trigger(&self) -> bool {
        match self.high_entropy_since {
            Some(since) => {
                let now = Instant::now();
                now.duration_since(since) >= self.window_duration
            }
            None => false,
        }
    }

    /// Get current average entropy.
    pub fn average(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.samples.iter().map(|(_, e)| e).sum();
        sum / self.samples.len() as f32
    }

    /// Get minimum entropy in window.
    pub fn minimum(&self) -> f32 {
        self.samples
            .iter()
            .map(|(_, e)| *e)
            .fold(f32::INFINITY, f32::min)
    }

    /// Get the number of samples in window.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Check if window is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Clear all samples.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.high_entropy_since = None;
    }

    fn prune_old_samples(&mut self, now: Instant) {
        while let Some((timestamp, _)) = self.samples.front() {
            if now.duration_since(*timestamp) > self.window_duration {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }
}

impl Default for EntropyWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU utilization trigger state.
///
/// Monitors GPU usage to ensure dreams stay within budget.
///
/// # Constitution Compliance
///
/// GPU usage MUST stay below 30% during dreams.
/// Reference: `docs2/constitution.yaml` line 398: `gpu_usage: "<30%"`
#[derive(Debug, Clone)]
pub struct GpuTriggerState {
    /// Current GPU usage [0.0, 1.0]
    pub current_usage: f32,

    /// Threshold for trigger
    /// Constitution: 0.30 (30%) - NOT 80%
    /// Reference: docs2/constitution.yaml line 398
    pub threshold: f32,

    /// Recent usage samples for smoothing
    samples: VecDeque<f32>,

    /// Maximum samples to retain
    max_samples: usize,

    /// Whether trigger has fired (reset after dream)
    pub triggered: bool,
}

impl GpuTriggerState {
    /// Create a new GPU trigger state with constitution defaults.
    ///
    /// Threshold is 0.30 (30%) per constitution, NOT 80%.
    pub fn new() -> Self {
        Self {
            current_usage: 0.0,
            threshold: 0.30,  // CORRECTED: Constitution says <30%, not 80%
            samples: VecDeque::with_capacity(10),
            max_samples: 10,
            triggered: false,
        }
    }

    /// Create with custom threshold (for testing).
    ///
    /// # Panics
    ///
    /// Panics if threshold > 0.30 (violates constitution).
    #[track_caller]
    pub fn with_threshold(threshold: f32) -> Self {
        assert!(
            threshold <= 0.30,
            "GpuTriggerState: threshold={} cannot exceed 0.30 per constitution gpu_usage: '<30%'",
            threshold
        );
        let mut state = Self::new();
        state.threshold = threshold.clamp(0.0, 0.30);
        state
    }

    /// Update with new GPU usage reading.
    pub fn update(&mut self, usage: f32) {
        let usage = usage.clamp(0.0, 1.0);

        self.samples.push_back(usage);
        while self.samples.len() > self.max_samples {
            self.samples.pop_front();
        }

        self.current_usage = self.average();
    }

    /// Check if GPU usage exceeds threshold (should abort dream).
    pub fn should_trigger(&self) -> bool {
        !self.triggered && self.current_usage >= self.threshold
    }

    /// Mark trigger as fired.
    pub fn mark_triggered(&mut self) {
        self.triggered = true;
    }

    /// Reset trigger state after dream completes.
    pub fn reset(&mut self) {
        self.triggered = false;
        self.samples.clear();
        self.current_usage = 0.0;
    }

    /// Get smoothed average usage.
    pub fn average(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.samples.iter().sum();
        sum / self.samples.len() as f32
    }
}

impl Default for GpuTriggerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Reason for triggering a dream cycle (extended).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtendedTriggerReason {
    /// Activity below 0.15 for idle_duration (10 min)
    IdleTimeout,

    /// Entropy above 0.7 for 5 minutes
    HighEntropy,

    /// GPU usage approaching threshold (consolidation needed)
    GpuOverload,

    /// Memory pressure requires consolidation
    MemoryPressure,

    /// Manual trigger by user/system
    Manual,

    /// Scheduled dream time
    Scheduled,
}

impl std::fmt::Display for ExtendedTriggerReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IdleTimeout => write!(f, "idle_timeout"),
            Self::HighEntropy => write!(f, "high_entropy"),
            Self::GpuOverload => write!(f, "gpu_overload"),
            Self::MemoryPressure => write!(f, "memory_pressure"),
            Self::Manual => write!(f, "manual"),
            Self::Scheduled => write!(f, "scheduled"),
        }
    }
}

// ============================================================================
// TESTS - NO MOCK DATA
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    // All tests use real UUIDs and real time

    #[test]
    fn test_hebbian_config_constitution_compliance() {
        let config = HebbianConfig::default();

        // Constitution mandated values
        assert_eq!(config.learning_rate, 0.01, "learning_rate must match constitution");
        assert_eq!(config.weight_decay, 0.001, "weight_decay must match constitution");
        assert_eq!(config.weight_floor, 0.05, "weight_floor must match constitution");
        assert_eq!(config.weight_cap, 1.0, "weight_cap must match constitution");

        // CRITICAL: coupling_strength is 0.9, NOT 10.0
        assert_eq!(config.coupling_strength, 0.9,
            "coupling_strength must be 0.9 per constitution line 393, NOT 10.0");
    }

    #[test]
    fn test_hebbian_config_validation_passes() {
        let config = HebbianConfig::default();
        config.validate(); // Should not panic
    }

    #[test]
    #[should_panic(expected = "learning_rate")]
    fn test_hebbian_config_validation_rejects_bad_learning_rate() {
        let mut config = HebbianConfig::default();
        config.learning_rate = -0.1;
        config.validate();
    }

    #[test]
    fn test_node_activation_uses_real_uuid() {
        let real_uuid = Uuid::new_v4(); // Real UUID, not nil
        let activation = NodeActivation::new(real_uuid, 0.5);

        assert_eq!(activation.node_id, real_uuid);
        assert_eq!(activation.phi, 0.5);
        assert!(activation.timestamp.is_some());
    }

    #[test]
    #[should_panic(expected = "nil UUID")]
    fn test_node_activation_rejects_nil_uuid() {
        NodeActivation::new(Uuid::nil(), 0.5);
    }

    #[test]
    fn test_node_activation_clamping() {
        let uuid = Uuid::new_v4();

        let over = NodeActivation::new(uuid, 1.5);
        assert_eq!(over.phi, 1.0, "phi must clamp to 1.0");

        let under = NodeActivation::new(uuid, -0.5);
        assert_eq!(under.phi, 0.0, "phi must clamp to 0.0");
    }

    #[test]
    fn test_node_activation_significance_threshold() {
        let uuid = Uuid::new_v4();

        assert!(NodeActivation::new(uuid, 0.5).is_significant());
        assert!(NodeActivation::new(uuid, 0.11).is_significant());
        assert!(!NodeActivation::new(uuid, 0.1).is_significant());
        assert!(!NodeActivation::new(uuid, 0.05).is_significant());
    }

    #[test]
    fn test_hyperbolic_walk_config_constitution_compliance() {
        let config = HyperbolicWalkConfig::default();

        assert_eq!(config.temperature, 2.0, "temperature must be 2.0 per constitution");
        assert_eq!(config.min_blind_spot_distance, 0.7, "semantic_leap must be 0.7 per constitution");
    }

    #[test]
    fn test_walk_step_validates_poincare_ball() {
        // Valid position inside ball
        let mut valid_pos = [0.0f32; 64];
        valid_pos[0] = 0.5; // norm = 0.5 < 1.0
        let step = WalkStep::new(valid_pos, [0.0; 64], 0.0, 0);
        assert!(step.position_norm() < 1.0);
    }

    #[test]
    #[should_panic(expected = "inside Poincare ball")]
    fn test_walk_step_rejects_outside_ball() {
        let invalid_pos = [0.2f32; 64]; // norm = sqrt(64 * 0.04) = 1.6 > 1.0
        WalkStep::new(invalid_pos, [0.0; 64], 0.0, 0);
    }

    #[test]
    fn test_entropy_window_uses_real_time() {
        // Use short window with REAL time, not mocked
        // Window duration is 50ms
        let mut window = EntropyWindow::with_params(Duration::from_millis(50), 0.7);

        // Push first sample above threshold - starts tracking
        window.push(0.8);
        assert!(!window.should_trigger(), "should not trigger immediately");

        // Wait for half the window duration
        thread::sleep(Duration::from_millis(25));
        window.push(0.85); // Still above threshold
        assert!(!window.should_trigger(), "should not trigger at half window");

        // Wait for remaining time plus margin
        thread::sleep(Duration::from_millis(30));
        window.push(0.9); // Still above threshold

        // Now we've had high entropy for ~55ms >= 50ms window
        assert!(window.should_trigger(),
            "should trigger after sustained high entropy for window duration");
    }

    #[test]
    fn test_entropy_window_below_threshold_resets_tracking() {
        let mut window = EntropyWindow::with_params(Duration::from_millis(50), 0.7);

        // Start with high entropy at t=0
        window.push(0.8);
        thread::sleep(Duration::from_millis(30));

        // t=30: Drop below threshold - resets tracking
        window.push(0.5);
        assert!(!window.should_trigger(), "should not trigger after dropping below threshold");

        // t=30: Resume high entropy - tracking restarts from here
        window.push(0.9);
        assert!(!window.should_trigger(), "should not trigger immediately after resuming");

        // t=60: Still not enough time since tracking restarted at t=30
        thread::sleep(Duration::from_millis(30));
        window.push(0.92);
        assert!(!window.should_trigger(),
            "should NOT trigger - only 30ms since tracking restarted");

        // t=90: Now 60ms since tracking restarted at t=30 (60ms > 50ms window)
        thread::sleep(Duration::from_millis(30));
        window.push(0.95);
        assert!(window.should_trigger(),
            "should trigger after sustained high entropy following reset (60ms > 50ms window)");
    }

    #[test]
    fn test_gpu_trigger_constitution_compliance() {
        let state = GpuTriggerState::new();

        // CRITICAL: threshold is 0.30, NOT 0.80
        assert_eq!(state.threshold, 0.30,
            "GPU threshold must be 0.30 per constitution line 398: 'gpu_usage: <30%'");
    }

    #[test]
    #[should_panic(expected = "cannot exceed 0.30")]
    fn test_gpu_trigger_rejects_high_threshold() {
        GpuTriggerState::with_threshold(0.80); // Should panic
    }

    #[test]
    fn test_gpu_trigger_threshold_behavior() {
        let mut state = GpuTriggerState::new();

        state.update(0.25);
        assert!(!state.should_trigger(), "0.25 < 0.30 should not trigger");

        state.update(0.35);
        assert!(state.should_trigger(), "0.35 > 0.30 should trigger");
    }

    #[test]
    fn test_gpu_trigger_once_only() {
        let mut state = GpuTriggerState::new();

        state.update(0.35);
        assert!(state.should_trigger());

        state.mark_triggered();
        assert!(!state.should_trigger(), "should not trigger again until reset");
    }

    #[test]
    fn test_gpu_trigger_reset_allows_retrigger() {
        let mut state = GpuTriggerState::new();

        state.update(0.35);
        state.mark_triggered();
        state.reset();

        state.update(0.35);
        assert!(state.should_trigger(), "should trigger again after reset");
    }

    #[test]
    fn test_extended_trigger_reason_display() {
        assert_eq!(ExtendedTriggerReason::IdleTimeout.to_string(), "idle_timeout");
        assert_eq!(ExtendedTriggerReason::HighEntropy.to_string(), "high_entropy");
        assert_eq!(ExtendedTriggerReason::GpuOverload.to_string(), "gpu_overload");
        assert_eq!(ExtendedTriggerReason::MemoryPressure.to_string(), "memory_pressure");
        assert_eq!(ExtendedTriggerReason::Manual.to_string(), "manual");
        assert_eq!(ExtendedTriggerReason::Scheduled.to_string(), "scheduled");
    }
}
