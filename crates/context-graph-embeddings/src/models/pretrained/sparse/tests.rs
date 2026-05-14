//! Tests for the sparse embedding model.

#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::models::pretrained::shared::pretrained_test_model_path;
    use crate::models::pretrained::sparse::{
        SparseModel, SparseVector, SPARSE_PROJECTED_DIMENSION, SPARSE_VOCAB_SIZE,
    };
    use crate::traits::{EmbeddingModel, SingleModelConfig};
    use crate::types::{ModelId, ModelInput};

    fn create_test_model() -> SparseModel {
        let model_path = pretrained_test_model_path("sparse");
        SparseModel::new(&model_path, SingleModelConfig::default())
            .expect("Failed to create SparseModel")
    }

    fn create_test_splade_model() -> SparseModel {
        let model_path = pretrained_test_model_path("splade-v3");
        SparseModel::new_splade(&model_path, SingleModelConfig::default())
            .expect("Failed to create Splade SparseModel")
    }

    // ==================== Construction Tests ====================

    #[test]
    fn test_new_creates_unloaded_model() {
        let model = create_test_model();
        assert!(!model.is_initialized());
        assert!(model.supports_true_batch());
    }

    // ==================== Sparse Vector Tests ====================

    #[test]
    fn test_sparse_vector_new() {
        let indices = vec![10, 100, 500];
        let weights = vec![0.5, 0.3, 0.8];
        let sparse = SparseVector::new(indices.clone(), weights.clone());

        assert_eq!(sparse.indices, indices);
        assert_eq!(sparse.weights, weights);
        assert_eq!(sparse.dimension, SPARSE_VOCAB_SIZE);
        assert_eq!(sparse.dimension, 30522);
    }

    #[tokio::test]
    async fn test_sparse_embed_true_batch_returns_ordered_projected_vectors() {
        let model = create_test_model();
        model.load().await.expect("load sparse model");
        let inputs = vec![
            ModelInput::text("def add(a, b): return a + b").expect("input"),
            ModelInput::text("class Counter: pass").expect("input"),
            ModelInput::text("async def fetch(client): return await client.get('/')")
                .expect("input"),
        ];

        let embeddings = EmbeddingModel::embed_true_batch(&model, &inputs)
            .await
            .expect("sparse true batch");

        assert_eq!(embeddings.len(), inputs.len());
        for embedding in &embeddings {
            embedding
                .validate()
                .expect("valid sparse true-batch projected vector");
            assert_eq!(embedding.model_id, ModelId::Sparse);
            assert_eq!(embedding.vector.len(), SPARSE_PROJECTED_DIMENSION);
            assert!(embedding.is_projected);
        }
        model.unload().await.expect("unload sparse model");
    }

    #[tokio::test]
    async fn test_splade_embed_true_batch_returns_ordered_projected_vectors() {
        let model = create_test_splade_model();
        model.load().await.expect("load splade model");
        let inputs = vec![
            ModelInput::text("def normalize(items): return [x.strip() for x in items]")
                .expect("input"),
            ModelInput::text("raise ValueError('missing witness')").expect("input"),
        ];

        let embeddings = EmbeddingModel::embed_true_batch(&model, &inputs)
            .await
            .expect("splade true batch");

        assert_eq!(embeddings.len(), inputs.len());
        for embedding in &embeddings {
            embedding
                .validate()
                .expect("valid splade true-batch projected vector");
            assert_eq!(embedding.model_id, ModelId::Splade);
            assert_eq!(embedding.vector.len(), SPARSE_PROJECTED_DIMENSION);
            assert!(embedding.is_projected);
        }
        model.unload().await.expect("unload splade model");
    }
}
