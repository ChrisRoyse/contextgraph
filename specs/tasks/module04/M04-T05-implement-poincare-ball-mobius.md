---
id: "M04-T05"
title: "Implement PoincareBall Mobius Operations"
description: |
  Implement PoincareBall struct with Mobius algebra operations.
  Methods: mobius_add(x, y), distance(x, y), exp_map(x, v), log_map(x, y).
  Distance formula: d(x,y) = (2/sqrt(|c|)) * arctanh(sqrt(|c| * ||x-y||^2 / ((1-|c|||x||^2)(1-|c|||y||^2))))
  Performance target: <10us per distance computation.
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 4
sequence: 8
depends_on:
  - "M04-T04"
spec_refs:
  - "TECH-GRAPH-004 Section 5.2"
  - "REQ-KG-051"
files_to_create:
  - path: "crates/context-graph-graph/src/hyperbolic/mobius.rs"
    description: "PoincareBall and Mobius operations"
files_to_modify:
  - path: "crates/context-graph-graph/src/hyperbolic/mod.rs"
    description: "Add mobius module"
test_file: "crates/context-graph-graph/tests/mobius_tests.rs"
---

## Context

Mobius operations are the fundamental algebraic operations in the Poincare ball model. Unlike Euclidean operations, addition and scalar multiplication must respect the hyperbolic geometry. These operations enable computing distances, geodesics, and mappings between tangent spaces and the manifold - all essential for entailment cone containment checks.

## Scope

### In Scope
- Define PoincareBall struct holding HyperbolicConfig
- Implement mobius_add(x, y) for hyperbolic vector addition
- Implement distance(x, y) for Poincare ball distance
- Implement exp_map(x, v) mapping tangent vector to manifold point
- Implement log_map(x, y) mapping points to tangent vector
- Handle numerical edge cases (near boundary)

### Out of Scope
- EntailmentCone (see M04-T06)
- CUDA acceleration (see M04-T23)

## Definition of Done

### Signatures

```rust
use crate::config::HyperbolicConfig;
use crate::hyperbolic::poincare::PoincarePoint;

/// Poincare ball model with Mobius algebra operations
pub struct PoincareBall {
    config: HyperbolicConfig,
}

impl PoincareBall {
    /// Create a new Poincare ball with given configuration
    pub fn new(config: HyperbolicConfig) -> Self {
        Self { config }
    }

    /// Mobius addition in Poincare ball
    ///
    /// Formula: x ⊕ y = ((1 + 2c<x,y> + c||y||^2)x + (1 - c||x||^2)y) /
    ///                  (1 + 2c<x,y> + c^2||x||^2||y||^2)
    ///
    /// where c = |curvature|
    pub fn mobius_add(&self, x: &PoincarePoint, y: &PoincarePoint) -> PoincarePoint {
        let c = self.config.abs_curvature();
        let x_norm_sq = x.norm_squared();
        let y_norm_sq = y.norm_squared();

        // Inner product <x, y>
        let xy_dot: f32 = x.coords.iter()
            .zip(y.coords.iter())
            .map(|(a, b)| a * b)
            .sum();

        let num_coeff_x = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
        let num_coeff_y = 1.0 - c * x_norm_sq;
        let denom = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

        let mut result = PoincarePoint::origin();
        for i in 0..64 {
            result.coords[i] = (num_coeff_x * x.coords[i] + num_coeff_y * y.coords[i]) / denom;
        }

        result.project(&self.config);
        result
    }

    /// Compute Poincare ball distance between two points
    ///
    /// Formula: d(x,y) = (2/sqrt(|c|)) * arctanh(sqrt(|c|) * ||(-x) ⊕ y||)
    ///
    /// Simplified: d(x,y) = (2/sqrt(|c|)) * arctanh(sqrt(|c| * ||x-y||^2 /
    ///             ((1 - |c|||x||^2)(1 - |c|||y||^2))))
    ///
    /// Performance target: <10us
    pub fn distance(&self, x: &PoincarePoint, y: &PoincarePoint) -> f32 {
        let c = self.config.abs_curvature();
        let sqrt_c = c.sqrt();

        // ||x - y||^2
        let diff_norm_sq: f32 = x.coords.iter()
            .zip(y.coords.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum();

        let x_norm_sq = x.norm_squared();
        let y_norm_sq = y.norm_squared();

        // Conformal factors
        let lambda_x = 1.0 - c * x_norm_sq;
        let lambda_y = 1.0 - c * y_norm_sq;

        // Avoid division by zero near boundary
        let denom = (lambda_x * lambda_y).max(self.config.eps);

        let arg = (c * diff_norm_sq / denom).sqrt().min(1.0 - self.config.eps);

        (2.0 / sqrt_c) * arg.atanh()
    }

    /// Exponential map: tangent vector at x -> point on manifold
    ///
    /// Maps a vector v in the tangent space at x to a point on the Poincare ball
    /// along the geodesic starting at x with initial direction v
    pub fn exp_map(&self, x: &PoincarePoint, v: &[f32; 64]) -> PoincarePoint {
        let c = self.config.abs_curvature();
        let sqrt_c = c.sqrt();

        // ||v|| in tangent space
        let v_norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();

        if v_norm < self.config.eps {
            return x.clone();
        }

        // Conformal factor at x
        let x_norm_sq = x.norm_squared();
        let lambda_x = 1.0 - c * x_norm_sq;

        // Scaled tangent norm
        let scaled_norm = sqrt_c * v_norm / lambda_x;

        let tanh_factor = scaled_norm.tanh();

        let mut result = PoincarePoint::origin();
        for i in 0..64 {
            let v_unit = v[i] / v_norm;
            result.coords[i] = tanh_factor * v_unit / sqrt_c;
        }

        // Mobius add with x
        self.mobius_add(x, &result)
    }

    /// Logarithmic map: point on manifold -> tangent vector at x
    ///
    /// Returns the tangent vector at x that points toward y
    /// Inverse of exp_map
    pub fn log_map(&self, x: &PoincarePoint, y: &PoincarePoint) -> [f32; 64] {
        let c = self.config.abs_curvature();
        let sqrt_c = c.sqrt();

        // (-x) ⊕ y
        let mut neg_x = x.clone();
        for c in &mut neg_x.coords {
            *c = -*c;
        }
        let diff = self.mobius_add(&neg_x, y);

        let diff_norm = diff.norm();
        if diff_norm < self.config.eps {
            return [0.0; 64];
        }

        // Conformal factor at x
        let x_norm_sq = x.norm_squared();
        let lambda_x = 1.0 - c * x_norm_sq;

        // arctanh(sqrt(c) * ||(-x) ⊕ y||)
        let arg = (sqrt_c * diff_norm).min(1.0 - self.config.eps);
        let scale = (2.0 * lambda_x / sqrt_c) * arg.atanh() / diff_norm;

        let mut result = [0.0; 64];
        for i in 0..64 {
            result[i] = scale * diff.coords[i];
        }

        result
    }

    /// Get reference to configuration
    pub fn config(&self) -> &HyperbolicConfig {
        &self.config
    }
}
```

### Constraints
- All results must stay inside Poincare ball (project if needed)
- Handle numerical stability near boundary (norm close to 1)
- Handle degenerate cases (zero vectors, identical points)
- Performance: distance() must complete in <10us

### Acceptance Criteria
- [ ] mobius_add() implements Mobius addition formula correctly
- [ ] distance() returns Poincare ball distance in <10us
- [ ] exp_map() maps tangent vector to point on manifold
- [ ] log_map() returns tangent vector from x to y
- [ ] All operations handle boundary cases (norm near 1.0)
- [ ] Unit tests verify mathematical properties (symmetry, identity)
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm

**Mobius Addition:**
1. Compute |c|, ||x||^2, ||y||^2, <x,y>
2. Compute numerator coefficients
3. Compute denominator
4. Combine and project result

**Distance:**
1. Compute ||x-y||^2
2. Compute conformal factors
3. Apply arctanh formula
4. Scale by 2/sqrt(|c|)

**Exp Map:**
1. Compute tangent norm
2. Apply tanh scaling
3. Mobius add with base point

**Log Map:**
1. Compute (-x) ⊕ y
2. Compute arctanh of norm
3. Scale by conformal factor

### Edge Cases
- x = y: distance = 0, log_map = zero vector
- x = origin: simplified formulas
- v = zero vector: exp_map returns x
- Points near boundary: use eps to avoid division by zero

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph mobius
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] distance(x, x) == 0 for any x
- [ ] distance(x, y) == distance(y, x) (symmetry)
- [ ] exp_map(x, log_map(x, y)) ≈ y
- [ ] log_map(x, exp_map(x, v)) ≈ v
- [ ] mobius_add(x, origin) == x
- [ ] Benchmark shows <10us per distance call
