//! Graph Linking Benchmark CLI.
//!
//! This binary runs benchmarks for all graph linking components:
//! NN-Descent, EdgeBuilder, BackgroundGraphBuilder, Graph Expansion,
//! Weight Projection (optional), and R-GCN (optional).
//!
//! ## Usage
//!
//! ```bash
//! # Run benchmark at default tier (1K memories)
//! cargo run -p context-graph-benchmark --bin graph-linking-bench --release
//!
//! # Run benchmark at specific tier
//! cargo run -p context-graph-benchmark --bin graph-linking-bench --release -- \
//!     --tier 3 \
//!     --output benchmark_results/graph_linking.json
//!
//! # Quick benchmark (tier 1, minimal iterations)
//! cargo run -p context-graph-benchmark --bin graph-linking-bench --release -- \
//!     --quick
//!
//! # Run specific components only
//! cargo run -p context-graph-benchmark --bin graph-linking-bench --release -- \
//!     --tier 2 \
//!     --components nn-descent,edge-builder,graph-expansion
//!
//! # Compare with previous baseline
//! cargo run -p context-graph-benchmark --bin graph-linking-bench --release -- \
//!     --tier 2 \
//!     --compare baseline.json
//! ```
//!
//! ## Scale Tiers
//!
//! - Tier 1: 100 memories, 5 topics
//! - Tier 2: 1K memories, 20 topics (default)
//! - Tier 3: 10K memories, 100 topics
//! - Tier 4: 100K memories, 500 topics
//! - Tier 5: 1M memories, 2000 topics
//! - Tier 6: 10M memories, 10000 topics
//!
//! ## Benchmark Components
//!
//! - nn-descent: K-NN graph construction
//! - edge-builder: Typed edge creation from embedder agreement
//! - background-builder: Batch processing simulation
//! - graph-expansion: Pipeline stage expansion metrics
//! - weight-projection: Learned weight inference (if model available)
//! - rgcn: GNN message passing (if model available)

use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use context_graph_benchmark::datasets::graph_linking::{
    GraphLinkingDataset, GraphLinkingDatasetConfig, ScaleTier,
};
use context_graph_benchmark::runners::graph_linking::{
    GraphLinkingBenchmarkConfig, GraphLinkingBenchmarkResults, GraphLinkingBenchmarkRunner,
};

/// Benchmark components to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum Component {
    /// NN-Descent K-NN graph construction.
    NnDescent,
    /// EdgeBuilder typed edge creation.
    EdgeBuilder,
    /// BackgroundGraphBuilder batch processing.
    BackgroundBuilder,
    /// Graph Expansion pipeline stage.
    GraphExpansion,
    /// Weight Projection learned inference.
    WeightProjection,
    /// R-GCN GNN message passing.
    Rgcn,
    /// All components.
    All,
}

/// Output format for results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
enum OutputFormat {
    /// Pretty table output.
    #[default]
    Table,
    /// JSON output.
    Json,
    /// Markdown report.
    Markdown,
}

/// Graph Linking Benchmark CLI.
#[derive(Parser, Debug)]
#[command(name = "graph-linking-bench")]
#[command(about = "Benchmark graph linking components (NN-Descent, EdgeBuilder, GNN)")]
struct Args {
    /// Scale tier (1-6).
    #[arg(short, long, default_value = "2")]
    tier: u8,

    /// Number of iterations for latency measurements.
    #[arg(short, long, default_value = "10")]
    iterations: usize,

    /// Sample size for pair-based benchmarks.
    #[arg(short, long, default_value = "1000")]
    sample_size: usize,

    /// K value for K-NN construction.
    #[arg(short, long, default_value = "20")]
    k: usize,

    /// Components to benchmark (comma-separated or multiple flags).
    #[arg(short, long, value_enum, num_args = 1.., default_values = ["all"])]
    components: Vec<Component>,

    /// Output format.
    #[arg(short, long, value_enum, default_value = "table")]
    format: OutputFormat,

    /// Output file path for results.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Compare with previous baseline JSON file.
    #[arg(long)]
    compare: Option<PathBuf>,

    /// Run quick benchmark (tier 1, minimal iterations).
    #[arg(long)]
    quick: bool,

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
    let _ = tracing::subscriber::set_global_default(subscriber);

    println!("=======================================================================");
    println!("  GRAPH LINKING BENCHMARK");
    println!("  NN-Descent | EdgeBuilder | Graph Expansion | GNN");
    println!("=======================================================================");
    println!();

    // Determine tier
    let tier = if args.quick {
        ScaleTier::Tier1_100
    } else {
        match args.tier {
            1 => ScaleTier::Tier1_100,
            2 => ScaleTier::Tier2_1K,
            3 => ScaleTier::Tier3_10K,
            4 => ScaleTier::Tier4_100K,
            5 => ScaleTier::Tier5_1M,
            6 => ScaleTier::Tier6_10M,
            _ => {
                eprintln!("ERROR: Invalid tier {}. Valid tiers are 1-6.", args.tier);
                std::process::exit(1);
            }
        }
    };

    // Determine which components to benchmark
    let all_components = args.components.contains(&Component::All);

    // Build config
    let iterations = if args.quick { 3 } else { args.iterations };
    let sample_size = if args.quick { 50 } else { args.sample_size };

    let config = GraphLinkingBenchmarkConfig {
        tier,
        iterations,
        sample_size,
        k: args.k,
        benchmark_nn_descent: all_components || args.components.contains(&Component::NnDescent),
        benchmark_edge_builder: all_components || args.components.contains(&Component::EdgeBuilder),
        benchmark_background_builder: all_components
            || args.components.contains(&Component::BackgroundBuilder),
        benchmark_graph_expansion: all_components
            || args.components.contains(&Component::GraphExpansion),
        benchmark_weight_projection: all_components
            || args.components.contains(&Component::WeightProjection),
        benchmark_rgcn: all_components || args.components.contains(&Component::Rgcn),
        weight_projection_path: None,
        rgcn_path: None,
        seed: args.seed,
        show_progress: true,
    };

    println!("Configuration:");
    println!("  Tier: {} ({} memories)", tier.level(), tier.size());
    println!("  Iterations: {}", iterations);
    println!("  Sample size: {}", sample_size);
    println!("  K (neighbors): {}", args.k);
    println!();

    // Generate dataset
    println!("Generating synthetic dataset...");
    let dataset_config = GraphLinkingDatasetConfig::for_tier(tier);
    let dataset = GraphLinkingDataset::generate(dataset_config);
    println!(
        "  Generated {} memories in {} topics",
        dataset.memories.len(),
        dataset.topic_clusters.len()
    );
    println!();

    // Run benchmark
    let mut runner = GraphLinkingBenchmarkRunner::new(config);
    let results = runner.run(&dataset);

    // Output results
    match args.format {
        OutputFormat::Table => print_table(&results),
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&results)?;
            println!("{}", json);
        }
        OutputFormat::Markdown => {
            println!("{}", results.report.to_markdown());
        }
    }

    // Compare with baseline if provided
    if let Some(baseline_path) = args.compare {
        compare_with_baseline(&baseline_path, &results)?;
    }

    // Save results if output path provided
    if let Some(output_path) = args.output {
        save_results(&results, &output_path)?;
    }

    // Print final summary
    println!();
    if results.all_targets_met() {
        println!("SUCCESS: ALL PERFORMANCE TARGETS MET");
    } else {
        println!("WARNING: SOME TARGETS NOT MET");
        println!();
        for check in &results.validation.checks {
            if !check.passed {
                println!(
                    "  [FAIL] {}: {} (expected {})",
                    check.description, check.actual, check.expected
                );
            }
        }
    }

    Ok(())
}

/// Print results as a formatted table.
fn print_table(results: &GraphLinkingBenchmarkResults) {
    let report = &results.report;

    println!();
    println!("=======================================================================");
    println!("  BENCHMARK RESULTS");
    println!("=======================================================================");
    println!();

    // NN-Descent
    if report.nn_descent.num_nodes > 0 {
        println!("NN-DESCENT:");
        println!("  Nodes:               {}", report.nn_descent.num_nodes);
        println!("  K (neighbors):       {}", report.nn_descent.k);
        println!("  Iterations:          {}", report.nn_descent.iterations);
        println!(
            "  Convergence rate:    {:.4}",
            report.nn_descent.convergence_rate
        );
        println!(
            "  Mean latency:        {:.2}ms",
            report.nn_descent.latency.mean_ms()
        );
        println!(
            "  P99 latency:         {:.2}ms",
            report.nn_descent.latency.p99_ms()
        );
        println!(
            "  Throughput:          {:.0} edges/sec",
            report.nn_descent.edges_per_sec
        );
        println!();
    }

    // EdgeBuilder
    if report.edge_builder.pairs_processed > 0 {
        println!("EDGE BUILDER:");
        println!(
            "  Pairs processed:     {}",
            report.edge_builder.pairs_processed
        );
        println!(
            "  Edges created:       {}",
            report.edge_builder.edges_created
        );
        println!(
            "  Mean latency/pair:   {:.3}ms",
            report.edge_builder.latency_per_pair.mean_ms()
        );
        println!(
            "  P99 latency/pair:    {:.3}ms",
            report.edge_builder.latency_per_pair.p99_ms()
        );
        println!(
            "  Throughput:          {:.0} pairs/sec",
            report.edge_builder.pairs_per_sec
        );

        // Edge type distribution
        if !report.edge_builder.edge_type_distribution.is_empty() {
            println!("  Edge types:");
            for (edge_type, count) in &report.edge_builder.edge_type_distribution {
                println!("    {}: {}", edge_type, count);
            }
        }
        println!();
    }

    // Background Builder
    if report.background_builder.batches_processed > 0 {
        println!("BACKGROUND BUILDER:");
        println!(
            "  Batches processed:   {}",
            report.background_builder.batches_processed
        );
        println!(
            "  Total fingerprints:  {}",
            report.background_builder.total_fingerprints
        );
        println!(
            "  Total edges:         {}",
            report.background_builder.total_edges
        );
        println!(
            "  Batch latency (mean): {:.2}ms",
            report.background_builder.batch_processing_latency.mean_ms()
        );
        println!();
    }

    // Graph Expansion
    if report.graph_expansion.candidates_in > 0 {
        println!("GRAPH EXPANSION:");
        println!(
            "  Candidates in:       {}",
            report.graph_expansion.candidates_in
        );
        println!(
            "  Candidates out:      {}",
            report.graph_expansion.candidates_out
        );
        println!(
            "  Expansion ratio:     {:.2}x",
            report.graph_expansion.expansion_ratio
        );
        println!(
            "  Edges/candidate:     {:.1}",
            report.graph_expansion.edges_per_candidate
        );
        println!(
            "  Mean latency:        {:.3}ms",
            report.graph_expansion.latency.mean_ms()
        );
        println!();
    }

    // Weight Projection
    if report.weight_projection.inferences > 0 {
        println!("WEIGHT PROJECTION:");
        println!(
            "  Inferences:          {}",
            report.weight_projection.inferences
        );
        println!(
            "  Mean latency:        {:.3}ms",
            report.weight_projection.latency_per_inference.mean_ms()
        );
        println!(
            "  Throughput:          {:.0} inf/sec",
            report.weight_projection.throughput_per_sec
        );

        if let Some(ref cmp) = report.weight_projection.heuristic_comparison {
            println!("  vs Heuristic:");
            println!("    Mean abs diff:     {:.3}", cmp.mean_abs_diff);
            println!("    Correlation:       {:.3}", cmp.correlation);
            println!(
                "    Learned higher:    {:.1}%",
                cmp.learned_higher_pct * 100.0
            );
        }
        println!();
    }

    // R-GCN
    if report.rgcn.nodes > 0 {
        println!("R-GCN (GNN):");
        println!("  Avg nodes:           {}", report.rgcn.nodes);
        println!("  Avg edges:           {}", report.rgcn.edges);
        println!(
            "  Mean latency:        {:.3}ms",
            report.rgcn.total_latency.mean_ms()
        );
        println!(
            "  P99 latency:         {:.3}ms",
            report.rgcn.total_latency.p99_ms()
        );
        println!("  Nodes/sec:           {:.0}", report.rgcn.nodes_per_sec);
        println!("  Edges/sec:           {:.0}", report.rgcn.edges_per_sec);
        println!();
    }

    // Validation Summary
    println!("VALIDATION:");
    for check in &results.validation.checks {
        let status = if check.passed { "PASS" } else { "FAIL" };
        println!("  [{}] {}", status, check.description);
        println!(
            "        Actual: {}, Expected: {}",
            check.actual, check.expected
        );
    }
    println!();
    println!(
        "  Targets met: {}/{}",
        results.validation.checks_passed, results.validation.checks_total
    );
    println!();

    // Timing
    println!("TOTAL DURATION: {:.2}s", report.total_duration_secs);
}

/// Compare results with a baseline file.
fn compare_with_baseline(
    baseline_path: &PathBuf,
    current: &GraphLinkingBenchmarkResults,
) -> Result<()> {
    let baseline_json = std::fs::read_to_string(baseline_path)?;
    let baseline: GraphLinkingBenchmarkResults = serde_json::from_str(&baseline_json)?;

    println!();
    println!("=======================================================================");
    println!("  COMPARISON WITH BASELINE");
    println!("=======================================================================");
    println!();

    // NN-Descent comparison
    if baseline.report.nn_descent.num_nodes > 0 && current.report.nn_descent.num_nodes > 0 {
        let baseline_latency = baseline.report.nn_descent.latency.mean_ms();
        let current_latency = current.report.nn_descent.latency.mean_ms();
        let diff_pct = ((current_latency - baseline_latency) / baseline_latency) * 100.0;
        let status = if diff_pct <= 0.0 {
            "IMPROVED"
        } else if diff_pct <= 10.0 {
            "SIMILAR"
        } else {
            "REGRESSED"
        };
        println!(
            "NN-Descent latency: {:.2}ms -> {:.2}ms ({:+.1}%) [{}]",
            baseline_latency, current_latency, diff_pct, status
        );
    }

    // EdgeBuilder comparison
    if baseline.report.edge_builder.pairs_processed > 0
        && current.report.edge_builder.pairs_processed > 0
    {
        let baseline_throughput = baseline.report.edge_builder.pairs_per_sec;
        let current_throughput = current.report.edge_builder.pairs_per_sec;
        let diff_pct = ((current_throughput - baseline_throughput) / baseline_throughput) * 100.0;
        let status = if diff_pct >= 0.0 {
            "IMPROVED"
        } else if diff_pct >= -10.0 {
            "SIMILAR"
        } else {
            "REGRESSED"
        };
        println!(
            "EdgeBuilder throughput: {:.0} -> {:.0} pairs/sec ({:+.1}%) [{}]",
            baseline_throughput, current_throughput, diff_pct, status
        );
    }

    // Graph Expansion comparison
    if baseline.report.graph_expansion.candidates_in > 0
        && current.report.graph_expansion.candidates_in > 0
    {
        let baseline_ratio = baseline.report.graph_expansion.expansion_ratio;
        let current_ratio = current.report.graph_expansion.expansion_ratio;
        println!(
            "Graph expansion ratio: {:.2}x -> {:.2}x",
            baseline_ratio, current_ratio
        );
    }

    println!();

    Ok(())
}

/// Save results to JSON file.
fn save_results(results: &GraphLinkingBenchmarkResults, output_path: &PathBuf) -> Result<()> {
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
