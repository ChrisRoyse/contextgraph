//! EmbeddingModel trait implementation for TemporalRecentModel.

use async_trait::async_trait;

use crate::error::EmbeddingResult;
use crate::traits::EmbeddingModel;
use crate::types::{InputType, ModelEmbedding, ModelId, ModelInput};

use super::compute::compute_decay_embedding;
use super::model::TemporalRecentModel;
use super::timestamp::extract_timestamp;

#[async_trait]
impl EmbeddingModel for TemporalRecentModel {
    fn model_id(&self) -> ModelId {
        ModelId::TemporalRecent
    }

    fn supported_input_types(&self) -> &[InputType] {
        // TemporalRecent supports Text input (timestamp via instruction field)
        &[InputType::Text]
    }

    fn is_initialized(&self) -> bool {
        self.is_initialized()
    }

    async fn embed(&self, input: &ModelInput) -> EmbeddingResult<ModelEmbedding> {
        // 1. Validate input type
        self.validate_input(input)?;

        let start = std::time::Instant::now();

        // 2. Extract timestamp from input
        let timestamp = extract_timestamp(input);

        // 3. Compute decay embedding
        let vector = compute_decay_embedding(timestamp, self.reference_time, &self.decay_rates);

        let latency_us = start.elapsed().as_micros() as u64;

        // 4. Create and return ModelEmbedding
        let embedding = ModelEmbedding::new(ModelId::TemporalRecent, vector, latency_us);

        // Validate output (checks dimension, NaN, Inf)
        embedding.validate()?;

        Ok(embedding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn test_model_id_is_temporal_recent() {
        let model = TemporalRecentModel::new();

        assert_eq!(model.model_id(), ModelId::TemporalRecent);
    }

    #[test]
    fn test_supported_input_types() {
        let model = TemporalRecentModel::new();
        let types = model.supported_input_types();

        assert_eq!(types.len(), 1, "Should support exactly 1 input type");
        assert_eq!(types[0], InputType::Text, "Should support Text input");
    }

    #[test]
    fn test_dimension_is_512() {
        let model = TemporalRecentModel::new();

        assert_eq!(model.dimension(), 512);
    }

    #[test]
    fn test_is_pretrained_returns_false() {
        let model = TemporalRecentModel::new();

        assert!(!model.is_pretrained(), "Custom models are not pretrained");
    }

    #[tokio::test]
    async fn test_embed_returns_512d_vector() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test content").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        assert_eq!(embedding.vector.len(), 512, "Must return exactly 512D");
    }

    #[tokio::test]
    async fn test_embed_model_id_correct() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        assert_eq!(embedding.model_id, ModelId::TemporalRecent);
    }

    #[tokio::test]
    async fn test_embed_l2_normalized() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test normalization").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();

        assert!(
            (norm - 1.0).abs() < 0.001,
            "Vector MUST be L2 normalized, got norm = {}",
            norm
        );
    }

    #[tokio::test]
    async fn test_embed_no_nan_values() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        let has_nan = embedding.vector.iter().any(|x| x.is_nan());

        assert!(!has_nan, "Output must not contain NaN values");
    }

    #[tokio::test]
    async fn test_embed_no_inf_values() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        let has_inf = embedding.vector.iter().any(|x| x.is_infinite());

        assert!(!has_inf, "Output must not contain Inf values");
    }

    #[tokio::test]
    async fn test_embed_records_latency() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test latency").expect("Failed to create input");

        let embedding = model.embed(&input).await.expect("Embed should succeed");

        assert!(
            embedding.latency_us < 2_000_000,
            "Latency should be under 2 seconds"
        );
    }

    #[tokio::test]
    async fn test_embed_latency_under_2ms() {
        let model = TemporalRecentModel::new();
        let input = ModelInput::text("test performance").expect("Failed to create input");

        let start = std::time::Instant::now();
        let _embedding = model.embed(&input).await.expect("Embed should succeed");
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 2,
            "Latency must be under 2ms, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_deterministic_with_same_timestamp() {
        let ref_time = Utc::now();
        let model = TemporalRecentModel::with_reference_time(ref_time);

        let timestamp = "timestamp:2024-01-15T10:30:00Z";
        let input1 =
            ModelInput::text_with_instruction("content", timestamp).expect("Failed to create");
        let input2 =
            ModelInput::text_with_instruction("content", timestamp).expect("Failed to create");

        let embedding1 = model.embed(&input1).await.expect("First embed");
        let embedding2 = model.embed(&input2).await.expect("Second embed");

        assert_eq!(
            embedding1.vector, embedding2.vector,
            "Same timestamp must produce identical embeddings"
        );
    }

    #[tokio::test]
    async fn test_different_timestamps_different_embeddings() {
        let ref_time = Utc::now();
        let model = TemporalRecentModel::with_reference_time(ref_time);

        let ts1 = ref_time - Duration::hours(1);
        let ts2 = ref_time - Duration::hours(24);

        let input1 =
            ModelInput::text_with_instruction("content", format!("timestamp:{}", ts1.to_rfc3339()))
                .expect("Failed to create");
        let input2 =
            ModelInput::text_with_instruction("content", format!("timestamp:{}", ts2.to_rfc3339()))
                .expect("Failed to create");

        let embedding1 = model.embed(&input1).await.expect("First embed");
        let embedding2 = model.embed(&input2).await.expect("Second embed");

        assert_ne!(
            embedding1.vector, embedding2.vector,
            "Different timestamps must produce different embeddings"
        );
    }

    #[tokio::test]
    async fn test_unsupported_code_input() {
        use crate::error::EmbeddingError;

        let model = TemporalRecentModel::new();
        let input = ModelInput::code("fn main() {}", "rust").expect("Failed to create input");

        let result = model.embed(&input).await;

        assert!(result.is_err(), "Code input should be rejected");
        match result {
            Err(EmbeddingError::UnsupportedModality {
                model_id,
                input_type,
            }) => {
                assert_eq!(model_id, ModelId::TemporalRecent);
                assert_eq!(input_type, InputType::Code);
            }
            other => panic!("Expected UnsupportedModality error, got {:?}", other),
        }
    }
}
