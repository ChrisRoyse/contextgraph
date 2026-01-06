//! Comprehensive tests for cross-space similarity engine.
//!
//! # Testing Philosophy
//!
//! Per TASK-L007 requirements:
//! - **NO MOCK DATA** - All tests use real computed values
//! - **Deterministic** - Seeded random for reproducibility
//! - **Edge cases** - Empty, identical, single-item scenarios
//! - **Performance** - Verify <5ms pair, <50ms batch 100

use super::*;
use crate::types::fingerprint::{
    JohariFingerprint, PurposeVector, SemanticFingerprint, SparseVector,
    TeleologicalFingerprint, NUM_EMBEDDERS,
};
use uuid::Uuid;

/// Create a test fingerprint with deterministic data based on seed.
///
/// Uses linear congruential generator for reproducibility.
/// This ensures tests are deterministic across runs.
fn create_test_fingerprint(seed: u64) -> TeleologicalFingerprint {
    // Simple LCG for reproducible "random" values
    let mut state = seed;
    let lcg_next = |s: &mut u64| -> f32 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        // Map to [-1, 1] range
        ((*s >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
    };

    // Create semantic fingerprint with populated embeddings
    let mut semantic = SemanticFingerprint::zeroed();

    // Populate E1 (1024D)
    for v in semantic.e1_semantic.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E2 (512D)
    for v in semantic.e2_temporal_recent.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E3 (512D)
    for v in semantic.e3_temporal_periodic.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E4 (512D)
    for v in semantic.e4_temporal_positional.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E5 (768D)
    for v in semantic.e5_causal.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E6 sparse (10 active elements)
    let indices: Vec<u16> = (0..10).map(|i| (i * 100) as u16).collect();
    let values: Vec<f32> = (0..10).map(|_| lcg_next(&mut state).abs()).collect();
    semantic.e6_sparse = SparseVector { indices, values };

    // Populate E7 (256D)
    for v in semantic.e7_code.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E8 (384D)
    for v in semantic.e8_graph.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E9 (10000D) - just first 100 for efficiency
    for v in semantic.e9_hdc.iter_mut().take(100) {
        *v = lcg_next(&mut state);
    }

    // Populate E10 (768D)
    for v in semantic.e10_multimodal.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E11 (384D)
    for v in semantic.e11_entity.iter_mut() {
        *v = lcg_next(&mut state);
    }

    // Populate E12 (ColBERT tokens) - 5 tokens of 128D each
    semantic.e12_late_interaction = (0..5)
        .map(|_| (0..128).map(|_| lcg_next(&mut state)).collect())
        .collect();

    // Populate E13 sparse (10 active elements)
    let indices13: Vec<u16> = (0..10).map(|i| (i * 200) as u16).collect();
    let values13: Vec<f32> = (0..10).map(|_| lcg_next(&mut state).abs()).collect();
    semantic.e13_splade = SparseVector {
        indices: indices13,
        values: values13,
    };

    // Create purpose vector
    let alignments: [f32; NUM_EMBEDDERS] = std::array::from_fn(|_| lcg_next(&mut state));
    let purpose_vector = PurposeVector::new(alignments);

    // Create johari fingerprint
    let johari = JohariFingerprint::default();

    // Create content hash
    let content_hash = [seed as u8; 32];

    TeleologicalFingerprint::new(semantic, purpose_vector, johari, content_hash)
}

/// Create a fingerprint with all zero embeddings (edge case).
fn create_empty_fingerprint() -> TeleologicalFingerprint {
    let semantic = SemanticFingerprint::zeroed();
    let purpose_vector = PurposeVector::default();
    let johari = JohariFingerprint::default();
    let content_hash = [0u8; 32];
    TeleologicalFingerprint::new(semantic, purpose_vector, johari, content_hash)
}

// ============================================================================
// Core Similarity Tests
// ============================================================================

#[tokio::test]
async fn test_uniform_weighting_produces_valid_score() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    let config = CrossSpaceConfig {
        weighting_strategy: WeightingStrategy::Uniform,
        include_breakdown: true,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await;

    assert!(result.is_ok(), "Should succeed: {:?}", result.err());
    let sim = result.unwrap();

    println!(
        "[TEST] score={:.4}, confidence={:.4}, active={}/{}",
        sim.score,
        sim.confidence,
        sim.active_count(),
        NUM_EMBEDDERS
    );

    assert!(
        sim.score >= 0.0 && sim.score <= 1.0,
        "Score out of range: {}",
        sim.score
    );
    assert!(sim.confidence >= 0.0 && sim.confidence <= 1.0);
    assert!(sim.space_scores.is_some(), "Breakdown should be included");
    assert!(
        sim.active_count() > 0,
        "Should have at least one active space"
    );
}

#[tokio::test]
async fn test_insufficient_spaces_returns_error() {
    let engine = DefaultCrossSpaceEngine::new();

    // Create fingerprints with no populated embeddings
    let fp1 = create_empty_fingerprint();
    let fp2 = create_empty_fingerprint();

    let config = CrossSpaceConfig {
        min_active_spaces: 5, // Require 5 but have 0
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await;

    println!("[TEST] result: {:?}", result);

    assert!(
        matches!(
            result,
            Err(SimilarityError::InsufficientSpaces { required: 5, .. })
        ),
        "Expected InsufficientSpaces error, got {:?}",
        result
    );
}

#[tokio::test]
async fn test_rrf_uses_existing_implementation() {
    let engine = DefaultCrossSpaceEngine::new();

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    let ranked_lists = vec![
        (0, vec![id1, id2]), // Space 0: id1=rank0, id2=rank1
        (1, vec![id2, id1]), // Space 1: id2=rank0, id1=rank1
    ];

    let scores = engine.compute_rrf_from_ranks(&ranked_lists, 60.0);

    // id1: 1/(60+0+1) + 1/(60+1+1) = 1/61 + 1/62
    // id2: 1/(60+1+1) + 1/(60+0+1) = 1/62 + 1/61
    // Should be equal (symmetric case)
    let score1 = scores.get(&id1).unwrap();
    let score2 = scores.get(&id2).unwrap();

    println!("[TEST] RRF id1={:.6}, id2={:.6}", score1, score2);

    // Both should be: 1/61 + 1/62 ≈ 0.03260
    let expected = 1.0 / 61.0 + 1.0 / 62.0;

    assert!(
        (score1 - expected).abs() < 0.0001,
        "RRF score1 mismatch: {} vs expected {}",
        score1,
        expected
    );
    assert!(
        (score1 - score2).abs() < 0.0001,
        "RRF symmetric case failed: {} != {}",
        score1,
        score2
    );
}

#[tokio::test]
async fn test_multi_utl_formula_computes_correctly() {
    let engine = DefaultCrossSpaceEngine::new();

    let params = MultiUtlParams {
        semantic_deltas: [0.1; NUM_EMBEDDERS],
        coherence_deltas: [0.1; NUM_EMBEDDERS],
        tau_weights: [1.0; NUM_EMBEDDERS],
        lambda_s: 1.0,
        lambda_c: 1.0,
        w_e: 1.0,
        phi: 0.0, // cos(0) = 1
    };

    let score = engine.compute_multi_utl(&params).await;

    // L = sigmoid(2.0 * sum * sum * 1 * 1)
    // sum = 13 * 1.0 * 1.0 * 0.1 = 1.3
    // raw = 2.0 * 1.3 * 1.3 * 1 * 1 = 3.38
    // sigmoid(3.38) ≈ 0.967

    println!("[TEST] Multi-UTL score={:.4}", score);

    assert!(
        score > 0.9 && score < 1.0,
        "Multi-UTL unexpected: {}",
        score
    );
}

#[tokio::test]
async fn test_identical_fingerprints_produce_high_similarity() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp = create_test_fingerprint(42);

    let config = CrossSpaceConfig::default();

    let result = engine.compute_similarity(&fp, &fp, &config).await.unwrap();

    println!("[TEST] Self-similarity score={:.4}", result.score);

    // Same fingerprint should have similarity close to 1.0
    // Note: normalized cosine of identical vectors = (1 + 1) / 2 = 1.0
    assert!(
        result.score > 0.99,
        "Self-similarity too low: {}",
        result.score
    );
}

// ============================================================================
// Performance Tests
// ============================================================================

#[tokio::test]
async fn test_performance_pair_under_5ms() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);
    let config = CrossSpaceConfig::default();

    let start = std::time::Instant::now();
    let _ = engine.compute_similarity(&fp1, &fp2, &config).await;
    let elapsed = start.elapsed();

    println!("[TEST] Pair similarity took {:?}", elapsed);

    assert!(
        elapsed.as_millis() < 5,
        "Took {}ms, expected <5ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn test_batch_100_under_50ms() {
    let engine = DefaultCrossSpaceEngine::new();
    let query = create_test_fingerprint(0);
    let candidates: Vec<_> = (1..=100)
        .map(|i| create_test_fingerprint(i as u64))
        .collect();
    let config = CrossSpaceConfig::default();

    let start = std::time::Instant::now();
    let results = engine
        .compute_batch(&query, &candidates, &config)
        .await
        .unwrap();
    let elapsed = start.elapsed();

    println!("[TEST] Batch 100 took {:?}", elapsed);

    assert_eq!(results.len(), 100);
    assert!(
        elapsed.as_millis() < 50,
        "Took {}ms, expected <50ms",
        elapsed.as_millis()
    );
}

// ============================================================================
// Edge Case Tests (Required by TASK-L007)
// ============================================================================

#[tokio::test]
async fn edge_case_empty_fingerprints() {
    println!("[BEFORE] Both fingerprints have 0 active embeddings");

    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_empty_fingerprint();
    let fp2 = create_empty_fingerprint();
    let config = CrossSpaceConfig {
        min_active_spaces: 1,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await;

    println!("[AFTER] result: {:?}", result);

    // Expected: Err(InsufficientSpaces { required: 1, found: 0 })
    assert!(
        matches!(
            result,
            Err(SimilarityError::InsufficientSpaces {
                required: 1,
                found: 0
            })
        ),
        "Expected InsufficientSpaces error with required=1, found=0"
    );
}

#[tokio::test]
async fn edge_case_identical_vectors() {
    let fp = create_test_fingerprint(42);
    let config = CrossSpaceConfig::default();

    println!("[BEFORE] Comparing fingerprint to itself, expecting score ≈ 1.0");

    let engine = DefaultCrossSpaceEngine::new();
    let result = engine.compute_similarity(&fp, &fp, &config).await.unwrap();

    println!(
        "[AFTER] score={:.6}, expected ≈ 1.0",
        result.score
    );

    // Expected: score very close to 1.0
    assert!(
        result.score > 0.99,
        "Self-similarity should be ≈ 1.0, got {}",
        result.score
    );
}

#[tokio::test]
async fn edge_case_rrf_single_item() {
    let id = Uuid::new_v4();
    let ranked_lists = vec![(0, vec![id])];

    println!("[BEFORE] Single item in single list, k=60");

    let engine = DefaultCrossSpaceEngine::new();
    let scores = engine.compute_rrf_from_ranks(&ranked_lists, 60.0);

    let rrf_score = scores.get(&id).unwrap();
    println!("[AFTER] RRF score for id: {:.6}", rrf_score);

    // Expected: 1/(60+0+1) = 1/61 ≈ 0.01639
    let expected = 1.0 / 61.0;
    assert!(
        (rrf_score - expected).abs() < 0.00001,
        "RRF single item: {} vs expected {}",
        rrf_score,
        expected
    );
}

// ============================================================================
// Missing Space Handling Tests
// ============================================================================

#[tokio::test]
async fn test_missing_space_skip() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    let config = CrossSpaceConfig {
        missing_space_handling: MissingSpaceHandling::Skip,
        min_active_spaces: 1,
        include_breakdown: true,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await.unwrap();

    println!(
        "[TEST] Skip handling: score={:.4}, active={}",
        result.score,
        result.active_count()
    );

    assert!(result.score >= 0.0 && result.score <= 1.0);
}

#[tokio::test]
async fn test_missing_space_require_all_with_populated() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    let config = CrossSpaceConfig {
        missing_space_handling: MissingSpaceHandling::RequireAll,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await;

    println!("[TEST] RequireAll with populated: {:?}", result.is_ok());

    // Should succeed because test fingerprints have all spaces populated
    assert!(result.is_ok());
}

// ============================================================================
// Weighting Strategy Tests
// ============================================================================

#[tokio::test]
async fn test_weighting_strategy_static() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    // Custom weights emphasizing E1 Semantic
    let mut weights = [0.05; NUM_EMBEDDERS];
    weights[0] = 0.40; // E1 gets 40%

    let config = CrossSpaceConfig {
        weighting_strategy: WeightingStrategy::Static(weights),
        include_breakdown: true,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await.unwrap();

    println!(
        "[TEST] Static weighting: score={:.4}, E1 weight={}",
        result.score,
        result.space_weights.as_ref().unwrap()[0]
    );

    assert!(result.score >= 0.0 && result.score <= 1.0);
}

#[tokio::test]
async fn test_weighting_strategy_purpose_aligned() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    let config = CrossSpaceConfig {
        weighting_strategy: WeightingStrategy::PurposeAligned,
        use_purpose_weighting: true,
        include_breakdown: true,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await.unwrap();

    println!(
        "[TEST] Purpose-aligned: score={:.4}, purpose_contribution={:?}",
        result.score, result.purpose_contribution
    );

    assert!(result.score >= 0.0 && result.score <= 1.0);
    assert!(result.purpose_contribution.is_some());
}

// ============================================================================
// Explanation Tests
// ============================================================================

#[tokio::test]
async fn test_explain_generates_valid_explanation() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    let config = CrossSpaceConfig {
        include_breakdown: true,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await.unwrap();
    let explanation = engine.explain(&result);

    println!("[TEST] Explanation summary: {}", explanation.summary);
    println!("[TEST] Key factors: {:?}", explanation.key_factors);
    println!("[TEST] Confidence: {}", explanation.confidence_explanation);

    assert!(!explanation.summary.is_empty());
    assert!(explanation.summary.contains("similarity"));
}

// ============================================================================
// Confidence Tests
// ============================================================================

#[tokio::test]
async fn test_confidence_calculation() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(1);
    let fp2 = create_test_fingerprint(2);

    let config = CrossSpaceConfig {
        include_breakdown: true,
        ..Default::default()
    };

    let result = engine.compute_similarity(&fp1, &fp2, &config).await.unwrap();

    println!(
        "[TEST] Confidence: {:.4}, active_count: {}",
        result.confidence,
        result.active_count()
    );

    // Confidence should be reasonable for populated fingerprints
    assert!(
        result.confidence >= 0.0 && result.confidence <= 1.0,
        "Confidence out of range: {}",
        result.confidence
    );
}

// ============================================================================
// Batch Error Handling Tests
// ============================================================================

#[tokio::test]
async fn test_batch_propagates_errors() {
    let engine = DefaultCrossSpaceEngine::new();
    let query = create_test_fingerprint(0);

    // Mix of valid and invalid fingerprints
    let candidates = vec![
        create_test_fingerprint(1),
        create_empty_fingerprint(), // This will fail
        create_test_fingerprint(3),
    ];

    let config = CrossSpaceConfig {
        min_active_spaces: 5, // Will fail for empty fingerprint
        ..Default::default()
    };

    let result = engine.compute_batch(&query, &candidates, &config).await;

    println!("[TEST] Batch with error: {:?}", result.is_err());

    // Should return BatchError wrapping InsufficientSpaces for index 1
    assert!(
        matches!(result, Err(SimilarityError::BatchError { index: 1, .. })),
        "Expected BatchError at index 1, got {:?}",
        result
    );
}

// ============================================================================
// Determinism Tests
// ============================================================================

#[tokio::test]
async fn test_deterministic_output() {
    let engine = DefaultCrossSpaceEngine::new();
    let fp1 = create_test_fingerprint(42);
    let fp2 = create_test_fingerprint(123);
    let config = CrossSpaceConfig::default();

    // Compute twice
    let result1 = engine
        .compute_similarity(&fp1, &fp2, &config)
        .await
        .unwrap();
    let result2 = engine
        .compute_similarity(&fp1, &fp2, &config)
        .await
        .unwrap();

    println!(
        "[TEST] Determinism: run1={:.6}, run2={:.6}",
        result1.score, result2.score
    );

    // Should be exactly equal
    assert_eq!(
        result1.score, result2.score,
        "Determinism violation: {} != {}",
        result1.score, result2.score
    );
    assert_eq!(result1.active_spaces, result2.active_spaces);
}
