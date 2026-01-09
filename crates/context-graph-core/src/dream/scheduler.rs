//! DreamScheduler - Trigger logic for when to enter dream state
//!
//! The DreamScheduler monitors system activity and determines when to
//! trigger a dream cycle based on constitution-mandated criteria.
//!
//! ## Trigger Conditions (Constitution Section dream.trigger, line 446)
//!
//! - Activity level < 0.15 for 10 minutes
//! - No active queries
//! - Sufficient time since last dream cycle
//!
//! ## Usage
//!
//! ```ignore
//! let mut scheduler = DreamScheduler::new();
//!
//! // Update with activity measurements
//! scheduler.update_activity(0.1);
//!
//! // Check if dream should trigger
//! if scheduler.should_trigger_dream() {
//!     // Start dream cycle
//! }
//! ```

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::debug;

use super::constants;

/// Maximum number of activity samples to retain
const MAX_ACTIVITY_SAMPLES: usize = 60;

/// Minimum cooldown between dream cycles (30 minutes)
const DREAM_COOLDOWN: Duration = Duration::from_secs(1800);

/// Scheduler for determining when to trigger dream cycles
#[derive(Debug, Clone)]
pub struct DreamScheduler {
    /// Activity threshold below which dream may trigger (Constitution: 0.15)
    activity_threshold: f32,

    /// Duration of low activity required (Constitution: 10 minutes)
    idle_duration_trigger: Duration,

    /// Last recorded activity timestamp
    last_activity: Option<Instant>,

    /// Recent activity samples for averaging
    activity_samples: VecDeque<ActivitySample>,

    /// Last dream completion time
    last_dream_completed: Option<Instant>,

    /// Time when low activity period started
    low_activity_start: Option<Instant>,

    /// Current computed average activity
    average_activity: f32,
}

/// A single activity measurement with timestamp
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct ActivitySample {
    /// Activity level [0.0, 1.0]
    activity: f32,
    /// When this sample was recorded
    timestamp: Instant,
}

/// Decision about whether to trigger a dream cycle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TriggerDecision {
    /// Trigger dream cycle now
    Trigger(TriggerReason),
    /// Wait for conditions to be met
    Wait(WaitReason),
    /// Blocked from triggering
    Blocked(BlockReason),
}

/// Reason for triggering a dream cycle
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TriggerReason {
    /// Activity has been below threshold for required duration
    IdleTimeout,
    /// Memory pressure requires consolidation
    MemoryPressure,
    /// Manually triggered by user or system
    Manual,
    /// Scheduled dream time
    Scheduled,
}

/// Reason for waiting to trigger
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WaitReason {
    /// Not enough time has passed with low activity
    InsufficientIdleTime {
        current: Duration,
        required: Duration,
    },
    /// Activity level is too high
    HighActivity { current: f32, threshold: f32 },
}

/// Reason dream is blocked from triggering
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BlockReason {
    /// Cooldown period active from recent dream
    CooldownActive { remaining: Duration },
    /// Dream already in progress
    DreamInProgress,
    /// Resources unavailable
    ResourcesUnavailable,
}

impl DreamScheduler {
    /// Create a new DreamScheduler with constitution-mandated defaults
    pub fn new() -> Self {
        Self {
            activity_threshold: constants::ACTIVITY_THRESHOLD,
            idle_duration_trigger: constants::IDLE_DURATION_TRIGGER,
            last_activity: None,
            activity_samples: VecDeque::with_capacity(MAX_ACTIVITY_SAMPLES),
            last_dream_completed: None,
            low_activity_start: None,
            average_activity: 1.0, // Start high to prevent immediate trigger
        }
    }

    /// Create with custom thresholds (for testing)
    pub fn with_thresholds(activity_threshold: f32, idle_duration: Duration) -> Self {
        let mut scheduler = Self::new();
        scheduler.activity_threshold = activity_threshold;
        scheduler.idle_duration_trigger = idle_duration;
        scheduler
    }

    /// Update the scheduler with a new activity measurement
    ///
    /// Activity should be in the range [0.0, 1.0] where:
    /// - 0.0 = no activity (idle)
    /// - 1.0 = maximum activity
    pub fn update_activity(&mut self, activity: f32) {
        let activity = activity.clamp(0.0, 1.0);
        let now = Instant::now();

        // Record sample
        self.activity_samples.push_back(ActivitySample {
            activity,
            timestamp: now,
        });

        // Trim old samples (keep last MAX_ACTIVITY_SAMPLES)
        while self.activity_samples.len() > MAX_ACTIVITY_SAMPLES {
            self.activity_samples.pop_front();
        }

        // Update average
        self.average_activity = self.compute_average_activity();

        // Track low activity period
        if activity < self.activity_threshold {
            if self.low_activity_start.is_none() {
                debug!(
                    "Low activity period started: activity={:.3} < threshold={:.3}",
                    activity, self.activity_threshold
                );
                self.low_activity_start = Some(now);
            }
        } else {
            if self.low_activity_start.is_some() {
                debug!(
                    "Low activity period ended: activity={:.3} >= threshold={:.3}",
                    activity, self.activity_threshold
                );
            }
            self.low_activity_start = None;
        }

        self.last_activity = Some(now);
    }

    /// Check if a dream cycle should be triggered
    ///
    /// Returns true if:
    /// - Activity has been below threshold for idle_duration_trigger
    /// - No cooldown is active
    pub fn should_trigger_dream(&self) -> bool {
        matches!(self.check_trigger(), TriggerDecision::Trigger(_))
    }

    /// Get detailed trigger decision with reasons
    pub fn check_trigger(&self) -> TriggerDecision {
        // Check cooldown
        if let Some(last_dream) = self.last_dream_completed {
            let elapsed = last_dream.elapsed();
            if elapsed < DREAM_COOLDOWN {
                return TriggerDecision::Blocked(BlockReason::CooldownActive {
                    remaining: DREAM_COOLDOWN - elapsed,
                });
            }
        }

        // Check if low activity period is active
        let Some(low_start) = self.low_activity_start else {
            return TriggerDecision::Wait(WaitReason::HighActivity {
                current: self.average_activity,
                threshold: self.activity_threshold,
            });
        };

        // Check if idle duration is met
        let idle_duration = low_start.elapsed();
        if idle_duration >= self.idle_duration_trigger {
            debug!(
                "Dream trigger condition met: idle {:?} >= required {:?}",
                idle_duration, self.idle_duration_trigger
            );
            return TriggerDecision::Trigger(TriggerReason::IdleTimeout);
        }

        TriggerDecision::Wait(WaitReason::InsufficientIdleTime {
            current: idle_duration,
            required: self.idle_duration_trigger,
        })
    }

    /// Record that a dream cycle has completed
    pub fn record_dream_completion(&mut self) {
        self.last_dream_completed = Some(Instant::now());
        self.low_activity_start = None;
        debug!("Dream completion recorded");
    }

    /// Get time since last activity update
    pub fn time_since_last_activity(&self) -> Duration {
        self.last_activity
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Get the current average activity level
    pub fn get_average_activity(&self) -> f32 {
        self.average_activity
    }

    /// Get time remaining in cooldown, if any
    pub fn cooldown_remaining(&self) -> Option<Duration> {
        self.last_dream_completed.and_then(|last| {
            let elapsed = last.elapsed();
            if elapsed < DREAM_COOLDOWN {
                Some(DREAM_COOLDOWN - elapsed)
            } else {
                None
            }
        })
    }

    /// Get the current idle duration if in low activity period
    pub fn current_idle_duration(&self) -> Option<Duration> {
        self.low_activity_start.map(|start| start.elapsed())
    }

    /// Compute average activity from recent samples
    fn compute_average_activity(&self) -> f32 {
        if self.activity_samples.is_empty() {
            return 1.0;
        }

        let sum: f32 = self.activity_samples.iter().map(|s| s.activity).sum();
        sum / self.activity_samples.len() as f32
    }

    /// Get the activity threshold
    pub fn activity_threshold(&self) -> f32 {
        self.activity_threshold
    }

    /// Get the required idle duration
    pub fn idle_duration_trigger(&self) -> Duration {
        self.idle_duration_trigger
    }

    /// Force trigger (for testing/manual override)
    pub fn force_trigger(&mut self) {
        self.low_activity_start = Some(Instant::now() - self.idle_duration_trigger);
        self.last_dream_completed = None;
    }
}

impl Default for DreamScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = DreamScheduler::new();

        assert_eq!(scheduler.activity_threshold, 0.15);
        assert_eq!(scheduler.idle_duration_trigger, Duration::from_secs(600));
        assert!(!scheduler.should_trigger_dream());
    }

    #[test]
    fn test_activity_threshold_constitution_compliance() {
        // Constitution mandates activity < 0.15 for 10 minutes
        let scheduler = DreamScheduler::new();

        assert_eq!(scheduler.activity_threshold, constants::ACTIVITY_THRESHOLD);
        assert_eq!(
            scheduler.idle_duration_trigger,
            constants::IDLE_DURATION_TRIGGER
        );
    }

    #[test]
    fn test_activity_updates() {
        let mut scheduler = DreamScheduler::new();

        scheduler.update_activity(0.5);
        assert!(scheduler.average_activity > 0.0);
        assert!(scheduler.low_activity_start.is_none());

        scheduler.update_activity(0.1);
        assert!(scheduler.low_activity_start.is_some());
    }

    #[test]
    fn test_high_activity_prevents_trigger() {
        let mut scheduler = DreamScheduler::new();

        // High activity should not trigger
        for _ in 0..10 {
            scheduler.update_activity(0.8);
        }

        assert!(!scheduler.should_trigger_dream());

        match scheduler.check_trigger() {
            TriggerDecision::Wait(WaitReason::HighActivity { .. }) => {}
            other => panic!("Expected HighActivity wait reason, got {:?}", other),
        }
    }

    #[test]
    fn test_low_activity_triggers_after_duration() {
        // Use short idle duration for testing
        let mut scheduler = DreamScheduler::with_thresholds(0.15, Duration::from_millis(10));

        // Update with low activity
        scheduler.update_activity(0.05);

        // Wait for the idle duration
        std::thread::sleep(Duration::from_millis(15));

        // Update again to process
        scheduler.update_activity(0.05);

        assert!(scheduler.should_trigger_dream());
    }

    #[test]
    fn test_cooldown_prevents_trigger() {
        let mut scheduler = DreamScheduler::with_thresholds(0.15, Duration::from_millis(10));

        // Record a dream completion
        scheduler.record_dream_completion();

        // Even with low activity, cooldown should prevent trigger
        scheduler.update_activity(0.05);
        std::thread::sleep(Duration::from_millis(15));
        scheduler.update_activity(0.05);

        assert!(!scheduler.should_trigger_dream());

        match scheduler.check_trigger() {
            TriggerDecision::Blocked(BlockReason::CooldownActive { .. }) => {}
            other => panic!("Expected CooldownActive block reason, got {:?}", other),
        }
    }

    #[test]
    fn test_force_trigger() {
        let mut scheduler = DreamScheduler::new();

        // Force trigger should work
        scheduler.force_trigger();

        assert!(scheduler.should_trigger_dream());
    }

    #[test]
    fn test_average_activity_computation() {
        let mut scheduler = DreamScheduler::new();

        scheduler.update_activity(0.2);
        scheduler.update_activity(0.4);
        scheduler.update_activity(0.6);

        // Average should be (0.2 + 0.4 + 0.6) / 3 = 0.4
        let avg = scheduler.get_average_activity();
        assert!((avg - 0.4).abs() < 0.01, "Expected ~0.4, got {}", avg);
    }
}
