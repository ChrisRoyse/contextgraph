//! E1 Semantic Embedder Benchmark CLI.
//!
//! This binary runs a comprehensive benchmark of the E1 semantic embedder
//! (intfloat/e5-large-v2, 1024D) as THE semantic foundation per ARCH-12.
//!
//! ## Usage
//!
//! ```bash
//! # Synthetic data benchmark (no GPU required)
//! cargo run -p context-graph-benchmark --bin e1-semantic-bench --release -- \
//!     --num-documents 1000 \
//!     --num-queries 100
//!
//! # Real data benchmark (GPU required)
//! cargo run -p context-graph-benchmark --bin e1-semantic-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark/temp_wikipedia \
//!     --use-realdata \
//!     --max-chunks 5000
//!
//! # Full benchmark
//! cargo run -p context-graph-benchmark --bin e1-semantic-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark_diverse \
//!     --use-realdata \
//!     --benchmark all \
//!     --output benchmark_results/e1_semantic.json
//! ```
//!
//! ## Targets (from CLAUDE.md)
//!
//! - MRR@10: >= 0.70
//! - P@10: >= 0.60
//! - Topic separation ratio: >= 1.5
//! - Noise robustness (0.2 noise): MRR >= 0.55
//! - Single embed latency: < 5ms P95
//!
//! ## Benchmark Phases
//!
//! - basic: Run basic retrieval metrics (P@K, R@K, MRR, NDCG, MAP)
//! - separation: Run topic separation analysis
//! - noise: Run noise robustness analysis
//! - ablation: Run E1 vs E1+enhancers comparison
//! - all: Run all phases (default)

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;
use uuid::Uuid;

use context_graph_benchmark::datasets::e1_semantic::{
    E1SemanticDatasetConfig, E1SemanticDatasetGenerator, SemanticDocument,
};
use context_graph_benchmark::runners::e1_semantic::{
    E1SemanticBenchmarkConfig, E1SemanticBenchmarkResults, E1SemanticBenchmarkRunner,
};

#[cfg(feature = "real-embeddings")]
use std::fs::{self, File};
#[cfg(feature = "real-embeddings")]
use std::io::Write;
#[cfg(feature = "real-embeddings")]
use std::time::Instant;
#[cfg(feature = "real-embeddings")]
use tracing::{info, warn};

#[cfg(feature = "real-embeddings")]
use context_graph_benchmark::realdata::{embedder::RealDataEmbedder, loader::DatasetLoader};
#[cfg(feature = "real-embeddings")]
use context_graph_core::types::fingerprint::SemanticFingerprint;

/// Benchmark phases to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BenchmarkPhase {
    /// Basic retrieval metrics only.
    Basic,
    /// Topic separation analysis.
    Separation,
    /// Noise robustness analysis.
    Noise,
    /// Ablation study (E1 vs E1+enhancers).
    Ablation,
    /// All phases (default).
    All,
}

/// E1 Semantic Embedder Benchmark CLI.
#[derive(Parser, Debug)]
#[command(name = "e1-semantic-bench")]
#[command(about = "Benchmark E1 semantic embedder (intfloat/e5-large-v2, 1024D)")]
struct Args {
    /// Data directory containing chunks.jsonl and metadata.json.
    #[arg(long, default_value = "data/hf_benchmark_diverse")]
    data_dir: PathBuf,

    /// Output path for benchmark results JSON.
    #[arg(long, default_value = "benchmark_results/e1_semantic.json")]
    output: PathBuf,

    /// Use real data from HuggingFace dataset (requires --features real-embeddings).
    #[arg(long)]
    use_realdata: bool,

    /// Maximum chunks to load from real data (0 = unlimited).
    #[arg(long, default_value = "0")]
    max_chunks: usize,

    /// Number of synthetic documents to generate.
    #[arg(long, default_value = "5000")]
    num_documents: usize,

    /// Number of synthetic queries to generate.
    #[arg(long, default_value = "500")]
    num_queries: usize,

    /// Benchmark phase to run.
    #[arg(long, value_enum, default_value = "all")]
    benchmark: BenchmarkPhase,

    /// Random seed for reproducibility.
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Top-K values for retrieval metrics.
    #[arg(long, num_args = 1.., default_values = ["1", "5", "10", "20"])]
    top_k: Vec<usize>,

    /// Noise levels for robustness testing.
    #[arg(long, num_args = 1.., default_values = ["0.0", "0.1", "0.2", "0.3"])]
    noise_levels: Vec<f64>,

    /// Skip ablation study.
    #[arg(long)]
    skip_ablation: bool,

    /// Skip domain analysis.
    #[arg(long)]
    skip_domain_analysis: bool,

    /// Verbose output.
    #[arg(short, long)]
    verbose: bool,

    /// Run BEIR evaluation mode (requires queries.jsonl and qrels.json in data-dir).
    /// This computes official BEIR metrics (NDCG@10, MRR@10, MAP) against ground truth.
    #[arg(long)]
    beir: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    println!("=======================================================================");
    println!("  E1 SEMANTIC EMBEDDER BENCHMARK");
    println!("  Foundation: intfloat/e5-large-v2 (1024D)");
    println!("=======================================================================");
    println!();
    println!("ARCH-12: E1 is THE semantic foundation - all retrieval starts here");
    println!("ARCH-16: E1 provides baseline retrieval; enhancers refine");
    println!();

    if args.beir {
        #[cfg(feature = "real-embeddings")]
        {
            run_beir_benchmark(&args)
        }

        #[cfg(not(feature = "real-embeddings"))]
        {
            eprintln!("ERROR: --beir requires --features real-embeddings");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  cargo run -p context-graph-benchmark --bin e1-semantic-bench \\");
            eprintln!("      --release --features real-embeddings -- \\");
            eprintln!("      --beir --data-dir data/beir_scifact");
            eprintln!();
            std::process::exit(1);
        }
    } else if args.use_realdata {
        #[cfg(feature = "real-embeddings")]
        {
            run_realdata_benchmark(&args)
        }

        #[cfg(not(feature = "real-embeddings"))]
        {
            eprintln!("ERROR: --use-realdata requires --features real-embeddings");
            eprintln!();
            eprintln!("Usage:");
            eprintln!("  cargo run -p context-graph-benchmark --bin e1-semantic-bench \\");
            eprintln!("      --release --features real-embeddings -- \\");
            eprintln!("      --use-realdata --data-dir data/hf_benchmark_diverse");
            eprintln!();
            std::process::exit(1);
        }
    } else {
        run_synthetic_benchmark(&args)
    }
}

/// Run benchmark with synthetic data.
fn run_synthetic_benchmark(args: &Args) -> Result<()> {
    println!("Running SYNTHETIC DATA benchmark...");
    println!();

    // Generate synthetic dataset
    println!("Step 1: Generating synthetic dataset...");
    let dataset_config = E1SemanticDatasetConfig {
        num_documents: args.num_documents,
        num_queries: args.num_queries,
        seed: args.seed,
        ..Default::default()
    };

    let mut generator = E1SemanticDatasetGenerator::new(dataset_config);
    let dataset = generator.generate();

    println!("  Documents: {}", dataset.documents.len());
    println!("  Queries: {}", dataset.queries.len());
    println!("  Topics: {}", dataset.stats.num_topics);
    println!("  Domains: {}", dataset.stats.num_domains);
    println!();

    // Generate synthetic embeddings
    println!("Step 2: Generating synthetic embeddings...");
    let embeddings = generate_synthetic_embeddings(&dataset.documents, args.seed);
    println!("  Generated {} embeddings (1024D)", embeddings.len());
    println!();

    // Run benchmark
    println!("Step 3: Running benchmark suite...");
    let bench_config = E1SemanticBenchmarkConfig {
        k_values: args.top_k.clone(),
        noise_levels: args.noise_levels.clone(),
        num_separation_pairs: 5000,
        run_ablation: !args.skip_ablation && matches!(args.benchmark, BenchmarkPhase::Ablation | BenchmarkPhase::All),
        run_domain_analysis: !args.skip_domain_analysis,
        seed: args.seed,
        show_progress: true,
    };

    let mut runner = E1SemanticBenchmarkRunner::new(bench_config);
    let results = runner.run_from_dataset(&dataset, &embeddings);

    // Print results
    print_results(&results);

    // Save results
    save_results(&results, &args.output)?;

    if results.all_targets_met() {
        println!("\nSUCCESS: ALL TARGETS MET");
        Ok(())
    } else {
        println!("\nWARNING: SOME TARGETS NOT MET");
        Ok(())
    }
}

/// Generate synthetic embeddings for documents.
///
/// Creates embeddings that cluster by topic for testing.
fn generate_synthetic_embeddings(
    documents: &[SemanticDocument],
    seed: u64,
) -> HashMap<Uuid, Vec<f32>> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let dim = 1024;

    // Create topic centroids
    let topics: std::collections::HashSet<usize> = documents.iter().map(|d| d.topic_id).collect();
    let mut topic_centroids: HashMap<usize, Vec<f32>> = HashMap::new();

    for topic_id in topics {
        let mut centroid = vec![0.0f32; dim];
        for c in &mut centroid {
            *c = rng.gen::<f32>() * 2.0 - 1.0;
        }
        // Normalize
        let norm: f32 = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        for c in &mut centroid {
            *c /= norm;
        }
        topic_centroids.insert(topic_id, centroid);
    }

    // Generate embeddings around centroids
    let mut embeddings = HashMap::new();
    for doc in documents {
        let centroid = topic_centroids.get(&doc.topic_id).unwrap();
        let mut emb = centroid.clone();

        // Add noise to differentiate documents within topic
        for e in &mut emb {
            *e += rng.gen::<f32>() * 0.2 - 0.1;
        }

        // Normalize
        let norm: f32 = emb.iter().map(|x| x * x).sum::<f32>().sqrt();
        for e in &mut emb {
            *e /= norm;
        }

        embeddings.insert(doc.id, emb);
    }

    embeddings
}

/// Run benchmark with real data (requires GPU).
#[cfg(feature = "real-embeddings")]
fn run_realdata_benchmark(args: &Args) -> Result<()> {
    let total_start = Instant::now();

    info!("Running REAL DATA benchmark...");
    info!("");

    // Step 1: Load dataset
    info!("Step 1: Loading dataset from: {}", args.data_dir.display());
    let load_start = Instant::now();

    let loader = DatasetLoader::new().with_max_chunks(args.max_chunks);
    let dataset = loader.load_from_dir(&args.data_dir)?;

    let load_time = load_start.elapsed();
    info!("  Loaded {} chunks from {} topics", dataset.chunks.len(), dataset.topic_count());
    info!("  Load time: {:?}", load_time);

    // Step 2: Embed chunks with E1 (semantic embedder)
    info!("");
    info!("Step 2: Generating E1 embeddings (this may take a while)...");
    let embed_start = Instant::now();

    let embedder = RealDataEmbedder::default();
    let embedded = tokio::runtime::Runtime::new()?
        .block_on(async { embedder.embed_dataset(&dataset).await })?;

    let embed_time = embed_start.elapsed();
    info!("  Embedded {} chunks", embedded.fingerprints.len());
    info!("  Embedding time: {:?}", embed_time);
    info!("  Rate: {:.1} chunks/sec", embedded.fingerprints.len() as f64 / embed_time.as_secs_f64());

    // Extract E1 embeddings specifically
    let e1_embeddings: HashMap<Uuid, Vec<f32>> = embedded
        .fingerprints
        .iter()
        .map(|(id, fp)| (*id, fp.e1_semantic.clone()))
        .collect();

    info!("  E1 embedding dimension: {}", e1_embeddings.values().next().map(|e| e.len()).unwrap_or(0));

    // Convert to SemanticDocument format
    let documents: Vec<SemanticDocument> = dataset
        .chunks
        .iter()
        .map(|chunk| {
            use context_graph_benchmark::datasets::e1_semantic::SemanticDomain;
            SemanticDocument {
                id: chunk.uuid(),
                text: chunk.text.clone(),
                domain: SemanticDomain::General, // Default for real data
                topic: chunk.topic_hint.clone(),
                topic_id: embedded.topic_assignments.get(&chunk.uuid()).copied().unwrap_or(0),
                source_dataset: Some(chunk.doc_id.clone()),
            }
        })
        .collect();

    // Generate queries from the dataset
    let queries = generate_queries_from_realdata(&documents, &e1_embeddings, args.num_queries, args.seed);

    info!("");
    info!("Step 3: Running benchmark suite...");

    let bench_config = E1SemanticBenchmarkConfig {
        k_values: args.top_k.clone(),
        noise_levels: args.noise_levels.clone(),
        num_separation_pairs: 5000,
        run_ablation: !args.skip_ablation && matches!(args.benchmark, BenchmarkPhase::Ablation | BenchmarkPhase::All),
        run_domain_analysis: !args.skip_domain_analysis,
        seed: args.seed,
        show_progress: true,
    };

    let mut runner = E1SemanticBenchmarkRunner::new(bench_config);
    let results = runner.run(&documents, &queries, &e1_embeddings, &embedded.topic_assignments);

    let total_time = total_start.elapsed();

    // Print results
    print_results(&results);

    // Save results
    save_results(&results, &args.output)?;

    info!("");
    info!("=======================================================================");
    info!("  TIMING SUMMARY");
    info!("=======================================================================");
    info!("  Dataset loading:      {:?}", load_time);
    info!("  Embedding generation: {:?}", embed_time);
    info!("  Benchmark execution:  {:?}", std::time::Duration::from_millis(results.timings.total_ms));
    info!("  Total time:           {:?}", total_time);
    info!("");

    if results.all_targets_met() {
        info!("SUCCESS: ALL TARGETS MET");
        Ok(())
    } else {
        warn!("WARNING: SOME TARGETS NOT MET");
        Ok(())
    }
}

/// Run BEIR benchmark with ground truth qrels.
///
/// This evaluates E1 against standard BEIR benchmarks like SciFact.
/// Expected e5-large-v2 SciFact scores:
///   - NDCG@10: 72.24
///   - MRR@10: 68.80
///   - MAP@10: 67.90
#[cfg(feature = "real-embeddings")]
fn run_beir_benchmark(args: &Args) -> Result<()> {
    use std::io::BufRead;

    let total_start = Instant::now();

    println!("Running BEIR EVALUATION benchmark...");
    println!("  Data directory: {}", args.data_dir.display());
    println!();

    // Load BEIR queries
    let queries_path = args.data_dir.join("queries.jsonl");
    if !queries_path.exists() {
        anyhow::bail!("queries.jsonl not found in {}. Run the BEIR download script first.", args.data_dir.display());
    }

    let queries_file = std::fs::File::open(&queries_path)?;
    let reader = std::io::BufReader::new(queries_file);
    let mut beir_queries: Vec<(String, String)> = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let obj: serde_json::Value = serde_json::from_str(&line)?;
        let query_id = obj["query_id"].as_str().unwrap_or("").to_string();
        let text = obj["text"].as_str().unwrap_or("").to_string();
        beir_queries.push((query_id, text));
    }
    println!("  Loaded {} queries", beir_queries.len());

    // Load BEIR qrels
    let qrels_path = args.data_dir.join("qrels.json");
    if !qrels_path.exists() {
        anyhow::bail!("qrels.json not found in {}. Run the BEIR download script first.", args.data_dir.display());
    }
    let qrels_json: serde_json::Value = serde_json::from_reader(std::fs::File::open(&qrels_path)?)?;
    let qrels: HashMap<String, HashMap<String, i64>> = qrels_json
        .as_object()
        .map(|obj| {
            obj.iter()
                .map(|(qid, docs)| {
                    let doc_scores: HashMap<String, i64> = docs
                        .as_object()
                        .map(|d| {
                            d.iter()
                                .map(|(did, score)| (did.clone(), score.as_i64().unwrap_or(0)))
                                .collect()
                        })
                        .unwrap_or_default();
                    (qid.clone(), doc_scores)
                })
                .collect()
        })
        .unwrap_or_default();
    println!("  Loaded qrels for {} queries", qrels.len());

    // Load doc_id mapping (original_doc_id -> our doc_id)
    let doc_id_map_path = args.data_dir.join("doc_id_map.json");
    let doc_id_map: HashMap<String, String> = if doc_id_map_path.exists() {
        serde_json::from_reader(std::fs::File::open(&doc_id_map_path)?)?
    } else {
        HashMap::new()
    };
    println!("  Loaded {} doc_id mappings", doc_id_map.len());

    // Load metadata for expected scores
    let metadata_path = args.data_dir.join("metadata.json");
    let metadata: serde_json::Value = if metadata_path.exists() {
        serde_json::from_reader(std::fs::File::open(&metadata_path)?)?
    } else {
        serde_json::json!({})
    };
    let expected_ndcg10 = metadata["expected_ndcg10"].as_f64().unwrap_or(0.0);
    let expected_mrr10 = metadata["expected_mrr10"].as_f64().unwrap_or(0.0);

    println!();
    info!("Step 1: Loading corpus from: {}", args.data_dir.display());
    let load_start = Instant::now();

    let loader = DatasetLoader::new().with_max_chunks(args.max_chunks);
    let dataset = loader.load_from_dir(&args.data_dir)?;
    let load_time = load_start.elapsed();

    info!("  Loaded {} documents", dataset.chunks.len());
    info!("  Load time: {:?}", load_time);

    // Build original_doc_id -> chunk UUID mapping
    let mut orig_id_to_uuid: HashMap<String, Uuid> = HashMap::new();
    for chunk in &dataset.chunks {
        // The chunk's original_doc_id is stored in chunk metadata
        // We need to find it - check if doc_id matches our mapping
        for (orig_id, mapped_id) in &doc_id_map {
            if &chunk.doc_id == mapped_id {
                orig_id_to_uuid.insert(orig_id.clone(), chunk.uuid());
                break;
            }
        }
    }
    info!("  Mapped {} documents to UUIDs", orig_id_to_uuid.len());

    // Step 2: Embed corpus with E1
    info!("");
    info!("Step 2: Generating E1 embeddings for corpus...");
    let embed_start = Instant::now();

    let embedder = RealDataEmbedder::default();
    let embedded = tokio::runtime::Runtime::new()?
        .block_on(async { embedder.embed_dataset(&dataset).await })?;

    let embed_time = embed_start.elapsed();
    info!("  Embedded {} documents", embedded.fingerprints.len());
    info!("  Embedding time: {:?}", embed_time);

    // Extract E1 embeddings
    let e1_embeddings: HashMap<Uuid, Vec<f32>> = embedded
        .fingerprints
        .iter()
        .map(|(id, fp)| (*id, fp.e1_semantic.clone()))
        .collect();

    // Step 3: Embed queries and evaluate
    info!("");
    info!("Step 3: Evaluating queries with ground truth qrels...");

    // Get the embedding provider for query embedding
    let provider = context_graph_embeddings::get_warm_provider()
        .map_err(|e| anyhow::anyhow!("Failed to get embedding provider: {}", e))?;

    let mut ndcg_scores: Vec<f64> = Vec::new();
    let mut mrr_scores: Vec<f64> = Vec::new();
    let mut ap_scores: Vec<f64> = Vec::new();
    let mut evaluated = 0;

    // Only evaluate queries that have qrels
    let queries_with_qrels: Vec<_> = beir_queries
        .iter()
        .filter(|(qid, _)| qrels.contains_key(qid))
        .collect();

    println!("  Evaluating {} queries with ground truth...", queries_with_qrels.len());

    for (i, (query_id, query_text)) in queries_with_qrels.iter().enumerate() {
        if i % 50 == 0 {
            print!("\r  Progress: {}/{}", i, queries_with_qrels.len());
            std::io::Write::flush(&mut std::io::stdout())?;
        }

        // Embed query with E1 (using "query: " prefix per e5-large-v2 format)
        let query_with_prefix = format!("query: {}", query_text);
        let query_output = tokio::runtime::Runtime::new()?
            .block_on(async {
                provider.embed_all(&query_with_prefix).await
            })
            .map_err(|e| anyhow::anyhow!("Failed to embed query: {}", e))?;
        let query_embedding = query_output.fingerprint.e1_semantic;

        // Retrieve top-k documents by cosine similarity
        let mut scores: Vec<(Uuid, f64)> = e1_embeddings
            .iter()
            .map(|(id, emb)| {
                let sim = cosine_similarity(&query_embedding, emb);
                (*id, sim)
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Get ground truth relevant docs for this query
        let relevant_docs = qrels.get(query_id.as_str()).cloned().unwrap_or_default();

        // Compute metrics
        let k = 10;
        let top_k: Vec<Uuid> = scores.iter().take(k).map(|(id, _)| *id).collect();

        // Convert UUIDs back to original doc IDs for comparison
        let uuid_to_orig: HashMap<Uuid, String> = orig_id_to_uuid
            .iter()
            .map(|(k, v)| (*v, k.clone()))
            .collect();

        // NDCG@10
        let mut dcg = 0.0;
        let mut idcg = 0.0;

        // Compute DCG
        for (rank, uuid) in top_k.iter().enumerate() {
            if let Some(orig_id) = uuid_to_orig.get(uuid) {
                if let Some(&rel) = relevant_docs.get(orig_id) {
                    if rel > 0 {
                        dcg += (rel as f64) / (rank as f64 + 2.0).log2();
                    }
                }
            }
        }

        // Compute IDCG (ideal ranking)
        let mut ideal_rels: Vec<i64> = relevant_docs.values().copied().filter(|&r| r > 0).collect();
        ideal_rels.sort_by(|a, b| b.cmp(a));
        for (rank, &rel) in ideal_rels.iter().take(k).enumerate() {
            idcg += (rel as f64) / (rank as f64 + 2.0).log2();
        }

        let ndcg = if idcg > 0.0 { dcg / idcg } else { 0.0 };
        ndcg_scores.push(ndcg);

        // MRR (first relevant result)
        let mut rr = 0.0;
        for (rank, uuid) in top_k.iter().enumerate() {
            if let Some(orig_id) = uuid_to_orig.get(uuid) {
                if let Some(&rel) = relevant_docs.get(orig_id) {
                    if rel > 0 {
                        rr = 1.0 / (rank as f64 + 1.0);
                        break;
                    }
                }
            }
        }
        mrr_scores.push(rr);

        // Average Precision
        let mut relevant_so_far = 0;
        let mut precision_sum = 0.0;
        for (rank, uuid) in scores.iter().enumerate() {
            if let Some(orig_id) = uuid_to_orig.get(&uuid.0) {
                if let Some(&rel) = relevant_docs.get(orig_id) {
                    if rel > 0 {
                        relevant_so_far += 1;
                        precision_sum += relevant_so_far as f64 / (rank as f64 + 1.0);
                    }
                }
            }
        }
        let num_relevant = relevant_docs.values().filter(|&&r| r > 0).count();
        let ap = if num_relevant > 0 {
            precision_sum / num_relevant as f64
        } else {
            0.0
        };
        ap_scores.push(ap);

        evaluated += 1;
    }

    println!("\r  Evaluated {} queries                    ", evaluated);

    let avg_ndcg10 = ndcg_scores.iter().sum::<f64>() / ndcg_scores.len().max(1) as f64 * 100.0;
    let avg_mrr10 = mrr_scores.iter().sum::<f64>() / mrr_scores.len().max(1) as f64 * 100.0;
    let avg_map = ap_scores.iter().sum::<f64>() / ap_scores.len().max(1) as f64 * 100.0;

    let total_time = total_start.elapsed();

    // Print results
    println!();
    println!("=======================================================================");
    println!("  BEIR EVALUATION RESULTS");
    println!("=======================================================================");
    println!();
    println!("RETRIEVAL QUALITY:");
    println!("  NDCG@10:    {:.2}%  (expected: {:.2}%)", avg_ndcg10, expected_ndcg10);
    println!("  MRR@10:     {:.2}%  (expected: {:.2}%)", avg_mrr10, expected_mrr10);
    println!("  MAP:        {:.2}%", avg_map);
    println!();

    // Check against expected scores
    let ndcg_diff = (avg_ndcg10 - expected_ndcg10).abs();
    let mrr_diff = (avg_mrr10 - expected_mrr10).abs();

    println!("VALIDATION:");
    if ndcg_diff <= 5.0 {
        println!("  [PASS] NDCG@10 within 5% of expected ({:.2}% vs {:.2}%)", avg_ndcg10, expected_ndcg10);
    } else {
        println!("  [WARN] NDCG@10 differs by {:.2}% from expected", ndcg_diff);
    }
    if mrr_diff <= 5.0 {
        println!("  [PASS] MRR@10 within 5% of expected ({:.2}% vs {:.2}%)", avg_mrr10, expected_mrr10);
    } else {
        println!("  [WARN] MRR@10 differs by {:.2}% from expected", mrr_diff);
    }
    println!();

    println!("TIMING:");
    println!("  Dataset loading:      {:?}", load_time);
    println!("  Embedding generation: {:?}", embed_time);
    println!("  Total time:           {:?}", total_time);
    println!();

    // Save results
    let beir_results = serde_json::json!({
        "dataset": args.data_dir.to_string_lossy(),
        "num_documents": dataset.chunks.len(),
        "num_queries": evaluated,
        "ndcg10": avg_ndcg10,
        "mrr10": avg_mrr10,
        "map": avg_map,
        "expected_ndcg10": expected_ndcg10,
        "expected_mrr10": expected_mrr10,
        "load_time_ms": load_time.as_millis(),
        "embed_time_ms": embed_time.as_millis(),
        "total_time_ms": total_time.as_millis(),
    });

    let output_path = args.output.with_file_name(
        args.output
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + "_beir.json",
    );
    fs::create_dir_all(output_path.parent().unwrap_or(&std::path::PathBuf::from(".")))?;
    let mut file = File::create(&output_path)?;
    file.write_all(serde_json::to_string_pretty(&beir_results)?.as_bytes())?;
    println!("Results saved to: {}", output_path.display());

    Ok(())
}

/// Cosine similarity between two vectors.
#[cfg(feature = "real-embeddings")]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        (dot / (norm_a * norm_b)) as f64
    }
}

/// Generate queries from real data.
#[cfg(feature = "real-embeddings")]
fn generate_queries_from_realdata(
    documents: &[SemanticDocument],
    embeddings: &HashMap<Uuid, Vec<f32>>,
    num_queries: usize,
    seed: u64,
) -> Vec<context_graph_benchmark::datasets::e1_semantic::SemanticQuery> {
    use context_graph_benchmark::datasets::e1_semantic::{SemanticQuery, SemanticQueryType};
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use std::collections::HashSet;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut queries = Vec::new();

    // Group documents by topic
    let mut topic_docs: HashMap<usize, Vec<&SemanticDocument>> = HashMap::new();
    for doc in documents {
        topic_docs.entry(doc.topic_id).or_default().push(doc);
    }

    // Generate same-topic queries
    for _ in 0..(num_queries * 7 / 10).min(documents.len()) {
        if let Some(doc) = documents.choose(&mut rng) {
            let relevant: HashSet<Uuid> = topic_docs
                .get(&doc.topic_id)
                .map(|docs| docs.iter().map(|d| d.id).collect())
                .unwrap_or_default();

            if relevant.is_empty() {
                continue;
            }

            // Safely truncate text at char boundary
            let preview: String = doc.text.chars().take(100).collect();
            queries.push(SemanticQuery {
                id: Uuid::new_v4(),
                text: format!("Find documents similar to: {}", preview),
                target_domain: doc.domain,
                target_topic: doc.topic.clone(),
                target_topic_id: doc.topic_id,
                query_type: SemanticQueryType::SameTopic,
                relevant_docs: relevant.clone(),
                relevance_scores: relevant.iter().map(|id| (*id, 1.0)).collect(),
                noise_level: 0.0,
            });
        }
    }

    // Generate off-topic queries
    for _ in 0..(num_queries / 10) {
        queries.push(SemanticQuery {
            id: Uuid::new_v4(),
            text: "Cooking recipes for vegetarian dishes".to_string(),
            target_domain: context_graph_benchmark::datasets::e1_semantic::SemanticDomain::General,
            target_topic: "cooking".to_string(),
            target_topic_id: usize::MAX,
            query_type: SemanticQueryType::OffTopic,
            relevant_docs: HashSet::new(),
            relevance_scores: HashMap::new(),
            noise_level: 0.0,
        });
    }

    queries.truncate(num_queries);
    queries.shuffle(&mut rng);
    queries
}

/// Print benchmark results to console.
fn print_results(results: &E1SemanticBenchmarkResults) {
    println!();
    println!("=======================================================================");
    println!("  E1 SEMANTIC BENCHMARK RESULTS");
    println!("=======================================================================");
    println!();

    // Retrieval Metrics
    println!("RETRIEVAL QUALITY:");
    let mrr = results.metrics.retrieval.mrr;
    let mrr_status = if mrr >= 0.70 { "PASS" } else { "FAIL" };
    println!("  [{}] MRR:                    {:.3} (target: >= 0.70)", mrr_status, mrr);

    if let Some(&p10) = results.metrics.retrieval.precision_at.get(&10) {
        let p10_status = if p10 >= 0.60 { "PASS" } else { "FAIL" };
        println!("  [{}] P@10:                   {:.3} (target: >= 0.60)", p10_status, p10);
    }

    if let Some(&r10) = results.metrics.retrieval.recall_at.get(&10) {
        println!("       R@10:                   {:.3}", r10);
    }

    if let Some(&ndcg10) = results.metrics.retrieval.ndcg_at.get(&10) {
        println!("       NDCG@10:                {:.3}", ndcg10);
    }

    println!("       MAP:                    {:.3}", results.metrics.retrieval.map);
    println!();

    // Topic Separation (informational - not a standard retrieval benchmark)
    println!("TOPIC SEPARATION (informational):");
    let sep = results.metrics.topic_separation.separation_ratio;
    println!(
        "       Separation ratio:       {:.2}x",
        sep
    );
    println!(
        "       Intra-topic sim:        {:.3}",
        results.metrics.topic_separation.intra_topic_similarity
    );
    println!(
        "       Inter-topic sim:        {:.3}",
        results.metrics.topic_separation.inter_topic_similarity
    );
    println!(
        "       Silhouette score:       {:.3}",
        results.metrics.topic_separation.silhouette_score
    );
    println!(
        "       Boundary F1:            {:.3}",
        results.metrics.topic_separation.boundary_f1
    );
    println!();

    // Noise Robustness
    println!("NOISE ROBUSTNESS:");
    for (noise, mrr) in &results.metrics.noise_robustness.mrr_degradation {
        let status = if *noise == 0.2 {
            if *mrr >= 0.55 { "PASS" } else { "FAIL" }
        } else {
            "    "
        };
        println!(
            "  [{}] MRR at noise {:.1}:       {:.3}{}",
            status,
            noise,
            mrr,
            if *noise == 0.2 { " (target: >= 0.55)" } else { "" }
        );
    }
    println!(
        "       Avg degradation:        {:.1}%",
        results.metrics.noise_robustness.avg_relative_degradation * 100.0
    );
    println!();

    // Domain Coverage (if available)
    if !results.metrics.domain_coverage.per_domain_metrics.is_empty() {
        println!("DOMAIN COVERAGE:");
        for (domain, metrics) in &results.metrics.domain_coverage.per_domain_metrics {
            println!("       {}: MRR={:.3}", domain, metrics.mrr);
        }
        if let Some(worst) = &results.metrics.domain_coverage.worst_domain {
            println!("       Worst domain:           {}", worst);
        }
        if let Some(best) = &results.metrics.domain_coverage.best_domain {
            println!("       Best domain:            {}", best);
        }
        println!();
    }

    // Per Query Type (if available)
    if !results.per_query_type.is_empty() {
        println!("PER QUERY TYPE:");
        for (query_type, metrics) in &results.per_query_type {
            println!("       {}: MRR={:.3}", query_type, metrics.mrr);
        }
        println!();
    }

    // Composite Score
    println!("COMPOSITE SCORE:");
    println!(
        "       Overall score:          {:.3}",
        results.metrics.composite.overall_score
    );
    println!(
        "       Retrieval score:        {:.3}",
        results.metrics.composite.retrieval_score
    );
    println!(
        "       Separation score:       {:.3}",
        results.metrics.composite.separation_score
    );
    println!(
        "       Robustness score:       {:.3}",
        results.metrics.composite.robustness_score
    );
    println!();

    // Validation Summary
    println!("VALIDATION SUMMARY:");
    for check in &results.validation.checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        println!("  [{}] {}: {} ({})", status, check.description, check.actual, check.expected);
    }
    println!();
    println!(
        "  Targets met: {}/{}",
        results.validation.checks_passed, results.validation.checks_total
    );
    println!();

    // Dataset Stats
    println!("DATASET STATISTICS:");
    println!("       Documents:              {}", results.dataset_stats.num_documents);
    println!("       Queries:                {}", results.dataset_stats.num_queries);
    println!("       Topics:                 {}", results.dataset_stats.num_topics);
    println!("       Domains:                {}", results.dataset_stats.num_domains);
    println!("       Embedding dimension:    {}", results.dataset_stats.embedding_dimension);
    println!();

    // Timings
    println!("TIMING:");
    println!("       Total benchmark:        {}ms", results.timings.total_ms);
    println!("       Retrieval phase:        {}ms", results.timings.retrieval_ms);
    println!("       Separation phase:       {}ms", results.timings.separation_ms);
    println!("       Noise robustness:       {}ms", results.timings.noise_robustness_ms);
    if let Some(domain_ms) = results.timings.domain_analysis_ms {
        println!("       Domain analysis:        {}ms", domain_ms);
    }
    if let Some(ablation_ms) = results.timings.ablation_ms {
        println!("       Ablation study:         {}ms", ablation_ms);
    }
}

/// Save results to JSON file.
fn save_results(results: &E1SemanticBenchmarkResults, output_path: &PathBuf) -> Result<()> {
    use std::fs::{self, File};
    use std::io::Write;

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(results)?;
    let mut file = File::create(output_path)?;
    file.write_all(json.as_bytes())?;

    println!("Results saved to: {}", output_path.display());

    Ok(())
}
