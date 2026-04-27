//! Scaling analysis benchmarks.
//!
//! Measures how performance degrades as corpus size increases.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use context_graph_benchmark::baseline::SingleEmbedderBaseline;
use context_graph_benchmark::config::{Tier, TierConfig};
use context_graph_benchmark::datasets::{DatasetGenerator, GeneratorConfig};

/// Benchmark search latency at different corpus sizes.
fn bench_search_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("search_scaling");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // Test at exponentially increasing corpus sizes
    let sizes = [100, 500, 1000, 5000, 10000];

    for &size in &sizes {
        // Generate dataset
        let mut config = TierConfig::for_tier(Tier::Tier0);
        config.memory_count = size;
        config.query_count = 50;

        let gen_config = GeneratorConfig {
            seed: 42,
            ..Default::default()
        };
        let mut generator = DatasetGenerator::with_config(gen_config);
        let dataset = generator.generate_dataset(&config);

        // Build single-embedder baseline
        let mut baseline = SingleEmbedderBaseline::new();
        for (id, fp) in &dataset.fingerprints {
            baseline.insert(*id, &fp.e1_semantic);
        }

        group.throughput(Throughput::Elements(1)); // Per-query throughput

        group.bench_with_input(
            BenchmarkId::new("single_embedder", size),
            &(&baseline, &dataset),
            |b, (baseline, dataset)| {
                let mut query_idx = 0;
                b.iter(|| {
                    let query = &dataset.queries[query_idx % dataset.queries.len()];
                    let results = black_box(baseline.search(&query.embedding, 10));
                    query_idx += 1;
                    results
                })
            },
        );
    }

    group.finish();
}

/// Benchmark index build time at different corpus sizes.
fn bench_index_build_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build_scaling");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);

    let sizes = [100, 500, 1000, 2000];

    for &size in &sizes {
        let mut config = TierConfig::for_tier(Tier::Tier0);
        config.memory_count = size;

        let gen_config = GeneratorConfig {
            seed: 42,
            ..Default::default()
        };
        let mut generator = DatasetGenerator::with_config(gen_config);
        let dataset = generator.generate_dataset(&config);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("build_baseline", size),
            &dataset,
            |b, dataset| {
                b.iter(|| {
                    let mut baseline = SingleEmbedderBaseline::new();
                    for (id, fp) in &dataset.fingerprints {
                        baseline.insert(*id, &fp.e1_semantic);
                    }
                    black_box(baseline)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark memory usage estimation at different corpus sizes.
fn bench_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_estimation");
    group.measurement_time(Duration::from_secs(3));
    group.sample_size(100);

    let sizes = [100, 1000, 10000, 100000, 1000000];

    for &size in &sizes {
        group.throughput(Throughput::Elements(1));

        group.bench_with_input(
            BenchmarkId::new("single_embedder", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let estimate = context_graph_benchmark::scaling::memory_profiler::estimate_single_embedder_memory(size);
                    black_box(estimate.total())
                })
            },
        );

        group.bench_with_input(BenchmarkId::new("multi_space", size), &size, |b, &size| {
            b.iter(|| {
                let estimate =
                    context_graph_benchmark::scaling::memory_profiler::estimate_multispace_memory(
                        size,
                    );
                black_box(estimate.total())
            })
        });
    }

    group.finish();
}

/// Benchmark k-means clustering at different corpus sizes.
fn bench_clustering_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("clustering_scaling");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    let sizes = [100, 500, 1000];

    for &size in &sizes {
        let mut config = TierConfig::for_tier(Tier::Tier0);
        config.memory_count = size;
        config.topic_count = 5;

        let gen_config = GeneratorConfig {
            seed: 42,
            ..Default::default()
        };
        let mut generator = DatasetGenerator::with_config(gen_config);
        let dataset = generator.generate_dataset(&config);

        // Build baseline
        let mut baseline = SingleEmbedderBaseline::new();
        for (id, fp) in &dataset.fingerprints {
            baseline.insert(*id, &fp.e1_semantic);
        }

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("kmeans", size),
            &(&baseline, config.topic_count),
            |b, (baseline, n_clusters)| {
                b.iter(|| black_box(baseline.detect_topics_kmeans(*n_clusters, 50)))
            },
        );
    }

    group.finish();
}

criterion_group!(
    scaling_benches,
    bench_search_scaling,
    bench_index_build_scaling,
    bench_memory_scaling,
    bench_clustering_scaling
);

criterion_main!(scaling_benches);
