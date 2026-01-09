//! Domain search NT weight modulation benchmarks.

use criterion::{black_box, BenchmarkId, Criterion, Throughput};
use std::time::Duration;

use super::config;

/// Benchmark domain search NT weight modulation.
pub fn bench_domain_search_modulation(c: &mut Criterion) {
    use context_graph_core::marblestone::{Domain, NeurotransmitterWeights};

    let mut group = c.benchmark_group("domain_search_modulation");
    group.measurement_time(Duration::from_secs(3));

    // Benchmark NT profile creation for each domain
    let domains = [
        Domain::Code,
        Domain::Legal,
        Domain::Medical,
        Domain::Creative,
        Domain::Research,
        Domain::General,
    ];

    for domain in domains {
        group.bench_with_input(
            BenchmarkId::new("nt_profile", format!("{:?}", domain)),
            &domain,
            |b, &d| b.iter(|| NeurotransmitterWeights::for_domain(black_box(d))),
        );
    }

    // Benchmark net activation computation
    // CANONICAL FORMULA: net_activation = excitatory - inhibitory + (modulatory * 0.5)
    group.bench_function("net_activation", |b| {
        let nt = NeurotransmitterWeights::for_domain(Domain::Code);
        b.iter(|| {
            let net = black_box(nt.excitatory) - black_box(nt.inhibitory)
                + (black_box(nt.modulatory) * 0.5);
            black_box(net)
        })
    });

    // Benchmark modulation formula
    // CANONICAL: modulated_score = base * (1.0 + net_activation + domain_bonus)
    group.bench_function("modulation_formula", |b| {
        let base_similarity = 0.75f32;
        let net_activation = 0.5f32;
        let domain_bonus = 0.1f32;

        b.iter(|| {
            let modulated = black_box(base_similarity)
                * (1.0 + black_box(net_activation) + black_box(domain_bonus));
            black_box(modulated.clamp(0.0, 1.0))
        })
    });

    // Batch modulation benchmark
    for &batch_size in config::BATCH_SIZES {
        let base_similarities: Vec<f32> = (0..batch_size)
            .map(|i| 0.5 + (i as f32 / batch_size as f32) * 0.4)
            .collect();
        let net_activation = 0.5f32;
        let domain_bonus = 0.1f32;

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_modulation", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let results: Vec<f32> = base_similarities
                        .iter()
                        .map(|&base| {
                            let modulated = base * (1.0 + net_activation + domain_bonus);
                            modulated.clamp(0.0, 1.0)
                        })
                        .collect();
                    black_box(results)
                })
            },
        );
    }

    // Re-ranking benchmark (sort by modulated score)
    for &batch_size in config::BATCH_SIZES {
        let items: Vec<(i64, f32, f32)> = (0..batch_size as i64)
            .map(|i| {
                let base = 0.5 + (i as f32 / batch_size as f32) * 0.4;
                let modulated = base * 1.5;
                (i, base, modulated)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("rerank", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    let mut items_clone = items.clone();
                    items_clone
                        .sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
                    black_box(items_clone)
                })
            },
        );
    }

    group.finish();
}
