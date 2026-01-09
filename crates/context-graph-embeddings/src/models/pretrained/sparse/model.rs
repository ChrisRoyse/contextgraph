//! SparseModel struct and core implementation.
//!
//! This module contains the main SparseModel struct and its core methods
//! including construction, loading, unloading, and embedding.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader};
use crate::traits::SingleModelConfig;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::forward::{extract_text, gpu_forward_sparse};
use super::loader::load_mlm_head;
use super::types::{ModelState, SparseVector};

/// Sparse embedding model using naver/splade-cocondenser-ensembledistil.
///
/// This model produces high-dimensional sparse vectors (30522D) optimized for
/// lexical-aware semantic search. Uses BERT backbone with MLM head.
///
/// # Architecture
///
/// SPLADE learns sparse representations where each dimension corresponds to
/// a vocabulary term. Non-zero entries indicate important terms for retrieval.
///
/// The output can be converted to dense format (1536D) for multi-array storage compatibility.
///
/// # Construction
///
/// ```rust,no_run
/// use context_graph_embeddings::models::SparseModel;
/// use context_graph_embeddings::traits::SingleModelConfig;
/// use context_graph_embeddings::error::EmbeddingResult;
/// use std::path::Path;
///
/// async fn example() -> EmbeddingResult<()> {
///     let model = SparseModel::new(
///         Path::new("models/sparse"),
///         SingleModelConfig::default(),
///     )?;
///     model.load().await?;  // Must load before embed
///     Ok(())
/// }
/// ```
pub struct SparseModel {
    /// Model weights and inference engine.
    #[allow(dead_code)]
    pub(crate) model_state: std::sync::RwLock<ModelState>,

    /// Path to model weights directory.
    #[allow(dead_code)]
    pub(crate) model_path: PathBuf,

    /// Configuration for this model instance.
    #[allow(dead_code)]
    pub(crate) config: SingleModelConfig,

    /// Whether model weights are loaded and ready.
    pub(crate) loaded: AtomicBool,

    /// Memory used by model weights (bytes).
    #[allow(dead_code)]
    pub(crate) memory_size: usize,

    /// Model ID (Sparse for E6, Splade for E13).
    /// Both use the same SPLADE architecture but report different IDs.
    pub(crate) model_id: ModelId,
}

impl SparseModel {
    /// Create a new SparseModel instance.
    ///
    /// Model is NOT loaded after construction. Call `load()` before `embed()`.
    ///
    /// # Arguments
    /// * `model_path` - Path to directory containing model weights
    /// * `config` - Device placement and quantization settings
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails
    pub fn new(model_path: &Path, config: SingleModelConfig) -> EmbeddingResult<Self> {
        Self::with_model_id(model_path, config, ModelId::Sparse)
    }

    /// Create a new SparseModel instance for the Splade model (E13).
    ///
    /// This creates a model with the same architecture as `new()` but reports
    /// `ModelId::Splade` instead of `ModelId::Sparse`.
    ///
    /// # Arguments
    /// * `model_path` - Path to directory containing model weights
    /// * `config` - Device placement and quantization settings
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails
    pub fn new_splade(model_path: &Path, config: SingleModelConfig) -> EmbeddingResult<Self> {
        Self::with_model_id(model_path, config, ModelId::Splade)
    }

    /// Create a SparseModel with a specific model ID.
    ///
    /// # Arguments
    /// * `model_path` - Path to directory containing model weights
    /// * `config` - Device placement and quantization settings
    /// * `model_id` - The ModelId to report (Sparse or Splade)
    ///
    /// # Errors
    /// - `EmbeddingError::ConfigError` if config validation fails
    fn with_model_id(
        model_path: &Path,
        config: SingleModelConfig,
        model_id: ModelId,
    ) -> EmbeddingResult<Self> {
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
            model_id,
        })
    }

    /// Load model weights into memory.
    ///
    /// # GPU Pipeline
    ///
    /// 1. Initialize CUDA device
    /// 2. Load config.json, tokenizer.json, and model.safetensors
    /// 3. Load BERT backbone weights
    /// 4. Load MLM head weights for vocabulary projection
    /// 5. Transfer all weight tensors to GPU VRAM
    pub async fn load(&self) -> EmbeddingResult<()> {
        // Initialize GPU device
        let device = init_gpu().map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel GPU init failed: {}", e),
        })?;

        // Load tokenizer from model directory
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer =
            Tokenizer::from_file(&tokenizer_path).map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::Sparse,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            })?;

        // Load BERT backbone weights from safetensors
        let loader = GpuModelLoader::new().map_err(|e| EmbeddingError::GpuError {
            message: format!("SparseModel loader init failed: {}", e),
        })?;

        // SPLADE uses "bert." prefix for all weights
        let weights = loader
            .load_bert_weights_with_prefix(&self.model_path, "bert.")
            .map_err(|e| EmbeddingError::ModelLoadError {
                model_id: ModelId::Sparse,
                source: Box::new(std::io::Error::other(format!(
                    "SparseModel BERT weight load failed: {}",
                    e
                ))),
            })?;

        // Load MLM head weights
        let safetensors_path = self.model_path.join("model.safetensors");
        let mlm_head = load_mlm_head(&safetensors_path, device, &weights.config)?;

        // Load projection matrix (REQUIRED - no fallback)
        let projection =
            super::projection::ProjectionMatrix::load(&self.model_path).map_err(|e| {
                EmbeddingError::ModelLoadError {
                    model_id: self.model_id,
                    source: Box::new(std::io::Error::other(format!(
                        "ProjectionMatrix load failed: {}",
                        e
                    ))),
                }
            })?;

        tracing::info!(
            "ProjectionMatrix loaded: shape [{}, {}], checksum {:02x}{:02x}{:02x}{:02x}...",
            super::types::SPARSE_VOCAB_SIZE,
            super::types::SPARSE_PROJECTED_DIMENSION,
            projection.checksum()[0],
            projection.checksum()[1],
            projection.checksum()[2],
            projection.checksum()[3]
        );

        tracing::info!(
            "SparseModel loaded: {} BERT params + MLM head, hidden_size={}",
            weights.param_count(),
            weights.config.hidden_size
        );

        // Update state
        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("Failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Loaded {
            weights: Box::new(weights),
            tokenizer: Box::new(tokenizer),
            mlm_head,
            projection: Box::new(projection),
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
        tracing::info!("SparseModel unloaded");
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

    /// Embed text to sparse vector format.
    ///
    /// Returns full sparse representation with term indices and weights.
    #[allow(dead_code)]
    pub async fn embed_sparse(&self, input: &ModelInput) -> EmbeddingResult<SparseVector> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        self.validate_input(input)?;
        let text = extract_text(input)?;

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("SparseModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded {
                weights,
                tokenizer,
                mlm_head,
                projection: _, // Not used for sparse output
            } => gpu_forward_sparse(&text, weights, tokenizer, mlm_head),
            _ => Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Sparse,
            }),
        }
    }

    /// Embed input to dense 1536D vector (for multi-array storage compatibility).
    /// Per Constitution E6_Sparse: "~30K 5%active" projects to 1536D.
    ///
    /// # Pipeline
    /// 1. Validate input is text type
    /// 2. Extract text and tokenize
    /// 3. Forward through BERT + MLM head -> SparseVector (30522D)
    /// 4. Project sparse -> dense via learned ProjectionMatrix (1536D)
    /// 5. Return L2-normalized embedding
    ///
    /// # Errors
    /// - `NotInitialized` - Model not loaded
    /// - `UnsupportedModality` - Input is not text
    /// - `GpuError` - GPU operation failed (NO CPU fallback - AP-007)
    pub async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        self.validate_input(input)?;
        let text = extract_text(input)?;

        let state = self
            .model_state
            .read()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("SparseModel failed to acquire read lock: {}", e),
            })?;

        match &*state {
            ModelState::Loaded {
                weights,
                tokenizer,
                mlm_head,
                projection,
            } => {
                let start = Instant::now();

                // Step 1: Forward through BERT + MLM head to get sparse vector
                let sparse_vector = gpu_forward_sparse(&text, weights, tokenizer, mlm_head)?;

                tracing::debug!(
                    "Sparse vector: {} non-zero elements, sparsity={:.2}%",
                    sparse_vector.nnz(),
                    sparse_vector.sparsity() * 100.0
                );

                // Step 2: Project sparse (30522D) -> dense (1536D) using learned matrix
                // CRITICAL: No hash fallback (AP-007). Uses real GPU matmul.
                let dense_vector =
                    projection
                        .project(&sparse_vector)
                        .map_err(|e| EmbeddingError::GpuError {
                            message: format!("Sparse projection failed: {}", e),
                        })?;

                let latency_us = start.elapsed().as_micros() as u64;

                tracing::debug!(
                    "Projected to {}D dense vector in {}us",
                    dense_vector.len(),
                    latency_us
                );

                // Step 3: Return as ModelEmbedding
                Ok(ModelEmbedding {
                    model_id: self.model_id,
                    vector: dense_vector,
                    latency_us,
                    attention_weights: None,
                    is_projected: true, // Mark as projected from sparse
                })
            }
            _ => Err(EmbeddingError::NotInitialized {
                model_id: self.model_id,
            }),
        }
    }

    /// Validate input is text type.
    pub(crate) fn validate_input(&self, input: &ModelInput) -> EmbeddingResult<()> {
        match input {
            ModelInput::Text { .. } => Ok(()),
            _ => Err(EmbeddingError::UnsupportedModality {
                model_id: ModelId::Sparse,
                input_type: InputType::from(input),
            }),
        }
    }

    /// Check if model is initialized.
    pub fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    /// Get model ID.
    pub fn model_id(&self) -> ModelId {
        self.model_id
    }
}

// Implement Send and Sync manually since RwLock is involved
unsafe impl Send for SparseModel {}
unsafe impl Sync for SparseModel {}
