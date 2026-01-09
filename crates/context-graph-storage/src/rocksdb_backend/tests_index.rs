//! Secondary index operation tests.
//!
//! Tests use REAL data stored in RocksDB - NO mocks per constitution.yaml.
//! Each test stores nodes via store_node() then queries via index methods.
//!
//! # Test Strategy
//! 1. Create temp database
//! 2. Store nodes with known attributes
//! 3. Query using index methods
//! 4. Verify correct nodes are returned
//!
//! # Edge Cases Covered
//! - Empty database queries
//! - Pagination with limit/offset
//! - Special characters in tags
//! - Time range boundaries

use chrono::{Duration, Utc};

use super::tests_node::{create_node_with_tags, create_temp_db, create_valid_test_node};
use context_graph_core::types::JohariQuadrant;

// =========================================================================
// get_nodes_by_quadrant Tests
// =========================================================================

#[test]
fn test_get_nodes_by_quadrant_empty() {
    println!("=== TEST: get_nodes_by_quadrant on empty quadrant ===");
    let (_tmp, db) = create_temp_db();

    println!("BEFORE: Database has 0 nodes in Blind quadrant");
    let result = db.get_nodes_by_quadrant(JohariQuadrant::Blind, None, 0);

    println!("AFTER: Query result = {:?}", result.is_ok());
    assert!(result.is_ok());
    let nodes = result.unwrap();
    println!("RESULT: Empty quadrant returns {} nodes", nodes.len());
    assert!(
        nodes.is_empty(),
        "Empty quadrant should return empty Vec, NOT error"
    );
}

#[test]
fn test_get_nodes_by_quadrant_finds_nodes() {
    println!("=== TEST: get_nodes_by_quadrant finds stored nodes ===");
    let (_tmp, db) = create_temp_db();

    // Store 3 nodes in Open quadrant
    let mut node1 = create_valid_test_node();
    node1.quadrant = JohariQuadrant::Open;
    db.store_node(&node1).expect("store 1");

    let mut node2 = create_valid_test_node();
    node2.quadrant = JohariQuadrant::Open;
    db.store_node(&node2).expect("store 2");

    let mut node3 = create_valid_test_node();
    node3.quadrant = JohariQuadrant::Hidden;
    db.store_node(&node3).expect("store 3");

    println!("BEFORE: Stored 2 Open nodes, 1 Hidden node");
    let open_nodes = db
        .get_nodes_by_quadrant(JohariQuadrant::Open, None, 0)
        .unwrap();
    let hidden_nodes = db
        .get_nodes_by_quadrant(JohariQuadrant::Hidden, None, 0)
        .unwrap();

    println!(
        "AFTER: Open={}, Hidden={}",
        open_nodes.len(),
        hidden_nodes.len()
    );
    assert_eq!(open_nodes.len(), 2, "Should find 2 Open nodes");
    assert_eq!(hidden_nodes.len(), 1, "Should find 1 Hidden node");
    assert!(open_nodes.contains(&node1.id), "Should contain node1");
    assert!(open_nodes.contains(&node2.id), "Should contain node2");
    assert!(hidden_nodes.contains(&node3.id), "Should contain node3");
    println!("RESULT: All nodes found in correct quadrants ✓");
}

#[test]
fn test_get_nodes_by_quadrant_with_limit() {
    println!("=== TEST: get_nodes_by_quadrant with limit ===");
    let (_tmp, db) = create_temp_db();

    // Store 5 nodes
    for i in 0..5 {
        let mut node = create_valid_test_node();
        node.quadrant = JohariQuadrant::Open;
        db.store_node(&node)
            .unwrap_or_else(|_| panic!("store {}", i));
    }

    println!("BEFORE: Stored 5 nodes, querying with limit=3");
    let result = db
        .get_nodes_by_quadrant(JohariQuadrant::Open, Some(3), 0)
        .unwrap();

    println!("AFTER: Got {} nodes", result.len());
    assert_eq!(result.len(), 3, "Limit should cap results at 3");
}

#[test]
fn test_get_nodes_by_quadrant_with_offset() {
    println!("=== TEST: get_nodes_by_quadrant with offset ===");
    let (_tmp, db) = create_temp_db();

    // Store 5 nodes
    for i in 0..5 {
        let mut node = create_valid_test_node();
        node.quadrant = JohariQuadrant::Open;
        db.store_node(&node)
            .unwrap_or_else(|_| panic!("store {}", i));
    }

    println!("BEFORE: Stored 5 nodes, querying with offset=2");
    let result = db
        .get_nodes_by_quadrant(JohariQuadrant::Open, None, 2)
        .unwrap();

    println!("AFTER: Got {} nodes", result.len());
    assert_eq!(result.len(), 3, "Offset 2 from 5 nodes = 3 results");
}

#[test]
fn test_get_nodes_by_quadrant_all_quadrants() {
    println!("=== TEST: get_nodes_by_quadrant for all quadrants ===");
    let (_tmp, db) = create_temp_db();

    // Store one node in each quadrant
    for quadrant in JohariQuadrant::all() {
        let mut node = create_valid_test_node();
        node.quadrant = quadrant;
        db.store_node(&node)
            .unwrap_or_else(|_| panic!("store {:?}", quadrant));
    }

    // Query each quadrant
    for quadrant in JohariQuadrant::all() {
        let result = db.get_nodes_by_quadrant(quadrant, None, 0).unwrap();
        assert_eq!(result.len(), 1, "Should find 1 node in {:?}", quadrant);
        println!("  {:?}: {} nodes", quadrant, result.len());
    }
    println!("RESULT: All 4 quadrants queried successfully ✓");
}

// =========================================================================
// get_nodes_by_tag Tests
// =========================================================================

#[test]
fn test_get_nodes_by_tag_empty() {
    println!("=== TEST: get_nodes_by_tag on nonexistent tag ===");
    let (_tmp, db) = create_temp_db();

    println!("BEFORE: Database has no nodes with tag 'nonexistent'");
    let result = db.get_nodes_by_tag("nonexistent", None, 0);

    println!("AFTER: Query result = {:?}", result.is_ok());
    assert!(result.is_ok());
    let nodes = result.unwrap();
    assert!(nodes.is_empty(), "Nonexistent tag should return empty Vec");
}

#[test]
fn test_get_nodes_by_tag_finds_nodes() {
    println!("=== TEST: get_nodes_by_tag finds tagged nodes ===");
    let (_tmp, db) = create_temp_db();

    let node1 = create_node_with_tags(vec!["important", "work"]);
    db.store_node(&node1).unwrap();

    let node2 = create_node_with_tags(vec!["important", "personal"]);
    db.store_node(&node2).unwrap();

    let node3 = create_node_with_tags(vec!["personal"]);
    db.store_node(&node3).unwrap();

    println!("BEFORE: Stored 3 nodes with various tags");
    let important = db.get_nodes_by_tag("important", None, 0).unwrap();
    let personal = db.get_nodes_by_tag("personal", None, 0).unwrap();
    let work = db.get_nodes_by_tag("work", None, 0).unwrap();

    println!(
        "AFTER: important={}, personal={}, work={}",
        important.len(),
        personal.len(),
        work.len()
    );
    assert_eq!(important.len(), 2, "Should find 2 'important' nodes");
    assert_eq!(personal.len(), 2, "Should find 2 'personal' nodes");
    assert_eq!(work.len(), 1, "Should find 1 'work' node");
    assert!(important.contains(&node1.id));
    assert!(important.contains(&node2.id));
    println!("RESULT: All tagged nodes found correctly ✓");
}

#[test]
fn test_get_nodes_by_tag_with_pagination() {
    println!("=== TEST: get_nodes_by_tag with pagination ===");
    let (_tmp, db) = create_temp_db();

    // Store 10 nodes with same tag
    let mut all_ids = Vec::new();
    for i in 0..10 {
        let node = create_node_with_tags(vec!["paginated"]);
        all_ids.push(node.id);
        db.store_node(&node)
            .unwrap_or_else(|_| panic!("store {}", i));
    }

    println!("BEFORE: Stored 10 nodes with tag 'paginated'");

    // First page
    let page1 = db.get_nodes_by_tag("paginated", Some(3), 0).unwrap();
    assert_eq!(page1.len(), 3, "First page should have 3 items");

    // Second page
    let page2 = db.get_nodes_by_tag("paginated", Some(3), 3).unwrap();
    assert_eq!(page2.len(), 3, "Second page should have 3 items");

    // No overlap
    for id in &page1 {
        assert!(!page2.contains(id), "Pages should not overlap");
    }

    println!(
        "AFTER: Page1={} items, Page2={} items, no overlap",
        page1.len(),
        page2.len()
    );
    println!("RESULT: Pagination works correctly ✓");
}

#[test]
fn test_get_nodes_by_tag_similar_tags() {
    println!("=== TEST: get_nodes_by_tag distinguishes similar tags ===");
    let (_tmp, db) = create_temp_db();

    // Tags that could be confused with prefix matching
    let node1 = create_node_with_tags(vec!["test"]);
    db.store_node(&node1).unwrap();

    let node2 = create_node_with_tags(vec!["testing"]);
    db.store_node(&node2).unwrap();

    let node3 = create_node_with_tags(vec!["test-case"]);
    db.store_node(&node3).unwrap();

    // Each tag query should return exactly 1 node
    let test = db.get_nodes_by_tag("test", None, 0).unwrap();
    let testing = db.get_nodes_by_tag("testing", None, 0).unwrap();
    let test_case = db.get_nodes_by_tag("test-case", None, 0).unwrap();

    assert_eq!(test.len(), 1, "'test' should find exactly 1 node");
    assert_eq!(testing.len(), 1, "'testing' should find exactly 1 node");
    assert_eq!(test_case.len(), 1, "'test-case' should find exactly 1 node");
    assert!(test.contains(&node1.id));
    assert!(testing.contains(&node2.id));
    assert!(test_case.contains(&node3.id));
    println!("RESULT: Similar tags correctly distinguished ✓");
}

// =========================================================================
// get_nodes_by_source Tests
// =========================================================================

#[test]
fn test_get_nodes_by_source_empty() {
    println!("=== TEST: get_nodes_by_source on unknown source ===");
    let (_tmp, db) = create_temp_db();

    let result = db.get_nodes_by_source("unknown-source", None, 0);
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "Unknown source should return empty Vec"
    );
}

#[test]
fn test_get_nodes_by_source_finds_nodes() {
    println!("=== TEST: get_nodes_by_source finds nodes ===");
    let (_tmp, db) = create_temp_db();

    let mut node1 = create_valid_test_node();
    node1.metadata.source = Some("api-gateway".to_string());
    db.store_node(&node1).unwrap();

    let mut node2 = create_valid_test_node();
    node2.metadata.source = Some("api-gateway".to_string());
    db.store_node(&node2).unwrap();

    let mut node3 = create_valid_test_node();
    node3.metadata.source = Some("web-scraper".to_string());
    db.store_node(&node3).unwrap();

    println!("BEFORE: Stored 2 api-gateway nodes, 1 web-scraper node");
    let api_nodes = db.get_nodes_by_source("api-gateway", None, 0).unwrap();
    let web_nodes = db.get_nodes_by_source("web-scraper", None, 0).unwrap();

    println!(
        "AFTER: api-gateway={}, web-scraper={}",
        api_nodes.len(),
        web_nodes.len()
    );
    assert_eq!(api_nodes.len(), 2, "Should find 2 api-gateway nodes");
    assert_eq!(web_nodes.len(), 1, "Should find 1 web-scraper node");
    assert!(api_nodes.contains(&node1.id));
    assert!(api_nodes.contains(&node2.id));
    assert!(web_nodes.contains(&node3.id));
    println!("RESULT: Source query works correctly ✓");
}

#[test]
fn test_get_nodes_by_source_no_source() {
    println!("=== TEST: Nodes without source are not indexed ===");
    let (_tmp, db) = create_temp_db();

    // Create node without source
    let mut node = create_valid_test_node();
    node.metadata.source = None;
    db.store_node(&node).unwrap();

    // Create node with source
    let mut node2 = create_valid_test_node();
    node2.metadata.source = Some("has-source".to_string());
    db.store_node(&node2).unwrap();

    let with_source = db.get_nodes_by_source("has-source", None, 0).unwrap();
    assert_eq!(with_source.len(), 1);
    assert!(with_source.contains(&node2.id));
    println!("RESULT: Only nodes with source are indexed ✓");
}

// =========================================================================
// get_nodes_in_time_range Tests
// =========================================================================

#[test]
fn test_get_nodes_in_time_range_empty() {
    println!("=== TEST: get_nodes_in_time_range on empty database ===");
    let (_tmp, db) = create_temp_db();

    let start = Utc::now() - Duration::hours(1);
    let end = Utc::now();

    let result = db.get_nodes_in_time_range(start, end, None, 0);
    assert!(result.is_ok());
    assert!(
        result.unwrap().is_empty(),
        "Empty database should return empty Vec"
    );
}

#[test]
fn test_get_nodes_in_time_range_finds_nodes() {
    println!("=== TEST: get_nodes_in_time_range finds nodes ===");
    let (_tmp, db) = create_temp_db();

    // Store nodes (all created "now")
    let node1 = create_valid_test_node();
    let created_at = node1.created_at;
    db.store_node(&node1).unwrap();

    let node2 = create_valid_test_node();
    db.store_node(&node2).unwrap();

    println!("BEFORE: Stored 2 nodes at approximately now");

    // Query with range that includes all nodes
    let start = created_at - Duration::seconds(1);
    let end = Utc::now() + Duration::seconds(1);

    let result = db.get_nodes_in_time_range(start, end, None, 0).unwrap();

    println!("AFTER: Found {} nodes in time range", result.len());
    assert_eq!(result.len(), 2, "Should find 2 nodes in range");
    assert!(result.contains(&node1.id));
    assert!(result.contains(&node2.id));
    println!("RESULT: Time range query works correctly ✓");
}

#[test]
fn test_get_nodes_in_time_range_respects_boundaries() {
    println!("=== TEST: Time range respects start/end boundaries ===");
    let (_tmp, db) = create_temp_db();

    let node = create_valid_test_node();
    let node_time = node.created_at;
    db.store_node(&node).unwrap();

    println!("BEFORE: Node created at {:?}", node_time);

    // Range BEFORE node
    let before = db
        .get_nodes_in_time_range(
            node_time - Duration::hours(2),
            node_time - Duration::hours(1),
            None,
            0,
        )
        .unwrap();
    println!("  Range before: {} nodes", before.len());
    assert!(before.is_empty(), "Range before node should be empty");

    // Range AFTER node
    let after = db
        .get_nodes_in_time_range(
            node_time + Duration::hours(1),
            node_time + Duration::hours(2),
            None,
            0,
        )
        .unwrap();
    println!("  Range after: {} nodes", after.len());
    assert!(after.is_empty(), "Range after node should be empty");

    // Range INCLUDING node
    let including = db
        .get_nodes_in_time_range(
            node_time - Duration::seconds(1),
            node_time + Duration::seconds(1),
            None,
            0,
        )
        .unwrap();
    println!("  Range including: {} nodes", including.len());
    assert_eq!(including.len(), 1, "Range including node should find it");
    assert!(including.contains(&node.id));

    println!("RESULT: Time boundaries respected ✓");
}

#[test]
fn test_get_nodes_in_time_range_with_pagination() {
    println!("=== TEST: get_nodes_in_time_range with pagination ===");
    let (_tmp, db) = create_temp_db();

    // Store 5 nodes
    let mut first_time = None;
    for i in 0..5 {
        let node = create_valid_test_node();
        if first_time.is_none() {
            first_time = Some(node.created_at);
        }
        db.store_node(&node)
            .unwrap_or_else(|_| panic!("store {}", i));
    }

    let start = first_time.unwrap() - Duration::seconds(1);
    let end = Utc::now() + Duration::seconds(1);

    // First page
    let page1 = db.get_nodes_in_time_range(start, end, Some(2), 0).unwrap();
    assert_eq!(page1.len(), 2, "First page should have 2 items");

    // Second page
    let page2 = db.get_nodes_in_time_range(start, end, Some(2), 2).unwrap();
    assert_eq!(page2.len(), 2, "Second page should have 2 items");

    // No overlap
    for id in &page1 {
        assert!(!page2.contains(id), "Pages should not overlap");
    }

    println!("RESULT: Time range pagination works ✓");
}

// =========================================================================
// Pagination Tests (Cross-Method)
// =========================================================================

#[test]
fn test_pagination_correctness() {
    println!("=== TEST: Pagination returns correct subsets ===");
    let (_tmp, db) = create_temp_db();

    // Store 10 nodes with same tag
    let mut all_ids = Vec::new();
    for _ in 0..10 {
        let node = create_node_with_tags(vec!["bulk"]);
        all_ids.push(node.id);
        db.store_node(&node).unwrap();
    }

    // Get all via no limit
    let all = db.get_nodes_by_tag("bulk", None, 0).unwrap();
    assert_eq!(all.len(), 10, "Should find all 10 nodes");

    // Get pages of 3
    let mut collected = Vec::new();
    for page in 0..4 {
        let offset = page * 3;
        let result = db.get_nodes_by_tag("bulk", Some(3), offset).unwrap();
        collected.extend(result);
    }

    // Should have collected all 10 (3+3+3+1)
    assert_eq!(collected.len(), 10, "Should collect all 10 via pagination");

    // All original IDs should be present
    for id in &all_ids {
        assert!(
            collected.contains(id),
            "All IDs should be collected via pagination"
        );
    }

    println!("RESULT: All 10 nodes collected via pagination ✓");
}

// =========================================================================
// Edge Case Tests
// =========================================================================

#[test]
fn edge_case_tag_with_special_chars() {
    println!("=== EDGE CASE: Tag with special characters ===");
    let (_tmp, db) = create_temp_db();

    // Test various special character tags
    let test_tags = &[
        "tag:with:colons",
        "tag/with/slashes",
        "tag with spaces",
        "tag-with-dashes",
        "tag_with_underscores",
    ];

    for tag in test_tags {
        let node = create_node_with_tags(vec![tag]);
        db.store_node(&node).unwrap();

        let result = db.get_nodes_by_tag(tag, None, 0).unwrap();
        assert_eq!(result.len(), 1, "Should find node with tag '{}'", tag);
        assert!(result.contains(&node.id));
        println!("  '{}': FOUND ✓", tag);
    }

    println!("RESULT: All special character tags work ✓");
}

#[test]
fn edge_case_unicode_tag() {
    println!("=== EDGE CASE: Unicode tag ===");
    let (_tmp, db) = create_temp_db();

    let unicode_tag = "日本語タグ";
    let node = create_node_with_tags(vec![unicode_tag]);
    db.store_node(&node).unwrap();

    let result = db.get_nodes_by_tag(unicode_tag, None, 0).unwrap();
    assert_eq!(result.len(), 1, "Should find node with unicode tag");
    assert!(result.contains(&node.id));

    println!("RESULT: Unicode tag '{}' works ✓", unicode_tag);
}

#[test]
fn edge_case_offset_beyond_results() {
    println!("=== EDGE CASE: Offset beyond total results ===");
    let (_tmp, db) = create_temp_db();

    for _ in 0..3 {
        let mut node = create_valid_test_node();
        node.quadrant = JohariQuadrant::Open;
        db.store_node(&node).unwrap();
    }

    println!("BEFORE: 3 nodes stored, querying with offset=100");
    let result = db
        .get_nodes_by_quadrant(JohariQuadrant::Open, None, 100)
        .unwrap();

    println!("AFTER: Got {} nodes", result.len());
    assert!(
        result.is_empty(),
        "Offset beyond results should return empty Vec"
    );
}

#[test]
fn edge_case_limit_zero() {
    println!("=== EDGE CASE: Limit of zero ===");
    let (_tmp, db) = create_temp_db();

    let node = create_valid_test_node();
    db.store_node(&node).unwrap();

    println!("BEFORE: 1 node stored, querying with limit=0");
    let result = db.get_nodes_by_quadrant(node.quadrant, Some(0), 0).unwrap();

    println!("AFTER: Got {} nodes", result.len());
    assert!(result.is_empty(), "Limit 0 should return empty Vec");
}

#[test]
fn edge_case_limit_greater_than_results() {
    println!("=== EDGE CASE: Limit greater than available results ===");
    let (_tmp, db) = create_temp_db();

    for _ in 0..3 {
        let mut node = create_valid_test_node();
        node.quadrant = JohariQuadrant::Blind;
        db.store_node(&node).unwrap();
    }

    println!("BEFORE: 3 nodes stored, querying with limit=100");
    let result = db
        .get_nodes_by_quadrant(JohariQuadrant::Blind, Some(100), 0)
        .unwrap();

    println!("AFTER: Got {} nodes", result.len());
    assert_eq!(result.len(), 3, "Limit > count should return all available");
}

#[test]
fn edge_case_empty_tag() {
    println!("=== EDGE CASE: Empty tag string ===");
    let (_tmp, db) = create_temp_db();

    // Query for empty tag (should find nothing, not error)
    let result = db.get_nodes_by_tag("", None, 0).unwrap();
    assert!(result.is_empty(), "Empty tag should return empty Vec");

    println!("RESULT: Empty tag query handled gracefully ✓");
}

#[test]
fn edge_case_source_with_special_chars() {
    println!("=== EDGE CASE: Source with special characters ===");
    let (_tmp, db) = create_temp_db();

    let test_sources = &[
        "https://example.com/api/v1",
        "file:///home/user/data",
        "source:with:colons",
        "source with spaces",
    ];

    for source in test_sources {
        let mut node = create_valid_test_node();
        node.metadata.source = Some(source.to_string());
        db.store_node(&node).unwrap();

        let result = db.get_nodes_by_source(source, None, 0).unwrap();
        assert_eq!(result.len(), 1, "Should find node with source '{}'", source);
        println!("  '{}': FOUND ✓", source);
    }

    println!("RESULT: All special character sources work ✓");
}

// =========================================================================
// Full State Verification Tests
// =========================================================================

#[test]
fn evidence_index_consistency() {
    println!("=== EVIDENCE: Index consistency after store_node ===");
    let (_tmp, db) = create_temp_db();

    // Create a node with all indexable attributes
    let mut node = create_node_with_tags(vec!["evidence", "verification"]);
    node.quadrant = JohariQuadrant::Unknown;
    node.metadata.source = Some("evidence-source".to_string());
    let node_id = node.id;
    let created_at = node.created_at;

    db.store_node(&node).unwrap();

    println!("STORED: Node {} with quadrant=Unknown, tags=[evidence,verification], source=evidence-source",
        node_id);

    // Verify via quadrant query
    let quadrant_result = db
        .get_nodes_by_quadrant(JohariQuadrant::Unknown, None, 0)
        .unwrap();
    assert!(
        quadrant_result.contains(&node_id),
        "Node should be in Unknown quadrant index"
    );
    println!("  ✓ Found in Unknown quadrant index");

    // Verify via tag queries
    let tag1_result = db.get_nodes_by_tag("evidence", None, 0).unwrap();
    assert!(
        tag1_result.contains(&node_id),
        "Node should be in 'evidence' tag index"
    );
    println!("  ✓ Found in 'evidence' tag index");

    let tag2_result = db.get_nodes_by_tag("verification", None, 0).unwrap();
    assert!(
        tag2_result.contains(&node_id),
        "Node should be in 'verification' tag index"
    );
    println!("  ✓ Found in 'verification' tag index");

    // Verify via source query
    let source_result = db.get_nodes_by_source("evidence-source", None, 0).unwrap();
    assert!(
        source_result.contains(&node_id),
        "Node should be in source index"
    );
    println!("  ✓ Found in 'evidence-source' source index");

    // Verify via temporal query
    let temporal_result = db
        .get_nodes_in_time_range(
            created_at - Duration::seconds(1),
            created_at + Duration::seconds(1),
            None,
            0,
        )
        .unwrap();
    assert!(
        temporal_result.contains(&node_id),
        "Node should be in temporal index"
    );
    println!("  ✓ Found in temporal index");

    println!("RESULT: All 5 indexes correctly contain the node ✓");
}

#[test]
fn evidence_multiple_nodes_all_indexes() {
    println!("=== EVIDENCE: Multiple nodes across all indexes ===");
    let (_tmp, db) = create_temp_db();

    // Store nodes with different attributes
    let mut nodes = Vec::new();

    for i in 0..4 {
        let quadrant = match i % 4 {
            0 => JohariQuadrant::Open,
            1 => JohariQuadrant::Hidden,
            2 => JohariQuadrant::Blind,
            _ => JohariQuadrant::Unknown,
        };

        let mut node = create_node_with_tags(vec!["shared-tag"]);
        node.quadrant = quadrant;
        node.metadata.source = Some(format!("source-{}", i));
        nodes.push((node.id, quadrant));
        db.store_node(&node).unwrap();
    }

    // Verify each node is in correct quadrant
    for (node_id, quadrant) in &nodes {
        let result = db.get_nodes_by_quadrant(*quadrant, None, 0).unwrap();
        assert!(
            result.contains(node_id),
            "Node {} should be in {:?}",
            node_id,
            quadrant
        );
    }
    println!("  ✓ All nodes in correct quadrant indexes");

    // Verify shared tag
    let tag_result = db.get_nodes_by_tag("shared-tag", None, 0).unwrap();
    assert_eq!(tag_result.len(), 4, "All 4 nodes should have 'shared-tag'");
    println!("  ✓ All 4 nodes found via shared tag");

    // Verify individual sources
    for i in 0..4 {
        let result = db
            .get_nodes_by_source(&format!("source-{}", i), None, 0)
            .unwrap();
        assert_eq!(result.len(), 1, "Each source should have 1 node");
    }
    println!("  ✓ All individual sources resolve correctly");

    println!("RESULT: Multiple nodes verified across all index types ✓");
}
