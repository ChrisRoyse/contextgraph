//! EmbeddingModel trait implementation tests.

use super::super::encoding::apply_text_identity_residual;
use super::*;

#[tokio::test]
async fn test_embed_text_succeeds() {
    let model = HdcModel::default_model();
    let input = ModelInput::text("Hello, HDC world!").unwrap();
    let result = model.embed(&input).await;
    assert!(result.is_ok());
    let embedding = result.unwrap();
    assert_eq!(embedding.model_id, ModelId::Hdc);
    assert_eq!(embedding.dimension(), HDC_PROJECTED_DIMENSION);
}

#[tokio::test]
async fn test_text_identity_residual_splits_ngram_bag_ties() {
    let model = HdcModel::default_model();
    let left = "abcab";
    let right = "bcabc";

    let base_hv = model.encode_text(left);
    let mut left_vector = model.project_to_float(&base_hv);
    let mut right_vector = left_vector.clone();
    apply_text_identity_residual(&mut left_vector, model.seed(), left);
    apply_text_identity_residual(&mut right_vector, model.seed(), right);
    assert_ne!(
        left_vector
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        right_vector
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );

    let left_embedding = model.embed(&ModelInput::text(left).unwrap()).await.unwrap();
    assert_eq!(
        left_embedding
            .vector
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        left_vector
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
    let right_embedding = model
        .embed(&ModelInput::text(right).unwrap())
        .await
        .unwrap();
    assert_ne!(
        left_embedding
            .vector
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        right_embedding
            .vector
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>()
    );
}
