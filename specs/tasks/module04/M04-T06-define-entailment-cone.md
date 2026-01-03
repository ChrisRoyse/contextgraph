---
id: "M04-T06"
title: "Define EntailmentCone Struct"
description: |
  Implement EntailmentCone struct for O(1) IS-A hierarchy queries.
  Fields: apex (PoincarePoint), aperture (f32), aperture_factor (f32), depth (u32).
  Include methods: new(), effective_aperture(), contains(), membership_score().
  Constraint: aperture in [0, pi/2], aperture_factor in [0.5, 2.0].
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 3
sequence: 9
depends_on:
  - "M04-T03"
  - "M04-T05"
spec_refs:
  - "TECH-GRAPH-004 Section 6"
  - "REQ-KG-052, REQ-KG-053"
files_to_create:
  - path: "crates/context-graph-graph/src/entailment/cones.rs"
    description: "EntailmentCone struct and methods"
files_to_modify:
  - path: "crates/context-graph-graph/src/entailment/mod.rs"
    description: "Add cones module"
test_file: "crates/context-graph-graph/tests/entailment_tests.rs"
---

## Context

EntailmentCone represents a cone in hyperbolic space rooted at a concept node. The cone's interior contains all concepts that are subsumed by (entailed by) the root concept. This enables O(1) IS-A hierarchy queries: to check if concept A is a subconcept of B, simply check if A's position lies within B's cone. The aperture decreases with hierarchy depth - broader cones for general concepts, narrower cones for specific ones.

## Scope

### In Scope
- Define EntailmentCone struct with 4 fields
- Implement new() constructor from ConeConfig
- Implement effective_aperture() method
- Stub contains() and membership_score() (implemented in M04-T07)
- Add Serde serialization (268 bytes expected)

### Out of Scope
- Full containment logic (see M04-T07)
- Training/adaptation (see M04-T07)
- CUDA kernels (see M04-T24)

## Definition of Done

### Signatures

```rust
use serde::{Deserialize, Serialize};
use crate::hyperbolic::poincare::PoincarePoint;
use crate::config::ConeConfig;

/// Entailment cone for O(1) IS-A hierarchy queries
///
/// A cone rooted at `apex` with angular width `aperture * aperture_factor`
/// contains all points (concepts) that are entailed by the apex concept.
///
/// Serialized size: 268 bytes (256 coords + 4 aperture + 4 factor + 4 depth)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EntailmentCone {
    /// Apex point of the cone in Poincare ball
    pub apex: PoincarePoint,
    /// Base aperture in radians (computed from depth)
    pub aperture: f32,
    /// Adjustment factor for aperture (learned during training)
    pub aperture_factor: f32,
    /// Depth in hierarchy (0 = root concept)
    pub depth: u32,
}

impl EntailmentCone {
    /// Create a new entailment cone at given apex position
    ///
    /// # Arguments
    /// * `apex` - Position in Poincare ball
    /// * `depth` - Hierarchy depth (affects aperture)
    /// * `config` - ConeConfig for aperture computation
    pub fn new(apex: PoincarePoint, depth: u32, config: &ConeConfig) -> Self {
        let aperture = config.compute_aperture(depth);
        Self {
            apex,
            aperture,
            aperture_factor: 1.0,
            depth,
        }
    }

    /// Create cone with custom aperture (for deserialization/testing)
    pub fn with_aperture(apex: PoincarePoint, aperture: f32, depth: u32) -> Self {
        Self {
            apex,
            aperture,
            aperture_factor: 1.0,
            depth,
        }
    }

    /// Get the effective aperture after applying adjustment factor
    ///
    /// Result is clamped to valid range [0, pi/2]
    pub fn effective_aperture(&self) -> f32 {
        let effective = self.aperture * self.aperture_factor;
        effective.clamp(0.0, std::f32::consts::FRAC_PI_2)
    }

    /// Check if a point is contained within this cone
    ///
    /// Performance target: <50us
    ///
    /// Note: Full implementation in M04-T07
    pub fn contains(&self, _point: &PoincarePoint, _ball: &crate::hyperbolic::mobius::PoincareBall) -> bool {
        // Implemented in M04-T07
        todo!("Containment logic implemented in M04-T07")
    }

    /// Compute soft membership score for a point
    ///
    /// Returns 1.0 if contained, exponentially decaying value if outside.
    ///
    /// CANONICAL FORMULA:
    /// - If contained: score = 1.0
    /// - If not contained: score = exp(-2.0 * (angle - aperture))
    ///
    /// Note: Full implementation in M04-T07
    pub fn membership_score(&self, _point: &PoincarePoint, _ball: &crate::hyperbolic::mobius::PoincareBall) -> f32 {
        // Implemented in M04-T07
        todo!("Membership score implemented in M04-T07")
    }

    /// Update aperture factor based on training signal
    ///
    /// Note: Full implementation in M04-T07
    pub fn update_aperture(&mut self, _delta: f32) {
        // Implemented in M04-T07
        todo!("Update aperture implemented in M04-T07")
    }

    /// Validate cone parameters
    pub fn is_valid(&self) -> bool {
        self.apex.is_valid()
            && self.aperture > 0.0
            && self.aperture <= std::f32::consts::FRAC_PI_2
            && self.aperture_factor >= 0.5
            && self.aperture_factor <= 2.0
    }
}

impl Default for EntailmentCone {
    fn default() -> Self {
        Self {
            apex: PoincarePoint::origin(),
            aperture: 1.0,
            aperture_factor: 1.0,
            depth: 0,
        }
    }
}
```

### Constraints
- apex MUST be valid PoincarePoint (||coords|| < 1)
- aperture in [0, pi/2] radians
- aperture_factor in [0.5, 2.0] for bounded adjustment
- depth >= 0
- Serialized size: 268 bytes

### Acceptance Criteria
- [ ] EntailmentCone struct with apex, aperture, aperture_factor, depth
- [ ] new() computes aperture from depth using ConeConfig
- [ ] effective_aperture() = aperture * aperture_factor
- [ ] contains() returns bool in <50us (stub for now)
- [ ] membership_score() returns soft [0,1] score (stub for now)
- [ ] Serde serialization produces 268 bytes (256 coords + 4 aperture + 4 factor + 4 depth)
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define struct with 4 fields
2. Implement new(apex, depth, config):
   - aperture = config.compute_aperture(depth)
   - aperture_factor = 1.0
3. Implement effective_aperture():
   - return (aperture * aperture_factor).clamp(0, pi/2)
4. Add todo!() stubs for contains, membership_score, update_aperture

### Edge Cases
- depth = 0: Uses base_aperture (widest cone)
- Very deep node: Uses min_aperture (narrowest cone)
- apex at origin: Special case, cone axis is undefined
- aperture_factor at bounds: Clamp effective_aperture

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph entailment
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] new() with depth=0 creates cone with base_aperture
- [ ] effective_aperture() respects aperture_factor
- [ ] is_valid() returns true for properly constructed cones
- [ ] Serialization roundtrip preserves all fields
- [ ] size_of serialized data is 268 bytes
