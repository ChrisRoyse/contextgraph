//! Unit tests for the BGE-M3 Dense embedding model.
//!
//! These tests cover paths that do NOT require the weights to be present on
//! disk. Weights-loading tests live under an `#[ignore]` guard so they can
//! be run manually after downloading the BAAI/bge-m3 snapshot.

use crate::models::pretrained::shared::pretrained_test_model_path;
use crate::traits::SingleModelConfig;
use crate::types::ModelId;

use super::constants::{
    BGE_M3_DENSE_DIMENSION, BGE_M3_DENSE_LATENCY_BUDGET_MS, BGE_M3_DENSE_MAX_TOKENS,
    XLM_R_BOS_TOKEN_ID, XLM_R_PAD_TOKEN_ID, XLM_R_POSITION_OFFSET, XLM_R_WEIGHT_PREFIX,
};
use super::types::BgeM3DenseModel;

#[test]
fn test_bge_m3_dense_constants_match_spec() {
    assert_eq!(BGE_M3_DENSE_DIMENSION, 1024);
    assert_eq!(BGE_M3_DENSE_MAX_TOKENS, 8192);
    assert_eq!(BGE_M3_DENSE_LATENCY_BUDGET_MS, 50);
    assert_eq!(XLM_R_BOS_TOKEN_ID, 0);
    assert_eq!(XLM_R_PAD_TOKEN_ID, 1);
    assert_eq!(XLM_R_POSITION_OFFSET, 2);
    assert_eq!(XLM_R_WEIGHT_PREFIX, "");
}

#[test]
fn test_new_with_valid_config_succeeds() {
    let path = pretrained_test_model_path("bge-m3-dense");
    let config = SingleModelConfig::default();

    let model = BgeM3DenseModel::new(&path, config).expect("new() should accept defaults");
    assert!(
        !model.is_initialized(),
        "fresh model must not report loaded"
    );
}

#[test]
fn test_new_with_zero_batch_size_fails() {
    let path = pretrained_test_model_path("bge-m3-dense");
    let config = SingleModelConfig {
        max_batch_size: 0,
        ..Default::default()
    };

    // Use pattern-matching rather than `expect_err` because `BgeM3DenseModel`
    // deliberately does not implement `Debug` — the inner `RwLock<ModelState>`
    // wraps GPU tensors that carry raw device pointers.
    match BgeM3DenseModel::new(&path, config) {
        Ok(_) => panic!("zero batch size must fail"),
        Err(e) => {
            let msg = format!("{:?}", e);
            assert!(msg.contains("max_batch_size"), "unexpected error: {}", msg);
        }
    }
}

#[test]
fn test_embed_on_unloaded_model_fails_with_not_initialized() {
    use crate::types::ModelInput;

    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let err = rt
        .block_on(async {
            use crate::traits::EmbeddingModel;
            model
                .embed(&ModelInput::Text {
                    content: "hello".to_string(),
                    instruction: None,
                })
                .await
        })
        .expect_err("embed must fail before load");

    let msg = format!("{:?}", err);
    assert!(msg.contains("NotInitialized"), "unexpected error: {}", msg);
}

#[test]
fn test_model_id_reports_bge_m3_dense() {
    use crate::traits::EmbeddingModel;

    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");
    assert_eq!(model.model_id(), ModelId::BgeM3Dense);
    assert!(model.supports_true_batch());
}

#[test]
fn test_prepare_input_text_passes_through_without_prefix() {
    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");

    let raw = "The quick brown fox jumps over the lazy dog.";
    let prepared = model
        .prepare_input(&crate::types::ModelInput::Text {
            content: raw.to_string(),
            instruction: Some("query".to_string()),
        })
        .expect("prepare");

    // BGE-M3 dense path does not prepend query:/passage: — raw text flows through.
    assert_eq!(prepared, raw);
}

#[test]
fn test_prepare_input_rejects_non_text_modalities() {
    use crate::types::ImageFormat;

    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");

    let err = model
        .prepare_input(&crate::types::ModelInput::Image {
            bytes: vec![0u8; 32],
            format: ImageFormat::Png,
        })
        .expect_err("image must be rejected");
    let msg = format!("{:?}", err);
    assert!(msg.contains("UnsupportedModality"), "unexpected: {}", msg);
}

// Weights-dependent tests follow. They are ignored by default because they
// load the multi-GB artifact, but still use the same fail-closed real artifact
// resolver as the rest of the pretrained integration tests.

#[test]
#[ignore = "requires real BAAI/bge-m3 weights under the ME-JEPA model artifact root"]
fn test_load_real_weights_populates_state() {
    use crate::traits::EmbeddingModel;

    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async { EmbeddingModel::load(&model).await })
        .expect("load real BAAI/bge-m3 weights");
    assert!(model.is_initialized());
}

#[test]
#[ignore = "requires real BAAI/bge-m3 weights under the ME-JEPA model artifact root"]
fn test_embed_real_text_produces_normalised_1024d_vector() {
    use crate::traits::EmbeddingModel;
    use crate::types::ModelInput;

    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        EmbeddingModel::load(&model).await.expect("load");
        let out = model
            .embed(&ModelInput::Text {
                content: "The quick brown fox.".to_string(),
                instruction: None,
            })
            .await
            .expect("embed");

        assert_eq!(out.vector.len(), BGE_M3_DENSE_DIMENSION);
        let norm: f32 = out.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!(
            "E14 real embed output: len={}, norm={:.6}, latency_us={}, first5={:?}",
            out.vector.len(),
            norm,
            out.latency_us,
            &out.vector[..5]
        );
        assert!(
            (norm - 1.0).abs() < 1e-3,
            "BGE-M3 output must be L2-normalised, got norm={}",
            norm
        );
    });
}

#[test]
#[ignore = "requires real BAAI/bge-m3 weights under the ME-JEPA model artifact root"]
fn test_embed_true_batch_real_text_produces_ordered_1024d_vectors() {
    use crate::traits::EmbeddingModel;
    use crate::types::ModelInput;

    let path = pretrained_test_model_path("bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default()).expect("new");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        EmbeddingModel::load(&model).await.expect("load");
        let inputs = vec![
            ModelInput::text("Python AST chunk with an assertion repair").expect("input"),
            ModelInput::text("SWE-bench oracle confirms the patch works").expect("input"),
        ];
        let out = EmbeddingModel::embed_true_batch(&model, &inputs)
            .await
            .expect("bge true batch");

        assert_eq!(out.len(), inputs.len());
        for embedding in &out {
            embedding.validate().expect("valid BGE true-batch vector");
            assert_eq!(embedding.vector.len(), BGE_M3_DENSE_DIMENSION);
        }
    });
}
