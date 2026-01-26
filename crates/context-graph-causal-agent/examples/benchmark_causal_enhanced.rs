//! Enhanced Benchmark for Causal Discovery LLM.
//!
//! Run with: cargo run -p context-graph-causal-agent --example benchmark_causal_enhanced --features cuda --release
//!
//! This benchmark tests:
//! 1. Few-shot vs non-few-shot prompt performance
//! 2. Mechanism type detection accuracy
//! 3. Direction detection (ACausesB, BCausesA, Bidirectional)
//! 4. Confidence calibration

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use context_graph_causal_agent::CausalLinkDirection;
use serde::{Deserialize, Serialize};

/// A query/claim from the dataset.
#[derive(Debug, Deserialize)]
struct Query {
    query_id: String,
    text: String,
}

/// A document chunk from the dataset.
#[derive(Debug, Deserialize)]
struct Chunk {
    id: String,
    #[allow(dead_code)]
    doc_id: String,
    original_doc_id: String,
    title: String,
    text: String,
    #[serde(default)]
    #[allow(dead_code)]
    word_count: usize,
}

/// Detailed result for a single pair.
#[derive(Debug, Serialize, Clone)]
struct DetailedPairResult {
    query_id: String,
    doc_id: String,
    claim: String,
    evidence_title: String,
    has_causal_link: bool,
    direction: String,
    confidence: f32,
    mechanism: String,
    mechanism_type: Option<String>,
    inference_time_ms: u64,
}

/// Results for one mode (few-shot or non-few-shot).
#[derive(Debug, Serialize)]
struct ModeResults {
    mode: String,
    total_pairs: usize,
    pairs_with_causal_link: usize,
    pairs_without_causal_link: usize,
    avg_confidence: f32,
    avg_inference_time_ms: f64,
    min_inference_time_ms: u64,
    max_inference_time_ms: u64,
    total_time_sec: f64,
    throughput_pairs_per_sec: f64,
    direction_distribution: HashMap<String, usize>,
    mechanism_type_distribution: HashMap<String, usize>,
    model_load_time_sec: f64,
    detailed_results: Vec<DetailedPairResult>,
}

/// Overall benchmark comparison.
#[derive(Debug, Serialize)]
struct BenchmarkComparison {
    timestamp: String,
    dataset: String,
    few_shot_results: ModeResults,
    non_few_shot_results: ModeResults,
    comparison: ComparisonMetrics,
}

/// Comparison between modes.
#[derive(Debug, Serialize)]
struct ComparisonMetrics {
    inference_speedup: f64,
    confidence_delta: f32,
    causal_link_agreement_rate: f64,
    direction_agreement_rate: f64,
    mechanism_type_coverage_few_shot: f64,
    mechanism_type_coverage_non_few_shot: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Enhanced Causal Discovery Benchmark (Few-shot Comparison)    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Find workspace root
    let mut workspace_root = std::env::current_dir()?;
    while !workspace_root.join("data").exists() {
        if !workspace_root.pop() {
            return Err("Could not find workspace root".into());
        }
    }

    let data_dir = workspace_root.join("data/beir_scifact");
    let model_dir = workspace_root.join("models/hermes-2-pro");

    println!("Data directory: {:?}", data_dir);
    println!("Model directory: {:?}", model_dir);
    println!();

    // Load data
    println!("Loading SciFact data...");
    let load_start = Instant::now();

    let queries = load_queries(&data_dir.join("queries.jsonl"))?;
    let chunks = load_chunks(&data_dir.join("chunks.jsonl"))?;
    let qrels = load_qrels(&data_dir.join("qrels.json"))?;

    println!(
        "  Loaded {} queries, {} chunks, {} qrels in {:?}",
        queries.len(),
        chunks.len(),
        qrels.len(),
        load_start.elapsed()
    );

    // Build document index by original_doc_id
    let doc_index: HashMap<String, &Chunk> = chunks
        .iter()
        .map(|c| (c.original_doc_id.clone(), c))
        .collect();

    // Create pairs for benchmarking (query + relevant doc)
    let mut pairs: Vec<(&Query, &Chunk)> = Vec::new();
    for (query_id, doc_ids) in &qrels {
        if let Some(query) = queries.iter().find(|q| &q.query_id == query_id) {
            for (doc_id, _relevance) in doc_ids {
                if let Some(chunk) = doc_index.get(doc_id) {
                    pairs.push((query, *chunk));
                }
            }
        }
    }

    println!("  Created {} claim-evidence pairs for benchmarking", pairs.len());
    println!();

    // Limit pairs for benchmarking
    let max_pairs = std::env::var("BENCHMARK_PAIRS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let pairs = &pairs[..pairs.len().min(max_pairs)];
    println!("Running benchmark on {} pairs (set BENCHMARK_PAIRS env to change)\n", pairs.len());

    // Run with few-shot enabled
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  Running with FEW-SHOT prompts (5 examples)");
    println!("═══════════════════════════════════════════════════════════════\n");
    let few_shot_results = run_benchmark(&model_dir, pairs, true).await?;

    // Run without few-shot
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("  Running WITHOUT few-shot prompts (zero-shot)");
    println!("═══════════════════════════════════════════════════════════════\n");
    let non_few_shot_results = run_benchmark(&model_dir, pairs, false).await?;

    // Calculate comparison metrics
    let comparison = calculate_comparison(&few_shot_results, &non_few_shot_results);

    // Print comparison summary
    print_comparison_summary(&few_shot_results, &non_few_shot_results, &comparison);

    // Save full results
    let benchmark = BenchmarkComparison {
        timestamp: chrono::Utc::now().to_rfc3339(),
        dataset: "SciFact (BEIR)".to_string(),
        few_shot_results,
        non_few_shot_results,
        comparison,
    };

    let output_path = workspace_root.join("benchmark_results/causal_benchmark_enhanced.json");
    std::fs::create_dir_all(output_path.parent().unwrap())?;
    let output_file = File::create(&output_path)?;
    serde_json::to_writer_pretty(output_file, &benchmark)?;
    println!("\nFull results saved to: {:?}", output_path);

    Ok(())
}

async fn run_benchmark(
    model_dir: &PathBuf,
    pairs: &[(&Query, &Chunk)],
    use_few_shot: bool,
) -> Result<ModeResults, Box<dyn std::error::Error>> {
    let config = LlmConfig {
        model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
        causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
        graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
        validation_grammar_path: model_dir.join("validation.gbnf"),
        n_gpu_layers: u32::MAX,
        temperature: 0.0,
        max_tokens: 256,
        use_few_shot,
        ..Default::default()
    };

    let llm = CausalDiscoveryLLM::with_config(config)?;

    println!("Loading model...");
    let model_load_start = Instant::now();
    llm.load().await?;
    let model_load_time = model_load_start.elapsed();
    println!("✓ Model loaded in {:.2?}\n", model_load_time);

    println!("Running benchmark...\n");
    println!("┌────┬───────────────────────────────────────────────────────────────────────┐");
    println!("│ #  │ Claim (truncated) → Result                                            │");
    println!("├────┼───────────────────────────────────────────────────────────────────────┤");

    let benchmark_start = Instant::now();
    let mut results: Vec<DetailedPairResult> = Vec::new();
    let mut direction_counts: HashMap<String, usize> = HashMap::new();
    let mut mechanism_type_counts: HashMap<String, usize> = HashMap::new();

    for (i, (query, chunk)) in pairs.iter().enumerate() {
        let claim = &query.text;
        let evidence = format!("{}: {}", chunk.title, truncate(&chunk.text, 500));

        let start = Instant::now();
        let result = llm.analyze_causal_relationship(claim, &evidence).await;
        let elapsed = start.elapsed();

        match result {
            Ok(analysis) => {
                let direction_str = format!("{:?}", analysis.direction);
                *direction_counts.entry(direction_str.clone()).or_insert(0) += 1;

                let mech_type = analysis.mechanism_type.map(|m| m.as_str().to_string());
                if let Some(ref mt) = mech_type {
                    *mechanism_type_counts.entry(mt.clone()).or_insert(0) += 1;
                }

                let symbol = if analysis.has_causal_link { "✓" } else { "✗" };
                let conf = format!("{:.2}", analysis.confidence);
                let mech_type_str = mech_type.clone().unwrap_or_else(|| "N/A".to_string());

                println!(
                    "│ {:>2} │ {} {}  [{} {} {}] {:>5}ms │",
                    i + 1,
                    symbol,
                    truncate(claim, 28),
                    &direction_str[..direction_str.len().min(8)],
                    conf,
                    truncate(&mech_type_str, 8),
                    elapsed.as_millis()
                );

                results.push(DetailedPairResult {
                    query_id: query.query_id.clone(),
                    doc_id: chunk.original_doc_id.clone(),
                    claim: claim.clone(),
                    evidence_title: chunk.title.clone(),
                    has_causal_link: analysis.has_causal_link,
                    direction: direction_str,
                    confidence: analysis.confidence,
                    mechanism: analysis.mechanism,
                    mechanism_type: mech_type,
                    inference_time_ms: elapsed.as_millis() as u64,
                });
            }
            Err(e) => {
                println!("│ {:>2} │ ✗ {} ERROR: {:?} │", i + 1, truncate(claim, 30), e);
            }
        }
    }

    println!("└────┴───────────────────────────────────────────────────────────────────────┘\n");

    let total_time = benchmark_start.elapsed();

    // Calculate statistics
    let pairs_with_causal = results.iter().filter(|r| r.has_causal_link).count();
    let pairs_without_causal = results.len() - pairs_with_causal;
    let avg_confidence: f32 = results.iter().map(|r| r.confidence).sum::<f32>() / results.len() as f32;
    let inference_times: Vec<u64> = results.iter().map(|r| r.inference_time_ms).collect();
    let avg_time = inference_times.iter().sum::<u64>() as f64 / inference_times.len() as f64;
    let min_time = *inference_times.iter().min().unwrap_or(&0);
    let max_time = *inference_times.iter().max().unwrap_or(&0);
    let throughput = results.len() as f64 / total_time.as_secs_f64();

    let mode_name = if use_few_shot { "few_shot" } else { "zero_shot" };

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║              {} RESULTS                              ║", mode_name.to_uppercase());
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Total Pairs Analyzed: {:>40} ║", results.len());
    println!("║ Pairs with Causal Link: {:>38} ║", pairs_with_causal);
    println!("║ Pairs without Causal Link: {:>35} ║", pairs_without_causal);
    println!("║ Average Confidence: {:>41.2} ║", avg_confidence);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Average Inference Time: {:>34.0}ms ║", avg_time);
    println!("║ Min/Max Inference Time: {:>25}ms / {}ms ║", min_time, max_time);
    println!("║ Throughput: {:>42.2} pairs/sec ║", throughput);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Direction Distribution:                                       ║");
    for (direction, count) in &direction_counts {
        println!("║   {:20}: {:>38} ║", direction, count);
    }
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Mechanism Type Distribution:                                  ║");
    for (mech_type, count) in &mechanism_type_counts {
        println!("║   {:20}: {:>38} ║", mech_type, count);
    }
    let no_mech_type = results.iter().filter(|r| r.mechanism_type.is_none()).count();
    if no_mech_type > 0 {
        println!("║   {:20}: {:>38} ║", "N/A (not detected)", no_mech_type);
    }
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(ModeResults {
        mode: mode_name.to_string(),
        total_pairs: results.len(),
        pairs_with_causal_link: pairs_with_causal,
        pairs_without_causal_link: pairs_without_causal,
        avg_confidence,
        avg_inference_time_ms: avg_time,
        min_inference_time_ms: min_time,
        max_inference_time_ms: max_time,
        total_time_sec: total_time.as_secs_f64(),
        throughput_pairs_per_sec: throughput,
        direction_distribution: direction_counts,
        mechanism_type_distribution: mechanism_type_counts,
        model_load_time_sec: model_load_time.as_secs_f64(),
        detailed_results: results,
    })
}

fn calculate_comparison(few_shot: &ModeResults, non_few_shot: &ModeResults) -> ComparisonMetrics {
    let inference_speedup = few_shot.avg_inference_time_ms / non_few_shot.avg_inference_time_ms;
    let confidence_delta = few_shot.avg_confidence - non_few_shot.avg_confidence;

    // Calculate agreement rates
    let mut causal_agreements = 0;
    let mut direction_agreements = 0;
    let total = few_shot.detailed_results.len().min(non_few_shot.detailed_results.len());

    for (fs, nfs) in few_shot.detailed_results.iter().zip(non_few_shot.detailed_results.iter()) {
        if fs.has_causal_link == nfs.has_causal_link {
            causal_agreements += 1;
        }
        if fs.direction == nfs.direction {
            direction_agreements += 1;
        }
    }

    let causal_link_agreement_rate = causal_agreements as f64 / total as f64;
    let direction_agreement_rate = direction_agreements as f64 / total as f64;

    // Mechanism type coverage
    let fs_with_mech_type = few_shot.detailed_results.iter()
        .filter(|r| r.mechanism_type.is_some()).count();
    let nfs_with_mech_type = non_few_shot.detailed_results.iter()
        .filter(|r| r.mechanism_type.is_some()).count();

    let mechanism_type_coverage_few_shot = fs_with_mech_type as f64 / few_shot.total_pairs as f64;
    let mechanism_type_coverage_non_few_shot = nfs_with_mech_type as f64 / non_few_shot.total_pairs as f64;

    ComparisonMetrics {
        inference_speedup,
        confidence_delta,
        causal_link_agreement_rate,
        direction_agreement_rate,
        mechanism_type_coverage_few_shot,
        mechanism_type_coverage_non_few_shot,
    }
}

fn print_comparison_summary(few_shot: &ModeResults, non_few_shot: &ModeResults, comparison: &ComparisonMetrics) {
    println!("\n");
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    COMPARISON SUMMARY                          ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Metric                    │ Few-Shot    │ Zero-Shot   │ Δ     ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Avg Inference Time        │ {:>7.0}ms   │ {:>7.0}ms   │ {:>+.1}x ║",
        few_shot.avg_inference_time_ms,
        non_few_shot.avg_inference_time_ms,
        comparison.inference_speedup
    );
    println!(
        "║ Avg Confidence            │ {:>7.2}     │ {:>7.2}     │ {:>+.2} ║",
        few_shot.avg_confidence,
        non_few_shot.avg_confidence,
        comparison.confidence_delta
    );
    println!(
        "║ Throughput (pairs/sec)    │ {:>7.2}     │ {:>7.2}     │       ║",
        few_shot.throughput_pairs_per_sec,
        non_few_shot.throughput_pairs_per_sec
    );
    println!(
        "║ Causal Links Detected     │ {:>7}     │ {:>7}     │       ║",
        few_shot.pairs_with_causal_link,
        non_few_shot.pairs_with_causal_link
    );
    println!(
        "║ Mechanism Type Coverage   │ {:>6.0}%     │ {:>6.0}%     │       ║",
        comparison.mechanism_type_coverage_few_shot * 100.0,
        comparison.mechanism_type_coverage_non_few_shot * 100.0
    );
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Causal Link Agreement Rate: {:>6.1}%                            ║",
        comparison.causal_link_agreement_rate * 100.0
    );
    println!(
        "║ Direction Agreement Rate:   {:>6.1}%                            ║",
        comparison.direction_agreement_rate * 100.0
    );
    println!("╚═══════════════════════════════════════════════════════════════╝");
}

fn load_queries(path: &PathBuf) -> Result<Vec<Query>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut queries = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            let query: Query = serde_json::from_str(&line)?;
            queries.push(query);
        }
    }

    Ok(queries)
}

fn load_chunks(path: &PathBuf) -> Result<Vec<Chunk>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut chunks = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            let chunk: Chunk = serde_json::from_str(&line)?;
            chunks.push(chunk);
        }
    }

    Ok(chunks)
}

fn load_qrels(path: &PathBuf) -> Result<HashMap<String, HashMap<String, i32>>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let qrels: HashMap<String, HashMap<String, i32>> = serde_json::from_reader(file)?;
    Ok(qrels)
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
