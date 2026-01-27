//! Multi-Domain Graph Relationship Discovery Benchmark
//!
//! Run with: cargo run -p context-graph-graph-agent --example benchmark_graph_multi --features cuda --release
//!
//! This benchmark tests the generalized E8 (V_connectivity) graph relationship
//! discovery system across all 4 domains: Code, Legal, Academic, and General.
//! It validates detection of 20 relationship types and measures:
//! - Per-domain accuracy
//! - Per-relationship-type accuracy
//! - Domain detection accuracy (does LLM correctly identify content domain?)
//! - Cross-domain false positive rate
//! - Latency percentiles (P50, P95, P99)

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use context_graph_graph_agent::{
    ContentDomain, GraphDiscoveryConfig, GraphDiscoveryService, RelationshipType,
};
use serde::{Deserialize, Serialize};

// ============================================================================
// TEST PAIR STRUCTURES
// ============================================================================

/// A test pair loaded from JSON.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TestPair {
    id: String,
    name: String,
    content_a: String,
    content_b: String,
    expected_relationship: String,
    expected_direction: String,
    expected_has_connection: bool,
    #[serde(default)]
    notes: Option<String>,
}

/// A domain's test data.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DomainPairs {
    version: String,
    domain: String,
    description: String,
    relationship_types_tested: Vec<String>,
    pairs: Vec<TestPair>,
}

// ============================================================================
// RESULT STRUCTURES
// ============================================================================

/// Result for a single test pair.
#[derive(Debug, Clone, Serialize)]
struct PairResult {
    id: String,
    name: String,
    domain: String,
    content_a_snippet: String,
    content_b_snippet: String,
    expected_relationship: String,
    expected_has_connection: bool,
    actual_relationship: String,
    actual_has_connection: bool,
    actual_domain: String,
    actual_category: String,
    confidence: f32,
    direction: String,
    description: String,
    inference_time_ms: u64,
    is_correct: bool,
    domain_detection_correct: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Per-relationship type statistics.
#[derive(Debug, Clone, Serialize, Default)]
struct RelationshipTypeStats {
    correct: usize,
    total: usize,
    accuracy: f64,
    avg_confidence: f32,
    confidences: Vec<f32>,
}

/// Per-domain statistics.
#[derive(Debug, Clone, Serialize)]
struct DomainStats {
    domain: String,
    total_pairs: usize,
    correct: usize,
    accuracy: f64,
    domain_detection_correct: usize,
    domain_detection_accuracy: f64,
    avg_confidence: f32,
    avg_inference_time_ms: f64,
    relationship_breakdown: HashMap<String, RelationshipTypeStats>,
}

/// Latency percentiles.
#[derive(Debug, Serialize)]
struct LatencyStats {
    p50_ms: u64,
    p95_ms: u64,
    p99_ms: u64,
    min_ms: u64,
    max_ms: u64,
    avg_ms: f64,
}

/// Overall benchmark results.
#[derive(Debug, Serialize)]
struct BenchmarkResults {
    timestamp: String,
    model: String,
    total_pairs: usize,
    overall_accuracy: f64,
    overall_avg_confidence: f32,
    domain_results: HashMap<String, DomainStats>,
    cross_domain_false_positive_rate: f64,
    latency: LatencyStats,
    relationship_type_breakdown: HashMap<String, RelationshipTypeStats>,
    category_breakdown: HashMap<String, RelationshipTypeStats>,
    model_load_time_sec: f64,
    total_benchmark_time_sec: f64,
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn truncate(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").trim().to_string();
    if s.len() <= max_len {
        s
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

fn calculate_percentile(sorted_values: &[u64], percentile: f64) -> u64 {
    if sorted_values.is_empty() {
        return 0;
    }
    let idx = ((percentile / 100.0) * (sorted_values.len() - 1) as f64).round() as usize;
    sorted_values[idx.min(sorted_values.len() - 1)]
}

fn load_pairs_from_file(path: &PathBuf) -> Result<DomainPairs, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let pairs: DomainPairs = serde_json::from_reader(reader)?;
    Ok(pairs)
}

// ============================================================================
// MAIN BENCHMARK
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("╔══════════════════════════════════════════════════════════════════════════════════╗");
    println!("║        Multi-Domain Graph Relationship Discovery Benchmark                        ║");
    println!("║           E8 (V_connectivity) - Testing 4 Domains × 20 Relationship Types         ║");
    println!("╚══════════════════════════════════════════════════════════════════════════════════╝\n");

    // Find workspace root
    let mut workspace_root = std::env::current_dir()?;
    while !workspace_root.join("models").exists() {
        if !workspace_root.pop() {
            return Err("Could not find workspace root".into());
        }
    }

    let model_dir = workspace_root.join("models/hermes-2-pro");
    let data_dir = workspace_root.join("data/graph_benchmark");

    println!("Model directory: {:?}", model_dir);
    println!("Data directory: {:?}\n", data_dir);

    // Load test pairs from all domain files
    let domain_files = vec![
        ("code", "code_pairs.json"),
        ("legal", "legal_pairs.json"),
        ("academic", "academic_pairs.json"),
        ("general", "general_pairs.json"),
        ("negative", "negative_pairs.json"),
    ];

    let mut all_pairs: Vec<(String, TestPair)> = Vec::new();

    for (domain_name, filename) in &domain_files {
        let path = data_dir.join(filename);
        if path.exists() {
            match load_pairs_from_file(&path) {
                Ok(domain_pairs) => {
                    println!("Loaded {} pairs from {} ({})", domain_pairs.pairs.len(), filename, domain_name);
                    for pair in domain_pairs.pairs {
                        all_pairs.push((domain_name.to_string(), pair));
                    }
                }
                Err(e) => {
                    println!("Warning: Failed to load {}: {}", filename, e);
                }
            }
        } else {
            println!("Warning: File not found: {:?}", path);
        }
    }

    println!("\nTotal test pairs loaded: {}\n", all_pairs.len());

    if all_pairs.is_empty() {
        return Err("No test pairs loaded. Please ensure data files exist.".into());
    }

    // Initialize LLM with Hermes 2 Pro and GBNF grammar constraints
    let config = LlmConfig {
        model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
        causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
        graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
        validation_grammar_path: model_dir.join("validation.gbnf"),
        n_gpu_layers: u32::MAX, // Full GPU offload
        temperature: 0.0,       // Deterministic for reliable JSON output
        max_tokens: 256,
        ..Default::default()
    };

    println!("Initializing CausalDiscoveryLLM...");
    let llm = CausalDiscoveryLLM::with_config(config)?;

    println!("Loading model (~6GB VRAM required)...");
    let model_load_start = Instant::now();
    llm.load().await?;
    let model_load_time = model_load_start.elapsed();
    println!("✓ Model loaded in {:.2?}\n", model_load_time);

    // Create GraphDiscoveryService
    let graph_config = GraphDiscoveryConfig {
        min_confidence: 0.5,
        ..Default::default()
    };

    let service = GraphDiscoveryService::with_config(Arc::new(llm), graph_config);
    let llm = service.llm();

    // Run benchmark
    println!("Running multi-domain relationship benchmark...\n");
    println!("┌────┬────────────────────────────────┬──────────┬─────────────┬─────────────┬──────┬────────┬───┐");
    println!("│ #  │ Test Pair                      │ Domain   │ Expected    │ Actual      │ Conf │ Time   │ ✓ │");
    println!("├────┼────────────────────────────────┼──────────┼─────────────┼─────────────┼──────┼────────┼───┤");

    let benchmark_start = Instant::now();
    let mut results: Vec<PairResult> = Vec::new();
    let mut inference_times: Vec<u64> = Vec::new();

    // Track per-domain stats
    let mut domain_stats: HashMap<String, Vec<PairResult>> = HashMap::new();
    for (domain, _) in &domain_files {
        domain_stats.insert(domain.to_string(), Vec::new());
    }

    // Track cross-domain false positives (negative pairs that detected a relationship)
    let mut cross_domain_fp = 0;
    let mut cross_domain_total = 0;

    for (i, (domain, pair)) in all_pairs.iter().enumerate() {
        let start = Instant::now();
        let analysis = llm.analyze_relationship(&pair.content_a, &pair.content_b).await;
        let elapsed = start.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        inference_times.push(elapsed_ms);

        let expected_rel = RelationshipType::from_str(&pair.expected_relationship);
        let expected_domain = ContentDomain::from_str(domain);

        match analysis {
            Ok(result) => {
                // Check correctness: relationship type and connection status
                let is_correct = (result.has_connection == pair.expected_has_connection)
                    && (result.relationship_type == expected_rel
                        || (!pair.expected_has_connection && !result.has_connection));

                // Check domain detection correctness
                let domain_detection_correct = if domain == "negative" {
                    // For negative pairs, any domain detection is fine
                    true
                } else {
                    result.domain == expected_domain
                };

                // Track cross-domain false positives
                if domain == "negative" {
                    cross_domain_total += 1;
                    if result.has_connection {
                        cross_domain_fp += 1;
                    }
                }

                let symbol = if is_correct { "✓" } else { "✗" };
                let expected_str = if pair.expected_has_connection {
                    expected_rel.as_str()
                } else {
                    "none"
                };
                let actual_str = if result.has_connection {
                    result.relationship_type.as_str()
                } else {
                    "none"
                };

                println!(
                    "│{:>3} │ {:30} │ {:8} │ {:11} │ {:11} │ {:.2} │ {:5}ms │ {} │",
                    i + 1,
                    truncate(&pair.name, 30),
                    truncate(domain, 8),
                    truncate(expected_str, 11),
                    truncate(actual_str, 11),
                    result.confidence,
                    elapsed_ms,
                    symbol
                );

                let pair_result = PairResult {
                    id: pair.id.clone(),
                    name: pair.name.clone(),
                    domain: domain.clone(),
                    content_a_snippet: truncate(&pair.content_a, 100),
                    content_b_snippet: truncate(&pair.content_b, 100),
                    expected_relationship: pair.expected_relationship.clone(),
                    expected_has_connection: pair.expected_has_connection,
                    actual_relationship: result.relationship_type.as_str().to_string(),
                    actual_has_connection: result.has_connection,
                    actual_domain: result.domain.as_str().to_string(),
                    actual_category: result.category.as_str().to_string(),
                    confidence: result.confidence,
                    direction: result.direction.as_str().to_string(),
                    description: result.description,
                    inference_time_ms: elapsed_ms,
                    is_correct,
                    domain_detection_correct,
                    error: None,
                };

                if let Some(domain_results) = domain_stats.get_mut(domain) {
                    domain_results.push(pair_result.clone());
                }
                results.push(pair_result);
            }
            Err(e) => {
                let error_msg = format!("{:?}", e);
                println!(
                    "│{:>3} │ {:30} │ {:8} │ ERROR: {}",
                    i + 1,
                    truncate(&pair.name, 30),
                    domain,
                    truncate(&error_msg, 40)
                );

                let pair_result = PairResult {
                    id: pair.id.clone(),
                    name: pair.name.clone(),
                    domain: domain.clone(),
                    content_a_snippet: truncate(&pair.content_a, 100),
                    content_b_snippet: truncate(&pair.content_b, 100),
                    expected_relationship: pair.expected_relationship.clone(),
                    expected_has_connection: pair.expected_has_connection,
                    actual_relationship: "error".to_string(),
                    actual_has_connection: false,
                    actual_domain: "unknown".to_string(),
                    actual_category: "unknown".to_string(),
                    confidence: 0.0,
                    direction: "none".to_string(),
                    description: String::new(),
                    inference_time_ms: elapsed_ms,
                    is_correct: false,
                    domain_detection_correct: false,
                    error: Some(error_msg),
                };

                if let Some(domain_results) = domain_stats.get_mut(domain) {
                    domain_results.push(pair_result.clone());
                }
                results.push(pair_result);
            }
        }
    }

    println!("└────┴────────────────────────────────┴──────────┴─────────────┴─────────────┴──────┴────────┴───┘\n");

    let total_time = benchmark_start.elapsed();

    // Calculate statistics
    let total_correct = results.iter().filter(|r| r.is_correct).count();
    let overall_accuracy = total_correct as f64 / results.len() as f64;
    let overall_avg_confidence: f32 =
        results.iter().map(|r| r.confidence).sum::<f32>() / results.len() as f32;

    // Calculate latency stats
    inference_times.sort();
    let latency = LatencyStats {
        p50_ms: calculate_percentile(&inference_times, 50.0),
        p95_ms: calculate_percentile(&inference_times, 95.0),
        p99_ms: calculate_percentile(&inference_times, 99.0),
        min_ms: *inference_times.first().unwrap_or(&0),
        max_ms: *inference_times.last().unwrap_or(&0),
        avg_ms: inference_times.iter().sum::<u64>() as f64 / inference_times.len().max(1) as f64,
    };

    // Calculate per-domain stats
    let mut domain_results_map: HashMap<String, DomainStats> = HashMap::new();

    for (domain_name, domain_results) in &domain_stats {
        if domain_results.is_empty() {
            continue;
        }

        let correct = domain_results.iter().filter(|r| r.is_correct).count();
        let domain_detection_correct = domain_results
            .iter()
            .filter(|r| r.domain_detection_correct)
            .count();

        let avg_confidence: f32 =
            domain_results.iter().map(|r| r.confidence).sum::<f32>() / domain_results.len() as f32;
        let avg_time: f64 = domain_results.iter().map(|r| r.inference_time_ms as f64).sum::<f64>()
            / domain_results.len() as f64;

        // Per-relationship breakdown
        let mut rel_breakdown: HashMap<String, RelationshipTypeStats> = HashMap::new();
        for result in domain_results {
            let entry = rel_breakdown
                .entry(result.expected_relationship.clone())
                .or_default();
            entry.total += 1;
            entry.confidences.push(result.confidence);
            if result.is_correct {
                entry.correct += 1;
            }
        }

        // Calculate accuracy and avg confidence per relationship type
        for (_, stats) in rel_breakdown.iter_mut() {
            stats.accuracy = stats.correct as f64 / stats.total.max(1) as f64;
            stats.avg_confidence =
                stats.confidences.iter().sum::<f32>() / stats.confidences.len().max(1) as f32;
        }

        domain_results_map.insert(
            domain_name.clone(),
            DomainStats {
                domain: domain_name.clone(),
                total_pairs: domain_results.len(),
                correct,
                accuracy: correct as f64 / domain_results.len() as f64,
                domain_detection_correct,
                domain_detection_accuracy: domain_detection_correct as f64
                    / domain_results.len() as f64,
                avg_confidence,
                avg_inference_time_ms: avg_time,
                relationship_breakdown: rel_breakdown,
            },
        );
    }

    // Calculate overall relationship type breakdown
    let mut overall_rel_breakdown: HashMap<String, RelationshipTypeStats> = HashMap::new();
    for result in &results {
        let entry = overall_rel_breakdown
            .entry(result.expected_relationship.clone())
            .or_default();
        entry.total += 1;
        entry.confidences.push(result.confidence);
        if result.is_correct {
            entry.correct += 1;
        }
    }
    for (_, stats) in overall_rel_breakdown.iter_mut() {
        stats.accuracy = stats.correct as f64 / stats.total.max(1) as f64;
        stats.avg_confidence =
            stats.confidences.iter().sum::<f32>() / stats.confidences.len().max(1) as f32;
    }

    // Calculate category breakdown
    let mut category_breakdown: HashMap<String, RelationshipTypeStats> = HashMap::new();
    for result in &results {
        let rel_type = RelationshipType::from_str(&result.expected_relationship);
        let category = rel_type.category().as_str().to_string();
        let entry = category_breakdown.entry(category).or_default();
        entry.total += 1;
        entry.confidences.push(result.confidence);
        if result.is_correct {
            entry.correct += 1;
        }
    }
    for (_, stats) in category_breakdown.iter_mut() {
        stats.accuracy = stats.correct as f64 / stats.total.max(1) as f64;
        stats.avg_confidence =
            stats.confidences.iter().sum::<f32>() / stats.confidences.len().max(1) as f32;
    }

    // Cross-domain false positive rate
    let cross_domain_fpr = if cross_domain_total > 0 {
        cross_domain_fp as f64 / cross_domain_total as f64
    } else {
        0.0
    };

    let benchmark_results = BenchmarkResults {
        timestamp: Utc::now().to_rfc3339(),
        model: "Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf".to_string(),
        total_pairs: results.len(),
        overall_accuracy,
        overall_avg_confidence,
        domain_results: domain_results_map.clone(),
        cross_domain_false_positive_rate: cross_domain_fpr,
        latency,
        relationship_type_breakdown: overall_rel_breakdown,
        category_breakdown,
        model_load_time_sec: model_load_time.as_secs_f64(),
        total_benchmark_time_sec: total_time.as_secs_f64(),
    };

    // Print summary
    println!("╔══════════════════════════════════════════════════════════════════════════════════╗");
    println!("║                        MULTI-DOMAIN BENCHMARK RESULTS                            ║");
    println!("╠══════════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ Total Pairs:                    {:>52} ║",
        results.len()
    );
    println!(
        "║ Overall Accuracy:               {:>51.1}% ║",
        overall_accuracy * 100.0
    );
    println!(
        "║ Average Confidence:             {:>52.3} ║",
        overall_avg_confidence
    );
    println!(
        "║ Cross-Domain False Positive Rate:{:>50.1}% ║",
        cross_domain_fpr * 100.0
    );
    println!("╠══════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ PER-DOMAIN RESULTS                                                               ║");
    println!("╟──────────────────────────────────────────────────────────────────────────────────╢");

    for (domain_name, stats) in &domain_results_map {
        println!(
            "║ {:10}: {:>2}/{:<2} ({:>5.1}%) | Domain Det: {:>5.1}% | Conf: {:.2} | Time: {:>5.0}ms ║",
            domain_name,
            stats.correct,
            stats.total_pairs,
            stats.accuracy * 100.0,
            stats.domain_detection_accuracy * 100.0,
            stats.avg_confidence,
            stats.avg_inference_time_ms
        );
    }

    println!("╠══════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ LATENCY PERCENTILES                                                              ║");
    println!("╟──────────────────────────────────────────────────────────────────────────────────╢");
    println!(
        "║ P50: {:>4}ms | P95: {:>4}ms | P99: {:>4}ms | Min: {:>4}ms | Max: {:>4}ms | Avg: {:>5.0}ms ║",
        benchmark_results.latency.p50_ms,
        benchmark_results.latency.p95_ms,
        benchmark_results.latency.p99_ms,
        benchmark_results.latency.min_ms,
        benchmark_results.latency.max_ms,
        benchmark_results.latency.avg_ms
    );

    println!("╠══════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ PER-CATEGORY ACCURACY                                                            ║");
    println!("╟──────────────────────────────────────────────────────────────────────────────────╢");

    for (category, stats) in &benchmark_results.category_breakdown {
        println!(
            "║   {:15}: {:>2}/{:<2} ({:>5.1}%) | Avg Conf: {:.2}                                ║",
            category,
            stats.correct,
            stats.total,
            stats.accuracy * 100.0,
            stats.avg_confidence
        );
    }

    println!("╠══════════════════════════════════════════════════════════════════════════════════╣");
    println!("║ TIMING                                                                           ║");
    println!("╟──────────────────────────────────────────────────────────────────────────────────╢");
    println!(
        "║ Model Load Time:                {:>50.2}s ║",
        model_load_time.as_secs_f64()
    );
    println!(
        "║ Total Benchmark Time:           {:>50.2}s ║",
        total_time.as_secs_f64()
    );
    println!("╚══════════════════════════════════════════════════════════════════════════════════╝");

    // Check success criteria
    println!("\n╔══════════════════════════════════════════════════════════════════════════════════╗");
    println!("║ SUCCESS CRITERIA CHECK                                                           ║");
    println!("╟──────────────────────────────────────────────────────────────────────────────────╢");

    let checks = vec![
        ("Overall Accuracy >= 65%", overall_accuracy >= 0.65),
        (
            "Code Domain Accuracy >= 75%",
            domain_results_map
                .get("code")
                .map(|s| s.accuracy >= 0.75)
                .unwrap_or(false),
        ),
        (
            "Legal Domain Accuracy >= 60%",
            domain_results_map
                .get("legal")
                .map(|s| s.accuracy >= 0.60)
                .unwrap_or(false),
        ),
        (
            "Academic Domain Accuracy >= 60%",
            domain_results_map
                .get("academic")
                .map(|s| s.accuracy >= 0.60)
                .unwrap_or(false),
        ),
        ("Cross-Domain FPR <= 10%", cross_domain_fpr <= 0.10),
        (
            "P95 Latency <= 1000ms",
            benchmark_results.latency.p95_ms <= 1000,
        ),
    ];

    let mut all_passed = true;
    for (criterion, passed) in &checks {
        let symbol = if *passed { "✓" } else { "✗" };
        all_passed = all_passed && *passed;
        println!("║ {} {:74} ║", symbol, criterion);
    }

    println!("╟──────────────────────────────────────────────────────────────────────────────────╢");
    if all_passed {
        println!("║ ✓ ALL CRITERIA PASSED                                                           ║");
    } else {
        println!("║ ✗ SOME CRITERIA FAILED - See above for details                                  ║");
    }
    println!("╚══════════════════════════════════════════════════════════════════════════════════╝");

    // Save results
    let results_dir = workspace_root.join("benchmark_results");
    std::fs::create_dir_all(&results_dir)?;

    let summary_path = results_dir.join("graph_multi_domain_benchmark.json");
    let summary_file = File::create(&summary_path)?;
    serde_json::to_writer_pretty(summary_file, &benchmark_results)?;
    println!("\nSummary results saved to: {:?}", summary_path);

    let detailed_path = results_dir.join("graph_multi_domain_detailed.json");
    let detailed_file = File::create(&detailed_path)?;
    serde_json::to_writer_pretty(detailed_file, &results)?;
    println!("Detailed results saved to: {:?}", detailed_path);

    Ok(())
}
