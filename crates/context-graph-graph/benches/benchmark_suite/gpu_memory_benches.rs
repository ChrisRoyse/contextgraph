//! GPU memory manager benchmarks.

use criterion::{black_box, BenchmarkId, Criterion};
use std::time::Duration;

/// Benchmark GPU memory manager operations.
pub fn bench_gpu_memory_manager(c: &mut Criterion) {
    use context_graph_graph::{GpuMemoryConfig, GpuMemoryManager, MemoryCategory};

    let mut group = c.benchmark_group("gpu_memory_manager");
    group.measurement_time(Duration::from_secs(3));

    // Use default config which sets 24GB budget
    let memory_config = GpuMemoryConfig::default();

    // Create manager (may fail if config invalid, skip benchmark if so)
    let manager = match GpuMemoryManager::new(memory_config) {
        Ok(m) => m,
        Err(e) => {
            eprintln!(
                "GPU Memory Manager creation failed: {:?} - skipping benchmarks",
                e
            );
            group.finish();
            return;
        }
    };

    // Allocation benchmarks with smaller sizes to fit in budget
    // Note: AllocationHandle is freed on drop, so allocation + deallocation is measured
    let allocation_sizes = [1024, 1024 * 1024, 64 * 1024 * 1024, 256 * 1024 * 1024];

    for &size in &allocation_sizes {
        let size_label = if size >= 1024 * 1024 {
            format!("{}MB", size / (1024 * 1024))
        } else {
            format!("{}KB", size / 1024)
        };

        group.bench_with_input(
            BenchmarkId::new("allocate_free", &size_label),
            &size,
            |b, &sz| {
                b.iter(|| {
                    // Handle is freed on drop when it goes out of scope
                    let _handle =
                        manager.allocate(black_box(sz), black_box(MemoryCategory::FaissIndex));
                })
            },
        );
    }

    // Stats retrieval benchmark
    group.bench_function("stats", |b| b.iter(|| manager.stats()));

    // Available memory check benchmark
    group.bench_function("available", |b| b.iter(|| manager.available()));

    // Used memory check benchmark
    group.bench_function("used", |b| b.iter(|| manager.used()));

    // Budget check benchmark
    group.bench_function("budget", |b| b.iter(|| manager.budget()));

    // Low memory check benchmark
    group.bench_function("is_low_memory", |b| b.iter(|| manager.is_low_memory()));

    // Category available benchmark
    group.bench_function("category_available", |b| {
        b.iter(|| manager.category_available(black_box(MemoryCategory::FaissIndex)))
    });

    // Try allocate (non-failing) benchmark
    group.bench_function("try_allocate", |b| {
        b.iter(|| {
            let _handle = manager.try_allocate(
                black_box(1024 * 1024),
                black_box(MemoryCategory::WorkingMemory),
            );
        })
    });

    group.finish();
}
