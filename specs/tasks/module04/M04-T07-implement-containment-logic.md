---
id: "M04-T07"
title: "Implement EntailmentCone Containment Logic"
description: |
  Implement EntailmentCone containment check algorithm.
  Algorithm:
  1. Compute tangent = log_map(apex, point)
  2. Compute to_origin = log_map(apex, origin)
  3. angle = arccos(dot(tangent, to_origin) / (||tangent|| * ||to_origin||))
  4. Return angle <= effective_aperture()

  CANONICAL FORMULA for membership_score():
  - If contained: 1.0
  - If not contained: exp(-2.0 * (angle - aperture))

  Include update_aperture() for training.
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 3
sequence: 10
depends_on:
  - "M04-T06"
spec_refs:
  - "TECH-GRAPH-004 Section 6"
  - "REQ-KG-053"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/entailment/cones.rs"
    description: "Implement contains(), membership_score(), update_aperture()"
test_file: "crates/context-graph-graph/tests/entailment_tests.rs"
---

## Context

Containment checking is the core operation for O(1) IS-A hierarchy queries. A point is contained in a cone if the angle between the point direction (from apex) and the cone axis (toward origin) is less than or equal to the effective aperture. This geometric interpretation leverages hyperbolic space properties where hierarchy depth correlates with distance from origin.

## Scope

### In Scope
- Replace contains() stub with full implementation
- Replace membership_score() stub with canonical formula
- Implement update_aperture() for training
- Handle all edge cases (apex at origin, point at apex, etc.)
- Ensure <50us performance target

### Out of Scope
- CUDA batch containment (see M04-T24)
- Integration tests (see M04-T25)

## Definition of Done

### Signatures

```rust
impl EntailmentCone {
    /// Check if a point is contained within this cone
    ///
    /// Algorithm:
    /// 1. Compute tangent = log_map(apex, point) - direction to point in tangent space
    /// 2. Compute to_origin = log_map(apex, origin) - direction to origin (cone axis)
    /// 3. angle = arccos(dot(tangent, to_origin) / (||tangent|| * ||to_origin||))
    /// 4. Return angle <= effective_aperture()
    ///
    /// Performance target: <50us
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
    /// Returns value in [0, 1] where 1 = fully contained, 0 = far outside
    pub fn membership_score(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let angle = self.compute_angle(point, ball);
        let aperture = self.effective_aperture();

        if angle <= aperture {
            1.0
        } else {
            (-2.0 * (angle - aperture)).exp()
        }
    }

    /// Compute angle between point direction and cone axis
    ///
    /// Internal helper for contains() and membership_score()
    fn compute_angle(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let config = ball.config();

        // Handle degenerate case: point at apex
        let apex_to_point_dist = ball.distance(&self.apex, point);
        if apex_to_point_dist < config.eps {
            return 0.0; // Point at apex is always contained
        }

        // Handle degenerate case: apex at origin
        if self.apex.norm() < config.eps {
            // Cone at origin: all points are "toward origin" at angle 0
            // This is a degenerate cone that contains everything
            return 0.0;
        }

        // Compute tangent vectors in tangent space at apex
        let tangent = ball.log_map(&self.apex, point);
        let to_origin = ball.log_map(&self.apex, &PoincarePoint::origin());

        // Compute norms
        let tangent_norm: f32 = tangent.iter().map(|x| x * x).sum::<f32>().sqrt();
        let to_origin_norm: f32 = to_origin.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Handle degenerate cases
        if tangent_norm < config.eps || to_origin_norm < config.eps {
            return 0.0;
        }

        // Compute dot product
        let dot: f32 = tangent.iter()
            .zip(to_origin.iter())
            .map(|(a, b)| a * b)
            .sum();

        // Compute angle using arccos
        let cos_angle = (dot / (tangent_norm * to_origin_norm)).clamp(-1.0, 1.0);
        cos_angle.acos()
    }

    /// Update aperture factor based on training signal
    ///
    /// # Arguments
    /// * `delta` - Adjustment to aperture factor
    ///   - Positive delta widens the cone (more inclusive)
    ///   - Negative delta narrows the cone (more exclusive)
    ///
    /// Result is clamped to [0.5, 2.0] range
    pub fn update_aperture(&mut self, delta: f32) {
        self.aperture_factor = (self.aperture_factor + delta).clamp(0.5, 2.0);
    }

    /// Batch containment check for multiple points
    ///
    /// More efficient than individual calls for large batches
    pub fn contains_batch(&self, points: &[PoincarePoint], ball: &PoincareBall) -> Vec<bool> {
        points.iter().map(|p| self.contains(p, ball)).collect()
    }

    /// Batch membership scores for multiple points
    pub fn membership_scores_batch(&self, points: &[PoincarePoint], ball: &PoincareBall) -> Vec<f32> {
        points.iter().map(|p| self.membership_score(p, ball)).collect()
    }
}
```

### Constraints
- Performance: <50us per containment check
- membership_score MUST use canonical formula: exp(-2.0 * (angle - aperture))
- aperture_factor MUST stay in [0.5, 2.0] range
- All edge cases must be handled without panics

### Acceptance Criteria
- [ ] contains() returns true for points within cone
- [ ] contains() returns false for points outside cone
- [ ] membership_score() uses canonical formula (see above)
- [ ] update_aperture() adjusts aperture_factor based on training signal
- [ ] Edge cases handled: apex at origin, point at apex, degenerate cones
- [ ] Performance: <50us per containment check
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm

**contains(point):**
```
1. angle = compute_angle(point)
2. return angle <= effective_aperture()
```

**membership_score(point):**
```
1. angle = compute_angle(point)
2. aperture = effective_aperture()
3. if angle <= aperture: return 1.0
4. else: return exp(-2.0 * (angle - aperture))
```

**compute_angle(point):**
```
1. if point == apex: return 0.0
2. if apex == origin: return 0.0 (degenerate cone)
3. tangent = log_map(apex, point)
4. to_origin = log_map(apex, origin)
5. cos_angle = dot(tangent, to_origin) / (||tangent|| * ||to_origin||)
6. return acos(cos_angle)
```

**update_aperture(delta):**
```
1. aperture_factor = (aperture_factor + delta).clamp(0.5, 2.0)
```

### Edge Cases
- Point at apex: angle = 0, always contained
- Apex at origin: degenerate cone, angle = 0, all contained
- Point very close to apex: numerical instability, use eps threshold
- Angle exactly at boundary: contained (<=, not <)
- NaN from acos: clamp cos_angle to [-1, 1]

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph entailment
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Point at apex returns true for contains()
- [ ] Point along cone axis returns true for contains()
- [ ] Point far outside cone returns false for contains()
- [ ] membership_score() returns 1.0 for contained points
- [ ] membership_score() returns ~0.37 for point at 0.5 rad beyond aperture
- [ ] update_aperture() clamps to valid range
- [ ] Benchmark shows <50us per check
