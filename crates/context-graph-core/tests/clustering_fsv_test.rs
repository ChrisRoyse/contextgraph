//! Full State Verification test for TASK-P4-001: Clustering Module Foundation
//!
//! This test verifies the source of truth for all clustering types:
//! - ClusterMembership
//! - Cluster
//! - ClusterError
//!
//! MANUAL TESTING: Run with `cargo test --test clustering_fsv_test -- --nocapture`

use context_graph_core::clustering::{Cluster, ClusterError, ClusterMembership};
use context_graph_core::teleological::Embedder;
use uuid::Uuid;

// ============================================================================
// TEST 1: ClusterMembership - Normal Path
// ============================================================================

#[test]
fn fsv_cluster_membership_happy_path() {
    println!("\n=== FSV: ClusterMembership Happy Path ===");

    // SETUP: Create synthetic membership data
    let memory_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
    let space = Embedder::Semantic;
    let cluster_id = 42;
    let probability = 0.95;

    // ACT: Create membership
    let membership = ClusterMembership::new(memory_id, space, cluster_id, probability, true);

    // VERIFY: Source of truth inspection
    println!("BEFORE: Created ClusterMembership");
    println!(
        "  memory_id: {} (expected: 550e8400-e29b-41d4-a716-446655440000)",
        membership.memory_id
    );
    println!("  space: {:?} (expected: Semantic)", membership.space);
    println!("  cluster_id: {} (expected: 42)", membership.cluster_id);
    println!(
        "  probability: {} (expected: 0.95)",
        membership.membership_probability
    );
    println!(
        "  is_core_point: {} (expected: true)",
        membership.is_core_point
    );

    assert_eq!(
        membership.memory_id.to_string(),
        "550e8400-e29b-41d4-a716-446655440000"
    );
    assert_eq!(membership.space, Embedder::Semantic);
    assert_eq!(membership.cluster_id, 42);
    assert!((membership.membership_probability - 0.95).abs() < f32::EPSILON);
    assert!(membership.is_core_point);
    assert!(!membership.is_noise());
    assert!(membership.is_confident());

    println!("AFTER: All assertions passed");
    println!("[FSV PASS] ClusterMembership happy path verified");
}

// ============================================================================
// TEST 2: ClusterMembership - Noise Path
// ============================================================================

#[test]
fn fsv_cluster_membership_noise_path() {
    println!("\n=== FSV: ClusterMembership Noise Path ===");

    let memory_id = Uuid::new_v4();

    // ACT: Create noise membership
    let noise = ClusterMembership::noise(memory_id, Embedder::Code);

    // VERIFY
    println!("BEFORE: Created noise membership");
    println!("  cluster_id: {} (expected: -1)", noise.cluster_id);
    println!(
        "  probability: {} (expected: 0.0)",
        noise.membership_probability
    );
    println!("  is_core_point: {} (expected: false)", noise.is_core_point);
    println!("  is_noise(): {} (expected: true)", noise.is_noise());

    assert_eq!(noise.cluster_id, -1);
    assert_eq!(noise.membership_probability, 0.0);
    assert!(!noise.is_core_point);
    assert!(noise.is_noise());
    assert!(!noise.is_confident());

    println!("AFTER: All noise assertions passed");
    println!("[FSV PASS] ClusterMembership noise path verified");
}

// ============================================================================
// TEST 3: ClusterMembership - Probability Clamping Edge Cases
// ============================================================================

#[test]
fn fsv_cluster_membership_probability_clamping() {
    println!("\n=== FSV: ClusterMembership Probability Clamping ===");

    let mem_id = Uuid::new_v4();

    // EDGE CASE 1: Probability > 1.0
    let high = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 1.5, false);
    println!(
        "EDGE CASE 1: probability=1.5 -> clamped to {}",
        high.membership_probability
    );
    assert_eq!(
        high.membership_probability, 1.0,
        "probability > 1.0 must clamp to 1.0"
    );

    // EDGE CASE 2: Probability < 0.0
    let low = ClusterMembership::new(mem_id, Embedder::Semantic, 1, -0.5, false);
    println!(
        "EDGE CASE 2: probability=-0.5 -> clamped to {}",
        low.membership_probability
    );
    assert_eq!(
        low.membership_probability, 0.0,
        "probability < 0.0 must clamp to 0.0"
    );

    // EDGE CASE 3: Boundary 0.8 (confidence threshold)
    let borderline = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.8, false);
    println!(
        "EDGE CASE 3: probability=0.8 -> is_confident={}",
        borderline.is_confident()
    );
    assert!(borderline.is_confident(), "0.8 is the threshold for confident");

    // EDGE CASE 4: Just below threshold
    let below = ClusterMembership::new(mem_id, Embedder::Semantic, 1, 0.79, false);
    println!(
        "EDGE CASE 4: probability=0.79 -> is_confident={}",
        below.is_confident()
    );
    assert!(!below.is_confident(), "0.79 is below the threshold");

    println!("[FSV PASS] All probability edge cases verified");
}

// ============================================================================
// TEST 4: ClusterMembership - Serialization Roundtrip
// ============================================================================

#[test]
fn fsv_cluster_membership_serialization() {
    println!("\n=== FSV: ClusterMembership Serialization ===");

    let mem_id = Uuid::parse_str("12345678-1234-1234-1234-123456789012").unwrap();
    let original = ClusterMembership::new(mem_id, Embedder::Causal, 7, 0.88, true);

    // ACT: Serialize to JSON
    let json = serde_json::to_string(&original).expect("serialize");
    println!("SERIALIZED JSON: {}", json);

    // VERIFY: JSON contains expected fields
    assert!(json.contains("12345678-1234-1234-1234-123456789012"));
    assert!(json.contains("Causal"));
    assert!(json.contains("7"));
    assert!(json.contains("0.88"));
    assert!(json.contains("true"));

    // ACT: Deserialize back
    let restored: ClusterMembership = serde_json::from_str(&json).expect("deserialize");

    // VERIFY: Roundtrip preserves all data
    assert_eq!(original.memory_id, restored.memory_id);
    assert_eq!(original.space, restored.space);
    assert_eq!(original.cluster_id, restored.cluster_id);
    assert!((original.membership_probability - restored.membership_probability).abs() < f32::EPSILON);
    assert_eq!(original.is_core_point, restored.is_core_point);

    println!("RESTORED: memory_id={}, space={:?}", restored.memory_id, restored.space);
    println!("[FSV PASS] Serialization roundtrip verified");
}

// ============================================================================
// TEST 5: Cluster - Happy Path
// ============================================================================

#[test]
fn fsv_cluster_happy_path() {
    println!("\n=== FSV: Cluster Happy Path ===");

    // SETUP: Create cluster
    let centroid = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    let mut cluster = Cluster::new(10, Embedder::Code, centroid.clone(), 25);

    println!("BEFORE update_silhouette:");
    println!("  id: {} (expected: 10)", cluster.id);
    println!("  space: {:?} (expected: Code)", cluster.space);
    println!("  member_count: {} (expected: 25)", cluster.member_count);
    println!(
        "  silhouette_score: {} (expected: 0.0)",
        cluster.silhouette_score
    );
    println!("  centroid.len(): {} (expected: 5)", cluster.centroid.len());

    // ACT: Update silhouette
    cluster.update_silhouette(0.75);

    println!("AFTER update_silhouette:");
    println!(
        "  silhouette_score: {} (expected: 0.75)",
        cluster.silhouette_score
    );
    println!(
        "  is_high_quality: {} (expected: true)",
        cluster.is_high_quality()
    );

    assert_eq!(cluster.id, 10);
    assert_eq!(cluster.space, Embedder::Code);
    assert_eq!(cluster.centroid, centroid);
    assert_eq!(cluster.member_count, 25);
    assert!((cluster.silhouette_score - 0.75).abs() < f32::EPSILON);
    assert!(cluster.is_high_quality());

    println!("[FSV PASS] Cluster happy path verified");
}

// ============================================================================
// TEST 6: Cluster - Silhouette Edge Cases
// ============================================================================

#[test]
fn fsv_cluster_silhouette_edge_cases() {
    println!("\n=== FSV: Cluster Silhouette Edge Cases ===");

    let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 1024], 5);

    // EDGE CASE 1: Score > 1.0 (clamp to 1.0)
    cluster.update_silhouette(2.5);
    println!(
        "EDGE CASE 1: score=2.5 -> clamped to {}",
        cluster.silhouette_score
    );
    assert_eq!(cluster.silhouette_score, 1.0);

    // EDGE CASE 2: Score < -1.0 (clamp to -1.0)
    cluster.update_silhouette(-3.0);
    println!(
        "EDGE CASE 2: score=-3.0 -> clamped to {}",
        cluster.silhouette_score
    );
    assert_eq!(cluster.silhouette_score, -1.0);

    // EDGE CASE 3: Exactly at threshold (0.3)
    cluster.update_silhouette(0.3);
    println!(
        "EDGE CASE 3: score=0.3 -> is_high_quality={}",
        cluster.is_high_quality()
    );
    assert!(cluster.is_high_quality());

    // EDGE CASE 4: Just below threshold
    cluster.update_silhouette(0.29);
    println!(
        "EDGE CASE 4: score=0.29 -> is_high_quality={}",
        cluster.is_high_quality()
    );
    assert!(!cluster.is_high_quality());

    println!("[FSV PASS] All silhouette edge cases verified");
}

// ============================================================================
// TEST 7: Cluster - Centroid Update
// ============================================================================

#[test]
fn fsv_cluster_centroid_update() {
    println!("\n=== FSV: Cluster Centroid Update ===");

    let mut cluster = Cluster::new(1, Embedder::Semantic, vec![0.0; 4], 5);
    let original_updated = cluster.updated_at;

    println!("BEFORE: centroid={:?}, members={}", cluster.centroid, cluster.member_count);

    // ACT: Update centroid
    std::thread::sleep(std::time::Duration::from_millis(10));
    cluster.update_centroid(vec![1.0, 2.0, 3.0, 4.0], 15);

    println!("AFTER: centroid={:?}, members={}", cluster.centroid, cluster.member_count);

    assert_eq!(cluster.centroid, vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(cluster.member_count, 15);
    assert!(cluster.updated_at > original_updated, "updated_at should increase");

    println!("[FSV PASS] Centroid update verified");
}

// ============================================================================
// TEST 8: ClusterError - All Variants
// ============================================================================

#[test]
fn fsv_cluster_error_variants() {
    println!("\n=== FSV: ClusterError Variants ===");

    // InsufficientData
    let err1 = ClusterError::insufficient_data(3, 1);
    let msg1 = err1.to_string();
    println!("InsufficientData: {}", msg1);
    assert!(msg1.contains("required 3") && msg1.contains("actual 1"));

    // DimensionMismatch
    let err2 = ClusterError::dimension_mismatch(1024, 512);
    let msg2 = err2.to_string();
    println!("DimensionMismatch: {}", msg2);
    assert!(msg2.contains("expected 1024") && msg2.contains("actual 512"));

    // NoValidClusters
    let err3 = ClusterError::NoValidClusters;
    let msg3 = err3.to_string();
    println!("NoValidClusters: {}", msg3);
    assert!(msg3.contains("No valid clusters"));

    // InvalidParameter
    let err4 = ClusterError::invalid_parameter("test message");
    let msg4 = err4.to_string();
    println!("InvalidParameter: {}", msg4);
    assert!(msg4.contains("test message"));

    // SpaceNotInitialized
    let err5 = ClusterError::SpaceNotInitialized(Embedder::Multimodal);
    let msg5 = err5.to_string();
    println!("SpaceNotInitialized: {}", msg5);
    assert!(msg5.contains("Multimodal"));

    println!("[FSV PASS] All ClusterError variants verified");
}

// ============================================================================
// TEST 9: All 13 Embedders Work
// ============================================================================

#[test]
fn fsv_all_embedders_comprehensive() {
    println!("\n=== FSV: All 13 Embedders ===");

    let mem_id = Uuid::new_v4();

    for embedder in Embedder::all() {
        // Create membership for this embedder
        let membership = ClusterMembership::new(mem_id, embedder, 1, 0.5, false);
        assert_eq!(membership.space, embedder);

        // Create cluster for this embedder
        let cluster = Cluster::new(1, embedder, vec![0.0; 10], 5);
        assert_eq!(cluster.space, embedder);

        // Create error for this embedder
        let err = ClusterError::SpaceNotInitialized(embedder);
        assert!(err.to_string().contains(&format!("{:?}", embedder)));

        println!("  {:?}: membership ✓, cluster ✓, error ✓", embedder);
    }

    println!("[FSV PASS] All 13 embedders work with all types");
}

// ============================================================================
// TEST 10: Re-exports Work at Crate Level
// ============================================================================

#[test]
fn fsv_reexports_work() {
    println!("\n=== FSV: Re-exports at Crate Level ===");

    // These should compile because they are re-exported from lib.rs
    let _: context_graph_core::ClusterMembership =
        context_graph_core::ClusterMembership::noise(Uuid::new_v4(), Embedder::Semantic);

    let _: context_graph_core::Cluster =
        context_graph_core::Cluster::new(1, Embedder::Semantic, vec![0.0], 1);

    let _: context_graph_core::ClusterError =
        context_graph_core::ClusterError::NoValidClusters;

    println!("  ClusterMembership re-export: ✓");
    println!("  Cluster re-export: ✓");
    println!("  ClusterError re-export: ✓");

    println!("[FSV PASS] All re-exports work at crate level");
}
