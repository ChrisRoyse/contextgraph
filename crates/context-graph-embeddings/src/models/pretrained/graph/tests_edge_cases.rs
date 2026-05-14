//! Edge case tests for GraphModel.

#[cfg(test)]
mod tests {
    use crate::models::pretrained::graph::GraphModel;
    use crate::models::pretrained::shared::pretrained_test_model_path;
    use crate::traits::{EmbeddingModel, SingleModelConfig};
    use crate::types::ModelInput;

    async fn create_and_load_model() -> GraphModel {
        let model_path = pretrained_test_model_path("semantic");
        let model = GraphModel::new(&model_path, SingleModelConfig::default())
            .expect("Failed to create GraphModel");
        model.load().await.expect("Failed to load model");
        model
    }

    #[tokio::test]
    async fn test_edge_case_1_empty_text_content() {
        let model = create_and_load_model().await;

        // Empty string should error on ModelInput::text()
        let result = ModelInput::text("");
        assert!(
            result.is_err(),
            "Empty text string should error on ModelInput::text"
        );

        // Test with whitespace text
        let input = ModelInput::text(" ").expect("Whitespace input should work");
        let result = model.embed(&input).await;

        assert!(result.is_ok(), "Whitespace input should not error");
        let embedding = result.unwrap();
        assert_eq!(embedding.vector.len(), 1024);
    }
}
