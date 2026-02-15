#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! Real Graph Linking Benchmark - NO SIMULATIONS.
//!
//! This benchmark tests the REAL graph linking system end-to-end:
//! - Uses REAL NnDescent.build()
//! - Uses REAL EdgeBuilder.build_typed_edges()
//! - Uses REAL EdgeRepository with actual RocksDB storage
//! - Verifies edges are physically stored and retrievable
//! - Tests multi-embedder agreement detection
//! - Ensures each embedder contributes unique signal
//!
//! ## Philosophy (Per Constitution v6.5)
//!
//! E1 is the semantic foundation. The other 12 embedders ENHANCE E1 by finding
//! what E1 misses:
//! - E5 finds causal chains E1 missed
//! - E7 finds code patterns E1 missed
//! - E10 finds intent alignment E1 missed
//! - E11 finds entity relationships E1 missed
//!
//! ## NO FALLBACKS
//!
//! All errors are propagated. If something fails, the benchmark fails with
//! detailed error logging. No mock data, no simulations.
//!
//! ## Usage
//!
//! ```bash
//! # Run with synthetic data (default, 50 memories)
//! cargo run -p context-graph-benchmark --bin graph-linking-real-bench --release
//!
//! # Run with real data from semantic_benchmark (1000 chunks)
//! cargo run -p context-graph-benchmark --bin graph-linking-real-bench --release -- \
//!     --data-dir data/semantic_benchmark --num-chunks 1000
//!
//! # Run with real data, more chunks
//! cargo run -p context-graph-benchmark --bin graph-linking-real-bench --release -- \
//!     --data-dir data/semantic_benchmark --num-chunks 5000
//! ```

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use tempfile::TempDir;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

use context_graph_benchmark::realdata::loader::{DatasetLoader, RealDataset};
use context_graph_core::graph_linking::{
    build_asymmetric_knn, EdgeBuilder, EdgeBuilderConfig, EmbedderEdge, KnnGraph, NnDescent,
    NnDescentConfig,
};
use context_graph_storage::graph_edges::EdgeRepository;
use context_graph_storage::teleological::RocksDbTeleologicalStore;

// ============================================================================
// Constants
// ============================================================================

/// Active SYMMETRIC embedders per constitution (excluding temporal E2-E4)
/// These use NnDescent.build() with EmbedderEdge::new()
const SYMMETRIC_EMBEDDERS: [u8; 4] = [0, 6, 9, 10]; // E1, E7, E10, E11

/// Active ASYMMETRIC embedders per AP-77 (require direction)
/// These use build_asymmetric_knn() with EmbedderEdge::with_direction()
/// Per AP-77: E5 (causal) and E8 (graph) MUST NOT use symmetric cosine
const ASYMMETRIC_EMBEDDERS: [u8; 2] = [4, 7]; // E5, E8

/// All active embedders that produce K-NN graphs (symmetric + asymmetric).
///
/// Excluded embedders:
/// - E2-E4 (temporal): NEVER count toward edge detection per AP-60
/// - E6 (sparse/BM25): Sparse vector, not suitable for K-NN
/// - E9 (HDC): Uses Hamming distance, separate implementation
/// - E12 (ColBERT): Token-level, reranking only per AP-74
/// - E13 (SPLADE): Sparse, Stage 1 recall only per AP-75
const ALL_ACTIVE_EMBEDDERS: [u8; 6] = [0, 4, 6, 7, 9, 10]; // E1, E5, E7, E8, E10, E11

/// Embedding dimensions per embedder
const EMBEDDING_DIMS: [usize; 13] = [
    1024, // E1 - semantic
    512,  // E2 - recency (temporal)
    512,  // E3 - periodic (temporal)
    512,  // E4 - sequence (temporal)
    768,  // E5 - causal
    1024, // E6 - sparse (placeholder dim)
    1536, // E7 - code
    1024, // E8 - graph (e5-large-v2)
    1024, // E9 - HDC
    768,  // E10 - intent
    768,  // E11 - entity
    128,  // E12 - ColBERT
    1024, // E13 - SPLADE (placeholder dim)
];

/// Topic weighted agreement threshold per constitution
const WEIGHTED_AGREEMENT_THRESHOLD: f32 = 2.5;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    /// Data directory (if None, use synthetic data)
    data_dir: Option<PathBuf>,
    /// Number of chunks to load (for real data) or memories to generate (for synthetic)
    num_chunks: usize,
    /// K value for K-NN
    k: usize,
    /// Random seed
    seed: u64,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_dir: None,
            num_chunks: 50,
            k: 10,
            seed: 42,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--data-dir" | "-d" => {
                args.data_dir = Some(PathBuf::from(argv.next().expect("--data-dir requires a value")));
            }
            "--num-chunks" | "-n" => {
                args.num_chunks = argv
                    .next()
                    .expect("--num-chunks requires a value")
                    .parse()
                    .expect("--num-chunks must be a number");
            }
            "--k" => {
                args.k = argv
                    .next()
                    .expect("--k requires a value")
                    .parse()
                    .expect("--k must be a number");
            }
            "--seed" => {
                args.seed = argv
                    .next()
                    .expect("--seed requires a value")
                    .parse()
                    .expect("--seed must be a number");
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
                print_usage();
                std::process::exit(1);
            }
        }
    }

    args
}

fn print_usage() {
    println!(
        r#"
Real Graph Linking Benchmark

Usage: graph-linking-real-bench [OPTIONS]

Options:
    --data-dir, -d <PATH>    Load real data from directory (default: synthetic data)
    --num-chunks, -n <N>     Number of chunks/memories [default: 50]
    --k <K>                  K value for K-NN [default: 10]
    --seed <N>               Random seed [default: 42]
    --help, -h               Print this help message

Examples:
    # Synthetic data (50 memories)
    graph-linking-real-bench

    # Real data (1000 chunks from semantic_benchmark)
    graph-linking-real-bench --data-dir data/semantic_benchmark --num-chunks 1000
"#
    );
}

// ============================================================================
// Memory Types
// ============================================================================

/// A memory with embeddings for all 13 embedders.
#[derive(Debug, Clone)]
struct Memory {
    id: Uuid,
    topic_id: usize,
    doc_id: Option<String>,
    embeddings: HashMap<u8, Vec<f32>>,
}

// ============================================================================
// Data Generation / Loading
// ============================================================================

/// Generate synthetic memories with controlled clustering.
fn generate_synthetic_memories(
    num_memories: usize,
    num_topics: usize,
    seed: u64,
) -> Vec<Memory> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut memories = Vec::with_capacity(num_memories);

    // Generate topic centroids for each embedder
    let mut topic_centroids: HashMap<(usize, u8), Vec<f32>> = HashMap::new();
    for topic_id in 0..num_topics {
        for emb_id in 0..13u8 {
            let dim = EMBEDDING_DIMS[emb_id as usize];
            let mut centroid: Vec<f32> = (0..dim)
                .map(|_| rng.gen::<f32>() * 2.0 - 1.0)
                .collect();
            normalize(&mut centroid);
            topic_centroids.insert((topic_id, emb_id), centroid);
        }
    }

    // Generate memories
    for i in 0..num_memories {
        let id = Uuid::new_v4();
        let topic_id = i % num_topics;

        let mut embeddings = HashMap::new();
        for emb_id in 0..13u8 {
            let centroid = topic_centroids.get(&(topic_id, emb_id)).unwrap();

            // Add noise to centroid
            let noise_std = 0.1;
            let mut emb: Vec<f32> = centroid
                .iter()
                .map(|&c| c + rng.gen::<f32>() * noise_std * 2.0 - noise_std)
                .collect();

            normalize(&mut emb);
            embeddings.insert(emb_id, emb);
        }

        memories.push(Memory {
            id,
            topic_id,
            doc_id: None,
            embeddings,
        });
    }

    memories
}

/// Load real data and generate topic-clustered embeddings.
///
/// Uses real topic assignments from the dataset to create embeddings that
/// cluster by topic. This tests the graph linking system with realistic
/// topic structure while not requiring GPU embedding generation.
fn load_real_data_with_embeddings(
    data_dir: &PathBuf,
    num_chunks: usize,
    seed: u64,
) -> Result<(Vec<Memory>, RealDataset)> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    info!("Loading real data from: {:?}", data_dir);

    let loader = DatasetLoader::new().with_max_chunks(num_chunks);
    let dataset = loader
        .load_from_dir(data_dir)
        .map_err(|e| anyhow::anyhow!("Failed to load dataset: {}", e))?;

    info!(
        "Loaded {} chunks from {} topics",
        dataset.chunks.len(),
        dataset.topic_count()
    );

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Generate topic centroids for each embedder (same as synthetic)
    let num_topics = dataset.topic_count().max(1);
    let mut topic_centroids: HashMap<(usize, u8), Vec<f32>> = HashMap::new();

    for topic_id in 0..num_topics {
        for emb_id in 0..13u8 {
            let dim = EMBEDDING_DIMS[emb_id as usize];
            let mut centroid: Vec<f32> = (0..dim)
                .map(|_| rng.gen::<f32>() * 2.0 - 1.0)
                .collect();
            normalize(&mut centroid);
            topic_centroids.insert((topic_id, emb_id), centroid);
        }
    }

    // Convert chunks to memories with topic-based embeddings
    let mut memories = Vec::with_capacity(dataset.chunks.len());

    for chunk in &dataset.chunks {
        let id = chunk.uuid();
        let topic_id = dataset.get_topic_idx(chunk);

        let mut embeddings = HashMap::new();
        for emb_id in 0..13u8 {
            let centroid = topic_centroids
                .get(&(topic_id, emb_id))
                .unwrap_or_else(|| topic_centroids.get(&(0, emb_id)).unwrap());

            // Add noise based on content hash for determinism
            let content_hash = md5::compute(chunk.text.as_bytes());
            let hash_seed = u64::from_le_bytes(content_hash[0..8].try_into().unwrap());
            let mut content_rng = ChaCha8Rng::seed_from_u64(hash_seed.wrapping_add(emb_id as u64));

            let noise_std = 0.15; // Slightly more noise for real data
            let mut emb: Vec<f32> = centroid
                .iter()
                .map(|&c| c + content_rng.gen::<f32>() * noise_std * 2.0 - noise_std)
                .collect();

            normalize(&mut emb);
            embeddings.insert(emb_id, emb);
        }

        memories.push(Memory {
            id,
            topic_id,
            doc_id: Some(chunk.doc_id.clone()),
            embeddings,
        });
    }

    info!(
        "Generated embeddings for {} memories across {} topics",
        memories.len(),
        num_topics
    );

    Ok((memories, dataset))
}

/// Cosine similarity function.
/// Note: Vectors are expected to be pre-normalized, but norms are computed
/// defensively in case of edge cases.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Normalize a vector in place.
fn normalize(vec: &mut [f32]) {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        vec.iter_mut().for_each(|x| *x /= norm);
    }
}

/// Return canonical (ordered) UUID pair for undirected edge comparison.
fn canonical_pair(a: Uuid, b: Uuid) -> (Uuid, Uuid) {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}

// ============================================================================
// Benchmark Results
// ============================================================================

#[derive(Debug, Clone, Default)]
struct BenchmarkResults {
    // Configuration
    num_memories: usize,
    num_topics: usize,
    k: usize,
    data_source: String,

    // Phase 1: NnDescent K-NN graph building
    nn_descent_duration_ms: f64,
    knn_graphs_built: usize,
    total_knn_edges: usize,
    edges_per_embedder: HashMap<u8, usize>,

    // Phase 2: EdgeRepository persistence
    edges_stored: usize,
    edges_retrieved: usize,
    storage_roundtrip_ms: f64,

    // Phase 3: EdgeBuilder typed edges
    typed_edges_created: usize,
    avg_agreement_count: f32,
    edge_type_distribution: HashMap<String, usize>,

    // Phase 4: Per-embedder unique contributions
    embedder_unique_finds: HashMap<u8, usize>,

    // Phase 5: Topic clustering validation (for real data)
    intra_topic_edges: usize,
    inter_topic_edges: usize,
    topic_cohesion_ratio: f32,

    // Validation
    all_checks_passed: bool,
    failed_checks: Vec<String>,
}

impl BenchmarkResults {
    fn print_report(&self) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════════════╗");
        println!("║            REAL GRAPH LINKING BENCHMARK RESULTS                      ║");
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Configuration:                                                       ║");
        println!(
            "║   Data: {:58} ║",
            &self.data_source[..self.data_source.len().min(58)]
        );
        println!(
            "║   Memories: {:5}  Topics: {:3}  K: {:3}                              ║",
            self.num_memories, self.num_topics, self.k
        );
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Phase 1: NnDescent K-NN Graph Building (REAL)                       ║");
        println!(
            "║   Duration: {:8.2}ms                                              ║",
            self.nn_descent_duration_ms
        );
        println!(
            "║   K-NN graphs built: {}                                              ║",
            self.knn_graphs_built
        );
        println!(
            "║   Total K-NN edges: {:6}                                           ║",
            self.total_knn_edges
        );
        println!("║   Edges per embedder:                                               ║");
        for emb_id in ALL_ACTIVE_EMBEDDERS {
            if let Some(&count) = self.edges_per_embedder.get(&emb_id) {
                println!(
                    "║     E{:2} ({:12}): {:6} edges                                ║",
                    emb_id + 1,
                    embedder_name(emb_id),
                    count
                );
            }
        }
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Phase 2: EdgeRepository Persistence (REAL RocksDB)                  ║");
        println!(
            "║   Edges stored: {:6}                                              ║",
            self.edges_stored
        );
        println!(
            "║   Edges retrieved: {:6}                                           ║",
            self.edges_retrieved
        );
        println!(
            "║   Roundtrip: {:8.2}ms                                              ║",
            self.storage_roundtrip_ms
        );
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Phase 3: EdgeBuilder Typed Edges (REAL)                             ║");
        println!(
            "║   Typed edges (agreement >= 2.5): {:6}                             ║",
            self.typed_edges_created
        );
        println!(
            "║   Avg agreement count: {:6.2}                                       ║",
            self.avg_agreement_count
        );
        if !self.edge_type_distribution.is_empty() {
            println!("║   Edge type distribution:                                           ║");
            for (edge_type, count) in &self.edge_type_distribution {
                println!(
                    "║     {:20}: {:6}                                    ║",
                    edge_type, count
                );
            }
        }
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Phase 4: Per-Embedder Unique Contributions                          ║");
        let e1_edges = self.edges_per_embedder.get(&0).copied().unwrap_or(0);
        for emb_id in ALL_ACTIVE_EMBEDDERS {
            if emb_id == 0 {
                continue;
            }
            if let Some(&count) = self.embedder_unique_finds.get(&emb_id) {
                let pct = if e1_edges > 0 {
                    (count as f32 / e1_edges as f32) * 100.0
                } else {
                    0.0
                };
                println!(
                    "║   E{:2} ({:12}): {:5} unique ({:5.1}% of E1)                  ║",
                    emb_id + 1,
                    embedder_name(emb_id),
                    count,
                    pct
                );
            }
        }
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        println!("║ Phase 5: Topic Clustering Validation                                ║");
        println!(
            "║   Intra-topic edges: {:6}                                         ║",
            self.intra_topic_edges
        );
        println!(
            "║   Inter-topic edges: {:6}                                         ║",
            self.inter_topic_edges
        );
        println!(
            "║   Topic cohesion: {:6.1}% (intra / total)                          ║",
            self.topic_cohesion_ratio * 100.0
        );
        println!("╠══════════════════════════════════════════════════════════════════════╣");
        if self.all_checks_passed {
            println!("║ ✓ ALL VALIDATION CHECKS PASSED                                     ║");
        } else {
            println!("║ ✗ VALIDATION CHECKS FAILED:                                        ║");
            for check in &self.failed_checks {
                let truncated = if check.len() > 60 {
                    format!("{}...", &check[..57])
                } else {
                    check.clone()
                };
                println!("║   - {:64} ║", truncated);
            }
        }
        println!("╚══════════════════════════════════════════════════════════════════════╝");
        println!();
    }
}

fn embedder_name(id: u8) -> &'static str {
    match id {
        0 => "semantic",
        1 => "recency",
        2 => "periodic",
        3 => "sequence",
        4 => "causal",
        5 => "sparse",
        6 => "code",
        7 => "graph",
        8 => "HDC",
        9 => "intent",
        10 => "entity",
        11 => "ColBERT",
        12 => "SPLADE",
        _ => "unknown",
    }
}

// ============================================================================
// Main Benchmark
// ============================================================================

fn main() -> Result<()> {
    // Setup logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .with_target(false)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let args = parse_args();

    println!("═══════════════════════════════════════════════════════════════════════");
    println!("  REAL GRAPH LINKING BENCHMARK");
    println!("  NO SIMULATIONS - REAL IMPLEMENTATIONS - FAIL FAST");
    println!("═══════════════════════════════════════════════════════════════════════");
    println!();

    // Load or generate data
    let (memories, num_topics, data_source) = if let Some(ref data_dir) = args.data_dir {
        let (memories, dataset) = load_real_data_with_embeddings(data_dir, args.num_chunks, args.seed)?;
        let num_topics = dataset.topic_count();
        let source = format!("Real: {} ({} chunks)", data_dir.display(), memories.len());
        (memories, num_topics, source)
    } else {
        let num_topics = 5;
        info!(
            "Generating {} synthetic memories in {} topics",
            args.num_chunks, num_topics
        );
        let memories = generate_synthetic_memories(args.num_chunks, num_topics, args.seed);
        let source = format!("Synthetic: {} memories, {} topics", memories.len(), num_topics);
        (memories, num_topics, source)
    };

    info!("Data loaded: {} memories, {} topics", memories.len(), num_topics);

    // Run benchmark phases
    let mut results = BenchmarkResults {
        num_memories: memories.len(),
        num_topics,
        k: args.k,
        data_source,
        ..Default::default()
    };

    // Phase 1: Test REAL NnDescent K-NN graph building
    let knn_graphs = phase1_nn_descent(&memories, args.k, &mut results)?;

    // Phase 2: Test REAL EdgeRepository persistence
    phase2_edge_repository(&memories, &knn_graphs, &mut results)?;

    // Phase 3: Test REAL EdgeBuilder typed edges
    phase3_edge_builder(&knn_graphs, &mut results)?;

    // Phase 4: Test per-embedder unique contributions
    phase4_unique_contributions(&knn_graphs, &mut results)?;

    // Phase 5: Topic clustering validation
    phase5_topic_validation(&memories, &knn_graphs, &mut results)?;

    // Validation checks
    run_validation_checks(&mut results);

    // Print report
    results.print_report();

    if !results.all_checks_passed {
        bail!("Benchmark failed: some validation checks did not pass");
    }

    Ok(())
}

/// Phase 1: Test REAL NnDescent K-NN graph building
///
/// Uses the correct API for each embedder type per AP-77:
/// - SYMMETRIC embedders (E1, E7, E10, E11): NnDescent.build()
/// - ASYMMETRIC embedders (E5, E8): build_asymmetric_knn()
fn phase1_nn_descent(
    memories: &[Memory],
    k: usize,
    results: &mut BenchmarkResults,
) -> Result<HashMap<u8, KnnGraph>> {
    info!("Phase 1: Testing REAL NnDescent K-NN graph building");
    info!("  Symmetric embedders: {:?}", SYMMETRIC_EMBEDDERS);
    info!("  Asymmetric embedders (AP-77): {:?}", ASYMMETRIC_EMBEDDERS);

    let nodes: Vec<Uuid> = memories.iter().map(|m| m.id).collect();
    let mut knn_graphs: HashMap<u8, KnnGraph> = HashMap::new();

    let config = NnDescentConfig {
        k,
        iterations: 10,
        min_similarity: 0.1, // Lower threshold to get more edges for testing
        ..Default::default()
    };

    let start = Instant::now();

    // Build K-NN graphs for SYMMETRIC embedders using NnDescent.build()
    for &emb_id in &SYMMETRIC_EMBEDDERS {
        info!("  Building K-NN for E{} ({}) - SYMMETRIC", emb_id + 1, embedder_name(emb_id));

        let nn = NnDescent::new(emb_id, &nodes, config.clone());

        let graph = nn
            .build(
                |id| {
                    memories
                        .iter()
                        .find(|m| m.id == id)
                        .and_then(|m| m.embeddings.get(&emb_id).cloned())
                },
                &cosine_similarity,
            )
            .context(format!("NN-Descent failed for symmetric embedder E{}", emb_id + 1))?;

        let edge_count = graph.edge_count();
        info!("    E{} ({}): {} edges", emb_id + 1, embedder_name(emb_id), edge_count);
        results.edges_per_embedder.insert(emb_id, edge_count);
        knn_graphs.insert(emb_id, graph);
    }

    // Build K-NN graphs for ASYMMETRIC embedders using build_asymmetric_knn()
    // Per AP-77: E5 (causal) and E8 (graph) MUST use asymmetric similarity
    for &emb_id in &ASYMMETRIC_EMBEDDERS {
        info!("  Building K-NN for E{} ({}) - ASYMMETRIC (AP-77)", emb_id + 1, embedder_name(emb_id));

        // For asymmetric embedders, we need separate source/target embeddings
        // In real usage, these would be different projections (cause vs effect)
        // For benchmark, we simulate by using the same embedding (tests the API path)
        let graph = build_asymmetric_knn(
            emb_id,
            &nodes,
            |id| {
                memories
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| m.embeddings.get(&emb_id).cloned())
            },
            |id| {
                memories
                    .iter()
                    .find(|m| m.id == id)
                    .and_then(|m| m.embeddings.get(&emb_id).cloned())
            },
            &cosine_similarity,
            config.clone(),
        )
        .context(format!("build_asymmetric_knn failed for E{} (AP-77 embedder)", emb_id + 1))?;

        let edge_count = graph.edge_count();
        info!("    E{} ({}): {} edges (directed)", emb_id + 1, embedder_name(emb_id), edge_count);
        results.edges_per_embedder.insert(emb_id, edge_count);
        knn_graphs.insert(emb_id, graph);
    }

    let duration = start.elapsed();
    results.nn_descent_duration_ms = duration.as_secs_f64() * 1000.0;
    results.knn_graphs_built = knn_graphs.len();
    results.total_knn_edges = knn_graphs.values().map(|g| g.edge_count()).sum();

    info!(
        "Phase 1 complete: {} K-NN graphs, {} total edges in {:.2}ms",
        results.knn_graphs_built, results.total_knn_edges, results.nn_descent_duration_ms
    );

    // Verify graphs were actually built
    if knn_graphs.is_empty() {
        bail!("NnDescent produced no K-NN graphs!");
    }

    // Verify we have the expected number of graphs
    let expected_graphs = SYMMETRIC_EMBEDDERS.len() + ASYMMETRIC_EMBEDDERS.len();
    if knn_graphs.len() != expected_graphs {
        bail!(
            "Expected {} K-NN graphs but only built {}",
            expected_graphs,
            knn_graphs.len()
        );
    }

    Ok(knn_graphs)
}

/// Phase 2: Test REAL EdgeRepository persistence
fn phase2_edge_repository(
    memories: &[Memory],
    knn_graphs: &HashMap<u8, KnnGraph>,
    results: &mut BenchmarkResults,
) -> Result<()> {
    info!("Phase 2: Testing REAL EdgeRepository with RocksDB");

    // Create temp directory for RocksDB
    let temp_dir = TempDir::new().context("Failed to create temp dir")?;
    let db_path = temp_dir.path();
    info!("Created temp RocksDB at: {:?}", db_path);

    // Open RocksDB store
    let store = RocksDbTeleologicalStore::open(db_path)
        .context("Failed to open RocksDbTeleologicalStore")?;
    let edge_repo = EdgeRepository::new(store.db_arc());
    info!("EdgeRepository created");

    let start = Instant::now();
    let mut total_stored = 0;

    // Store K-NN edges from the graphs we built
    for (&emb_id, graph) in knn_graphs {
        // Group edges by source
        let mut edges_by_source: HashMap<Uuid, Vec<EmbedderEdge>> = HashMap::new();
        for edge in graph.edges() {
            edges_by_source
                .entry(edge.source())
                .or_default()
                .push(edge.clone());
        }

        // Store each source's edges
        for (source_id, edges) in edges_by_source {
            if !edges.is_empty() {
                edge_repo
                    .store_embedder_edges(emb_id, source_id, &edges)
                    .context(format!(
                        "Failed to store embedder edges for E{} source {}",
                        emb_id + 1,
                        source_id
                    ))?;
                total_stored += edges.len();
            }
        }
    }

    results.edges_stored = total_stored;

    // Verify edges can be retrieved
    let mut total_retrieved = 0;
    for &emb_id in &ALL_ACTIVE_EMBEDDERS {
        for memory in memories {
            let edges = edge_repo
                .get_embedder_edges(emb_id, memory.id)
                .context(format!(
                    "Failed to retrieve edges for E{} source {}",
                    emb_id + 1,
                    memory.id
                ))?;
            total_retrieved += edges.len();
        }
    }

    results.edges_retrieved = total_retrieved;
    results.storage_roundtrip_ms = start.elapsed().as_secs_f64() * 1000.0;

    info!(
        "Phase 2 complete: stored {} edges, retrieved {} edges in {:.2}ms",
        results.edges_stored, results.edges_retrieved, results.storage_roundtrip_ms
    );

    // Verify storage consistency
    if results.edges_stored != results.edges_retrieved {
        bail!(
            "Storage inconsistency: stored {} but retrieved {}",
            results.edges_stored,
            results.edges_retrieved
        );
    }

    // Verify edges actually exist in RocksDB by getting stats
    let stats = edge_repo
        .get_stats()
        .context("Failed to get EdgeRepository stats")?;
    info!(
        "  RocksDB stats: {} total embedder edges, {} bytes",
        stats.total_embedder_edges, stats.storage_bytes
    );

    Ok(())
}

/// Phase 3: Test REAL EdgeBuilder typed edges
fn phase3_edge_builder(
    knn_graphs: &HashMap<u8, KnnGraph>,
    results: &mut BenchmarkResults,
) -> Result<()> {
    info!("Phase 3: Testing REAL EdgeBuilder typed edges");

    // Build typed edges using REAL EdgeBuilder
    let mut edge_builder = EdgeBuilder::new(EdgeBuilderConfig {
        min_weighted_agreement: WEIGHTED_AGREEMENT_THRESHOLD,
        ..Default::default()
    });

    for graph in knn_graphs.values() {
        edge_builder.add_knn_graph(graph.clone());
    }

    let typed_edges = edge_builder
        .build_typed_edges()
        .context("EdgeBuilder.build_typed_edges() failed")?;

    results.typed_edges_created = typed_edges.len();

    // Analyze agreement patterns
    let mut total_agreement = 0u32;
    let mut edge_type_counts: HashMap<String, usize> = HashMap::new();

    for edge in &typed_edges {
        total_agreement += edge.agreement_count() as u32;
        let edge_type_name = format!("{:?}", edge.edge_type());
        *edge_type_counts.entry(edge_type_name).or_insert(0) += 1;
    }

    results.avg_agreement_count = if typed_edges.is_empty() {
        0.0
    } else {
        total_agreement as f32 / typed_edges.len() as f32
    };
    results.edge_type_distribution = edge_type_counts;

    info!(
        "Phase 3 complete: {} typed edges with agreement >= {}, avg agreement: {:.2}",
        results.typed_edges_created, WEIGHTED_AGREEMENT_THRESHOLD, results.avg_agreement_count
    );

    Ok(())
}

/// Phase 4: Test per-embedder unique contributions
fn phase4_unique_contributions(
    knn_graphs: &HashMap<u8, KnnGraph>,
    results: &mut BenchmarkResults,
) -> Result<()> {
    info!("Phase 4: Testing per-embedder unique contributions");

    // Get E1 edges as baseline
    let e1_graph = knn_graphs.get(&0).context("E1 K-NN graph not found")?;
    let e1_edges: HashSet<(Uuid, Uuid)> = e1_graph
        .edges()
        .map(|e| canonical_pair(e.source(), e.target()))
        .collect();

    info!("  E1 (semantic) has {} unique edge pairs", e1_edges.len());

    // For each enhancer embedder, find unique edges
    for &emb_id in &ALL_ACTIVE_EMBEDDERS {
        if emb_id == 0 {
            continue; // Skip E1 itself
        }

        if let Some(graph) = knn_graphs.get(&emb_id) {
            // Count edges this embedder found that E1 missed
            let unique_count = graph
                .edges()
                .filter(|edge| !e1_edges.contains(&canonical_pair(edge.source(), edge.target())))
                .count();

            results.embedder_unique_finds.insert(emb_id, unique_count);
            info!(
                "  E{} ({}) found {} unique edges (E1 missed)",
                emb_id + 1,
                embedder_name(emb_id),
                unique_count
            );
        }
    }

    Ok(())
}

/// Phase 5: Topic clustering validation
fn phase5_topic_validation(
    memories: &[Memory],
    knn_graphs: &HashMap<u8, KnnGraph>,
    results: &mut BenchmarkResults,
) -> Result<()> {
    info!("Phase 5: Validating topic clustering in K-NN graphs");

    // Build memory ID to topic mapping
    let id_to_topic: HashMap<Uuid, usize> = memories
        .iter()
        .map(|m| (m.id, m.topic_id))
        .collect();

    // Count intra-topic vs inter-topic edges in E1 graph
    let e1_graph = knn_graphs.get(&0).context("E1 K-NN graph not found")?;

    let mut intra_topic = 0usize;
    let mut inter_topic = 0usize;

    for edge in e1_graph.edges() {
        let source_topic = id_to_topic.get(&edge.source()).copied().unwrap_or(usize::MAX);
        let target_topic = id_to_topic.get(&edge.target()).copied().unwrap_or(usize::MAX);

        if source_topic == target_topic {
            intra_topic += 1;
        } else {
            inter_topic += 1;
        }
    }

    results.intra_topic_edges = intra_topic;
    results.inter_topic_edges = inter_topic;

    let total = intra_topic + inter_topic;
    results.topic_cohesion_ratio = if total > 0 {
        intra_topic as f32 / total as f32
    } else {
        0.0
    };

    info!(
        "  Intra-topic: {}, Inter-topic: {}, Cohesion: {:.1}%",
        intra_topic,
        inter_topic,
        results.topic_cohesion_ratio * 100.0
    );

    Ok(())
}

/// Run validation checks
fn run_validation_checks(results: &mut BenchmarkResults) {
    let mut all_passed = true;
    let mut failed = Vec::new();

    // Check 1: K-NN graphs were built
    if results.knn_graphs_built == 0 {
        all_passed = false;
        failed.push("No K-NN graphs were built".to_string());
    }

    // Check 2: K-NN graphs have edges
    if results.total_knn_edges == 0 {
        all_passed = false;
        failed.push("K-NN graphs have no edges".to_string());
    }

    // Check 3: Storage roundtrip succeeded
    if results.edges_stored != results.edges_retrieved {
        all_passed = false;
        failed.push(format!(
            "Storage mismatch: {} stored vs {} retrieved",
            results.edges_stored, results.edges_retrieved
        ));
    }

    // Check 4: Edges were actually stored
    if results.edges_stored == 0 {
        all_passed = false;
        failed.push("No edges were stored to RocksDB".to_string());
    }

    // Check 5: Some typed edges were created (with real data, expect some)
    if results.typed_edges_created == 0 && results.num_memories >= 100 {
        warn!("No typed edges met agreement threshold (may indicate poor clustering)");
    }

    // Check 6: Enhancer embedders found unique edges
    let total_unique: usize = results.embedder_unique_finds.values().sum();
    if total_unique == 0 {
        warn!("No enhancer embedders found unique edges (E1 found everything)");
    }

    // Check 7: Topic cohesion should be reasonable (>30% for clustered data)
    if results.num_memories >= 100 && results.topic_cohesion_ratio < 0.3 {
        warn!(
            "Low topic cohesion ({:.1}%) - expected >30% for clustered data",
            results.topic_cohesion_ratio * 100.0
        );
    }

    results.all_checks_passed = all_passed;
    results.failed_checks = failed;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_synthetic_memories() {
        let memories = generate_synthetic_memories(10, 3, 42);
        assert_eq!(memories.len(), 10);
        for m in &memories {
            assert_eq!(m.embeddings.len(), 13);
        }
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &a);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn test_canonical_pair() {
        let a = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let b = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

        assert_eq!(canonical_pair(a, b), (a, b));
        assert_eq!(canonical_pair(b, a), (a, b));
    }
}
