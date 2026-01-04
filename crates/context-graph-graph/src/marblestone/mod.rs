//! Marblestone neurotransmitter integration.
//!
//! This module re-exports types from context-graph-core and adds
//! graph-specific operations for NT-weighted edge traversal.
//!
//! # Neurotransmitter Model (GraphEdge - CANONICAL)
//!
//! Edges in the graph have NT weights that modulate effective edge weight
//! based on the current domain context:
//!
//! ```text
//! net_activation = excitatory - inhibitory + (modulatory * 0.5)
//! domain_bonus = 0.1 if edge_domain == query_domain else 0.0
//! steering_factor = 0.5 + steering_reward  // Range [0.5, 1.5]
//! w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
//! Result clamped to [0.0, 1.0]
//! ```
//!
//! # Domain-Specific Modulation
//!
//! Different domains activate different NT profiles:
//! - Code: e=0.6, i=0.3, m=0.4 → net_act=0.5 → multiplier 1.6
//! - Legal: e=0.4, i=0.4, m=0.2 → net_act=0.1 → multiplier 1.2
//! - Medical: e=0.5, i=0.3, m=0.5 → net_act=0.45 → multiplier 1.55
//! - Creative: e=0.8, i=0.1, m=0.6 → net_act=1.0 → multiplier 2.1
//! - Research: e=0.6, i=0.2, m=0.5 → net_act=0.65 → multiplier 1.75
//! - General: e=0.5, i=0.2, m=0.3 → net_act=0.45 → multiplier 1.55
//!
//! # Components
//!
//! - Re-exports from context-graph-core
//! - Validation: `validate_or_error()` for Result-returning validation (M04-T14a)
//! - Domain-aware search (M04-T19 COMPLETE)
//! - Standalone modulation utilities (M04-T22 COMPLETE)
//!
//! # Constitution Reference
//!
//! - edge_model.nt_weights: Definition and formula
//! - edge_model.nt_weights.domain: Code|Legal|Medical|Creative|Research|General
//! - AP-001: Never unwrap() in prod - all errors properly typed

mod validation;

// Re-export from core for convenience
pub use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};

// Re-export validation functions (M04-T14a)
pub use validation::{compute_effective_validated, validate_or_error};

// Re-export domain-aware search (M04-T19)
pub use crate::search::domain_search::{
    domain_aware_search, domain_nt_summary, expected_domain_boost, DomainSearchResult,
    DomainSearchResults,
};

// =============================================================================
// STANDALONE UTILITY FUNCTIONS (M04-T22)
// =============================================================================

use crate::storage::edges::GraphEdge;

/// Domain match bonus constant (matches GraphEdge implementation).
pub const DOMAIN_MATCH_BONUS: f32 = 0.1;

/// Get effective edge weight with Marblestone modulation.
///
/// This is a **standalone wrapper** around `GraphEdge.get_modulated_weight()`.
/// Use when you have an edge reference and want functional-style API.
///
/// # Canonical Formula (delegated to GraphEdge)
///
/// ```text
/// net_activation = excitatory - inhibitory + (modulatory * 0.5)
/// domain_bonus = 0.1 if edge_domain == query_domain else 0.0
/// steering_factor = 0.5 + steering_reward  // Range [0.5, 1.5]
/// w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
/// Result clamped to [0.0, 1.0]
/// ```
///
/// # Arguments
/// * `edge` - The graph edge to compute weight for
/// * `query_domain` - The domain context for the query
///
/// # Returns
/// Effective weight in [0.0, 1.0]
#[inline]
pub fn get_modulated_weight(edge: &GraphEdge, query_domain: Domain) -> f32 {
    edge.get_modulated_weight(query_domain)
}

/// Compute traversal cost from edge weight.
///
/// For pathfinding algorithms (BFS, DFS, A*), lower cost = preferred path.
/// This inverts the modulated weight: high weight = low cost.
///
/// # Formula
/// ```text
/// cost = 1.0 - get_modulated_weight(edge, query_domain)
/// ```
#[inline]
pub fn traversal_cost(edge: &GraphEdge, query_domain: Domain) -> f32 {
    1.0 - edge.get_modulated_weight(query_domain)
}

/// Calculate the modulation ratio (effective / base).
///
/// Shows how much the modulation changes the weight:
/// - ratio > 1.0: Weight boosted
/// - ratio < 1.0: Weight suppressed
/// - ratio = 1.0: No change
///
/// Returns 1.0 if base_weight is zero (avoid division by zero).
#[inline]
pub fn modulation_ratio(edge: &GraphEdge, query_domain: Domain) -> f32 {
    let effective = edge.get_modulated_weight(query_domain);
    if edge.weight > 1e-6 {
        effective / edge.weight
    } else {
        1.0
    }
}

/// Batch compute modulated weights for multiple edges.
pub fn get_modulated_weights_batch(edges: &[GraphEdge], query_domain: Domain) -> Vec<f32> {
    edges
        .iter()
        .map(|e| e.get_modulated_weight(query_domain))
        .collect()
}

/// Batch compute traversal costs for multiple edges.
pub fn traversal_costs_batch(edges: &[GraphEdge], query_domain: Domain) -> Vec<f32> {
    edges.iter().map(|e| traversal_cost(e, query_domain)).collect()
}

/// Modulation effect classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModulationEffect {
    /// Weight increased (ratio > 1.05)
    Boosted,
    /// Weight unchanged (ratio in [0.95, 1.05])
    Neutral,
    /// Weight decreased (ratio < 0.95)
    Suppressed,
}

/// Determine if edge is boosted or suppressed for a domain.
#[inline]
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

/// Expected modulation multiplier for a domain (assuming match + neutral steering).
///
/// # Formula
/// ```text
/// nt = NeurotransmitterWeights::for_domain(domain)
/// net_activation = nt.excitatory - nt.inhibitory + (nt.modulatory * 0.5)
/// multiplier = 1.0 + net_activation + DOMAIN_MATCH_BONUS
/// ```
/// Note: Assumes neutral steering (steering_factor = 1.0).
pub fn expected_domain_modulation(domain: Domain) -> f32 {
    let nt = NeurotransmitterWeights::for_domain(domain);
    let net_activation = nt.excitatory - nt.inhibitory + (nt.modulatory * 0.5);
    1.0 + net_activation + DOMAIN_MATCH_BONUS
}

/// Detailed modulation summary for debugging.
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
    /// Create summary from edge and query domain.
    pub fn from_edge(edge: &GraphEdge, query_domain: Domain) -> Self {
        let nt = &edge.neurotransmitter_weights;
        let net_activation = nt.excitatory - nt.inhibitory + (nt.modulatory * 0.5);
        let domain_bonus = if edge.domain == query_domain {
            DOMAIN_MATCH_BONUS
        } else {
            0.0
        };
        let steering_factor = 0.5 + edge.steering_reward;
        let effective = edge.get_modulated_weight(query_domain);
        let ratio = if edge.weight > 1e-6 {
            effective / edge.weight
        } else {
            1.0
        };

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
            "base={:.3} eff={:.3} (net_act={:+.3} dom={:.2} steer={:.2}) ratio={:.2}x {:?}",
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

// =============================================================================
// TESTS (M04-T22)
// =============================================================================

#[cfg(test)]
mod modulation_tests {
    use super::*;
    use uuid::Uuid;

    fn make_test_edge(weight: f32, domain: Domain) -> GraphEdge {
        let mut edge = GraphEdge::new(
            1,
            Uuid::new_v4(),
            Uuid::new_v4(),
            EdgeType::Semantic,
            weight,
            domain,
        );
        // Set steering_reward to 0.5 so steering_factor = 1.0 (neutral)
        edge.steering_reward = 0.5;
        edge
    }

    #[test]
    fn test_standalone_matches_method() {
        let edge = make_test_edge(0.8, Domain::Code);
        let standalone = get_modulated_weight(&edge, Domain::Code);
        let method = edge.get_modulated_weight(Domain::Code);
        assert!(
            (standalone - method).abs() < 1e-6,
            "Standalone should match method: {} vs {}",
            standalone,
            method
        );
    }

    #[test]
    fn test_code_domain_modulation() {
        let edge = make_test_edge(0.5, Domain::Code);
        // Code: e=0.6, i=0.3, m=0.4
        // net_activation = 0.6 - 0.3 + 0.2 = 0.5
        // domain_bonus = 0.1 (matching)
        // steering_factor = 0.5 + 0.5 = 1.0
        // w_eff = 0.5 * (1.0 + 0.5 + 0.1) * 1.0 = 0.5 * 1.6 = 0.8
        let w = get_modulated_weight(&edge, Domain::Code);
        assert!((w - 0.8).abs() < 0.01, "Expected ~0.8, got {}", w);
    }

    #[test]
    fn test_domain_mismatch_no_bonus() {
        let edge = make_test_edge(0.5, Domain::Code);
        // Query Legal, edge is Code
        // net_activation = 0.5 (Code profile still applies)
        // domain_bonus = 0.0 (no match)
        // w_eff = 0.5 * (1.0 + 0.5 + 0.0) * 1.0 = 0.5 * 1.5 = 0.75
        let w = get_modulated_weight(&edge, Domain::Legal);
        assert!((w - 0.75).abs() < 0.01, "Expected ~0.75, got {}", w);
    }

    #[test]
    fn test_traversal_cost_inversion() {
        let edge = make_test_edge(0.8, Domain::Code);
        let effective = get_modulated_weight(&edge, Domain::Code);
        let cost = traversal_cost(&edge, Domain::Code);
        assert!(
            (cost + effective - 1.0).abs() < 1e-6,
            "cost + effective should = 1.0"
        );
    }

    #[test]
    fn test_modulation_ratio() {
        let edge = make_test_edge(0.5, Domain::Code);
        let ratio = modulation_ratio(&edge, Domain::Code);
        // effective = 0.8, base = 0.5, ratio = 1.6
        assert!((ratio - 1.6).abs() < 0.01, "Expected ratio ~1.6, got {}", ratio);
    }

    #[test]
    fn test_modulation_effect_boosted() {
        let edge = make_test_edge(0.5, Domain::Code);
        let effect = modulation_effect(&edge, Domain::Code);
        assert_eq!(effect, ModulationEffect::Boosted);
    }

    #[test]
    fn test_zero_base_weight() {
        let edge = make_test_edge(0.0, Domain::Code);
        let w = get_modulated_weight(&edge, Domain::Code);
        assert!((w - 0.0).abs() < 1e-6, "Zero base should give zero effective");
        let ratio = modulation_ratio(&edge, Domain::Code);
        assert!((ratio - 1.0).abs() < 1e-6, "Zero base should give ratio 1.0");
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
        // First edge (Code) has domain match, should be highest ratio
    }

    #[test]
    fn test_expected_domain_modulation() {
        // Code: net_act = 0.5, mult = 1.0 + 0.5 + 0.1 = 1.6
        let code_mod = expected_domain_modulation(Domain::Code);
        assert!((code_mod - 1.6).abs() < 0.01, "Code should be ~1.6, got {}", code_mod);

        // General: net_act = 0.5 - 0.2 + 0.15 = 0.45, mult = 1.55
        let general_mod = expected_domain_modulation(Domain::General);
        assert!(
            (general_mod - 1.55).abs() < 0.01,
            "General should be ~1.55, got {}",
            general_mod
        );
    }

    #[test]
    fn test_modulation_summary() {
        let edge = make_test_edge(0.5, Domain::Code);
        let summary = ModulationSummary::from_edge(&edge, Domain::Code);

        assert!((summary.base_weight - 0.5).abs() < 1e-6);
        assert!((summary.net_activation - 0.5).abs() < 0.01);
        assert!((summary.domain_bonus - 0.1).abs() < 1e-6);
        assert!((summary.steering_factor - 1.0).abs() < 1e-6);
        assert_eq!(summary.effect, ModulationEffect::Boosted);
    }

    #[test]
    fn test_clamping_high_values() {
        let mut edge = make_test_edge(1.0, Domain::Creative);
        edge.steering_reward = 1.0; // steering_factor = 1.5
        // Creative: net_act = 0.8 - 0.1 + 0.3 = 1.0
        // w_eff = 1.0 * (1.0 + 1.0 + 0.1) * 1.5 = 3.15 -> clamped to 1.0
        let w = get_modulated_weight(&edge, Domain::Creative);
        assert!((w - 1.0).abs() < 1e-6, "Should clamp to 1.0");
    }

    #[test]
    fn test_steering_affects_output() {
        // Use low base weight (0.2) to avoid clamping at high modulation
        // General: net_act = 0.5 - 0.2 + 0.15 = 0.45
        // With domain match bonus = 0.1: mult = 1.0 + 0.45 + 0.1 = 1.55
        // w1 = 0.2 * 1.55 * 0.5 = 0.155 (steering_factor = 0.5)
        // w2 = 0.2 * 1.55 * 1.5 = 0.465 (steering_factor = 1.5)
        // ratio = 0.465 / 0.155 = 3.0
        let mut edge1 = make_test_edge(0.2, Domain::General);
        edge1.steering_reward = 0.0; // steering_factor = 0.5

        let mut edge2 = make_test_edge(0.2, Domain::General);
        edge2.steering_reward = 1.0; // steering_factor = 1.5

        let w1 = get_modulated_weight(&edge1, Domain::General);
        let w2 = get_modulated_weight(&edge2, Domain::General);

        println!("Steering test: w1={}, w2={}, ratio={}", w1, w2, w2 / w1);

        // w2 should be 3x w1 (1.5 / 0.5 = 3)
        assert!(w2 > w1, "Higher steering should give higher weight");
        assert!((w2 / w1 - 3.0).abs() < 0.1, "Ratio should be ~3x, got {}", w2 / w1);
    }

    // ========== EDGE CASE TESTS WITH BEFORE/AFTER STATE ==========

    #[test]
    fn test_edge_case_zero_weight() {
        let edge = make_test_edge(0.0, Domain::Code);
        println!("BEFORE: base_weight={}, domain={:?}", edge.weight, edge.domain);
        let w = get_modulated_weight(&edge, Domain::Code);
        println!("AFTER: effective_weight={}", w);
        assert!((w - 0.0).abs() < 1e-6, "Zero base must give zero effective");
    }

    #[test]
    fn test_edge_case_clamp() {
        let mut edge = make_test_edge(1.0, Domain::Creative);
        edge.steering_reward = 1.0;
        println!("BEFORE: base={}, steering_reward={}, domain=Creative", edge.weight, edge.steering_reward);
        let w = get_modulated_weight(&edge, Domain::Creative);
        println!("AFTER: effective={} (expected: clamped to 1.0)", w);
        assert!((w - 1.0).abs() < 1e-6, "Must clamp to 1.0");
    }

    #[test]
    fn test_edge_case_domain_mismatch() {
        let edge = make_test_edge(0.5, Domain::Code);
        println!("BEFORE: edge_domain=Code, query_domain=Legal");
        let w_match = get_modulated_weight(&edge, Domain::Code);
        let w_mismatch = get_modulated_weight(&edge, Domain::Legal);
        println!("AFTER: match_weight={}, mismatch_weight={}", w_match, w_mismatch);
        println!("Difference (domain bonus): {}", w_match - w_mismatch);
        assert!(w_match > w_mismatch, "Domain match must give higher weight");
    }

    #[test]
    fn test_modulation_summary_display() {
        let edge = make_test_edge(0.5, Domain::Code);
        let summary = ModulationSummary::from_edge(&edge, Domain::Code);
        let display = format!("{}", summary);
        println!("ModulationSummary Display: {}", display);
        assert!(display.contains("base="));
        assert!(display.contains("eff="));
        assert!(display.contains("Boosted"));
    }

    #[test]
    fn test_batch_traversal_costs() {
        let edges = vec![
            make_test_edge(0.3, Domain::Code),
            make_test_edge(0.5, Domain::Legal),
            make_test_edge(0.7, Domain::General),
        ];
        let costs = traversal_costs_batch(&edges, Domain::Code);
        assert_eq!(costs.len(), 3);

        // Verify each cost = 1.0 - modulated_weight
        for (i, edge) in edges.iter().enumerate() {
            let expected_cost = 1.0 - edge.get_modulated_weight(Domain::Code);
            assert!(
                (costs[i] - expected_cost).abs() < 1e-6,
                "Cost mismatch for edge {}: {} vs {}",
                i, costs[i], expected_cost
            );
        }
    }

    #[test]
    fn test_domain_match_bonus_constant() {
        assert!((DOMAIN_MATCH_BONUS - 0.1).abs() < 1e-6, "DOMAIN_MATCH_BONUS should be 0.1");
    }

    #[test]
    fn test_all_domain_modulations() {
        // Verify all domain modulation values are correct
        let domains = [
            (Domain::Code, 1.6),      // 1.0 + 0.5 + 0.1
            (Domain::Legal, 1.2),     // 1.0 + 0.1 + 0.1
            (Domain::Medical, 1.55),  // 1.0 + 0.45 + 0.1
            (Domain::Creative, 2.1),  // 1.0 + 1.0 + 0.1
            (Domain::Research, 1.75), // 1.0 + 0.65 + 0.1
            (Domain::General, 1.55),  // 1.0 + 0.45 + 0.1
        ];

        for (domain, expected) in domains {
            let actual = expected_domain_modulation(domain);
            assert!(
                (actual - expected).abs() < 0.01,
                "Domain {:?}: expected {}, got {}",
                domain, expected, actual
            );
        }
    }
}
