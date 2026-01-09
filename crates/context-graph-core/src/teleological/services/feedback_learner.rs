//! TASK-TELEO-014: FeedbackLearner Implementation
//!
//! Implements GWT (Global Workspace Theory) feedback learning loop. The service
//! collects feedback events from embedder interactions and uses gradient-based
//! learning to adjust embedder weights for improved teleological alignment.
//!
//! # Core Responsibilities
//!
//! 1. Record positive/negative/neutral feedback events
//! 2. Compute gradients from accumulated feedback
//! 3. Apply momentum-based gradient updates
//! 4. Track per-embedder adjustment values
//! 5. Integrate with GWT consciousness feedback
//!
//! # From teleoplan.md
//!
//! "GWT feedback provides the 'reward signal' - when a retrieval leads to successful
//! task completion (high consciousness), we reinforce the embedder weights that
//! contributed most to that retrieval."

use uuid::Uuid;

use super::super::types::NUM_EMBEDDERS;

/// Configuration for the FeedbackLearner service.
#[derive(Clone, Debug)]
pub struct FeedbackLearnerConfig {
    /// Learning rate for gradient updates (default: 0.01)
    pub learning_rate: f32,
    /// Momentum for gradient accumulation (default: 0.9)
    pub momentum: f32,
    /// Scale factor for positive rewards (default: 1.0)
    pub reward_scale: f32,
    /// Scale factor for negative penalties (default: 0.5)
    pub penalty_scale: f32,
    /// Minimum feedback count before learning (default: 10)
    pub min_feedback_count: usize,
}

impl Default for FeedbackLearnerConfig {
    fn default() -> Self {
        Self {
            learning_rate: 0.01,
            momentum: 0.9,
            reward_scale: 1.0,
            penalty_scale: 0.5,
            min_feedback_count: 10,
        }
    }
}

/// Type of feedback received for a teleological vector.
#[derive(Clone, Debug, PartialEq)]
pub enum FeedbackType {
    /// Positive feedback with magnitude (successful retrieval, high consciousness)
    Positive { magnitude: f32 },
    /// Negative feedback with magnitude (failed retrieval, low consciousness)
    Negative { magnitude: f32 },
    /// Neutral feedback (neither reward nor penalty)
    Neutral,
}

impl FeedbackType {
    /// Get the signed reward value for this feedback type.
    ///
    /// - Positive returns +magnitude * reward_scale
    /// - Negative returns -magnitude * penalty_scale
    /// - Neutral returns 0.0
    pub fn reward_value(&self, config: &FeedbackLearnerConfig) -> f32 {
        match self {
            FeedbackType::Positive { magnitude } => magnitude * config.reward_scale,
            FeedbackType::Negative { magnitude } => -magnitude * config.penalty_scale,
            FeedbackType::Neutral => 0.0,
        }
    }

    /// Check if this is positive feedback.
    #[inline]
    pub fn is_positive(&self) -> bool {
        matches!(self, FeedbackType::Positive { .. })
    }

    /// Check if this is negative feedback.
    #[inline]
    pub fn is_negative(&self) -> bool {
        matches!(self, FeedbackType::Negative { .. })
    }

    /// Check if this is neutral feedback.
    #[inline]
    pub fn is_neutral(&self) -> bool {
        matches!(self, FeedbackType::Neutral)
    }
}

/// A single feedback event for a teleological vector.
#[derive(Clone, Debug)]
pub struct FeedbackEvent {
    /// UUID of the TeleologicalVector this feedback is for
    pub vector_id: Uuid,
    /// Type and magnitude of feedback
    pub feedback_type: FeedbackType,
    /// Timestamp when feedback was recorded (milliseconds since epoch)
    pub timestamp: u64,
    /// Optional context string describing the feedback source
    pub context: Option<String>,
    /// Per-embedder contribution weights at time of feedback (0.0-1.0 each)
    /// Used to attribute feedback to specific embedders
    pub embedder_contributions: [f32; NUM_EMBEDDERS],
}

impl FeedbackEvent {
    /// Create a new feedback event.
    ///
    /// # Arguments
    /// * `vector_id` - UUID of the TeleologicalVector
    /// * `feedback_type` - Type of feedback (positive/negative/neutral)
    /// * `timestamp` - Milliseconds since epoch
    ///
    /// # Panics
    ///
    /// Does not panic. Embedder contributions default to uniform (1/13 each).
    pub fn new(vector_id: Uuid, feedback_type: FeedbackType, timestamp: u64) -> Self {
        Self {
            vector_id,
            feedback_type,
            timestamp,
            context: None,
            embedder_contributions: [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS],
        }
    }

    /// Create with context.
    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Create with explicit embedder contributions.
    ///
    /// # Panics
    ///
    /// Panics if contributions don't sum to approximately 1.0 (FAIL FAST).
    pub fn with_contributions(mut self, contributions: [f32; NUM_EMBEDDERS]) -> Self {
        let sum: f32 = contributions.iter().sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "FAIL FAST: embedder contributions must sum to ~1.0, got {}",
            sum
        );
        self.embedder_contributions = contributions;
        self
    }
}

/// Result of a learning cycle.
#[derive(Clone, Debug)]
pub struct LearningResult {
    /// Per-embedder adjustments computed from gradients
    pub adjustments: Vec<f32>,
    /// Change in overall confidence
    pub confidence_delta: f32,
    /// Number of feedback events processed
    pub events_processed: usize,
}

impl LearningResult {
    /// Create a new learning result.
    pub fn new(adjustments: Vec<f32>, confidence_delta: f32, events_processed: usize) -> Self {
        assert!(
            adjustments.len() == NUM_EMBEDDERS,
            "FAIL FAST: adjustments must have {} elements, got {}",
            NUM_EMBEDDERS,
            adjustments.len()
        );
        Self {
            adjustments,
            confidence_delta,
            events_processed,
        }
    }

    /// Check if any learning occurred.
    #[inline]
    pub fn has_adjustments(&self) -> bool {
        self.adjustments.iter().any(|&a| a.abs() > f32::EPSILON)
    }

    /// Get total adjustment magnitude (L1 norm).
    pub fn total_adjustment_magnitude(&self) -> f32 {
        self.adjustments.iter().map(|a| a.abs()).sum()
    }
}

/// TELEO-014: GWT Feedback Learning Loop Service.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ===== FeedbackLearnerConfig Tests =====

    #[test]
    fn test_config_default() {
        let config = FeedbackLearnerConfig::default();

        assert!((config.learning_rate - 0.01).abs() < f32::EPSILON);
        assert!((config.momentum - 0.9).abs() < f32::EPSILON);
        assert!((config.reward_scale - 1.0).abs() < f32::EPSILON);
        assert!((config.penalty_scale - 0.5).abs() < f32::EPSILON);
        assert_eq!(config.min_feedback_count, 10);

        println!("[PASS] FeedbackLearnerConfig::default has correct values");
    }

    // ===== FeedbackType Tests =====

    #[test]
    fn test_feedback_type_positive_reward() {
        let config = FeedbackLearnerConfig::default();
        let ft = FeedbackType::Positive { magnitude: 0.8 };

        let reward = ft.reward_value(&config);
        // 0.8 * 1.0 = 0.8
        assert!((reward - 0.8).abs() < f32::EPSILON);
        assert!(ft.is_positive());
        assert!(!ft.is_negative());
        assert!(!ft.is_neutral());

        println!("[PASS] FeedbackType::Positive computes correct reward");
    }

    #[test]
    fn test_feedback_type_negative_reward() {
        let config = FeedbackLearnerConfig::default();
        let ft = FeedbackType::Negative { magnitude: 0.6 };

        let reward = ft.reward_value(&config);
        // -0.6 * 0.5 = -0.3
        assert!((reward - (-0.3)).abs() < f32::EPSILON);
        assert!(!ft.is_positive());
        assert!(ft.is_negative());
        assert!(!ft.is_neutral());

        println!("[PASS] FeedbackType::Negative computes correct penalty");
    }

    #[test]
    fn test_feedback_type_neutral_reward() {
        let config = FeedbackLearnerConfig::default();
        let ft = FeedbackType::Neutral;

        let reward = ft.reward_value(&config);
        assert!((reward - 0.0).abs() < f32::EPSILON);
        assert!(!ft.is_positive());
        assert!(!ft.is_negative());
        assert!(ft.is_neutral());

        println!("[PASS] FeedbackType::Neutral returns zero reward");
    }

    // ===== FeedbackEvent Tests =====

    #[test]
    fn test_feedback_event_new() {
        let id = Uuid::new_v4();
        let event = FeedbackEvent::new(id, FeedbackType::Positive { magnitude: 0.9 }, 12345);

        assert_eq!(event.vector_id, id);
        assert_eq!(event.timestamp, 12345);
        assert!(event.context.is_none());

        // Default contributions should be uniform
        let expected = 1.0 / NUM_EMBEDDERS as f32;
        for contrib in &event.embedder_contributions {
            assert!((*contrib - expected).abs() < 0.001);
        }

        println!("[PASS] FeedbackEvent::new creates event with uniform contributions");
    }

    #[test]
    fn test_feedback_event_with_context() {
        let event = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, 1000)
            .with_context("test context");

        assert_eq!(event.context, Some("test context".to_string()));

        println!("[PASS] FeedbackEvent::with_context sets context");
    }

    #[test]
    fn test_feedback_event_with_contributions() {
        let mut contributions = [0.0f32; NUM_EMBEDDERS];
        contributions[0] = 0.5;
        contributions[5] = 0.3;
        contributions[12] = 0.2;

        let event = FeedbackEvent::new(
            Uuid::new_v4(),
            FeedbackType::Positive { magnitude: 1.0 },
            2000,
        )
        .with_contributions(contributions);

        assert!((event.embedder_contributions[0] - 0.5).abs() < f32::EPSILON);
        assert!((event.embedder_contributions[5] - 0.3).abs() < f32::EPSILON);
        assert!((event.embedder_contributions[12] - 0.2).abs() < f32::EPSILON);

        println!("[PASS] FeedbackEvent::with_contributions sets custom contributions");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_feedback_event_contributions_must_sum_to_one() {
        let contributions = [0.1f32; NUM_EMBEDDERS]; // Sums to 1.3, not 1.0

        let _ = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, 0)
            .with_contributions(contributions);
    }

    // ===== LearningResult Tests =====

    #[test]
    fn test_learning_result_new() {
        let adjustments = vec![0.1; NUM_EMBEDDERS];
        let result = LearningResult::new(adjustments.clone(), 0.05, 20);

        assert_eq!(result.adjustments, adjustments);
        assert!((result.confidence_delta - 0.05).abs() < f32::EPSILON);
        assert_eq!(result.events_processed, 20);
        assert!(result.has_adjustments());

        println!("[PASS] LearningResult::new creates valid result");
    }

    #[test]
    fn test_learning_result_no_adjustments() {
        let result = LearningResult::new(vec![0.0; NUM_EMBEDDERS], 0.0, 0);
        assert!(!result.has_adjustments());

        println!("[PASS] LearningResult::has_adjustments returns false for zero adjustments");
    }

    #[test]
    fn test_learning_result_total_magnitude() {
        let mut adjustments = vec![0.0; NUM_EMBEDDERS];
        adjustments[0] = 0.3;
        adjustments[1] = -0.2;
        adjustments[5] = 0.1;

        let result = LearningResult::new(adjustments, 0.0, 3);
        let magnitude = result.total_adjustment_magnitude();

        // |0.3| + |-0.2| + |0.1| = 0.6
        assert!((magnitude - 0.6).abs() < f32::EPSILON);

        println!("[PASS] LearningResult::total_adjustment_magnitude computes L1 norm");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_learning_result_wrong_length() {
        let _ = LearningResult::new(vec![0.1; 5], 0.0, 0); // Wrong length
    }

    // ===== FeedbackLearner Tests =====

    #[test]
    fn test_feedback_learner_new() {
        let learner = FeedbackLearner::new();

        assert_eq!(learner.feedback_buffer_size(), 0);
        assert_eq!(learner.total_events_processed(), 0);
        assert!(!learner.should_learn());

        for i in 0..NUM_EMBEDDERS {
            assert!((learner.get_adjustment_for_embedder(i) - 0.0).abs() < f32::EPSILON);
        }

        println!("[PASS] FeedbackLearner::new creates empty learner");
    }

    #[test]
    fn test_feedback_learner_with_config() {
        let config = FeedbackLearnerConfig {
            learning_rate: 0.05,
            momentum: 0.8,
            reward_scale: 2.0,
            penalty_scale: 1.0,
            min_feedback_count: 5,
        };

        let learner = FeedbackLearner::with_config(config.clone());

        assert!((learner.config().learning_rate - 0.05).abs() < f32::EPSILON);
        assert!((learner.config().momentum - 0.8).abs() < f32::EPSILON);
        assert_eq!(learner.config().min_feedback_count, 5);

        println!("[PASS] FeedbackLearner::with_config uses custom config");
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_feedback_learner_invalid_learning_rate() {
        let config = FeedbackLearnerConfig {
            learning_rate: 0.0, // Invalid
            ..Default::default()
        };
        let _ = FeedbackLearner::with_config(config);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_feedback_learner_invalid_momentum() {
        let config = FeedbackLearnerConfig {
            momentum: 1.0, // Invalid (must be < 1.0)
            ..Default::default()
        };
        let _ = FeedbackLearner::with_config(config);
    }

    #[test]
    fn test_record_feedback() {
        let mut learner = FeedbackLearner::new();

        for i in 0..5 {
            let event = FeedbackEvent::new(
                Uuid::new_v4(),
                FeedbackType::Positive { magnitude: 0.7 },
                i as u64,
            );
            learner.record_feedback(event);
        }

        assert_eq!(learner.feedback_buffer_size(), 5);
        assert!(!learner.should_learn()); // Need 10 by default

        println!("[PASS] record_feedback adds events to buffer");
    }

    #[test]
    fn test_should_learn_threshold() {
        let mut learner = FeedbackLearner::new();

        for i in 0..9 {
            let event = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, i as u64);
            learner.record_feedback(event);
        }

        assert!(!learner.should_learn()); // 9 < 10

        let event = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, 9);
        learner.record_feedback(event);

        assert!(learner.should_learn()); // 10 >= 10

        println!("[PASS] should_learn returns true when buffer >= min_feedback_count");
    }

    #[test]
    fn test_compute_gradient_positive_feedback() {
        let learner = FeedbackLearner::new();

        // Create events with known contributions
        let mut contributions = [0.0f32; NUM_EMBEDDERS];
        contributions[0] = 1.0; // All contribution to embedder 0

        let events: Vec<FeedbackEvent> = (0..5)
            .map(|i| {
                FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Positive { magnitude: 1.0 },
                    i as u64,
                )
                .with_contributions(contributions)
            })
            .collect();

        let gradient = learner.compute_gradient(&events);

        // Gradient[0] should be positive (reward = 1.0 * 1.0 * 1.0 = 1.0, avg = 1.0)
        assert!(
            gradient[0] > 0.0,
            "Gradient[0] = {} should be positive",
            gradient[0]
        );
        // Other embedders should have zero gradient
        for grad in gradient.iter().skip(1) {
            assert!((*grad - 0.0).abs() < f32::EPSILON);
        }

        println!("[PASS] compute_gradient produces positive gradient for positive feedback");
        println!("  - gradient[0] = {:.4}", gradient[0]);
    }

    #[test]
    fn test_compute_gradient_negative_feedback() {
        let learner = FeedbackLearner::new();

        let mut contributions = [0.0f32; NUM_EMBEDDERS];
        contributions[5] = 1.0; // All contribution to embedder 5

        let events: Vec<FeedbackEvent> = (0..3)
            .map(|i| {
                FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Negative { magnitude: 0.8 },
                    i as u64,
                )
                .with_contributions(contributions)
            })
            .collect();

        let gradient = learner.compute_gradient(&events);

        // Gradient[5] should be negative (penalty = -0.8 * 0.5 * 1.0 = -0.4)
        assert!(
            gradient[5] < 0.0,
            "Gradient[5] = {} should be negative",
            gradient[5]
        );

        println!("[PASS] compute_gradient produces negative gradient for negative feedback");
        println!("  - gradient[5] = {:.4}", gradient[5]);
    }

    #[test]
    fn test_compute_gradient_mixed_feedback() {
        let learner = FeedbackLearner::new();

        // Uniform contributions
        let uniform = [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS];

        let mut events = Vec::new();
        // 3 positive
        for i in 0..3 {
            events.push(
                FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Positive { magnitude: 1.0 },
                    i as u64,
                )
                .with_contributions(uniform),
            );
        }
        // 2 negative
        for i in 3..5 {
            events.push(
                FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Negative { magnitude: 1.0 },
                    i as u64,
                )
                .with_contributions(uniform),
            );
        }

        let gradient = learner.compute_gradient(&events);

        // Net reward per embedder: (3 * 1.0 - 2 * 0.5) / 5 / 13 = (3 - 1) / 5 / 13 = 0.031
        // All embedders should have same small positive gradient
        for &g in &gradient {
            assert!(g > 0.0, "Gradient should be positive, got {}", g);
        }

        println!("[PASS] compute_gradient handles mixed feedback correctly");
    }

    #[test]
    fn test_apply_gradient() {
        let mut learner = FeedbackLearner::new();

        let gradient = vec![0.5; NUM_EMBEDDERS];

        learner.apply_gradient(&gradient);

        // After first application: m = (1 - 0.9) * 0.5 = 0.05
        // adjustment += 0.01 * 0.05 = 0.0005
        let adj = learner.get_adjustment_for_embedder(0);
        assert!(adj > 0.0, "Adjustment should be positive");
        assert!((adj - 0.0005).abs() < 0.0001);

        println!("[PASS] apply_gradient updates adjustments with momentum");
        println!("  - adjustment[0] = {:.6}", adj);
    }

    #[test]
    fn test_apply_gradient_momentum_accumulation() {
        let mut learner = FeedbackLearner::new();

        let gradient = vec![1.0; NUM_EMBEDDERS];

        // Apply same gradient multiple times
        for _ in 0..10 {
            learner.apply_gradient(&gradient);
        }

        let adj = learner.get_adjustment_for_embedder(0);

        // Momentum should cause adjustments to grow faster than without momentum
        assert!(adj > 0.005, "Adjustment should accumulate, got {}", adj);

        println!("[PASS] apply_gradient accumulates momentum over iterations");
        println!("  - adjustment[0] after 10 iterations = {:.6}", adj);
    }

    #[test]
    fn test_learn_full_cycle() {
        let config = FeedbackLearnerConfig {
            min_feedback_count: 5,
            ..Default::default()
        };
        let mut learner = FeedbackLearner::with_config(config);

        // Add positive feedback with bias toward embedder 0
        let mut contributions = [0.0f32; NUM_EMBEDDERS];
        contributions[0] = 0.8;
        contributions[1] = 0.2;

        for i in 0..10 {
            let event = FeedbackEvent::new(
                Uuid::new_v4(),
                FeedbackType::Positive { magnitude: 0.9 },
                i as u64,
            )
            .with_contributions(contributions);
            learner.record_feedback(event);
        }

        assert!(learner.should_learn());

        let result = learner.learn();

        assert_eq!(result.events_processed, 10);
        assert!(result.has_adjustments());
        assert!(result.confidence_delta > 0.0); // Positive feedback = positive delta
        assert_eq!(learner.feedback_buffer_size(), 0); // Buffer cleared

        // Embedder 0 should have higher adjustment than embedder 1
        let adj0 = learner.get_adjustment_for_embedder(0);
        let adj1 = learner.get_adjustment_for_embedder(1);
        assert!(adj0 > adj1, "adj0={} should be > adj1={}", adj0, adj1);

        println!("[PASS] learn() performs full learning cycle");
        println!("  - events_processed: {}", result.events_processed);
        println!("  - confidence_delta: {:.4}", result.confidence_delta);
        println!("  - adjustment[0]: {:.6}", adj0);
        println!("  - adjustment[1]: {:.6}", adj1);
    }

    #[test]
    fn test_learn_empty_buffer() {
        let mut learner = FeedbackLearner::new();

        let result = learner.learn();

        assert_eq!(result.events_processed, 0);
        assert!(!result.has_adjustments());

        println!("[PASS] learn() handles empty buffer gracefully");
    }

    #[test]
    fn test_reset_feedback_buffer() {
        let mut learner = FeedbackLearner::new();

        for i in 0..5 {
            let event = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, i as u64);
            learner.record_feedback(event);
        }

        assert_eq!(learner.feedback_buffer_size(), 5);

        learner.reset_feedback_buffer();

        assert_eq!(learner.feedback_buffer_size(), 0);

        println!("[PASS] reset_feedback_buffer clears buffer");
    }

    #[test]
    fn test_reset_all_state() {
        let mut learner = FeedbackLearner::new();

        // Add some events and learn
        let uniform = [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS];
        for i in 0..15 {
            let event = FeedbackEvent::new(
                Uuid::new_v4(),
                FeedbackType::Positive { magnitude: 1.0 },
                i as u64,
            )
            .with_contributions(uniform);
            learner.record_feedback(event);
        }
        learner.learn();

        assert!(learner.total_events_processed() > 0);
        assert!(learner.get_adjustment_for_embedder(0) != 0.0);

        learner.reset();

        assert_eq!(learner.feedback_buffer_size(), 0);
        assert_eq!(learner.total_events_processed(), 0);
        assert!((learner.cumulative_confidence_delta() - 0.0).abs() < f32::EPSILON);
        for i in 0..NUM_EMBEDDERS {
            assert!((learner.get_adjustment_for_embedder(i) - 0.0).abs() < f32::EPSILON);
        }

        println!("[PASS] reset() clears all state");
    }

    #[test]
    fn test_adjustment_clamping() {
        let config = FeedbackLearnerConfig {
            learning_rate: 1.0, // High learning rate to force clamping
            momentum: 0.0,
            min_feedback_count: 1,
            ..Default::default()
        };
        let mut learner = FeedbackLearner::with_config(config);

        // Apply extreme positive gradient
        let gradient = vec![10.0; NUM_EMBEDDERS];
        for _ in 0..10 {
            learner.apply_gradient(&gradient);
        }

        let adj = learner.get_adjustment_for_embedder(0);
        assert!(
            adj <= 1.0,
            "Adjustment should be clamped to <= 1.0, got {}",
            adj
        );
        assert!((adj - 1.0).abs() < f32::EPSILON);

        // Apply extreme negative gradient
        let gradient = vec![-20.0; NUM_EMBEDDERS];
        for _ in 0..10 {
            learner.apply_gradient(&gradient);
        }

        let adj = learner.get_adjustment_for_embedder(0);
        assert!(
            adj >= -1.0,
            "Adjustment should be clamped to >= -1.0, got {}",
            adj
        );
        assert!((adj - (-1.0)).abs() < f32::EPSILON);

        println!("[PASS] Adjustments are clamped to [-1.0, 1.0]");
    }

    #[test]
    fn test_total_events_tracking() {
        let config = FeedbackLearnerConfig {
            min_feedback_count: 3,
            ..Default::default()
        };
        let mut learner = FeedbackLearner::with_config(config);

        // First batch
        for i in 0..5 {
            let event = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, i as u64);
            learner.record_feedback(event);
        }
        learner.learn();
        assert_eq!(learner.total_events_processed(), 5);

        // Second batch
        for i in 0..7 {
            let event = FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Neutral, i as u64);
            learner.record_feedback(event);
        }
        learner.learn();
        assert_eq!(learner.total_events_processed(), 12);

        println!("[PASS] total_events_processed accumulates across learn() calls");
    }

    #[test]
    fn test_cumulative_confidence_delta() {
        let config = FeedbackLearnerConfig {
            min_feedback_count: 2,
            ..Default::default()
        };
        let mut learner = FeedbackLearner::with_config(config);

        // Positive feedback increases confidence
        let uniform = [1.0 / NUM_EMBEDDERS as f32; NUM_EMBEDDERS];
        for _ in 0..5 {
            let event =
                FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Positive { magnitude: 1.0 }, 0)
                    .with_contributions(uniform);
            learner.record_feedback(event);
        }
        learner.learn();

        let delta1 = learner.cumulative_confidence_delta();
        assert!(delta1 > 0.0, "Positive feedback should increase confidence");

        // Negative feedback decreases confidence
        for _ in 0..5 {
            let event =
                FeedbackEvent::new(Uuid::new_v4(), FeedbackType::Negative { magnitude: 1.0 }, 0)
                    .with_contributions(uniform);
            learner.record_feedback(event);
        }
        learner.learn();

        let delta2 = learner.cumulative_confidence_delta();
        assert!(
            delta2 < delta1,
            "Negative feedback should decrease cumulative delta"
        );

        println!("[PASS] cumulative_confidence_delta tracks confidence changes");
        println!("  - After positive: {:.4}", delta1);
        println!("  - After negative: {:.4}", delta2);
    }

    #[test]
    #[should_panic(expected = "FAIL FAST")]
    fn test_get_adjustment_out_of_bounds() {
        let learner = FeedbackLearner::new();
        let _ = learner.get_adjustment_for_embedder(13);
    }

    #[test]
    fn test_default_impl() {
        let learner = FeedbackLearner::default();
        assert_eq!(learner.feedback_buffer_size(), 0);

        println!("[PASS] FeedbackLearner::default() works");
    }

    // ===== Real Gradient Calculation Tests =====

    #[test]
    fn test_gradient_calculation_real_scenario() {
        let learner = FeedbackLearner::new();

        // Simulate retrieval where embedders 0, 3, 5 contributed most
        let mut contributions = [0.0f32; NUM_EMBEDDERS];
        contributions[0] = 0.4;
        contributions[3] = 0.35;
        contributions[5] = 0.25;

        // Create 20 events: 15 positive, 5 negative
        let mut events = Vec::new();
        for i in 0..15 {
            events.push(
                FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Positive { magnitude: 0.8 },
                    i as u64,
                )
                .with_contributions(contributions),
            );
        }
        for i in 15..20 {
            events.push(
                FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Negative { magnitude: 0.6 },
                    i as u64,
                )
                .with_contributions(contributions),
            );
        }

        let gradient = learner.compute_gradient(&events);

        // Net reward per event:
        // Positive: 0.8 * 1.0 = 0.8, 15 events
        // Negative: -0.6 * 0.5 = -0.3, 5 events
        // Total reward: 15 * 0.8 + 5 * (-0.3) = 12 - 1.5 = 10.5
        // Average reward per event: 10.5 / 20 = 0.525

        // Expected gradient for embedder 0: 0.4 * 0.525 = 0.21
        let expected_g0 = 0.4 * 0.525;
        assert!(
            (gradient[0] - expected_g0).abs() < 0.01,
            "gradient[0] = {} (expected ~{})",
            gradient[0],
            expected_g0
        );

        // Expected gradient for embedder 3: 0.35 * 0.525 = 0.18375
        let expected_g3 = 0.35 * 0.525;
        assert!(
            (gradient[3] - expected_g3).abs() < 0.01,
            "gradient[3] = {} (expected ~{})",
            gradient[3],
            expected_g3
        );

        // Embedders with zero contribution should have zero gradient
        assert!(
            gradient[1].abs() < f32::EPSILON,
            "gradient[1] should be 0, got {}",
            gradient[1]
        );

        println!("[PASS] Real gradient calculation matches expected values");
        println!(
            "  - gradient[0] = {:.6} (expected {:.6})",
            gradient[0], expected_g0
        );
        println!(
            "  - gradient[3] = {:.6} (expected {:.6})",
            gradient[3], expected_g3
        );
        println!("  - gradient[5] = {:.6}", gradient[5]);
    }

    #[test]
    fn test_multiple_learn_cycles_convergence() {
        let config = FeedbackLearnerConfig {
            min_feedback_count: 5,
            learning_rate: 0.1,
            momentum: 0.9,
            ..Default::default()
        };
        let mut learner = FeedbackLearner::with_config(config);

        // Simulate consistent positive feedback for embedder 0
        let mut contributions = [0.0f32; NUM_EMBEDDERS];
        contributions[0] = 1.0;

        for cycle in 0..10 {
            for _ in 0..10 {
                let event = FeedbackEvent::new(
                    Uuid::new_v4(),
                    FeedbackType::Positive { magnitude: 1.0 },
                    cycle as u64,
                )
                .with_contributions(contributions);
                learner.record_feedback(event);
            }

            learner.learn();
        }

        let final_adj = learner.get_adjustment_for_embedder(0);

        // With consistent positive feedback (lr=0.1, momentum=0.9, 10 cycles of 10 events),
        // mathematical convergence: momentum converges to 1.0, adjustment accumulates as
        // sum of lr * m_i where m_i follows geometric series. After 10 cycles: ~0.41
        // Expected range: [0.35, 0.50] based on momentum-SGD convergence properties
        assert!(
            final_adj > 0.35,
            "After 10 cycles, adjustment should be substantial (>0.35), got {}",
            final_adj
        );
        assert!(
            final_adj < 0.50,
            "After only 10 cycles, adjustment should not yet exceed 0.50, got {}",
            final_adj
        );

        println!("[PASS] Multiple learning cycles show convergence");
        println!("  - Final adjustment[0] = {:.4}", final_adj);
        println!(
            "  - Total events processed = {}",
            learner.total_events_processed()
        );
    }
}
