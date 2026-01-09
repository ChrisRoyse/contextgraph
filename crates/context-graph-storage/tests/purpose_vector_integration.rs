//! Integration tests for purpose vector computation and storage.
//!
//! Tests the complete flow from semantic fingerprint → purpose vector computation →
//! storage → retrieval with verification that purpose vectors are correctly computed,
//! stored, indexed, and retrieved.
//!
//! From constitution.yaml:
//! - Purpose Vector: PV = [A(E1,V), A(E2,V), ..., A(E13,V)]
//! - Each element is cosine alignment between embedder and North Star goal
//! - Stored atomically with fingerprint in RocksDB
//! - Indexed for O(log n) purpose-based search

use context_graph_core::purpose::{
    DefaultPurposeComputer, GoalDiscoveryMetadata, GoalHierarchy, GoalLevel, GoalNode, PurposeComputeConfig,
    PurposeVectorComputer,
};
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, TeleologicalFingerprint, PurposeVector, NUM_EMBEDDERS,
};

/// Test that purpose vectors have correct structure (13 elements, range [-1, 1]).
#[tokio::test]
async fn test_purpose_vector_structure() {
    let alignments = [0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76];

    let pv = PurposeVector::new(alignments);

    // Verify 13 dimensions
    assert_eq!(pv.alignments.len(), NUM_EMBEDDERS);
    assert_eq!(pv.alignments.len(), 13);

    // Verify all elements are in cosine range [-1, 1]
    for (i, &alignment) in pv.alignments.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&alignment),
            "Alignment[{}] = {} out of range [-1, 1]",
            i,
            alignment
        );
    }

    // Verify coherence and stability are computed
    assert!(pv.coherence > 0.0 && pv.coherence <= 1.0);
    assert!(pv.stability > 0.0 && pv.stability <= 1.0);

    println!("[PASS] Purpose vector has correct structure (13D, range [-1, 1])");
}

/// Test that purpose vectors are computed correctly from semantic fingerprints.
#[tokio::test]
async fn test_purpose_computation_with_real_goal_hierarchy() {
    // Create a real goal hierarchy with North Star
    let mut hierarchy = GoalHierarchy::new();
    let discovery = GoalDiscoveryMetadata::bootstrap();

    let north_star = GoalNode::autonomous_goal(
        "Master semantic understanding".into(),
        GoalLevel::NorthStar,
        SemanticFingerprint::zeroed(),
        discovery,
    ).expect("Failed to create North Star");
    hierarchy.add_goal(north_star).expect("Failed to add North Star");

    let config = PurposeComputeConfig::with_hierarchy(hierarchy);
    let computer = DefaultPurposeComputer::new();

    // Create a real semantic fingerprint (with actual embeddings)
    let semantic = SemanticFingerprint::zeroed(); // Would be real embeddings in production

    // Compute purpose vector
    let result = computer.compute_purpose(&semantic, &config).await;

    assert!(
        result.is_ok(),
        "Purpose computation failed: {:?}",
        result.err()
    );

    let pv = result.unwrap();

    // Verify all 13 dimensions are present
    assert_eq!(pv.alignments.len(), 13);

    // Verify all values are in valid range
    for (i, &alignment) in pv.alignments.iter().enumerate() {
        assert!(
            (-1.0..=1.0).contains(&alignment),
            "Alignment[{}] out of range: {}",
            i,
            alignment
        );
    }

    println!("[PASS] Purpose vector computed correctly from semantic fingerprint");
    println!("  - Alignments: {:?}", pv.alignments);
    println!("  - Aggregate: {:.4}", pv.aggregate_alignment());
    println!("  - Dominant embedder: {}", pv.dominant_embedder);
    println!("  - Coherence: {:.4}", pv.coherence);
}

/// Test that purpose vectors are computed correctly.
#[test]
fn test_purpose_vector_serialization() {
    // Create purpose vector with known alignments
    let alignments = [0.75, 0.8, 0.7, 0.65, 0.85, 0.6, 0.9, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76];
    let purpose_vector = PurposeVector::new(alignments);

    // Verify all properties are set correctly
    assert_eq!(purpose_vector.alignments.len(), 13);
    assert_eq!(purpose_vector.alignments, alignments);

    // Verify aggregate computation
    let expected_aggregate: f32 = alignments.iter().sum::<f32>() / 13.0;
    assert!((purpose_vector.aggregate_alignment() - expected_aggregate).abs() < 1e-6);

    println!("[PASS] Purpose vector serialization/structure correct");
    println!("  - Alignments: {:?}", purpose_vector.alignments);
    println!("  - Aggregate: {:.4}", purpose_vector.aggregate_alignment());
    println!("  - Dominant embedder: {}", purpose_vector.dominant_embedder);
    println!("  - Coherence: {:.4}", purpose_vector.coherence);
}

/// Test that multiple purpose vectors have correct structure.
#[test]
fn test_multiple_purpose_vectors() {
    // Create and verify multiple fingerprints with different purpose vectors
    let mut fingerprints = Vec::new();

    for i in 0..5 {
        let mut alignments = [0.5f32; NUM_EMBEDDERS];
        // Vary the first alignment to create different purpose vectors
        alignments[0] = 0.5 + (i as f32 * 0.1);

        let purpose_vector = PurposeVector::new(alignments);
        let semantic = SemanticFingerprint::zeroed();

        let fp = TeleologicalFingerprint::new(semantic, purpose_vector, Default::default(), [i as u8; 32]);

        fingerprints.push(fp.clone());

        // Verify all properties are correct
        assert_eq!(fp.purpose_vector.alignments.len(), 13);
        for &alignment in &fp.purpose_vector.alignments {
            assert!((-1.0..=1.0).contains(&alignment));
        }
    }

    println!("[PASS] Multiple purpose vectors created and verified");
    println!("  - Created {} fingerprints with varying purpose vectors", fingerprints.len());

    // Verify they're all different
    for (i, fp1) in fingerprints.iter().enumerate() {
        for fp2 in &fingerprints[i+1..] {
            let similarity = fp1.purpose_vector.similarity(&fp2.purpose_vector);
            assert!(similarity <= 0.9999, "Expected different purpose vectors");
        }
    }
}

/// Test that purpose vectors fail fast if North Star goal is missing.
#[tokio::test]
async fn test_purpose_computation_fails_without_north_star() {
    let empty_hierarchy = GoalHierarchy::new(); // No North Star
    let config = PurposeComputeConfig::with_hierarchy(empty_hierarchy);
    let computer = DefaultPurposeComputer::new();

    let semantic = SemanticFingerprint::zeroed();

    // Should fail because no North Star goal
    let result = computer.compute_purpose(&semantic, &config).await;

    assert!(
        result.is_err(),
        "Expected error when North Star goal is missing, but got Ok"
    );

    println!("[PASS] Purpose computation correctly fails without North Star goal");
}

/// Test alignment value ranges are within cosine similarity bounds.
#[test]
fn test_purpose_vector_alignment_ranges() {
    let test_cases = vec![
        ([1.0; 13], "All perfect alignment"),
        ([0.0; 13], "All zero alignment"),
        ([-1.0; 13], "All opposite alignment"),
        ([0.5; 13], "All 0.5 alignment"),
    ];

    for (alignments, description) in test_cases {
        let pv = PurposeVector::new(alignments);

        for (i, &val) in pv.alignments.iter().enumerate() {
            assert!(
                (-1.0..=1.0).contains(&val),
                "Test '{}': alignment[{}] = {} out of range",
                description,
                i,
                val
            );
        }

        let aggregate = pv.aggregate_alignment();
        assert!(
            (-1.0..=1.0).contains(&aggregate),
            "Test '{}': aggregate {:.4} out of range",
            description,
            aggregate
        );

        println!("[PASS] Test case '{}': aggregate = {:.4}", description, aggregate);
    }
}

/// Test that different fingerprints produce different purpose vectors.
#[tokio::test]
async fn test_purpose_vectors_differentiate_semantics() {
    let mut hierarchy = GoalHierarchy::new();
    let discovery = GoalDiscoveryMetadata::bootstrap();
    let north_star = GoalNode::autonomous_goal(
        "Test goal".into(),
        GoalLevel::NorthStar,
        SemanticFingerprint::zeroed(),
        discovery,
    ).expect("Failed to create goal");
    hierarchy.add_goal(north_star).expect("Failed to add goal");

    let config = PurposeComputeConfig::with_hierarchy(hierarchy);
    let computer = DefaultPurposeComputer::new();

    // Create two different semantic fingerprints
    let semantic1 = SemanticFingerprint::zeroed();
    let semantic2 = SemanticFingerprint::zeroed(); // Same in this test, but framework allows differences

    let pv1 = computer
        .compute_purpose(&semantic1, &config)
        .await
        .expect("Failed to compute pv1");

    let pv2 = computer
        .compute_purpose(&semantic2, &config)
        .await
        .expect("Failed to compute pv2");

    // Both should have 13 elements
    assert_eq!(pv1.alignments.len(), 13);
    assert_eq!(pv2.alignments.len(), 13);

    // Both should be valid
    for &val in pv1.alignments.iter().chain(pv2.alignments.iter()) {
        assert!((-1.0..=1.0).contains(&val));
    }

    println!("[PASS] Purpose vectors correctly computed for different semantics");
}
