//! Serialization functions for graph edges.
//!
//! Provides efficient binary serialization for:
//! - `EmbedderEdge`: K-NN edges per embedder
//! - `TypedEdge`: Multi-relation typed edges
//!
//! # Format
//!
//! Uses bincode with a version prefix for future compatibility.
//! All serialization is deterministic for consistent hashing.

use super::types::{GraphEdgeStorageError, GraphEdgeStorageResult};
use context_graph_core::graph_linking::{EmbedderEdge, TypedEdge};

/// Current serialization version for graph edges.
pub const GRAPH_EDGE_VERSION: u8 = 1;

/// Serialize a vector of EmbedderEdge (K-NN neighbors for one source).
///
/// # Format
///
/// ```text
/// [version: u8][count: u32][edges: EmbedderEdge...]
/// ```
///
/// Each EmbedderEdge is serialized as:
/// ```text
/// [target_uuid: 16 bytes][similarity: f32]
/// ```
pub fn serialize_embedder_edges(edges: &[EmbedderEdge]) -> GraphEdgeStorageResult<Vec<u8>> {
    let mut buffer = Vec::with_capacity(1 + 4 + edges.len() * 20);

    // Version prefix
    buffer.push(GRAPH_EDGE_VERSION);

    // Edge count
    let count = edges.len() as u32;
    buffer.extend_from_slice(&count.to_le_bytes());

    // Each edge: target UUID (16) + similarity (4)
    for edge in edges {
        buffer.extend_from_slice(edge.target().as_bytes());
        buffer.extend_from_slice(&edge.similarity().to_le_bytes());
    }

    Ok(buffer)
}

/// Deserialize a vector of EmbedderEdge.
///
/// # Arguments
///
/// * `data` - Serialized edge data
/// * `source` - Source UUID (not stored in serialized format)
/// * `embedder_id` - Embedder ID (not stored in serialized format)
///
/// # Errors
///
/// Returns error if:
/// - Version mismatch
/// - Data truncated
/// - Invalid UUID bytes
pub fn deserialize_embedder_edges(
    data: &[u8],
    source: uuid::Uuid,
    embedder_id: u8,
) -> GraphEdgeStorageResult<Vec<EmbedderEdge>> {
    if data.is_empty() {
        return Err(GraphEdgeStorageError::deserialization(
            "deserialize_embedder_edges",
            "empty data",
        ));
    }

    // Check version
    let version = data[0];
    if version != GRAPH_EDGE_VERSION {
        return Err(GraphEdgeStorageError::deserialization(
            "deserialize_embedder_edges",
            format!(
                "version mismatch: expected {}, got {}",
                GRAPH_EDGE_VERSION, version
            ),
        ));
    }

    // Read count
    if data.len() < 5 {
        return Err(GraphEdgeStorageError::deserialization(
            "deserialize_embedder_edges",
            "data too short for count",
        ));
    }
    let count = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;

    // Verify data length
    let expected_len = 5 + count * 20;
    if data.len() < expected_len {
        return Err(GraphEdgeStorageError::deserialization(
            "deserialize_embedder_edges",
            format!(
                "data too short: expected {} bytes, got {}",
                expected_len,
                data.len()
            ),
        ));
    }

    // Parse edges
    let mut edges = Vec::with_capacity(count);
    let mut offset = 5;

    for _ in 0..count {
        // Parse target UUID
        let uuid_bytes: [u8; 16] = data[offset..offset + 16].try_into().map_err(|_| {
            GraphEdgeStorageError::deserialization(
                "deserialize_embedder_edges",
                "invalid UUID bytes",
            )
        })?;
        let target = uuid::Uuid::from_bytes(uuid_bytes);
        offset += 16;

        // Parse similarity
        let sim_bytes: [u8; 4] = data[offset..offset + 4].try_into().map_err(|_| {
            GraphEdgeStorageError::deserialization(
                "deserialize_embedder_edges",
                "invalid similarity bytes",
            )
        })?;
        let similarity = f32::from_le_bytes(sim_bytes);
        offset += 4;

        // Create edge (validation happens in EmbedderEdge::new_unchecked or we bypass it)
        // Note: We use unsafe construction here since data was validated on write
        edges.push(EmbedderEdge::from_storage(
            source,
            target,
            embedder_id,
            similarity,
        ));
    }

    Ok(edges)
}

/// Serialize a TypedEdge.
///
/// # Format
///
/// Uses bincode for TypedEdge which includes:
/// - source/target UUIDs
/// - edge type
/// - weight
/// - direction (for asymmetric edges)
/// - embedder agreement scores
pub fn serialize_typed_edge(edge: &TypedEdge) -> GraphEdgeStorageResult<Vec<u8>> {
    let mut buffer = Vec::with_capacity(128);

    // Version prefix
    buffer.push(GRAPH_EDGE_VERSION);

    // Use bincode for the complex structure
    let edge_bytes = bincode::serialize(edge).map_err(|e| {
        GraphEdgeStorageError::serialization(
            "serialize_typed_edge",
            format!("bincode error: {}", e),
        )
    })?;

    buffer.extend_from_slice(&edge_bytes);
    Ok(buffer)
}

/// Deserialize a TypedEdge.
pub fn deserialize_typed_edge(data: &[u8]) -> GraphEdgeStorageResult<TypedEdge> {
    if data.is_empty() {
        return Err(GraphEdgeStorageError::deserialization(
            "deserialize_typed_edge",
            "empty data",
        ));
    }

    // Check version
    let version = data[0];
    if version != GRAPH_EDGE_VERSION {
        return Err(GraphEdgeStorageError::deserialization(
            "deserialize_typed_edge",
            format!(
                "version mismatch: expected {}, got {}",
                GRAPH_EDGE_VERSION, version
            ),
        ));
    }

    // Deserialize with bincode
    let edge: TypedEdge = bincode::deserialize(&data[1..]).map_err(|e| {
        GraphEdgeStorageError::deserialization(
            "deserialize_typed_edge",
            format!("bincode error: {}", e),
        )
    })?;

    Ok(edge)
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::graph_linking::{DirectedRelation, GraphLinkEdgeType};
    use uuid::Uuid;

    fn create_test_embedder_edges(
        source: Uuid,
        embedder_id: u8,
        count: usize,
    ) -> Vec<EmbedderEdge> {
        (0..count)
            .map(|i| {
                let target = Uuid::new_v4();
                let similarity = 0.9 - (i as f32 * 0.05);
                // Use the from_storage constructor for tests
                EmbedderEdge::from_storage(source, target, embedder_id, similarity)
            })
            .collect()
    }

    fn default_thresholds() -> [f32; 14] {
        [0.5; 14]
    }

    #[test]
    fn test_embedder_edges_roundtrip() {
        let source = Uuid::new_v4();
        let embedder_id = 0; // E1
        let edges = create_test_embedder_edges(source, embedder_id, 5);

        let serialized = serialize_embedder_edges(&edges).unwrap();
        let deserialized = deserialize_embedder_edges(&serialized, source, embedder_id).unwrap();

        assert_eq!(edges.len(), deserialized.len());
        for (orig, deser) in edges.iter().zip(deserialized.iter()) {
            assert_eq!(orig.target(), deser.target());
            assert!((orig.similarity() - deser.similarity()).abs() < 0.0001);
        }
    }

    #[test]
    fn test_embedder_edges_empty() {
        let edges: Vec<EmbedderEdge> = vec![];
        let serialized = serialize_embedder_edges(&edges).unwrap();

        let source = Uuid::new_v4();
        let deserialized = deserialize_embedder_edges(&serialized, source, 0).unwrap();

        assert!(deserialized.is_empty());
    }

    #[test]
    fn test_embedder_edges_version_check() {
        let data = vec![99, 0, 0, 0, 0]; // Wrong version
        let result = deserialize_embedder_edges(&data, Uuid::nil(), 0);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("version mismatch"));
    }

    #[test]
    fn test_typed_edge_roundtrip() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = [0.0f32; 14];
        scores[0] = 0.85; // E1 high
        scores[6] = 0.75; // E7 high

        let thresholds = default_thresholds();
        let edge = TypedEdge::from_scores(
            source,
            target,
            scores,
            &thresholds,
            DirectedRelation::Symmetric,
        )
        .unwrap();

        let serialized = serialize_typed_edge(&edge).unwrap();
        let deserialized = deserialize_typed_edge(&serialized).unwrap();

        assert_eq!(edge.source(), deserialized.source());
        assert_eq!(edge.target(), deserialized.target());
        assert_eq!(edge.edge_type(), deserialized.edge_type());
        assert!((edge.weight() - deserialized.weight()).abs() < 0.0001);
    }

    #[test]
    fn test_typed_edge_causal() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scores = [0.0f32; 14];
        scores[4] = 0.75; // E5 Causal

        let edge = TypedEdge::new(
            source,
            target,
            GraphLinkEdgeType::CausalChain,
            0.75,
            DirectedRelation::Forward,
            scores,
            1,
            0b0000_0001_0000, // E5 only
        )
        .unwrap();

        let serialized = serialize_typed_edge(&edge).unwrap();
        let deserialized = deserialize_typed_edge(&serialized).unwrap();

        assert_eq!(edge.edge_type(), GraphLinkEdgeType::CausalChain);
        assert_eq!(edge.direction(), DirectedRelation::Forward);
        assert_eq!(deserialized.direction(), DirectedRelation::Forward);
    }

    #[test]
    fn test_typed_edge_empty_data() {
        let result = deserialize_typed_edge(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty data"));
    }
}
