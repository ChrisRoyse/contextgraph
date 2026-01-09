//! Private helper functions for key formatting.
//!
//! These functions are used internally by the RocksDB backend
//! to create properly formatted keys for various column families.

use chrono::{DateTime, Utc};
use context_graph_core::marblestone::EdgeType;
use context_graph_core::types::NodeId;

use crate::serialization::serialize_uuid;

/// Format temporal index key: 8-byte timestamp (millis, big-endian) + 16-byte UUID.
///
/// Big-endian ensures lexicographic ordering matches temporal ordering,
/// enabling efficient range scans by time.
#[inline]
pub(crate) fn format_temporal_key(timestamp: DateTime<Utc>, id: &NodeId) -> Vec<u8> {
    let millis = timestamp.timestamp_millis() as u64;
    let mut key = Vec::with_capacity(24);
    key.extend_from_slice(&millis.to_be_bytes());
    key.extend_from_slice(&serialize_uuid(id));
    key
}

/// Format tag index key: tag_bytes + ':' + 16-byte UUID.
///
/// Enables prefix scans by tag name.
#[inline]
pub(crate) fn format_tag_key(tag: &str, id: &NodeId) -> Vec<u8> {
    let mut key = Vec::with_capacity(tag.len() + 1 + 16);
    key.extend_from_slice(tag.as_bytes());
    key.push(b':');
    key.extend_from_slice(&serialize_uuid(id));
    key
}

/// Format source index key: source_bytes + ':' + 16-byte UUID.
///
/// Enables prefix scans by source.
#[inline]
pub(crate) fn format_source_key(source: &str, id: &NodeId) -> Vec<u8> {
    let mut key = Vec::with_capacity(source.len() + 1 + 16);
    key.extend_from_slice(source.as_bytes());
    key.push(b':');
    key.extend_from_slice(&serialize_uuid(id));
    key
}

/// Format edge key: 16-byte source_uuid + 16-byte target_uuid + 1-byte edge_type.
///
/// Total: 33 bytes. Uses big-endian UUID bytes for proper lexicographic ordering.
/// This enables efficient prefix scans by source_id.
///
/// # Key Structure
/// - Bytes 0-15: source_id UUID (16 bytes)
/// - Bytes 16-31: target_id UUID (16 bytes)
/// - Byte 32: edge_type as u8 (1 byte)
#[inline]
pub(crate) fn format_edge_key(
    source_id: &NodeId,
    target_id: &NodeId,
    edge_type: EdgeType,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(33);
    key.extend_from_slice(&serialize_uuid(source_id));
    key.extend_from_slice(&serialize_uuid(target_id));
    key.push(edge_type as u8);
    key
}

/// Format edge prefix for source_id: just the 16-byte source_uuid.
///
/// Used for prefix scans to find all edges from a source node.
/// The prefix extractor in column_families.rs is configured for 16-byte prefixes.
#[inline]
pub(crate) fn format_edge_prefix(source_id: &NodeId) -> Vec<u8> {
    serialize_uuid(source_id).to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_format_temporal_key() {
        let id = uuid::Uuid::new_v4();
        let timestamp = Utc::now();

        let key = format_temporal_key(timestamp, &id);

        assert_eq!(key.len(), 24, "Temporal key should be 8+16 bytes");

        // First 8 bytes are timestamp (big-endian)
        let millis_bytes: [u8; 8] = key[0..8].try_into().unwrap();
        let millis = u64::from_be_bytes(millis_bytes);
        assert_eq!(millis, timestamp.timestamp_millis() as u64);

        // Last 16 bytes are UUID
        let uuid_bytes: [u8; 16] = key[8..24].try_into().unwrap();
        assert_eq!(uuid_bytes, serialize_uuid(&id));
    }

    #[test]
    fn test_format_tag_key() {
        let id = uuid::Uuid::new_v4();
        let tag = "important";

        let key = format_tag_key(tag, &id);

        assert_eq!(key.len(), 9 + 1 + 16, "Tag key should be tag_len + 1 + 16");

        // Verify structure: tag_bytes + ':' + uuid_bytes
        assert!(key.starts_with(tag.as_bytes()));
        assert_eq!(key[9], b':');
    }

    #[test]
    fn test_format_source_key() {
        let id = uuid::Uuid::new_v4();
        let source = "web-scraper";

        let key = format_source_key(source, &id);

        assert_eq!(
            key.len(),
            11 + 1 + 16,
            "Source key should be source_len + 1 + 16"
        );

        // Verify structure: source_bytes + ':' + uuid_bytes
        assert!(key.starts_with(source.as_bytes()));
        assert_eq!(key[11], b':');
    }

    #[test]
    fn test_format_edge_key() {
        let source = uuid::Uuid::new_v4();
        let target = uuid::Uuid::new_v4();
        let edge_type = EdgeType::Causal;

        let key = format_edge_key(&source, &target, edge_type);

        assert_eq!(key.len(), 33, "Key should be 16+16+1=33 bytes");

        // First 16 bytes = source UUID
        let source_bytes: [u8; 16] = key[0..16].try_into().unwrap();
        assert_eq!(source_bytes, serialize_uuid(&source));

        // Next 16 bytes = target UUID
        let target_bytes: [u8; 16] = key[16..32].try_into().unwrap();
        assert_eq!(target_bytes, serialize_uuid(&target));

        // Last byte = edge_type
        assert_eq!(key[32], edge_type as u8);
    }

    #[test]
    fn test_format_edge_prefix() {
        let source = uuid::Uuid::new_v4();
        let prefix = format_edge_prefix(&source);

        assert_eq!(prefix.len(), 16, "Prefix should be 16 bytes");
        assert_eq!(prefix.as_slice(), serialize_uuid(&source).as_slice());
    }
}
