//! EmbeddingModel trait implementation for LateInteractionModel.

use async_trait::async_trait;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::model::LateInteractionModel;
use super::types::{
    LATE_INTERACTION_DIMENSION, LATE_INTERACTION_LATENCY_BUDGET_MS, LATE_INTERACTION_MAX_TOKENS,
};

#[async_trait]
impl EmbeddingModel for LateInteractionModel {
    fn model_id(&self) -> ModelId {
        ModelId::LateInteraction
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized()
    }

    async fn load(&self) -> EmbeddingResult<()> {
        LateInteractionModel::load(self).await
    }

    async fn unload(&self) -> EmbeddingResult<()> {
        LateInteractionModel::unload(self).await
    }

    fn dimension(&self) -> usize {
        LATE_INTERACTION_DIMENSION
    }

    fn projected_dimension(&self) -> usize {
        LATE_INTERACTION_DIMENSION
    }

    fn max_tokens(&self) -> usize {
        LATE_INTERACTION_MAX_TOKENS
    }

    fn latency_budget_ms(&self) -> u32 {
        LATE_INTERACTION_LATENCY_BUDGET_MS as u32
    }

    fn is_pretrained(&self) -> bool {
        true
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id(),
            });
        }

        self.validate_input(input)?;

        let start = std::time::Instant::now();

        // Extract content for embedding
        let content = Self::extract_content(input)?;

        // Get per-token embeddings via GPU forward pass
        let token_embs = self.embed_tokens(&content).await?;

        // Pool to single 128D vector for fusion
        let vector = self.pool_tokens(&token_embs);

        let latency_us = start.elapsed().as_micros() as u64;
        Ok(ModelEmbedding::new(
            ModelId::LateInteraction,
            vector,
            latency_us,
        ))
    }

    fn supports_true_batch(&self) -> bool {
        true
    }

    async fn embed_true_batch(
        &self,
        inputs: &[ModelInput],
    ) -> EmbeddingResult<Vec<ModelEmbedding>> {
        LateInteractionModel::embed_batch(self, inputs).await
    }
}
