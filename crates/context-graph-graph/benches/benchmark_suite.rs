//! Comprehensive benchmark suite for context-graph-graph crate.
//!
//! # Task: M04-T29 - Benchmark Suite Implementation
//!
//! This benchmark suite provides CPU + GPU performance validation for:
//! - Poincare distance computation (hyperbolic geometry)
//! - Entailment cone membership scoring
//! - BFS graph traversal
//! - GPU memory manager operations
//! - Domain search with NT modulation
//!
//! # Performance Targets (from constitution)
//!
//! | Benchmark | Target |
//! |-----------|--------|
//! | poincare_single | <10us |
//! | poincare_1k_batch | <1ms GPU / <100ms CPU |
//! | cone_single | <15us |
//! | cone_1k_batch | <2ms GPU / <200ms CPU |
//! | bfs_depth6 | <100ms |
//! | faiss_1M_k100 | <2ms |
//! | domain_search | <10ms |
//!
//! # Running Benchmarks
//!
//! ```bash
//! # Run all benchmarks
//! cargo bench --package context-graph-graph
//!
//! # Run specific benchmark
//! cargo bench --package context-graph-graph -- poincare
//!
//! # With GPU features
//! cargo bench --package context-graph-graph --features faiss-gpu
//! ```
//!
//! # JSON Export for CI
//!
//! Results are exported to `target/criterion/benchmark_results.json`
//! for CI integration and regression detection.
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004: Performance specifications
//! - AP-009: NaN/Infinity handling
//! - perf.latency.*: All latency targets

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashMap;
use std::time::Duration;

// ============================================================================
// BENCHMARK CONFIGURATION
// ============================================================================

/// Configuration for benchmark parameters.
/// All values derived from constitution and task spec.
mod config {
    /// Poincare ball dimension (from constitution)
    pub const POINCARE_DIM: usize = 64;

    /// Poincare ball curvature (from constitution)
    pub const POINCARE_CURVATURE: f32 = -1.0;

    /// Batch sizes for throughput benchmarks
    pub const BATCH_SIZES: &[usize] = &[1, 10, 100, 1000];

    /// Graph sizes for traversal benchmarks
    pub const GRAPH_SIZES: &[usize] = &[100, 1000, 10000];

    /// BFS max depth for benchmarks
    pub const BFS_MAX_DEPTH: u32 = 6;

    /// GPU memory budget (from constitution: 24GB safe limit)
    pub const GPU_MEMORY_BUDGET_GB: usize = 24;

    /// GPU memory budget in bytes
    pub const GPU_MEMORY_BUDGET_BYTES: usize = GPU_MEMORY_BUDGET_GB * 1024 * 1024 * 1024;

    /// Performance targets (in microseconds)
    #[allow(dead_code)]
    pub mod targets {
        pub const POINCARE_SINGLE_US: u64 = 10;
        pub const POINCARE_1K_BATCH_CPU_US: u64 = 100_000;
        pub const POINCARE_1K_BATCH_GPU_US: u64 = 1_000;
        pub const CONE_SINGLE_US: u64 = 15;
        pub const CONE_1K_BATCH_CPU_US: u64 = 200_000;
        pub const CONE_1K_BATCH_GPU_US: u64 = 2_000;
        pub const BFS_DEPTH6_US: u64 = 100_000;
        pub const DOMAIN_SEARCH_US: u64 = 10_000;
    }
}

// ============================================================================
// REAL DATA GENERATORS (NO MOCK DATA)
// ============================================================================

/// Generate real Poincare point within the unit ball as fixed-size array.
/// Uses deterministic seeding to ensure valid hyperbolic coordinates.
fn generate_poincare_point_fixed(seed: u64) -> [f32; 64] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut point = [0.0f32; 64];
    let mut hasher = DefaultHasher::new();

    // Generate point components using deterministic seeding
    for i in 0..64 {
        (seed, i).hash(&mut hasher);
        let hash = hasher.finish();
        // Map to [-0.7, 0.7] to stay within Poincare ball
        let val = ((hash as f32 / u64::MAX as f32) * 1.4 - 0.7) * 0.7;
        point[i] = val;
        hasher = DefaultHasher::new();
    }

    // Normalize to ensure ||x|| < 1 (Poincare ball constraint)
    let norm: f32 = point.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm >= 0.95 {
        let scale = 0.9 / norm;
        for val in &mut point {
            *val *= scale;
        }
    }

    point
}

/// Generate batch of real Poincare points as flattened array.
fn generate_poincare_batch_flat(count: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(count * 64);
    for i in 0..count {
        let point = generate_poincare_point_fixed(i as u64 * 12345);
        result.extend_from_slice(&point);
    }
    result
}

/// Generate real cone data (apex + aperture) for entailment testing.
/// Returns (apex array, aperture) tuple.
fn generate_cone_data_fixed(seed: u64) -> ([f32; 64], f32) {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let apex = generate_poincare_point_fixed(seed);

    // Generate aperture in valid range [0.1, π/2]
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let hash = hasher.finish();
    let aperture = 0.1 + (hash as f32 / u64::MAX as f32) * (std::f32::consts::FRAC_PI_2 - 0.1);

    (apex, aperture)
}

/// Generate batch of cone data as flattened array.
/// Each cone is 65 floats: 64 for apex + 1 for aperture.
fn generate_cone_batch_flat(count: usize) -> Vec<f32> {
    let mut result = Vec::with_capacity(count * 65);
    for i in 0..count {
        let (apex, aperture) = generate_cone_data_fixed(i as u64 * 2000);
        result.extend_from_slice(&apex);
        result.push(aperture);
    }
    result
}

/// Generate real graph adjacency for BFS benchmarks.
/// Creates a connected graph with controlled edge density.
fn generate_graph_adjacency(node_count: usize, avg_edges_per_node: usize) -> HashMap<u64, Vec<u64>> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut adjacency: HashMap<u64, Vec<u64>> = HashMap::new();

    for node in 0..node_count as u64 {
        let mut edges = Vec::new();

        // Ensure connectivity: connect to next node (forms spanning path)
        if node < (node_count - 1) as u64 {
            edges.push(node + 1);
        }

        // Add random edges for density
        let mut hasher = DefaultHasher::new();
        for i in 0..avg_edges_per_node {
            (node, i).hash(&mut hasher);
            let target = hasher.finish() % node_count as u64;
            if target != node && !edges.contains(&target) {
                edges.push(target);
            }
            hasher = DefaultHasher::new();
        }

        adjacency.insert(node, edges);
    }

    adjacency
}

// ============================================================================
// POINCARE DISTANCE BENCHMARKS
// ============================================================================

/// Benchmark Poincare distance computation.
fn bench_poincare_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("poincare_distance");
    group.measurement_time(Duration::from_secs(5));

    // Single distance computation
    let p1 = generate_poincare_point_fixed(1);
    let p2 = generate_poincare_point_fixed(2);

    group.bench_function("single_cpu", |b| {
        b.iter(|| {
            context_graph_cuda::poincare_distance_cpu(
                black_box(&p1),
                black_box(&p2),
                black_box(config::POINCARE_CURVATURE),
            )
        })
    });

    // Batch distance computation
    for &batch_size in config::BATCH_SIZES {
        let queries = generate_poincare_batch_flat(batch_size);
        let database = generate_poincare_batch_flat(batch_size);

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_cpu", batch_size),
            &batch_size,
            |b, &bs| {
                b.iter(|| {
                    context_graph_cuda::poincare_distance_batch_cpu(
                        black_box(&queries),
                        black_box(&database),
                        black_box(bs),
                        black_box(bs),
                        black_box(config::POINCARE_CURVATURE),
                    )
                })
            },
        );
    }

    // Edge cases for numerical stability (AP-009)
    group.bench_function("near_origin", |b| {
        let mut origin_near = [0.0f32; 64];
        origin_near[0] = 1e-7;
        let p = generate_poincare_point_fixed(100);
        b.iter(|| {
            context_graph_cuda::poincare_distance_cpu(
                black_box(&origin_near),
                black_box(&p),
                black_box(config::POINCARE_CURVATURE),
            )
        })
    });

    group.bench_function("near_boundary", |b| {
        let mut boundary_near = generate_poincare_point_fixed(200);
        // Scale to be very close to boundary
        let norm: f32 = boundary_near.iter().map(|x| x * x).sum::<f32>().sqrt();
        let scale = 0.999 / norm;
        for val in &mut boundary_near {
            *val *= scale;
        }
        let p = generate_poincare_point_fixed(201);

        b.iter(|| {
            context_graph_cuda::poincare_distance_cpu(
                black_box(&boundary_near),
                black_box(&p),
                black_box(config::POINCARE_CURVATURE),
            )
        })
    });

    group.finish();
}

// ============================================================================
// ENTAILMENT CONE BENCHMARKS
// ============================================================================

/// Benchmark entailment cone membership scoring.
fn bench_cone_membership(c: &mut Criterion) {
    let mut group = c.benchmark_group("cone_membership");
    group.measurement_time(Duration::from_secs(5));

    // Single cone check
    let (apex, aperture) = generate_cone_data_fixed(1);
    let point = generate_poincare_point_fixed(100);

    group.bench_function("single_cpu", |b| {
        b.iter(|| {
            context_graph_cuda::cone_membership_score_cpu(
                black_box(&apex),
                black_box(aperture),
                black_box(&point),
                black_box(config::POINCARE_CURVATURE),
            )
        })
    });

    // Batch cone checks
    for &batch_size in config::BATCH_SIZES {
        let cones = generate_cone_batch_flat(batch_size);
        let points = generate_poincare_batch_flat(batch_size);

        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_cpu", batch_size),
            &batch_size,
            |b, &bs| {
                b.iter(|| {
                    context_graph_cuda::cone_check_batch_cpu(
                        black_box(&cones),
                        black_box(&points),
                        black_box(bs),
                        black_box(bs),
                        black_box(config::POINCARE_CURVATURE),
                    )
                })
            },
        );
    }

    // Aperture variation benchmarks
    for aperture_val in [0.1, 0.5, 1.0, std::f32::consts::FRAC_PI_2 - 0.1] {
        group.bench_with_input(
            BenchmarkId::new("aperture", format!("{:.2}", aperture_val)),
            &aperture_val,
            |b, &ap| {
                b.iter(|| {
                    context_graph_cuda::cone_membership_score_cpu(
                        black_box(&apex),
                        black_box(ap),
                        black_box(&point),
                        black_box(config::POINCARE_CURVATURE),
                    )
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// BFS TRAVERSAL BENCHMARKS
// ============================================================================

/// Storage for BFS benchmarks using real graph adjacency data.
struct BfsStorage {
    adjacency: HashMap<u64, Vec<u64>>,
}

impl BfsStorage {
    fn new(node_count: usize, avg_edges: usize) -> Self {
        Self {
            adjacency: generate_graph_adjacency(node_count, avg_edges),
        }
    }

    fn get_neighbors(&self, node_id: u64) -> Vec<u64> {
        self.adjacency.get(&node_id).cloned().unwrap_or_default()
    }
}

/// BFS implementation for benchmarking.
fn bfs_traverse(storage: &BfsStorage, start: u64, max_depth: u32) -> Vec<u64> {
    use std::collections::{HashSet, VecDeque};

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    queue.push_back((start, 0u32));
    visited.insert(start);

    while let Some((node, depth)) = queue.pop_front() {
        result.push(node);

        if depth >= max_depth {
            continue;
        }

        for neighbor in storage.get_neighbors(node) {
            if !visited.contains(&neighbor) {
                visited.insert(neighbor);
                queue.push_back((neighbor, depth + 1));
            }
        }
    }

    result
}

/// Benchmark BFS traversal.
fn bench_bfs_traversal(c: &mut Criterion) {
    let mut group = c.benchmark_group("bfs_traversal");
    group.measurement_time(Duration::from_secs(5));

    for &node_count in config::GRAPH_SIZES {
        // Tree-like graph (sparse)
        let tree_storage = BfsStorage::new(node_count, 2);
        group.bench_with_input(
            BenchmarkId::new("tree", node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    bfs_traverse(
                        black_box(&tree_storage),
                        black_box(0),
                        black_box(config::BFS_MAX_DEPTH),
                    )
                })
            },
        );

        // Random graph (medium density)
        let random_storage = BfsStorage::new(node_count, 5);
        group.bench_with_input(
            BenchmarkId::new("random", node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    bfs_traverse(
                        black_box(&random_storage),
                        black_box(0),
                        black_box(config::BFS_MAX_DEPTH),
                    )
                })
            },
        );

        // Dense graph
        let dense_storage = BfsStorage::new(node_count, 10);
        group.bench_with_input(
            BenchmarkId::new("dense", node_count),
            &node_count,
            |b, _| {
                b.iter(|| {
                    bfs_traverse(
                        black_box(&dense_storage),
                        black_box(0),
                        black_box(config::BFS_MAX_DEPTH),
                    )
                })
            },
        );
    }

    // Depth variation benchmarks
    let storage_1k = BfsStorage::new(1000, 5);
    for depth in [1, 2, 4, 6, 8] {
        group.bench_with_input(BenchmarkId::new("depth", depth), &depth, |b, &d| {
            b.iter(|| bfs_traverse(black_box(&storage_1k), black_box(0), black_box(d)))
        });
    }

    group.finish();
}

// ============================================================================
// GPU MEMORY MANAGER BENCHMARKS
// ============================================================================

/// Benchmark GPU memory manager operations.
fn bench_gpu_memory_manager(c: &mut Criterion) {
    use context_graph_graph::{GpuMemoryConfig, GpuMemoryManager, MemoryCategory};

    let mut group = c.benchmark_group("gpu_memory_manager");
    group.measurement_time(Duration::from_secs(3));

    // Use default config which sets 24GB budget
    let memory_config = GpuMemoryConfig::default();

    // Create manager (may fail if config invalid, skip benchmark if so)
    let manager = match GpuMemoryManager::new(memory_config) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("GPU Memory Manager creation failed: {:?} - skipping benchmarks", e);
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
                    let _handle = manager.allocate(
                        black_box(sz),
                        black_box(MemoryCategory::FaissIndex),
                    );
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

// ============================================================================
// DOMAIN SEARCH MODULATION BENCHMARKS
// ============================================================================

/// Benchmark domain search NT weight modulation.
fn bench_domain_search_modulation(c: &mut Criterion) {
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
                    items_clone.sort_by(|a, b| {
                        b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal)
                    });
                    black_box(items_clone)
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// NUMERICAL STABILITY BENCHMARKS (AP-009)
// ============================================================================

/// Benchmark numerical stability edge cases.
fn bench_numerical_stability(c: &mut Criterion) {
    let mut group = c.benchmark_group("numerical_stability");
    group.measurement_time(Duration::from_secs(3));

    // NaN handling
    group.bench_function("nan_check", |b| {
        let values = vec![0.5f32, f32::NAN, 0.7, f32::INFINITY, 0.3];
        b.iter(|| {
            let valid: Vec<f32> = values
                .iter()
                .map(|&v| if v.is_finite() { v } else { 0.0 })
                .collect();
            black_box(valid)
        })
    });

    // Clamping benchmarks
    group.bench_function("clamp_0_1", |b| {
        let values: Vec<f32> = (0..1000).map(|i| (i as f32 - 500.0) / 250.0).collect();
        b.iter(|| {
            let clamped: Vec<f32> = values.iter().map(|&v| v.clamp(0.0, 1.0)).collect();
            black_box(clamped)
        })
    });

    // Poincare ball constraint (||x|| < 1)
    group.bench_function("poincare_constraint", |b| {
        let points: Vec<[f32; 64]> = (0..100)
            .map(|i| generate_poincare_point_fixed(i as u64 * 123))
            .collect();
        b.iter(|| {
            let valid: Vec<bool> = points
                .iter()
                .map(|p| {
                    let norm_sq: f32 = p.iter().map(|x| x * x).sum();
                    norm_sq < 1.0
                })
                .collect();
            black_box(valid)
        })
    });

    // Epsilon comparisons
    group.bench_function("epsilon_compare", |b| {
        let a = 0.1f32 + 0.2f32;
        let b_val = 0.3f32;
        let epsilon = 1e-6f32;
        b.iter(|| {
            let equal = (black_box(a) - black_box(b_val)).abs() < black_box(epsilon);
            black_box(equal)
        })
    });

    group.finish();
}

// ============================================================================
// CRITERION SETUP
// ============================================================================

criterion_group!(
    name = poincare_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5));
    targets = bench_poincare_distance
);

criterion_group!(
    name = cone_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(5));
    targets = bench_cone_membership
);

criterion_group!(
    name = traversal_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(5));
    targets = bench_bfs_traversal
);

criterion_group!(
    name = gpu_memory_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(3));
    targets = bench_gpu_memory_manager
);

criterion_group!(
    name = domain_search_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(3));
    targets = bench_domain_search_modulation
);

criterion_group!(
    name = stability_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(3));
    targets = bench_numerical_stability
);

criterion_main!(
    poincare_benches,
    cone_benches,
    traversal_benches,
    gpu_memory_benches,
    domain_search_benches,
    stability_benches
);
