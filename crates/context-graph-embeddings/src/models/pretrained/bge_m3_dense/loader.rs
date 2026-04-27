//! Weight loading for BgeM3DenseModel.
//!
//! Reuses `GpuModelLoader::load_bert_weights_with_prefix` with BGE-M3's flat
//! checkpoint keys. The encoder layer structure is architecturally identical
//! to BERT, so no new weight structs are needed.

use std::sync::atomic::Ordering;

use tokenizers::Tokenizer;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::gpu::{init_gpu, GpuModelLoader};
use crate::types::ModelId;

use super::constants::{BGE_M3_DENSE_DIMENSION, XLM_R_WEIGHT_PREFIX};
use super::types::{BgeM3DenseModel, ModelState};

impl BgeM3DenseModel {
    /// Load model weights into GPU VRAM.
    ///
    /// # Pipeline
    /// 1. Initialise CUDA device.
    /// 2. Load the XLM-R SentencePiece tokenizer from `tokenizer.json`.
    /// 3. Load BGE-M3 weights from `model.safetensors` using the flat
    ///    checkpoint keys that HuggingFace/FlagEmbedding ships with.
    ///
    /// # Errors
    /// - GPU init failure (no CUDA / driver mismatch).
    /// - Missing `tokenizer.json`, `config.json`, or `model.safetensors`.
    /// - Dimension mismatch (expected 1024-D hidden size).
    /// - Insufficient VRAM (~2.3 GB at FP32).
    pub async fn load(&self) -> EmbeddingResult<()> {
        tracing::info!(
            target: "context_graph_embeddings::bge_m3_dense",
            model_path = %self.model_path.display(),
            "Loading BgeM3DenseModel (BAAI/bge-m3 dense head, XLM-RoBERTa-Large)..."
        );

        // GPU init.
        let _device = init_gpu().map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::bge_m3_dense",
                error = %e,
                "BgeM3Dense GPU init FAILED. Check CUDA drivers and nvidia-smi."
            );
            EmbeddingError::GpuError {
                message: format!("BgeM3Dense GPU init failed: {}", e),
            }
        })?;

        // Tokenizer. `tokenizers::Tokenizer::from_file` auto-detects the
        // SentencePiece format from `tokenizer.json`, so we do not need a
        // separate crate for XLM-R specifically.
        let tokenizer_path = self.model_path.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::bge_m3_dense",
                error = %e,
                tokenizer_path = %tokenizer_path.display(),
                "BgeM3Dense tokenizer load FAILED."
            );
            EmbeddingError::ModelLoadError {
                model_id: ModelId::BgeM3Dense,
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

        // Safetensors with XLM-R prefix.
        let loader = GpuModelLoader::new().map_err(|e| {
            tracing::error!(
                target: "context_graph_embeddings::bge_m3_dense",
                error = %e,
                "BgeM3Dense GPU loader init FAILED."
            );
            EmbeddingError::GpuError {
                message: format!("BgeM3Dense loader init failed: {}", e),
            }
        })?;

        let weights = loader
            .load_bert_weights_with_prefix(&self.model_path, XLM_R_WEIGHT_PREFIX)
            .map_err(|e| {
                tracing::error!(
                    target: "context_graph_embeddings::bge_m3_dense",
                    error = %e,
                    model_path = %self.model_path.display(),
                    "BgeM3Dense weight load FAILED."
                );
                EmbeddingError::ModelLoadError {
                    model_id: ModelId::BgeM3Dense,
                    source: Box::new(std::io::Error::other(format!(
                        "BgeM3Dense weight load failed: {}",
                        e
                    ))),
                }
            })?;

        // Validate the hidden size.
        if weights.config.hidden_size != BGE_M3_DENSE_DIMENSION {
            tracing::error!(
                target: "context_graph_embeddings::bge_m3_dense",
                expected = BGE_M3_DENSE_DIMENSION,
                actual = weights.config.hidden_size,
                "BgeM3Dense dimension mismatch."
            );
            return Err(EmbeddingError::InvalidDimension {
                expected: BGE_M3_DENSE_DIMENSION,
                actual: weights.config.hidden_size,
            });
        }

        tracing::info!(
            "BgeM3Dense loaded: {} params, {:.2} MB VRAM, hidden_size={}, layers={}, heads={}",
            weights.param_count(),
            weights.vram_bytes() as f64 / (1024.0 * 1024.0),
            weights.config.hidden_size,
            weights.config.num_hidden_layers,
            weights.config.num_attention_heads,
        );

        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("BgeM3Dense failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Loaded {
            weights: Box::new(weights),
            tokenizer: Box::new(tokenizer),
        };
        self.loaded.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Unload model weights from GPU VRAM.
    pub async fn unload(&self) -> EmbeddingResult<()> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::BgeM3Dense,
            });
        }

        let mut state = self
            .model_state
            .write()
            .map_err(|e| EmbeddingError::InternalError {
                message: format!("BgeM3Dense failed to acquire write lock: {}", e),
            })?;

        *state = ModelState::Unloaded;
        self.loaded.store(false, Ordering::SeqCst);
        tracing::info!("BgeM3Dense unloaded");
        Ok(())
    }
}
