//! E8 Graph Embedder Real Data Benchmark
//!
//! Tests E8 Graph asymmetric similarity using real Wikipedia document data.
//! Builds document graphs from chunk relationships and evaluates:
//! - Neighbor retrieval effectiveness
//! - Asymmetric similarity validation (1.5 ± 0.15 ratio)
//! - Hub detection accuracy
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark with real data:
//! cargo run -p context-graph-benchmark --bin graph-realdata-bench --release \
//!     --features real-embeddings -- --data-dir data/hf_benchmark_diverse
//!
//! # Quick test:
//! cargo run -p context-graph-benchmark --bin graph-realdata-bench --release \
//!     --features real-embeddings -- --max-chunks 500
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use context_graph_benchmark::realdata::loader::DatasetLoader;
use context_graph_benchmark::runners::graph_realdata::{
    E8GraphRealdataConfig, E8GraphRealdataResults, E8GraphRealdataRunner,
};

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    data_dir: PathBuf,
    output_path: PathBuf,
    max_chunks: usize,
    num_queries: usize,
    seed: u64,
    run_asymmetric: bool,
    run_hub_detection: bool,
    hub_threshold: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark_diverse"),
            output_path: PathBuf::from("benchmark_results/graph_realdata.json"),
            max_chunks: 5000,
            num_queries: 100,
            seed: 42,
            run_asymmetric: true,
            run_hub_detection: true,
            hub_threshold: 5,
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
            "--hub-threshold" => {
                args.hub_threshold = argv
                    .next()
                    .expect("--hub-threshold requires a value")
                    .parse()
                    .expect("--hub-threshold must be a number");
            }
            "--no-asymmetric" => {
                args.run_asymmetric = false;
            }
            "--no-hub-detection" => {
                args.run_hub_detection = false;
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
E8 Graph Embedder Real Data Benchmark

Usage: graph-realdata-bench [OPTIONS]

Options:
    --data-dir, -d <PATH>     Dataset directory [default: data/hf_benchmark_diverse]
    --output, -o <PATH>       Output JSON file path [default: benchmark_results/graph_realdata.json]
    --max-chunks, -n <N>      Maximum chunks to load [default: 5000]
    --num-queries, -q <N>     Number of test queries [default: 100]
    --seed <N>                Random seed [default: 42]
    --hub-threshold <N>       Min connections for hub [default: 5]
    --no-asymmetric           Skip asymmetric validation
    --no-hub-detection        Skip hub detection
    --help, -h                Print this help message
"#
    );
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("E8 Graph Real Data Benchmark");
    println!("============================\n");

    let args = parse_args();
    let start_time = Instant::now();

    // Load dataset
    println!("Loading dataset from: {}", args.data_dir.display());
    let loader = DatasetLoader::new();
    let mut dataset = match loader.load_from_dir(&args.data_dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading dataset: {}", e);
            std::process::exit(1);
        }
    };

    // Limit chunks if needed
    if dataset.chunks.len() > args.max_chunks {
        dataset.chunks.truncate(args.max_chunks);
    }
    println!("Loaded {} chunks", dataset.chunks.len());

    // Configure benchmark
    let config = E8GraphRealdataConfig {
        num_queries: args.num_queries,
        k_values: vec![1, 5, 10, 20],
        seed: args.seed,
        run_asymmetric_validation: args.run_asymmetric,
        run_hub_detection: args.run_hub_detection,
        hub_threshold: args.hub_threshold,
    };

    // Run benchmark
    println!("\nRunning E8 graph benchmark...");
    let mut runner = E8GraphRealdataRunner::new(config);
    let results = runner.run(&dataset);

    // Print results summary
    print_summary(&results);

    // Write results
    let elapsed = start_time.elapsed();
    println!("\n=== Benchmark Complete ===");
    println!("Duration: {:.2}s", elapsed.as_secs_f64());

    // Ensure output directory exists
    if let Some(parent) = args.output_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let file = File::create(&args.output_path).expect("Failed to create output file");
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &results).expect("Failed to write results");

    println!("Results written to: {}", args.output_path.display());
}

fn print_summary(results: &E8GraphRealdataResults) {
    println!("\n=== Results Summary ===");

    // Graph stats
    println!("\nGraph Statistics:");
    println!("  Nodes: {}", results.graph_stats.num_nodes);
    println!("  Edges: {}", results.graph_stats.num_edges);
    println!("  Hubs: {}", results.graph_stats.num_hubs);
    println!("  Avg degree: {:.2}", results.graph_stats.avg_degree);
    println!("  Documents: {}", results.graph_stats.num_documents);

    // Overall metrics
    println!("\nRetrieval Metrics:");
    println!("  MRR: {:.4}", results.overall.mrr);
    println!("  MAP: {:.4}", results.overall.map);
    for k in &[1, 5, 10, 20] {
        if let Some(&p) = results.overall.precision_at_k.get(k) {
            println!("  P@{}: {:.4}", k, p);
        }
    }
    for k in &[1, 5, 10, 20] {
        if let Some(&r) = results.overall.recall_at_k.get(k) {
            println!("  R@{}: {:.4}", k, r);
        }
    }

    // Asymmetric validation
    if let Some(ref asym) = results.asymmetric {
        println!("\nAsymmetric Validation:");
        println!(
            "  Forward similarity mean: {:.4}",
            asym.forward_similarity_mean
        );
        println!(
            "  Reverse similarity mean: {:.4}",
            asym.reverse_similarity_mean
        );
        println!(
            "  Asymmetric ratio: {:.2} (expected 1.5 ± 0.15)",
            asym.asymmetric_ratio
        );
        println!(
            "  Ratio valid: {}",
            if asym.ratio_valid { "YES" } else { "NO" }
        );
        println!("  Pairs tested: {}", asym.pairs_tested);
    }

    // Hub detection
    if let Some(ref hub) = results.hub_detection {
        println!("\nHub Detection:");
        println!("  Precision: {:.4}", hub.hub_precision);
        println!("  Recall: {:.4}", hub.hub_recall);
        println!("  F1 score: {:.4}", hub.hub_f1);
        println!("  Hubs found: {}/{}", hub.hubs_found, hub.total_hubs);
    }

    // Timings
    println!("\nTimings:");
    println!("  Graph build: {}ms", results.timings.graph_build_ms);
    println!("  Query: {}ms", results.timings.query_ms);
    if let Some(ms) = results.timings.asymmetric_ms {
        println!("  Asymmetric: {}ms", ms);
    }
    if let Some(ms) = results.timings.hub_detection_ms {
        println!("  Hub detection: {}ms", ms);
    }
    println!("  Total: {}ms", results.timings.total_ms);
}
