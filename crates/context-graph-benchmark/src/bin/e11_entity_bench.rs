//! E11 Entity Embedder Benchmark CLI.
//!
//! This binary runs a comprehensive benchmark of the E11 entity embedder
//! (all-MiniLM-L6-v2, 384D) for entity-based retrieval and TransE operations.
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark (GPU required)
//! cargo run -p context-graph-benchmark --bin e11-entity-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark_diverse \
//!     --output benchmark_results/e11_entity.json
//!
//! # Entity extraction only
//! cargo run -p context-graph-benchmark --bin e11-entity-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark_diverse \
//!     --benchmark extraction
//!
//! # TransE validation only
//! cargo run -p context-graph-benchmark --bin e11-entity-bench --release \
//!     --features real-embeddings -- \
//!     --data-dir data/hf_benchmark_diverse \
//!     --benchmark transe
//! ```
//!
//! ## Targets (from CLAUDE.md)
//!
//! - Entity extraction F1: >= 0.85
//! - TransE valid triple score: > -0.5
//! - TransE invalid triple score: < -1.5
//! - E11 contribution (vs E1-only): >= +5%
//! - Entity search latency: < 100ms
//!
//! ## Benchmark Phases
//!
//! - extraction: Entity extraction with canonicalization
//! - retrieval: E1-only vs E11-only vs E1+E11 hybrid comparison
//! - transe: TransE relationship inference and scoring
//! - validation: Knowledge validation with threshold optimization
//! - graph: Entity co-occurrence graph construction
//! - all: Run all phases (default)

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

/// Benchmark phases to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BenchmarkPhase {
    /// Entity extraction benchmark only.
    Extraction,
    /// Entity-based retrieval comparison.
    Retrieval,
    /// TransE relationship inference.
    TransE,
    /// Knowledge validation benchmark.
    Validation,
    /// Entity graph construction.
    Graph,
    /// All phases (default).
    All,
}

/// E11 Entity Embedder Benchmark CLI.
#[derive(Parser, Debug)]
#[command(name = "e11-entity-bench")]
#[command(about = "Benchmark E11 entity embedder (all-MiniLM-L6-v2, 384D)")]
struct Args {
    /// Data directory containing chunks.jsonl and metadata.json.
    #[arg(long, default_value = "data/hf_benchmark_diverse")]
    data_dir: PathBuf,

    /// Output path for benchmark results JSON.
    #[arg(long, default_value = "benchmark_results/e11_entity.json")]
    output: PathBuf,

    /// Maximum chunks to load from real data (0 = unlimited).
    #[arg(long, default_value = "1000")]
    max_chunks: usize,

    /// Number of queries for retrieval benchmark.
    #[arg(long, default_value = "100")]
    num_queries: usize,

    /// Benchmark phase to run.
    #[arg(long, value_enum, default_value = "all")]
    benchmark: BenchmarkPhase,

    /// Random seed for reproducibility.
    #[arg(long, default_value = "42")]
    seed: u64,

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
    println!("  E11 ENTITY EMBEDDER BENCHMARK");
    println!("  Model: all-MiniLM-L6-v2 (384D)");
    println!("=======================================================================");
    println!();
    println!("ARCH-12: E1 is THE semantic foundation, E11 enhances with entity facts");
    println!("ARCH-20: E11 SHOULD use entity linking for disambiguation");
    println!("E11 is RELATIONAL_ENHANCER with topic_weight 0.5");
    println!();

    #[cfg(feature = "real-embeddings")]
    {
        run_benchmark(&args)
    }

    #[cfg(not(feature = "real-embeddings"))]
    {
        eprintln!("ERROR: This benchmark requires --features real-embeddings");
        eprintln!();
        eprintln!("Usage:");
        eprintln!("  cargo run -p context-graph-benchmark --bin e11-entity-bench \\");
        eprintln!("      --release --features real-embeddings -- \\");
        eprintln!("      --data-dir data/hf_benchmark_diverse");
        eprintln!();
        std::process::exit(1);
    }
}

/// Run the E11 entity benchmark.
#[cfg(feature = "real-embeddings")]
fn run_benchmark(args: &Args) -> Result<()> {
    use std::fs::{self, File};
    use std::io::Write;
    use std::time::Instant;
    use tracing::{info, warn};

    use context_graph_benchmark::runners::e11_entity::{
        E11EntityBenchmarkConfig, E11EntityBenchmarkResults, E11EntityBenchmarkRunner,
    };

    let total_start = Instant::now();

    info!("Running E11 ENTITY benchmark...");
    info!("  Data directory: {}", args.data_dir.display());
    info!("  Max chunks: {}", args.max_chunks);
    info!("  Num queries: {}", args.num_queries);
    info!("  Benchmark: {:?}", args.benchmark);
    info!("");

    // Configure which benchmarks to run
    let (run_extraction, run_retrieval, run_transe, run_validation, run_graph) = match args.benchmark {
        BenchmarkPhase::Extraction => (true, false, false, false, false),
        BenchmarkPhase::Retrieval => (false, true, false, false, false),
        BenchmarkPhase::TransE => (false, false, true, false, false),
        BenchmarkPhase::Validation => (false, false, false, true, false),
        BenchmarkPhase::Graph => (false, false, false, false, true),
        BenchmarkPhase::All => (true, true, true, true, true),
    };

    // Build benchmark config
    let config = E11EntityBenchmarkConfig {
        max_chunks: args.max_chunks,
        num_queries: args.num_queries,
        seed: args.seed,
        run_all: matches!(args.benchmark, BenchmarkPhase::All),
        run_extraction,
        run_retrieval,
        run_transe,
        run_validation,
        run_graph,
        output_path: Some(args.output.to_string_lossy().to_string()),
        ..Default::default()
    };

    // Create and run benchmark
    let mut runner = E11EntityBenchmarkRunner::new(config);
    let data_dir = args.data_dir.to_string_lossy().to_string();

    let results = tokio::runtime::Runtime::new()?
        .block_on(async { runner.run(&data_dir).await })
        .map_err(|e| anyhow::anyhow!("Benchmark failed: {}", e))?;

    let total_time = total_start.elapsed();

    // Print results
    print_results(&results);

    // Print timing summary
    println!();
    println!("=======================================================================");
    println!("  TIMING SUMMARY");
    println!("=======================================================================");
    println!("  Dataset loading:      {}ms", results.timings.dataset_load_ms);
    println!("  Provider init:        {}ms", results.timings.embedding_ms);
    if results.timings.extraction_benchmark_ms > 0 {
        println!("  Extraction benchmark: {}ms", results.timings.extraction_benchmark_ms);
    }
    if results.timings.retrieval_benchmark_ms > 0 {
        println!("  Retrieval benchmark:  {}ms", results.timings.retrieval_benchmark_ms);
    }
    if results.timings.transe_benchmark_ms > 0 {
        println!("  TransE benchmark:     {}ms", results.timings.transe_benchmark_ms);
    }
    if results.timings.validation_benchmark_ms > 0 {
        println!("  Validation benchmark: {}ms", results.timings.validation_benchmark_ms);
    }
    if results.timings.graph_benchmark_ms > 0 {
        println!("  Graph benchmark:      {}ms", results.timings.graph_benchmark_ms);
    }
    println!("  Total:                {:?}", total_time);
    println!();

    // Save results
    save_results(&results, &args.output)?;

    if results.all_targets_met {
        info!("SUCCESS: ALL TARGETS MET");
        Ok(())
    } else {
        warn!("WARNING: SOME TARGETS NOT MET");
        Ok(())
    }
}

/// Print benchmark results to console.
#[cfg(feature = "real-embeddings")]
fn print_results(results: &context_graph_benchmark::runners::e11_entity::E11EntityBenchmarkResults) {
    use context_graph_benchmark::metrics::e11_entity::thresholds;

    println!();
    println!("=======================================================================");
    println!("  E11 ENTITY BENCHMARK RESULTS");
    println!("=======================================================================");
    println!();

    // Dataset stats
    println!("DATASET STATISTICS:");
    println!("  Documents:          {}", results.dataset_stats.num_documents);
    println!("  Docs with entities: {}", results.dataset_stats.docs_with_entities);
    println!("  Total entities:     {}", results.dataset_stats.total_entities);
    println!("  Unique entities:    {}", results.dataset_stats.unique_entities);
    println!("  Avg entities/doc:   {:.2}", results.dataset_stats.avg_entities_per_doc);
    println!("  Valid triples:      {}", results.dataset_stats.num_valid_triples);
    println!("  Invalid triples:    {}", results.dataset_stats.num_invalid_triples);
    println!("  Entity pairs:       {}", results.dataset_stats.num_entity_pairs);
    println!();

    // Extraction metrics
    if let Some(ref extraction) = results.extraction_results {
        println!("ENTITY EXTRACTION:");
        let f1_status = if extraction.metrics.f1_score >= thresholds::EXTRACTION_F1_MIN { "PASS" } else { "FAIL" };
        println!("  [{}] F1 Score:          {:.3} (target: >= {:.2})", f1_status, extraction.metrics.f1_score, thresholds::EXTRACTION_F1_MIN);
        println!("       Precision:         {:.3}", extraction.metrics.precision);
        println!("       Recall:            {:.3}", extraction.metrics.recall);
        let canon_status = if extraction.metrics.canonicalization_accuracy >= thresholds::CANONICALIZATION_ACCURACY_MIN { "PASS" } else { "FAIL" };
        println!("  [{}] Canonicalization:  {:.3} (target: >= {:.2})", canon_status, extraction.metrics.canonicalization_accuracy, thresholds::CANONICALIZATION_ACCURACY_MIN);
        println!();

        if !extraction.sample_extractions.is_empty() {
            println!("  Sample Extractions:");
            for sample in extraction.sample_extractions.iter().take(3) {
                println!("    \"{}\"", sample.text_snippet);
                println!("      Predicted: {:?}", sample.predicted_entities);
                println!("      Actual:    {:?}", sample.ground_truth_entities);
                println!();
            }
        }
    }

    // Retrieval metrics
    if let Some(ref retrieval) = results.retrieval_results {
        println!("ENTITY-BASED RETRIEVAL:");
        println!("       MRR (E1-only):     {:.3}", retrieval.metrics.mrr_e1_only);
        println!("       MRR (E11-only):    {:.3}", retrieval.metrics.mrr_e11_only);
        println!("       MRR (E1+E11):      {:.3}", retrieval.metrics.mrr_e1_e11_hybrid);
        let contrib_status = if retrieval.metrics.e11_contribution_pct >= thresholds::E11_CONTRIBUTION_PCT_MIN { "PASS" } else { "FAIL" };
        println!("  [{}] E11 Contribution:  {:.1}% (target: >= {:.0}%)", contrib_status, retrieval.metrics.e11_contribution_pct, thresholds::E11_CONTRIBUTION_PCT_MIN);
        println!("       Queries evaluated: {}", retrieval.metrics.queries_evaluated);
        println!();
    }

    // TransE metrics
    if let Some(ref transe) = results.transe_results {
        println!("TransE RELATIONSHIP INFERENCE:");
        let valid_status = if transe.metrics.valid_triple_avg_score > thresholds::TRANSE_VALID_SCORE_MIN { "PASS" } else { "FAIL" };
        println!("  [{}] Valid triple avg:    {:.3} (target: > {:.1})", valid_status, transe.metrics.valid_triple_avg_score, thresholds::TRANSE_VALID_SCORE_MIN);
        let invalid_status = if transe.metrics.invalid_triple_avg_score < thresholds::TRANSE_INVALID_SCORE_MAX { "PASS" } else { "FAIL" };
        println!("  [{}] Invalid triple avg:  {:.3} (target: < {:.1})", invalid_status, transe.metrics.invalid_triple_avg_score, thresholds::TRANSE_INVALID_SCORE_MAX);
        println!("       Separation score:    {:.3}", transe.metrics.separation_score);
        let mrr_status = if transe.metrics.relationship_inference_mrr >= thresholds::RELATION_INFERENCE_MRR_MIN { "PASS" } else { "FAIL" };
        println!("  [{}] Inference MRR:       {:.3} (target: >= {:.2})", mrr_status, transe.metrics.relationship_inference_mrr, thresholds::RELATION_INFERENCE_MRR_MIN);
        println!();

        if !transe.relationship_inferences.is_empty() {
            println!("  Sample Relationship Inferences:");
            for inf in transe.relationship_inferences.iter().take(5) {
                let expected = inf.expected_relation.as_deref().unwrap_or("unknown");
                let match_str = if inf.expected_relation.as_ref() == Some(&inf.predicted_relation) { "MATCH" } else { "MISMATCH" };
                println!("    {} -> {}: predicted='{}', expected='{}' [{}]",
                    inf.head, inf.tail, inf.predicted_relation, expected, match_str);
            }
            println!();
        }
    }

    // Validation metrics
    if let Some(ref validation) = results.validation_results {
        println!("KNOWLEDGE VALIDATION:");
        println!("       Accuracy:          {:.3}", validation.accuracy);
        println!("       Optimal threshold: {:.2}", validation.optimal_threshold);
        println!("       True positives:    {}", validation.confusion_matrix.true_positives);
        println!("       True negatives:    {}", validation.confusion_matrix.true_negatives);
        println!("       False positives:   {}", validation.confusion_matrix.false_positives);
        println!("       False negatives:   {}", validation.confusion_matrix.false_negatives);
        println!();
    }

    // Graph metrics
    if let Some(ref graph) = results.graph_results {
        println!("ENTITY GRAPH CONSTRUCTION:");
        println!("       Nodes discovered:  {}", graph.metrics.nodes_discovered);
        println!("       Edges inferred:    {}", graph.metrics.edges_inferred);
        println!("       Graph density:     {:.4}", graph.metrics.graph_density);
        println!("       Edge score mean:   {:.3}", graph.metrics.avg_edge_score);
        println!("       Edge score std:    {:.3}", graph.metrics.edge_score_std_dev);
        println!();

        if !graph.top_entities.is_empty() {
            println!("  Top Entities:");
            for (entity, count) in graph.top_entities.iter().take(5) {
                println!("    {} ({})", entity, count);
            }
            println!();
        }

        if !graph.top_relationships.is_empty() {
            println!("  Top Relationships:");
            for (e1, e2, rel, weight) in graph.top_relationships.iter().take(5) {
                println!("    {} --[{}]--> {} (weight: {:.3})", e1, rel, e2, weight);
            }
            println!();
        }
    }

    // Overall quality score
    println!("=======================================================================");
    println!("  OVERALL QUALITY");
    println!("=======================================================================");
    println!("  Quality score:      {:.3}", results.metrics.overall_quality_score());
    let status = if results.all_targets_met { "PASS" } else { "FAIL" };
    println!("  All targets met:    {} [{}]", results.all_targets_met, status);
    println!();
}

/// Save results to JSON file.
#[cfg(feature = "real-embeddings")]
fn save_results(
    results: &context_graph_benchmark::runners::e11_entity::E11EntityBenchmarkResults,
    output_path: &PathBuf,
) -> Result<()> {
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
