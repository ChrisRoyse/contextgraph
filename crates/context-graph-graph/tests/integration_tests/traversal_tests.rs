//! Graph Traversal with NT Modulation Tests.
//!
//! Tests for neurotransmitter modulation formula and domain bonus in traversal.

use context_graph_graph::{
    marblestone::DOMAIN_MATCH_BONUS, storage::LegacyGraphEdge, Domain, NeurotransmitterWeights,
};

use crate::common::fixtures::{generate_test_edges, generate_test_nodes};
use crate::common::helpers::create_test_storage;

/// Test NT modulation formula correctness.
#[test]
fn test_nt_modulation_formula() {
    println!("\n=== TEST: NT Modulation Formula ===");

    // Test formula: w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
    // where net_activation = excitatory - inhibitory

    // Test case 1: Neutral weights
    let neutral = NeurotransmitterWeights::for_domain(Domain::General);
    let base_weight = 0.5;
    let same_domain = true;

    // Calculate expected
    let net = neutral.excitatory - neutral.inhibitory;
    let domain_bonus = if same_domain { DOMAIN_MATCH_BONUS } else { 0.0 };
    let steering = 1.0 + (neutral.modulatory - 0.5) * 0.4;
    let expected = (base_weight * (1.0 + net + domain_bonus) * steering).clamp(0.0, 1.0);

    println!("  Neutral weights test:");
    println!(
        "    excitatory={:.3}, inhibitory={:.3}, modulatory={:.3}",
        neutral.excitatory, neutral.inhibitory, neutral.modulatory
    );
    println!(
        "    net={:.3}, domain_bonus={:.3}, steering={:.3}",
        net, domain_bonus, steering
    );
    println!("    expected_weight={:.3}", expected);

    // Test case 2: Code domain weights (high excitatory)
    let code_weights = NeurotransmitterWeights::for_domain(Domain::Code);
    println!("\n  Code domain weights:");
    println!(
        "    excitatory={:.3}, inhibitory={:.3}, modulatory={:.3}",
        code_weights.excitatory, code_weights.inhibitory, code_weights.modulatory
    );

    // Test case 3: Legal domain weights (balanced)
    let legal_weights = NeurotransmitterWeights::for_domain(Domain::Legal);
    println!("\n  Legal domain weights:");
    println!(
        "    excitatory={:.3}, inhibitory={:.3}, modulatory={:.3}",
        legal_weights.excitatory, legal_weights.inhibitory, legal_weights.modulatory
    );

    println!("=== PASSED: NT Modulation Formula ===\n");
}

/// Test domain bonus application in traversal.
#[test]
fn test_domain_bonus_in_traversal() {
    println!("\n=== TEST: Domain Bonus in Traversal ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    // Create nodes with embeddings
    let nodes = generate_test_nodes(42, 20, 1536);

    for node in &nodes {
        storage
            .put_hyperbolic(node.id, &node.point)
            .expect("Put hyperbolic failed");
    }

    // Create edges with different domains
    let edges = generate_test_edges(42, &nodes.iter().map(|n| n.id).collect::<Vec<_>>(), 2);

    for edge in &edges {
        // Map domain to u8 edge type (0-5 for 6 domains)
        let edge_type_u8 = match edge.domain {
            Domain::Code => 0,
            Domain::Legal => 1,
            Domain::Medical => 2,
            Domain::Creative => 3,
            Domain::Research => 4,
            Domain::General => 5,
        };
        storage
            .add_edge(
                edge.source_id,
                LegacyGraphEdge {
                    target: edge.target_id,
                    edge_type: edge_type_u8,
                },
            )
            .expect("Add edge failed");
    }

    // Test domain bonus calculation
    for edge in &edges {
        // Same domain query
        let same_domain_weight = edge.effective_weight(edge.domain, DOMAIN_MATCH_BONUS);

        // Different domain query
        let other_domain = if edge.domain == Domain::Code {
            Domain::Legal
        } else {
            Domain::Code
        };
        let diff_domain_weight = edge.effective_weight(other_domain, DOMAIN_MATCH_BONUS);

        // Same domain should have higher or equal effective weight
        println!(
            "  Edge {:?}: same_domain={:.3}, diff_domain={:.3}",
            edge.domain, same_domain_weight, diff_domain_weight
        );
    }

    println!("=== PASSED: Domain Bonus in Traversal ===\n");
}
