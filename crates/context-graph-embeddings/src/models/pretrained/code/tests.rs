//! Core tests for the CodeModel.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use std::path::PathBuf;

    use crate::error::EmbeddingError;
    use crate::models::pretrained::code::{
        CodeModel, CODE_LATENCY_BUDGET_MS, CODE_MAX_TOKENS, CODE_MODEL_NAME, CODE_NATIVE_DIMENSION,
        CODE_PROJECTED_DIMENSION,
    };
    use crate::traits::{EmbeddingModel, SingleModelConfig};
    use crate::types::{ModelId, ModelInput};
    use serial_test::serial;

    /// Get the workspace root directory for test model paths.
    pub(crate) fn workspace_root() -> PathBuf {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(manifest_dir)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub(crate) fn create_test_model() -> CodeModel {
        let model_path = workspace_root().join("models/code-1536");
        CodeModel::new(&model_path, SingleModelConfig::default())
            .expect("Failed to create CodeModel")
    }

    pub(crate) async fn create_and_load_model() -> CodeModel {
        let model = create_test_model();
        model.load().await.expect("Failed to load model");
        model
    }

    // ==================== Construction Tests ====================

    #[test]
    fn test_new_creates_unloaded_model() {
        let model = create_test_model();
        assert!(!model.is_initialized());
    }

    #[test]
    fn test_new_with_zero_batch_size_fails() {
        let config = SingleModelConfig {
            max_batch_size: 0,
            ..Default::default()
        };
        let model_path = workspace_root().join("models/code");
        let result = CodeModel::new(&model_path, config);
        assert!(matches!(result, Err(EmbeddingError::ConfigError { .. })));
    }

    // ==================== Trait Implementation Tests ====================

    #[test]
    fn test_model_id() {
        let model = create_test_model();
        assert_eq!(model.model_id(), ModelId::Code);
    }

    #[test]
    fn test_native_dimension() {
        let model = create_test_model();
        assert_eq!(model.dimension(), 1536);
    }

    #[test]
    fn test_projected_dimension() {
        let model = create_test_model();
        assert_eq!(model.projected_dimension(), 1536);
    }

    #[test]
    fn test_max_tokens() {
        let model = create_test_model();
        assert_eq!(model.max_tokens(), 32768);
    }

    #[test]
    fn test_latency_budget_ms() {
        let model = create_test_model();
        assert_eq!(model.latency_budget_ms(), 10);
    }

    #[test]
    fn test_is_pretrained() {
        let model = create_test_model();
        assert!(model.is_pretrained());
    }

    #[test]
    fn test_supported_input_types() {
        use crate::types::InputType;
        let model = create_test_model();
        let types = model.supported_input_types();
        assert!(types.contains(&InputType::Code));
        assert!(types.contains(&InputType::Text));
        assert!(!types.contains(&InputType::Image));
        assert!(!types.contains(&InputType::Audio));
    }

    // ==================== State Transition Tests ====================

    #[tokio::test]
    async fn test_load_sets_initialized() {
        let model = create_test_model();
        assert!(!model.is_initialized());
        model.load().await.expect("Load should succeed");
        assert!(model.is_initialized());
    }

    #[tokio::test]
    async fn test_unload_clears_initialized() {
        let model = create_and_load_model().await;
        assert!(model.is_initialized());
        model.unload().await.expect("Unload should succeed");
        assert!(!model.is_initialized());
    }

    #[tokio::test]
    async fn test_unload_when_not_loaded_fails() {
        let model = create_test_model();
        let result = model.unload().await;
        assert!(matches!(result, Err(EmbeddingError::NotInitialized { .. })));
    }

    // Serial: Multiple load/unload cycles require exclusive VRAM access
    #[tokio::test]
    #[serial]
    async fn test_state_transitions_full_cycle() {
        let model = create_test_model();
        assert!(!model.is_initialized());
        model.load().await.unwrap();
        assert!(model.is_initialized());
        model.unload().await.unwrap();
        assert!(!model.is_initialized());
        model.load().await.unwrap();
        assert!(model.is_initialized());
    }

    // ==================== Embedding Tests ====================

    #[tokio::test]
    async fn test_embed_before_load_fails() {
        let model = create_test_model();
        let input = ModelInput::code("fn main() {}", "rust").expect("Failed to create input");
        let result = model.embed(&input).await;
        assert!(matches!(result, Err(EmbeddingError::NotInitialized { .. })));
    }

    #[tokio::test]
    async fn test_embed_code_returns_projected_dimension() {
        let model = create_and_load_model().await;
        let input = ModelInput::code("fn main() { println!(\"Hello\"); }", "rust").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed should succeed");
        assert_eq!(embedding.vector.len(), CODE_PROJECTED_DIMENSION);
    }

    #[tokio::test]
    async fn test_embed_text_returns_projected_dimension() {
        let model = create_and_load_model().await;
        let input = ModelInput::text("Parse the JSON response").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed should succeed");
        assert_eq!(embedding.vector.len(), CODE_PROJECTED_DIMENSION);
    }

    #[tokio::test]
    async fn test_embed_returns_l2_normalized_vector() {
        let model = create_and_load_model().await;
        let input = ModelInput::code("def hello(): pass", "python").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed");
        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 0.001,
            "L2 norm should be ~1.0, got {}",
            norm
        );
    }

    #[tokio::test]
    async fn test_embed_no_nan_values() {
        let model = create_and_load_model().await;
        let input = ModelInput::code("console.log('test');", "javascript").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed");
        assert!(!embedding.vector.iter().any(|x| x.is_nan()));
    }

    #[tokio::test]
    async fn test_embed_no_inf_values() {
        let model = create_and_load_model().await;
        let input = ModelInput::code("int main() { return 0; }", "c").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed");
        assert!(!embedding.vector.iter().any(|x| x.is_infinite()));
    }

    #[tokio::test]
    async fn test_embed_deterministic() {
        let model = create_and_load_model().await;
        let input = ModelInput::code("func main() {}", "go").expect("Input");
        let emb1 = model.embed(&input).await.expect("Embed 1");
        let emb2 = model.embed(&input).await.expect("Embed 2");
        assert_eq!(emb1.vector, emb2.vector);
    }

    #[tokio::test]
    async fn test_embed_different_inputs_differ() {
        let model = create_and_load_model().await;
        let input1 = ModelInput::code("fn foo() {}", "rust").expect("Input");
        let input2 = ModelInput::code("fn bar() {}", "rust").expect("Input");
        let emb1 = model.embed(&input1).await.expect("Embed 1");
        let emb2 = model.embed(&input2).await.expect("Embed 2");
        assert_ne!(emb1.vector, emb2.vector);
    }

    #[tokio::test]
    async fn test_embed_model_id_is_code() {
        let model = create_and_load_model().await;
        let input = ModelInput::code("print('hello')", "python").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed");
        assert_eq!(embedding.model_id, ModelId::Code);
    }

    // ==================== CONSTANTS TESTS ====================

    #[test]
    fn test_constants_are_correct() {
        assert_eq!(CODE_NATIVE_DIMENSION, 1536);
        assert_eq!(CODE_PROJECTED_DIMENSION, 1536);
        assert_eq!(CODE_MAX_TOKENS, 32768);
        assert_eq!(CODE_LATENCY_BUDGET_MS, 10);
        assert_eq!(CODE_MODEL_NAME, "Qodo/Qodo-Embed-1-1.5B");
    }

    #[test]
    fn test_model_id_dimension_matches_constant() {
        assert_eq!(ModelId::Code.dimension(), CODE_NATIVE_DIMENSION);
        assert_eq!(
            ModelId::Code.projected_dimension(),
            CODE_PROJECTED_DIMENSION
        );
    }

    #[test]
    fn test_model_id_latency_matches_constant() {
        assert_eq!(ModelId::Code.latency_budget_ms(), CODE_LATENCY_BUDGET_MS);
    }

    #[test]
    fn test_model_id_max_tokens_matches_constant() {
        assert_eq!(ModelId::Code.max_tokens(), CODE_MAX_TOKENS);
    }
}
