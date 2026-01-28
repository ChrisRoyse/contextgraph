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
//! | search_e5_hybrid | < 150% of search_e5 |
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
// CONSTANTS - Embedding dimensions per constitution
// ============================================================================

/// E1 embedding dimension per constitution.yaml
const E1_DIM: usize = 1024;

/// E5 embedding dimension per constitution.yaml
const E5_DIM: usize = 768;

/// E8 graph embedding dimension per constitution.yaml (1024D after upgrade)
const E8_DIM: usize = 1024;

/// E11 entity embedding dimension (768D KEPLER per constitution.yaml)
const E11_DIM: usize = 768;

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

/// Generate a random normalized E5 embedding (768D).
fn generate_e5_embedding(rng: &mut StdRng) -> Vec<f32> {
    let mut embedding: Vec<f32> = (0..E5_DIM).map(|_| rng.gen::<f32>() - 0.5).collect();

    // Normalize to unit length
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }

    embedding
}

/// Generate a random normalized E8 embedding (1024D).
fn generate_e8_embedding(rng: &mut StdRng) -> Vec<f32> {
    let mut embedding: Vec<f32> = (0..E8_DIM).map(|_| rng.gen::<f32>() - 0.5).collect();

    // Normalize to unit length
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }

    embedding
}

/// Generate a random normalized E11 embedding (768D KEPLER).
fn generate_e11_embedding(rng: &mut StdRng) -> Vec<f32> {
    let mut embedding: Vec<f32> = (0..E11_DIM).map(|_| rng.gen::<f32>() - 0.5).collect();

    // Normalize to unit length
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }

    embedding
}

/// Sample causal relationship explanations for benchmark diversity.
/// These are LLM-style explanations that would normally cluster together.
const SAMPLE_EXPLANATIONS: &[&str] = &[
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
/// These represent unique documents from which relationships were extracted.
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

/// Sample cause statements.
const SAMPLE_CAUSES: &[&str] = &[
    "Chronic stress activates HPA axis",
    "Connection pool exhaustion",
    "Unclosed database connections in error paths",
    "Missing mutex synchronization",
    "Network partition without quorum detection",
];

/// Sample effect statements.
const SAMPLE_EFFECTS: &[&str] = &[
    "Hippocampal neurodegeneration",
    "Query latency bottleneck",
    "Cascading service failures",
    "Shared state corruption",
    "Split-brain data inconsistency",
];

/// Mechanism types for test relationships.
const MECHANISM_TYPES: &[&str] = &["direct", "mediated", "feedback", "temporal"];

/// Create a realistic test CausalRelationship with real data.
fn create_test_relationship(rng: &mut StdRng, source_id: Uuid, index: usize) -> CausalRelationship {
    CausalRelationship::new(
        SAMPLE_CAUSES[index % SAMPLE_CAUSES.len()].to_string(),
        SAMPLE_EFFECTS[index % SAMPLE_EFFECTS.len()].to_string(),
        SAMPLE_EXPLANATIONS[index % SAMPLE_EXPLANATIONS.len()].to_string(),
        generate_e5_embedding(rng), // e5_as_cause
        generate_e5_embedding(rng), // e5_as_effect
        generate_e1_embedding(rng), // e1_semantic
        SAMPLE_SOURCES[index % SAMPLE_SOURCES.len()].to_string(),
        source_id,
        0.75 + (index % 25) as f32 * 0.01, // Varied confidence 0.75-0.99
        MECHANISM_TYPES[index % MECHANISM_TYPES.len()].to_string(),
    )
}

/// Create a test CausalRelationship with source-anchored embeddings for hybrid search testing.
fn create_test_relationship_with_source_embeddings(
    rng: &mut StdRng,
    source_id: Uuid,
    index: usize,
) -> CausalRelationship {
    create_test_relationship(rng, source_id, index)
        .with_source_embeddings(generate_e5_embedding(rng), generate_e5_embedding(rng))
}

/// Create a test CausalRelationship WITH E11 entity embedding.
fn create_test_relationship_with_e11(
    rng: &mut StdRng,
    source_id: Uuid,
    index: usize,
) -> CausalRelationship {
    create_test_relationship(rng, source_id, index).with_entity_embedding(generate_e11_embedding(rng))
}

/// Create a relationship with ALL embeddings (E5 source + E8 graph + E11 entity).
fn create_test_relationship_full(
    rng: &mut StdRng,
    source_id: Uuid,
    index: usize,
) -> CausalRelationship {
    create_test_relationship(rng, source_id, index)
        .with_source_embeddings(generate_e5_embedding(rng), generate_e5_embedding(rng))
        .with_graph_embeddings(generate_e8_embedding(rng), generate_e8_embedding(rng))
        .with_entity_embedding(generate_e11_embedding(rng))
}

// ============================================================================
// BENCHMARKS - CRUD Operations
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

// ============================================================================
// BENCHMARKS - Search Operations
// ============================================================================

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
/// Tests filtering by "direct", "mediated", "feedback", or "temporal".
fn bench_search_with_filter(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships (mixed mechanism types)
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

    // Benchmark filtered by "direct"
    group.bench_function("filter_direct", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_relationships(
                        black_box(&query_embedding),
                        10,
                        Some(black_box("direct")),
                    )
                    .await
                    .expect("BENCH ERROR: search failed");
                black_box(results)
            })
        })
    });

    // Benchmark filtered by "mediated"
    group.bench_function("filter_mediated", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_relationships(
                        black_box(&query_embedding),
                        10,
                        Some(black_box("mediated")),
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

// ============================================================================
// BENCHMARKS - E5 Asymmetric Search
// ============================================================================

/// Benchmark: E5 asymmetric causal search.
/// Compares search_causal_e5 vs search_causal_e5_hybrid.
fn bench_e5_search_comparison(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships WITH source embeddings
    println!("Populating 500 causal relationships with source embeddings...");
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship_with_source_embeddings(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let query_embedding = generate_e5_embedding(&mut rng);

    let mut group = c.benchmark_group("causal/e5_search");

    // Benchmark E5 search (explanation only)
    group.bench_function("e5_causes_only", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_e5(black_box(&query_embedding), true, 10)
                    .await
                    .expect("BENCH ERROR: search_causal_e5 failed");
                black_box(results)
            })
        })
    });

    // Benchmark E5 hybrid search (source + explanation)
    group.bench_function("e5_hybrid_causes", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_e5_hybrid(black_box(&query_embedding), true, 10, 0.6, 0.4)
                    .await
                    .expect("BENCH ERROR: search_causal_e5_hybrid failed");
                black_box(results)
            })
        })
    });

    // Benchmark E5 effects search
    group.bench_function("e5_effects_only", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_e5(black_box(&query_embedding), false, 10)
                    .await
                    .expect("BENCH ERROR: search_causal_e5 failed");
                black_box(results)
            })
        })
    });

    // Benchmark E5 hybrid effects search
    group.bench_function("e5_hybrid_effects", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_e5_hybrid(black_box(&query_embedding), false, 10, 0.6, 0.4)
                    .await
                    .expect("BENCH ERROR: search_causal_e5_hybrid failed");
                black_box(results)
            })
        })
    });

    group.finish();

    // Verify both methods return results
    let e5_results = rt
        .block_on(async { store.search_causal_e5(&query_embedding, true, 10).await })
        .expect("VERIFICATION FAILED: e5 search failed");
    let hybrid_results = rt
        .block_on(async { store.search_causal_e5_hybrid(&query_embedding, true, 10, 0.6, 0.4).await })
        .expect("VERIFICATION FAILED: hybrid search failed");

    println!(
        "VERIFICATION: E5 returned {} results (top: {:.4}), Hybrid returned {} results (top: {:.4})",
        e5_results.len(),
        e5_results.first().map(|r| r.1).unwrap_or(0.0),
        hybrid_results.len(),
        hybrid_results.first().map(|r| r.1).unwrap_or(0.0),
    );
}

/// Benchmark: E5 hybrid search with different weight configurations.
fn bench_hybrid_weight_variations(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships with source embeddings
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship_with_source_embeddings(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let query_embedding = generate_e5_embedding(&mut rng);

    let mut group = c.benchmark_group("causal/hybrid_weights");

    // Test different weight combinations
    let weight_configs = [
        ("source_heavy_0.8_0.2", 0.8, 0.2),
        ("balanced_0.6_0.4", 0.6, 0.4),
        ("equal_0.5_0.5", 0.5, 0.5),
        ("explanation_heavy_0.3_0.7", 0.3, 0.7),
    ];

    for (name, source_w, explanation_w) in weight_configs {
        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let results = store
                        .search_causal_e5_hybrid(
                            black_box(&query_embedding),
                            true,
                            10,
                            source_w,
                            explanation_w,
                        )
                        .await
                        .expect("BENCH ERROR: search_causal_e5_hybrid failed");
                    black_box(results)
                })
            })
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARKS - E11 Entity Search
// ============================================================================

/// Benchmark: E11 entity search performance.
/// Target: < 15ms for 500 relationships (brute force scan)
fn bench_e11_search(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships WITH E11 embeddings
    println!("Populating 500 causal relationships with E11 embeddings...");
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship_with_e11(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let query_embedding = generate_e11_embedding(&mut rng);

    c.bench_function("causal/e11_entity_search_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_e11(black_box(&query_embedding), 10)
                    .await
                    .expect("BENCH ERROR: search_causal_e11 failed");
                black_box(results)
            })
        })
    });

    // Verify: Search returns results
    let results = rt
        .block_on(async { store.search_causal_e11(&query_embedding, 10).await })
        .expect("VERIFICATION FAILED: e11 search failed");
    println!(
        "VERIFICATION: E11 search returned {} results, top similarity: {:.4}",
        results.len(),
        results.first().map(|r| r.1).unwrap_or(0.0),
    );
}

/// Benchmark: E11 search scaling behavior.
fn bench_e11_search_scaling(c: &mut Criterion) {
    let tiers = [100, 500, 1000, 5000];

    let mut group = c.benchmark_group("causal/e11_search_scaling");
    group.sample_size(20);

    for tier_size in tiers {
        let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
        let store = RocksDbTeleologicalStore::open(temp_dir.path())
            .expect("BENCH ERROR: Failed to open RocksDB store");

        let rt =
            tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
        let mut rng = StdRng::seed_from_u64(42);

        println!("Populating {} causal relationships with E11...", tier_size);
        for i in 0..tier_size {
            let source_id = Uuid::new_v4();
            let rel = create_test_relationship_with_e11(&mut rng, source_id, i);
            rt.block_on(async { store.store_causal_relationship(&rel).await })
                .expect("BENCH ERROR: Failed to pre-store relationship");
        }

        let query_embedding = generate_e11_embedding(&mut rng);

        group.throughput(Throughput::Elements(tier_size as u64));
        group.bench_with_input(
            BenchmarkId::new("e11_brute_force", tier_size),
            &tier_size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let results = store
                            .search_causal_e11(black_box(&query_embedding), 10)
                            .await
                            .expect("BENCH ERROR: search_causal_e11 failed");
                        black_box(results)
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: Compare E11 vs E1 search performance.
fn bench_e11_vs_e1_comparison(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships with ALL embeddings
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship_with_e11(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let e1_query = generate_e1_embedding(&mut rng);
    let e11_query = generate_e11_embedding(&mut rng);

    let mut group = c.benchmark_group("causal/e11_vs_e1");

    group.bench_function("e1_semantic_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_relationships(black_box(&e1_query), 10, None)
                    .await
                    .expect("BENCH ERROR: search failed");
                black_box(results)
            })
        })
    });

    group.bench_function("e11_entity_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_e11(black_box(&e11_query), 10)
                    .await
                    .expect("BENCH ERROR: search_causal_e11 failed");
                black_box(results)
            })
        })
    });

    group.finish();
}

// ============================================================================
// BENCHMARKS - Multi-Embedder Fusion with E11
// ============================================================================

/// Benchmark: Full multi-embedder search (E1 + E5 + E8 + E11).
/// Target: < 50ms for 500 relationships
fn bench_multi_embedder_search(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate with 500 relationships with ALL embeddings
    println!("Populating 500 relationships with full embeddings...");
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship_full(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let e1_query = generate_e1_embedding(&mut rng);
    let e5_query = generate_e5_embedding(&mut rng);
    let e8_query = generate_e8_embedding(&mut rng);
    let e11_query = generate_e11_embedding(&mut rng);
    let config = context_graph_core::types::MultiEmbedderConfig::new();

    c.bench_function("causal/multi_embedder_search_500", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = store
                    .search_causal_multi_embedder(
                        black_box(&e1_query),
                        black_box(&e5_query),
                        black_box(&e8_query),
                        black_box(&e11_query),
                        true, // search_causes
                        10,
                        black_box(&config),
                    )
                    .await
                    .expect("BENCH ERROR: multi_embedder search failed");
                black_box(results)
            })
        })
    });

    // Verify: Search returns results
    let results = rt
        .block_on(async {
            store
                .search_causal_multi_embedder(&e1_query, &e5_query, &e8_query, &e11_query, true, 10, &config)
                .await
        })
        .expect("VERIFICATION FAILED: multi_embedder search failed");
    println!(
        "VERIFICATION: Multi-embedder search returned {} results, top RRF score: {:.4}",
        results.len(),
        results.first().map(|r| r.rrf_score).unwrap_or(0.0),
    );
}

/// Benchmark: E11 weight sensitivity analysis.
fn bench_e11_weight_sensitivity(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
    let store = RocksDbTeleologicalStore::open(temp_dir.path())
        .expect("BENCH ERROR: Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Pre-populate
    for i in 0..500 {
        let source_id = Uuid::new_v4();
        let rel = create_test_relationship_full(&mut rng, source_id, i);
        rt.block_on(async { store.store_causal_relationship(&rel).await })
            .expect("BENCH ERROR: Failed to pre-store relationship");
    }

    let e1_query = generate_e1_embedding(&mut rng);
    let e5_query = generate_e5_embedding(&mut rng);
    let e8_query = generate_e8_embedding(&mut rng);
    let e11_query = generate_e11_embedding(&mut rng);

    let mut group = c.benchmark_group("causal/e11_weight_sensitivity");

    let weights = [
        ("e11_0.00", 0.40, 0.40, 0.20, 0.00),
        ("e11_0.10", 0.35, 0.35, 0.20, 0.10),
        ("e11_0.20", 0.30, 0.35, 0.15, 0.20), // default
        ("e11_0.30", 0.25, 0.30, 0.15, 0.30),
    ];

    for (name, e1_w, e5_w, e8_w, e11_w) in weights {
        let config =
            context_graph_core::types::MultiEmbedderConfig::with_weights(e1_w, e5_w, e8_w, e11_w);

        group.bench_function(name, |b| {
            b.iter(|| {
                rt.block_on(async {
                    let results = store
                        .search_causal_multi_embedder(
                            black_box(&e1_query),
                            black_box(&e5_query),
                            black_box(&e8_query),
                            black_box(&e11_query),
                            true,
                            10,
                            black_box(&config),
                        )
                        .await
                        .expect("BENCH ERROR: multi_embedder search failed");
                    black_box(results)
                })
            })
        });
    }

    group.finish();
}

// ============================================================================
// BENCHMARKS - E11 HNSW vs Brute-Force Comparison
// ============================================================================

/// Benchmark: E11 HNSW index vs brute-force search.
/// This benchmark measures the performance improvement from using HNSW indexing.
///
/// # Targets
///
/// | Relationships | Brute Force | HNSW Target | Speedup |
/// |---------------|-------------|-------------|---------|
/// | 100           | ~395 µs     | ~200 µs     | 2x      |
/// | 500           | ~2.05 ms    | ~400 µs     | 5x      |
/// | 1000          | ~4.88 ms    | ~500 µs     | 10x     |
/// | 5000          | ~42.6 ms    | ~1 ms       | 42x     |
fn bench_e11_hnsw_vs_brute_force(c: &mut Criterion) {
    let tiers = [100, 500, 1000, 5000];

    let mut group = c.benchmark_group("causal/e11_hnsw_comparison");
    group.sample_size(20);

    for tier_size in tiers {
        let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
        let store = RocksDbTeleologicalStore::open(temp_dir.path())
            .expect("BENCH ERROR: Failed to open RocksDB store");

        let rt =
            tokio::runtime::Runtime::new().expect("BENCH ERROR: Failed to create tokio runtime");
        let mut rng = StdRng::seed_from_u64(42);

        // Pre-populate with relationships WITH E11 embeddings
        println!("Populating {} causal relationships with E11 for HNSW comparison...", tier_size);
        for i in 0..tier_size {
            let source_id = Uuid::new_v4();
            let rel = create_test_relationship_with_e11(&mut rng, source_id, i);
            rt.block_on(async { store.store_causal_relationship(&rel).await })
                .expect("BENCH ERROR: Failed to pre-store relationship");
        }

        // Verify count
        let count = rt
            .block_on(async { store.count_causal_relationships().await })
            .expect("VERIFICATION FAILED: count_causal_relationships failed");
        assert_eq!(count, tier_size, "VERIFICATION FAILED: Count mismatch");

        let query_embedding = generate_e11_embedding(&mut rng);

        group.throughput(Throughput::Elements(tier_size as u64));

        // Benchmark HNSW search (via search_causal_e11 which now uses HNSW)
        group.bench_with_input(
            BenchmarkId::new("hnsw", tier_size),
            &tier_size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let results = store
                            .search_causal_e11(black_box(&query_embedding), 10)
                            .await
                            .expect("BENCH ERROR: search_causal_e11 failed");
                        black_box(results)
                    })
                })
            },
        );

        // Benchmark brute-force search
        group.bench_with_input(
            BenchmarkId::new("brute_force", tier_size),
            &tier_size,
            |b, _| {
                b.iter(|| {
                    rt.block_on(async {
                        let results = store
                            .search_causal_e11_brute_force(black_box(&query_embedding), 10)
                            .await
                            .expect("BENCH ERROR: search_causal_e11_brute_force failed");
                        black_box(results)
                    })
                })
            },
        );
    }

    group.finish();
}

/// Benchmark: E11 HNSW index rebuild time on startup.
/// Measures how long it takes to rebuild the index from existing relationships.
fn bench_e11_hnsw_rebuild(c: &mut Criterion) {
    let tiers = [100, 500, 1000, 5000];

    let mut group = c.benchmark_group("causal/e11_hnsw_rebuild");
    group.sample_size(10);

    for tier_size in tiers {
        group.bench_with_input(
            BenchmarkId::new("rebuild", tier_size),
            &tier_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        // Setup: Create a store with relationships, close it
                        let temp_dir = TempDir::new().expect("BENCH ERROR: Failed to create temp directory");
                        let path = temp_dir.path().to_path_buf();

                        {
                            let store = RocksDbTeleologicalStore::open(&path)
                                .expect("BENCH ERROR: Failed to open RocksDB store");
                            let rt = tokio::runtime::Runtime::new()
                                .expect("BENCH ERROR: Failed to create tokio runtime");
                            let mut rng = StdRng::seed_from_u64(42);

                            for i in 0..size {
                                let source_id = Uuid::new_v4();
                                let rel = create_test_relationship_with_e11(&mut rng, source_id, i);
                                rt.block_on(async { store.store_causal_relationship(&rel).await })
                                    .expect("BENCH ERROR: Failed to pre-store relationship");
                            }
                        }
                        // Store is dropped, database is closed

                        (path, temp_dir)
                    },
                    |(path, _temp_dir)| {
                        // Benchmark: Reopen store (triggers index rebuild)
                        let _store = RocksDbTeleologicalStore::open(black_box(&path))
                            .expect("BENCH ERROR: Failed to reopen RocksDB store");
                    },
                    criterion::BatchSize::PerIteration,
                )
            },
        );
    }

    group.finish();
}

// ============================================================================
// BENCHMARKS - Delete Operations
// ============================================================================

/// Benchmark: Delete causal relationship.
/// Tests primary delete + secondary index update.
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
    bench_e5_search_comparison,
    bench_hybrid_weight_variations,
    bench_delete_causal_relationship,
    // E11 Entity Search benchmarks
    bench_e11_search,
    bench_e11_search_scaling,
    bench_e11_vs_e1_comparison,
    // Multi-Embedder Fusion benchmarks
    bench_multi_embedder_search,
    bench_e11_weight_sensitivity,
    // E11 HNSW vs Brute-Force comparison benchmarks
    bench_e11_hnsw_vs_brute_force,
    bench_e11_hnsw_rebuild,
);
criterion_main!(benches);
