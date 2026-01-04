---
id: "M05-T05"
title: "Define LifecycleStage Enum (Marblestone)"
description: |
  Implement LifecycleStage enum with Marblestone-inspired dynamic learning rates.
  Variants: Infancy (0-50 interactions), Growth (50-500), Maturity (500+).
  Include methods: get_lambda_weights(), is_novelty_seeking(), is_coherence_preserving(),
  name(), entropy_trigger(), coherence_trigger().
  REQ-UTL-030 through REQ-UTL-035 compliance.
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 2
sequence: 5
depends_on: []
spec_refs:
  - "TECH-UTL-005 Section 3"
  - "SPEC-UTL-005 Section 8.2"
  - "REQ-UTL-030"
  - "REQ-UTL-031"
  - "REQ-UTL-032"
  - "REQ-UTL-033"
files_to_create:
  - path: "crates/context-graph-utl/src/lifecycle/stage.rs"
    description: "LifecycleStage enum with Marblestone-inspired dynamic learning rates"
files_to_modify:
  - path: "crates/context-graph-utl/src/lifecycle/mod.rs"
    description: "Add stage module and re-export LifecycleStage"
  - path: "crates/context-graph-utl/src/lib.rs"
    description: "Re-export LifecycleStage at crate root"
test_file: "crates/context-graph-utl/tests/lifecycle_tests.rs"
---

## Overview

The LifecycleStage enum represents the three developmental phases of the learning system, inspired by Marblestone's developmental learning theory. Each stage has distinct lambda weights that modulate the balance between novelty-seeking (exploration) and coherence-preserving (consolidation) behaviors.

## Marblestone Developmental Theory

The lifecycle stages model cognitive development:

1. **Infancy (0-50 interactions)**: High novelty-seeking, rapid learning, low consolidation threshold
2. **Growth (50-500 interactions)**: Balanced exploration and consolidation
3. **Maturity (500+ interactions)**: High coherence-preserving, selective learning, high consolidation threshold

## Implementation Requirements

### File: `crates/context-graph-utl/src/lifecycle/stage.rs`

```rust
//! LifecycleStage enum for Marblestone-inspired developmental phases.
//!
//! # Lifecycle Stages
//!
//! The system progresses through three developmental stages:
//!
//! - **Infancy**: Rapid exploration, high novelty-seeking (lambda_novelty=0.7)
//! - **Growth**: Balanced learning (lambda_novelty=0.5)
//! - **Maturity**: Selective consolidation (lambda_novelty=0.3)
//!
//! # Marblestone Theory Reference
//!
//! Based on developmental learning theory where early learning prioritizes
//! exploration while mature learning prioritizes coherence preservation.
//!
//! # Constitution Reference
//!
//! - REQ-UTL-030: Infancy stage definition
//! - REQ-UTL-031: Growth stage definition
//! - REQ-UTL-032: Maturity stage definition
//! - REQ-UTL-033: Lambda weight retrieval per stage

use serde::{Deserialize, Serialize};

use crate::lifecycle::LifecycleLambdaWeights;

/// Lifecycle stage representing developmental phase of the learning system.
///
/// Each stage has distinct characteristics:
/// - **Infancy**: 0-50 interactions, novelty-seeking, rapid learning
/// - **Growth**: 50-500 interactions, balanced exploration/consolidation
/// - **Maturity**: 500+ interactions, coherence-preserving, selective learning
///
/// # Example
///
/// ```
/// use context_graph_utl::lifecycle::LifecycleStage;
///
/// let stage = LifecycleStage::Infancy;
/// assert!(stage.is_novelty_seeking());
/// assert!(!stage.is_coherence_preserving());
///
/// let weights = stage.get_lambda_weights();
/// assert_eq!(weights.lambda_novelty, 0.7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum LifecycleStage {
    /// Early developmental phase (0-50 interactions).
    /// High novelty-seeking, low consolidation threshold.
    /// Lambda weights: novelty=0.7, consolidation=0.3
    #[default]
    Infancy = 0,

    /// Middle developmental phase (50-500 interactions).
    /// Balanced exploration and consolidation.
    /// Lambda weights: novelty=0.5, consolidation=0.5
    Growth = 1,

    /// Mature developmental phase (500+ interactions).
    /// High coherence-preserving, selective learning.
    /// Lambda weights: novelty=0.3, consolidation=0.7
    Maturity = 2,
}

impl LifecycleStage {
    /// Get the lambda weights for this lifecycle stage.
    ///
    /// Returns weights that modulate learning between novelty and consolidation.
    ///
    /// # Stage Weights
    ///
    /// | Stage    | lambda_novelty | lambda_consolidation |
    /// |----------|----------------|----------------------|
    /// | Infancy  | 0.7            | 0.3                  |
    /// | Growth   | 0.5            | 0.5                  |
    /// | Maturity | 0.3            | 0.7                  |
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// let stage = LifecycleStage::Maturity;
    /// let weights = stage.get_lambda_weights();
    /// assert_eq!(weights.lambda_consolidation, 0.7);
    /// ```
    #[inline]
    pub fn get_lambda_weights(&self) -> LifecycleLambdaWeights {
        match self {
            Self::Infancy => LifecycleLambdaWeights::infancy(),
            Self::Growth => LifecycleLambdaWeights::growth(),
            Self::Maturity => LifecycleLambdaWeights::maturity(),
        }
    }

    /// Check if this stage prioritizes novelty-seeking behavior.
    ///
    /// Returns true for Infancy stage where lambda_novelty > lambda_consolidation.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert!(LifecycleStage::Infancy.is_novelty_seeking());
    /// assert!(!LifecycleStage::Growth.is_novelty_seeking());
    /// assert!(!LifecycleStage::Maturity.is_novelty_seeking());
    /// ```
    #[inline]
    pub fn is_novelty_seeking(&self) -> bool {
        matches!(self, Self::Infancy)
    }

    /// Check if this stage prioritizes coherence-preserving behavior.
    ///
    /// Returns true for Maturity stage where lambda_consolidation > lambda_novelty.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert!(!LifecycleStage::Infancy.is_coherence_preserving());
    /// assert!(!LifecycleStage::Growth.is_coherence_preserving());
    /// assert!(LifecycleStage::Maturity.is_coherence_preserving());
    /// ```
    #[inline]
    pub fn is_coherence_preserving(&self) -> bool {
        matches!(self, Self::Maturity)
    }

    /// Get the string name of this lifecycle stage.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert_eq!(LifecycleStage::Infancy.name(), "Infancy");
    /// assert_eq!(LifecycleStage::Growth.name(), "Growth");
    /// assert_eq!(LifecycleStage::Maturity.name(), "Maturity");
    /// ```
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Infancy => "Infancy",
            Self::Growth => "Growth",
            Self::Maturity => "Maturity",
        }
    }

    /// Get the entropy trigger threshold for this stage.
    ///
    /// Higher values mean more tolerance for high-entropy (surprising) content.
    ///
    /// | Stage    | Entropy Trigger |
    /// |----------|-----------------|
    /// | Infancy  | 0.9             |
    /// | Growth   | 0.7             |
    /// | Maturity | 0.6             |
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// // Infancy accepts more surprising content
    /// assert!(LifecycleStage::Infancy.entropy_trigger() > LifecycleStage::Maturity.entropy_trigger());
    /// ```
    #[inline]
    pub fn entropy_trigger(&self) -> f32 {
        match self {
            Self::Infancy => 0.9,
            Self::Growth => 0.7,
            Self::Maturity => 0.6,
        }
    }

    /// Get the coherence trigger threshold for this stage.
    ///
    /// Higher values mean more stringent coherence requirements for storage.
    ///
    /// | Stage    | Coherence Trigger |
    /// |----------|-------------------|
    /// | Infancy  | 0.2               |
    /// | Growth   | 0.4               |
    /// | Maturity | 0.5               |
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// // Maturity requires higher coherence
    /// assert!(LifecycleStage::Maturity.coherence_trigger() > LifecycleStage::Infancy.coherence_trigger());
    /// ```
    #[inline]
    pub fn coherence_trigger(&self) -> f32 {
        match self {
            Self::Infancy => 0.2,
            Self::Growth => 0.4,
            Self::Maturity => 0.5,
        }
    }

    /// Determine the lifecycle stage based on interaction count.
    ///
    /// # Thresholds
    ///
    /// - Infancy: 0-49 interactions
    /// - Growth: 50-499 interactions
    /// - Maturity: 500+ interactions
    ///
    /// # Arguments
    ///
    /// * `interaction_count` - Total number of interactions
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert_eq!(LifecycleStage::from_interaction_count(25), LifecycleStage::Infancy);
    /// assert_eq!(LifecycleStage::from_interaction_count(100), LifecycleStage::Growth);
    /// assert_eq!(LifecycleStage::from_interaction_count(1000), LifecycleStage::Maturity);
    /// ```
    #[inline]
    pub fn from_interaction_count(interaction_count: u64) -> Self {
        if interaction_count < 50 {
            Self::Infancy
        } else if interaction_count < 500 {
            Self::Growth
        } else {
            Self::Maturity
        }
    }

    /// Check if transition to next stage is possible.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleStage;
    ///
    /// assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Growth));
    /// assert!(!LifecycleStage::Maturity.can_transition_to(LifecycleStage::Infancy));
    /// ```
    #[inline]
    pub fn can_transition_to(&self, target: Self) -> bool {
        match (self, target) {
            (Self::Infancy, Self::Growth) => true,
            (Self::Growth, Self::Maturity) => true,
            _ => false,
        }
    }
}

impl std::fmt::Display for LifecycleStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// TESTS - REAL DATA ONLY, NO MOCKS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========== CONSTRUCTION TESTS ==========

    #[test]
    fn test_default_is_infancy() {
        let stage = LifecycleStage::default();
        assert_eq!(stage, LifecycleStage::Infancy);
    }

    #[test]
    fn test_repr_values() {
        assert_eq!(LifecycleStage::Infancy as u8, 0);
        assert_eq!(LifecycleStage::Growth as u8, 1);
        assert_eq!(LifecycleStage::Maturity as u8, 2);
    }

    // ========== LAMBDA WEIGHTS TESTS ==========

    #[test]
    fn test_infancy_lambda_weights() {
        let stage = LifecycleStage::Infancy;
        let weights = stage.get_lambda_weights();
        assert_eq!(weights.lambda_novelty, 0.7);
        assert_eq!(weights.lambda_consolidation, 0.3);
    }

    #[test]
    fn test_growth_lambda_weights() {
        let stage = LifecycleStage::Growth;
        let weights = stage.get_lambda_weights();
        assert_eq!(weights.lambda_novelty, 0.5);
        assert_eq!(weights.lambda_consolidation, 0.5);
    }

    #[test]
    fn test_maturity_lambda_weights() {
        let stage = LifecycleStage::Maturity;
        let weights = stage.get_lambda_weights();
        assert_eq!(weights.lambda_novelty, 0.3);
        assert_eq!(weights.lambda_consolidation, 0.7);
    }

    #[test]
    fn test_lambda_weights_sum_to_one() {
        for stage in [LifecycleStage::Infancy, LifecycleStage::Growth, LifecycleStage::Maturity] {
            let weights = stage.get_lambda_weights();
            let sum = weights.lambda_novelty + weights.lambda_consolidation;
            assert!((sum - 1.0).abs() < 1e-6, "Weights must sum to 1.0 for {:?}", stage);
        }
    }

    // ========== BEHAVIOR FLAGS TESTS ==========

    #[test]
    fn test_is_novelty_seeking() {
        assert!(LifecycleStage::Infancy.is_novelty_seeking());
        assert!(!LifecycleStage::Growth.is_novelty_seeking());
        assert!(!LifecycleStage::Maturity.is_novelty_seeking());
    }

    #[test]
    fn test_is_coherence_preserving() {
        assert!(!LifecycleStage::Infancy.is_coherence_preserving());
        assert!(!LifecycleStage::Growth.is_coherence_preserving());
        assert!(LifecycleStage::Maturity.is_coherence_preserving());
    }

    // ========== NAME TESTS ==========

    #[test]
    fn test_name() {
        assert_eq!(LifecycleStage::Infancy.name(), "Infancy");
        assert_eq!(LifecycleStage::Growth.name(), "Growth");
        assert_eq!(LifecycleStage::Maturity.name(), "Maturity");
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", LifecycleStage::Infancy), "Infancy");
        assert_eq!(format!("{}", LifecycleStage::Growth), "Growth");
        assert_eq!(format!("{}", LifecycleStage::Maturity), "Maturity");
    }

    // ========== THRESHOLD TESTS ==========

    #[test]
    fn test_entropy_trigger_values() {
        assert_eq!(LifecycleStage::Infancy.entropy_trigger(), 0.9);
        assert_eq!(LifecycleStage::Growth.entropy_trigger(), 0.7);
        assert_eq!(LifecycleStage::Maturity.entropy_trigger(), 0.6);
    }

    #[test]
    fn test_coherence_trigger_values() {
        assert_eq!(LifecycleStage::Infancy.coherence_trigger(), 0.2);
        assert_eq!(LifecycleStage::Growth.coherence_trigger(), 0.4);
        assert_eq!(LifecycleStage::Maturity.coherence_trigger(), 0.5);
    }

    #[test]
    fn test_entropy_decreases_with_maturity() {
        assert!(LifecycleStage::Infancy.entropy_trigger() > LifecycleStage::Growth.entropy_trigger());
        assert!(LifecycleStage::Growth.entropy_trigger() > LifecycleStage::Maturity.entropy_trigger());
    }

    #[test]
    fn test_coherence_increases_with_maturity() {
        assert!(LifecycleStage::Infancy.coherence_trigger() < LifecycleStage::Growth.coherence_trigger());
        assert!(LifecycleStage::Growth.coherence_trigger() < LifecycleStage::Maturity.coherence_trigger());
    }

    // ========== FROM INTERACTION COUNT TESTS ==========

    #[test]
    fn test_from_interaction_count_infancy() {
        assert_eq!(LifecycleStage::from_interaction_count(0), LifecycleStage::Infancy);
        assert_eq!(LifecycleStage::from_interaction_count(25), LifecycleStage::Infancy);
        assert_eq!(LifecycleStage::from_interaction_count(49), LifecycleStage::Infancy);
    }

    #[test]
    fn test_from_interaction_count_growth() {
        assert_eq!(LifecycleStage::from_interaction_count(50), LifecycleStage::Growth);
        assert_eq!(LifecycleStage::from_interaction_count(250), LifecycleStage::Growth);
        assert_eq!(LifecycleStage::from_interaction_count(499), LifecycleStage::Growth);
    }

    #[test]
    fn test_from_interaction_count_maturity() {
        assert_eq!(LifecycleStage::from_interaction_count(500), LifecycleStage::Maturity);
        assert_eq!(LifecycleStage::from_interaction_count(1000), LifecycleStage::Maturity);
        assert_eq!(LifecycleStage::from_interaction_count(u64::MAX), LifecycleStage::Maturity);
    }

    // ========== TRANSITION TESTS ==========

    #[test]
    fn test_can_transition_forward() {
        assert!(LifecycleStage::Infancy.can_transition_to(LifecycleStage::Growth));
        assert!(LifecycleStage::Growth.can_transition_to(LifecycleStage::Maturity));
    }

    #[test]
    fn test_cannot_transition_backward() {
        assert!(!LifecycleStage::Growth.can_transition_to(LifecycleStage::Infancy));
        assert!(!LifecycleStage::Maturity.can_transition_to(LifecycleStage::Growth));
        assert!(!LifecycleStage::Maturity.can_transition_to(LifecycleStage::Infancy));
    }

    #[test]
    fn test_cannot_skip_stages() {
        assert!(!LifecycleStage::Infancy.can_transition_to(LifecycleStage::Maturity));
    }

    // ========== SERIALIZATION TESTS ==========

    #[test]
    fn test_serde_roundtrip() {
        for stage in [LifecycleStage::Infancy, LifecycleStage::Growth, LifecycleStage::Maturity] {
            let json = serde_json::to_string(&stage).expect("Serialize failed");
            let recovered: LifecycleStage = serde_json::from_str(&json).expect("Deserialize failed");
            assert_eq!(stage, recovered);
        }
    }

    // ========== EQUALITY TESTS ==========

    #[test]
    fn test_equality() {
        assert_eq!(LifecycleStage::Infancy, LifecycleStage::Infancy);
        assert_ne!(LifecycleStage::Infancy, LifecycleStage::Growth);
        assert_ne!(LifecycleStage::Growth, LifecycleStage::Maturity);
    }

    #[test]
    fn test_clone() {
        let stage = LifecycleStage::Growth;
        let cloned = stage.clone();
        assert_eq!(stage, cloned);
    }

    #[test]
    fn test_copy() {
        let stage = LifecycleStage::Maturity;
        let copied = stage;
        assert_eq!(stage, copied);
    }
}
```

## Acceptance Criteria

### Signatures (MUST MATCH EXACTLY)

- [ ] `LifecycleStage::Infancy`, `LifecycleStage::Growth`, `LifecycleStage::Maturity` variants
- [ ] `LifecycleStage::get_lambda_weights(&self) -> LifecycleLambdaWeights`
- [ ] `LifecycleStage::is_novelty_seeking(&self) -> bool`
- [ ] `LifecycleStage::is_coherence_preserving(&self) -> bool`
- [ ] `LifecycleStage::name(&self) -> &'static str`
- [ ] `LifecycleStage::entropy_trigger(&self) -> f32`
- [ ] `LifecycleStage::coherence_trigger(&self) -> f32`
- [ ] `LifecycleStage::from_interaction_count(count: u64) -> Self`

### Trait Implementations

- [ ] `Default` for `LifecycleStage` (returns `Infancy`)
- [ ] `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, `Hash`
- [ ] `Serialize`, `Deserialize` (serde)
- [ ] `Display` (returns stage name)
- [ ] `#[repr(u8)]` for efficient storage

### Lambda Weights Verification

- [ ] Infancy: `lambda_novelty=0.7`, `lambda_consolidation=0.3`
- [ ] Growth: `lambda_novelty=0.5`, `lambda_consolidation=0.5`
- [ ] Maturity: `lambda_novelty=0.3`, `lambda_consolidation=0.7`
- [ ] All weights sum to 1.0

### Threshold Verification

- [ ] Entropy trigger decreases with maturity (0.9 -> 0.7 -> 0.6)
- [ ] Coherence trigger increases with maturity (0.2 -> 0.4 -> 0.5)

## Verification Commands

```bash
# 1. Build the crate
cargo build -p context-graph-utl

# 2. Run lifecycle tests
cargo test -p context-graph-utl lifecycle -- --nocapture

# 3. Run specific stage tests
cargo test -p context-graph-utl test_infancy_lambda_weights
cargo test -p context-graph-utl test_from_interaction_count

# 4. Run clippy
cargo clippy -p context-graph-utl -- -D warnings

# 5. Run doc tests
cargo test -p context-graph-utl --doc
```

## Dependencies

This task has no dependencies and can be implemented first.

**Note**: This task depends on M05-T06 (LifecycleLambdaWeights) for the return type of `get_lambda_weights()`. Implement M05-T06 first or use a stub.

## Notes for Implementer

1. The `LifecycleLambdaWeights` type is defined in M05-T06
2. The lifecycle module should be at `crates/context-graph-utl/src/lifecycle/`
3. Tests are co-located in `#[cfg(test)]` module per constitution
4. All thresholds match SPEC-UTL-005 Section 8.2

---

*Task Version: 1.0.0*
*Created: 2026-01-04*
*Module: 05 - UTL Integration*
