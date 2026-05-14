//! Test implementation of EmbeddingModel for testing the trait.

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Test implementation of EmbeddingModel for testing the trait.
pub struct TestModel {
    model_id: ModelId,
    supported_types: Vec<InputType>,
    initialized: AtomicBool,
    embed_calls: AtomicU64,
}

impl TestModel {
    /// Create a new TestModel with the given model ID and supported types.
    pub fn new(model_id: ModelId, supported_types: Vec<InputType>) -> Self {
        Self {
            model_id,
            supported_types,
            initialized: AtomicBool::new(true),
            embed_calls: AtomicU64::new(0),
        }
    }

    /// Set the initialization state of the model.
    pub fn set_initialized(&self, initialized: bool) {
        self.initialized.store(initialized, Ordering::SeqCst);
    }

    /// Number of single-input embed calls observed.
    pub fn embed_calls(&self) -> u64 {
        self.embed_calls.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl EmbeddingModel for TestModel {
    fn model_id(&self) -> ModelId {
        self.model_id
    }

    fn supported_input_types(&self) -> &[InputType] {
        &self.supported_types
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        self.embed_calls.fetch_add(1, Ordering::SeqCst);

        // Check initialization
        if !self.is_initialized() {
            return Err(EmbeddingError::NotInitialized {
                model_id: self.model_id,
            });
        }

        // Validate input type
        self.validate_input(input)?;

        // Generate a deterministic embedding based on content hash
        let hash = input.content_hash();
        let dim = self.dimension();
        let mut vector = Vec::with_capacity(dim);

        // Generate deterministic values from hash
        let mut state = hash;
        for _ in 0..dim {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let val = ((state >> 33) as f32) / (u32::MAX as f32) - 0.5;
            vector.push(val);
        }

        Ok(ModelEmbedding::new(self.model_id, vector, 100))
    }

    fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }
}
