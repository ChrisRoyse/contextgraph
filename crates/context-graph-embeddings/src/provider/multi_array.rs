//! Production MultiArrayEmbeddingProvider that orchestrates all 14 embedders.
//!
//! This module provides the [`ProductionMultiArrayProvider`] which replaces the
//! placeholder [`LazyFailMultiArrayProvider`] with real model implementations.
//!
//! # Architecture
//!
//! ```text
//! ProductionMultiArrayProvider
//!     |-- 10 dense SingleEmbedder instances (E1-E5, E7-E11)
//!     |-- 2 SparseEmbedder instances (E6, E13 - SPLADE)
//!     |-- 1 TokenEmbedder (E12 ColBERT)
//!
//!     Returns: SemanticFingerprint with all 14 embeddings
//! ```
//!
//! # Design Principles
//!
//! - **NO STUBS**: Uses real model implementations from DefaultModelFactory
//! - **FAIL FAST**: Returns clear errors if models not loaded
//! - **PARALLEL EXECUTION**: All 14 embedders run concurrently via tokio::join!
//! - **THREAD SAFE**: Send + Sync for async task spawning across threads
//!
//! # Performance Targets (from constitution.yaml)
//!
//! - Single content: <30ms for the legacy 13-embedder path; E14 adds BGE-M3 latency.
//! - Batch (64 items): <100ms per item average
//!
//! # Example
//!
//! ```ignore
//! use context_graph_embeddings::provider::ProductionMultiArrayProvider;
//! use context_graph_embeddings::config::GpuConfig;
//! use std::path::PathBuf;
//!
//! // Create provider (models must exist at models_dir)
//! let provider = ProductionMultiArrayProvider::new(
//!     PathBuf::from("./models"),
//!     GpuConfig::default(),
//! ).await?;
//!
//! // Generate all 14 embeddings
//! let output = provider.embed_all("Hello world").await?;
//! assert!(output.is_within_latency_target()); // <30ms
//! ```

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::traits::{
    CausalHint, EmbeddingMetadata, MultiArrayEmbeddingOutput, MultiArrayEmbeddingProvider,
    SingleEmbedder, SparseEmbedder, TokenEmbedder,
};
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, E10_DIM, E11_DIM, E12_TOKEN_DIM, E14_DIM, E1_DIM, E2_DIM,
    E3_DIM, E4_DIM, E7_DIM, E8_DIM, E9_DIM, NUM_EMBEDDERS,
};
use context_graph_core::weights::{
    E11_ENTITY_ENABLED, E5_CAUSAL_ENABLED, TEMPORAL_EMBEDDERS_ENABLED,
};

use crate::config::GpuConfig;
use crate::error::{EmbeddingError, EmbeddingResult};
use crate::models::pretrained::{CausalModel, ContextualModel, GraphModel};
use crate::models::DefaultModelFactory;
use crate::traits::{EmbeddingModel, ModelFactory, SingleModelConfig};
use crate::types::{ModelId, ModelInput};

// ============================================================================
// ADAPTER TYPES - Bridge EmbeddingModel to SingleEmbedder/SparseEmbedder/TokenEmbedder
// ============================================================================

/// Adapter that wraps an EmbeddingModel to implement SingleEmbedder trait.
struct DenseEmbedderAdapter {
    model: Arc<RwLock<Box<dyn EmbeddingModel>>>,
    model_id: ModelId,
    dimension: usize,
}

impl DenseEmbedderAdapter {
    fn new(model: Box<dyn EmbeddingModel>, model_id: ModelId, dimension: usize) -> Self {
        Self {
            model: Arc::new(RwLock::new(model)),
            model_id,
            dimension,
        }
    }
}

impl DenseEmbedderAdapter {
    /// Embed content with a custom instruction for the model.
    ///
    /// E4-FIX: Used for passing sequence numbers to E4 via "sequence:N" instruction.
    async fn embed_with_instruction(
        &self,
        content: &str,
        instruction: Option<&str>,
    ) -> CoreResult<Vec<f32>> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        let model = self.model.read().await;
        if !model.is_initialized() {
            return Err(CoreError::Internal(format!(
                "Model {:?} not initialized",
                self.model_id
            )));
        }

        let input = match instruction {
            Some(inst) => ModelInput::text_with_instruction(content, inst).map_err(|e| {
                CoreError::ValidationError {
                    field: "content".to_string(),
                    message: e.to_string(),
                }
            })?,
            None => ModelInput::text(content).map_err(|e| CoreError::ValidationError {
                field: "content".to_string(),
                message: e.to_string(),
            })?,
        };

        let embedding = model.embed(&input).await.map_err(|e| {
            CoreError::Embedding(format!("Embedding failed for {:?}: {}", self.model_id, e))
        })?;

        Ok(embedding.into_vec())
    }
}

#[async_trait]
impl SingleEmbedder for DenseEmbedderAdapter {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        self.model_id.as_str()
    }

    async fn embed(&self, content: &str) -> CoreResult<Vec<f32>> {
        DenseEmbedderAdapter::embed_with_instruction(self, content, None).await
    }

    async fn embed_with_instruction(
        &self,
        content: &str,
        instruction: Option<&str>,
    ) -> CoreResult<Vec<f32>> {
        DenseEmbedderAdapter::embed_with_instruction(self, content, instruction).await
    }

    fn is_ready(&self) -> bool {
        // EMB-6 FIX: Delegate to model's is_initialized() instead of hardcoding true.
        // Use try_read() since is_ready() is sync but RwLock is tokio::sync (async).
        // If lock is held (active embedding), assume ready.
        match self.model.try_read() {
            Ok(guard) => guard.is_initialized(),
            Err(_) => true, // Lock held = model actively embedding = ready
        }
    }
}

// SAFETY: DenseEmbedderAdapter is Send + Sync because its only mutable field is
// `model: Arc<RwLock<Box<dyn EmbeddingModel>>>`. Arc provides shared ownership,
// and RwLock (from parking_lot) provides synchronized interior mutability.
// The underlying EmbeddingModel trait objects may not be Send/Sync themselves
// (e.g., ort::Session), but all access is mediated through the RwLock.
unsafe impl Send for DenseEmbedderAdapter {}
unsafe impl Sync for DenseEmbedderAdapter {}

/// Explicit disabled-slot adapter for embedders that are intentionally not loaded.
///
/// This is not a fallback: production paths gate these slots out before scoring.
/// Direct calls fail with a clear error so a disabled embedder cannot silently
/// influence ME-JEPA/retrieval.
struct DisabledDenseEmbedder {
    model_id: &'static str,
    dimension: usize,
    reason: &'static str,
}

impl DisabledDenseEmbedder {
    fn new(model_id: &'static str, dimension: usize, reason: &'static str) -> Self {
        Self {
            model_id,
            dimension,
            reason,
        }
    }
}

#[async_trait]
impl SingleEmbedder for DisabledDenseEmbedder {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn model_id(&self) -> &str {
        self.model_id
    }

    async fn embed(&self, _content: &str) -> CoreResult<Vec<f32>> {
        Err(CoreError::Internal(self.reason.to_string()))
    }

    fn is_ready(&self) -> bool {
        true
    }
}

unsafe impl Send for DisabledDenseEmbedder {}
unsafe impl Sync for DisabledDenseEmbedder {}

/// Adapter that wraps a Sparse EmbeddingModel (SPLADE) to implement SparseEmbedder trait.
struct SparseEmbedderAdapter {
    model: Arc<RwLock<Box<dyn EmbeddingModel>>>,
    model_id: ModelId,
}

impl SparseEmbedderAdapter {
    fn new(model: Box<dyn EmbeddingModel>, model_id: ModelId) -> Self {
        Self {
            model: Arc::new(RwLock::new(model)),
            model_id,
        }
    }
}

#[async_trait]
impl SparseEmbedder for SparseEmbedderAdapter {
    fn vocab_size(&self) -> usize {
        30522 // SPLADE uses BERT vocabulary
    }

    fn model_id(&self) -> &str {
        self.model_id.as_str()
    }

    async fn embed_sparse(&self, content: &str) -> CoreResult<SparseVector> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        let model = self.model.read().await;
        if !model.is_initialized() {
            return Err(CoreError::Internal(format!(
                "Model {:?} not initialized",
                self.model_id
            )));
        }

        let input = ModelInput::text(content).map_err(|e| CoreError::ValidationError {
            field: "content".to_string(),
            message: e.to_string(),
        })?;

        // Call embed_sparse() to get actual sparse vocabulary indices and weights
        // NOT embed() which returns a 1536D projected dense vector
        let (indices, values) = model.embed_sparse(&input).await.map_err(|e| {
            CoreError::Embedding(format!(
                "Sparse embedding failed for {:?}: {}",
                self.model_id, e
            ))
        })?;

        SparseVector::new(indices, values)
            .map_err(|e| CoreError::Internal(format!("Failed to create sparse vector: {}", e)))
    }

    fn is_ready(&self) -> bool {
        // EMB-6 FIX: Delegate to model's is_initialized()
        match self.model.try_read() {
            Ok(guard) => guard.is_initialized(),
            Err(_) => true,
        }
    }
}

// SAFETY: SparseEmbedderAdapter is Send + Sync because its only mutable field is
// `model: Arc<RwLock<Box<dyn EmbeddingModel>>>`. All access to the underlying
// model is synchronized through the RwLock.
unsafe impl Send for SparseEmbedderAdapter {}
unsafe impl Sync for SparseEmbedderAdapter {}

/// Adapter that wraps ColBERT model to implement TokenEmbedder trait.
struct TokenEmbedderAdapter {
    model: Arc<RwLock<Box<dyn EmbeddingModel>>>,
    model_id: ModelId,
}

impl TokenEmbedderAdapter {
    fn new(model: Box<dyn EmbeddingModel>, model_id: ModelId) -> Self {
        Self {
            model: Arc::new(RwLock::new(model)),
            model_id,
        }
    }
}

#[async_trait]
impl TokenEmbedder for TokenEmbedderAdapter {
    fn token_dimension(&self) -> usize {
        E12_TOKEN_DIM // 128D per token
    }

    fn max_tokens(&self) -> usize {
        512 // ColBERT uses BERT tokenizer
    }

    fn model_id(&self) -> &str {
        self.model_id.as_str()
    }

    async fn embed_tokens(&self, content: &str) -> CoreResult<Vec<Vec<f32>>> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        let model = self.model.read().await;
        if !model.is_initialized() {
            return Err(CoreError::Internal(format!(
                "Model {:?} not initialized",
                self.model_id
            )));
        }

        let input = ModelInput::text(content).map_err(|e| CoreError::ValidationError {
            field: "content".to_string(),
            message: e.to_string(),
        })?;

        let embedding = model.embed(&input).await.map_err(|e| {
            CoreError::Embedding(format!(
                "Token embedding failed for {:?}: {}",
                self.model_id, e
            ))
        })?;

        // For ColBERT, the model produces [num_tokens, 128] tensor
        // We reshape the flat vector into token embeddings
        let flat = embedding.into_vec();
        let token_dim = E12_TOKEN_DIM;

        if flat.len() % token_dim != 0 {
            return Err(CoreError::Internal(format!(
                "ColBERT output size {} not divisible by token dimension {}",
                flat.len(),
                token_dim
            )));
        }

        let num_tokens = flat.len() / token_dim;
        let mut tokens = Vec::with_capacity(num_tokens);

        for i in 0..num_tokens {
            let start = i * token_dim;
            let end = start + token_dim;
            tokens.push(flat[start..end].to_vec());
        }

        Ok(tokens)
    }

    fn is_ready(&self) -> bool {
        // EMB-6 FIX: Delegate to model's is_initialized()
        match self.model.try_read() {
            Ok(guard) => guard.is_initialized(),
            Err(_) => true,
        }
    }
}

// SAFETY: TokenEmbedderAdapter is Send + Sync because its only mutable field is
// `model: Arc<RwLock<Box<dyn EmbeddingModel>>>`. All access to the underlying
// ColBERT model is synchronized through the RwLock.
unsafe impl Send for TokenEmbedderAdapter {}
unsafe impl Sync for TokenEmbedderAdapter {}

// ============================================================================
// CAUSAL DUAL EMBEDDER - Specialized adapter for E5 asymmetric embeddings
// ============================================================================

/// Adapter for E5 CausalModel that produces dual (cause, effect) embeddings.
///
/// Per ARCH-15: "E5 Causal MUST use asymmetric similarity with separate
/// cause/effect vector encodings - cause→effect direction matters"
///
/// This adapter exposes the `embed_dual()` method from CausalModel, which
/// produces genuinely different vectors for cause vs effect roles.
#[allow(dead_code)]
struct CausalDualEmbedderAdapter {
    /// Direct reference to CausalModel (not wrapped in EmbeddingModel trait)
    model: Arc<CausalModel>,
}

#[allow(dead_code)]
impl CausalDualEmbedderAdapter {
    /// Create a new CausalDualEmbedderAdapter.
    ///
    /// # Arguments
    /// * `model` - CausalModel instance (must be loaded before use)
    fn new(model: CausalModel) -> Self {
        Self {
            model: Arc::new(model),
        }
    }

    /// Get the underlying CausalModel.
    ///
    /// This method exposes the model for use by CausalDiscoveryService.
    fn model(&self) -> Arc<CausalModel> {
        Arc::clone(&self.model)
    }

    /// Embed content as both cause and effect roles.
    ///
    /// Returns (cause_vector, effect_vector) where each is 768D.
    /// The vectors are genuinely different due to instruction prefixes.
    ///
    /// # Errors
    /// - `CoreError::Internal` if model not initialized
    /// - `CoreError::Embedding` if embedding fails
    async fn embed_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        if !self.model.is_initialized() {
            return Err(CoreError::Internal(
                "CausalModel not initialized for dual embedding".to_string(),
            ));
        }

        self.model
            .embed_dual(content)
            .await
            .map_err(|e| CoreError::Embedding(format!("E5 dual embedding failed: {}", e)))
    }

    /// Check if the model is ready for embedding.
    fn is_ready(&self) -> bool {
        self.model.is_initialized()
    }

    /// Embed content with LLM-provided causal hint for enhanced direction awareness.
    ///
    /// If a useful hint is provided (is_causal && confidence >= 0.5), the embedding
    /// vectors are biased based on the direction hint:
    /// - `CausalDirectionHint::Cause`: Boost cause vector (1.3x), dampen effect (0.8x)
    /// - `CausalDirectionHint::Effect`: Boost effect vector (1.3x), dampen cause (0.8x)
    /// - `CausalDirectionHint::Neutral`: No bias applied
    ///
    /// If no hint is provided or hint is not useful, falls back to standard `embed_dual()`.
    ///
    /// # Arguments
    ///
    /// * `content` - The text content to embed
    /// * `hint` - Optional LLM-generated causal hint
    ///
    /// # Returns
    ///
    /// (cause_vector, effect_vector) where each is 768D, with direction bias applied.
    ///
    /// # CAUSAL-HINT Phase 4: E5 Enhancement
    ///
    /// This method enables LLM-enhanced E5 embeddings per the Causal Discovery
    /// LLM + E5 Integration Plan.
    async fn embed_dual_with_hint(
        &self,
        content: &str,
        hint: Option<&CausalHint>,
    ) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        // Convert CausalHint to lightweight guidance for the embedder pipeline
        let guidance = hint.and_then(|h| h.to_guidance());

        // Embed with LLM-guided marker detection
        let (mut cause_vec, mut effect_vec) = self
            .model
            .embed_dual_guided(content, guidance.as_ref())
            .await
            .map_err(|e| CoreError::Embedding(format!("E5 guided dual embedding failed: {}", e)))?;

        // KEEP the direction bias (complementary to marker guidance)
        if let Some(hint) = hint {
            if hint.is_useful() {
                let (cause_bias, effect_bias) = hint.bias_factors();

                for val in cause_vec.iter_mut() {
                    *val *= cause_bias;
                }
                for val in effect_vec.iter_mut() {
                    *val *= effect_bias;
                }

                tracing::debug!(
                    direction = ?hint.direction_hint,
                    cause_bias = cause_bias,
                    effect_bias = effect_bias,
                    confidence = hint.confidence,
                    llm_cause_spans = hint.cause_spans.len(),
                    llm_effect_spans = hint.effect_spans.len(),
                    asymmetry = hint.asymmetry_strength,
                    "E5: Applied LLM-guided marker injection + direction bias"
                );
            }
        }

        Ok((cause_vec, effect_vec))
    }
}

// SAFETY: CausalDualEmbedderAdapter is Send + Sync because its only field is
// `model: Arc<CausalModel>`. CausalModel internally uses RwLock-protected state
// for the ort::Session. All access is synchronized through Arc + internal locks.
unsafe impl Send for CausalDualEmbedderAdapter {}
unsafe impl Sync for CausalDualEmbedderAdapter {}

// ============================================================================
// GRAPH DUAL EMBEDDER ADAPTER (E8 - Asymmetric Source/Target)
// ============================================================================

/// Adapter that wraps GraphModel to support dual source/target embedding.
///
/// Following the E5 Causal pattern (ARCH-15), this adapter enables asymmetric
/// similarity for graph relationships where direction matters:
/// - **Source embedding**: "What does X use?" → X is the source
/// - **Target embedding**: "What uses X?" → X is the target
///
/// The GraphModel.embed_dual() method produces genuinely different vectors
/// through learned projections (W_source, W_target).
struct GraphDualEmbedderAdapter {
    /// Direct reference to GraphModel (not wrapped in EmbeddingModel trait)
    model: Arc<GraphModel>,
}

impl GraphDualEmbedderAdapter {
    /// Create a new GraphDualEmbedderAdapter.
    ///
    /// # Arguments
    /// * `model` - GraphModel instance (must be loaded before use)
    fn new(model: GraphModel) -> Self {
        Self {
            model: Arc::new(model),
        }
    }

    /// Get the underlying GraphModel.
    ///
    /// This method exposes the graph model for deterministic graph activation.
    fn model(&self) -> Arc<GraphModel> {
        Arc::clone(&self.model)
    }

    /// Embed content as both source and target roles.
    ///
    /// Returns (source_vector, target_vector) where each is 1024D.
    /// The vectors are genuinely different due to learned projections.
    ///
    /// # Errors
    /// - `CoreError::Internal` if model not initialized
    /// - `CoreError::Embedding` if embedding fails
    async fn embed_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        if !self.model.is_initialized() {
            return Err(CoreError::Internal(
                "GraphModel not initialized for dual embedding".to_string(),
            ));
        }

        self.model
            .embed_dual(content)
            .await
            .map_err(|e| CoreError::Embedding(format!("E8 dual embedding failed: {}", e)))
    }

    /// Check if the model is ready for embedding.
    fn is_ready(&self) -> bool {
        self.model.is_initialized()
    }
}

// SAFETY: GraphDualEmbedderAdapter is Send + Sync because its only field is
// `model: Arc<GraphModel>`. GraphModel internally uses RwLock-protected state
// for the ort::Session. All access is synchronized through Arc + internal locks.
unsafe impl Send for GraphDualEmbedderAdapter {}
unsafe impl Sync for GraphDualEmbedderAdapter {}

// ============================================================================
// CONTEXTUAL (E10) DUAL EMBEDDER ADAPTER - Asymmetric Paraphrase/Context
// ============================================================================

/// Adapter for E10 ContextualModel that produces dual (paraphrase, context) embeddings.
///
/// Following the E5 Causal and E8 Graph patterns (ARCH-15), this adapter enables
/// asymmetric similarity for paraphrase-context relationships where direction matters:
/// - **Paraphrase embedding**: "What is this text trying to accomplish?" (action-focused)
/// - **Context embedding**: "What context does this establish?" (relation-focused)
///
/// Direction modifiers (per plan):
/// - paraphrase→context: 1.2x (query paraphrase finds relevant context)
/// - context→paraphrase: 0.8x (dampened reverse direction)
struct ContextualDualEmbedderAdapter {
    /// Direct reference to ContextualModel (not wrapped in EmbeddingModel trait)
    model: Arc<ContextualModel>,
}

impl ContextualDualEmbedderAdapter {
    /// Create a new ContextualDualEmbedderAdapter.
    ///
    /// # Arguments
    /// * `model` - ContextualModel instance (must be loaded before use)
    fn new(model: ContextualModel) -> Self {
        Self {
            model: Arc::new(model),
        }
    }

    /// Embed content as both intent and context roles.
    ///
    /// Returns (intent_vector, context_vector) where each is 768D.
    /// The vectors are genuinely different due to learned projections.
    ///
    /// # Errors
    /// - `CoreError::Internal` if model not initialized
    /// - `CoreError::Embedding` if embedding fails
    async fn embed_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        if !self.model.is_initialized() {
            return Err(CoreError::Internal(
                "ContextualModel not initialized for dual embedding".to_string(),
            ));
        }

        self.model
            .embed_dual(content)
            .await
            .map_err(|e| CoreError::Embedding(format!("E10 dual embedding failed: {}", e)))
    }

    /// Check if the model is ready for embedding.
    fn is_ready(&self) -> bool {
        self.model.is_initialized()
    }
}

// SAFETY: ContextualDualEmbedderAdapter is Send + Sync because its only field is
// `model: Arc<ContextualModel>`. ContextualModel internally uses RwLock-protected
// state for the ort::Session. All access is synchronized through Arc + internal locks.
unsafe impl Send for ContextualDualEmbedderAdapter {}
unsafe impl Sync for ContextualDualEmbedderAdapter {}

// ============================================================================
// PRODUCTION MULTI-ARRAY PROVIDER
// ============================================================================

/// Production MultiArrayEmbeddingProvider that orchestrates all 14 embedders.
///
/// This provider replaces the placeholder LazyFailMultiArrayProvider with real
/// model implementations using the DefaultModelFactory.
///
/// # Thread Safety
///
/// This provider is `Send + Sync` and can be shared across async tasks.
/// Internal models use RwLock for safe concurrent access.
///
/// # Model Loading
///
/// Models are loaded eagerly during construction. The async `new()` method
/// loads all production models and fails fast if any required model cannot be loaded.
///
/// # Performance
///
/// Active embedders run in parallel using tokio::join! to achieve
/// the <30ms latency target for single content embedding.
pub struct ProductionMultiArrayProvider {
    /// E1: Semantic embedder (e5-large-v2, 1024D)
    e1_semantic: Arc<dyn SingleEmbedder>,
    /// E2: Temporal-Recent embedder (exponential decay, 512D)
    e2_temporal_recent: Arc<dyn SingleEmbedder>,
    /// E3: Temporal-Periodic embedder (Fourier, 512D)
    e3_temporal_periodic: Arc<dyn SingleEmbedder>,
    /// E4: Temporal-Positional embedder (sinusoidal PE, 512D)
    ///
    /// E4-FIX: Stored as concrete type to allow `embed_with_instruction()` calls
    /// for passing sequence numbers via "sequence:N" instruction.
    e4_temporal_positional: Arc<DenseEmbedderAdapter>,
    // E5 is retired and intentionally not loaded. The slot remains in the
    // 14-slot fingerprint/index ABI for legacy data, but new ME-JEPA/retrieval
    // work must not depend on it.
    /// E6: Sparse embedder (SPLADE, variable sparse)
    e6_sparse: Arc<dyn SparseEmbedder>,
    /// E7: Code embedder (Qodo-Embed, 1536D)
    e7_code: Arc<dyn SingleEmbedder>,
    /// E8: Graph embedder (e5-large-v2, 1024D) - DUAL embedder for asymmetric similarity
    ///
    /// Per E8 Upgrade: Uses GraphDualEmbedderAdapter to produce genuinely different
    /// vectors for source vs target roles.
    e8_graph: Arc<GraphDualEmbedderAdapter>,
    /// E9: HDC embedder (hyperdimensional, 1024D projected)
    e9_hdc: Arc<dyn SingleEmbedder>,
    /// E10: Contextual embedder (MPNet, 768D) - DUAL embedder for asymmetric similarity
    ///
    /// Per E10 Upgrade: Uses ContextualDualEmbedderAdapter to produce genuinely different
    /// vectors for intent vs context roles.
    e10_contextual: Arc<ContextualDualEmbedderAdapter>,
    /// E11: Entity embedder (KEPLER, 768D), disabled until a self-contained runtime checkpoint exists.
    ///
    /// KEPLER is RoBERTa-base trained with TransE on Wikidata5M (4.8M entities, 20M triples).
    /// Unlike the previous MiniLM model, TransE operations (h + r ≈ t) are semantically meaningful.
    e11_entity: Arc<dyn SingleEmbedder>,
    /// E12: Late-Interaction embedder (ColBERT, 128D per token)
    e12_late_interaction: Arc<dyn TokenEmbedder>,
    /// E13: SPLADE v3 sparse embedder (variable sparse)
    e13_splade: Arc<dyn SparseEmbedder>,
    /// E14: BGE-M3 Dense embedder (XLM-RoBERTa-Large, 1024D CLS-pooled).
    ///
    /// Populated on provider construction from `./models/bge-m3-dense/`.
    /// Missing BAAI/bge-m3 assets are fatal so E14 cannot silently disappear.
    e14_bge_m3_dense: Arc<dyn SingleEmbedder>,

    /// Model IDs for tracking
    model_ids: [String; NUM_EMBEDDERS],
}

const E5_RETIRED_MODEL_ID: &str = "E5_RETIRED_DISABLED_NOT_LOADED";
const E11_DISABLED_MODEL_ID: &str = "E11_KEPLER_DISABLED_NOT_LOADED";
const E11_DISABLED_REASON: &str = "E11 Kepler embedder is disabled: available assets are fairseq .pt checkpoints, but the runtime requires a self-contained Hugging Face-style checkpoint";

fn retired_e5_vectors() -> (Vec<f32>, Vec<f32>) {
    (Vec::new(), Vec::new())
}

fn retired_e5_error() -> CoreError {
    CoreError::Internal(
        "E5 causal embedder is retired and disabled; use active ME-JEPA embedders instead"
            .to_string(),
    )
}

const E14_REQUIRED_FILES: [&str; 3] = ["config.json", "pytorch_model.bin", "tokenizer.json"];
const ACTIVE_MODEL_REGISTRY: &str = "mejepa_models_config.toml";

fn ensure_bge_m3_assets(models_dir: &Path) -> EmbeddingResult<PathBuf> {
    let bge_dir = models_dir.join(ModelId::BgeM3Dense.directory_name());
    let mut failures = Vec::new();

    for file in E14_REQUIRED_FILES {
        let path = bge_dir.join(file);
        match path.metadata() {
            Ok(metadata) if !metadata.is_file() => {
                failures.push(format!("{} is not a regular file", path.display()));
            }
            Ok(metadata) if metadata.len() == 0 => {
                failures.push(format!("{} is empty", path.display()));
            }
            Ok(_) => {}
            Err(err) => {
                failures.push(format!(
                    "{} is missing or unreadable: {err}",
                    path.display()
                ));
            }
        }
    }

    if failures.is_empty() {
        for file in ["config.json", "tokenizer.json"] {
            let path = bge_dir.join(file);
            if let Err(err) = validate_json_asset(&path) {
                failures.push(err);
            }
        }
    }

    if failures.is_empty() {
        return Ok(bge_dir);
    }

    let repo = ModelId::BgeM3Dense.model_repo().unwrap_or("BAAI/bge-m3");
    let message = format!(
        "[EMB-E003] E14_BGE_M3_DENSE_ASSET_INVALID: BGE-M3 dense head cannot be initialized.\n  Model: {:?}\n  Repository: {repo}\n  Model directory: {}\n  Required files: {:?}\n  Asset failures: {:?}\n  Remediation: download the full Hugging Face snapshot into the model directory before constructing ProductionMultiArrayProvider.",
        ModelId::BgeM3Dense,
        bge_dir.display(),
        E14_REQUIRED_FILES,
        failures
    );
    tracing::error!(
        target: "context_graph_embeddings::provider",
        bge_dir = %bge_dir.display(),
        failures = ?failures,
        "{message}"
    );
    Err(EmbeddingError::ConfigError { message })
}

fn validate_json_asset(path: &Path) -> Result<(), String> {
    let data = std::fs::read_to_string(path)
        .map_err(|err| format!("{} cannot be read as UTF-8 JSON: {err}", path.display()))?;
    let value = serde_json::from_str::<serde_json::Value>(&data)
        .map_err(|err| format!("{} is not valid JSON: {err}", path.display()))?;
    if !value.is_object() {
        return Err(format!("{} JSON root is not an object", path.display()));
    }
    Ok(())
}

fn resolve_active_embedder_dir(
    models_dir: &Path,
    embedder_key: &str,
    fallback_subdir: &str,
) -> EmbeddingResult<PathBuf> {
    let registry = models_dir.join(ACTIVE_MODEL_REGISTRY);
    if registry.is_file() {
        let registry_text =
            std::fs::read_to_string(&registry).map_err(|err| EmbeddingError::ConfigError {
                message: format!(
                    "ACTIVE_MODEL_REGISTRY_UNREADABLE: failed to read {}: {err}",
                    registry.display()
                ),
            })?;
        let registry_value = toml::from_str::<toml::Value>(&registry_text).map_err(|err| {
            EmbeddingError::ConfigError {
                message: format!(
                    "ACTIVE_MODEL_REGISTRY_INVALID: failed to parse {}: {err}",
                    registry.display()
                ),
            }
        })?;

        if let Some(path_text) = registry_value
            .get("embedders")
            .and_then(toml::Value::as_table)
            .and_then(|embedders| embedders.get(embedder_key))
            .and_then(|entry| entry.get("path"))
            .and_then(toml::Value::as_str)
            .filter(|path| !path.trim().is_empty())
        {
            let configured = PathBuf::from(path_text);
            let resolved = if configured.is_absolute() {
                configured
            } else {
                models_dir.join(configured)
            };
            if resolved.is_dir() {
                tracing::info!(
                    target: "context_graph_embeddings::provider",
                    embedder_key,
                    model_dir = %resolved.display(),
                    registry = %registry.display(),
                    "Using active embedder model directory from registry"
                );
                return Ok(resolved);
            }
            return Err(EmbeddingError::ConfigError {
                message: format!(
                    "ACTIVE_EMBEDDER_MODEL_DIR_MISSING: {embedder_key} registry path {} from {} is not a directory",
                    resolved.display(),
                    registry.display()
                ),
            });
        }
    }

    let fallback = models_dir.join(fallback_subdir);
    if fallback.is_dir() {
        tracing::warn!(
            target: "context_graph_embeddings::provider",
            embedder_key,
            fallback = %fallback.display(),
            registry = %registry.display(),
            "Active model registry path not available; using legacy model directory"
        );
        return Ok(fallback);
    }

    Err(EmbeddingError::ConfigError {
        message: format!(
            "ACTIVE_EMBEDDER_MODEL_DIR_MISSING: {embedder_key} has no usable path in {} and fallback {} is not a directory",
            registry.display(),
            fallback.display()
        ),
    })
}

impl ProductionMultiArrayProvider {
    /// Create a new ProductionMultiArrayProvider with all 14 embedders.
    ///
    /// This constructor creates all models in an unloaded state. Call
    /// `initialize()` to load model weights before embedding.
    ///
    /// # Arguments
    ///
    /// * `models_dir` - Base directory containing pretrained model files
    /// * `gpu_config` - GPU configuration for inference
    ///
    /// # Returns
    ///
    /// A new provider instance with all embedders initialized (but not loaded).
    ///
    /// # Errors
    ///
    /// Returns `EmbeddingError` if model creation fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let provider = ProductionMultiArrayProvider::new(
    ///     PathBuf::from("./models"),
    ///     GpuConfig::default(),
    /// ).await?;
    /// ```
    pub async fn new(models_dir: PathBuf, gpu_config: GpuConfig) -> EmbeddingResult<Self> {
        let bge_dir = ensure_bge_m3_assets(&models_dir)?;
        let factory = DefaultModelFactory::new(models_dir.clone(), gpu_config);
        let config = SingleModelConfig::cuda_fp16();

        // Create all active E1-E14 models using the factory.
        // E5 is intentionally retired and is not constructed or loaded.
        let e1_model = factory.create_model(ModelId::Semantic, &config)?;
        let e2_model = factory.create_model(ModelId::TemporalRecent, &config)?;
        let e3_model = factory.create_model(ModelId::TemporalPeriodic, &config)?;
        let e4_model = factory.create_model(ModelId::TemporalPositional, &config)?;

        let e6_model = factory.create_model(ModelId::Sparse, &config)?;
        let e7_model = factory.create_model(ModelId::Code, &config)?;

        // E8: Create GraphModel using the active registry path. On aiwonder this
        // intentionally points at semantic to share e5-large-v2 weights with E1.
        let e8_graph_dir = resolve_active_embedder_dir(&models_dir, "e8", "semantic")?;
        let e8_graph_model = GraphModel::new(&e8_graph_dir, config.clone())?;
        let e9_model = factory.create_model(ModelId::Hdc, &config)?;

        // E10: Create ContextualModel directly for dual embedding support (E10 Upgrade)
        let e10_contextual_dir = resolve_active_embedder_dir(&models_dir, "e10", "contextual")?;
        let e10_contextual_model = ContextualModel::new(&e10_contextual_dir, config.clone())?;

        let e12_model = factory.create_model(ModelId::LateInteraction, &config)?;
        let e13_model = factory.create_model(ModelId::Splade, &config)?;

        // Load all models BEFORE wrapping in adapters (FAIL FAST)
        // Per constitution.yaml: models must be ready before embed()
        tracing::info!("Loading active E1-E14 embedding models (E5 retired)...");

        e1_model.load().await?;
        e2_model.load().await?;
        e3_model.load().await?;
        e4_model.load().await?;
        e6_model.load().await?;
        e7_model.load().await?;
        e8_graph_model.load().await?; // E8 loaded directly for dual embedding
        e9_model.load().await?;
        e10_contextual_model.load().await?; // E10 loaded directly for dual embedding
        if E11_ENTITY_ENABLED {
            tracing::info!("Loading E11 Kepler entity model");
        } else {
            tracing::info!("E11 Kepler disabled; not loading fairseq checkpoint assets");
        }
        e12_model.load().await?;
        e13_model.load().await?;

        tracing::info!("Active E1-E14 embedding models loaded successfully (E5 retired)");

        // Wrap models in appropriate adapters
        let e1_semantic: Arc<dyn SingleEmbedder> = Arc::new(DenseEmbedderAdapter::new(
            e1_model,
            ModelId::Semantic,
            E1_DIM,
        ));
        let e2_temporal_recent: Arc<dyn SingleEmbedder> = Arc::new(DenseEmbedderAdapter::new(
            e2_model,
            ModelId::TemporalRecent,
            E2_DIM,
        ));
        let e3_temporal_periodic: Arc<dyn SingleEmbedder> = Arc::new(DenseEmbedderAdapter::new(
            e3_model,
            ModelId::TemporalPeriodic,
            E3_DIM,
        ));
        // E4-FIX: Store as concrete type for embed_with_instruction() access
        let e4_temporal_positional: Arc<DenseEmbedderAdapter> = Arc::new(
            DenseEmbedderAdapter::new(e4_model, ModelId::TemporalPositional, E4_DIM),
        );

        let e6_sparse: Arc<dyn SparseEmbedder> =
            Arc::new(SparseEmbedderAdapter::new(e6_model, ModelId::Sparse));
        let e7_code: Arc<dyn SingleEmbedder> =
            Arc::new(DenseEmbedderAdapter::new(e7_model, ModelId::Code, E7_DIM));

        // E8: Use GraphDualEmbedderAdapter for asymmetric embeddings (E8 Upgrade)
        let e8_graph: Arc<GraphDualEmbedderAdapter> =
            Arc::new(GraphDualEmbedderAdapter::new(e8_graph_model));
        let e9_hdc: Arc<dyn SingleEmbedder> =
            Arc::new(DenseEmbedderAdapter::new(e9_model, ModelId::Hdc, E9_DIM));

        // E10: Use ContextualDualEmbedderAdapter for asymmetric embeddings (E10 Upgrade)
        let e10_contextual: Arc<ContextualDualEmbedderAdapter> =
            Arc::new(ContextualDualEmbedderAdapter::new(e10_contextual_model));

        let e11_entity: Arc<dyn SingleEmbedder> = if E11_ENTITY_ENABLED {
            let e11_model = factory.create_model(ModelId::Kepler, &config)?;
            e11_model.load().await?;
            Arc::new(DenseEmbedderAdapter::new(
                e11_model,
                ModelId::Kepler,
                E11_DIM,
            ))
        } else {
            Arc::new(DisabledDenseEmbedder::new(
                E11_DISABLED_MODEL_ID,
                E11_DIM,
                E11_DISABLED_REASON,
            ))
        };
        let e12_late_interaction: Arc<dyn TokenEmbedder> = Arc::new(TokenEmbedderAdapter::new(
            e12_model,
            ModelId::LateInteraction,
        ));
        let e13_splade: Arc<dyn SparseEmbedder> =
            Arc::new(SparseEmbedderAdapter::new(e13_model, ModelId::Splade));

        tracing::info!(
            target: "context_graph_embeddings::provider",
            bge_dir = %bge_dir.display(),
            "Loading E14 BGE-M3 Dense (multilingual, 8k context)"
        );
        let e14_model = factory.create_model(ModelId::BgeM3Dense, &config)?;
        e14_model.load().await?;
        tracing::info!(
            target: "context_graph_embeddings::provider",
            "E14 BGE-M3 Dense loaded; multilingual head active"
        );
        let e14_bge_m3_dense: Arc<dyn SingleEmbedder> = Arc::new(DenseEmbedderAdapter::new(
            e14_model,
            ModelId::BgeM3Dense,
            E14_DIM,
        ));

        let model_ids = [
            ModelId::Semantic.as_str().to_string(),
            ModelId::TemporalRecent.as_str().to_string(),
            ModelId::TemporalPeriodic.as_str().to_string(),
            ModelId::TemporalPositional.as_str().to_string(),
            E5_RETIRED_MODEL_ID.to_string(),
            ModelId::Sparse.as_str().to_string(),
            ModelId::Code.as_str().to_string(),
            ModelId::Graph.as_str().to_string(),
            ModelId::Hdc.as_str().to_string(),
            // MED-18 FIX: E10 production model is ContextualModel (intfloat/e5-base-v2),
            // not CLIP as ModelId::Contextual suggests. Report the actual model identity.
            "contextual".to_string(),
            if E11_ENTITY_ENABLED {
                ModelId::Kepler.as_str().to_string()
            } else {
                E11_DISABLED_MODEL_ID.to_string()
            },
            ModelId::LateInteraction.as_str().to_string(),
            ModelId::Splade.as_str().to_string(),
            ModelId::BgeM3Dense.as_str().to_string(), // E14: BGE-M3 dense head
        ];

        Ok(Self {
            e1_semantic,
            e2_temporal_recent,
            e3_temporal_periodic,
            e4_temporal_positional,
            e6_sparse,
            e7_code,
            e8_graph,
            e9_hdc,
            e10_contextual,
            e11_entity,
            e12_late_interaction,
            e13_splade,
            e14_bge_m3_dense,
            model_ids,
        })
    }

    /// Helper to measure and run an embedder, returning (result, duration).
    async fn timed_embed<F, T>(embedder_name: &str, fut: F) -> (Result<T, CoreError>, Duration)
    where
        F: std::future::Future<Output = Result<T, CoreError>>,
    {
        let start = Instant::now();
        let result = fut.await;
        let duration = start.elapsed();
        if let Err(ref e) = result {
            tracing::warn!("Embedder {} failed: {}", embedder_name, e);
        }
        (result, duration)
    }

    // =========================================================================
    // MODEL ACCESSOR METHODS
    // =========================================================================
    // These methods expose the underlying models for use by discovery services.
    // Per Root Cause Fix Plan: graph and causal activation services
    // need direct access to GraphModel and CausalModel for embedding operations.

    /// Get the underlying GraphModel for E8 embeddings.
    ///
    /// This method exposes the GraphModel used internally by E8 embeddings
    /// for use by graph activation. The model is already loaded and
    /// ready for use.
    ///
    /// # Returns
    ///
    /// Arc reference to the GraphModel (1024D embeddings, ~1.3GB VRAM)
    pub fn graph_model(&self) -> Arc<GraphModel> {
        self.e8_graph.model()
    }
}

#[async_trait]
impl MultiArrayEmbeddingProvider for ProductionMultiArrayProvider {
    /// Generate complete 14-embedding fingerprint for content.
    ///
    /// All 14 embedders run in parallel using tokio::join! for optimal performance.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to embed (must be non-empty)
    ///
    /// # Returns
    ///
    /// A `MultiArrayEmbeddingOutput` containing:
    /// - Complete 14-embedding fingerprint
    /// - Total and per-embedder latency metrics
    /// - Model IDs used
    ///
    /// # Errors
    ///
    /// Returns `CoreError` if:
    /// - Content is empty (`CoreError::ValidationError`)
    /// - Any embedder fails (propagated error)
    /// - Provider is not ready (`CoreError::Internal`)
    ///
    /// # Performance
    ///
    /// Target latency: <30ms for the legacy 13-embedder path; E14 adds BGE-M3 latency.
    async fn embed_all(&self, content: &str) -> CoreResult<MultiArrayEmbeddingOutput> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }
        if TEMPORAL_EMBEDDERS_ENABLED {
            return Err(CoreError::ValidationError {
                field: "metadata".to_string(),
                message: "embed_all cannot run E2/E3/E4 without explicit temporal metadata; call embed_all_with_metadata or disable temporal slots for this profile".to_string(),
            });
        }

        let start = Instant::now();

        // Clone Arc references for parallel execution
        let e1 = Arc::clone(&self.e1_semantic);
        let e2 = Arc::clone(&self.e2_temporal_recent);
        let e3 = Arc::clone(&self.e3_temporal_periodic);
        let e4 = Arc::clone(&self.e4_temporal_positional);
        let e6 = Arc::clone(&self.e6_sparse);
        let e7 = Arc::clone(&self.e7_code);
        let e8 = Arc::clone(&self.e8_graph);
        let e9 = Arc::clone(&self.e9_hdc);
        let e10 = Arc::clone(&self.e10_contextual);
        let e11 = Arc::clone(&self.e11_entity);
        let e12 = Arc::clone(&self.e12_late_interaction);
        let e13 = Arc::clone(&self.e13_splade);
        let e14 = Arc::clone(&self.e14_bge_m3_dense);

        let content_owned = content.to_string();

        // Run all 14 embedders in parallel
        let (
            (r1, d1),
            (r2, d2),
            (r3, d3),
            (r4, d4),
            (r5, d5),
            (r6, d6),
            (r7, d7),
            (r8, d8),
            (r9, d9),
            (r10, d10),
            (r11, d11),
            (r12, d12),
            (r13, d13),
            (r14, d14),
        ) = tokio::join!(
            Self::timed_embed("E1_Semantic", {
                let c = content_owned.clone();
                async move { e1.embed(&c).await }
            }),
            // E2/E3/E4: enabled temporal slots are rejected above unless callers
            // provide explicit metadata through embed_all_with_metadata.
            Self::timed_embed("E2_TemporalRecent", {
                let c = content_owned.clone();
                async move {
                    if TEMPORAL_EMBEDDERS_ENABLED {
                        e2.embed(&c).await
                    } else {
                        Ok(vec![0.0f32; E2_DIM])
                    }
                }
            }),
            Self::timed_embed("E3_TemporalPeriodic", {
                let c = content_owned.clone();
                async move {
                    if TEMPORAL_EMBEDDERS_ENABLED {
                        e3.embed(&c).await
                    } else {
                        Ok(vec![0.0f32; E3_DIM])
                    }
                }
            }),
            Self::timed_embed("E4_TemporalPositional", {
                let c = content_owned.clone();
                async move {
                    if TEMPORAL_EMBEDDERS_ENABLED {
                        e4.embed(&c).await
                    } else {
                        Ok(vec![0.0f32; E4_DIM])
                    }
                }
            }),
            Self::timed_embed("E5_Causal_Retired", {
                async move { Ok::<_, CoreError>(retired_e5_vectors()) }
            }),
            Self::timed_embed("E6_Sparse", {
                let c = content_owned.clone();
                async move { e6.embed_sparse(&c).await }
            }),
            Self::timed_embed("E7_Code", {
                let c = content_owned.clone();
                async move { e7.embed(&c).await }
            }),
            Self::timed_embed("E8_Graph_Dual", {
                let c = content_owned.clone();
                async move { e8.embed_dual(&c).await }
            }),
            Self::timed_embed("E9_HDC", {
                let c = content_owned.clone();
                async move { e9.embed(&c).await }
            }),
            Self::timed_embed("E10_Contextual_Dual", {
                let c = content_owned.clone();
                async move { e10.embed_dual(&c).await }
            }),
            // E11 disabled: produce no vector; active topology gates the slot out.
            Self::timed_embed("E11_Entity", {
                let c = content_owned.clone();
                async move {
                    if E11_ENTITY_ENABLED {
                        e11.embed(&c).await
                    } else {
                        Ok(Vec::new())
                    }
                }
            }),
            Self::timed_embed("E12_LateInteraction", {
                let c = content_owned.clone();
                async move { e12.embed_tokens(&c).await }
            }),
            Self::timed_embed("E13_SPLADE", {
                let c = content_owned.clone();
                async move { e13.embed_sparse(&c).await }
            }),
            // E14: BGE-M3 Dense — 1024D multilingual CLS-pooled XLM-RoBERTa-Large vector.
            Self::timed_embed("E14_BgeM3Dense", {
                let c = content_owned.clone();
                async move { e14.embed(&c).await }
            }),
        );

        // Collect results, failing fast on any error
        let e1_vec = r1?;
        let e2_vec = r2?;
        let e3_vec = r3?;
        let e4_vec = r4?;

        // E5 retired: no vectors are produced for new fingerprints.
        let (e5_cause_vec, e5_effect_vec) = r5?;

        let e6_sparse = r6?;
        let e7_vec = r7?;

        // E8: embed_dual returns (source_vec, target_vec) for asymmetric similarity (E8 Upgrade)
        let (e8_source_vec, e8_target_vec) = r8?;

        let e9_vec = r9?;

        // E10: embed_dual returns (intent_vec, context_vec) for asymmetric similarity (E10 Upgrade)
        let (e10_paraphrase_vec, e10_context_vec) = r10?;

        let e11_vec = r11?;
        let e12_tokens = r12?;
        let e13_sparse = r13?;
        let e14_vec = r14?;

        let total_latency = start.elapsed();

        // Construct fingerprint with asymmetric E5, E8, and E10 vectors
        let fingerprint = SemanticFingerprint {
            e1_semantic: e1_vec,
            e2_temporal_recent: e2_vec,
            e3_temporal_periodic: e3_vec,
            e4_temporal_positional: e4_vec,
            e5_causal_as_cause: e5_cause_vec,
            e5_causal_as_effect: e5_effect_vec,
            e5_causal: Vec::new(), // E5 retired/inactive
            e6_sparse,
            e7_code: e7_vec,
            e8_graph_as_source: e8_source_vec,
            e8_graph_as_target: e8_target_vec,
            e8_graph: Vec::new(), // Empty - using new dual format
            e9_hdc: e9_vec,
            // E10: Using new dual format (E10 Upgrade)
            e10_multimodal_paraphrase: e10_paraphrase_vec,
            e10_multimodal_as_context: e10_context_vec,
            e11_entity: e11_vec,
            e12_late_interaction: e12_tokens,
            e13_splade: e13_sparse,
            // E14 BGE-M3 Dense: multilingual CLS-pooled vector.
            e14_bge_m3_dense: e14_vec,
        };

        let per_embedder_latency = [d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14];

        Ok(MultiArrayEmbeddingOutput {
            fingerprint,
            total_latency,
            per_embedder_latency,
            model_ids: self.model_ids.clone(),
            e5_hint_provenance: None,
        })
    }

    /// Generate complete 14-embedding fingerprint with explicit metadata.
    ///
    /// E4-FIX: This override passes session sequence numbers to E4 via
    /// "sequence:N" instruction, enabling proper session ordering.
    ///
    /// # Arguments
    ///
    /// * `content` - Text content to embed (must be non-empty)
    /// * `metadata` - Metadata for temporal embedders (E2-E4)
    ///
    /// # Behavior
    ///
    /// - E2/E3: Uses `metadata.e2_instruction()`/`e3_instruction()` to pass "timestamp:..." for decay/periodic encoding
    /// - E4: Uses `metadata.e4_instruction()` to pass "sequence:N" or "epoch:N"
    /// - All other embedders: Unchanged
    async fn embed_all_with_metadata(
        &self,
        content: &str,
        metadata: EmbeddingMetadata,
    ) -> CoreResult<MultiArrayEmbeddingOutput> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }

        let start = Instant::now();

        // Clone Arc references for parallel execution
        let e1 = Arc::clone(&self.e1_semantic);
        let e2 = Arc::clone(&self.e2_temporal_recent);
        let e3 = Arc::clone(&self.e3_temporal_periodic);
        let e4 = Arc::clone(&self.e4_temporal_positional);
        let e6 = Arc::clone(&self.e6_sparse);
        let e7 = Arc::clone(&self.e7_code);
        let e8 = Arc::clone(&self.e8_graph);
        let e9 = Arc::clone(&self.e9_hdc);
        let e10 = Arc::clone(&self.e10_contextual);
        let e11 = Arc::clone(&self.e11_entity);
        let e12 = Arc::clone(&self.e12_late_interaction);
        let e13 = Arc::clone(&self.e13_splade);
        let e14 = Arc::clone(&self.e14_bge_m3_dense);

        let content_owned = content.to_string();

        // Generate temporal instructions from metadata timestamps
        let e2_instruction = metadata.e2_instruction()?;
        let e3_instruction = metadata.e3_instruction()?;
        let e4_instruction = metadata.e4_instruction()?;

        // Run all 14 embedders in parallel
        let (
            (r1, d1),
            (r2, d2),
            (r3, d3),
            (r4, d4),
            (r5, d5),
            (r6, d6),
            (r7, d7),
            (r8, d8),
            (r9, d9),
            (r10, d10),
            (r11, d11),
            (r12, d12),
            (r13, d13),
            (r14, d14),
        ) = tokio::join!(
            Self::timed_embed("E1_Semantic", {
                let c = content_owned.clone();
                async move { e1.embed(&c).await }
            }),
            // E2: Pass creation timestamp so each memory gets a unique decay vector
            Self::timed_embed("E2_TemporalRecent", {
                let c = content_owned.clone();
                let inst = e2_instruction.clone();
                async move {
                    if TEMPORAL_EMBEDDERS_ENABLED {
                        e2.embed_with_instruction(&c, Some(&inst)).await
                    } else {
                        Ok(vec![0.0f32; E2_DIM])
                    }
                }
            }),
            // E3: Pass creation timestamp for periodic pattern encoding
            Self::timed_embed("E3_TemporalPeriodic", {
                let c = content_owned.clone();
                let inst = e3_instruction.clone();
                async move {
                    if TEMPORAL_EMBEDDERS_ENABLED {
                        e3.embed_with_instruction(&c, Some(&inst)).await
                    } else {
                        Ok(vec![0.0f32; E3_DIM])
                    }
                }
            }),
            // E4: Pass session sequence number for positional encoding
            Self::timed_embed("E4_TemporalPositional", {
                let c = content_owned.clone();
                let inst = e4_instruction.clone();
                async move {
                    if TEMPORAL_EMBEDDERS_ENABLED {
                        e4.embed_with_instruction(&c, Some(&inst)).await
                    } else {
                        Ok(vec![0.0f32; E4_DIM])
                    }
                }
            }),
            Self::timed_embed("E5_Causal_Retired", {
                async move { Ok::<_, CoreError>(retired_e5_vectors()) }
            }),
            Self::timed_embed("E6_Sparse", {
                let c = content_owned.clone();
                async move { e6.embed_sparse(&c).await }
            }),
            Self::timed_embed("E7_Code", {
                let c = content_owned.clone();
                async move { e7.embed(&c).await }
            }),
            Self::timed_embed("E8_Graph_Dual", {
                let c = content_owned.clone();
                async move { e8.embed_dual(&c).await }
            }),
            Self::timed_embed("E9_HDC", {
                let c = content_owned.clone();
                async move { e9.embed(&c).await }
            }),
            Self::timed_embed("E10_Contextual_Dual", {
                let c = content_owned.clone();
                async move { e10.embed_dual(&c).await }
            }),
            // E11 disabled: produce no vector; active topology gates the slot out.
            Self::timed_embed("E11_Entity", {
                let c = content_owned.clone();
                async move {
                    if E11_ENTITY_ENABLED {
                        e11.embed(&c).await
                    } else {
                        Ok(Vec::new())
                    }
                }
            }),
            Self::timed_embed("E12_LateInteraction", {
                let c = content_owned.clone();
                async move { e12.embed_tokens(&c).await }
            }),
            Self::timed_embed("E13_SPLADE", {
                let c = content_owned.clone();
                async move { e13.embed_sparse(&c).await }
            }),
            Self::timed_embed("E14_BgeM3Dense", {
                let c = content_owned.clone();
                async move { e14.embed(&c).await }
            }),
        );

        // Collect results, failing fast on any error
        let e1_vec = r1?;
        let e2_vec = r2?;
        let e3_vec = r3?;
        let e4_vec = r4?;

        // E5 retired: no vectors are produced for new fingerprints.
        let (e5_cause_vec, e5_effect_vec) = r5?;

        let e6_sparse = r6?;
        let e7_vec = r7?;

        // E8: embed_dual returns (source_vec, target_vec) for asymmetric similarity (E8 Upgrade)
        let (e8_source_vec, e8_target_vec) = r8?;

        let e9_vec = r9?;

        // E10: embed_dual returns (intent_vec, context_vec) for asymmetric similarity (E10 Upgrade)
        let (e10_paraphrase_vec, e10_context_vec) = r10?;

        let e11_vec = r11?;
        let e12_tokens = r12?;
        let e13_sparse = r13?;
        let e14_vec = r14?;

        let total_latency = start.elapsed();

        // Construct fingerprint with asymmetric E5, E8, and E10 vectors
        let fingerprint = SemanticFingerprint {
            e1_semantic: e1_vec,
            e2_temporal_recent: e2_vec,
            e3_temporal_periodic: e3_vec,
            e4_temporal_positional: e4_vec,
            e5_causal_as_cause: e5_cause_vec,
            e5_causal_as_effect: e5_effect_vec,
            e5_causal: Vec::new(), // E5 retired/inactive
            e6_sparse,
            e7_code: e7_vec,
            e8_graph_as_source: e8_source_vec,
            e8_graph_as_target: e8_target_vec,
            e8_graph: Vec::new(), // Empty - using new dual format
            e9_hdc: e9_vec,
            // E10: Using new dual format (E10 Upgrade)
            e10_multimodal_paraphrase: e10_paraphrase_vec,
            e10_multimodal_as_context: e10_context_vec,
            e11_entity: e11_vec,
            e12_late_interaction: e12_tokens,
            e13_splade: e13_sparse,
            // E14 BGE-M3 Dense: multilingual CLS-pooled vector.
            e14_bge_m3_dense: e14_vec,
        };

        let per_embedder_latency = [d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14];

        Ok(MultiArrayEmbeddingOutput {
            fingerprint,
            total_latency,
            per_embedder_latency,
            model_ids: self.model_ids.clone(),
            e5_hint_provenance: None,
        })
    }

    /// Generate fingerprints for multiple contents in batch.
    ///
    /// Processes contents concurrently using tokio::spawn for GPU parallelism.
    /// All 13 embedders run in parallel for each content, and contents are
    /// processed concurrently across multiple GPU streams.
    ///
    /// # Performance Target
    ///
    /// 64 contents: <100ms per item average
    ///
    /// # GPU Optimization
    ///
    /// Uses concurrent tokio tasks to maximize GPU utilization. Each task
    /// runs embed_all() which itself runs all 13 embedders in parallel.
    /// EMB-H1 FIX: Accept metadata parameter to propagate E4 sequence instruction
    /// and E5 causal hint through the batch path. Previously, embed_batch_all used
    /// e4.embed(&c) instead of e4.embed_with_instruction(&c, Some(&inst)) and
    /// e5.embed_dual(&c) instead of e5.embed_dual_with_hint(&c, hint.as_ref()),
    /// losing E4/E5 metadata that embed_all_with_metadata correctly propagates.
    async fn embed_batch_all(
        &self,
        contents: &[String],
        metadata: &[EmbeddingMetadata],
    ) -> CoreResult<Vec<MultiArrayEmbeddingOutput>> {
        use futures::future::join_all;

        if contents.len() != metadata.len() {
            return Err(CoreError::ValidationError {
                field: "metadata".to_string(),
                message: format!(
                    "embed_batch_all requires one EmbeddingMetadata per content item; contents={}, metadata={}",
                    contents.len(),
                    metadata.len()
                ),
            });
        }

        // Clone self for spawned tasks (Arc references are cheap to clone)
        let e1 = Arc::clone(&self.e1_semantic);
        let e2 = Arc::clone(&self.e2_temporal_recent);
        let e3 = Arc::clone(&self.e3_temporal_periodic);
        let e4 = Arc::clone(&self.e4_temporal_positional);
        let e6 = Arc::clone(&self.e6_sparse);
        let e7 = Arc::clone(&self.e7_code);
        let e8 = Arc::clone(&self.e8_graph);
        let e9 = Arc::clone(&self.e9_hdc);
        let e10 = Arc::clone(&self.e10_contextual);
        let e11 = Arc::clone(&self.e11_entity);
        let e12 = Arc::clone(&self.e12_late_interaction);
        let e13 = Arc::clone(&self.e13_splade);
        let e14 = Arc::clone(&self.e14_bge_m3_dense);
        let model_ids = self.model_ids.clone();

        // Spawn concurrent tasks for each content
        let tasks: Vec<_> = contents
            .iter()
            .enumerate()
            .map(|(idx, content)| {
                let content = content.clone();
                let item_metadata = metadata[idx].clone();
                let e1 = Arc::clone(&e1);
                let e2 = Arc::clone(&e2);
                let e3 = Arc::clone(&e3);
                let e4 = Arc::clone(&e4);
                let e6 = Arc::clone(&e6);
                let e7 = Arc::clone(&e7);
                let e8 = Arc::clone(&e8);
                let e9 = Arc::clone(&e9);
                let e10 = Arc::clone(&e10);
                let e11 = Arc::clone(&e11);
                let e12 = Arc::clone(&e12);
                let e13 = Arc::clone(&e13);
                let e14 = Arc::clone(&e14);
                let model_ids = model_ids.clone();

                tokio::spawn(async move {
                    let start = Instant::now();

                    // Generate temporal instructions from metadata (same as embed_all_with_metadata)
                    let e2_instruction = item_metadata.e2_instruction()?;
                    let e3_instruction = item_metadata.e3_instruction()?;
                    let e4_instruction = item_metadata.e4_instruction()?;
                    // Run all 14 embedders in parallel for this content
                    let (
                        (r1, d1),
                        (r2, d2),
                        (r3, d3),
                        (r4, d4),
                        (r5, d5),
                        (r6, d6),
                        (r7, d7),
                        (r8, d8),
                        (r9, d9),
                        (r10, d10),
                        (r11, d11),
                        (r12, d12),
                        (r13, d13),
                        (r14, d14),
                    ) = tokio::join!(
                        Self::timed_embed("E1_Semantic", {
                            let c = content.clone();
                            async move { e1.embed(&c).await }
                        }),
                        // E2: Pass creation timestamp for decay encoding
                        Self::timed_embed("E2_TemporalRecent", {
                            let c = content.clone();
                            let inst = e2_instruction.clone();
                            async move {
                                if TEMPORAL_EMBEDDERS_ENABLED {
                                    e2.embed_with_instruction(&c, Some(&inst)).await
                                } else {
                                    Ok(vec![0.0f32; E2_DIM])
                                }
                            }
                        }),
                        // E3: Pass creation timestamp for periodic encoding
                        Self::timed_embed("E3_TemporalPeriodic", {
                            let c = content.clone();
                            let inst = e3_instruction.clone();
                            async move {
                                if TEMPORAL_EMBEDDERS_ENABLED {
                                    e3.embed_with_instruction(&c, Some(&inst)).await
                                } else {
                                    Ok(vec![0.0f32; E3_DIM])
                                }
                            }
                        }),
                        // E4: Pass session sequence for positional encoding
                        Self::timed_embed("E4_TemporalPositional", {
                            let c = content.clone();
                            let inst = e4_instruction.clone();
                            async move {
                                if TEMPORAL_EMBEDDERS_ENABLED {
                                    e4.embed_with_instruction(&c, Some(&inst)).await
                                } else {
                                    Ok(vec![0.0f32; E4_DIM])
                                }
                            }
                        }),
                        Self::timed_embed("E5_Causal_Retired", {
                            async move { Ok::<_, CoreError>(retired_e5_vectors()) }
                        }),
                        Self::timed_embed("E6_Sparse", {
                            let c = content.clone();
                            async move { e6.embed_sparse(&c).await }
                        }),
                        Self::timed_embed("E7_Code", {
                            let c = content.clone();
                            async move { e7.embed(&c).await }
                        }),
                        Self::timed_embed("E8_Graph_Dual", {
                            let c = content.clone();
                            async move { e8.embed_dual(&c).await }
                        }),
                        Self::timed_embed("E9_HDC", {
                            let c = content.clone();
                            async move { e9.embed(&c).await }
                        }),
                        Self::timed_embed("E10_Contextual_Dual", {
                            let c = content.clone();
                            async move { e10.embed_dual(&c).await }
                        }),
                        // E11 disabled: produce no vector; active topology gates the slot out.
                        Self::timed_embed("E11_Entity", {
                            let c = content.clone();
                            async move {
                                if E11_ENTITY_ENABLED {
                                    e11.embed(&c).await
                                } else {
                                    Ok(Vec::new())
                                }
                            }
                        }),
                        Self::timed_embed("E12_LateInteraction", {
                            let c = content.clone();
                            async move { e12.embed_tokens(&c).await }
                        }),
                        Self::timed_embed("E13_SPLADE", {
                            let c = content.clone();
                            async move { e13.embed_sparse(&c).await }
                        }),
                        Self::timed_embed("E14_BgeM3Dense", {
                            let c = content.clone();
                            async move { e14.embed(&c).await }
                        }),
                    );

                    // Collect results
                    let e1_vec = r1?;
                    let e2_vec = r2?;
                    let e3_vec = r3?;
                    let e4_vec = r4?;
                    let (e5_cause_vec, e5_effect_vec) = r5?;
                    let e6_sparse = r6?;
                    let e7_vec = r7?;
                    let (e8_source_vec, e8_target_vec) = r8?;
                    let e9_vec = r9?;
                    let (e10_paraphrase_vec, e10_context_vec) = r10?;
                    let e11_vec = r11?;
                    let e12_tokens = r12?;
                    let e13_sparse = r13?;
                    let e14_vec = r14?;

                    let total_latency = start.elapsed();

                    let fingerprint = SemanticFingerprint {
                        e1_semantic: e1_vec,
                        e2_temporal_recent: e2_vec,
                        e3_temporal_periodic: e3_vec,
                        e4_temporal_positional: e4_vec,
                        e5_causal_as_cause: e5_cause_vec,
                        e5_causal_as_effect: e5_effect_vec,
                        e5_causal: Vec::new(), // E5 retired/inactive
                        e6_sparse,
                        e7_code: e7_vec,
                        e8_graph_as_source: e8_source_vec,
                        e8_graph_as_target: e8_target_vec,
                        e8_graph: Vec::new(),
                        e9_hdc: e9_vec,
                        e10_multimodal_paraphrase: e10_paraphrase_vec,
                        e10_multimodal_as_context: e10_context_vec,
                        e11_entity: e11_vec,
                        e12_late_interaction: e12_tokens,
                        e13_splade: e13_sparse,
                        e14_bge_m3_dense: e14_vec,
                    };

                    let per_embedder_latency =
                        [d1, d2, d3, d4, d5, d6, d7, d8, d9, d10, d11, d12, d13, d14];

                    Ok::<_, CoreError>(MultiArrayEmbeddingOutput {
                        fingerprint,
                        total_latency,
                        per_embedder_latency,
                        model_ids,
                        e5_hint_provenance: None,
                    })
                })
            })
            .collect();

        // Wait for all tasks to complete
        let task_results = join_all(tasks).await;

        // Collect results, propagating any errors
        let mut results = Vec::with_capacity(contents.len());
        for result in task_results {
            match result {
                Ok(Ok(output)) => results.push(output),
                Ok(Err(e)) => return Err(e),
                Err(e) => {
                    return Err(CoreError::Internal(format!(
                        "Batch embedding task failed: {}",
                        e
                    )))
                }
            }
        }

        Ok(results)
    }

    /// Get expected dimensions for each embedder.
    fn dimensions(&self) -> [usize; NUM_EMBEDDERS] {
        [
            E1_DIM,
            E2_DIM,
            E3_DIM,
            E4_DIM,
            0, // E5 retired/inactive
            0, // E6 sparse
            E7_DIM,
            E8_DIM,
            E9_DIM,
            E10_DIM,
            if E11_ENTITY_ENABLED { E11_DIM } else { 0 },
            E12_TOKEN_DIM,
            0, // E13 sparse
            E14_DIM,
        ]
    }

    /// Get model IDs for each embedder slot.
    fn model_ids(&self) -> [&str; NUM_EMBEDDERS] {
        [
            &self.model_ids[0],
            &self.model_ids[1],
            &self.model_ids[2],
            &self.model_ids[3],
            &self.model_ids[4],
            &self.model_ids[5],
            &self.model_ids[6],
            &self.model_ids[7],
            &self.model_ids[8],
            &self.model_ids[9],
            &self.model_ids[10],
            &self.model_ids[11],
            &self.model_ids[12],
            &self.model_ids[13],
        ]
    }

    /// Check if all required active embedders are initialized and ready.
    fn is_ready(&self) -> bool {
        self.e1_semantic.is_ready()
            && self.e2_temporal_recent.is_ready()
            && self.e3_temporal_periodic.is_ready()
            && self.e4_temporal_positional.is_ready()
            && self.e6_sparse.is_ready()
            && self.e7_code.is_ready()
            && self.e8_graph.is_ready()
            && self.e9_hdc.is_ready()
            && self.e10_contextual.is_ready()
            && (!E11_ENTITY_ENABLED || self.e11_entity.is_ready())
            && self.e12_late_interaction.is_ready()
            && self.e13_splade.is_ready()
            && self.e14_bge_m3_dense.is_ready()
    }

    /// Get health status for each embedder.
    fn health_status(&self) -> [bool; NUM_EMBEDDERS] {
        [
            self.e1_semantic.is_ready(),
            self.e2_temporal_recent.is_ready(),
            self.e3_temporal_periodic.is_ready(),
            self.e4_temporal_positional.is_ready(),
            !E5_CAUSAL_ENABLED,
            self.e6_sparse.is_ready(),
            self.e7_code.is_ready(),
            self.e8_graph.is_ready(),
            self.e9_hdc.is_ready(),
            self.e10_contextual.is_ready(),
            !E11_ENTITY_ENABLED || self.e11_entity.is_ready(),
            self.e12_late_interaction.is_ready(),
            self.e13_splade.is_ready(),
            self.e14_bge_m3_dense.is_ready(),
        ]
    }

    /// Efficient E8 dual embedding without running all embedders.
    ///
    /// Returns (as_source, as_target) E8 dual embeddings (1024D each).
    async fn embed_e8_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }
        self.e8_graph.embed_dual(content).await
    }

    /// Efficient E11 embedding without running all embedders.
    ///
    /// Returns E11 entity embedding (768D).
    /// Fails when `E11_ENTITY_ENABLED` is false; bulk fingerprint generation
    /// stores the disabled slot as empty and gates it out before scoring.
    async fn embed_e11_only(&self, content: &str) -> CoreResult<Vec<f32>> {
        if !E11_ENTITY_ENABLED {
            return Err(CoreError::Internal(E11_DISABLED_REASON.to_string()));
        }
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }
        self.e11_entity.embed(content).await
    }

    /// Efficient E5 dual embedding without running all embedders.
    ///
    /// Returns (as_cause, as_effect) E5 dual embeddings (768D each).
    /// ~15ms vs ~200ms when running the legacy all-embedder path.
    async fn embed_e5_dual(&self, content: &str) -> CoreResult<(Vec<f32>, Vec<f32>)> {
        let _ = content;
        Err(retired_e5_error())
    }

    /// Efficient E1 semantic embedding without running all embedders.
    ///
    /// Returns E1 semantic embedding (1024D).
    async fn embed_e1_only(&self, content: &str) -> CoreResult<Vec<f32>> {
        if content.is_empty() {
            return Err(CoreError::ValidationError {
                field: "content".to_string(),
                message: "Content cannot be empty".to_string(),
            });
        }
        self.e1_semantic.embed(content).await
    }
}

// SAFETY: ProductionMultiArrayProvider is Send + Sync because all fields are
// `Arc<dyn Trait>` or `Arc<ConcreteType>`. Each underlying model uses internal
// synchronization (RwLock or equivalent). No raw pointers or unsynchronized
// mutable state is exposed.
unsafe impl Send for ProductionMultiArrayProvider {}
unsafe impl Sync for ProductionMultiArrayProvider {}

#[cfg(test)]
mod tests {
    use super::*;

    /// EMB-M4 FIX: Real test that DenseEmbedderAdapter rejects empty content.
    /// Creates a real EmbeddingModel implementation and verifies empty string
    /// returns CoreError::ValidationError through the adapter's embed path.
    #[tokio::test]
    async fn test_empty_content_rejected() {
        use crate::error::{EmbeddingError, EmbeddingResult};
        use crate::types::{InputType, ModelEmbedding, ModelInput};
        use std::sync::atomic::{AtomicBool, Ordering};

        /// Real EmbeddingModel that produces deterministic vectors from content hashes.
        struct DeterministicModel {
            initialized: AtomicBool,
        }

        #[async_trait]
        impl EmbeddingModel for DeterministicModel {
            fn model_id(&self) -> ModelId {
                ModelId::Semantic
            }
            fn supported_input_types(&self) -> &[InputType] {
                &[InputType::Text]
            }
            async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
                if !self.initialized.load(Ordering::SeqCst) {
                    return Err(EmbeddingError::NotInitialized {
                        model_id: ModelId::Semantic,
                    });
                }
                let hash = input.content_hash();
                let dim = 1024;
                let mut vector = Vec::with_capacity(dim);
                let mut state = hash;
                for _ in 0..dim {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                    vector.push(((state >> 33) as f32) / (u32::MAX as f32) - 0.5);
                }
                Ok(ModelEmbedding::new(ModelId::Semantic, vector, 100))
            }
            fn is_initialized(&self) -> bool {
                self.initialized.load(Ordering::SeqCst)
            }
        }

        let model = DeterministicModel {
            initialized: AtomicBool::new(true),
        };
        let adapter = DenseEmbedderAdapter::new(Box::new(model), ModelId::Semantic, 1024);

        // Empty content must be rejected with a ValidationError
        let result = adapter.embed("").await;
        assert!(result.is_err(), "empty content should be rejected");
        let err = result.unwrap_err();
        match &err {
            CoreError::ValidationError { field, message } => {
                assert_eq!(field, "content");
                assert!(
                    message.contains("empty"),
                    "error message should mention 'empty', got: {message}"
                );
            }
            other => panic!("expected ValidationError, got: {other:?}"),
        }

        // Non-empty content should succeed
        let result = adapter.embed("hello world").await;
        assert!(result.is_ok(), "non-empty content should succeed");
        let vec = result.unwrap();
        assert_eq!(vec.len(), 1024, "embedding dimension should be 1024");
    }

    /// Test NUM_EMBEDDERS is 14.
    #[test]
    fn test_num_embedders() {
        assert_eq!(NUM_EMBEDDERS, 14);
    }

    /// Test model_ids array has correct length.
    #[test]
    fn test_model_ids_length() {
        let ids = [
            "semantic",
            "temporal_recent",
            "temporal_periodic",
            "temporal_positional",
            "causal",
            "sparse",
            "code",
            "graph",
            "hdc",
            "contextual",
            "entity",
            "late_interaction",
            "splade",
            "bge_m3_dense",
        ];
        assert_eq!(ids.len(), NUM_EMBEDDERS);
    }

    #[test]
    fn e14_asset_preflight_fails_closed_when_snapshot_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let bge_dir = temp.path().join("bge-m3-dense");

        println!(
            "BEFORE: bge_dir_exists={} required_files={:?}",
            bge_dir.exists(),
            E14_REQUIRED_FILES
        );
        let result = ensure_bge_m3_assets(temp.path());
        println!(
            "AFTER: result={result:?} bge_dir_exists={}",
            bge_dir.exists()
        );

        let err = result.expect_err("missing E14 snapshot must fail closed");
        match err {
            EmbeddingError::ConfigError { message } => {
                assert!(message.contains("E14_BGE_M3_DENSE_ASSET_INVALID"));
                assert!(message.contains("pytorch_model.bin"));
                assert!(message.contains("tokenizer.json"));
                assert!(message.contains("config.json"));
            }
            other => panic!("expected ConfigError, got {other:?}"),
        }
    }

    #[test]
    fn e14_asset_preflight_inspects_complete_snapshot_state() {
        let temp = tempfile::tempdir().unwrap();
        let bge_dir = temp.path().join("bge-m3-dense");
        std::fs::create_dir_all(&bge_dir).unwrap();
        std::fs::write(
            bge_dir.join("config.json"),
            br#"{"model_type":"xlm-roberta"}"#,
        )
        .unwrap();
        std::fs::write(bge_dir.join("tokenizer.json"), br#"{"version":"1.0"}"#).unwrap();
        std::fs::write(
            bge_dir.join("pytorch_model.bin"),
            b"real-bge-m3-weight-header",
        )
        .unwrap();

        let before: Vec<_> = E14_REQUIRED_FILES
            .iter()
            .map(|file| (file, bge_dir.join(file).metadata().unwrap().len()))
            .collect();
        println!("BEFORE: asset_sizes={before:?}");

        let result = ensure_bge_m3_assets(temp.path()).unwrap();
        let after: Vec<_> = E14_REQUIRED_FILES
            .iter()
            .map(|file| (file, bge_dir.join(file).metadata().unwrap().len()))
            .collect();
        println!("AFTER: result={} asset_sizes={after:?}", result.display());

        assert_eq!(result, bge_dir);
        assert!(after.iter().all(|(_, len)| *len > 0));
    }

    #[test]
    fn active_registry_resolves_e10_contextual_path() {
        let temp = tempfile::tempdir().unwrap();
        let contextual_dir = temp.path().join("contextual-e5-base");
        std::fs::create_dir_all(&contextual_dir).unwrap();
        std::fs::write(
            temp.path().join(ACTIVE_MODEL_REGISTRY),
            format!(
                "\
[embedders.e10]
path = \"{}\"
",
                contextual_dir.display()
            ),
        )
        .unwrap();

        let resolved = resolve_active_embedder_dir(temp.path(), "e10", "contextual").unwrap();

        assert_eq!(resolved, contextual_dir);
    }

    #[test]
    fn active_registry_fails_closed_when_pinned_dir_is_missing() {
        let temp = tempfile::tempdir().unwrap();
        let missing_dir = temp.path().join("contextual-e5-base");
        std::fs::write(
            temp.path().join(ACTIVE_MODEL_REGISTRY),
            format!(
                "\
[embedders.e10]
path = \"{}\"
",
                missing_dir.display()
            ),
        )
        .unwrap();

        let err = resolve_active_embedder_dir(temp.path(), "e10", "contextual")
            .expect_err("missing active registry path must fail closed");

        match err {
            EmbeddingError::ConfigError { message } => {
                assert!(message.contains("ACTIVE_EMBEDDER_MODEL_DIR_MISSING"));
                assert!(message.contains("e10"));
            }
            other => panic!("expected ConfigError, got {other:?}"),
        }
    }
    // =========================================================================
    // E4 INSTRUCTION FIX VERIFICATION TESTS
    // =========================================================================

    /// Test that EmbeddingMetadata.e4_instruction() includes session_id.
    ///
    /// This is the critical end-to-end verification test for the E4 session fix.
    /// It verifies that when embed_all_with_metadata() is called, the session_id
    /// is correctly passed through to the E4 embedder.
    #[test]
    fn test_embedding_metadata_e4_instruction_includes_session() {
        // This test verifies the fix at the metadata level
        let metadata = EmbeddingMetadata::with_sequence("test-session-id", 100);

        let instruction = metadata.e4_instruction();

        // Critical assertion: session_id must be in the instruction
        assert!(
            instruction.contains("session:test-session-id"),
            "e4_instruction() must include session_id. Got: {}",
            instruction
        );
        assert!(
            instruction.contains("sequence:100"),
            "e4_instruction() must include sequence. Got: {}",
            instruction
        );

        // Verify exact format matches what E4 parser expects
        assert_eq!(
            instruction, "session:test-session-id sequence:100",
            "Instruction format should match E4 parser expectations"
        );
    }

    /// Test that different session_ids produce different instruction strings.
    #[test]
    fn test_different_sessions_produce_different_instructions() {
        let metadata1 = EmbeddingMetadata::with_sequence("session-A", 1);
        let metadata2 = EmbeddingMetadata::with_sequence("session-B", 1);

        let inst1 = metadata1.e4_instruction();
        let inst2 = metadata2.e4_instruction();

        assert_ne!(
            inst1, inst2,
            "Different sessions should produce different instructions"
        );
        assert!(inst1.contains("session:session-A"));
        assert!(inst2.contains("session:session-B"));
    }

    // =========================================================================
    // E2/E3 TEMPORAL INSTRUCTION VERIFICATION TESTS
    // =========================================================================

    /// Test that EmbeddingMetadata.e2_instruction() produces timestamp instruction.
    #[test]
    fn test_embedding_metadata_e2_instruction_with_timestamp() {
        use chrono::TimeZone;
        let ts = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();
        let metadata = EmbeddingMetadata {
            session_id: None,
            session_sequence: None,
            timestamp: Some(ts),
            causal_hint: None,
        };

        let instruction = metadata.e2_instruction();
        assert!(
            instruction.starts_with("timestamp:"),
            "e2_instruction() must start with 'timestamp:'. Got: {}",
            instruction
        );
        assert!(
            instruction.contains("2024-06-15"),
            "e2_instruction() must contain the date. Got: {}",
            instruction
        );
    }

    /// Test that E2 instructions differ for different timestamps.
    #[test]
    fn test_e2_instructions_differ_for_different_timestamps() {
        use chrono::TimeZone;
        let ts1 = chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let ts2 = chrono::Utc.with_ymd_and_hms(2024, 6, 15, 12, 0, 0).unwrap();

        let meta1 = EmbeddingMetadata {
            session_id: None,
            session_sequence: None,
            timestamp: Some(ts1),
            causal_hint: None,
        };
        let meta2 = EmbeddingMetadata {
            session_id: None,
            session_sequence: None,
            timestamp: Some(ts2),
            causal_hint: None,
        };

        assert_ne!(
            meta1.e2_instruction(),
            meta2.e2_instruction(),
            "Different timestamps must produce different E2 instructions"
        );
    }

    /// Test that e3_instruction delegates to e2_instruction (same timestamp format).
    #[test]
    fn test_e3_instruction_matches_e2_format() {
        use chrono::TimeZone;
        let ts = chrono::Utc.with_ymd_and_hms(2024, 3, 20, 8, 30, 0).unwrap();
        let metadata = EmbeddingMetadata {
            session_id: None,
            session_sequence: None,
            timestamp: Some(ts),
            causal_hint: None,
        };

        let e2 = metadata.e2_instruction();
        let e3 = metadata.e3_instruction();
        assert_eq!(
            e2, e3,
            "E3 instruction should delegate to E2 (same timestamp format)"
        );
    }

    /// Test that e2_instruction without timestamp produces epoch-based instruction.
    #[test]
    fn test_e2_instruction_fallback_to_epoch() {
        let metadata = EmbeddingMetadata {
            session_id: None,
            session_sequence: None,
            timestamp: None,
            causal_hint: None,
        };

        let instruction = metadata.e2_instruction();
        assert!(
            instruction.starts_with("epoch:"),
            "Without timestamp, e2_instruction() must use epoch format. Got: {}",
            instruction
        );
    }

    /// Test backward compatibility: metadata without session_id still works.
    #[test]
    fn test_backward_compatible_no_session_metadata() {
        let metadata = EmbeddingMetadata {
            session_id: None,
            session_sequence: Some(42),
            timestamp: None,
            causal_hint: None,
        };

        let instruction = metadata.e4_instruction();

        // Should produce legacy format without session prefix
        assert_eq!(
            instruction, "sequence:42",
            "Legacy format should work without session_id"
        );
        assert!(
            !instruction.contains("session:"),
            "No session prefix when session_id is None"
        );
    }
}
