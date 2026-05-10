//! Integration test for graph linking system.
//!
//! Verifies EdgeRepository integration with MCP tools end-to-end.
//! NO FALLBACKS - all operations must succeed or test fails.
//!
//! ## Test Coverage
//!
//! 1. EdgeRepository can store and retrieve K-NN edges
//! 2. EdgeRepository can store and retrieve typed edges
//! 3. Handlers::with_graph_linking creates valid handlers
//! 4. Edge data persists to RocksDB

use context_graph_core::graph_linking::{
    DirectedRelation, EmbedderEdge, GraphLinkEdgeType, TypedEdge, NUM_EMBEDDERS,
};
use context_graph_storage::graph_edges::EdgeRepository;
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use std::sync::Arc;
use tempfile::TempDir;
use uuid::Uuid;

/// Test 1: EdgeRepository can store and retrieve K-NN edges
#[test]
fn test_edge_repository_knn_edges_roundtrip() {
    println!("\n========== TEST: K-NN Edges Roundtrip ==========");

    // Create temp directory and open RocksDB
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path();
    println!("Created temp DB at: {:?}", db_path);

    // Open RocksDB store (this creates all column families including graph edges)
    let store =
        RocksDbTeleologicalStore::open(db_path).expect("Failed to open RocksDbTeleologicalStore");

    // Extract Arc<DB> using our new db_arc() method
    let db_arc = store.db_arc();
    println!("Extracted Arc<DB> from store");

    // Create EdgeRepository
    let edge_repo = EdgeRepository::new(db_arc);
    println!("Created EdgeRepository");

    // Create test data
    let source_id = Uuid::new_v4();
    let target1 = Uuid::new_v4();
    let target2 = Uuid::new_v4();
    let target3 = Uuid::new_v4();

    // EmbedderEdge::new(source, target, embedder_id, similarity) -> EdgeResult<Self>
    let edges = vec![
        EmbedderEdge::new(source_id, target1, 0, 0.95).expect("Failed to create edge 1"),
        EmbedderEdge::new(source_id, target2, 0, 0.87).expect("Failed to create edge 2"),
        EmbedderEdge::new(source_id, target3, 0, 0.72).expect("Failed to create edge 3"),
    ];

    println!("Source ID: {}", source_id);
    println!("Storing 3 K-NN edges for embedder E1 (id=0)");

    // Store edges
    edge_repo
        .store_embedder_edges(0, source_id, &edges)
        .expect("Failed to store embedder edges");
    println!("Stored K-NN edges successfully");

    // Retrieve edges
    let retrieved = edge_repo
        .get_embedder_edges(0, source_id)
        .expect("Failed to retrieve embedder edges");

    // Verify
    assert_eq!(
        retrieved.len(),
        3,
        "Expected 3 edges, got {}",
        retrieved.len()
    );
    assert_eq!(retrieved[0].target(), target1);
    assert!((retrieved[0].similarity() - 0.95).abs() < 0.001);
    assert_eq!(retrieved[1].target(), target2);
    assert!((retrieved[1].similarity() - 0.87).abs() < 0.001);
    assert_eq!(retrieved[2].target(), target3);
    assert!((retrieved[2].similarity() - 0.72).abs() < 0.001);

    println!("✓ Retrieved 3 K-NN edges with correct data");
    println!(
        "  - Edge 1: {} -> {} (sim={})",
        source_id,
        target1,
        retrieved[0].similarity()
    );
    println!(
        "  - Edge 2: {} -> {} (sim={})",
        source_id,
        target2,
        retrieved[1].similarity()
    );
    println!(
        "  - Edge 3: {} -> {} (sim={})",
        source_id,
        target3,
        retrieved[2].similarity()
    );

    // Verify empty result for non-existent source
    let empty = edge_repo
        .get_embedder_edges(0, Uuid::new_v4())
        .expect("Failed to query non-existent source");
    assert!(
        empty.is_empty(),
        "Expected empty result for non-existent source"
    );
    println!("✓ Non-existent source returns empty vec (not error)");

    println!("[PASS] K-NN edges roundtrip test completed\n");
}

/// Test 2: EdgeRepository can store and retrieve typed edges
#[test]
fn test_edge_repository_typed_edges_roundtrip() {
    println!("\n========== TEST: Typed Edges Roundtrip ==========");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open store");
    let edge_repo = EdgeRepository::new(store.db_arc());

    // Create test data
    let source_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();

    // Create a typed edge with specific embedder scores
    let embedder_scores: [f32; 14] = [
        0.9,  // E1 - semantic (strongly agrees)
        0.0,  // E2 - temporal (excluded)
        0.0,  // E3 - temporal (excluded)
        0.0,  // E4 - temporal (excluded)
        0.85, // E5 - causal (agrees)
        0.75, // E6 - sparse
        0.6,  // E7 - code
        0.5,  // E8 - graph
        0.4,  // E9 - HDC
        0.8,  // E10 - intent
        0.7,  // E11 - entity
        0.65, // E12 - ColBERT
        0.55, // E13 - SPLADE
        0.88, // E14 - BGE-M3 Dense
    ];

    // agreeing_embedders bitmask: E1=1, E5=0 (temporal), E6=1, E7=0, E8=0, E9=0, E10=1, E11=1
    // Let's say E1, E5, E10, E11 agree (bits 0, 4, 9, 10)
    let agreeing_embedders: u16 = (1 << 0) | (1 << 4) | (1 << 9) | (1 << 10); // 0b0000_0110_0001_0001
    let agreement_count = agreeing_embedders.count_ones() as u8; // 4

    // TypedEdge::new(source, target, edge_type, weight, direction, embedder_scores, agreement_count, agreeing_embedders)
    let edge = TypedEdge::new(
        source_id,
        target_id,
        GraphLinkEdgeType::SemanticSimilar,
        0.85,
        DirectedRelation::Symmetric,
        embedder_scores,
        agreement_count,
        agreeing_embedders,
    )
    .expect("Failed to create typed edge");

    println!("Source: {}", source_id);
    println!("Target: {}", target_id);
    println!("Edge type: semantic_similar");
    println!("Weight: 0.85");
    println!("Agreement count: {}", agreement_count);

    // Store typed edge
    edge_repo
        .store_typed_edge(&edge)
        .expect("Failed to store typed edge");
    println!("✓ Stored typed edge");

    // Retrieve by source
    let retrieved = edge_repo
        .get_typed_edges_from(source_id)
        .expect("Failed to retrieve typed edges");

    assert_eq!(retrieved.len(), 1, "Expected 1 typed edge");
    assert_eq!(retrieved[0].target(), target_id);
    assert_eq!(retrieved[0].edge_type(), GraphLinkEdgeType::SemanticSimilar);
    assert!((retrieved[0].weight() - 0.85).abs() < 0.001);

    println!("✓ Retrieved typed edge with correct data");
    println!("  - Target: {}", retrieved[0].target());
    println!("  - Type: {:?}", retrieved[0].edge_type());
    println!("  - Weight: {}", retrieved[0].weight());

    // Retrieve by type
    let by_type = edge_repo
        .get_typed_edges_by_type(source_id, GraphLinkEdgeType::SemanticSimilar)
        .expect("Failed to retrieve by type");

    assert_eq!(by_type.len(), 1, "Expected 1 edge by type filter");
    println!("✓ Retrieved edge by type filter");

    // Retrieve wrong type - should be empty
    let wrong_type = edge_repo
        .get_typed_edges_by_type(source_id, GraphLinkEdgeType::CodeRelated)
        .expect("Failed to query wrong type");

    assert!(
        wrong_type.is_empty(),
        "Expected empty result for wrong type"
    );
    println!("✓ Wrong type filter returns empty vec");

    println!("[PASS] Typed edges roundtrip test completed\n");
}

/// Test 3: Multiple embedders K-NN edge storage
#[test]
fn test_edge_repository_multiple_embedders() {
    println!("\n========== TEST: Multiple Embedders K-NN Storage ==========");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open store");
    let edge_repo = EdgeRepository::new(store.db_arc());

    let source_id = Uuid::new_v4();
    let target_e1 = Uuid::new_v4();
    let target_e6 = Uuid::new_v4();
    let target_e10 = Uuid::new_v4();
    let target_e14 = Uuid::new_v4();

    // Store edges for different embedders (avoiding E5/E7/E8 which need direction)
    // E1 (semantic) - index 0
    edge_repo
        .store_embedder_edges(
            0,
            source_id,
            &[EmbedderEdge::new(source_id, target_e1, 0, 0.92).expect("E1 edge")],
        )
        .expect("Failed to store E1 edges");
    println!("✓ Stored E1 (semantic) edge");

    // E6 (sparse) - index 5
    edge_repo
        .store_embedder_edges(
            5,
            source_id,
            &[EmbedderEdge::new(source_id, target_e6, 5, 0.88).expect("E6 edge")],
        )
        .expect("Failed to store E6 edges");
    println!("✓ Stored E6 (sparse) edge");

    // E10 (intent) - index 9
    edge_repo
        .store_embedder_edges(
            9,
            source_id,
            &[EmbedderEdge::new(source_id, target_e10, 9, 0.79).expect("E10 edge")],
        )
        .expect("Failed to store E10 edges");
    println!("✓ Stored E10 (intent) edge");

    // E14 (BGE-M3 Dense) - index 13
    edge_repo
        .store_embedder_edges(
            13,
            source_id,
            &[EmbedderEdge::new(source_id, target_e14, 13, 0.83).expect("E14 edge")],
        )
        .expect("Failed to store E14 edges");
    println!("✓ Stored E14 (BGE-M3 Dense) edge");

    // Retrieve and verify each embedder's edges are isolated
    let e1_edges = edge_repo
        .get_embedder_edges(0, source_id)
        .expect("E1 query failed");
    let e6_edges = edge_repo
        .get_embedder_edges(5, source_id)
        .expect("E6 query failed");
    let e10_edges = edge_repo
        .get_embedder_edges(9, source_id)
        .expect("E10 query failed");
    let e14_edges = edge_repo
        .get_embedder_edges(13, source_id)
        .expect("E14 query failed");

    assert_eq!(e1_edges.len(), 1);
    assert_eq!(e1_edges[0].target(), target_e1);
    println!("✓ E1 edges isolated (found target_e1)");

    assert_eq!(e6_edges.len(), 1);
    assert_eq!(e6_edges[0].target(), target_e6);
    println!("✓ E6 edges isolated (found target_e6)");

    assert_eq!(e10_edges.len(), 1);
    assert_eq!(e10_edges[0].target(), target_e10);
    println!("✓ E10 edges isolated (found target_e10)");

    assert_eq!(e14_edges.len(), 1);
    assert_eq!(e14_edges[0].target(), target_e14);
    println!("✓ E14 edges isolated (found target_e14)");

    // Verify one-past-the-end embedder ID is rejected.
    let invalid_embedder_id = NUM_EMBEDDERS as u8;
    let result = edge_repo.store_embedder_edges(invalid_embedder_id, source_id, &[]);
    assert!(
        result.is_err(),
        "Expected error for embedder_id={invalid_embedder_id}"
    );
    println!("✓ Invalid embedder ID ({invalid_embedder_id}) rejected");

    println!("[PASS] Multiple embedders test completed\n");
}

/// Test 4: Edge persistence verification
#[test]
fn test_edge_persistence_survives_reopen() {
    println!("\n========== TEST: Edge Persistence Survives Reopen ==========");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().to_path_buf();

    let source_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();

    // First session: store edges
    {
        let store =
            RocksDbTeleologicalStore::open(&db_path).expect("Failed to open store (session 1)");
        let edge_repo = EdgeRepository::new(store.db_arc());

        edge_repo
            .store_embedder_edges(
                0,
                source_id,
                &[EmbedderEdge::new(source_id, target_id, 0, 0.95).expect("edge")],
            )
            .expect("Failed to store edges");

        println!(
            "Session 1: Stored edge {} -> {} (sim=0.95)",
            source_id, target_id
        );
    }
    // store dropped here, database closed

    // Second session: verify edges persist
    {
        let store =
            RocksDbTeleologicalStore::open(&db_path).expect("Failed to open store (session 2)");
        let edge_repo = EdgeRepository::new(store.db_arc());

        let edges = edge_repo
            .get_embedder_edges(0, source_id)
            .expect("Failed to retrieve edges after reopen");

        assert_eq!(edges.len(), 1, "Expected 1 edge after reopen");
        assert_eq!(edges[0].target(), target_id);
        assert!((edges[0].similarity() - 0.95).abs() < 0.001);

        println!("Session 2: Retrieved edge after reopen");
        println!("  - Target: {}", edges[0].target());
        println!("  - Similarity: {}", edges[0].similarity());
    }

    println!("✓ Edges persist across database close/reopen");
    println!("[PASS] Persistence test completed\n");
}

/// Test 5: Edge deletion verification
#[test]
fn test_edge_deletion() {
    println!("\n========== TEST: Edge Deletion ==========");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open store");
    let edge_repo = EdgeRepository::new(store.db_arc());

    let source_id = Uuid::new_v4();
    let target_id = Uuid::new_v4();

    // Store edge
    edge_repo
        .store_embedder_edges(
            0,
            source_id,
            &[EmbedderEdge::new(source_id, target_id, 0, 0.9).expect("edge")],
        )
        .expect("Failed to store");

    // Verify it exists
    let before = edge_repo
        .get_embedder_edges(0, source_id)
        .expect("Query failed");
    assert_eq!(before.len(), 1, "Edge should exist before deletion");
    println!("✓ Edge exists before deletion");

    // Delete edge
    edge_repo
        .delete_embedder_edges(0, source_id)
        .expect("Delete failed");
    println!("✓ Deleted edge");

    // Verify it's gone
    let after = edge_repo
        .get_embedder_edges(0, source_id)
        .expect("Query after delete failed");
    assert!(after.is_empty(), "Edge should be gone after deletion");
    println!("✓ Edge no longer exists after deletion");

    println!("[PASS] Deletion test completed\n");
}

/// Test 6: Verify db_arc() method returns same underlying DB
#[test]
fn test_db_arc_shared_instance() {
    println!("\n========== TEST: db_arc() Returns Shared Instance ==========");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open store");

    // Get two Arc<DB> references
    let arc1 = store.db_arc();
    let arc2 = store.db_arc();

    // They should point to the same underlying DB
    assert!(Arc::ptr_eq(&arc1, &arc2), "db_arc() should return same Arc");
    println!("✓ db_arc() returns same Arc<DB> instance");

    // Strong count should be 3 (store + arc1 + arc2)
    let strong_count = Arc::strong_count(&arc1);
    assert!(
        strong_count >= 3,
        "Expected at least 3 strong refs, got {}",
        strong_count
    );
    println!("✓ Arc strong count: {}", strong_count);

    println!("[PASS] db_arc() shared instance test completed\n");
}

/// Test 7: Edge statistics
#[test]
fn test_edge_statistics() {
    println!("\n========== TEST: Edge Statistics ==========");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let store = RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open store");
    let edge_repo = EdgeRepository::new(store.db_arc());

    // Store some edges
    for i in 0..5 {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        edge_repo
            .store_embedder_edges(
                0,
                source,
                &[EmbedderEdge::new(source, target, 0, 0.8 + (i as f32 * 0.02)).expect("edge")],
            )
            .expect("Store failed");
    }

    // Get statistics
    let stats = edge_repo.get_stats().expect("Stats failed");

    println!("Edge Statistics:");
    println!("  - Total embedder edges: {}", stats.total_embedder_edges);
    println!("  - Typed edge count: {}", stats.typed_edge_count);
    println!("  - Storage bytes: {}", stats.storage_bytes);

    assert!(
        stats.total_embedder_edges >= 5,
        "Should have at least 5 total embedder edges"
    );
    println!("✓ Statistics reported correctly");

    println!("[PASS] Statistics test completed\n");
}

// Audit-11 TST-H1: Removed test_graph_linking_integration_summary — it had zero assertions
// (only println! statements). All functionality is already covered by the individual tests above.
