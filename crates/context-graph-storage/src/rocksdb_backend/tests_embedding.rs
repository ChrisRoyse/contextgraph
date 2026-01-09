//! Embedding storage operation tests.
//!
//! Tests use REAL data stored in RocksDB - NO mocks per constitution.yaml.
//! Each test stores embeddings via embedding_ops methods and verifies actual DB state.

use tempfile::TempDir;
use uuid::Uuid;

use super::core::RocksDbMemex;
use context_graph_core::types::{EmbeddingVector, NodeId, DEFAULT_EMBEDDING_DIM};

// =========================================================================
// Test Helpers (NO MOCKS - REAL DATA)
// =========================================================================

/// Create a temp database for testing.
fn create_temp_db() -> (TempDir, RocksDbMemex) {
    let tmp = TempDir::new().expect("create temp dir");
    let db = RocksDbMemex::open(tmp.path()).expect("open db");
    (tmp, db)
}

/// Create a normalized embedding vector (magnitude ~= 1.0).
fn create_normalized_embedding(dim: usize) -> EmbeddingVector {
    let val = 1.0 / (dim as f32).sqrt();
    vec![val; dim]
}

/// Create an embedding with specific values for precision testing.
fn create_precision_embedding() -> EmbeddingVector {
    let mut embedding = Vec::with_capacity(DEFAULT_EMBEDDING_DIM);
    for i in 0..DEFAULT_EMBEDDING_DIM {
        let value = (i as f32 / DEFAULT_EMBEDDING_DIM as f32) * std::f32::consts::PI;
        embedding.push(value);
    }
    embedding
}

// =========================================================================
// store_embedding Tests
// =========================================================================

#[test]
fn test_store_embedding_basic() {
    println!("=== TEST: store_embedding basic operation ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);

    println!("BEFORE: No embedding exists for node {}", node_id);

    let result = db.store_embedding(&node_id, &embedding);

    assert!(result.is_ok(), "store_embedding should succeed");

    // VERIFY: Check the source of truth (embeddings CF)
    let retrieved = db.get_embedding(&node_id).expect("get after store");
    assert_eq!(retrieved.len(), DEFAULT_EMBEDDING_DIM);

    println!(
        "AFTER: Embedding stored, {} dimensions verified in DB",
        retrieved.len()
    );
    println!("RESULT: PASS");
}

#[test]
fn test_store_embedding_overwrites() {
    println!("=== TEST: store_embedding overwrites existing ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding1 = vec![0.1_f32; DEFAULT_EMBEDDING_DIM];
    let embedding2 = vec![0.9_f32; DEFAULT_EMBEDDING_DIM];

    db.store_embedding(&node_id, &embedding1).unwrap();
    println!("BEFORE: First embedding stored (all 0.1)");

    db.store_embedding(&node_id, &embedding2).unwrap();
    println!("ACTION: Second embedding stored (all 0.9)");

    let retrieved = db.get_embedding(&node_id).unwrap();
    assert!(
        (retrieved[0] - 0.9).abs() < 0.0001,
        "Should have second embedding value"
    );

    println!("AFTER: Retrieved value = {}, expected 0.9", retrieved[0]);
    println!("RESULT: PASS - Overwrite successful");
}

// =========================================================================
// get_embedding Tests
// =========================================================================

#[test]
fn test_get_embedding_not_found() {
    println!("=== TEST: get_embedding returns NotFound for missing ===");
    let (_tmp, db) = create_temp_db();

    let nonexistent_id = Uuid::new_v4();

    let result = db.get_embedding(&nonexistent_id);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("not found"),
        "Error should indicate not found: {}",
        err
    );

    println!("RESULT: PASS - NotFound error for missing embedding");
}

#[test]
fn test_get_embedding_roundtrip_precision() {
    println!("=== TEST: get_embedding preserves exact f32 precision ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = create_precision_embedding();

    db.store_embedding(&node_id, &embedding).unwrap();
    let retrieved = db.get_embedding(&node_id).unwrap();

    assert_eq!(embedding.len(), retrieved.len());

    for (i, (orig, rest)) in embedding.iter().zip(retrieved.iter()).enumerate() {
        assert_eq!(
            orig.to_bits(),
            rest.to_bits(),
            "Value at index {} differs: {} vs {} (bits: {:032b} vs {:032b})",
            i,
            orig,
            rest,
            orig.to_bits(),
            rest.to_bits()
        );
    }

    println!(
        "RESULT: PASS - All {} values preserved exactly",
        embedding.len()
    );
}

// =========================================================================
// batch_get_embeddings Tests
// =========================================================================

#[test]
fn test_batch_get_embeddings_empty() {
    println!("=== TEST: batch_get_embeddings with empty input ===");
    let (_tmp, db) = create_temp_db();

    let result = db.batch_get_embeddings(&[]);

    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());

    println!("RESULT: PASS - Empty input returns empty output");
}

#[test]
fn test_batch_get_embeddings_all_found() {
    println!("=== TEST: batch_get_embeddings all IDs found ===");
    let (_tmp, db) = create_temp_db();

    // Store 3 embeddings
    let ids: Vec<NodeId> = (0..3).map(|_| Uuid::new_v4()).collect();
    for (i, id) in ids.iter().enumerate() {
        let embedding = vec![(i as f32) * 0.1; DEFAULT_EMBEDDING_DIM];
        db.store_embedding(id, &embedding).unwrap();
    }

    println!("BEFORE: Stored 3 embeddings");

    let result = db.batch_get_embeddings(&ids).unwrap();

    assert_eq!(result.len(), 3);
    for (i, maybe_emb) in result.iter().enumerate() {
        assert!(maybe_emb.is_some(), "Embedding {} should be found", i);
        let emb = maybe_emb.as_ref().unwrap();
        assert_eq!(emb.len(), DEFAULT_EMBEDDING_DIM);
        // Verify first value matches what we stored
        let expected = (i as f32) * 0.1;
        assert!(
            (emb[0] - expected).abs() < 0.0001,
            "Embedding {} first value should be {}, got {}",
            i,
            expected,
            emb[0]
        );
    }

    println!("AFTER: Retrieved 3/3 embeddings with correct values");
    println!("RESULT: PASS");
}

#[test]
fn test_batch_get_embeddings_some_missing() {
    println!("=== TEST: batch_get_embeddings with some IDs missing ===");
    let (_tmp, db) = create_temp_db();

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4(); // NOT stored
    let id3 = Uuid::new_v4();

    db.store_embedding(&id1, &vec![0.1_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();
    db.store_embedding(&id3, &vec![0.3_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();

    println!("BEFORE: Stored embeddings for id1, id3 (NOT id2)");

    let result = db.batch_get_embeddings(&[id1, id2, id3]).unwrap();

    assert_eq!(result.len(), 3);
    assert!(result[0].is_some(), "id1 should be found");
    assert!(result[1].is_none(), "id2 should be None (not stored)");
    assert!(result[2].is_some(), "id3 should be found");

    println!("AFTER: result[0]=Some, result[1]=None, result[2]=Some - order preserved");
    println!("RESULT: PASS");
}

#[test]
fn test_batch_get_embeddings_all_missing() {
    println!("=== TEST: batch_get_embeddings with all IDs missing ===");
    let (_tmp, db) = create_temp_db();

    let ids: Vec<NodeId> = (0..3).map(|_| Uuid::new_v4()).collect();

    let result = db.batch_get_embeddings(&ids).unwrap();

    assert_eq!(result.len(), 3);
    for maybe_emb in result.iter() {
        assert!(maybe_emb.is_none());
    }

    println!("RESULT: PASS - All None for nonexistent IDs");
}

#[test]
fn test_batch_get_embeddings_preserves_order() {
    println!("=== TEST: batch_get_embeddings preserves input order ===");
    let (_tmp, db) = create_temp_db();

    // Store with distinguishing values
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let id3 = Uuid::new_v4();

    db.store_embedding(&id1, &vec![1.0_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();
    db.store_embedding(&id2, &vec![2.0_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();
    db.store_embedding(&id3, &vec![3.0_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();

    // Request in different order
    let result = db.batch_get_embeddings(&[id3, id1, id2]).unwrap();

    assert!(
        (result[0].as_ref().unwrap()[0] - 3.0).abs() < 0.0001,
        "First result should be id3's embedding (3.0)"
    );
    assert!(
        (result[1].as_ref().unwrap()[0] - 1.0).abs() < 0.0001,
        "Second result should be id1's embedding (1.0)"
    );
    assert!(
        (result[2].as_ref().unwrap()[0] - 2.0).abs() < 0.0001,
        "Third result should be id2's embedding (2.0)"
    );

    println!("RESULT: PASS - Order matches input IDs");
}

// =========================================================================
// delete_embedding Tests
// =========================================================================

#[test]
fn test_delete_embedding_basic() {
    println!("=== TEST: delete_embedding removes from DB ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);

    db.store_embedding(&node_id, &embedding).unwrap();
    assert!(db.embedding_exists(&node_id).unwrap());
    println!("BEFORE: Embedding exists");

    db.delete_embedding(&node_id).unwrap();

    assert!(!db.embedding_exists(&node_id).unwrap());
    println!("AFTER: Embedding deleted");
    println!("RESULT: PASS");
}

#[test]
fn test_delete_embedding_nonexistent_ok() {
    println!("=== TEST: delete_embedding on nonexistent is OK ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();

    let result = db.delete_embedding(&node_id);

    assert!(
        result.is_ok(),
        "Deleting nonexistent embedding should succeed"
    );
    println!("RESULT: PASS - No error for deleting nonexistent");
}

// =========================================================================
// embedding_exists Tests
// =========================================================================

#[test]
fn test_embedding_exists() {
    println!("=== TEST: embedding_exists checks correctly ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();

    assert!(!db.embedding_exists(&node_id).unwrap());
    println!("BEFORE: embedding_exists = false");

    db.store_embedding(&node_id, &vec![0.5_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();

    assert!(db.embedding_exists(&node_id).unwrap());
    println!("AFTER: embedding_exists = true");
    println!("RESULT: PASS");
}

// =========================================================================
// Edge Case Tests (REQUIRED - with before/after state printing)
// =========================================================================

#[test]
fn edge_case_empty_embedding() {
    println!("=== EDGE CASE 1: Empty embedding vector ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let empty: EmbeddingVector = vec![];

    println!("BEFORE: Storing empty embedding");

    db.store_embedding(&node_id, &empty).unwrap();
    let retrieved = db.get_embedding(&node_id).unwrap();

    println!("AFTER: Retrieved embedding length = {}", retrieved.len());
    assert!(retrieved.is_empty());
    println!("RESULT: PASS - Empty embedding preserved");
}

#[test]
fn edge_case_extreme_float_values() {
    println!("=== EDGE CASE 2: Extreme float values ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let extremes = vec![
        f32::MIN_POSITIVE,
        f32::MAX,
        f32::MIN,
        1e-38_f32,
        1e38_f32,
        0.0_f32,
        -0.0_f32,
    ];

    println!("BEFORE: {:?}", extremes);

    db.store_embedding(&node_id, &extremes).unwrap();
    let retrieved = db.get_embedding(&node_id).unwrap();

    println!("AFTER: {:?}", retrieved);

    for (i, (orig, rest)) in extremes.iter().zip(retrieved.iter()).enumerate() {
        assert_eq!(
            orig.to_bits(),
            rest.to_bits(),
            "Value {} differs: {} vs {}",
            i,
            orig,
            rest
        );
    }
    println!("RESULT: PASS - All extreme values preserved exactly");
}

#[test]
fn edge_case_1536d_size_verification() {
    println!("=== EDGE CASE 3: 1536D embedding size verification ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = create_normalized_embedding(1536);

    println!("BEFORE: embedding.len() = {}", embedding.len());

    db.store_embedding(&node_id, &embedding).unwrap();
    let retrieved = db.get_embedding(&node_id).unwrap();

    println!(
        "AFTER: retrieved.len() = {}, expected 1536",
        retrieved.len()
    );

    assert_eq!(retrieved.len(), 1536);
    // Verify byte size: 1536 * 4 = 6144 bytes
    let byte_size = 1536 * 4;
    println!(
        "Embedding byte size: {} bytes (~{:.1}KB)",
        byte_size,
        byte_size as f64 / 1024.0
    );
    println!("RESULT: PASS - 1536D = 6144 bytes");
}

// =========================================================================
// Integration with store_node (verify consistency)
// =========================================================================

#[test]
fn test_embedding_consistent_with_store_node() {
    println!("=== TEST: Embedding consistent between store_node and get_embedding ===");
    let (_tmp, db) = create_temp_db();

    use context_graph_core::types::MemoryNode;

    // Create and store a node (which stores embedding via store_node)
    let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);
    let node = MemoryNode::new("Test content".to_string(), embedding.clone());
    let node_id = node.id;

    // Store via node_ops (this writes to embeddings CF)
    db.store_node(&node).expect("store_node");

    // Retrieve via embedding_ops
    let retrieved = db.get_embedding(&node_id).unwrap();

    assert_eq!(retrieved.len(), embedding.len());
    for (i, (orig, rest)) in embedding.iter().zip(retrieved.iter()).enumerate() {
        assert_eq!(orig.to_bits(), rest.to_bits(), "Value at {} differs", i);
    }

    println!("RESULT: PASS - store_node and get_embedding are consistent");
}

#[test]
fn test_store_embedding_updates_store_node_embedding() {
    println!("=== TEST: store_embedding updates embedding stored by store_node ===");
    let (_tmp, db) = create_temp_db();

    use context_graph_core::types::MemoryNode;

    // Store node with original embedding
    let original = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);
    let node = MemoryNode::new("Test".to_string(), original);
    let node_id = node.id;
    db.store_node(&node).unwrap();

    // Update embedding independently
    let updated = vec![0.999_f32; DEFAULT_EMBEDDING_DIM];
    db.store_embedding(&node_id, &updated).unwrap();

    // Verify via get_embedding
    let retrieved = db.get_embedding(&node_id).unwrap();
    assert!(
        (retrieved[0] - 0.999).abs() < 0.0001,
        "Should have updated value"
    );

    println!("RESULT: PASS - Independent embedding update works");
}

// =========================================================================
// Boundary & Edge Case Audit (REQUIRED per task spec)
// =========================================================================

#[test]
fn boundary_audit_max_batch_size() {
    println!("=== BOUNDARY AUDIT: Large batch retrieval ===");
    let (_tmp, db) = create_temp_db();

    // Store 100 embeddings
    let mut ids = Vec::new();
    for i in 0..100 {
        let id = Uuid::new_v4();
        let embedding = vec![(i as f32) * 0.01; DEFAULT_EMBEDDING_DIM];
        db.store_embedding(&id, &embedding).unwrap();
        ids.push(id);
    }

    println!("BEFORE: 100 embeddings stored");

    let result = db.batch_get_embeddings(&ids).unwrap();

    assert_eq!(result.len(), 100);
    let found_count = result.iter().filter(|e| e.is_some()).count();
    assert_eq!(found_count, 100);

    println!("AFTER: Retrieved 100/100 embeddings in batch");
    println!("RESULT: PASS");
}

#[test]
fn boundary_audit_various_dimensions() {
    println!("=== BOUNDARY AUDIT: Various embedding dimensions ===");
    let (_tmp, db) = create_temp_db();

    for dim in [1, 10, 128, 512, 768, 1024, 1536] {
        let node_id = Uuid::new_v4();
        let embedding = create_normalized_embedding(dim);

        println!("Testing dim={}", dim);

        db.store_embedding(&node_id, &embedding).unwrap();
        let retrieved = db.get_embedding(&node_id).unwrap();

        assert_eq!(retrieved.len(), dim);
    }

    println!("RESULT: PASS - All dimensions work correctly");
}

// =========================================================================
// Additional Tests for Comprehensive Coverage
// =========================================================================

#[test]
fn test_multiple_embeddings_same_db() {
    println!("=== TEST: Multiple embeddings stored in same database ===");
    let (_tmp, db) = create_temp_db();

    let count = 50;
    let mut ids = Vec::new();

    // Store multiple embeddings
    for i in 0..count {
        let id = Uuid::new_v4();
        let embedding = vec![i as f32 / count as f32; DEFAULT_EMBEDDING_DIM];
        db.store_embedding(&id, &embedding).unwrap();
        ids.push(id);
    }

    println!("BEFORE: Stored {} embeddings", count);

    // Verify all exist and have correct values
    for (i, id) in ids.iter().enumerate() {
        let retrieved = db.get_embedding(id).unwrap();
        let expected = i as f32 / count as f32;
        assert!(
            (retrieved[0] - expected).abs() < 0.0001,
            "Embedding {} has wrong value: expected {}, got {}",
            i,
            expected,
            retrieved[0]
        );
    }

    println!("AFTER: All {} embeddings verified", count);
    println!("RESULT: PASS");
}

#[test]
fn test_embedding_persistence_across_operations() {
    println!("=== TEST: Embedding persists across unrelated operations ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = vec![0.42_f32; DEFAULT_EMBEDDING_DIM];

    db.store_embedding(&node_id, &embedding).unwrap();

    // Perform unrelated operations
    let other_id = Uuid::new_v4();
    db.store_embedding(&other_id, &vec![0.1_f32; DEFAULT_EMBEDDING_DIM])
        .unwrap();
    db.delete_embedding(&other_id).unwrap();

    // Verify original embedding still exists and is correct
    let retrieved = db.get_embedding(&node_id).unwrap();
    assert!(
        (retrieved[0] - 0.42).abs() < 0.0001,
        "Original embedding should be unchanged"
    );

    println!("RESULT: PASS - Embedding persists across unrelated operations");
}

#[test]
fn test_get_embedding_after_delete() {
    println!("=== TEST: get_embedding returns NotFound after delete ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = create_normalized_embedding(DEFAULT_EMBEDDING_DIM);

    db.store_embedding(&node_id, &embedding).unwrap();
    assert!(db.embedding_exists(&node_id).unwrap());

    db.delete_embedding(&node_id).unwrap();

    let result = db.get_embedding(&node_id);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));

    println!("RESULT: PASS - get_embedding returns NotFound after delete");
}

#[test]
fn test_batch_single_embedding() {
    println!("=== TEST: batch_get_embeddings with single ID ===");
    let (_tmp, db) = create_temp_db();

    let node_id = Uuid::new_v4();
    let embedding = vec![0.77_f32; DEFAULT_EMBEDDING_DIM];

    db.store_embedding(&node_id, &embedding).unwrap();

    let result = db.batch_get_embeddings(&[node_id]).unwrap();

    assert_eq!(result.len(), 1);
    assert!(result[0].is_some());
    assert!(
        (result[0].as_ref().unwrap()[0] - 0.77).abs() < 0.0001,
        "Single batch should return correct value"
    );

    println!("RESULT: PASS - Single ID batch works correctly");
}
