//! Tag and temporal index tests.
//!
//! Tests tag indexing, multiple tags per node, and temporal range queries.

use super::common::{create_node_with_content, create_test_node, setup_db};
use chrono::{Duration, Utc};

#[test]
fn test_tag_index_consistency() {
    let (db, _tmp) = setup_db();
    let mut node = create_test_node();
    node.metadata.tags = vec!["rust".to_string(), "async".to_string()];

    println!("=== TAG INDEX TEST ===");
    println!("BEFORE: tags={:?}", node.metadata.tags);
    db.store_node(&node).expect("store");

    // Query for rust tag
    let rust_nodes = db.get_nodes_by_tag("rust", None, 0).expect("query rust");
    println!("VERIFY: 'rust' tag has {} nodes", rust_nodes.len());
    assert!(
        rust_nodes.contains(&node.id),
        "node should be indexed under 'rust'"
    );

    // Update tags - remove rust, add tokio
    node.metadata.tags = vec!["async".to_string(), "tokio".to_string()];
    println!("TRIGGER: Update tags to {:?}", node.metadata.tags);
    db.update_node(&node).expect("update");

    // Verify indexes updated
    let rust_after = db.get_nodes_by_tag("rust", None, 0).expect("query rust");
    let tokio_nodes = db.get_nodes_by_tag("tokio", None, 0).expect("query tokio");

    println!(
        "VERIFY: rust={}, tokio={}",
        rust_after.len(),
        tokio_nodes.len()
    );
    assert!(
        !rust_after.contains(&node.id),
        "node should NOT be in 'rust' index after update"
    );
    assert!(
        tokio_nodes.contains(&node.id),
        "node SHOULD be in 'tokio' index after update"
    );
    println!("RESULT: PASSED");
}

#[test]
fn test_multiple_tags_per_node() {
    let (db, _tmp) = setup_db();
    let mut node = create_test_node();
    node.metadata.tags = vec!["tag1".to_string(), "tag2".to_string(), "tag3".to_string()];

    println!("=== MULTIPLE TAGS TEST ===");
    db.store_node(&node).expect("store");

    // Verify node appears in all tag indexes
    for tag in &["tag1", "tag2", "tag3"] {
        let ids = db.get_nodes_by_tag(tag, None, 0).expect("query");
        println!(
            "VERIFY: '{}' contains node: {}",
            tag,
            ids.contains(&node.id)
        );
        assert!(ids.contains(&node.id), "node should be in '{}' index", tag);
    }
    println!("RESULT: PASSED");
}

#[test]
fn test_temporal_index() {
    let (db, _tmp) = setup_db();
    let node1 = create_test_node();
    let node2 = create_test_node();
    let node3 = create_test_node();

    db.store_node(&node1).expect("store 1");
    std::thread::sleep(std::time::Duration::from_millis(10));
    db.store_node(&node2).expect("store 2");
    std::thread::sleep(std::time::Duration::from_millis(10));
    db.store_node(&node3).expect("store 3");

    println!("=== TEMPORAL INDEX TEST ===");
    let start = node1.created_at - Duration::seconds(1);
    let end = Utc::now() + Duration::seconds(1);

    let nodes = db
        .get_nodes_in_time_range(start, end, None, 0)
        .expect("query");
    println!(
        "VERIFY: Found {} nodes in range [{} to {}]",
        nodes.len(),
        start,
        end
    );
    assert!(nodes.len() >= 3, "should find at least 3 nodes");
    assert!(nodes.contains(&node1.id), "should contain node1");
    assert!(nodes.contains(&node2.id), "should contain node2");
    assert!(nodes.contains(&node3.id), "should contain node3");
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_source_index() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: SOURCE INDEX ===");

    let mut node = create_test_node();
    node.metadata.source = Some("test-source-url".to_string());
    db.store_node(&node).expect("store");

    let by_source = db
        .get_nodes_by_source("test-source-url", None, 0)
        .expect("query");
    println!("VERIFY: source index contains {} nodes", by_source.len());
    assert!(
        by_source.contains(&node.id),
        "node should be indexed by source"
    );
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_empty_result() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: EMPTY RESULT ===");

    let result = db
        .get_nodes_by_tag("nonexistent-xyz-tag-12345", None, 0)
        .expect("query");
    println!(
        "BEFORE: Empty DB, TRIGGER: Query nonexistent tag, AFTER: len={}",
        result.len()
    );
    assert!(
        result.is_empty(),
        "should return empty vec for nonexistent tag"
    );
    println!("RESULT: PASSED");
}

#[test]
fn edge_case_limit_and_offset() {
    let (db, _tmp) = setup_db();
    println!("=== EDGE CASE: LIMIT AND OFFSET ===");

    // Store 100 nodes with same tag
    for i in 0..100 {
        let mut node = create_node_with_content(&format!("N{}", i));
        node.metadata.tags.push("test-limit".to_string());
        db.store_node(&node).expect("store");
    }

    // Query with limit
    let limited = db
        .get_nodes_by_tag("test-limit", Some(10), 0)
        .expect("query");
    println!(
        "BEFORE: 100 nodes, TRIGGER: limit=10, AFTER: len={}",
        limited.len()
    );
    assert_eq!(limited.len(), 10, "should return exactly 10 nodes");

    // Query with offset
    let offset = db
        .get_nodes_by_tag("test-limit", Some(10), 50)
        .expect("query");
    println!("TRIGGER: limit=10, offset=50, AFTER: len={}", offset.len());
    assert_eq!(offset.len(), 10, "should return 10 nodes with offset");

    println!("RESULT: PASSED");
}
