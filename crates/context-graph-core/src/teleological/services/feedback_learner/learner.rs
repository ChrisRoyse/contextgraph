//! TELEO-014: Feedback Learning Loop Service.
//!
//! Collects feedback from embedder interactions and uses gradient-based learning
//! to optimize embedder weights for improved teleological alignment.

use super::config::FeedbackLearnerConfig;
use super::types::{FeedbackEvent, FeedbackType, LearningResult};
use crate::teleological::types::NUM_EMBEDDERS;

/// TELEO-014: Feedback Learning Loop Service.
///
/// Collects feedback from embedder interactions and uses gradient-based learning
/// to optimize embedder weights for improved teleological alignment.
///
/// # Example
///
/// ```
/// use uuid::Uuid;
/// use context_graph_core::teleological::services::feedback_learner::{
///     FeedbackLearner, FeedbackEvent, FeedbackType,
/// };
///
/// let mut learner = FeedbackLearner::new();
///
/// // Record positive feedback
/// for i in 0..15 {
///     let event = FeedbackEvent::new(
///         Uuid::new_v4(),
///         FeedbackType::Positive { magnitude: 0.8 },
///         i as u64 * 1000,
///     );
///     learner.record_feedback(event);
/// }
///
/// // Learn from accumulated feedback
/// if learner.should_learn() {
///     let result = learner.learn();
///     assert!(result.events_processed >= 10);
/// }
/// ```
pub struct FeedbackLearner {
    /// Service configuration
    config: FeedbackLearnerConfig,
    /// Buffer of feedback events awaiting processing
    feedback_buffer: Vec<FeedbackEvent>,
    /// Accumulated per-embedder adjustments
    adjustments: [f32; NUM_EMBEDDERS],
    /// Momentum buffer for gradient smoothing
    momentum_buffer: [f32; NUM_EMBEDDERS],
    /// Total feedback events processed
    total_events: usize,
    /// Cumulative confidence delta
    cumulative_confidence_delta: f32,
}

impl FeedbackLearner {
    /// Create a new FeedbackLearner with default configuration.
    pub fn new() -> Self {
        Self {
            config: FeedbackLearnerConfig::default(),
            feedback_buffer: Vec::new(),
            adjustments: [0.0; NUM_EMBEDDERS],
            momentum_buffer: [0.0; NUM_EMBEDDERS],
            total_events: 0,
            cumulative_confidence_delta: 0.0,
        }
    }

    /// Create with custom configuration.
    pub fn with_config(config: FeedbackLearnerConfig) -> Self {
        assert!(
            config.learning_rate > 0.0 && config.learning_rate <= 1.0,
            "FAIL FAST: learning_rate must be in (0.0, 1.0], got {}",
            config.learning_rate
        );
        assert!(
            config.momentum >= 0.0 && config.momentum < 1.0,
            "FAIL FAST: momentum must be in [0.0, 1.0), got {}",
            config.momentum
        );
        assert!(
            config.reward_scale > 0.0,
            "FAIL FAST: reward_scale must be positive, got {}",
            config.reward_scale
        );
        assert!(
            config.penalty_scale > 0.0,
            "FAIL FAST: penalty_scale must be positive, got {}",
            config.penalty_scale
        );
        assert!(
            config.min_feedback_count > 0,
            "FAIL FAST: min_feedback_count must be positive"
        );

        Self {
            config,
            feedback_buffer: Vec::new(),
            adjustments: [0.0; NUM_EMBEDDERS],
            momentum_buffer: [0.0; NUM_EMBEDDERS],
            total_events: 0,
            cumulative_confidence_delta: 0.0,
        }
    }

    /// Record a feedback event for later learning.
    ///
    /// Events are buffered until `learn()` is called.
    pub fn record_feedback(&mut self, event: FeedbackEvent) {
        self.feedback_buffer.push(event);
    }

    /// Check if enough feedback has been collected to learn.
    ///
    /// Returns true if feedback count >= min_feedback_count.
    #[inline]
    pub fn should_learn(&self) -> bool {
        self.feedback_buffer.len() >= self.config.min_feedback_count
    }

    /// Perform learning from accumulated feedback.
    ///
    /// Computes gradients, applies momentum, and updates adjustments.
    /// Clears the feedback buffer after learning.
    ///
    /// # Returns
    ///
    /// A `LearningResult` containing the computed adjustments and confidence delta.
    pub fn learn(&mut self) -> LearningResult {
        if self.feedback_buffer.is_empty() {
            return LearningResult::new(vec![0.0; NUM_EMBEDDERS], 0.0, 0);
        }

        let events_count = self.feedback_buffer.len();

        // Compute gradient from buffered events
        let gradient = self.compute_gradient(&self.feedback_buffer);

        // Apply gradient with momentum
        self.apply_gradient(&gradient);

        // Compute confidence delta (positive feedback increases confidence)
        let confidence_delta = self.compute_confidence_delta(&self.feedback_buffer);
        self.cumulative_confidence_delta += confidence_delta;

        // Update total events
        self.total_events += events_count;

        // Clear buffer
        self.feedback_buffer.clear();

        LearningResult::new(self.adjustments.to_vec(), confidence_delta, events_count)
    }

    /// Compute gradient from feedback events.
    ///
    /// For each embedder, the gradient is the sum of:
    /// `contribution[i] * reward_value` across all events
    ///
    /// Positive feedback creates positive gradients, negative creates negative.
    pub fn compute_gradient(&self, events: &[FeedbackEvent]) -> Vec<f32> {
        let mut gradient = vec![0.0f32; NUM_EMBEDDERS];

        if events.is_empty() {
            return gradient;
        }

        for event in events {
            let reward = event.feedback_type.reward_value(&self.config);

            for (i, &contrib) in event.embedder_contributions.iter().enumerate() {
                // Gradient = contribution * reward (positive or negative)
                gradient[i] += contrib * reward;
            }
        }

        // Normalize by event count to get average gradient
        let n = events.len() as f32;
        for g in &mut gradient {
            *g /= n;
        }

        gradient
    }

    /// Apply gradient with momentum to update adjustments.
    ///
    /// Uses momentum-based SGD: `m = momentum * m + (1 - momentum) * gradient`
    /// Then: `adjustment += learning_rate * m`
    pub fn apply_gradient(&mut self, gradient: &[f32]) {
        assert!(
            gradient.len() == NUM_EMBEDDERS,
            "FAIL FAST: gradient must have {} elements, got {}",
            NUM_EMBEDDERS,
            gradient.len()
        );

        let lr = self.config.learning_rate;
        let momentum = self.config.momentum;

        for ((momentum_buf, adj), &grad) in self
            .momentum_buffer
            .iter_mut()
            .zip(self.adjustments.iter_mut())
            .zip(gradient.iter())
        {
            // Update momentum buffer with exponential moving average
            *momentum_buf = momentum * *momentum_buf + (1.0 - momentum) * grad;

            // Apply learning rate to momentum
            *adj += lr * *momentum_buf;

            // Clamp adjustments to reasonable range [-1.0, 1.0]
            *adj = adj.clamp(-1.0, 1.0);
        }
    }

    /// Compute confidence delta from feedback events.
    ///
    /// Positive feedback increases confidence, negative decreases it.
    fn compute_confidence_delta(&self, events: &[FeedbackEvent]) -> f32 {
        if events.is_empty() {
            return 0.0;
        }

        let mut delta = 0.0f32;

        for event in events {
            match &event.feedback_type {
                FeedbackType::Positive { magnitude } => {
                    delta += magnitude * 0.01; // Small increase per positive
                }
                FeedbackType::Negative { magnitude } => {
                    delta -= magnitude * 0.02; // Larger decrease per negative
                }
                FeedbackType::Neutral => {}
            }
        }

        // Normalize by total count
        let total = events.len() as f32;
        delta / total
    }

    /// Get the current adjustment for a specific embedder.
    ///
    /// # Arguments
    /// * `idx` - Embedder index (0-12)
    ///
    /// # Panics
    ///
    /// Panics if idx >= NUM_EMBEDDERS (FAIL FAST).
    #[inline]
    pub fn get_adjustment_for_embedder(&self, idx: usize) -> f32 {
        assert!(
            idx < NUM_EMBEDDERS,
            "FAIL FAST: embedder index {} out of bounds (max {})",
            idx,
            NUM_EMBEDDERS - 1
        );
        self.adjustments[idx]
    }

    /// Get all current adjustments.
    #[inline]
    pub fn get_all_adjustments(&self) -> &[f32; NUM_EMBEDDERS] {
        &self.adjustments
    }

    /// Reset the feedback buffer without learning.
    pub fn reset_feedback_buffer(&mut self) {
        self.feedback_buffer.clear();
    }

    /// Get the current feedback buffer size.
    #[inline]
    pub fn feedback_buffer_size(&self) -> usize {
        self.feedback_buffer.len()
    }

    /// Get the total number of events processed.
    #[inline]
    pub fn total_events_processed(&self) -> usize {
        self.total_events
    }

    /// Get the cumulative confidence delta.
    #[inline]
    pub fn cumulative_confidence_delta(&self) -> f32 {
        self.cumulative_confidence_delta
    }

    /// Get the current configuration.
    #[inline]
    pub fn config(&self) -> &FeedbackLearnerConfig {
        &self.config
    }

    /// Reset all state (adjustments, momentum, counters).
    pub fn reset(&mut self) {
        self.feedback_buffer.clear();
        self.adjustments = [0.0; NUM_EMBEDDERS];
        self.momentum_buffer = [0.0; NUM_EMBEDDERS];
        self.total_events = 0;
        self.cumulative_confidence_delta = 0.0;
    }
}

impl Default for FeedbackLearner {
    fn default() -> Self {
        Self::new()
    }
}
