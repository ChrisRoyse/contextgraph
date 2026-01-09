//! Model loading functionality for SemanticModel.
//!
//! Handles GPU initialization, tokenizer loading, and weight loading.

use std::sync::atomic::Ordering;

use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader};
use crate::types::ModelId;

use super::constants::SEMANTIC_DIMENSION;
use super::types::{ModelState, SemanticModel};

impl SemanticModel {
    /// Load model weights into memory.
    ///
    /// # GPU Pipeline
    ///
    /// 1. Initialize CUDA device (RTX 5090 32GB)
    /// 2. Load config.json and tokenizer.json
    /// 3. Load model.safetensors via memory-mapped VarBuilder
    /// 4. Transfer all weight tensors to GPU VRAM
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - GPU initialization fails (no CUDA, driver mismatch)
    /// - Model files missing (config.json, tokenizer.json, model.safetensors)
    /// - Weight loading fails (shape mismatch, corrupt file)
    /// - Insufficient VRAM (~1.3GB required for FP32)
    pub async fn load(&self) -> EmbeddingResult<()> {
        tracing::info!(
            target: "context_graph_embeddings::semantic",
            model_path = %self.model_path.display(),
            "Loading SemanticModel (intfloat/e5-large-v2)..."
        );

        // Initialize GPU device
        let _device = init_gpu().map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::semantic",
                error = %e,
                "SemanticModel GPU initialization FAILED. \
                 Troubleshooting: 1) Verify CUDA drivers installed 2) Check nvidia-smi output 3) Ensure GPU has 2GB+ VRAM"
            );
            EmbeddingError::GpuError {
                message: format!("SemanticModel GPU init failed: {}", e),
            }
        })?;

        // Load tokenizer from model directory
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::semantic",
                error = %e,
                tokenizer_path = %tokenizer_path.display(),
                "SemanticModel tokenizer load FAILED. \
                 Troubleshooting: 1) Verify tokenizer.json exists 2) Check file permissions 3) Validate JSON format"
            );
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Semantic,
                source: Box::new(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!(
                        "Tokenizer load failed at {}: {}",
                        tokenizer_path.display(),
                        e
                    ),
                )),
            }
        })?;

        // Load BERT weights from safetensors
        let loader = GpuModelLoader::new().map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::semantic",
                error = %e,
                "SemanticModel GPU loader initialization FAILED. \
                 Troubleshooting: 1) Verify CUDA context active 2) Check GPU memory availability"
            );
            EmbeddingError::GpuError {
                message: format!("SemanticModel loader init failed: {}", e),
            }
        })?;

        let weights = loader.load_bert_weights(&self.model_path).map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::semantic",
                error = %e,
                model_path = %self.model_path.display(),
                "SemanticModel weight loading FAILED. \
                 Troubleshooting: 1) Verify model.safetensors exists 2) Check file integrity 3) Ensure 2GB+ VRAM available 4) Validate weight tensor shapes"
            );
            EmbeddingError::ModelLoadError {
                model_id: ModelId::Semantic,
                source: Box::new(std::io::Error::other(format!(
                    "SemanticModel weight load failed: {}",
                    e
                ))),
            }
        })?;

        // Validate loaded config matches expected dimensions
        if weights.config.hidden_size != SEMANTIC_DIMENSION {
            tracing::error!(
                target: "context_graph_embeddings::semantic",
                expected = SEMANTIC_DIMENSION,
                actual = weights.config.hidden_size,
                "SemanticModel dimension mismatch. Model hidden_size does not match expected SEMANTIC_DIMENSION. \
                 Troubleshooting: 1) Verify correct model variant 2) Check config.json hidden_size value"
            );
            return Err(EmbeddingError::InvalidDimension {
                expected: SEMANTIC_DIMENSION,
                actual: weights.config.hidden_size,
            });
        }

        tracing::info!(
            "SemanticModel loaded: {} params, {:.2} MB VRAM, hidden_size={}",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0),
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
        };
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Unload model weights from memory.
    ///
    /// # Errors
    /// - `EmbeddingError::NotInitialized` if model not loaded
    pub async fn unload(&self) -> EmbeddingResult<()> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Semantic,
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
        tracing::info!("SemanticModel unloaded");
        Ok(())
    }
}
