//! UTL benchmark suite for performance validation (M05-T25)
//!
//! Performance targets (per constitution.yaml and CUDA Report):
//! - compute_learning_magnitude: <100μs mean
//! - Full UTL computation: <10ms P99
//! - Surprise calculation: <5ms P99
//! - Emotional weight: <1ms mean
//! - Coherence tracking: <2ms mean

use context_graph_core::types::EmotionalState;
use context_graph_utl::{
    coherence::CoherenceTracker,
    compute_learning_magnitude, compute_learning_magnitude_validated,
    config::{CoherenceConfig, EmotionalConfig, SurpriseConfig, UtlConfig},
    emotional::EmotionalWeightCalculator,
    johari::JohariClassifier,
    lifecycle::LifecycleManager,
    phase::PhaseOscillator,
    processor::UtlProcessor,
    surprise::SurpriseCalculator,
};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// =============================================================================
// Helper Functions: Deterministic Data Generation
// =============================================================================

fn generate_embedding(dim: usize, seed: u64) -> Vec<f32> {
    (0..dim)
        .map(|i| ((i as f64 + seed as f64) * 0.1).sin() as f32)
        .collect()
}

fn generate_context(count: usize, dim: usize) -> Vec<Vec<f32>> {
    (0..count)
        .map(|i| generate_embedding(dim, i as u64 * 7))
        .collect()
}

// =============================================================================
// Core Formula Benchmarks
// =============================================================================

fn bench_compute_learning_magnitude(c: &mut Criterion) {
    c.bench_function("compute_learning_magnitude", |b| {
        b.iter(|| {
            compute_learning_magnitude(
                black_box(0.6),
                black_box(0.7),
                black_box(1.2),
                black_box(1.0),
            )
        })
    });
}

fn bench_compute_learning_magnitude_validated(c: &mut Criterion) {
    c.bench_function("compute_learning_magnitude_validated", |b| {
        b.iter(|| {
            compute_learning_magnitude_validated(
                black_box(0.6),
                black_box(0.7),
                black_box(1.2),
                black_box(1.0),
            )
        })
    });
}

// =============================================================================
// Full UTL Pipeline Benchmarks
// =============================================================================

fn bench_full_utl_computation(c: &mut Criterion) {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = generate_embedding(1536, 42);
    let context = generate_context(50, 1536);
    let content = "Test content for UTL computation with emotional variance!";

    c.bench_function("full_utl_computation", |b| {
        b.iter(|| {
            processor.compute_learning(
                black_box(content),
                black_box(&embedding),
                black_box(&context),
            )
        })
    });
}

fn bench_full_utl_with_emotional_state(c: &mut Criterion) {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = generate_embedding(1536, 42);
    let context = generate_context(50, 1536);
    let content = "Test content for UTL computation with focused state!";

    c.bench_function("full_utl_with_emotional_state", |b| {
        b.iter(|| {
            processor.compute_learning_with_state(
                black_box(content),
                black_box(&embedding),
                black_box(&context),
                black_box(EmotionalState::Focused),
            )
        })
    });
}

// =============================================================================
// Component Benchmarks
// =============================================================================

fn bench_surprise_calculation(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);
    let observed = generate_embedding(1536, 42);
    let context = generate_context(50, 1536);

    c.bench_function("surprise_calculation", |b| {
        b.iter(|| calculator.compute_surprise(black_box(&observed), black_box(&context)))
    });
}

fn bench_emotional_weight(c: &mut Criterion) {
    let config = EmotionalConfig::default();
    let calculator = EmotionalWeightCalculator::new(&config);
    let content = "EXCELLENT! This is an amazing and wonderful discovery!!!";

    c.bench_function("emotional_weight", |b| {
        b.iter(|| calculator.compute_emotional_weight(black_box(content), EmotionalState::Neutral))
    });
}

fn bench_emotional_weight_empty(c: &mut Criterion) {
    let config = EmotionalConfig::default();
    let calculator = EmotionalWeightCalculator::new(&config);
    let content = "";

    c.bench_function("emotional_weight_empty", |b| {
        b.iter(|| calculator.compute_emotional_weight(black_box(content), EmotionalState::Neutral))
    });
}

fn bench_coherence_tracking(c: &mut Criterion) {
    let config = CoherenceConfig::default();
    let tracker = CoherenceTracker::new(&config);
    let embedding = generate_embedding(1536, 42);
    let context = generate_context(50, 1536);

    c.bench_function("coherence_tracking", |b| {
        b.iter(|| tracker.compute_coherence(black_box(&embedding), black_box(&context)))
    });
}

fn bench_johari_classification(c: &mut Criterion) {
    let classifier = JohariClassifier::default();

    c.bench_function("johari_classification", |b| {
        b.iter(|| classifier.classify(black_box(0.6), black_box(0.7)))
    });
}

fn bench_phase_oscillator(c: &mut Criterion) {
    let config = context_graph_utl::config::PhaseConfig::default();
    let oscillator = PhaseOscillator::new(&config);

    c.bench_function("phase_oscillator_get", |b| {
        b.iter(|| black_box(oscillator.phase()))
    });
}

fn bench_lifecycle_stage(c: &mut Criterion) {
    let config = context_graph_utl::config::LifecycleConfig::default();
    let manager = LifecycleManager::new(&config);

    c.bench_function("lifecycle_stage_get", |b| {
        b.iter(|| black_box(manager.current_stage()))
    });
}

fn bench_lifecycle_weights(c: &mut Criterion) {
    let config = context_graph_utl::config::LifecycleConfig::default();
    let manager = LifecycleManager::new(&config);

    c.bench_function("lifecycle_weights_get", |b| {
        b.iter(|| black_box(manager.current_weights()))
    });
}

// =============================================================================
// Scaling Benchmarks
// =============================================================================

fn bench_context_scaling(c: &mut Criterion) {
    let mut processor = UtlProcessor::with_defaults();
    let embedding = generate_embedding(1536, 42);
    let content = "Test content";

    let mut group = c.benchmark_group("utl_context_scaling");
    for size in [10, 25, 50, 100].iter() {
        let context = generate_context(*size, 1536);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &context, |b, ctx| {
            b.iter(|| {
                processor.compute_learning(
                    black_box(content),
                    black_box(&embedding),
                    black_box(ctx),
                )
            })
        });
    }
    group.finish();
}

fn bench_embedding_dimension_scaling(c: &mut Criterion) {
    let mut processor = UtlProcessor::with_defaults();
    let content = "Test content for dimension scaling";

    let mut group = c.benchmark_group("utl_embedding_dimension");
    for dim in [384, 768, 1536, 3072].iter() {
        let embedding = generate_embedding(*dim, 42);
        let context = generate_context(20, *dim);
        group.throughput(Throughput::Elements(*dim as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &(embedding, context),
            |b, (emb, ctx)| {
                b.iter(|| {
                    processor.compute_learning(black_box(content), black_box(emb), black_box(ctx))
                })
            },
        );
    }
    group.finish();
}

fn bench_surprise_context_scaling(c: &mut Criterion) {
    let config = SurpriseConfig::default();
    let calculator = SurpriseCalculator::new(&config);
    let observed = generate_embedding(1536, 42);

    let mut group = c.benchmark_group("surprise_context_scaling");
    for size in [10, 25, 50, 100, 200].iter() {
        let context = generate_context(*size, 1536);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &context, |b, ctx| {
            b.iter(|| calculator.compute_surprise(black_box(&observed), black_box(ctx)))
        });
    }
    group.finish();
}

// =============================================================================
// Content Length Benchmarks
// =============================================================================

fn bench_emotional_content_scaling(c: &mut Criterion) {
    let config = EmotionalConfig::default();
    let calculator = EmotionalWeightCalculator::new(&config);

    let short_content = "Short text.";
    let medium_content = "This is a medium length text that contains more words and emotional content! It's quite exciting!";
    let long_content = "This is a much longer text that contains many sentences. It has various emotional words like amazing, terrible, wonderful, and frightening! The content goes on and on to test how the emotional weight calculator handles longer inputs. We want to ensure that performance remains acceptable even with substantial text content. More words here to make it even longer.";

    let mut group = c.benchmark_group("emotional_content_length");

    group.bench_function("short", |b| {
        b.iter(|| {
            calculator.compute_emotional_weight(black_box(short_content), EmotionalState::Neutral)
        })
    });

    group.bench_function("medium", |b| {
        b.iter(|| {
            calculator.compute_emotional_weight(black_box(medium_content), EmotionalState::Neutral)
        })
    });

    group.bench_function("long", |b| {
        b.iter(|| {
            calculator.compute_emotional_weight(black_box(long_content), EmotionalState::Neutral)
        })
    });

    group.finish();
}

// =============================================================================
// Batch Processing Benchmarks
// =============================================================================

fn bench_batch_computations(c: &mut Criterion) {
    let mut processor = UtlProcessor::with_defaults();
    let content = "Batch test content";

    let embeddings: Vec<Vec<f32>> = (0..100)
        .map(|i| generate_embedding(1536, i as u64))
        .collect();
    let context = generate_context(20, 1536);

    c.bench_function("batch_100_computations", |b| {
        b.iter(|| {
            for emb in embeddings.iter() {
                let _ = processor.compute_learning(
                    black_box(content),
                    black_box(emb),
                    black_box(&context),
                );
            }
        })
    });
}

// =============================================================================
// Processor Creation Benchmarks
// =============================================================================

fn bench_processor_creation(c: &mut Criterion) {
    c.bench_function("processor_creation_default", |b| {
        b.iter(|| black_box(UtlProcessor::with_defaults()))
    });
}

fn bench_processor_creation_custom(c: &mut Criterion) {
    let config = UtlConfig::default();

    c.bench_function("processor_creation_custom", |b| {
        b.iter(|| black_box(UtlProcessor::new(config.clone())))
    });
}

// =============================================================================
// Criterion Groups
// =============================================================================

criterion_group!(
    name = core_benches;
    config = Criterion::default();
    targets =
        bench_compute_learning_magnitude,
        bench_compute_learning_magnitude_validated,
);

criterion_group!(
    name = pipeline_benches;
    config = Criterion::default();
    targets =
        bench_full_utl_computation,
        bench_full_utl_with_emotional_state,
);

criterion_group!(
    name = component_benches;
    config = Criterion::default();
    targets =
        bench_surprise_calculation,
        bench_emotional_weight,
        bench_emotional_weight_empty,
        bench_coherence_tracking,
        bench_johari_classification,
        bench_phase_oscillator,
        bench_lifecycle_stage,
        bench_lifecycle_weights,
);

criterion_group!(
    name = scaling_benches;
    config = Criterion::default().sample_size(50);
    targets =
        bench_context_scaling,
        bench_embedding_dimension_scaling,
        bench_surprise_context_scaling,
        bench_emotional_content_scaling,
);

criterion_group!(
    name = batch_benches;
    config = Criterion::default().sample_size(20);
    targets =
        bench_batch_computations,
);

criterion_group!(
    name = creation_benches;
    config = Criterion::default();
    targets =
        bench_processor_creation,
        bench_processor_creation_custom,
);

criterion_main!(
    core_benches,
    pipeline_benches,
    component_benches,
    scaling_benches,
    batch_benches,
    creation_benches,
);
