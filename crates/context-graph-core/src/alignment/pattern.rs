//! Alignment pattern detection and analysis.
//!
//! Provides types and functions for detecting specific alignment patterns
//! that indicate issues or opportunities in the goal alignment.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::fingerprint::{AlignmentThreshold, PurposeVector, NUM_EMBEDDERS};

/// Detected alignment pattern with context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentPattern {
    /// Pattern type
    pub pattern_type: PatternType,
    /// Severity level (0 = info, 1 = warning, 2 = critical)
    pub severity: u8,
    /// Human-readable description
    pub description: String,
    /// Affected goal IDs (if applicable)
    pub affected_goals: Vec<Uuid>,
    /// Suggested action
    pub suggestion: String,
}

/// Types of alignment patterns that can be detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PatternType {
    /// High tactical alignment without strategic direction
    TacticalWithoutStrategic,
    /// Child goals diverging from parent direction
    DivergentHierarchy,
    /// One or more goals below critical threshold
    CriticalMisalignment,
    /// High variance across embedding spaces
    InconsistentAlignment,
    /// North Star misalignment (most severe)
    NorthStarDrift,
    /// All goals optimally aligned (positive pattern)
    OptimalAlignment,
    /// Hierarchical coherence (positive pattern)
    HierarchicalCoherence,
}

impl PatternType {
    /// Check if this is a positive (good) pattern.
    #[inline]
    pub fn is_positive(&self) -> bool {
        matches!(self, Self::OptimalAlignment | Self::HierarchicalCoherence)
    }

    /// Check if this is a negative (problematic) pattern.
    #[inline]
    pub fn is_negative(&self) -> bool {
        !self.is_positive()
    }

    /// Get default severity for this pattern type.
    pub fn default_severity(&self) -> u8 {
        match self {
            Self::OptimalAlignment | Self::HierarchicalCoherence => 0,
            Self::TacticalWithoutStrategic | Self::InconsistentAlignment => 1,
            Self::DivergentHierarchy | Self::CriticalMisalignment => 2,
            Self::NorthStarDrift => 2,
        }
    }
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TacticalWithoutStrategic => write!(f, "Tactical Without Strategic"),
            Self::DivergentHierarchy => write!(f, "Divergent Hierarchy"),
            Self::CriticalMisalignment => write!(f, "Critical Misalignment"),
            Self::InconsistentAlignment => write!(f, "Inconsistent Alignment"),
            Self::NorthStarDrift => write!(f, "North Star Drift"),
            Self::OptimalAlignment => write!(f, "Optimal Alignment"),
            Self::HierarchicalCoherence => write!(f, "Hierarchical Coherence"),
        }
    }
}

impl AlignmentPattern {
    /// Create a new pattern.
    pub fn new(
        pattern_type: PatternType,
        description: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            severity: pattern_type.default_severity(),
            pattern_type,
            description: description.into(),
            affected_goals: Vec::new(),
            suggestion: suggestion.into(),
        }
    }

    /// Add affected goals.
    pub fn with_affected_goals(mut self, goals: Vec<Uuid>) -> Self {
        self.affected_goals = goals;
        self
    }

    /// Override severity.
    pub fn with_severity(mut self, severity: u8) -> Self {
        self.severity = severity.min(2);
        self
    }

    /// Check if this is a critical pattern.
    #[inline]
    pub fn is_critical(&self) -> bool {
        self.severity == 2
    }

    /// Check if this is a warning pattern.
    #[inline]
    pub fn is_warning(&self) -> bool {
        self.severity == 1
    }

    /// Check if this is an info/positive pattern.
    #[inline]
    pub fn is_info(&self) -> bool {
        self.severity == 0
    }
}

/// Per-embedder alignment breakdown.
///
/// Provides detailed alignment information for each of the 13 embedding spaces.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedderBreakdown {
    /// Alignment value for each embedder
    pub alignments: [f32; NUM_EMBEDDERS],
    /// Threshold classification for each embedder
    pub thresholds: [AlignmentThreshold; NUM_EMBEDDERS],
    /// Index of best-aligned embedder
    pub best_embedder: usize,
    /// Index of worst-aligned embedder
    pub worst_embedder: usize,
    /// Standard deviation across embedders
    pub std_dev: f32,
    /// Mean alignment
    pub mean: f32,
}

impl EmbedderBreakdown {
    /// Create from a purpose vector.
    pub fn from_purpose_vector(pv: &PurposeVector) -> Self {
        let alignments = pv.alignments;
        let mut thresholds = [AlignmentThreshold::Critical; NUM_EMBEDDERS];

        let mut best_idx = 0;
        let mut best_val = alignments[0];
        let mut worst_idx = 0;
        let mut worst_val = alignments[0];
        let mut sum = 0.0f32;

        for (i, &alignment) in alignments.iter().enumerate() {
            thresholds[i] = AlignmentThreshold::classify(alignment);
            sum += alignment;

            if alignment > best_val {
                best_val = alignment;
                best_idx = i;
            }
            if alignment < worst_val {
                worst_val = alignment;
                worst_idx = i;
            }
        }

        let mean = sum / NUM_EMBEDDERS as f32;

        // Compute standard deviation
        let variance: f32 = alignments
            .iter()
            .map(|x: &f32| (*x - mean).powi(2))
            .sum::<f32>()
            / NUM_EMBEDDERS as f32;
        let std_dev = variance.sqrt();

        Self {
            alignments,
            thresholds,
            best_embedder: best_idx,
            worst_embedder: worst_idx,
            std_dev,
            mean,
        }
    }

    /// Get embedder name by index.
    pub fn embedder_name(idx: usize) -> &'static str {
        match idx {
            0 => "E1_Semantic",
            1 => "E2_Temporal_Recent",
            2 => "E3_Temporal_Periodic",
            3 => "E4_Temporal_Positional",
            4 => "E5_Causal",
            5 => "E6_Sparse",
            6 => "E7_Code",
            7 => "E8_Graph",
            8 => "E9_HDC",
            9 => "E10_Multimodal",
            10 => "E11_Entity",
            11 => "E12_LateInteraction",
            12 => "E13_SPLADE",
            _ => "Unknown",
        }
    }

    /// Count embedders at each threshold level.
    pub fn threshold_counts(&self) -> (usize, usize, usize, usize) {
        let mut optimal = 0;
        let mut acceptable = 0;
        let mut warning = 0;
        let mut critical = 0;

        for threshold in &self.thresholds {
            match threshold {
                AlignmentThreshold::Optimal => optimal += 1,
                AlignmentThreshold::Acceptable => acceptable += 1,
                AlignmentThreshold::Warning => warning += 1,
                AlignmentThreshold::Critical => critical += 1,
            }
        }

        (optimal, acceptable, warning, critical)
    }

    /// Get embedders that are misaligned (Warning or Critical).
    pub fn misaligned_embedders(&self) -> Vec<(usize, &'static str, f32)> {
        self.alignments
            .iter()
            .enumerate()
            .filter(|(_, &a)| AlignmentThreshold::classify(a).is_misaligned())
            .map(|(i, &a)| (i, Self::embedder_name(i), a))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_type_is_positive() {
        assert!(PatternType::OptimalAlignment.is_positive());
        assert!(PatternType::HierarchicalCoherence.is_positive());
        assert!(!PatternType::CriticalMisalignment.is_positive());
        assert!(!PatternType::NorthStarDrift.is_positive());

        println!("[VERIFIED] PatternType::is_positive distinguishes good/bad patterns");
    }

    #[test]
    fn test_pattern_type_severity() {
        assert_eq!(PatternType::OptimalAlignment.default_severity(), 0);
        assert_eq!(PatternType::TacticalWithoutStrategic.default_severity(), 1);
        assert_eq!(PatternType::CriticalMisalignment.default_severity(), 2);
        assert_eq!(PatternType::NorthStarDrift.default_severity(), 2);

        println!("[VERIFIED] PatternType severity levels are correct");
    }

    #[test]
    fn test_alignment_pattern_creation() {
        let pattern = AlignmentPattern::new(
            PatternType::CriticalMisalignment,
            "Goal X is below critical threshold",
            "Review and adjust goal X alignment",
        )
        .with_affected_goals(vec![Uuid::new_v4()])
        .with_severity(2);

        assert_eq!(pattern.pattern_type, PatternType::CriticalMisalignment);
        assert_eq!(pattern.severity, 2);
        assert_eq!(pattern.affected_goals.len(), 1);
        assert!(pattern.is_critical());
        assert!(!pattern.is_warning());
        assert!(!pattern.is_info());

        println!("[VERIFIED] AlignmentPattern creation and builder work");
        println!("  - description: {}", pattern.description);
        println!("  - suggestion: {}", pattern.suggestion);
    }

    #[test]
    fn test_embedder_breakdown_from_purpose_vector() {
        // Create a purpose vector with varying alignments
        let mut alignments = [0.75; NUM_EMBEDDERS];
        alignments[0] = 0.95; // Best
        alignments[5] = 0.40; // Worst (critical)
        alignments[8] = 0.60; // Warning

        let pv = PurposeVector::new(alignments);
        let breakdown = EmbedderBreakdown::from_purpose_vector(&pv);

        assert_eq!(breakdown.best_embedder, 0);
        assert_eq!(breakdown.worst_embedder, 5);
        assert!(breakdown.std_dev > 0.0);

        let (optimal, acceptable, warning, critical) = breakdown.threshold_counts();
        assert!(optimal > 0);
        assert!(critical > 0);

        println!("[VERIFIED] EmbedderBreakdown computes correctly");
        println!(
            "  - best_embedder: {} ({})",
            breakdown.best_embedder,
            EmbedderBreakdown::embedder_name(breakdown.best_embedder)
        );
        println!(
            "  - worst_embedder: {} ({})",
            breakdown.worst_embedder,
            EmbedderBreakdown::embedder_name(breakdown.worst_embedder)
        );
        println!(
            "  - mean: {:.3}, std_dev: {:.3}",
            breakdown.mean, breakdown.std_dev
        );
        println!(
            "  - thresholds: optimal={}, acceptable={}, warning={}, critical={}",
            optimal, acceptable, warning, critical
        );
    }

    #[test]
    fn test_embedder_breakdown_misaligned() {
        let mut alignments = [0.80; NUM_EMBEDDERS];
        alignments[2] = 0.50; // Critical
        alignments[6] = 0.60; // Warning
        alignments[10] = 0.55; // Warning (exactly at boundary)

        let pv = PurposeVector::new(alignments);
        let breakdown = EmbedderBreakdown::from_purpose_vector(&pv);

        let misaligned = breakdown.misaligned_embedders();
        assert_eq!(misaligned.len(), 3);

        // Verify the misaligned embedders
        let indices: Vec<usize> = misaligned.iter().map(|(i, _, _)| *i).collect();
        assert!(indices.contains(&2));
        assert!(indices.contains(&6));
        assert!(indices.contains(&10));

        println!("[VERIFIED] misaligned_embedders correctly identifies problematic spaces");
        for (idx, name, alignment) in &misaligned {
            println!("  - {}: {} = {:.2}", idx, name, alignment);
        }
    }

    #[test]
    fn test_pattern_type_display() {
        assert_eq!(
            format!("{}", PatternType::NorthStarDrift),
            "North Star Drift"
        );
        assert_eq!(
            format!("{}", PatternType::OptimalAlignment),
            "Optimal Alignment"
        );

        println!("[VERIFIED] PatternType Display trait works");
    }

    #[test]
    fn test_embedder_name_all_valid() {
        for i in 0..NUM_EMBEDDERS {
            let name = EmbedderBreakdown::embedder_name(i);
            assert!(!name.is_empty());
            assert_ne!(name, "Unknown");
            println!("  E{}: {}", i + 1, name);
        }

        // Invalid index should return Unknown
        assert_eq!(EmbedderBreakdown::embedder_name(99), "Unknown");

        println!("[VERIFIED] All 13 embedder names are defined");
    }
}
