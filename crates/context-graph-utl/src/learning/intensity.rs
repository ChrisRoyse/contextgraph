//! Learning intensity category for quick classification.

use serde::{Deserialize, Serialize};

/// Learning intensity category for quick classification.
///
/// Categorizes learning magnitude into Low, Medium, or High buckets
/// for efficient filtering and prioritization.
///
/// # Constitution Reference
///
/// Thresholds based on UTL learning score ranges:
/// - Low: < 0.3 (minimal learning potential)
/// - Medium: 0.3 - 0.7 (moderate learning)
/// - High: > 0.7 (strong learning signal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum LearningIntensity {
    /// Learning magnitude < 0.3
    Low = 0,
    /// Learning magnitude 0.3 - 0.7
    Medium = 1,
    /// Learning magnitude > 0.7
    High = 2,
}

impl LearningIntensity {
    /// Returns all intensity variants.
    pub fn all() -> [LearningIntensity; 3] {
        [Self::Low, Self::Medium, Self::High]
    }

    /// Returns a human-readable description of this intensity.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Low => "Low learning potential (magnitude < 0.3)",
            Self::Medium => "Moderate learning (magnitude 0.3 - 0.7)",
            Self::High => "Strong learning signal (magnitude > 0.7)",
        }
    }

    /// Classify a magnitude into an intensity category.
    #[inline]
    pub fn from_magnitude(magnitude: f32) -> Self {
        if magnitude < 0.3 {
            Self::Low
        } else if magnitude < 0.7 {
            Self::Medium
        } else {
            Self::High
        }
    }
}

impl std::fmt::Display for LearningIntensity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_intensity_categories() {
        // Test all three categories
        assert_eq!(LearningIntensity::Low as u8, 0);
        assert_eq!(LearningIntensity::Medium as u8, 1);
        assert_eq!(LearningIntensity::High as u8, 2);

        // Test all() helper
        let all = LearningIntensity::all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0], LearningIntensity::Low);
        assert_eq!(all[1], LearningIntensity::Medium);
        assert_eq!(all[2], LearningIntensity::High);

        // Test description()
        assert!(LearningIntensity::Low.description().contains("Low"));
        assert!(LearningIntensity::Medium.description().contains("Moderate"));
        assert!(LearningIntensity::High.description().contains("Strong"));

        // Test Display
        assert_eq!(format!("{}", LearningIntensity::Low), "Low");
        assert_eq!(format!("{}", LearningIntensity::Medium), "Medium");
        assert_eq!(format!("{}", LearningIntensity::High), "High");
    }

    #[test]
    fn test_from_magnitude() {
        assert_eq!(
            LearningIntensity::from_magnitude(0.1),
            LearningIntensity::Low
        );
        assert_eq!(
            LearningIntensity::from_magnitude(0.29),
            LearningIntensity::Low
        );
        assert_eq!(
            LearningIntensity::from_magnitude(0.3),
            LearningIntensity::Medium
        );
        assert_eq!(
            LearningIntensity::from_magnitude(0.5),
            LearningIntensity::Medium
        );
        assert_eq!(
            LearningIntensity::from_magnitude(0.69),
            LearningIntensity::Medium
        );
        assert_eq!(
            LearningIntensity::from_magnitude(0.7),
            LearningIntensity::High
        );
        assert_eq!(
            LearningIntensity::from_magnitude(0.9),
            LearningIntensity::High
        );
    }
}
