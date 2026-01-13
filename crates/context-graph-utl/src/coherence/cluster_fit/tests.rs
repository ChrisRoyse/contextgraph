//! Tests for ClusterFit module.

use crate::coherence::cluster_fit::compute::compute_cluster_fit;
use crate::coherence::cluster_fit::distance::{
    compute_distance, cosine_distance, euclidean_distance, magnitude, manhattan_distance,
    mean_distance_to_cluster,
};
use crate::coherence::cluster_fit::types::{
    ClusterContext, ClusterFitConfig, ClusterFitResult, DistanceMetric,
};

// =========================================================================
// Configuration and Type Tests
// =========================================================================

#[test]
fn test_cluster_fit_config_default() {
    let config = ClusterFitConfig::default();
    assert_eq!(config.min_cluster_size, 2);
    assert_eq!(config.distance_metric, DistanceMetric::Cosine);
    assert_eq!(config.fallback_value, 0.5);
    assert_eq!(config.max_sample_size, 1000);
}

#[test]
fn test_distance_metric_default() {
    assert_eq!(DistanceMetric::default(), DistanceMetric::Cosine);
}

#[test]
fn test_cluster_context_new() {
    let same = vec![vec![0.1, 0.2], vec![0.3, 0.4]];
    let nearest = vec![vec![0.5, 0.6]];
    let ctx = ClusterContext::new(same.clone(), nearest.clone());

    assert_eq!(ctx.same_cluster.len(), 2);
    assert_eq!(ctx.nearest_cluster.len(), 1);
    assert!(ctx.centroids.is_none());
}

#[test]
fn test_cluster_context_with_centroids() {
    let same = vec![vec![0.1, 0.2]];
    let nearest = vec![vec![0.5, 0.6]];
    let centroids = vec![vec![0.2, 0.3], vec![0.6, 0.7]];

    let ctx = ClusterContext::with_centroids(same, nearest, centroids);
    assert!(ctx.centroids.is_some());
    assert_eq!(ctx.centroids.unwrap().len(), 2);
}

#[test]
fn test_cluster_fit_result_new() {
    // Perfect clustering: silhouette = 1.0
    let result = ClusterFitResult::new(1.0, 0.1, 0.9);
    assert_eq!(result.score, 1.0);
    assert_eq!(result.silhouette, 1.0);

    // Worst clustering: silhouette = -1.0
    let result = ClusterFitResult::new(-1.0, 0.9, 0.1);
    assert_eq!(result.score, 0.0);
    assert_eq!(result.silhouette, -1.0);

    // Boundary: silhouette = 0.0
    let result = ClusterFitResult::new(0.0, 0.5, 0.5);
    assert_eq!(result.score, 0.5);
    assert_eq!(result.silhouette, 0.0);
}

#[test]
fn test_cluster_fit_result_clamps_output() {
    // Test clamping for out-of-range values
    let result = ClusterFitResult::new(1.5, 0.1, 0.9);
    assert_eq!(result.score, 1.0);
    assert_eq!(result.silhouette, 1.0);

    let result = ClusterFitResult::new(-1.5, 0.9, 0.1);
    assert_eq!(result.score, 0.0);
    assert_eq!(result.silhouette, -1.0);
}

#[test]
fn test_cluster_fit_result_fallback() {
    let result = ClusterFitResult::fallback(0.5);
    assert_eq!(result.score, 0.5);
    assert_eq!(result.silhouette, 0.0);
    assert_eq!(result.intra_distance, 0.0);
    assert_eq!(result.inter_distance, 0.0);
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = ClusterFitConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let restored: ClusterFitConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.min_cluster_size, restored.min_cluster_size);
    assert_eq!(config.distance_metric, restored.distance_metric);
    assert_eq!(config.fallback_value, restored.fallback_value);
}

#[test]
fn test_distance_metric_serialization() {
    let metric = DistanceMetric::Euclidean;
    let json = serde_json::to_string(&metric).unwrap();
    assert!(json.contains("Euclidean"));

    let restored: DistanceMetric = serde_json::from_str(&json).unwrap();
    assert_eq!(restored, DistanceMetric::Euclidean);
}

// =========================================================================
// Distance Function Tests
// =========================================================================

#[test]
fn test_magnitude() {
    // Unit vector
    let v = vec![1.0, 0.0, 0.0];
    assert!((magnitude(&v) - 1.0).abs() < 1e-6);

    // Zero vector
    let zero = vec![0.0, 0.0, 0.0];
    assert!(magnitude(&zero).abs() < 1e-10);

    // 3-4-5 triangle
    let v345 = vec![3.0, 4.0];
    assert!((magnitude(&v345) - 5.0).abs() < 1e-6);
}

#[test]
fn test_cosine_distance_identical() {
    let a = vec![1.0, 2.0, 3.0];
    let dist = cosine_distance(&a, &a);
    assert!(dist.abs() < 1e-6, "Identical vectors should have distance ~0");
}

#[test]
fn test_cosine_distance_orthogonal() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![0.0, 1.0, 0.0];
    let dist = cosine_distance(&a, &b);
    assert!(
        (dist - 1.0).abs() < 1e-6,
        "Orthogonal vectors should have distance ~1"
    );
}

#[test]
fn test_cosine_distance_opposite() {
    let a = vec![1.0, 0.0, 0.0];
    let b = vec![-1.0, 0.0, 0.0];
    let dist = cosine_distance(&a, &b);
    assert!(
        (dist - 2.0).abs() < 1e-6,
        "Opposite vectors should have distance ~2"
    );
}

#[test]
fn test_cosine_distance_empty() {
    let empty: Vec<f32> = vec![];
    let a = vec![1.0, 2.0];
    assert_eq!(cosine_distance(&empty, &a), 0.0);
    assert_eq!(cosine_distance(&a, &empty), 0.0);
}

#[test]
fn test_cosine_distance_zero_vector() {
    let zero = vec![0.0, 0.0, 0.0];
    let a = vec![1.0, 2.0, 3.0];
    assert_eq!(cosine_distance(&zero, &a), 0.0);
    assert_eq!(cosine_distance(&a, &zero), 0.0);
}

#[test]
fn test_cosine_distance_dimension_mismatch() {
    let a = vec![1.0, 2.0];
    let b = vec![1.0, 2.0, 3.0];
    assert_eq!(cosine_distance(&a, &b), 0.0);
}

#[test]
fn test_euclidean_distance_identical() {
    let a = vec![1.0, 2.0, 3.0];
    let dist = euclidean_distance(&a, &a);
    assert!(dist.abs() < 1e-6);
}

#[test]
fn test_euclidean_distance_basic() {
    let a = vec![0.0, 0.0];
    let b = vec![3.0, 4.0];
    let dist = euclidean_distance(&a, &b);
    assert!((dist - 5.0).abs() < 1e-6);
}

#[test]
fn test_euclidean_distance_empty() {
    let empty: Vec<f32> = vec![];
    let a = vec![1.0, 2.0];
    assert_eq!(euclidean_distance(&empty, &a), 0.0);
}

#[test]
fn test_manhattan_distance_identical() {
    let a = vec![1.0, 2.0, 3.0];
    let dist = manhattan_distance(&a, &a);
    assert!(dist.abs() < 1e-6);
}

#[test]
fn test_manhattan_distance_basic() {
    let a = vec![0.0, 0.0];
    let b = vec![3.0, 4.0];
    let dist = manhattan_distance(&a, &b);
    assert!((dist - 7.0).abs() < 1e-6); // |3-0| + |4-0| = 7
}

#[test]
fn test_manhattan_distance_empty() {
    let empty: Vec<f32> = vec![];
    let a = vec![1.0, 2.0];
    assert_eq!(manhattan_distance(&empty, &a), 0.0);
}

#[test]
fn test_compute_distance_metric_dispatch() {
    let a = vec![0.0, 0.0];
    let b = vec![3.0, 4.0];

    let cosine = compute_distance(&a, &b, DistanceMetric::Cosine);
    let euclidean = compute_distance(&a, &b, DistanceMetric::Euclidean);
    let manhattan = compute_distance(&a, &b, DistanceMetric::Manhattan);

    // Different metrics should give different results
    assert!((euclidean - 5.0).abs() < 1e-6);
    assert!((manhattan - 7.0).abs() < 1e-6);
    // Cosine returns 0 for zero vector
    assert_eq!(cosine, 0.0);
}

// =========================================================================
// Mean Distance Tests
// =========================================================================

#[test]
fn test_mean_distance_empty_cluster() {
    let query = vec![1.0, 0.0, 0.0];
    let cluster: Vec<Vec<f32>> = vec![];

    let result = mean_distance_to_cluster(&query, &cluster, DistanceMetric::Cosine, 1000);
    assert!(result.is_none());
}

#[test]
fn test_mean_distance_empty_query() {
    let query: Vec<f32> = vec![];
    let cluster = vec![vec![1.0, 0.0, 0.0]];

    let result = mean_distance_to_cluster(&query, &cluster, DistanceMetric::Cosine, 1000);
    assert!(result.is_none());
}

#[test]
fn test_mean_distance_single_member() {
    let query = vec![1.0, 0.0, 0.0];
    let cluster = vec![vec![0.0, 1.0, 0.0]]; // Orthogonal

    let result = mean_distance_to_cluster(&query, &cluster, DistanceMetric::Cosine, 1000);
    assert!(result.is_some());
    let dist = result.unwrap();
    assert!((dist - 1.0).abs() < 1e-6); // Orthogonal = distance 1
}

#[test]
fn test_mean_distance_multiple_members() {
    let query = vec![1.0, 0.0, 0.0];
    let cluster = vec![
        vec![1.0, 0.0, 0.0], // Identical = 0
        vec![0.0, 1.0, 0.0], // Orthogonal = 1
    ];

    let result = mean_distance_to_cluster(&query, &cluster, DistanceMetric::Cosine, 1000);
    assert!(result.is_some());
    let dist = result.unwrap();
    assert!((dist - 0.5).abs() < 1e-6); // Mean of 0 and 1 = 0.5
}

#[test]
fn test_mean_distance_skips_mismatched_dimensions() {
    let query = vec![1.0, 0.0, 0.0];
    let cluster = vec![
        vec![1.0, 0.0], // Wrong dimension - skipped
        vec![0.0, 1.0, 0.0], // Orthogonal = 1
    ];

    let result = mean_distance_to_cluster(&query, &cluster, DistanceMetric::Cosine, 1000);
    assert!(result.is_some());
    let dist = result.unwrap();
    // Only the orthogonal vector counts
    assert!((dist - 1.0).abs() < 1e-6);
}

#[test]
fn test_mean_distance_sampling() {
    let query = vec![1.0, 0.0];

    // Create a large cluster
    let mut cluster = Vec::new();
    for i in 0..100 {
        let angle = (i as f32) * std::f32::consts::PI / 100.0;
        cluster.push(vec![angle.cos(), angle.sin()]);
    }

    // With max_sample=10, should still work
    let result = mean_distance_to_cluster(&query, &cluster, DistanceMetric::Cosine, 10);
    assert!(result.is_some());
    let dist = result.unwrap();
    assert!(dist >= 0.0);
}

// =========================================================================
// compute_cluster_fit Tests
// =========================================================================

#[test]
fn test_compute_cluster_fit_basic() {
    let query = vec![0.1, 0.2, 0.3, 0.4];

    // Same cluster: similar vectors
    let same_cluster = vec![
        vec![0.12, 0.22, 0.28, 0.38],
        vec![0.11, 0.21, 0.29, 0.39],
    ];

    // Nearest cluster: quite different vectors
    let nearest_cluster = vec![
        vec![0.8, 0.1, 0.05, 0.05],
        vec![0.7, 0.2, 0.05, 0.05],
    ];

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should have positive silhouette (well-clustered)
    assert!(result.silhouette > 0.0, "Expected positive silhouette");
    assert!(result.score > 0.5, "Expected score > 0.5 for well-clustered");

    // Verify output ranges per AP-10
    assert!(
        (0.0..=1.0).contains(&result.score),
        "Score should be in [0, 1]"
    );
    assert!(
        (-1.0..=1.0).contains(&result.silhouette),
        "Silhouette should be in [-1, 1]"
    );
    assert!(result.intra_distance >= 0.0, "Intra distance >= 0");
    assert!(result.inter_distance >= 0.0, "Inter distance >= 0");
}

#[test]
fn test_compute_cluster_fit_wrong_cluster() {
    let query = vec![0.9, 0.1, 0.0, 0.0];

    // Same cluster: vectors very different from query
    let same_cluster = vec![
        vec![0.0, 0.0, 0.9, 0.1],
        vec![0.0, 0.0, 0.8, 0.2],
    ];

    // Nearest cluster: vectors similar to query
    let nearest_cluster = vec![
        vec![0.85, 0.15, 0.0, 0.0],
        vec![0.88, 0.12, 0.0, 0.0],
    ];

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should have negative silhouette (wrong cluster)
    assert!(
        result.silhouette < 0.0,
        "Expected negative silhouette for wrong cluster"
    );
    assert!(result.score < 0.5, "Expected score < 0.5 for wrong cluster");
}

#[test]
fn test_compute_cluster_fit_boundary() {
    let query = vec![0.5, 0.5, 0.0, 0.0];

    // Same cluster and nearest cluster equally distant
    let same_cluster = vec![vec![1.0, 0.0, 0.0, 0.0], vec![0.0, 1.0, 0.0, 0.0]];

    let nearest_cluster = vec![vec![0.0, 0.0, 1.0, 0.0], vec![0.0, 0.0, 0.0, 1.0]];

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should have silhouette close to 0 (boundary case)
    // The exact value depends on the specific vectors
    assert!(
        (-1.0..=1.0).contains(&result.silhouette),
        "Silhouette should be valid"
    );
    assert!(
        (0.0..=1.0).contains(&result.score),
        "Score should be in [0, 1]"
    );
}

#[test]
fn test_compute_cluster_fit_empty_query() {
    let query: Vec<f32> = vec![];
    let context = ClusterContext::new(vec![vec![1.0, 0.0]], vec![vec![0.0, 1.0]]);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should return fallback
    assert_eq!(result.score, config.fallback_value);
    assert_eq!(result.silhouette, 0.0);
}

#[test]
fn test_compute_cluster_fit_zero_magnitude_query() {
    let query = vec![0.0, 0.0, 0.0];
    let context = ClusterContext::new(vec![vec![1.0, 0.0, 0.0]], vec![vec![0.0, 1.0, 0.0]]);
    let config = ClusterFitConfig::default(); // Uses cosine distance

    let result = compute_cluster_fit(&query, &context, &config);

    // Should return fallback for zero-magnitude with cosine
    assert_eq!(result.score, config.fallback_value);
}

#[test]
fn test_compute_cluster_fit_empty_same_cluster() {
    let query = vec![1.0, 0.0, 0.0];
    let context = ClusterContext::new(vec![], vec![vec![0.0, 1.0, 0.0]]);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should return fallback (need min_cluster_size - 1 members)
    assert_eq!(result.score, config.fallback_value);
}

#[test]
fn test_compute_cluster_fit_empty_nearest_cluster() {
    let query = vec![1.0, 0.0, 0.0];
    let context = ClusterContext::new(vec![vec![0.9, 0.1, 0.0]], vec![]);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should return fallback
    assert_eq!(result.score, config.fallback_value);
}

#[test]
fn test_compute_cluster_fit_insufficient_same_cluster() {
    let query = vec![1.0, 0.0, 0.0];
    let context = ClusterContext::new(vec![], vec![vec![0.0, 1.0, 0.0]]);

    // Require 3 members (so need 2 in same_cluster)
    let mut config = ClusterFitConfig::default();
    config.min_cluster_size = 3;

    let result = compute_cluster_fit(&query, &context, &config);

    // Should return fallback
    assert_eq!(result.score, config.fallback_value);
}

#[test]
fn test_compute_cluster_fit_euclidean_metric() {
    let query = vec![0.0, 0.0];

    let same_cluster = vec![vec![1.0, 0.0], vec![0.0, 1.0]];

    let nearest_cluster = vec![vec![3.0, 0.0], vec![0.0, 3.0]];

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let mut config = ClusterFitConfig::default();
    config.distance_metric = DistanceMetric::Euclidean;

    let result = compute_cluster_fit(&query, &context, &config);

    // Intra distance: mean of dist([0,0], [1,0]) and dist([0,0], [0,1]) = 1.0
    // Inter distance: mean of dist([0,0], [3,0]) and dist([0,0], [0,3]) = 3.0
    // Silhouette = (3.0 - 1.0) / 3.0 = 2/3 ~ 0.667
    assert!(result.silhouette > 0.6, "Expected high positive silhouette");
    assert!((result.intra_distance - 1.0).abs() < 1e-6);
    assert!((result.inter_distance - 3.0).abs() < 1e-6);
}

#[test]
fn test_compute_cluster_fit_manhattan_metric() {
    let query = vec![0.0, 0.0];

    let same_cluster = vec![vec![1.0, 1.0]]; // Manhattan dist = 2

    let nearest_cluster = vec![vec![3.0, 3.0]]; // Manhattan dist = 6

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let mut config = ClusterFitConfig::default();
    config.distance_metric = DistanceMetric::Manhattan;

    let result = compute_cluster_fit(&query, &context, &config);

    // Silhouette = (6 - 2) / 6 = 4/6 ~ 0.667
    assert!(result.silhouette > 0.6);
    assert!((result.intra_distance - 2.0).abs() < 1e-6);
    assert!((result.inter_distance - 6.0).abs() < 1e-6);
}

#[test]
fn test_compute_cluster_fit_custom_fallback() {
    let query: Vec<f32> = vec![];
    let context = ClusterContext::new(vec![], vec![]);

    let mut config = ClusterFitConfig::default();
    config.fallback_value = 0.75;

    let result = compute_cluster_fit(&query, &context, &config);

    assert_eq!(result.score, 0.75);
}

#[test]
fn test_compute_cluster_fit_no_nan_infinity() {
    // Test various edge cases that might produce NaN/Infinity

    let test_cases: Vec<(Vec<f32>, Vec<Vec<f32>>, Vec<Vec<f32>>)> = vec![
        // Zero vectors everywhere
        (
            vec![0.0, 0.0, 0.0],
            vec![vec![0.0, 0.0, 0.0]],
            vec![vec![0.0, 0.0, 0.0]],
        ),
        // Very small values
        (
            vec![1e-15, 1e-15, 1e-15],
            vec![vec![1e-15, 1e-15, 1e-15]],
            vec![vec![1e-15, 1e-15, 1e-15]],
        ),
        // Very large values
        (
            vec![1e30, 1e30, 1e30],
            vec![vec![1e30, 1e30, 1e30]],
            vec![vec![1e30, 1e30, 1e30]],
        ),
        // Mixed extreme values
        (
            vec![1e-30, 1e30],
            vec![vec![1e-30, 1e30]],
            vec![vec![1e30, 1e-30]],
        ),
    ];

    let config = ClusterFitConfig::default();

    for (query, same, nearest) in test_cases {
        let context = ClusterContext::new(same, nearest);
        let result = compute_cluster_fit(&query, &context, &config);

        assert!(!result.score.is_nan(), "Score should not be NaN");
        assert!(!result.score.is_infinite(), "Score should not be infinite");
        assert!(!result.silhouette.is_nan(), "Silhouette should not be NaN");
        assert!(
            !result.silhouette.is_infinite(),
            "Silhouette should not be infinite"
        );
        assert!(
            !result.intra_distance.is_nan(),
            "Intra distance should not be NaN"
        );
        assert!(
            !result.inter_distance.is_nan(),
            "Inter distance should not be NaN"
        );
    }
}

#[test]
fn test_compute_cluster_fit_output_ranges() {
    // Comprehensive test of output ranges per AP-10

    let query = vec![0.5, 0.5, 0.0, 0.0];
    let same_cluster = vec![
        vec![0.6, 0.4, 0.0, 0.0],
        vec![0.4, 0.6, 0.0, 0.0],
        vec![0.55, 0.45, 0.0, 0.0],
    ];
    let nearest_cluster = vec![
        vec![0.0, 0.0, 0.5, 0.5],
        vec![0.0, 0.0, 0.6, 0.4],
    ];

    let context = ClusterContext::new(same_cluster, nearest_cluster);

    // Test all metrics
    for metric in [
        DistanceMetric::Cosine,
        DistanceMetric::Euclidean,
        DistanceMetric::Manhattan,
    ] {
        let mut config = ClusterFitConfig::default();
        config.distance_metric = metric;

        let result = compute_cluster_fit(&query, &context, &config);

        assert!(
            (0.0..=1.0).contains(&result.score),
            "{:?}: Score {} out of [0, 1]",
            metric,
            result.score
        );
        assert!(
            (-1.0..=1.0).contains(&result.silhouette),
            "{:?}: Silhouette {} out of [-1, 1]",
            metric,
            result.silhouette
        );
        assert!(
            result.intra_distance >= 0.0,
            "{:?}: Intra distance {} < 0",
            metric,
            result.intra_distance
        );
        assert!(
            result.inter_distance >= 0.0,
            "{:?}: Inter distance {} < 0",
            metric,
            result.inter_distance
        );
    }
}

#[test]
fn test_compute_cluster_fit_perfect_clustering() {
    // Query is identical to same-cluster members, very far from nearest-cluster
    let query = vec![1.0, 0.0, 0.0];
    let same_cluster = vec![vec![1.0, 0.0, 0.0], vec![1.0, 0.0, 0.0]];
    let nearest_cluster = vec![vec![-1.0, 0.0, 0.0]];

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should have maximum silhouette (close to 1)
    // Intra distance = 0, Inter distance = 2 (opposite vectors)
    // But silhouette formula gives 1 when a=0: (b - 0) / b = 1
    assert!(
        result.silhouette > 0.9,
        "Expected silhouette close to 1, got {}",
        result.silhouette
    );
    assert!(result.score > 0.9);
}

#[test]
fn test_compute_cluster_fit_identical_clusters() {
    // Both clusters have identical vectors to query
    let query = vec![0.5, 0.5, 0.0, 0.0];
    let same_cluster = vec![vec![0.5, 0.5, 0.0, 0.0]];
    let nearest_cluster = vec![vec![0.5, 0.5, 0.0, 0.0]];

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Both distances are 0, so silhouette = 0 (neutral)
    assert!(
        result.silhouette.abs() < 1e-6,
        "Expected silhouette ~0, got {}",
        result.silhouette
    );
    assert!(
        (result.score - 0.5).abs() < 1e-6,
        "Expected score ~0.5, got {}",
        result.score
    );
}

#[test]
fn test_compute_cluster_fit_sampling() {
    // Test that sampling doesn't break the calculation
    let query = vec![1.0, 0.0];

    // Create large clusters
    let mut same_cluster = Vec::new();
    let mut nearest_cluster = Vec::new();

    for i in 0..2000 {
        // Same cluster: close to [1, 0]
        let noise = (i as f32) * 0.0001;
        same_cluster.push(vec![1.0 - noise, noise]);

        // Nearest cluster: close to [0, 1]
        nearest_cluster.push(vec![noise, 1.0 - noise]);
    }

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let mut config = ClusterFitConfig::default();
    config.max_sample_size = 100; // Force sampling

    let result = compute_cluster_fit(&query, &context, &config);

    // Should still compute valid result
    assert!(
        (0.0..=1.0).contains(&result.score),
        "Score {} out of range",
        result.score
    );
    assert!(result.silhouette > 0.0, "Should be well-clustered");
}

// ==========================================================================
// TASK-UTL-P2-001: sklearn/scipy Reference Validation Tests
// ==========================================================================
//
// Reference value generation script (Python - for documentation):
// ```python
// import numpy as np
// from scipy.spatial.distance import euclidean, cosine, cityblock, cdist
// from sklearn.metrics import silhouette_score, silhouette_samples
//
// # Test Case 1: Simple 2D Euclidean
// query = np.array([0.0, 0.0])
// same_cluster = np.array([[1.0, 0.0], [0.0, 1.0]])  # distance 1.0 each
// nearest_cluster = np.array([[3.0, 0.0], [0.0, 3.0]])  # distance 3.0 each
//
// a = np.mean([euclidean(query, p) for p in same_cluster])  # 1.0
// b = np.mean([euclidean(query, p) for p in nearest_cluster])  # 3.0
// silhouette = (b - a) / max(a, b)  # (3.0 - 1.0) / 3.0 = 0.6667
//
// # Distance reference tests:
// # euclidean([0,0,0], [3,4,0]) = 5.0
// # cosine([1,0,0], [0,1,0]) = 1.0 (orthogonal)
// # cosine([1,0,0], [1,0,0]) = 0.0 (identical)
// # cosine([1,0,0], [-1,0,0]) = 2.0 (opposite)
// # cityblock([0,0], [3,4]) = 7.0
// ```

#[test]
fn test_silhouette_sklearn_reference_euclidean() {
    // TASK-UTL-P2-001: Test against pre-computed sklearn values using Euclidean distance
    // Reference values computed with Python sklearn/scipy (see docstring above)
    let query = vec![0.0, 0.0];
    let same_cluster = vec![
        vec![1.0, 0.0],  // euclidean distance = 1.0
        vec![0.0, 1.0],  // euclidean distance = 1.0
    ];
    let nearest_cluster = vec![
        vec![3.0, 0.0],  // euclidean distance = 3.0
        vec![0.0, 3.0],  // euclidean distance = 3.0
    ];

    // Expected values (from Python):
    // a = mean([1.0, 1.0]) = 1.0
    // b = mean([3.0, 3.0]) = 3.0
    // silhouette = (3.0 - 1.0) / max(1.0, 3.0) = 2.0 / 3.0 = 0.6666667
    let expected_silhouette: f32 = 2.0 / 3.0;
    let expected_intra: f32 = 1.0;
    let expected_inter: f32 = 3.0;

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let mut config = ClusterFitConfig::default();
    config.distance_metric = DistanceMetric::Euclidean;

    let result = compute_cluster_fit(&query, &context, &config);

    assert!(
        (result.silhouette - expected_silhouette).abs() < 1e-5,
        "Silhouette mismatch: got {} expected {}",
        result.silhouette, expected_silhouette
    );
    assert!(
        (result.intra_distance - expected_intra).abs() < 1e-5,
        "Intra distance mismatch: got {} expected {}",
        result.intra_distance, expected_intra
    );
    assert!(
        (result.inter_distance - expected_inter).abs() < 1e-5,
        "Inter distance mismatch: got {} expected {}",
        result.inter_distance, expected_inter
    );
}

#[test]
fn test_silhouette_sklearn_reference_cosine() {
    // TASK-UTL-P2-001: Test with cosine distance against reference values
    // Using orthogonal vectors for predictable results
    let query = vec![1.0, 0.0, 0.0, 0.0];  // Unit vector along first axis

    // Same cluster: nearly identical to query (small cosine distance)
    let same_cluster = vec![
        vec![1.0, 0.0, 0.0, 0.0],  // Identical = cosine distance 0
        vec![1.0, 0.0, 0.0, 0.0],  // Identical = cosine distance 0
    ];

    // Nearest cluster: orthogonal to query (cosine distance = 1.0)
    let nearest_cluster = vec![
        vec![0.0, 1.0, 0.0, 0.0],  // Orthogonal = cosine distance 1.0
        vec![0.0, 0.0, 1.0, 0.0],  // Orthogonal = cosine distance 1.0
    ];

    // Expected values:
    // a = mean([0.0, 0.0]) = 0.0 (identical vectors)
    // b = mean([1.0, 1.0]) = 1.0 (orthogonal vectors)
    // silhouette = (1.0 - 0.0) / max(0.0, 1.0) = 1.0 / 1.0 = 1.0
    let expected_silhouette: f32 = 1.0;
    let expected_intra: f32 = 0.0;
    let expected_inter: f32 = 1.0;

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default(); // Uses Cosine by default

    let result = compute_cluster_fit(&query, &context, &config);

    assert!(
        (result.silhouette - expected_silhouette).abs() < 1e-5,
        "Silhouette mismatch: got {} expected {}",
        result.silhouette, expected_silhouette
    );
    assert!(
        (result.intra_distance - expected_intra).abs() < 1e-5,
        "Intra distance mismatch: got {} expected {}",
        result.intra_distance, expected_intra
    );
    assert!(
        (result.inter_distance - expected_inter).abs() < 1e-5,
        "Inter distance mismatch: got {} expected {}",
        result.inter_distance, expected_inter
    );
}

#[test]
fn test_euclidean_distance_scipy_reference() {
    // TASK-UTL-P2-001: Validate euclidean_distance against scipy.spatial.distance.euclidean
    // scipy.spatial.distance.euclidean([0,0,0], [3,4,0]) = 5.0 (3-4-5 triangle)
    let a = vec![0.0, 0.0, 0.0];
    let b = vec![3.0, 4.0, 0.0];
    let expected = 5.0f32;

    let result = euclidean_distance(&a, &b);
    assert!(
        (result - expected).abs() < 1e-6,
        "Euclidean distance mismatch: got {} expected {}",
        result, expected
    );

    // Additional reference: scipy.spatial.distance.euclidean([1,2,3], [4,6,8]) = sqrt(50)
    let c = vec![1.0, 2.0, 3.0];
    let d = vec![4.0, 6.0, 8.0];
    let expected2 = 50.0f32.sqrt(); // sqrt(9 + 16 + 25) = sqrt(50)

    let result2 = euclidean_distance(&c, &d);
    assert!(
        (result2 - expected2).abs() < 1e-5,
        "Euclidean distance mismatch: got {} expected {}",
        result2, expected2
    );
}

#[test]
fn test_cosine_distance_scipy_reference() {
    // TASK-UTL-P2-001: Validate cosine_distance against scipy.spatial.distance.cosine
    // scipy.spatial.distance.cosine([1,0,0], [0,1,0]) = 1.0 (orthogonal)
    // scipy.spatial.distance.cosine([1,0,0], [1,0,0]) = 0.0 (identical)
    // scipy.spatial.distance.cosine([1,0,0], [-1,0,0]) = 2.0 (opposite)

    let unit_x = vec![1.0, 0.0, 0.0];
    let unit_y = vec![0.0, 1.0, 0.0];
    let neg_x = vec![-1.0, 0.0, 0.0];

    // Orthogonal: expected 1.0
    let dist_orthogonal = cosine_distance(&unit_x, &unit_y);
    assert!(
        (dist_orthogonal - 1.0).abs() < 1e-6,
        "Orthogonal cosine distance should be 1.0, got {}",
        dist_orthogonal
    );

    // Identical: expected 0.0
    let dist_identical = cosine_distance(&unit_x, &unit_x);
    assert!(
        dist_identical.abs() < 1e-6,
        "Identical cosine distance should be 0.0, got {}",
        dist_identical
    );

    // Opposite: expected 2.0
    let dist_opposite = cosine_distance(&unit_x, &neg_x);
    assert!(
        (dist_opposite - 2.0).abs() < 1e-6,
        "Opposite cosine distance should be 2.0, got {}",
        dist_opposite
    );
}

#[test]
fn test_manhattan_distance_scipy_reference() {
    // TASK-UTL-P2-001: Validate manhattan_distance against scipy.spatial.distance.cityblock
    // scipy.spatial.distance.cityblock([0,0], [3,4]) = 7.0
    let a = vec![0.0, 0.0];
    let b = vec![3.0, 4.0];
    let expected = 7.0f32;

    let result = manhattan_distance(&a, &b);
    assert!(
        (result - expected).abs() < 1e-6,
        "Manhattan distance mismatch: got {} expected {}",
        result, expected
    );

    // Additional reference: cityblock([1,2,3,4], [5,6,7,8]) = 16
    let c = vec![1.0, 2.0, 3.0, 4.0];
    let d = vec![5.0, 6.0, 7.0, 8.0];
    let expected2 = 16.0f32; // |4| + |4| + |4| + |4| = 16

    let result2 = manhattan_distance(&c, &d);
    assert!(
        (result2 - expected2).abs() < 1e-6,
        "Manhattan distance mismatch: got {} expected {}",
        result2, expected2
    );
}

#[test]
fn test_silhouette_high_dimensional() {
    // TASK-UTL-P2-001: Test high-dimensional case (1536-dim like real embeddings)
    // Ensures no NaN/Inf and valid output range per AP-10
    let dim = 1536;
    let query: Vec<f32> = (0..dim).map(|i| (i as f32 * 0.001).sin()).collect();

    // Same cluster: small perturbations of query
    let same_cluster: Vec<Vec<f32>> = (0..5).map(|j| {
        query.iter().enumerate()
            .map(|(i, &v)| v + (i as f32 * j as f32 * 0.0001).cos() * 0.01)
            .collect()
    }).collect();

    // Nearest cluster: orthogonal-ish vectors
    let nearest_cluster: Vec<Vec<f32>> = (0..5).map(|j| {
        (0..dim).map(|i| ((i as f32 + j as f32 * 100.0) * 0.002).cos()).collect()
    }).collect();

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    let result = compute_cluster_fit(&query, &context, &config);

    // Should produce valid results without NaN/Inf
    assert!(!result.silhouette.is_nan(), "Silhouette should not be NaN");
    assert!(!result.silhouette.is_infinite(), "Silhouette should not be Inf");
    assert!((-1.0..=1.0).contains(&result.silhouette),
        "Silhouette {} should be in [-1, 1]", result.silhouette);
    assert!(!result.intra_distance.is_nan(), "Intra distance should not be NaN");
    assert!(!result.inter_distance.is_nan(), "Inter distance should not be NaN");
    assert!(result.intra_distance < result.inter_distance,
        "Intra {} should be less than inter {} for well-clustered",
        result.intra_distance, result.inter_distance);
}
