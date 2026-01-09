//! Bootstrap configuration types for autonomous North Star initialization.
//!
//! This module defines the types needed to bootstrap a North Star goal from
//! project documents when no explicit configuration exists.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for goals
#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct GoalId(pub Uuid);

impl GoalId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for GoalId {
    fn default() -> Self {
        Self::new()
    }
}

/// Section weights for document bootstrap
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SectionWeights {
    /// Weight for first/last paragraphs
    pub position_weight: f32, // default: 1.5
    /// Weight based on semantic density
    pub density_weight: f32, // default: 1.2
    /// Apply IDF weighting
    pub apply_idf: bool, // default: true
}

impl Default for SectionWeights {
    fn default() -> Self {
        Self {
            position_weight: 1.5,
            density_weight: 1.2,
            apply_idf: true,
        }
    }
}

/// Configuration for autonomous bootstrap
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BootstrapConfig {
    /// Enable auto-initialization when no North Star exists
    pub auto_init: bool,

    /// Fallback description if no documents found
    pub fallback_description: String,

    /// File patterns to scan for context
    pub source_patterns: Vec<String>,

    /// Weighting for document sections
    pub section_weights: SectionWeights,

    /// Minimum confidence threshold for accepting a goal (0.0 to 1.0)
    pub min_confidence: f32,

    /// Maximum number of source documents to consider
    pub max_sources: usize,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            auto_init: true,
            fallback_description: "General purpose knowledge acquisition".into(),
            source_patterns: vec![
                "*.md".into(),
                "constitution.yaml".into(),
                "package.json".into(),
                "README*".into(),
            ],
            section_weights: SectionWeights::default(),
            min_confidence: 0.3,
            max_sources: 5,
        }
    }
}

/// Result of bootstrap attempt
#[derive(Clone, Debug)]
pub struct BootstrapResult {
    pub success: bool,
    pub north_star_id: Option<GoalId>,
    pub description: String,
    pub source_documents: Vec<String>,
    pub chunk_count: usize,
    pub lineage_event_id: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_id_creation() {
        let id1 = GoalId::new();
        let id2 = GoalId::new();
        assert_ne!(id1, id2, "GoalIds should be unique");
    }

    #[test]
    fn test_goal_id_default() {
        let id = GoalId::default();
        assert!(!id.0.is_nil(), "Default GoalId should not be nil");
    }

    #[test]
    fn test_goal_id_serialization() {
        let id = GoalId::new();
        let serialized = serde_json::to_string(&id).expect("serialize");
        let deserialized: GoalId = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_section_weights_default() {
        let weights = SectionWeights::default();
        assert!((weights.position_weight - 1.5).abs() < f32::EPSILON);
        assert!((weights.density_weight - 1.2).abs() < f32::EPSILON);
        assert!(weights.apply_idf);
    }

    #[test]
    fn test_section_weights_serialization() {
        let weights = SectionWeights {
            position_weight: 2.0,
            density_weight: 1.5,
            apply_idf: false,
        };
        let serialized = serde_json::to_string(&weights).expect("serialize");
        let deserialized: SectionWeights = serde_json::from_str(&serialized).expect("deserialize");
        assert!((deserialized.position_weight - 2.0).abs() < f32::EPSILON);
        assert!((deserialized.density_weight - 1.5).abs() < f32::EPSILON);
        assert!(!deserialized.apply_idf);
    }

    #[test]
    fn test_bootstrap_config_default() {
        let config = BootstrapConfig::default();
        assert!(config.auto_init);
        assert_eq!(
            config.fallback_description,
            "General purpose knowledge acquisition"
        );
        assert!(config.source_patterns.contains(&"*.md".to_string()));
        assert!(config
            .source_patterns
            .contains(&"constitution.yaml".to_string()));
        assert!(config.source_patterns.contains(&"package.json".to_string()));
        assert!(config.source_patterns.contains(&"README*".to_string()));
    }

    #[test]
    fn test_bootstrap_config_serialization() {
        let config = BootstrapConfig::default();
        let serialized = serde_json::to_string(&config).expect("serialize");
        let deserialized: BootstrapConfig = serde_json::from_str(&serialized).expect("deserialize");
        assert_eq!(config.auto_init, deserialized.auto_init);
        assert_eq!(
            config.fallback_description,
            deserialized.fallback_description
        );
        assert_eq!(config.source_patterns, deserialized.source_patterns);
    }

    #[test]
    fn test_bootstrap_result_creation() {
        let result = BootstrapResult {
            success: true,
            north_star_id: Some(GoalId::new()),
            description: "Test bootstrap".into(),
            source_documents: vec!["README.md".into(), "CONSTITUTION.yaml".into()],
            chunk_count: 42,
            lineage_event_id: "evt_001".into(),
        };
        assert!(result.success);
        assert!(result.north_star_id.is_some());
        assert_eq!(result.description, "Test bootstrap");
        assert_eq!(result.source_documents.len(), 2);
        assert_eq!(result.chunk_count, 42);
    }

    #[test]
    fn test_bootstrap_result_failure() {
        let result = BootstrapResult {
            success: false,
            north_star_id: None,
            description: "No documents found".into(),
            source_documents: vec![],
            chunk_count: 0,
            lineage_event_id: "evt_fail_001".into(),
        };
        assert!(!result.success);
        assert!(result.north_star_id.is_none());
        assert!(result.source_documents.is_empty());
        assert_eq!(result.chunk_count, 0);
    }

    #[test]
    fn test_goal_id_hash() {
        use std::collections::HashSet;
        let id1 = GoalId::new();
        let id2 = id1.clone();
        let id3 = GoalId::new();

        let mut set = HashSet::new();
        set.insert(id1.clone());
        assert!(set.contains(&id2), "Clone should hash to same value");
        assert!(!set.contains(&id3), "Different GoalId should not be found");
    }
}
