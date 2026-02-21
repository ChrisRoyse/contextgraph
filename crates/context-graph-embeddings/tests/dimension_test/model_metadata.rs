//! Model metadata tests for ModelId.
//!
//! Tests that verify ModelId enum properties like repr values and classifications.

use context_graph_embeddings::dimensions::MODEL_COUNT;
use context_graph_embeddings::ModelId;

/// Test ModelId repr(u8) values match index order.
#[test]
fn test_model_id_repr_order() {
    assert_eq!(ModelId::Semantic as u8, 0, "Semantic should be 0");
    assert_eq!(
        ModelId::TemporalRecent as u8,
        1,
        "TemporalRecent should be 1"
    );
    assert_eq!(
        ModelId::TemporalPeriodic as u8,
        2,
        "TemporalPeriodic should be 2"
    );
    assert_eq!(
        ModelId::TemporalPositional as u8,
        3,
        "TemporalPositional should be 3"
    );
    assert_eq!(ModelId::Causal as u8, 4, "Causal should be 4");
    assert_eq!(ModelId::Sparse as u8, 5, "Sparse should be 5");
    assert_eq!(ModelId::Code as u8, 6, "Code should be 6");
    assert_eq!(ModelId::Graph as u8, 7, "Graph should be 7");
    assert_eq!(ModelId::Hdc as u8, 8, "Hdc should be 8");
    assert_eq!(ModelId::Contextual as u8, 9, "Multimodal should be 9");
    assert_eq!(ModelId::Entity as u8, 10, "Entity should be 10");
    assert_eq!(
        ModelId::LateInteraction as u8,
        11,
        "LateInteraction should be 11"
    );
    assert_eq!(ModelId::Splade as u8, 12, "Splade should be 12");
    println!("[PASS] ModelId repr(u8) values match E1-E13 order");
}

/// Test is_custom() classification.
#[test]
fn test_is_custom_classification() {
    let custom_models = [
        ModelId::TemporalRecent,
        ModelId::TemporalPeriodic,
        ModelId::TemporalPositional,
        ModelId::Hdc,
    ];

    for model_id in ModelId::all() {
        let expected_custom = custom_models.contains(model_id);
        assert_eq!(
            model_id.is_custom(),
            expected_custom,
            "{:?}.is_custom() should be {}",
            model_id,
            expected_custom
        );
        assert_eq!(
            model_id.is_pretrained(),
            !expected_custom,
            "{:?}.is_pretrained() should be {}",
            model_id,
            !expected_custom
        );
    }
    println!("[PASS] is_custom() and is_pretrained() classifications verified");
}

/// Test custom models count.
#[test]
fn test_custom_models_count() {
    let custom_count: usize = ModelId::custom().count();
    assert_eq!(
        custom_count, 4,
        "Expected 4 custom models, got {}",
        custom_count
    );

    let pretrained_count: usize = ModelId::pretrained().count();
    assert_eq!(
        pretrained_count, 10,
        "Expected 10 pretrained models, got {}",
        pretrained_count
    );

    assert_eq!(
        custom_count + pretrained_count,
        MODEL_COUNT,
        "Custom + pretrained should equal MODEL_COUNT"
    );
    println!("[PASS] 4 custom + 10 pretrained = 14 total models");
}
