//! Curator - Quality assessment and curation
//!
//! TASK-STEERING-001: Implements the Curator component for memory quality.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Constitution Reference
//!
//! See steering.components.curator requirements.
//!
//! ## Responsibilities
//!
//! - Memory quality assessment
//! - Low-quality detection and flagging
//! - Quality trend monitoring
//! - Curation recommendations

use super::feedback::CuratorFeedback;

/// Memory quality curator.
///
/// The Curator monitors memory quality and provides recommendations:
/// - Tracks average quality across memories
/// - Flags low-quality memories
/// - Provides actionable recommendations
#[derive(Debug, Clone)]
pub struct Curator {
    /// Minimum quality threshold [0, 1]
    pub min_quality: f32,
}

impl Curator {
    /// Create a new Curator with default configuration.
    ///
    /// Default values:
    /// - min_quality: 0.3 (flag memories with quality < 0.3)
    pub fn new() -> Self {
        Self { min_quality: 0.3 }
    }

    /// Create a Curator with custom minimum quality threshold.
    ///
    /// # Arguments
    /// * `min_quality` - Minimum acceptable quality [0, 1]
    pub fn with_min_quality(min_quality: f32) -> Self {
        Self {
            min_quality: min_quality.clamp(0.0, 1.0),
        }
    }

    /// Evaluate memory quality and return feedback.
    ///
    /// # Arguments
    /// * `avg_quality` - Average quality across all memories [0, 1]
    /// * `low_quality_count` - Number of memories below quality threshold
    ///
    /// # Returns
    /// CuratorFeedback with quality metrics and recommendations.
    pub fn evaluate(&self, avg_quality: f32, low_quality_count: usize) -> CuratorFeedback {
        let mut recommendations = Vec::new();

        if avg_quality < 0.5 {
            recommendations.push(
                "Consider reviewing recent additions for quality - average is below 0.5"
                    .to_string(),
            );
        }

        if avg_quality < 0.3 {
            recommendations
                .push("Quality is critically low. Immediate review recommended.".to_string());
        }

        if low_quality_count > 10 {
            recommendations.push(format!(
                "High number of low-quality memories detected: {}. Consider cleanup.",
                low_quality_count
            ));
        }

        if low_quality_count > 50 {
            recommendations.push(
                "Excessive low-quality content. Consider batch cleanup or stricter ingestion."
                    .to_string(),
            );
        }

        CuratorFeedback::new(avg_quality, low_quality_count, recommendations)
    }

    /// Compute curator score [-1, 1].
    ///
    /// The score is based on average quality:
    /// - 1.0 quality -> score = 1.0
    /// - 0.5 quality -> score = 0.0
    /// - 0.0 quality -> score = -1.0
    ///
    /// Formula: score = (avg_quality * 2.0 - 1.0), clamped to [-1, 1]
    ///
    /// # Arguments
    /// * `avg_quality` - Average memory quality [0, 1]
    ///
    /// # Returns
    /// Score in [-1, 1] where positive = good quality, negative = poor quality
    pub fn score(&self, avg_quality: f32) -> f32 {
        // Map quality [0, 1] to score [-1, 1]
        // 0.0 -> -1.0 (bad)
        // 0.5 -> 0.0 (neutral)
        // 1.0 -> 1.0 (good)
        (avg_quality * 2.0 - 1.0).clamp(-1.0, 1.0)
    }

    /// Check if a memory is low quality.
    ///
    /// # Arguments
    /// * `quality` - Memory quality score [0, 1]
    ///
    /// # Returns
    /// true if the quality is below the minimum threshold
    pub fn is_low_quality(&self, quality: f32) -> bool {
        quality < self.min_quality
    }

    /// Get quality status based on average quality.
    ///
    /// # Arguments
    /// * `avg_quality` - Average quality [0, 1]
    ///
    /// # Returns
    /// Status string: "excellent", "good", "fair", "poor", or "critical"
    pub fn quality_status(&self, avg_quality: f32) -> &'static str {
        if avg_quality > 0.85 {
            "excellent"
        } else if avg_quality > 0.7 {
            "good"
        } else if avg_quality > 0.5 {
            "fair"
        } else if avg_quality > 0.3 {
            "poor"
        } else {
            "critical"
        }
    }

    /// Compute quality distribution from a set of quality scores.
    ///
    /// # Arguments
    /// * `qualities` - Slice of quality scores [0, 1]
    ///
    /// # Returns
    /// Tuple of (excellent_count, good_count, fair_count, poor_count, critical_count)
    pub fn compute_distribution(&self, qualities: &[f32]) -> (usize, usize, usize, usize, usize) {
        let mut excellent = 0;
        let mut good = 0;
        let mut fair = 0;
        let mut poor = 0;
        let mut critical = 0;

        for &q in qualities {
            if q > 0.85 {
                excellent += 1;
            } else if q > 0.7 {
                good += 1;
            } else if q > 0.5 {
                fair += 1;
            } else if q > 0.3 {
                poor += 1;
            } else {
                critical += 1;
            }
        }

        (excellent, good, fair, poor, critical)
    }

    /// Get priority for quality improvement.
    ///
    /// # Arguments
    /// * `avg_quality` - Average quality [0, 1]
    /// * `low_quality_count` - Number of low-quality items
    ///
    /// # Returns
    /// Priority level: "none", "low", "medium", "high", or "critical"
    pub fn improvement_priority(&self, avg_quality: f32, low_quality_count: usize) -> &'static str {
        if avg_quality > 0.7 && low_quality_count < 5 {
            "none"
        } else if avg_quality > 0.6 && low_quality_count < 20 {
            "low"
        } else if avg_quality > 0.4 && low_quality_count < 50 {
            "medium"
        } else if avg_quality > 0.2 {
            "high"
        } else {
            "critical"
        }
    }
}

impl Default for Curator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curator_default() {
        let c = Curator::new();
        assert_eq!(c.min_quality, 0.3);
    }

    #[test]
    fn test_curator_with_min_quality() {
        let c = Curator::with_min_quality(0.5);
        assert_eq!(c.min_quality, 0.5);
    }

    #[test]
    fn test_curator_min_quality_clamping() {
        let c = Curator::with_min_quality(1.5);
        assert_eq!(c.min_quality, 1.0);

        let c = Curator::with_min_quality(-0.5);
        assert_eq!(c.min_quality, 0.0);
    }

    #[test]
    fn test_score_mapping() {
        let c = Curator::new();

        // Full quality -> max score
        assert_eq!(c.score(1.0), 1.0);

        // Zero quality -> min score
        assert_eq!(c.score(0.0), -1.0);

        // Half quality -> neutral
        assert_eq!(c.score(0.5), 0.0);
    }

    #[test]
    fn test_is_low_quality() {
        let c = Curator::with_min_quality(0.3);

        assert!(c.is_low_quality(0.1));
        assert!(c.is_low_quality(0.29));
        assert!(!c.is_low_quality(0.3));
        assert!(!c.is_low_quality(0.5));
        assert!(!c.is_low_quality(0.9));
    }

    #[test]
    fn test_quality_status() {
        let c = Curator::new();

        assert_eq!(c.quality_status(0.95), "excellent");
        assert_eq!(c.quality_status(0.75), "good");
        assert_eq!(c.quality_status(0.55), "fair");
        assert_eq!(c.quality_status(0.35), "poor");
        assert_eq!(c.quality_status(0.25), "critical");
    }

    #[test]
    fn test_compute_distribution() {
        let c = Curator::new();
        let qualities = vec![0.9, 0.85, 0.75, 0.6, 0.4, 0.2, 0.1];
        let (excellent, good, fair, poor, critical) = c.compute_distribution(&qualities);

        assert_eq!(excellent, 1); // 0.9
                                  // 0.85 is not > 0.85, so it goes to good
        assert_eq!(good, 2); // 0.85, 0.75
        assert_eq!(fair, 1); // 0.6
        assert_eq!(poor, 1); // 0.4
        assert_eq!(critical, 2); // 0.2, 0.1
    }

    #[test]
    fn test_evaluate_recommendations() {
        let c = Curator::new();

        // Good quality, few low-quality items
        let feedback = c.evaluate(0.8, 5);
        assert!(feedback.recommendations.is_empty());

        // Low quality
        let feedback = c.evaluate(0.4, 5);
        assert_eq!(feedback.recommendations.len(), 1);

        // Critical quality with many low-quality items
        let feedback = c.evaluate(0.2, 55);
        assert!(feedback.recommendations.len() >= 3);
    }

    #[test]
    fn test_improvement_priority() {
        let c = Curator::new();

        assert_eq!(c.improvement_priority(0.8, 2), "none");
        assert_eq!(c.improvement_priority(0.65, 15), "low");
        assert_eq!(c.improvement_priority(0.45, 40), "medium");
        assert_eq!(c.improvement_priority(0.25, 100), "high");
        assert_eq!(c.improvement_priority(0.1, 200), "critical");
    }
}
