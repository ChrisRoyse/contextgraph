//! Causal Embedding Benchmark Binary
//!
//! 8-phase benchmark for evaluating E5 causal embedder quality.
//!
//! Usage:
//!   causal-embedding-bench [OPTIONS]
//!
//! Options:
//!   --phase <N>          Run only phase N (1-8), default: all
//!   --data-dir <PATH>    Dataset directory, default: data/causal_benchmark
//!   --output <PATH>      Output JSON file, default: benchmark_results/causal_{timestamp}.json
//!   --compare <A> <B>    Compare two result JSON files
//!   --quick              Run quick subset (20% of data)
//!   --verbose            Print per-query details

use clap::Parser;
use context_graph_benchmark::causal_bench::{comparison, data_loader, metrics, phases, report};
use std::path::{Path, PathBuf};

#[derive(Parser, Debug)]
#[command(name = "causal-embedding-bench")]
#[command(about = "8-phase causal embedding benchmark for E5 quality evaluation")]
struct Args {
    /// Run only this phase (1-8). Default: all phases.
    #[arg(long)]
    phase: Option<u8>,

    /// Path to benchmark dataset directory.
    #[arg(long, default_value = "data/causal_benchmark")]
    data_dir: PathBuf,

    /// Path to save JSON output.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Compare two saved result files and print delta report.
    #[arg(long, num_args = 2)]
    compare: Option<Vec<PathBuf>>,

    /// Quick mode: use 20% of data for rapid iteration.
    #[arg(long)]
    quick: bool,

    /// Verbose: print per-query details.
    #[arg(long)]
    verbose: bool,

    /// Model name for report metadata.
    #[arg(long, default_value = "nomic-embed-text-v1.5 (untuned)")]
    model_name: String,

    /// Use GPU (real CausalModel) instead of synthetic scores.
    /// Requires `real-embeddings` feature and CUDA GPU.
    #[arg(long)]
    gpu: bool,

    /// Path to CausalModel weights directory (used with --gpu).
    #[arg(long, default_value = "models/causal")]
    model_path: PathBuf,

    /// Enable E12 ColBERT MaxSim reranking in P6 simulated search.
    #[arg(long)]
    enable_rerank: bool,
}

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("context_graph_benchmark=info".parse().unwrap()),
        )
        .init();

    let args = Args::parse();

    // Handle comparison mode
    if let Some(paths) = &args.compare {
        if paths.len() != 2 {
            anyhow::bail!("--compare requires exactly 2 paths");
        }
        return run_comparison(&paths[0], &paths[1]);
    }

    // Load dataset
    tracing::info!("Loading dataset from {}", args.data_dir.display());

    let gt_path = args.data_dir.join("ground_truth.jsonl");
    let queries_path = args.data_dir.join("queries.jsonl");

    if !gt_path.exists() {
        anyhow::bail!(
            "Ground truth file not found: {}\nRun from project root or use --data-dir",
            gt_path.display()
        );
    }

    let mut pairs = data_loader::load_ground_truth(&gt_path)?;
    let mut queries = data_loader::load_queries(&queries_path)?;

    tracing::info!("Loaded {} pairs, {} queries", pairs.len(), queries.len());

    // Quick mode: subsample
    if args.quick {
        pairs = data_loader::subsample(&pairs, 0.2);
        queries = data_loader::subsample_queries(&queries, 0.2);
        tracing::info!(
            "Quick mode: {} pairs, {} queries",
            pairs.len(),
            queries.len()
        );
    }

    // Log domain distribution
    let domains = data_loader::unique_domains(&pairs);
    tracing::info!("Domains: {:?}", domains);
    for domain in &domains {
        let count = pairs.iter().filter(|p| p.domain == *domain).count();
        tracing::info!("  {}: {} pairs", domain, count);
    }

    // Configure embedding provider
    let provider: std::sync::Arc<
        dyn context_graph_benchmark::causal_bench::provider::EmbeddingProvider,
    > = if args.gpu {
        #[cfg(feature = "real-embeddings")]
        {
            use context_graph_benchmark::causal_bench::provider::EmbeddingProvider;
            tracing::info!("Loading GPU provider from {}", args.model_path.display());
            let gpu =
                context_graph_benchmark::causal_bench::provider::GpuProvider::new(&args.model_path)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
            tracing::info!(
                "GPU provider ready ({})",
                if gpu.is_gpu() { "trained" } else { "base" }
            );
            std::sync::Arc::new(gpu)
        }
        #[cfg(not(feature = "real-embeddings"))]
        {
            anyhow::bail!("--gpu requires the `real-embeddings` feature. Rebuild with: cargo build --release --features real-embeddings");
        }
    } else {
        std::sync::Arc::new(
            context_graph_benchmark::causal_bench::provider::SyntheticProvider::new(),
        )
    };

    // Configure benchmark
    let config = phases::BenchConfig {
        data_dir: args.data_dir.clone(),
        quick: args.quick,
        verbose: args.verbose,
        provider,
        enable_rerank: args.enable_rerank,
        ..Default::default()
    };

    // Run phases
    let phase_results = if let Some(phase_num) = args.phase {
        tracing::info!("Running phase {} only", phase_num);
        match phases::run_single_phase(phase_num, &pairs, &queries, &config) {
            Some(result) => vec![result],
            None => {
                anyhow::bail!("Invalid phase number: {}. Must be 1-8.", phase_num);
            }
        }
    } else {
        tracing::info!("Running all 8 phases");
        phases::run_all_phases(&pairs, &queries, &config)
    };

    // Build report
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let full_report = metrics::FullBenchmarkReport {
        model_name: args.model_name.clone(),
        timestamp: timestamp.clone(),
        phases: phase_results,
        overall_pass_count: 0, // Filled below
        overall_total: 0,
        model_pass_count: 0,
        model_total: 0,
    };

    let full_report = metrics::FullBenchmarkReport {
        overall_pass_count: full_report.count_pass(),
        overall_total: full_report.phases.len(),
        model_pass_count: full_report.count_model_pass(),
        model_total: full_report.count_model_total(),
        ..full_report
    };

    // Print summary
    report::print_summary(&full_report);

    // Save output
    let output_path = args.output.unwrap_or_else(|| {
        let ts = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        PathBuf::from(format!("benchmark_results/causal_{}.json", ts))
    });

    report::save_json(&full_report, &output_path)?;

    // Also save markdown
    let md_path = output_path.with_extension("md");
    report::save_markdown(&full_report, &md_path)?;

    tracing::info!("Results saved to {}", output_path.display());
    tracing::info!("Markdown saved to {}", md_path.display());

    Ok(())
}

fn run_comparison(baseline_path: &Path, tuned_path: &Path) -> anyhow::Result<()> {
    tracing::info!("Loading baseline: {}", baseline_path.display());
    let baseline = comparison::load_results(baseline_path)?;

    tracing::info!("Loading tuned: {}", tuned_path.display());
    let tuned = comparison::load_results(tuned_path)?;

    let comp = comparison::compare_reports(&baseline, &tuned);
    comparison::print_comparison(&comp);

    // Save comparison report
    let comp_path = tuned_path.with_file_name("comparison.json");
    let json = serde_json::to_string_pretty(&comp)?;
    if let Some(parent) = comp_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&comp_path, json)?;

    let md = comparison::generate_comparison_markdown(&comp);
    let md_path = tuned_path.with_file_name("comparison.md");
    std::fs::write(&md_path, md)?;

    tracing::info!("Comparison saved to {}", comp_path.display());
    Ok(())
}
