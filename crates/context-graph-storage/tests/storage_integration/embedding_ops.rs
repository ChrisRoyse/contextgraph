//! Embedding operations tests.
//!
//! Tests batch get, existence check, and delete operations for embeddings.

use super::common::{create_node_with_content, create_test_node, setup_db};

#[test]
fn test_embedding_batch_get() {
    let (db, _tmp) = setup_db();
    let mut ids = Vec::new();

    println!("=== EMBEDDING BATCH GET TEST ===");

    // Store 5 nodes
    for i in 0..5 {
        let node = create_node_with_content(&format!("Batch {}", i));
        ids.push(node.id);
        db.store_node(&node).expect("store");
    }

    // Batch get embeddings
    let embeddings = db.batch_get_embeddings(&ids).expect("batch get");
    println!("VERIFY: got {} embeddings", embeddings.len());
    assert_eq!(embeddings.len(), 5);

    for (i, emb_opt) in embeddings.iter().enumerate() {
        assert!(emb_opt.is_some(), "embedding {} should exist", i);
        assert_eq!(
            emb_opt.as_ref().unwrap().len(),
            1536,
            "embedding {} should have 1536 dimensions",
            i
        );
    }
    println!("RESULT: PASSED");
}

#[test]
fn test_embedding_exists() {
    let (db, _tmp) = setup_db();
    let node = create_test_node();
    let fake_id = uuid::Uuid::new_v4();

    db.store_node(&node).expect("store");

    println!("=== EMBEDDING EXISTS TEST ===");

    let exists = db.embedding_exists(&node.id).expect("check exists");
    println!("VERIFY: embedding_exists for stored node = {}", exists);
    assert!(exists, "embedding should exist for stored node");

    let not_exists = db.embedding_exists(&fake_id).expect("check not exists");
    println!("VERIFY: embedding_exists for fake id = {}", not_exists);
    assert!(!not_exists, "embedding should not exist for fake id");

    println!("RESULT: PASSED");
}

#[test]
fn test_delete_embedding() {
    let (db, _tmp) = setup_db();
    let node = create_test_node();
    let node_id = node.id;

    db.store_node(&node).expect("store");

    println!("=== DELETE EMBEDDING TEST ===");

    // Verify embedding exists
    assert!(
        db.embedding_exists(&node_id).expect("check"),
        "embedding should exist"
    );

    // Delete embedding
    db.delete_embedding(&node_id).expect("delete embedding");

    // Verify embedding is gone
    let exists = db.embedding_exists(&node_id).expect("check after delete");
    println!("VERIFY: embedding_exists after delete = {}", exists);
    assert!(!exists, "embedding should not exist after delete");

    println!("RESULT: PASSED");
}
