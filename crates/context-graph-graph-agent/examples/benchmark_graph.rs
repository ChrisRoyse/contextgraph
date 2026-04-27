//! Benchmark for Graph Relationship Discovery using SciFact data.
//!
//! Run with: cargo run -p context-graph-graph-agent --example benchmark_graph --features cuda --release
//!
//! This benchmark tests the graph discovery service's ability to identify structural
//! relationships between scientific documents from the SciFact dataset.
//!
//! # Metrics Collected
//!
//! - LLM inference time per pair
//! - Relationship type distribution
//! - Confidence score distribution
//! - Discovery cycle throughput
//! - Scanner heuristic accuracy (markers found vs LLM confirmed)

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use context_graph_graph_agent::{
    GraphDiscoveryConfig, GraphDiscoveryService, MemoryForGraphAnalysis,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    memory_a_id: String,
    memory_b_id: String,
    memory_a_snippet: String,
    memory_b_snippet: String,
    has_connection: bool,
    relationship_type: String,
    direction: String,
    confidence: f32,
    description: String,
    inference_time_ms: u64,
    heuristic_markers_a: usize,
    heuristic_markers_b: usize,
}

/// Per-relationship-type statistics.
#[derive(Debug, Default, Serialize)]
struct RelationshipStats {
    count: usize,
    avg_confidence: f32,
    avg_inference_time_ms: f64,
    confidences: Vec<f32>,
}

/// Overall benchmark results.
#[derive(Debug, Serialize)]
struct BenchmarkResults {
    // Basic counts
    total_pairs: usize,
    pairs_with_connection: usize,
    pairs_without_connection: usize,

    // Confidence stats
    avg_confidence: f32,
    min_confidence: f32,
    max_confidence: f32,
    median_confidence: f32,

    // Timing stats
    avg_inference_time_ms: f64,
    min_inference_time_ms: u64,
    max_inference_time_ms: u64,
    median_inference_time_ms: u64,
    p95_inference_time_ms: u64,
    p99_inference_time_ms: u64,
    total_time_sec: f64,
    throughput_pairs_per_sec: f64,

    // Model load time
    model_load_time_sec: f64,

    // Relationship type distribution
    relationship_distribution: HashMap<String, RelationshipStats>,

    // Direction distribution
    direction_distribution: HashMap<String, usize>,

    // Heuristic analysis
    heuristic_accuracy: HeuristicAccuracy,
}

/// Heuristic accuracy metrics.
#[derive(Debug, Default, Serialize)]
struct HeuristicAccuracy {
    /// Pairs where heuristics found markers
    pairs_with_markers: usize,
    /// Pairs where LLM confirmed connection AND heuristics found markers
    true_positives: usize,
    /// Pairs where LLM confirmed but heuristics missed
    false_negatives: usize,
    /// Pairs where heuristics found markers but LLM rejected
    false_positives: usize,
    /// Precision: TP / (TP + FP)
    precision: f64,
    /// Recall: TP / (TP + FN)
    recall: f64,
    /// F1 score: 2 * (precision * recall) / (precision + recall)
    f1_score: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║         Graph Relationship Discovery Benchmark (SciFact Dataset)              ║");
    println!("║                    E8 (1024D) + Qwen2.5-3B LLM Pipeline                        ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝\n");

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

    // Create memory pairs for graph analysis
    // We'll pair queries with their relevant documents (simulating memory pairs)
    let mut memories: Vec<MemoryForGraphAnalysis> = Vec::new();

    for (query_id, doc_ids) in &qrels {
        if let Some(query) = queries.iter().find(|q| &q.query_id == query_id) {
            // Add query as a memory
            memories.push(MemoryForGraphAnalysis {
                id: Uuid::new_v4(),
                content: query.text.clone(),
                created_at: Utc::now(),
                session_id: Some(query_id.clone()),
                e1_embedding: Vec::new(), // Stub - scanner uses heuristics
                source_type: Some("Query".to_string()),
                file_path: None,
            });

            // Add relevant documents as memories
            for doc_id in doc_ids.keys() {
                if let Some(chunk) = doc_index.get(doc_id) {
                    memories.push(MemoryForGraphAnalysis {
                        id: Uuid::new_v4(),
                        content: format!("{}: {}", chunk.title, chunk.text),
                        created_at: Utc::now(),
                        session_id: Some(query_id.clone()),
                        e1_embedding: Vec::new(),
                        source_type: Some("Document".to_string()),
                        file_path: Some(doc_id.clone()),
                    });
                }
            }
        }
    }

    println!("  Created {} memories for graph analysis", memories.len());
    println!();

    // Limit memories for benchmarking
    let max_memories = std::env::var("BENCHMARK_MEMORIES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(50);

    let memories = &memories[..memories.len().min(max_memories)];
    println!(
        "Running benchmark on {} memories (set BENCHMARK_MEMORIES env to change)\n",
        memories.len()
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

    println!("Initializing CausalDiscoveryLLM (NO FALLBACKS)...");
    let llm = CausalDiscoveryLLM::with_config(config)?;

    println!("Loading model (~6GB VRAM required)...");
    let model_load_start = Instant::now();
    llm.load().await?;
    let model_load_time = model_load_start.elapsed();
    println!("✓ Model loaded in {:.2?}\n", model_load_time);

    // Create GraphDiscoveryService
    let graph_config = GraphDiscoveryConfig {
        batch_size: max_memories,
        min_confidence: 0.5,
        ..Default::default()
    };

    let service = GraphDiscoveryService::with_config(Arc::new(llm), graph_config);

    // Run benchmark
    println!("Running graph discovery benchmark...\n");
    println!(
        "┌────┬──────────────────────────────────────────────────────────────────────────────┐"
    );
    println!(
        "│ #  │ Memory A → Memory B                               │ Result                  │"
    );
    println!(
        "├────┼──────────────────────────────────────────────────────────────────────────────┤"
    );

    let benchmark_start = Instant::now();
    let mut results: Vec<PairResult> = Vec::new();
    let mut direction_counts: HashMap<String, usize> = HashMap::new();
    let mut relationship_stats: HashMap<String, RelationshipStats> = HashMap::new();
    let mut heuristic_acc = HeuristicAccuracy::default();

    // Run discovery cycle
    let cycle_result = service.run_discovery_cycle(memories).await?;

    // Analyze each pair from the scanner
    // For detailed per-pair timing, we need to analyze individually
    let llm = service.llm();

    // Create pairs from consecutive memories (simple pairing strategy)
    let pairs: Vec<(&MemoryForGraphAnalysis, &MemoryForGraphAnalysis)> =
        memories.windows(2).map(|w| (&w[0], &w[1])).collect();

    let max_pairs = std::env::var("BENCHMARK_PAIRS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let pairs = &pairs[..pairs.len().min(max_pairs)];

    for (i, (mem_a, mem_b)) in pairs.iter().enumerate() {
        // Count heuristic markers
        let markers_a = context_graph_graph_agent::GraphMarkers::count_all_markers(&mem_a.content);
        let markers_b = context_graph_graph_agent::GraphMarkers::count_all_markers(&mem_b.content);
        let has_markers = markers_a > 0 || markers_b > 0;

        if has_markers {
            heuristic_acc.pairs_with_markers += 1;
        }

        let start = Instant::now();
        let analysis = llm
            .analyze_relationship(&mem_a.content, &mem_b.content)
            .await;
        let elapsed = start.elapsed();

        match analysis {
            Ok(result) => {
                let rel_type_str = result.relationship_type.as_str().to_string();
                let direction_str = result.direction.as_str().to_string();

                // Update direction counts
                *direction_counts.entry(direction_str.clone()).or_insert(0) += 1;

                // Update relationship stats
                let stats = relationship_stats.entry(rel_type_str.clone()).or_default();
                stats.count += 1;
                stats.confidences.push(result.confidence);

                // Update heuristic accuracy
                if result.has_connection {
                    if has_markers {
                        heuristic_acc.true_positives += 1;
                    } else {
                        heuristic_acc.false_negatives += 1;
                    }
                } else if has_markers {
                    heuristic_acc.false_positives += 1;
                }

                let symbol = if result.has_connection { "✓" } else { "✗" };
                let conf = format!("{:.2}", result.confidence);

                println!(
                    "│ {:>2} │ {} → {} │ {} {} {} {:>5}ms │",
                    i + 1,
                    truncate(&mem_a.content, 20),
                    truncate(&mem_b.content, 20),
                    symbol,
                    truncate(&rel_type_str, 10),
                    conf,
                    elapsed.as_millis()
                );

                results.push(PairResult {
                    memory_a_id: mem_a.id.to_string(),
                    memory_b_id: mem_b.id.to_string(),
                    memory_a_snippet: truncate(&mem_a.content, 100),
                    memory_b_snippet: truncate(&mem_b.content, 100),
                    has_connection: result.has_connection,
                    relationship_type: rel_type_str,
                    direction: direction_str,
                    confidence: result.confidence,
                    description: result.description,
                    inference_time_ms: elapsed.as_millis() as u64,
                    heuristic_markers_a: markers_a,
                    heuristic_markers_b: markers_b,
                });
            }
            Err(e) => {
                println!(
                    "│ {:>2} │ {} → {} │ ERROR: {:?} │",
                    i + 1,
                    truncate(&mem_a.content, 20),
                    truncate(&mem_b.content, 20),
                    e
                );
            }
        }
    }

    println!(
        "└────┴──────────────────────────────────────────────────────────────────────────────┘\n"
    );

    let total_time = benchmark_start.elapsed();

    // Calculate statistics
    let pairs_with_conn = results.iter().filter(|r| r.has_connection).count();
    let pairs_without_conn = results.len() - pairs_with_conn;

    // Confidence stats
    let mut confidences: Vec<f32> = results.iter().map(|r| r.confidence).collect();
    confidences.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let avg_confidence = if !confidences.is_empty() {
        confidences.iter().sum::<f32>() / confidences.len() as f32
    } else {
        0.0
    };
    let min_confidence = *confidences.first().unwrap_or(&0.0);
    let max_confidence = *confidences.last().unwrap_or(&0.0);
    let median_confidence = if !confidences.is_empty() {
        confidences[confidences.len() / 2]
    } else {
        0.0
    };

    // Timing stats
    let mut times: Vec<u64> = results.iter().map(|r| r.inference_time_ms).collect();
    times.sort();

    let avg_time = if !times.is_empty() {
        times.iter().sum::<u64>() as f64 / times.len() as f64
    } else {
        0.0
    };
    let min_time = *times.first().unwrap_or(&0);
    let max_time = *times.last().unwrap_or(&0);
    let median_time = if !times.is_empty() {
        times[times.len() / 2]
    } else {
        0
    };
    let p95_time = if !times.is_empty() {
        times[(times.len() as f64 * 0.95) as usize]
    } else {
        0
    };
    let p99_time = if !times.is_empty() {
        times[(times.len() as f64 * 0.99) as usize]
    } else {
        0
    };

    let throughput = results.len() as f64 / total_time.as_secs_f64();

    // Finalize relationship stats
    for (_, stats) in relationship_stats.iter_mut() {
        if !stats.confidences.is_empty() {
            stats.avg_confidence =
                stats.confidences.iter().sum::<f32>() / stats.confidences.len() as f32;
        }
    }

    // Calculate heuristic accuracy metrics
    let tp = heuristic_acc.true_positives as f64;
    let fp = heuristic_acc.false_positives as f64;
    let fn_ = heuristic_acc.false_negatives as f64;

    heuristic_acc.precision = if tp + fp > 0.0 { tp / (tp + fp) } else { 0.0 };
    heuristic_acc.recall = if tp + fn_ > 0.0 { tp / (tp + fn_) } else { 0.0 };
    heuristic_acc.f1_score = if heuristic_acc.precision + heuristic_acc.recall > 0.0 {
        2.0 * (heuristic_acc.precision * heuristic_acc.recall)
            / (heuristic_acc.precision + heuristic_acc.recall)
    } else {
        0.0
    };

    let benchmark_results = BenchmarkResults {
        total_pairs: results.len(),
        pairs_with_connection: pairs_with_conn,
        pairs_without_connection: pairs_without_conn,
        avg_confidence,
        min_confidence,
        max_confidence,
        median_confidence,
        avg_inference_time_ms: avg_time,
        min_inference_time_ms: min_time,
        max_inference_time_ms: max_time,
        median_inference_time_ms: median_time,
        p95_inference_time_ms: p95_time,
        p99_inference_time_ms: p99_time,
        total_time_sec: total_time.as_secs_f64(),
        throughput_pairs_per_sec: throughput,
        model_load_time_sec: model_load_time.as_secs_f64(),
        relationship_distribution: relationship_stats,
        direction_distribution: direction_counts,
        heuristic_accuracy: heuristic_acc,
    };

    // Print summary
    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                           BENCHMARK RESULTS                                    ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ Total Pairs Analyzed:       {:>50} ║", results.len());
    println!("║ Pairs with Connection:      {:>50} ║", pairs_with_conn);
    println!("║ Pairs without Connection:   {:>50} ║", pairs_without_conn);
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ CONFIDENCE METRICS                                                            ║");
    println!("║   Average Confidence:       {:>50.3} ║", avg_confidence);
    println!("║   Min Confidence:           {:>50.3} ║", min_confidence);
    println!("║   Max Confidence:           {:>50.3} ║", max_confidence);
    println!(
        "║   Median Confidence:        {:>50.3} ║",
        median_confidence
    );
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ TIMING METRICS                                                                ║");
    println!(
        "║   Model Load Time:          {:>48.2}s ║",
        model_load_time.as_secs_f64()
    );
    println!(
        "║   Total Benchmark Time:     {:>48.2}s ║",
        total_time.as_secs_f64()
    );
    println!("║   Avg Inference Time:       {:>47.0}ms ║", avg_time);
    println!("║   Min Inference Time:       {:>47}ms ║", min_time);
    println!("║   Max Inference Time:       {:>47}ms ║", max_time);
    println!("║   Median Inference Time:    {:>47}ms ║", median_time);
    println!("║   P95 Inference Time:       {:>47}ms ║", p95_time);
    println!("║   P99 Inference Time:       {:>47}ms ║", p99_time);
    println!(
        "║   Throughput:               {:>44.2} pairs/sec ║",
        throughput
    );
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ RELATIONSHIP TYPE DISTRIBUTION                                                ║");
    for (rel_type, stats) in &benchmark_results.relationship_distribution {
        if stats.count > 0 {
            println!(
                "║   {:15}: {:>4} (avg conf: {:.2})                                      ║",
                rel_type, stats.count, stats.avg_confidence
            );
        }
    }
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ DIRECTION DISTRIBUTION                                                        ║");
    for (direction, count) in &benchmark_results.direction_distribution {
        println!("║   {:20}: {:>55} ║", direction, count);
    }
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!("║ HEURISTIC ACCURACY (Scanner vs LLM)                                           ║");
    println!(
        "║   Pairs with Markers:       {:>50} ║",
        benchmark_results.heuristic_accuracy.pairs_with_markers
    );
    println!(
        "║   True Positives:           {:>50} ║",
        benchmark_results.heuristic_accuracy.true_positives
    );
    println!(
        "║   False Positives:          {:>50} ║",
        benchmark_results.heuristic_accuracy.false_positives
    );
    println!(
        "║   False Negatives:          {:>50} ║",
        benchmark_results.heuristic_accuracy.false_negatives
    );
    println!(
        "║   Precision:                {:>50.3} ║",
        benchmark_results.heuristic_accuracy.precision
    );
    println!(
        "║   Recall:                   {:>50.3} ║",
        benchmark_results.heuristic_accuracy.recall
    );
    println!(
        "║   F1 Score:                 {:>50.3} ║",
        benchmark_results.heuristic_accuracy.f1_score
    );
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");

    // Print discovery cycle stats
    println!("\n╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                        DISCOVERY CYCLE RESULTS                                 ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║   Candidates Found:         {:>50} ║",
        cycle_result.candidates_found
    );
    println!(
        "║   Relationships Confirmed:  {:>50} ║",
        cycle_result.relationships_confirmed
    );
    println!(
        "║   Relationships Rejected:   {:>50} ║",
        cycle_result.relationships_rejected
    );
    println!(
        "║   Embeddings Generated:     {:>50} ║",
        cycle_result.embeddings_generated
    );
    println!(
        "║   Edges Created:            {:>50} ║",
        cycle_result.edges_created
    );
    println!(
        "║   Errors:                   {:>50} ║",
        cycle_result.errors
    );
    println!(
        "║   Duration:                 {:>48.2}s ║",
        cycle_result.duration.as_secs_f64()
    );
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");

    // Save results to file
    let output_path = workspace_root.join("benchmark_results/graph_benchmark.json");
    std::fs::create_dir_all(output_path.parent().unwrap())?;
    let output_file = File::create(&output_path)?;
    serde_json::to_writer_pretty(output_file, &benchmark_results)?;
    println!("\nResults saved to: {:?}", output_path);

    // Save detailed pair results
    let detailed_path = workspace_root.join("benchmark_results/graph_benchmark_detailed.json");
    let detailed_file = File::create(&detailed_path)?;
    serde_json::to_writer_pretty(detailed_file, &results)?;
    println!("Detailed results saved to: {:?}", detailed_path);

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
    let s = s.replace('\n', " ");
    if s.len() <= max_len {
        format!("{:width$}", s, width = max_len)
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
