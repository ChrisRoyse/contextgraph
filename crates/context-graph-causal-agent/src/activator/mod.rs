//! E5 Embedder Activator for confirmed causal relationships.
//!
//! Takes confirmed causal pairs from LLM analysis and:
//! 1. Generates asymmetric E5 embeddings using CausalModel
//! 2. Updates the teleological store with new embeddings
//! 3. Adds edges to the CausalGraph
//!
//! # Architecture
//!
//! The activator bridges the causal discovery pipeline to the storage layer:
//!
//! ```text
//! LLM Analysis Result
//!         │
//!         ▼
//! ┌─────────────────┐
//! │ E5 Activator    │
//! │  - Load memories│
//! │  - embed_dual() │
//! │  - Update FPs   │
//! │  - Add edges    │
//! └─────────────────┘
//!         │
//!         ▼
//! Storage Layer (TeleologicalStore + CausalGraph)
//! ```

use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use context_graph_core::causal::{CausalEdge, CausalGraph, CausalNode};
use context_graph_embeddings::models::CausalModel;

use crate::error::{CausalAgentError, CausalAgentResult};
use crate::types::{CausalAnalysisResult, CausalLinkDirection, DirectionalEmbeddings};

/// Configuration for the E5 activator.
#[derive(Debug, Clone)]
pub struct ActivatorConfig {
    /// Minimum confidence to embed a relationship.
    pub min_confidence: f32,

    /// Whether to update existing E5 embeddings (vs skip if present).
    pub update_existing: bool,

    /// Whether to add nodes to CausalGraph if not present.
    pub auto_create_nodes: bool,
}

impl Default for ActivatorConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            update_existing: true,
            auto_create_nodes: true,
        }
    }
}

/// Statistics from activation operations.
#[derive(Debug, Clone, Default)]
pub struct ActivationStats {
    /// Number of relationships processed.
    pub processed: usize,

    /// Number of E5 embeddings generated.
    pub embeddings_generated: usize,

    /// Number of graph edges created.
    pub edges_created: usize,

    /// Number of relationships skipped (low confidence).
    pub skipped_low_confidence: usize,

    /// Number of relationships skipped (already exists).
    pub skipped_existing: usize,

    /// Number of errors encountered.
    pub errors: usize,
}

/// Activator for embedding confirmed causal relationships.
///
/// Coordinates between:
/// - CausalModel (for E5 asymmetric embeddings)
/// - TeleologicalStore (for fingerprint updates)
/// - CausalGraph (for relationship storage)
pub struct E5EmbedderActivator {
    /// Configuration.
    config: ActivatorConfig,

    /// Causal graph for storing relationships.
    causal_graph: Arc<RwLock<CausalGraph>>,

    /// Real E5 CausalModel for asymmetric embeddings.
    /// When None, falls back to hash-based placeholders (testing mode).
    causal_model: Option<Arc<CausalModel>>,

    /// Statistics.
    stats: RwLock<ActivationStats>,
}

impl E5EmbedderActivator {
    /// Create a new activator with default configuration (testing mode, no real embeddings).
    pub fn new(causal_graph: Arc<RwLock<CausalGraph>>) -> Self {
        Self::with_config(causal_graph, ActivatorConfig::default())
    }

    /// Create with custom configuration (testing mode, no real embeddings).
    pub fn with_config(causal_graph: Arc<RwLock<CausalGraph>>, config: ActivatorConfig) -> Self {
        Self {
            config,
            causal_graph,
            causal_model: None,
            stats: RwLock::new(ActivationStats::default()),
        }
    }

    /// Create with a real CausalModel for production use.
    ///
    /// # Arguments
    /// * `causal_graph` - The causal graph for storing relationships
    /// * `causal_model` - The E5 CausalModel for generating real embeddings
    /// * `config` - Configuration options
    pub fn with_model(
        causal_graph: Arc<RwLock<CausalGraph>>,
        causal_model: Arc<CausalModel>,
        config: ActivatorConfig,
    ) -> Self {
        Self {
            config,
            causal_graph,
            causal_model: Some(causal_model),
            stats: RwLock::new(ActivationStats::default()),
        }
    }

    /// Activate E5 embedding for a confirmed causal relationship.
    ///
    /// # Arguments
    ///
    /// * `cause_id` - UUID of the cause memory
    /// * `effect_id` - UUID of the effect memory
    /// * `cause_content` - Text content of the cause memory
    /// * `effect_content` - Text content of the effect memory
    /// * `analysis` - LLM analysis result
    ///
    /// # Returns
    ///
    /// Tuple of (cause_embedding, effect_embedding) if successful
    pub async fn activate_relationship(
        &self,
        cause_id: Uuid,
        effect_id: Uuid,
        cause_content: &str,
        effect_content: &str,
        analysis: &CausalAnalysisResult,
    ) -> CausalAgentResult<(Vec<f32>, Vec<f32>)> {
        let mut stats = self.stats.write();
        stats.processed += 1;

        // Check confidence threshold
        if analysis.confidence < self.config.min_confidence {
            debug!(
                cause = %cause_id,
                effect = %effect_id,
                confidence = analysis.confidence,
                threshold = self.config.min_confidence,
                "Skipping low confidence relationship"
            );
            stats.skipped_low_confidence += 1;
            return Err(CausalAgentError::ConfigError {
                message: format!(
                    "Confidence {} below threshold {}",
                    analysis.confidence, self.config.min_confidence
                ),
            });
        }

        // Check if relationship already exists in graph
        {
            let graph = self.causal_graph.read();
            if graph.has_direct_cause(cause_id, effect_id) {
                if !self.config.update_existing {
                    debug!(
                        cause = %cause_id,
                        effect = %effect_id,
                        "Skipping existing relationship"
                    );
                    stats.skipped_existing += 1;
                    return Err(CausalAgentError::ConfigError {
                        message: "Relationship already exists".to_string(),
                    });
                }
            }
        }

        // Generate E5 embeddings with direction awareness and LLM guidance
        let embeddings = self
            .generate_e5_embeddings(cause_content, effect_content, &analysis.direction, analysis)
            .await?;

        // Count embeddings generated (2 for unidirectional, 4 for bidirectional)
        let emb_count = if embeddings.is_bidirectional() { 4 } else { 2 };
        stats.embeddings_generated += emb_count;

        let cause_embedding = embeddings.cause_primary.clone();
        let effect_embedding = embeddings.effect_primary.clone();

        // Add edge to causal graph
        self.add_graph_edge(cause_id, effect_id, cause_content, effect_content, analysis)?;
        stats.edges_created += 1;

        info!(
            cause = %cause_id,
            effect = %effect_id,
            confidence = analysis.confidence,
            mechanism = %analysis.mechanism,
            "Activated E5 causal relationship"
        );

        Ok((cause_embedding, effect_embedding))
    }

    /// Generate E5 asymmetric embeddings for cause and effect with direction awareness.
    ///
    /// When a real CausalModel is available, uses embed_dual() for genuine asymmetric embeddings.
    /// In test-mode, falls back to hash-based placeholders for testing without GPU.
    /// In production (without test-mode feature), fails fast if CausalModel unavailable.
    ///
    /// # Arguments
    /// * `cause_content` - Text content positioned as cause (A in A→B)
    /// * `effect_content` - Text content positioned as effect (B in A→B)
    /// * `direction` - LLM-detected causal direction
    ///
    /// # Returns
    /// DirectionalEmbeddings with appropriate vectors for the detected direction
    ///
    /// # Errors
    /// Returns error if CausalModel is not available and test-mode feature is not enabled.
    async fn generate_e5_embeddings(
        &self,
        cause_content: &str,
        effect_content: &str,
        direction: &CausalLinkDirection,
        analysis: &CausalAnalysisResult,
    ) -> CausalAgentResult<DirectionalEmbeddings> {
        match &self.causal_model {
            Some(causal_model) => {
                // Use real CausalModel for production embeddings with LLM guidance
                self.generate_real_e5_embeddings(causal_model, cause_content, effect_content, direction, analysis)
                    .await
            }
            None => {
                // Production mode: fail fast - CausalModel is required
                #[cfg(not(feature = "test-mode"))]
                {
                    return Err(CausalAgentError::ConfigError {
                        message: "CausalModel required in production but not available. \
                                  Use E5EmbedderActivator::with_model() or enable test-mode feature."
                            .to_string(),
                    });
                }

                // Test mode: allow placeholder embeddings
                #[cfg(feature = "test-mode")]
                {
                    debug!("Using hash-based placeholder embeddings (test-mode, no CausalModel)");
                    self.generate_placeholder_embeddings(cause_content, effect_content, direction)
                }
            }
        }
    }

    /// Generate real E5 embeddings using CausalModel with LLM-guided marker injection.
    ///
    /// Per ARCH-15: Uses asymmetric E5 with separate cause/effect encodings.
    /// Per AP-77: Direction modifiers are applied at search time, not embedding time.
    ///
    /// Constructs CausalHintGuidance from the CausalAnalysisResult so the embedder
    /// catches domain-specific causation the static word lists miss.
    async fn generate_real_e5_embeddings(
        &self,
        causal_model: &Arc<CausalModel>,
        cause_content: &str,
        effect_content: &str,
        direction: &CausalLinkDirection,
        analysis: &CausalAnalysisResult,
    ) -> CausalAgentResult<DirectionalEmbeddings> {
        // Construct guidance from the analysis result
        let asymmetry_strength = match direction {
            CausalLinkDirection::ACausesB | CausalLinkDirection::BCausesA => {
                analysis.confidence.clamp(0.5, 1.0)
            }
            CausalLinkDirection::Bidirectional => 0.2, // Low asymmetry for feedback
            CausalLinkDirection::NoCausalLink => 0.0,
        };

        // Extract key causal phrases from the mechanism text
        let key_phrases: Vec<String> = analysis.mechanism
            .split_whitespace()
            .filter(|w| w.len() > 4)
            .take(5)
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| !w.is_empty())
            .collect();

        let guidance = context_graph_core::traits::CausalHintGuidance {
            cause_spans: Vec::new(),  // No char-level spans available from pair analysis
            effect_spans: Vec::new(),
            key_phrases,
            asymmetry_strength,
            confidence: analysis.confidence,
        };

        let guidance_ref = if analysis.confidence >= 0.5 { Some(&guidance) } else { None };

        match direction {
            CausalLinkDirection::ACausesB => {
                // A is cause, B is effect
                let (cause_vec, _) = causal_model
                    .embed_dual_guided(cause_content, guidance_ref)
                    .await
                    .map_err(|e| CausalAgentError::EmbeddingError {
                        message: format!("Failed to embed cause: {}", e),
                    })?;
                let (_, effect_vec) = causal_model
                    .embed_dual_guided(effect_content, guidance_ref)
                    .await
                    .map_err(|e| CausalAgentError::EmbeddingError {
                        message: format!("Failed to embed effect: {}", e),
                    })?;
                info!(
                    cause_dim = cause_vec.len(),
                    effect_dim = effect_vec.len(),
                    "Generated real E5 embeddings (A→B) with LLM guidance"
                );
                Ok(DirectionalEmbeddings::forward(cause_vec, effect_vec))
            }
            CausalLinkDirection::BCausesA => {
                // B is cause, A is effect (swap roles)
                let (cause_vec, _) = causal_model
                    .embed_dual_guided(effect_content, guidance_ref)
                    .await
                    .map_err(|e| CausalAgentError::EmbeddingError {
                        message: format!("Failed to embed cause: {}", e),
                    })?;
                let (_, effect_vec) = causal_model
                    .embed_dual_guided(cause_content, guidance_ref)
                    .await
                    .map_err(|e| CausalAgentError::EmbeddingError {
                        message: format!("Failed to embed effect: {}", e),
                    })?;
                info!(
                    cause_dim = cause_vec.len(),
                    effect_dim = effect_vec.len(),
                    "Generated real E5 embeddings (B→A) with LLM guidance"
                );
                Ok(DirectionalEmbeddings::backward(cause_vec, effect_vec))
            }
            CausalLinkDirection::Bidirectional => {
                // Both act as cause AND effect (feedback loop)
                let (a_cause, a_effect) = causal_model
                    .embed_dual_guided(cause_content, guidance_ref)
                    .await
                    .map_err(|e| CausalAgentError::EmbeddingError {
                        message: format!("Failed to embed A dual: {}", e),
                    })?;
                let (b_cause, b_effect) = causal_model
                    .embed_dual_guided(effect_content, guidance_ref)
                    .await
                    .map_err(|e| CausalAgentError::EmbeddingError {
                        message: format!("Failed to embed B dual: {}", e),
                    })?;
                info!(
                    a_dim = a_cause.len(),
                    b_dim = b_cause.len(),
                    "Generated real E5 bidirectional embeddings (A↔B) with LLM guidance"
                );
                Ok(DirectionalEmbeddings::bidirectional(
                    a_cause, a_effect, b_cause, b_effect,
                ))
            }
            CausalLinkDirection::NoCausalLink => {
                Err(CausalAgentError::ConfigError {
                    message: "Cannot generate embeddings for non-causal relationship".to_string(),
                })
            }
        }
    }

    /// Generate placeholder embeddings based on content hash.
    ///
    /// Used for testing when no real CausalModel is available.
    #[cfg(feature = "test-mode")]
    fn generate_placeholder_embeddings(
        &self,
        cause_content: &str,
        effect_content: &str,
        direction: &CausalLinkDirection,
    ) -> CausalAgentResult<DirectionalEmbeddings> {
        match direction {
            CausalLinkDirection::ACausesB => {
                let cause_vec = self.hash_to_embedding(cause_content, 768, true);
                let effect_vec = self.hash_to_embedding(effect_content, 768, false);
                Ok(DirectionalEmbeddings::forward(cause_vec, effect_vec))
            }
            CausalLinkDirection::BCausesA => {
                let cause_vec = self.hash_to_embedding(effect_content, 768, true);
                let effect_vec = self.hash_to_embedding(cause_content, 768, false);
                Ok(DirectionalEmbeddings::backward(cause_vec, effect_vec))
            }
            CausalLinkDirection::Bidirectional => {
                let a_cause = self.hash_to_embedding(cause_content, 768, true);
                let a_effect = self.hash_to_embedding(cause_content, 768, false);
                let b_cause = self.hash_to_embedding(effect_content, 768, true);
                let b_effect = self.hash_to_embedding(effect_content, 768, false);
                Ok(DirectionalEmbeddings::bidirectional(
                    a_cause, a_effect, b_cause, b_effect,
                ))
            }
            CausalLinkDirection::NoCausalLink => Err(CausalAgentError::ConfigError {
                message: "Cannot generate embeddings for non-causal relationship".to_string(),
            }),
        }
    }

    /// Generate a deterministic embedding from content hash.
    ///
    /// This is a placeholder for testing until CausalModel integration.
    #[cfg(feature = "test-mode")]
    fn hash_to_embedding(&self, content: &str, dim: usize, is_cause: bool) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        if is_cause {
            "cause".hash(&mut hasher);
        } else {
            "effect".hash(&mut hasher);
        }
        let hash = hasher.finish();

        // Generate pseudo-random normalized vector
        let mut embedding = Vec::with_capacity(dim);
        let mut current = hash;
        let mut sum_sq = 0.0f32;

        for _ in 0..dim {
            current = current.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let val = ((current >> 32) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            embedding.push(val);
            sum_sq += val * val;
        }

        // Normalize
        let norm = sum_sq.sqrt();
        if norm > f32::EPSILON {
            for val in &mut embedding {
                *val /= norm;
            }
        }

        embedding
    }

    /// Add an edge to the causal graph.
    fn add_graph_edge(
        &self,
        cause_id: Uuid,
        effect_id: Uuid,
        cause_content: &str,
        effect_content: &str,
        analysis: &CausalAnalysisResult,
    ) -> CausalAgentResult<()> {
        let mut graph = self.causal_graph.write();

        // Auto-create nodes if configured
        if self.config.auto_create_nodes {
            if !graph.has_node(cause_id) {
                let node = CausalNode::with_id(
                    cause_id,
                    truncate_name(cause_content, 50),
                    "memory",
                );
                graph.add_node(node);
            }

            if !graph.has_node(effect_id) {
                let node = CausalNode::with_id(
                    effect_id,
                    truncate_name(effect_content, 50),
                    "memory",
                );
                graph.add_node(node);
            }
        }

        // Add edge based on direction
        match analysis.direction {
            CausalLinkDirection::ACausesB => {
                graph.add_edge(CausalEdge::new(
                    cause_id,
                    effect_id,
                    analysis.confidence,
                    &analysis.mechanism,
                ));
            }
            CausalLinkDirection::BCausesA => {
                graph.add_edge(CausalEdge::new(
                    effect_id,
                    cause_id,
                    analysis.confidence,
                    &analysis.mechanism,
                ));
            }
            CausalLinkDirection::Bidirectional => {
                // Add edges in both directions
                graph.add_edge(CausalEdge::new(
                    cause_id,
                    effect_id,
                    analysis.confidence * 0.8, // Slightly lower for bidirectional
                    &format!("{} (forward)", analysis.mechanism),
                ));
                graph.add_edge(CausalEdge::new(
                    effect_id,
                    cause_id,
                    analysis.confidence * 0.8,
                    &format!("{} (backward)", analysis.mechanism),
                ));
            }
            CausalLinkDirection::NoCausalLink => {
                // Should not happen, but handle gracefully
                warn!(
                    cause = %cause_id,
                    effect = %effect_id,
                    "Attempted to add edge for non-causal relationship"
                );
            }
        }

        Ok(())
    }

    /// Get activation statistics.
    pub fn stats(&self) -> ActivationStats {
        self.stats.read().clone()
    }

    /// Reset activation statistics.
    pub fn reset_stats(&self) {
        let mut stats = self.stats.write();
        *stats = ActivationStats::default();
    }

    /// Get the configuration.
    pub fn config(&self) -> &ActivatorConfig {
        &self.config
    }

    /// Get a reference to the causal graph.
    pub fn causal_graph(&self) -> &Arc<RwLock<CausalGraph>> {
        &self.causal_graph
    }
}

/// Truncate content to create a node name.
fn truncate_name(content: &str, max_len: usize) -> String {
    let trimmed = content.trim();
    if trimmed.len() <= max_len {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..max_len - 3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // This test requires test-mode feature for placeholder embeddings
    #[cfg(feature = "test-mode")]
    #[tokio::test]
    async fn test_activate_relationship() {
        let graph = Arc::new(RwLock::new(CausalGraph::new()));
        let activator = E5EmbedderActivator::new(graph.clone());

        let cause_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();

        let analysis = CausalAnalysisResult {
            has_causal_link: true,
            direction: CausalLinkDirection::ACausesB,
            confidence: 0.85,
            mechanism: "Direct causation".to_string(),
            mechanism_type: None,
            raw_response: None,
            llm_provenance: None,
        };

        let result = activator
            .activate_relationship(
                cause_id,
                effect_id,
                "The bug caused the crash",
                "The crash affected users",
                &analysis,
            )
            .await;

        assert!(result.is_ok());

        // Check graph has the edge
        let graph = graph.read();
        assert!(graph.has_node(cause_id));
        assert!(graph.has_node(effect_id));
        assert!(graph.has_direct_cause(cause_id, effect_id));
    }

    #[tokio::test]
    async fn test_skip_low_confidence() {
        let graph = Arc::new(RwLock::new(CausalGraph::new()));
        let activator = E5EmbedderActivator::new(graph);

        let analysis = CausalAnalysisResult {
            has_causal_link: true,
            direction: CausalLinkDirection::ACausesB,
            confidence: 0.3, // Below threshold
            mechanism: "Weak evidence".to_string(),
            mechanism_type: None,
            raw_response: None,
            llm_provenance: None,
        };

        let result = activator
            .activate_relationship(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "Content A",
                "Content B",
                &analysis,
            )
            .await;

        assert!(result.is_err());

        let stats = activator.stats();
        assert_eq!(stats.skipped_low_confidence, 1);
    }

    #[test]
    fn test_truncate_name() {
        assert_eq!(truncate_name("Short", 50), "Short");
        assert_eq!(
            truncate_name("This is a very long name that exceeds the maximum length", 20),
            "This is a very lo..."
        );
    }

    #[cfg(feature = "test-mode")]
    #[test]
    fn test_hash_to_embedding() {
        let graph = Arc::new(RwLock::new(CausalGraph::new()));
        let activator = E5EmbedderActivator::new(graph);

        let emb1 = activator.hash_to_embedding("test content", 768, true);
        let emb2 = activator.hash_to_embedding("test content", 768, true);
        let emb3 = activator.hash_to_embedding("test content", 768, false);

        // Same input = same output
        assert_eq!(emb1, emb2);

        // Different role = different output
        assert_ne!(emb1, emb3);

        // Check normalization
        let norm: f32 = emb1.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.001);
    }

    // Test fail-fast behavior when CausalModel is not available in production mode
    #[cfg(not(feature = "test-mode"))]
    #[tokio::test]
    async fn test_e5_activator_fails_fast_without_model() {
        let graph = Arc::new(RwLock::new(CausalGraph::new()));
        let activator = E5EmbedderActivator::new(graph); // No model

        let analysis = CausalAnalysisResult {
            has_causal_link: true,
            direction: CausalLinkDirection::ACausesB,
            confidence: 0.85,
            mechanism: "Direct causation".to_string(),
            mechanism_type: None,
            raw_response: None,
            llm_provenance: None,
        };

        let result = activator
            .activate_relationship(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "Cause content",
                "Effect content",
                &analysis,
            )
            .await;

        // Should fail because CausalModel is required in production
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("CausalModel required") || err.contains("production"),
            "Error should mention CausalModel requirement: {}",
            err
        );
    }

    // Test that placeholder embeddings work in test-mode
    #[cfg(feature = "test-mode")]
    #[tokio::test]
    async fn test_e5_activator_uses_placeholders_in_test_mode() {
        let graph = Arc::new(RwLock::new(CausalGraph::new()));
        let activator = E5EmbedderActivator::new(graph.clone()); // No model

        let cause_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();

        let analysis = CausalAnalysisResult {
            has_causal_link: true,
            direction: CausalLinkDirection::ACausesB,
            confidence: 0.85,
            mechanism: "Direct causation".to_string(),
            mechanism_type: None,
            raw_response: None,
            llm_provenance: None,
        };

        let result = activator
            .activate_relationship(
                cause_id,
                effect_id,
                "Cause content",
                "Effect content",
                &analysis,
            )
            .await;

        // Should succeed with placeholder embeddings in test-mode
        assert!(
            result.is_ok(),
            "Should use placeholder embeddings in test-mode: {:?}",
            result
        );

        let (cause_emb, effect_emb) = result.unwrap();
        assert_eq!(cause_emb.len(), 768, "E5 embeddings should be 768D");
        assert_eq!(effect_emb.len(), 768, "E5 embeddings should be 768D");

        // Cause and effect should be different
        assert_ne!(cause_emb, effect_emb, "Cause and effect embeddings should differ");
    }
}
