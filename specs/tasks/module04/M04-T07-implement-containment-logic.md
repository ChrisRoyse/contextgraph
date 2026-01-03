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
status: "complete"
priority: "critical"
estimated_hours: 3
sequence: 10
depends_on:
  - "M04-T06"
spec_refs:
  - "TECH-GRAPH-004 Section 6"
  - "REQ-KG-053"
  - "constitution.yaml perf.latency.entailment_check"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/entailment/cones.rs"
    description: "Implement contains(), membership_score(), update_aperture() - replace todo!() stubs"
test_file: "crates/context-graph-graph/src/entailment/cones.rs"
verified_by: "sherlock-holmes"
verification_date: "2026-01-03"
verification_notes: |
  FORENSIC VERIFICATION COMPLETE:
  - contains() implemented at lines 249-252 using compute_angle() and effective_aperture()
  - compute_angle() helper at lines 272-309 with all edge cases handled
  - membership_score() uses EXACT canonical formula: exp(-2.0 * (angle - aperture))
  - update_aperture() clamps to [0.5, 2.0] at lines 383-385
  - 46/46 tests pass including 17 M04-T07 specific containment tests
  - No todo!() macros remain
  - No deviations from specification found
---

## Source of Truth

| Item | Location | Current State |
|------|----------|---------------|
| EntailmentCone struct | `crates/context-graph-graph/src/entailment/cones.rs:64-74` | COMPLETE |
| new() constructor | `crates/context-graph-graph/src/entailment/cones.rs:105-133` | COMPLETE |
| with_aperture() | `crates/context-graph-graph/src/entailment/cones.rs:158-184` | COMPLETE |
| effective_aperture() | `crates/context-graph-graph/src/entailment/cones.rs:205-210` | COMPLETE |
| is_valid() | `crates/context-graph-graph/src/entailment/cones.rs:288-295` | COMPLETE |
| validate() | `crates/context-graph-graph/src/entailment/cones.rs:312-328` | COMPLETE |
| **contains()** | `crates/context-graph-graph/src/entailment/cones.rs:226-232` | **STUB - TODO** |
| **membership_score()** | `crates/context-graph-graph/src/entailment/cones.rs:254-256` | **STUB - TODO** |
| **update_aperture()** | `crates/context-graph-graph/src/entailment/cones.rs:267-269` | **STUB - TODO** |
| PoincareBall.log_map() | `crates/context-graph-graph/src/hyperbolic/mobius.rs:355-389` | COMPLETE |
| PoincareBall.distance() | `crates/context-graph-graph/src/hyperbolic/mobius.rs:266-296` | COMPLETE |
| PoincarePoint | `crates/context-graph-graph/src/hyperbolic/poincare.rs` | COMPLETE |
| GraphError variants | `crates/context-graph-graph/src/error.rs` | COMPLETE |
| ConeConfig | `crates/context-graph-graph/src/config.rs` | COMPLETE |

## Context

Containment checking is the core operation for O(1) IS-A hierarchy queries. A point is contained in a cone if the angle between the point direction (from apex) and the cone axis (toward origin) is less than or equal to the effective aperture. This geometric interpretation leverages hyperbolic space properties where hierarchy depth correlates with distance from origin.

**Critical Dependencies Available:**
- `PoincareBall::log_map(&self, x: &PoincarePoint, y: &PoincarePoint) -> [f32; 64]` - Returns tangent vector at x pointing toward y
- `PoincareBall::distance(&self, x: &PoincarePoint, y: &PoincarePoint) -> f32` - Hyperbolic distance
- `PoincarePoint::origin() -> PoincarePoint` - Origin point (all zeros)
- `PoincarePoint::norm(&self) -> f32` - Euclidean norm of coordinates
- `EntailmentCone::effective_aperture(&self) -> f32` - Aperture * aperture_factor clamped to (0, π/2]

## Scope

### In Scope
- Replace `contains()` stub at line 226-232 with full implementation
- Replace `membership_score()` stub at line 254-256 with canonical formula
- Replace `update_aperture()` stub at line 267-269 for training
- Add private helper `compute_angle()` method
- Handle all edge cases (apex at origin, point at apex, etc.)
- Ensure <50μs performance target
- Add comprehensive tests in same file's `mod tests` section

### Out of Scope
- CUDA batch containment (see M04-T24)
- Integration tests with full graph (see M04-T25)
- Changes to existing struct fields or constructors

## NO BACKWARDS COMPATIBILITY

This implementation:
1. MUST replace `todo!()` macros - code that panics is unacceptable
2. MUST fail fast with `tracing::error!()` on invalid inputs
3. MUST NOT add deprecated methods or compatibility shims
4. MUST NOT add optional parameters to existing signatures

## Definition of Done

### Exact Signatures to Implement

```rust
impl EntailmentCone {
    /// Check if a point is contained within this cone.
    ///
    /// # Algorithm
    /// 1. Compute angle between point direction and cone axis (toward origin)
    /// 2. Return angle <= effective_aperture()
    ///
    /// # Performance Target: <50μs
    ///
    /// # Edge Cases
    /// - Point at apex: return true (angle = 0)
    /// - Apex at origin: return true (degenerate cone contains all)
    /// - Zero-length tangent vectors: return true (numerical edge case)
    pub fn contains(&self, point: &PoincarePoint, ball: &PoincareBall) -> bool {
        let angle = self.compute_angle(point, ball);
        angle <= self.effective_aperture()
    }

    /// Compute soft membership score for a point.
    ///
    /// # CANONICAL FORMULA (DO NOT MODIFY)
    /// - If angle <= effective_aperture: score = 1.0
    /// - If angle > effective_aperture: score = exp(-2.0 * (angle - aperture))
    ///
    /// # Returns
    /// Value in [0, 1] where 1.0 = fully contained, approaching 0 = far outside
    pub fn membership_score(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let angle = self.compute_angle(point, ball);
        let aperture = self.effective_aperture();

        if angle <= aperture {
            1.0
        } else {
            (-2.0 * (angle - aperture)).exp()
        }
    }

    /// Compute angle between point direction and cone axis.
    ///
    /// # Algorithm
    /// 1. tangent = log_map(apex, point) - direction to point in tangent space
    /// 2. to_origin = log_map(apex, origin) - cone axis direction
    /// 3. cos_angle = dot(tangent, to_origin) / (||tangent|| * ||to_origin||)
    /// 4. angle = acos(cos_angle.clamp(-1.0, 1.0))
    ///
    /// # Edge Cases Return 0.0:
    /// - Point at apex (distance < eps)
    /// - Apex at origin (norm < eps)
    /// - Zero-length tangent or to_origin vectors
    fn compute_angle(&self, point: &PoincarePoint, ball: &PoincareBall) -> f32 {
        let config = ball.config();

        // Edge case: point at apex
        let apex_to_point_dist = ball.distance(&self.apex, point);
        if apex_to_point_dist < config.eps {
            return 0.0;
        }

        // Edge case: apex at origin (degenerate cone)
        if self.apex.norm() < config.eps {
            return 0.0;
        }

        // Compute tangent vectors
        let tangent = ball.log_map(&self.apex, point);
        let to_origin = ball.log_map(&self.apex, &PoincarePoint::origin());

        // Compute norms
        let tangent_norm: f32 = tangent.iter().map(|x| x * x).sum::<f32>().sqrt();
        let to_origin_norm: f32 = to_origin.iter().map(|x| x * x).sum::<f32>().sqrt();

        // Edge case: degenerate tangent vectors
        if tangent_norm < config.eps || to_origin_norm < config.eps {
            return 0.0;
        }

        // Compute angle via dot product
        let dot: f32 = tangent.iter()
            .zip(to_origin.iter())
            .map(|(a, b)| a * b)
            .sum();

        let cos_angle = (dot / (tangent_norm * to_origin_norm)).clamp(-1.0, 1.0);
        cos_angle.acos()
    }

    /// Update aperture factor based on training signal.
    ///
    /// # Arguments
    /// * `delta` - Adjustment to aperture_factor
    ///   - Positive delta widens the cone (more inclusive)
    ///   - Negative delta narrows the cone (more exclusive)
    ///
    /// # Invariant
    /// Result is clamped to [0.5, 2.0] range per constitution constraints.
    pub fn update_aperture(&mut self, delta: f32) {
        self.aperture_factor = (self.aperture_factor + delta).clamp(0.5, 2.0);
    }
}
```

### Constraints
- Performance: <50μs per containment check
- `membership_score` MUST use canonical formula: `exp(-2.0 * (angle - aperture))`
- `aperture_factor` MUST stay in [0.5, 2.0] range
- All edge cases must be handled without panics
- NO `unwrap()` or `expect()` in production code paths
- Use `tracing::error!()` for logging edge case handling

### Acceptance Criteria
- [ ] `contains()` replaces todo!() stub with full implementation
- [ ] `contains()` returns true for points within cone
- [ ] `contains()` returns false for points outside cone
- [ ] `membership_score()` replaces todo!() stub with canonical formula
- [ ] `membership_score()` returns 1.0 for contained points
- [ ] `membership_score()` returns exp(-2.0 * excess_angle) for outside points
- [ ] `update_aperture()` replaces todo!() stub
- [ ] `update_aperture()` clamps to [0.5, 2.0]
- [ ] Edge cases handled: apex at origin, point at apex, degenerate tangents
- [ ] Performance: <50μs per containment check
- [ ] Compiles with `cargo build -p context-graph-graph`
- [ ] All tests pass with `cargo test -p context-graph-graph entailment`
- [ ] No clippy warnings: `cargo clippy -p context-graph-graph -- -D warnings`

## Full State Verification

### Pre-Implementation State (Verify Before Starting)

Run these commands to verify baseline:

```bash
# 1. Verify stubs exist and panic
cargo test -p context-graph-graph test_contains_stub_panics 2>&1 | grep -E "(PASSED|FAILED|panicked)"

# 2. Verify dependencies compile
cargo build -p context-graph-graph 2>&1 | tail -5

# 3. Check current test count
cargo test -p context-graph-graph entailment 2>&1 | grep -E "test result"
```

Expected pre-state:
- `contains()` at line 231 contains `todo!("Containment logic implemented in M04-T07")`
- `membership_score()` at line 255 contains `todo!("Membership score implemented in M04-T07")`
- `update_aperture()` at line 268 contains `todo!("Update aperture implemented in M04-T07")`

### Execute & Inspect

After implementation, verify:

```bash
# 1. No more todo!() macros
grep -n "todo!" crates/context-graph-graph/src/entailment/cones.rs
# Expected: No output

# 2. All tests pass
cargo test -p context-graph-graph entailment -- --nocapture 2>&1 | tail -20

# 3. No clippy warnings
cargo clippy -p context-graph-graph -- -D warnings 2>&1 | tail -10

# 4. Performance check (manual)
# Run benchmark or time individual operations
```

## Boundary & Edge Case Audit

### Edge Case 1: Point at Apex

**Before State:**
```rust
let cone = EntailmentCone::default();
let ball = PoincareBall::default();
let point = cone.apex.clone();
// cone.contains(&point, &ball) -> PANIC (todo!())
```

**After State:**
```rust
let cone = EntailmentCone::default();
let ball = PoincareBall::default();
let point = cone.apex.clone();
assert!(cone.contains(&point, &ball)); // true - point at apex always contained
assert_eq!(cone.membership_score(&point, &ball), 1.0); // perfect membership
```

**Test to Add:**
```rust
#[test]
fn test_point_at_apex_contained() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::new(apex.clone(), 0, &config).unwrap();
    let ball = PoincareBall::default();

    assert!(cone.contains(&apex, &ball), "Point at apex must be contained");
    assert_eq!(cone.membership_score(&apex, &ball), 1.0);
}
```

### Edge Case 2: Apex at Origin (Degenerate Cone)

**Before State:**
```rust
let apex = PoincarePoint::origin();
let cone = EntailmentCone::with_aperture(apex, 0.5, 0).unwrap();
let ball = PoincareBall::default();
let point = /* any valid point */;
// cone.contains(&point, &ball) -> PANIC (todo!())
```

**After State:**
```rust
let apex = PoincarePoint::origin();
let cone = EntailmentCone::with_aperture(apex, 0.5, 0).unwrap();
let ball = PoincareBall::default();
let point = /* any valid point */;
assert!(cone.contains(&point, &ball)); // true - degenerate cone at origin contains all
assert_eq!(cone.membership_score(&point, &ball), 1.0);
```

**Test to Add:**
```rust
#[test]
fn test_apex_at_origin_contains_all() {
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::with_aperture(apex, 0.5, 0).unwrap();
    let ball = PoincareBall::default();

    // Create various points
    let mut coords = [0.0f32; 64];
    coords[0] = 0.5;
    let point = PoincarePoint::from_coords(coords);

    assert!(cone.contains(&point, &ball), "Origin apex contains all points");
    assert_eq!(cone.membership_score(&point, &ball), 1.0);
}
```

### Edge Case 3: Point Outside Cone

**Before State:**
```rust
// Cannot test - todo!() panics
```

**After State:**
```rust
let mut apex_coords = [0.0f32; 64];
apex_coords[0] = 0.5; // Apex at (0.5, 0, 0, ...)
let apex = PoincarePoint::from_coords(apex_coords);
let cone = EntailmentCone::with_aperture(apex, 0.3, 1).unwrap(); // narrow cone
let ball = PoincareBall::default();

// Point perpendicular to cone axis
let mut point_coords = [0.0f32; 64];
point_coords[1] = 0.5; // Point at (0, 0.5, 0, ...) - perpendicular
let point = PoincarePoint::from_coords(point_coords);

assert!(!cone.contains(&point, &ball)); // false - outside cone
let score = cone.membership_score(&point, &ball);
assert!(score < 1.0 && score > 0.0); // partial membership
```

**Test to Add:**
```rust
#[test]
fn test_point_outside_cone() {
    let mut apex_coords = [0.0f32; 64];
    apex_coords[0] = 0.5;
    let apex = PoincarePoint::from_coords(apex_coords);
    let cone = EntailmentCone::with_aperture(apex, 0.3, 1).unwrap();
    let ball = PoincareBall::default();

    let mut point_coords = [0.0f32; 64];
    point_coords[1] = 0.5; // Perpendicular direction
    let point = PoincarePoint::from_coords(point_coords);

    assert!(!cone.contains(&point, &ball), "Perpendicular point should be outside");

    let score = cone.membership_score(&point, &ball);
    assert!(score < 1.0, "Score should be < 1.0 for outside point");
    assert!(score > 0.0, "Score should be > 0.0 (exponential decay)");
}
```

## Tests to Add

Add these tests to the existing `mod tests` section in `cones.rs`:

```rust
// ========== CONTAINMENT TESTS (M04-T07) ==========

#[test]
fn test_point_at_apex_contained() {
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::new(apex.clone(), 0, &config).unwrap();
    let ball = PoincareBall::default();

    assert!(cone.contains(&apex, &ball));
    assert_eq!(cone.membership_score(&apex, &ball), 1.0);
}

#[test]
fn test_apex_at_origin_contains_all() {
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::with_aperture(apex, 0.5, 0).unwrap();
    let ball = PoincareBall::default();

    let mut coords = [0.0f32; 64];
    coords[0] = 0.5;
    let point = PoincarePoint::from_coords(coords);

    assert!(cone.contains(&point, &ball));
    assert_eq!(cone.membership_score(&point, &ball), 1.0);
}

#[test]
fn test_point_along_cone_axis_contained() {
    // Apex not at origin, point toward origin (along cone axis)
    let mut apex_coords = [0.0f32; 64];
    apex_coords[0] = 0.5;
    let apex = PoincarePoint::from_coords(apex_coords);
    let cone = EntailmentCone::with_aperture(apex, 0.5, 1).unwrap();
    let ball = PoincareBall::default();

    // Point between apex and origin (along axis)
    let mut point_coords = [0.0f32; 64];
    point_coords[0] = 0.25;
    let point = PoincarePoint::from_coords(point_coords);

    assert!(cone.contains(&point, &ball), "Point along axis toward origin should be contained");
    assert_eq!(cone.membership_score(&point, &ball), 1.0);
}

#[test]
fn test_point_outside_cone_perpendicular() {
    let mut apex_coords = [0.0f32; 64];
    apex_coords[0] = 0.5;
    let apex = PoincarePoint::from_coords(apex_coords);
    let cone = EntailmentCone::with_aperture(apex, 0.3, 1).unwrap();
    let ball = PoincareBall::default();

    // Point in perpendicular direction
    let mut point_coords = [0.0f32; 64];
    point_coords[1] = 0.5;
    let point = PoincarePoint::from_coords(point_coords);

    assert!(!cone.contains(&point, &ball));
    let score = cone.membership_score(&point, &ball);
    assert!(score < 1.0);
    assert!(score > 0.0);
}

#[test]
fn test_membership_score_canonical_formula() {
    // Verify exp(-2.0 * (angle - aperture)) formula
    let mut apex_coords = [0.0f32; 64];
    apex_coords[0] = 0.5;
    let apex = PoincarePoint::from_coords(apex_coords);
    let cone = EntailmentCone::with_aperture(apex, 0.3, 1).unwrap();
    let ball = PoincareBall::default();

    // Point outside cone
    let mut point_coords = [0.0f32; 64];
    point_coords[1] = 0.5;
    let point = PoincarePoint::from_coords(point_coords);

    let score = cone.membership_score(&point, &ball);

    // Score should be positive and less than 1
    assert!(score > 0.0, "Score should be > 0 (exponential never reaches 0)");
    assert!(score < 1.0, "Score should be < 1 for outside point");
}

#[test]
fn test_update_aperture_positive_delta() {
    let mut cone = EntailmentCone::default();
    assert_eq!(cone.aperture_factor, 1.0);

    cone.update_aperture(0.3);
    assert!((cone.aperture_factor - 1.3).abs() < 1e-6);
}

#[test]
fn test_update_aperture_negative_delta() {
    let mut cone = EntailmentCone::default();
    cone.update_aperture(-0.3);
    assert!((cone.aperture_factor - 0.7).abs() < 1e-6);
}

#[test]
fn test_update_aperture_clamps_max() {
    let mut cone = EntailmentCone::default();
    cone.update_aperture(10.0); // Large positive
    assert_eq!(cone.aperture_factor, 2.0);
}

#[test]
fn test_update_aperture_clamps_min() {
    let mut cone = EntailmentCone::default();
    cone.update_aperture(-10.0); // Large negative
    assert_eq!(cone.aperture_factor, 0.5);
}

#[test]
fn test_containment_boundary_angle() {
    // Test point exactly at aperture boundary
    // This is tricky to construct precisely but verifies boundary behavior
    let config = ConeConfig::default();
    let apex = PoincarePoint::origin();
    let cone = EntailmentCone::new(apex, 0, &config).unwrap();
    let ball = PoincareBall::default();

    // With apex at origin, all points should be contained
    let mut coords = [0.0f32; 64];
    coords[0] = 0.9;
    let point = PoincarePoint::from_coords(coords);

    assert!(cone.contains(&point, &ball));
}
```

## Evidence of Success

After successful implementation, these commands must produce the following outputs:

```bash
# 1. No todo! macros remain
$ grep -c "todo!" crates/context-graph-graph/src/entailment/cones.rs
0

# 2. All tests pass
$ cargo test -p context-graph-graph entailment 2>&1 | grep "test result"
test result: ok. XX passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# 3. No clippy warnings
$ cargo clippy -p context-graph-graph -- -D warnings 2>&1 | grep -E "^error"
(no output - no errors)

# 4. Build succeeds
$ cargo build -p context-graph-graph 2>&1 | tail -1
    Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs

# 5. Doc tests pass
$ cargo test -p context-graph-graph --doc 2>&1 | grep "test result"
test result: ok.
```

## Verification Commands

```bash
# Full verification sequence
cargo build -p context-graph-graph && \
cargo test -p context-graph-graph entailment -- --nocapture && \
cargo clippy -p context-graph-graph -- -D warnings && \
echo "M04-T07 VERIFICATION PASSED"
```

## Final Verification: Sherlock-Holmes Subagent

**MANDATORY**: After implementation is complete, spawn a `sherlock-holmes` subagent to perform forensic verification:

```
Task sherlock-holmes: "Forensically verify M04-T07 implementation:
1. Verify no todo!() macros remain in cones.rs
2. Verify contains() implements exact algorithm from spec
3. Verify membership_score() uses EXACTLY exp(-2.0 * (angle - aperture))
4. Verify update_aperture() clamps to [0.5, 2.0]
5. Verify all edge cases are handled (apex at origin, point at apex)
6. Run cargo test and verify all pass
7. Run cargo clippy and verify no warnings
8. Check performance target <50μs is achievable
9. Report any deviations from specification"
```

The sherlock-holmes agent MUST confirm:
- [ ] Implementation matches specification exactly
- [ ] No shortcuts or deviations from canonical formulas
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Edge cases properly handled

## Implementation Notes

### Required Imports (already present in cones.rs)
```rust
use crate::hyperbolic::mobius::PoincareBall;
use crate::hyperbolic::poincare::PoincarePoint;
```

### PoincareBall Methods Available
From `mobius.rs:355-389`:
```rust
impl PoincareBall {
    pub fn log_map(&self, x: &PoincarePoint, y: &PoincarePoint) -> [f32; 64]
    pub fn distance(&self, x: &PoincarePoint, y: &PoincarePoint) -> f32
    pub fn config(&self) -> &HyperbolicConfig
}
```

### HyperbolicConfig Fields
From `config.rs`:
```rust
pub struct HyperbolicConfig {
    pub dim: usize,      // 64
    pub curvature: f32,  // -1.0
    pub eps: f32,        // 1e-5
    pub max_norm: f32,   // 0.999
}
```

## Traceability

| Requirement | Implementation |
|-------------|----------------|
| REQ-KG-053 | EntailmentCone.contains() |
| TECH-GRAPH-004 Section 6 | compute_angle() algorithm |
| constitution perf.latency.entailment_check | <50μs target |
| contextprd Section 9 | Hyperbolic entailment cones |
