//! GPU tensor conversions for FusedEmbedding (requires candle feature).

use crate::types::dimensions::TOP_K_EXPERTS;

use super::core::FusedEmbedding;

impl FusedEmbedding {
    /// Convert fused embedding vector to GPU tensor.
    ///
    /// # Arguments
    /// * `device` - The GPU device to create the tensor on
    ///
    /// # Returns
    /// A 1D tensor of shape [1536] containing the fused embedding.
    ///
    /// # Example
    /// ```
    /// # use context_graph_embeddings::types::FusedEmbedding;
    /// # use context_graph_embeddings::gpu::{init_gpu, device};
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// init_gpu()?;
    /// let fused = FusedEmbedding::new(
    ///     vec![0.1f32; 1536], [0.125f32; 8], [0, 1, 2, 3], 100, 12345,
    /// )?;
    /// let tensor = fused.to_tensor(device())?;
    /// assert_eq!(tensor.dims(), &[1536]);
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

    /// Create fused embedding from GPU tensor.
    ///
    /// # Arguments
    /// * `tensor` - A 1D tensor of shape [1536]
    /// * `expert_weights` - Expert weights from gating network
    /// * `selected_experts` - Top-K selected expert indices
    /// * `content_hash` - Hash of original content
    ///
    /// # Returns
    /// A new FusedEmbedding with the tensor values.
    #[cfg(feature = "candle")]
    pub fn from_tensor(
        tensor: &candle_core::Tensor,
        expert_weights: [f32; 8],
        selected_experts: [u8; TOP_K_EXPERTS],
        content_hash: u64,
    ) -> candle_core::Result<Self> {
        let vector: Vec<f32> = tensor.to_vec1()?;
        Ok(Self {
            vector,
            expert_weights,
            selected_experts,
            pipeline_latency_us: 0,
            content_hash,
            aux_data: None,
        })
    }

    /// Convert batch of fused embeddings to GPU tensor.
    ///
    /// # Arguments
    /// * `embeddings` - Slice of fused embeddings
    /// * `device` - The GPU device
    ///
    /// # Returns
    /// A 2D tensor of shape [batch_size, 1536].
    #[cfg(feature = "candle")]
    pub fn batch_to_tensor(
        embeddings: &[Self],
        device: &candle_core::Device,
    ) -> candle_core::Result<candle_core::Tensor> {
        use crate::types::dimensions::FUSED_OUTPUT;

        if embeddings.is_empty() {
            return candle_core::Tensor::zeros(
                (0, FUSED_OUTPUT),
                candle_core::DType::F32,
                device,
            );
        }

        let batch_size = embeddings.len();
        let data: Vec<f32> = embeddings
            .iter()
            .flat_map(|e| e.vector.iter().copied())
            .collect();

        candle_core::Tensor::from_slice(&data, (batch_size, FUSED_OUTPUT), device)
    }
}
