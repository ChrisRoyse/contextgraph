//! Structural Causal Model
//!
//! TASK-CAUSAL-001: Implements SCM for representing causal relationships.
//! NO BACKWARDS COMPATIBILITY - FAIL FAST WITH ROBUST LOGGING.
//!
//! ## Overview
//!
//! A Structural Causal Model (SCM) represents causal relationships as a
//! directed graph where:
//! - Nodes represent variables/events/concepts
//! - Edges represent causal mechanisms (A causes B)
//! - Edge weights represent causal strength
//!
//! ## Features
//!
//! - Add/remove nodes and edges
//! - Query causes and effects
//! - Path finding for causal chains
//! - Domain-aware causal modeling

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Node in a causal graph.
///
/// Represents a variable, event, or concept that can be part of
/// causal relationships.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalNode {
    /// Unique identifier
    pub id: Uuid,
    /// Human-readable name
    pub name: String,
    /// Domain/category (e.g., "physics", "economics", "biology")
    pub domain: String,
}

impl CausalNode {
    /// Create a new CausalNode.
    pub fn new(name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            domain: domain.into(),
        }
    }

    /// Create with a specific UUID.
    pub fn with_id(id: Uuid, name: impl Into<String>, domain: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            domain: domain.into(),
        }
    }
}

/// Directed edge in a causal graph.
///
/// Represents a causal relationship: source causes target.
/// Enhanced with optional embedding storage for direct E5-based retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalEdge {
    /// Source node (the cause)
    pub source: Uuid,
    /// Target node (the effect)
    pub target: Uuid,
    /// Causal strength [0, 1]
    pub strength: f32,
    /// Description of the causal mechanism
    pub mechanism: String,

    // ========== EMBEDDING STORAGE (Phase 2b) ==========
    /// Cause embedding (768D E5 vector) for direct retrieval.
    /// Embedded with CausalModel.embed_as_cause().
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cause_embedding: Option<Vec<f32>>,

    /// Effect embedding (768D E5 vector) for direct retrieval.
    /// Embedded with CausalModel.embed_as_effect().
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effect_embedding: Option<Vec<f32>>,

    // ========== BIDIRECTIONAL SUPPORT ==========
    /// Whether this edge represents a bidirectional (feedback loop) relationship.
    /// When true, reverse_embeddings should also be populated.
    #[serde(default)]
    pub is_bidirectional: bool,

    /// For bidirectional edges: reverse embeddings (cause_secondary, effect_secondary).
    /// These allow searching in both directions efficiently.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reverse_embeddings: Option<(Vec<f32>, Vec<f32>)>,

    // ========== LLM PROVENANCE ==========
    /// LLM confidence score for this relationship [0, 1].
    /// Separate from strength to preserve original LLM assessment.
    #[serde(default)]
    pub llm_confidence: f32,

    /// Type of causal mechanism: "direct", "mediated", "feedback", "temporal".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mechanism_type: Option<String>,
}

impl CausalEdge {
    /// Create a new CausalEdge with minimal fields.
    pub fn new(source: Uuid, target: Uuid, strength: f32, mechanism: impl Into<String>) -> Self {
        Self {
            source,
            target,
            strength: strength.clamp(0.0, 1.0),
            mechanism: mechanism.into(),
            cause_embedding: None,
            effect_embedding: None,
            is_bidirectional: false,
            reverse_embeddings: None,
            llm_confidence: 0.0,
            mechanism_type: None,
        }
    }

    /// Create a CausalEdge with embeddings for direct retrieval.
    ///
    /// # Arguments
    /// * `source` - UUID of the cause node
    /// * `target` - UUID of the effect node
    /// * `strength` - Causal strength [0, 1]
    /// * `mechanism` - Description of the causal mechanism
    /// * `cause_embedding` - E5 cause embedding (768D)
    /// * `effect_embedding` - E5 effect embedding (768D)
    pub fn with_embeddings(
        source: Uuid,
        target: Uuid,
        strength: f32,
        mechanism: impl Into<String>,
        cause_embedding: Vec<f32>,
        effect_embedding: Vec<f32>,
    ) -> Self {
        Self {
            source,
            target,
            strength: strength.clamp(0.0, 1.0),
            mechanism: mechanism.into(),
            cause_embedding: Some(cause_embedding),
            effect_embedding: Some(effect_embedding),
            is_bidirectional: false,
            reverse_embeddings: None,
            llm_confidence: 0.0,
            mechanism_type: None,
        }
    }

    /// Create a bidirectional CausalEdge (feedback loop).
    ///
    /// # Arguments
    /// * `source` - UUID of node A
    /// * `target` - UUID of node B
    /// * `strength` - Causal strength [0, 1]
    /// * `mechanism` - Description of the feedback mechanism
    /// * `a_cause` - A embedded as cause (768D)
    /// * `b_effect` - B embedded as effect (768D)
    /// * `b_cause` - B embedded as cause (768D)
    /// * `a_effect` - A embedded as effect (768D)
    pub fn bidirectional(
        source: Uuid,
        target: Uuid,
        strength: f32,
        mechanism: impl Into<String>,
        a_cause: Vec<f32>,
        b_effect: Vec<f32>,
        b_cause: Vec<f32>,
        a_effect: Vec<f32>,
    ) -> Self {
        Self {
            source,
            target,
            strength: strength.clamp(0.0, 1.0),
            mechanism: mechanism.into(),
            cause_embedding: Some(a_cause),
            effect_embedding: Some(b_effect),
            is_bidirectional: true,
            reverse_embeddings: Some((b_cause, a_effect)),
            llm_confidence: 0.0,
            mechanism_type: Some("feedback".to_string()),
        }
    }

    /// Set LLM provenance information.
    pub fn with_llm_provenance(mut self, confidence: f32, mechanism_type: Option<String>) -> Self {
        self.llm_confidence = confidence.clamp(0.0, 1.0);
        self.mechanism_type = mechanism_type;
        self
    }

    /// Check if this is a strong causal relationship.
    pub fn is_strong(&self) -> bool {
        self.strength >= 0.7
    }

    /// Check if this is a weak causal relationship.
    pub fn is_weak(&self) -> bool {
        self.strength < 0.3
    }

    /// Check if this edge has embeddings stored.
    pub fn has_embeddings(&self) -> bool {
        self.cause_embedding.is_some() && self.effect_embedding.is_some()
    }

    /// Get the embedding dimension (if embeddings are present).
    pub fn embedding_dimension(&self) -> Option<usize> {
        self.cause_embedding.as_ref().map(|e| e.len())
    }
}

/// Structural Causal Model represented as a directed graph.
///
/// The SCM stores nodes and directed edges representing causal
/// relationships between concepts.
#[derive(Debug, Clone, Default)]
pub struct CausalGraph {
    /// Nodes indexed by UUID
    nodes: HashMap<Uuid, CausalNode>,
    /// Directed edges (causal relationships)
    edges: Vec<CausalEdge>,
    /// Index: node -> outgoing edges (effects)
    effects_index: HashMap<Uuid, Vec<usize>>,
    /// Index: node -> incoming edges (causes)
    causes_index: HashMap<Uuid, Vec<usize>>,
}

impl CausalGraph {
    /// Create a new empty CausalGraph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a node to the graph.
    pub fn add_node(&mut self, node: CausalNode) {
        self.nodes.insert(node.id, node);
    }

    /// Remove a node and all its connected edges.
    pub fn remove_node(&mut self, id: Uuid) -> Option<CausalNode> {
        if let Some(node) = self.nodes.remove(&id) {
            // Remove edges involving this node
            self.edges.retain(|e| e.source != id && e.target != id);
            // Rebuild indices
            self.rebuild_indices();
            Some(node)
        } else {
            None
        }
    }

    /// Get a node by ID.
    pub fn get_node(&self, id: Uuid) -> Option<&CausalNode> {
        self.nodes.get(&id)
    }

    /// Check if a node exists.
    pub fn has_node(&self, id: Uuid) -> bool {
        self.nodes.contains_key(&id)
    }

    /// Add a directed edge (causal relationship).
    pub fn add_edge(&mut self, edge: CausalEdge) {
        let edge_idx = self.edges.len();

        // Update indices
        self.effects_index
            .entry(edge.source)
            .or_default()
            .push(edge_idx);
        self.causes_index
            .entry(edge.target)
            .or_default()
            .push(edge_idx);

        self.edges.push(edge);
    }

    /// Remove an edge between source and target.
    pub fn remove_edge(&mut self, source: Uuid, target: Uuid) -> bool {
        let initial_len = self.edges.len();
        self.edges
            .retain(|e| !(e.source == source && e.target == target));

        if self.edges.len() != initial_len {
            self.rebuild_indices();
            true
        } else {
            false
        }
    }

    /// Get all edges where this node is the cause (outgoing edges).
    pub fn get_effects(&self, cause_id: Uuid) -> Vec<&CausalEdge> {
        self.effects_index
            .get(&cause_id)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// Get all edges where this node is the effect (incoming edges).
    pub fn get_causes(&self, effect_id: Uuid) -> Vec<&CausalEdge> {
        self.causes_index
            .get(&effect_id)
            .map(|indices| indices.iter().map(|&i| &self.edges[i]).collect())
            .unwrap_or_default()
    }

    /// Get direct effect nodes (nodes directly caused by source).
    pub fn get_direct_effects(&self, source: Uuid) -> Vec<Uuid> {
        self.get_effects(source).iter().map(|e| e.target).collect()
    }

    /// Get direct cause nodes (nodes that directly cause target).
    pub fn get_direct_causes(&self, target: Uuid) -> Vec<Uuid> {
        self.get_causes(target).iter().map(|e| e.source).collect()
    }

    /// Check if there's a direct causal relationship from source to target.
    pub fn has_direct_cause(&self, source: Uuid, target: Uuid) -> bool {
        self.get_effects(source).iter().any(|e| e.target == target)
    }

    /// Get all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &CausalNode> {
        self.nodes.values()
    }

    /// Get all edges.
    pub fn edges(&self) -> impl Iterator<Item = &CausalEdge> {
        self.edges.iter()
    }

    /// Count nodes.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Count edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get nodes by domain.
    pub fn nodes_by_domain(&self, domain: &str) -> Vec<&CausalNode> {
        self.nodes.values().filter(|n| n.domain == domain).collect()
    }

    /// Get all unique domains.
    pub fn domains(&self) -> Vec<&str> {
        let mut domains: Vec<_> = self.nodes.values().map(|n| n.domain.as_str()).collect();
        domains.sort();
        domains.dedup();
        domains
    }

    /// Find causal path from source to target (BFS).
    ///
    /// Returns the path as a vector of node UUIDs, or None if no path exists.
    pub fn find_path(&self, source: Uuid, target: Uuid) -> Option<Vec<Uuid>> {
        if source == target {
            return Some(vec![source]);
        }

        use std::collections::{HashSet, VecDeque};

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // (current_node, path_so_far)
        queue.push_back((source, vec![source]));
        visited.insert(source);

        while let Some((current, path)) = queue.pop_front() {
            for effect in self.get_direct_effects(current) {
                if effect == target {
                    let mut final_path = path.clone();
                    final_path.push(target);
                    return Some(final_path);
                }

                if !visited.contains(&effect) {
                    visited.insert(effect);
                    let mut new_path = path.clone();
                    new_path.push(effect);
                    queue.push_back((effect, new_path));
                }
            }
        }

        None
    }

    /// Get average causal strength in the graph.
    pub fn average_strength(&self) -> f32 {
        if self.edges.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.edges.iter().map(|e| e.strength).sum();
        sum / self.edges.len() as f32
    }

    /// Rebuild indices after edge modification.
    fn rebuild_indices(&mut self) {
        self.effects_index.clear();
        self.causes_index.clear();

        for (idx, edge) in self.edges.iter().enumerate() {
            self.effects_index.entry(edge.source).or_default().push(idx);
            self.causes_index.entry(edge.target).or_default().push(idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> CausalGraph {
        let mut graph = CausalGraph::new();

        // Create nodes
        let a = CausalNode::new("Event A", "physics");
        let b = CausalNode::new("Event B", "physics");
        let c = CausalNode::new("Event C", "chemistry");
        let d = CausalNode::new("Event D", "chemistry");

        let a_id = a.id;
        let b_id = b.id;
        let c_id = c.id;
        let d_id = d.id;

        graph.add_node(a);
        graph.add_node(b);
        graph.add_node(c);
        graph.add_node(d);

        // Create edges: A -> B -> D, A -> C -> D
        graph.add_edge(CausalEdge::new(a_id, b_id, 0.8, "direct"));
        graph.add_edge(CausalEdge::new(b_id, d_id, 0.7, "direct"));
        graph.add_edge(CausalEdge::new(a_id, c_id, 0.6, "indirect"));
        graph.add_edge(CausalEdge::new(c_id, d_id, 0.5, "indirect"));

        graph
    }

    #[test]
    fn test_add_node() {
        let mut graph = CausalGraph::new();
        let node = CausalNode::new("Test", "domain");
        let id = node.id;
        graph.add_node(node);

        assert!(graph.has_node(id));
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn test_add_edge() {
        let mut graph = CausalGraph::new();
        let a = CausalNode::new("A", "test");
        let b = CausalNode::new("B", "test");
        let a_id = a.id;
        let b_id = b.id;

        graph.add_node(a);
        graph.add_node(b);
        graph.add_edge(CausalEdge::new(a_id, b_id, 0.9, "test"));

        assert_eq!(graph.edge_count(), 1);
        assert!(graph.has_direct_cause(a_id, b_id));
    }

    #[test]
    fn test_get_effects() {
        let graph = create_test_graph();
        let nodes: Vec<_> = graph.nodes().collect();
        let a_id = nodes.iter().find(|n| n.name == "Event A").unwrap().id;

        let effects = graph.get_effects(a_id);
        assert_eq!(effects.len(), 2); // A -> B and A -> C
    }

    #[test]
    fn test_get_causes() {
        let graph = create_test_graph();
        let nodes: Vec<_> = graph.nodes().collect();
        let d_id = nodes.iter().find(|n| n.name == "Event D").unwrap().id;

        let causes = graph.get_causes(d_id);
        assert_eq!(causes.len(), 2); // B -> D and C -> D
    }

    #[test]
    fn test_find_path() {
        let graph = create_test_graph();
        let nodes: Vec<_> = graph.nodes().collect();
        let a_id = nodes.iter().find(|n| n.name == "Event A").unwrap().id;
        let d_id = nodes.iter().find(|n| n.name == "Event D").unwrap().id;

        let path = graph.find_path(a_id, d_id);
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.len() == 3); // A -> B/C -> D
        assert_eq!(path[0], a_id);
        assert_eq!(*path.last().unwrap(), d_id);
    }

    #[test]
    fn test_domains() {
        let graph = create_test_graph();
        let domains = graph.domains();
        assert_eq!(domains.len(), 2);
        assert!(domains.contains(&"physics"));
        assert!(domains.contains(&"chemistry"));
    }

    #[test]
    fn test_nodes_by_domain() {
        let graph = create_test_graph();
        let physics_nodes = graph.nodes_by_domain("physics");
        assert_eq!(physics_nodes.len(), 2);
    }

    #[test]
    fn test_remove_node() {
        let mut graph = create_test_graph();
        let nodes: Vec<_> = graph.nodes().collect();
        let b_id = nodes.iter().find(|n| n.name == "Event B").unwrap().id;

        let initial_edges = graph.edge_count();
        graph.remove_node(b_id);

        assert!(!graph.has_node(b_id));
        assert!(graph.edge_count() < initial_edges);
    }

    #[test]
    fn test_average_strength() {
        let graph = create_test_graph();
        let avg = graph.average_strength();
        // (0.8 + 0.7 + 0.6 + 0.5) / 4 = 0.65
        assert!((avg - 0.65).abs() < 0.01);
    }

    #[test]
    fn test_causal_edge_strength_classification() {
        let strong = CausalEdge::new(Uuid::new_v4(), Uuid::new_v4(), 0.8, "strong");
        let weak = CausalEdge::new(Uuid::new_v4(), Uuid::new_v4(), 0.2, "weak");
        let medium = CausalEdge::new(Uuid::new_v4(), Uuid::new_v4(), 0.5, "medium");

        assert!(strong.is_strong());
        assert!(!strong.is_weak());

        assert!(weak.is_weak());
        assert!(!weak.is_strong());

        assert!(!medium.is_strong());
        assert!(!medium.is_weak());
    }
}
