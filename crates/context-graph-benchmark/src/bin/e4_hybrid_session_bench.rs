//! E4 Hybrid Session Clustering Benchmark CLI.
//!
//! This binary runs a comprehensive benchmark of the E4 hybrid session+position
//! encoding implementation using real data from the HuggingFace benchmark dataset.
//!
//! ## Usage
//!
//! ```bash
//! # Quick test with limited data
//! cargo run -p context-graph-benchmark --bin e4-hybrid-session-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark/temp_wikipedia \
//!     --max-chunks 500 \
//!     --num-sessions 20
//!
//! # Full benchmark
//! cargo run -p context-graph-benchmark --bin e4-hybrid-session-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark/temp_wikipedia \
//!     --num-sessions 100 \
//!     --output benchmark_results/e4_hybrid_session.json
//! ```
//!
//! ## Targets
//!
//! - Session separation ratio: ≥ 2.0x
//! - Intra-session ordering: ≥ 80%
//! - Before/after symmetry: ≥ 0.8
//! - Hybrid vs legacy improvement: ≥ +10%
//!
//! ## Note
//!
//! This benchmark requires `--features real-embeddings` and a CUDA GPU.
//! Unlike other benchmarks, there is no synthetic fallback mode because
//! the E4 hybrid encoding requires real session metadata to be meaningful.

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[cfg(feature = "real-embeddings")]
use std::collections::HashMap;
#[cfg(feature = "real-embeddings")]
use std::fs::{self, File};
#[cfg(feature = "real-embeddings")]
use std::io::Write;
#[cfg(feature = "real-embeddings")]
use std::time::Instant;
#[cfg(feature = "real-embeddings")]
use tracing::{info, warn};
#[cfg(feature = "real-embeddings")]
use uuid::Uuid;

#[cfg(feature = "real-embeddings")]
use context_graph_benchmark::datasets::temporal_sessions::{
    SessionGenerator, SessionGeneratorConfig,
};
#[cfg(feature = "real-embeddings")]
use context_graph_benchmark::runners::e4_hybrid_session::{
    E4HybridSessionBenchmarkConfig, E4HybridSessionBenchmarkResults, E4HybridSessionBenchmarkRunner,
};
#[cfg(feature = "real-embeddings")]
use context_graph_benchmark::realdata::{
    embedder::RealDataEmbedder,
    loader::DatasetLoader,
};
#[cfg(feature = "real-embeddings")]
use context_graph_core::types::fingerprint::SemanticFingerprint;

/// E4 Hybrid Session Clustering Benchmark CLI.
#[derive(Parser, Debug)]
#[command(name = "e4-hybrid-session-bench")]
#[command(about = "Benchmark E4 hybrid session+position encoding using real data")]
struct Args {
    /// Data directory containing chunks.jsonl and metadata.json.
    #[arg(long, default_value = "data/hf_benchmark/temp_wikipedia")]
    data_dir: PathBuf,

    /// Output path for benchmark results JSON.
    #[arg(long, default_value = "benchmark_results/e4_hybrid_session.json")]
    output: PathBuf,

    /// Maximum chunks to load (0 = unlimited).
    #[arg(long, default_value = "0")]
    max_chunks: usize,

    /// Number of sessions to generate.
    #[arg(long, default_value = "100")]
    num_sessions: usize,

    /// Maximum chunks per session.
    #[arg(long, default_value = "20")]
    chunks_per_session: usize,

    /// Random seed for reproducibility.
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Run legacy E4 comparison benchmark.
    #[arg(long, default_value = "true")]
    run_legacy_comparison: bool,

    /// Run timestamp baseline benchmark.
    #[arg(long, default_value = "true")]
    run_timestamp_baseline: bool,

    /// Verbose output.
    #[arg(short, long)]
    verbose: bool,
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
    println!("  E4 HYBRID SESSION CLUSTERING BENCHMARK");
    println!("=======================================================================");
    println!();

    #[cfg(feature = "real-embeddings")]
    {
        run_real_data_benchmark(&args)
    }

    #[cfg(not(feature = "real-embeddings"))]
    {
        let _ = args; // Suppress unused warning
        eprintln!("ERROR: This benchmark requires --features real-embeddings");
        eprintln!();
        eprintln!("The E4 hybrid session+position encoding benchmark requires real");
        eprintln!("embeddings to produce meaningful results. Unlike other benchmarks,");
        eprintln!("synthetic embeddings cannot properly test session clustering because");
        eprintln!("the E4 encoder needs actual session metadata to create meaningful");
        eprintln!("session signatures.");
        eprintln!();
        eprintln!("Usage:");
        eprintln!("  cargo run -p context-graph-benchmark --bin e4-hybrid-session-bench \\");
        eprintln!("      --release --features real-embeddings -- \\");
        eprintln!("      --data-dir data/hf_benchmark/temp_wikipedia");
        eprintln!();
        std::process::exit(1);
    }
}

/// Run benchmark with real data (requires GPU).
#[cfg(feature = "real-embeddings")]
fn run_real_data_benchmark(args: &Args) -> Result<()> {
    let total_start = Instant::now();

    // Step 1: Load dataset
    info!("Step 1: Loading dataset from: {}", args.data_dir.display());
    let load_start = Instant::now();

    let loader = DatasetLoader::new().with_max_chunks(args.max_chunks);
    let dataset = loader.load_from_dir(&args.data_dir)?;

    let load_time = load_start.elapsed();
    info!("  Loaded {} chunks from {} topics", dataset.chunks.len(), dataset.topic_count());
    info!("  Load time: {:?}", load_time);

    if dataset.chunks.len() < args.num_sessions * 5 {
        warn!("  Warning: Dataset may be too small for {} sessions", args.num_sessions);
        warn!("  Consider using --max-chunks 0 to load all data");
    }

    // Step 2: Generate sessions from topically related chunks
    info!("");
    info!("Step 2: Generating temporal sessions...");
    let session_start = Instant::now();

    let session_config = SessionGeneratorConfig {
        min_session_length: 5,
        max_session_length: args.chunks_per_session,
        num_sessions: args.num_sessions,
        coherence_threshold: 0.5,
        seed: args.seed,
        ..Default::default()
    };

    let mut generator = SessionGenerator::new(session_config.clone());
    let ground_truth = generator.generate(&dataset);

    let session_time = session_start.elapsed();
    info!(
        "  Generated {} sessions with {} total chunks",
        ground_truth.sessions.len(),
        ground_truth.stats.total_session_chunks
    );
    info!(
        "  Session lengths: min={}, max={}, avg={:.1}",
        ground_truth.stats.min_session_length,
        ground_truth.stats.max_session_length,
        ground_truth.stats.avg_session_length
    );
    info!("  Session generation time: {:?}", session_time);

    if ground_truth.sessions.is_empty() {
        warn!("  ERROR: No sessions generated! Check data directory and settings.");
        std::process::exit(1);
    }

    // Step 3: Embed chunks with E4 session-aware encoding
    info!("");
    info!("Step 3: Generating embeddings with E4 session metadata (this may take a while)...");
    let embed_start = Instant::now();

    let embedder = RealDataEmbedder::default();

    info!("  Embedding {} chunks across {} sessions...",
        ground_truth.stats.total_session_chunks,
        ground_truth.sessions.len()
    );

    // Use embed_sessions_batched which passes session_id and sequence_position to E4
    let fingerprints: HashMap<Uuid, SemanticFingerprint> = tokio::runtime::Runtime::new()?
        .block_on(async {
            embedder
                .embed_sessions_batched(&ground_truth.sessions)
                .await
        })?;

    let embed_time = embed_start.elapsed();
    info!("  Embedded {} chunks", fingerprints.len());
    info!("  Embedding time: {:?}", embed_time);
    info!("  Rate: {:.1} chunks/sec", fingerprints.len() as f64 / embed_time.as_secs_f64());

    // Verify E4 embeddings are present and non-trivial
    verify_e4_embeddings(&fingerprints);

    // Step 4: Run benchmark
    info!("");
    info!("Step 4: Running benchmark suite...");
    let bench_start = Instant::now();

    let config = E4HybridSessionBenchmarkConfig {
        session_config,
        run_legacy_comparison: args.run_legacy_comparison,
        run_timestamp_baseline: args.run_timestamp_baseline,
        seed: args.seed,
        ..Default::default()
    };

    let runner = E4HybridSessionBenchmarkRunner::new(config);
    let results = runner.run(&ground_truth.sessions, &fingerprints);

    let bench_time = bench_start.elapsed();
    info!("  Benchmark time: {:?}", bench_time);

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
    info!("  Session generation:   {:?}", session_time);
    info!("  Embedding generation: {:?}", embed_time);
    info!("  Benchmark execution:  {:?}", bench_time);
    info!("  Total time:           {:?}", total_time);
    info!("");

    if results.all_targets_met() {
        info!("SUCCESS: ALL TARGETS MET");
        Ok(())
    } else {
        warn!("WARNING: SOME TARGETS NOT MET");
        // Still return Ok - the benchmark ran successfully, just didn't meet all targets
        Ok(())
    }
}

/// Verify E4 embeddings are present and have meaningful variation.
#[cfg(feature = "real-embeddings")]
fn verify_e4_embeddings(fingerprints: &HashMap<Uuid, SemanticFingerprint>) {
    let total = fingerprints.len();
    if total == 0 {
        warn!("  WARNING: No fingerprints generated!");
        return;
    }

    // Check E4 embedding presence
    let e4_present = fingerprints.values()
        .filter(|fp| !fp.e4_temporal_positional.is_empty())
        .count();

    info!("  E4 embedding coverage: {}/{} ({:.1}%)",
        e4_present, total, e4_present as f64 / total as f64 * 100.0);

    if e4_present < total {
        warn!("  WARNING: Some chunks missing E4 embeddings!");
    }

    // Check for trivial embeddings (all zeros or all same)
    let sample_embeddings: Vec<_> = fingerprints.values()
        .take(10)
        .filter(|fp| !fp.e4_temporal_positional.is_empty())
        .map(|fp| &fp.e4_temporal_positional)
        .collect();

    if sample_embeddings.len() >= 2 {
        // Check if first two are identical (would indicate broken embedding)
        let sim = cosine_similarity(&sample_embeddings[0], &sample_embeddings[1]);
        if sim > 0.999 {
            warn!("  WARNING: Sample E4 embeddings are nearly identical (sim={:.4})", sim);
            warn!("  This suggests E4 encoder may not be using session metadata properly.");
        } else {
            info!("  E4 embedding variation verified (sample sim={:.4})", sim);
        }
    }
}

/// Compute cosine similarity between two vectors.
#[cfg(feature = "real-embeddings")]
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Print benchmark results to console.
#[cfg(feature = "real-embeddings")]
fn print_results(results: &E4HybridSessionBenchmarkResults) {
    info!("");
    info!("=======================================================================");
    info!("  E4 HYBRID SESSION BENCHMARK RESULTS");
    info!("=======================================================================");
    info!("");

    // Session Clustering
    info!("SESSION CLUSTERING:");
    let sep = results.metrics.clustering.session_separation_ratio;
    let sep_status = if sep >= 2.0 { "PASS" } else { "FAIL" };
    info!(
        "  [{}] Separation ratio:       {:.2}x (target: >= 2.0x)",
        sep_status, sep
    );
    info!(
        "       Intra-session sim:      {:.3}",
        results.metrics.clustering.intra_session_similarity
    );
    info!(
        "       Inter-session sim:      {:.3}",
        results.metrics.clustering.inter_session_similarity
    );
    info!(
        "       Silhouette score:       {:.3}",
        results.metrics.clustering.silhouette_score
    );
    info!(
        "       Boundary F1:            {:.3}",
        results.metrics.clustering.boundary_f1
    );
    info!("");

    // Intra-Session Ordering
    info!("INTRA-SESSION ORDERING:");
    let ord = results.metrics.ordering.ordering_accuracy;
    let ord_status = if ord >= 0.80 { "PASS" } else { "FAIL" };
    info!(
        "  [{}] Ordering accuracy:      {:.1}% (target: >= 80%)",
        ord_status,
        ord * 100.0
    );

    let sym = results.metrics.ordering.symmetry_score;
    let sym_status = if sym >= 0.80 { "PASS" } else { "FAIL" };
    info!(
        "  [{}] Before/after symmetry:  {:.2} (target: >= 0.8)",
        sym_status, sym
    );
    info!(
        "       Kendall's tau:          {:.3}",
        results.metrics.ordering.kendalls_tau
    );
    info!(
        "       Before accuracy:        {:.1}%",
        results.metrics.ordering.before_accuracy * 100.0
    );
    info!(
        "       After accuracy:         {:.1}%",
        results.metrics.ordering.after_accuracy * 100.0
    );
    info!(
        "       Position MRR:           {:.3}",
        results.metrics.ordering.position_mrr
    );
    info!("");

    // Hybrid Effectiveness
    info!("HYBRID EFFECTIVENESS:");
    let imp = results.metrics.hybrid_effectiveness.vs_legacy_improvement;
    let imp_status = if imp >= 0.10 { "PASS" } else { "FAIL" };
    info!(
        "  [{}] vs Legacy improvement:  {:+.1}% (target: >= +10%)",
        imp_status,
        imp * 100.0
    );
    info!(
        "       vs Timestamp improvement: {:+.1}%",
        results.metrics.hybrid_effectiveness.vs_timestamp_improvement * 100.0
    );

    if let Some(legacy) = &results.legacy_comparison {
        info!(
            "       Legacy separation ratio:  {:.2}x",
            legacy.clustering.session_separation_ratio
        );
    }
    if let Some(timestamp) = &results.timestamp_baseline {
        info!(
            "       Timestamp separation:     {:.2}x",
            timestamp.clustering.session_separation_ratio
        );
    }
    info!("");

    // Composite Score
    info!("COMPOSITE SCORE:");
    info!(
        "       Overall score:          {:.3}",
        results.metrics.composite.overall_score
    );
    info!(
        "       Clustering score:       {:.3}",
        results.metrics.composite.clustering_score
    );
    info!(
        "       Ordering score:         {:.3}",
        results.metrics.composite.ordering_score
    );
    info!(
        "       Effectiveness score:    {:.3}",
        results.metrics.composite.effectiveness_score
    );
    info!("");

    // Validation Summary
    info!("VALIDATION SUMMARY:");
    for check in &results.validation.checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        info!(
            "  [{}] {}: {} ({})",
            status, check.description, check.actual, check.expected
        );
    }
    info!("");
    info!(
        "  Targets met: {}/{}",
        results.validation.checks_passed, results.validation.checks_total
    );
    info!("");

    // Dataset Stats
    info!("DATASET STATISTICS:");
    info!("       Sessions:               {}", results.dataset_stats.num_sessions);
    info!("       Total chunks:           {}", results.dataset_stats.num_chunks);
    info!("       Chunks with embeddings: {}", results.dataset_stats.num_chunks_with_embeddings);
    info!("       Avg session length:     {:.1}", results.dataset_stats.avg_session_length);
    info!("       Topics:                 {}", results.dataset_stats.num_topics);
}

/// Save results to JSON file.
#[cfg(feature = "real-embeddings")]
fn save_results(results: &E4HybridSessionBenchmarkResults, output_path: &PathBuf) -> Result<()> {
    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(results)?;
    let mut file = File::create(output_path)?;
    file.write_all(json.as_bytes())?;

    info!("Results saved to: {}", output_path.display());

    Ok(())
}
