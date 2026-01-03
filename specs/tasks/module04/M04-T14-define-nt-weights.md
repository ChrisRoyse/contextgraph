---
id: "M04-T14"
title: "Implement NeurotransmitterWeights for Edges (Marblestone)"
description: |
  Implement NeurotransmitterWeights struct from Module 2 in graph context.
  Fields: excitatory (f32), inhibitory (f32), modulatory (f32), all in [0,1].
  Include for_domain(Domain) factory with domain-specific profiles.

  CANONICAL FORMULA for net_activation():
  net_activation = excitatory - inhibitory + (modulatory * 0.5)
layer: "logic"
status: "pending"
priority: "high"
estimated_hours: 2
sequence: 19
depends_on: []
spec_refs:
  - "TECH-GRAPH-004 Section 4.1"
  - "REQ-KG-065"
files_to_create:
  - path: "crates/context-graph-graph/src/storage/edges.rs"
    description: "NeurotransmitterWeights struct and Domain enum"
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "Add edges module"
test_file: "crates/context-graph-graph/tests/marblestone_tests.rs"
---

## Context

NeurotransmitterWeights implements the Marblestone neuroscience-inspired modulation system. Based on biological neural networks, three weight classes modulate edge importance: excitatory (increases activation), inhibitory (decreases activation), and modulatory (adjusts sensitivity). This enables domain-specific search behavior without retraining the graph.

## Scope

### In Scope
- NeurotransmitterWeights struct with 3 f32 fields
- Domain enum (Code, Legal, Medical, Creative, Research, General)
- for_domain() factory method
- Default implementation
- Serde serialization

### Out of Scope
- Weight validation (see M04-T14a)
- net_activation() implementation (see M04-T14a)
- GraphEdge struct (see M04-T15)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/storage/edges.rs

use serde::{Deserialize, Serialize};

/// Cognitive domains for domain-aware retrieval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Domain {
    /// Software development, algorithms, systems
    Code,
    /// Legal documents, contracts, regulations
    Legal,
    /// Medical literature, clinical notes, research
    Medical,
    /// Creative writing, art, design
    Creative,
    /// Academic research, scientific papers
    Research,
    /// General knowledge, misc content
    General,
}

impl Domain {
    /// Get all domain variants
    pub fn all() -> &'static [Domain] {
        &[
            Domain::Code,
            Domain::Legal,
            Domain::Medical,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ]
    }

    /// Parse domain from string (case-insensitive)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "code" | "software" | "programming" => Some(Domain::Code),
            "legal" | "law" => Some(Domain::Legal),
            "medical" | "health" | "clinical" => Some(Domain::Medical),
            "creative" | "art" | "writing" => Some(Domain::Creative),
            "research" | "academic" | "science" => Some(Domain::Research),
            "general" | "misc" | "" => Some(Domain::General),
            _ => None,
        }
    }
}

impl std::fmt::Display for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Domain::Code => write!(f, "Code"),
            Domain::Legal => write!(f, "Legal"),
            Domain::Medical => write!(f, "Medical"),
            Domain::Creative => write!(f, "Creative"),
            Domain::Research => write!(f, "Research"),
            Domain::General => write!(f, "General"),
        }
    }
}

impl Default for Domain {
    fn default() -> Self {
        Domain::General
    }
}

/// Neurotransmitter-inspired weights for edge modulation
///
/// Based on Marblestone architecture, these weights modulate edge importance
/// during traversal and search operations. Each weight should be in [0, 1].
///
/// # CANONICAL FORMULA
/// ```text
/// net_activation = excitatory - inhibitory + (modulatory * 0.5)
/// ```
///
/// The result is in range [-1.5, 1.5] for valid weights.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NeurotransmitterWeights {
    /// Excitatory weight - increases edge activation
    /// Higher values make the edge more likely to be traversed
    /// Range: [0.0, 1.0]
    pub excitatory: f32,

    /// Inhibitory weight - decreases edge activation
    /// Higher values suppress the edge during traversal
    /// Range: [0.0, 1.0]
    pub inhibitory: f32,

    /// Modulatory weight - adjusts sensitivity
    /// Adds additional activation scaled by 0.5
    /// Range: [0.0, 1.0]
    pub modulatory: f32,
}

impl NeurotransmitterWeights {
    /// Create new weights with specified values
    ///
    /// # Arguments
    /// * `excitatory` - Excitatory weight [0, 1]
    /// * `inhibitory` - Inhibitory weight [0, 1]
    /// * `modulatory` - Modulatory weight [0, 1]
    ///
    /// # Note
    /// Use validate() to check weight ranges (see M04-T14a)
    pub fn new(excitatory: f32, inhibitory: f32, modulatory: f32) -> Self {
        Self {
            excitatory,
            inhibitory,
            modulatory,
        }
    }

    /// Create domain-specific weights
    ///
    /// # Arguments
    /// * `domain` - Target cognitive domain
    ///
    /// # Returns
    /// Weights optimized for the specified domain's access patterns
    pub fn for_domain(domain: Domain) -> Self {
        match domain {
            Domain::Code => Self {
                // Code: High precision, moderate inhibition, some modulation
                excitatory: 0.7,
                inhibitory: 0.3,
                modulatory: 0.2,
            },
            Domain::Legal => Self {
                // Legal: High precision, strong inhibition (avoid wrong info)
                excitatory: 0.6,
                inhibitory: 0.4,
                modulatory: 0.1,
            },
            Domain::Medical => Self {
                // Medical: High precision, strong inhibition (safety critical)
                excitatory: 0.6,
                inhibitory: 0.5,
                modulatory: 0.1,
            },
            Domain::Creative => Self {
                // Creative: Exploratory, low inhibition, high modulation
                excitatory: 0.8,
                inhibitory: 0.2,
                modulatory: 0.5,
            },
            Domain::Research => Self {
                // Research: Balanced exploration and precision
                excitatory: 0.7,
                inhibitory: 0.35,
                modulatory: 0.3,
            },
            Domain::General => Self::default(),
        }
    }

    /// Create neutral weights (no modulation effect)
    ///
    /// net_activation with neutral weights = 0.5 - 0.5 + 0 = 0
    pub fn neutral() -> Self {
        Self {
            excitatory: 0.5,
            inhibitory: 0.5,
            modulatory: 0.0,
        }
    }

    /// Create maximum activation weights
    ///
    /// net_activation = 1.0 - 0.0 + (1.0 * 0.5) = 1.5
    pub fn max_activation() -> Self {
        Self {
            excitatory: 1.0,
            inhibitory: 0.0,
            modulatory: 1.0,
        }
    }

    /// Create minimum activation weights
    ///
    /// net_activation = 0.0 - 1.0 + (0.0 * 0.5) = -1.0
    pub fn min_activation() -> Self {
        Self {
            excitatory: 0.0,
            inhibitory: 1.0,
            modulatory: 0.0,
        }
    }

    /// Linearly interpolate between two weight sets
    ///
    /// # Arguments
    /// * `other` - Target weights
    /// * `t` - Interpolation factor [0, 1]
    ///
    /// # Returns
    /// Interpolated weights: self * (1-t) + other * t
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            excitatory: self.excitatory * (1.0 - t) + other.excitatory * t,
            inhibitory: self.inhibitory * (1.0 - t) + other.inhibitory * t,
            modulatory: self.modulatory * (1.0 - t) + other.modulatory * t,
        }
    }

    /// Get domain-based bonus weight
    ///
    /// Returns 0.1 for exact domain match, 0.0 otherwise.
    /// Used in edge weight modulation formula.
    pub fn domain_bonus(edge_domain: Domain, query_domain: Domain) -> f32 {
        if edge_domain == query_domain {
            0.1
        } else {
            0.0
        }
    }
}

impl Default for NeurotransmitterWeights {
    /// Default weights: balanced excitation/inhibition, no modulation
    ///
    /// net_activation = 0.5 - 0.5 + (0.0 * 0.5) = 0.0
    fn default() -> Self {
        Self {
            excitatory: 0.5,
            inhibitory: 0.5,
            modulatory: 0.0,
        }
    }
}

/// Display weights in compact format
impl std::fmt::Display for NeurotransmitterWeights {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NT(e={:.2}, i={:.2}, m={:.2})",
            self.excitatory, self.inhibitory, self.modulatory
        )
    }
}
```

### Constraints
- All weights should be in [0.0, 1.0] (validated in M04-T14a)
- net_activation formula: `excitatory - inhibitory + (modulatory * 0.5)`
- Default: excitatory=0.5, inhibitory=0.5, modulatory=0.0 (neutral)
- for_domain(Code) = {0.7, 0.3, 0.2}
- for_domain(Creative) = {0.8, 0.2, 0.5}

### Acceptance Criteria
- [ ] NeurotransmitterWeights struct with 3 f32 fields
- [ ] Default: excitatory=0.5, inhibitory=0.5, modulatory=0.0
- [ ] for_domain(Code) = {0.7, 0.3, 0.2}
- [ ] for_domain(Creative) = {0.8, 0.2, 0.5}
- [ ] Serde serialization works
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Define Domain enum with 6 variants
2. Define NeurotransmitterWeights struct with 3 f32 fields
3. Implement for_domain() with match on each domain
4. Implement Default with neutral values
5. Add helper methods (neutral, max_activation, min_activation, lerp)

### Edge Cases
- Unknown domain string: Return None from from_str
- Interpolation t outside [0,1]: Clamp to valid range
- Display formatting: Use 2 decimal places

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph marblestone
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Default weights are neutral
- [ ] Domain-specific weights match spec
- [ ] Serialization roundtrip works

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights() {
        let weights = NeurotransmitterWeights::default();
        assert!((weights.excitatory - 0.5).abs() < 1e-6);
        assert!((weights.inhibitory - 0.5).abs() < 1e-6);
        assert!((weights.modulatory - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_code_domain() {
        let weights = NeurotransmitterWeights::for_domain(Domain::Code);
        assert!((weights.excitatory - 0.7).abs() < 1e-6);
        assert!((weights.inhibitory - 0.3).abs() < 1e-6);
        assert!((weights.modulatory - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_creative_domain() {
        let weights = NeurotransmitterWeights::for_domain(Domain::Creative);
        assert!((weights.excitatory - 0.8).abs() < 1e-6);
        assert!((weights.inhibitory - 0.2).abs() < 1e-6);
        assert!((weights.modulatory - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_neutral_weights() {
        let weights = NeurotransmitterWeights::neutral();
        // net_activation = 0.5 - 0.5 + 0 = 0
        assert_eq!(weights, NeurotransmitterWeights::default());
    }

    #[test]
    fn test_domain_bonus() {
        assert!((NeurotransmitterWeights::domain_bonus(Domain::Code, Domain::Code) - 0.1).abs() < 1e-6);
        assert!((NeurotransmitterWeights::domain_bonus(Domain::Code, Domain::Legal) - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_lerp() {
        let a = NeurotransmitterWeights::new(0.0, 0.0, 0.0);
        let b = NeurotransmitterWeights::new(1.0, 1.0, 1.0);

        let mid = a.lerp(&b, 0.5);
        assert!((mid.excitatory - 0.5).abs() < 1e-6);
        assert!((mid.inhibitory - 0.5).abs() < 1e-6);
        assert!((mid.modulatory - 0.5).abs() < 1e-6);
    }

    #[test]
    fn test_serde_roundtrip() {
        let weights = NeurotransmitterWeights::for_domain(Domain::Code);
        let json = serde_json::to_string(&weights).unwrap();
        let loaded: NeurotransmitterWeights = serde_json::from_str(&json).unwrap();
        assert_eq!(weights, loaded);
    }

    #[test]
    fn test_domain_from_str() {
        assert_eq!(Domain::from_str("code"), Some(Domain::Code));
        assert_eq!(Domain::from_str("CODE"), Some(Domain::Code));
        assert_eq!(Domain::from_str("programming"), Some(Domain::Code));
        assert_eq!(Domain::from_str("unknown"), None);
    }
}
```
