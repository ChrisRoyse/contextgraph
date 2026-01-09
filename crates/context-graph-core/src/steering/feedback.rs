//! Steering feedback types and computation
//!
//! TASK-STEERING-001: Defines feedback structures for the steering subsystem.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See `get_steering_feedback` MCP tool requirements (line 538).

use serde::{Deserialize, Serialize};

/// Steering reward in range [-1, 1].
///
/// The reward is computed as the weighted average of component scores:
/// - Gardener: Graph health and connectivity
/// - Curator: Memory quality
/// - Assessor: Performance metrics
///
/// Each component score is in [-1, 1] and the final reward is their average,
/// also clamped to [-1, 1].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SteeringReward {
    /// Overall reward [-1, 1]
    pub value: f32,
    /// Gardener's contribution [-1, 1]
    pub gardener_score: f32,
    /// Curator's contribution [-1, 1]
    pub curator_score: f32,
    /// Assessor's contribution [-1, 1]
    pub assessor_score: f32,
}

impl SteeringReward {
    /// Create a new SteeringReward from component scores.
    ///
    /// All inputs are clamped to [-1, 1] before averaging.
    /// The final value is also clamped to [-1, 1].
    ///
    /// # Arguments
    /// * `gardener` - Gardener's score (graph health)
    /// * `curator` - Curator's score (quality)
    /// * `assessor` - Assessor's score (performance)
    pub fn new(gardener: f32, curator: f32, assessor: f32) -> Self {
        let g = gardener.clamp(-1.0, 1.0);
        let c = curator.clamp(-1.0, 1.0);
        let a = assessor.clamp(-1.0, 1.0);
        let value = ((g + c + a) / 3.0).clamp(-1.0, 1.0);

        Self {
            value,
            gardener_score: g,
            curator_score: c,
            assessor_score: a,
        }
    }

    /// Check if the reward is positive (system is doing well).
    pub fn is_positive(&self) -> bool {
        self.value > 0.0
    }

    /// Check if the reward is negative (system needs improvement).
    pub fn is_negative(&self) -> bool {
        self.value < 0.0
    }

    /// Get the dominant factor (component with highest absolute contribution).
    pub fn dominant_factor(&self) -> &'static str {
        let abs_g = self.gardener_score.abs();
        let abs_c = self.curator_score.abs();
        let abs_a = self.assessor_score.abs();

        if abs_g >= abs_c && abs_g >= abs_a {
            "gardener"
        } else if abs_c >= abs_g && abs_c >= abs_a {
            "curator"
        } else {
            "assessor"
        }
    }

    /// Get the limiting factor (component with lowest score).
    pub fn limiting_factor(&self) -> &'static str {
        if self.gardener_score <= self.curator_score && self.gardener_score <= self.assessor_score {
            "gardener"
        } else if self.curator_score <= self.gardener_score
            && self.curator_score <= self.assessor_score
        {
            "curator"
        } else {
            "assessor"
        }
    }
}

impl Default for SteeringReward {
    fn default() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }
}

/// Detailed feedback from the Gardener component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GardenerFeedback {
    /// Number of edges that could be pruned (below weight threshold)
    pub edges_pruned: usize,
    /// Number of dead ends (orphan nodes) removed or flagged
    pub dead_ends_removed: usize,
    /// Graph connectivity score [0, 1]
    pub connectivity: f32,
}

impl GardenerFeedback {
    /// Create new GardenerFeedback.
    pub fn new(edges_pruned: usize, dead_ends_removed: usize, connectivity: f32) -> Self {
        Self {
            edges_pruned,
            dead_ends_removed,
            connectivity: connectivity.clamp(0.0, 1.0),
        }
    }

    /// Check if graph health is good (connectivity > 0.7).
    pub fn is_healthy(&self) -> bool {
        self.connectivity > 0.7
    }
}

/// Detailed feedback from the Curator component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuratorFeedback {
    /// Average memory quality [0, 1]
    pub avg_quality: f32,
    /// Number of low-quality memories flagged
    pub low_quality_count: usize,
    /// Actionable recommendations
    pub recommendations: Vec<String>,
}

impl CuratorFeedback {
    /// Create new CuratorFeedback.
    pub fn new(avg_quality: f32, low_quality_count: usize, recommendations: Vec<String>) -> Self {
        Self {
            avg_quality: avg_quality.clamp(0.0, 1.0),
            low_quality_count,
            recommendations,
        }
    }

    /// Check if quality is good (avg_quality > 0.7).
    pub fn is_high_quality(&self) -> bool {
        self.avg_quality > 0.7
    }

    /// Check if there are concerns (recommendations not empty).
    pub fn has_concerns(&self) -> bool {
        !self.recommendations.is_empty()
    }
}

/// Detailed feedback from the Assessor component.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssessorFeedback {
    /// Retrieval accuracy [0, 1]
    pub retrieval_accuracy: f32,
    /// Learning efficiency [0, 1]
    pub learning_efficiency: f32,
    /// Performance trend: positive = improving, negative = declining
    pub trend: f32,
}

impl AssessorFeedback {
    /// Create new AssessorFeedback.
    pub fn new(retrieval_accuracy: f32, learning_efficiency: f32, trend: f32) -> Self {
        Self {
            retrieval_accuracy: retrieval_accuracy.clamp(0.0, 1.0),
            learning_efficiency: learning_efficiency.clamp(0.0, 1.0),
            trend,
        }
    }

    /// Check if performance is good (accuracy > 0.8).
    pub fn is_performing_well(&self) -> bool {
        self.retrieval_accuracy > 0.8
    }

    /// Check if performance is improving.
    pub fn is_improving(&self) -> bool {
        self.trend > 0.02
    }

    /// Check if performance is declining.
    pub fn is_declining(&self) -> bool {
        self.trend < -0.02
    }

    /// Get trend description.
    pub fn trend_description(&self) -> &'static str {
        if self.is_improving() {
            "improving"
        } else if self.is_declining() {
            "declining"
        } else {
            "stable"
        }
    }
}

/// Complete steering feedback with reward and component details.
///
/// This is returned by the `get_steering_feedback` MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteeringFeedback {
    /// Aggregate steering reward [-1, 1]
    pub reward: SteeringReward,
    /// Detailed feedback from Gardener
    pub gardener_details: GardenerFeedback,
    /// Detailed feedback from Curator
    pub curator_details: CuratorFeedback,
    /// Detailed feedback from Assessor
    pub assessor_details: AssessorFeedback,
}

impl SteeringFeedback {
    /// Get a summary of the steering feedback.
    pub fn summary(&self) -> String {
        let status = if self.reward.is_positive() {
            "healthy"
        } else {
            "needs attention"
        };
        let limiting = self.reward.limiting_factor();
        let trend = self.assessor_details.trend_description();

        format!(
            "System {}: reward={:.2}, limiting_factor={}, trend={}",
            status, self.reward.value, limiting, trend
        )
    }

    /// Check if any component needs immediate attention (score < -0.5).
    pub fn needs_immediate_attention(&self) -> bool {
        self.reward.gardener_score < -0.5
            || self.reward.curator_score < -0.5
            || self.reward.assessor_score < -0.5
    }

    /// Get the component that needs the most attention.
    pub fn priority_improvement(&self) -> &'static str {
        self.reward.limiting_factor()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_steering_reward_new() {
        let reward = SteeringReward::new(0.5, 0.3, 0.8);
        assert!((reward.value - 0.533).abs() < 0.01);
        assert_eq!(reward.gardener_score, 0.5);
        assert_eq!(reward.curator_score, 0.3);
        assert_eq!(reward.assessor_score, 0.8);
    }

    #[test]
    fn test_steering_reward_clamping() {
        let reward = SteeringReward::new(2.0, -2.0, 0.5);
        assert_eq!(reward.gardener_score, 1.0);
        assert_eq!(reward.curator_score, -1.0);
        assert_eq!(reward.assessor_score, 0.5);
        // (1.0 + -1.0 + 0.5) / 3 = 0.167
        assert!((reward.value - 0.167).abs() < 0.01);
    }

    #[test]
    fn test_dominant_factor() {
        let reward = SteeringReward::new(0.9, 0.3, 0.5);
        assert_eq!(reward.dominant_factor(), "gardener");

        let reward = SteeringReward::new(0.3, -0.9, 0.5);
        assert_eq!(reward.dominant_factor(), "curator");

        let reward = SteeringReward::new(0.3, 0.3, 0.9);
        assert_eq!(reward.dominant_factor(), "assessor");
    }

    #[test]
    fn test_limiting_factor() {
        let reward = SteeringReward::new(0.1, 0.5, 0.9);
        assert_eq!(reward.limiting_factor(), "gardener");

        let reward = SteeringReward::new(0.5, -0.2, 0.9);
        assert_eq!(reward.limiting_factor(), "curator");
    }

    #[test]
    fn test_assessor_feedback_trend() {
        let feedback = AssessorFeedback::new(0.9, 0.8, 0.05);
        assert!(feedback.is_improving());
        assert_eq!(feedback.trend_description(), "improving");

        let feedback = AssessorFeedback::new(0.9, 0.8, -0.05);
        assert!(feedback.is_declining());
        assert_eq!(feedback.trend_description(), "declining");

        let feedback = AssessorFeedback::new(0.9, 0.8, 0.01);
        assert_eq!(feedback.trend_description(), "stable");
    }
}
