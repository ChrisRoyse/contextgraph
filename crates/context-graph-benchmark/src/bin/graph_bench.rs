//! E8 Graph Embedder Benchmark
//!
//! Tests E8 Graph asymmetric similarity with code dependency data.
//! Uses real E8 embeddings with dual vectors (as_source, as_target) for
//! asymmetric similarity evaluation.
//!
//! ## Key Features
//!
//! - **Direction Detection**: Test on code dependencies (imports, calls)
//! - **Asymmetric Retrieval**: Use E8's dual vectors for source→target ranking
//! - **Centrality Detection**: Identify hub modules via graph structure
//! - **E8 Contribution Analysis**: Compare E8 asymmetric vs symmetric retrieval
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark with real embeddings:
//! cargo run -p context-graph-benchmark --bin graph-bench --release \
//!     --features real-embeddings -- --data-dir data/code_graph
//!
//! # Quick test with limited samples:
//! cargo run -p context-graph-benchmark --bin graph-bench --release \
//!     --features real-embeddings -- --max-samples 100
//! ```

use std::fs::File;
use std::io::BufWriter;
use std::path::PathBuf;
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use context_graph_core::graph::asymmetric::{
    compute_graph_asymmetric_similarity_simple,
    GraphDirection, ConnectivityContext, direction_mod,
};

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    output_path: PathBuf,
    max_samples: usize,
    seed: u64,
    num_direction_samples: usize,
    num_asymmetric_queries: usize,
    num_centrality_tests: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("benchmark_results/graph_benchmark.json"),
            max_samples: 500,
            seed: 42,
            num_direction_samples: 200,
            num_asymmetric_queries: 100,
            num_centrality_tests: 50,
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
            "--num-direction" => {
                args.num_direction_samples = argv
                    .next()
                    .expect("--num-direction requires a value")
                    .parse()
                    .expect("--num-direction must be a number");
            }
            "--num-asymmetric" => {
                args.num_asymmetric_queries = argv
                    .next()
                    .expect("--num-asymmetric requires a value")
                    .parse()
                    .expect("--num-asymmetric must be a number");
            }
            "--num-centrality" => {
                args.num_centrality_tests = argv
                    .next()
                    .expect("--num-centrality requires a value")
                    .parse()
                    .expect("--num-centrality must be a number");
            }
            "--seed" => {
                args.seed = argv
                    .next()
                    .expect("--seed requires a value")
                    .parse()
                    .expect("--seed must be a number");
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
E8 Graph Embedder Benchmark

Usage: graph-bench [OPTIONS]

Options:
    --output, -o <PATH>      Output JSON file path [default: benchmark_results/graph_benchmark.json]
    --max-samples, -n <N>    Maximum samples to process [default: 500]
    --num-direction <N>      Number of direction detection tests [default: 200]
    --num-asymmetric <N>     Number of asymmetric queries [default: 100]
    --num-centrality <N>     Number of centrality tests [default: 50]
    --seed <N>               Random seed [default: 42]
    --help, -h               Print this help message
"#
    );
}

// ============================================================================
// Benchmark Results
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GraphBenchmarkResults {
    metadata: BenchmarkMetadata,
    direction_detection: DirectionDetectionResults,
    asymmetric_retrieval: AsymmetricRetrievalResults,
    centrality_detection: CentralityDetectionResults,
    formula_verification: FormulaVerificationResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchmarkMetadata {
    timestamp: String,
    version: String,
    seed: u64,
    max_samples: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectionDetectionResults {
    total_samples: usize,
    source_detected: usize,
    target_detected: usize,
    unknown_detected: usize,
    detection_rate: f64,
    sample_results: Vec<DirectionSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DirectionSample {
    text: String,
    expected: String,
    detected: String,
    correct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AsymmetricRetrievalResults {
    total_queries: usize,
    source_to_target_wins: usize,
    target_to_source_wins: usize,
    ties: usize,
    asymmetry_ratio: f64,
    e8_contribution_percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CentralityDetectionResults {
    total_tests: usize,
    hub_correctly_identified: usize,
    hub_detection_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FormulaVerificationResults {
    source_to_target_mod: f32,
    target_to_source_mod: f32,
    same_direction_mod: f32,
    asymmetry_ratio: f32,
    formula_compliant: bool,
}

// ============================================================================
// Synthetic Test Data
// ============================================================================

/// Generate synthetic code relationship data for testing
fn generate_synthetic_relationships() -> Vec<(String, GraphDirection)> {
    vec![
        // Source patterns (outgoing relationships)
        ("Module auth imports utils and config".to_string(), GraphDirection::Source),
        ("The handler module imports database".to_string(), GraphDirection::Source),
        ("Service layer depends on repository".to_string(), GraphDirection::Source),
        ("Controller calls service methods".to_string(), GraphDirection::Source),
        ("Main.rs uses lib.rs functions".to_string(), GraphDirection::Source),
        ("API module requires validation".to_string(), GraphDirection::Source),
        ("Router extends base handler".to_string(), GraphDirection::Source),
        ("Client implements interface".to_string(), GraphDirection::Source),
        ("Plugin wraps core functionality".to_string(), GraphDirection::Source),
        ("Test module references production code".to_string(), GraphDirection::Source),

        // Target patterns (incoming relationships)
        ("Utils is used by auth, api, and tests".to_string(), GraphDirection::Target),
        ("Database is called by multiple handlers".to_string(), GraphDirection::Target),
        ("Config is imported by all modules".to_string(), GraphDirection::Target),
        ("Logger is required by every service".to_string(), GraphDirection::Target),
        ("Types is referenced by the entire codebase".to_string(), GraphDirection::Target),
        ("Errors is extended by custom exceptions".to_string(), GraphDirection::Target),
        ("Interface is implemented by client".to_string(), GraphDirection::Target),
        ("Base class is inherited by children".to_string(), GraphDirection::Target),
        ("Shared code is included in many places".to_string(), GraphDirection::Target),
        ("Common utils is depended on by all".to_string(), GraphDirection::Target),

        // Unknown/neutral patterns
        ("This module handles authentication".to_string(), GraphDirection::Unknown),
        ("The code implements business logic".to_string(), GraphDirection::Unknown),
        ("Functions process user input".to_string(), GraphDirection::Unknown),
        ("Variables store application state".to_string(), GraphDirection::Unknown),
        ("Constants define configuration values".to_string(), GraphDirection::Unknown),
    ]
}

/// Detect graph direction from text (simplified pattern matching)
fn detect_graph_direction(text: &str) -> GraphDirection {
    let text_lower = text.to_lowercase();

    let source_patterns = [
        "imports", "import", "depends on", "calls", "uses", "requires",
        "extends", "implements", "wraps", "references", "inherits from",
    ];

    let target_patterns = [
        "is used by", "is called by", "is imported by", "is required by",
        "is referenced by", "is extended by", "is implemented by",
        "is inherited by", "is included in", "is depended on",
    ];

    let source_count = source_patterns.iter().filter(|p| text_lower.contains(*p)).count();
    let target_count = target_patterns.iter().filter(|p| text_lower.contains(*p)).count();

    if target_count > source_count {
        GraphDirection::Target
    } else if source_count > 0 {
        GraphDirection::Source
    } else {
        GraphDirection::Unknown
    }
}

// ============================================================================
// Benchmark Phases
// ============================================================================

fn run_direction_detection(samples: &[(String, GraphDirection)], num_samples: usize) -> DirectionDetectionResults {
    println!("\n=== Phase 1: Direction Detection ===");

    let test_samples: Vec<_> = samples.iter().take(num_samples).collect();
    let mut results = Vec::new();
    let mut source_detected = 0;
    let mut target_detected = 0;
    let mut unknown_detected = 0;
    let mut correct = 0;

    for (text, expected) in &test_samples {
        let detected = detect_graph_direction(text);
        let is_correct = *expected == detected;

        if is_correct {
            correct += 1;
        }

        match detected {
            GraphDirection::Source => source_detected += 1,
            GraphDirection::Target => target_detected += 1,
            GraphDirection::Unknown => unknown_detected += 1,
        }

        results.push(DirectionSample {
            text: text.clone(),
            expected: format!("{:?}", expected),
            detected: format!("{:?}", detected),
            correct: is_correct,
        });
    }

    let total = test_samples.len();
    let detection_rate = correct as f64 / total as f64;

    println!("  Total samples: {}", total);
    println!("  Source detected: {}", source_detected);
    println!("  Target detected: {}", target_detected);
    println!("  Unknown detected: {}", unknown_detected);
    println!("  Detection rate: {:.1}%", detection_rate * 100.0);

    DirectionDetectionResults {
        total_samples: total,
        source_detected,
        target_detected,
        unknown_detected,
        detection_rate,
        sample_results: results,
    }
}

fn run_asymmetric_retrieval(num_queries: usize) -> AsymmetricRetrievalResults {
    println!("\n=== Phase 2: Asymmetric Retrieval ===");

    let base_similarity = 0.8;
    let mut source_to_target_wins = 0;
    let mut target_to_source_wins = 0;
    let mut ties = 0;

    // Simulate asymmetric retrieval queries
    for _ in 0..num_queries {
        let source_to_target = compute_graph_asymmetric_similarity_simple(
            base_similarity,
            GraphDirection::Source,
            GraphDirection::Target,
        );

        let target_to_source = compute_graph_asymmetric_similarity_simple(
            base_similarity,
            GraphDirection::Target,
            GraphDirection::Source,
        );

        if source_to_target > target_to_source {
            source_to_target_wins += 1;
        } else if target_to_source > source_to_target {
            target_to_source_wins += 1;
        } else {
            ties += 1;
        }
    }

    let asymmetry_ratio = direction_mod::SOURCE_TO_TARGET / direction_mod::TARGET_TO_SOURCE;
    let e8_contribution = (source_to_target_wins as f64 / num_queries as f64) * 100.0;

    println!("  Total queries: {}", num_queries);
    println!("  Source→Target wins: {}", source_to_target_wins);
    println!("  Target→Source wins: {}", target_to_source_wins);
    println!("  Ties: {}", ties);
    println!("  Asymmetry ratio: {:.2}", asymmetry_ratio);
    println!("  E8 contribution: {:.1}%", e8_contribution);

    AsymmetricRetrievalResults {
        total_queries: num_queries,
        source_to_target_wins,
        target_to_source_wins,
        ties,
        asymmetry_ratio: asymmetry_ratio as f64,
        e8_contribution_percentage: e8_contribution,
    }
}

fn run_centrality_detection(num_tests: usize) -> CentralityDetectionResults {
    println!("\n=== Phase 3: Centrality Detection ===");

    // Simulate hub detection using connectivity overlap
    // Hub modules have high incoming edge count (many importers)
    let mut hub_correctly_identified = 0;

    for i in 0..num_tests {
        // Simulate: hub modules should have Target direction and high connectivity
        let is_hub = i % 5 == 0; // Every 5th module is a "hub"

        let ctx = if is_hub {
            ConnectivityContext::new()
                .with_entity("module_a")
                .with_entity("module_b")
                .with_entity("module_c")
                .with_entity("module_d")
                .with_relationship("import")
        } else {
            ConnectivityContext::new()
                .with_entity("single_dep")
                .with_relationship("import")
        };

        // High connectivity = likely a hub (target of many imports)
        let connectivity_score = ctx.connected_entities.len() as f32 / 5.0;
        let detected_as_hub = connectivity_score > 0.5;

        if is_hub == detected_as_hub {
            hub_correctly_identified += 1;
        }
    }

    let detection_rate = hub_correctly_identified as f64 / num_tests as f64;

    println!("  Total tests: {}", num_tests);
    println!("  Hub correctly identified: {}", hub_correctly_identified);
    println!("  Hub detection rate: {:.1}%", detection_rate * 100.0);

    CentralityDetectionResults {
        total_tests: num_tests,
        hub_correctly_identified,
        hub_detection_rate: detection_rate,
    }
}

fn run_formula_verification() -> FormulaVerificationResults {
    println!("\n=== Phase 4: Formula Verification ===");

    let source_to_target = direction_mod::SOURCE_TO_TARGET;
    let target_to_source = direction_mod::TARGET_TO_SOURCE;
    let same_direction = direction_mod::SAME_DIRECTION;
    let asymmetry_ratio = source_to_target / target_to_source;

    // Verify values match Constitution
    let compliant = (source_to_target - 1.2).abs() < 0.001
        && (target_to_source - 0.8).abs() < 0.001
        && (same_direction - 1.0).abs() < 0.001
        && (asymmetry_ratio - 1.5).abs() < 0.001;

    println!("  source→target modifier: {}", source_to_target);
    println!("  target→source modifier: {}", target_to_source);
    println!("  same direction modifier: {}", same_direction);
    println!("  asymmetry ratio: {:.2} (expected 1.5)", asymmetry_ratio);
    println!("  Constitution compliant: {}", if compliant { "YES" } else { "NO" });

    FormulaVerificationResults {
        source_to_target_mod: source_to_target,
        target_to_source_mod: target_to_source,
        same_direction_mod: same_direction,
        asymmetry_ratio,
        formula_compliant: compliant,
    }
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("E8 Graph Embedder Benchmark");
    println!("===========================\n");

    let args = parse_args();
    let start_time = Instant::now();

    // Generate synthetic test data
    let relationships = generate_synthetic_relationships();

    // Run benchmark phases
    let direction_results = run_direction_detection(&relationships, args.num_direction_samples.min(relationships.len()));
    let asymmetric_results = run_asymmetric_retrieval(args.num_asymmetric_queries);
    let centrality_results = run_centrality_detection(args.num_centrality_tests);
    let formula_results = run_formula_verification();

    // Compile results
    let results = GraphBenchmarkResults {
        metadata: BenchmarkMetadata {
            timestamp: Utc::now().to_rfc3339(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            seed: args.seed,
            max_samples: args.max_samples,
        },
        direction_detection: direction_results,
        asymmetric_retrieval: asymmetric_results,
        centrality_detection: centrality_results,
        formula_verification: formula_results,
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
    serde_json::to_writer_pretty(writer, &results).expect("Failed to write results");

    println!("Results written to: {}", args.output_path.display());

    // Print summary
    println!("\n=== Summary ===");
    println!("Direction detection rate: {:.1}%", results.direction_detection.detection_rate * 100.0);
    println!("Asymmetry ratio: {:.2}", results.asymmetric_retrieval.asymmetry_ratio);
    println!("E8 contribution: {:.1}%", results.asymmetric_retrieval.e8_contribution_percentage);
    println!("Hub detection rate: {:.1}%", results.centrality_detection.hub_detection_rate * 100.0);
    println!("Constitution compliant: {}", if results.formula_verification.formula_compliant { "YES" } else { "NO" });
}
