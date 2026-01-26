//! Binary key formats for RocksDB edge storage.
//!
//! # Key Formats
//!
//! ## EdgeStorageKey (embedder_edges CF)
//! Format: [embedder_id: u8][source_uuid: 16 bytes] = 17 bytes fixed
//! - Enables prefix scan for all neighbors of a node within an embedder
//! - Sorted by embedder, then by source UUID
//!
//! ## TypedEdgeStorageKey (typed_edges CF)
//! Format: [source_uuid: 16 bytes][target_uuid: 16 bytes] = 32 bytes fixed
//! - Enables lookup of specific edge between two nodes
//! - Sorted by source, then by target
//!
//! ## TypedEdgeByTypeKey (typed_edges_by_type CF)
//! Format: [edge_type: u8][source_uuid: 16 bytes] = 17 bytes fixed
//! - Enables prefix scan for all edges of a specific type

use uuid::Uuid;

use super::{EdgeError, EdgeResult, GraphLinkEdgeType};

/// Storage key for embedder-specific K-NN edges.
///
/// Key format: [embedder_id: u8][source_uuid: 16 bytes] = 17 bytes fixed
///
/// # Examples
///
/// ```
/// use uuid::Uuid;
/// use context_graph_core::graph_linking::EdgeStorageKey;
///
/// let source = Uuid::new_v4();
/// let key = EdgeStorageKey::new(0, source); // E1 Semantic
///
/// let bytes = key.to_bytes();
/// assert_eq!(bytes.len(), 17);
///
/// let recovered = EdgeStorageKey::from_bytes(&bytes).unwrap();
/// assert_eq!(recovered.embedder_id(), 0);
/// assert_eq!(recovered.source_uuid(), source);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeStorageKey {
    embedder_id: u8,
    source_uuid: Uuid,
}

impl EdgeStorageKey {
    /// Key size in bytes: 1 (embedder) + 16 (UUID) = 17
    pub const SIZE: usize = 17;

    /// Create a new edge storage key.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Embedder index (0-12)
    /// * `source_uuid` - Source node UUID
    pub fn new(embedder_id: u8, source_uuid: Uuid) -> Self {
        Self {
            embedder_id,
            source_uuid,
        }
    }

    /// Get the embedder ID.
    #[inline]
    pub fn embedder_id(&self) -> u8 {
        self.embedder_id
    }

    /// Get the source UUID.
    #[inline]
    pub fn source_uuid(&self) -> Uuid {
        self.source_uuid
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.embedder_id;
        bytes[1..17].copy_from_slice(self.source_uuid.as_bytes());
        bytes
    }

    /// Deserialize from bytes.
    ///
    /// # Errors
    ///
    /// Returns `KeyDeserializationError` if bytes length != 17.
    pub fn from_bytes(bytes: &[u8]) -> EdgeResult<Self> {
        if bytes.len() != Self::SIZE {
            return Err(EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Expected {} bytes, got {}", Self::SIZE, bytes.len()),
            });
        }

        let embedder_id = bytes[0];
        let source_uuid = Uuid::from_slice(&bytes[1..17]).map_err(|e| {
            EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Invalid UUID: {}", e),
            }
        })?;

        Ok(Self {
            embedder_id,
            source_uuid,
        })
    }

    /// Create a prefix key for scanning all edges of an embedder.
    ///
    /// Returns a 1-byte prefix containing just the embedder ID.
    pub fn embedder_prefix(embedder_id: u8) -> [u8; 1] {
        [embedder_id]
    }
}

/// Storage key for typed multi-relation edges.
///
/// Key format: [source_uuid: 16 bytes][target_uuid: 16 bytes] = 32 bytes fixed
///
/// # Examples
///
/// ```
/// use uuid::Uuid;
/// use context_graph_core::graph_linking::TypedEdgeStorageKey;
///
/// let source = Uuid::new_v4();
/// let target = Uuid::new_v4();
/// let key = TypedEdgeStorageKey::new(source, target);
///
/// let bytes = key.to_bytes();
/// assert_eq!(bytes.len(), 32);
///
/// let recovered = TypedEdgeStorageKey::from_bytes(&bytes).unwrap();
/// assert_eq!(recovered.source_uuid(), source);
/// assert_eq!(recovered.target_uuid(), target);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypedEdgeStorageKey {
    source_uuid: Uuid,
    target_uuid: Uuid,
}

impl TypedEdgeStorageKey {
    /// Key size in bytes: 16 (source UUID) + 16 (target UUID) = 32
    pub const SIZE: usize = 32;

    /// Create a new typed edge storage key.
    pub fn new(source_uuid: Uuid, target_uuid: Uuid) -> Self {
        Self {
            source_uuid,
            target_uuid,
        }
    }

    /// Get the source UUID.
    #[inline]
    pub fn source_uuid(&self) -> Uuid {
        self.source_uuid
    }

    /// Get the target UUID.
    #[inline]
    pub fn target_uuid(&self) -> Uuid {
        self.target_uuid
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0..16].copy_from_slice(self.source_uuid.as_bytes());
        bytes[16..32].copy_from_slice(self.target_uuid.as_bytes());
        bytes
    }

    /// Deserialize from bytes.
    ///
    /// # Errors
    ///
    /// Returns `KeyDeserializationError` if bytes length != 32.
    pub fn from_bytes(bytes: &[u8]) -> EdgeResult<Self> {
        if bytes.len() != Self::SIZE {
            return Err(EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Expected {} bytes, got {}", Self::SIZE, bytes.len()),
            });
        }

        let source_uuid = Uuid::from_slice(&bytes[0..16]).map_err(|e| {
            EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Invalid source UUID: {}", e),
            }
        })?;

        let target_uuid = Uuid::from_slice(&bytes[16..32]).map_err(|e| {
            EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Invalid target UUID: {}", e),
            }
        })?;

        Ok(Self {
            source_uuid,
            target_uuid,
        })
    }

    /// Create a prefix key for scanning all edges from a source node.
    pub fn source_prefix(source_uuid: Uuid) -> [u8; 16] {
        *source_uuid.as_bytes()
    }
}

/// Storage key for typed edges indexed by type (secondary index).
///
/// Key format: [edge_type: u8][source_uuid: 16 bytes] = 17 bytes fixed
///
/// This enables efficient queries like "find all causal edges" or
/// "find all multi-agreement edges".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypedEdgeByTypeKey {
    edge_type: GraphLinkEdgeType,
    source_uuid: Uuid,
}

impl TypedEdgeByTypeKey {
    /// Key size in bytes: 1 (edge type) + 16 (UUID) = 17
    pub const SIZE: usize = 17;

    /// Create a new typed edge by type key.
    pub fn new(edge_type: GraphLinkEdgeType, source_uuid: Uuid) -> Self {
        Self {
            edge_type,
            source_uuid,
        }
    }

    /// Get the edge type.
    #[inline]
    pub fn edge_type(&self) -> GraphLinkEdgeType {
        self.edge_type
    }

    /// Get the source UUID.
    #[inline]
    pub fn source_uuid(&self) -> Uuid {
        self.source_uuid
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.edge_type.as_u8();
        bytes[1..17].copy_from_slice(self.source_uuid.as_bytes());
        bytes
    }

    /// Deserialize from bytes.
    ///
    /// # Errors
    ///
    /// Returns `KeyDeserializationError` if bytes length != 17 or edge type invalid.
    pub fn from_bytes(bytes: &[u8]) -> EdgeResult<Self> {
        if bytes.len() != Self::SIZE {
            return Err(EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Expected {} bytes, got {}", Self::SIZE, bytes.len()),
            });
        }

        let edge_type = GraphLinkEdgeType::from_u8(bytes[0]).ok_or_else(|| {
            EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Invalid edge type: {}", bytes[0]),
            }
        })?;

        let source_uuid = Uuid::from_slice(&bytes[1..17]).map_err(|e| {
            EdgeError::KeyDeserializationError {
                bytes_len: bytes.len(),
                reason: format!("Invalid UUID: {}", e),
            }
        })?;

        Ok(Self {
            edge_type,
            source_uuid,
        })
    }

    /// Create a prefix key for scanning all edges of a specific type.
    pub fn type_prefix(edge_type: GraphLinkEdgeType) -> [u8; 1] {
        [edge_type.as_u8()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== EdgeStorageKey Tests ==========

    #[test]
    fn test_edge_storage_key_size() {
        assert_eq!(EdgeStorageKey::SIZE, 17);
    }

    #[test]
    fn test_edge_storage_key_roundtrip() {
        let source = Uuid::new_v4();
        let key = EdgeStorageKey::new(5, source);

        let bytes = key.to_bytes();
        assert_eq!(bytes.len(), EdgeStorageKey::SIZE);

        let recovered = EdgeStorageKey::from_bytes(&bytes).unwrap();
        assert_eq!(recovered.embedder_id(), 5);
        assert_eq!(recovered.source_uuid(), source);
    }

    #[test]
    fn test_edge_storage_key_invalid_length() {
        let bytes = [0u8; 10]; // Too short
        let result = EdgeStorageKey::from_bytes(&bytes);
        assert!(matches!(
            result,
            Err(EdgeError::KeyDeserializationError { bytes_len: 10, .. })
        ));
    }

    #[test]
    fn test_edge_storage_key_prefix() {
        let prefix = EdgeStorageKey::embedder_prefix(4);
        assert_eq!(prefix, [4]);
    }

    #[test]
    fn test_edge_storage_key_ordering() {
        // Keys should sort by embedder first, then by UUID
        let uuid1 = Uuid::from_bytes([0; 16]);
        let uuid2 = Uuid::from_bytes([0xFF; 16]);

        let key1 = EdgeStorageKey::new(0, uuid2);
        let key2 = EdgeStorageKey::new(1, uuid1);

        // embedder 0 should come before embedder 1, regardless of UUID
        assert!(key1.to_bytes() < key2.to_bytes());
    }

    // ========== TypedEdgeStorageKey Tests ==========

    #[test]
    fn test_typed_edge_storage_key_size() {
        assert_eq!(TypedEdgeStorageKey::SIZE, 32);
    }

    #[test]
    fn test_typed_edge_storage_key_roundtrip() {
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let key = TypedEdgeStorageKey::new(source, target);

        let bytes = key.to_bytes();
        assert_eq!(bytes.len(), TypedEdgeStorageKey::SIZE);

        let recovered = TypedEdgeStorageKey::from_bytes(&bytes).unwrap();
        assert_eq!(recovered.source_uuid(), source);
        assert_eq!(recovered.target_uuid(), target);
    }

    #[test]
    fn test_typed_edge_storage_key_invalid_length() {
        let bytes = [0u8; 20]; // Wrong size
        let result = TypedEdgeStorageKey::from_bytes(&bytes);
        assert!(matches!(
            result,
            Err(EdgeError::KeyDeserializationError { bytes_len: 20, .. })
        ));
    }

    #[test]
    fn test_typed_edge_storage_key_prefix() {
        let source = Uuid::new_v4();
        let prefix = TypedEdgeStorageKey::source_prefix(source);
        assert_eq!(prefix.len(), 16);
        assert_eq!(&prefix, source.as_bytes());
    }

    // ========== TypedEdgeByTypeKey Tests ==========

    #[test]
    fn test_typed_edge_by_type_key_size() {
        assert_eq!(TypedEdgeByTypeKey::SIZE, 17);
    }

    #[test]
    fn test_typed_edge_by_type_key_roundtrip() {
        let source = Uuid::new_v4();
        let key = TypedEdgeByTypeKey::new(GraphLinkEdgeType::CausalChain, source);

        let bytes = key.to_bytes();
        assert_eq!(bytes.len(), TypedEdgeByTypeKey::SIZE);

        let recovered = TypedEdgeByTypeKey::from_bytes(&bytes).unwrap();
        assert_eq!(recovered.edge_type(), GraphLinkEdgeType::CausalChain);
        assert_eq!(recovered.source_uuid(), source);
    }

    #[test]
    fn test_typed_edge_by_type_key_invalid_type() {
        let mut bytes = [0u8; 17];
        bytes[0] = 99; // Invalid edge type
        bytes[1..17].copy_from_slice(Uuid::new_v4().as_bytes());

        let result = TypedEdgeByTypeKey::from_bytes(&bytes);
        assert!(matches!(
            result,
            Err(EdgeError::KeyDeserializationError { .. })
        ));
    }

    #[test]
    fn test_typed_edge_by_type_key_prefix() {
        let prefix = TypedEdgeByTypeKey::type_prefix(GraphLinkEdgeType::MultiAgreement);
        assert_eq!(prefix, [7]); // MultiAgreement = 7
    }

    #[test]
    fn test_typed_edge_by_type_key_ordering() {
        // Keys should sort by edge type first
        let uuid1 = Uuid::from_bytes([0xFF; 16]);
        let uuid2 = Uuid::from_bytes([0; 16]);

        let key1 = TypedEdgeByTypeKey::new(GraphLinkEdgeType::SemanticSimilar, uuid1); // type=0
        let key2 = TypedEdgeByTypeKey::new(GraphLinkEdgeType::CodeRelated, uuid2); // type=1

        // SemanticSimilar (0) should come before CodeRelated (1)
        assert!(key1.to_bytes() < key2.to_bytes());
    }
}
