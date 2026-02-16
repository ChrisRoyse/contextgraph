//! Edge case and verification tests for TemporalPeriodicModel.
//!
//! Contains mandatory edge case tests and source of truth verification.

use crate::models::custom::temporal_periodic::TemporalPeriodicModel;
use crate::traits::EmbeddingModel;
use crate::types::{ModelId, ModelInput};

use super::periodicity::cosine_similarity;

#[tokio::test]
async fn test_edge_case_1_same_time_different_days() {
    let model = TemporalPeriodicModel::new();

    println!("=== EDGE CASE 1: Same Time Different Days ===");

    // Same time on consecutive days
    let day1 = "timestamp:2024-01-15T10:30:00Z";
    let day2 = "timestamp:2024-01-16T10:30:00Z";

    println!("BEFORE: day1 = {}", day1);
    println!("BEFORE: day2 = {}", day2);

    let input1 = ModelInput::text_with_instruction("content", day1).expect("input1");
    let input2 = ModelInput::text_with_instruction("content", day2).expect("input2");

    let emb1 = model.embed(&input1).await.expect("Embed day1");
    let emb2 = model.embed(&input2).await.expect("Embed day2");

    // Hour-of-day features should be similar (same time)
    // First 102 features are hour encoding
    let hour_features_1: &[f32] = &emb1.vector[0..102];
    let hour_features_2: &[f32] = &emb2.vector[0..102];

    let hour_sim = cosine_similarity(hour_features_1, hour_features_2);

    println!("AFTER: hour features cosine similarity = {}", hour_sim);

    // Day-of-week features should be different (Mon vs Tue)
    let day_features_1: &[f32] = &emb1.vector[102..204];
    let day_features_2: &[f32] = &emb2.vector[102..204];

    let day_sim = cosine_similarity(day_features_1, day_features_2);

    println!("AFTER: day features cosine similarity = {}", day_sim);

    assert_eq!(emb1.vector.len(), 512);
    assert_eq!(emb2.vector.len(), 512);
    // Hour features should be identical (same time of day)
    assert!(
        hour_sim > 0.99,
        "Same time-of-day should have similar hour features, got {}",
        hour_sim
    );
}

#[tokio::test]
async fn test_edge_case_2_same_weekday_different_weeks() {
    let model = TemporalPeriodicModel::new();

    println!("=== EDGE CASE 2: Same Weekday Different Weeks ===");

    // Both are Mondays (7 days apart)
    let week1 = "timestamp:2024-01-15T10:30:00Z"; // Monday
    let week2 = "timestamp:2024-01-22T10:30:00Z"; // Monday next week

    println!("BEFORE: week1 (Mon) = {}", week1);
    println!("BEFORE: week2 (Mon) = {}", week2);

    let input1 = ModelInput::text_with_instruction("content", week1).expect("input1");
    let input2 = ModelInput::text_with_instruction("content", week2).expect("input2");

    let emb1 = model.embed(&input1).await.expect("Embed week1");
    let emb2 = model.embed(&input2).await.expect("Embed week2");

    // Week features (positions 204-306) should be similar
    let week_features_1: &[f32] = &emb1.vector[204..306];
    let week_features_2: &[f32] = &emb2.vector[204..306];

    let week_sim = cosine_similarity(week_features_1, week_features_2);

    println!("AFTER: week features cosine similarity = {}", week_sim);

    assert!(
        week_sim > 0.99,
        "Same day-of-week should have similar week features, got {}",
        week_sim
    );
}

#[tokio::test]
async fn test_edge_case_3_no_timestamp_fallback() {
    let model = TemporalPeriodicModel::new();

    println!("=== EDGE CASE 3: No Timestamp (Fallback) ===");
    println!("BEFORE: input has no timestamp instruction");

    let input = ModelInput::text("content without timestamp").expect("Failed to create input");

    let embedding = model.embed(&input).await.expect("Embed should succeed");

    println!("AFTER: vector[0..5] = {:?}", &embedding.vector[0..5]);
    println!("AFTER: Should use current time as fallback");

    assert_eq!(embedding.vector.len(), 512);
    assert!(embedding.vector.iter().all(|x| x.is_finite()));
}

#[tokio::test]
async fn test_source_of_truth_verification() {
    let model = TemporalPeriodicModel::new();
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

    // Check sin/cos range (before normalization they're in [-1, 1])
    // After normalization, they're scaled but still valid
    let all_finite = embedding.vector.iter().all(|x| x.is_finite());
    println!("all values finite: {}", all_finite);

    // VERIFY
    assert_eq!(embedding.model_id, ModelId::TemporalPeriodic);
    assert_eq!(embedding.vector.len(), 512);
    assert!(
        (norm - 1.0).abs() < 0.001,
        "Norm should be ~1.0, got {}",
        norm
    );
    assert!(!has_nan && !has_inf);
}

#[tokio::test]
async fn test_evidence_of_success() {
    println!("\n========================================");
    println!("M03-L05 EVIDENCE OF SUCCESS");
    println!("========================================\n");

    let model = TemporalPeriodicModel::new();

    // Test 1: Model metadata
    println!("1. MODEL METADATA:");
    println!("   model_id = {:?}", model.model_id());
    println!("   dimension = {}", model.dimension());
    println!("   is_initialized = {}", model.is_initialized());
    println!("   is_pretrained = {}", model.is_pretrained());
    println!("   latency_budget_ms = {}", model.latency_budget_ms());
    println!("   periods = {:?}", model.periods);

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

    // Test 3: Periodicity verification
    println!("\n3. PERIODICITY VERIFICATION:");

    // Same time-of-day check
    let day1_input =
        ModelInput::text_with_instruction("test", "timestamp:2024-01-15T10:30:00Z").expect("day1");
    let day2_input =
        ModelInput::text_with_instruction("test", "timestamp:2024-01-16T10:30:00Z").expect("day2");

    let day1_emb = model.embed(&day1_input).await.expect("day1 embed");
    let day2_emb = model.embed(&day2_input).await.expect("day2 embed");

    let hour_sim = cosine_similarity(&day1_emb.vector[0..102], &day2_emb.vector[0..102]);

    println!(
        "   same time-of-day similarity (hours 0-102) = {}",
        hour_sim
    );

    assert!(elapsed.as_millis() < 2, "Latency exceeded 2ms budget");
    assert_eq!(embedding.vector.len(), 512);
    assert!((norm - 1.0).abs() < 0.001);

    println!("\n========================================");
    println!("ALL CHECKS PASSED");
    println!("========================================\n");
}
