//! Benchmarks for measuring the effectiveness of source-anchored hybrid embeddings.
//!
//! These benchmarks measure QUALITY metrics rather than speed:
//! - Source diversity: unique source documents in top-K results
//! - Score variance: how spread out are similarity scores (clustered = low variance)
//! - Result overlap: how different are results between E5-only vs hybrid
//!
//! # The Problem We're Solving
//!
//! LLM-generated explanations tend to cluster together in embedding space because:
//! 1. They share similar vocabulary (HPA axis, cortisol, etc.)
//! 2. They follow the same prompt structure
//! 3. They use similar causal markers (causes, leads to)
//!
//! This benchmark creates relationships with SIMILAR explanations but DIFFERENT
//! source content, then measures whether hybrid search returns more diverse results.
//!
//! # Usage
//!
//! ```bash
//! cargo bench -p context-graph-storage --bench hybrid_effectiveness_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::HashSet;
use tempfile::TempDir;
use uuid::Uuid;

use context_graph_core::types::CausalRelationship;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// ============================================================================
// CONSTANTS
// ============================================================================

const E1_DIM: usize = 1024;
const E5_DIM: usize = 768;

// ============================================================================
// TEST DATA GENERATION
// ============================================================================

/// Generate a random normalized E1 embedding.
fn generate_e1_embedding(rng: &mut StdRng) -> Vec<f32> {
    let mut embedding: Vec<f32> = (0..E1_DIM).map(|_| rng.gen::<f32>() - 0.5).collect();
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }
    embedding
}

/// Generate a random normalized E5 embedding.
fn generate_e5_embedding(rng: &mut StdRng) -> Vec<f32> {
    let mut embedding: Vec<f32> = (0..E5_DIM).map(|_| rng.gen::<f32>() - 0.5).collect();
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }
    embedding
}

/// Generate an E5 embedding that clusters around a base embedding.
/// This simulates LLM explanations that are structurally similar.
fn generate_clustered_e5_embedding(base: &[f32], rng: &mut StdRng, noise_scale: f32) -> Vec<f32> {
    let mut embedding: Vec<f32> = base
        .iter()
        .map(|&x| x + (rng.gen::<f32>() - 0.5) * noise_scale)
        .collect();

    // Normalize
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        embedding.iter_mut().for_each(|x| *x /= norm);
    }
    embedding
}

/// Template for LLM-style explanations that all look similar.
/// The `{}` is replaced with topic-specific content.
const EXPLANATION_TEMPLATE: &str =
    "This causal relationship describes how {} leads to significant downstream effects. \
     The mechanism involves sustained activation of the underlying pathways, which over time \
     causes progressive changes. This has implications for understanding the broader system.";

/// Unique source documents - these are intentionally very different.
const UNIQUE_SOURCES: &[&str] = &[
    "Research paper: Stress and cognition in rodent models. Journal of Neuroscience 2023.",
    "Database performance optimization guide. PostgreSQL documentation v15.",
    "Memory management in Rust: A practical guide. Rustacean Station podcast transcript.",
    "Kubernetes networking deep dive. CNCF webinar recording.",
    "Financial modeling with Python. Quantitative Finance textbook Chapter 12.",
    "Climate change impact assessment report. IPCC Working Group II.",
    "Machine learning model deployment patterns. MLOps handbook.",
    "Cellular biology: mitochondrial function. Medical physiology textbook.",
    "Supply chain optimization algorithms. Operations Research journal.",
    "Social media influence on behavior. Psychology Today article.",
    "Quantum computing fundamentals. IBM Research whitepaper.",
    "Urban planning and transportation. City planning commission report.",
    "Cybersecurity threat landscape 2024. NIST technical document.",
    "Agricultural yield prediction using satellite data. Remote Sensing journal.",
    "Pharmaceutical drug interaction database. FDA clinical guidelines.",
];

/// Topic terms to inject into the template.
const TOPICS: &[&str] = &[
    "chronic stress exposure",
    "connection pool exhaustion",
    "memory allocation patterns",
    "network latency spikes",
    "market volatility",
    "temperature increases",
    "model drift",
    "ATP production",
    "inventory shortages",
    "social media usage",
    "qubit decoherence",
    "traffic congestion",
    "malware propagation",
    "soil degradation",
    "drug metabolism",
];

/// Create a relationship with clustered explanation embedding but unique source embedding.
fn create_clustered_relationship(
    rng: &mut StdRng,
    source_id: Uuid,
    index: usize,
    base_e5: &[f32],
) -> CausalRelationship {
    let topic = TOPICS[index % TOPICS.len()];
    let explanation = EXPLANATION_TEMPLATE.replace("{}", topic);
    let source = UNIQUE_SOURCES[index % UNIQUE_SOURCES.len()].to_string();

    // Explanation embedding clusters around the base (simulating LLM clustering)
    let e5_as_cause = generate_clustered_e5_embedding(base_e5, rng, 0.1);
    let e5_as_effect = generate_clustered_e5_embedding(base_e5, rng, 0.1);

    // Source embedding is random (each source is unique)
    let e5_source_cause = generate_e5_embedding(rng);
    let e5_source_effect = generate_e5_embedding(rng);

    CausalRelationship::new(
        format!("{} triggers cascade", topic),
        "Progressive system changes".to_string(),
        explanation,
        e5_as_cause,
        e5_as_effect,
        generate_e1_embedding(rng),
        source,
        source_id,
        0.8 + (index % 20) as f32 * 0.01,
        ["direct", "mediated", "feedback", "temporal"][index % 4].to_string(),
    )
    .with_source_embeddings(e5_source_cause, e5_source_effect)
}

// ============================================================================
// DIVERSITY METRICS
// ============================================================================

/// Calculate the number of unique source documents in the top-K results.
fn count_unique_sources(results: &[(Uuid, f32)], relationships: &[CausalRelationship]) -> usize {
    let result_ids: HashSet<_> = results.iter().map(|(id, _)| *id).collect();

    let sources: HashSet<_> = relationships
        .iter()
        .filter(|r| result_ids.contains(&r.id))
        .map(|r| &r.source_content)
        .collect();

    sources.len()
}

/// Calculate variance of similarity scores (lower = more clustered).
fn score_variance(results: &[(Uuid, f32)]) -> f32 {
    if results.is_empty() {
        return 0.0;
    }

    let mean: f32 = results.iter().map(|(_, s)| s).sum::<f32>() / results.len() as f32;
    let variance: f32 =
        results.iter().map(|(_, s)| (s - mean).powi(2)).sum::<f32>() / results.len() as f32;

    variance
}

/// Calculate overlap between two result sets.
fn result_overlap(a: &[(Uuid, f32)], b: &[(Uuid, f32)]) -> f32 {
    let set_a: HashSet<_> = a.iter().map(|(id, _)| *id).collect();
    let set_b: HashSet<_> = b.iter().map(|(id, _)| *id).collect();

    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

// ============================================================================
// EFFECTIVENESS BENCHMARKS
// ============================================================================

/// Benchmark: Compare diversity between E5-only and hybrid search.
///
/// Creates relationships with similar explanations but different sources,
/// then measures which search method returns more diverse results.
fn bench_diversity_comparison(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store =
        RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(42);

    // Create a base E5 embedding that all explanations will cluster around
    let base_e5 = generate_e5_embedding(&mut rng);

    // Store relationships with clustered explanations
    println!("\nCreating 100 relationships with clustered explanations...");
    let mut stored_relationships = Vec::new();

    for i in 0..100 {
        let source_id = Uuid::new_v4();
        let rel = create_clustered_relationship(&mut rng, source_id, i, &base_e5);
        let id = rt
            .block_on(async { store.store_causal_relationship(&rel).await })
            .expect("Failed to store");

        // Store for later lookup
        let mut stored = rel.clone();
        stored.id = id;
        stored_relationships.push(stored);
    }

    // Create a query that's similar to the clustered explanations
    let query = generate_clustered_e5_embedding(&base_e5, &mut rng, 0.05);

    let mut group = c.benchmark_group("effectiveness/diversity");
    group.sample_size(50);

    // Measure E5-only search
    let mut e5_diversity_samples = Vec::new();
    group.bench_function("measure_e5_only", |b| {
        b.iter(|| {
            let results = rt
                .block_on(async { store.search_causal_e5(black_box(&query), true, 10).await })
                .expect("E5 search failed");

            let diversity = count_unique_sources(&results, &stored_relationships);
            e5_diversity_samples.push(diversity);
            black_box(results)
        })
    });

    // Measure hybrid search
    let mut hybrid_diversity_samples = Vec::new();
    group.bench_function("measure_hybrid", |b| {
        b.iter(|| {
            let results = rt
                .block_on(async {
                    store
                        .search_causal_e5_hybrid(black_box(&query), true, 10, 0.6, 0.4)
                        .await
                })
                .expect("Hybrid search failed");

            let diversity = count_unique_sources(&results, &stored_relationships);
            hybrid_diversity_samples.push(diversity);
            black_box(results)
        })
    });

    group.finish();

    // Print diversity analysis
    let e5_results = rt
        .block_on(async { store.search_causal_e5(&query, true, 10).await })
        .expect("E5 search failed");

    let hybrid_results = rt
        .block_on(async {
            store
                .search_causal_e5_hybrid(&query, true, 10, 0.6, 0.4)
                .await
        })
        .expect("Hybrid search failed");

    let e5_unique = count_unique_sources(&e5_results, &stored_relationships);
    let hybrid_unique = count_unique_sources(&hybrid_results, &stored_relationships);
    let e5_variance = score_variance(&e5_results);
    let hybrid_variance = score_variance(&hybrid_results);
    let overlap = result_overlap(&e5_results, &hybrid_results);

    println!("\n========== DIVERSITY ANALYSIS ==========");
    println!("E5-only unique sources in top-10:  {}", e5_unique);
    println!("Hybrid unique sources in top-10:   {}", hybrid_unique);
    println!(
        "Diversity improvement:             {:+.1}%",
        (hybrid_unique as f32 - e5_unique as f32) / e5_unique as f32 * 100.0
    );
    println!();
    println!("E5-only score variance:  {:.6}", e5_variance);
    println!("Hybrid score variance:   {:.6}", hybrid_variance);
    println!();
    println!("Result overlap (Jaccard): {:.1}%", overlap * 100.0);
    println!("=========================================\n");
}

/// Benchmark: Test different noise levels to understand clustering severity.
fn bench_clustering_severity(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(123);

    let base_e5 = generate_e5_embedding(&mut rng);

    // Test different noise levels
    let noise_levels = [0.05, 0.1, 0.2, 0.3];

    println!("\n========== CLUSTERING SEVERITY ANALYSIS ==========");

    for noise in noise_levels {
        // Create fresh store for each noise level
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let store =
            RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open RocksDB store");

        let mut stored_rels = Vec::new();

        // Create relationships with varying noise
        for i in 0..50 {
            let source_id = Uuid::new_v4();
            let topic = TOPICS[i % TOPICS.len()];
            let explanation = EXPLANATION_TEMPLATE.replace("{}", topic);
            let source = UNIQUE_SOURCES[i % UNIQUE_SOURCES.len()].to_string();

            let e5_as_cause = generate_clustered_e5_embedding(&base_e5, &mut rng, noise);
            let e5_as_effect = generate_clustered_e5_embedding(&base_e5, &mut rng, noise);
            let e5_source_cause = generate_e5_embedding(&mut rng);
            let e5_source_effect = generate_e5_embedding(&mut rng);

            let rel = CausalRelationship::new(
                format!("{} triggers cascade", topic),
                "Changes".to_string(),
                explanation,
                e5_as_cause,
                e5_as_effect,
                generate_e1_embedding(&mut rng),
                source,
                source_id,
                0.85,
                "direct".to_string(),
            )
            .with_source_embeddings(e5_source_cause, e5_source_effect);

            let id = rt
                .block_on(async { store.store_causal_relationship(&rel).await })
                .expect("Failed to store");

            let mut stored = rel.clone();
            stored.id = id;
            stored_rels.push(stored);
        }

        let query = generate_clustered_e5_embedding(&base_e5, &mut rng, noise / 2.0);

        let e5_results = rt
            .block_on(async { store.search_causal_e5(&query, true, 10).await })
            .expect("E5 search failed");

        let hybrid_results = rt
            .block_on(async {
                store
                    .search_causal_e5_hybrid(&query, true, 10, 0.6, 0.4)
                    .await
            })
            .expect("Hybrid search failed");

        let e5_unique = count_unique_sources(&e5_results, &stored_rels);
        let hybrid_unique = count_unique_sources(&hybrid_results, &stored_rels);

        println!(
            "Noise={:.2}: E5 unique={}, Hybrid unique={}, Improvement={:+}",
            noise,
            e5_unique,
            hybrid_unique,
            hybrid_unique as i32 - e5_unique as i32
        );
    }

    println!("==================================================\n");

    // Simple benchmark to satisfy criterion
    let mut group = c.benchmark_group("effectiveness/clustering_severity");
    group.sample_size(10);
    group.bench_function("analysis_complete", |b| b.iter(|| black_box(42)));
    group.finish();
}

/// Benchmark: Measure result stability across multiple queries.
fn bench_result_stability(c: &mut Criterion) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store =
        RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open RocksDB store");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let mut rng = StdRng::seed_from_u64(456);

    let base_e5 = generate_e5_embedding(&mut rng);

    // Create relationships
    let mut stored_rels = Vec::new();
    for i in 0..100 {
        let source_id = Uuid::new_v4();
        let rel = create_clustered_relationship(&mut rng, source_id, i, &base_e5);
        let id = rt
            .block_on(async { store.store_causal_relationship(&rel).await })
            .expect("Failed to store");

        let mut stored = rel.clone();
        stored.id = id;
        stored_rels.push(stored);
    }

    // Generate multiple similar queries and measure stability
    println!("\n========== RESULT STABILITY ANALYSIS ==========");

    let num_queries = 10;
    let mut e5_all_results: Vec<Vec<(Uuid, f32)>> = Vec::new();
    let mut hybrid_all_results: Vec<Vec<(Uuid, f32)>> = Vec::new();

    for _ in 0..num_queries {
        let query = generate_clustered_e5_embedding(&base_e5, &mut rng, 0.08);

        let e5_res = rt
            .block_on(async { store.search_causal_e5(&query, true, 10).await })
            .expect("E5 search failed");

        let hybrid_res = rt
            .block_on(async {
                store
                    .search_causal_e5_hybrid(&query, true, 10, 0.6, 0.4)
                    .await
            })
            .expect("Hybrid search failed");

        e5_all_results.push(e5_res);
        hybrid_all_results.push(hybrid_res);
    }

    // Calculate average pairwise overlap
    let mut e5_overlaps = Vec::new();
    let mut hybrid_overlaps = Vec::new();

    for i in 0..num_queries {
        for j in (i + 1)..num_queries {
            e5_overlaps.push(result_overlap(&e5_all_results[i], &e5_all_results[j]));
            hybrid_overlaps.push(result_overlap(
                &hybrid_all_results[i],
                &hybrid_all_results[j],
            ));
        }
    }

    let avg_e5_overlap: f32 = e5_overlaps.iter().sum::<f32>() / e5_overlaps.len() as f32;
    let avg_hybrid_overlap: f32 =
        hybrid_overlaps.iter().sum::<f32>() / hybrid_overlaps.len() as f32;

    println!(
        "E5-only avg result overlap:   {:.1}%",
        avg_e5_overlap * 100.0
    );
    println!(
        "Hybrid avg result overlap:    {:.1}%",
        avg_hybrid_overlap * 100.0
    );
    println!();
    println!("Higher overlap = more consistent results for similar queries");
    println!("Lower overlap = more sensitive to query variations");
    println!("================================================\n");

    // Simple benchmark
    let mut group = c.benchmark_group("effectiveness/stability");
    group.sample_size(10);
    group.bench_function("analysis_complete", |b| b.iter(|| black_box(42)));
    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    benches,
    bench_diversity_comparison,
    bench_clustering_severity,
    bench_result_stability,
);
criterion_main!(benches);
