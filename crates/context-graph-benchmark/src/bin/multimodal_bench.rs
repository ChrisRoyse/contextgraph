//! E10 Multimodal (Paraphrase/Context) Embedder Benchmark
//!
//! Tests E10 Multimodal asymmetric similarity with paraphrase/context embeddings.
//! Uses dual vectors (as_paraphrase, as_context) for asymmetric retrieval evaluation.
//!
//! ## Key Features
//!
//! - **Paraphrase Detection**: Test paraphrase vs context direction classification
//! - **Context Matching**: Evaluate paraphrase→context retrieval (MRR, P@K)
//! - **Asymmetric Validation**: Verify direction handling (E5-base-v2 provides natural asymmetry via prefixes)
//! - **Ablation Study**: Compare E1+E10 vs E1 only vs full 13-space
//! - **Blend Analysis**: Test `blendWithSemantic` parameter behavior
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark:
//! cargo run -p context-graph-benchmark --bin multimodal-bench --release
//!
//! # Quick test with limited samples:
//! cargo run -p context-graph-benchmark --bin multimodal-bench --release -- --max-samples 100
//!
//! # With custom output:
//! cargo run -p context-graph-benchmark --bin multimodal-bench --release -- \
//!     --output benchmark_results/e10_results.json
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use context_graph_benchmark::datasets::multimodal::E10MultimodalDatasetConfig;
use context_graph_benchmark::runners::multimodal::{
    E10MultimodalBenchmarkConfig, E10MultimodalBenchmarkResults, E10MultimodalBenchmarkRunner,
};

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    output_path: PathBuf,
    max_samples: usize,
    seed: u64,
    num_documents: usize,
    num_paraphrase_queries: usize,
    num_context_queries: usize,
    skip_ablation: bool,
    blend_values: Vec<f64>,
    verbose: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("benchmark_results/multimodal_benchmark.json"),
            max_samples: 1000,
            seed: 42,
            num_documents: 500,
            num_paraphrase_queries: 50,
            num_context_queries: 50,
            skip_ablation: false,
            blend_values: vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0],
            verbose: false,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--output" | "-o" => {
                args.output_path = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--max-samples" | "-n" => {
                args.max_samples = argv
                    .next()
                    .expect("--max-samples requires a value")
                    .parse()
                    .expect("--max-samples must be a number");
            }
            "--num-documents" => {
                args.num_documents = argv
                    .next()
                    .expect("--num-documents requires a value")
                    .parse()
                    .expect("--num-documents must be a number");
            }
            "--num-paraphrase-queries" => {
                args.num_paraphrase_queries = argv
                    .next()
                    .expect("--num-paraphrase-queries requires a value")
                    .parse()
                    .expect("--num-paraphrase-queries must be a number");
            }
            "--num-context-queries" => {
                args.num_context_queries = argv
                    .next()
                    .expect("--num-context-queries requires a value")
                    .parse()
                    .expect("--num-context-queries must be a number");
            }
            "--seed" => {
                args.seed = argv
                    .next()
                    .expect("--seed requires a value")
                    .parse()
                    .expect("--seed must be a number");
            }
            "--skip-ablation" => {
                args.skip_ablation = true;
            }
            "--verbose" | "-v" => {
                args.verbose = true;
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
E10 Multimodal (Paraphrase/Context) Embedder Benchmark

Tests E10's dual paraphrase/context embeddings for asymmetric similarity.
E5-base-v2 provides natural asymmetry via query:/passage: prefixes - neutral modifiers (1.0/1.0).

Usage: multimodal-bench [OPTIONS]

Options:
    --output, -o <PATH>           Output JSON file [default: benchmark_results/multimodal_benchmark.json]
    --max-samples, -n <N>         Maximum samples [default: 1000]
    --num-documents <N>           Number of documents to generate [default: 500]
    --num-paraphrase-queries <N>  Number of paraphrase queries [default: 50]
    --num-context-queries <N>     Number of context queries [default: 50]
    --seed <N>                    Random seed [default: 42]
    --skip-ablation               Skip ablation study (faster)
    --verbose, -v                 Verbose output
    --help, -h                    Print this help message

Benchmark Phases:
  1. Paraphrase Detection - Classify queries as paraphrase vs context
  2. Context Matching     - Evaluate paraphrase→context retrieval (MRR, P@K)
  3. Asymmetric Valid.    - Verify direction handling (E5-base-v2 provides natural asymmetry)
  4. Ablation Study       - Compare E1+E10 vs E1 only vs full 13-space
"#
    );
}

// ============================================================================
// Benchmark Output
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MultimodalBenchmarkOutput {
    metadata: BenchmarkMetadata,
    results: E10MultimodalBenchmarkResults,
    summary: BenchmarkSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkMetadata {
    timestamp: String,
    version: String,
    seed: u64,
    max_samples: usize,
    num_documents: usize,
    num_paraphrase_queries: usize,
    num_context_queries: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkSummary {
    // Paraphrase Detection
    paraphrase_detection_accuracy: f64,
    paraphrase_precision: f64,
    paraphrase_recall: f64,

    // Context Matching
    mrr: f64,
    precision_at_1: f64,
    precision_at_5: f64,
    precision_at_10: f64,
    ndcg_at_10: f64,

    // Asymmetric Validation
    asymmetry_ratio: f64,
    paraphrase_to_context_modifier: f32,
    context_to_paraphrase_modifier: f32,
    formula_compliant: bool,

    // Ablation (if run)
    e1_only_mrr: Option<f64>,
    e10_only_mrr: Option<f64>,
    e1_e10_blend_mrr: Option<f64>,
    e10_contribution_percentage: Option<f64>,
    optimal_blend_value: Option<f64>,

    // Performance
    total_duration_ms: u64,

    // Pass/Fail
    all_tests_passed: bool,
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("E10 Multimodal (Paraphrase/Context) Embedder Benchmark");
    println!("===================================================\n");

    let args = parse_args();
    let start_time = Instant::now();

    // Print configuration
    println!("Configuration:");
    println!("  Documents: {}", args.num_documents);
    println!("  Paraphrase queries: {}", args.num_paraphrase_queries);
    println!("  Context queries: {}", args.num_context_queries);
    println!("  Seed: {}", args.seed);
    println!("  Run ablation: {}", !args.skip_ablation);
    println!();

    // Build benchmark config
    let config = E10MultimodalBenchmarkConfig {
        dataset: E10MultimodalDatasetConfig {
            num_documents: args.num_documents,
            num_paraphrase_queries: args.num_paraphrase_queries,
            num_context_queries: args.num_context_queries,
            seed: args.seed,
            ..Default::default()
        },
        run_paraphrase_detection: true,
        run_context_matching: true,
        run_asymmetric_validation: true,
        run_ablation: !args.skip_ablation,
        k_values: vec![1, 5, 10, 20],
        blend_values: args.blend_values.clone(),
    };

    // Run benchmark
    let runner = E10MultimodalBenchmarkRunner::new(config);
    let results = runner.run();

    // Print phase results
    print_phase_results(&results, args.verbose);

    // Verify formula compliance
    let formula_compliant = verify_formula_compliance(&results);

    // Build summary
    let summary = build_summary(&results, formula_compliant);

    // Compile output
    let output = MultimodalBenchmarkOutput {
        metadata: BenchmarkMetadata {
            timestamp: Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            seed: args.seed,
            max_samples: args.max_samples,
            num_documents: args.num_documents,
            num_paraphrase_queries: args.num_paraphrase_queries,
            num_context_queries: args.num_context_queries,
        },
        results: results.clone(),
        summary: summary.clone(),
    };

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
    serde_json::to_writer_pretty(writer, &output).expect("Failed to write results");

    println!("Results written to: {}", args.output_path.display());

    // Print summary
    print_summary(&summary);

    // Exit with appropriate code
    if summary.all_tests_passed {
        println!("\n✓ All E10 benchmark tests PASSED");
        std::process::exit(0);
    } else {
        println!("\n✗ Some E10 benchmark tests FAILED");
        std::process::exit(1);
    }
}

fn print_phase_results(results: &E10MultimodalBenchmarkResults, verbose: bool) {
    // Phase 1: Paraphrase Detection
    println!("\n=== Phase 1: Paraphrase Detection ===");
    let para = &results.metrics.paraphrase_detection;
    println!("  Total queries: {}", para.total_queries);
    println!("  Accuracy: {:.1}%", para.accuracy * 100.0);
    println!("  Paraphrase precision: {:.3}", para.paraphrase_precision);
    println!("  Paraphrase recall: {:.3}", para.paraphrase_recall);
    println!("  Paraphrase F1: {:.3}", para.paraphrase_f1);
    println!("  Correct paraphrase: {}", para.correct_paraphrase);
    println!("  Correct context: {}", para.correct_context);
    println!("  Misclassified: {}", para.misclassified);

    if verbose {
        println!("  Per-domain accuracy:");
        for (domain, acc) in &para.per_domain_accuracy {
            println!("    {}: {:.1}%", domain, acc * 100.0);
        }
    }

    // Phase 2: Context Matching
    println!("\n=== Phase 2: Context Matching ===");
    let context = &results.metrics.context_matching;
    println!("  Total queries: {}", context.total_queries);
    println!("  MRR: {:.3}", context.mrr);
    for (k, p) in &context.precision_at_k {
        println!("  P@{}: {:.3}", k, p);
    }
    if let Some(ndcg) = context.ndcg_at_k.get(&10) {
        println!("  NDCG@10: {:.3}", ndcg);
    }

    // Phase 3: Asymmetric Validation
    println!("\n=== Phase 3: Asymmetric Validation ===");
    let asym = &results.metrics.asymmetric_retrieval;
    println!("  Total queries: {}", asym.total_queries);
    println!(
        "  Paraphrase→Context modifier: {}",
        asym.paraphrase_to_context_modifier
    );
    println!(
        "  Context→Paraphrase modifier: {}",
        asym.context_to_paraphrase_modifier
    );
    println!(
        "  Observed ratio: {:.2} (expected 1.0 - E5-base-v2 handles asymmetry via prefixes)",
        asym.observed_asymmetry_ratio
    );
    println!(
        "  Formula compliant: {}",
        if asym.formula_compliant { "YES" } else { "NO" }
    );

    if verbose {
        println!("  Wins breakdown:");
        println!(
            "    Paraphrase→Context wins: {}",
            asym.paraphrase_to_context_wins
        );
        println!(
            "    Context→Paraphrase wins: {}",
            asym.context_to_paraphrase_wins
        );
        println!("    Ties: {}", asym.ties);
        println!(
            "    E10 contribution: {:.1}%",
            asym.e10_contribution_percentage
        );
    }

    // Phase 4: Ablation Study
    if let Some(ablation) = &results.metrics.ablation {
        println!("\n=== Phase 4: Ablation Study ===");
        println!("  E1 only MRR: {:.3}", ablation.e1_only_mrr);
        println!("  E10 only MRR: {:.3}", ablation.e10_only_mrr);
        println!("  E1+E10 blend MRR: {:.3}", ablation.e1_e10_blend_mrr);
        println!("  Full 13-space MRR: {:.3}", ablation.full_13_space_mrr);
        println!(
            "  E10 contribution: +{:.3} (+{:.1}%)",
            ablation.e10_contribution, ablation.e10_contribution_percentage
        );

        // Find optimal blend
        if let Some(best) = ablation
            .blend_analysis
            .iter()
            .max_by(|a, b| a.mrr.partial_cmp(&b.mrr).unwrap())
        {
            println!(
                "  Optimal blend: {:.1} (MRR={:.3})",
                best.blend_value, best.mrr
            );
        }

        if verbose {
            println!("  Blend sweep:");
            for point in &ablation.blend_analysis {
                println!(
                    "    blend={:.1}: MRR={:.3}, P@5={:.3}",
                    point.blend_value, point.mrr, point.precision_at_5
                );
            }
        }
    }

    // Timings
    println!("\n=== Timings ===");
    let timings = &results.timings;
    println!("  Dataset generation: {}ms", timings.dataset_generation_ms);
    println!(
        "  Paraphrase detection: {}ms",
        timings.paraphrase_detection_ms
    );
    println!("  Context matching: {}ms", timings.context_matching_ms);
    println!(
        "  Asymmetric validation: {}ms",
        timings.asymmetric_validation_ms
    );
    if let Some(ablation_ms) = timings.ablation_ms {
        println!("  Ablation study: {}ms", ablation_ms);
    }
    println!("  Total: {}ms", timings.total_ms);
}

fn verify_formula_compliance(results: &E10MultimodalBenchmarkResults) -> bool {
    let asym = &results.metrics.asymmetric_retrieval;

    // E5-base-v2 handles asymmetry via prefixes - neutral modifiers (1.0/1.0), ratio ~1.0
    let i2c_ok = (asym.paraphrase_to_context_modifier - 1.0).abs() < 0.001;
    let c2i_ok = (asym.context_to_paraphrase_modifier - 1.0).abs() < 0.001;
    let ratio_ok = (asym.observed_asymmetry_ratio - 1.0).abs() < 0.1;

    i2c_ok && c2i_ok && ratio_ok
}

fn build_summary(
    results: &E10MultimodalBenchmarkResults,
    formula_compliant: bool,
) -> BenchmarkSummary {
    let para = &results.metrics.paraphrase_detection;
    let context = &results.metrics.context_matching;
    let asym = &results.metrics.asymmetric_retrieval;

    // Find optimal blend value
    let optimal_blend = results.metrics.ablation.as_ref().and_then(|abl| {
        abl.blend_analysis
            .iter()
            .max_by(|a, b| a.mrr.partial_cmp(&b.mrr).unwrap())
            .map(|p| p.blend_value)
    });

    // Determine pass/fail
    // Success criteria from benchmark plan:
    // - MRR > 0.6 on quality benchmarks
    // - Asymmetry modifiers applied correctly (within 1% tolerance)
    // - Direction detection works
    let mrr_passes = context.mrr > 0.6;
    let asymmetry_passes = formula_compliant;
    let paraphrase_passes = para.accuracy > 0.7;

    let all_passed = mrr_passes && asymmetry_passes && paraphrase_passes;

    BenchmarkSummary {
        // Paraphrase Detection
        paraphrase_detection_accuracy: para.accuracy,
        paraphrase_precision: para.paraphrase_precision,
        paraphrase_recall: para.paraphrase_recall,

        // Context Matching
        mrr: context.mrr,
        precision_at_1: *context.precision_at_k.get(&1).unwrap_or(&0.0),
        precision_at_5: *context.precision_at_k.get(&5).unwrap_or(&0.0),
        precision_at_10: *context.precision_at_k.get(&10).unwrap_or(&0.0),
        ndcg_at_10: *context.ndcg_at_k.get(&10).unwrap_or(&0.0),

        // Asymmetric Validation
        asymmetry_ratio: asym.observed_asymmetry_ratio,
        paraphrase_to_context_modifier: asym.paraphrase_to_context_modifier,
        context_to_paraphrase_modifier: asym.context_to_paraphrase_modifier,
        formula_compliant,

        // Ablation
        e1_only_mrr: results.metrics.ablation.as_ref().map(|a| a.e1_only_mrr),
        e10_only_mrr: results.metrics.ablation.as_ref().map(|a| a.e10_only_mrr),
        e1_e10_blend_mrr: results
            .metrics
            .ablation
            .as_ref()
            .map(|a| a.e1_e10_blend_mrr),
        e10_contribution_percentage: results
            .metrics
            .ablation
            .as_ref()
            .map(|a| a.e10_contribution_percentage),
        optimal_blend_value: optimal_blend,

        // Performance
        total_duration_ms: results.timings.total_ms,

        // Pass/Fail
        all_tests_passed: all_passed,
    }
}

fn print_summary(summary: &BenchmarkSummary) {
    println!("\n=== Summary ===");

    // Paraphrase Detection
    let paraphrase_status = if summary.paraphrase_detection_accuracy > 0.7 {
        "✓"
    } else {
        "✗"
    };
    println!(
        "{} Paraphrase detection accuracy: {:.1}% (target: >70%)",
        paraphrase_status,
        summary.paraphrase_detection_accuracy * 100.0
    );

    // MRR
    let mrr_status = if summary.mrr > 0.6 { "✓" } else { "✗" };
    println!(
        "{} Context matching MRR: {:.3} (target: >0.6)",
        mrr_status, summary.mrr
    );

    // Asymmetry
    let asym_status = if summary.formula_compliant {
        "✓"
    } else {
        "✗"
    };
    println!(
        "{} Asymmetry ratio: {:.2} (target: 1.0 ±0.1 - E5-base-v2 handles asymmetry via prefixes)",
        asym_status, summary.asymmetry_ratio
    );
    println!(
        "  Paraphrase→Context: {} (expected 1.0)",
        summary.paraphrase_to_context_modifier
    );
    println!(
        "  Context→Paraphrase: {} (expected 1.0)",
        summary.context_to_paraphrase_modifier
    );

    // Ablation
    if let (Some(e1), Some(blend), Some(contrib)) = (
        summary.e1_only_mrr,
        summary.e1_e10_blend_mrr,
        summary.e10_contribution_percentage,
    ) {
        let ablation_status = if blend > e1 { "✓" } else { "✗" };
        println!(
            "{} E10 contribution: +{:.1}% (E1={:.3} → E1+E10={:.3})",
            ablation_status, contrib, e1, blend
        );

        if let Some(optimal) = summary.optimal_blend_value {
            println!("  Optimal blend value: {:.1}", optimal);
        }
    }

    // Duration
    println!("  Duration: {}ms", summary.total_duration_ms);
}
