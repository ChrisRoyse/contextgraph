---
id: "M04-T02"
title: "Define HyperbolicConfig for Poincare Ball"
description: |
  Implement HyperbolicConfig struct for 64D Poincare ball model.
  Fields: dim (64), curvature (-1.0), eps (1e-7), max_norm (1.0 - 1e-5).
  NOTE: Curvature MUST be negative (validated in M04-T02a).
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 1
sequence: 4
depends_on:
  - "M04-T00"
spec_refs:
  - "TECH-GRAPH-004 Section 5"
  - "REQ-KG-050, REQ-KG-054"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/config.rs"
    description: "Add HyperbolicConfig struct"
test_file: "crates/context-graph-graph/tests/config_tests.rs"
---

## Context

HyperbolicConfig defines parameters for the Poincare ball model used in hyperbolic geometry operations. The Poincare ball is an n-dimensional model of hyperbolic space where all points lie within the unit ball (||x|| < 1). This geometry naturally encodes hierarchical relationships, making it ideal for IS-A hierarchies in knowledge graphs.

## Scope

### In Scope
- Define HyperbolicConfig struct with 4 fields
- Implement Default trait
- Add Serde serialization
- Document each field's purpose

### Out of Scope
- Validation of curvature (see M04-T02a)
- PoincarePoint struct (see M04-T04)
- Mobius operations (see M04-T05)

## Definition of Done

### Signatures

```rust
use serde::{Deserialize, Serialize};

/// Configuration for Poincare ball hyperbolic space model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicConfig {
    /// Dimension of hyperbolic space (typically 64)
    pub dim: usize,
    /// Curvature of hyperbolic space (MUST be negative)
    pub curvature: f32,
    /// Epsilon for numerical stability
    pub eps: f32,
    /// Maximum norm for points (keeps points inside ball boundary)
    pub max_norm: f32,
}

impl Default for HyperbolicConfig {
    fn default() -> Self {
        Self {
            dim: 64,
            curvature: -1.0,
            eps: 1e-7,
            max_norm: 1.0 - 1e-5,
        }
    }
}

impl HyperbolicConfig {
    /// Create a new config with custom curvature
    pub fn with_curvature(curvature: f32) -> Self {
        Self {
            curvature,
            ..Default::default()
        }
    }

    /// Get the absolute value of curvature
    pub fn abs_curvature(&self) -> f32 {
        self.curvature.abs()
    }
}
```

### Constraints
- dim MUST be positive (typically 64)
- curvature MUST be negative (enforced in M04-T02a)
- eps MUST be positive and small
- max_norm MUST be in (0, 1) to keep points inside ball

### Acceptance Criteria
- [ ] HyperbolicConfig struct with 4 fields
- [ ] Default returns dim=64, curvature=-1.0
- [ ] max_norm ensures points stay within ball boundary
- [ ] eps prevents numerical instability
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define struct with 4 fields
2. Derive Debug, Clone, Serialize, Deserialize
3. Implement Default with spec values
4. Add helper methods (with_curvature, abs_curvature)

### Edge Cases
- curvature = 0: Degenerates to Euclidean space (invalid for hyperbolic)
- curvature > 0: Becomes spherical geometry (invalid for Poincare ball)
- max_norm >= 1: Points could be outside or on ball boundary (invalid)

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph config
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Default values match specification
- [ ] JSON serialization/deserialization roundtrips correctly
- [ ] abs_curvature() returns positive value
