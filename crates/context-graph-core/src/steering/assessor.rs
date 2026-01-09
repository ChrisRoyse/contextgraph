//! Assessor - Performance evaluation
//!
//! TASK-STEERING-001: Implements the Assessor component for performance.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See steering.components.assessor requirements.
//!
//! ## Responsibilities
//!
//! - Retrieval accuracy monitoring
//! - Learning efficiency tracking
//! - Performance trend analysis
//! - Improvement recommendations

use super::feedback::AssessorFeedback;

/// Performance assessor.
///
/// The Assessor monitors system performance:
/// - Retrieval accuracy (how well queries are satisfied)
/// - Learning efficiency (how effectively new knowledge is integrated)
/// - Performance trends (improving/stable/declining)
#[derive(Debug, Clone)]
pub struct Assessor {
    /// Target retrieval accuracy [0, 1]
    pub target_accuracy: f32,
}

impl Assessor {
    /// Create a new Assessor with default configuration.
    ///
    /// Default values:
    /// - target_accuracy: 0.8 (aim for 80% retrieval accuracy)
    pub fn new() -> Self {
        Self {
            target_accuracy: 0.8,
        }
    }

    /// Create an Assessor with custom target accuracy.
    ///
    /// # Arguments
    /// * `target_accuracy` - Target retrieval accuracy [0, 1]
    pub fn with_target_accuracy(target_accuracy: f32) -> Self {
        Self {
            target_accuracy: target_accuracy.clamp(0.0, 1.0),
        }
    }

    /// Evaluate performance and return feedback.
    ///
    /// # Arguments
    /// * `accuracy` - Current retrieval accuracy [0, 1]
    /// * `efficiency` - Learning efficiency [0, 1]
    /// * `prev_accuracy` - Previous accuracy for trend calculation
    ///
    /// # Returns
    /// AssessorFeedback with performance metrics and trend.
    pub fn evaluate(
        &self,
        accuracy: f32,
        efficiency: f32,
        prev_accuracy: Option<f32>,
    ) -> AssessorFeedback {
        let trend = match prev_accuracy {
            Some(prev) => accuracy - prev,
            None => 0.0,
        };

        AssessorFeedback::new(accuracy, efficiency, trend)
    }

    /// Compute assessor score [-1, 1].
    ///
    /// The score is based on both accuracy and efficiency:
    /// - Combined metric = (accuracy + efficiency) / 2
    /// - Score = (combined - 0.5) * 2, clamped to [-1, 1]
    ///
    /// This means:
    /// - accuracy=1, efficiency=1 -> score = 1.0
    /// - accuracy=0.5, efficiency=0.5 -> score = 0.0
    /// - accuracy=0, efficiency=0 -> score = -1.0
    ///
    /// # Arguments
    /// * `accuracy` - Retrieval accuracy [0, 1]
    /// * `efficiency` - Learning efficiency [0, 1]
    ///
    /// # Returns
    /// Score in [-1, 1] where positive = good performance
    pub fn score(&self, accuracy: f32, efficiency: f32) -> f32 {
        let combined = (accuracy.clamp(0.0, 1.0) + efficiency.clamp(0.0, 1.0)) / 2.0;
        // Map [0, 1] to [-1, 1]
        // 0.0 -> -1.0, 0.5 -> 0.0, 1.0 -> 1.0
        ((combined - 0.5) * 2.0).clamp(-1.0, 1.0)
    }

    /// Check if accuracy meets target.
    ///
    /// # Arguments
    /// * `accuracy` - Current accuracy [0, 1]
    ///
    /// # Returns
    /// true if accuracy >= target_accuracy
    pub fn meets_target(&self, accuracy: f32) -> bool {
        accuracy >= self.target_accuracy
    }

    /// Get performance status based on accuracy.
    ///
    /// # Arguments
    /// * `accuracy` - Retrieval accuracy [0, 1]
    ///
    /// # Returns
    /// Status string: "excellent", "good", "acceptable", "poor", or "critical"
    pub fn performance_status(&self, accuracy: f32) -> &'static str {
        if accuracy > 0.9 {
            "excellent"
        } else if accuracy > 0.8 {
            "good"
        } else if accuracy > 0.6 {
            "acceptable"
        } else if accuracy > 0.4 {
            "poor"
        } else {
            "critical"
        }
    }

    /// Calculate gap to target.
    ///
    /// # Arguments
    /// * `accuracy` - Current accuracy [0, 1]
    ///
    /// # Returns
    /// Gap as (target - accuracy), positive if below target
    pub fn gap_to_target(&self, accuracy: f32) -> f32 {
        (self.target_accuracy - accuracy).max(0.0)
    }

    /// Analyze trend from a series of accuracy measurements.
    ///
    /// # Arguments
    /// * `measurements` - Recent accuracy measurements (oldest first)
    ///
    /// # Returns
    /// Trend analysis: (trend_value, description)
    pub fn analyze_trend(&self, measurements: &[f32]) -> (f32, &'static str) {
        if measurements.len() < 2 {
            return (0.0, "insufficient_data");
        }

        // Calculate simple linear trend
        let n = measurements.len() as f32;
        let sum_x: f32 = (0..measurements.len()).map(|i| i as f32).sum();
        let sum_y: f32 = measurements.iter().sum();
        let sum_xy: f32 = measurements
            .iter()
            .enumerate()
            .map(|(i, &y)| i as f32 * y)
            .sum();
        let sum_x2: f32 = (0..measurements.len()).map(|i| (i as f32).powi(2)).sum();

        let denominator = n * sum_x2 - sum_x.powi(2);
        if denominator.abs() < 1e-10 {
            return (0.0, "stable");
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denominator;

        let description = if slope > 0.02 {
            "improving"
        } else if slope < -0.02 {
            "declining"
        } else {
            "stable"
        };

        (slope, description)
    }

    /// Get recommendations based on performance.
    ///
    /// # Arguments
    /// * `accuracy` - Current accuracy [0, 1]
    /// * `efficiency` - Learning efficiency [0, 1]
    /// * `trend` - Performance trend (positive = improving)
    ///
    /// # Returns
    /// Vector of recommendation strings
    pub fn get_recommendations(&self, accuracy: f32, efficiency: f32, trend: f32) -> Vec<String> {
        let mut recommendations = Vec::new();

        if accuracy < self.target_accuracy {
            recommendations.push(format!(
                "Accuracy ({:.1}%) is below target ({:.1}%). Gap: {:.1}%",
                accuracy * 100.0,
                self.target_accuracy * 100.0,
                self.gap_to_target(accuracy) * 100.0
            ));
        }

        if efficiency < 0.5 {
            recommendations.push(format!(
                "Learning efficiency ({:.1}%) is low. Consider reviewing learning parameters.",
                efficiency * 100.0
            ));
        }

        if trend < -0.05 {
            recommendations.push(
                "Performance is declining rapidly. Immediate attention required.".to_string(),
            );
        } else if trend < -0.02 {
            recommendations.push("Performance is declining. Monitor closely.".to_string());
        }

        if accuracy < 0.5 && efficiency < 0.5 {
            recommendations.push(
                "Both accuracy and efficiency are low. Consider system-wide review.".to_string(),
            );
        }

        recommendations
    }
}

impl Default for Assessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assessor_default() {
        let a = Assessor::new();
        assert_eq!(a.target_accuracy, 0.8);
    }

    #[test]
    fn test_assessor_with_target() {
        let a = Assessor::with_target_accuracy(0.9);
        assert_eq!(a.target_accuracy, 0.9);
    }

    #[test]
    fn test_score_mapping() {
        let a = Assessor::new();

        // Perfect performance
        assert_eq!(a.score(1.0, 1.0), 1.0);

        // Zero performance
        assert_eq!(a.score(0.0, 0.0), -1.0);

        // Average performance
        assert_eq!(a.score(0.5, 0.5), 0.0);

        // Mixed performance
        let score = a.score(0.8, 0.6);
        // (0.8 + 0.6) / 2 = 0.7
        // (0.7 - 0.5) * 2 = 0.4
        assert!((score - 0.4).abs() < 0.01);
    }

    #[test]
    fn test_meets_target() {
        let a = Assessor::with_target_accuracy(0.8);

        assert!(a.meets_target(0.85));
        assert!(a.meets_target(0.80));
        assert!(!a.meets_target(0.79));
        assert!(!a.meets_target(0.5));
    }

    #[test]
    fn test_performance_status() {
        let a = Assessor::new();

        assert_eq!(a.performance_status(0.95), "excellent");
        assert_eq!(a.performance_status(0.85), "good");
        assert_eq!(a.performance_status(0.65), "acceptable");
        assert_eq!(a.performance_status(0.45), "poor");
        assert_eq!(a.performance_status(0.35), "critical");
    }

    #[test]
    fn test_gap_to_target() {
        let a = Assessor::with_target_accuracy(0.8);

        assert!((a.gap_to_target(0.6) - 0.2).abs() < 0.001);
        assert_eq!(a.gap_to_target(0.8), 0.0);
        assert_eq!(a.gap_to_target(0.9), 0.0); // Can't have negative gap
    }

    #[test]
    fn test_analyze_trend() {
        let a = Assessor::new();

        // Insufficient data
        let (_trend, desc) = a.analyze_trend(&[0.5]);
        assert_eq!(desc, "insufficient_data");

        // Improving trend
        let (trend, desc) = a.analyze_trend(&[0.5, 0.6, 0.7, 0.8]);
        assert!(trend > 0.0);
        assert_eq!(desc, "improving");

        // Declining trend
        let (trend, desc) = a.analyze_trend(&[0.8, 0.7, 0.6, 0.5]);
        assert!(trend < 0.0);
        assert_eq!(desc, "declining");

        // Stable trend
        let (_trend, desc) = a.analyze_trend(&[0.7, 0.7, 0.71, 0.69, 0.7]);
        assert_eq!(desc, "stable");
    }

    #[test]
    fn test_evaluate() {
        let a = Assessor::new();

        let feedback = a.evaluate(0.85, 0.75, Some(0.80));
        assert_eq!(feedback.retrieval_accuracy, 0.85);
        assert_eq!(feedback.learning_efficiency, 0.75);
        assert!((feedback.trend - 0.05).abs() < 0.01);

        // Without previous accuracy
        let feedback = a.evaluate(0.85, 0.75, None);
        assert_eq!(feedback.trend, 0.0);
    }

    #[test]
    fn test_get_recommendations() {
        let a = Assessor::with_target_accuracy(0.8);

        // No recommendations for good performance
        let recs = a.get_recommendations(0.85, 0.7, 0.02);
        assert!(recs.is_empty());

        // Below target accuracy
        let recs = a.get_recommendations(0.7, 0.7, 0.0);
        assert!(!recs.is_empty());
        assert!(recs[0].contains("below target"));

        // Multiple issues
        let recs = a.get_recommendations(0.4, 0.3, -0.1);
        assert!(recs.len() >= 3);
    }
}
