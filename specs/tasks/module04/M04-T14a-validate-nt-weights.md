---
id: "M04-T14a"
title: "Implement NT Weight Range Validation"
description: |
  Add validate() method to NeurotransmitterWeights that ensures all weights in [0,1].
  Add net_activation() method implementing the canonical formula.
  Returns Result<(), GraphError::InvalidConfig> if any weight outside range.
layer: "logic"
status: "pending"
priority: "high"
estimated_hours: 1
sequence: 20
depends_on:
  - "M04-T14"
spec_refs:
  - "TECH-GRAPH-004 Section 4.1"
  - "REQ-KG-065"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/edges.rs"
    description: "Add validate() and net_activation() methods to NeurotransmitterWeights"
test_file: "crates/context-graph-graph/tests/marblestone_tests.rs"
---

## Context

Validation ensures NeurotransmitterWeights stay within biological plausibility bounds [0, 1]. The net_activation() method implements the canonical formula that converts the three weight components into a single activation value used for edge modulation. This is a critical calculation used throughout the graph traversal and search operations.

## Scope

### In Scope
- validate() method returning Result<(), GraphError>
- net_activation() method implementing canonical formula
- Validation error messages with specific weight name
- is_valid() convenience method

### Out of Scope
- Automatic clamping (fail fast approach)
- Weight adjustment/normalization
- GraphEdge integration (see M04-T15)

## Definition of Done

### Signatures

```rust
// Add to crates/context-graph-graph/src/storage/edges.rs
// (within impl NeurotransmitterWeights)

use crate::error::{GraphError, GraphResult};

impl NeurotransmitterWeights {
    /// Validate that all weights are in valid range [0.0, 1.0]
    ///
    /// # Returns
    /// * `Ok(())` if all weights are valid
    /// * `Err(GraphError::InvalidConfig)` if any weight is out of range
    ///
    /// # Example
    /// ```
    /// let weights = NeurotransmitterWeights::new(0.5, 0.5, 0.0);
    /// assert!(weights.validate().is_ok());
    ///
    /// let invalid = NeurotransmitterWeights::new(-0.1, 0.5, 0.0);
    /// assert!(invalid.validate().is_err());
    /// ```
    pub fn validate(&self) -> GraphResult<()> {
        if self.excitatory < 0.0 || self.excitatory > 1.0 {
            return Err(GraphError::InvalidConfig(format!(
                "excitatory weight {} out of range [0.0, 1.0]",
                self.excitatory
            )));
        }

        if self.inhibitory < 0.0 || self.inhibitory > 1.0 {
            return Err(GraphError::InvalidConfig(format!(
                "inhibitory weight {} out of range [0.0, 1.0]",
                self.inhibitory
            )));
        }

        if self.modulatory < 0.0 || self.modulatory > 1.0 {
            return Err(GraphError::InvalidConfig(format!(
                "modulatory weight {} out of range [0.0, 1.0]",
                self.modulatory
            )));
        }

        Ok(())
    }

    /// Check if weights are valid without returning detailed error
    ///
    /// # Returns
    /// * `true` if all weights are in [0.0, 1.0]
    /// * `false` otherwise
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Calculate net activation using canonical formula
    ///
    /// # CANONICAL FORMULA
    /// ```text
    /// net_activation = excitatory - inhibitory + (modulatory * 0.5)
    /// ```
    ///
    /// # Returns
    /// Net activation value in range [-1.5, 1.5] for valid weights
    ///
    /// # Note
    /// Does not validate weights. Call validate() first if unsure.
    ///
    /// # Example
    /// ```
    /// let weights = NeurotransmitterWeights::new(0.7, 0.3, 0.2);
    /// let activation = weights.net_activation();
    /// // activation = 0.7 - 0.3 + (0.2 * 0.5) = 0.5
    /// assert!((activation - 0.5).abs() < 1e-6);
    /// ```
    pub fn net_activation(&self) -> f32 {
        self.excitatory - self.inhibitory + (self.modulatory * 0.5)
    }

    /// Calculate net activation with validation
    ///
    /// # Returns
    /// * `Ok(f32)` - Net activation if weights are valid
    /// * `Err(GraphError)` - Validation error if weights are invalid
    pub fn net_activation_validated(&self) -> GraphResult<f32> {
        self.validate()?;
        Ok(self.net_activation())
    }

    /// Get the theoretical minimum net activation
    ///
    /// With valid weights: 0.0 - 1.0 + (0.0 * 0.5) = -1.0
    pub const MIN_NET_ACTIVATION: f32 = -1.0;

    /// Get the theoretical maximum net activation
    ///
    /// With valid weights: 1.0 - 0.0 + (1.0 * 0.5) = 1.5
    pub const MAX_NET_ACTIVATION: f32 = 1.5;

    /// Normalize net activation to [0, 1] range
    ///
    /// Maps [-1.0, 1.5] to [0.0, 1.0]
    pub fn normalized_activation(&self) -> f32 {
        let raw = self.net_activation();
        // Linear mapping from [-1.0, 1.5] to [0.0, 1.0]
        // normalized = (raw - min) / (max - min)
        // normalized = (raw - (-1.0)) / (1.5 - (-1.0))
        // normalized = (raw + 1.0) / 2.5
        (raw + 1.0) / 2.5
    }

    /// Create weights that produce a target net activation
    ///
    /// Uses balanced excitatory/inhibitory with modulatory adjustment.
    /// Only works for activations in [-1.0, 1.5].
    ///
    /// # Arguments
    /// * `target` - Desired net activation value
    ///
    /// # Returns
    /// * `Some(weights)` if target is achievable with valid weights
    /// * `None` if target is outside achievable range
    pub fn from_activation(target: f32) -> Option<Self> {
        if target < Self::MIN_NET_ACTIVATION || target > Self::MAX_NET_ACTIVATION {
            return None;
        }

        // Strategy: Use modulatory to reach target
        // If target >= 0: excitatory=0.5, inhibitory=0.0, modulatory=(target-0.5)/0.5
        // If target < 0: excitatory=0.0, inhibitory=-target, modulatory=0.0

        if target >= 0.0 {
            // Positive activation
            if target <= 0.5 {
                // Achievable with just excitatory
                Some(Self::new(target + 0.5, 0.5, 0.0))
            } else {
                // Need modulatory help
                let base = 0.5; // excitatory - inhibitory = 0.5 - 0.0
                let needed_from_mod = target - base;
                let modulatory = (needed_from_mod / 0.5).min(1.0);
                let remaining = target - base - (modulatory * 0.5);

                Some(Self::new((0.5 + remaining).min(1.0), 0.0, modulatory))
            }
        } else {
            // Negative activation
            let inhibitory = (-target).min(1.0);
            Some(Self::new(0.0, inhibitory, 0.0))
        }
    }
}

#[cfg(test)]
mod validation_tests {
    use super::*;

    #[test]
    fn test_validate_valid() {
        let weights = NeurotransmitterWeights::new(0.5, 0.5, 0.0);
        assert!(weights.validate().is_ok());
    }

    #[test]
    fn test_validate_boundary() {
        // Boundary values should be valid
        let lower = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
        assert!(lower.validate().is_ok());

        let upper = NeurotransmitterWeights::new(1.0, 1.0, 1.0);
        assert!(upper.validate().is_ok());
    }

    #[test]
    fn test_validate_negative_excitatory() {
        let weights = NeurotransmitterWeights::new(-0.1, 0.5, 0.0);
        let err = weights.validate().unwrap_err();
        match err {
            GraphError::InvalidConfig(msg) => {
                assert!(msg.contains("excitatory"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_validate_over_one_inhibitory() {
        let weights = NeurotransmitterWeights::new(0.5, 1.1, 0.0);
        let err = weights.validate().unwrap_err();
        match err {
            GraphError::InvalidConfig(msg) => {
                assert!(msg.contains("inhibitory"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_validate_over_one_modulatory() {
        let weights = NeurotransmitterWeights::new(0.5, 0.5, 1.5);
        let err = weights.validate().unwrap_err();
        match err {
            GraphError::InvalidConfig(msg) => {
                assert!(msg.contains("modulatory"));
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_net_activation_formula() {
        // Test canonical formula: excitatory - inhibitory + (modulatory * 0.5)
        let weights = NeurotransmitterWeights::new(0.7, 0.3, 0.2);
        let activation = weights.net_activation();
        // 0.7 - 0.3 + (0.2 * 0.5) = 0.4 + 0.1 = 0.5
        assert!((activation - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_net_activation_neutral() {
        let weights = NeurotransmitterWeights::default();
        let activation = weights.net_activation();
        // 0.5 - 0.5 + (0.0 * 0.5) = 0.0
        assert!(activation.abs() < 1e-6);
    }

    #[test]
    fn test_net_activation_max() {
        let weights = NeurotransmitterWeights::max_activation();
        let activation = weights.net_activation();
        // 1.0 - 0.0 + (1.0 * 0.5) = 1.5
        assert!((activation - 1.5).abs() < 1e-6);
    }

    #[test]
    fn test_net_activation_min() {
        let weights = NeurotransmitterWeights::min_activation();
        let activation = weights.net_activation();
        // 0.0 - 1.0 + (0.0 * 0.5) = -1.0
        assert!((activation - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_net_activation_range() {
        // For all valid weights, activation should be in [-1.5, 1.5]
        // Actually min is -1.0 (e=0, i=1, m=0), max is 1.5 (e=1, i=0, m=1)
        for e in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for i in [0.0, 0.25, 0.5, 0.75, 1.0] {
                for m in [0.0, 0.25, 0.5, 0.75, 1.0] {
                    let w = NeurotransmitterWeights::new(e, i, m);
                    let a = w.net_activation();
                    assert!(a >= -1.5 && a <= 1.5,
                        "Activation {} out of range for ({}, {}, {})", a, e, i, m);
                }
            }
        }
    }

    #[test]
    fn test_normalized_activation() {
        let neutral = NeurotransmitterWeights::default();
        let norm = neutral.normalized_activation();
        // Raw: 0.0, mapped to (0 + 1) / 2.5 = 0.4
        assert!((norm - 0.4).abs() < 1e-6);

        let max = NeurotransmitterWeights::max_activation();
        let norm = max.normalized_activation();
        // Raw: 1.5, mapped to (1.5 + 1) / 2.5 = 1.0
        assert!((norm - 1.0).abs() < 1e-6);

        let min = NeurotransmitterWeights::min_activation();
        let norm = min.normalized_activation();
        // Raw: -1.0, mapped to (-1 + 1) / 2.5 = 0.0
        assert!(norm.abs() < 1e-6);
    }

    #[test]
    fn test_is_valid() {
        assert!(NeurotransmitterWeights::default().is_valid());
        assert!(!NeurotransmitterWeights::new(-0.1, 0.5, 0.0).is_valid());
    }
}
```

### Constraints
- validate() returns Err for any weight < 0 or > 1
- Error message must specify which weight is invalid
- net_activation() uses CANONICAL formula exactly
- net_activation() result is in [-1.5, 1.5] for valid weights
- No auto-clamping - fail fast on invalid weights

### Acceptance Criteria
- [ ] validate() returns Ok(()) for weights in [0,1]
- [ ] validate() returns Err for weight < 0
- [ ] validate() returns Err for weight > 1
- [ ] net_activation() = excitatory - inhibitory + (modulatory * 0.5)
- [ ] net_activation() result is in [-1.5, 1.5] range
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. validate():
   - Check excitatory in [0, 1], return error if not
   - Check inhibitory in [0, 1], return error if not
   - Check modulatory in [0, 1], return error if not
   - Return Ok(())

2. net_activation():
   - Apply formula: excitatory - inhibitory + (modulatory * 0.5)
   - Return result (no validation, no clamping)

### Edge Cases
- NaN weights: Comparison will fail, return error
- Infinity weights: Comparison will correctly catch
- Boundary values (0.0, 1.0): Must be accepted

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph validation
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Boundary values (0.0, 1.0) pass validation
- [ ] Negative values fail with correct error message
- [ ] Values > 1.0 fail with correct error message
- [ ] net_activation matches formula exactly
