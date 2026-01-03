---
id: "M04-T02a"
title: "Implement Curvature Validation for HyperbolicConfig"
description: |
  Add validate() method to HyperbolicConfig that ensures curvature < 0.
  Returns Result<(), GraphError::InvalidConfig> if curvature >= 0.
  Also validates: dim > 0, eps > 0, max_norm in (0, 1).
layer: "foundation"
status: "pending"
priority: "high"
estimated_hours: 0.5
sequence: 5
depends_on:
  - "M04-T02"
spec_refs:
  - "TECH-GRAPH-004 Section 5"
  - "REQ-KG-054"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/config.rs"
    description: "Add validate() method to HyperbolicConfig"
test_file: "crates/context-graph-graph/tests/config_tests.rs"
---

## Context

Validation ensures HyperbolicConfig parameters are mathematically valid for the Poincare ball model. Negative curvature is essential - positive or zero curvature would represent different geometries (spherical or Euclidean). This validation prevents subtle mathematical errors that could lead to incorrect distance calculations or containment checks.

## Scope

### In Scope
- Add validate() method to HyperbolicConfig
- Validate curvature < 0
- Validate dim > 0
- Validate eps > 0
- Validate max_norm in (0, 1)
- Return appropriate GraphError variants

### Out of Scope
- GraphError definition (see M04-T08)
- Automatic validation on construction

## Definition of Done

### Signatures

```rust
use crate::error::GraphError;

impl HyperbolicConfig {
    /// Validate that all configuration parameters are mathematically valid
    ///
    /// # Errors
    /// Returns `GraphError::InvalidConfig` if:
    /// - curvature >= 0 (must be negative for hyperbolic space)
    /// - dim == 0 (dimension must be positive)
    /// - eps <= 0 (epsilon must be positive)
    /// - max_norm <= 0 or max_norm >= 1 (must be in open interval (0, 1))
    pub fn validate(&self) -> Result<(), GraphError> {
        if self.curvature >= 0.0 {
            return Err(GraphError::InvalidConfig(
                format!("Curvature must be negative, got {}", self.curvature)
            ));
        }
        if self.dim == 0 {
            return Err(GraphError::InvalidConfig(
                "Dimension must be positive".to_string()
            ));
        }
        if self.eps <= 0.0 {
            return Err(GraphError::InvalidConfig(
                format!("Epsilon must be positive, got {}", self.eps)
            ));
        }
        if self.max_norm <= 0.0 || self.max_norm >= 1.0 {
            return Err(GraphError::InvalidConfig(
                format!("max_norm must be in (0, 1), got {}", self.max_norm)
            ));
        }
        Ok(())
    }
}
```

### Constraints
- Must return GraphError::InvalidConfig for validation failures
- Error messages must be descriptive (include actual value)
- All checks must be performed (not short-circuit on first error)

### Acceptance Criteria
- [ ] validate() returns Ok(()) for valid config
- [ ] validate() returns Err for curvature >= 0
- [ ] validate() returns Err for dim == 0
- [ ] validate() returns Err for eps <= 0
- [ ] validate() returns Err for max_norm not in (0,1)
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Check curvature < 0
2. Check dim > 0
3. Check eps > 0
4. Check 0 < max_norm < 1
5. Return Ok(()) if all pass, Err with first failure otherwise

### Edge Cases
- curvature = 0.0: Invalid (Euclidean, not hyperbolic)
- curvature = f32::INFINITY: Invalid (should fail < 0 check)
- curvature = f32::NAN: Should fail the comparison
- max_norm = 1.0: Invalid (boundary is not part of ball)

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph config
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Default config passes validation
- [ ] Config with curvature = 0.0 fails
- [ ] Config with curvature = 1.0 fails
- [ ] Config with dim = 0 fails
- [ ] Config with eps = 0.0 fails
- [ ] Config with max_norm = 1.0 fails
- [ ] Config with max_norm = 0.0 fails
