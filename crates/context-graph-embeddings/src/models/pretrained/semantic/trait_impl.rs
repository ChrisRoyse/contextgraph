//! EmbeddingModel trait implementation for SemanticModel.

use async_trait::async_trait;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::types::SemanticModel;

#[async_trait]
impl EmbeddingModel for SemanticModel {
    fn model_id(&self) -> ModelId {
        ModelId::Semantic
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized()
    }

    async fn load(&self) -> EmbeddingResult<()> {
        SemanticModel::load(self).await
    }

    async fn unload(&self) -> EmbeddingResult<()> {
        SemanticModel::unload(self).await
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        // 1. Check initialized
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::Semantic,
            });
        }

        // 2. Validate input type
        self.validate_input(input)?;

        // 3. Embed using internal method
        self.embed_single(input).await
    }

    fn supports_true_batch(&self) -> bool {
        true
    }

    async fn embed_true_batch(
        &self,
        inputs: &[ModelInput],
    ) -> EmbeddingResult<Vec<ModelEmbedding>> {
        SemanticModel::embed_batch(self, inputs).await
    }
}
