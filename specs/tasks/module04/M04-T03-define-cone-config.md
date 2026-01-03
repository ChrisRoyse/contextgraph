---
id: "M04-T03"
title: "Define ConeConfig for Entailment Cones"
description: |
  Implement ConeConfig struct for EntailmentCone parameters.
  Fields: min_aperture (0.1 rad), max_aperture (1.5 rad), base_aperture (1.0 rad),
  aperture_decay (0.85 per level), membership_threshold (0.7).
  Include compute_aperture(depth) method.
layer: "foundation"
status: "pending"
priority: "high"
estimated_hours: 1.5
sequence: 6
depends_on:
  - "M04-T00"
spec_refs:
  - "TECH-GRAPH-004 Section 6"
  - "REQ-KG-052"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/config.rs"
    description: "Add ConeConfig struct"
test_file: "crates/context-graph-graph/tests/config_tests.rs"
---

## Context

ConeConfig defines parameters for entailment cones used in O(1) IS-A hierarchy queries. Entailment cones are geometric structures in hyperbolic space where containment represents taxonomic relationships. A concept's cone contains all concepts it entails (subsumes). The aperture decreases with depth to create narrower cones for more specific concepts.

## Scope

### In Scope
- Define ConeConfig struct with 5 fields
- Implement Default trait
- Implement compute_aperture(depth) method
- Add Serde serialization

### Out of Scope
- EntailmentCone struct (see M04-T06)
- Containment logic (see M04-T07)

## Definition of Done

### Signatures

```rust
use serde::{Deserialize, Serialize};

/// Configuration for entailment cones in hyperbolic space
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConeConfig {
    /// Minimum cone aperture in radians
    pub min_aperture: f32,
    /// Maximum cone aperture in radians
    pub max_aperture: f32,
    /// Base aperture for depth 0 nodes (root concepts)
    pub base_aperture: f32,
    /// Decay factor per hierarchy level (0-1)
    pub aperture_decay: f32,
    /// Threshold for soft membership score
    pub membership_threshold: f32,
}

impl Default for ConeConfig {
    fn default() -> Self {
        Self {
            min_aperture: 0.1,      // ~5.7 degrees
            max_aperture: 1.5,      // ~85.9 degrees
            base_aperture: 1.0,     // ~57.3 degrees
            aperture_decay: 0.85,   // 15% narrower per level
            membership_threshold: 0.7,
        }
    }
}

impl ConeConfig {
    /// Compute aperture for a node at given depth
    ///
    /// Formula: aperture = base_aperture * decay^depth
    /// Result is clamped to [min_aperture, max_aperture]
    ///
    /// # Arguments
    /// * `depth` - Depth in hierarchy (0 = root)
    ///
    /// # Returns
    /// Aperture in radians
    pub fn compute_aperture(&self, depth: u32) -> f32 {
        let raw = self.base_aperture * self.aperture_decay.powi(depth as i32);
        raw.clamp(self.min_aperture, self.max_aperture)
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> Result<(), crate::error::GraphError> {
        if self.min_aperture <= 0.0 {
            return Err(crate::error::GraphError::InvalidConfig(
                "min_aperture must be positive".to_string()
            ));
        }
        if self.max_aperture <= self.min_aperture {
            return Err(crate::error::GraphError::InvalidConfig(
                "max_aperture must be greater than min_aperture".to_string()
            ));
        }
        if self.aperture_decay <= 0.0 || self.aperture_decay >= 1.0 {
            return Err(crate::error::GraphError::InvalidConfig(
                "aperture_decay must be in (0, 1)".to_string()
            ));
        }
        if self.membership_threshold <= 0.0 || self.membership_threshold >= 1.0 {
            return Err(crate::error::GraphError::InvalidConfig(
                "membership_threshold must be in (0, 1)".to_string()
            ));
        }
        Ok(())
    }
}
```

### Constraints
- All apertures are in radians
- min_aperture < max_aperture
- aperture_decay in (0, 1)
- membership_threshold in (0, 1)
- compute_aperture result always in [min_aperture, max_aperture]

### Acceptance Criteria
- [ ] ConeConfig struct with 5 fields
- [ ] compute_aperture(0) returns base_aperture
- [ ] compute_aperture(n) = base * decay^n, clamped to [min, max]
- [ ] Default values match spec
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define struct with 5 fields
2. Derive Debug, Clone, Serialize, Deserialize
3. Implement Default with spec values
4. Implement compute_aperture:
   - raw = base_aperture * decay^depth
   - return raw.clamp(min_aperture, max_aperture)

### Edge Cases
- depth = 0: Returns base_aperture (before clamping)
- Very large depth: Returns min_aperture (decay converges to 0)
- aperture_decay = 1.0: All depths have same aperture (invalid)

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph config
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] compute_aperture(0) == 1.0 for default config
- [ ] compute_aperture(1) == 0.85 for default config
- [ ] compute_aperture(100) == 0.1 (clamped to min) for default config
- [ ] JSON serialization/deserialization roundtrips correctly
