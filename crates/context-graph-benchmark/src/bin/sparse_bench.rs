#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! E6 Sparse Embedder Benchmark CLI
//!
//! Runs comprehensive benchmarks for E6 (V_selectivity) sparse embedder using
//! real Wikipedia data and actual embeddings.
//!
//! ## Usage
//!
//! ```bash
//! # Run with real embeddings (requires GPU)
//! cargo run --release -p context-graph-benchmark --bin sparse-bench \
//!   --features real-embeddings -- \
//!   --data-dir data/hf_benchmark/temp_wikipedia
//!
//! # Run with synthetic data (no GPU required)
//! cargo run --release -p context-graph-benchmark --bin sparse-bench -- \
//!   --synthetic --num-documents 100 --num-queries 20
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;


#[cfg(feature = "real-embeddings")]
use context_graph_benchmark::realdata::embedder::EmbeddedDataset;
#[cfg(feature = "real-embeddings")]
use context_graph_benchmark::realdata::RealDataEmbedder;

/// E6 Sparse Embedder Benchmark Suite
#[derive(Parser, Debug)]
#[command(name = "sparse-bench")]
#[command(about = "Benchmark E6 sparse embedder using real data or synthetic generation")]
struct Args {
    /// Data directory containing chunks.jsonl and metadata.json
    #[arg(long, default_value = "data/hf_benchmark/temp_wikipedia")]
    data_dir: PathBuf,

    /// Output directory for results
    #[arg(long, default_value = "benchmark_results")]
    output_dir: PathBuf,

    /// Maximum chunks to load (0 = all)
    #[arg(long, default_value = "5000")]
    max_chunks: usize,

    /// Number of query samples
    #[arg(long, default_value = "100")]
    num_queries: usize,

    /// Random seed for reproducibility
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Run with synthetic data instead of real data
    #[arg(long)]
    synthetic: bool,

    /// Number of documents for synthetic mode
    #[arg(long, default_value = "100")]
    num_documents: usize,

    /// Run threshold sweep analysis
    #[arg(long)]
    threshold_sweep: bool,

    /// Checkpoint directory for embedding progress
    #[arg(long)]
    checkpoint_dir: Option<PathBuf>,

    /// Checkpoint interval (embeddings between saves)
    #[arg(long, default_value = "1000")]
    checkpoint_interval: usize,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Results from E6 sparse benchmark
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct E6RealBenchmarkResults {
    /// MRR@10 for E6 sparse embedder
    pub e6_mrr_at_10: f64,
    /// MRR@10 for E1 semantic (baseline)
    pub e1_mrr_at_10: f64,
    /// MRR@10 for E13 SPLADE (comparison)
    pub e13_mrr_at_10: f64,
    /// E6 improvement over E1 baseline
    pub e6_vs_e1_delta: f64,
    /// E6 vs E13 comparison
    pub e6_vs_e13_delta: f64,
    /// Average active terms in E6 sparse vectors
    pub avg_active_terms: f64,
    /// Sparsity ratio (proportion of zeros)
    pub sparsity_ratio: f64,
    /// Number of queries evaluated
    pub query_count: usize,
    /// Number of documents in corpus
    pub corpus_size: usize,
    /// Per-topic MRR breakdown
    pub per_topic_mrr: HashMap<String, f64>,
    /// Timing information
    pub timings: BenchmarkTimings,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BenchmarkTimings {
    pub data_load_ms: u64,
    pub embedding_ms: u64,
    pub evaluation_ms: u64,
    pub total_ms: u64,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let log_level = if args.verbose { Level::DEBUG } else { Level::INFO };
    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("=======================================================================");
    info!("  E6 SPARSE EMBEDDER BENCHMARK (REAL DATA)");
    info!("=======================================================================");
    info!("");

    if args.synthetic {
        info!("Mode: SYNTHETIC DATA");
        run_synthetic_benchmark(&args)
    } else {
        info!("Mode: REAL DATA");
        #[cfg(feature = "real-embeddings")]
        {
            run_real_data_benchmark(&args)
        }
        #[cfg(not(feature = "real-embeddings"))]
        {
            warn!("Real embeddings not enabled. Run with --features real-embeddings");
            warn!("Falling back to synthetic benchmark...");
            run_synthetic_benchmark(&args)
        }
    }
}

#[cfg(feature = "real-embeddings")]
fn run_real_data_benchmark(args: &Args) -> Result<()> {
    use context_graph_core::types::fingerprint::SemanticFingerprint;

    let total_start = Instant::now();

    // Step 1: Load dataset
    info!("Loading dataset from: {}", args.data_dir.display());
    let load_start = Instant::now();

    let loader = DatasetLoader::new()
        .with_max_chunks(args.max_chunks);

    let dataset = loader.load_from_dir(&args.data_dir)?;
    let load_time = load_start.elapsed();

    info!("  Loaded {} chunks from {} topics", dataset.chunks.len(), dataset.topic_count());
    info!("  Load time: {:?}", load_time);

    // Step 2: Generate embeddings
    info!("");
    info!("Generating embeddings (this may take a while)...");
    let embed_start = Instant::now();

    let embedder = RealDataEmbedder::default();
    let embedded = tokio::runtime::Runtime::new()?.block_on(async {
        embedder.embed_dataset_batched(
            &dataset,
            args.checkpoint_dir.as_deref(),
            args.checkpoint_interval,
        ).await
    })?;
    let embed_time = embed_start.elapsed();

    info!("  Embedded {} fingerprints", embedded.fingerprints.len());
    info!("  Embedding time: {:?}", embed_time);

    // Step 3: Run E6 sparse evaluation
    info!("");
    info!("Running E6 sparse evaluation...");
    let eval_start = Instant::now();

    let results = evaluate_e6_sparse(&dataset, &embedded, args.num_queries, args.seed);
    let eval_time = eval_start.elapsed();

    let total_time = total_start.elapsed();

    // Create final results
    let benchmark_results = E6RealBenchmarkResults {
        e6_mrr_at_10: results.e6_mrr,
        e1_mrr_at_10: results.e1_mrr,
        e13_mrr_at_10: results.e13_mrr,
        e6_vs_e1_delta: results.e6_mrr - results.e1_mrr,
        e6_vs_e13_delta: results.e6_mrr - results.e13_mrr,
        avg_active_terms: results.avg_active_terms,
        sparsity_ratio: results.sparsity_ratio,
        query_count: results.query_count,
        corpus_size: embedded.fingerprints.len(),
        per_topic_mrr: results.per_topic_mrr,
        timings: BenchmarkTimings {
            data_load_ms: load_time.as_millis() as u64,
            embedding_ms: embed_time.as_millis() as u64,
            evaluation_ms: eval_time.as_millis() as u64,
            total_ms: total_time.as_millis() as u64,
        },
    };

    // Print and save results
    print_results(&benchmark_results);
    save_results(&benchmark_results, &args.output_dir)?;

    Ok(())
}

#[cfg(feature = "real-embeddings")]
struct EvaluationResults {
    e6_mrr: f64,
    e1_mrr: f64,
    e13_mrr: f64,
    avg_active_terms: f64,
    sparsity_ratio: f64,
    query_count: usize,
    per_topic_mrr: HashMap<String, f64>,
}

#[cfg(feature = "real-embeddings")]
fn evaluate_e6_sparse(
    dataset: &RealDataset,
    embedded: &EmbeddedDataset,
    num_queries: usize,
    seed: u64,
) -> EvaluationResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Sample query chunks
    let mut query_indices: Vec<usize> = (0..dataset.chunks.len()).collect();
    query_indices.shuffle(&mut rng);
    let query_indices: Vec<usize> = query_indices.into_iter().take(num_queries).collect();

    let mut e6_mrr_sum = 0.0;
    let mut e1_mrr_sum = 0.0;
    let mut e13_mrr_sum = 0.0;
    let mut query_count = 0;
    let mut topic_mrr_sums: HashMap<String, (f64, usize)> = HashMap::new();

    // Sparsity analysis
    let mut total_active_terms = 0usize;
    let mut total_vectors = 0usize;
    let vocab_size = 30522; // BERT vocab size

    // Pre-compute all embeddings for fast lookup
    let emb_ids: Vec<Uuid> = embedded.fingerprints.keys().copied().collect();

    for &query_idx in &query_indices {
        let query_chunk = &dataset.chunks[query_idx];
        let query_uuid = query_chunk.uuid();
        let query_topic = dataset.get_topic_idx(query_chunk);
        let query_topic_name = query_chunk.topic_hint.clone();
        let query_doc_id = &query_chunk.doc_id;

        let Some(query_fp) = embedded.fingerprints.get(&query_uuid) else { continue };

        // Skip if E6 sparse is empty
        if query_fp.e6_sparse.indices.is_empty() { continue; }

        // Track sparsity
        total_active_terms += query_fp.e6_sparse.indices.len();
        total_vectors += 1;

        // Exclude same-document chunks
        let same_doc_uuids: HashSet<Uuid> = embedded.chunk_info.iter()
            .filter(|(_, info)| &info.doc_id == query_doc_id)
            .map(|(id, _)| *id)
            .collect();

        // E6 Sparse scoring
        let mut e6_scores: Vec<(Uuid, f32)> = embedded.fingerprints.iter()
            .filter(|(id, _)| !same_doc_uuids.contains(id))
            .map(|(id, fp)| {
                let sim = query_fp.e6_sparse.dot(&fp.e6_sparse);
                (*id, sim)
            })
            .collect();
        e6_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // E1 Semantic scoring (baseline)
        let mut e1_scores: Vec<(Uuid, f32)> = embedded.fingerprints.iter()
            .filter(|(id, _)| !same_doc_uuids.contains(id))
            .map(|(id, fp)| {
                let sim = cosine_similarity(&query_fp.e1_semantic, &fp.e1_semantic);
                (*id, sim)
            })
            .collect();
        e1_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // E13 SPLADE scoring (comparison)
        let mut e13_scores: Vec<(Uuid, f32)> = embedded.fingerprints.iter()
            .filter(|(id, _)| !same_doc_uuids.contains(id))
            .map(|(id, fp)| {
                let sim = query_fp.e13_splade.dot(&fp.e13_splade);
                (*id, sim)
            })
            .collect();
        e13_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Find relevant documents (same topic, different document)
        let relevant: Vec<Uuid> = embedded.topic_assignments.iter()
            .filter(|(id, topic)| **topic == query_topic && !same_doc_uuids.contains(id))
            .map(|(id, _)| *id)
            .collect();

        if relevant.is_empty() { continue; }

        // Compute MRR@10 for each embedder
        if let Some(pos) = e6_scores.iter().take(10).position(|(id, _)| relevant.contains(id)) {
            e6_mrr_sum += 1.0 / (pos + 1) as f64;
        }
        if let Some(pos) = e1_scores.iter().take(10).position(|(id, _)| relevant.contains(id)) {
            e1_mrr_sum += 1.0 / (pos + 1) as f64;
        }
        if let Some(pos) = e13_scores.iter().take(10).position(|(id, _)| relevant.contains(id)) {
            e13_mrr_sum += 1.0 / (pos + 1) as f64;
        }

        // Per-topic tracking
        let entry = topic_mrr_sums.entry(query_topic_name).or_insert((0.0, 0));
        if let Some(pos) = e6_scores.iter().take(10).position(|(id, _)| relevant.contains(id)) {
            entry.0 += 1.0 / (pos + 1) as f64;
        }
        entry.1 += 1;

        query_count += 1;
    }

    let avg_active_terms = if total_vectors > 0 {
        total_active_terms as f64 / total_vectors as f64
    } else {
        0.0
    };

    let sparsity_ratio = if total_vectors > 0 {
        1.0 - (avg_active_terms / vocab_size as f64)
    } else {
        0.0
    };

    let per_topic_mrr: HashMap<String, f64> = topic_mrr_sums.into_iter()
        .map(|(topic, (sum, count))| (topic, if count > 0 { sum / count as f64 } else { 0.0 }))
        .collect();

    EvaluationResults {
        e6_mrr: if query_count > 0 { e6_mrr_sum / query_count as f64 } else { 0.0 },
        e1_mrr: if query_count > 0 { e1_mrr_sum / query_count as f64 } else { 0.0 },
        e13_mrr: if query_count > 0 { e13_mrr_sum / query_count as f64 } else { 0.0 },
        avg_active_terms,
        sparsity_ratio,
        query_count,
        per_topic_mrr,
    }
}

#[cfg(feature = "real-embeddings")]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a < 1e-10 || norm_b < 1e-10 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

fn run_synthetic_benchmark(args: &Args) -> Result<()> {
    use context_graph_benchmark::datasets::sparse::{E6SparseDatasetConfig, KeywordDomain};
    use context_graph_benchmark::runners::sparse::{E6SparseBenchmarkConfig, E6SparseBenchmarkRunner};

    info!("Running with synthetic dataset (no real embeddings)");
    info!("  Documents: {}", args.num_documents);
    info!("  Queries: {}", args.num_queries);
    info!("");

    let config = E6SparseBenchmarkConfig {
        dataset: E6SparseDatasetConfig {
            num_documents: args.num_documents,
            num_queries: args.num_queries,
            keyword_domains: KeywordDomain::all(),
            seed: args.seed,
            anti_example_ratio: 0.3,
        },
        k_values: vec![1, 5, 10, 20],
        run_ablation: true,
        run_threshold_sweep: args.threshold_sweep,
        threshold_values: vec![0.005, 0.01, 0.02, 0.05, 0.1],
        simulate_embeddings: true,
    };

    let runner = E6SparseBenchmarkRunner::new(config);
    let results = runner.run();

    println!("{}", results.summary());

    // Save results
    fs::create_dir_all(&args.output_dir)?;
    let json_path = args.output_dir.join("sparse_synthetic_benchmark.json");
    let json = serde_json::to_string_pretty(&results)?;
    fs::write(&json_path, json)?;
    info!("Results saved to: {}", json_path.display());

    Ok(())
}

fn print_results(results: &E6RealBenchmarkResults) {
    println!();
    println!("=======================================================================");
    println!("  E6 SPARSE BENCHMARK RESULTS (REAL DATA)");
    println!("=======================================================================");
    println!();
    println!("## Retrieval Quality (MRR@10)");
    println!("  E6 Sparse:    {:.4}", results.e6_mrr_at_10);
    println!("  E1 Semantic:  {:.4} (baseline)", results.e1_mrr_at_10);
    println!("  E13 SPLADE:   {:.4}", results.e13_mrr_at_10);
    println!();
    println!("## E6 Performance vs Baselines");
    println!("  E6 vs E1:     {:+.4} ({:+.2}%)",
        results.e6_vs_e1_delta,
        if results.e1_mrr_at_10 > 0.0 { results.e6_vs_e1_delta / results.e1_mrr_at_10 * 100.0 } else { 0.0 });
    println!("  E6 vs E13:    {:+.4} ({:+.2}%)",
        results.e6_vs_e13_delta,
        if results.e13_mrr_at_10 > 0.0 { results.e6_vs_e13_delta / results.e13_mrr_at_10 * 100.0 } else { 0.0 });
    println!();
    println!("## Sparsity Analysis");
    println!("  Avg Active Terms: {:.1}", results.avg_active_terms);
    println!("  Sparsity Ratio:   {:.4} ({:.2}% zeros)",
        results.sparsity_ratio, results.sparsity_ratio * 100.0);
    println!("  Vocabulary Size:  30522 (BERT)");
    println!();
    println!("## Dataset Statistics");
    println!("  Corpus Size:  {} documents", results.corpus_size);
    println!("  Query Count:  {} queries", results.query_count);
    println!();
    println!("## Per-Topic MRR@10");
    let mut topics: Vec<_> = results.per_topic_mrr.iter().collect();
    topics.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
    for (topic, mrr) in topics.iter().take(10) {
        println!("  {}: {:.4}", topic, mrr);
    }
    if topics.len() > 10 {
        println!("  ... and {} more topics", topics.len() - 10);
    }
    println!();
    println!("## Performance");
    println!("  Data Load:    {}ms", results.timings.data_load_ms);
    println!("  Embedding:    {}ms", results.timings.embedding_ms);
    println!("  Evaluation:   {}ms", results.timings.evaluation_ms);
    println!("  Total:        {}ms", results.timings.total_ms);
    println!();

    // Target evaluation
    let e6_target = results.e6_mrr_at_10 >= 0.50;
    let delta_target = results.e6_vs_e1_delta > 0.0;
    let sparsity_target = results.sparsity_ratio > 0.95;

    println!("## Target Evaluation");
    println!("  E6 MRR@10 >= 0.50:     {} ({:.4})",
        if e6_target { "PASS" } else { "FAIL" }, results.e6_mrr_at_10);
    println!("  E6 > E1 (delta > 0):   {} ({:+.4})",
        if delta_target { "PASS" } else { "FAIL" }, results.e6_vs_e1_delta);
    println!("  Sparsity > 0.95:       {} ({:.4})",
        if sparsity_target { "PASS" } else { "FAIL" }, results.sparsity_ratio);
    println!();

    let all_pass = e6_target && delta_target && sparsity_target;
    println!("=======================================================================");
    if all_pass {
        println!("  OVERALL: PASS - All targets met");
    } else {
        println!("  OVERALL: NEEDS ATTENTION - Some targets not met");
    }
    println!("=======================================================================");
}

fn save_results(results: &E6RealBenchmarkResults, output_dir: &PathBuf) -> Result<()> {
    fs::create_dir_all(output_dir)?;

    // Save JSON
    let json_path = output_dir.join("e6_sparse_benchmark.json");
    let json = serde_json::to_string_pretty(results)?;
    fs::write(&json_path, &json)?;
    info!("JSON results saved to: {}", json_path.display());

    // Save Markdown report
    let md_path = output_dir.join("e6_sparse_benchmark_report.md");
    let report = generate_markdown_report(results);
    fs::write(&md_path, &report)?;
    info!("Markdown report saved to: {}", md_path.display());

    Ok(())
}

fn generate_markdown_report(results: &E6RealBenchmarkResults) -> String {
    let mut report = String::new();

    report.push_str("# E6 Sparse Embedder Benchmark Report\n\n");
    report.push_str(&format!("Generated: {}\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

    report.push_str("## Overview\n\n");
    report.push_str("This benchmark evaluates the E6 (V_selectivity) sparse embedder using real Wikipedia data.\n");
    report.push_str("E6 uses keyword-based sparse representations for exact term matching.\n\n");

    report.push_str("## Summary\n\n");
    report.push_str("| Metric | Value | Target | Status |\n");
    report.push_str("|--------|-------|--------|--------|\n");
    report.push_str(&format!("| E6 MRR@10 | {:.4} | >= 0.50 | {} |\n",
        results.e6_mrr_at_10,
        if results.e6_mrr_at_10 >= 0.50 { "PASS" } else { "FAIL" }));
    report.push_str(&format!("| E6 vs E1 Delta | {:+.4} | > 0.00 | {} |\n",
        results.e6_vs_e1_delta,
        if results.e6_vs_e1_delta > 0.0 { "PASS" } else { "FAIL" }));
    report.push_str(&format!("| Sparsity | {:.4} | > 0.95 | {} |\n\n",
        results.sparsity_ratio,
        if results.sparsity_ratio > 0.95 { "PASS" } else { "FAIL" }));

    report.push_str("## Retrieval Quality\n\n");
    report.push_str("| Embedder | MRR@10 | Notes |\n");
    report.push_str("|----------|--------|-------|\n");
    report.push_str(&format!("| E6 Sparse | {:.4} | Keyword-based sparse |\n", results.e6_mrr_at_10));
    report.push_str(&format!("| E1 Semantic | {:.4} | Dense baseline |\n", results.e1_mrr_at_10));
    report.push_str(&format!("| E13 SPLADE | {:.4} | Learned sparse |\n\n", results.e13_mrr_at_10));

    report.push_str("## Sparsity Analysis\n\n");
    report.push_str(&format!("- **Average Active Terms**: {:.1}\n", results.avg_active_terms));
    report.push_str(&format!("- **Sparsity Ratio**: {:.4} ({:.2}% zeros)\n",
        results.sparsity_ratio, results.sparsity_ratio * 100.0));
    report.push_str("- **Vocabulary Size**: 30522 (BERT tokenizer)\n\n");

    report.push_str("## Dataset\n\n");
    report.push_str(&format!("- **Corpus Size**: {} documents\n", results.corpus_size));
    report.push_str(&format!("- **Query Count**: {} queries\n", results.query_count));
    report.push_str(&format!("- **Topics**: {} unique topics\n\n", results.per_topic_mrr.len()));

    report.push_str("## Performance\n\n");
    report.push_str(&format!("- **Data Loading**: {}ms\n", results.timings.data_load_ms));
    report.push_str(&format!("- **Embedding**: {}ms\n", results.timings.embedding_ms));
    report.push_str(&format!("- **Evaluation**: {}ms\n", results.timings.evaluation_ms));
    report.push_str(&format!("- **Total**: {}ms\n\n", results.timings.total_ms));

    if !results.per_topic_mrr.is_empty() {
        report.push_str("## Per-Topic Performance\n\n");
        report.push_str("| Topic | MRR@10 |\n");
        report.push_str("|-------|--------|\n");
        let mut topics: Vec<_> = results.per_topic_mrr.iter().collect();
        topics.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
        for (topic, mrr) in topics.iter().take(15) {
            report.push_str(&format!("| {} | {:.4} |\n", topic, mrr));
        }
        report.push_str("\n");
    }

    report
}
