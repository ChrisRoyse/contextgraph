//! Tests for ModelId and TokenizerFamily.

use super::*;
use std::path::{Path, PathBuf};

#[test]
fn test_all_returns_15_variants() {
    // 15 variants since E14 BgeM3Dense joined Entity + Kepler + E1-E13.
    assert_eq!(ModelId::all().len(), 15);
}

#[test]
fn test_variant_order_matches_spec() {
    let all = ModelId::all();
    assert_eq!(all[0], ModelId::Semantic); // E1
    assert_eq!(all[4], ModelId::Causal); // E5
    assert_eq!(all[11], ModelId::LateInteraction); // E12
    assert_eq!(all[12], ModelId::Splade); // E13
}

#[test]
fn test_semantic_dimension() {
    assert_eq!(ModelId::Semantic.dimension(), 1024);
}

#[test]
fn test_temporal_custom_flag() {
    assert!(ModelId::TemporalRecent.is_custom());
    assert!(ModelId::TemporalPeriodic.is_custom());
    assert!(ModelId::TemporalPositional.is_custom());
    assert!(ModelId::Hdc.is_custom());
}

#[test]
fn test_pretrained_repo() {
    assert_eq!(ModelId::Semantic.model_repo(), Some("intfloat/e5-large-v2"));
    assert_eq!(ModelId::TemporalRecent.model_repo(), None);
}

#[test]
fn test_model_path() {
    let base = Path::new("/home/cabdru/contextgraph/models");
    assert_eq!(
        ModelId::Semantic.model_path(base),
        PathBuf::from("/home/cabdru/contextgraph/models/semantic")
    );
    assert_eq!(
        ModelId::LateInteraction.model_path(base),
        PathBuf::from("/home/cabdru/contextgraph/models/late-interaction")
    );
}

#[test]
fn test_max_tokens() {
    assert_eq!(ModelId::Causal.max_tokens(), 512);
    // EMB-1 FIX: Multimodal uses BERT tokenizer (512 tokens), not CLIP (77)
    assert_eq!(ModelId::Contextual.max_tokens(), 512);
    assert_eq!(ModelId::Semantic.max_tokens(), 512);
}

#[test]
fn test_u8_round_trip() {
    for id in ModelId::all() {
        let byte = *id as u8;
        let recovered = ModelId::try_from(byte).expect("valid u8 should convert");
        assert_eq!(*id, recovered);
    }
}

#[test]
fn test_serde_round_trip() {
    for id in ModelId::all() {
        let json = serde_json::to_string(id).expect("serialization should succeed");
        let recovered: ModelId =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(*id, recovered);
    }
}

#[test]
fn test_display() {
    assert_eq!(format!("{}", ModelId::Semantic), "Semantic (E1)");
    assert_eq!(
        format!("{}", ModelId::LateInteraction),
        "LateInteraction (E12)"
    );
}

#[test]
fn test_pretrained_count() {
    let pretrained: Vec<_> = ModelId::pretrained().collect();
    // 15 total - 4 custom = 11 pretrained (incl. E14 BgeM3Dense).
    assert_eq!(pretrained.len(), 11);
}

#[test]
fn test_custom_count() {
    let custom: Vec<_> = ModelId::custom().collect();
    assert_eq!(custom.len(), 4); // TemporalRecent, TemporalPeriodic, TemporalPositional, Hdc
}

#[test]
fn test_projected_dimensions() {
    // Sparse projects from ~30K to 1536
    assert_eq!(ModelId::Sparse.dimension(), 30522);
    assert_eq!(ModelId::Sparse.projected_dimension(), 1536);

    // Code is now native 1536D (Qodo-Embed-1-1.5B) - no projection needed
    assert_eq!(ModelId::Code.dimension(), 1536);
    assert_eq!(ModelId::Code.projected_dimension(), 1536);

    // HDC projects from 10K-bit to 1024
    assert_eq!(ModelId::Hdc.dimension(), 10000);
    assert_eq!(ModelId::Hdc.projected_dimension(), 1024);

    // Others unchanged
    assert_eq!(ModelId::Semantic.projected_dimension(), 1024);
}

#[test]
fn test_latency_budgets() {
    assert_eq!(ModelId::Semantic.latency_budget_ms(), 5);
    assert_eq!(ModelId::Hdc.latency_budget_ms(), 1);
    assert_eq!(ModelId::Contextual.latency_budget_ms(), 15);
}

#[test]
fn test_tokenizer_families() {
    // BERT family: Semantic, Sparse, Graph, Entity, LateInteraction
    assert_eq!(
        ModelId::Semantic.tokenizer_family(),
        TokenizerFamily::BertWordpiece
    );
    assert_eq!(
        ModelId::Sparse.tokenizer_family(),
        TokenizerFamily::BertWordpiece
    );

    // BERT family: Causal (nomic-embed uses BERT tokenizer)
    assert_eq!(
        ModelId::Causal.tokenizer_family(),
        TokenizerFamily::BertWordpiece
    );

    // BERT WordPiece family: Code (Qodo-Embed uses BERT tokenizer)
    assert_eq!(
        ModelId::Code.tokenizer_family(),
        TokenizerFamily::BertWordpiece
    );

    // EMB-1 FIX: Multimodal uses BERT family, not CLIP (e5-base-v2 is BERT-based)
    assert_eq!(
        ModelId::Contextual.tokenizer_family(),
        TokenizerFamily::BertWordpiece
    );

    // Custom: no tokenizer
    assert_eq!(
        ModelId::TemporalRecent.tokenizer_family(),
        TokenizerFamily::None
    );
}

#[test]
fn test_invalid_u8_conversion() {
    // Before: attempt conversion of invalid value (15 is outside valid range 0-14)
    let result = ModelId::try_from(15u8);

    // After: verify error
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Invalid ModelId: must be 0-14");
    println!("Edge Case 1 PASSED: Invalid u8 (15) correctly rejected");
}

#[test]
fn test_maximum_enum_value() {
    // Before: get max valid value (BgeM3Dense = 14 is now the max)
    let max_valid = ModelId::BgeM3Dense as u8;
    println!("Before: max valid u8 = {}", max_valid);

    // After: verify round-trip
    let recovered = ModelId::try_from(max_valid).expect("max valid should convert");
    assert_eq!(recovered, ModelId::BgeM3Dense);
    println!("After: recovered = {:?}", recovered);
    println!("Edge Case 2 PASSED: Maximum value (14) converts correctly");
}

#[test]
fn test_custom_model_no_repo() {
    // Before: check all custom models
    for model in ModelId::custom() {
        println!("Before: checking {:?}", model);
        let repo = model.model_repo();

        // After: verify None
        assert!(
            repo.is_none(),
            "Custom model {:?} should have no repo",
            model
        );
        println!("After: {:?}.model_repo() = None (correct)", model);
    }
    println!("Edge Case 3 PASSED: All 4 custom models return None for repo");
}

#[test]
fn test_production_returns_14_variants() {
    // EMB-M2: production() must return exactly NUM_EMBEDDERS (14) models after E14.
    let prod = ModelId::production();
    assert_eq!(prod.len(), 14, "production() must return 14 models");
}

#[test]
fn test_production_excludes_entity() {
    // EMB-M2: production() excludes legacy Entity variant
    let prod = ModelId::production();
    assert!(
        !prod.contains(&ModelId::Entity),
        "production() must NOT contain legacy Entity"
    );
}

#[test]
fn test_production_includes_kepler() {
    // EMB-M2: production() includes Kepler as the E11 production replacement
    let prod = ModelId::production();
    assert!(
        prod.contains(&ModelId::Kepler),
        "production() must contain Kepler"
    );
}

#[test]
fn test_production_order_matches_pipeline() {
    // EMB-M2: Verify production() order is the active E1-E14 slot order.
    let prod = ModelId::production();
    assert_eq!(prod[0], ModelId::Semantic); // E1
    assert_eq!(prod[4], ModelId::Causal); // E5
    assert_eq!(prod[9], ModelId::Contextual); // E10
    assert_eq!(prod[10], ModelId::Kepler); // E11 production
    assert_eq!(prod[11], ModelId::LateInteraction); // E12
    assert_eq!(prod[12], ModelId::Splade); // E13
    assert_eq!(prod[13], ModelId::BgeM3Dense); // E14
}
