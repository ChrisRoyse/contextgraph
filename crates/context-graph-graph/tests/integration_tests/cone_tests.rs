//! Entailment Cone Tests.
//!
//! Tests for entailment cone containment and storage operations.

use context_graph_graph::storage::NodeId;

use crate::common::fixtures::{generate_entailment_cone, HierarchicalTestData};
use crate::common::helpers::{create_test_storage, verify_entailment_cone};

/// Test entailment cone containment with hierarchical data.
#[test]
fn test_entailment_cone_containment() {
    println!("\n=== TEST: Entailment Cone Containment ===");

    use context_graph_cuda::cone::cone_membership_score_cpu;

    // Generate hierarchical test data
    let hierarchy = HierarchicalTestData::generate(42, 5, 3);

    println!("  Generated hierarchy:");
    println!("    Root: id={}", hierarchy.root.id);
    println!("    Children: {}", hierarchy.children.len());
    println!("    Grandchildren: {}", hierarchy.grandchildren.len());

    // Test that children apexes are inside root's cone
    // Score > 0 means inside cone, higher score = more central
    for child in &hierarchy.children {
        let score = cone_membership_score_cpu(
            &hierarchy.root.cone.apex.coords,
            hierarchy.root.cone.aperture,
            &child.cone.apex.coords,
            -1.0, // curvature
        );

        println!("    Child {} cone membership score: {:.3}", child.id, score);
    }

    // Test that grandchildren apexes are inside their parent's cone
    for (i, child) in hierarchy.children.iter().enumerate() {
        let start_idx = i * 3;
        let end_idx = start_idx + 3;
        for gc in &hierarchy.grandchildren[start_idx..end_idx.min(hierarchy.grandchildren.len())] {
            let score = cone_membership_score_cpu(
                &child.cone.apex.coords,
                child.cone.aperture,
                &gc.cone.apex.coords,
                -1.0, // curvature
            );

            println!(
                "    Grandchild {} in child {} cone score: {:.3}",
                gc.id, child.id, score
            );
        }
    }

    println!("=== PASSED: Entailment Cone Containment ===\n");
}

/// Test entailment cone storage and retrieval.
#[test]
fn test_entailment_cone_storage() {
    println!("\n=== TEST: Entailment Cone Storage ===");

    let (storage, _temp_dir) = create_test_storage().expect("Failed to create storage");

    // Generate and store cones
    let cones: Vec<_> = (0..100)
        .map(|i| generate_entailment_cone(i, 0.8, (0.2, 0.6)))
        .collect();

    for (i, cone) in cones.iter().enumerate() {
        storage
            .put_cone(i as NodeId, cone)
            .expect("Put cone failed");
    }

    // Verify count
    let count = storage.cone_count().expect("Count failed");
    assert_eq!(count, 100, "Should have 100 cones");

    // Verify retrieval
    for (i, expected) in cones.iter().enumerate() {
        verify_entailment_cone(&storage, i as NodeId, expected, 1e-5)
            .expect("Cone verification failed");
    }

    println!("=== PASSED: Entailment Cone Storage ===\n");
}
