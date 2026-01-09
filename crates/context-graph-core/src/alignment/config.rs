//! Alignment computation configuration.
//!
//! Provides configuration options for the GoalAlignmentCalculator.
//!
//! # Performance Budget
//! From constitution.yaml: alignment computation must be <5ms.

use crate::purpose::GoalHierarchy;

use super::misalignment::MisalignmentThresholds;
use super::score::LevelWeights;

/// Configuration for alignment computation.
///
/// # Constraints
/// - `timeout_ms`: Maximum computation time (default: 5ms per constitution.yaml)
/// - `level_weights`: Must sum to 1.0
/// - `detect_patterns`: Enable/disable pattern detection (slight overhead)
///
/// Note: This struct is not Serialize/Deserialize because GoalHierarchy
/// does not implement these traits.
#[derive(Debug, Clone)]
pub struct AlignmentConfig {
    /// Goal hierarchy to align against.
    ///
    /// Must contain a North Star goal for computation to succeed.
    pub hierarchy: GoalHierarchy,

    /// Level-based weights for composite score.
    pub level_weights: LevelWeights,

    /// Thresholds for misalignment pattern detection.
    pub misalignment_thresholds: MisalignmentThresholds,

    /// Whether to detect misalignment patterns.
    ///
    /// Adds ~0.1ms overhead but provides actionable insights.
    pub detect_patterns: bool,

    /// Maximum computation time in milliseconds.
    ///
    /// From constitution.yaml: <5ms latency budget.
    /// Default: 5
    pub timeout_ms: u64,

    /// Whether to include per-embedder breakdown in results.
    ///
    /// Useful for debugging but increases output size.
    pub include_embedder_breakdown: bool,

    /// Minimum alignment threshold for relevance.
    ///
    /// Alignments below this value are treated as 0.
    /// Default: 0.0 (no minimum)
    pub min_alignment: f32,

    /// Whether to validate hierarchy before computation.
    ///
    /// Recommended for production to catch configuration errors early.
    pub validate_hierarchy: bool,
}

impl Default for AlignmentConfig {
    fn default() -> Self {
        Self {
            hierarchy: GoalHierarchy::new(),
            level_weights: LevelWeights::default(),
            misalignment_thresholds: MisalignmentThresholds::default(),
            detect_patterns: true,
            timeout_ms: 5,
            include_embedder_breakdown: false,
            min_alignment: 0.0,
            validate_hierarchy: true,
        }
    }
}

impl AlignmentConfig {
    /// Create a new config with the given hierarchy.
    pub fn with_hierarchy(hierarchy: GoalHierarchy) -> Self {
        Self {
            hierarchy,
            ..Default::default()
        }
    }

    /// Set level weights.
    ///
    /// # Panics
    /// Panics if weights don't sum to 1.0.
    pub fn with_weights(mut self, weights: LevelWeights) -> Self {
        weights.validate().expect("LevelWeights must sum to 1.0");
        self.level_weights = weights;
        self
    }

    /// Set misalignment thresholds.
    pub fn with_misalignment_thresholds(mut self, thresholds: MisalignmentThresholds) -> Self {
        self.misalignment_thresholds = thresholds;
        self
    }

    /// Enable or disable pattern detection.
    pub fn with_pattern_detection(mut self, enabled: bool) -> Self {
        self.detect_patterns = enabled;
        self
    }

    /// Set timeout in milliseconds.
    pub fn with_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Enable per-embedder breakdown in results.
    pub fn with_embedder_breakdown(mut self, enabled: bool) -> Self {
        self.include_embedder_breakdown = enabled;
        self
    }

    /// Set minimum alignment threshold.
    pub fn with_min_alignment(mut self, min: f32) -> Self {
        self.min_alignment = min.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable hierarchy validation.
    pub fn with_validation(mut self, enabled: bool) -> Self {
        self.validate_hierarchy = enabled;
        self
    }

    /// Validate configuration.
    ///
    /// # Errors
    /// Returns error if:
    /// - Level weights don't sum to 1.0
    /// - Hierarchy validation is enabled and hierarchy is invalid
    /// - Hierarchy has no North Star
    pub fn validate(&self) -> Result<(), String> {
        // Validate weights
        self.level_weights.validate().map_err(|e| e.to_string())?;

        // Validate hierarchy if enabled
        if self.validate_hierarchy {
            self.hierarchy
                .validate()
                .map_err(|e| format!("Hierarchy validation failed: {}", e))?;
        }

        // Check North Star exists
        if !self.hierarchy.has_north_star() && !self.hierarchy.is_empty() {
            return Err("Hierarchy has goals but no North Star".to_string());
        }

        Ok(())
    }

    /// Create a fast config (pattern detection disabled, no validation).
    ///
    /// Use for high-throughput scenarios where validation is done elsewhere.
    pub fn fast(hierarchy: GoalHierarchy) -> Self {
        Self {
            hierarchy,
            detect_patterns: false,
            include_embedder_breakdown: false,
            validate_hierarchy: false,
            ..Default::default()
        }
    }

    /// Create a debug config (all features enabled).
    pub fn debug(hierarchy: GoalHierarchy) -> Self {
        Self {
            hierarchy,
            detect_patterns: true,
            include_embedder_breakdown: true,
            validate_hierarchy: true,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::purpose::{GoalDiscoveryMetadata, GoalLevel, GoalNode};
    use crate::types::fingerprint::SemanticFingerprint;

    fn test_discovery() -> GoalDiscoveryMetadata {
        GoalDiscoveryMetadata::bootstrap()
    }

    fn create_test_hierarchy() -> GoalHierarchy {
        let mut hierarchy = GoalHierarchy::new();

        let ns = GoalNode::autonomous_goal(
            "North Star".into(),
            GoalLevel::NorthStar,
            SemanticFingerprint::zeroed(),
            test_discovery(),
        )
        .expect("Failed to create North Star");
        let ns_id = ns.id;
        hierarchy.add_goal(ns).unwrap();

        let s1 = GoalNode::child_goal(
            "Strategic 1".into(),
            GoalLevel::Strategic,
            ns_id,
            SemanticFingerprint::zeroed(),
            test_discovery(),
        )
        .expect("Failed to create Strategic goal");
        hierarchy.add_goal(s1).unwrap();

        hierarchy
    }

    #[test]
    fn test_config_default() {
        let config = AlignmentConfig::default();

        assert!(config.hierarchy.is_empty());
        assert!(config.detect_patterns);
        assert_eq!(config.timeout_ms, 5);
        assert!(!config.include_embedder_breakdown);
        assert_eq!(config.min_alignment, 0.0);
        assert!(config.validate_hierarchy);

        println!("[VERIFIED] AlignmentConfig::default has correct values");
        println!(
            "  - timeout_ms: {} (constitution.yaml: <5ms)",
            config.timeout_ms
        );
    }

    #[test]
    fn test_config_with_hierarchy() {
        let hierarchy = create_test_hierarchy();
        let config = AlignmentConfig::with_hierarchy(hierarchy);

        assert!(config.hierarchy.has_north_star());
        assert_eq!(config.hierarchy.len(), 2);

        println!("[VERIFIED] AlignmentConfig::with_hierarchy sets hierarchy correctly");
    }

    #[test]
    fn test_config_builder_pattern() {
        let hierarchy = create_test_hierarchy();
        let config = AlignmentConfig::with_hierarchy(hierarchy)
            .with_pattern_detection(false)
            .with_timeout_ms(10)
            .with_embedder_breakdown(true)
            .with_min_alignment(0.5)
            .with_validation(false);

        assert!(!config.detect_patterns);
        assert_eq!(config.timeout_ms, 10);
        assert!(config.include_embedder_breakdown);
        assert_eq!(config.min_alignment, 0.5);
        assert!(!config.validate_hierarchy);

        println!("[VERIFIED] Builder pattern works correctly");
    }

    #[test]
    fn test_config_min_alignment_clamping() {
        let config = AlignmentConfig::default().with_min_alignment(2.0);
        assert_eq!(config.min_alignment, 1.0);

        let config = AlignmentConfig::default().with_min_alignment(-0.5);
        assert_eq!(config.min_alignment, 0.0);

        println!("[VERIFIED] min_alignment is clamped to [0.0, 1.0]");
    }

    #[test]
    fn test_config_validate_success() {
        let hierarchy = create_test_hierarchy();
        let config = AlignmentConfig::with_hierarchy(hierarchy);

        assert!(config.validate().is_ok());
        println!("[VERIFIED] Valid config passes validation");
    }

    #[test]
    fn test_config_validate_empty_hierarchy() {
        let config = AlignmentConfig::default();

        // Empty hierarchy should be OK (nothing to validate)
        assert!(config.validate().is_ok());
        println!("[VERIFIED] Empty hierarchy is valid");
    }

    #[test]
    fn test_config_validate_invalid_weights() {
        let config = AlignmentConfig::default();
        let mut invalid_config = config.clone();
        invalid_config.level_weights = LevelWeights {
            north_star: 0.5,
            strategic: 0.5,
            tactical: 0.5,
            immediate: 0.5,
        };

        let result = invalid_config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("1.0"));

        println!("[VERIFIED] Invalid weights fail validation");
    }

    #[test]
    fn test_config_fast() {
        let hierarchy = create_test_hierarchy();
        let config = AlignmentConfig::fast(hierarchy);

        assert!(!config.detect_patterns);
        assert!(!config.include_embedder_breakdown);
        assert!(!config.validate_hierarchy);

        println!("[VERIFIED] AlignmentConfig::fast disables overhead features");
    }

    #[test]
    fn test_config_debug() {
        let hierarchy = create_test_hierarchy();
        let config = AlignmentConfig::debug(hierarchy);

        assert!(config.detect_patterns);
        assert!(config.include_embedder_breakdown);
        assert!(config.validate_hierarchy);

        println!("[VERIFIED] AlignmentConfig::debug enables all features");
    }
}
