//! Memory pruning and consolidation types for curation
//!
//! This module defines types for memory curation operations including
//! pruning of low-value memories and consolidation of similar memories.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for memories
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct MemoryId(pub Uuid);

impl MemoryId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MemoryId {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory pruning configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PruningConfig {
    pub enabled: bool,

    /// Minimum age before pruning eligible (days)
    pub min_age_days: u32, // default: 30

    /// Alignment below which pruning is considered
    pub min_alignment: f32, // default: 0.40

    /// Preserve memories with many connections
    pub preserve_connected: bool, // default: true

    /// Minimum connections to preserve
    pub min_connections: u32, // default: 3
}

impl Default for PruningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_age_days: 30,
            min_alignment: 0.40,
            preserve_connected: true,
            min_connections: 3,
        }
    }
}

/// Memory consolidation configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsolidationConfig {
    pub enabled: bool,

    /// Similarity threshold for merging
    pub similarity_threshold: f32, // default: 0.92

    /// Maximum merges per day
    pub max_daily_merges: u32, // default: 50

    /// Alignment difference threshold
    pub theta_diff_threshold: f32, // default: 0.05
}

impl Default for ConsolidationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            similarity_threshold: 0.92,
            max_daily_merges: 50,
            theta_diff_threshold: 0.05,
        }
    }
}

/// Memory state for curation
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryCurationState {
    #[default]
    Active,
    Dormant {
        since: DateTime<Utc>,
    },
    Archived {
        since: DateTime<Utc>,
    },
    PendingDeletion {
        scheduled: DateTime<Utc>,
    },
}

/// Reason for pruning a memory
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PruneReason {
    LowAlignment,
    NeverRetrieved,
    Isolated,
    Stale,
}

/// Pruning candidate
#[derive(Clone, Debug)]
pub struct PruningCandidate {
    pub memory_id: MemoryId,
    pub alignment: f32,
    pub age_days: u32,
    pub last_retrieved: Option<DateTime<Utc>>,
    pub connection_count: u32,
    pub state: MemoryCurationState,
    pub prune_reason: PruneReason,
}

impl PruningCandidate {
    /// Check if this candidate should be preserved based on config
    pub fn should_preserve(&self, config: &PruningConfig) -> bool {
        if config.preserve_connected && self.connection_count >= config.min_connections {
            return true;
        }
        false
    }

    /// Get a human-readable explanation for the prune reason
    pub fn reason_description(&self) -> &'static str {
        match self.prune_reason {
            PruneReason::LowAlignment => "Alignment below threshold",
            PruneReason::NeverRetrieved => "Never retrieved since creation",
            PruneReason::Isolated => "No connections to other memories",
            PruneReason::Stale => "Not accessed for extended period",
        }
    }
}

/// Consolidation candidate pair
#[derive(Clone, Debug)]
pub struct ConsolidationCandidate {
    pub memory_a: MemoryId,
    pub memory_b: MemoryId,
    pub similarity: f32,
    pub theta_diff: f32,
    pub can_merge: bool,
}

impl ConsolidationCandidate {
    /// Create a new consolidation candidate
    pub fn new(
        memory_a: MemoryId,
        memory_b: MemoryId,
        similarity: f32,
        theta_diff: f32,
        config: &ConsolidationConfig,
    ) -> Self {
        let can_merge =
            similarity >= config.similarity_threshold && theta_diff <= config.theta_diff_threshold;
        Self {
            memory_a,
            memory_b,
            similarity,
            theta_diff,
            can_merge,
        }
    }

    /// Check if this pair can be merged
    pub fn is_mergeable(&self) -> bool {
        self.can_merge
    }
}

/// Report from pruning operation
#[derive(Clone, Debug, Default)]
pub struct PruningReport {
    pub candidates_found: usize,
    pub marked_dormant: usize,
    pub archived: usize,
    pub deleted: usize,
    pub preserved: usize,
}

/// Report from consolidation operation
#[derive(Clone, Debug, Default)]
pub struct ConsolidationReport {
    pub candidates_found: usize,
    pub merged: usize,
    pub skipped: usize,
    pub daily_limit_reached: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    // MemoryId tests
    #[test]
    fn test_memory_id_new() {
        let id1 = MemoryId::new();
        let id2 = MemoryId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_memory_id_default() {
        let id1 = MemoryId::default();
        let id2 = MemoryId::default();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_memory_id_equality() {
        let uuid = Uuid::new_v4();
        let id1 = MemoryId(uuid);
        let id2 = MemoryId(uuid);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_memory_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        let id1 = MemoryId::new();
        let id2 = id1.clone();
        set.insert(id1);
        assert!(set.contains(&id2));
    }

    #[test]
    fn test_memory_id_serialization() {
        let id = MemoryId::new();
        let json = serde_json::to_string(&id).expect("serialize");
        let deserialized: MemoryId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, deserialized);
    }

    // PruningConfig tests
    #[test]
    fn test_pruning_config_default() {
        let config = PruningConfig::default();
        assert!(config.enabled);
        assert_eq!(config.min_age_days, 30);
        assert!((config.min_alignment - 0.40).abs() < f32::EPSILON);
        assert!(config.preserve_connected);
        assert_eq!(config.min_connections, 3);
    }

    #[test]
    fn test_pruning_config_serialization() {
        let config = PruningConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: PruningConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.min_age_days, config.min_age_days);
        assert!((deserialized.min_alignment - config.min_alignment).abs() < f32::EPSILON);
        assert_eq!(deserialized.preserve_connected, config.preserve_connected);
        assert_eq!(deserialized.min_connections, config.min_connections);
    }

    #[test]
    fn test_pruning_config_custom() {
        let config = PruningConfig {
            enabled: false,
            min_age_days: 60,
            min_alignment: 0.50,
            preserve_connected: false,
            min_connections: 5,
        };
        assert!(!config.enabled);
        assert_eq!(config.min_age_days, 60);
        assert!((config.min_alignment - 0.50).abs() < f32::EPSILON);
        assert!(!config.preserve_connected);
        assert_eq!(config.min_connections, 5);
    }

    // ConsolidationConfig tests
    #[test]
    fn test_consolidation_config_default() {
        let config = ConsolidationConfig::default();
        assert!(config.enabled);
        assert!((config.similarity_threshold - 0.92).abs() < f32::EPSILON);
        assert_eq!(config.max_daily_merges, 50);
        assert!((config.theta_diff_threshold - 0.05).abs() < f32::EPSILON);
    }

    #[test]
    fn test_consolidation_config_serialization() {
        let config = ConsolidationConfig::default();
        let json = serde_json::to_string(&config).expect("serialize");
        let deserialized: ConsolidationConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.enabled, config.enabled);
        assert!(
            (deserialized.similarity_threshold - config.similarity_threshold).abs() < f32::EPSILON
        );
        assert_eq!(deserialized.max_daily_merges, config.max_daily_merges);
        assert!(
            (deserialized.theta_diff_threshold - config.theta_diff_threshold).abs() < f32::EPSILON
        );
    }

    #[test]
    fn test_consolidation_config_custom() {
        let config = ConsolidationConfig {
            enabled: false,
            similarity_threshold: 0.95,
            max_daily_merges: 100,
            theta_diff_threshold: 0.03,
        };
        assert!(!config.enabled);
        assert!((config.similarity_threshold - 0.95).abs() < f32::EPSILON);
        assert_eq!(config.max_daily_merges, 100);
        assert!((config.theta_diff_threshold - 0.03).abs() < f32::EPSILON);
    }

    // MemoryCurationState tests
    #[test]
    fn test_memory_curation_state_default() {
        let state = MemoryCurationState::default();
        assert_eq!(state, MemoryCurationState::Active);
    }

    #[test]
    fn test_memory_curation_state_variants() {
        let now = Utc::now();

        let active = MemoryCurationState::Active;
        let dormant = MemoryCurationState::Dormant { since: now };
        let archived = MemoryCurationState::Archived { since: now };
        let pending = MemoryCurationState::PendingDeletion { scheduled: now };

        assert_eq!(active, MemoryCurationState::Active);
        assert_ne!(active, dormant);
        assert_ne!(dormant, archived);
        assert_ne!(archived, pending);
    }

    #[test]
    fn test_memory_curation_state_serialization() {
        let now = Utc::now();

        let active = MemoryCurationState::Active;
        let json = serde_json::to_string(&active).expect("serialize");
        let deserialized: MemoryCurationState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, active);

        let dormant = MemoryCurationState::Dormant { since: now };
        let json = serde_json::to_string(&dormant).expect("serialize");
        let deserialized: MemoryCurationState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, dormant);

        let archived = MemoryCurationState::Archived { since: now };
        let json = serde_json::to_string(&archived).expect("serialize");
        let deserialized: MemoryCurationState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, archived);

        let pending = MemoryCurationState::PendingDeletion { scheduled: now };
        let json = serde_json::to_string(&pending).expect("serialize");
        let deserialized: MemoryCurationState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized, pending);
    }

    // PruneReason tests
    #[test]
    fn test_prune_reason_equality() {
        assert_eq!(PruneReason::LowAlignment, PruneReason::LowAlignment);
        assert_ne!(PruneReason::LowAlignment, PruneReason::NeverRetrieved);
        assert_ne!(PruneReason::NeverRetrieved, PruneReason::Isolated);
        assert_ne!(PruneReason::Isolated, PruneReason::Stale);
    }

    #[test]
    fn test_prune_reason_serialization() {
        let reasons = [
            PruneReason::LowAlignment,
            PruneReason::NeverRetrieved,
            PruneReason::Isolated,
            PruneReason::Stale,
        ];

        for reason in reasons {
            let json = serde_json::to_string(&reason).expect("serialize");
            let deserialized: PruneReason = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(deserialized, reason);
        }
    }

    // PruningCandidate tests
    #[test]
    fn test_pruning_candidate_should_preserve_connected() {
        let config = PruningConfig {
            preserve_connected: true,
            min_connections: 3,
            ..Default::default()
        };

        let candidate_preserved = PruningCandidate {
            memory_id: MemoryId::new(),
            alignment: 0.30,
            age_days: 60,
            last_retrieved: None,
            connection_count: 5,
            state: MemoryCurationState::Active,
            prune_reason: PruneReason::LowAlignment,
        };
        assert!(candidate_preserved.should_preserve(&config));

        let candidate_not_preserved = PruningCandidate {
            memory_id: MemoryId::new(),
            alignment: 0.30,
            age_days: 60,
            last_retrieved: None,
            connection_count: 2,
            state: MemoryCurationState::Active,
            prune_reason: PruneReason::LowAlignment,
        };
        assert!(!candidate_not_preserved.should_preserve(&config));
    }

    #[test]
    fn test_pruning_candidate_should_preserve_disabled() {
        let config = PruningConfig {
            preserve_connected: false,
            min_connections: 3,
            ..Default::default()
        };

        let candidate = PruningCandidate {
            memory_id: MemoryId::new(),
            alignment: 0.30,
            age_days: 60,
            last_retrieved: None,
            connection_count: 10,
            state: MemoryCurationState::Active,
            prune_reason: PruneReason::LowAlignment,
        };
        assert!(!candidate.should_preserve(&config));
    }

    #[test]
    fn test_pruning_candidate_reason_description() {
        let base = PruningCandidate {
            memory_id: MemoryId::new(),
            alignment: 0.30,
            age_days: 60,
            last_retrieved: None,
            connection_count: 0,
            state: MemoryCurationState::Active,
            prune_reason: PruneReason::LowAlignment,
        };

        let mut candidate = base.clone();
        candidate.prune_reason = PruneReason::LowAlignment;
        assert_eq!(candidate.reason_description(), "Alignment below threshold");

        candidate.prune_reason = PruneReason::NeverRetrieved;
        assert_eq!(
            candidate.reason_description(),
            "Never retrieved since creation"
        );

        candidate.prune_reason = PruneReason::Isolated;
        assert_eq!(
            candidate.reason_description(),
            "No connections to other memories"
        );

        candidate.prune_reason = PruneReason::Stale;
        assert_eq!(
            candidate.reason_description(),
            "Not accessed for extended period"
        );
    }

    // ConsolidationCandidate tests
    #[test]
    fn test_consolidation_candidate_new_mergeable() {
        let config = ConsolidationConfig::default();
        let candidate = ConsolidationCandidate::new(
            MemoryId::new(),
            MemoryId::new(),
            0.95, // > 0.92 threshold
            0.03, // < 0.05 threshold
            &config,
        );
        assert!(candidate.can_merge);
        assert!(candidate.is_mergeable());
    }

    #[test]
    fn test_consolidation_candidate_new_low_similarity() {
        let config = ConsolidationConfig::default();
        let candidate = ConsolidationCandidate::new(
            MemoryId::new(),
            MemoryId::new(),
            0.85, // < 0.92 threshold
            0.03,
            &config,
        );
        assert!(!candidate.can_merge);
        assert!(!candidate.is_mergeable());
    }

    #[test]
    fn test_consolidation_candidate_new_high_theta_diff() {
        let config = ConsolidationConfig::default();
        let candidate = ConsolidationCandidate::new(
            MemoryId::new(),
            MemoryId::new(),
            0.95,
            0.10, // > 0.05 threshold
            &config,
        );
        assert!(!candidate.can_merge);
        assert!(!candidate.is_mergeable());
    }

    #[test]
    fn test_consolidation_candidate_boundary_conditions() {
        let config = ConsolidationConfig::default();

        // Exactly at thresholds - should be mergeable
        let candidate = ConsolidationCandidate::new(
            MemoryId::new(),
            MemoryId::new(),
            0.92, // exactly at threshold
            0.05, // exactly at threshold
            &config,
        );
        assert!(candidate.can_merge);

        // Just below similarity threshold
        let candidate =
            ConsolidationCandidate::new(MemoryId::new(), MemoryId::new(), 0.919, 0.05, &config);
        assert!(!candidate.can_merge);

        // Just above theta diff threshold
        let candidate =
            ConsolidationCandidate::new(MemoryId::new(), MemoryId::new(), 0.92, 0.051, &config);
        assert!(!candidate.can_merge);
    }

    #[test]
    fn test_consolidation_candidate_custom_config() {
        let config = ConsolidationConfig {
            enabled: true,
            similarity_threshold: 0.80,
            max_daily_merges: 100,
            theta_diff_threshold: 0.10,
        };

        let candidate =
            ConsolidationCandidate::new(MemoryId::new(), MemoryId::new(), 0.85, 0.08, &config);
        assert!(candidate.can_merge);
    }

    // PruningReport tests
    #[test]
    fn test_pruning_report_default() {
        let report = PruningReport::default();
        assert_eq!(report.candidates_found, 0);
        assert_eq!(report.marked_dormant, 0);
        assert_eq!(report.archived, 0);
        assert_eq!(report.deleted, 0);
        assert_eq!(report.preserved, 0);
    }

    #[test]
    fn test_pruning_report_custom() {
        let report = PruningReport {
            candidates_found: 100,
            marked_dormant: 20,
            archived: 30,
            deleted: 40,
            preserved: 10,
        };
        assert_eq!(report.candidates_found, 100);
        assert_eq!(report.marked_dormant, 20);
        assert_eq!(report.archived, 30);
        assert_eq!(report.deleted, 40);
        assert_eq!(report.preserved, 10);
    }

    // ConsolidationReport tests
    #[test]
    fn test_consolidation_report_default() {
        let report = ConsolidationReport::default();
        assert_eq!(report.candidates_found, 0);
        assert_eq!(report.merged, 0);
        assert_eq!(report.skipped, 0);
        assert!(!report.daily_limit_reached);
    }

    #[test]
    fn test_consolidation_report_custom() {
        let report = ConsolidationReport {
            candidates_found: 50,
            merged: 30,
            skipped: 20,
            daily_limit_reached: true,
        };
        assert_eq!(report.candidates_found, 50);
        assert_eq!(report.merged, 30);
        assert_eq!(report.skipped, 20);
        assert!(report.daily_limit_reached);
    }
}
