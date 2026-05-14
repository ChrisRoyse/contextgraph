//! `EmbeddingModel` trait implementation for `BgeM3DenseModel`.

use async_trait::async_trait;

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::types::BgeM3DenseModel;

#[async_trait]
impl EmbeddingModel for BgeM3DenseModel {
    fn model_id(&self) -> ModelId {
        ModelId::BgeM3Dense
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized()
    }

    async fn load(&self) -> EmbeddingResult<()> {
        BgeM3DenseModel::load(self).await
    }

    async fn unload(&self) -> EmbeddingResult<()> {
        BgeM3DenseModel::unload(self).await
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: ModelId::BgeM3Dense,
            });
        }
        self.validate_input(input)?;
        self.embed_single(input).await
    }

    fn supports_true_batch(&self) -> bool {
        true
    }

    async fn embed_true_batch(
        &self,
        inputs: &[ModelInput],
    ) -> EmbeddingResult<Vec<ModelEmbedding>> {
        BgeM3DenseModel::embed_batch(self, inputs).await
    }
}
