---
id: "M05-T06"
title: "Define LifecycleLambdaWeights Struct (Marblestone)"
description: |
  Implement LifecycleLambdaWeights struct for dynamic learning rate modulation.
  Fields: lambda_novelty (f32), lambda_consolidation (f32).
  Invariant: lambda_novelty + lambda_consolidation = 1.0.
  Methods: new(novelty, consolidation) with validation, apply(delta_s, delta_c),
  is_balanced(), is_novelty_dominant(), is_consolidation_dominant().
  REQ-UTL-034 compliance for weight application.
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 1.5
sequence: 6
depends_on: []
spec_refs:
  - "TECH-UTL-005 Section 3.2"
  - "REQ-UTL-034"
files_to_create:
  - path: "crates/context-graph-utl/src/lifecycle/lambda.rs"
    description: "LifecycleLambdaWeights struct for Marblestone dynamic learning modulation"
files_to_modify:
  - path: "crates/context-graph-utl/src/lifecycle/mod.rs"
    description: "Add lambda module and re-export LifecycleLambdaWeights"
  - path: "crates/context-graph-utl/src/lib.rs"
    description: "Re-export LifecycleLambdaWeights at crate root"
test_file: "crates/context-graph-utl/tests/lifecycle_tests.rs"
---

## Overview

The LifecycleLambdaWeights struct encapsulates the dual learning rate weights used in the Marblestone-inspired developmental learning system. These weights modulate the balance between novelty-seeking (surprise-driven) learning and coherence-preserving (consolidation) learning.

## Mathematical Foundation

The UTL learning equation uses lambda weights to modulate components:

```
L_weighted = lambda_novelty * delta_s + lambda_consolidation * delta_c
```

Where:
- `lambda_novelty` controls sensitivity to surprising/novel information
- `lambda_consolidation` controls preference for coherent/consistent information
- `lambda_novelty + lambda_consolidation = 1.0` (invariant)

## Implementation Requirements

### File: `crates/context-graph-utl/src/lifecycle/lambda.rs`

```rust
//! LifecycleLambdaWeights for Marblestone dynamic learning rate modulation.
//!
//! # Lambda Weights
//!
//! Lambda weights modulate the balance between:
//! - **Novelty**: Sensitivity to surprising/unexpected information (delta_s)
//! - **Consolidation**: Preference for coherent/consistent information (delta_c)
//!
//! # Invariant
//!
//! `lambda_novelty + lambda_consolidation = 1.0` (enforced at construction)
//!
//! # Marblestone Theory
//!
//! Based on developmental learning theory where:
//! - Early stages (Infancy): High novelty weight for exploration
//! - Late stages (Maturity): High consolidation weight for stability
//!
//! # REQ-UTL-034 Compliance
//!
//! Weight application formula:
//! `L_weighted = lambda_novelty * delta_s + lambda_consolidation * delta_c`

use serde::{Deserialize, Serialize};

use crate::error::UtlError;

/// Lambda weights for modulating learning between novelty and consolidation.
///
/// # Invariant
///
/// `lambda_novelty + lambda_consolidation = 1.0`
///
/// # Example
///
/// ```
/// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
///
/// // Balanced weights (Growth stage)
/// let weights = LifecycleLambdaWeights::new(0.5, 0.5).unwrap();
/// assert!(weights.is_balanced());
///
/// // Apply to learning signals
/// let delta_s = 0.8; // High surprise
/// let delta_c = 0.4; // Low coherence
/// let weighted = weights.apply(delta_s, delta_c);
/// assert_eq!(weighted, 0.6); // 0.5 * 0.8 + 0.5 * 0.4
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LifecycleLambdaWeights {
    /// Weight for novelty/surprise component (0.0 to 1.0).
    /// Higher values prioritize learning from surprising information.
    pub lambda_novelty: f32,

    /// Weight for consolidation/coherence component (0.0 to 1.0).
    /// Higher values prioritize maintaining coherent knowledge.
    pub lambda_consolidation: f32,
}

impl LifecycleLambdaWeights {
    /// Tolerance for floating-point comparison in invariant check.
    const EPSILON: f32 = 1e-6;

    /// Create new lambda weights with validation.
    ///
    /// # Arguments
    ///
    /// * `novelty` - Weight for novelty component (0.0 to 1.0)
    /// * `consolidation` - Weight for consolidation component (0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// * `Ok(LifecycleLambdaWeights)` if weights sum to 1.0
    /// * `Err(UtlError::InvalidLambdaWeights)` if invariant violated
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// // Valid weights
    /// let weights = LifecycleLambdaWeights::new(0.7, 0.3).unwrap();
    /// assert_eq!(weights.lambda_novelty, 0.7);
    ///
    /// // Invalid weights (don't sum to 1.0)
    /// let result = LifecycleLambdaWeights::new(0.5, 0.3);
    /// assert!(result.is_err());
    /// ```
    pub fn new(novelty: f32, consolidation: f32) -> Result<Self, UtlError> {
        // Validate range
        if novelty < 0.0 || novelty > 1.0 {
            return Err(UtlError::InvalidLambdaWeights {
                novelty,
                consolidation,
                reason: format!("lambda_novelty must be in [0, 1], got {}", novelty),
            });
        }
        if consolidation < 0.0 || consolidation > 1.0 {
            return Err(UtlError::InvalidLambdaWeights {
                novelty,
                consolidation,
                reason: format!("lambda_consolidation must be in [0, 1], got {}", consolidation),
            });
        }

        // Validate invariant: weights sum to 1.0
        let sum = novelty + consolidation;
        if (sum - 1.0).abs() > Self::EPSILON {
            return Err(UtlError::InvalidLambdaWeights {
                novelty,
                consolidation,
                reason: format!(
                    "Weights must sum to 1.0, got {} + {} = {}",
                    novelty, consolidation, sum
                ),
            });
        }

        Ok(Self {
            lambda_novelty: novelty,
            lambda_consolidation: consolidation,
        })
    }

    /// Create weights for Infancy stage (high novelty-seeking).
    ///
    /// Returns: `lambda_novelty=0.7, lambda_consolidation=0.3`
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// let weights = LifecycleLambdaWeights::infancy();
    /// assert_eq!(weights.lambda_novelty, 0.7);
    /// assert!(weights.is_novelty_dominant());
    /// ```
    #[inline]
    pub fn infancy() -> Self {
        Self {
            lambda_novelty: 0.7,
            lambda_consolidation: 0.3,
        }
    }

    /// Create weights for Growth stage (balanced).
    ///
    /// Returns: `lambda_novelty=0.5, lambda_consolidation=0.5`
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// let weights = LifecycleLambdaWeights::growth();
    /// assert!(weights.is_balanced());
    /// ```
    #[inline]
    pub fn growth() -> Self {
        Self {
            lambda_novelty: 0.5,
            lambda_consolidation: 0.5,
        }
    }

    /// Create weights for Maturity stage (high consolidation).
    ///
    /// Returns: `lambda_novelty=0.3, lambda_consolidation=0.7`
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// let weights = LifecycleLambdaWeights::maturity();
    /// assert_eq!(weights.lambda_consolidation, 0.7);
    /// assert!(weights.is_consolidation_dominant());
    /// ```
    #[inline]
    pub fn maturity() -> Self {
        Self {
            lambda_novelty: 0.3,
            lambda_consolidation: 0.7,
        }
    }

    /// Apply weights to surprise and coherence signals.
    ///
    /// Formula: `lambda_novelty * delta_s + lambda_consolidation * delta_c`
    ///
    /// # Arguments
    ///
    /// * `delta_s` - Surprise/entropy signal (typically 0.0 to 1.0)
    /// * `delta_c` - Coherence signal (typically 0.0 to 1.0)
    ///
    /// # Returns
    ///
    /// Weighted combination of signals.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// let weights = LifecycleLambdaWeights::growth();
    /// let result = weights.apply(0.8, 0.4);
    /// assert_eq!(result, 0.6); // 0.5 * 0.8 + 0.5 * 0.4
    /// ```
    #[inline]
    pub fn apply(&self, delta_s: f32, delta_c: f32) -> f32 {
        self.lambda_novelty * delta_s + self.lambda_consolidation * delta_c
    }

    /// Apply weights and return individual components as tuple.
    ///
    /// # Arguments
    ///
    /// * `delta_s` - Surprise/entropy signal
    /// * `delta_c` - Coherence signal
    ///
    /// # Returns
    ///
    /// Tuple of (weighted_novelty, weighted_consolidation)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// let weights = LifecycleLambdaWeights::infancy();
    /// let (novelty, consolidation) = weights.apply_components(0.8, 0.4);
    /// assert_eq!(novelty, 0.56);     // 0.7 * 0.8
    /// assert_eq!(consolidation, 0.12); // 0.3 * 0.4
    /// ```
    #[inline]
    pub fn apply_components(&self, delta_s: f32, delta_c: f32) -> (f32, f32) {
        (
            self.lambda_novelty * delta_s,
            self.lambda_consolidation * delta_c,
        )
    }

    /// Check if weights are balanced (equal novelty and consolidation).
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// assert!(LifecycleLambdaWeights::growth().is_balanced());
    /// assert!(!LifecycleLambdaWeights::infancy().is_balanced());
    /// ```
    #[inline]
    pub fn is_balanced(&self) -> bool {
        (self.lambda_novelty - self.lambda_consolidation).abs() < Self::EPSILON
    }

    /// Check if novelty weight dominates consolidation.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// assert!(LifecycleLambdaWeights::infancy().is_novelty_dominant());
    /// assert!(!LifecycleLambdaWeights::maturity().is_novelty_dominant());
    /// ```
    #[inline]
    pub fn is_novelty_dominant(&self) -> bool {
        self.lambda_novelty > self.lambda_consolidation + Self::EPSILON
    }

    /// Check if consolidation weight dominates novelty.
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// assert!(LifecycleLambdaWeights::maturity().is_consolidation_dominant());
    /// assert!(!LifecycleLambdaWeights::infancy().is_consolidation_dominant());
    /// ```
    #[inline]
    pub fn is_consolidation_dominant(&self) -> bool {
        self.lambda_consolidation > self.lambda_novelty + Self::EPSILON
    }

    /// Interpolate between two weight configurations.
    ///
    /// Useful for smooth transitions between lifecycle stages.
    ///
    /// # Arguments
    ///
    /// * `other` - Target weights
    /// * `t` - Interpolation factor (0.0 = self, 1.0 = other)
    ///
    /// # Example
    ///
    /// ```
    /// use context_graph_utl::lifecycle::LifecycleLambdaWeights;
    ///
    /// let infancy = LifecycleLambdaWeights::infancy();
    /// let growth = LifecycleLambdaWeights::growth();
    ///
    /// let midpoint = infancy.lerp(&growth, 0.5);
    /// assert_eq!(midpoint.lambda_novelty, 0.6); // (0.7 + 0.5) / 2
    /// ```
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t_clamped = t.clamp(0.0, 1.0);
        let novelty = self.lambda_novelty + t_clamped * (other.lambda_novelty - self.lambda_novelty);
        let consolidation = self.lambda_consolidation + t_clamped * (other.lambda_consolidation - self.lambda_consolidation);

        Self {
            lambda_novelty: novelty,
            lambda_consolidation: consolidation,
        }
    }
}

impl Default for LifecycleLambdaWeights {
    /// Default weights are balanced (Growth stage).
    fn default() -> Self {
        Self::growth()
    }
}

impl std::fmt::Display for LifecycleLambdaWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LambdaWeights(novelty={:.2}, consolidation={:.2})",
            self.lambda_novelty, self.lambda_consolidation
        )
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
    fn test_new_valid_weights() {
        let weights = LifecycleLambdaWeights::new(0.7, 0.3).unwrap();
        assert_eq!(weights.lambda_novelty, 0.7);
        assert_eq!(weights.lambda_consolidation, 0.3);
    }

    #[test]
    fn test_new_balanced_weights() {
        let weights = LifecycleLambdaWeights::new(0.5, 0.5).unwrap();
        assert!(weights.is_balanced());
    }

    #[test]
    fn test_new_invalid_sum_too_low() {
        let result = LifecycleLambdaWeights::new(0.3, 0.3);
        assert!(result.is_err());
        if let Err(UtlError::InvalidLambdaWeights { reason, .. }) = result {
            assert!(reason.contains("sum to 1.0"));
        }
    }

    #[test]
    fn test_new_invalid_sum_too_high() {
        let result = LifecycleLambdaWeights::new(0.7, 0.7);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_invalid_negative_novelty() {
        let result = LifecycleLambdaWeights::new(-0.1, 1.1);
        assert!(result.is_err());
        if let Err(UtlError::InvalidLambdaWeights { reason, .. }) = result {
            assert!(reason.contains("lambda_novelty"));
        }
    }

    #[test]
    fn test_new_invalid_over_one() {
        let result = LifecycleLambdaWeights::new(1.5, -0.5);
        assert!(result.is_err());
    }

    // ========== FACTORY METHODS TESTS ==========

    #[test]
    fn test_infancy_weights() {
        let weights = LifecycleLambdaWeights::infancy();
        assert_eq!(weights.lambda_novelty, 0.7);
        assert_eq!(weights.lambda_consolidation, 0.3);
    }

    #[test]
    fn test_growth_weights() {
        let weights = LifecycleLambdaWeights::growth();
        assert_eq!(weights.lambda_novelty, 0.5);
        assert_eq!(weights.lambda_consolidation, 0.5);
    }

    #[test]
    fn test_maturity_weights() {
        let weights = LifecycleLambdaWeights::maturity();
        assert_eq!(weights.lambda_novelty, 0.3);
        assert_eq!(weights.lambda_consolidation, 0.7);
    }

    #[test]
    fn test_factory_weights_sum_to_one() {
        for weights in [
            LifecycleLambdaWeights::infancy(),
            LifecycleLambdaWeights::growth(),
            LifecycleLambdaWeights::maturity(),
        ] {
            let sum = weights.lambda_novelty + weights.lambda_consolidation;
            assert!((sum - 1.0).abs() < 1e-6);
        }
    }

    // ========== DEFAULT TESTS ==========

    #[test]
    fn test_default_is_growth() {
        let default = LifecycleLambdaWeights::default();
        let growth = LifecycleLambdaWeights::growth();
        assert_eq!(default, growth);
    }

    // ========== APPLY TESTS ==========

    #[test]
    fn test_apply_balanced() {
        let weights = LifecycleLambdaWeights::growth();
        let result = weights.apply(0.8, 0.4);
        assert!((result - 0.6).abs() < 1e-6); // 0.5 * 0.8 + 0.5 * 0.4 = 0.6
    }

    #[test]
    fn test_apply_infancy() {
        let weights = LifecycleLambdaWeights::infancy();
        let result = weights.apply(1.0, 0.0);
        assert!((result - 0.7).abs() < 1e-6); // 0.7 * 1.0 + 0.3 * 0.0 = 0.7
    }

    #[test]
    fn test_apply_maturity() {
        let weights = LifecycleLambdaWeights::maturity();
        let result = weights.apply(0.0, 1.0);
        assert!((result - 0.7).abs() < 1e-6); // 0.3 * 0.0 + 0.7 * 1.0 = 0.7
    }

    #[test]
    fn test_apply_zero_inputs() {
        let weights = LifecycleLambdaWeights::growth();
        let result = weights.apply(0.0, 0.0);
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_apply_max_inputs() {
        let weights = LifecycleLambdaWeights::growth();
        let result = weights.apply(1.0, 1.0);
        assert!((result - 1.0).abs() < 1e-6);
    }

    // ========== APPLY COMPONENTS TESTS ==========

    #[test]
    fn test_apply_components() {
        let weights = LifecycleLambdaWeights::infancy();
        let (novelty, consolidation) = weights.apply_components(0.8, 0.4);
        assert!((novelty - 0.56).abs() < 1e-6);     // 0.7 * 0.8
        assert!((consolidation - 0.12).abs() < 1e-6); // 0.3 * 0.4
    }

    #[test]
    fn test_apply_components_sum_equals_apply() {
        let weights = LifecycleLambdaWeights::maturity();
        let delta_s = 0.6;
        let delta_c = 0.8;
        let (novelty, consolidation) = weights.apply_components(delta_s, delta_c);
        let combined = weights.apply(delta_s, delta_c);
        assert!((novelty + consolidation - combined).abs() < 1e-6);
    }

    // ========== DOMINANCE TESTS ==========

    #[test]
    fn test_is_balanced() {
        assert!(LifecycleLambdaWeights::growth().is_balanced());
        assert!(!LifecycleLambdaWeights::infancy().is_balanced());
        assert!(!LifecycleLambdaWeights::maturity().is_balanced());
    }

    #[test]
    fn test_is_novelty_dominant() {
        assert!(LifecycleLambdaWeights::infancy().is_novelty_dominant());
        assert!(!LifecycleLambdaWeights::growth().is_novelty_dominant());
        assert!(!LifecycleLambdaWeights::maturity().is_novelty_dominant());
    }

    #[test]
    fn test_is_consolidation_dominant() {
        assert!(!LifecycleLambdaWeights::infancy().is_consolidation_dominant());
        assert!(!LifecycleLambdaWeights::growth().is_consolidation_dominant());
        assert!(LifecycleLambdaWeights::maturity().is_consolidation_dominant());
    }

    // ========== LERP TESTS ==========

    #[test]
    fn test_lerp_at_zero() {
        let infancy = LifecycleLambdaWeights::infancy();
        let growth = LifecycleLambdaWeights::growth();
        let result = infancy.lerp(&growth, 0.0);
        assert_eq!(result, infancy);
    }

    #[test]
    fn test_lerp_at_one() {
        let infancy = LifecycleLambdaWeights::infancy();
        let growth = LifecycleLambdaWeights::growth();
        let result = infancy.lerp(&growth, 1.0);
        assert_eq!(result, growth);
    }

    #[test]
    fn test_lerp_midpoint() {
        let infancy = LifecycleLambdaWeights::infancy();
        let growth = LifecycleLambdaWeights::growth();
        let result = infancy.lerp(&growth, 0.5);
        assert!((result.lambda_novelty - 0.6).abs() < 1e-6);
        assert!((result.lambda_consolidation - 0.4).abs() < 1e-6);
    }

    #[test]
    fn test_lerp_preserves_invariant() {
        let infancy = LifecycleLambdaWeights::infancy();
        let maturity = LifecycleLambdaWeights::maturity();
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let result = infancy.lerp(&maturity, t);
            let sum = result.lambda_novelty + result.lambda_consolidation;
            assert!((sum - 1.0).abs() < 1e-6, "Invariant violated at t={}", t);
        }
    }

    #[test]
    fn test_lerp_clamps_t() {
        let infancy = LifecycleLambdaWeights::infancy();
        let growth = LifecycleLambdaWeights::growth();

        let result_negative = infancy.lerp(&growth, -0.5);
        assert_eq!(result_negative, infancy);

        let result_over = infancy.lerp(&growth, 1.5);
        assert_eq!(result_over, growth);
    }

    // ========== DISPLAY TESTS ==========

    #[test]
    fn test_display() {
        let weights = LifecycleLambdaWeights::infancy();
        let display = format!("{}", weights);
        assert!(display.contains("0.70"));
        assert!(display.contains("0.30"));
    }

    // ========== SERIALIZATION TESTS ==========

    #[test]
    fn test_serde_roundtrip() {
        for weights in [
            LifecycleLambdaWeights::infancy(),
            LifecycleLambdaWeights::growth(),
            LifecycleLambdaWeights::maturity(),
        ] {
            let json = serde_json::to_string(&weights).expect("Serialize failed");
            let recovered: LifecycleLambdaWeights = serde_json::from_str(&json).expect("Deserialize failed");
            assert_eq!(weights, recovered);
        }
    }

    // ========== CLONE/COPY TESTS ==========

    #[test]
    fn test_clone() {
        let weights = LifecycleLambdaWeights::infancy();
        let cloned = weights.clone();
        assert_eq!(weights, cloned);
    }

    #[test]
    fn test_copy() {
        let weights = LifecycleLambdaWeights::growth();
        let copied = weights;
        assert_eq!(weights, copied);
    }
}
```

## Acceptance Criteria

### Signatures (MUST MATCH EXACTLY)

- [ ] `LifecycleLambdaWeights::new(novelty: f32, consolidation: f32) -> Result<Self, UtlError>`
- [ ] `LifecycleLambdaWeights::infancy() -> Self`
- [ ] `LifecycleLambdaWeights::growth() -> Self`
- [ ] `LifecycleLambdaWeights::maturity() -> Self`
- [ ] `LifecycleLambdaWeights::apply(&self, delta_s: f32, delta_c: f32) -> f32`
- [ ] `LifecycleLambdaWeights::apply_components(&self, delta_s: f32, delta_c: f32) -> (f32, f32)`
- [ ] `LifecycleLambdaWeights::is_balanced(&self) -> bool`
- [ ] `LifecycleLambdaWeights::is_novelty_dominant(&self) -> bool`
- [ ] `LifecycleLambdaWeights::is_consolidation_dominant(&self) -> bool`
- [ ] `LifecycleLambdaWeights::lerp(&self, other: &Self, t: f32) -> Self`

### Trait Implementations

- [ ] `Default` for `LifecycleLambdaWeights` (returns balanced/Growth weights)
- [ ] `Clone`, `Copy`, `Debug`, `PartialEq`
- [ ] `Serialize`, `Deserialize` (serde)
- [ ] `Display`

### Invariant Verification

- [ ] `new()` validates sum = 1.0, returns error otherwise
- [ ] All factory methods produce valid weights (sum = 1.0)
- [ ] `lerp()` preserves invariant for all t values

### Weight Values

- [ ] Infancy: `lambda_novelty=0.7`, `lambda_consolidation=0.3`
- [ ] Growth: `lambda_novelty=0.5`, `lambda_consolidation=0.5`
- [ ] Maturity: `lambda_novelty=0.3`, `lambda_consolidation=0.7`

## Verification Commands

```bash
# 1. Build the crate
cargo build -p context-graph-utl

# 2. Run lifecycle tests
cargo test -p context-graph-utl lifecycle -- --nocapture

# 3. Run specific lambda tests
cargo test -p context-graph-utl test_apply_balanced
cargo test -p context-graph-utl test_lerp_preserves_invariant

# 4. Run clippy
cargo clippy -p context-graph-utl -- -D warnings

# 5. Run doc tests
cargo test -p context-graph-utl --doc
```

## Dependencies

This task has no dependencies.

**Note**: This task is used by M05-T05 (LifecycleStage) for `get_lambda_weights()`.

## Notes for Implementer

1. The `UtlError::InvalidLambdaWeights` variant should be defined in M05-T23
2. For initial implementation, can define a minimal UtlError in error.rs
3. Tests are co-located in `#[cfg(test)]` module per constitution
4. The EPSILON constant (1e-6) ensures floating-point comparisons are robust

---

*Task Version: 1.0.0*
*Created: 2026-01-04*
*Module: 05 - UTL Integration*
