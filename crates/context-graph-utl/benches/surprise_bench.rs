//! Surprise computation benchmark suite (M05-T25)
//!
//! Performance targets:
//! - compute_surprise: <5ms P99 for 50 context vectors
//! - compute_kl_divergence: <100μs
//! - compute_cosine_distance: <50μs for 1536-dim vectors

use context_graph_utl::config::SurpriseConfig;
use context_graph_utl::surprise::{
    compute_cosine_distance, compute_kl_divergence, SurpriseCalculator,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// =============================================================================
// Helper Functions
// =============================================================================

fn generate_embedding(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| ((i as f64 + seed as f64) * 0.1).sin() as f32)
        .collect()
}

fn generate_normalized_distribution(size: usize, seed: u64) -> Vec<f32> {
    let raw: Vec<f32> = (0..size)
        .map(|i| ((i as f64 + seed as f64) * 0.3).sin().abs() as f32 + 0.01)
        .collect();
    let sum: f32 = raw.iter().sum();
    raw.iter().map(|v| v / sum).collect()
}

// =============================================================================
// Core Surprise Benchmarks
// =============================================================================

fn bench_compute_surprise(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);

    // Standard test case: 384-dim embedding, 10 context vectors
    let current: Vec<f32> = generate_embedding(384, 42);
    let history: Vec<Vec<f32>> = (0..10)
        .map(|j| generate_embedding(384, j as u64 * 7))
        .collect();

    c.bench_function("compute_surprise_384dim_10ctx", |b| {
        b.iter(|| calculator.compute_surprise(black_box(&current), black_box(&history)))
    });
}

fn bench_compute_surprise_large(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);

    // Production-like: 1536-dim embedding, 50 context vectors
    let current: Vec<f32> = generate_embedding(1536, 42);
    let history: Vec<Vec<f32>> = (0..50)
        .map(|j| generate_embedding(1536, j as u64 * 7))
        .collect();

    c.bench_function("compute_surprise_1536dim_50ctx", |b| {
        b.iter(|| calculator.compute_surprise(black_box(&current), black_box(&history)))
    });
}

fn bench_compute_surprise_empty_context(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);

    let current: Vec<f32> = generate_embedding(1536, 42);
    let history: Vec<Vec<f32>> = vec![];

    c.bench_function("compute_surprise_empty_context", |b| {
        b.iter(|| calculator.compute_surprise(black_box(&current), black_box(&history)))
    });
}

// =============================================================================
// KL Divergence Benchmarks
// =============================================================================

fn bench_compute_kl_divergence(c: &mut Criterion) {
    let p: Vec<f32> = vec![0.25, 0.25, 0.25, 0.25];
    let q: Vec<f32> = vec![0.1, 0.2, 0.3, 0.4];

    c.bench_function("compute_kl_divergence_4elem", |b| {
        b.iter(|| compute_kl_divergence(black_box(&p), black_box(&q), 1e-10))
    });
}

fn bench_compute_kl_divergence_large(c: &mut Criterion) {
    let p = generate_normalized_distribution(100, 42);
    let q = generate_normalized_distribution(100, 123);

    c.bench_function("compute_kl_divergence_100elem", |b| {
        b.iter(|| compute_kl_divergence(black_box(&p), black_box(&q), 1e-10))
    });
}

fn bench_kl_divergence_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("kl_divergence_scaling");

    for size in [4, 16, 64, 256, 1024].iter() {
        let p = generate_normalized_distribution(*size, 42);
        let q = generate_normalized_distribution(*size, 123);

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &(p, q), |b, (p, q)| {
            b.iter(|| compute_kl_divergence(black_box(p), black_box(q), 1e-10))
        });
    }

    group.finish();
}

// =============================================================================
// Cosine Distance Benchmarks
// =============================================================================

fn bench_compute_cosine_distance(c: &mut Criterion) {
    let a: Vec<f32> = generate_embedding(384, 42);
    let b: Vec<f32> = generate_embedding(384, 123);

    c.bench_function("compute_cosine_distance_384dim", |b_iter| {
        b_iter.iter(|| compute_cosine_distance(black_box(&a), black_box(&b)))
    });
}

fn bench_compute_cosine_distance_large(c: &mut Criterion) {
    let a: Vec<f32> = generate_embedding(1536, 42);
    let b: Vec<f32> = generate_embedding(1536, 123);

    c.bench_function("compute_cosine_distance_1536dim", |b_iter| {
        b_iter.iter(|| compute_cosine_distance(black_box(&a), black_box(&b)))
    });
}

fn bench_cosine_distance_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_distance_scaling");

    for dim in [128, 384, 768, 1536, 3072].iter() {
        let a = generate_embedding(*dim, 42);
        let b = generate_embedding(*dim, 123);

        group.throughput(Throughput::Elements(*dim as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &(a, b),
            |b_iter, (a, b)| b_iter.iter(|| compute_cosine_distance(black_box(a), black_box(b))),
        );
    }

    group.finish();
}

// =============================================================================
// Context Scaling Benchmarks
// =============================================================================

fn bench_surprise_context_scaling(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);
    let current = generate_embedding(1536, 42);

    let mut group = c.benchmark_group("surprise_context_scaling");

    for context_size in [1, 5, 10, 25, 50, 100].iter() {
        let history: Vec<Vec<f32>> = (0..*context_size)
            .map(|j| generate_embedding(1536, j as u64 * 7))
            .collect();

        group.throughput(Throughput::Elements(*context_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(context_size),
            &history,
            |b, hist| b.iter(|| calculator.compute_surprise(black_box(&current), black_box(hist))),
        );
    }

    group.finish();
}

// =============================================================================
// Embedding Dimension Scaling Benchmarks
// =============================================================================

fn bench_surprise_dimension_scaling(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);

    let mut group = c.benchmark_group("surprise_dimension_scaling");

    for dim in [128, 384, 768, 1536, 3072].iter() {
        let current = generate_embedding(*dim, 42);
        let history: Vec<Vec<f32>> = (0..20)
            .map(|j| generate_embedding(*dim, j as u64 * 7))
            .collect();

        group.throughput(Throughput::Elements(*dim as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &(current, history),
            |b, (curr, hist)| {
                b.iter(|| calculator.compute_surprise(black_box(curr), black_box(hist)))
            },
        );
    }

    group.finish();
}

// =============================================================================
// Edge Case Benchmarks
// =============================================================================

fn bench_surprise_identical_vectors(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);

    // All vectors identical - should compute quickly with low surprise
    let embedding: Vec<f32> = vec![0.5; 1536];
    let history: Vec<Vec<f32>> = (0..20).map(|_| vec![0.5; 1536]).collect();

    c.bench_function("compute_surprise_identical", |b| {
        b.iter(|| calculator.compute_surprise(black_box(&embedding), black_box(&history)))
    });
}

fn bench_surprise_orthogonal_vectors(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);

    // Orthogonal vectors - maximum surprise
    let embedding: Vec<f32> = (0..1536).map(|i| if i < 768 { 1.0 } else { 0.0 }).collect();
    let history: Vec<Vec<f32>> = (0..20)
        .map(|_| {
            (0..1536)
                .map(|i| if i >= 768 { 1.0 } else { 0.0 })
                .collect()
        })
        .collect();

    c.bench_function("compute_surprise_orthogonal", |b| {
        b.iter(|| calculator.compute_surprise(black_box(&embedding), black_box(&history)))
    });
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    name = core_surprise;
    config = Criterion::default();
    targets =
        bench_compute_surprise,
        bench_compute_surprise_large,
        bench_compute_surprise_empty_context,
);

criterion_group!(
    name = kl_divergence;
    config = Criterion::default();
    targets =
        bench_compute_kl_divergence,
        bench_compute_kl_divergence_large,
        bench_kl_divergence_scaling,
);

criterion_group!(
    name = cosine_distance;
    config = Criterion::default();
    targets =
        bench_compute_cosine_distance,
        bench_compute_cosine_distance_large,
        bench_cosine_distance_scaling,
);

criterion_group!(
    name = scaling;
    config = Criterion::default().sample_size(50);
    targets =
        bench_surprise_context_scaling,
        bench_surprise_dimension_scaling,
);

criterion_group!(
    name = edge_cases;
    config = Criterion::default();
    targets =
        bench_surprise_identical_vectors,
        bench_surprise_orthogonal_vectors,
);

criterion_main!(
    core_surprise,
    kl_divergence,
    cosine_distance,
    scaling,
    edge_cases,
);
