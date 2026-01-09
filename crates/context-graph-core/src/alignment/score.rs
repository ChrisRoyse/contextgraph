//! Goal alignment scoring types.
//!
//! Defines the data structures for computing and representing alignment scores
//! across the goal hierarchy levels (NorthStar, Strategic, Tactical, Immediate).
//!
//! From constitution.yaml thresholds:
//! - Optimal: θ ≥ 0.75
//! - Acceptable: θ ∈ [0.70, 0.75)
//! - Warning: θ ∈ [0.55, 0.70)
//! - Critical: θ < 0.55

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::purpose::GoalLevel;
use crate::types::fingerprint::AlignmentThreshold;

/// Level-based weights for computing composite alignment score.
///
/// From TASK-L003 specification:
/// - north_star: 0.4 (highest influence)
/// - strategic: 0.3
/// - tactical: 0.2
/// - immediate: 0.1 (lowest influence)
///
/// # Invariant
/// All weights MUST sum to 1.0. This is enforced by `validate()`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LevelWeights {
    /// Weight for North Star goal alignment (default: 0.4)
    pub north_star: f32,
    /// Weight for Strategic goals (default: 0.3)
    pub strategic: f32,
    /// Weight for Tactical goals (default: 0.2)
    pub tactical: f32,
    /// Weight for Immediate goals (default: 0.1)
    pub immediate: f32,
}

impl Default for LevelWeights {
    fn default() -> Self {
        Self {
            north_star: 0.4,
            strategic: 0.3,
            tactical: 0.2,
            immediate: 0.1,
        }
    }
}

impl LevelWeights {
    /// Create new weights. Returns error if they don't sum to 1.0.
    ///
    /// # Errors
    /// Returns error if weights don't sum to 1.0 (within epsilon tolerance).
    pub fn new(
        north_star: f32,
        strategic: f32,
        tactical: f32,
        immediate: f32,
    ) -> Result<Self, &'static str> {
        let weights = Self {
            north_star,
            strategic,
            tactical,
            immediate,
        };
        weights.validate()?;
        Ok(weights)
    }

    /// Validate that weights sum to 1.0.
    ///
    /// # Errors
    /// Returns error if sum deviates from 1.0 by more than 0.001.
    pub fn validate(&self) -> Result<(), &'static str> {
        let sum = self.north_star + self.strategic + self.tactical + self.immediate;
        if (sum - 1.0).abs() > 0.001 {
            return Err("LevelWeights must sum to 1.0");
        }
        if self.north_star < 0.0
            || self.strategic < 0.0
            || self.tactical < 0.0
            || self.immediate < 0.0
        {
            return Err("LevelWeights cannot be negative");
        }
        Ok(())
    }

    /// Get weight for a specific goal level.
    #[inline]
    pub fn for_level(&self, level: GoalLevel) -> f32 {
        match level {
            GoalLevel::NorthStar => self.north_star,
            GoalLevel::Strategic => self.strategic,
            GoalLevel::Tactical => self.tactical,
            GoalLevel::Immediate => self.immediate,
        }
    }

    /// Sum of all weights (should be 1.0).
    #[inline]
    pub fn sum(&self) -> f32 {
        self.north_star + self.strategic + self.tactical + self.immediate
    }
}

/// Alignment score for a single goal.
///
/// Contains the raw alignment value, threshold classification,
/// and metadata about the goal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalScore {
    /// Goal identifier (UUID)
    pub goal_id: Uuid,
    /// Goal level in hierarchy
    pub level: GoalLevel,
    /// Raw alignment score [-1.0, 1.0] (cosine similarity)
    pub alignment: f32,
    /// Weighted contribution to composite score
    pub weighted_contribution: f32,
    /// Threshold classification
    pub threshold: AlignmentThreshold,
}

impl GoalScore {
    /// Create a new goal score.
    ///
    /// Automatically classifies the alignment into the appropriate threshold.
    pub fn new(goal_id: Uuid, level: GoalLevel, alignment: f32, weight: f32) -> Self {
        Self {
            goal_id,
            level,
            alignment,
            weighted_contribution: alignment * weight,
            threshold: AlignmentThreshold::classify(alignment),
        }
    }

    /// Check if this goal is misaligned (Warning or Critical).
    #[inline]
    pub fn is_misaligned(&self) -> bool {
        self.threshold.is_misaligned()
    }

    /// Check if this goal is critically misaligned.
    #[inline]
    pub fn is_critical(&self) -> bool {
        self.threshold.is_critical()
    }
}

/// Complete alignment score for a fingerprint against a goal hierarchy.
///
/// Contains:
/// - Composite score (weighted average across all goals)
/// - Per-level breakdown
/// - Individual goal scores
/// - Misalignment flags
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoalAlignmentScore {
    /// Weighted composite alignment score [0.0, 1.0]
    ///
    /// Computed as: Σ(level_weight × level_avg_alignment)
    pub composite_score: f32,

    /// Overall threshold classification for the composite score
    pub threshold: AlignmentThreshold,

    /// North Star alignment (single goal)
    pub north_star_alignment: f32,

    /// Average alignment for Strategic level goals
    pub strategic_alignment: f32,

    /// Average alignment for Tactical level goals
    pub tactical_alignment: f32,

    /// Average alignment for Immediate level goals
    pub immediate_alignment: f32,

    /// Individual goal scores (for detailed analysis)
    pub goal_scores: Vec<GoalScore>,

    /// Weights used for computation
    pub weights: LevelWeights,

    /// Count of misaligned goals (Warning + Critical)
    pub misaligned_count: usize,

    /// Count of critically misaligned goals
    pub critical_count: usize,
}

impl GoalAlignmentScore {
    /// Create a new GoalAlignmentScore from individual goal scores.
    ///
    /// Computes composite score and aggregates per-level averages.
    pub fn compute(goal_scores: Vec<GoalScore>, weights: LevelWeights) -> Self {
        // Separate scores by level
        let mut north_star_scores: Vec<f32> = Vec::new();
        let mut strategic_scores: Vec<f32> = Vec::new();
        let mut tactical_scores: Vec<f32> = Vec::new();
        let mut immediate_scores: Vec<f32> = Vec::new();

        let mut misaligned_count = 0;
        let mut critical_count = 0;

        for score in &goal_scores {
            if score.is_misaligned() {
                misaligned_count += 1;
            }
            if score.is_critical() {
                critical_count += 1;
            }

            match score.level {
                GoalLevel::NorthStar => north_star_scores.push(score.alignment),
                GoalLevel::Strategic => strategic_scores.push(score.alignment),
                GoalLevel::Tactical => tactical_scores.push(score.alignment),
                GoalLevel::Immediate => immediate_scores.push(score.alignment),
            }
        }

        // Compute level averages (default to 0.0 if no goals at that level)
        let avg = |scores: &[f32]| -> f32 {
            if scores.is_empty() {
                0.0
            } else {
                scores.iter().sum::<f32>() / scores.len() as f32
            }
        };

        let north_star_alignment = avg(&north_star_scores);
        let strategic_alignment = avg(&strategic_scores);
        let tactical_alignment = avg(&tactical_scores);
        let immediate_alignment = avg(&immediate_scores);

        // Compute weighted composite
        // Only include levels that have goals
        let mut composite = 0.0;
        let mut weight_sum = 0.0;

        if !north_star_scores.is_empty() {
            composite += weights.north_star * north_star_alignment;
            weight_sum += weights.north_star;
        }
        if !strategic_scores.is_empty() {
            composite += weights.strategic * strategic_alignment;
            weight_sum += weights.strategic;
        }
        if !tactical_scores.is_empty() {
            composite += weights.tactical * tactical_alignment;
            weight_sum += weights.tactical;
        }
        if !immediate_scores.is_empty() {
            composite += weights.immediate * immediate_alignment;
            weight_sum += weights.immediate;
        }

        // Normalize by actual weight sum (handles missing levels)
        let composite_score = if weight_sum > 0.0 {
            composite / weight_sum
        } else {
            0.0
        };

        Self {
            composite_score,
            threshold: AlignmentThreshold::classify(composite_score),
            north_star_alignment,
            strategic_alignment,
            tactical_alignment,
            immediate_alignment,
            goal_scores,
            weights,
            misaligned_count,
            critical_count,
        }
    }

    /// Create an empty score (no goals).
    pub fn empty(weights: LevelWeights) -> Self {
        Self {
            composite_score: 0.0,
            threshold: AlignmentThreshold::Critical,
            north_star_alignment: 0.0,
            strategic_alignment: 0.0,
            tactical_alignment: 0.0,
            immediate_alignment: 0.0,
            goal_scores: Vec::new(),
            weights,
            misaligned_count: 0,
            critical_count: 0,
        }
    }

    /// Check if any goals are misaligned.
    #[inline]
    pub fn has_misalignment(&self) -> bool {
        self.misaligned_count > 0
    }

    /// Check if any goals are critically misaligned.
    #[inline]
    pub fn has_critical(&self) -> bool {
        self.critical_count > 0
    }

    /// Get misaligned goals.
    pub fn misaligned_goals(&self) -> Vec<&GoalScore> {
        self.goal_scores
            .iter()
            .filter(|s| s.is_misaligned())
            .collect()
    }

    /// Get critically misaligned goals.
    pub fn critical_goals(&self) -> Vec<&GoalScore> {
        self.goal_scores
            .iter()
            .filter(|s| s.is_critical())
            .collect()
    }

    /// Number of goals scored.
    #[inline]
    pub fn goal_count(&self) -> usize {
        self.goal_scores.len()
    }
}

impl Default for GoalAlignmentScore {
    fn default() -> Self {
        Self::empty(LevelWeights::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_weights_default() {
        let weights = LevelWeights::default();
        assert_eq!(weights.north_star, 0.4);
        assert_eq!(weights.strategic, 0.3);
        assert_eq!(weights.tactical, 0.2);
        assert_eq!(weights.immediate, 0.1);
        assert!((weights.sum() - 1.0).abs() < 0.001);
        assert!(weights.validate().is_ok());

        println!("[VERIFIED] LevelWeights::default() sums to 1.0");
        println!("  - north_star: {}", weights.north_star);
        println!("  - strategic: {}", weights.strategic);
        println!("  - tactical: {}", weights.tactical);
        println!("  - immediate: {}", weights.immediate);
    }

    #[test]
    fn test_level_weights_new_valid() {
        let weights = LevelWeights::new(0.5, 0.25, 0.15, 0.1).unwrap();
        assert!((weights.sum() - 1.0).abs() < 0.001);
        println!("[VERIFIED] LevelWeights::new accepts valid weights that sum to 1.0");
    }

    #[test]
    fn test_level_weights_new_invalid() {
        let result = LevelWeights::new(0.5, 0.5, 0.5, 0.5);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "LevelWeights must sum to 1.0");
        println!("[VERIFIED] LevelWeights::new rejects weights that don't sum to 1.0");
    }

    #[test]
    fn test_level_weights_negative() {
        let result = LevelWeights::new(1.5, -0.5, 0.0, 0.0);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("negative"));
        println!("[VERIFIED] LevelWeights::new rejects negative weights");
    }

    #[test]
    fn test_level_weights_for_level() {
        let weights = LevelWeights::default();
        assert_eq!(weights.for_level(GoalLevel::NorthStar), 0.4);
        assert_eq!(weights.for_level(GoalLevel::Strategic), 0.3);
        assert_eq!(weights.for_level(GoalLevel::Tactical), 0.2);
        assert_eq!(weights.for_level(GoalLevel::Immediate), 0.1);
        println!("[VERIFIED] for_level returns correct weight for each GoalLevel");
    }

    #[test]
    fn test_goal_score_creation() {
        let goal_id = Uuid::new_v4();
        let score = GoalScore::new(goal_id, GoalLevel::Strategic, 0.80, 0.3);

        assert_eq!(score.goal_id, goal_id);
        assert_eq!(score.level, GoalLevel::Strategic);
        assert_eq!(score.alignment, 0.80);
        assert!((score.weighted_contribution - 0.24).abs() < 0.001);
        assert_eq!(score.threshold, AlignmentThreshold::Optimal);
        assert!(!score.is_misaligned());
        assert!(!score.is_critical());

        println!("[VERIFIED] GoalScore::new correctly initializes all fields");
        println!(
            "  - alignment: {} -> threshold: {:?}",
            score.alignment, score.threshold
        );
        println!("  - weighted_contribution: {}", score.weighted_contribution);
    }

    #[test]
    fn test_goal_score_misalignment_detection() {
        let optimal = GoalScore::new(Uuid::new_v4(), GoalLevel::NorthStar, 0.80, 1.0);
        let acceptable = GoalScore::new(Uuid::new_v4(), GoalLevel::NorthStar, 0.72, 1.0);
        let warning = GoalScore::new(Uuid::new_v4(), GoalLevel::NorthStar, 0.60, 1.0);
        let critical = GoalScore::new(Uuid::new_v4(), GoalLevel::NorthStar, 0.40, 1.0);

        assert!(!optimal.is_misaligned());
        assert!(!optimal.is_critical());

        assert!(!acceptable.is_misaligned());
        assert!(!acceptable.is_critical());

        assert!(warning.is_misaligned());
        assert!(!warning.is_critical());

        assert!(critical.is_misaligned());
        assert!(critical.is_critical());

        println!("[VERIFIED] GoalScore correctly detects misalignment and critical states");
    }

    #[test]
    fn test_goal_alignment_score_compute() {
        let scores = vec![
            GoalScore::new(Uuid::new_v4(), GoalLevel::NorthStar, 0.85, 0.4),
            GoalScore::new(Uuid::new_v4(), GoalLevel::Strategic, 0.75, 0.3),
            GoalScore::new(Uuid::new_v4(), GoalLevel::Strategic, 0.65, 0.3),
            GoalScore::new(Uuid::new_v4(), GoalLevel::Tactical, 0.55, 0.2),
            GoalScore::new(Uuid::new_v4(), GoalLevel::Immediate, 0.45, 0.1),
        ];

        let alignment = GoalAlignmentScore::compute(scores, LevelWeights::default());

        // North Star: 0.85
        // Strategic avg: (0.75 + 0.65) / 2 = 0.70
        // Tactical avg: 0.55
        // Immediate avg: 0.45
        // Composite = 0.4*0.85 + 0.3*0.70 + 0.2*0.55 + 0.1*0.45 = 0.34 + 0.21 + 0.11 + 0.045 = 0.705

        assert_eq!(alignment.north_star_alignment, 0.85);
        assert!((alignment.strategic_alignment - 0.70).abs() < 0.001);
        assert_eq!(alignment.tactical_alignment, 0.55);
        assert_eq!(alignment.immediate_alignment, 0.45);
        assert!((alignment.composite_score - 0.705).abs() < 0.01);
        assert_eq!(alignment.goal_count(), 5);

        // Misalignment: s2 (0.65, Warning), t1 (0.55, Warning), i1 (0.45, Critical)
        // AlignmentThreshold::classify: Warning is [0.55, 0.70), Critical is <0.55
        assert_eq!(alignment.misaligned_count, 3);
        assert_eq!(alignment.critical_count, 1);

        println!("[VERIFIED] GoalAlignmentScore::compute calculates correct values");
        println!("  - composite_score: {:.3}", alignment.composite_score);
        println!(
            "  - north_star_alignment: {:.2}",
            alignment.north_star_alignment
        );
        println!(
            "  - strategic_alignment: {:.2}",
            alignment.strategic_alignment
        );
        println!(
            "  - tactical_alignment: {:.2}",
            alignment.tactical_alignment
        );
        println!(
            "  - immediate_alignment: {:.2}",
            alignment.immediate_alignment
        );
        println!("  - misaligned_count: {}", alignment.misaligned_count);
        println!("  - critical_count: {}", alignment.critical_count);
    }

    #[test]
    fn test_goal_alignment_score_empty() {
        let alignment = GoalAlignmentScore::empty(LevelWeights::default());

        assert_eq!(alignment.composite_score, 0.0);
        assert_eq!(alignment.threshold, AlignmentThreshold::Critical);
        assert_eq!(alignment.goal_count(), 0);
        assert!(!alignment.has_misalignment());
        assert!(!alignment.has_critical());

        println!("[VERIFIED] GoalAlignmentScore::empty creates valid empty state");
    }

    #[test]
    fn test_goal_alignment_score_partial_hierarchy() {
        // Only NorthStar and Strategic (no Tactical/Immediate)
        let scores = vec![
            GoalScore::new(Uuid::new_v4(), GoalLevel::NorthStar, 0.90, 0.4),
            GoalScore::new(Uuid::new_v4(), GoalLevel::Strategic, 0.80, 0.3),
        ];

        let alignment = GoalAlignmentScore::compute(scores, LevelWeights::default());

        // Should normalize to available levels only
        // weight_sum = 0.4 + 0.3 = 0.7
        // composite = (0.4*0.90 + 0.3*0.80) / 0.7 = (0.36 + 0.24) / 0.7 = 0.857
        assert!((alignment.composite_score - 0.857).abs() < 0.01);
        assert_eq!(alignment.tactical_alignment, 0.0);
        assert_eq!(alignment.immediate_alignment, 0.0);

        println!("[VERIFIED] GoalAlignmentScore handles partial hierarchy correctly");
        println!(
            "  - composite_score (normalized): {:.3}",
            alignment.composite_score
        );
    }

    #[test]
    fn test_goal_alignment_score_misaligned_goals() {
        let ns_id = Uuid::new_v4();
        let scores = vec![
            GoalScore::new(ns_id, GoalLevel::NorthStar, 0.50, 0.4), // Critical
            GoalScore::new(Uuid::new_v4(), GoalLevel::Strategic, 0.60, 0.3), // Warning
            GoalScore::new(Uuid::new_v4(), GoalLevel::Strategic, 0.80, 0.3), // Optimal
        ];

        let alignment = GoalAlignmentScore::compute(scores, LevelWeights::default());

        assert!(alignment.has_misalignment());
        assert!(alignment.has_critical());

        let misaligned = alignment.misaligned_goals();
        assert_eq!(misaligned.len(), 2);

        let critical = alignment.critical_goals();
        assert_eq!(critical.len(), 1);
        assert_eq!(critical[0].goal_id, ns_id);

        println!("[VERIFIED] misaligned_goals and critical_goals filter correctly");
    }
}
