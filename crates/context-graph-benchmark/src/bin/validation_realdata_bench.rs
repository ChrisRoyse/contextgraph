#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! Unified Real Data Benchmark Validation Suite
//!
//! Comprehensive validation that tests all components of the unified real data
//! benchmark infrastructure against the real HuggingFace dataset (58,895 chunks).
//!
//! ## Features
//!
//! - Validates ARCH compliance from CLAUDE.md
//! - Tests EmbedderName enum and category assignments
//! - Validates temporal injector (E2/E3/E4)
//! - Tests ground truth generation
//! - Validates asymmetric embedders (E5/E8/E10)
//! - Tests E1 foundation role
//! - Validates fusion strategies
//! - Generates optimization recommendations
//!
//! ## Usage
//!
//! ```bash
//! # Run full validation suite
//! cargo run -p context-graph-benchmark --bin validation-realdata-bench --release -- \
//!   --data-dir data/hf_benchmark_diverse \
//!   --max-chunks 10000 \
//!   --output benchmark_results/validation
//!
//! # Run specific phases
//! cargo run -p context-graph-benchmark --bin validation-realdata-bench -- \
//!   --phases config,arch,asymmetric \
//!   --output benchmark_results/validation_quick
//!
//! # Quick validation with limited data
//! cargo run -p context-graph-benchmark --bin validation-realdata-bench -- \
//!   --max-chunks 1000 --fail-fast
//! ```

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;

use context_graph_benchmark::validation::{
    ValidationBenchmarkConfig, ValidationBenchmarkResults, ValidationPhase,
    ValidationDatasetInfo, ValidationSummary, PhaseResult, ValidationReportGenerator,
    generate_console_summary, Recommendation, RecommendationCategory,
    BenchmarkResultsAggregator,
};
use context_graph_benchmark::validation::config_validation::ConfigValidator;
use context_graph_benchmark::validation::temporal_validation::TemporalValidator;
use context_graph_benchmark::validation::ground_truth_validation::GroundTruthValidator;
use context_graph_benchmark::validation::arch_compliance::ArchComplianceValidator;
use context_graph_benchmark::validation::asymmetric_validation::AsymmetricValidator;
use context_graph_benchmark::validation::ablation_validation::AblationValidator;
use context_graph_benchmark::validation::e1_foundation::E1FoundationValidator;
use context_graph_benchmark::validation::fusion_validation::FusionValidator;

use context_graph_benchmark::realdata::config::TemporalInjectionConfig;
use context_graph_benchmark::realdata::loader::DatasetLoader;
use context_graph_benchmark::realdata::temporal_injector::TemporalMetadataInjector;
use context_graph_benchmark::realdata::ground_truth::GroundTruthGenerator;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    /// Directory containing chunks.jsonl and metadata.json.
    data_dir: PathBuf,
    /// Directory containing benchmark results (for aggregation).
    results_dir: PathBuf,
    /// Maximum chunks to load (0 = unlimited).
    max_chunks: usize,
    /// Phases to run (empty = all).
    phases: Vec<ValidationPhase>,
    /// Output directory for results.
    output_dir: PathBuf,
    /// Show verbose output.
    verbose: bool,
    /// Stop on first critical failure.
    fail_fast: bool,
    /// Random seed.
    seed: u64,
    /// Number of queries for metric validation.
    num_queries: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark_diverse"),
            results_dir: PathBuf::from("benchmark_results"),
            max_chunks: 10000,
            phases: vec![],
            output_dir: PathBuf::from("benchmark_results/validation"),
            verbose: true,
            fail_fast: false,
            seed: 42,
            num_queries: 50,
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
            "--results-dir" | "-r" => {
                args.results_dir = PathBuf::from(argv.next().expect("--results-dir requires a value"));
            }
            "--max-chunks" | "-n" => {
                args.max_chunks = argv
                    .next()
                    .expect("--max-chunks requires a value")
                    .parse()
                    .expect("--max-chunks must be a number");
            }
            "--phases" | "-p" => {
                let phase_str = argv.next().expect("--phases requires a value");
                args.phases = phase_str
                    .split(',')
                    .filter_map(|s| ValidationPhase::from_str(s.trim()))
                    .collect();
            }
            "--output" | "-o" => {
                args.output_dir = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--verbose" | "-v" => {
                args.verbose = true;
            }
            "--quiet" | "-q" => {
                args.verbose = false;
            }
            "--fail-fast" | "-f" => {
                args.fail_fast = true;
            }
            "--seed" => {
                args.seed = argv
                    .next()
                    .expect("--seed requires a value")
                    .parse()
                    .expect("--seed must be a number");
            }
            "--num-queries" => {
                args.num_queries = argv
                    .next()
                    .expect("--num-queries requires a value")
                    .parse()
                    .expect("--num-queries must be a number");
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

    // Default to all phases if none specified
    if args.phases.is_empty() {
        args.phases = ValidationPhase::all();
    }

    args
}

fn print_usage() {
    eprintln!(
        r#"
Unified Real Data Benchmark Validation Suite

USAGE:
    validation-realdata-bench [OPTIONS]

OPTIONS:
    --data-dir, -d <PATH>       Directory with chunks.jsonl and metadata.json
                                Default: data/hf_benchmark_diverse

    --results-dir, -r <PATH>    Directory with benchmark results (for aggregation)
                                Default: benchmark_results

    --max-chunks, -n <NUM>      Maximum chunks to load (0 = unlimited)
                                Default: 10000

    --phases, -p <LIST>         Comma-separated phases to run
                                Options: config, temporal, ground_truth, arch,
                                         asymmetric, ablation, e1_foundation, fusion
                                Default: all phases

    --output, -o <PATH>         Output directory for results
                                Default: benchmark_results/validation

    --verbose, -v               Show verbose output (default)

    --quiet, -q                 Suppress progress output

    --fail-fast, -f             Stop on first critical failure

    --seed <NUM>                Random seed for reproducibility
                                Default: 42

    --num-queries <NUM>         Number of queries for metric validation
                                Default: 50

    --help, -h                  Show this help message

PHASES:
    config          - EmbedderName enum and category validation
    temporal        - E2/E3/E4 temporal injector validation
    ground_truth    - Per-embedder ground truth validation
    arch            - ARCH compliance validation (CLAUDE.md rules)
    asymmetric      - E5/E8/E10 asymmetric ratio validation
    ablation        - Contribution analysis validation
    e1_foundation   - E1 as foundation validation
    fusion          - Fusion strategy validation

EXAMPLES:
    # Full validation suite
    validation-realdata-bench --data-dir data/hf_benchmark_diverse

    # Quick validation (config + arch only)
    validation-realdata-bench --phases config,arch --max-chunks 1000

    # Fail fast on critical issues
    validation-realdata-bench --fail-fast

OUTPUT FILES:
    validation/results.json         - Full validation results
    validation/report.md            - Markdown report with PASS/FAIL
    validation/recommendations.md   - Optimization recommendations
    validation/arch_compliance.json - ARCH rule details

SUCCESS CRITERIA:
    - All 13 embedders evaluated (13/13)
    - Embedder categories: 7+3+2+1 = 13
    - Topic weights: Semantic=1.0, Temporal=0.0, Others=0.5
    - ARCH-09 threshold: weighted_agreement >= 2.5
    - AP-73: E2-E4 not in fusion
    - E5/E8/E10 asymmetric ratio: 1.35-1.65
    - E1 MRR baseline: > 0.60
"#
    );
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let args = parse_args();
    let start_time = Utc::now();
    let total_start = Instant::now();

    if args.verbose {
        eprintln!("=== Unified Real Data Benchmark Validation ===");
        eprintln!("Data directory: {}", args.data_dir.display());
        eprintln!("Max chunks: {}", if args.max_chunks == 0 { "unlimited".to_string() } else { args.max_chunks.to_string() });
        eprintln!("Phases: {:?}", args.phases.iter().map(|p| p.as_str()).collect::<Vec<_>>());
        eprintln!("Output: {}", args.output_dir.display());
        eprintln!("Fail fast: {}", args.fail_fast);
        eprintln!();
    }

    // Create output directory
    if let Err(e) = fs::create_dir_all(&args.output_dir) {
        eprintln!("Failed to create output directory: {}", e);
        std::process::exit(1);
    }

    // Build config
    let config = ValidationBenchmarkConfig {
        data_dir: args.data_dir.clone(),
        max_chunks: args.max_chunks,
        phases: args.phases.clone(),
        output_dir: args.output_dir.clone(),
        verbose: args.verbose,
        fail_fast: args.fail_fast,
        seed: args.seed,
        num_queries: args.num_queries,
    };

    // Initialize results
    let mut phase_results: HashMap<ValidationPhase, PhaseResult> = HashMap::new();
    let mut dataset_info = ValidationDatasetInfo::default();
    let mut recommendations: Vec<Recommendation> = Vec::new();
    let mut critical_failure = false;

    // ========================================================================
    // Load aggregated benchmark results (if available)
    // ========================================================================
    let mut aggregator = BenchmarkResultsAggregator::new(&args.results_dir);
    let _ = aggregator.load_all();

    let measured_ratios = if aggregator.has_results() {
        if args.verbose {
            eprintln!("Loaded benchmark results from: {}", args.results_dir.display());
            eprintln!("  Files loaded: {:?}", aggregator.loaded_files());
        }
        Some(aggregator.get_all_asymmetric_ratios())
    } else {
        if args.verbose {
            eprintln!("No benchmark results found in {}. Using structural validation only.", args.results_dir.display());
        }
        None
    };

    // ========================================================================
    // Phase: Config Validation
    // ========================================================================
    if args.phases.contains(&ValidationPhase::Config) {
        if args.verbose {
            eprintln!("Running Config Validation...");
        }
        let phase_start = Instant::now();
        let result = ConfigValidator::validate();
        let phase_result = ConfigValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

        if !phase_result.all_passed {
            critical_failure = true;
            if args.fail_fast {
                eprintln!("Config validation failed (fail-fast enabled)");
            }
        }
        phase_results.insert(ValidationPhase::Config, phase_result);
    }

    // ========================================================================
    // Phase: ARCH Compliance Validation
    // ========================================================================
    if args.phases.contains(&ValidationPhase::Arch) && (!args.fail_fast || !critical_failure) {
        if args.verbose {
            eprintln!("Running ARCH Compliance Validation...");
        }
        let phase_start = Instant::now();
        let result = ArchComplianceValidator::validate();
        let phase_result = ArchComplianceValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

        if !phase_result.all_passed {
            critical_failure = true;
        }
        phase_results.insert(ValidationPhase::Arch, phase_result);
    }

    // ========================================================================
    // Phase: Asymmetric Validation
    // ========================================================================
    if args.phases.contains(&ValidationPhase::Asymmetric) && (!args.fail_fast || !critical_failure) {
        if args.verbose {
            eprintln!("Running Asymmetric Validation...");
            if let Some(ref ratios) = measured_ratios {
                eprintln!("  Using measured ratios: {:?}", ratios);
            }
        }
        let phase_start = Instant::now();
        let result = AsymmetricValidator::validate(measured_ratios.as_ref(), None);
        recommendations.extend(result.recommendations.clone());
        let phase_result = AsymmetricValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

        if !phase_result.all_passed {
            critical_failure = true;
        }
        phase_results.insert(ValidationPhase::Asymmetric, phase_result);
    }

    // ========================================================================
    // Phase: Ablation Validation
    // ========================================================================
    if args.phases.contains(&ValidationPhase::Ablation) && (!args.fail_fast || !critical_failure) {
        if args.verbose {
            eprintln!("Running Ablation Validation...");
        }
        let phase_start = Instant::now();
        // Pass aggregated results for measured contribution validation
        let aggregated = if aggregator.has_results() { Some(aggregator.results()) } else { None };
        let result = AblationValidator::validate(None, aggregated);
        recommendations.extend(result.recommendations.clone());
        let phase_result = AblationValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

        if !phase_result.all_passed {
            critical_failure = true;
        }
        phase_results.insert(ValidationPhase::Ablation, phase_result);
    }

    // ========================================================================
    // Phase: E1 Foundation Validation
    // ========================================================================
    if args.phases.contains(&ValidationPhase::E1Foundation) && (!args.fail_fast || !critical_failure) {
        if args.verbose {
            eprintln!("Running E1 Foundation Validation...");
        }
        let phase_start = Instant::now();

        // Get E1 MRR from aggregator if available
        let e1_mrr = aggregator.get_e1_baseline_mrr();
        if args.verbose && e1_mrr.is_some() {
            eprintln!("  Using measured E1 MRR: {:.3}", e1_mrr.unwrap());
        }

        let result = E1FoundationValidator::validate(e1_mrr, None, None, None, None);
        recommendations.extend(result.recommendations.clone());
        let phase_result = E1FoundationValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

        if !phase_result.all_passed {
            critical_failure = true;
        }
        phase_results.insert(ValidationPhase::E1Foundation, phase_result);
    }

    // ========================================================================
    // Phase: Fusion Validation
    // ========================================================================
    if args.phases.contains(&ValidationPhase::Fusion) && (!args.fail_fast || !critical_failure) {
        if args.verbose {
            eprintln!("Running Fusion Validation...");
        }
        let phase_start = Instant::now();
        let result = FusionValidator::validate(None);
        recommendations.extend(result.recommendations.clone());
        let phase_result = FusionValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

        if !phase_result.all_passed {
            critical_failure = true;
        }
        phase_results.insert(ValidationPhase::Fusion, phase_result);
    }

    // ========================================================================
    // Phases requiring data: Temporal, GroundTruth
    // ========================================================================
    let data_phases = [ValidationPhase::Temporal, ValidationPhase::GroundTruth];
    let need_data = args.phases.iter().any(|p| data_phases.contains(p));

    if need_data && (!args.fail_fast || !critical_failure) {
        // Try to load dataset
        if args.verbose {
            eprintln!("Loading dataset from {}...", args.data_dir.display());
        }

        let loader = DatasetLoader::new().with_max_chunks(args.max_chunks);
        match loader.load_from_dir(&args.data_dir) {
            Ok(dataset) => {
                dataset_info.loaded = true;
                dataset_info.total_chunks = dataset.chunks.len();
                dataset_info.chunks_used = dataset.chunks.len();
                dataset_info.num_topics = dataset.topic_to_idx.len();
                dataset_info.num_documents = dataset.metadata.total_documents;

                if args.verbose {
                    eprintln!("Loaded {} chunks, {} topics", dataset_info.chunks_used, dataset_info.num_topics);
                }

                // Phase: Temporal Validation
                let temporal_metadata = if args.phases.contains(&ValidationPhase::Temporal) && (!args.fail_fast || !critical_failure) {
                    if args.verbose {
                        eprintln!("Running Temporal Validation...");
                    }
                    let phase_start = Instant::now();

                    let temporal_config = TemporalInjectionConfig::default();
                    let mut injector = TemporalMetadataInjector::new(temporal_config.clone(), args.seed);
                    let metadata = injector.inject(&dataset);

                    // Get measured temporal metrics from aggregator
                    let measured_temporal = aggregator.get_temporal_metrics();
                    let result = TemporalValidator::validate(&temporal_config, Some(&metadata), dataset.chunks.len(), measured_temporal);
                    let phase_result = TemporalValidator::to_phase_result(result, phase_start.elapsed().as_millis() as u64);

                    if !phase_result.all_passed {
                        critical_failure = true;
                    }
                    phase_results.insert(ValidationPhase::Temporal, phase_result);
                    Some(metadata)
                } else {
                    // Generate temporal metadata anyway for ground truth phase
                    let temporal_config = TemporalInjectionConfig::default();
                    let mut injector = TemporalMetadataInjector::new(temporal_config, args.seed);
                    Some(injector.inject(&dataset))
                };

                // Phase: Ground Truth Validation
                if args.phases.contains(&ValidationPhase::GroundTruth) && (!args.fail_fast || !critical_failure) {
                    if let Some(metadata) = &temporal_metadata {
                        if args.verbose {
                            eprintln!("Running Ground Truth Validation...");
                        }
                        let gt_phase_start = Instant::now();

                        let mut gt_generator = GroundTruthGenerator::new(args.seed, args.num_queries);
                        let ground_truth = gt_generator.generate(&dataset, metadata);

                        let result = GroundTruthValidator::validate(Some(&ground_truth));
                        let phase_result = GroundTruthValidator::to_phase_result(result, gt_phase_start.elapsed().as_millis() as u64);

                        if !phase_result.all_passed {
                            critical_failure = true;
                        }
                        phase_results.insert(ValidationPhase::GroundTruth, phase_result);
                    }
                }
            }
            Err(e) => {
                dataset_info.loaded = false;
                eprintln!("Warning: Could not load dataset: {}", e);
                eprintln!("Skipping data-dependent phases (temporal, ground_truth)");

                // Add skip results for data phases
                for phase in &data_phases {
                    if args.phases.contains(phase) {
                        let mut phase_result = PhaseResult::new(*phase);
                        phase_result.add_check(
                            context_graph_benchmark::validation::ValidationCheck::new(
                                "dataset_load",
                                "Load dataset",
                            ).fail(&e.to_string(), "dataset available")
                        );
                        phase_result.finalize(0);
                        phase_results.insert(*phase, phase_result);
                    }
                }
            }
        }
    }

    // ========================================================================
    // Build Summary
    // ========================================================================
    let end_time = Utc::now();
    let duration_secs = total_start.elapsed().as_secs_f64();

    let mut summary = ValidationSummary::default();
    let mut arch_rules_passed = 0usize;
    let mut arch_rules_total = 0usize;

    for (phase, result) in &phase_results {
        summary.total_checks += result.passed + result.failed + result.warnings + result.skipped;
        summary.passed_checks += result.passed;
        summary.failed_checks += result.failed;
        summary.warning_checks += result.warnings;
        summary.skipped_checks += result.skipped;

        summary.per_phase.insert(*phase, context_graph_benchmark::validation::PhaseSummary {
            passed: result.passed,
            failed: result.failed,
            total: result.passed + result.failed,
        });

        // Count critical failures
        for check in &result.checks {
            if check.is_critical_failure() {
                summary.critical_failures += 1;
            }
        }

        // Count ARCH rules for ARCH phase
        if *phase == ValidationPhase::Arch {
            for check in &result.checks {
                if check.name.starts_with("ARCH-") || check.name.starts_with("AP-") {
                    arch_rules_total += 1;
                    if check.is_passed() {
                        arch_rules_passed += 1;
                    }
                }
            }
        }
    }

    summary.all_passed = summary.failed_checks == 0;
    summary.arch_rules_passed = arch_rules_passed;
    summary.arch_rules_total = arch_rules_total;

    // ========================================================================
    // Build Results
    // ========================================================================
    let results = ValidationBenchmarkResults {
        config,
        start_time,
        end_time,
        duration_secs,
        dataset_info,
        phase_results,
        summary,
        recommendations,
    };

    // ========================================================================
    // Write Output Files
    // ========================================================================
    // JSON results
    let json_path = args.output_dir.join("results.json");
    let json_file = File::create(&json_path).expect("Failed to create results.json");
    let writer = BufWriter::new(json_file);
    serde_json::to_writer_pretty(writer, &results).expect("Failed to write JSON");

    if args.verbose {
        eprintln!("\nResults written to: {}", json_path.display());
    }

    // Markdown report
    let report_path = args.output_dir.join("report.md");
    let report_gen = ValidationReportGenerator::new(results.clone());
    report_gen.write_to_file(&report_path).expect("Failed to write report");

    if args.verbose {
        eprintln!("Report written to: {}", report_path.display());
    }

    // ARCH compliance JSON
    if let Some(arch_result) = results.phase_results.get(&ValidationPhase::Arch) {
        let arch_path = args.output_dir.join("arch_compliance.json");
        let arch_file = File::create(&arch_path).expect("Failed to create arch_compliance.json");
        let writer = BufWriter::new(arch_file);
        serde_json::to_writer_pretty(writer, &arch_result).expect("Failed to write ARCH JSON");

        if args.verbose {
            eprintln!("ARCH compliance written to: {}", arch_path.display());
        }
    }

    // Recommendations markdown
    if !results.recommendations.is_empty() {
        let rec_path = args.output_dir.join("recommendations.md");
        let rec_content = generate_recommendations_markdown(&results.recommendations);
        fs::write(&rec_path, rec_content).expect("Failed to write recommendations");

        if args.verbose {
            eprintln!("Recommendations written to: {}", rec_path.display());
        }
    }

    // ========================================================================
    // Console Summary
    // ========================================================================
    let console_summary = generate_console_summary(&results);
    println!("{}", console_summary);

    // Exit code
    if results.summary.all_passed {
        if args.verbose {
            eprintln!("\nValidation PASSED");
        }
        std::process::exit(0);
    } else {
        if args.verbose {
            eprintln!("\nValidation FAILED ({} failures, {} critical)",
                results.summary.failed_checks, results.summary.critical_failures);
        }
        std::process::exit(1);
    }
}

fn generate_recommendations_markdown(recommendations: &[Recommendation]) -> String {
    let mut md = String::new();
    md.push_str("# Optimization Recommendations\n\n");

    // Critical
    let critical: Vec<_> = recommendations.iter()
        .filter(|r| matches!(r.category, RecommendationCategory::Critical))
        .collect();
    if !critical.is_empty() {
        md.push_str("## Critical Issues (Must Fix)\n\n");
        for (i, rec) in critical.iter().enumerate() {
            md.push_str(&format!("{}. [{}] {}\n", i + 1, rec.component, rec.description));
        }
        md.push_str("\n");
    }

    // Performance
    let perf: Vec<_> = recommendations.iter()
        .filter(|r| matches!(r.category, RecommendationCategory::Performance))
        .collect();
    if !perf.is_empty() {
        md.push_str("## Performance Optimizations\n\n");
        for (i, rec) in perf.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, rec.description));
        }
        md.push_str("\n");
    }

    // Parameter Tuning
    let params: Vec<_> = recommendations.iter()
        .filter(|r| matches!(r.category, RecommendationCategory::ParameterTuning))
        .collect();
    if !params.is_empty() {
        md.push_str("## Parameter Tuning\n\n");
        md.push_str("| Parameter | Current | Recommended | Reason |\n");
        md.push_str("|-----------|---------|-------------|--------|\n");
        for rec in params {
            md.push_str(&format!("| {} | {} | {} | {} |\n",
                rec.component,
                rec.current.as_deref().unwrap_or("-"),
                rec.recommended.as_deref().unwrap_or("-"),
                rec.reason));
        }
        md.push_str("\n");
    }

    // Best Practices
    let best: Vec<_> = recommendations.iter()
        .filter(|r| matches!(r.category, RecommendationCategory::BestPractice))
        .collect();
    if !best.is_empty() {
        md.push_str("## Best Practices\n\n");
        for (i, rec) in best.iter().enumerate() {
            md.push_str(&format!("{}. {}\n", i + 1, rec.description));
        }
        md.push_str("\n");
    }

    md
}
