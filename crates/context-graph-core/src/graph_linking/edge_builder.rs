//! EdgeBuilder service for deriving typed edges from K-NN graphs.
//!
//! The EdgeBuilder analyzes K-NN graphs from multiple embedders to create
//! typed edges based on embedder agreement patterns.
//!
//! # Algorithm
//!
//! 1. For each memory pair that appears in any K-NN graph:
//!    - Collect similarity scores from all embedders
//!    - Determine edge type based on which embedders agree
//!    - Compute weighted agreement score
//! 2. Create TypedEdge if weighted_agreement >= threshold
//!
//! # Architecture Reference
//!
//! - ARCH-09: Topic threshold: weighted_agreement >= 2.5
//! - AP-60: Temporal embedders (E2-E4) NEVER count toward edge type detection
//! - AP-77: E5 MUST NOT use symmetric cosine

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

use super::{
    DirectedRelation, EdgeResult, EdgeThresholds, GraphLinkEdgeType, KnnGraph, TypedEdge,
    WeightProjector, DEFAULT_THRESHOLDS,
};

/// Per-embedder similarity thresholds for agreement calculation.
/// Index 0-12 corresponds to embedders E1-E13.
pub const DEFAULT_EMBEDDER_THRESHOLDS: [f32; 14] = [
    0.75, // E1 Semantic
    0.70, // E2 Recency (temporal)
    0.70, // E3 Periodic (temporal)
    0.70, // E4 Sequence (temporal)
    0.60, // E5 Causal
    0.50, // E6 Sparse keywords
    0.70, // E7 Code
    0.60, // E8 Graph
    0.60, // E9 HDC
    0.70, // E10 Paraphrase
    0.65, // E11 Entity
    0.60, // E12 ColBERT
    0.50, // E13 SPLADE
    0.70, // E14 BGE-M3 Dense
];

/// Configuration for the EdgeBuilder.
pub struct EdgeBuilderConfig {
    /// Minimum weighted agreement for creating typed edges.
    /// Default: 2.5 (same as topic threshold per ARCH-09).
    pub min_weighted_agreement: f32,

    /// Per-embedder thresholds for agreement calculation (index 0-12 = E1-E13).
    pub embedder_thresholds: [f32; 14],

    /// Thresholds for typed edge creation.
    pub edge_thresholds: EdgeThresholds,

    /// Whether to include temporal embedders in agreement calculation.
    /// Per AP-60, this should be false.
    pub include_temporal: bool,

    /// Optional learned weight projection model.
    ///
    /// When set, edge weights are computed using a trained neural network
    /// instead of the constitution-based weighted agreement.
    pub learned_projection: Option<Arc<dyn WeightProjector>>,
}

impl Default for EdgeBuilderConfig {
    fn default() -> Self {
        Self {
            min_weighted_agreement: 2.5,
            embedder_thresholds: DEFAULT_EMBEDDER_THRESHOLDS,
            edge_thresholds: DEFAULT_THRESHOLDS,
            include_temporal: false, // AP-60: Never include temporal
            learned_projection: None,
        }
    }
}

impl std::fmt::Debug for EdgeBuilderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EdgeBuilderConfig")
            .field("min_weighted_agreement", &self.min_weighted_agreement)
            .field("embedder_thresholds", &self.embedder_thresholds)
            .field("edge_thresholds", &self.edge_thresholds)
            .field("include_temporal", &self.include_temporal)
            .field("learned_projection", &self.learned_projection.is_some())
            .finish()
    }
}

impl Clone for EdgeBuilderConfig {
    fn clone(&self) -> Self {
        Self {
            min_weighted_agreement: self.min_weighted_agreement,
            embedder_thresholds: self.embedder_thresholds,
            edge_thresholds: self.edge_thresholds,
            include_temporal: self.include_temporal,
            learned_projection: self.learned_projection.clone(),
        }
    }
}

impl EdgeBuilderConfig {
    /// Set minimum weighted agreement threshold.
    pub fn with_min_weighted_agreement(mut self, threshold: f32) -> Self {
        self.min_weighted_agreement = threshold;
        self
    }

    /// Set per-embedder thresholds.
    pub fn with_embedder_thresholds(mut self, thresholds: [f32; 14]) -> Self {
        self.embedder_thresholds = thresholds;
        self
    }

    /// Set edge type thresholds.
    pub fn with_edge_thresholds(mut self, thresholds: EdgeThresholds) -> Self {
        self.edge_thresholds = thresholds;
        self
    }

    /// Set learned weight projection model.
    ///
    /// When set, edge weights are computed using the trained model instead
    /// of constitution-based weighted agreement.
    pub fn with_learned_projection(mut self, projector: Arc<dyn WeightProjector>) -> Self {
        self.learned_projection = Some(projector);
        self
    }
}

/// Builder for creating typed edges from K-NN graphs.
///
/// # Example
///
/// ```ignore
/// use context_graph_core::graph_linking::edge_builder::{EdgeBuilder, EdgeBuilderConfig};
///
/// let config = EdgeBuilderConfig::default();
/// let mut builder = EdgeBuilder::new(config);
///
/// // Add K-NN graphs from each embedder
/// builder.add_knn_graph(e1_graph);  // E1 semantic
/// builder.add_knn_graph(e7_graph);  // E7 code
/// builder.add_knn_graph(e11_graph); // E11 entity
///
/// // Build typed edges
/// let typed_edges = builder.build_typed_edges()?;
/// ```
pub struct EdgeBuilder {
    /// Configuration.
    config: EdgeBuilderConfig,
    /// K-NN graphs indexed by embedder ID.
    knn_graphs: HashMap<u8, KnnGraph>,
}

impl EdgeBuilder {
    /// Create a new EdgeBuilder with the given configuration.
    pub fn new(config: EdgeBuilderConfig) -> Self {
        Self {
            config,
            knn_graphs: HashMap::new(),
        }
    }

    /// Create an EdgeBuilder with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(EdgeBuilderConfig::default())
    }

    /// Add a K-NN graph for an embedder.
    pub fn add_knn_graph(&mut self, graph: KnnGraph) {
        self.knn_graphs.insert(graph.embedder_id(), graph);
    }

    /// Build typed edges from the K-NN graphs.
    ///
    /// Analyzes all memory pairs that appear in any K-NN graph and creates
    /// TypedEdge instances based on embedder agreement patterns.
    pub fn build_typed_edges(&self) -> EdgeResult<Vec<TypedEdge>> {
        // Collect all unique memory pairs from all K-NN graphs
        let memory_pairs = self.collect_memory_pairs();

        // For each pair, compute embedder scores and create typed edge
        let mut typed_edges = Vec::new();

        for (source, target) in memory_pairs {
            if let Some(typed_edge) = self.create_typed_edge(source, target)? {
                typed_edges.push(typed_edge);
            }
        }

        Ok(typed_edges)
    }

    /// Collect all unique (source, target) pairs from K-NN graphs.
    fn collect_memory_pairs(&self) -> HashSet<(Uuid, Uuid)> {
        let mut pairs = HashSet::new();

        for graph in self.knn_graphs.values() {
            for edge in graph.edges() {
                // Always store pairs with smaller UUID first for consistency
                let (a, b) = if edge.source() < edge.target() {
                    (edge.source(), edge.target())
                } else {
                    (edge.target(), edge.source())
                };
                pairs.insert((a, b));
            }
        }

        pairs
    }

    /// Create a typed edge for a memory pair if it meets the threshold.
    fn create_typed_edge(&self, source: Uuid, target: Uuid) -> EdgeResult<Option<TypedEdge>> {
        // Collect similarity scores from all embedders
        let mut embedder_scores = [0.0f32; 14];
        let mut direction = DirectedRelation::Symmetric;
        let mut has_directed = false;

        for (embedder_id, graph) in &self.knn_graphs {
            let id = *embedder_id as usize;
            if id >= 13 {
                continue;
            }

            // Look for edge in either direction
            let neighbors = graph.get_neighbors(source);
            if let Some(edge) = neighbors.iter().find(|e| e.target() == target) {
                embedder_scores[id] = edge.similarity();

                // Track direction for asymmetric embedders (E5, E8)
                if edge.is_asymmetric() {
                    has_directed = true;
                    if edge.direction().is_forward() {
                        direction = DirectedRelation::Forward;
                    }
                }
            }

            // Also check reverse direction
            let reverse_neighbors = graph.get_neighbors(target);
            if let Some(edge) = reverse_neighbors.iter().find(|e| e.target() == source) {
                // Only update if we didn't already find it
                if embedder_scores[id] == 0.0 {
                    embedder_scores[id] = edge.similarity();
                }

                // Track reverse direction
                if edge.is_asymmetric() && edge.direction().is_forward() {
                    has_directed = true;
                    direction = DirectedRelation::Backward;
                }
            }
        }

        // Compute weighted agreement (excluding temporal per AP-60)
        let (weighted_agreement, agreement_count, agreeing_embedders) =
            self.compute_weighted_agreement(&embedder_scores);

        // Check if meets threshold
        if weighted_agreement < self.config.min_weighted_agreement {
            return Ok(None);
        }

        // Determine edge type based on which embedders agree
        let edge_type = self.determine_edge_type(&embedder_scores);

        // For non-asymmetric edge types, use Symmetric direction
        if !has_directed {
            direction = DirectedRelation::Symmetric;
        }

        // Compute weight: use learned projection if available, else fallback to normalized agreement
        let weight = if let Some(ref projector) = self.config.learned_projection {
            projector.project(&embedder_scores)
        } else {
            // Fallback: normalize weighted agreement to [0, 1]
            let max_agreement = 8.5; // 7×1.0 (semantic) + 2×0.5 (relational) + 1×0.5 (structural)
            (weighted_agreement / max_agreement).min(1.0)
        };

        let typed_edge = TypedEdge::new(
            source,
            target,
            edge_type,
            weight,
            direction,
            embedder_scores,
            agreement_count,
            agreeing_embedders,
        )?;

        Ok(Some(typed_edge))
    }

    /// Compute weighted agreement from embedder scores.
    ///
    /// Returns (weighted_agreement, agreement_count, agreeing_embedders_bitfield).
    ///
    /// Per AP-60: Temporal embedders (E2-E4) are NEVER counted.
    fn compute_weighted_agreement(&self, scores: &[f32; 14]) -> (f32, u8, u16) {
        let mut weighted_sum = 0.0f32;
        let mut count = 0u8;
        let mut agreeing = 0u16;

        // Category weights per constitution
        let category_weights = [
            1.0, // E1 Semantic
            0.0, // E2 Recency (temporal - excluded)
            0.0, // E3 Periodic (temporal - excluded)
            0.0, // E4 Sequence (temporal - excluded)
            1.0, // E5 Causal
            1.0, // E6 Sparse
            1.0, // E7 Code
            0.5, // E8 Graph (relational)
            0.5, // E9 HDC (structural)
            1.0, // E10 Paraphrase
            0.5, // E11 Entity (relational)
            1.0, // E12 ColBERT
            1.0, // E13 SPLADE
        ];

        for (i, &score) in scores.iter().enumerate() {
            if score >= self.config.embedder_thresholds[i] {
                let weight = if self.config.include_temporal {
                    1.0 // Uniform weight if temporal included
                } else {
                    category_weights[i]
                };

                if weight > 0.0 {
                    weighted_sum += weight;
                    count += 1;
                    agreeing |= 1 << i;
                }
            }
        }

        (weighted_sum, count, agreeing)
    }

    /// Determine edge type based on which embedders have high similarity.
    fn determine_edge_type(&self, scores: &[f32; 14]) -> GraphLinkEdgeType {
        let thresholds = self.config.embedder_thresholds;

        // Check for specific edge types in priority order

        // Causal chain: E5 high
        if scores[4] >= thresholds[4] {
            return GraphLinkEdgeType::CausalChain;
        }

        // Code related: E7 high
        if scores[6] >= thresholds[6] {
            return GraphLinkEdgeType::CodeRelated;
        }

        // Entity shared: E11 high
        if scores[10] >= thresholds[10] {
            return GraphLinkEdgeType::EntityShared;
        }

        // Graph connected: E8 high
        if scores[7] >= thresholds[7] {
            return GraphLinkEdgeType::GraphConnected;
        }

        // Paraphrase aligned: E10 high
        if scores[9] >= thresholds[9] {
            return GraphLinkEdgeType::ParaphraseAligned;
        }

        // Keyword overlap: E6 or E13 high
        if scores[5] >= thresholds[5] || scores[12] >= thresholds[12] {
            return GraphLinkEdgeType::KeywordOverlap;
        }

        // Note: Per AP-60, temporal embedders (E2-E4) are never used for edge type detection

        // Default to semantic similar if E1 is high
        if scores[0] >= thresholds[0] {
            return GraphLinkEdgeType::SemanticSimilar;
        }

        // Multi-agreement: multiple embedders agree but none dominant
        GraphLinkEdgeType::MultiAgreement
    }

    /// Get the number of K-NN graphs loaded.
    pub fn graph_count(&self) -> usize {
        self.knn_graphs.len()
    }

    /// Get statistics about the loaded K-NN graphs.
    pub fn stats(&self) -> EdgeBuilderStats {
        let total_nodes: usize = self.knn_graphs.values().map(|g| g.node_count()).sum();
        let total_edges: usize = self.knn_graphs.values().map(|g| g.edge_count()).sum();

        EdgeBuilderStats {
            graph_count: self.knn_graphs.len(),
            total_nodes,
            total_edges,
            embedder_ids: self.knn_graphs.keys().copied().collect(),
        }
    }
}

/// Statistics about the EdgeBuilder state.
#[derive(Debug, Clone)]
pub struct EdgeBuilderStats {
    /// Number of K-NN graphs loaded.
    pub graph_count: usize,
    /// Total nodes across all graphs.
    pub total_nodes: usize,
    /// Total edges across all graphs.
    pub total_edges: usize,
    /// Which embedder IDs have graphs.
    pub embedder_ids: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_linking::EmbedderEdge;

    fn create_test_graph(embedder_id: u8, edges: Vec<(Uuid, Uuid, f32)>) -> KnnGraph {
        let mut graph = KnnGraph::new(embedder_id, 20);
        for (source, target, similarity) in edges {
            let edge = EmbedderEdge::new(source, target, embedder_id, similarity).unwrap();
            graph.add_edge(edge);
        }
        graph
    }

    #[test]
    fn test_edge_builder_default() {
        let builder = EdgeBuilder::with_defaults();
        assert_eq!(builder.graph_count(), 0);
    }

    #[test]
    fn test_add_knn_graph() {
        let mut builder = EdgeBuilder::with_defaults();

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();

        let graph = create_test_graph(0, vec![(node1, node2, 0.85)]);
        builder.add_knn_graph(graph);

        assert_eq!(builder.graph_count(), 1);
    }

    #[test]
    fn test_collect_memory_pairs() {
        let mut builder = EdgeBuilder::with_defaults();

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();
        let node3 = Uuid::new_v4();

        // E1 graph
        builder.add_knn_graph(create_test_graph(
            0,
            vec![(node1, node2, 0.8), (node1, node3, 0.7)],
        ));

        // E7 graph with overlapping pair
        builder.add_knn_graph(create_test_graph(6, vec![(node1, node2, 0.9)]));

        let pairs = builder.collect_memory_pairs();
        assert_eq!(pairs.len(), 2); // (node1, node2) and (node1, node3)
    }

    #[test]
    fn test_compute_weighted_agreement() {
        let builder = EdgeBuilder::with_defaults();

        // E1=0.8, E7=0.75 should give agreement from both
        let mut scores = [0.0f32; 14];
        scores[0] = 0.8; // E1 semantic
        scores[6] = 0.75; // E7 code

        let (weighted, count, agreeing) = builder.compute_weighted_agreement(&scores);

        // E1 (weight 1.0) + E7 (weight 1.0) = 2.0
        assert!((weighted - 2.0).abs() < 0.01);
        assert_eq!(count, 2);
        assert!(agreeing & (1 << 0) != 0); // E1
        assert!(agreeing & (1 << 6) != 0); // E7
    }

    #[test]
    fn test_temporal_excluded() {
        let builder = EdgeBuilder::with_defaults();

        // E2=0.9 (temporal) should NOT count toward agreement
        let mut scores = [0.0f32; 14];
        scores[1] = 0.9; // E2 recency

        let (weighted, count, agreeing) = builder.compute_weighted_agreement(&scores);

        // Temporal has weight 0.0
        assert_eq!(weighted, 0.0);
        assert_eq!(count, 0);
        assert_eq!(agreeing, 0);
    }

    #[test]
    fn test_determine_edge_type_causal() {
        let builder = EdgeBuilder::with_defaults();

        let mut scores = [0.0f32; 14];
        scores[4] = 0.75; // E5 causal

        let edge_type = builder.determine_edge_type(&scores);
        assert_eq!(edge_type, GraphLinkEdgeType::CausalChain);
    }

    #[test]
    fn test_determine_edge_type_code() {
        let builder = EdgeBuilder::with_defaults();

        let mut scores = [0.0f32; 14];
        scores[6] = 0.75; // E7 code

        let edge_type = builder.determine_edge_type(&scores);
        assert_eq!(edge_type, GraphLinkEdgeType::CodeRelated);
    }

    #[test]
    fn test_determine_edge_type_semantic() {
        let builder = EdgeBuilder::with_defaults();

        let mut scores = [0.0f32; 14];
        scores[0] = 0.8; // E1 semantic

        let edge_type = builder.determine_edge_type(&scores);
        assert_eq!(edge_type, GraphLinkEdgeType::SemanticSimilar);
    }

    #[test]
    fn test_build_typed_edges_below_threshold() {
        let mut builder =
            EdgeBuilder::new(EdgeBuilderConfig::default().with_min_weighted_agreement(2.5));

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();

        // Only E1 with 0.8 - weighted agreement = 1.0 < 2.5
        builder.add_knn_graph(create_test_graph(0, vec![(node1, node2, 0.8)]));

        let edges = builder.build_typed_edges().unwrap();
        assert!(
            edges.is_empty(),
            "Edge should not be created below threshold"
        );
    }

    #[test]
    fn test_build_typed_edges_above_threshold() {
        let mut builder =
            EdgeBuilder::new(EdgeBuilderConfig::default().with_min_weighted_agreement(2.0));

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();

        // E1=0.8 + E7=0.75 - weighted agreement = 2.0 >= 2.0
        builder.add_knn_graph(create_test_graph(0, vec![(node1, node2, 0.8)]));
        builder.add_knn_graph(create_test_graph(6, vec![(node1, node2, 0.75)]));

        let edges = builder.build_typed_edges().unwrap();
        assert_eq!(edges.len(), 1);

        let edge = &edges[0];
        assert_eq!(edge.edge_type(), GraphLinkEdgeType::CodeRelated); // E7 takes priority
    }

    #[test]
    fn test_stats() {
        let mut builder = EdgeBuilder::with_defaults();

        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();

        builder.add_knn_graph(create_test_graph(0, vec![(node1, node2, 0.8)]));

        let stats = builder.stats();
        assert_eq!(stats.graph_count, 1);
        assert_eq!(stats.total_nodes, 1);
        assert_eq!(stats.total_edges, 1);
        assert!(stats.embedder_ids.contains(&0));
    }
}
