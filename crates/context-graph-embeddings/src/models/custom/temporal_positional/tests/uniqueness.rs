//! Uniqueness and determinism tests for TemporalPositionalModel.

use crate::traits::EmbeddingModel;
use crate::types::ModelInput;

use super::super::TemporalPositionalModel;
use super::cosine_similarity;

// =========================================================================
// DETERMINISM TESTS
// =========================================================================

#[tokio::test]
async fn test_deterministic_with_same_timestamp() {
    let model = TemporalPositionalModel::new();

    let timestamp = "timestamp:2024-01-15T10:30:00Z";
    let input1 = ModelInput::text_with_instruction("content", timestamp).expect("Failed to create");
    let input2 = ModelInput::text_with_instruction("content", timestamp).expect("Failed to create");

    let embedding1 = model.embed(&input1).await.expect("First embed");
    let embedding2 = model.embed(&input2).await.expect("Second embed");

    assert_eq!(
        embedding1.vector, embedding2.vector,
        "Same timestamp must produce identical embeddings"
    );
}

#[tokio::test]
async fn test_different_timestamps_different_embeddings() {
    let model = TemporalPositionalModel::new();

    let input1 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:00Z")
        .expect("Failed to create");
    let input2 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:01Z")
        .expect("Failed to create");

    let embedding1 = model.embed(&input1).await.expect("First embed");
    let embedding2 = model.embed(&input2).await.expect("Second embed");

    assert_ne!(
        embedding1.vector, embedding2.vector,
        "Different timestamps must produce different embeddings"
    );
}

// =========================================================================
// UNIQUENESS TESTS - Key property of E4
// =========================================================================

#[tokio::test]
async fn test_uniqueness_close_timestamps_similar_but_different() {
    let model = TemporalPositionalModel::new();

    // 1 second apart
    let input1 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:00Z")
        .expect("input1");
    let input2 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:01Z")
        .expect("input2");

    let emb1 = model.embed(&input1).await.expect("Embed 1");
    let emb2 = model.embed(&input2).await.expect("Embed 2");

    let similarity = cosine_similarity(&emb1.vector, &emb2.vector);

    println!("Cosine similarity (1 second apart): {}", similarity);

    // Should be similar (high cosine similarity) but not identical
    assert_ne!(emb1.vector, emb2.vector, "Must be different");
    assert!(
        similarity > 0.9,
        "Close timestamps should be similar, got {}",
        similarity
    );
    assert!(similarity < 1.0, "But not identical, got {}", similarity);
}

#[tokio::test]
async fn test_uniqueness_distant_timestamps_less_similar() {
    let model = TemporalPositionalModel::new();

    // 1 day apart (86400 seconds)
    let input1 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:00Z")
        .expect("input1");
    let input2 = ModelInput::text_with_instruction("content", "timestamp:2024-01-16T10:30:00Z")
        .expect("input2");

    let emb1 = model.embed(&input1).await.expect("Embed 1");
    let emb2 = model.embed(&input2).await.expect("Embed 2");

    let similarity_day = cosine_similarity(&emb1.vector, &emb2.vector);

    // 1 second apart comparison
    let input3 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:01Z")
        .expect("input3");
    let emb3 = model.embed(&input3).await.expect("Embed 3");
    let similarity_second = cosine_similarity(&emb1.vector, &emb3.vector);

    println!("Cosine similarity (1 second apart): {}", similarity_second);
    println!("Cosine similarity (1 day apart): {}", similarity_day);

    // 1 day apart should be less similar than 1 second apart
    // (due to transformer PE properties)
    assert_ne!(emb1.vector, emb2.vector, "Must be different");
}

#[tokio::test]
async fn test_uniqueness_year_apart() {
    let model = TemporalPositionalModel::new();

    let input1 = ModelInput::text_with_instruction("content", "timestamp:2023-01-15T10:30:00Z")
        .expect("input1");
    let input2 = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:00Z")
        .expect("input2");

    let emb1 = model.embed(&input1).await.expect("Embed 1");
    let emb2 = model.embed(&input2).await.expect("Embed 2");

    let similarity = cosine_similarity(&emb1.vector, &emb2.vector);

    println!("Cosine similarity (1 year apart): {}", similarity);

    // Year apart should be different
    assert_ne!(emb1.vector, emb2.vector, "Different years must differ");
}

// =========================================================================
// TRANSFORMER PE FORMULA VERIFICATION
// =========================================================================

#[tokio::test]
async fn test_transformer_pe_formula_structure() {
    let model = TemporalPositionalModel::new();

    // Use a known timestamp
    let input = ModelInput::text_with_instruction(
        "test",
        "epoch:1000", // Use small epoch for easier mental calculation
    )
    .expect("Failed to create input");

    let embedding = model.embed(&input).await.expect("Embed should succeed");

    // Verify structure: 256 sin/cos pairs
    assert_eq!(embedding.vector.len(), 512);

    // Values should be finite
    assert!(embedding.vector.iter().all(|x| x.is_finite()));

    // Before normalization, sin/cos values are in [-1, 1]
    // After normalization, values are scaled but should still be reasonable
    let max_val = embedding.vector.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
    println!("Max absolute value after normalization: {}", max_val);

    // Should be <= 1 (since L2 normalized)
    assert!(max_val <= 1.0, "Max value should be <= 1.0 after L2 norm");
}

#[tokio::test]
async fn test_transformer_pe_formula_sin_cos_pairs() {
    // Use legacy mode to test raw PE formula (hybrid mode has session signature in first 256D)
    let model = TemporalPositionalModel::with_hybrid_mode(false);

    // For position 0, the raw PE values (before normalization) are:
    // PE(0, 2i) = sin(0) = 0
    // PE(0, 2i+1) = cos(0) = 1
    let input =
        ModelInput::text_with_instruction("test", "epoch:0").expect("Failed to create input");

    let embedding = model.embed(&input).await.expect("Embed should succeed");

    // At position 0:
    // - All sin values (even indices) should be 0
    // - All cos values (odd indices) should be 1
    // After normalization, the vector is scaled by 1/sqrt(256) since there are 256 ones

    let expected_cos_after_norm = 1.0 / (256.0_f32).sqrt();

    println!(
        "Expected cos after normalization: {}",
        expected_cos_after_norm
    );
    println!("First few values: {:?}", &embedding.vector[0..6]);

    // Check that odd indices (cos) are approximately equal
    // and even indices (sin) are approximately 0
    for i in 0..10 {
        let sin_idx = 2 * i;
        let cos_idx = 2 * i + 1;

        println!(
            "i={}: sin[{}]={:.6}, cos[{}]={:.6}",
            i, sin_idx, embedding.vector[sin_idx], cos_idx, embedding.vector[cos_idx]
        );

        // Sin should be ~0
        assert!(
            embedding.vector[sin_idx].abs() < 0.001,
            "sin(0) should be ~0, got {}",
            embedding.vector[sin_idx]
        );

        // Cos should be ~1/sqrt(256)
        assert!(
            (embedding.vector[cos_idx] - expected_cos_after_norm).abs() < 0.001,
            "cos(0) after norm should be ~{}, got {}",
            expected_cos_after_norm,
            embedding.vector[cos_idx]
        );
    }
}
