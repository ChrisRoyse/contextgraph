---
id: "M04-T22"
title: "Implement get_modulated_weight Function (Marblestone)"
description: |
  Implement get_modulated_weight(edge, domain) standalone function.

  CANONICAL FORMULA:
  net_activation = excitatory - inhibitory + (modulatory * 0.5)
  effective_weight = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
  Result clamped to [0.0, 1.0]

  Pure function with no side effects.
  Used by traversal and search operations for domain-aware edge weighting.
layer: "surface"
status: "pending"
priority: "high"
estimated_hours: 1
sequence: 30
depends_on:
  - "M04-T16"
  - "M04-T17"
  - "M04-T17a"
spec_refs:
  - "TECH-GRAPH-004 Section 8"
  - "REQ-KG-065"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/marblestone/mod.rs"
    description: "Add get_modulated_weight function"
test_file: "crates/context-graph-graph/tests/marblestone_tests.rs"
---

## Context

The `get_modulated_weight` function is a core building block for Marblestone brain-inspired modulation throughout the knowledge graph. It provides a pure, side-effect-free computation of effective edge weights based on neurotransmitter profiles and domain context. This function is used by traversal algorithms (BFS, DFS, A*), search operations, and ranking systems to apply domain-aware edge weighting.

The CANONICAL FORMULA ensures consistency across all Marblestone operations:
- `net_activation = excitatory - inhibitory + (modulatory * 0.5)`
- `effective_weight = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor`

## Scope

### In Scope
- `get_modulated_weight()` pure function
- `net_activation()` helper
- Domain bonus calculation (0.1 for exact match)
- Steering factor application
- Result clamping to [0, 1]

### Out of Scope
- NT profile learning
- Edge mutation/updates
- Storage operations
- Traversal implementation (uses this function)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/marblestone/mod.rs

pub mod domain_search;

use crate::storage::edges::{GraphEdge, Domain, NeurotransmitterWeights};

/// Domain bonus for matching domains
pub const DOMAIN_MATCH_BONUS: f32 = 0.1;

/// Get effective edge weight with Marblestone modulation
///
/// This is a pure function with no side effects. It computes the
/// effective weight of an edge considering:
/// - Base edge weight
/// - Neurotransmitter activation (excitatory/inhibitory/modulatory)
/// - Domain matching bonus
/// - Steering factor from reinforcement learning
///
/// CANONICAL FORMULA:
/// ```text
/// net_activation = excitatory - inhibitory + (modulatory * 0.5)
/// effective_weight = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
/// ```
///
/// Result is clamped to [0.0, 1.0].
///
/// # Arguments
/// * `edge` - The graph edge to compute weight for
/// * `query_domain` - The domain context for the query
///
/// # Returns
/// * Effective weight in [0.0, 1.0]
///
/// # Example
/// ```rust
/// let edge = GraphEdge::semantic(1, 10, 20, 0.8);
/// let effective = get_modulated_weight(&edge, Domain::Code);
///
/// println!("Base: {}, Effective: {}", edge.weight, effective);
/// ```
pub fn get_modulated_weight(edge: &GraphEdge, query_domain: Domain) -> f32 {
    // Get NT weights for this edge
    let nt = &edge.neurotransmitter_weights;

    // CANONICAL FORMULA: net_activation
    let net_activation = nt.net_activation();

    // Domain bonus for matching
    let domain_bonus = if edge.domain == query_domain {
        DOMAIN_MATCH_BONUS
    } else {
        0.0
    };

    // Steering factor (from RL training, clamped to reasonable range)
    let steering_factor = edge.steering_reward.max(0.1).min(2.0);

    // CANONICAL FORMULA: effective_weight
    let effective = edge.weight * (1.0 + net_activation + domain_bonus) * steering_factor;

    // Clamp to [0.0, 1.0]
    effective.max(0.0).min(1.0)
}

/// Get effective edge weight using specific NT weights (not from edge)
///
/// Useful when computing with domain-specific profiles.
pub fn get_modulated_weight_with_nt(
    base_weight: f32,
    edge_domain: Domain,
    query_domain: Domain,
    nt_weights: &NeurotransmitterWeights,
    steering_factor: f32,
) -> f32 {
    let net_activation = nt_weights.net_activation();

    let domain_bonus = if edge_domain == query_domain {
        DOMAIN_MATCH_BONUS
    } else {
        0.0
    };

    let steering = steering_factor.max(0.1).min(2.0);
    let effective = base_weight * (1.0 + net_activation + domain_bonus) * steering;

    effective.max(0.0).min(1.0)
}

/// Calculate the boost ratio for an edge
///
/// Returns effective_weight / base_weight, showing how much
/// the modulation changes the weight.
pub fn modulation_ratio(edge: &GraphEdge, query_domain: Domain) -> f32 {
    let effective = get_modulated_weight(edge, query_domain);
    if edge.weight > 1e-6 {
        effective / edge.weight
    } else {
        1.0
    }
}

/// Check if an edge is boosted or suppressed for a domain
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationEffect {
    /// Weight increased
    Boosted,
    /// Weight unchanged (ratio near 1.0)
    Neutral,
    /// Weight decreased
    Suppressed,
}

/// Determine the modulation effect for an edge
pub fn modulation_effect(edge: &GraphEdge, query_domain: Domain) -> ModulationEffect {
    let ratio = modulation_ratio(edge, query_domain);

    if ratio > 1.05 {
        ModulationEffect::Boosted
    } else if ratio < 0.95 {
        ModulationEffect::Suppressed
    } else {
        ModulationEffect::Neutral
    }
}

/// Get expected modulation for a domain (without specific edge)
///
/// Returns the multiplier that would be applied to base_weight
/// assuming domain match and default steering.
pub fn expected_domain_modulation(domain: Domain) -> f32 {
    let nt = NeurotransmitterWeights::for_domain(domain);
    let net_activation = nt.net_activation();

    // Assuming domain match and neutral steering
    1.0 + net_activation + DOMAIN_MATCH_BONUS
}

/// Apply modulation to edge weight for traversal cost
///
/// For traversal, lower weight = lower cost = preferred path.
/// This inverts the modulation: high effective weight = low cost.
pub fn traversal_cost(edge: &GraphEdge, query_domain: Domain) -> f32 {
    let effective = get_modulated_weight(edge, query_domain);

    // Invert: high weight = low cost
    1.0 - effective
}

/// Batch modulation for multiple edges
pub fn get_modulated_weights_batch(
    edges: &[GraphEdge],
    query_domain: Domain,
) -> Vec<f32> {
    edges.iter()
        .map(|e| get_modulated_weight(e, query_domain))
        .collect()
}

/// Summary of modulation for debugging
#[derive(Debug, Clone)]
pub struct ModulationSummary {
    pub base_weight: f32,
    pub effective_weight: f32,
    pub net_activation: f32,
    pub domain_bonus: f32,
    pub steering_factor: f32,
    pub ratio: f32,
    pub effect: ModulationEffect,
}

impl ModulationSummary {
    pub fn from_edge(edge: &GraphEdge, query_domain: Domain) -> Self {
        let nt = &edge.neurotransmitter_weights;
        let net_activation = nt.net_activation();
        let domain_bonus = if edge.domain == query_domain { DOMAIN_MATCH_BONUS } else { 0.0 };
        let steering_factor = edge.steering_reward.max(0.1).min(2.0);
        let effective = get_modulated_weight(edge, query_domain);
        let ratio = if edge.weight > 1e-6 { effective / edge.weight } else { 1.0 };

        Self {
            base_weight: edge.weight,
            effective_weight: effective,
            net_activation,
            domain_bonus,
            steering_factor,
            ratio,
            effect: modulation_effect(edge, query_domain),
        }
    }
}

impl std::fmt::Display for ModulationSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "base={:.3} eff={:.3} (net_act={:+.3} dom_bonus={:.2} steer={:.2}) ratio={:.2}x {:?}",
            self.base_weight,
            self.effective_weight,
            self.net_activation,
            self.domain_bonus,
            self.steering_factor,
            self.ratio,
            self.effect,
        )
    }
}
```

### Constraints
- MUST be pure function (no side effects)
- MUST use CANONICAL formula exactly
- Result MUST be clamped to [0.0, 1.0]
- Domain match bonus = 0.1
- Steering factor clamped to [0.1, 2.0]

### Acceptance Criteria
- [ ] get_modulated_weight() is pure function
- [ ] Applies CANONICAL NT modulation formula
- [ ] Result clamped to [0.0, 1.0]
- [ ] Domain match adds bonus (0.1 for exact match)
- [ ] Unit tests verify edge cases
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
```
get_modulated_weight(edge, query_domain):
    # Step 1: Get NT weights
    nt = edge.neurotransmitter_weights

    # Step 2: CANONICAL net_activation formula
    net_activation = nt.excitatory - nt.inhibitory + (nt.modulatory * 0.5)

    # Step 3: Domain bonus
    domain_bonus = 0.1 if edge.domain == query_domain else 0.0

    # Step 4: Steering factor (clamped)
    steering = clamp(edge.steering_reward, 0.1, 2.0)

    # Step 5: CANONICAL effective_weight formula
    effective = edge.weight * (1.0 + net_activation + domain_bonus) * steering

    # Step 6: Clamp result
    return clamp(effective, 0.0, 1.0)
```

### Domain NT Profiles (reference)
| Domain | Excitatory | Inhibitory | Modulatory | Net Activation |
|--------|------------|------------|------------|----------------|
| Code | 0.7 | 0.3 | 0.2 | +0.50 |
| Creative | 0.8 | 0.2 | 0.5 | +0.85 |
| Legal | 0.5 | 0.4 | 0.3 | +0.25 |
| Medical | 0.6 | 0.3 | 0.2 | +0.40 |
| Research | 0.6 | 0.2 | 0.4 | +0.60 |
| General | 0.5 | 0.5 | 0.0 | 0.00 |

### Edge Cases
- Zero base weight: Returns 0.0
- Negative net activation: May reduce weight below base
- Very high activation: Clamped to 1.0 maximum
- Zero steering factor: Clamped to 0.1 minimum
- Mismatched domain: No bonus, may still boost/suppress

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph modulated_weight
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Formula matches spec exactly
- [ ] Pure function (no state changes)
- [ ] Domain bonus applied correctly
- [ ] Steering factor used correctly

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_edge(weight: f32, domain: Domain) -> GraphEdge {
        let mut edge = GraphEdge::new(
            1, 10, 20,
            EdgeType::Semantic,
            weight,
            domain,
        );
        edge.steering_reward = 1.0;  // Neutral steering
        edge
    }

    #[test]
    fn test_canonical_formula_code_domain() {
        let edge = make_test_edge(0.8, Domain::Code);

        // Code domain NT: exc=0.7, inh=0.3, mod=0.2
        // net_activation = 0.7 - 0.3 + (0.2 * 0.5) = 0.5
        // domain_bonus = 0.1 (matching)
        // steering = 1.0
        // effective = 0.8 * (1.0 + 0.5 + 0.1) * 1.0 = 0.8 * 1.6 = 1.28 -> clamped to 1.0

        let effective = get_modulated_weight(&edge, Domain::Code);
        assert!((effective - 1.0).abs() < 1e-6);  // Clamped
    }

    #[test]
    fn test_canonical_formula_general_domain() {
        let edge = make_test_edge(0.8, Domain::General);

        // General domain NT: exc=0.5, inh=0.5, mod=0.0
        // net_activation = 0.5 - 0.5 + 0.0 = 0.0
        // domain_bonus = 0.1 (matching)
        // steering = 1.0
        // effective = 0.8 * (1.0 + 0.0 + 0.1) * 1.0 = 0.8 * 1.1 = 0.88

        let effective = get_modulated_weight(&edge, Domain::General);
        assert!((effective - 0.88).abs() < 1e-6);
    }

    #[test]
    fn test_no_domain_bonus_when_mismatched() {
        let edge = make_test_edge(0.8, Domain::Code);

        // Query with different domain
        // Code edge NT still applies, but no domain bonus
        // net_activation = 0.5
        // domain_bonus = 0.0 (not matching)
        // effective = 0.8 * (1.0 + 0.5 + 0.0) * 1.0 = 0.8 * 1.5 = 1.2 -> clamped to 1.0

        let effective = get_modulated_weight(&edge, Domain::Legal);
        assert!((effective - 1.0).abs() < 1e-6);  // Still clamped
    }

    #[test]
    fn test_steering_factor() {
        let mut edge = make_test_edge(0.5, Domain::General);
        edge.steering_reward = 1.5;  // 50% boost from RL

        // net_activation = 0.0
        // domain_bonus = 0.1
        // steering = 1.5
        // effective = 0.5 * (1.0 + 0.0 + 0.1) * 1.5 = 0.5 * 1.1 * 1.5 = 0.825

        let effective = get_modulated_weight(&edge, Domain::General);
        assert!((effective - 0.825).abs() < 1e-6);
    }

    #[test]
    fn test_steering_factor_clamping() {
        let mut edge = make_test_edge(0.5, Domain::General);
        edge.steering_reward = 0.0;  // Would be 0, clamped to 0.1

        // steering = 0.1 (clamped from 0)
        // effective = 0.5 * 1.1 * 0.1 = 0.055

        let effective = get_modulated_weight(&edge, Domain::General);
        assert!((effective - 0.055).abs() < 1e-6);
    }

    #[test]
    fn test_zero_base_weight() {
        let edge = make_test_edge(0.0, Domain::Code);

        let effective = get_modulated_weight(&edge, Domain::Code);
        assert!((effective - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_clamping_to_one() {
        let mut edge = make_test_edge(1.0, Domain::Creative);
        edge.steering_reward = 2.0;

        // Creative: net_activation = 0.85
        // domain_bonus = 0.1
        // steering = 2.0
        // effective = 1.0 * (1.0 + 0.85 + 0.1) * 2.0 = 1.0 * 1.95 * 2.0 = 3.9 -> clamped to 1.0

        let effective = get_modulated_weight(&edge, Domain::Creative);
        assert!((effective - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_modulation_summary() {
        let edge = make_test_edge(0.5, Domain::Code);
        let summary = ModulationSummary::from_edge(&edge, Domain::Code);

        assert!((summary.base_weight - 0.5).abs() < 1e-6);
        assert!((summary.net_activation - 0.5).abs() < 1e-6);
        assert!((summary.domain_bonus - 0.1).abs() < 1e-6);
        assert_eq!(summary.effect, ModulationEffect::Boosted);
    }

    #[test]
    fn test_batch_modulation() {
        let edges = vec![
            make_test_edge(0.3, Domain::Code),
            make_test_edge(0.5, Domain::Legal),
            make_test_edge(0.7, Domain::General),
        ];

        let weights = get_modulated_weights_batch(&edges, Domain::Code);

        assert_eq!(weights.len(), 3);
        // First edge (Code) should have highest boost due to domain match
    }

    #[test]
    fn test_traversal_cost_inversion() {
        let edge = make_test_edge(0.8, Domain::Code);

        let effective = get_modulated_weight(&edge, Domain::Code);
        let cost = traversal_cost(&edge, Domain::Code);

        // cost = 1.0 - effective
        assert!((cost + effective - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_expected_domain_modulation() {
        // Code: 1.0 + 0.5 + 0.1 = 1.6
        let code_mod = expected_domain_modulation(Domain::Code);
        assert!((code_mod - 1.6).abs() < 1e-6);

        // General: 1.0 + 0.0 + 0.1 = 1.1
        let general_mod = expected_domain_modulation(Domain::General);
        assert!((general_mod - 1.1).abs() < 1e-6);
    }
}
```
