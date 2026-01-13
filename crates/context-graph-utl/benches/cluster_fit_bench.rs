//! ClusterFit benchmark suite for performance validation
//!
//! TASK-UTL-P2-002: Performance target: < 2ms p95 for 1000 embeddings at 1536 dimensions
//!
//! Constitution reference (UTL-002):
//! delta_C = 0.4 * Connectivity + 0.4 * ClusterFit + 0.2 * Consistency
//!
//! Run with:
//! - `cargo bench -p context-graph-utl --bench cluster_fit_bench`
//! - `cargo bench -p context-graph-utl --bench cluster_fit_bench cluster_fit_1000 -- --noplot`

use context_graph_utl::coherence::{
    compute_cluster_fit, ClusterContext, ClusterFitConfig, DistanceMetric,
};
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

// =============================================================================
// Helper Functions: Deterministic Data Generation
// =============================================================================

/// Generate a deterministic embedding vector.
/// Same seed always produces same vector for reproducible benchmarks.
fn generate_embedding(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| ((i as f64 + seed as f64) * 0.1).sin() as f32)
        .collect()
}

/// Generate a cluster of embeddings.
fn generate_cluster(count: usize, dim: usize, base_seed: u64) -> Vec<Vec<f32>> {
    (0..count)
        .map(|i| generate_embedding(dim, base_seed + i as u64 * 7))
        .collect()
}

// =============================================================================
// Main Performance Target Benchmark
// =============================================================================

/// Primary benchmark: 1000 embeddings at 1536 dimensions
/// Target: < 2ms p95 latency
fn bench_cluster_fit_1000_embeddings(c: &mut Criterion) {
    let dim = 1536;
    let cluster_size = 1000;

    let query = generate_embedding(dim, 42);
    let same_cluster = generate_cluster(cluster_size, dim, 100);
    let nearest_cluster = generate_cluster(cluster_size / 2, dim, 10000);

    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default(); // Cosine distance

    c.bench_function("cluster_fit_1000_embeddings_1536d", |b| {
        b.iter(|| {
            compute_cluster_fit(
                black_box(&query),
                black_box(&context),
                black_box(&config),
            )
        })
    });
}

// =============================================================================
// Cluster Size Scaling Benchmarks
// =============================================================================

fn bench_cluster_fit_cluster_scaling(c: &mut Criterion) {
    let dim = 1536;
    let query = generate_embedding(dim, 42);

    let mut group = c.benchmark_group("cluster_fit_cluster_scaling");

    for cluster_size in [10, 50, 100, 500, 1000].iter() {
        let same_cluster = generate_cluster(*cluster_size, dim, 100);
        let nearest_cluster = generate_cluster(*cluster_size / 2, dim, 10000);
        let context = ClusterContext::new(same_cluster, nearest_cluster);
        let config = ClusterFitConfig::default();

        group.throughput(Throughput::Elements(*cluster_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(cluster_size),
            &context,
            |b, ctx| {
                b.iter(|| {
                    compute_cluster_fit(
                        black_box(&query),
                        black_box(ctx),
                        black_box(&config),
                    )
                })
            },
        );
    }
    group.finish();
}

// =============================================================================
// Embedding Dimension Scaling Benchmarks
// =============================================================================

fn bench_cluster_fit_dimension_scaling(c: &mut Criterion) {
    let cluster_size = 100;

    let mut group = c.benchmark_group("cluster_fit_dimension_scaling");

    for dim in [384, 768, 1536, 3072].iter() {
        let query = generate_embedding(*dim, 42);
        let same_cluster = generate_cluster(cluster_size, *dim, 100);
        let nearest_cluster = generate_cluster(cluster_size / 2, *dim, 10000);
        let context = ClusterContext::new(same_cluster, nearest_cluster);
        let config = ClusterFitConfig::default();

        group.throughput(Throughput::Elements(*dim as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &(query, context, config),
            |b, (q, ctx, cfg)| {
                b.iter(|| {
                    compute_cluster_fit(black_box(q), black_box(ctx), black_box(cfg))
                })
            },
        );
    }
    group.finish();
}

// =============================================================================
// Distance Metric Comparison Benchmarks
// =============================================================================

fn bench_cluster_fit_distance_metrics(c: &mut Criterion) {
    let dim = 1536;
    let cluster_size = 100;

    let query = generate_embedding(dim, 42);
    let same_cluster = generate_cluster(cluster_size, dim, 100);
    let nearest_cluster = generate_cluster(cluster_size / 2, dim, 10000);
    let context = ClusterContext::new(same_cluster, nearest_cluster);

    let mut group = c.benchmark_group("cluster_fit_distance_metrics");

    for (name, metric) in [
        ("cosine", DistanceMetric::Cosine),
        ("euclidean", DistanceMetric::Euclidean),
        ("manhattan", DistanceMetric::Manhattan),
    ] {
        let mut config = ClusterFitConfig::default();
        config.distance_metric = metric;

        group.bench_with_input(BenchmarkId::from_parameter(name), &config, |b, cfg| {
            b.iter(|| {
                compute_cluster_fit(
                    black_box(&query),
                    black_box(&context),
                    black_box(cfg),
                )
            })
        });
    }
    group.finish();
}

// =============================================================================
// Batch Processing Benchmark
// =============================================================================

fn bench_cluster_fit_batch(c: &mut Criterion) {
    let dim = 1536;
    let cluster_size = 100;

    // Pre-generate 100 queries
    let queries: Vec<Vec<f32>> = (0..100)
        .map(|i| generate_embedding(dim, i as u64 * 11))
        .collect();

    let same_cluster = generate_cluster(cluster_size, dim, 100);
    let nearest_cluster = generate_cluster(cluster_size / 2, dim, 10000);
    let context = ClusterContext::new(same_cluster, nearest_cluster);
    let config = ClusterFitConfig::default();

    c.bench_function("cluster_fit_batch_100_queries", |b| {
        b.iter(|| {
            for query in queries.iter() {
                let _ = compute_cluster_fit(
                    black_box(query),
                    black_box(&context),
                    black_box(&config),
                );
            }
        })
    });
}

// =============================================================================
// Sampling Threshold Benchmark
// =============================================================================

fn bench_cluster_fit_sampling(c: &mut Criterion) {
    let dim = 1536;
    let query = generate_embedding(dim, 42);

    // Create cluster larger than max_sample_size (default 1000)
    let large_cluster = generate_cluster(2000, dim, 100);
    let nearest_cluster = generate_cluster(500, dim, 10000);
    let context = ClusterContext::new(large_cluster, nearest_cluster);

    let mut group = c.benchmark_group("cluster_fit_sampling");

    // Compare with and without sampling
    for max_sample in [100, 500, 1000, 2000].iter() {
        let mut config = ClusterFitConfig::default();
        config.max_sample_size = *max_sample;

        group.throughput(Throughput::Elements(*max_sample as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(max_sample),
            &config,
            |b, cfg| {
                b.iter(|| {
                    compute_cluster_fit(
                        black_box(&query),
                        black_box(&context),
                        black_box(cfg),
                    )
                })
            },
        );
    }
    group.finish();
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    name = target_benches;
    config = Criterion::default();
    targets = bench_cluster_fit_1000_embeddings
);

criterion_group!(
    name = scaling_benches;
    config = Criterion::default().sample_size(50);
    targets =
        bench_cluster_fit_cluster_scaling,
        bench_cluster_fit_dimension_scaling
);

criterion_group!(
    name = comparison_benches;
    config = Criterion::default();
    targets =
        bench_cluster_fit_distance_metrics,
        bench_cluster_fit_sampling
);

criterion_group!(
    name = batch_benches;
    config = Criterion::default().sample_size(20);
    targets = bench_cluster_fit_batch
);

criterion_main!(target_benches, scaling_benches, comparison_benches, batch_benches);
