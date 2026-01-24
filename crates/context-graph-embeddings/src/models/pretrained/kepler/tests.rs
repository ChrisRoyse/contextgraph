//! Tests for KeplerModel.

use super::*;

#[test]
fn test_kepler_dimension() {
    assert_eq!(KEPLER_DIMENSION, 768);
}

#[test]
fn test_kepler_max_tokens() {
    assert_eq!(KEPLER_MAX_TOKENS, 512);
}

#[test]
fn test_encode_entity_with_type() {
    let text = KeplerModel::encode_entity("Paris", Some("location"));
    assert_eq!(text, "[LOCATION] Paris");
}

#[test]
fn test_encode_entity_without_type() {
    let text = KeplerModel::encode_entity("France", None);
    assert_eq!(text, "France");
}

#[test]
fn test_encode_relation() {
    let text = KeplerModel::encode_relation("capital_of");
    assert_eq!(text, "capital of");
}

#[test]
fn test_encode_triple() {
    let text = KeplerModel::encode_triple("Paris", "capital_of", "France");
    assert_eq!(text, "Paris capital of France");
}

#[test]
fn test_transe_score_perfect() {
    let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
    let r: Vec<f32> = vec![0.5; KEPLER_DIMENSION];
    let t: Vec<f32> = vec![1.5; KEPLER_DIMENSION];

    let score = KeplerModel::transe_score(&h, &r, &t);
    assert!(score.abs() < 1e-5, "Perfect triple should have score ~0");
}

#[test]
fn test_transe_score_imperfect() {
    let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
    let r: Vec<f32> = vec![0.5; KEPLER_DIMENSION];
    let t: Vec<f32> = vec![2.0; KEPLER_DIMENSION]; // Wrong tail

    let score = KeplerModel::transe_score(&h, &r, &t);
    assert!(score < 0.0, "Imperfect triple should have negative score");
}

#[test]
fn test_predict_tail() {
    let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
    let r: Vec<f32> = vec![0.5; KEPLER_DIMENSION];

    let predicted = KeplerModel::predict_tail(&h, &r);
    assert_eq!(predicted.len(), KEPLER_DIMENSION);
    assert!((predicted[0] - 1.5).abs() < 1e-5);
}

#[test]
fn test_predict_relation() {
    let h: Vec<f32> = vec![1.0; KEPLER_DIMENSION];
    let t: Vec<f32> = vec![1.5; KEPLER_DIMENSION];

    let predicted = KeplerModel::predict_relation(&h, &t);
    assert_eq!(predicted.len(), KEPLER_DIMENSION);
    assert!((predicted[0] - 0.5).abs() < 1e-5);
}

#[test]
fn test_score_to_confidence() {
    // Perfect score (0) should give confidence 1.0
    assert!((KeplerModel::score_to_confidence(0.0) - 1.0).abs() < 1e-5);

    // Very bad score (-15) should give confidence 0.0
    assert!((KeplerModel::score_to_confidence(-15.0) - 0.0).abs() < 1e-5);

    // Middle score (-7.5) should give confidence 0.5
    assert!((KeplerModel::score_to_confidence(-7.5) - 0.5).abs() < 1e-5);
}

#[test]
fn test_validation_from_score() {
    assert_eq!(KeplerModel::validation_from_score(-3.0), "VALID");
    assert_eq!(KeplerModel::validation_from_score(-7.0), "UNCERTAIN");
    assert_eq!(KeplerModel::validation_from_score(-12.0), "INVALID");

    // Boundary tests
    assert_eq!(KeplerModel::validation_from_score(-5.0), "UNCERTAIN");
    assert_eq!(KeplerModel::validation_from_score(-10.0), "UNCERTAIN");
    assert_eq!(KeplerModel::validation_from_score(-10.1), "INVALID");
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::path::Path;

    /// Integration test that requires the KEPLER model to be downloaded.
    /// Run with: cargo test --features real-embeddings kepler_integration
    #[tokio::test]
    #[ignore = "requires KEPLER model weights"]
    async fn test_kepler_load_and_embed() {
        use crate::traits::{EmbeddingModel, SingleModelConfig};

        let model_path = Path::new("/home/cabdru/contextgraph/models/kepler");
        if !model_path.exists() {
            eprintln!("Skipping test: KEPLER model not found at {:?}", model_path);
            return;
        }

        let model = KeplerModel::new(model_path, SingleModelConfig::default())
            .expect("Failed to create KeplerModel");

        model.load().await.expect("Failed to load KeplerModel");

        assert!(model.is_initialized());

        // Test embedding
        let input = crate::types::ModelInput::Text {
            content: "Paris is the capital of France".to_string(),
            instruction: None,
        };

        let embedding = model.embed(&input).await.expect("Failed to embed");

        assert_eq!(embedding.vector.len(), KEPLER_DIMENSION);
        assert_eq!(embedding.model_id, crate::types::ModelId::Kepler);
    }

    /// Test TransE semantics with real KEPLER embeddings.
    #[tokio::test]
    #[ignore = "requires KEPLER model weights"]
    async fn test_kepler_transe_semantics() {
        use crate::traits::{EmbeddingModel, SingleModelConfig};

        let model_path = Path::new("/home/cabdru/contextgraph/models/kepler");
        if !model_path.exists() {
            eprintln!("Skipping test: KEPLER model not found at {:?}", model_path);
            return;
        }

        let model = KeplerModel::new(model_path, SingleModelConfig::default())
            .expect("Failed to create KeplerModel");

        model.load().await.expect("Failed to load KeplerModel");

        // Embed known entities and relation
        let paris_input = crate::types::ModelInput::Text {
            content: KeplerModel::encode_entity("Paris", Some("LOCATION")),
            instruction: None,
        };
        let france_input = crate::types::ModelInput::Text {
            content: KeplerModel::encode_entity("France", Some("LOCATION")),
            instruction: None,
        };
        let germany_input = crate::types::ModelInput::Text {
            content: KeplerModel::encode_entity("Germany", Some("LOCATION")),
            instruction: None,
        };
        let capital_input = crate::types::ModelInput::Text {
            content: KeplerModel::encode_relation("capital_of"),
            instruction: None,
        };

        let paris = model.embed(&paris_input).await.unwrap().vector;
        let france = model.embed(&france_input).await.unwrap().vector;
        let germany = model.embed(&germany_input).await.unwrap().vector;
        let capital = model.embed(&capital_input).await.unwrap().vector;

        // Valid triple: Paris is capital of France
        let valid_score = KeplerModel::transe_score(&paris, &capital, &france);

        // Invalid triple: Paris is capital of Germany
        let invalid_score = KeplerModel::transe_score(&paris, &capital, &germany);

        println!("Valid triple (Paris, capital_of, France): {}", valid_score);
        println!(
            "Invalid triple (Paris, capital_of, Germany): {}",
            invalid_score
        );

        // KEPLER should distinguish these - valid should have higher score
        assert!(
            valid_score > invalid_score,
            "KEPLER should score valid triple higher than invalid"
        );
    }
}
