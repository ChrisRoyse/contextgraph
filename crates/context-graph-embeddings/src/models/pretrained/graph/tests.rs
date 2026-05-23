//! Tests for GraphModel.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::models::pretrained::graph::{GraphModel, GRAPH_DIMENSION};
    use crate::models::pretrained::shared::pretrained_test_model_path;
    use crate::traits::{EmbeddingModel, SingleModelConfig};
    use crate::types::ModelInput;

    fn create_test_model() -> GraphModel {
        let model_path = pretrained_test_model_path("semantic");
        GraphModel::new(&model_path, SingleModelConfig::default())
            .expect("Failed to create GraphModel")
    }

    fn max_abs_delta(left: &[f32], right: &[f32]) -> f32 {
        left.iter()
            .zip(right.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f32, f32::max)
    }

    #[test]
    fn test_new_creates_unloaded_model() {
        let model = create_test_model();
        assert!(!model.is_initialized());
    }

    #[tokio::test]
    async fn test_embed_text_returns_1024d() {
        let model = create_test_model();
        model.load().await.expect("Failed to load model");
        let input = ModelInput::text("Alice works at Anthropic").expect("Input");
        let embedding = model.embed(&input).await.expect("Embed should succeed");
        assert_eq!(embedding.vector.len(), GRAPH_DIMENSION);
        assert_eq!(embedding.vector.len(), 1024);
    }

    #[tokio::test]
    async fn test_embed_text_uses_graph_source_projection() {
        let model = create_test_model();
        model.load().await.expect("Failed to load model");

        let content = "module alpha imports beta";
        let input = ModelInput::text(content).expect("Input");
        let embedding = model.embed(&input).await.expect("Embed should succeed");
        let (source_vec, target_vec) = model
            .embed_dual(content)
            .await
            .expect("Dual embed should succeed");

        assert_eq!(embedding.vector.len(), GRAPH_DIMENSION);
        assert_eq!(source_vec.len(), GRAPH_DIMENSION);
        assert_eq!(target_vec.len(), GRAPH_DIMENSION);
        assert!(
            max_abs_delta(&embedding.vector, &source_vec) < 1e-5,
            "standard GraphModel embed() must return source-projected E8 vectors"
        );
        assert!(
            max_abs_delta(&source_vec, &target_vec) > 1e-6,
            "source and target projections must remain distinct"
        );
    }

    #[tokio::test]
    async fn test_embed_true_batch_returns_ordered_1024d_vectors() {
        let model = create_test_model();
        model.load().await.expect("Failed to load model");
        assert!(model.supports_true_batch());

        let inputs = vec![
            ModelInput::text("module alpha imports beta").expect("input"),
            ModelInput::text("function gamma writes a witness-chain entry").expect("input"),
        ];
        let embeddings = EmbeddingModel::embed_true_batch(&model, &inputs)
            .await
            .expect("graph true batch");

        assert_eq!(embeddings.len(), inputs.len());
        for embedding in &embeddings {
            embedding.validate().expect("valid graph true-batch vector");
            assert_eq!(embedding.vector.len(), GRAPH_DIMENSION);
        }

        let (first_source, _) = model
            .embed_dual("module alpha imports beta")
            .await
            .expect("dual embed");
        assert!(
            max_abs_delta(&embeddings[0].vector, &first_source) < 1e-5,
            "GraphModel true-batch output must use the same source projection as single embed"
        );
    }
}
