//! MCP Intent Tool Integration benchmark binary.
//!
//! Benchmarks E10's role as an E1 ENHANCER in MCP tools.
//!
//! ## Usage
//!
//! ```bash
//! # Run with defaults
//! cargo run -p context-graph-benchmark --bin mcp-intent-bench --release
//!
//! # Custom configuration
//! cargo run -p context-graph-benchmark --bin mcp-intent-bench --release -- \
//!   --memories 1000 \
//!   --queries 100 \
//!   --output benchmark_results/mcp_intent.json \
//!   --run-ablation
//! ```
//!
//! ## Benchmark Phases
//!
//! 1. **E10 Enhancement Value**: Measure E10's improvement over E1-only
//! 2. **MCP Tool Integration**: End-to-end tool benchmarks
//! 3. **Asymmetric Validation**: Validate direction modifiers (1.2/0.8)
//! 4. **Constitutional Compliance**: Verify ARCH rules are followed

use std::fs;
use std::path::PathBuf;

use context_graph_benchmark::datasets::mcp_intent::MCPIntentDatasetConfig;
use context_graph_benchmark::runners::mcp_intent::{
    MCPIntentBenchmarkConfig, MCPIntentBenchmarkResults, MCPIntentBenchmarkRunner,
};

/// CLI arguments.
#[derive(Debug)]
struct Args {
    /// Output path for results JSON.
    output_path: PathBuf,

    /// Number of memories to generate.
    num_memories: usize,

    /// Number of intent queries.
    num_intent_queries: usize,

    /// Number of context queries.
    num_context_queries: usize,

    /// Number of asymmetric pairs.
    num_asymmetric_pairs: usize,

    /// Number of E1-strong queries.
    num_e1_strong_queries: usize,

    /// Number of E1-weak queries.
    num_e1_weak_queries: usize,

    /// Random seed.
    seed: u64,

    /// Skip enhancement phase.
    skip_enhancement: bool,

    /// Skip tool phase.
    skip_tool: bool,

    /// Skip asymmetric phase.
    skip_asymmetric: bool,

    /// Skip compliance phase.
    skip_compliance: bool,

    /// Blend values to sweep (comma-separated).
    blend_values: Vec<f64>,

    /// Verbose output.
    verbose: bool,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("benchmark_results/mcp_intent_benchmark.json"),
            num_memories: 1000,
            num_intent_queries: 100,
            num_context_queries: 100,
            num_asymmetric_pairs: 50,
            num_e1_strong_queries: 30,
            num_e1_weak_queries: 30,
            seed: 42,
            skip_enhancement: false,
            skip_tool: false,
            skip_asymmetric: false,
            skip_compliance: false,
            // Updated blend sweep to focus on lower weights (around 0.1 default)
            blend_values: vec![0.05, 0.1, 0.15, 0.2, 0.25, 0.3],
            verbose: false,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1).peekable();

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                if let Some(val) = argv.next() {
                    args.output_path = PathBuf::from(val);
                }
            }
            "-m" | "--memories" => {
                if let Some(val) = argv.next() {
                    args.num_memories = val.parse().unwrap_or(args.num_memories);
                }
            }
            "-q" | "--queries" => {
                if let Some(val) = argv.next() {
                    args.num_intent_queries = val.parse().unwrap_or(args.num_intent_queries);
                    args.num_context_queries = args.num_intent_queries;
                }
            }
            "--intent-queries" => {
                if let Some(val) = argv.next() {
                    args.num_intent_queries = val.parse().unwrap_or(args.num_intent_queries);
                }
            }
            "--context-queries" => {
                if let Some(val) = argv.next() {
                    args.num_context_queries = val.parse().unwrap_or(args.num_context_queries);
                }
            }
            "--asymmetric-pairs" => {
                if let Some(val) = argv.next() {
                    args.num_asymmetric_pairs = val.parse().unwrap_or(args.num_asymmetric_pairs);
                }
            }
            "--e1-strong-queries" => {
                if let Some(val) = argv.next() {
                    args.num_e1_strong_queries = val.parse().unwrap_or(args.num_e1_strong_queries);
                }
            }
            "--e1-weak-queries" => {
                if let Some(val) = argv.next() {
                    args.num_e1_weak_queries = val.parse().unwrap_or(args.num_e1_weak_queries);
                }
            }
            "-s" | "--seed" => {
                if let Some(val) = argv.next() {
                    args.seed = val.parse().unwrap_or(args.seed);
                }
            }
            "--skip-enhancement" => {
                args.skip_enhancement = true;
            }
            "--skip-tool" => {
                args.skip_tool = true;
            }
            "--skip-asymmetric" => {
                args.skip_asymmetric = true;
            }
            "--skip-compliance" => {
                args.skip_compliance = true;
            }
            "--blend-values" => {
                if let Some(val) = argv.next() {
                    args.blend_values = val
                        .split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                }
            }
            "-v" | "--verbose" => {
                args.verbose = true;
            }
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", arg);
            }
        }
    }

    args
}

fn print_usage() {
    println!(
        r#"MCP Intent Tool Integration Benchmark

USAGE:
    mcp-intent-bench [OPTIONS]

OPTIONS:
    -o, --output <PATH>         Output JSON file path [default: benchmark_results/mcp_intent_benchmark.json]
    -m, --memories <N>          Number of memories to generate [default: 1000]
    -q, --queries <N>           Number of intent/context queries each [default: 100]
    --intent-queries <N>        Number of intent queries [default: 100]
    --context-queries <N>       Number of context queries [default: 100]
    --asymmetric-pairs <N>      Number of asymmetric validation pairs [default: 50]
    --e1-strong-queries <N>     Number of E1-strong queries [default: 30]
    --e1-weak-queries <N>       Number of E1-weak queries [default: 30]
    -s, --seed <N>              Random seed [default: 42]
    --skip-enhancement          Skip enhancement phase
    --skip-tool                 Skip tool phase
    --skip-asymmetric           Skip asymmetric validation phase
    --skip-compliance           Skip compliance phase
    --blend-values <LIST>       Comma-separated blend values [default: 0.05,0.1,0.15,0.2,0.25,0.3]
    -v, --verbose               Verbose output
    -h, --help                  Print help

BENCHMARK PHASES:
    1. E10 Enhancement Value    - Measure E10's improvement over E1-only
    2. MCP Tool Integration     - End-to-end tool benchmarks
    3. Asymmetric Validation    - Validate direction handling (E5-base-v2 provides natural asymmetry via prefixes)
    4. Constitutional Compliance - Verify ARCH rules

SUCCESS CRITERIA:
    - MRR improvement (E1+E10 vs E1): > 5%
    - Optimal blend value: [0.05, 0.2]
    - Asymmetry ratio: 1.0 ± 0.15 (E5-base-v2 handles asymmetry via prefixes)
    - E1-strong refine rate: >= 70%
    - E1-weak broaden rate: >= 50%
    - Tool p95 latency: < 2000ms
"#
    );
}

fn main() {
    let args = parse_args();

    println!("=== MCP Intent Tool Integration Benchmark ===");
    println!();

    // Build configuration
    let config = MCPIntentBenchmarkConfig {
        dataset: MCPIntentDatasetConfig {
            num_memories: args.num_memories,
            num_intent_queries: args.num_intent_queries,
            num_context_queries: args.num_context_queries,
            num_asymmetric_pairs: args.num_asymmetric_pairs,
            num_e1_strong_queries: args.num_e1_strong_queries,
            num_e1_weak_queries: args.num_e1_weak_queries,
            seed: args.seed,
            ..Default::default()
        },
        run_enhancement_phase: !args.skip_enhancement,
        run_tool_phase: !args.skip_tool,
        run_asymmetric_phase: !args.skip_asymmetric,
        run_compliance_phase: !args.skip_compliance,
        blend_values: args.blend_values.clone(),
        ..Default::default()
    };

    println!("Configuration:");
    println!("  Memories: {}", args.num_memories);
    println!("  Intent queries: {}", args.num_intent_queries);
    println!("  Context queries: {}", args.num_context_queries);
    println!("  Asymmetric pairs: {}", args.num_asymmetric_pairs);
    println!("  E1-strong queries: {}", args.num_e1_strong_queries);
    println!("  E1-weak queries: {}", args.num_e1_weak_queries);
    println!("  Seed: {}", args.seed);
    println!("  Blend values: {:?}", args.blend_values);
    println!();

    // Run benchmark
    println!("Running benchmark...");
    let runner = MCPIntentBenchmarkRunner::new(config);
    let results = runner.run();

    // Print results
    print_results(&results, args.verbose);

    // Write results to file
    if let Err(e) = write_results(&results, &args.output_path) {
        eprintln!("Failed to write results: {}", e);
        std::process::exit(1);
    }

    println!();
    println!("Results written to: {}", args.output_path.display());

    // Exit with appropriate code
    if results.success {
        println!();
        println!("SUCCESS: All criteria met!");
        std::process::exit(0);
    } else {
        println!();
        println!("WARNING: Some criteria not met. See results for details.");
        std::process::exit(1);
    }
}

fn print_results(results: &MCPIntentBenchmarkResults, verbose: bool) {
    println!();
    println!("=== Benchmark Results ===");
    println!();

    // Timing
    println!("Timing:");
    println!("  Total: {}ms", results.timings.total_ms);
    println!("  Dataset generation: {}ms", results.timings.dataset_generation_ms);
    println!("  Enhancement phase: {}ms", results.timings.enhancement_phase_ms);
    println!("  Tool phase: {}ms", results.timings.tool_phase_ms);
    println!("  Asymmetric phase: {}ms", results.timings.asymmetric_phase_ms);
    println!("  Compliance phase: {}ms", results.timings.compliance_phase_ms);
    println!();

    // Dataset stats
    let stats = &results.dataset_stats;
    println!("Dataset:");
    println!("  Total memories: {}", stats.total_memories);
    println!("  Intent memories: {}", stats.intent_memories);
    println!("  Context memories: {}", stats.context_memories);
    println!("  E1 dim: {}, E10 dim: {}", stats.e1_dim, stats.e10_dim);
    println!();

    // Enhancement metrics
    let enh = &results.metrics.enhancement;
    println!("Phase 1: E10 Enhancement Value");
    println!("  E1-only MRR: {:.4}", enh.e1_only_mrr);
    println!("  E1+E10 blend MRR: {:.4}", enh.e1_e10_blend_mrr);
    println!(
        "  Improvement: {:.1}% {}",
        enh.improvement_percent,
        if enh.improvement_percent > 5.0 { "✓" } else { "✗" }
    );
    println!(
        "  Optimal blend: {:.2} {}",
        enh.optimal_blend,
        if enh.optimal_blend >= 0.2 && enh.optimal_blend <= 0.4 { "✓" } else { "✗" }
    );
    println!(
        "  E1-strong refine rate: {:.1}% {}",
        enh.e1_strong_refine_rate * 100.0,
        if enh.e1_strong_refine_rate >= 0.70 { "✓" } else { "✗" }
    );
    println!(
        "  E1-weak broaden rate: {:.1}% {}",
        enh.e1_weak_broaden_rate * 100.0,
        if enh.e1_weak_broaden_rate >= 0.50 { "✓" } else { "✗" }
    );
    println!();

    if verbose {
        println!("  Blend sweep:");
        for point in &enh.blend_sweep {
            println!(
                "    blend={:.1}: MRR={:.4}, P@5={:.4}, P@10={:.4}",
                point.blend, point.mrr, point.precision_at_5, point.precision_at_10
            );
        }
        println!();
    }

    // Tool metrics
    let tools = &results.metrics.tools;
    println!("Phase 2: MCP Tool Integration");
    println!(
        "  search_by_intent: MRR={:.4}, p95={:.1}ms {}",
        tools.search_by_intent.mrr,
        tools.search_by_intent.latency_p95_ms,
        if tools.search_by_intent.latency_p95_ms < 2000.0 { "✓" } else { "✗" }
    );
    println!(
        "  find_contextual_matches: MRR={:.4}, p95={:.1}ms {}",
        tools.find_contextual_matches.mrr,
        tools.find_contextual_matches.latency_p95_ms,
        if tools.find_contextual_matches.latency_p95_ms < 2000.0 { "✓" } else { "✗" }
    );
    println!(
        "  search_graph (intent_search): MRR={:.4}, p95={:.1}ms {}",
        tools.search_graph_intent.mrr,
        tools.search_graph_intent.latency_p95_ms,
        if tools.search_graph_intent.latency_p95_ms < 2000.0 { "✓" } else { "✗" }
    );
    println!();

    // Asymmetric validation
    let asym = &results.metrics.asymmetric;
    println!("Phase 3: Asymmetric Validation");
    println!("  Total pairs: {}", asym.total_pairs);
    println!(
        "  Observed ratio: {:.3} (expected 1.0 ± 0.15 - E5-base-v2 handles asymmetry via prefixes) {}",
        asym.ratio,
        if asym.compliant { "✓" } else { "✗" }
    );
    println!("  Intent→Context MRR: {:.4}", asym.intent_to_context_mrr);
    println!("  Context→Intent MRR: {:.4}", asym.context_to_intent_mrr);
    println!();

    // Constitutional compliance
    let comp = &results.metrics.compliance;
    println!("Phase 4: Constitutional Compliance");
    println!(
        "  ARCH-12 (E1 foundation): {}",
        if comp.arch_12_e1_foundation { "✓ PASS" } else { "✗ FAIL" }
    );
    println!(
        "  ARCH-17 (enhancement behavior): {}",
        if comp.arch_17_enhancement_behavior { "✓ PASS" } else { "✗ FAIL" }
    );
    println!(
        "  AP-02 (no cross-comparison): {}",
        if comp.ap_02_no_cross_comparison { "✓ PASS" } else { "✗ FAIL" }
    );
    println!("  Compliance score: {:.2}", comp.score);
    println!();

    // Overall
    println!("=== Summary ===");
    println!("  Overall score: {:.2}", results.metrics.overall_score());
    println!(
        "  Success criteria: {}",
        if results.success { "✓ ALL MET" } else { "✗ SOME FAILED" }
    );
}

fn write_results(results: &MCPIntentBenchmarkResults, path: &PathBuf) -> Result<(), String> {
    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    // Serialize to JSON
    let json = serde_json::to_string_pretty(results)
        .map_err(|e| format!("Failed to serialize results: {}", e))?;

    // Write to file
    fs::write(path, json).map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}
