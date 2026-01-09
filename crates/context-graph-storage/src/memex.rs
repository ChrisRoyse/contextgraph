//! Memex storage trait abstraction.
//!
//! The `Memex` trait defines the storage contract for `MemoryNode` and `GraphEdge`
//! persistence. Named after Vannevar Bush's conceptual "memex" memory machine
//! from his 1945 essay "As We May Think".
//!
//! # Design Philosophy
//!
//! The Memex trait enables:
//! 1. **Testing**: Easy mocking for unit tests without RocksDB
//! 2. **Flexibility**: Future backends (PostgreSQL, distributed stores)
//! 3. **Abstraction**: Higher layers depend on trait, not concrete type
//! 4. **Object Safety**: Can be used as `dyn Memex` for runtime polymorphism
//!
//! # Implementors
//!
//! - [`RocksDbMemex`](crate::RocksDbMemex): Production RocksDB implementation
//!
//! # Constitution Reference
//!
//! - SEC-06: All delete operations must be soft deletes with 30-day recovery
//! - AP-010: `store_memory` requires rationale (enforced via `NodeMetadata`)
//!
//! # Example
//!
//! ```rust
//! use context_graph_storage::{Memex, RocksDbMemex, StorageHealth};
//! use tempfile::TempDir;
//!
//! let tmp = TempDir::new().unwrap();
//! let memex = RocksDbMemex::open(tmp.path()).unwrap();
//!
//! // Use via trait for abstraction
//! fn check_storage(storage: &dyn Memex) -> bool {
//!     storage.health_check().map(|h| h.is_healthy).unwrap_or(false)
//! }
//!
//! assert!(check_storage(&memex));
//! ```

use context_graph_core::marblestone::EdgeType;
use context_graph_core::types::{EmbeddingVector, GraphEdge, JohariQuadrant, MemoryNode, NodeId};

use crate::rocksdb_backend::StorageError;

/// Storage health status and metrics.
///
/// Returned by [`Memex::health_check()`] to provide a snapshot of
/// storage system health and approximate statistics.
///
/// # Fields
///
/// All count fields are approximate and may not reflect exact values,
/// especially during concurrent operations. This is acceptable for
/// health monitoring purposes.
///
/// # Example
///
/// ```rust
/// use context_graph_storage::StorageHealth;
///
/// // Create a health status manually (e.g., from metrics)
/// let health = StorageHealth {
///     is_healthy: true,
///     node_count: 1000,
///     edge_count: 5000,
///     storage_bytes: 10 * 1024 * 1024, // 10MB
/// };
///
/// println!("Healthy: {}", health.is_healthy);
/// println!("Nodes: {}", health.node_count);
/// println!("Edges: {}", health.edge_count);
/// println!("Size: {} bytes", health.storage_bytes);
///
/// assert!(health.is_healthy);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct StorageHealth {
    /// Whether storage is operational and all components accessible.
    ///
    /// When `false`, the storage layer may be in a degraded state
    /// and operations may fail.
    pub is_healthy: bool,

    /// Approximate number of nodes stored.
    ///
    /// This is an estimate and may not reflect exact count during
    /// concurrent operations. Use for monitoring, not exact counting.
    pub node_count: u64,

    /// Approximate number of edges stored.
    ///
    /// This is an estimate and may not reflect exact count during
    /// concurrent operations.
    pub edge_count: u64,

    /// Approximate storage size in bytes.
    ///
    /// Includes all column families and indexes. May not account
    /// for all RocksDB internal overhead.
    pub storage_bytes: u64,
}

impl Default for StorageHealth {
    /// Creates a default healthy status with zero counts.
    ///
    /// # Example
    ///
    /// ```rust
    /// use context_graph_storage::StorageHealth;
    ///
    /// let health = StorageHealth::default();
    /// assert!(health.is_healthy);
    /// assert_eq!(health.node_count, 0);
    /// ```
    fn default() -> Self {
        Self {
            is_healthy: true,
            node_count: 0,
            edge_count: 0,
            storage_bytes: 0,
        }
    }
}

/// Storage abstraction trait for the Context Graph system.
///
/// The Memex trait defines the core storage contract for persisting
/// `MemoryNode` and `GraphEdge` entities. Named after Vannevar Bush's
/// conceptual memory machine, it provides a clean abstraction over
/// the underlying storage engine.
///
/// # Design Benefits
///
/// - **Testing**: Mock implementations for fast unit tests without I/O
/// - **Flexibility**: Swap backends (RocksDB, PostgreSQL, distributed)
/// - **Dependency Injection**: Higher layers depend on trait, not concrete type
/// - **Object Safety**: Can be used as `dyn Memex` for runtime polymorphism
///
/// # Object Safety
///
/// This trait is object-safe:
/// - All methods take `&self` (no `&mut self` required due to RocksDB's internal locking)
/// - Returns concrete types (no associated types or generics)
/// - No `Self` in return position
///
/// # Thread Safety
///
/// Implementors MUST be `Send + Sync` for cross-thread usage.
/// RocksDB internally handles concurrent access, so `&self` methods are safe.
///
/// # Example: Using via Trait Object
///
/// ```rust
/// use context_graph_storage::{Memex, RocksDbMemex, MemoryNode, JohariQuadrant};
/// use tempfile::TempDir;
///
/// fn store_and_query(storage: &dyn Memex, content: &str) -> Result<Vec<uuid::Uuid>, context_graph_storage::StorageError> {
///     // Create a normalized embedding
///     let dim = 1536;
///     let val = 1.0_f32 / (dim as f32).sqrt();
///     let embedding = vec![val; dim];
///
///     // Store a node
///     let mut node = MemoryNode::new(content.to_string(), embedding);
///     node.quadrant = JohariQuadrant::Open;
///     storage.store_node(&node)?;
///
///     // Query by quadrant
///     storage.query_by_quadrant(JohariQuadrant::Open, Some(10))
/// }
///
/// let tmp = TempDir::new().unwrap();
/// let memex = RocksDbMemex::open(tmp.path()).unwrap();
/// let ids = store_and_query(&memex, "Test content").unwrap();
/// assert_eq!(ids.len(), 1);
/// ```
pub trait Memex: Send + Sync {
    // =========================================================================
    // Node Operations
    // =========================================================================

    /// Stores a memory node to persistent storage.
    ///
    /// Validates the node before storage and writes atomically to all
    /// relevant column families (nodes, embeddings, johari index,
    /// temporal index, tags index, sources index).
    ///
    /// # Arguments
    ///
    /// * `node` - The `MemoryNode` to store. Must pass `node.validate()`.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Node stored successfully
    /// * `Err(StorageError)` - Storage failed
    ///
    /// # Errors
    ///
    /// * `StorageError::ValidationFailed` - Node failed validation
    ///   (embedding dimension, normalization, content size, etc.)
    /// * `StorageError::Serialization` - MessagePack serialization failed
    /// * `StorageError::WriteFailed` - RocksDB write operation failed
    ///
    /// # Constitution Reference
    ///
    /// - AP-010: Nodes should have `metadata.rationale` set before storage
    ///
    /// `Constraint: latency < 5ms`
    fn store_node(&self, node: &MemoryNode) -> Result<(), StorageError>;

    /// Retrieves a memory node by its unique ID.
    ///
    /// Fetches the node from the primary nodes column family and
    /// deserializes it.
    ///
    /// # Arguments
    ///
    /// * `id` - The `NodeId` (UUID) of the node to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(MemoryNode)` - The retrieved node
    /// * `Err(StorageError)` - Retrieval failed
    ///
    /// # Errors
    ///
    /// * `StorageError::NotFound` - No node with this ID exists
    /// * `StorageError::Serialization` - Deserialization failed (data corruption)
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// `Constraint: latency < 1ms`
    fn get_node(&self, id: &NodeId) -> Result<MemoryNode, StorageError>;

    /// Updates an existing memory node.
    ///
    /// Validates the updated node, then atomically updates the primary
    /// record and all affected indexes. Does NOT create if node doesn't exist.
    ///
    /// # Arguments
    ///
    /// * `node` - The updated `MemoryNode`. The `node.id` must match an existing node.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Node updated successfully
    /// * `Err(StorageError)` - Update failed
    ///
    /// # Errors
    ///
    /// * `StorageError::NotFound` - No node with this ID exists
    /// * `StorageError::ValidationFailed` - Updated node failed validation
    /// * `StorageError::Serialization` - Serialization failed
    /// * `StorageError::WriteFailed` - RocksDB write operation failed
    ///
    /// # Index Maintenance
    ///
    /// When quadrant or tags change, the implementation must:
    /// 1. Remove old index entries
    /// 2. Add new index entries
    ///
    /// `Constraint: latency < 5ms`
    fn update_node(&self, node: &MemoryNode) -> Result<(), StorageError>;

    /// Deletes a memory node.
    ///
    /// # Arguments
    ///
    /// * `id` - The `NodeId` of the node to delete
    /// * `soft_delete` - If `true`, marks as deleted for 30-day recovery (SEC-06).
    ///   If `false`, permanently removes the node.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Node deleted successfully
    /// * `Err(StorageError)` - Deletion failed
    ///
    /// # Errors
    ///
    /// * `StorageError::NotFound` - No node with this ID exists
    /// * `StorageError::WriteFailed` - RocksDB delete operation failed
    ///
    /// # Constitution Reference
    ///
    /// - SEC-06: Soft delete 30-day recovery is the default.
    ///   Only use `soft_delete=false` when explicitly user-requested.
    ///
    /// `Constraint: latency < 2ms`
    fn delete_node(&self, id: &NodeId, soft_delete: bool) -> Result<(), StorageError>;

    // =========================================================================
    // Edge Operations
    // =========================================================================

    /// Stores a graph edge to persistent storage.
    ///
    /// Edges are keyed by composite key: `source_id | target_id | edge_type`.
    /// Storing an edge with the same key overwrites the existing edge.
    ///
    /// # Arguments
    ///
    /// * `edge` - The `GraphEdge` to store
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Edge stored successfully
    /// * `Err(StorageError)` - Storage failed
    ///
    /// # Errors
    ///
    /// * `StorageError::Serialization` - Bincode serialization failed
    /// * `StorageError::WriteFailed` - RocksDB write operation failed
    ///
    /// `Constraint: latency < 2ms`
    fn store_edge(&self, edge: &GraphEdge) -> Result<(), StorageError>;

    /// Retrieves a graph edge by its composite key.
    ///
    /// The composite key is formed from source ID, target ID, and edge type.
    ///
    /// # Arguments
    ///
    /// * `source_id` - The source node's `NodeId`
    /// * `target_id` - The target node's `NodeId`
    /// * `edge_type` - The type of edge (Semantic, Temporal, Causal, etc.)
    ///
    /// # Returns
    ///
    /// * `Ok(GraphEdge)` - The retrieved edge
    /// * `Err(StorageError)` - Retrieval failed
    ///
    /// # Errors
    ///
    /// * `StorageError::NotFound` - No edge with this composite key exists
    /// * `StorageError::Serialization` - Deserialization failed
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// `Constraint: latency < 1ms`
    fn get_edge(
        &self,
        source_id: &NodeId,
        target_id: &NodeId,
        edge_type: EdgeType,
    ) -> Result<GraphEdge, StorageError>;

    /// Gets all outgoing edges from a node.
    ///
    /// Uses prefix scan on the composite key for efficient retrieval
    /// of all edges originating from the specified node.
    ///
    /// # Arguments
    ///
    /// * `source_id` - The source node's `NodeId`
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<GraphEdge>)` - All outgoing edges (may be empty)
    /// * `Err(StorageError)` - Retrieval failed
    ///
    /// # Errors
    ///
    /// * `StorageError::Serialization` - Deserialization of an edge failed
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// # Performance
    ///
    /// Efficient O(k) where k is the number of outgoing edges,
    /// using RocksDB prefix scan.
    ///
    /// `Constraint: latency < 5ms for typical node (< 100 edges)`
    fn get_edges_from(&self, source_id: &NodeId) -> Result<Vec<GraphEdge>, StorageError>;

    /// Gets all incoming edges to a node.
    ///
    /// **Note**: This requires a full scan of the edges column family
    /// as edges are keyed by source, not target. Less efficient than
    /// `get_edges_from()`.
    ///
    /// # Arguments
    ///
    /// * `target_id` - The target node's `NodeId`
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<GraphEdge>)` - All incoming edges (may be empty)
    /// * `Err(StorageError)` - Retrieval failed
    ///
    /// # Errors
    ///
    /// * `StorageError::Serialization` - Deserialization of an edge failed
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// # Performance
    ///
    /// O(n) where n is total edge count due to full scan.
    /// Consider maintaining a reverse index for large graphs.
    ///
    /// `Constraint: latency < 50ms for graphs with < 100K edges`
    fn get_edges_to(&self, target_id: &NodeId) -> Result<Vec<GraphEdge>, StorageError>;

    // =========================================================================
    // Query Operations
    // =========================================================================

    /// Queries nodes by Johari quadrant.
    ///
    /// Uses the secondary Johari index for efficient quadrant-based lookups.
    ///
    /// # Arguments
    ///
    /// * `quadrant` - The `JohariQuadrant` to query (Open, Blind, Hidden, Unknown)
    /// * `limit` - Maximum results to return. `None` returns all matching nodes.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<NodeId>)` - Node IDs in the specified quadrant
    /// * `Err(StorageError)` - Query failed
    ///
    /// # Errors
    ///
    /// * `StorageError::IndexCorrupted` - Invalid UUID in index
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// # Johari Quadrants
    ///
    /// - `Open`: High coherence, low entropy (known knowns)
    /// - `Blind`: Low coherence, high entropy (unknown knowns)
    /// - `Hidden`: Low coherence, low entropy (known unknowns)
    /// - `Unknown`: High coherence, high entropy (unknown unknowns)
    ///
    /// `Constraint: latency < 10ms for limit <= 100`
    fn query_by_quadrant(
        &self,
        quadrant: JohariQuadrant,
        limit: Option<usize>,
    ) -> Result<Vec<NodeId>, StorageError>;

    /// Queries nodes by tag (exact match).
    ///
    /// Uses the secondary tags index for efficient tag-based lookups.
    ///
    /// # Arguments
    ///
    /// * `tag` - The tag to search for (exact match, case-sensitive)
    /// * `limit` - Maximum results to return. `None` returns all matching nodes.
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<NodeId>)` - Node IDs with the specified tag
    /// * `Err(StorageError)` - Query failed
    ///
    /// # Errors
    ///
    /// * `StorageError::IndexCorrupted` - Invalid UUID in index
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// `Constraint: latency < 10ms for limit <= 100`
    fn query_by_tag(&self, tag: &str, limit: Option<usize>) -> Result<Vec<NodeId>, StorageError>;

    // =========================================================================
    // Embedding Operations
    // =========================================================================

    /// Retrieves an embedding vector by node ID.
    ///
    /// Embeddings are stored separately from nodes for efficient
    /// vector operations without loading full node data.
    ///
    /// # Arguments
    ///
    /// * `id` - The `NodeId` whose embedding to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(EmbeddingVector)` - The 1536-dimensional embedding
    /// * `Err(StorageError)` - Retrieval failed
    ///
    /// # Errors
    ///
    /// * `StorageError::NotFound` - No embedding for this node ID
    /// * `StorageError::Serialization` - Deserialization failed (corrupt data)
    /// * `StorageError::ReadFailed` - RocksDB read operation failed
    ///
    /// `Constraint: latency < 1ms`
    fn get_embedding(&self, id: &NodeId) -> Result<EmbeddingVector, StorageError>;

    // =========================================================================
    // Health & Diagnostics
    // =========================================================================

    /// Checks storage health and returns system metrics.
    ///
    /// Verifies all storage components are accessible and returns
    /// approximate statistics about stored data.
    ///
    /// # Returns
    ///
    /// * `Ok(StorageHealth)` - Health status and metrics
    /// * `Err(StorageError)` - Health check failed
    ///
    /// # Errors
    ///
    /// * `StorageError::ColumnFamilyNotFound` - A required CF is missing
    /// * `StorageError::ReadFailed` - Could not read storage metrics
    ///
    /// # Usage
    ///
    /// Call periodically for monitoring:
    /// ```
    /// use context_graph_storage::StorageHealth;
    ///
    /// // Create a health snapshot for monitoring
    /// let health = StorageHealth::default();
    /// if !health.is_healthy {
    ///     eprintln!("Storage unhealthy!");
    /// }
    /// assert!(health.is_healthy);
    /// ```
    ///
    /// `Constraint: latency < 10ms`
    fn health_check(&self) -> Result<StorageHealth, StorageError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RocksDbMemex;
    use tempfile::TempDir;

    #[test]
    fn test_trait_object_safe() {
        // Verify trait is object-safe by creating Box<dyn Memex>
        println!("=== TRAIT OBJECT SAFETY TEST ===");
        println!("BEFORE: Attempting to create Box<dyn Memex>");

        let tmp = TempDir::new().expect("create temp dir");
        let db = RocksDbMemex::open(tmp.path()).expect("open db");

        // This line compiles only if trait is object-safe
        let _boxed: Box<dyn Memex> = Box::new(db);

        println!("AFTER: Box<dyn Memex> created successfully");
        println!("RESULT: PASS - Trait is object-safe");
    }

    #[test]
    fn test_storage_health_default() {
        println!("=== STORAGE HEALTH DEFAULT TEST ===");
        let health = StorageHealth::default();

        println!("BEFORE: Creating default StorageHealth");
        println!(
            "AFTER: is_healthy={}, node_count={}, edge_count={}, storage_bytes={}",
            health.is_healthy, health.node_count, health.edge_count, health.storage_bytes
        );

        assert!(health.is_healthy);
        assert_eq!(health.node_count, 0);
        assert_eq!(health.edge_count, 0);
        assert_eq!(health.storage_bytes, 0);
        println!("RESULT: PASS - StorageHealth::default() works correctly");
    }

    #[test]
    fn test_storage_health_debug_clone() {
        println!("=== STORAGE HEALTH DEBUG/CLONE TEST ===");
        let health = StorageHealth {
            is_healthy: true,
            node_count: 100,
            edge_count: 50,
            storage_bytes: 1024,
        };

        println!("BEFORE: Creating custom StorageHealth");
        let cloned = health.clone();
        assert_eq!(health, cloned);
        println!("AFTER: Cloned successfully, equality verified");

        let debug = format!("{:?}", health);
        assert!(debug.contains("is_healthy: true"));
        assert!(debug.contains("node_count: 100"));
        println!("Debug output: {}", debug);
        println!("RESULT: PASS - Clone and Debug traits work correctly");
    }

    #[test]
    fn test_storage_health_partial_eq() {
        println!("=== STORAGE HEALTH PARTIAL EQ TEST ===");
        let h1 = StorageHealth {
            is_healthy: true,
            node_count: 10,
            edge_count: 5,
            storage_bytes: 512,
        };
        let h2 = StorageHealth {
            is_healthy: true,
            node_count: 10,
            edge_count: 5,
            storage_bytes: 512,
        };
        let h3 = StorageHealth {
            is_healthy: false,
            node_count: 10,
            edge_count: 5,
            storage_bytes: 512,
        };

        println!("BEFORE: Testing PartialEq");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        println!("AFTER: Equal instances matched, different instances differed");
        println!("RESULT: PASS - PartialEq works correctly");
    }
}
