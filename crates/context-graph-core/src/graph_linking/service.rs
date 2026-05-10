//! GraphLinkService - High-level service for graph linking operations.
//!
//! Provides a unified API for:
//! - Building K-NN graphs from fingerprints
//! - Deriving typed edges from K-NN graphs
//! - Querying neighbors and traversing the graph
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │                  GraphLinkService                         │
//! ├──────────────────────────────────────────────────────────┤
//! │  ┌────────────────┐  ┌─────────────┐  ┌──────────────┐  │
//! │  │  NnDescent     │  │ EdgeBuilder │  │  KnnGraph    │  │
//! │  │  (Algorithm)   │  │ (Typed Edges)│ │  (In-Memory) │  │
//! │  └───────┬────────┘  └──────┬──────┘  └──────┬───────┘  │
//! │          │                  │                │          │
//! │          └──────────────────┴────────────────┘          │
//! │                         │                                │
//! │              ┌──────────▼──────────┐                    │
//! │              │   EdgeRepository    │                    │
//! │              │   (RocksDB)         │                    │
//! │              └─────────────────────┘                    │
//! └──────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use uuid::Uuid;

use super::{
    DirectedRelation, EdgeBuilder, EdgeBuilderConfig, EdgeResult, GraphLinkEdgeType, KnnGraph,
    KnnGraphStats, NnDescent, NnDescentConfig, TypedEdge,
};

/// Configuration for the GraphLinkService.
#[derive(Debug, Clone)]
pub struct GraphLinkServiceConfig {
    /// NN-Descent configuration.
    pub nn_descent: NnDescentConfig,
    /// EdgeBuilder configuration.
    pub edge_builder: EdgeBuilderConfig,
    /// Which embedders to build K-NN graphs for.
    /// Default: [0, 6, 7, 9, 13] = E1, E7, E8, E10, E14.
    /// E5 is retired and E11 is disabled; sparse/late-interaction embedders
    /// use specialized indexes outside this K-NN graph path.
    pub active_embedders: Vec<u8>,
}

impl Default for GraphLinkServiceConfig {
    fn default() -> Self {
        Self {
            nn_descent: NnDescentConfig::default(),
            edge_builder: EdgeBuilderConfig::default(),
            // Build K-NN graphs for active dense semantic/code/graph embedders (not temporal).
            active_embedders: vec![0, 6, 7, 9, 13], // E1, E7, E8, E10, E14
        }
    }
}

impl GraphLinkServiceConfig {
    /// Builder pattern for custom configuration.
    pub fn with_active_embedders(mut self, embedders: Vec<u8>) -> Self {
        self.active_embedders = embedders;
        self
    }

    /// Set NN-Descent configuration.
    pub fn with_nn_descent(mut self, config: NnDescentConfig) -> Self {
        self.nn_descent = config;
        self
    }

    /// Set EdgeBuilder configuration.
    pub fn with_edge_builder(mut self, config: EdgeBuilderConfig) -> Self {
        self.edge_builder = config;
        self
    }
}

/// Result of building K-NN graphs.
#[derive(Debug, Clone)]
pub struct BuildResult {
    /// Number of nodes processed.
    pub node_count: usize,
    /// K-NN graphs built per embedder.
    pub graphs: HashMap<u8, KnnGraph>,
    /// Typed edges derived from the graphs.
    pub typed_edges: Vec<TypedEdge>,
    /// Statistics per embedder.
    pub stats: HashMap<u8, KnnGraphStats>,
}

/// Result of a neighbor query.
#[derive(Debug, Clone)]
pub struct NeighborResult {
    /// The queried node.
    pub source: Uuid,
    /// Embedder ID queried.
    pub embedder_id: u8,
    /// Neighbors found.
    pub neighbors: Vec<NeighborInfo>,
}

/// Information about a neighbor.
#[derive(Debug, Clone)]
pub struct NeighborInfo {
    /// Neighbor node ID.
    pub node_id: Uuid,
    /// Similarity score.
    pub similarity: f32,
    /// Direction (for asymmetric embedders).
    pub direction: DirectedRelation,
}

/// Result of typed edge query.
#[derive(Debug, Clone)]
pub struct TypedEdgeResult {
    /// Source node.
    pub source: Uuid,
    /// Edges from source.
    pub edges: Vec<TypedEdgeInfo>,
}

/// Information about a typed edge.
#[derive(Debug, Clone)]
pub struct TypedEdgeInfo {
    /// Target node ID.
    pub target: Uuid,
    /// Edge type.
    pub edge_type: GraphLinkEdgeType,
    /// Edge weight.
    pub weight: f32,
    /// Direction.
    pub direction: DirectedRelation,
    /// Agreeing embedders (bitmask).
    pub agreeing_embedders: u16,
}

/// Result of multi-hop traversal.
#[derive(Debug, Clone)]
pub struct TraversalResult {
    /// Starting node.
    pub start: Uuid,
    /// Paths discovered (each path is a sequence of nodes).
    pub paths: Vec<TraversalPath>,
    /// Total nodes visited.
    pub nodes_visited: usize,
}

/// A single traversal path.
#[derive(Debug, Clone)]
pub struct TraversalPath {
    /// Nodes in the path (including start).
    pub nodes: Vec<Uuid>,
    /// Edges connecting the nodes.
    pub edges: Vec<TypedEdgeInfo>,
    /// Total path weight (product or sum of edge weights).
    pub total_weight: f32,
}

/// High-level service for graph linking operations.
///
/// This service coordinates:
/// - Building K-NN graphs using NN-Descent
/// - Deriving typed edges using EdgeBuilder
/// - Querying the graph structure
///
/// # Example
///
/// ```ignore
/// use context_graph_core::graph_linking::service::{GraphLinkService, GraphLinkServiceConfig};
///
/// let config = GraphLinkServiceConfig::default();
/// let service = GraphLinkService::new(config);
///
/// // Build K-NN graphs from fingerprints
/// let result = service.build_from_fingerprints(&fingerprints, &similarity_fn)?;
///
/// // Query neighbors
/// let neighbors = service.get_neighbors(node_id, 0)?; // E1 neighbors
/// ```
pub struct GraphLinkService {
    /// Configuration.
    config: GraphLinkServiceConfig,
    /// In-memory K-NN graphs (optional caching).
    knn_graphs: HashMap<u8, KnnGraph>,
    /// Typed edges (optional caching).
    typed_edges: Vec<TypedEdge>,
}

impl GraphLinkService {
    /// Create a new GraphLinkService with the given configuration.
    pub fn new(config: GraphLinkServiceConfig) -> Self {
        Self {
            config,
            knn_graphs: HashMap::new(),
            typed_edges: Vec::new(),
        }
    }

    /// Create a GraphLinkService with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(GraphLinkServiceConfig::default())
    }

    /// Build K-NN graphs and typed edges from node data.
    ///
    /// # Arguments
    ///
    /// * `nodes` - Node IDs to include
    /// * `get_embedding` - Function to retrieve embedding for a node and embedder
    /// * `similarity` - Similarity function
    ///
    /// # Returns
    ///
    /// BuildResult containing K-NN graphs and typed edges.
    pub fn build<F, S>(
        &mut self,
        nodes: &[Uuid],
        get_embedding: F,
        similarity: S,
    ) -> EdgeResult<BuildResult>
    where
        F: Fn(Uuid, u8) -> Option<Vec<f32>>,
        S: Fn(&[f32], &[f32]) -> f32,
    {
        let mut graphs = HashMap::new();
        let mut stats = HashMap::new();

        // Build K-NN graph for each active embedder
        for &embedder_id in &self.config.active_embedders {
            let graph = if embedder_id == 4 || embedder_id == 7 {
                // Asymmetric embedders (E5, E8) - need source/target embeddings
                // For now, use symmetric approach (can be enhanced later)
                let nn = NnDescent::new(embedder_id, nodes, self.config.nn_descent.clone());
                nn.build(|id| get_embedding(id, embedder_id), &similarity)?
            } else {
                // Symmetric embedders
                let nn = NnDescent::new(embedder_id, nodes, self.config.nn_descent.clone());
                nn.build(|id| get_embedding(id, embedder_id), &similarity)?
            };

            stats.insert(embedder_id, graph.stats());
            graphs.insert(embedder_id, graph);
        }

        // Build typed edges from K-NN graphs
        let mut edge_builder = EdgeBuilder::new(self.config.edge_builder.clone());
        for graph in graphs.values() {
            edge_builder.add_knn_graph(graph.clone());
        }
        let typed_edges = edge_builder.build_typed_edges()?;

        // Cache results
        self.knn_graphs = graphs.clone();
        self.typed_edges = typed_edges.clone();

        Ok(BuildResult {
            node_count: nodes.len(),
            graphs,
            typed_edges,
            stats,
        })
    }

    /// Get neighbors for a node in a specific embedder space.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node to query
    /// * `embedder_id` - Which embedder's K-NN graph to use
    ///
    /// # Returns
    ///
    /// NeighborResult with neighbors sorted by similarity descending.
    pub fn get_neighbors(&self, node_id: Uuid, embedder_id: u8) -> Option<NeighborResult> {
        let graph = self.knn_graphs.get(&embedder_id)?;
        let edges = graph.get_neighbors_sorted(node_id);

        let neighbors = edges
            .into_iter()
            .map(|e| NeighborInfo {
                node_id: e.target(),
                similarity: e.similarity(),
                direction: e.direction(),
            })
            .collect();

        Some(NeighborResult {
            source: node_id,
            embedder_id,
            neighbors,
        })
    }

    /// Get typed edges from a node.
    ///
    /// # Arguments
    ///
    /// * `node_id` - Node to query
    /// * `edge_type` - Optional filter by edge type
    pub fn get_typed_edges(
        &self,
        node_id: Uuid,
        edge_type: Option<GraphLinkEdgeType>,
    ) -> TypedEdgeResult {
        let edges: Vec<TypedEdgeInfo> = self
            .typed_edges
            .iter()
            .filter(|e| e.source() == node_id)
            .filter(|e| edge_type.is_none_or(|t| e.edge_type() == t))
            .map(|e| TypedEdgeInfo {
                target: e.target(),
                edge_type: e.edge_type(),
                weight: e.weight(),
                direction: e.direction(),
                agreeing_embedders: e.agreeing_embedders(),
            })
            .collect();

        TypedEdgeResult {
            source: node_id,
            edges,
        }
    }

    /// Traverse the graph from a starting node.
    ///
    /// # Arguments
    ///
    /// * `start` - Starting node
    /// * `max_hops` - Maximum traversal depth
    /// * `edge_type` - Optional filter by edge type
    /// * `min_weight` - Minimum edge weight to follow
    pub fn traverse(
        &self,
        start: Uuid,
        max_hops: usize,
        edge_type: Option<GraphLinkEdgeType>,
        min_weight: f32,
    ) -> TraversalResult {
        let mut paths = Vec::new();
        let mut visited = std::collections::HashSet::new();
        visited.insert(start);

        // BFS-style traversal
        let mut current_paths: Vec<(Vec<Uuid>, Vec<TypedEdgeInfo>, f32)> =
            vec![(vec![start], vec![], 1.0)];

        for _hop in 0..max_hops {
            let mut new_paths = Vec::new();

            for (path, edges, weight) in current_paths {
                let last_node = *path.last().unwrap();
                let outgoing = self.get_typed_edges(last_node, edge_type);

                for edge in outgoing.edges {
                    if edge.weight < min_weight {
                        continue;
                    }
                    if visited.contains(&edge.target) {
                        continue;
                    }

                    let mut new_path = path.clone();
                    new_path.push(edge.target);

                    let mut new_edges = edges.clone();
                    new_edges.push(edge.clone());

                    let new_weight = weight * edge.weight;

                    visited.insert(edge.target);
                    new_paths.push((new_path, new_edges, new_weight));
                }
            }

            if new_paths.is_empty() {
                break;
            }

            // Store completed paths
            for (path, edges, weight) in &new_paths {
                paths.push(TraversalPath {
                    nodes: path.clone(),
                    edges: edges.clone(),
                    total_weight: *weight,
                });
            }

            current_paths = new_paths;
        }

        TraversalResult {
            start,
            paths,
            nodes_visited: visited.len(),
        }
    }

    /// Get statistics for all K-NN graphs.
    pub fn stats(&self) -> HashMap<u8, KnnGraphStats> {
        self.knn_graphs
            .iter()
            .map(|(id, g)| (*id, g.stats()))
            .collect()
    }

    /// Get the number of typed edges.
    pub fn typed_edge_count(&self) -> usize {
        self.typed_edges.len()
    }

    /// Clear all cached data.
    pub fn clear(&mut self) {
        self.knn_graphs.clear();
        self.typed_edges.clear();
    }

    /// Check if graphs have been built.
    pub fn is_built(&self) -> bool {
        !self.knn_graphs.is_empty()
    }

    /// Get reference to K-NN graphs.
    pub fn knn_graphs(&self) -> &HashMap<u8, KnnGraph> {
        &self.knn_graphs
    }

    /// Get reference to typed edges.
    pub fn typed_edges(&self) -> &[TypedEdge] {
        &self.typed_edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::NUM_EMBEDDERS;

    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }

    #[test]
    fn test_service_default() {
        let service = GraphLinkService::with_defaults();
        assert!(!service.is_built());
        assert_eq!(service.typed_edge_count(), 0);
    }

    #[test]
    fn test_default_active_embedders_exclude_retired_slots() {
        let config = GraphLinkServiceConfig::default();
        assert_eq!(config.active_embedders, vec![0, 6, 7, 9, 13]);
        assert!(
            !config.active_embedders.contains(&4),
            "E5 must stay retired"
        );
        assert!(
            !config.active_embedders.contains(&10),
            "E11 must stay disabled"
        );
    }

    #[test]
    fn test_build_simple() {
        let nodes: Vec<Uuid> = (0..10).map(|_| Uuid::new_v4()).collect();

        // Create embeddings for each node and embedder
        let embeddings: HashMap<(Uuid, u8), Vec<f32>> = nodes
            .iter()
            .flat_map(|id| {
                (0..NUM_EMBEDDERS).map(move |emb_id| {
                    let mut emb = vec![0.0; 32];
                    // Create clustered embeddings
                    let cluster = id.as_u128() as usize % 3;
                    emb[cluster] = 1.0;
                    emb[(cluster + 1) % 32] = 0.5;
                    ((*id, emb_id as u8), emb)
                })
            })
            .collect();

        let config = GraphLinkServiceConfig::default().with_active_embedders(vec![0]); // Only E1 for simplicity

        let mut service = GraphLinkService::new(config);
        let result = service
            .build(
                &nodes,
                |id, emb_id| embeddings.get(&(id, emb_id)).cloned(),
                cosine_similarity,
            )
            .unwrap();

        assert_eq!(result.node_count, 10);
        assert!(result.graphs.contains_key(&0)); // E1 graph built
        assert!(service.is_built());
    }

    #[test]
    fn test_get_neighbors() {
        let nodes: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        let embeddings: HashMap<(Uuid, u8), Vec<f32>> = nodes
            .iter()
            .enumerate()
            .flat_map(|(i, id)| {
                (0..NUM_EMBEDDERS).map(move |emb_id| {
                    let mut emb = vec![0.0; 16];
                    emb[i % 16] = 1.0;
                    emb[(i + 1) % 16] = 0.5;
                    ((*id, emb_id as u8), emb)
                })
            })
            .collect();

        let config = GraphLinkServiceConfig::default().with_active_embedders(vec![0]);

        let mut service = GraphLinkService::new(config);
        service
            .build(
                &nodes,
                |id, emb_id| embeddings.get(&(id, emb_id)).cloned(),
                cosine_similarity,
            )
            .unwrap();

        // Query neighbors for first node
        let result = service.get_neighbors(nodes[0], 0);
        assert!(result.is_some());

        let neighbors = result.unwrap();
        assert_eq!(neighbors.source, nodes[0]);
        assert_eq!(neighbors.embedder_id, 0);
    }

    #[test]
    fn test_stats() {
        let nodes: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        let embeddings: HashMap<(Uuid, u8), Vec<f32>> = nodes
            .iter()
            .enumerate()
            .flat_map(|(i, id)| {
                (0..NUM_EMBEDDERS).map(move |emb_id| {
                    let mut emb = vec![0.0; 16];
                    emb[i % 16] = 1.0;
                    ((*id, emb_id as u8), emb)
                })
            })
            .collect();

        let config = GraphLinkServiceConfig::default().with_active_embedders(vec![0, 6]); // E1 and E7

        let mut service = GraphLinkService::new(config);
        service
            .build(
                &nodes,
                |id, emb_id| embeddings.get(&(id, emb_id)).cloned(),
                cosine_similarity,
            )
            .unwrap();

        let stats = service.stats();
        assert!(stats.contains_key(&0));
        assert!(stats.contains_key(&6));
    }

    #[test]
    fn test_clear() {
        let mut service = GraphLinkService::with_defaults();
        // Manually add some data
        service.knn_graphs.insert(0, KnnGraph::new(0, 20));
        assert!(service.is_built());

        service.clear();
        assert!(!service.is_built());
    }
}
