---
id: "M04-T27"
title: "Fix Containment Formula Conflicts"
description: |
  Ensure consistent containment formula across all implementations.
  Three conflicting formulas were found during analysis.

  CANONICAL FORMULA (use everywhere):
  - Compute angle between point direction and cone axis
  - If angle <= effective_aperture: contained, score = 1.0
  - If angle > effective_aperture: not contained, score = exp(-2.0 * (angle - aperture))

  Update M04-T07 implementation and all test cases to use this formula.
layer: "surface"
status: "pending"
priority: "medium"
estimated_hours: 2
sequence: 34
depends_on:
  - "M04-T18"
  - "M04-T19"
  - "M04-T20"
spec_refs:
  - "TECH-GRAPH-004 Section 6"
  - "REQ-KG-053"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/entailment/cones.rs"
    description: "Ensure canonical formula in contains() and membership_score()"
  - path: "crates/context-graph-graph/tests/entailment_tests.rs"
    description: "Update test expected values to match canonical formula"
test_file: "crates/context-graph-graph/tests/entailment_tests.rs"
---

## Context

During the Module 4 analysis, three different formulas for entailment cone membership were found in different parts of the specification and existing code:

1. **Original Spec Formula**: Uses hyperbolic distance directly
2. **Implementation 1**: Uses angle from log_map with linear decay
3. **Implementation 2**: Uses angle with exponential decay

This task establishes a single CANONICAL FORMULA that MUST be used everywhere:

**CANONICAL MEMBERSHIP SCORE FORMULA:**
```
tangent = log_map(apex, point)
to_origin = log_map(apex, origin)
angle = arccos(dot(tangent, to_origin) / (||tangent|| * ||to_origin||))

if angle <= effective_aperture:
    score = 1.0  // Fully contained
else:
    score = exp(-2.0 * (angle - aperture))  // Exponential decay outside
```

This formula was chosen because:
- It provides smooth gradient for training
- Exponential decay prevents hard boundaries
- -2.0 decay rate balances precision and recall
- Compatible with both CPU and GPU implementations

## Scope

### In Scope
- Audit all containment formula usages
- Update EntailmentCone::contains() implementation
- Update EntailmentCone::membership_score() implementation
- Update all test expected values
- Document canonical formula in code comments

### Out of Scope
- CUDA kernel formula (covered in M04-T24)
- Query builder API (separate concern)
- Cone training/aperture learning

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/entailment/cones.rs

impl EntailmentCone {
    /// Check if a point is contained within this entailment cone
    ///
    /// CANONICAL FORMULA:
    /// 1. Compute tangent from apex to point via log_map
    /// 2. Compute tangent from apex to origin (cone axis direction)
    /// 3. Compute angle between these tangent vectors
    /// 4. Return true if angle <= effective_aperture
    ///
    /// Performance: O(d) where d = dimension, target <50us
    ///
    /// # Arguments
    /// * `point` - The Poincare point to check
    /// * `ball` - The Poincare ball for geodesic operations
    ///
    /// # Returns
    /// * `true` if point is within the cone
    pub fn contains(&self, point: &PoincarePoint, ball: &PoincareBall) -> bool {
        let angle = self.compute_angle(point, ball);
        angle <= self.effective_aperture()
    }

    /// Compute soft membership score for a point
    ///
    /// CANONICAL FORMULA:
    /// - If angle <= effective_aperture: score = 1.0
    /// - If angle > effective_aperture: score = exp(-2.0 * (angle - aperture))
    ///
    /// The exponential decay provides:
    /// - Smooth gradient for training
    /// - Score in (0, 1] range
    /// - Higher scores for points closer to cone interior
    ///
    /// Decay rate of -2.0 was empirically chosen for good precision/recall balance.
    ///
    /// # Arguments
    /// * `point` - The Poincare point to score
    /// * `ball` - The Poincare ball for geodesic operations
    ///
    /// # Returns
    /// * Score in (0, 1] where 1.0 = fully contained
    pub fn membership_score(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let angle = self.compute_angle(point, ball);
        let aperture = self.effective_aperture();

        if angle <= aperture {
            // Inside cone: full membership
            1.0
        } else {
            // Outside cone: exponential decay
            // CANONICAL: exp(-2.0 * (angle - aperture))
            (-2.0 * (angle - aperture)).exp()
        }
    }

    /// Compute angle between point direction and cone axis
    ///
    /// This is the core geometric computation used by both
    /// contains() and membership_score().
    ///
    /// Algorithm:
    /// 1. Compute log_map(apex, point) - tangent toward point
    /// 2. Compute log_map(apex, origin) - tangent toward origin (axis)
    /// 3. Return arccos of normalized dot product
    fn compute_angle(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        // Handle special case: point at apex
        if self.apex.distance_squared(point) < 1e-10 {
            return 0.0;  // At apex, angle is 0
        }

        // Compute tangent from apex to point
        let tangent_to_point = ball.log_map(&self.apex, point);

        // Compute tangent from apex to origin (cone axis direction)
        let origin = PoincarePoint::origin();
        let tangent_to_origin = ball.log_map(&self.apex, &origin);

        // Handle special case: apex at origin
        let origin_norm = tangent_to_origin.norm();
        if origin_norm < 1e-10 {
            // Apex at origin: cone opens toward all points equally
            // Return 0 (all points are "along the axis")
            return 0.0;
        }

        let point_norm = tangent_to_point.norm();
        if point_norm < 1e-10 {
            return 0.0;
        }

        // Compute cosine of angle
        let dot = tangent_to_point.dot(&tangent_to_origin);
        let cos_angle = dot / (point_norm * origin_norm);

        // Clamp to valid range (numerical stability)
        let cos_angle = cos_angle.max(-1.0).min(1.0);

        cos_angle.acos()
    }

    /// Get effective aperture (base aperture * adjustment factor)
    pub fn effective_aperture(&self) -> f32 {
        self.aperture * self.aperture_factor
    }

    /// Update aperture factor based on training signal
    ///
    /// # Arguments
    /// * `delta` - Adjustment to aperture_factor
    /// * `learning_rate` - Step size for update
    pub fn update_aperture(&mut self, delta: f32, learning_rate: f32) {
        self.aperture_factor += delta * learning_rate;
        // Clamp to valid range
        self.aperture_factor = self.aperture_factor.max(0.5).min(2.0);
    }
}

/// Decay rate for membership score outside cone
///
/// CANONICAL VALUE: 2.0
/// - Higher values = steeper decay (more selective)
/// - Lower values = gentler decay (more inclusive)
pub const MEMBERSHIP_DECAY_RATE: f32 = 2.0;

/// Compute membership score using canonical formula (standalone function)
///
/// Useful for batch operations where cone struct is not available.
pub fn canonical_membership_score(
    angle: f32,
    aperture: f32,
) -> f32 {
    if angle <= aperture {
        1.0
    } else {
        (-MEMBERSHIP_DECAY_RATE * (angle - aperture)).exp()
    }
}
```

### Constraints
- MUST use exact canonical formula (no variations)
- MUST handle edge cases (apex at origin, point at apex)
- MUST document formula in code comments
- MUST update all test expected values

### Acceptance Criteria
- [ ] Single canonical formula used in contains()
- [ ] Single canonical formula used in membership_score()
- [ ] All tests use consistent expected values
- [ ] Documentation updated with formula
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Formula Derivation

The membership score formula creates a smooth transition at the cone boundary:

```
score
  1.0 |=========\
      |          \
  0.5 |           \___
      |               \_____
  0.0 |                     \________
      +-----------------------------------> angle
            aperture
```

At the boundary (angle = aperture):
- contains() returns false (strict)
- membership_score() returns 1.0 (soft boundary)

Just outside (angle = aperture + 0.5):
- membership_score() = exp(-2.0 * 0.5) = exp(-1.0) = 0.368

Far outside (angle = aperture + 1.0):
- membership_score() = exp(-2.0 * 1.0) = exp(-2.0) = 0.135

### Audit Checklist

Files to audit for formula consistency:
1. `crates/context-graph-graph/src/entailment/cones.rs` - Main implementation
2. `crates/context-graph-cuda/kernels/cone_check.cu` - CUDA kernel
3. `crates/context-graph-graph/tests/entailment_tests.rs` - Unit tests
4. `crates/context-graph-graph/tests/integration_tests.rs` - Integration tests

### Edge Cases

| Case | Angle | Score |
|------|-------|-------|
| Point at apex | 0.0 | 1.0 |
| Point on axis inside | < aperture | 1.0 |
| Point on boundary | = aperture | 1.0 (soft) |
| Point just outside | aperture + 0.1 | ~0.82 |
| Point far outside | aperture + 2.0 | ~0.02 |
| Apex at origin | 0.0 (special) | 1.0 |

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph entailment
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Formula matches spec exactly
- [ ] Edge cases handled correctly
- [ ] GPU kernel uses same formula
- [ ] All tests pass with updated expected values

### Test Cases

```rust
#[cfg(test)]
mod canonical_formula_tests {
    use super::*;

    #[test]
    fn test_canonical_formula_inside_cone() {
        let apex = PoincarePoint::from_coords(&[0.1; 64]);
        let cone = EntailmentCone::new(apex, 0.8, 1.0, 0);
        let ball = PoincareBall::new(&HyperbolicConfig::default());

        // Point that should be inside (angle < 0.8)
        let point = make_point_at_angle(&cone.apex, 0.5);

        assert!(cone.contains(&point, &ball));
        assert!((cone.membership_score(&point, &ball) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_canonical_formula_on_boundary() {
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::new(apex, 0.5, 1.0, 0);
        let ball = PoincareBall::new(&HyperbolicConfig::default());

        // Point exactly on boundary
        let point = make_point_at_angle(&cone.apex, 0.5);

        // Soft boundary: membership score should be 1.0
        let score = cone.membership_score(&point, &ball);
        assert!((score - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_canonical_formula_outside_cone() {
        let apex = PoincarePoint::origin();
        let aperture = 0.5f32;
        let cone = EntailmentCone::new(apex, aperture, 1.0, 0);
        let ball = PoincareBall::new(&HyperbolicConfig::default());

        // Point outside (angle = 0.7)
        let angle = 0.7f32;
        let point = make_point_at_angle(&cone.apex, angle);

        assert!(!cone.contains(&point, &ball));

        // CANONICAL: exp(-2.0 * (0.7 - 0.5)) = exp(-0.4)
        let expected = (-2.0f32 * (angle - aperture)).exp();
        let score = cone.membership_score(&point, &ball);

        assert!((score - expected).abs() < 0.1,
            "Score {} expected {}", score, expected);
    }

    #[test]
    fn test_canonical_formula_decay_rate() {
        // Verify decay rate of 2.0 produces expected values
        assert!((canonical_membership_score(0.6, 0.5) - 0.8187).abs() < 0.01);  // exp(-0.2)
        assert!((canonical_membership_score(1.0, 0.5) - 0.3679).abs() < 0.01);  // exp(-1.0)
        assert!((canonical_membership_score(1.5, 0.5) - 0.1353).abs() < 0.01);  // exp(-2.0)
    }

    #[test]
    fn test_point_at_apex() {
        let apex = PoincarePoint::from_coords(&[0.2; 64]);
        let cone = EntailmentCone::new(apex.clone(), 0.5, 1.0, 0);
        let ball = PoincareBall::new(&HyperbolicConfig::default());

        // Point exactly at apex
        let score = cone.membership_score(&apex, &ball);
        assert!((score - 1.0).abs() < 1e-6, "Point at apex should have score 1.0");
    }

    #[test]
    fn test_apex_at_origin() {
        let apex = PoincarePoint::origin();
        let cone = EntailmentCone::new(apex, 0.5, 1.0, 0);
        let ball = PoincareBall::new(&HyperbolicConfig::default());

        // Any point should be handled gracefully
        let point = PoincarePoint::from_coords(&[0.1; 64]);
        let score = cone.membership_score(&point, &ball);

        // Should not panic, score should be valid
        assert!(score >= 0.0 && score <= 1.0);
    }

    fn make_point_at_angle(apex: &PoincarePoint, angle: f32) -> PoincarePoint {
        // Create point at specified angle from origin direction
        // Simplified: just create point with appropriate offset
        let mut coords = [0.0f32; 64];
        coords[0] = apex.coords[0] + 0.3 * angle.cos();
        coords[1] = apex.coords[1] + 0.3 * angle.sin();

        // Ensure inside ball
        let mut norm_sq = 0.0f32;
        for c in &coords { norm_sq += c * c; }
        if norm_sq >= 0.99 {
            let scale = 0.95 / norm_sq.sqrt();
            for c in &mut coords { *c *= scale; }
        }

        PoincarePoint::from_coords(&coords)
    }
}
```
