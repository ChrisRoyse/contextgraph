//! K-NN graph structure for neighbor lookup.
//!
//! The KnnGraph stores the k nearest neighbors for each node according to
//! a specific embedder's similarity metric. This is used for efficient
//! graph traversal and neighbor-based search.

use std::collections::HashMap;
use uuid::Uuid;

use super::EmbedderEdge;

/// A K-NN graph for a specific embedder.
///
/// Stores the k nearest neighbors for each node, enabling efficient
/// neighbor lookup and graph traversal.
///
/// # Examples
///
/// ```
/// use uuid::Uuid;
/// use context_graph_core::graph_linking::{KnnGraph, EmbedderEdge};
///
/// // Create a new K-NN graph for E1 (semantic)
/// let mut graph = KnnGraph::new(0, 20); // E1, k=20
///
/// let node = Uuid::new_v4();
/// let neighbor1 = Uuid::new_v4();
/// let neighbor2 = Uuid::new_v4();
///
/// // Add edges
/// let edge1 = EmbedderEdge::new(node, neighbor1, 0, 0.9).unwrap();
/// let edge2 = EmbedderEdge::new(node, neighbor2, 0, 0.8).unwrap();
///
/// graph.add_edge(edge1);
/// graph.add_edge(edge2);
///
/// // Query neighbors
/// let neighbors = graph.get_neighbors(node);
/// assert_eq!(neighbors.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct KnnGraph {
    /// Which embedder this graph is for (0-12)
    embedder_id: u8,
    /// Target number of neighbors per node
    k: usize,
    /// Node UUID -> list of edges to neighbors
    adjacency: HashMap<Uuid, Vec<EmbedderEdge>>,
    /// Total number of edges in the graph
    edge_count: usize,
}

impl KnnGraph {
    /// Create a new empty K-NN graph.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Which embedder this graph is for (0-12)
    /// * `k` - Target number of neighbors per node
    pub fn new(embedder_id: u8, k: usize) -> Self {
        Self {
            embedder_id,
            k,
            adjacency: HashMap::new(),
            edge_count: 0,
        }
    }

    /// Create a K-NN graph with estimated capacity.
    ///
    /// # Arguments
    ///
    /// * `embedder_id` - Which embedder this graph is for (0-12)
    /// * `k` - Target number of neighbors per node
    /// * `estimated_nodes` - Estimated number of nodes for pre-allocation
    pub fn with_capacity(embedder_id: u8, k: usize, estimated_nodes: usize) -> Self {
        Self {
            embedder_id,
            k,
            adjacency: HashMap::with_capacity(estimated_nodes),
            edge_count: 0,
        }
    }

    /// Get the embedder ID.
    #[inline]
    pub fn embedder_id(&self) -> u8 {
        self.embedder_id
    }

    /// Get the target k value.
    #[inline]
    pub fn k(&self) -> usize {
        self.k
    }

    /// Get the number of nodes in the graph.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.adjacency.len()
    }

    /// Get the total number of edges in the graph.
    #[inline]
    pub fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Add an edge to the graph.
    ///
    /// If the node already has k neighbors and this edge has higher similarity
    /// than the lowest, the lowest is evicted.
    pub fn add_edge(&mut self, edge: EmbedderEdge) {
        let source = edge.source();
        let neighbors = self.adjacency.entry(source).or_default();

        // Check if this target already exists
        if let Some(existing_idx) = neighbors.iter().position(|e| e.target() == edge.target()) {
            // Update if higher similarity
            if edge.similarity() > neighbors[existing_idx].similarity() {
                neighbors[existing_idx] = edge;
            }
            return;
        }

        // If under capacity, just add
        if neighbors.len() < self.k {
            neighbors.push(edge);
            self.edge_count += 1;
            return;
        }

        // Find the neighbor with lowest similarity
        let (min_idx, min_sim) = neighbors
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| a.similarity().partial_cmp(&b.similarity()).unwrap())
            .map(|(i, e)| (i, e.similarity()))
            .unwrap();

        // Replace if new edge has higher similarity
        if edge.similarity() > min_sim {
            neighbors[min_idx] = edge;
        }
    }

    /// Add multiple edges in batch.
    pub fn add_edges(&mut self, edges: impl IntoIterator<Item = EmbedderEdge>) {
        for edge in edges {
            self.add_edge(edge);
        }
    }

    /// Get the neighbors of a node.
    ///
    /// Returns an empty slice if the node is not in the graph.
    pub fn get_neighbors(&self, node: Uuid) -> &[EmbedderEdge] {
        self.adjacency.get(&node).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Get the neighbors of a node sorted by similarity (descending).
    pub fn get_neighbors_sorted(&self, node: Uuid) -> Vec<EmbedderEdge> {
        let mut neighbors: Vec<_> = self.get_neighbors(node).to_vec();
        neighbors.sort_by(|a, b| b.similarity().partial_cmp(&a.similarity()).unwrap());
        neighbors
    }

    /// Check if a node exists in the graph.
    #[inline]
    pub fn contains_node(&self, node: Uuid) -> bool {
        self.adjacency.contains_key(&node)
    }

    /// Get all nodes in the graph.
    pub fn nodes(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.adjacency.keys().copied()
    }

    /// Get all edges in the graph.
    pub fn edges(&self) -> impl Iterator<Item = &EmbedderEdge> {
        self.adjacency.values().flat_map(|v| v.iter())
    }

    /// Remove a node and all its edges.
    ///
    /// Returns the removed edges, if the node existed.
    pub fn remove_node(&mut self, node: Uuid) -> Option<Vec<EmbedderEdge>> {
        if let Some(edges) = self.adjacency.remove(&node) {
            self.edge_count -= edges.len();
            Some(edges)
        } else {
            None
        }
    }

    /// Clear all nodes and edges from the graph.
    pub fn clear(&mut self) {
        self.adjacency.clear();
        self.edge_count = 0;
    }

    /// Shrink the internal storage to fit the current data.
    pub fn shrink_to_fit(&mut self) {
        self.adjacency.shrink_to_fit();
        for neighbors in self.adjacency.values_mut() {
            neighbors.shrink_to_fit();
        }
    }

    /// Get statistics about the graph.
    pub fn stats(&self) -> KnnGraphStats {
        let neighbor_counts: Vec<usize> = self.adjacency.values().map(|v| v.len()).collect();

        let (min_neighbors, max_neighbors, avg_neighbors) = if neighbor_counts.is_empty() {
            (0, 0, 0.0)
        } else {
            let min = *neighbor_counts.iter().min().unwrap();
            let max = *neighbor_counts.iter().max().unwrap();
            let avg = neighbor_counts.iter().sum::<usize>() as f64 / neighbor_counts.len() as f64;
            (min, max, avg)
        };

        let similarities: Vec<f32> = self.edges().map(|e| e.similarity()).collect();
        let (min_sim, max_sim, avg_sim) = if similarities.is_empty() {
            (0.0, 0.0, 0.0)
        } else {
            let min = similarities.iter().cloned().fold(f32::INFINITY, f32::min);
            let max = similarities.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let avg = similarities.iter().sum::<f32>() / similarities.len() as f32;
            (min, max, avg)
        };

        KnnGraphStats {
            embedder_id: self.embedder_id,
            k: self.k,
            node_count: self.adjacency.len(),
            edge_count: self.edge_count,
            min_neighbors,
            max_neighbors,
            avg_neighbors,
            min_similarity: min_sim,
            max_similarity: max_sim,
            avg_similarity: avg_sim,
        }
    }
}

/// Statistics about a K-NN graph.
#[derive(Debug, Clone)]
pub struct KnnGraphStats {
    /// Embedder ID
    pub embedder_id: u8,
    /// Target k value
    pub k: usize,
    /// Number of nodes
    pub node_count: usize,
    /// Total number of edges
    pub edge_count: usize,
    /// Minimum neighbors per node
    pub min_neighbors: usize,
    /// Maximum neighbors per node
    pub max_neighbors: usize,
    /// Average neighbors per node
    pub avg_neighbors: f64,
    /// Minimum similarity score
    pub min_similarity: f32,
    /// Maximum similarity score
    pub max_similarity: f32,
    /// Average similarity score
    pub avg_similarity: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edge(source: Uuid, target: Uuid, similarity: f32) -> EmbedderEdge {
        EmbedderEdge::new(source, target, 0, similarity).unwrap()
    }

    #[test]
    fn test_new_graph() {
        let graph = KnnGraph::new(0, 20);
        assert_eq!(graph.embedder_id(), 0);
        assert_eq!(graph.k(), 20);
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_edge() {
        let mut graph = KnnGraph::new(0, 20);
        let node = Uuid::new_v4();
        let neighbor = Uuid::new_v4();

        let edge = make_edge(node, neighbor, 0.9);
        graph.add_edge(edge);

        assert_eq!(graph.node_count(), 1);
        assert_eq!(graph.edge_count(), 1);
        assert!(graph.contains_node(node));
    }

    #[test]
    fn test_get_neighbors() {
        let mut graph = KnnGraph::new(0, 20);
        let node = Uuid::new_v4();
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();

        graph.add_edge(make_edge(node, n1, 0.9));
        graph.add_edge(make_edge(node, n2, 0.8));

        let neighbors = graph.get_neighbors(node);
        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn test_get_neighbors_sorted() {
        let mut graph = KnnGraph::new(0, 20);
        let node = Uuid::new_v4();
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        let n3 = Uuid::new_v4();

        graph.add_edge(make_edge(node, n1, 0.5));
        graph.add_edge(make_edge(node, n2, 0.9));
        graph.add_edge(make_edge(node, n3, 0.7));

        let sorted = graph.get_neighbors_sorted(node);
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0].target(), n2); // 0.9 highest
        assert_eq!(sorted[1].target(), n3); // 0.7
        assert_eq!(sorted[2].target(), n1); // 0.5 lowest
    }

    #[test]
    fn test_eviction_at_capacity() {
        let mut graph = KnnGraph::new(0, 3); // k=3
        let node = Uuid::new_v4();
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        let n3 = Uuid::new_v4();
        let n4 = Uuid::new_v4();

        // Add 3 edges (at capacity)
        graph.add_edge(make_edge(node, n1, 0.5));
        graph.add_edge(make_edge(node, n2, 0.7));
        graph.add_edge(make_edge(node, n3, 0.6));

        assert_eq!(graph.get_neighbors(node).len(), 3);

        // Add higher similarity edge - should evict n1 (0.5)
        graph.add_edge(make_edge(node, n4, 0.8));

        let neighbors = graph.get_neighbors(node);
        assert_eq!(neighbors.len(), 3);

        // n1 should be evicted
        let targets: Vec<_> = neighbors.iter().map(|e| e.target()).collect();
        assert!(!targets.contains(&n1), "n1 should have been evicted");
        assert!(targets.contains(&n4), "n4 should be present");
    }

    #[test]
    fn test_no_eviction_for_lower_similarity() {
        let mut graph = KnnGraph::new(0, 3);
        let node = Uuid::new_v4();
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        let n3 = Uuid::new_v4();
        let n4 = Uuid::new_v4();

        // Add 3 edges with high similarity
        graph.add_edge(make_edge(node, n1, 0.9));
        graph.add_edge(make_edge(node, n2, 0.8));
        graph.add_edge(make_edge(node, n3, 0.85));

        // Try to add lower similarity edge - should NOT be added
        graph.add_edge(make_edge(node, n4, 0.7));

        let neighbors = graph.get_neighbors(node);
        assert_eq!(neighbors.len(), 3);

        let targets: Vec<_> = neighbors.iter().map(|e| e.target()).collect();
        assert!(!targets.contains(&n4), "n4 should not be added");
    }

    #[test]
    fn test_update_existing_edge() {
        let mut graph = KnnGraph::new(0, 20);
        let node = Uuid::new_v4();
        let neighbor = Uuid::new_v4();

        // Add edge with 0.5 similarity
        graph.add_edge(make_edge(node, neighbor, 0.5));
        assert_eq!(graph.get_neighbors(node)[0].similarity(), 0.5);

        // Update with higher similarity
        graph.add_edge(make_edge(node, neighbor, 0.9));
        assert_eq!(graph.get_neighbors(node)[0].similarity(), 0.9);
        assert_eq!(graph.edge_count(), 1); // Still just 1 edge

        // Try to update with lower similarity - should not change
        graph.add_edge(make_edge(node, neighbor, 0.3));
        assert_eq!(graph.get_neighbors(node)[0].similarity(), 0.9);
    }

    #[test]
    fn test_remove_node() {
        let mut graph = KnnGraph::new(0, 20);
        let node = Uuid::new_v4();
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();

        graph.add_edge(make_edge(node, n1, 0.9));
        graph.add_edge(make_edge(node, n2, 0.8));

        assert_eq!(graph.edge_count(), 2);

        let removed = graph.remove_node(node);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().len(), 2);
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_clear() {
        let mut graph = KnnGraph::new(0, 20);
        let node = Uuid::new_v4();
        let neighbor = Uuid::new_v4();

        graph.add_edge(make_edge(node, neighbor, 0.9));
        assert_eq!(graph.node_count(), 1);

        graph.clear();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_stats() {
        let mut graph = KnnGraph::new(0, 20);
        let n1 = Uuid::new_v4();
        let n2 = Uuid::new_v4();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();
        let t3 = Uuid::new_v4();

        // n1 has 2 neighbors
        graph.add_edge(make_edge(n1, t1, 0.9));
        graph.add_edge(make_edge(n1, t2, 0.7));

        // n2 has 1 neighbor
        graph.add_edge(make_edge(n2, t3, 0.8));

        let stats = graph.stats();
        assert_eq!(stats.embedder_id, 0);
        assert_eq!(stats.k, 20);
        assert_eq!(stats.node_count, 2);
        assert_eq!(stats.edge_count, 3);
        assert_eq!(stats.min_neighbors, 1);
        assert_eq!(stats.max_neighbors, 2);
        assert!((stats.avg_neighbors - 1.5).abs() < 0.001);
        assert!((stats.min_similarity - 0.7).abs() < 0.001);
        assert!((stats.max_similarity - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_empty_stats() {
        let graph = KnnGraph::new(0, 20);
        let stats = graph.stats();

        assert_eq!(stats.node_count, 0);
        assert_eq!(stats.edge_count, 0);
        assert_eq!(stats.min_neighbors, 0);
        assert_eq!(stats.max_neighbors, 0);
        assert_eq!(stats.avg_neighbors, 0.0);
    }
}
