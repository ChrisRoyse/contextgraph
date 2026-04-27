//! Benchmark for Causal Discovery LLM using SciFact data.
//!
//! Run with: cargo run -p context-graph-causal-agent --example benchmark_causal --features cuda --release
//!
//! This benchmark tests the causal discovery LLM's ability to identify causal relationships
//! between scientific claims and evidence from the SciFact dataset.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::Instant;

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use serde::{Deserialize, Serialize};

/// A query/claim from the dataset.
#[derive(Debug, Deserialize)]
struct Query {
    query_id: String,
    text: String,
}

/// A document chunk from the dataset.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Chunk {
    id: String,
    doc_id: String,
    original_doc_id: String,
    title: String,
    text: String,
    #[serde(default)]
    word_count: usize,
}

/// Benchmark result for a single pair.
#[derive(Debug, Serialize)]
struct PairResult {
    query_id: String,
    doc_id: String,
    claim: String,
    evidence_title: String,
    has_causal_link: bool,
    direction: String,
    confidence: f32,
    mechanism: String,
    inference_time_ms: u64,
}

/// Overall benchmark results.
#[derive(Debug, Serialize)]
struct BenchmarkResults {
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
    model_load_time_sec: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     Causal Discovery LLM Benchmark (SciFact Dataset)          ║");
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
            for doc_id in doc_ids.keys() {
                if let Some(chunk) = doc_index.get(doc_id) {
                    pairs.push((query, *chunk));
                }
            }
        }
    }

    println!(
        "  Created {} claim-evidence pairs for benchmarking",
        pairs.len()
    );
    println!();

    // Limit pairs for benchmarking (full dataset takes too long)
    let max_pairs = std::env::var("BENCHMARK_PAIRS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let pairs = &pairs[..pairs.len().min(max_pairs)];
    println!(
        "Running benchmark on {} pairs (set BENCHMARK_PAIRS env to change)\n",
        pairs.len()
    );

    // Initialize LLM with Hermes 2 Pro and GBNF grammar constraints
    let config = LlmConfig {
        model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
        causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
        graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
        validation_grammar_path: model_dir.join("validation.gbnf"),
        n_gpu_layers: u32::MAX, // Full GPU offload
        temperature: 0.0,       // Deterministic
        max_tokens: 256,
        ..Default::default()
    };

    let llm = CausalDiscoveryLLM::with_config(config)?;

    println!("Loading model...");
    let model_load_start = Instant::now();
    llm.load().await?;
    let model_load_time = model_load_start.elapsed();
    println!("✓ Model loaded in {:.2?}\n", model_load_time);

    // Run benchmark
    println!("Running benchmark...\n");
    println!("┌────┬──────────────────────────────────────────────────────────────┐");
    println!("│ #  │ Claim (truncated) → Result                                   │");
    println!("├────┼──────────────────────────────────────────────────────────────┤");

    let benchmark_start = Instant::now();
    let mut results: Vec<PairResult> = Vec::new();
    let mut direction_counts: HashMap<String, usize> = HashMap::new();

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

                let symbol = if analysis.has_causal_link {
                    "✓"
                } else {
                    "✗"
                };
                let conf = format!("{:.2}", analysis.confidence);

                println!(
                    "│ {:>2} │ {} {}  [{} {}] {:>6}ms │",
                    i + 1,
                    symbol,
                    truncate(claim, 35),
                    &direction_str[..direction_str.len().min(10)],
                    conf,
                    elapsed.as_millis()
                );

                results.push(PairResult {
                    query_id: query.query_id.clone(),
                    doc_id: chunk.original_doc_id.clone(),
                    claim: claim.clone(),
                    evidence_title: chunk.title.clone(),
                    has_causal_link: analysis.has_causal_link,
                    direction: direction_str,
                    confidence: analysis.confidence,
                    mechanism: analysis.mechanism,
                    inference_time_ms: elapsed.as_millis() as u64,
                });
            }
            Err(e) => {
                println!("│ {:>2} │ ✗ {} ERROR: {:?}│", i + 1, truncate(claim, 30), e);
            }
        }
    }

    println!("└────┴──────────────────────────────────────────────────────────────┘\n");

    let total_time = benchmark_start.elapsed();

    // Calculate statistics
    let pairs_with_causal = results.iter().filter(|r| r.has_causal_link).count();
    let pairs_without_causal = results.len() - pairs_with_causal;
    let avg_confidence: f32 =
        results.iter().map(|r| r.confidence).sum::<f32>() / results.len() as f32;
    let inference_times: Vec<u64> = results.iter().map(|r| r.inference_time_ms).collect();
    let avg_time = inference_times.iter().sum::<u64>() as f64 / inference_times.len() as f64;
    let min_time = *inference_times.iter().min().unwrap_or(&0);
    let max_time = *inference_times.iter().max().unwrap_or(&0);
    let throughput = results.len() as f64 / total_time.as_secs_f64();

    let benchmark_results = BenchmarkResults {
        total_pairs: results.len(),
        pairs_with_causal_link: pairs_with_causal,
        pairs_without_causal_link: pairs_without_causal,
        avg_confidence,
        avg_inference_time_ms: avg_time,
        min_inference_time_ms: min_time,
        max_inference_time_ms: max_time,
        total_time_sec: total_time.as_secs_f64(),
        throughput_pairs_per_sec: throughput,
        direction_distribution: direction_counts.clone(),
        model_load_time_sec: model_load_time.as_secs_f64(),
    };

    // Print summary
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                     BENCHMARK RESULTS                          ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Total Pairs Analyzed: {:>40} ║", results.len());
    println!("║ Pairs with Causal Link: {:>38} ║", pairs_with_causal);
    println!(
        "║ Pairs without Causal Link: {:>35} ║",
        pairs_without_causal
    );
    println!("║ Average Confidence: {:>41.2} ║", avg_confidence);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!(
        "║ Model Load Time: {:>40.2}s ║",
        model_load_time.as_secs_f64()
    );
    println!(
        "║ Total Benchmark Time: {:>36.2}s ║",
        total_time.as_secs_f64()
    );
    println!("║ Average Inference Time: {:>34.0}ms ║", avg_time);
    println!("║ Min Inference Time: {:>38}ms ║", min_time);
    println!("║ Max Inference Time: {:>38}ms ║", max_time);
    println!("║ Throughput: {:>42.2} pairs/sec ║", throughput);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Direction Distribution:                                       ║");
    for (direction, count) in &direction_counts {
        println!("║   {:20}: {:>38} ║", direction, count);
    }
    println!("╚═══════════════════════════════════════════════════════════════╝");

    // Save results to file
    let output_path = workspace_root.join("benchmark_results/causal_benchmark.json");
    std::fs::create_dir_all(output_path.parent().unwrap())?;
    let output_file = File::create(&output_path)?;
    serde_json::to_writer_pretty(output_file, &benchmark_results)?;
    println!("\nResults saved to: {:?}", output_path);

    Ok(())
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

fn load_qrels(
    path: &PathBuf,
) -> Result<HashMap<String, HashMap<String, i32>>, Box<dyn std::error::Error>> {
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
