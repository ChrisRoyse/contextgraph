//! Edge case tests for the CodeModel.

#[cfg(test)]
mod tests {
    use crate::models::pretrained::code::{CodeModel, CODE_PROJECTED_DIMENSION};
    use crate::models::pretrained::shared::pretrained_test_model_path;
    use crate::traits::{EmbeddingModel, SingleModelConfig};
    use crate::types::ModelInput;

    async fn create_and_load_model() -> CodeModel {
        let model_path = pretrained_test_model_path("code-1536");
        let model =
            CodeModel::new(&model_path, SingleModelConfig::default()).expect("Failed to create");
        model.load().await.expect("Failed to load model");
        model
    }

    #[tokio::test]
    async fn test_edge_case_1_empty_code_content() {
        let model = create_and_load_model().await;

        // Empty string should error on ModelInput::code()
        let result = ModelInput::code("", "rust");
        assert!(
            result.is_err(),
            "Empty code string should error on ModelInput::code"
        );

        // Test with whitespace code
        let input = ModelInput::code(" ", "rust").expect("Whitespace input should work");
        let result = model.embed(&input).await;

        assert!(result.is_ok(), "Whitespace input should not error");
        let embedding = result.unwrap();
        assert_eq!(embedding.vector.len(), CODE_PROJECTED_DIMENSION);
    }
}
