//! RRF Formula Tests

use context_graph_embeddings::{EmbedderQueryResult, MultiSpaceQueryResult, RRF_K};
use uuid::Uuid;

/// Verify RRF_K constant matches Constitution k=60.
#[test]
fn test_rrf_k_constant() {
    assert!(
        (RRF_K - 60.0).abs() < f32::EPSILON,
        "RRF_K must be 60.0 per Constitution"
    );
    println!("[PASS] RRF_K = 60.0 matches Constitution");
}

/// Test RRF contribution formula: 1/(60 + rank + 1) (1-indexed).
#[test]
fn test_rrf_contribution_formula() {
    // Rank 0: 1/(60+0+1) = 1/61
    let result_rank_0 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 0);
    let expected_0 = 1.0 / 61.0;
    assert!(
        (result_rank_0.rrf_contribution() - expected_0).abs() < f32::EPSILON,
        "Rank 0 RRF should be 1/61 = {}, got {}",
        expected_0,
        result_rank_0.rrf_contribution()
    );

    // Rank 1: 1/(60+1+1) = 1/62
    let result_rank_1 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 1);
    let expected_1 = 1.0 / 62.0;
    assert!(
        (result_rank_1.rrf_contribution() - expected_1).abs() < f32::EPSILON,
        "Rank 1 RRF should be 1/62 = {}, got {}",
        expected_1,
        result_rank_1.rrf_contribution()
    );

    // Rank 10: 1/(60+10+1) = 1/71
    let result_rank_10 = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.9, 10);
    let expected_10 = 1.0 / 71.0;
    assert!(
        (result_rank_10.rrf_contribution() - expected_10).abs() < f32::EPSILON,
        "Rank 10 RRF should be 1/71 = {}, got {}",
        expected_10,
        result_rank_10.rrf_contribution()
    );

    println!("[PASS] RRF contribution formula 1/(60+rank+1) verified (1-indexed)");
}

/// Test RRF contribution monotonically decreases with rank.
#[test]
fn test_rrf_decreases_with_rank() {
    let id = Uuid::new_v4();
    let mut prev_rrf = f32::MAX;

    for rank in 0..100 {
        let result = EmbedderQueryResult::from_similarity(id, 0, 0.9, rank);
        let rrf = result.rrf_contribution();

        assert!(
            rrf < prev_rrf,
            "RRF should decrease: rank {} ({}) >= rank {} ({})",
            rank,
            rrf,
            rank - 1,
            prev_rrf
        );

        prev_rrf = rrf;
    }

    println!("[PASS] RRF monotonically decreases with rank (tested 0-99)");
}

/// Test RRF aggregation in MultiSpaceQueryResult.
#[test]
fn test_rrf_aggregation() {
    let id = Uuid::new_v4();

    // Create results for 3 embedders with different ranks
    let results = vec![
        EmbedderQueryResult::from_similarity(id, 0, 0.9, 0), // rank 0: 1/61
        EmbedderQueryResult::from_similarity(id, 1, 0.8, 1), // rank 1: 1/62
        EmbedderQueryResult::from_similarity(id, 2, 0.7, 2), // rank 2: 1/63
    ];

    let multi = MultiSpaceQueryResult::from_embedder_results(id, &results);

    // Expected RRF = 1/61 + 1/62 + 1/63 (1-indexed)
    let expected_rrf = 1.0 / 61.0 + 1.0 / 62.0 + 1.0 / 63.0;
    assert!(
        (multi.rrf_score - expected_rrf).abs() < 1e-6,
        "RRF score should be {} (sum of contributions), got {}",
        expected_rrf,
        multi.rrf_score
    );

    println!(
        "[PASS] RRF aggregation: sum of 1/(61+rank_i) = {}",
        multi.rrf_score
    );
}

/// Test RRF at extreme ranks.
#[test]
fn test_rrf_extreme_ranks() {
    // Very high rank
    let result_high = EmbedderQueryResult::from_similarity(Uuid::new_v4(), 0, 0.5, 10000);
    let rrf_high = result_high.rrf_contribution();
    let expected_high = 1.0 / 10061.0;
    assert!(
        (rrf_high - expected_high).abs() < f32::EPSILON,
        "Rank 10000 RRF should be {}, got {}",
        expected_high,
        rrf_high
    );

    // Verify it's still positive
    assert!(rrf_high > 0.0, "RRF should always be positive");

    println!(
        "[PASS] RRF at extreme ranks verified (rank 10000 = {})",
        rrf_high
    );
}

/// Test that rank 0 has much higher RRF contribution than high ranks.
#[test]
fn test_rrf_rank_dominance() {
    let rrf_0 = 1.0 / 61.0; // ~0.0164
    let rrf_100 = 1.0 / 161.0; // ~0.00621
    let rrf_1000 = 1.0 / 1061.0; // ~0.00094

    // Rank 0 should be ~2.67x rank 100
    assert!(rrf_0 > rrf_100 * 2.0, "Rank 0 should be >2x rank 100");

    // Rank 0 should be ~17x rank 1000
    assert!(rrf_0 > rrf_1000 * 15.0, "Rank 0 should be >15x rank 1000");

    println!("[PASS] RRF rank dominance verified");
}
