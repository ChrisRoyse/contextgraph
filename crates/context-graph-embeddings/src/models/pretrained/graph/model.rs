//! Core GraphModel struct and lifecycle management.
//!
//! # Asymmetric Dual Embeddings
//!
//! Following the E5 Causal pattern (ARCH-15), this model supports asymmetric
//! source/target embeddings via `embed_dual()`:
//!
//! - **Source embedding**: Represents the entity as a source of outgoing relationships
//!   (e.g., "Module A imports B, C, D")
//! - **Target embedding**: Represents the entity as a target of incoming relationships
//!   (e.g., "Module X is imported by A, B, C")

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use candle_core::Tensor;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader};
use crate::traits::{EmbeddingModel, SingleModelConfig};
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::constants::GRAPH_DIMENSION;
use super::encoding::{encode_context, encode_relation};
use super::forward::gpu_forward;
use super::projections::{GraphProjectionWeights, GRAPH_PROJECTION_SEED};
use super::state::ModelState;

/// Graph embedding model using sentence-transformers/paraphrase-MiniLM-L6-v2.
///
/// Produces 384D vectors optimized for knowledge graph embeddings,
/// relation encoding, and graph structure understanding.
///
/// MiniLM is a distilled BERT model optimized for speed while maintaining
/// good semantic understanding. The paraphrase variant is trained on
/// paraphrase and semantic similarity data.
///
/// # Example
/// ```rust,no_run
/// use context_graph_embeddings::models::GraphModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = GraphModel::new(Path::new("models/graph"), SingleModelConfig::default())?;
///     model.load().await?;
///     let relation_text = GraphModel::encode_relation("Alice", "works_at", "Anthropic");
///     Ok(())
/// }
/// ```
pub struct GraphModel {
    #[allow(dead_code)]
    pub(crate) model_state: std::sync::RwLock<ModelState>,
    #[allow(dead_code)]
    pub(crate) model_path: PathBuf,
    #[allow(dead_code)]
    pub(crate) config: SingleModelConfig,
    pub(crate) loaded: AtomicBool,
    #[allow(dead_code)]
    pub(crate) memory_size: usize,
}

impl GraphModel {
    /// Create a new GraphModel instance. Call `load()` before `embed()`.
    pub fn new(model_path: &Path, config: SingleModelConfig) -> EmbeddingResult<Self> {
        if config.max_batch_size == 0 {
            return Err(EmbeddingError::ConfigError {
                message: "max_batch_size cannot be zero".to_string(),
            });
        }
        Ok(Self {
            model_state: std::sync::RwLock::new(ModelState::Unloaded),
            model_path: model_path.to_path_buf(),
            config,
            loaded: AtomicBool::new(false),
            memory_size: 0,
        })
    }

    /// Encode a relation triple into a text string for embedding.
    pub fn encode_relation(subject: &str, predicate: &str, object: &str) -> String {
        encode_relation(subject, predicate, object)
    }

    /// Encode a node with its neighboring relations into a context string.
    pub fn encode_context(node: &str, neighbors: &[(String, String)]) -> String {
        encode_context(node, neighbors)
    }

    /// Load model weights into memory.
    ///
    /// Initializes CUDA device, loads tokenizer.json and model.safetensors,
    /// and transfers weight tensors to GPU VRAM.
    pub async fn load(&self) -> EmbeddingResult<()> {
        let _device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("GraphModel GPU init failed: {}", e),
        })?;

        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Graph,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "GraphModel tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            }
        })?;

        let loader = GpuModelLoader::new().map_err(|e| EmbeddingError::GpuError {
            message: format!("GraphModel loader init failed: {}", e),
        })?;

        let weights = loader.load_bert_weights(&self.model_path).map_err(|e| {
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Graph,
                source: Box::new(std::io::Error::other(format!(
                    "GraphModel weight load failed: {}",
                    e
                ))),
            }
        })?;

        if weights.config.hidden_size != GRAPH_DIMENSION {
            return Err(EmbeddingError::InvalidDimension {
                expected: GRAPH_DIMENSION,
                actual: weights.config.hidden_size,
            });
        }

        // Initialize graph projection weights for asymmetric source/target embeddings
        let device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("GraphModel GPU init for projections failed: {}", e),
        })?;
        let projection = GraphProjectionWeights::initialize(
            GRAPH_DIMENSION,
            device,
            GRAPH_PROJECTION_SEED,
        )?;

        tracing::info!(
            "GraphModel loaded: {} params, {:.2} MB VRAM, hidden_size={}, with graph projections",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0),
            weights.config.hidden_size
        );

        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("GraphModel failed to acquire write lock: {}", e),
            })?;
        *state = ModelState::Loaded {
            weights: Box::new(weights),
            tokenizer: Box::new(tokenizer),
            projection,
        };
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Unload model weights from memory.
    pub async fn unload(&self) -> EmbeddingResult<()> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }
        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to acquire write lock: {}", e),
            })?;
        *state = ModelState::Unloaded;
        self.loaded.store(false, Ordering::SeqCst);
        tracing::info!("GraphModel unloaded");
        Ok(())
    }

    /// Embed a batch of inputs (more efficient than single embed).
    pub async fn embed_batch(&self, inputs: &[ModelInput]) -> EmbeddingResult<Vec<ModelEmbedding>> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.embed(input).await?);
        }
        Ok(results)
    }

    /// Extract text content from model input for embedding.
    fn extract_content(input: &ModelInput) -> EmbeddingResult<String> {
        match input {
            ModelInput::Text {
                content,
                instruction,
            } => {
                let mut full = content.clone();
                if let Some(inst) = instruction {
                    full = format!("{} {}", inst, full);
                }
                Ok(full)
            }
            _ => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::Graph,
                input_type: InputType::from(input),
            }),
        }
    }

    // =========================================================================
    // ASYMMETRIC DUAL EMBEDDING METHODS (Following E5 Causal Pattern)
    // =========================================================================
    //
    // These methods produce genuinely different source and target vectors through:
    // 1. Structural marker detection (source/target indicator tokens)
    // 2. Learned projections (W_source, W_target) initialized as perturbed identities
    //
    // This creates meaningful asymmetry for graph retrieval with:
    // - source→target direction: 1.2x amplification
    // - target→source direction: 0.8x dampening
    // =========================================================================

    /// Embed text as a potential SOURCE in graph relationships.
    ///
    /// Uses the base embedding projected through W_source matrix.
    /// Use this when embedding text that "points to" other entities
    /// (e.g., "Module A imports B, C, D").
    ///
    /// # Arguments
    /// * `content` - Text content to embed as a source
    ///
    /// # Returns
    /// 384D embedding vector with source-role semantics
    pub async fn embed_as_source(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        let (source_vec, _) = self.embed_dual(content).await?;
        Ok(source_vec)
    }

    /// Embed text as a potential TARGET in graph relationships.
    ///
    /// Uses the base embedding projected through W_target matrix.
    /// Use this when embedding text that "is pointed to" by other entities
    /// (e.g., "Module X is imported by A, B, C").
    ///
    /// # Arguments
    /// * `content` - Text content to embed as a target
    ///
    /// # Returns
    /// 384D embedding vector with target-role semantics
    pub async fn embed_as_target(&self, content: &str) -> EmbeddingResult<Vec<f32>> {
        let (_, target_vec) = self.embed_dual(content).await?;
        Ok(target_vec)
    }

    /// Embed text as BOTH source and target roles simultaneously.
    ///
    /// Produces two distinct 384D vectors from a single encoder pass:
    /// - source_vec: Base embedding projected through W_source
    /// - target_vec: Base embedding projected through W_target
    ///
    /// # Architecture
    ///
    /// ```text
    /// Input Text
    ///     |
    /// [Tokenize]
    ///     |
    /// [Encoder (single pass)]
    ///     |
    ///     +------------------------+
    ///     |                        |
    /// [W_source Projection]   [W_target Projection]
    ///     |                        |
    /// [L2 Normalize]          [L2 Normalize]
    ///     |                        |
    /// source_vec (384D)       target_vec (384D)
    /// ```
    ///
    /// # Arguments
    /// * `content` - Text content to embed in both roles
    ///
    /// # Returns
    /// Tuple of (source_vector, target_vector), each 384D
    ///
    /// # Performance
    /// Single encoder forward pass + dual projection (efficient).
    pub async fn embed_dual(&self, content: &str) -> EmbeddingResult<(Vec<f32>, Vec<f32>)> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Graph,
            });
        }

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("GraphModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded {
                weights,
                tokenizer,
                projection,
            } => {
                // Step 1: Get base embedding via standard forward pass
                let base_embedding = gpu_forward(content, weights, tokenizer)?;

                // Step 2: Convert to tensor for projection
                let device = init_gpu().map_err(|e| EmbeddingError::GpuError {
                    message: format!("GraphModel GPU init for projection failed: {}", e),
                })?;

                let base_tensor = Tensor::from_slice(&base_embedding, (1, GRAPH_DIMENSION), device)
                    .map_err(|e| EmbeddingError::GpuError {
                        message: format!("Failed to create base embedding tensor: {}", e),
                    })?;

                // Step 3: Apply source projection
                let source_tensor = projection.project_source(&base_tensor)?;
                let source_normalized = l2_normalize_tensor(&source_tensor)?;
                let source_vec = tensor_to_vec(&source_normalized)?;

                // Step 4: Apply target projection
                let target_tensor = projection.project_target(&base_tensor)?;
                let target_normalized = l2_normalize_tensor(&target_tensor)?;
                let target_vec = tensor_to_vec(&target_normalized)?;

                // Step 5: Validate dimensions (fail fast on implementation error)
                if source_vec.len() != GRAPH_DIMENSION || target_vec.len() != GRAPH_DIMENSION {
                    return Err(EmbeddingError::InternalError {
                        message: format!(
                            "E8 dual embedding dimension error: source={}, target={}, expected {}",
                            source_vec.len(),
                            target_vec.len(),
                            GRAPH_DIMENSION
                        ),
                    });
                }

                Ok((source_vec, target_vec))
            }
            ModelState::Unloaded => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Graph,
            }),
        }
    }
}

/// L2-normalize a tensor.
fn l2_normalize_tensor(tensor: &Tensor) -> EmbeddingResult<Tensor> {
    let squared = tensor
        .sqr()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("L2 norm squared failed: {}", e),
        })?;
    let sum = squared
        .sum_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("L2 norm sum failed: {}", e),
        })?;
    let norm = sum
        .sqrt()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("L2 norm sqrt failed: {}", e),
        })?;

    // Avoid division by zero
    let norm_val: f32 = norm
        .to_scalar()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("L2 norm to_scalar failed: {}", e),
        })?;

    if norm_val < f32::EPSILON {
        // Return the original tensor if norm is essentially zero
        return Ok(tensor.clone());
    }

    tensor
        .broadcast_div(&norm)
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("L2 normalize broadcast_div failed: {}", e),
        })
}

/// Convert tensor to Vec<f32>.
fn tensor_to_vec(tensor: &Tensor) -> EmbeddingResult<Vec<f32>> {
    let flattened = tensor
        .flatten_all()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Tensor flatten failed: {}", e),
        })?;
    flattened
        .to_vec1()
        .map_err(|e| EmbeddingError::GpuError {
            message: format!("Tensor to_vec1 failed: {}", e),
        })
}

#[async_trait]
impl EmbeddingModel for GraphModel {
    fn model_id(&self) -> ModelId {
        ModelId::Graph
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    async fn load(&self) -> EmbeddingResult<()> {
        GraphModel::load(self).await
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Graph,
            });
        }
        self.validate_input(input)?;
        let content = Self::extract_content(input)?;
        let start = std::time::Instant::now();

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("GraphModel failed to acquire read lock: {}", e),
            })?;

        let (weights, tokenizer) = match &*state {
            ModelState::Loaded { weights, tokenizer, .. } => (weights, tokenizer),
            _ => {
                return Err(EmbeddingError::NotInitialized {
                    model_id: ModelId::Graph,
                })
            }
        };

        let vector = gpu_forward(&content, weights, tokenizer)?;
        let latency_us = start.elapsed().as_micros() as u64;
        Ok(ModelEmbedding::new(ModelId::Graph, vector, latency_us))
    }
}

unsafe impl Send for GraphModel {}
unsafe impl Sync for GraphModel {}
