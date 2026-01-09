//! Additional tests for NodeMetadata.
//!
//! TASK-M02-003: NodeMetadata Comprehensive Tests
//! These tests complement the inline tests in metadata.rs.

use super::*;
use serde_json::json;

#[test]
fn test_node_metadata_custom_attributes() {
    let mut meta = NodeMetadata::new();

    meta.set_custom("string_val", json!("hello"));
    meta.set_custom("number_val", json!(42));
    meta.set_custom("bool_val", json!(true));
    meta.set_custom("array_val", json!([1, 2, 3]));
    meta.set_custom("object_val", json!({"key": "value"}));

    assert_eq!(meta.get_custom("string_val"), Some(&json!("hello")));
    assert_eq!(meta.get_custom("number_val"), Some(&json!(42)));
    assert_eq!(meta.get_custom("bool_val"), Some(&json!(true)));
    assert_eq!(meta.get_custom("array_val"), Some(&json!([1, 2, 3])));
    assert!(meta.get_custom("nonexistent").is_none());
}

#[test]
fn test_node_metadata_custom_overwrite() {
    let mut meta = NodeMetadata::new();

    meta.set_custom("key", json!("original"));
    assert_eq!(meta.get_custom("key"), Some(&json!("original")));

    meta.set_custom("key", json!("updated"));
    assert_eq!(meta.get_custom("key"), Some(&json!("updated")));
}

#[test]
fn test_node_metadata_custom_remove() {
    let mut meta = NodeMetadata::new();

    meta.set_custom("to_remove", json!("value"));
    assert!(meta.get_custom("to_remove").is_some());

    let removed = meta.remove_custom("to_remove");
    assert_eq!(removed, Some(json!("value")));
    assert!(meta.get_custom("to_remove").is_none());

    // Removing again should return None
    assert!(meta.remove_custom("to_remove").is_none());
}

#[test]
fn test_node_metadata_mark_consolidated() {
    let mut meta = NodeMetadata::new();

    assert!(!meta.consolidated);
    assert!(meta.consolidated_at.is_none());

    meta.mark_consolidated();

    assert!(meta.consolidated);
    assert!(meta.consolidated_at.is_some());

    // Timestamp should be recent
    let now = chrono::Utc::now();
    let diff = now - meta.consolidated_at.unwrap();
    assert!(
        diff.num_seconds() < 1,
        "Consolidated timestamp should be recent"
    );
}

#[test]
fn test_node_metadata_mark_deleted() {
    let mut meta = NodeMetadata::new();

    assert!(!meta.deleted);
    assert!(meta.deleted_at.is_none());

    meta.mark_deleted();

    assert!(meta.deleted);
    assert!(meta.deleted_at.is_some());

    let now = chrono::Utc::now();
    let diff = now - meta.deleted_at.unwrap();
    assert!(diff.num_seconds() < 1, "Deleted timestamp should be recent");
}

#[test]
fn test_node_metadata_restore() {
    let mut meta = NodeMetadata::new();

    meta.mark_deleted();
    assert!(meta.deleted);
    assert!(meta.deleted_at.is_some());

    meta.restore();
    assert!(!meta.deleted);
    assert!(meta.deleted_at.is_none());
}

#[test]
fn test_node_metadata_soft_delete_restore_cycle() {
    let mut meta = NodeMetadata::new();

    // Cycle 1
    meta.mark_deleted();
    assert!(meta.deleted);
    let deleted_at_1 = meta.deleted_at;
    assert!(deleted_at_1.is_some());

    meta.restore();
    assert!(!meta.deleted);
    assert!(meta.deleted_at.is_none());

    // Cycle 2 - verify timestamps are fresh
    std::thread::sleep(std::time::Duration::from_millis(10));
    meta.mark_deleted();
    assert!(meta.deleted);
    let deleted_at_2 = meta.deleted_at;
    assert!(deleted_at_2.is_some());
    assert!(
        deleted_at_2 != deleted_at_1,
        "New deletion should have fresh timestamp"
    );
}

#[test]
fn test_node_metadata_version_increment() {
    let mut meta = NodeMetadata::new();

    assert_eq!(meta.version, 1);

    meta.increment_version();
    assert_eq!(meta.version, 2);

    meta.increment_version();
    assert_eq!(meta.version, 3);
}

#[test]
fn test_node_metadata_estimated_size_basic() {
    let meta = NodeMetadata::new();
    let size = meta.estimated_size();

    // Should have some base size
    assert!(size > 0, "Size should be positive");
}

#[test]
fn test_node_metadata_estimated_size_with_data() {
    let mut meta = NodeMetadata::new();
    meta.source = Some("very long source string that takes up space".to_string());
    meta.language = Some("en-US".to_string());
    meta.add_tag("tag1");
    meta.add_tag("tag2");
    meta.add_tag("tag3");
    meta.child_ids.push(uuid::Uuid::new_v4());
    meta.child_ids.push(uuid::Uuid::new_v4());
    meta.set_custom("key1", json!("value1"));
    meta.set_custom("key2", json!(12345));

    let size_with_data = meta.estimated_size();
    let empty_size = NodeMetadata::new().estimated_size();

    assert!(
        size_with_data > empty_size,
        "Size with data {} should be > empty size {}",
        size_with_data,
        empty_size
    );
}

#[test]
fn test_node_metadata_hierarchical_relationships() {
    let parent_id = uuid::Uuid::new_v4();
    let child1 = uuid::Uuid::new_v4();
    let child2 = uuid::Uuid::new_v4();

    let mut meta = NodeMetadata::new();
    meta.parent_id = Some(parent_id);
    meta.child_ids.push(child1);
    meta.child_ids.push(child2);

    assert_eq!(meta.parent_id, Some(parent_id));
    assert_eq!(meta.child_ids.len(), 2);
    assert!(meta.child_ids.contains(&child1));
    assert!(meta.child_ids.contains(&child2));
}

#[test]
fn test_node_metadata_clone() {
    let mut original = NodeMetadata::new();
    original.source = Some("source".to_string());
    original.add_tag("tag");
    original.set_custom("key", json!(1));
    original.mark_consolidated();

    let cloned = original.clone();
    assert_eq!(original, cloned);

    // Verify deep clone (mutating clone doesn't affect original)
    let mut cloned_mut = original.clone();
    cloned_mut.add_tag("new_tag");
    assert_ne!(original.tags.len(), cloned_mut.tags.len());
}

#[test]
fn test_node_metadata_tag_order_preserved() {
    let mut meta = NodeMetadata::new();
    meta.add_tag("first");
    meta.add_tag("second");
    meta.add_tag("third");

    assert_eq!(meta.tags[0], "first");
    assert_eq!(meta.tags[1], "second");
    assert_eq!(meta.tags[2], "third");
}

#[test]
fn test_node_metadata_empty_string_tag() {
    let mut meta = NodeMetadata::new();
    meta.add_tag("");
    assert!(meta.has_tag(""));
    assert_eq!(meta.tags.len(), 1);
}

#[test]
fn test_node_metadata_unicode_tags() {
    let mut meta = NodeMetadata::new();
    meta.add_tag("日本語");
    meta.add_tag("émoji 🎉");
    meta.add_tag("مرحبا");

    assert!(meta.has_tag("日本語"));
    assert!(meta.has_tag("émoji 🎉"));
    assert!(meta.has_tag("مرحبا"));
    assert_eq!(meta.tags.len(), 3);
}

#[test]
fn test_node_metadata_utl_score_bounds() {
    let mut meta = NodeMetadata::new();

    meta.utl_score = Some(0.0);
    assert_eq!(meta.utl_score, Some(0.0));

    meta.utl_score = Some(1.0);
    assert_eq!(meta.utl_score, Some(1.0));

    meta.utl_score = Some(0.5);
    assert_eq!(meta.utl_score, Some(0.5));
}

#[test]
fn test_node_metadata_modality_all_variants() {
    let mut meta = NodeMetadata::new();

    meta.modality = crate::types::Modality::Text;
    assert_eq!(meta.modality, crate::types::Modality::Text);

    meta.modality = crate::types::Modality::Code;
    assert_eq!(meta.modality, crate::types::Modality::Code);

    meta.modality = crate::types::Modality::Image;
    assert_eq!(meta.modality, crate::types::Modality::Image);

    meta.modality = crate::types::Modality::Audio;
    assert_eq!(meta.modality, crate::types::Modality::Audio);

    meta.modality = crate::types::Modality::Structured;
    assert_eq!(meta.modality, crate::types::Modality::Structured);
}

#[test]
fn test_node_metadata_rationale_with_special_chars() {
    let mut meta = NodeMetadata::new();
    let rationale = r#"Rationale with "quotes", 'apostrophes', and
newlines plus unicode: 日本語"#;

    meta.rationale = Some(rationale.to_string());
    assert_eq!(meta.rationale, Some(rationale.to_string()));

    // Verify survives serialization
    let json_str = serde_json::to_string(&meta).unwrap();
    let restored: NodeMetadata = serde_json::from_str(&json_str).unwrap();
    assert_eq!(restored.rationale, Some(rationale.to_string()));
}

#[test]
fn test_node_metadata_multiple_child_ids() {
    let mut meta = NodeMetadata::new();
    for _ in 0..10 {
        meta.child_ids.push(uuid::Uuid::new_v4());
    }
    assert_eq!(meta.child_ids.len(), 10);
}

#[test]
fn test_node_metadata_custom_nested_json() {
    let mut meta = NodeMetadata::new();
    let nested = json!({"level1": {"level2": [1, 2, 3]}});
    meta.set_custom("nested", nested.clone());
    assert_eq!(meta.get_custom("nested"), Some(&nested));
}
