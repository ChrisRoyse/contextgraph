//! Batch and language-specific tests for the CodeModel.

#[cfg(test)]
mod tests {
    use crate::models::pretrained::code::{CodeModel, CODE_PROJECTED_DIMENSION};
    use crate::models::pretrained::shared::pretrained_test_model_path;
    use crate::traits::{EmbeddingModel, SingleModelConfig};
    use crate::types::{ModelId, ModelInput};

    fn create_test_model() -> CodeModel {
        let model_path = pretrained_test_model_path("code-1536");
        CodeModel::new(&model_path, SingleModelConfig::default())
            .expect("Failed to create CodeModel")
    }

    async fn create_and_load_model() -> CodeModel {
        let model = create_test_model();
        model.load().await.expect("Failed to load model");
        model
    }

    #[tokio::test]
    async fn test_embed_batch_multiple_inputs() {
        let model = create_and_load_model().await;
        let inputs = vec![
            ModelInput::code("fn one() {}", "rust").expect("Input"),
            ModelInput::code("fn two() {}", "rust").expect("Input"),
            ModelInput::code("fn three() {}", "rust").expect("Input"),
        ];
        let embeddings = model.embed_batch(&inputs).await.expect("Batch embed");
        assert_eq!(embeddings.len(), 3);
        assert!(model.supports_true_batch());
        for emb in &embeddings {
            assert_eq!(emb.vector.len(), CODE_PROJECTED_DIMENSION);
            assert_eq!(emb.model_id, ModelId::Code);
        }
    }

    #[tokio::test]
    async fn test_embed_true_batch_returns_ordered_1536d_vectors() {
        let model = create_and_load_model().await;
        let inputs = vec![
            ModelInput::code("def alpha():\n    return 1\n", "python").expect("Input"),
            ModelInput::code("def beta(value):\n    return value + 2\n", "python").expect("Input"),
            ModelInput::code(
                "def gamma(items):\n    return list(reversed(items))\n",
                "python",
            )
            .expect("Input"),
        ];

        let embeddings = EmbeddingModel::embed_true_batch(&model, &inputs)
            .await
            .expect("CodeModel true batch");

        assert_eq!(embeddings.len(), inputs.len());
        for emb in &embeddings {
            emb.validate().expect("valid code true-batch vector");
            assert_eq!(emb.vector.len(), CODE_PROJECTED_DIMENSION);
            assert_eq!(emb.model_id, ModelId::Code);
        }
    }
}
