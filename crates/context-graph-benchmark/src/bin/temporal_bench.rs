//! Temporal benchmark binary for evaluating E2/E3/E4 embedder effectiveness.
//!
//! This binary runs comprehensive temporal benchmarks and outputs results
//! in JSON and Markdown formats.
//!
//! # Usage
//!
//! ```bash
//! # Run full benchmark suite
//! cargo run -p context-graph-benchmark --bin temporal_bench -- --mode full
//!
//! # Run E2 recency benchmarks with adaptive half-life
//! cargo run -p context-graph-benchmark --bin temporal_bench -- \
//!   --mode e2-recency --adaptive-half-life \
//!   --decay-functions linear,exponential,step
//!
//! # Run ablation study with different weight configs
//! cargo run -p context-graph-benchmark --bin temporal_bench -- \
//!   --mode ablation --weight-configs "50/15/35,40/30/30,100/0/0"
//!
//! # Run scaling analysis across corpus sizes
//! cargo run -p context-graph-benchmark --bin temporal_bench -- \
//!   --mode scaling --corpus-sizes 1000,5000,10000,50000
//!
//! # Run regression test against baseline
//! cargo run -p context-graph-benchmark --bin temporal_bench -- \
//!   --mode regression --baseline baselines/temporal_baseline.json
//! ```

use std::env;
use std::path::PathBuf;

use context_graph_benchmark::datasets::temporal::{
    PeriodicPatternConfig, RecencyDistributionConfig, SequenceChainConfig, TemporalDatasetConfig,
};
use context_graph_benchmark::runners::temporal::{
    generate_temporal_report, run_ablation_benchmark, run_e2_recency_benchmark,
    run_e3_periodic_benchmark, run_e4_sequence_benchmark, run_regression_benchmark,
    run_scaling_benchmark, AblationConfig, PeriodicBenchmarkSettings, RecencyBenchmarkSettings,
    RegressionConfig, ScalingConfig, SequenceBenchmarkSettings, TemporalBenchmarkConfig,
    TemporalBenchmarkRunner,
};

/// Benchmark modes for targeted testing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BenchmarkMode {
    /// Full benchmark suite - all E2/E3/E4 tests
    Full,
    /// E2-focused recency tests: decay functions, adaptive half-life, time windows
    E2Recency,
    /// E3-focused periodic tests: hourly/weekly patterns, silhouette validation
    E3Periodic,
    /// E4-focused sequence tests: before/after, chain length, between queries
    E4Sequence,
    /// Weight configuration ablation study
    Ablation,
    /// Corpus size scaling analysis
    Scaling,
    /// Regression testing against baseline
    Regression,
}

impl BenchmarkMode {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "full" => Some(Self::Full),
            "e2-recency" | "e2_recency" | "recency" => Some(Self::E2Recency),
            "e3-periodic" | "e3_periodic" | "periodic" => Some(Self::E3Periodic),
            "e4-sequence" | "e4_sequence" | "sequence" => Some(Self::E4Sequence),
            "ablation" => Some(Self::Ablation),
            "scaling" => Some(Self::Scaling),
            "regression" => Some(Self::Regression),
            _ => None,
        }
    }
}

/// CLI configuration with enhanced options.
struct CliConfig {
    // Mode selection
    mode: BenchmarkMode,

    // Dataset parameters
    num_memories: usize,
    num_queries: usize,
    time_span_days: u32,
    seed: u64,

    // Output paths
    output: Option<PathBuf>,
    report: Option<PathBuf>,

    // Recency (E2) options
    decay_half_life_hours: u32,
    fresh_threshold_hours: u32,
    decay_functions: Vec<String>,
    adaptive_half_life: bool,

    // Periodic (E3) options
    peak_hours: Vec<u8>,
    peak_days: Vec<u8>,

    // Sequence (E4) options
    num_chains: usize,
    chain_length: usize,
    test_between: bool,
    chain_lengths: Vec<usize>,

    // Ablation options
    weight_configs: Vec<(f32, f32, f32)>,
    skip_ablation: bool,

    // Scaling options
    corpus_sizes: Vec<usize>,

    // Regression options
    baseline_path: Option<PathBuf>,
    tolerance_accuracy: f64,
    tolerance_latency: f64,

    // Output control
    verbose: bool,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            mode: BenchmarkMode::Full,
            num_memories: 1000,
            num_queries: 100,
            time_span_days: 30,
            seed: 42,
            output: None,
            report: None,
            decay_half_life_hours: 24,
            fresh_threshold_hours: 24,
            decay_functions: vec![
                "linear".to_string(),
                "exponential".to_string(),
                "step".to_string(),
            ],
            adaptive_half_life: false,
            peak_hours: vec![9, 10, 11, 14, 15, 16],
            peak_days: vec![1, 2, 3, 4, 5],
            num_chains: 50,
            chain_length: 10,
            test_between: false,
            chain_lengths: vec![3, 5, 10, 20, 50],
            weight_configs: vec![
                (0.50, 0.15, 0.35), // Optimized
                (0.40, 0.30, 0.30), // Legacy
                (1.0, 0.0, 0.0),    // E2 only
                (0.0, 1.0, 0.0),    // E3 only
                (0.0, 0.0, 1.0),    // E4 only
            ],
            skip_ablation: false,
            corpus_sizes: vec![1000, 5000, 10000, 50000, 100000],
            baseline_path: None,
            tolerance_accuracy: 0.05,
            tolerance_latency: 0.10,
            verbose: false,
        }
    }
}

fn parse_args() -> CliConfig {
    let args: Vec<String> = env::args().collect();
    let mut config = CliConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                i += 1;
                if i < args.len() {
                    if let Some(mode) = BenchmarkMode::from_str(&args[i]) {
                        config.mode = mode;
                    } else {
                        eprintln!("Unknown mode: {}. Valid modes: full, e2-recency, e3-periodic, e4-sequence, ablation, scaling, regression", args[i]);
                    }
                }
            }
            "--num-memories" => {
                i += 1;
                if i < args.len() {
                    config.num_memories = args[i].parse().unwrap_or(config.num_memories);
                }
            }
            "--num-queries" => {
                i += 1;
                if i < args.len() {
                    config.num_queries = args[i].parse().unwrap_or(config.num_queries);
                }
            }
            "--time-span-days" => {
                i += 1;
                if i < args.len() {
                    config.time_span_days = args[i].parse().unwrap_or(config.time_span_days);
                }
            }
            "--seed" => {
                i += 1;
                if i < args.len() {
                    config.seed = args[i].parse().unwrap_or(config.seed);
                }
            }
            "-o" | "--output" => {
                i += 1;
                if i < args.len() {
                    config.output = Some(PathBuf::from(&args[i]));
                }
            }
            "--report" => {
                i += 1;
                if i < args.len() {
                    config.report = Some(PathBuf::from(&args[i]));
                }
            }
            "--decay-half-life-hours" => {
                i += 1;
                if i < args.len() {
                    config.decay_half_life_hours =
                        args[i].parse().unwrap_or(config.decay_half_life_hours);
                }
            }
            "--fresh-threshold-hours" => {
                i += 1;
                if i < args.len() {
                    config.fresh_threshold_hours =
                        args[i].parse().unwrap_or(config.fresh_threshold_hours);
                }
            }
            "--decay-functions" => {
                i += 1;
                if i < args.len() {
                    config.decay_functions =
                        args[i].split(',').map(|s| s.trim().to_string()).collect();
                }
            }
            "--adaptive-half-life" => {
                config.adaptive_half_life = true;
            }
            "--peak-hours" => {
                i += 1;
                if i < args.len() {
                    config.peak_hours = args[i]
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                }
            }
            "--peak-days" => {
                i += 1;
                if i < args.len() {
                    config.peak_days = args[i]
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                }
            }
            "--num-chains" => {
                i += 1;
                if i < args.len() {
                    config.num_chains = args[i].parse().unwrap_or(config.num_chains);
                }
            }
            "--chain-length" => {
                i += 1;
                if i < args.len() {
                    config.chain_length = args[i].parse().unwrap_or(config.chain_length);
                }
            }
            "--test-between" => {
                config.test_between = true;
            }
            "--chain-lengths" => {
                i += 1;
                if i < args.len() {
                    config.chain_lengths = args[i]
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                }
            }
            "--weight-configs" => {
                i += 1;
                if i < args.len() {
                    config.weight_configs = parse_weight_configs(&args[i]);
                }
            }
            "--skip-ablation" => {
                config.skip_ablation = true;
            }
            "--corpus-sizes" => {
                i += 1;
                if i < args.len() {
                    config.corpus_sizes = args[i]
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                }
            }
            "--baseline" => {
                i += 1;
                if i < args.len() {
                    config.baseline_path = Some(PathBuf::from(&args[i]));
                }
            }
            "--tolerance-accuracy" => {
                i += 1;
                if i < args.len() {
                    config.tolerance_accuracy =
                        args[i].parse().unwrap_or(config.tolerance_accuracy);
                }
            }
            "--tolerance-latency" => {
                i += 1;
                if i < args.len() {
                    config.tolerance_latency = args[i].parse().unwrap_or(config.tolerance_latency);
                }
            }
            "-v" | "--verbose" => {
                config.verbose = true;
            }
            "-h" | "--help" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    config
}

fn parse_weight_configs(s: &str) -> Vec<(f32, f32, f32)> {
    s.split(',')
        .filter_map(|config| {
            let parts: Vec<f32> = config
                .trim()
                .split('/')
                .filter_map(|p| p.trim().parse().ok())
                .collect();
            if parts.len() == 3 {
                // Normalize to [0,1] if given as percentages
                let (e2, e3, e4) = (parts[0], parts[1], parts[2]);
                if e2 + e3 + e4 > 3.0 {
                    // Given as percentages (50/15/35)
                    Some((e2 / 100.0, e3 / 100.0, e4 / 100.0))
                } else {
                    Some((e2, e3, e4))
                }
            } else {
                None
            }
        })
        .collect()
}

fn print_help() {
    eprintln!("Temporal benchmark for E2/E3/E4 embedders");
    eprintln!();
    eprintln!("Usage: temporal_bench [OPTIONS]");
    eprintln!();
    eprintln!("MODE SELECTION:");
    eprintln!("  --mode <MODE>               Benchmark mode (default: full)");
    eprintln!("                              full        - All benchmarks");
    eprintln!("                              e2-recency  - E2-focused tests");
    eprintln!("                              e3-periodic - E3-focused tests");
    eprintln!("                              e4-sequence - E4-focused tests");
    eprintln!("                              ablation    - Weight configuration comparison");
    eprintln!("                              scaling     - Corpus size scaling");
    eprintln!("                              regression  - Compare against baseline");
    eprintln!();
    eprintln!("DATASET OPTIONS:");
    eprintln!("  --num-memories <N>          Number of memories to generate (default: 1000)");
    eprintln!("  --num-queries <N>           Number of queries per benchmark (default: 100)");
    eprintln!("  --time-span-days <N>        Time span in days (default: 30)");
    eprintln!("  --seed <N>                  Random seed (default: 42)");
    eprintln!();
    eprintln!("OUTPUT OPTIONS:");
    eprintln!("  -o, --output <PATH>         Output JSON file path");
    eprintln!("  --report <PATH>             Output Markdown report path");
    eprintln!("  -v, --verbose               Verbose output");
    eprintln!();
    eprintln!("E2 RECENCY OPTIONS:");
    eprintln!("  --decay-half-life-hours <N> Decay half-life in hours (default: 24)");
    eprintln!("  --fresh-threshold-hours <N> Fresh threshold in hours (default: 24)");
    eprintln!("  --decay-functions <LIST>    Comma-separated decay functions (default: linear,exponential,step)");
    eprintln!("  --adaptive-half-life        Enable adaptive half-life testing");
    eprintln!();
    eprintln!("E3 PERIODIC OPTIONS:");
    eprintln!("  --peak-hours <H,H,H>        Peak hours (default: 9,10,11,14,15,16)");
    eprintln!("  --peak-days <D,D,D>         Peak days (0=Sun, default: 1,2,3,4,5)");
    eprintln!();
    eprintln!("E4 SEQUENCE OPTIONS:");
    eprintln!("  --num-chains <N>            Number of sequence chains (default: 50)");
    eprintln!("  --chain-length <N>          Average chain length (default: 10)");
    eprintln!("  --chain-lengths <LIST>      Chain lengths for scaling (default: 3,5,10,20,50)");
    eprintln!("  --test-between              Test multi-anchor 'between' queries");
    eprintln!();
    eprintln!("ABLATION OPTIONS:");
    eprintln!("  --weight-configs <CONFIGS>  Weight configs (E2/E3/E4, default: 50/15/35,40/30/30,100/0/0)");
    eprintln!("  --skip-ablation             Skip ablation study");
    eprintln!();
    eprintln!("SCALING OPTIONS:");
    eprintln!("  --corpus-sizes <SIZES>      Corpus sizes (default: 1000,5000,10000,50000,100000)");
    eprintln!();
    eprintln!("REGRESSION OPTIONS:");
    eprintln!("  --baseline <FILE>           Regression baseline JSON file");
    eprintln!("  --tolerance-accuracy <F>    Accuracy regression tolerance (default: 0.05)");
    eprintln!("  --tolerance-latency <F>     Latency regression tolerance (default: 0.10)");
    eprintln!();
    eprintln!("  -h, --help                  Show this help message");
}

fn main() {
    let args = parse_args();

    if args.verbose {
        eprintln!("Temporal Benchmark Configuration:");
        eprintln!("  Mode: {:?}", args.mode);
        eprintln!("  Memories: {}", args.num_memories);
        eprintln!("  Queries: {}", args.num_queries);
        eprintln!("  Time span: {} days", args.time_span_days);
        eprintln!("  Seed: {}", args.seed);
        if args.adaptive_half_life {
            eprintln!("  Adaptive half-life: enabled");
        }
        eprintln!();
    }

    // Dispatch to appropriate benchmark mode
    let json_output = match args.mode {
        BenchmarkMode::Full => run_full_mode(&args),
        BenchmarkMode::E2Recency => run_e2_mode(&args),
        BenchmarkMode::E3Periodic => run_e3_mode(&args),
        BenchmarkMode::E4Sequence => run_e4_mode(&args),
        BenchmarkMode::Ablation => run_ablation_mode(&args),
        BenchmarkMode::Scaling => run_scaling_mode(&args),
        BenchmarkMode::Regression => run_regression_mode(&args),
    };

    // Output results
    if let Some(output_path) = &args.output {
        std::fs::write(output_path, &json_output).expect("Failed to write JSON output");
        if args.verbose {
            eprintln!("JSON results written to: {}", output_path.display());
        }
    } else {
        println!("{}", json_output);
    }
}

fn run_full_mode(args: &CliConfig) -> String {
    eprintln!("Running full temporal benchmark suite...");

    // Build dataset config
    let dataset_config = TemporalDatasetConfig {
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        time_span_days: args.time_span_days,
        base_timestamp: None,
        seed: args.seed,
        recency_config: RecencyDistributionConfig {
            distribution: "exponential".to_string(),
            decay_rate: 2.0,
            num_clusters: 5,
            fresh_fraction: 0.2,
            fresh_threshold_hours: args.fresh_threshold_hours,
        },
        periodic_config: PeriodicPatternConfig {
            peak_hours: args.peak_hours.clone(),
            peak_days: args.peak_days.clone(),
            concentration: 3.0,
            enable_weekly: true,
            enable_daily: true,
        },
        sequence_config: SequenceChainConfig {
            num_chains: args.num_chains,
            avg_chain_length: args.chain_length,
            length_variance: args.chain_length / 2,
            avg_gap_minutes: 5,
            session_gap_hours: 4,
        },
    };

    // Build benchmark config
    let config = TemporalBenchmarkConfig {
        dataset: dataset_config,
        recency: RecencyBenchmarkSettings {
            decay_half_life_ms: args.decay_half_life_hours as i64 * 60 * 60 * 1000,
            fresh_threshold_ms: args.fresh_threshold_hours as i64 * 60 * 60 * 1000,
            test_decay_functions: args.decay_functions.clone(),
        },
        periodic: PeriodicBenchmarkSettings::default(),
        sequence: SequenceBenchmarkSettings::default(),
        run_ablation: !args.skip_ablation,
        k_values: vec![1, 5, 10, 20],
    };

    // Run benchmarks
    let runner = TemporalBenchmarkRunner::new(config);
    let results = runner.run();

    // Print summary
    print_full_summary(&results);

    // Generate report if requested
    if let Some(report_path) = &args.report {
        let markdown_report = generate_temporal_report(&results);
        std::fs::write(report_path, &markdown_report).expect("Failed to write report");
        if args.verbose {
            eprintln!("Markdown report written to: {}", report_path.display());
        }
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn run_e2_mode(args: &CliConfig) -> String {
    eprintln!("Running E2 recency benchmark...");

    let config = RecencyBenchmarkSettings {
        decay_half_life_ms: args.decay_half_life_hours as i64 * 60 * 60 * 1000,
        fresh_threshold_ms: args.fresh_threshold_hours as i64 * 60 * 60 * 1000,
        test_decay_functions: args.decay_functions.clone(),
    };

    let dataset_config = TemporalDatasetConfig {
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        time_span_days: args.time_span_days,
        seed: args.seed,
        ..Default::default()
    };

    let results = run_e2_recency_benchmark(
        &dataset_config,
        &config,
        args.adaptive_half_life,
        if args.adaptive_half_life {
            Some(args.corpus_sizes.clone())
        } else {
            None
        },
    );

    // Print E2 summary
    eprintln!();
    eprintln!("=== E2 Recency Benchmark Results ===");
    eprintln!();
    eprintln!("Decay Function Comparison:");
    for (func, score) in &results.decay_function_scores {
        eprintln!("  {}: {:.3}", func, score);
    }
    eprintln!();
    eprintln!("Core Metrics:");
    eprintln!("  Decay accuracy: {:.3}", results.decay_accuracy);
    eprintln!(
        "  Recency-weighted MRR: {:.3}",
        results.recency_weighted_mrr
    );
    if let Some(adaptive) = &results.adaptive_half_life_results {
        eprintln!();
        eprintln!("Adaptive Half-Life Results:");
        for point in &adaptive.scaling_points {
            eprintln!(
                "  {} memories: fixed={:.3}, adaptive={:.3}",
                point.corpus_size, point.fixed_accuracy, point.adaptive_accuracy
            );
        }
    }

    // Write report if requested
    if let Some(report_path) = &args.report {
        let report = results.generate_report();
        std::fs::write(report_path, &report).expect("Failed to write report");
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn run_e3_mode(args: &CliConfig) -> String {
    eprintln!("Running E3 periodic benchmark...");

    let config = PeriodicBenchmarkSettings {
        test_hours: (0..24).collect(),
        test_days: (0..7).collect(),
    };

    let dataset_config = TemporalDatasetConfig {
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        time_span_days: args.time_span_days,
        seed: args.seed,
        periodic_config: PeriodicPatternConfig {
            peak_hours: args.peak_hours.clone(),
            peak_days: args.peak_days.clone(),
            concentration: 3.0,
            enable_weekly: true,
            enable_daily: true,
        },
        ..Default::default()
    };

    let results = run_e3_periodic_benchmark(&dataset_config, &config);

    // Print E3 summary
    eprintln!();
    eprintln!("=== E3 Periodic Benchmark Results ===");
    eprintln!();
    eprintln!("Silhouette Validation:");
    eprintln!(
        "  Hourly variance: {:.4}",
        results.silhouette_validation.hourly_variance
    );
    eprintln!(
        "  Daily variance: {:.4}",
        results.silhouette_validation.daily_variance
    );
    eprintln!(
        "  Variance check: {}",
        if results.silhouette_validation.is_valid {
            "PASS"
        } else {
            "FAIL"
        }
    );
    eprintln!();
    eprintln!("Pattern Detection:");
    eprintln!("  Hourly pattern F1: {:.3}", results.hourly_pattern_f1);
    eprintln!("  Weekly pattern F1: {:.3}", results.weekly_pattern_f1);
    eprintln!(
        "  Overall periodic score: {:.3}",
        results.overall_periodic_score
    );

    // Write report if requested
    if let Some(report_path) = &args.report {
        let report = results.generate_report();
        std::fs::write(report_path, &report).expect("Failed to write report");
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn run_e4_mode(args: &CliConfig) -> String {
    eprintln!("Running E4 sequence benchmark...");

    let config = SequenceBenchmarkSettings {
        boundary_tolerance: 2,
        test_directions: vec![
            "before".to_string(),
            "after".to_string(),
            "both".to_string(),
        ],
    };

    let dataset_config = TemporalDatasetConfig {
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        time_span_days: args.time_span_days,
        seed: args.seed,
        sequence_config: SequenceChainConfig {
            num_chains: args.num_chains,
            avg_chain_length: args.chain_length,
            length_variance: args.chain_length / 2,
            avg_gap_minutes: 5,
            session_gap_hours: 4,
        },
        ..Default::default()
    };

    let results = run_e4_sequence_benchmark(
        &dataset_config,
        &config,
        args.test_between,
        Some(args.chain_lengths.clone()),
    );

    // Print E4 summary
    eprintln!();
    eprintln!("=== E4 Sequence Benchmark Results ===");
    eprintln!();
    eprintln!("Direction Accuracy:");
    eprintln!("  Before accuracy: {:.3}", results.before_accuracy);
    eprintln!("  After accuracy: {:.3}", results.after_accuracy);
    eprintln!("  Combined: {:.3}", results.before_after_accuracy);

    // Check direction symmetry - large asymmetry indicates timestamp-based bias
    let asymmetry = (results.before_accuracy - results.after_accuracy).abs();
    if asymmetry > 0.2 {
        eprintln!("  ⚠️  WARNING: High direction asymmetry ({:.3})", asymmetry);
        eprintln!("     This may indicate sequence-based comparison is not working correctly.");
    } else {
        eprintln!("  ✓ Direction symmetry OK (asymmetry: {:.3})", asymmetry);
    }
    eprintln!();
    eprintln!("Chain Length Scaling:");
    for point in &results.chain_length_scaling {
        eprintln!(
            "  Length {}: accuracy={:.3}, tau={:.3}",
            point.length, point.sequence_accuracy, point.kendall_tau
        );
    }
    if let Some(between) = &results.between_query_results {
        eprintln!();
        eprintln!("Between Query Results:");
        eprintln!("  Precision: {:.3}", between.precision);
        eprintln!("  Recall: {:.3}", between.recall);
    }

    // Write report if requested
    if let Some(report_path) = &args.report {
        let report = results.generate_report();
        std::fs::write(report_path, &report).expect("Failed to write report");
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn run_ablation_mode(args: &CliConfig) -> String {
    eprintln!("Running ablation study...");

    let config = AblationConfig {
        weight_configs: args.weight_configs.clone(),
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        seed: args.seed,
    };

    let dataset_config = TemporalDatasetConfig {
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        time_span_days: args.time_span_days,
        seed: args.seed,
        ..Default::default()
    };

    let results = run_ablation_benchmark(&dataset_config, &config);

    // Print ablation summary
    eprintln!();
    eprintln!("=== Ablation Study Results ===");
    eprintln!();
    eprintln!("Weight Configuration Scores:");
    for result in &results.config_results {
        eprintln!(
            "  E2={:.0}%, E3={:.0}%, E4={:.0}%: combined={:.3}",
            result.e2_weight * 100.0,
            result.e3_weight * 100.0,
            result.e4_weight * 100.0,
            result.combined_score
        );
    }
    eprintln!();
    eprintln!("Interference Analysis:");
    eprintln!(
        "  Max individual score: {:.3}",
        results.interference.max_individual_score
    );
    eprintln!(
        "  Best combined score: {:.3}",
        results.interference.best_combined_score
    );
    eprintln!(
        "  Interference score: {:+.3}",
        results.interference.interference_score
    );
    eprintln!(
        "  Has negative interference: {}",
        results.interference.has_negative_interference
    );
    if results.interference.has_negative_interference {
        eprintln!("  Recommendation: {}", results.interference.recommendation);
    }

    // Write report if requested
    if let Some(report_path) = &args.report {
        let report = results.generate_report();
        std::fs::write(report_path, &report).expect("Failed to write report");
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn run_scaling_mode(args: &CliConfig) -> String {
    eprintln!("Running scaling benchmark...");

    let config = ScalingConfig {
        corpus_sizes: args.corpus_sizes.clone(),
        num_queries: args.num_queries,
        seed: args.seed,
        time_span_days: args.time_span_days,
    };

    let results = run_scaling_benchmark(&config);

    // Print scaling summary
    eprintln!();
    eprintln!("=== Scaling Benchmark Results ===");
    eprintln!();
    eprintln!("Performance by Corpus Size:");
    eprintln!(
        "{:>10} {:>12} {:>12} {:>12} {:>10} {:>10}",
        "Corpus", "Decay Acc", "Seq Acc", "Silhouette", "P50 ms", "P95 ms"
    );
    for point in &results.scaling_points {
        eprintln!(
            "{:>10} {:>12.3} {:>12.3} {:>12.3} {:>10.1} {:>10.1}",
            point.corpus_size,
            point.decay_accuracy,
            point.sequence_accuracy,
            point.hourly_silhouette,
            point.p50_latency_ms,
            point.p95_latency_ms
        );
    }
    eprintln!();
    eprintln!("Degradation Analysis:");
    eprintln!(
        "  Decay accuracy rate: {:.4}/10x",
        results.degradation.decay_accuracy_rate
    );
    eprintln!(
        "  Sequence accuracy rate: {:.4}/10x",
        results.degradation.sequence_accuracy_rate
    );
    eprintln!(
        "  Latency growth rate: {:.4}/10x",
        results.degradation.latency_growth_rate
    );

    // Write report if requested
    if let Some(report_path) = &args.report {
        let report = results.generate_report();
        std::fs::write(report_path, &report).expect("Failed to write report");
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn run_regression_mode(args: &CliConfig) -> String {
    eprintln!("Running regression test...");

    let baseline_path = args.baseline_path.clone().unwrap_or_else(|| {
        PathBuf::from("crates/context-graph-benchmark/baselines/temporal_baseline.json")
    });

    let config = RegressionConfig {
        baseline_path,
        tolerance_accuracy: args.tolerance_accuracy,
        tolerance_latency: args.tolerance_latency,
        num_memories: args.num_memories,
        num_queries: args.num_queries,
        seed: args.seed,
    };

    let results = run_regression_benchmark(&config);

    // Print regression summary
    eprintln!();
    eprintln!("=== Regression Test Results ===");
    eprintln!();
    eprintln!("Overall: {}", if results.passed { "PASS" } else { "FAIL" });
    eprintln!();
    if !results.failures.is_empty() {
        eprintln!("Failures:");
        for failure in &results.failures {
            eprintln!(
                "  {}: baseline={:.3}, current={:.3}, delta={:+.1}%",
                failure.metric_name,
                failure.baseline_value,
                failure.current_value,
                failure.delta_percent
            );
        }
    }
    if !results.warnings.is_empty() {
        eprintln!();
        eprintln!("Warnings:");
        for warning in &results.warnings {
            eprintln!("  {}", warning);
        }
    }

    // Write report if requested
    if let Some(report_path) = &args.report {
        let report = results.generate_report();
        std::fs::write(report_path, &report).expect("Failed to write report");
    }

    serde_json::to_string_pretty(&results).expect("Failed to serialize results")
}

fn print_full_summary(
    results: &context_graph_benchmark::runners::temporal::TemporalBenchmarkResults,
) {
    eprintln!();
    eprintln!("=== Temporal Benchmark Summary ===");
    eprintln!();
    eprintln!("Dataset:");
    eprintln!("  Total memories: {}", results.dataset_stats.total_memories);
    eprintln!("  Total queries: {}", results.dataset_stats.total_queries);
    eprintln!("  Chains: {}", results.dataset_stats.chain_count);
    eprintln!();
    eprintln!("E2 Recency:");
    eprintln!(
        "  Recency-weighted MRR: {:.3}",
        results.metrics.recency.recency_weighted_mrr
    );
    eprintln!(
        "  Decay accuracy: {:.3}",
        results.metrics.recency.decay_accuracy
    );
    eprintln!();
    eprintln!("E3 Periodic:");
    eprintln!(
        "  Periodic R@10: {:.3}",
        results
            .metrics
            .periodic
            .periodic_recall_at
            .get(&10)
            .copied()
            .unwrap_or(0.0)
    );
    eprintln!(
        "  Hourly cluster quality: {:.3}",
        results.metrics.periodic.hourly_cluster_quality
    );
    eprintln!();
    eprintln!("E4 Sequence:");
    eprintln!(
        "  Sequence accuracy: {:.3}",
        results.metrics.sequence.sequence_accuracy
    );
    eprintln!(
        "  Before/after accuracy: {:.3}",
        results.metrics.sequence.before_after_accuracy
    );
    eprintln!();
    eprintln!("Overall:");
    eprintln!(
        "  Temporal score: {:.3}",
        results.metrics.composite.overall_temporal_score
    );
    if let Some(ablation) = &results.ablation {
        eprintln!(
            "  Improvement over baseline: {:+.1}%",
            (ablation.full_score - ablation.baseline_score) / ablation.baseline_score.max(0.01)
                * 100.0
        );
    }
    eprintln!();
    eprintln!("Timings:");
    eprintln!("  Total: {}ms", results.timings.total_ms);
    eprintln!(
        "  Dataset generation: {}ms",
        results.timings.dataset_generation_ms
    );
}
