---
id: "M05-T07"
title: "Define LifecycleConfig and StageConfig Structs"
description: |
  Implement LifecycleConfig struct for lifecycle state machine configuration.
  Fields: infancy_threshold (50), growth_threshold (500),
  infancy (StageConfig), growth (StageConfig), maturity (StageConfig).
  StageConfig: entropy_trigger, coherence_trigger, min_importance_store, consolidation_threshold.
  Infancy: entropy_trigger=0.9, coherence_trigger=0.2, min_importance=0.1, consolidation=0.3.
  Growth: entropy_trigger=0.7, coherence_trigger=0.4, min_importance=0.3, consolidation=0.5.
  Maturity: entropy_trigger=0.6, coherence_trigger=0.5, min_importance=0.4, consolidation=0.6.
layer: "foundation"
status: "pending"
priority: "high"
estimated_hours: 1.5
sequence: 7
depends_on:
  - "M05-T05"
spec_refs:
  - "TECH-UTL-005 Section 10"
  - "SPEC-UTL-005 Section 8.4"
files_to_create:
  - path: "crates/context-graph-utl/src/lifecycle/config.rs"
    description: "LifecycleConfig and StageConfig structs"
files_to_modify:
  - path: "crates/context-graph-utl/src/lifecycle/mod.rs"
    description: "Add config module and re-export LifecycleConfig, StageConfig"
  - path: "crates/context-graph-utl/src/config.rs"
    description: "Re-export LifecycleConfig for UtlConfig integration"
test_file: "crates/context-graph-utl/tests/config_tests.rs"
---

## Overview

The LifecycleConfig struct provides the complete configuration for the lifecycle state machine, including transition thresholds and per-stage settings. StageConfig defines the behavioral parameters for each lifecycle stage.

## Configuration Structure

```
LifecycleConfig
├── infancy_threshold: u64 (50)      # Interactions to transition Infancy->Growth
├── growth_threshold: u64 (500)      # Interactions to transition Growth->Maturity
├── infancy: StageConfig             # Settings for Infancy stage
├── growth: StageConfig              # Settings for Growth stage
└── maturity: StageConfig            # Settings for Maturity stage

StageConfig
├── entropy_trigger: f32             # Max entropy before triggering exploration
├── coherence_trigger: f32           # Min coherence required for storage
├── min_importance_store: f32        # Min importance to store memory
└── consolidation_threshold: f32     # Threshold for consolidation trigger
```

## Implementation Requirements

### File: `crates/context-graph-utl/src/lifecycle/config.rs`

```rust
//! Lifecycle configuration for state machine behavior.
//!
//! # Configuration Overview
//!
//! The lifecycle state machine has:
//! - Transition thresholds between stages
//! - Per-stage behavioral settings
//!
//! # Stage Progression
//!
//! Infancy (0-49) -> Growth (50-499) -> Maturity (500+)
//!
//! # Constitution Reference
//!
//! - SPEC-UTL-005 Section 8.4: Lifecycle configuration
//! - TECH-UTL-005 Section 10: Configuration parameters

use serde::{Deserialize, Serialize};

use crate::lifecycle::LifecycleStage;

/// Configuration for a specific lifecycle stage.
///
/// Each stage has distinct thresholds that govern memory storage,
/// consolidation, and exploration behavior.
///
/// # Fields
///
/// - `entropy_trigger`: Maximum entropy tolerated before exploration is triggered
/// - `coherence_trigger`: Minimum coherence required before storing memory
/// - `min_importance_store`: Minimum importance score to store a memory
/// - `consolidation_threshold`: Threshold for triggering memory consolidation
///
/// # Example
///
/// ```
/// use context_graph_utl::lifecycle::StageConfig;
///
/// let config = StageConfig::infancy();
/// assert_eq!(config.entropy_trigger, 0.9);
/// assert!(config.should_store(0.15, 0.8)); // importance=0.15, entropy=0.8
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StageConfig {
    /// Maximum entropy before triggering exploration behavior.
    /// Higher values = more tolerance for surprising content.
    /// Range: [0.0, 1.0]
    pub entropy_trigger: f32,

    /// Minimum coherence required for memory storage.
    /// Higher values = more stringent coherence requirements.
    /// Range: [0.0, 1.0]
    pub coherence_trigger: f32,

    /// Minimum importance score required to store a memory.
    /// Higher values = more selective about what to store.
    /// Range: [0.0, 1.0]
    pub min_importance_store: f32,

    /// Threshold for triggering memory consolidation.
    /// When learning magnitude exceeds this, consolidation may occur.
    /// Range: [0.0, 1.0]
    pub consolidation_threshold: f32,
}

impl StageConfig {
    /// Create configuration for Infancy stage.
    ///
    /// Infancy is characterized by:
    /// - High entropy tolerance (0.9) - accept surprising content
    /// - Low coherence requirement (0.2) - store without much context
    /// - Low importance threshold (0.1) - store almost everything
    /// - Low consolidation threshold (0.3) - consolidate frequently
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::StageConfig;
    ///
    /// let config = StageConfig::infancy();
    /// assert_eq!(config.entropy_trigger, 0.9);
    /// assert_eq!(config.min_importance_store, 0.1);
    /// ```
    #[inline]
    pub fn infancy() -> Self {
        Self {
            entropy_trigger: 0.9,
            coherence_trigger: 0.2,
            min_importance_store: 0.1,
            consolidation_threshold: 0.3,
        }
    }

    /// Create configuration for Growth stage.
    ///
    /// Growth is characterized by:
    /// - Moderate entropy tolerance (0.7) - balanced exploration
    /// - Moderate coherence requirement (0.4) - some context needed
    /// - Moderate importance threshold (0.3) - selective storage
    /// - Moderate consolidation threshold (0.5) - balanced consolidation
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::StageConfig;
    ///
    /// let config = StageConfig::growth();
    /// assert_eq!(config.entropy_trigger, 0.7);
    /// assert_eq!(config.coherence_trigger, 0.4);
    /// ```
    #[inline]
    pub fn growth() -> Self {
        Self {
            entropy_trigger: 0.7,
            coherence_trigger: 0.4,
            min_importance_store: 0.3,
            consolidation_threshold: 0.5,
        }
    }

    /// Create configuration for Maturity stage.
    ///
    /// Maturity is characterized by:
    /// - Lower entropy tolerance (0.6) - prefer familiar content
    /// - Higher coherence requirement (0.5) - require good context
    /// - Higher importance threshold (0.4) - very selective storage
    /// - Higher consolidation threshold (0.6) - selective consolidation
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::StageConfig;
    ///
    /// let config = StageConfig::maturity();
    /// assert_eq!(config.entropy_trigger, 0.6);
    /// assert_eq!(config.min_importance_store, 0.4);
    /// ```
    #[inline]
    pub fn maturity() -> Self {
        Self {
            entropy_trigger: 0.6,
            coherence_trigger: 0.5,
            min_importance_store: 0.4,
            consolidation_threshold: 0.6,
        }
    }

    /// Create configuration for a specific lifecycle stage.
    ///
    /// # Arguments
    ///
    /// * `stage` - The lifecycle stage to get configuration for
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::{StageConfig, LifecycleStage};
    ///
    /// let config = StageConfig::for_stage(LifecycleStage::Growth);
    /// assert_eq!(config.entropy_trigger, 0.7);
    /// ```
    #[inline]
    pub fn for_stage(stage: LifecycleStage) -> Self {
        match stage {
            LifecycleStage::Infancy => Self::infancy(),
            LifecycleStage::Growth => Self::growth(),
            LifecycleStage::Maturity => Self::maturity(),
        }
    }

    /// Check if a memory should be stored based on importance and entropy.
    ///
    /// # Arguments
    ///
    /// * `importance` - Importance score of the memory [0, 1]
    /// * `entropy` - Entropy/surprise level [0, 1]
    ///
    /// # Returns
    ///
    /// `true` if memory should be stored
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::StageConfig;
    ///
    /// let config = StageConfig::infancy();
    ///
    /// // Low importance, acceptable entropy - should store in infancy
    /// assert!(config.should_store(0.15, 0.8));
    ///
    /// // Too low importance
    /// assert!(!config.should_store(0.05, 0.8));
    /// ```
    #[inline]
    pub fn should_store(&self, importance: f32, entropy: f32) -> bool {
        importance >= self.min_importance_store && entropy <= self.entropy_trigger
    }

    /// Check if consolidation should be triggered.
    ///
    /// # Arguments
    ///
    /// * `learning_magnitude` - Current learning magnitude [0, 1]
    /// * `coherence` - Current coherence level [0, 1]
    ///
    /// # Returns
    ///
    /// `true` if consolidation should be triggered
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::StageConfig;
    ///
    /// let config = StageConfig::growth();
    ///
    /// // High magnitude, high coherence - should consolidate
    /// assert!(config.should_consolidate(0.6, 0.5));
    ///
    /// // Low magnitude
    /// assert!(!config.should_consolidate(0.3, 0.5));
    /// ```
    #[inline]
    pub fn should_consolidate(&self, learning_magnitude: f32, coherence: f32) -> bool {
        learning_magnitude >= self.consolidation_threshold && coherence >= self.coherence_trigger
    }

    /// Validate that all values are in valid ranges.
    ///
    /// # Returns
    ///
    /// `true` if all values are in [0, 1]
    pub fn is_valid(&self) -> bool {
        (0.0..=1.0).contains(&self.entropy_trigger)
            && (0.0..=1.0).contains(&self.coherence_trigger)
            && (0.0..=1.0).contains(&self.min_importance_store)
            && (0.0..=1.0).contains(&self.consolidation_threshold)
    }
}

impl Default for StageConfig {
    /// Default is Growth stage configuration.
    fn default() -> Self {
        Self::growth()
    }
}

/// Complete lifecycle configuration including transition thresholds.
///
/// # Configuration Structure
///
/// - `infancy_threshold`: Interactions to transition from Infancy to Growth (default: 50)
/// - `growth_threshold`: Interactions to transition from Growth to Maturity (default: 500)
/// - `infancy`: StageConfig for Infancy stage
/// - `growth`: StageConfig for Growth stage
/// - `maturity`: StageConfig for Maturity stage
///
/// # Example
///
/// ```
/// use context_graph_utl::lifecycle::{LifecycleConfig, LifecycleStage};
///
/// let config = LifecycleConfig::default();
/// assert_eq!(config.infancy_threshold, 50);
///
/// // Get stage for interaction count
/// assert_eq!(config.stage_for_count(25), LifecycleStage::Infancy);
/// assert_eq!(config.stage_for_count(100), LifecycleStage::Growth);
/// assert_eq!(config.stage_for_count(1000), LifecycleStage::Maturity);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LifecycleConfig {
    /// Interaction count threshold to transition from Infancy to Growth.
    /// Default: 50 interactions
    pub infancy_threshold: u64,

    /// Interaction count threshold to transition from Growth to Maturity.
    /// Default: 500 interactions
    pub growth_threshold: u64,

    /// Configuration for Infancy stage.
    pub infancy: StageConfig,

    /// Configuration for Growth stage.
    pub growth: StageConfig,

    /// Configuration for Maturity stage.
    pub maturity: StageConfig,
}

impl LifecycleConfig {
    /// Create a new lifecycle configuration with custom thresholds.
    ///
    /// # Arguments
    ///
    /// * `infancy_threshold` - Interactions for Infancy->Growth transition
    /// * `growth_threshold` - Interactions for Growth->Maturity transition
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleConfig;
    ///
    /// // Custom thresholds for faster progression
    /// let config = LifecycleConfig::with_thresholds(25, 100);
    /// assert_eq!(config.infancy_threshold, 25);
    /// ```
    pub fn with_thresholds(infancy_threshold: u64, growth_threshold: u64) -> Self {
        Self {
            infancy_threshold,
            growth_threshold,
            infancy: StageConfig::infancy(),
            growth: StageConfig::growth(),
            maturity: StageConfig::maturity(),
        }
    }

    /// Determine the lifecycle stage for a given interaction count.
    ///
    /// # Arguments
    ///
    /// * `interaction_count` - Total number of interactions
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::{LifecycleConfig, LifecycleStage};
    ///
    /// let config = LifecycleConfig::default();
    /// assert_eq!(config.stage_for_count(0), LifecycleStage::Infancy);
    /// assert_eq!(config.stage_for_count(50), LifecycleStage::Growth);
    /// assert_eq!(config.stage_for_count(500), LifecycleStage::Maturity);
    /// ```
    #[inline]
    pub fn stage_for_count(&self, interaction_count: u64) -> LifecycleStage {
        if interaction_count < self.infancy_threshold {
            LifecycleStage::Infancy
        } else if interaction_count < self.growth_threshold {
            LifecycleStage::Growth
        } else {
            LifecycleStage::Maturity
        }
    }

    /// Get the stage configuration for a specific stage.
    ///
    /// # Arguments
    ///
    /// * `stage` - The lifecycle stage
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::{LifecycleConfig, LifecycleStage};
    ///
    /// let config = LifecycleConfig::default();
    /// let stage_config = config.get_stage_config(LifecycleStage::Maturity);
    /// assert_eq!(stage_config.entropy_trigger, 0.6);
    /// ```
    #[inline]
    pub fn get_stage_config(&self, stage: LifecycleStage) -> &StageConfig {
        match stage {
            LifecycleStage::Infancy => &self.infancy,
            LifecycleStage::Growth => &self.growth,
            LifecycleStage::Maturity => &self.maturity,
        }
    }

    /// Get the stage configuration for a given interaction count.
    ///
    /// # Arguments
    ///
    /// * `interaction_count` - Total number of interactions
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleConfig;
    ///
    /// let config = LifecycleConfig::default();
    /// let stage_config = config.config_for_count(100);
    /// assert_eq!(stage_config.entropy_trigger, 0.7); // Growth stage
    /// ```
    #[inline]
    pub fn config_for_count(&self, interaction_count: u64) -> &StageConfig {
        self.get_stage_config(self.stage_for_count(interaction_count))
    }

    /// Check if a transition should occur at the given interaction count.
    ///
    /// Returns the new stage if a transition occurs.
    ///
    /// # Arguments
    ///
    /// * `current_stage` - Current lifecycle stage
    /// * `interaction_count` - Total interaction count
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::{LifecycleConfig, LifecycleStage};
    ///
    /// let config = LifecycleConfig::default();
    ///
    /// // At threshold, should transition
    /// let result = config.check_transition(LifecycleStage::Infancy, 50);
    /// assert_eq!(result, Some(LifecycleStage::Growth));
    ///
    /// // Below threshold, no transition
    /// let result = config.check_transition(LifecycleStage::Infancy, 49);
    /// assert_eq!(result, None);
    /// ```
    pub fn check_transition(
        &self,
        current_stage: LifecycleStage,
        interaction_count: u64,
    ) -> Option<LifecycleStage> {
        let new_stage = self.stage_for_count(interaction_count);
        if new_stage != current_stage && current_stage.can_transition_to(new_stage) {
            Some(new_stage)
        } else {
            None
        }
    }

    /// Validate the configuration.
    ///
    /// # Returns
    ///
    /// `true` if configuration is valid
    pub fn is_valid(&self) -> bool {
        self.infancy_threshold < self.growth_threshold
            && self.infancy.is_valid()
            && self.growth.is_valid()
            && self.maturity.is_valid()
    }
}

impl Default for LifecycleConfig {
    /// Default configuration with standard thresholds.
    ///
    /// - infancy_threshold: 50
    /// - growth_threshold: 500
    fn default() -> Self {
        Self {
            infancy_threshold: 50,
            growth_threshold: 500,
            infancy: StageConfig::infancy(),
            growth: StageConfig::growth(),
            maturity: StageConfig::maturity(),
        }
    }
}

// ============================================================================
// TESTS - REAL DATA ONLY, NO MOCKS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== STAGE CONFIG TESTS ==========

    #[test]
    fn test_stage_config_infancy() {
        let config = StageConfig::infancy();
        assert_eq!(config.entropy_trigger, 0.9);
        assert_eq!(config.coherence_trigger, 0.2);
        assert_eq!(config.min_importance_store, 0.1);
        assert_eq!(config.consolidation_threshold, 0.3);
    }

    #[test]
    fn test_stage_config_growth() {
        let config = StageConfig::growth();
        assert_eq!(config.entropy_trigger, 0.7);
        assert_eq!(config.coherence_trigger, 0.4);
        assert_eq!(config.min_importance_store, 0.3);
        assert_eq!(config.consolidation_threshold, 0.5);
    }

    #[test]
    fn test_stage_config_maturity() {
        let config = StageConfig::maturity();
        assert_eq!(config.entropy_trigger, 0.6);
        assert_eq!(config.coherence_trigger, 0.5);
        assert_eq!(config.min_importance_store, 0.4);
        assert_eq!(config.consolidation_threshold, 0.6);
    }

    #[test]
    fn test_stage_config_for_stage() {
        assert_eq!(StageConfig::for_stage(LifecycleStage::Infancy), StageConfig::infancy());
        assert_eq!(StageConfig::for_stage(LifecycleStage::Growth), StageConfig::growth());
        assert_eq!(StageConfig::for_stage(LifecycleStage::Maturity), StageConfig::maturity());
    }

    #[test]
    fn test_stage_config_default() {
        let default = StageConfig::default();
        let growth = StageConfig::growth();
        assert_eq!(default, growth);
    }

    #[test]
    fn test_should_store() {
        let config = StageConfig::infancy();

        // Above importance threshold, below entropy threshold
        assert!(config.should_store(0.15, 0.8));

        // Below importance threshold
        assert!(!config.should_store(0.05, 0.8));

        // Above entropy threshold
        assert!(!config.should_store(0.15, 0.95));
    }

    #[test]
    fn test_should_consolidate() {
        let config = StageConfig::growth();

        // Above both thresholds
        assert!(config.should_consolidate(0.6, 0.5));

        // Below magnitude threshold
        assert!(!config.should_consolidate(0.3, 0.5));

        // Below coherence threshold
        assert!(!config.should_consolidate(0.6, 0.3));
    }

    #[test]
    fn test_stage_config_is_valid() {
        assert!(StageConfig::infancy().is_valid());
        assert!(StageConfig::growth().is_valid());
        assert!(StageConfig::maturity().is_valid());

        // Invalid config
        let invalid = StageConfig {
            entropy_trigger: 1.5, // Out of range
            ..StageConfig::growth()
        };
        assert!(!invalid.is_valid());
    }

    // ========== LIFECYCLE CONFIG TESTS ==========

    #[test]
    fn test_lifecycle_config_default() {
        let config = LifecycleConfig::default();
        assert_eq!(config.infancy_threshold, 50);
        assert_eq!(config.growth_threshold, 500);
    }

    #[test]
    fn test_lifecycle_config_with_thresholds() {
        let config = LifecycleConfig::with_thresholds(25, 100);
        assert_eq!(config.infancy_threshold, 25);
        assert_eq!(config.growth_threshold, 100);
    }

    #[test]
    fn test_stage_for_count() {
        let config = LifecycleConfig::default();

        // Infancy
        assert_eq!(config.stage_for_count(0), LifecycleStage::Infancy);
        assert_eq!(config.stage_for_count(49), LifecycleStage::Infancy);

        // Growth
        assert_eq!(config.stage_for_count(50), LifecycleStage::Growth);
        assert_eq!(config.stage_for_count(499), LifecycleStage::Growth);

        // Maturity
        assert_eq!(config.stage_for_count(500), LifecycleStage::Maturity);
        assert_eq!(config.stage_for_count(10000), LifecycleStage::Maturity);
    }

    #[test]
    fn test_get_stage_config() {
        let config = LifecycleConfig::default();

        assert_eq!(config.get_stage_config(LifecycleStage::Infancy).entropy_trigger, 0.9);
        assert_eq!(config.get_stage_config(LifecycleStage::Growth).entropy_trigger, 0.7);
        assert_eq!(config.get_stage_config(LifecycleStage::Maturity).entropy_trigger, 0.6);
    }

    #[test]
    fn test_config_for_count() {
        let config = LifecycleConfig::default();

        assert_eq!(config.config_for_count(25).entropy_trigger, 0.9);
        assert_eq!(config.config_for_count(100).entropy_trigger, 0.7);
        assert_eq!(config.config_for_count(1000).entropy_trigger, 0.6);
    }

    #[test]
    fn test_check_transition() {
        let config = LifecycleConfig::default();

        // At threshold, should transition
        assert_eq!(
            config.check_transition(LifecycleStage::Infancy, 50),
            Some(LifecycleStage::Growth)
        );
        assert_eq!(
            config.check_transition(LifecycleStage::Growth, 500),
            Some(LifecycleStage::Maturity)
        );

        // Below threshold, no transition
        assert_eq!(config.check_transition(LifecycleStage::Infancy, 49), None);
        assert_eq!(config.check_transition(LifecycleStage::Growth, 499), None);

        // Already at stage, no transition
        assert_eq!(config.check_transition(LifecycleStage::Growth, 100), None);
        assert_eq!(config.check_transition(LifecycleStage::Maturity, 1000), None);
    }

    #[test]
    fn test_lifecycle_config_is_valid() {
        assert!(LifecycleConfig::default().is_valid());

        // Invalid: growth < infancy threshold
        let invalid = LifecycleConfig {
            infancy_threshold: 100,
            growth_threshold: 50,
            ..LifecycleConfig::default()
        };
        assert!(!invalid.is_valid());
    }

    // ========== SERIALIZATION TESTS ==========

    #[test]
    fn test_stage_config_serde() {
        let config = StageConfig::growth();
        let json = serde_json::to_string(&config).expect("Serialize failed");
        let recovered: StageConfig = serde_json::from_str(&json).expect("Deserialize failed");
        assert_eq!(config, recovered);
    }

    #[test]
    fn test_lifecycle_config_serde() {
        let config = LifecycleConfig::default();
        let json = serde_json::to_string(&config).expect("Serialize failed");
        let recovered: LifecycleConfig = serde_json::from_str(&json).expect("Deserialize failed");
        assert_eq!(config, recovered);
    }

    // ========== PROGRESSION TESTS ==========

    #[test]
    fn test_entropy_decreases_with_maturity() {
        let config = LifecycleConfig::default();
        let infancy_entropy = config.infancy.entropy_trigger;
        let growth_entropy = config.growth.entropy_trigger;
        let maturity_entropy = config.maturity.entropy_trigger;

        assert!(infancy_entropy > growth_entropy);
        assert!(growth_entropy > maturity_entropy);
    }

    #[test]
    fn test_coherence_increases_with_maturity() {
        let config = LifecycleConfig::default();
        let infancy_coherence = config.infancy.coherence_trigger;
        let growth_coherence = config.growth.coherence_trigger;
        let maturity_coherence = config.maturity.coherence_trigger;

        assert!(infancy_coherence < growth_coherence);
        assert!(growth_coherence < maturity_coherence);
    }

    #[test]
    fn test_importance_increases_with_maturity() {
        let config = LifecycleConfig::default();
        let infancy_importance = config.infancy.min_importance_store;
        let growth_importance = config.growth.min_importance_store;
        let maturity_importance = config.maturity.min_importance_store;

        assert!(infancy_importance < growth_importance);
        assert!(growth_importance < maturity_importance);
    }
}
```

## Acceptance Criteria

### StageConfig Signatures

- [ ] `StageConfig::infancy() -> Self`
- [ ] `StageConfig::growth() -> Self`
- [ ] `StageConfig::maturity() -> Self`
- [ ] `StageConfig::for_stage(stage: LifecycleStage) -> Self`
- [ ] `StageConfig::should_store(&self, importance: f32, entropy: f32) -> bool`
- [ ] `StageConfig::should_consolidate(&self, magnitude: f32, coherence: f32) -> bool`
- [ ] `StageConfig::is_valid(&self) -> bool`

### LifecycleConfig Signatures

- [ ] `LifecycleConfig::with_thresholds(infancy: u64, growth: u64) -> Self`
- [ ] `LifecycleConfig::stage_for_count(&self, count: u64) -> LifecycleStage`
- [ ] `LifecycleConfig::get_stage_config(&self, stage: LifecycleStage) -> &StageConfig`
- [ ] `LifecycleConfig::config_for_count(&self, count: u64) -> &StageConfig`
- [ ] `LifecycleConfig::check_transition(&self, current: LifecycleStage, count: u64) -> Option<LifecycleStage>`
- [ ] `LifecycleConfig::is_valid(&self) -> bool`

### Default Values Verification

| Stage    | entropy_trigger | coherence_trigger | min_importance | consolidation |
|----------|-----------------|-------------------|----------------|---------------|
| Infancy  | 0.9             | 0.2               | 0.1            | 0.3           |
| Growth   | 0.7             | 0.4               | 0.3            | 0.5           |
| Maturity | 0.6             | 0.5               | 0.4            | 0.6           |

### Threshold Verification

- [ ] `infancy_threshold = 50`
- [ ] `growth_threshold = 500`
- [ ] Values progress correctly (entropy decreases, coherence increases)

## Verification Commands

```bash
# 1. Build the crate
cargo build -p context-graph-utl

# 2. Run config tests
cargo test -p context-graph-utl config -- --nocapture

# 3. Run specific tests
cargo test -p context-graph-utl test_stage_for_count
cargo test -p context-graph-utl test_check_transition

# 4. Run clippy
cargo clippy -p context-graph-utl -- -D warnings

# 5. Run doc tests
cargo test -p context-graph-utl --doc
```

## Dependencies

- M05-T05: LifecycleStage enum (for stage_for_count return type)

## Notes for Implementer

1. StageConfig values match SPEC-UTL-005 Section 8.4 exactly
2. Default thresholds (50, 500) are from TECH-UTL-005 Section 3
3. LifecycleStage dependency for `for_stage()` and `stage_for_count()`
4. Tests are co-located in `#[cfg(test)]` module per constitution

---

*Task Version: 1.0.0*
*Created: 2026-01-04*
*Module: 05 - UTL Integration*
