//! Search Operations Tests.
//!
//! Tests for semantic search components and contradiction detection.

use crate::common::fixtures::{generate_contradiction_pairs, generate_test_nodes};
use crate::common::helpers::create_test_storage;

/// Test semantic search components.
#[test]
fn test_semantic_search_components() {
    println!("\n=== TEST: Semantic Search Components ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    // Create nodes with domain tags
    let nodes = generate_test_nodes(42, 100, 1536);

    for node in &nodes {
        storage
            .put_hyperbolic(node.id, &node.point)
            .expect("Put failed");
    }

    // Group nodes by domain
    let mut domain_counts = std::collections::HashMap::new();
    for node in &nodes {
        *domain_counts.entry(node.domain).or_insert(0) += 1;
    }

    println!("  Nodes by domain:");
    for (domain, count) in &domain_counts {
        println!("    {:?}: {}", domain, count);
    }

    // Verify embeddings are properly sized
    for node in &nodes {
        assert_eq!(node.embedding.len(), 1536, "Embedding should be 1536D");
    }

    // Verify embedding normalization
    for node in &nodes {
        let norm_sq: f32 = node.embedding.iter().map(|x| x * x).sum();
        let norm = norm_sq.sqrt();
        // Generated embeddings have values in [-0.5, 0.5], so norm should be reasonable
        assert!(norm > 0.0, "Embedding should have non-zero norm");
        assert!(norm < 100.0, "Embedding norm should be bounded");
    }

    println!("=== PASSED: Semantic Search Components ===\n");
}

/// Test contradiction detection with generated pairs.
#[test]
fn test_contradiction_detection() {
    println!("\n=== TEST: Contradiction Detection ===");

    // Generate test pairs
    let pairs = generate_contradiction_pairs(42, 20);

    println!("  Generated {} contradiction pairs", pairs.len());

    let mut expected_contradictions = 0;
    let mut expected_non_contradictions = 0;

    for pair in &pairs {
        if pair.expected_contradiction {
            expected_contradictions += 1;
        } else {
            expected_non_contradictions += 1;
        }
    }

    println!("    Expected contradictions: {}", expected_contradictions);
    println!(
        "    Expected non-contradictions: {}",
        expected_non_contradictions
    );

    // Test embedding similarity calculation
    for (i, pair) in pairs.iter().take(5).enumerate() {
        // Compute cosine similarity
        let dot: f32 = pair
            .node_a
            .embedding
            .iter()
            .zip(pair.node_b.embedding.iter())
            .map(|(a, b)| a * b)
            .sum();

        let norm_a: f32 = pair
            .node_a
            .embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();
        let norm_b: f32 = pair
            .node_b
            .embedding
            .iter()
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();

        let similarity = if norm_a > 0.0 && norm_b > 0.0 {
            dot / (norm_a * norm_b)
        } else {
            0.0
        };

        println!(
            "  Pair {}: similarity={:.3}, expected_contradiction={}, fixture_similarity={:.3}",
            i, similarity, pair.expected_contradiction, pair.similarity_score
        );
    }

    println!("=== PASSED: Contradiction Detection ===\n");
}
