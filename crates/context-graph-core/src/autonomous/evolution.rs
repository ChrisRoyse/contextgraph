//! Goal hierarchy evolution types
//!
//! This module defines types for goal evolution including sub-goal discovery,
//! obsolescence detection, and weight adjustment for the autonomous North Star system.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::bootstrap::GoalId;
use super::curation::MemoryId;

/// Goal levels in the hierarchy
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum GoalLevel {
    NorthStar,   // Root goal
    Strategic,   // High-level goals
    Tactical,    // Mid-level goals
    Operational, // Low-level goals
}

/// Goal evolution configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalEvolutionConfig {
    /// Enable automatic sub-goal discovery
    pub auto_discover: bool,

    /// Minimum cluster size for goal creation
    pub min_cluster_size: usize, // default: 10

    /// Days without activity for obsolescence
    pub obsolescence_days: u32, // default: 60

    /// Weight adjustment learning rate
    pub weight_lr: f32, // default: 0.05

    /// Weight bounds for sub-goals
    pub weight_bounds: (f32, f32), // default: (0.3, 0.95)
}

impl Default for GoalEvolutionConfig {
    fn default() -> Self {
        Self {
            auto_discover: true,
            min_cluster_size: 10,
            obsolescence_days: 60,
            weight_lr: 0.05,
            weight_bounds: (0.3, 0.95),
        }
    }
}

/// Discovered sub-goal candidate
#[derive(Clone, Debug)]
pub struct SubGoalCandidate {
    pub suggested_description: String,
    pub level: GoalLevel,
    pub parent_id: GoalId,
    pub cluster_size: usize,
    pub centroid_alignment: f32,
    pub confidence: f32,
    pub supporting_memories: Vec<MemoryId>,
}

impl SubGoalCandidate {
    /// Check if this candidate meets minimum requirements
    pub fn is_viable(&self, config: &GoalEvolutionConfig) -> bool {
        self.cluster_size >= config.min_cluster_size && self.confidence >= 0.6
    }
}

/// Goal activity metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalActivityMetrics {
    pub goal_id: GoalId,
    pub new_aligned_memories_30d: u32,
    pub retrievals_14d: u32,
    pub avg_child_alignment: f32,
    pub weight_trend: f32,
    pub last_activity: DateTime<Utc>,
}

impl GoalActivityMetrics {
    /// Check if goal is active
    pub fn is_active(&self) -> bool {
        self.new_aligned_memories_30d > 0 || self.retrievals_14d > 0
    }

    /// Calculate activity score
    pub fn activity_score(&self) -> f32 {
        let memory_score = (self.new_aligned_memories_30d as f32).min(100.0) / 100.0;
        let retrieval_score = (self.retrievals_14d as f32).min(50.0) / 50.0;
        0.6 * memory_score + 0.4 * retrieval_score
    }
}

/// Goal state for lifecycle
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum GoalState {
    #[default]
    Active,
    Sunset {
        since: DateTime<Utc>,
    },
    Archived {
        since: DateTime<Utc>,
    },
}

/// Reasons for goal obsolescence
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ObsolescenceReason {
    NoNewMemories { days: u32 },
    NoRetrievals { days: u32 },
    ChildAlignmentDropping { current: f32, previous: f32 },
    UserDeprioritized,
}

/// Recommended action for obsolete goal
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ObsolescenceAction {
    Keep,
    MarkSunset,
    MigrateChildren { target: GoalId },
    Archive,
}

/// Goal obsolescence evaluation
#[derive(Clone, Debug)]
pub struct ObsolescenceEvaluation {
    pub goal_id: GoalId,
    pub is_obsolete: bool,
    pub reasons: Vec<ObsolescenceReason>,
    pub recommendation: ObsolescenceAction,
}

impl ObsolescenceEvaluation {
    /// Create evaluation for active goal
    pub fn active(goal_id: GoalId) -> Self {
        Self {
            goal_id,
            is_obsolete: false,
            reasons: vec![],
            recommendation: ObsolescenceAction::Keep,
        }
    }

    /// Create evaluation for obsolete goal
    pub fn obsolete(
        goal_id: GoalId,
        reasons: Vec<ObsolescenceReason>,
        recommendation: ObsolescenceAction,
    ) -> Self {
        Self {
            goal_id,
            is_obsolete: true,
            reasons,
            recommendation,
        }
    }

    /// Check if any action is needed
    pub fn needs_action(&self) -> bool {
        !matches!(self.recommendation, ObsolescenceAction::Keep)
    }
}

/// Weight adjustment record
#[derive(Clone, Debug)]
pub struct WeightAdjustment {
    pub goal_id: GoalId,
    pub old_weight: f32,
    pub new_weight: f32,
    pub reason: AdjustmentReason,
}

/// Reason for weight adjustment
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AdjustmentReason {
    HighRetrievalActivity,
    LowActivity,
    UserFeedback { magnitude: f32 },
    ChildGoalPromotion,
}

impl WeightAdjustment {
    /// Calculate the adjustment delta
    pub fn delta(&self) -> f32 {
        self.new_weight - self.old_weight
    }

    /// Check if this is a significant adjustment
    pub fn is_significant(&self) -> bool {
        self.delta().abs() >= 0.05
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // GoalLevel tests
    #[test]
    fn test_goal_level_equality() {
        assert_eq!(GoalLevel::NorthStar, GoalLevel::NorthStar);
        assert_ne!(GoalLevel::NorthStar, GoalLevel::Strategic);
        assert_ne!(GoalLevel::Strategic, GoalLevel::Tactical);
        assert_ne!(GoalLevel::Tactical, GoalLevel::Operational);
    }

    #[test]
    fn test_goal_level_serialization() {
        let levels = [
            GoalLevel::NorthStar,
            GoalLevel::Strategic,
            GoalLevel::Tactical,
            GoalLevel::Operational,
        ];

        for level in levels {
            let json = serde_json::to_string(&level).expect("serialize");
            let deserialized: GoalLevel = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, level);
        }
    }

    // GoalEvolutionConfig tests
    #[test]
    fn test_goal_evolution_config_default() {
        let config = GoalEvolutionConfig::default();
        assert!(config.auto_discover);
        assert_eq!(config.min_cluster_size, 10);
        assert_eq!(config.obsolescence_days, 60);
        assert!((config.weight_lr - 0.05).abs() < f32::EPSILON);
        assert!((config.weight_bounds.0 - 0.3).abs() < f32::EPSILON);
        assert!((config.weight_bounds.1 - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_goal_evolution_config_serialization() {
        let config = GoalEvolutionConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: GoalEvolutionConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.auto_discover, config.auto_discover);
        assert_eq!(deserialized.min_cluster_size, config.min_cluster_size);
        assert_eq!(deserialized.obsolescence_days, config.obsolescence_days);
        assert!((deserialized.weight_lr - config.weight_lr).abs() < f32::EPSILON);
        assert!((deserialized.weight_bounds.0 - config.weight_bounds.0).abs() < f32::EPSILON);
        assert!((deserialized.weight_bounds.1 - config.weight_bounds.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_goal_evolution_config_custom() {
        let config = GoalEvolutionConfig {
            auto_discover: false,
            min_cluster_size: 20,
            obsolescence_days: 90,
            weight_lr: 0.10,
            weight_bounds: (0.2, 0.9),
        };
        assert!(!config.auto_discover);
        assert_eq!(config.min_cluster_size, 20);
        assert_eq!(config.obsolescence_days, 90);
        assert!((config.weight_lr - 0.10).abs() < f32::EPSILON);
        assert!((config.weight_bounds.0 - 0.2).abs() < f32::EPSILON);
        assert!((config.weight_bounds.1 - 0.9).abs() < f32::EPSILON);
    }

    // SubGoalCandidate tests
    #[test]
    fn test_sub_goal_candidate_is_viable() {
        let config = GoalEvolutionConfig::default();

        let viable_candidate = SubGoalCandidate {
            suggested_description: "Test goal".into(),
            level: GoalLevel::Strategic,
            parent_id: GoalId::new(),
            cluster_size: 15,
            centroid_alignment: 0.8,
            confidence: 0.7,
            supporting_memories: vec![MemoryId::new()],
        };
        assert!(viable_candidate.is_viable(&config));
    }

    #[test]
    fn test_sub_goal_candidate_not_viable_small_cluster() {
        let config = GoalEvolutionConfig::default();

        let candidate = SubGoalCandidate {
            suggested_description: "Test goal".into(),
            level: GoalLevel::Strategic,
            parent_id: GoalId::new(),
            cluster_size: 5, // Below min_cluster_size of 10
            centroid_alignment: 0.8,
            confidence: 0.7,
            supporting_memories: vec![MemoryId::new()],
        };
        assert!(!candidate.is_viable(&config));
    }

    #[test]
    fn test_sub_goal_candidate_not_viable_low_confidence() {
        let config = GoalEvolutionConfig::default();

        let candidate = SubGoalCandidate {
            suggested_description: "Test goal".into(),
            level: GoalLevel::Strategic,
            parent_id: GoalId::new(),
            cluster_size: 15,
            centroid_alignment: 0.8,
            confidence: 0.5, // Below 0.6 threshold
            supporting_memories: vec![MemoryId::new()],
        };
        assert!(!candidate.is_viable(&config));
    }

    #[test]
    fn test_sub_goal_candidate_boundary_conditions() {
        let config = GoalEvolutionConfig::default();

        // Exactly at thresholds - should be viable
        let candidate = SubGoalCandidate {
            suggested_description: "Test goal".into(),
            level: GoalLevel::Strategic,
            parent_id: GoalId::new(),
            cluster_size: 10, // Exactly at min_cluster_size
            centroid_alignment: 0.8,
            confidence: 0.6, // Exactly at threshold
            supporting_memories: vec![],
        };
        assert!(candidate.is_viable(&config));

        // Just below thresholds
        let candidate = SubGoalCandidate {
            suggested_description: "Test goal".into(),
            level: GoalLevel::Strategic,
            parent_id: GoalId::new(),
            cluster_size: 9, // Just below
            centroid_alignment: 0.8,
            confidence: 0.6,
            supporting_memories: vec![],
        };
        assert!(!candidate.is_viable(&config));
    }

    // GoalActivityMetrics tests
    #[test]
    fn test_goal_activity_metrics_is_active() {
        let now = Utc::now();

        let active_with_memories = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 5,
            retrievals_14d: 0,
            avg_child_alignment: 0.7,
            weight_trend: 0.0,
            last_activity: now,
        };
        assert!(active_with_memories.is_active());

        let active_with_retrievals = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 0,
            retrievals_14d: 10,
            avg_child_alignment: 0.7,
            weight_trend: 0.0,
            last_activity: now,
        };
        assert!(active_with_retrievals.is_active());

        let inactive = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 0,
            retrievals_14d: 0,
            avg_child_alignment: 0.7,
            weight_trend: 0.0,
            last_activity: now,
        };
        assert!(!inactive.is_active());
    }

    #[test]
    fn test_goal_activity_metrics_activity_score() {
        let now = Utc::now();

        // Zero activity
        let metrics = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 0,
            retrievals_14d: 0,
            avg_child_alignment: 0.7,
            weight_trend: 0.0,
            last_activity: now,
        };
        assert!((metrics.activity_score() - 0.0).abs() < f32::EPSILON);

        // Maximum activity (capped at 100 memories and 50 retrievals)
        let metrics = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 150, // Capped to 100
            retrievals_14d: 100,           // Capped to 50
            avg_child_alignment: 0.7,
            weight_trend: 0.0,
            last_activity: now,
        };
        assert!((metrics.activity_score() - 1.0).abs() < f32::EPSILON);

        // Partial activity: 50 memories (0.5), 25 retrievals (0.5)
        // Score = 0.6 * 0.5 + 0.4 * 0.5 = 0.5
        let metrics = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 50,
            retrievals_14d: 25,
            avg_child_alignment: 0.7,
            weight_trend: 0.0,
            last_activity: now,
        };
        assert!((metrics.activity_score() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_goal_activity_metrics_serialization() {
        let now = Utc::now();
        let metrics = GoalActivityMetrics {
            goal_id: GoalId::new(),
            new_aligned_memories_30d: 10,
            retrievals_14d: 5,
            avg_child_alignment: 0.75,
            weight_trend: 0.02,
            last_activity: now,
        };

        let json = serde_json::to_string(&metrics).expect("serialize");
        let deserialized: GoalActivityMetrics = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.goal_id, metrics.goal_id);
        assert_eq!(
            deserialized.new_aligned_memories_30d,
            metrics.new_aligned_memories_30d
        );
        assert_eq!(deserialized.retrievals_14d, metrics.retrievals_14d);
        assert!(
            (deserialized.avg_child_alignment - metrics.avg_child_alignment).abs() < f32::EPSILON
        );
        assert!((deserialized.weight_trend - metrics.weight_trend).abs() < f32::EPSILON);
    }

    // GoalState tests
    #[test]
    fn test_goal_state_default() {
        let state = GoalState::default();
        assert_eq!(state, GoalState::Active);
    }

    #[test]
    fn test_goal_state_variants() {
        let now = Utc::now();

        let active = GoalState::Active;
        let sunset = GoalState::Sunset { since: now };
        let archived = GoalState::Archived { since: now };

        assert_eq!(active, GoalState::Active);
        assert_ne!(active, sunset);
        assert_ne!(sunset, archived);
    }

    #[test]
    fn test_goal_state_serialization() {
        let now = Utc::now();

        let active = GoalState::Active;
        let json = serde_json::to_string(&active).expect("serialize");
        let deserialized: GoalState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, active);

        let sunset = GoalState::Sunset { since: now };
        let json = serde_json::to_string(&sunset).expect("serialize");
        let deserialized: GoalState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, sunset);

        let archived = GoalState::Archived { since: now };
        let json = serde_json::to_string(&archived).expect("serialize");
        let deserialized: GoalState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, archived);
    }

    // ObsolescenceReason tests
    #[test]
    fn test_obsolescence_reason_equality() {
        assert_eq!(
            ObsolescenceReason::NoNewMemories { days: 30 },
            ObsolescenceReason::NoNewMemories { days: 30 }
        );
        assert_ne!(
            ObsolescenceReason::NoNewMemories { days: 30 },
            ObsolescenceReason::NoNewMemories { days: 60 }
        );
        assert_ne!(
            ObsolescenceReason::NoNewMemories { days: 30 },
            ObsolescenceReason::NoRetrievals { days: 30 }
        );
    }

    #[test]
    fn test_obsolescence_reason_serialization() {
        let reasons = [
            ObsolescenceReason::NoNewMemories { days: 30 },
            ObsolescenceReason::NoRetrievals { days: 14 },
            ObsolescenceReason::ChildAlignmentDropping {
                current: 0.5,
                previous: 0.7,
            },
            ObsolescenceReason::UserDeprioritized,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).expect("serialize");
            let deserialized: ObsolescenceReason =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, reason);
        }
    }

    // ObsolescenceAction tests
    #[test]
    fn test_obsolescence_action_equality() {
        assert_eq!(ObsolescenceAction::Keep, ObsolescenceAction::Keep);
        assert_ne!(ObsolescenceAction::Keep, ObsolescenceAction::MarkSunset);
        assert_ne!(ObsolescenceAction::MarkSunset, ObsolescenceAction::Archive);

        let target = GoalId::new();
        assert_eq!(
            ObsolescenceAction::MigrateChildren {
                target: target.clone()
            },
            ObsolescenceAction::MigrateChildren { target }
        );
    }

    #[test]
    fn test_obsolescence_action_serialization() {
        let actions = [
            ObsolescenceAction::Keep,
            ObsolescenceAction::MarkSunset,
            ObsolescenceAction::MigrateChildren {
                target: GoalId::new(),
            },
            ObsolescenceAction::Archive,
        ];

        for action in actions {
            let json = serde_json::to_string(&action).expect("serialize");
            let deserialized: ObsolescenceAction =
                serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, action);
        }
    }

    // ObsolescenceEvaluation tests
    #[test]
    fn test_obsolescence_evaluation_active() {
        let goal_id = GoalId::new();
        let eval = ObsolescenceEvaluation::active(goal_id.clone());

        assert_eq!(eval.goal_id, goal_id);
        assert!(!eval.is_obsolete);
        assert!(eval.reasons.is_empty());
        assert_eq!(eval.recommendation, ObsolescenceAction::Keep);
        assert!(!eval.needs_action());
    }

    #[test]
    fn test_obsolescence_evaluation_obsolete() {
        let goal_id = GoalId::new();
        let reasons = vec![
            ObsolescenceReason::NoNewMemories { days: 60 },
            ObsolescenceReason::NoRetrievals { days: 30 },
        ];
        let eval = ObsolescenceEvaluation::obsolete(
            goal_id.clone(),
            reasons.clone(),
            ObsolescenceAction::MarkSunset,
        );

        assert_eq!(eval.goal_id, goal_id);
        assert!(eval.is_obsolete);
        assert_eq!(eval.reasons.len(), 2);
        assert_eq!(eval.recommendation, ObsolescenceAction::MarkSunset);
        assert!(eval.needs_action());
    }

    #[test]
    fn test_obsolescence_evaluation_needs_action() {
        let goal_id = GoalId::new();

        let keep_eval = ObsolescenceEvaluation {
            goal_id: goal_id.clone(),
            is_obsolete: false,
            reasons: vec![],
            recommendation: ObsolescenceAction::Keep,
        };
        assert!(!keep_eval.needs_action());

        let sunset_eval = ObsolescenceEvaluation {
            goal_id: goal_id.clone(),
            is_obsolete: true,
            reasons: vec![ObsolescenceReason::NoNewMemories { days: 60 }],
            recommendation: ObsolescenceAction::MarkSunset,
        };
        assert!(sunset_eval.needs_action());

        let archive_eval = ObsolescenceEvaluation {
            goal_id: goal_id.clone(),
            is_obsolete: true,
            reasons: vec![ObsolescenceReason::UserDeprioritized],
            recommendation: ObsolescenceAction::Archive,
        };
        assert!(archive_eval.needs_action());

        let migrate_eval = ObsolescenceEvaluation {
            goal_id: goal_id.clone(),
            is_obsolete: true,
            reasons: vec![],
            recommendation: ObsolescenceAction::MigrateChildren {
                target: GoalId::new(),
            },
        };
        assert!(migrate_eval.needs_action());
    }

    // AdjustmentReason tests
    #[test]
    fn test_adjustment_reason_equality() {
        assert_eq!(
            AdjustmentReason::HighRetrievalActivity,
            AdjustmentReason::HighRetrievalActivity
        );
        assert_ne!(
            AdjustmentReason::HighRetrievalActivity,
            AdjustmentReason::LowActivity
        );
        assert_eq!(
            AdjustmentReason::UserFeedback { magnitude: 0.5 },
            AdjustmentReason::UserFeedback { magnitude: 0.5 }
        );
        assert_ne!(
            AdjustmentReason::UserFeedback { magnitude: 0.5 },
            AdjustmentReason::UserFeedback { magnitude: 0.8 }
        );
    }

    #[test]
    fn test_adjustment_reason_serialization() {
        let reasons = [
            AdjustmentReason::HighRetrievalActivity,
            AdjustmentReason::LowActivity,
            AdjustmentReason::UserFeedback { magnitude: 0.7 },
            AdjustmentReason::ChildGoalPromotion,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).expect("serialize");
            let deserialized: AdjustmentReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, reason);
        }
    }

    // WeightAdjustment tests
    #[test]
    fn test_weight_adjustment_delta() {
        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.7,
            reason: AdjustmentReason::HighRetrievalActivity,
        };
        assert!((adjustment.delta() - 0.2).abs() < f32::EPSILON);

        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.8,
            new_weight: 0.6,
            reason: AdjustmentReason::LowActivity,
        };
        assert!((adjustment.delta() - (-0.2)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_weight_adjustment_is_significant() {
        // Significant positive adjustment
        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.6,
            reason: AdjustmentReason::HighRetrievalActivity,
        };
        assert!(adjustment.is_significant());

        // Significant negative adjustment
        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.4,
            reason: AdjustmentReason::LowActivity,
        };
        assert!(adjustment.is_significant());

        // Exactly at threshold (0.05) - should be significant
        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.55,
            reason: AdjustmentReason::HighRetrievalActivity,
        };
        assert!(adjustment.is_significant());

        // Just below threshold - not significant
        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.54,
            reason: AdjustmentReason::HighRetrievalActivity,
        };
        assert!(!adjustment.is_significant());

        // No change - not significant
        let adjustment = WeightAdjustment {
            goal_id: GoalId::new(),
            old_weight: 0.5,
            new_weight: 0.5,
            reason: AdjustmentReason::HighRetrievalActivity,
        };
        assert!(!adjustment.is_significant());
    }

    #[test]
    fn test_weight_adjustment_various_reasons() {
        let reasons = [
            AdjustmentReason::HighRetrievalActivity,
            AdjustmentReason::LowActivity,
            AdjustmentReason::UserFeedback { magnitude: 0.5 },
            AdjustmentReason::ChildGoalPromotion,
        ];

        for reason in reasons {
            let adjustment = WeightAdjustment {
                goal_id: GoalId::new(),
                old_weight: 0.5,
                new_weight: 0.6,
                reason,
            };
            assert!((adjustment.delta() - 0.1).abs() < f32::EPSILON);
            assert!(adjustment.is_significant());
        }
    }
}
