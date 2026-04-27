//! Benchmark report generator CLI.
//!
//! Runs benchmarks and generates reports in JSON or Markdown format.
//!
//! # Usage
//!
//! ```bash
//! # Run benchmarks and output JSON
//! cargo run -p context-graph-benchmark --bin benchmark-report -- --format json --output results.json
//!
//! # Run benchmarks and output Markdown
//! cargo run -p context-graph-benchmark --bin benchmark-report -- --format markdown --output BENCHMARK_RESULTS.md
//!
//! # Run benchmarks and output both formats
//! cargo run -p context-graph-benchmark --bin benchmark-report -- --format both --output results
//!
//! # Run only specific tiers
//! cargo run -p context-graph-benchmark --bin benchmark-report -- --tiers 0,1,2 --format json
//! ```

use std::path::PathBuf;

use context_graph_benchmark::config::{BenchmarkConfig, Tier, TierConfig};
use context_graph_benchmark::reports::{BenchmarkReport, ReportFormat};
use context_graph_benchmark::runners::BenchmarkHarness;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let config = parse_args(&args);

    println!("Context Graph Benchmark Suite");
    println!("=============================\n");

    println!("Configuration:");
    println!(
        "  Tiers: {:?}",
        config
            .tiers
            .iter()
            .map(|t| t.tier.to_string())
            .collect::<Vec<_>>()
    );
    println!("  Seed: {}", config.seed);
    println!("  K values: {:?}", config.k_values);
    println!();

    // Run benchmarks
    println!("Running benchmarks...\n");
    let harness = BenchmarkHarness::with_config(config.benchmark_config.clone());
    let results = harness.run_full_suite();

    // Print summary
    println!("\n{}", results.summary());

    // Generate and write reports
    let report = BenchmarkReport::new(results);

    if let Some(output_path) = &config.output {
        println!("\nWriting report to {:?}...", output_path);
        report
            .write_to_file(config.format, output_path)
            .expect("Failed to write report");
        println!("Report written successfully.");
    } else {
        // Print to stdout
        let output = report.generate(config.format);

        if let Some(json) = output.json {
            println!("\n=== JSON Report ===\n");
            println!("{}", json);
        }

        if let Some(md) = output.markdown {
            println!("\n=== Markdown Report ===\n");
            println!("{}", md);
        }
    }
}

struct CliConfig {
    benchmark_config: BenchmarkConfig,
    tiers: Vec<TierConfig>,
    format: ReportFormat,
    output: Option<PathBuf>,
    seed: u64,
    k_values: Vec<usize>,
}

fn parse_args(args: &[String]) -> CliConfig {
    let mut format = ReportFormat::Json;
    let mut output: Option<PathBuf> = None;
    let mut tiers: Option<Vec<Tier>> = None;
    let mut seed = 42u64;
    let mut ci_mode = true;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--format" | "-f" => {
                i += 1;
                if i < args.len() {
                    format = match args[i].to_lowercase().as_str() {
                        "json" => ReportFormat::Json,
                        "markdown" | "md" => ReportFormat::Markdown,
                        "both" => ReportFormat::Both,
                        _ => {
                            eprintln!("Unknown format: {}. Using json.", args[i]);
                            ReportFormat::Json
                        }
                    };
                }
            }
            "--output" | "-o" => {
                i += 1;
                if i < args.len() {
                    output = Some(PathBuf::from(&args[i]));
                }
            }
            "--tiers" | "-t" => {
                i += 1;
                if i < args.len() {
                    tiers = Some(parse_tiers(&args[i]));
                }
            }
            "--seed" | "-s" => {
                i += 1;
                if i < args.len() {
                    seed = args[i].parse().unwrap_or(42);
                }
            }
            "--full" => {
                ci_mode = false;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    let tier_configs: Vec<TierConfig> = tiers
        .unwrap_or_else(|| {
            if ci_mode {
                Tier::ci_tiers().to_vec()
            } else {
                Tier::all().to_vec()
            }
        })
        .into_iter()
        .map(TierConfig::for_tier)
        .collect();

    let benchmark_config = BenchmarkConfig {
        tiers: tier_configs.clone(),
        k_values: vec![1, 5, 10, 20, 50],
        seed,
        ci_mode,
        latency_percentiles: vec![0.5, 0.95, 0.99],
        warmup_iterations: 5,
        measurement_iterations: 100,
    };

    CliConfig {
        benchmark_config,
        tiers: tier_configs,
        format,
        output,
        seed,
        k_values: vec![1, 5, 10, 20, 50],
    }
}

fn parse_tiers(s: &str) -> Vec<Tier> {
    s.split(',')
        .filter_map(|t| {
            let trimmed = t.trim();
            match trimmed {
                "0" => Some(Tier::Tier0),
                "1" => Some(Tier::Tier1),
                "2" => Some(Tier::Tier2),
                "3" => Some(Tier::Tier3),
                "4" => Some(Tier::Tier4),
                "5" => Some(Tier::Tier5),
                _ => {
                    eprintln!(
                        "Warning: Invalid tier '{}'. Valid tiers are 0-5. Skipping.",
                        trimmed
                    );
                    None
                }
            }
        })
        .collect()
}

fn print_help() {
    println!("Context Graph Benchmark Report Generator");
    println!();
    println!("USAGE:");
    println!("    benchmark-report [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -f, --format <FORMAT>    Output format: json, markdown, both [default: json]");
    println!("    -o, --output <PATH>      Output file path (without extension for 'both')");
    println!("    -t, --tiers <TIERS>      Comma-separated tier numbers (0-5) [default: 0,1,2,3]");
    println!("    -s, --seed <SEED>        Random seed for reproducibility [default: 42]");
    println!("    --full                   Run all tiers (0-5), not just CI tiers (0-3)");
    println!("    -h, --help               Print help information");
    println!();
    println!("EXAMPLES:");
    println!("    benchmark-report --format json --output results.json");
    println!("    benchmark-report --format markdown --output BENCHMARK_RESULTS.md");
    println!("    benchmark-report --tiers 0,1 --format both --output benchmark");
    println!("    benchmark-report --full --format json");
}
