//! QueryBuilder fluent API for constructing complex graph queries.
//!
//! The QueryBuilder provides a type-safe, fluent interface for building
//! graph queries that can combine semantic search, entailment checks,
//! and contradiction detection.
//!
//! # Example
//!
//! ```ignore
//! let results = QueryBuilder::semantic(&query_embedding)
//!     .with_domain(Domain::Code)
//!     .with_min_similarity(0.7)
//!     .with_top_k(50)
//!     .with_graph_context(2)
//!     .execute(&graph)
//!     .await?;
//! ```
//!
//! # Constitution Reference
//!
//! - ARCH-12: E1 is foundation - all retrieval starts with E1
//! - AP-001: Never unwrap() in prod - all errors properly typed

use uuid::Uuid;

use crate::error::{GraphError, GraphResult};
use crate::index::FaissGpuIndex;
use crate::search::Domain;
use crate::storage::edges::EdgeType;
use crate::storage::GraphStorage;

// Import wrapper functions from local modules
use super::contradiction::detect_contradictions_with_params;
use super::entailment::query_entailment_with_params;

// Import types from canonical locations
use crate::contradiction::{ContradictionParams, ContradictionResult};
use crate::entailment::{EntailmentDirection, EntailmentQueryParams, EntailmentResult};
use super::semantic::semantic_search_simple;
use super::types::{QueryMode, QueryResult, QueryStats, SemanticSearchOptions};

/// Fluent query builder for constructing complex graph queries.
///
/// Supports building queries for:
/// - Semantic search (vector similarity)
/// - Entailment queries (IS-A hierarchy)
/// - Contradiction detection
///
/// # Type Safety
///
/// The builder enforces that required parameters are set before execution.
/// Calling `execute()` on an incomplete builder will return an error.
#[derive(Debug, Clone)]
pub struct QueryBuilder {
    /// Query mode (semantic, entailment, contradiction)
    mode: QueryMode,

    /// Query embedding for semantic search
    embedding: Option<Vec<f32>>,

    /// Node ID for entailment/contradiction queries
    node_id: Option<i64>,

    /// UUID for contradiction queries
    node_uuid: Option<Uuid>,

    /// Domain filter
    domain: Option<Domain>,

    /// Edge type filters
    edge_types: Option<Vec<EdgeType>>,

    /// Minimum similarity threshold
    min_similarity: f32,

    /// Maximum results (top-k)
    top_k: usize,

    /// Whether to include graph context
    include_graph_context: bool,

    /// Graph context depth (BFS depth)
    graph_context_depth: usize,

    /// Entailment direction (for entailment queries)
    entailment_direction: EntailmentDirection,

    /// Contradiction threshold
    contradiction_threshold: f32,
}

impl Default for QueryBuilder {
    fn default() -> Self {
        Self {
            mode: QueryMode::Semantic,
            embedding: None,
            node_id: None,
            node_uuid: None,
            domain: None,
            edge_types: None,
            min_similarity: 0.0,
            top_k: 100,
            include_graph_context: false,
            graph_context_depth: 1,
            entailment_direction: EntailmentDirection::Descendants,
            contradiction_threshold: 0.5,
        }
    }
}

impl QueryBuilder {
    /// Create a new query builder for semantic search.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Query embedding vector
    ///
    /// # Example
    ///
    /// ```ignore
    /// let builder = QueryBuilder::semantic(&query_embedding);
    /// ```
    pub fn semantic(embedding: &[f32]) -> Self {
        Self {
            mode: QueryMode::Semantic,
            embedding: Some(embedding.to_vec()),
            ..Default::default()
        }
    }

    /// Create a new query builder for entailment queries.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node to query ancestors/descendants from
    /// * `direction` - Whether to find ancestors or descendants
    pub fn entailment(node_id: i64, direction: EntailmentDirection) -> Self {
        Self {
            mode: QueryMode::Entailment,
            node_id: Some(node_id),
            entailment_direction: direction,
            ..Default::default()
        }
    }

    /// Create a new query builder for contradiction detection.
    ///
    /// # Arguments
    ///
    /// * `embedding` - Embedding of the node to check
    /// * `node_uuid` - UUID of the node
    pub fn contradiction(embedding: &[f32], node_uuid: Uuid) -> Self {
        Self {
            mode: QueryMode::Contradiction,
            embedding: Some(embedding.to_vec()),
            node_uuid: Some(node_uuid),
            ..Default::default()
        }
    }

    /// Filter results by domain.
    #[must_use]
    pub fn with_domain(mut self, domain: Domain) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Filter by specific edge types (for graph traversal).
    #[must_use]
    pub fn with_edge_types(mut self, types: Vec<EdgeType>) -> Self {
        self.edge_types = Some(types);
        self
    }

    /// Set minimum similarity threshold.
    #[must_use]
    pub fn with_min_similarity(mut self, threshold: f32) -> Self {
        self.min_similarity = threshold;
        self
    }

    /// Set maximum results to return.
    #[must_use]
    pub fn with_top_k(mut self, k: usize) -> Self {
        self.top_k = k;
        self
    }

    /// Enable graph context enrichment.
    #[must_use]
    pub fn with_graph_context(mut self, depth: usize) -> Self {
        self.include_graph_context = true;
        self.graph_context_depth = depth;
        self
    }

    /// Set contradiction detection threshold.
    #[must_use]
    pub fn with_contradiction_threshold(mut self, threshold: f32) -> Self {
        self.contradiction_threshold = threshold;
        self
    }

    /// Validate the builder state.
    fn validate(&self) -> GraphResult<()> {
        match self.mode {
            QueryMode::Semantic | QueryMode::SemanticWithContext => {
                if self.embedding.is_none() {
                    return Err(GraphError::InvalidInput(
                        "Semantic query requires embedding".to_string(),
                    ));
                }
            }
            QueryMode::Entailment => {
                if self.node_id.is_none() {
                    return Err(GraphError::InvalidInput(
                        "Entailment query requires node_id".to_string(),
                    ));
                }
            }
            QueryMode::Contradiction => {
                if self.embedding.is_none() || self.node_uuid.is_none() {
                    return Err(GraphError::InvalidInput(
                        "Contradiction query requires embedding and node_uuid".to_string(),
                    ));
                }
            }
        }

        if self.top_k == 0 {
            return Err(GraphError::InvalidInput("top_k must be > 0".to_string()));
        }

        if !(0.0..=1.0).contains(&self.min_similarity) {
            return Err(GraphError::InvalidInput(format!(
                "min_similarity must be in [0.0, 1.0], got {}",
                self.min_similarity
            )));
        }

        Ok(())
    }

    /// Execute the query and return results.
    ///
    /// # Arguments
    ///
    /// * `index` - FAISS GPU index
    /// * `storage` - Graph storage
    ///
    /// # Returns
    ///
    /// `QueryResult` with matched nodes and statistics.
    ///
    /// # Errors
    ///
    /// - `GraphError::InvalidInput` if builder is incomplete
    /// - `GraphError::IndexNotTrained` if index is not trained
    pub async fn execute(
        self,
        index: &FaissGpuIndex,
        storage: &GraphStorage,
    ) -> GraphResult<QueryResult> {
        self.validate()?;

        match self.mode {
            QueryMode::Semantic | QueryMode::SemanticWithContext => {
                self.execute_semantic(index, storage).await
            }
            QueryMode::Entailment => self.execute_entailment(storage).await,
            QueryMode::Contradiction => self.execute_contradiction(index, storage).await,
        }
    }

    /// Execute semantic search.
    async fn execute_semantic(
        &self,
        index: &FaissGpuIndex,
        storage: &GraphStorage,
    ) -> GraphResult<QueryResult> {
        let embedding = self.embedding.as_ref().unwrap();

        let mut options = SemanticSearchOptions::default()
            .with_top_k(self.top_k)
            .with_min_similarity(self.min_similarity);

        if let Some(domain) = self.domain {
            options = options.with_domain(domain);
        }

        if let Some(ref types) = self.edge_types {
            options = options.with_edge_types(types.clone());
        }

        if self.include_graph_context {
            options = options.with_graph_context(self.graph_context_depth);
        }

        semantic_search_simple(index, storage, embedding, options).await
    }

    /// Execute entailment query.
    async fn execute_entailment(&self, storage: &GraphStorage) -> GraphResult<QueryResult> {
        let node_id = self.node_id.unwrap();

        let params = EntailmentQueryParams::default()
            .with_max_results(self.top_k)
            .with_min_score(self.min_similarity);

        // Note: entailment query is sync, but we keep async interface for consistency
        let results =
            query_entailment_with_params(storage, node_id, self.entailment_direction, params)?;

        // Convert entailment results to QueryResult
        Ok(convert_entailment_results(results))
    }

    /// Execute contradiction detection.
    async fn execute_contradiction(
        &self,
        index: &FaissGpuIndex,
        storage: &GraphStorage,
    ) -> GraphResult<QueryResult> {
        let embedding = self.embedding.as_ref().unwrap();
        let node_uuid = self.node_uuid.unwrap();

        let params = ContradictionParams::default()
            .threshold(self.contradiction_threshold)
            .semantic_k(self.top_k);

        // Note: contradiction detection is sync, but we keep async interface for consistency
        let results =
            detect_contradictions_with_params(index, storage, embedding, node_uuid, params)?;

        // Convert contradiction results to QueryResult
        Ok(convert_contradiction_results(results))
    }
}

/// Convert entailment results to a generic QueryResult.
fn convert_entailment_results(results: Vec<EntailmentResult>) -> QueryResult {
    use super::types::SearchResult;
    use std::time::Duration;

    let converted: Vec<SearchResult> = results
        .into_iter()
        .map(|r| {
            SearchResult::new(
                Uuid::from_u64_pair(r.node_id as u64, 0),
                r.node_id,
                r.membership_score,
                0.0, // No distance for entailment
            )
        })
        .collect();

    let count = converted.len();
    QueryResult::new(converted, count, Duration::from_micros(0)).with_stats(QueryStats::default())
}

/// Convert contradiction results to a generic QueryResult.
fn convert_contradiction_results(results: Vec<ContradictionResult>) -> QueryResult {
    use super::types::SearchResult;
    use std::time::Duration;

    let converted: Vec<SearchResult> = results
        .into_iter()
        .map(|r| {
            SearchResult::new(
                r.contradicting_node_id,
                uuid_to_i64(&r.contradicting_node_id),
                r.confidence,
                0.0, // No distance for contradiction
            )
        })
        .collect();

    let count = converted.len();
    QueryResult::new(converted, count, Duration::from_micros(0)).with_stats(QueryStats::default())
}

/// Convert UUID to i64.
#[inline]
fn uuid_to_i64(uuid: &Uuid) -> i64 {
    let bytes = uuid.as_bytes();
    i64::from_le_bytes([
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_builder_semantic() {
        let embedding = vec![0.0f32; 1536];
        let builder = QueryBuilder::semantic(&embedding)
            .with_domain(Domain::Code)
            .with_min_similarity(0.7)
            .with_top_k(50);

        assert_eq!(builder.mode, QueryMode::Semantic);
        assert_eq!(builder.domain, Some(Domain::Code));
        assert_eq!(builder.min_similarity, 0.7);
        assert_eq!(builder.top_k, 50);
    }

    #[test]
    fn test_query_builder_entailment() {
        let builder = QueryBuilder::entailment(42, EntailmentDirection::Ancestors)
            .with_top_k(100)
            .with_min_similarity(0.8);

        assert_eq!(builder.mode, QueryMode::Entailment);
        assert_eq!(builder.node_id, Some(42));
        assert_eq!(builder.entailment_direction, EntailmentDirection::Ancestors);
    }

    #[test]
    fn test_query_builder_contradiction() {
        let embedding = vec![0.0f32; 1536];
        let uuid = Uuid::new_v4();
        let builder = QueryBuilder::contradiction(&embedding, uuid)
            .with_contradiction_threshold(0.6);

        assert_eq!(builder.mode, QueryMode::Contradiction);
        assert_eq!(builder.node_uuid, Some(uuid));
        assert_eq!(builder.contradiction_threshold, 0.6);
    }

    #[test]
    fn test_query_builder_validate_semantic() {
        // Valid
        let builder = QueryBuilder::semantic(&[0.0f32; 1536]);
        assert!(builder.validate().is_ok());

        // Invalid - no embedding
        let mut builder = QueryBuilder::default();
        builder.mode = QueryMode::Semantic;
        assert!(builder.validate().is_err());
    }

    #[test]
    fn test_query_builder_validate_entailment() {
        // Valid
        let builder = QueryBuilder::entailment(42, EntailmentDirection::Ancestors);
        assert!(builder.validate().is_ok());

        // Invalid - no node_id
        let mut builder = QueryBuilder::default();
        builder.mode = QueryMode::Entailment;
        assert!(builder.validate().is_err());
    }

    #[test]
    fn test_query_builder_validate_top_k() {
        let builder = QueryBuilder::semantic(&[0.0f32; 1536]).with_top_k(0);
        assert!(builder.validate().is_err());
    }

    #[test]
    fn test_query_builder_validate_similarity() {
        let builder = QueryBuilder::semantic(&[0.0f32; 1536]).with_min_similarity(1.5);
        assert!(builder.validate().is_err());
    }

    #[test]
    fn test_query_builder_with_graph_context() {
        let builder = QueryBuilder::semantic(&[0.0f32; 1536]).with_graph_context(3);

        assert!(builder.include_graph_context);
        assert_eq!(builder.graph_context_depth, 3);
    }
}
