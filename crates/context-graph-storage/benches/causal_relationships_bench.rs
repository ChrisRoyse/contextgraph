//! Benchmarks for CausalRelationship storage operations.
//!
//! Performance validation for the LLM-Generated Causal Relationship Storage system.
//!
//! # Targets (per plan)
//!
//! | Operation | Target |
//! |-----------|--------|
//! | store_causal_relationship | < 1ms |
//! | get_causal_relationship | < 100μs |
//! | get_by_source (10 rels) | < 1ms |
//! | search (100 rels) | < 10ms |
//! | search (1000 rels) | < 100ms |
//! | search (5000 rels) | < 500ms |
//!
//! # Usage
//!
//! ```bash
//! cargo bench -p context-graph-storage --bench causal_relationships_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tempfile::TempDir;
use uuid::Uuid;

use context_graph_core::types::CausalRelationship;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// ============================================================================
// CONSTANTS - E1 embedding dimension per constitution (1024D)
// ============================================================================

/// E1 embedding dimension per constitution.yaml
const E1_DIM: usize = 1024;

// ============================================================================
// TEST DATA GENERATION (Real data, no mocks)
// ============================================================================

/// Generate a random normalized E1 embedding (1024D).
/// Uses seeded RNG for reproducibility.
fn generate_e1_embedding(rng: &mut StdRng) -> Vec<f32> {
    let mut embedding: Vec<f32> = (0..E1_DIM).map(|_| rng.gen::<f32>() - 0.5).collect();

    // Normalize to unit length (cosine similarity requirement)
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }

    embedding
}

/// Sample causal relationship descriptions for benchmark diversity.
const SAMPLE_DESCRIPTIONS: &[&str] = &[
    "This causal relationship describes how chronic stress leads to elevated cortisol levels. \
     The mechanism involves sustained activation of the HPA axis, which over time causes \
     hippocampal neurodegeneration. This has implications for understanding cognitive decline \
     in high-stress populations.",
    "Database connection pooling directly affects query latency under high load. When the pool \
     is exhausted, new requests must wait for connections to become available, creating a \
     bottleneck. This relationship is critical for capacity planning in production systems.",
    "Memory leaks in the authentication service caused cascading failures across dependent \
     microservices. The root cause was unclosed database connections in error paths, which \
     accumulated over time until OOM killer terminated the process.",
    "Improper mutex usage led to a race condition that corrupted shared state. Multiple threads \
     accessed the configuration map without synchronization, resulting in partial writes that \
     crashed downstream consumers.",
    "Network partition between data centers triggered split-brain in the consensus protocol. \
     The lack of quorum detection allowed both partitions to accept writes independently, \
     requiring manual reconciliation after connectivity was restored.",
];

/// Sample source content for benchmark diversity.
const SAMPLE_SOURCES: &[&str] = &[
    "Studies show that prolonged exposure to cortisol causes damage to hippocampal neurons. \
     This leads to memory impairment and cognitive decline over extended periods.",
    "When database connection pool is exhausted, the application experiences significant \
     latency spikes because new requests wait in queue for available connections.",
    "The service crashed with OOM after running for 72 hours. Investigation revealed \
     database connections were not being closed in the error handling path.",
    "Thread A and Thread B both accessed config_map without holding the mutex, causing \
     intermittent corruption that was difficult to reproduce.",
    "After the network split, both clusters accepted writes independently, resulting in \
     conflicting data that had to be manually merged when connectivity restored.",
];

/// Generate a realistic causal relationship description.
fn generate_description(index: usize) -> String {
    SAMPLE_DESCRIPTIONS[index % SAMPLE_DESCRIPTIONS.len()].to_string()
}

/// Generate source content that would have triggered causal detection.
fn generate_source_content(index: usize) -> String {
    SAMPLE_SOURCES[index % SAMPLE_SOURCES.len()].to_string()
}

/// Direction values for test relationship rotation.
const DIRECTIONS: &[&str] = &["cause", "effect", "bidirectional"];

/// Standard causal cues for test relationships.
const CAUSAL_CUES: &[&str] = &["causes", "leads to", "results in"];

/// Create a realistic test CausalRelationship with real data.
fn create_test_relationship(rng: &mut StdRng, source_id: Uuid, index: usize) -> CausalRelationship {
    CausalRelationship::new(
        generate_description(index),
        generate_e1_embedding(rng),
        generate_source_content(index),
        source_id,
        DIRECTIONS[index % DIRECTIONS.len()].to_string(),
        0.75 + (index % 25) as f32 * 0.01, // Varied confidence 0.75-0.99
        CAUSAL_CUES.iter().map(|s| s.to_string()).collect(),
    )
}

// ============================================================================
// BENCHMARKS
// ============================================================================

/// Benchmark: Store a single causal relationship.
/// Target: < 1ms per operation.
fn bench_store_causal_relationship(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);
    let store_count = std::sync::atomic::AtomicUsize::new(0);

    c.bench_function("causal/store_relationship", |b| {
        b.iter(|| {
            let source_id = Uuid::new_v4();
            let rel = create_test_relationship(&mut rng, source_id, 0);
            let id = rt.block_on(async {
                store
                    .store_causal_relationship(black_box(&rel))
                    .await
                    .expect("BENCH ERROR: store_causal_relationship failed")
            });
            store_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            id
        })
    });

    // Verify: Check final count to ensure stores actually happened (only if benchmark ran)
    let benched = store_count.load(std::sync::atomic::Ordering::Relaxed);
    if benched > 0 {
        let final_count = rt.block_on(async { store.count_causal_relationships().await.unwrap() });
        println!("VERIFICATION: Stored {} causal relationships", final_count);
        assert!(final_count > 0, "VERIFICATION FAILED: No relationships were stored");
    }
}

/// Benchmark: Retrieve a causal relationship by ID.
/// Target: < 100μs per operation.
fn bench_get_causal_relationship(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-store a relationship to retrieve
    let source_id = Uuid::new_v4();
    let rel = create_test_relationship(&mut rng, source_id, 0);
    let stored_id = rt
        .block_on(async { store.store_causal_relationship(&rel).await })
        .expect("BENCH ERROR: Failed to pre-store relationship");

    c.bench_function("causal/get_relationship", |b| {
        b.iter(|| {
            rt.block_on(async {
                store
                    .get_causal_relationship(black_box(stored_id))
                    .await
                    .expect("BENCH ERROR: get_causal_relationship failed")
                    .expect("BENCH ERROR: Relationship not found")
            })
        })
    });

    // Verify: The retrieved relationship has correct ID
    let retrieved = rt
        .block_on(async { store.get_causal_relationship(stored_id).await })
        .expect("VERIFICATION FAILED: get_causal_relationship failed")
        .expect("VERIFICATION FAILED: Relationship not found");
    assert_eq!(retrieved.id, stored_id, "VERIFICATION FAILED: ID mismatch");
    println!("VERIFICATION: Retrieved relationship {} correctly", stored_id);
}

/// Benchmark: Retrieve all causal relationships for a source fingerprint.
/// Uses secondary index for efficient lookup.
/// Target: < 1ms for 10 relationships.
fn bench_get_by_source(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    let source_id = Uuid::new_v4();

    // Pre-store 10 relationships with the same source
    for i in 0..10 {
        let rel = create_test_relationship(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    c.bench_function("causal/get_by_source_10", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .get_causal_relationships_by_source(black_box(source_id))
                    .await
                    .expect("BENCH ERROR: get_causal_relationships_by_source failed");
                black_box(results)
            })
        })
    });

    // Verify: Correct count returned
    let results = rt
        .block_on(async { store.get_causal_relationships_by_source(source_id).await })
        .expect("VERIFICATION FAILED: get_causal_relationships_by_source failed");
    assert_eq!(
        results.len(),
        10,
        "VERIFICATION FAILED: Expected 10 relationships, got {}",
        results.len()
    );
    println!(
        "VERIFICATION: Retrieved {} relationships for source {}",
        results.len(),
        source_id
    );
}

/// Benchmark: Search causal relationships by embedding similarity.
/// Tests scaling behavior across different collection sizes.
/// Uses brute-force scan (suitable for <10K relationships per design).
fn bench_search_scaling(c: &mut Criterion) {
    let tiers = [100, 500, 1000, 5000];

    let mut group = c.benchmark_group("causal/search_scaling");
    group.sample_size(20); // Reduce samples for larger tiers

    for tier_size in tiers {
        let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
        let store = RocksDbTeleologicalStore::open(temp_dir.path())
            .expect("BENCH ERROR: Failed to open RocksDB store");

        let rt =
            tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
        let mut rng = StdRng::seed_from_u64(42);

        // Pre-populate with relationships
        println!("Populating {} causal relationships for tier {}...", tier_size, tier_size);
        for i in 0..tier_size {
            let source_id = Uuid::new_v4();
            let rel = create_test_relationship(&mut rng, source_id, i);
            rt.block_on(async { store.store_causal_relationship(&rel).await })
                .expect("BENCH ERROR: Failed to pre-store relationship");
        }

        // Verify count before benchmark
        let count = rt
            .block_on(async { store.count_causal_relationships().await })
            .expect("VERIFICATION FAILED: count_causal_relationships failed");
        assert_eq!(
            count, tier_size,
            "VERIFICATION FAILED: Expected {} relationships, got {}",
            tier_size, count
        );
        println!("VERIFICATION: Populated {} relationships", count);

        // Generate query embedding
        let query_embedding = generate_e1_embedding(&mut rng);

        group.throughput(Throughput::Elements(tier_size as u64));
        group.bench_with_input(
            BenchmarkId::new("brute_force", tier_size),
            &tier_size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let results = store
                            .search_causal_relationships(black_box(&query_embedding), 10, None)
                            .await
                            .expect("BENCH ERROR: search_causal_relationships failed");
                        black_box(results)
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Search with direction filter.
/// Tests filtering by "cause", "effect", or "bidirectional".
fn bench_search_with_filter(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships (mixed directions)
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let query_embedding = generate_e1_embedding(&mut rng);

    let mut group = c.benchmark_group("causal/search_filtered");

    // Benchmark unfiltered search
    group.bench_function("no_filter", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_relationships(black_box(&query_embedding), 10, None)
                    .await
                    .expect("BENCH ERROR: search failed");
                black_box(results)
            })
        })
    });

    // Benchmark filtered by "cause"
    group.bench_function("filter_cause", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_relationships(
                        black_box(&query_embedding),
                        10,
                        Some(black_box("cause")),
                    )
                    .await
                    .expect("BENCH ERROR: search failed");
                black_box(results)
            })
        })
    });

    // Benchmark filtered by "effect"
    group.bench_function("filter_effect", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_relationships(
                        black_box(&query_embedding),
                        10,
                        Some(black_box("effect")),
                    )
                    .await
                    .expect("BENCH ERROR: search failed");
                black_box(results)
            })
        })
    });

    group.finish();

    // Verify: Search returns results
    let results = rt
        .block_on(async { store.search_causal_relationships(&query_embedding, 10, None).await })
        .expect("VERIFICATION FAILED: search failed");
    assert!(!results.is_empty(), "VERIFICATION FAILED: No search results returned");
    println!(
        "VERIFICATION: Search returned {} results, top similarity: {:.4}",
        results.len(),
        results[0].1
    );
}

/// Benchmark: Delete causal relationship.
/// Tests primary delete + secondary index update.
///
/// Uses iter_batched to ensure each iteration has a fresh relationship to delete,
/// avoiding complex state management within the benchmark closure.
fn bench_delete_causal_relationship(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let rng = std::sync::Mutex::new(StdRng::seed_from_u64(42));
    let delete_count = std::sync::atomic::AtomicUsize::new(0);

    c.bench_function("causal/delete_relationship", |b| {
        b.iter_batched(
            || {
                // Setup: create a relationship to delete
                let mut rng = rng.lock().unwrap();
                let source_id = Uuid::new_v4();
                let rel = create_test_relationship(&mut rng, source_id, 0);
                let id = rt
                    .block_on(async { store.store_causal_relationship(&rel).await })
                    .expect("BENCH ERROR: Failed to store relationship for delete");
                id
            },
            |id_to_delete| {
                // Benchmark: delete the relationship
                rt.block_on(async {
                    store
                        .delete_causal_relationship(black_box(id_to_delete))
                        .await
                        .expect("BENCH ERROR: delete_causal_relationship failed")
                });
                delete_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            },
            criterion::BatchSize::SmallInput,
        )
    });

    let total = delete_count.load(std::sync::atomic::Ordering::Relaxed);
    println!("VERIFICATION: Deleted {} relationships in benchmark", total);
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_store_causal_relationship,
    bench_get_causal_relationship,
    bench_get_by_source,
    bench_search_scaling,
    bench_search_with_filter,
    bench_delete_causal_relationship,
);
criterion_main!(benches);
