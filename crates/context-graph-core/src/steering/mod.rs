//! Steering Subsystem
//!
//! Implements the steering feedback system with three components:
//! - Gardener: Graph maintenance and pruning
//! - Curator: Quality assessment and curation
//! - Assessor: Performance evaluation
//!
//! ## Constitution Reference
//!
//! See steering.components and `get_steering_feedback` MCP tool (lines 538-539).
//!
//! ## Steering Reward
//!
//! The steering reward is computed as the weighted average of scores from
//! Gardener, Curator, and Assessor, normalized to [-1, 1]:
//!
//! ```text
//! SteeringReward = (gardener_score + curator_score + assessor_score) / 3
//! ```
//!
//! Each component score is clamped to [-1, 1] before averaging.

pub mod assessor;
pub mod curator;
pub mod feedback;
pub mod gardener;

pub use assessor::Assessor;
pub use curator::Curator;
pub use feedback::{
    AssessorFeedback, CuratorFeedback, GardenerFeedback, SteeringFeedback, SteeringReward,
};
pub use gardener::Gardener;

/// Steering system that coordinates Gardener, Curator, and Assessor.
///
/// This is the main entry point for computing steering feedback.
/// FAIL FAST: No mock data, no fallbacks.
#[derive(Debug, Default)]
pub struct SteeringSystem {
    /// Graph gardener for maintenance
    pub gardener: Gardener,
    /// Memory curator for quality
    pub curator: Curator,
    /// Performance assessor
    pub assessor: Assessor,
}

impl SteeringSystem {
    /// Create a new SteeringSystem with default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a SteeringSystem with custom configuration.
    pub fn with_config(
        prune_threshold: f32,
        orphan_age_days: u32,
        min_quality: f32,
        target_accuracy: f32,
    ) -> Self {
        Self {
            gardener: Gardener::with_config(prune_threshold, orphan_age_days),
            curator: Curator::with_min_quality(min_quality),
            assessor: Assessor::with_target_accuracy(target_accuracy),
        }
    }

    /// Compute comprehensive steering feedback.
    ///
    /// # Arguments
    /// * `edge_count` - Total number of edges in the graph
    /// * `orphan_count` - Number of orphan nodes (no connections)
    /// * `connectivity` - Graph connectivity score [0, 1]
    /// * `avg_quality` - Average memory quality score [0, 1]
    /// * `low_quality_count` - Number of memories below quality threshold
    /// * `retrieval_accuracy` - Recent retrieval accuracy [0, 1]
    /// * `learning_efficiency` - Learning efficiency metric [0, 1]
    /// * `prev_accuracy` - Previous accuracy for trend calculation
    ///
    /// # Returns
    /// Complete SteeringFeedback with reward and component details.
    #[allow(clippy::too_many_arguments)]
    pub fn compute_feedback(
        &self,
        edge_count: usize,
        orphan_count: usize,
        connectivity: f32,
        avg_quality: f32,
        low_quality_count: usize,
        retrieval_accuracy: f32,
        learning_efficiency: f32,
        prev_accuracy: Option<f32>,
    ) -> SteeringFeedback {
        // Get component scores
        let gardener_score = self.gardener.score(connectivity);
        let curator_score = self.curator.score(avg_quality);
        let assessor_score = self.assessor.score(retrieval_accuracy, learning_efficiency);

        // Compute aggregate reward
        let reward = SteeringReward::new(gardener_score, curator_score, assessor_score);

        // Get detailed feedback from each component
        let gardener_details = self
            .gardener
            .evaluate(edge_count, orphan_count, connectivity);
        let curator_details = self.curator.evaluate(avg_quality, low_quality_count);
        let assessor_details =
            self.assessor
                .evaluate(retrieval_accuracy, learning_efficiency, prev_accuracy);

        SteeringFeedback {
            reward,
            gardener_details,
            curator_details,
            assessor_details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steering_system_default() {
        let system = SteeringSystem::new();
        assert_eq!(system.gardener.prune_threshold, 0.1);
        assert_eq!(system.curator.min_quality, 0.3);
        assert_eq!(system.assessor.target_accuracy, 0.8);
    }

    #[test]
    fn test_steering_system_with_config() {
        let system = SteeringSystem::with_config(0.2, 60, 0.5, 0.9);
        assert_eq!(system.gardener.prune_threshold, 0.2);
        assert_eq!(system.gardener.orphan_age_days, 60);
        assert_eq!(system.curator.min_quality, 0.5);
        assert_eq!(system.assessor.target_accuracy, 0.9);
    }

    #[test]
    fn test_compute_feedback_positive() {
        let system = SteeringSystem::new();
        let feedback = system.compute_feedback(
            100,        // edge_count
            5,          // orphan_count
            0.9,        // connectivity (high)
            0.85,       // avg_quality (high)
            2,          // low_quality_count
            0.9,        // retrieval_accuracy (high)
            0.85,       // learning_efficiency (high)
            Some(0.85), // prev_accuracy
        );

        // All scores should be positive with good metrics
        assert!(feedback.reward.value > 0.0);
        assert!(feedback.reward.gardener_score > 0.0);
        assert!(feedback.reward.curator_score > 0.0);
        assert!(feedback.reward.assessor_score > 0.0);
    }

    #[test]
    fn test_compute_feedback_negative() {
        let system = SteeringSystem::new();
        let feedback = system.compute_feedback(
            100,       // edge_count
            50,        // orphan_count (high)
            0.2,       // connectivity (low)
            0.3,       // avg_quality (low)
            50,        // low_quality_count (high)
            0.3,       // retrieval_accuracy (low)
            0.2,       // learning_efficiency (low)
            Some(0.5), // prev_accuracy (was better)
        );

        // Scores should be negative with poor metrics
        assert!(feedback.reward.value < 0.0);
        assert!(feedback.reward.gardener_score < 0.0);
        assert!(feedback.reward.curator_score < 0.0);
        assert!(feedback.reward.assessor_score < 0.0);
    }

    #[test]
    fn test_reward_clamping() {
        let system = SteeringSystem::new();
        let feedback = system.compute_feedback(100, 0, 1.0, 1.0, 0, 1.0, 1.0, None);

        // Reward should be clamped to [-1, 1]
        assert!(feedback.reward.value <= 1.0);
        assert!(feedback.reward.value >= -1.0);
    }
}
