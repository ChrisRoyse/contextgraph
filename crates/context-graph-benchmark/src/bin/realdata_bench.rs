//! CLI for running real-data benchmarks.
//!
//! Usage:
//!     cargo run -p context-graph-benchmark --bin realdata-bench -- --help
//!
//! Examples:
//!     # Run benchmark on prepared Wikipedia data
//!     cargo run -p context-graph-benchmark --bin realdata-bench -- \
//!         --dataset ./benchmark_data/wikipedia \
//!         --max-chunks 10000 \
//!         --output results.json
//!
//!     # Run with real GPU embeddings (requires feature flag)
//!     cargo run -p context-graph-benchmark --bin realdata-bench --features real-embeddings -- \
//!         --dataset ./benchmark_data/wikipedia \
//!         --real-embeddings

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use context_graph_benchmark::realdata::{RealDataBenchConfig, RealDataBenchRunner};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config = parse_args(&args);

    println!("=== Real Data Benchmark ===");
    println!();
    println!("Configuration:");
    println!("  Dataset path: {}", config.dataset_path);
    println!(
        "  Max chunks: {}",
        if config.max_chunks == 0 {
            "unlimited".to_string()
        } else {
            config.max_chunks.to_string()
        }
    );
    println!("  Num queries: {}", config.num_queries);
    println!("  Seed: {}", config.seed);
    println!("  Embeddings: real (GPU)");
    println!();

    let mut runner = RealDataBenchRunner::with_config(config.clone());

    // Load dataset
    print!("Loading dataset... ");
    std::io::stdout().flush().unwrap();
    match runner.load_dataset() {
        Ok(dataset) => {
            println!("OK");
            println!(
                "  {} chunks, {} topics",
                dataset.chunks.len(),
                dataset.topic_count()
            );
        }
        Err(e) => {
            println!("FAILED");
            eprintln!("Error: {}", e);
            eprintln!();
            eprintln!("Make sure the dataset is prepared first:");
            eprintln!("  cd crates/context-graph-benchmark/scripts");
            eprintln!("  python prepare_wikipedia.py --output ../benchmark_data/wikipedia --max-docs 10000");
            std::process::exit(1);
        }
    }
    println!();

    // Generate embeddings
    print!("Generating embeddings... ");
    std::io::stdout().flush().unwrap();
    match runner.embed_dataset() {
        Ok(embedded) => {
            println!("OK");
            println!("  {} fingerprints generated", embedded.fingerprints.len());
        }
        Err(e) => {
            println!("FAILED");
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
    println!();

    // Run benchmarks
    println!("Running benchmarks...");
    let results = match runner.run_benchmarks() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };
    println!();

    // Print results
    println!("=== Results ===");
    println!();
    println!("Dataset Stats:");
    println!("  Total chunks: {}", results.dataset_stats.total_chunks);
    println!("  Total topics: {}", results.dataset_stats.total_topics);
    println!(
        "  Chunks/topic: {:.1}",
        results.dataset_stats.chunks_per_topic_avg
    );
    println!();

    println!("Retrieval (P@10 | R@10 | MRR):");
    let single_p10 = results
        .single_retrieval
        .precision_at
        .get(&10)
        .copied()
        .unwrap_or(0.0);
    let single_r10 = results
        .single_retrieval
        .recall_at
        .get(&10)
        .copied()
        .unwrap_or(0.0);
    let multi_p10 = results
        .multi_retrieval
        .precision_at
        .get(&10)
        .copied()
        .unwrap_or(0.0);
    let multi_r10 = results
        .multi_retrieval
        .recall_at
        .get(&10)
        .copied()
        .unwrap_or(0.0);
    println!(
        "  Single-embedder: {:.3} | {:.3} | {:.3}",
        single_p10, single_r10, results.single_retrieval.mrr
    );
    println!(
        "  Multi-space:     {:.3} | {:.3} | {:.3}",
        multi_p10, multi_r10, results.multi_retrieval.mrr
    );
    println!();

    println!("Clustering (Purity | NMI):");
    println!(
        "  Single-embedder: {:.3} | {:.3}",
        results.single_clustering.purity, results.single_clustering.nmi
    );
    println!(
        "  Multi-space:     {:.3} | {:.3}",
        results.multi_clustering.purity, results.multi_clustering.nmi
    );
    println!();

    println!("Improvements (multi-space vs single-embedder):");
    println!("  MRR:        {:+.1}%", results.improvements.mrr_pct);
    println!(
        "  P@10:       {:+.1}%",
        results.improvements.precision_10_pct
    );
    println!("  R@10:       {:+.1}%", results.improvements.recall_10_pct);
    println!("  Purity:     {:+.1}%", results.improvements.purity_pct);
    println!("  NMI:        {:+.1}%", results.improvements.nmi_pct);
    println!();

    // Determine winner
    let overall_improvement = (results.improvements.mrr_pct
        + results.improvements.precision_10_pct
        + results.improvements.recall_10_pct
        + results.improvements.purity_pct
        + results.improvements.nmi_pct)
        / 5.0;

    let winner = if overall_improvement > 0.0 {
        "Multi-space"
    } else {
        "Single-embedder"
    };
    println!(
        "Winner: {} (overall improvement: {:+.1}%)",
        winner, overall_improvement
    );
    println!();

    // Save results to file if output path provided
    if let Some(output) = get_output_path(&args) {
        let json = serde_json::to_string_pretty(&results).unwrap();
        let mut file = File::create(&output).expect("Failed to create output file");
        file.write_all(json.as_bytes())
            .expect("Failed to write output file");
        println!("Results saved to: {}", output.display());
    }
}

fn parse_args(args: &[String]) -> RealDataBenchConfig {
    let mut config = RealDataBenchConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            "--dataset" | "-d" => {
                i += 1;
                if i < args.len() {
                    config.dataset_path = args[i].clone();
                }
            }
            "--max-chunks" | "-n" => {
                i += 1;
                if i < args.len() {
                    config.max_chunks = args[i].parse().unwrap_or(10_000);
                }
            }
            "--queries" | "-q" => {
                i += 1;
                if i < args.len() {
                    config.num_queries = args[i].parse().unwrap_or(100);
                }
            }
            "--seed" | "-s" => {
                i += 1;
                if i < args.len() {
                    config.seed = args[i].parse().unwrap_or(42);
                }
            }
            "--output" | "-o" => {
                // Handled separately
                i += 1;
            }
            _ => {}
        }
        i += 1;
    }

    config
}

fn get_output_path(args: &[String]) -> Option<PathBuf> {
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--output" || args[i] == "-o" {
            i += 1;
            if i < args.len() {
                return Some(PathBuf::from(&args[i]));
            }
        }
        i += 1;
    }
    None
}

fn print_help() {
    println!("Real Data Benchmark for Context Graph");
    println!();
    println!("USAGE:");
    println!("    cargo run -p context-graph-benchmark --bin realdata-bench -- [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -d, --dataset <PATH>     Path to prepared dataset directory");
    println!("                             (default: ./benchmark_data/wikipedia)");
    println!("    -n, --max-chunks <N>     Maximum chunks to load (0 = unlimited)");
    println!("                             (default: 10000)");
    println!("    -q, --queries <N>        Number of benchmark queries");
    println!("                             (default: 100)");
    println!("    -s, --seed <N>           Random seed for reproducibility");
    println!("                             (default: 42)");
    println!("    --real-embeddings        Use real GPU embeddings (requires feature)");
    println!("    -o, --output <PATH>      Save results to JSON file");
    println!("    -h, --help               Print this help message");
    println!();
    println!("EXAMPLES:");
    println!("    # Prepare Wikipedia dataset first:");
    println!("    cd crates/context-graph-benchmark/scripts");
    println!("    pip install datasets");
    println!(
        "    python prepare_wikipedia.py --output ../benchmark_data/wikipedia --max-docs 10000"
    );
    println!();
    println!("    # Run benchmark:");
    println!("    cargo run -p context-graph-benchmark --bin realdata-bench -- \\");
    println!("        --dataset ./benchmark_data/wikipedia --max-chunks 10000");
    println!();
    println!("    # Save results:");
    println!("    cargo run -p context-graph-benchmark --bin realdata-bench -- \\");
    println!("        --output benchmark_results.json");
}
