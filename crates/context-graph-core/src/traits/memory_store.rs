//! Memory store trait for persistent storage.

use async_trait::async_trait;

use crate::error::CoreResult;
use crate::types::{JohariQuadrant, MemoryNode, Modality, NodeId};

/// Query options for memory search.
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    /// Maximum results to return
    pub top_k: usize,
    /// Minimum similarity threshold [0.0, 1.0]
    pub min_similarity: Option<f32>,
    /// Filter by Johari quadrant
    pub johari_filter: Option<JohariQuadrant>,
    /// Filter by modality
    pub modality_filter: Option<Modality>,
    /// Include soft-deleted nodes
    pub include_deleted: bool,
}

impl SearchOptions {
    /// Create new search options with the given top_k.
    pub fn new(top_k: usize) -> Self {
        Self {
            top_k,
            ..Default::default()
        }
    }

    /// Set minimum similarity threshold.
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = Some(threshold);
        self
    }

    /// Filter by Johari quadrant.
    pub fn with_johari_filter(mut self, quadrant: JohariQuadrant) -> Self {
        self.johari_filter = Some(quadrant);
        self
    }
}

/// Persistent memory storage abstraction.
///
/// Provides CRUD operations for memory nodes with vector search capability.
///
/// # Example
///
/// ```
/// use context_graph_core::traits::SearchOptions;
/// use context_graph_core::types::JohariQuadrant;
///
/// // Create search options for memory query
/// let options = SearchOptions::new(10)
///     .with_min_similarity(0.8)
///     .with_johari_filter(JohariQuadrant::Open);
/// assert_eq!(options.top_k, 10);
/// assert_eq!(options.min_similarity, Some(0.8));
/// ```
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Store a memory node, returning its ID.
    async fn store(&self, node: MemoryNode) -> CoreResult<NodeId>;

    /// Retrieve a node by ID, returns None if not found.
    async fn retrieve(&self, id: NodeId) -> CoreResult<Option<MemoryNode>>;

    /// Search for nodes by semantic similarity.
    ///
    /// Returns nodes with their similarity scores.
    async fn search(
        &self,
        query_embedding: &[f32],
        options: SearchOptions,
    ) -> CoreResult<Vec<(MemoryNode, f32)>>;

    /// Search by text query (embedding computed internally).
    async fn search_text(
        &self,
        query: &str,
        options: SearchOptions,
    ) -> CoreResult<Vec<(MemoryNode, f32)>>;

    /// Delete a node (soft or hard delete).
    ///
    /// Returns true if the node was found and deleted.
    async fn delete(&self, id: NodeId, soft: bool) -> CoreResult<bool>;

    /// Update an existing node.
    ///
    /// Returns true if the node was found and updated.
    async fn update(&self, node: MemoryNode) -> CoreResult<bool>;

    /// Get total node count (excluding soft-deleted).
    async fn count(&self) -> CoreResult<usize>;

    /// Compact storage (remove tombstones, optimize indices).
    async fn compact(&self) -> CoreResult<()>;
}
