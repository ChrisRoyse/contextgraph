//! Batch, validation, concurrency and constants tests for LateInteractionModel.

use super::tests::create_and_load_model;
use super::LATE_INTERACTION_DIMENSION;
use crate::traits::EmbeddingModel;
use crate::types::{ImageFormat, ModelId, ModelInput};

#[tokio::test]
async fn test_embed_tokens_true_batch_preserves_order_and_metadata() {
    let model = create_and_load_model().await;
    assert!(model.supports_true_batch());

    let texts = vec![
        "def alpha():\n    return 1\n".to_string(),
        "def beta(value):\n    return value + 2\n".to_string(),
        "def gamma(items):\n    return [item for item in items if item]\n".to_string(),
    ];

    let rows = model
        .embed_tokens_batch(&texts)
        .await
        .expect("ColBERT true token batch");

    assert_eq!(rows.len(), texts.len());
    let valid_counts = rows
        .iter()
        .map(|row| row.valid_token_count())
        .collect::<Vec<_>>();
    assert!(valid_counts.windows(2).any(|pair| pair[0] != pair[1]));

    for (idx, row) in rows.iter().enumerate() {
        row.validate_true_batch_output(ModelId::LateInteraction, idx)
            .expect("valid ColBERT true-batch row");
        assert_eq!(row.vectors.len(), row.tokens.len());
        assert_eq!(row.vectors.len(), row.mask.len());
        assert!(row.valid_token_count() > 0);
        assert!(row
            .vectors
            .iter()
            .all(|vector| vector.len() == LATE_INTERACTION_DIMENSION));
    }
}

#[tokio::test]
async fn test_embed_true_batch_returns_ordered_128d_vectors() {
    let model = create_and_load_model().await;
    let inputs = vec![
        ModelInput::text("def alpha():\n    return 1\n").expect("Input"),
        ModelInput::text("def beta(value):\n    return value + 2\n").expect("Input"),
        ModelInput::text("def gamma(items):\n    return list(reversed(items))\n").expect("Input"),
    ];

    let embeddings = EmbeddingModel::embed_true_batch(&model, &inputs)
        .await
        .expect("LateInteractionModel true batch");

    assert_eq!(embeddings.len(), inputs.len());
    for emb in &embeddings {
        emb.validate()
            .expect("valid ColBERT pooled true-batch vector");
        assert_eq!(emb.vector.len(), LATE_INTERACTION_DIMENSION);
        assert_eq!(emb.model_id, ModelId::LateInteraction);
    }
}

#[tokio::test]
async fn test_embed_true_batch_rejects_empty_and_wrong_modality() {
    let model = create_and_load_model().await;
    let empty_err = EmbeddingModel::embed_true_batch(&model, &[])
        .await
        .expect_err("empty ColBERT true batch must fail closed");
    assert!(format!("{empty_err:?}").contains("TrueBatchEmpty"));

    let wrong_modality_err = EmbeddingModel::embed_true_batch(
        &model,
        &[ModelInput::Image {
            bytes: vec![0u8; 32],
            format: ImageFormat::Png,
        }],
    )
    .await
    .expect_err("wrong modality must fail closed");
    assert!(format!("{wrong_modality_err:?}").contains("UnsupportedModality"));
}
