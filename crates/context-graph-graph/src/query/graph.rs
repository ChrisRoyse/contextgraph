//! High-level Graph API for CRUD operations.
//!
//! Provides a unified interface for interacting with the knowledge graph,
//! combining FAISS vector storage with RocksDB persistence.
//!
//! # Architecture
//!
//! The Graph struct owns:
//! - `FaissGpuIndex`: GPU-accelerated vector similarity search
//! - `GraphStorage`: RocksDB persistence for nodes, edges, and metadata
//!
//! # Thread Safety
//!
//! Graph is thread-safe via internal synchronization:
//! - GraphStorage uses Arc<DB> (cheap clone)
//! - FaissGpuIndex has internal locking
//!
//! # Constitution Reference
//!
//! - ARCH-12: E1 is foundation - all retrieval starts with E1
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - perf.latency.faiss_1M_k100: <2ms target

use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::config::IndexConfig;
use crate::error::{GraphError, GraphResult};
use crate::index::{FaissGpuIndex, GpuResources};
use crate::search::Domain;
use crate::storage::{GraphStorage, LegacyGraphEdge, StorageConfig};

use super::builder::QueryBuilder;
use super::types::{QueryResult, SemanticSearchOptions};

/// High-level graph interface combining FAISS and RocksDB.
///
/// # Example
///
/// ```ignore
/// let graph = Graph::open("/data/graph.db")?;
///
/// // Add a node
/// let embedding = vec![0.0f32; 1536];
/// let node_id = graph.add_node(&embedding, Domain::Code)?;
///
/// // Query similar nodes
/// let results = graph
///     .query()
///     .semantic(&query_embedding)
///     .with_min_similarity(0.7)
///     .execute()
///     .await?;
/// ```
pub struct Graph {
    /// FAISS GPU index for vector similarity search.
    index: FaissGpuIndex,

    /// RocksDB storage for graph persistence.
    storage: GraphStorage,

    /// Shared GPU resources (kept alive for FAISS index).
    #[allow(dead_code)]
    gpu_resources: Arc<GpuResources>,

    /// Index configuration.
    config: IndexConfig,
}

impl Graph {
    /// Open or create a graph at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path for RocksDB storage
    ///
    /// # Errors
    ///
    /// - `GraphError::StorageOpen` if storage cannot be opened
    /// - `GraphError::GpuResourceAllocation` if GPU resources fail
    pub fn open<P: AsRef<Path>>(path: P) -> GraphResult<Self> {
        Self::open_with_config(path, IndexConfig::default(), StorageConfig::default())
    }

    /// Open with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path for storage
    /// * `index_config` - FAISS index configuration
    /// * `storage_config` - RocksDB storage configuration
    pub fn open_with_config<P: AsRef<Path>>(
        path: P,
        index_config: IndexConfig,
        storage_config: StorageConfig,
    ) -> GraphResult<Self> {
        let storage = GraphStorage::open_and_migrate(path, storage_config)?;

        let gpu_resources = Arc::new(GpuResources::new(index_config.gpu_id)?);
        let index = FaissGpuIndex::with_resources(index_config.clone(), gpu_resources.clone())?;

        Ok(Self {
            index,
            storage,
            gpu_resources,
            config: index_config,
        })
    }

    /// Get a reference to the FAISS index.
    #[inline]
    pub fn index(&self) -> &FaissGpuIndex {
        &self.index
    }

    /// Get a reference to the graph storage.
    #[inline]
    pub fn storage(&self) -> &GraphStorage {
        &self.storage
    }

    /// Get the index configuration.
    #[inline]
    pub fn config(&self) -> &IndexConfig {
        &self.config
    }

    // ========== Node Operations ==========

    /// Add a node with an embedding vector.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Vector embedding (must match index dimension)
    /// * `domain` - Node domain classification
    ///
    /// # Returns
    ///
    /// The FAISS internal ID for the new node.
    ///
    /// # Errors
    ///
    /// - `GraphError::DimensionMismatch` if embedding dimension doesn't match
    /// - `GraphError::IndexNotTrained` if index needs training
    pub fn add_node(&mut self, embedding: &[f32], _domain: Domain) -> GraphResult<i64> {
        // Validate dimension
        if embedding.len() != self.config.dimension {
            return Err(GraphError::DimensionMismatch {
                expected: self.config.dimension,
                actual: embedding.len(),
            });
        }

        // Assign FAISS ID based on current count
        let faiss_id = self.index.len() as i64;

        // Add to FAISS index with ID
        self.index.add_with_ids(embedding, &[faiss_id])?;

        Ok(faiss_id)
    }

    /// Add a node with UUID.
    ///
    /// Associates a UUID with the FAISS internal ID for tracking.
    pub fn add_node_with_uuid(
        &mut self,
        embedding: &[f32],
        domain: Domain,
        uuid: Uuid,
    ) -> GraphResult<i64> {
        let faiss_id = self.add_node(embedding, domain)?;

        // Store UUID mapping (would need faiss_ids column family)
        // For now, just return the faiss_id
        log::debug!("Added node {} with FAISS ID {}", uuid, faiss_id);

        Ok(faiss_id)
    }

    // ========== Edge Operations ==========

    /// Add an edge between two nodes.
    ///
    /// Uses the adjacency list storage for efficient graph traversal.
    ///
    /// # Arguments
    ///
    /// * `source` - Source node i64 ID
    /// * `target` - Target node i64 ID
    /// * `edge_type` - Type of relationship (as u8)
    ///
    /// # Errors
    ///
    /// - `GraphError::Storage` if storage operation fails
    pub fn add_edge(&self, source: i64, target: i64, edge_type: u8) -> GraphResult<()> {
        let edge = LegacyGraphEdge { target, edge_type };
        self.storage.add_edge(source, edge)
    }

    /// Get all edges from a node (adjacency list).
    pub fn get_adjacency(&self, source: i64) -> GraphResult<Vec<LegacyGraphEdge>> {
        self.storage.get_adjacency(source)
    }

    /// Remove an edge between two nodes.
    ///
    /// Returns true if the edge was found and removed.
    pub fn remove_edge(&self, source: i64, target: i64) -> GraphResult<bool> {
        self.storage.remove_edge(source, target)
    }

    // ========== Query Operations ==========

    /// Create a new query builder.
    ///
    /// Returns a QueryBuilder for constructing complex queries.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let results = graph
    ///     .query_semantic(&embedding)
    ///     .with_min_similarity(0.7)
    ///     .execute(&graph.index(), &graph.storage())
    ///     .await?;
    /// ```
    pub fn query_semantic(&self, embedding: &[f32]) -> QueryBuilder {
        QueryBuilder::semantic(embedding)
    }

    /// Perform a simple semantic search.
    ///
    /// Convenience method for quick searches without the builder pattern.
    pub async fn search(
        &self,
        embedding: &[f32],
        top_k: usize,
        min_similarity: f32,
    ) -> GraphResult<QueryResult> {
        let options = SemanticSearchOptions::default()
            .with_top_k(top_k)
            .with_min_similarity(min_similarity);

        super::semantic::semantic_search_simple(&self.index, &self.storage, embedding, options)
            .await
    }

    /// Search with domain filter.
    pub async fn search_in_domain(
        &self,
        embedding: &[f32],
        domain: Domain,
        top_k: usize,
    ) -> GraphResult<QueryResult> {
        let options = SemanticSearchOptions::default()
            .with_top_k(top_k)
            .with_domain(domain);

        super::semantic::semantic_search_simple(&self.index, &self.storage, embedding, options)
            .await
    }

    // ========== Statistics ==========

    /// Get the number of vectors in the index.
    #[inline]
    pub fn vector_count(&self) -> usize {
        self.index.len()
    }

    /// Check if the index is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// Check if the index is trained.
    #[inline]
    pub fn is_trained(&self) -> bool {
        self.index.is_trained()
    }

    /// Get storage statistics.
    pub fn storage_stats(&self) -> GraphResult<GraphStats> {
        Ok(GraphStats {
            vector_count: self.index.len(),
            hyperbolic_count: self.storage.hyperbolic_count()?,
            cone_count: self.storage.cone_count()?,
            edge_count: self.storage.edge_count()?,
            adjacency_count: self.storage.adjacency_count()?,
        })
    }
}

/// Statistics about the graph.
#[derive(Debug, Clone)]
pub struct GraphStats {
    /// Number of vectors in FAISS index.
    pub vector_count: usize,

    /// Number of hyperbolic points stored.
    pub hyperbolic_count: usize,

    /// Number of entailment cones stored.
    pub cone_count: usize,

    /// Number of edges stored.
    pub edge_count: usize,

    /// Number of adjacency lists.
    pub adjacency_count: usize,
}

impl std::fmt::Display for GraphStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Graph: {} vectors, {} hyperbolic, {} cones, {} edges, {} adjacency lists",
            self.vector_count,
            self.hyperbolic_count,
            self.cone_count,
            self.edge_count,
            self.adjacency_count
        )
    }
}

/// Convert UUID to i64 for RocksDB key lookup.
#[inline]
#[allow(dead_code)]
fn uuid_to_i64(uuid: &Uuid) -> i64 {
    let bytes = uuid.as_bytes();
    i64::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ])
}

impl std::fmt::Debug for Graph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Graph")
            .field("vector_count", &self.index.len())
            .field("is_trained", &self.index.is_trained())
            .field("dimension", &self.config.dimension)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_stats_display() {
        let stats = GraphStats {
            vector_count: 1000,
            hyperbolic_count: 500,
            cone_count: 500,
            edge_count: 2000,
            adjacency_count: 800,
        };

        let display = stats.to_string();
        assert!(display.contains("1000 vectors"));
        assert!(display.contains("500 hyperbolic"));
        assert!(display.contains("2000 edges"));
    }

    #[test]
    fn test_uuid_to_i64_deterministic() {
        let uuid = Uuid::from_u128(0x12345678_9abc_def0_1234_567890abcdef);
        let i64_1 = uuid_to_i64(&uuid);
        let i64_2 = uuid_to_i64(&uuid);
        assert_eq!(i64_1, i64_2);
    }

    #[test]
    fn test_uuid_to_i64_different_uuids() {
        let uuid1 = Uuid::new_v4();
        let uuid2 = Uuid::new_v4();
        // Different UUIDs should (almost certainly) produce different i64s
        // This is a probabilistic test
        if uuid1 != uuid2 {
            let i64_1 = uuid_to_i64(&uuid1);
            let i64_2 = uuid_to_i64(&uuid2);
            // Note: Could technically collide but extremely unlikely
            assert_ne!(i64_1, i64_2);
        }
    }
}
