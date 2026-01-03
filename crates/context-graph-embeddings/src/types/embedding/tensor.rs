//! GPU tensor conversions for model embeddings.
//!
//! This module provides conversion methods between embeddings
//! and Candle tensors for GPU-accelerated operations.

use crate::types::ModelId;

use super::ModelEmbedding;

impl ModelEmbedding {
    /// Convert embedding to GPU tensor.
    ///
    /// # Arguments
    /// * `device` - The GPU device to create the tensor on
    ///
    /// # Returns
    /// A 1D tensor containing the embedding vector.
    ///
    /// # Example
    /// ```
    /// # use context_graph_embeddings::types::{ModelEmbedding, ModelId};
    /// # use context_graph_embeddings::gpu::{init_gpu, device};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// init_gpu()?;
    /// let embedding = ModelEmbedding::new(ModelId::Semantic, vec![0.1, 0.2, 0.3], 100);
    /// let tensor = embedding.to_tensor(device())?;
    /// assert_eq!(tensor.dims(), &[3]);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "candle")]
    pub fn to_tensor(
        &self,
        device: &candle_core::Device,
    ) -> candle_core::Result<candle_core::Tensor> {
        candle_core::Tensor::from_slice(&self.vector, (self.vector.len(),), device)
    }

    /// Create embedding from GPU tensor.
    ///
    /// # Arguments
    /// * `tensor` - A 1D tensor containing embedding values
    /// * `model_id` - The model ID for the new embedding
    ///
    /// # Returns
    /// A new ModelEmbedding with the tensor values and zero latency.
    ///
    /// # Example
    /// ```
    /// # use context_graph_embeddings::types::{ModelEmbedding, ModelId};
    /// # use context_graph_embeddings::gpu::{init_gpu, device};
    /// # use candle_core::Tensor;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// init_gpu()?;
    /// let tensor = Tensor::from_slice(&[0.1f32, 0.2, 0.3], (3,), device())?;
    /// let embedding = ModelEmbedding::from_tensor(&tensor, ModelId::Semantic)?;
    /// assert_eq!(embedding.dimension(), 3);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "candle")]
    pub fn from_tensor(
        tensor: &candle_core::Tensor,
        model_id: ModelId,
    ) -> candle_core::Result<Self> {
        let vector: Vec<f32> = tensor.to_vec1()?;
        Ok(Self::new(model_id, vector, 0))
    }

    /// Convert batch of embeddings to GPU tensor.
    ///
    /// # Arguments
    /// * `embeddings` - Slice of embeddings to batch
    /// * `device` - The GPU device to create the tensor on
    ///
    /// # Returns
    /// A 2D tensor of shape [batch_size, dim].
    #[cfg(feature = "candle")]
    pub fn batch_to_tensor(
        embeddings: &[Self],
        device: &candle_core::Device,
    ) -> candle_core::Result<candle_core::Tensor> {
        if embeddings.is_empty() {
            return candle_core::Tensor::zeros((0, 0), candle_core::DType::F32, device);
        }

        let dim = embeddings[0].dimension();
        let batch_size = embeddings.len();

        // Flatten all vectors into a single slice
        let data: Vec<f32> = embeddings
            .iter()
            .flat_map(|e| e.vector.iter().copied())
            .collect();

        candle_core::Tensor::from_slice(&data, (batch_size, dim), device)
    }

    /// Create batch of embeddings from GPU tensor.
    ///
    /// # Arguments
    /// * `tensor` - A 2D tensor of shape [batch_size, dim]
    /// * `model_id` - The model ID for all embeddings in the batch
    ///
    /// # Returns
    /// Vector of ModelEmbedding instances.
    #[cfg(feature = "candle")]
    pub fn batch_from_tensor(
        tensor: &candle_core::Tensor,
        model_id: ModelId,
    ) -> candle_core::Result<Vec<Self>> {
        let vectors: Vec<Vec<f32>> = tensor.to_vec2()?;
        Ok(vectors
            .into_iter()
            .map(|v| Self::new(model_id, v, 0))
            .collect())
    }
}
