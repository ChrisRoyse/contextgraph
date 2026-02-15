#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! Unified Real Data Benchmark
//!
//! Comprehensive benchmark that evaluates all 13 embedders using the real
//! HuggingFace dataset at data/hf_benchmark_diverse/ (58,895 chunks, 85MB Wikipedia data).
//!
//! ## Features
//!
//! - Loads real dataset from chunks.jsonl + metadata.json
//! - Injects temporal metadata for E2/E3/E4 benchmarking
//! - Generates per-embedder ground truth
//! - Embeds with all 13 embedders (GPU, with checkpointing)
//! - Evaluates retrieval metrics (MRR, P@K, R@K, NDCG, MAP)
//! - Compares fusion strategies (E1Only, MultiSpace, Pipeline)
//! - Validates constitutional compliance
//! - Generates markdown report
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark on all 13 embedders
//! cargo run -p context-graph-benchmark --bin unified-realdata-bench --release \
//!   --features real-embeddings -- \
//!   --data-dir data/hf_benchmark_diverse \
//!   --max-chunks 10000 \
//!   --output benchmark_results/unified_realdata.json
//!
//! # Quick test with limited chunks
//! cargo run -p context-graph-benchmark --bin unified-realdata-bench --release \
//!   --features real-embeddings -- \
//!   --max-chunks 500 --num-queries 20 --no-checkpoint
//!
//! # Specific embedders only
//! cargo run -p context-graph-benchmark --bin unified-realdata-bench --release \
//!   --features real-embeddings -- \
//!   --embedders E1,E5,E7,E10
//! ```

use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;


use context_graph_benchmark::realdata::config::{EmbedderName, UnifiedBenchmarkConfig};
use context_graph_benchmark::realdata::report::{generate_console_summary, ReportGenerator};
use context_graph_benchmark::runners::unified_realdata::UnifiedRealdataBenchmarkRunner;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    /// Directory containing chunks.jsonl and metadata.json.
    data_dir: PathBuf,
    /// Output path for JSON results.
    output_path: PathBuf,
    /// Maximum chunks to load (0 = unlimited).
    max_chunks: usize,
    /// Number of queries to generate.
    num_queries: usize,
    /// Random seed.
    seed: u64,
    /// Specific embedders to evaluate (comma-separated).
    embedders: Option<Vec<EmbedderName>>,
    /// Checkpoint directory.
    checkpoint_dir: Option<PathBuf>,
    /// Disable checkpointing.
    no_checkpoint: bool,
    /// Skip ablation study.
    no_ablation: bool,
    /// Skip fusion comparison.
    no_fusion: bool,
    /// Skip correlation analysis.
    no_correlation: bool,
    /// Quick test mode.
    quick: bool,
    /// Batch size for embedding.
    batch_size: usize,
    /// Show progress.
    verbose: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark_diverse"),
            output_path: PathBuf::from("benchmark_results/unified_realdata.json"),
            max_chunks: 10000,
            num_queries: 100,
            seed: 42,
            embedders: None,
            checkpoint_dir: Some(PathBuf::from("data/hf_benchmark_diverse/checkpoints")),
            no_checkpoint: false,
            no_ablation: false,
            no_fusion: false,
            no_correlation: false,
            quick: false,
            batch_size: 32,
            verbose: true,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--data-dir" | "-d" => {
                args.data_dir = PathBuf::from(argv.next().expect("--data-dir requires a value"));
            }
            "--output" | "-o" => {
                args.output_path = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--max-chunks" | "-n" => {
                args.max_chunks = argv
                    .next()
                    .expect("--max-chunks requires a value")
                    .parse()
                    .expect("--max-chunks must be a number");
            }
            "--num-queries" | "-q" => {
                args.num_queries = argv
                    .next()
                    .expect("--num-queries requires a value")
                    .parse()
                    .expect("--num-queries must be a number");
            }
            "--seed" => {
                args.seed = argv
                    .next()
                    .expect("--seed requires a value")
                    .parse()
                    .expect("--seed must be a number");
            }
            "--embedders" | "-e" => {
                let embedder_str = argv.next().expect("--embedders requires a value");
                let embedders: Vec<EmbedderName> = embedder_str
                    .split(',')
                    .filter_map(|s| EmbedderName::from_str(s.trim()))
                    .collect();
                if embedders.is_empty() {
                    eprintln!("Warning: No valid embedders parsed from '{}'", embedder_str);
                } else {
                    args.embedders = Some(embedders);
                }
            }
            "--checkpoint-dir" => {
                args.checkpoint_dir = Some(PathBuf::from(
                    argv.next().expect("--checkpoint-dir requires a value"),
                ));
            }
            "--no-checkpoint" => {
                args.no_checkpoint = true;
                args.checkpoint_dir = None;
            }
            "--no-ablation" => {
                args.no_ablation = true;
            }
            "--no-fusion" => {
                args.no_fusion = true;
            }
            "--no-correlation" => {
                args.no_correlation = true;
            }
            "--quick" => {
                args.quick = true;
            }
            "--batch-size" | "-b" => {
                args.batch_size = argv
                    .next()
                    .expect("--batch-size requires a value")
                    .parse()
                    .expect("--batch-size must be a number");
            }
            "--quiet" => {
                args.verbose = false;
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

    // Apply quick mode defaults
    if args.quick {
        args.max_chunks = 500;
        args.num_queries = 20;
        args.no_ablation = true;
        args.no_fusion = true;
        args.no_correlation = true;
        args.no_checkpoint = true;
        args.checkpoint_dir = None;
    }

    args
}

fn print_usage() {
    eprintln!(
        r#"
Unified Real Data Benchmark - All 13 Embedders

USAGE:
    unified-realdata-bench [OPTIONS]

OPTIONS:
    --data-dir, -d <PATH>       Directory with chunks.jsonl and metadata.json
                                Default: data/hf_benchmark_diverse

    --output, -o <PATH>         Output path for results JSON
                                Default: benchmark_results/unified_realdata.json

    --max-chunks, -n <NUM>      Maximum chunks to load (0 = unlimited)
                                Default: 10000

    --num-queries, -q <NUM>     Number of queries to generate
                                Default: 100

    --seed <NUM>                Random seed for reproducibility
                                Default: 42

    --embedders, -e <LIST>      Comma-separated list of embedders to evaluate
                                Example: E1,E5,E7,E10
                                Default: all 13 embedders

    --checkpoint-dir <PATH>     Directory for embedding checkpoints
                                Default: data/hf_benchmark_diverse/checkpoints

    --no-checkpoint             Disable checkpointing

    --no-ablation               Skip ablation study

    --no-fusion                 Skip fusion strategy comparison

    --no-correlation            Skip cross-embedder correlation analysis

    --quick                     Quick test mode (500 chunks, 20 queries, no analysis)

    --batch-size, -b <NUM>      Batch size for embedding
                                Default: 32

    --quiet                     Suppress progress output

    --help, -h                  Show this help message

EXAMPLES:
    # Full benchmark
    unified-realdata-bench --data-dir data/hf_benchmark_diverse

    # Quick test
    unified-realdata-bench --quick

    # Specific embedders
    unified-realdata-bench --embedders E1,E5,E7,E10 --max-chunks 5000

OUTPUT FILES:
    benchmark_results/unified_realdata.json      - Complete metrics (JSON)
    benchmark_results/unified_realdata_report.md - Human-readable report

NOTE:
    This benchmark requires --features real-embeddings and a CUDA GPU for
    full embedding. Without GPU, synthetic results will be generated.
"#
    );
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let args = parse_args();

    if args.verbose {
        eprintln!("=== Unified Real Data Benchmark ===");
        eprintln!("Data directory: {}", args.data_dir.display());
        eprintln!("Max chunks: {}", if args.max_chunks == 0 { "unlimited".to_string() } else { args.max_chunks.to_string() });
        eprintln!("Num queries: {}", args.num_queries);
        eprintln!("Seed: {}", args.seed);
        if let Some(ref embedders) = args.embedders {
            eprintln!("Embedders: {:?}", embedders);
        } else {
            eprintln!("Embedders: all 13");
        }
        eprintln!();
    }

    // Build configuration
    let mut config = UnifiedBenchmarkConfig::default()
        .with_data_dir(args.data_dir.clone())
        .with_max_chunks(args.max_chunks);

    config.num_queries = args.num_queries;
    config.seed = args.seed;
    config.batch_size = args.batch_size;
    config.show_progress = args.verbose;
    config.run_ablation = !args.no_ablation;
    config.run_fusion_comparison = !args.no_fusion;
    config.run_correlation_analysis = !args.no_correlation;

    if args.no_checkpoint {
        config = config.without_checkpoints();
    } else if let Some(ref dir) = args.checkpoint_dir {
        config.checkpoint_dir = Some(dir.clone());
    }

    if let Some(embedders) = args.embedders {
        config = config.with_embedders(embedders);
    }

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Configuration error: {}", e);
        std::process::exit(1);
    }

    // Create output directory
    if let Some(parent) = args.output_path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).expect("Failed to create output directory");
        }
    }

    // Create and run benchmark
    let mut runner = UnifiedRealdataBenchmarkRunner::new(config);

    if args.verbose {
        eprintln!("Starting benchmark...");
    }

    // Try to run with real embeddings, fall back to synthetic
    #[cfg(feature = "real-embeddings")]
    let results = match runner.run().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Benchmark with embeddings failed: {}", e);
            eprintln!("Falling back to synthetic results...");
            runner.run_without_embedding().expect("Synthetic benchmark failed")
        }
    };

    #[cfg(not(feature = "real-embeddings"))]
    let results = {
        if args.verbose {
            eprintln!("Note: Running without real-embeddings feature (synthetic mode)");
        }
        runner.run_without_embedding().expect("Benchmark failed")
    };

    // Write JSON results
    let json_file = File::create(&args.output_path).expect("Failed to create output file");
    let writer = BufWriter::new(json_file);
    serde_json::to_writer_pretty(writer, &results).expect("Failed to write JSON");

    if args.verbose {
        eprintln!("\nJSON results written to: {}", args.output_path.display());
    }

    // Generate and write markdown report
    let report_path = args.output_path.with_extension("md");
    let report_path = report_path.with_file_name(
        format!("{}_report.md", args.output_path.file_stem().unwrap().to_str().unwrap())
    );

    let generator = ReportGenerator::new(results.clone());
    generator.write_to_file(&report_path).expect("Failed to write report");

    if args.verbose {
        eprintln!("Markdown report written to: {}", report_path.display());
    }

    // Print console summary
    let summary = generate_console_summary(&results);
    println!("{}", summary);

    // Exit with appropriate code
    if results.constitutional_compliance.all_passed {
        std::process::exit(0);
    } else {
        eprintln!("\nWarning: Some constitutional compliance checks failed!");
        std::process::exit(1);
    }
}
