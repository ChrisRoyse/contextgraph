//! EmbeddingModel trait implementation for SparseModel.
//!
//! This module provides the trait implementation that allows SparseModel
//! to be used as part of the embedding ensemble.

use async_trait::async_trait;
use std::sync::atomic::Ordering;

use crate::error::EmbeddingResult;
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::model::SparseModel;

#[async_trait]
impl EmbeddingModel for SparseModel {
    fn model_id(&self) -> ModelId {
        // Use the instance field to return correct ID (Sparse or Splade)
        SparseModel::model_id(self)
    }

    fn supported_input_types(&self) -> &[InputType] {
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.loaded.load(Ordering::SeqCst)
    }

    async fn load(&self) -> EmbeddingResult<()> {
        SparseModel::load(self).await
    }

    async fn unload(&self) -> EmbeddingResult<()> {
        SparseModel::unload(self).await
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        // Delegate to the inherent impl
        SparseModel::embed(self, input).await
    }

    fn supports_true_batch(&self) -> bool {
        true
    }

    async fn embed_true_batch(
        &self,
        inputs: &[ModelInput],
    ) -> EmbeddingResult<Vec<ModelEmbedding>> {
        SparseModel::embed_batch(self, inputs).await
    }

    async fn embed_sparse(&self, input: &ModelInput) -> EmbeddingResult<(Vec<u16>, Vec<f32>)> {
        // Call the inherent sparse embedding method
        let sparse_vector = SparseModel::embed_sparse(self, input).await?;

        // Convert from local SparseVector (usize indices) to (u16, f32) for core SparseVector
        let indices: Vec<u16> = sparse_vector
            .indices
            .into_iter()
            .map(|i| i as u16)
            .collect();
        let values = sparse_vector.weights;

        Ok((indices, values))
    }
}
