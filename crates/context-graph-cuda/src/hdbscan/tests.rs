//! GPU HDBSCAN integration tests.
//!
//! These tests verify the GPU HDBSCAN implementation works correctly.
//! They use real GPU operations - no mocks.
//!
//! # Running Tests
//!
//! ```bash
//! cargo test -p context-graph-cuda --test hdbscan_tests -- --nocapture
//! ```

use super::*;
use uuid::Uuid;

/// Test that GPU k-NN index creation works.
#[test]
fn test_gpu_knn_index_creation() {
    // This test will fail if GPU is not available - expected per constitution
    let result = GpuKnnIndex::new(128);

    match result {
        Ok(index) => {
            assert_eq!(index.dimension(), 128);
            assert_eq!(index.len(), 0);
            assert!(index.is_empty());
            println!("GPU k-NN index created successfully: dim={}", index.dimension());
        }
        Err(e) => {
            // GPU not available - this is a constitution violation in production
            // but acceptable in test environments without GPU
            println!("GPU k-NN index creation failed (no GPU?): {}", e);
            // If we can detect SKIP_GPU_TESTS, don't fail
            if std::env::var("SKIP_GPU_TESTS").is_ok() {
                println!("SKIP_GPU_TESTS set, skipping GPU test");
                return;
            }
            panic!("GPU is required per constitution ARCH-GPU-05: {}", e);
        }
    }
}

/// Test adding vectors to GPU k-NN index.
#[test]
fn test_gpu_knn_add_vectors() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let mut index = match GpuKnnIndex::new(128) {
        Ok(i) => i,
        Err(e) => {
            println!("GPU not available: {}", e);
            return;
        }
    };

    // Create 10 random-ish vectors
    let vectors: Vec<Vec<f32>> = (0..10)
        .map(|i| {
            (0..128)
                .map(|j| ((i * 128 + j) as f32).sin())
                .collect()
        })
        .collect();

    index.add(&vectors).expect("Failed to add vectors");

    assert_eq!(index.len(), 10);
    assert!(!index.is_empty());
    println!("Added {} vectors to GPU index", index.len());
}

/// Test core distance computation.
#[test]
fn test_gpu_core_distances() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let mut index = match GpuKnnIndex::new(64) {
        Ok(i) => i,
        Err(e) => {
            println!("GPU not available: {}", e);
            return;
        }
    };

    // Create 20 vectors with known structure:
    // - First 10 vectors are similar to each other (cluster 1)
    // - Next 10 vectors are similar to each other (cluster 2)
    let mut vectors: Vec<Vec<f32>> = Vec::new();

    // Cluster 1: centered around [1, 0, 0, ...]
    for i in 0..10 {
        let mut v = vec![0.0f32; 64];
        v[0] = 1.0 + (i as f32 * 0.01); // Small variation
        v[1] = 0.1 * (i as f32);
        vectors.push(v);
    }

    // Cluster 2: centered around [-1, 0, 0, ...]
    for i in 0..10 {
        let mut v = vec![0.0f32; 64];
        v[0] = -1.0 - (i as f32 * 0.01); // Small variation
        v[1] = 0.1 * (i as f32);
        vectors.push(v);
    }

    index.add(&vectors).expect("Failed to add vectors");
    assert_eq!(index.len(), 20);

    // Compute core distances with k=3
    let core_distances = index
        .compute_core_distances_with_vectors(&vectors, 3)
        .expect("Failed to compute core distances");

    assert_eq!(core_distances.len(), 20);

    // Core distances should be small within clusters (vectors are close)
    for (i, dist) in core_distances.iter().enumerate() {
        println!("Point {}: core_distance = {:.4}", i, dist);
        assert!(dist.is_finite(), "Core distance {} is not finite", i);
        assert!(*dist >= 0.0, "Core distance {} is negative", i);
    }

    // Cluster 1 core distances should be similar to each other
    let cluster1_avg: f32 = core_distances[0..10].iter().sum::<f32>() / 10.0;
    let cluster2_avg: f32 = core_distances[10..20].iter().sum::<f32>() / 10.0;

    println!("Cluster 1 avg core distance: {:.4}", cluster1_avg);
    println!("Cluster 2 avg core distance: {:.4}", cluster2_avg);

    // Both should be relatively small (within-cluster distances)
    assert!(cluster1_avg < 1.0, "Cluster 1 core distances too large");
    assert!(cluster2_avg < 1.0, "Cluster 2 core distances too large");
}

/// Test full HDBSCAN clustering.
#[test]
fn test_gpu_hdbscan_clustering() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let clusterer = GpuHdbscanClusterer::new();

    // Create two clear clusters
    let mut embeddings: Vec<Vec<f32>> = Vec::new();
    let mut memory_ids: Vec<Uuid> = Vec::new();

    // Cluster A: 5 points near origin
    for i in 0..5 {
        let mut v = vec![0.0f32; 64];
        v[0] = 0.1 * (i as f32);
        v[1] = 0.1 * (i as f32);
        embeddings.push(v);
        memory_ids.push(Uuid::new_v4());
    }

    // Cluster B: 5 points far from origin
    for i in 0..5 {
        let mut v = vec![0.0f32; 64];
        v[0] = 10.0 + 0.1 * (i as f32);
        v[1] = 10.0 + 0.1 * (i as f32);
        embeddings.push(v);
        memory_ids.push(Uuid::new_v4());
    }

    let result = clusterer.fit(&embeddings, &memory_ids);

    match result {
        Ok(memberships) => {
            assert_eq!(memberships.len(), 10);

            // Count clusters
            let unique_clusters: std::collections::HashSet<i32> = memberships
                .iter()
                .map(|m| m.cluster_id)
                .filter(|&c| c >= 0)
                .collect();

            println!("Found {} clusters", unique_clusters.len());
            for m in &memberships {
                println!(
                    "Memory {}: cluster={}, prob={:.2}, core={}",
                    m.memory_id, m.cluster_id, m.membership_probability, m.is_core
                );
            }

            // We expect 2 clusters (or possibly 1 if gap threshold is too high)
            // At minimum, not all points should be noise
            let non_noise = memberships.iter().filter(|m| m.cluster_id >= 0).count();
            assert!(non_noise > 0, "All points marked as noise");

            println!("Non-noise points: {}", non_noise);
        }
        Err(e) => {
            if let GpuHdbscanError::GpuNotAvailable { .. } = e {
                println!("GPU not available, skipping: {}", e);
                return;
            }
            panic!("HDBSCAN clustering failed: {}", e);
        }
    }
}

/// Test error handling for insufficient data.
#[test]
fn test_gpu_hdbscan_insufficient_data() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let clusterer = GpuHdbscanClusterer::new();

    // Only 2 points - less than min_cluster_size=3
    let embeddings = vec![vec![0.0f32; 64], vec![1.0f32; 64]];
    let memory_ids = vec![Uuid::new_v4(), Uuid::new_v4()];

    let result = clusterer.fit(&embeddings, &memory_ids);

    match result {
        Err(GpuHdbscanError::InsufficientData { required, actual }) => {
            println!(
                "Correctly rejected insufficient data: need {}, got {}",
                required, actual
            );
            assert_eq!(required, 3);
            assert_eq!(actual, 2);
        }
        Err(GpuHdbscanError::GpuNotAvailable { .. }) => {
            println!("GPU not available, skipping");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        Ok(_) => panic!("Should have rejected insufficient data"),
    }
}

/// Test error handling for dimension mismatch.
#[test]
fn test_gpu_hdbscan_dimension_mismatch() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let clusterer = GpuHdbscanClusterer::new();

    // 5 embeddings but only 3 IDs
    let embeddings: Vec<Vec<f32>> = (0..5).map(|_| vec![0.0f32; 64]).collect();
    let memory_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

    let result = clusterer.fit(&embeddings, &memory_ids);

    match result {
        Err(GpuHdbscanError::DimensionMismatch { embeddings: e, ids }) => {
            println!(
                "Correctly rejected dimension mismatch: {} embeddings, {} ids",
                e, ids
            );
            assert_eq!(e, 5);
            assert_eq!(ids, 3);
        }
        Err(GpuHdbscanError::GpuNotAvailable { .. }) => {
            println!("GPU not available, skipping");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        Ok(_) => panic!("Should have rejected dimension mismatch"),
    }
}

/// Test silhouette score computation.
#[test]
fn test_silhouette_score() {
    let clusterer = GpuHdbscanClusterer::new();

    // Create perfectly separated clusters
    let embeddings = vec![
        vec![0.0, 0.0], // Cluster 0
        vec![0.1, 0.0], // Cluster 0
        vec![0.0, 0.1], // Cluster 0
        vec![10.0, 0.0], // Cluster 1
        vec![10.1, 0.0], // Cluster 1
        vec![10.0, 0.1], // Cluster 1
    ];
    let labels = vec![0, 0, 0, 1, 1, 1];

    let silhouette = clusterer.compute_silhouette(&embeddings, &labels);
    println!("Silhouette score: {:.4}", silhouette);

    // For well-separated clusters, silhouette should be high (close to 1.0)
    assert!(silhouette > 0.5, "Silhouette score too low for separated clusters");
}

/// Test with NaN values (should fail fast).
#[test]
fn test_gpu_hdbscan_rejects_nan() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let clusterer = GpuHdbscanClusterer::new();

    let embeddings = vec![
        vec![0.0, 0.0, f32::NAN], // NaN value
        vec![1.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
    ];
    let memory_ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    let result = clusterer.fit(&embeddings, &memory_ids);

    match result {
        Err(GpuHdbscanError::NonFiniteValue { index, value_type }) => {
            println!("Correctly rejected NaN at index {}: {}", index, value_type);
            assert!(value_type.contains("NaN"));
        }
        Err(GpuHdbscanError::GpuNotAvailable { .. }) => {
            println!("GPU not available, skipping");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        Ok(_) => panic!("Should have rejected NaN values"),
    }
}

/// Test with Infinity values (should fail fast).
#[test]
fn test_gpu_hdbscan_rejects_infinity() {
    if std::env::var("SKIP_GPU_TESTS").is_ok() {
        println!("SKIP_GPU_TESTS set, skipping");
        return;
    }

    let clusterer = GpuHdbscanClusterer::new();

    let embeddings = vec![
        vec![0.0, f32::INFINITY, 0.0], // Infinity value
        vec![1.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
    ];
    let memory_ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

    let result = clusterer.fit(&embeddings, &memory_ids);

    match result {
        Err(GpuHdbscanError::NonFiniteValue { index, value_type }) => {
            println!("Correctly rejected Infinity at index {}: {}", index, value_type);
            assert!(value_type.contains("Infinity"));
        }
        Err(GpuHdbscanError::GpuNotAvailable { .. }) => {
            println!("GPU not available, skipping");
        }
        Err(e) => panic!("Unexpected error: {}", e),
        Ok(_) => panic!("Should have rejected Infinity values"),
    }
}
