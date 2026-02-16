//! Source of truth and evidence verification tests for TemporalPositionalModel.

use crate::traits::EmbeddingModel;
use crate::types::{ModelId, ModelInput};

use super::super::TemporalPositionalModel;

// =========================================================================
// SOURCE OF TRUTH VERIFICATION (MANDATORY per spec)
// =========================================================================

#[tokio::test]
async fn test_source_of_truth_verification() {
    let model = TemporalPositionalModel::new();
    let input = ModelInput::text_with_instruction("test content", "timestamp:2024-01-15T10:30:00Z")
        .expect("Failed to create input");

    // Execute
    let embedding = model.embed(&input).await.expect("Embed should succeed");

    // INSPECT SOURCE OF TRUTH
    println!("=== SOURCE OF TRUTH VERIFICATION ===");
    println!("model_id: {:?}", embedding.model_id);
    println!("vector.len(): {}", embedding.vector.len());
    println!("vector[0..10]: {:?}", &embedding.vector[0..10]);
    println!("latency_us: {}", embedding.latency_us);

    let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    println!("L2 norm: {}", norm);

    let has_nan = embedding.vector.iter().any(|x| x.is_nan());
    let has_inf = embedding.vector.iter().any(|x| x.is_infinite());
    println!("has_nan: {}, has_inf: {}", has_nan, has_inf);

    let all_finite = embedding.vector.iter().all(|x| x.is_finite());
    println!("all values finite: {}", all_finite);

    // VERIFY
    assert_eq!(embedding.model_id, ModelId::TemporalPositional);
    assert_eq!(embedding.vector.len(), 512);
    assert!(
        (norm - 1.0).abs() < 0.001,
        "Norm should be ~1.0, got {}",
        norm
    );
    assert!(!has_nan && !has_inf);
}

// =========================================================================
// EVIDENCE OF SUCCESS TEST (MANDATORY per spec)
// =========================================================================

#[tokio::test]
async fn test_evidence_of_success() {
    println!("\n========================================");
    println!("M03-L06 EVIDENCE OF SUCCESS");
    println!("========================================\n");

    let model = TemporalPositionalModel::new();

    // Test 1: Model metadata
    println!("1. MODEL METADATA:");
    println!("   model_id = {:?}", model.model_id());
    println!("   dimension = {}", model.dimension());
    println!("   is_initialized = {}", model.is_initialized());
    println!("   is_pretrained = {}", model.is_pretrained());
    println!("   latency_budget_ms = {}", model.latency_budget_ms());
    println!("   base = {}", model.base());

    // Test 2: Embed and verify output
    let input = ModelInput::text_with_instruction("test", "timestamp:2024-01-15T10:30:00Z")
        .expect("Failed to create input");

    let start = std::time::Instant::now();
    let embedding = model.embed(&input).await.expect("Embed should succeed");
    let elapsed = start.elapsed();

    println!("\n2. EMBEDDING OUTPUT:");
    println!("   vector length = {}", embedding.vector.len());
    println!("   latency = {:?}", elapsed);
    println!("   first 10 values = {:?}", &embedding.vector[0..10]);

    let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    println!("   L2 norm = {}", norm);

    // Test 3: Uniqueness verification
    println!("\n3. UNIQUENESS VERIFICATION:");

    let ts1_input =
        ModelInput::text_with_instruction("test", "timestamp:2024-01-15T10:30:00Z").expect("ts1");
    let ts2_input =
        ModelInput::text_with_instruction("test", "timestamp:2024-01-15T10:30:01Z").expect("ts2");

    let emb1 = model.embed(&ts1_input).await.expect("emb1");
    let emb2 = model.embed(&ts2_input).await.expect("emb2");

    let vectors_differ = emb1.vector != emb2.vector;
    println!("   1-second-apart timestamps differ = {}", vectors_differ);

    // Test 4: Determinism
    println!("\n4. DETERMINISM CHECK:");
    let emb1_again = model.embed(&ts1_input).await.expect("emb1 again");
    let is_deterministic = emb1.vector == emb1_again.vector;
    println!("   same timestamp same output = {}", is_deterministic);

    assert!(elapsed.as_millis() < 2, "Latency exceeded 2ms budget");
    assert_eq!(embedding.vector.len(), 512);
    assert!((norm - 1.0).abs() < 0.001);
    assert!(vectors_differ, "Different timestamps must differ");
    assert!(is_deterministic, "Same timestamp must be deterministic");

    println!("\n========================================");
    println!("ALL CHECKS PASSED");
    println!("========================================\n");
}

// =========================================================================
// COMPARISON WITH OTHER TEMPORAL MODELS
// =========================================================================

/// Verify E4 produces different output than what E2/E3 would produce.
/// This confirms the models are distinct.
#[tokio::test]
async fn test_e4_distinct_from_other_temporal_models() {
    use crate::models::TemporalPeriodicModel;
    use crate::models::TemporalRecentModel;

    let e2 = TemporalRecentModel::new();
    let e3 = TemporalPeriodicModel::new();
    let e4 = TemporalPositionalModel::new();

    let input = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:00Z")
        .expect("input");

    let emb2 = e2.embed(&input).await.expect("E2 embed");
    let emb3 = e3.embed(&input).await.expect("E3 embed");
    let emb4 = e4.embed(&input).await.expect("E4 embed");

    // All should be 512D
    assert_eq!(emb2.vector.len(), 512);
    assert_eq!(emb3.vector.len(), 512);
    assert_eq!(emb4.vector.len(), 512);

    // But different model IDs
    assert_eq!(emb2.model_id, ModelId::TemporalRecent);
    assert_eq!(emb3.model_id, ModelId::TemporalPeriodic);
    assert_eq!(emb4.model_id, ModelId::TemporalPositional);

    // And different values
    assert_ne!(emb2.vector, emb4.vector, "E2 and E4 should differ");
    assert_ne!(emb3.vector, emb4.vector, "E3 and E4 should differ");
}
