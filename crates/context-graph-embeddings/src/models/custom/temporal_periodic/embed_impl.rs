//! EmbeddingModel trait implementation for TemporalPeriodicModel.
//!
//! Implements the async embedding interface for the trait.

use std::sync::atomic::Ordering;

use async_trait::async_trait;

use crate::error::EmbeddingResult;
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::model::TemporalPeriodicModel;

#[async_trait]
impl EmbeddingModel for TemporalPeriodicModel {
    fn model_id(&self) -> ModelId {
        ModelId::TemporalPeriodic
    }

    fn supported_input_types(&self) -> &[InputType] {
        // TemporalPeriodic supports Text input (timestamp via instruction field)
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        // 1. Validate input type
        self.validate_input(input)?;

        let start = std::time::Instant::now();

        // 2. Extract timestamp from input
        let timestamp = self.extract_timestamp(input)?;

        // 3. Compute Fourier embedding
        let vector = self.compute_fourier_embedding(timestamp);

        let latency_us = start.elapsed().as_micros() as u64;

        // 4. Create and return ModelEmbedding
        let embedding = ModelEmbedding::new(ModelId::TemporalPeriodic, vector, latency_us);

        // Validate output (checks dimension, NaN, Inf)
        embedding.validate()?;

        Ok(embedding)
    }
}
