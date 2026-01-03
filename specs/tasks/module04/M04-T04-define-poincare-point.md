---
id: "M04-T04"
title: "Define PoincarePoint for 64D Hyperbolic Space"
description: |
  Implement PoincarePoint struct with coords: [f32; 64].
  Constraint: ||coords|| < 1.0 (strict inequality for Poincare ball).
  Include methods: origin(), norm_squared(), norm(), project(&HyperbolicConfig).
  Use #[repr(C, align(64))] for SIMD optimization.
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 2
sequence: 7
depends_on:
  - "M04-T02"
spec_refs:
  - "TECH-GRAPH-004 Section 5.1"
  - "REQ-KG-050"
files_to_create:
  - path: "crates/context-graph-graph/src/hyperbolic/poincare.rs"
    description: "PoincarePoint struct implementation"
files_to_modify:
  - path: "crates/context-graph-graph/src/hyperbolic/mod.rs"
    description: "Add poincare module"
test_file: "crates/context-graph-graph/tests/poincare_tests.rs"
---

## Context

PoincarePoint represents a point in the 64-dimensional Poincare ball model of hyperbolic space. All points must lie strictly inside the unit ball (||x|| < 1). The Poincare ball model is particularly useful for encoding hierarchical structures because points near the boundary represent leaves while points near the center represent roots.

## Scope

### In Scope
- Define PoincarePoint struct with [f32; 64] array
- Implement origin(), norm_squared(), norm() methods
- Implement project() method to keep points inside ball
- Use #[repr(C, align(64))] for SIMD/cache optimization
- Implement Clone, Debug, PartialEq

### Out of Scope
- Mobius operations (see M04-T05)
- Distance calculations (see M04-T05)
- CUDA kernels (see M04-T23)

## Definition of Done

### Signatures

```rust
use crate::config::HyperbolicConfig;

/// Point in 64-dimensional Poincare ball model of hyperbolic space
///
/// Constraint: ||coords|| < 1.0 (strictly inside unit ball)
///
/// Memory layout optimized for SIMD operations with 64-byte alignment
#[repr(C, align(64))]
#[derive(Clone, Debug)]
pub struct PoincarePoint {
    /// Coordinates in 64-dimensional Euclidean embedding space
    pub coords: [f32; 64],
}

impl Default for PoincarePoint {
    fn default() -> Self {
        Self::origin()
    }
}

impl PoincarePoint {
    /// Create the origin point (all zeros)
    /// The origin is the center of the Poincare ball
    pub fn origin() -> Self {
        Self { coords: [0.0; 64] }
    }

    /// Create a point from coordinates
    /// Note: Does NOT validate norm - call project() if needed
    pub fn from_coords(coords: [f32; 64]) -> Self {
        Self { coords }
    }

    /// Compute squared Euclidean norm of coordinates
    /// More efficient than norm() when comparing magnitudes
    pub fn norm_squared(&self) -> f32 {
        self.coords.iter().map(|x| x * x).sum()
    }

    /// Compute Euclidean norm of coordinates
    pub fn norm(&self) -> f32 {
        self.norm_squared().sqrt()
    }

    /// Project point to stay strictly inside the Poincare ball
    ///
    /// If ||coords|| >= max_norm, rescale to ||coords|| = max_norm
    /// This prevents numerical instability at the boundary
    pub fn project(&mut self, config: &HyperbolicConfig) {
        let norm = self.norm();
        if norm >= config.max_norm {
            let scale = config.max_norm / norm;
            for c in &mut self.coords {
                *c *= scale;
            }
        }
    }

    /// Create a projected copy without modifying self
    pub fn projected(&self, config: &HyperbolicConfig) -> Self {
        let mut result = self.clone();
        result.project(config);
        result
    }

    /// Check if point is valid (inside unit ball)
    pub fn is_valid(&self) -> bool {
        self.norm_squared() < 1.0
    }
}

impl PartialEq for PoincarePoint {
    fn eq(&self, other: &Self) -> bool {
        self.coords == other.coords
    }
}
```

### Constraints
- Fixed 64 dimensions (compile-time constant)
- Memory alignment 64 bytes for cache line optimization
- All coordinates must be f32
- Norm must be strictly < 1.0 for valid points

### Acceptance Criteria
- [ ] PoincarePoint struct with [f32; 64] array
- [ ] origin() returns all zeros
- [ ] norm() computes Euclidean norm
- [ ] project() rescales if norm >= max_norm
- [ ] Memory alignment 64 bytes for cache efficiency
- [ ] Clone, Debug traits implemented
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define struct with #[repr(C, align(64))]
2. Implement origin(): return [0.0; 64]
3. Implement norm_squared(): sum of squares
4. Implement norm(): sqrt(norm_squared())
5. Implement project():
   - if norm >= max_norm: scale = max_norm / norm; multiply all coords by scale

### Edge Cases
- Point at origin: norm = 0, no projection needed
- Point exactly at boundary: project to max_norm
- Point outside ball: project to max_norm
- All coords = 0 except one: simple 1D case

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph poincare
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] origin().norm() == 0.0
- [ ] Point with coords [0.5, 0, ..., 0] has norm 0.5
- [ ] Point with norm > 1.0 is projected correctly
- [ ] size_of::<PoincarePoint>() is a multiple of 64
