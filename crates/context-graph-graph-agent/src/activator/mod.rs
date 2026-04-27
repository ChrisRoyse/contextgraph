//! E8 embedder activator for graph relationships.
//!
//! This module generates asymmetric E8 embeddings (source/target) for
//! confirmed graph relationships and stores them in the graph.
//!
//! # E8 Dimension Change
//!
//! E8 uses e5-large-v2 (1024D) instead of MiniLM (384D).
//! This shares the model with E1, avoiding extra VRAM usage.
//!
//! # Asymmetric Embeddings
//!
//! Like E5 (causal), E8 produces two vectors per memory:
//! - **source_vec**: Embedding for outgoing relationships (A -> ?)
//! - **target_vec**: Embedding for incoming relationships (? -> A)

use std::sync::Arc;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use uuid::Uuid;

use context_graph_embeddings::models::GraphModel;

use crate::error::{GraphAgentError, GraphAgentResult};
use crate::types::{GraphAnalysisResult, GraphLinkDirection, RelationshipType};

// These are re-exported for tests
#[cfg(test)]
use crate::types::{ContentDomain, RelationshipCategory};

/// Configuration for the E8 activator.
#[derive(Debug, Clone)]
pub struct ActivatorConfig {
    /// Minimum confidence threshold for activation.
    pub min_confidence: f32,

    /// Whether to update existing relationships.
    pub update_existing: bool,

    /// Bidirectional confidence multiplier (default: 0.8).
    pub bidirectional_multiplier: f32,
}

impl Default for ActivatorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            update_existing: true,
            bidirectional_multiplier: 0.8,
        }
    }
}

/// Statistics for activation operations.
#[derive(Debug, Clone, Default)]
pub struct ActivationStats {
    /// Total relationships processed.
    pub processed: usize,
    /// Embeddings generated.
    pub embeddings_generated: usize,
    /// Graph edges created.
    pub edges_created: usize,
    /// Skipped due to low confidence.
    pub skipped_low_confidence: usize,
    /// Skipped due to existing relationship.
    pub skipped_existing: usize,
    /// Errors encountered.
    pub errors: usize,
}

impl ActivationStats {
    /// Reset all statistics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Stored graph edge with metadata.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Source memory ID.
    pub source_id: Uuid,
    /// Target memory ID.
    pub target_id: Uuid,
    /// Relationship type.
    pub relationship_type: RelationshipType,
    /// Confidence score.
    pub confidence: f32,
    /// LLM description of the relationship.
    pub description: String,
    /// When the edge was created.
    pub created_at: DateTime<Utc>,
}

impl GraphEdge {
    /// Create a new graph edge.
    pub fn new(
        source_id: Uuid,
        target_id: Uuid,
        relationship_type: RelationshipType,
        confidence: f32,
        description: String,
    ) -> Self {
        Self {
            source_id,
            target_id,
            relationship_type,
            confidence,
            description,
            created_at: Utc::now(),
        }
    }
}

/// Simple in-memory graph storage for discovered relationships.
///
/// In production, this would be backed by persistent storage.
pub struct GraphStorage {
    /// All edges in the graph.
    edges: Vec<GraphEdge>,
}

impl GraphStorage {
    /// Create a new empty graph storage.
    pub fn new() -> Self {
        Self { edges: Vec::new() }
    }

    /// Add an edge to the graph.
    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }

    /// Check if a direct edge exists between two nodes.
    pub fn has_edge(&self, source_id: Uuid, target_id: Uuid) -> bool {
        self.edges
            .iter()
            .any(|e| e.source_id == source_id && e.target_id == target_id)
    }

    /// Get all edges from a source node.
    pub fn edges_from(&self, source_id: Uuid) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.source_id == source_id)
            .collect()
    }

    /// Get all edges to a target node.
    pub fn edges_to(&self, target_id: Uuid) -> Vec<&GraphEdge> {
        self.edges
            .iter()
            .filter(|e| e.target_id == target_id)
            .collect()
    }

    /// Get total number of edges.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get all edges.
    pub fn all_edges(&self) -> &[GraphEdge] {
        &self.edges
    }
}

impl Default for GraphStorage {
    fn default() -> Self {
        Self::new()
    }
}

/// E8 embedder activator for storing graph relationships.
///
/// This activator:
/// 1. Validates relationship confidence
/// 2. Generates asymmetric E8 embeddings using GraphModel
/// 3. Stores edges in the graph
///
/// GraphModel is REQUIRED for embedding generation.
/// Use `with_model()` to create activator with real embedding capability.
pub struct E8Activator {
    config: ActivatorConfig,
    graph: Arc<RwLock<GraphStorage>>,
    stats: RwLock<ActivationStats>,
    /// Graph model for generating real E8 embeddings.
    /// When None, activate_relationship() will fail fast with an error.
    graph_model: Option<Arc<GraphModel>>,
}

impl E8Activator {
    /// Create a new E8 activator without a model.
    ///
    /// `activate_relationship` will fail without a GraphModel.
    /// Use `with_model()` for production use.
    pub fn new() -> Self {
        Self::with_config(ActivatorConfig::default())
    }

    /// Create with custom configuration but no model.
    ///
    /// `activate_relationship` will fail without a GraphModel.
    /// Use `with_model()` for production use.
    pub fn with_config(config: ActivatorConfig) -> Self {
        Self {
            config,
            graph: Arc::new(RwLock::new(GraphStorage::new())),
            stats: RwLock::new(ActivationStats::default()),
            graph_model: None,
        }
    }

    /// Create with shared graph storage but no model.
    ///
    /// `activate_relationship` will fail without a GraphModel.
    /// Use `with_model()` for production use.
    pub fn with_graph(graph: Arc<RwLock<GraphStorage>>) -> Self {
        Self {
            config: ActivatorConfig::default(),
            graph,
            stats: RwLock::new(ActivationStats::default()),
            graph_model: None,
        }
    }

    /// Create with a real GraphModel for production use.
    ///
    /// This is the recommended constructor for production deployments.
    /// The GraphModel generates real 1024D asymmetric embeddings.
    ///
    /// # Arguments
    /// * `graph_model` - Loaded GraphModel for E8 embeddings
    /// * `config` - Configuration options
    pub fn with_model(graph_model: Arc<GraphModel>, config: ActivatorConfig) -> Self {
        Self {
            config,
            graph: Arc::new(RwLock::new(GraphStorage::new())),
            stats: RwLock::new(ActivationStats::default()),
            graph_model: Some(graph_model),
        }
    }

    /// Create with both GraphModel and shared graph storage.
    pub fn with_model_and_graph(
        graph_model: Arc<GraphModel>,
        graph: Arc<RwLock<GraphStorage>>,
        config: ActivatorConfig,
    ) -> Self {
        Self {
            config,
            graph,
            stats: RwLock::new(ActivationStats::default()),
            graph_model: Some(graph_model),
        }
    }

    /// Activate a confirmed graph relationship.
    ///
    /// # Arguments
    /// * `source_id` - Memory A ID
    /// * `target_id` - Memory B ID
    /// * `source_content` - Memory A content (for embedding)
    /// * `target_content` - Memory B content (for embedding)
    /// * `analysis` - LLM analysis result
    ///
    /// # Returns
    /// Tuple of (source_embedding, target_embedding) - 1024D asymmetric vectors
    ///
    /// # Errors
    /// Returns error if GraphModel is not available.
    pub async fn activate_relationship(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        source_content: &str,
        target_content: &str,
        analysis: &GraphAnalysisResult,
    ) -> GraphAgentResult<(Vec<f32>, Vec<f32>)> {
        // Phase 1: Validation and graph updates (sync, hold locks briefly)
        {
            let mut stats = self.stats.write();
            stats.processed += 1;

            // Check confidence threshold
            if analysis.confidence < self.config.min_confidence {
                stats.skipped_low_confidence += 1;
                return Err(GraphAgentError::ConfigError {
                    message: format!(
                        "Confidence {} below threshold {}",
                        analysis.confidence, self.config.min_confidence
                    ),
                });
            }
        } // Release stats lock

        // Check if relationship already exists and add edges
        let edges_added = {
            let mut graph = self.graph.write();
            if !self.config.update_existing {
                let (effective_source, effective_target) =
                    self.get_effective_source_target(source_id, target_id, analysis.direction);

                if graph.has_edge(effective_source, effective_target) {
                    self.stats.write().skipped_existing += 1;
                    return Err(GraphAgentError::ConfigError {
                        message: "Relationship already exists".to_string(),
                    });
                }
            }

            // Add edges based on direction
            self.add_edges_for_direction(&mut graph, source_id, target_id, analysis)
        }; // Release graph lock

        // Update edges_created stat
        self.stats.write().edges_created += edges_added;

        // Phase 2: Generate embeddings (async, no locks held)
        // GA-H1 FIX: Embed source and target SEPARATELY for genuine asymmetric signal.
        // Previously, both were concatenated into one string, destroying directional
        // differentiation. Causal-agent correctly embeds cause/effect separately;
        // graph-agent must do the same for E8 source/target.
        let (source_embedding, _) = self.generate_e8_embeddings(source_content).await?;
        let (_, target_embedding) = self.generate_e8_embeddings(target_content).await?;

        // Phase 3: Update final stats
        self.stats.write().embeddings_generated += 2;

        Ok((source_embedding, target_embedding))
    }

    /// Generate E8 asymmetric embeddings for source and target roles.
    ///
    /// When a real GraphModel is available, uses embed_dual() for genuine asymmetric embeddings.
    /// Without a GraphModel, fails fast -- use `with_model()` to provide one.
    async fn generate_e8_embeddings(
        &self,
        content: &str,
    ) -> GraphAgentResult<(Vec<f32>, Vec<f32>)> {
        match &self.graph_model {
            Some(model) => {
                // Use real GraphModel for production embeddings
                model
                    .embed_dual(content)
                    .await
                    .map_err(|e| GraphAgentError::EmbeddingError {
                        message: format!("Failed to generate E8 embeddings: {}", e),
                    })
            }
            None => {
                // Production mode: fail fast - GraphModel is required
                Err(GraphAgentError::ConfigError {
                    message: "GraphModel required in production but not available. \
                              Use E8Activator::with_model()."
                        .to_string(),
                })
            }
        }
    }

    /// Add edges to the graph based on relationship direction.
    fn add_edges_for_direction(
        &self,
        graph: &mut GraphStorage,
        memory_a_id: Uuid,
        memory_b_id: Uuid,
        analysis: &GraphAnalysisResult,
    ) -> usize {
        let mut edges_added = 0;

        match analysis.direction {
            GraphLinkDirection::AConnectsB => {
                // A -> B
                graph.add_edge(GraphEdge::new(
                    memory_a_id,
                    memory_b_id,
                    analysis.relationship_type,
                    analysis.confidence,
                    analysis.description.clone(),
                ));
                edges_added += 1;
            }
            GraphLinkDirection::BConnectsA => {
                // B -> A
                graph.add_edge(GraphEdge::new(
                    memory_b_id,
                    memory_a_id,
                    analysis.relationship_type,
                    analysis.confidence,
                    analysis.description.clone(),
                ));
                edges_added += 1;
            }
            GraphLinkDirection::Bidirectional => {
                // A <-> B (both directions with reduced confidence)
                let reduced_confidence = analysis.confidence * self.config.bidirectional_multiplier;

                graph.add_edge(GraphEdge::new(
                    memory_a_id,
                    memory_b_id,
                    analysis.relationship_type,
                    reduced_confidence,
                    analysis.description.clone(),
                ));
                graph.add_edge(GraphEdge::new(
                    memory_b_id,
                    memory_a_id,
                    analysis.relationship_type,
                    reduced_confidence,
                    analysis.description.clone(),
                ));
                edges_added += 2;
            }
            GraphLinkDirection::NoConnection => {
                // No edges to add
            }
        }

        edges_added
    }

    /// Get effective source and target based on direction.
    fn get_effective_source_target(
        &self,
        memory_a_id: Uuid,
        memory_b_id: Uuid,
        direction: GraphLinkDirection,
    ) -> (Uuid, Uuid) {
        match direction {
            GraphLinkDirection::AConnectsB | GraphLinkDirection::Bidirectional => {
                (memory_a_id, memory_b_id)
            }
            GraphLinkDirection::BConnectsA => (memory_b_id, memory_a_id),
            GraphLinkDirection::NoConnection => (memory_a_id, memory_b_id),
        }
    }

    /// Get current statistics.
    pub fn stats(&self) -> ActivationStats {
        self.stats.read().clone()
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        self.stats.write().reset();
    }

    /// Get the graph storage.
    pub fn graph(&self) -> Arc<RwLock<GraphStorage>> {
        Arc::clone(&self.graph)
    }
}

impl Default for E8Activator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_activator_default_config() {
        let activator = E8Activator::new();
        assert!((activator.config.min_confidence - 0.6).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_activate_low_confidence() {
        let activator = E8Activator::new();
        let analysis = GraphAnalysisResult {
            has_connection: true,
            direction: GraphLinkDirection::AConnectsB,
            relationship_type: RelationshipType::Imports,
            category: RelationshipCategory::Dependency,
            domain: ContentDomain::Code,
            confidence: 0.3, // Below threshold
            description: "A imports B".to_string(),
            raw_response: None,
            llm_provenance: None,
        };

        let result = activator
            .activate_relationship(Uuid::new_v4(), Uuid::new_v4(), "a", "b", &analysis)
            .await;

        assert!(result.is_err());
        assert_eq!(activator.stats().skipped_low_confidence, 1);
    }

    // Test that activation fails fast without GraphModel
    #[tokio::test]
    async fn test_e8_activator_fails_fast_without_model() {
        let activator = E8Activator::new(); // No model
        let analysis = GraphAnalysisResult {
            has_connection: true,
            direction: GraphLinkDirection::AConnectsB,
            relationship_type: RelationshipType::Imports,
            category: RelationshipCategory::Dependency,
            domain: ContentDomain::Code,
            confidence: 0.85,
            description: "A imports B".to_string(),
            raw_response: None,
            llm_provenance: None,
        };

        let result = activator
            .activate_relationship(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "source",
                "target",
                &analysis,
            )
            .await;

        // Should fail because GraphModel is required
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("GraphModel required"),
            "Error should mention GraphModel requirement: {}",
            err
        );
    }

    #[test]
    fn test_graph_storage() {
        let mut graph = GraphStorage::new();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();

        graph.add_edge(GraphEdge::new(
            source,
            target,
            RelationshipType::Imports,
            0.9,
            "Test".to_string(),
        ));

        assert!(graph.has_edge(source, target));
        assert!(!graph.has_edge(target, source));
        assert_eq!(graph.edge_count(), 1);
    }
}
