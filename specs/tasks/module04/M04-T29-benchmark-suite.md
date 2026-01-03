---
id: "M04-T29"
title: "Create Performance Benchmark Suite"
description: |
  Implement comprehensive criterion-based benchmarks validating all NFR targets:
  - FAISS semantic search <2ms for 1M vectors
  - Poincare distance batch <1ms for 1K pairs
  - Cone membership batch <2ms for 1K checks
  - BFS traversal <5ms for depth 5
  - Domain-aware re-ranking <1ms overhead

  RTX 5090 Compute Capability 12.0 optimizations with Green Contexts.
  NO MOCK DATA - all benchmarks use real GPU operations.
layer: "surface"
status: "pending"
priority: "high"
estimated_hours: 6
sequence: 36
depends_on:
  - "M04-T25"
  - "M04-T28"
spec_refs:
  - "TECH-GRAPH-004 Section 7"
  - "REQ-KG-NFR-001"
  - "REQ-KG-NFR-002"
  - "REQ-KG-NFR-003"
files_to_create:
  - path: "crates/context-graph-graph/benches/graph_benchmarks.rs"
    description: "Criterion benchmark suite for all graph operations"
  - path: "crates/context-graph-graph/benches/README.md"
    description: "Benchmark documentation with baseline targets"
files_to_modify:
  - path: "crates/context-graph-graph/Cargo.toml"
    description: "Add criterion dev-dependency and [[bench]] targets"
test_file: "N/A - benchmarks are validation"
---

## Context

All graph operations have strict NFR targets defined in TECH-GRAPH-004. This task creates a comprehensive benchmark suite using criterion to validate performance against these targets. Benchmarks MUST use real GPU operations on RTX 5090 - NO MOCK DATA.

### NFR Target Summary

| Operation | Target | Measurement |
|-----------|--------|-------------|
| FAISS semantic_search | <2ms | 1M vectors, k=100 |
| Poincare batch distance | <1ms | 1K x 1K matrix |
| Cone batch membership | <2ms | 1K cones x 1K points |
| BFS traversal | <5ms | depth=5, branching=10 |
| Domain re-ranking | <1ms overhead | 1K candidates |
| Entailment query | <3ms | 1K hierarchy depth |
| Contradiction detection | <5ms | Including semantic check |

### RTX 5090 Specifications

- Compute Capability: 12.0
- VRAM: 24GB GDDR7
- Memory Bandwidth: 1.8 TB/s
- CUDA Cores: 21,760
- Green Contexts for kernel scheduling

## Scope

### In Scope
- Criterion benchmark suite with statistical analysis
- All NFR target validations
- GPU warm-up and cold-start measurements
- Memory throughput benchmarks
- Comparative benchmarks (GPU vs CPU fallback)
- CI integration with baseline tracking

### Out of Scope
- Stress testing (separate task)
- Multi-GPU benchmarks
- Network latency testing
- End-to-end API benchmarks

## Definition of Done

### Cargo.toml Updates

```toml
# In crates/context-graph-graph/Cargo.toml

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports", "async_tokio"] }
rand = "0.8"
tempfile = "3.10"

[[bench]]
name = "graph_benchmarks"
harness = false

[[bench]]
name = "cuda_benchmarks"
harness = false
required-features = ["cuda"]
```

### Signatures

```rust
// In crates/context-graph-graph/benches/graph_benchmarks.rs

use criterion::{
    black_box, criterion_group, criterion_main,
    BenchmarkId, Criterion, Throughput,
};
use context_graph_graph::{
    storage::GraphStorage,
    query::{semantic_search, domain_aware_search, entailment_query, contradiction_detect},
    hyperbolic::{PoincareBall, PoincarePoint},
    entailment::EntailmentCone,
    traversal::bfs_traverse,
};
use context_graph_cuda::{
    kernels::{batch_poincare_distance, batch_cone_membership},
    CudaContext,
};

/// Benchmark configuration matching NFR targets
pub struct BenchConfig {
    /// Number of vectors in FAISS index
    pub faiss_vector_count: usize,
    /// Dimension of vectors
    pub vector_dim: usize,
    /// Number of results to retrieve
    pub top_k: usize,
    /// Batch size for GPU operations
    pub batch_size: usize,
    /// BFS traversal depth
    pub traversal_depth: usize,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            faiss_vector_count: 1_000_000,
            vector_dim: 1536,
            top_k: 100,
            batch_size: 1024,
            traversal_depth: 5,
        }
    }
}

/// Setup benchmark environment with real GPU resources
///
/// Creates:
/// - FAISS GPU index with specified vector count
/// - Graph storage with realistic node/edge distribution
/// - CUDA context for kernel benchmarks
///
/// # Returns
/// Tuple of (GraphStorage, CudaContext, test vectors)
fn setup_benchmark_env(config: &BenchConfig) -> (GraphStorage, CudaContext, Vec<[f32; 1536]>) {
    // Initialize CUDA context with RTX 5090 optimizations
    let cuda_ctx = CudaContext::new()
        .with_compute_capability(12, 0)
        .with_green_contexts(true)
        .build()
        .expect("RTX 5090 required for benchmarks");

    // Create graph storage with FAISS GPU index
    let storage = GraphStorage::builder()
        .with_faiss_config(FaissConfig {
            index_type: "IVF16384,PQ64x8".to_string(),
            nprobe: 128,
            use_gpu: true,
            gpu_id: 0,
        })
        .build()
        .expect("Failed to create graph storage");

    // Generate test vectors (NOT mock - real random vectors)
    let mut rng = rand::thread_rng();
    let vectors: Vec<[f32; 1536]> = (0..config.faiss_vector_count)
        .map(|_| {
            let mut v = [0.0f32; 1536];
            for x in &mut v {
                *x = rng.gen_range(-1.0..1.0);
            }
            // Normalize
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            for x in &mut v {
                *x /= norm;
            }
            v
        })
        .collect();

    // Insert vectors into FAISS index
    for (i, vec) in vectors.iter().enumerate() {
        storage.add_embedding(i as u64, vec).expect("Insert failed");
    }

    // Force index training/building
    storage.faiss_index().train_if_needed().expect("Training failed");

    (storage, cuda_ctx, vectors)
}

// ============================================================================
// FAISS SEMANTIC SEARCH BENCHMARKS
// Target: <2ms for 1M vectors, k=100
// ============================================================================

fn bench_faiss_semantic_search(c: &mut Criterion) {
    let config = BenchConfig::default();
    let (storage, _cuda_ctx, vectors) = setup_benchmark_env(&config);

    let mut group = c.benchmark_group("faiss_semantic_search");
    group.throughput(Throughput::Elements(config.top_k as u64));

    // Warm-up GPU
    for _ in 0..10 {
        let query = &vectors[0];
        let _ = storage.semantic_search(query, config.top_k, None);
    }

    group.bench_function(
        BenchmarkId::new("1M_vectors_k100", config.faiss_vector_count),
        |b| {
            let query = &vectors[rand::random::<usize>() % vectors.len()];
            b.iter(|| {
                black_box(storage.semantic_search(
                    black_box(query),
                    black_box(config.top_k),
                    None,
                ))
            })
        },
    );

    // Varying k values
    for k in [10, 50, 100, 500, 1000] {
        group.bench_function(
            BenchmarkId::new("1M_vectors", k),
            |b| {
                let query = &vectors[rand::random::<usize>() % vectors.len()];
                b.iter(|| {
                    black_box(storage.semantic_search(
                        black_box(query),
                        black_box(k),
                        None,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn bench_faiss_batch_search(c: &mut Criterion) {
    let config = BenchConfig::default();
    let (storage, _cuda_ctx, vectors) = setup_benchmark_env(&config);

    let mut group = c.benchmark_group("faiss_batch_search");

    for batch_size in [1, 10, 100, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64 * config.top_k as u64));

        let queries: Vec<_> = (0..batch_size)
            .map(|i| vectors[i % vectors.len()].clone())
            .collect();

        group.bench_function(
            BenchmarkId::new("batch", batch_size),
            |b| {
                b.iter(|| {
                    black_box(storage.batch_semantic_search(
                        black_box(&queries),
                        black_box(config.top_k),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// POINCARE DISTANCE BENCHMARKS
// Target: <1ms for 1K x 1K matrix
// ============================================================================

fn bench_poincare_distance_cpu(c: &mut Criterion) {
    let config = HyperbolicConfig::default();
    let ball = PoincareBall::new(&config);

    // Generate test points inside ball
    let points: Vec<PoincarePoint> = (0..1024)
        .map(|_| {
            let mut coords = [0.0f32; 64];
            for c in &mut coords {
                *c = rand::random::<f32>() * 0.8 - 0.4; // Stay inside ball
            }
            PoincarePoint::from_coords(&coords)
        })
        .collect();

    let mut group = c.benchmark_group("poincare_distance_cpu");
    group.throughput(Throughput::Elements(1024 * 1024));

    group.bench_function("1K_x_1K_matrix", |b| {
        b.iter(|| {
            let mut distances = Vec::with_capacity(1024 * 1024);
            for p1 in &points {
                for p2 in &points {
                    distances.push(ball.distance(black_box(p1), black_box(p2)));
                }
            }
            black_box(distances)
        })
    });

    group.finish();
}

fn bench_poincare_distance_gpu(c: &mut Criterion) {
    let cuda_ctx = CudaContext::new()
        .with_compute_capability(12, 0)
        .build()
        .expect("RTX 5090 required");

    // Generate test points
    let points: Vec<[f32; 64]> = (0..1024)
        .map(|_| {
            let mut coords = [0.0f32; 64];
            for c in &mut coords {
                *c = rand::random::<f32>() * 0.8 - 0.4;
            }
            coords
        })
        .collect();

    let mut group = c.benchmark_group("poincare_distance_gpu");
    group.throughput(Throughput::Elements(1024 * 1024));

    // Warm-up
    for _ in 0..5 {
        let _ = batch_poincare_distance(&cuda_ctx, &points, &points, -1.0);
    }

    group.bench_function("1K_x_1K_matrix_cuda", |b| {
        b.iter(|| {
            black_box(batch_poincare_distance(
                &cuda_ctx,
                black_box(&points),
                black_box(&points),
                -1.0,
            ))
        })
    });

    // Varying batch sizes
    for size in [256, 512, 1024, 2048] {
        let subset: Vec<_> = points.iter().take(size).cloned().collect();
        group.bench_function(
            BenchmarkId::new("cuda", format!("{}x{}", size, size)),
            |b| {
                b.iter(|| {
                    black_box(batch_poincare_distance(
                        &cuda_ctx,
                        black_box(&subset),
                        black_box(&subset),
                        -1.0,
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// CONE MEMBERSHIP BENCHMARKS
// Target: <2ms for 1K cones x 1K points
// ============================================================================

fn bench_cone_membership_cpu(c: &mut Criterion) {
    let config = HyperbolicConfig::default();
    let ball = PoincareBall::new(&config);

    // Generate cones
    let cones: Vec<EntailmentCone> = (0..1024)
        .map(|i| {
            let mut apex_coords = [0.0f32; 64];
            apex_coords[0] = (i as f32 / 1024.0) * 0.5;
            let apex = PoincarePoint::from_coords(&apex_coords);
            EntailmentCone::new(apex, 0.5, 1.0, i as u64)
        })
        .collect();

    // Generate points
    let points: Vec<PoincarePoint> = (0..1024)
        .map(|_| {
            let mut coords = [0.0f32; 64];
            for c in &mut coords {
                *c = rand::random::<f32>() * 0.8 - 0.4;
            }
            PoincarePoint::from_coords(&coords)
        })
        .collect();

    let mut group = c.benchmark_group("cone_membership_cpu");
    group.throughput(Throughput::Elements(1024 * 1024));

    group.bench_function("1K_cones_x_1K_points", |b| {
        b.iter(|| {
            let mut scores = Vec::with_capacity(1024 * 1024);
            for cone in &cones {
                for point in &points {
                    scores.push(cone.membership_score(black_box(point), &ball));
                }
            }
            black_box(scores)
        })
    });

    group.finish();
}

fn bench_cone_membership_gpu(c: &mut Criterion) {
    let cuda_ctx = CudaContext::new()
        .with_compute_capability(12, 0)
        .build()
        .expect("RTX 5090 required");

    // Generate cone data (apex coords + aperture)
    let cone_data: Vec<([f32; 64], f32)> = (0..1024)
        .map(|i| {
            let mut apex = [0.0f32; 64];
            apex[0] = (i as f32 / 1024.0) * 0.5;
            (apex, 0.5f32)
        })
        .collect();

    // Generate points
    let points: Vec<[f32; 64]> = (0..1024)
        .map(|_| {
            let mut coords = [0.0f32; 64];
            for c in &mut coords {
                *c = rand::random::<f32>() * 0.8 - 0.4;
            }
            coords
        })
        .collect();

    let mut group = c.benchmark_group("cone_membership_gpu");
    group.throughput(Throughput::Elements(1024 * 1024));

    // Warm-up
    for _ in 0..5 {
        let _ = batch_cone_membership(&cuda_ctx, &cone_data, &points);
    }

    group.bench_function("1K_cones_x_1K_points_cuda", |b| {
        b.iter(|| {
            black_box(batch_cone_membership(
                &cuda_ctx,
                black_box(&cone_data),
                black_box(&points),
            ))
        })
    });

    group.finish();
}

// ============================================================================
// BFS TRAVERSAL BENCHMARKS
// Target: <5ms for depth 5, branching factor 10
// ============================================================================

fn bench_bfs_traversal(c: &mut Criterion) {
    // Build a graph with known structure
    let storage = GraphStorage::builder()
        .with_faiss_config(FaissConfig::default())
        .build()
        .expect("Storage creation failed");

    // Create nodes with branching factor 10, depth 5
    // Total nodes: 1 + 10 + 100 + 1000 + 10000 + 100000 = 111,111
    let mut node_id = 0u64;
    let root_id = node_id;
    node_id += 1;

    let mut current_level = vec![root_id];
    for _depth in 0..5 {
        let mut next_level = Vec::new();
        for parent in &current_level {
            for _ in 0..10 {
                // Create child node
                let child_id = node_id;
                node_id += 1;

                // Add edge
                storage.add_edge(*parent, child_id, EdgeType::SUBSUMES, 1.0)
                    .expect("Edge creation failed");

                next_level.push(child_id);
            }
        }
        current_level = next_level;
    }

    let mut group = c.benchmark_group("bfs_traversal");

    for depth in [1, 2, 3, 4, 5] {
        group.bench_function(
            BenchmarkId::new("branching_10", format!("depth_{}", depth)),
            |b| {
                b.iter(|| {
                    black_box(bfs_traverse(
                        &storage,
                        black_box(root_id),
                        black_box(depth),
                        None,
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// DOMAIN-AWARE SEARCH BENCHMARKS
// Target: <1ms overhead over base semantic search
// ============================================================================

fn bench_domain_aware_search(c: &mut Criterion) {
    let config = BenchConfig::default();
    let (storage, _cuda_ctx, vectors) = setup_benchmark_env(&config);

    // Setup neurotransmitter state
    let nt_state = NTState {
        excitatory: 0.8,
        inhibitory: 0.2,
        modulatory: 0.5,
    };

    let domain_context = DomainContext {
        domain: DomainType::Code,
        nt_state,
        steering_factor: 1.0,
    };

    let mut group = c.benchmark_group("domain_aware_search");

    // Baseline: semantic search without domain awareness
    group.bench_function("baseline_semantic", |b| {
        let query = &vectors[0];
        b.iter(|| {
            black_box(storage.semantic_search(
                black_box(query),
                100,
                None,
            ))
        })
    });

    // With domain awareness
    group.bench_function("domain_aware_code", |b| {
        let query = &vectors[0];
        b.iter(|| {
            black_box(domain_aware_search(
                &storage,
                black_box(query),
                100,
                black_box(&domain_context),
            ))
        })
    });

    // Different domains
    for domain in [DomainType::Legal, DomainType::Medical, DomainType::Research] {
        let ctx = DomainContext {
            domain,
            nt_state: nt_state.clone(),
            steering_factor: 1.0,
        };

        group.bench_function(
            BenchmarkId::new("domain", format!("{:?}", domain)),
            |b| {
                let query = &vectors[0];
                b.iter(|| {
                    black_box(domain_aware_search(
                        &storage,
                        black_box(query),
                        100,
                        black_box(&ctx),
                    ))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// ENTAILMENT QUERY BENCHMARKS
// Target: <3ms for 1K hierarchy depth
// ============================================================================

fn bench_entailment_query(c: &mut Criterion) {
    let storage = GraphStorage::builder()
        .with_faiss_config(FaissConfig::default())
        .build()
        .expect("Storage creation failed");

    // Build deep hierarchy (1K levels)
    let mut prev_id = 0u64;
    for i in 1..1000 {
        let curr_id = i;
        // Create entailment cone for each node
        let apex = PoincarePoint::from_coords(&[i as f32 * 0.0005; 64]);
        storage.add_entailment_cone(curr_id, apex, 0.5, 1.0)
            .expect("Cone creation failed");

        // Link to parent
        storage.add_edge(curr_id, prev_id, EdgeType::ISA, 1.0)
            .expect("Edge creation failed");

        prev_id = curr_id;
    }

    let ball = PoincareBall::new(&HyperbolicConfig::default());

    let mut group = c.benchmark_group("entailment_query");

    // Query ancestors from deepest node
    group.bench_function("ancestors_1K_depth", |b| {
        b.iter(|| {
            black_box(entailment_query(
                &storage,
                &ball,
                black_box(999),
                EntailmentDirection::Ancestors,
                None,
            ))
        })
    });

    // Query descendants from root
    group.bench_function("descendants_1K_depth", |b| {
        b.iter(|| {
            black_box(entailment_query(
                &storage,
                &ball,
                black_box(0),
                EntailmentDirection::Descendants,
                None,
            ))
        })
    });

    group.finish();
}

// ============================================================================
// CONTRADICTION DETECTION BENCHMARKS
// Target: <5ms including semantic check
// ============================================================================

fn bench_contradiction_detection(c: &mut Criterion) {
    let config = BenchConfig {
        faiss_vector_count: 100_000, // Smaller for contradiction test
        ..Default::default()
    };
    let (storage, _cuda_ctx, vectors) = setup_benchmark_env(&config);

    // Add some explicit contradiction edges
    for i in 0..1000 {
        storage.add_edge(i, i + 1000, EdgeType::CONTRADICTS, 0.9)
            .expect("Edge creation failed");
    }

    let ball = PoincareBall::new(&HyperbolicConfig::default());

    let mut group = c.benchmark_group("contradiction_detection");

    group.bench_function("detect_with_semantic", |b| {
        b.iter(|| {
            black_box(contradiction_detect(
                &storage,
                &ball,
                black_box(&vectors[0]),
                black_box(100),
                0.9,
            ))
        })
    });

    group.bench_function("explicit_edges_only", |b| {
        b.iter(|| {
            black_box(storage.get_contradicting_edges(black_box(500)))
        })
    });

    group.finish();
}

// ============================================================================
// MEMORY THROUGHPUT BENCHMARKS
// ============================================================================

fn bench_memory_throughput(c: &mut Criterion) {
    let cuda_ctx = CudaContext::new()
        .with_compute_capability(12, 0)
        .build()
        .expect("RTX 5090 required");

    let mut group = c.benchmark_group("memory_throughput");

    // Test various data sizes
    for size_mb in [1, 10, 100, 1000] {
        let size_bytes = size_mb * 1024 * 1024;
        let data: Vec<f32> = vec![1.0f32; size_bytes / 4];

        group.throughput(Throughput::Bytes(size_bytes as u64));

        group.bench_function(
            BenchmarkId::new("host_to_device", format!("{}MB", size_mb)),
            |b| {
                b.iter(|| {
                    black_box(cuda_ctx.upload_to_device(black_box(&data)))
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    faiss_benches,
    bench_faiss_semantic_search,
    bench_faiss_batch_search,
);

criterion_group!(
    poincare_benches,
    bench_poincare_distance_cpu,
    bench_poincare_distance_gpu,
);

criterion_group!(
    cone_benches,
    bench_cone_membership_cpu,
    bench_cone_membership_gpu,
);

criterion_group!(
    traversal_benches,
    bench_bfs_traversal,
);

criterion_group!(
    domain_benches,
    bench_domain_aware_search,
);

criterion_group!(
    entailment_benches,
    bench_entailment_query,
);

criterion_group!(
    contradiction_benches,
    bench_contradiction_detection,
);

criterion_group!(
    memory_benches,
    bench_memory_throughput,
);

criterion_main!(
    faiss_benches,
    poincare_benches,
    cone_benches,
    traversal_benches,
    domain_benches,
    entailment_benches,
    contradiction_benches,
    memory_benches,
);
```

### Constraints
- MUST use criterion for statistical rigor
- MUST run on real RTX 5090 GPU (NO MOCK)
- MUST validate against NFR targets
- MUST include warm-up iterations
- MUST report throughput metrics

### Acceptance Criteria
- [ ] All benchmark groups implemented
- [ ] NFR targets validated with pass/fail threshold
- [ ] GPU warm-up included
- [ ] Statistical significance achieved (criterion default)
- [ ] Compiles with `cargo build -p context-graph-graph --release`
- [ ] Benchmarks run with `cargo bench -p context-graph-graph`
- [ ] HTML reports generated

## Implementation Approach

### Benchmark Structure

```
benches/
  graph_benchmarks.rs    # Main benchmark file
  README.md              # Documentation
```

### NFR Validation

Each benchmark group validates against its target:

```rust
// Example: FAISS validation in CI
#[test]
fn validate_faiss_nfr() {
    let start = Instant::now();
    let result = storage.semantic_search(&query, 100, None);
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() < 2,
        "FAISS NFR FAILED: {}ms > 2ms target", elapsed.as_millis());
}
```

### CI Integration

```yaml
# .github/workflows/benchmarks.yml
benchmark:
  runs-on: [self-hosted, gpu, rtx5090]
  steps:
    - uses: actions/checkout@v4
    - name: Run benchmarks
      run: cargo bench -p context-graph-graph -- --save-baseline main
    - name: Compare baseline
      run: cargo bench -p context-graph-graph -- --baseline main
```

## Verification

### Test Commands
```bash
# Build benchmarks
cargo build -p context-graph-graph --release --benches

# Run all benchmarks
cargo bench -p context-graph-graph

# Run specific group
cargo bench -p context-graph-graph faiss

# Run with baseline comparison
cargo bench -p context-graph-graph -- --save-baseline current
cargo bench -p context-graph-graph -- --baseline current

# Generate HTML reports
cargo bench -p context-graph-graph -- --plotting-backend plotters
```

### Expected Output

```
faiss_semantic_search/1M_vectors_k100
                        time:   [1.82 ms 1.85 ms 1.89 ms]  ✓ <2ms
                        thrpt:  [52,910 elem/s 54,054 elem/s 54,945 elem/s]

poincare_distance_gpu/1K_x_1K_matrix_cuda
                        time:   [0.78 ms 0.81 ms 0.84 ms]  ✓ <1ms
                        thrpt:  [1,218,696,844 elem/s ...]

cone_membership_gpu/1K_cones_x_1K_points_cuda
                        time:   [1.52 ms 1.58 ms 1.64 ms]  ✓ <2ms
                        thrpt:  [639,949,878 elem/s ...]

bfs_traversal/branching_10/depth_5
                        time:   [3.91 ms 4.05 ms 4.20 ms]  ✓ <5ms
```

## Manual Verification
- [ ] All benchmarks compile
- [ ] All benchmarks run on RTX 5090
- [ ] All NFR targets met
- [ ] HTML reports readable
- [ ] CI integration functional
