//! Large-scale Causal Discovery Benchmark.
//!
//! Run with: BENCHMARK_PAIRS=5000 cargo run -p context-graph-causal-agent --example benchmark_causal_large --features cuda --release
//!
//! Tests causal discovery on large datasets by generating pairs from:
//! 1. Adjacent chunks within same documents (high chance of causal content)
//! 2. Chunks from same topic area (medium chance)
//! 3. Random pairs (baseline - low chance expected)

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Instant;

use context_graph_causal_agent::llm::{CausalDiscoveryLLM, LlmConfig};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

/// A document chunk from the dataset.
#[derive(Debug, Clone, Deserialize)]
struct Chunk {
    id: String,
    doc_id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    chunk_idx: usize,
    text: String,
    #[serde(default)]
    topic_hint: Option<String>,
    #[serde(default)]
    source_dataset: Option<String>,
}

/// Pair type for analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
enum PairType {
    /// Adjacent chunks in same document (high causal likelihood)
    Adjacent,
    /// Same topic but different documents (medium likelihood)
    SameTopic,
    /// Random pairing (low likelihood - baseline)
    Random,
}

/// Result for a single pair.
#[derive(Debug, Serialize)]
struct PairResult {
    pair_type: PairType,
    chunk_a_id: String,
    chunk_b_id: String,
    has_causal_link: bool,
    direction: String,
    confidence: f32,
    mechanism_type: Option<String>,
    inference_time_ms: u64,
}

/// Aggregated results by pair type.
#[derive(Debug, Serialize)]
struct PairTypeStats {
    pair_type: String,
    total: usize,
    causal_links: usize,
    causal_rate: f64,
    avg_confidence: f32,
    avg_inference_time_ms: f64,
    direction_distribution: HashMap<String, usize>,
    mechanism_type_distribution: HashMap<String, usize>,
}

/// Overall benchmark results.
#[derive(Debug, Serialize)]
struct LargeBenchmarkResults {
    timestamp: String,
    dataset: String,
    total_pairs: usize,
    total_time_sec: f64,
    throughput_pairs_per_sec: f64,
    model_load_time_sec: f64,

    // By pair type
    adjacent_stats: PairTypeStats,
    same_topic_stats: PairTypeStats,
    random_stats: PairTypeStats,

    // Overall
    overall_causal_rate: f64,
    overall_avg_confidence: f32,
    overall_avg_inference_time_ms: f64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     Large-Scale Causal Discovery Benchmark                    ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Find workspace root
    let mut workspace_root = std::env::current_dir()?;
    while !workspace_root.join("data").exists() {
        if !workspace_root.pop() {
            return Err("Could not find workspace root".into());
        }
    }

    // Try different datasets in order of preference
    let datasets = [
        ("hf_benchmark_diverse", "Wikipedia/Diverse"),
        ("semantic_benchmark", "ArXiv/Semantic"),
        ("beir_scifact", "SciFact"),
    ];

    let mut data_path = None;
    let mut dataset_name = "";

    for (dir, name) in &datasets {
        let path = workspace_root.join(format!("data/{}/chunks.jsonl", dir));
        if path.exists() {
            data_path = Some(path);
            dataset_name = name;
            break;
        }
    }

    let data_path = data_path.ok_or("No dataset found")?;
    let model_dir = workspace_root.join("models/hermes-2-pro");

    println!("Dataset: {} ({:?})", dataset_name, data_path);
    println!("Model: {:?}", model_dir);
    println!();

    // Load chunks
    println!("Loading chunks...");
    let load_start = Instant::now();
    let chunks = load_chunks(&data_path)?;
    println!("  Loaded {} chunks in {:?}", chunks.len(), load_start.elapsed());

    // Get target pair count
    let target_pairs: usize = std::env::var("BENCHMARK_PAIRS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);

    println!("  Target pairs: {} (set BENCHMARK_PAIRS env to change)", target_pairs);

    // Generate pairs
    println!("\nGenerating pairs...");
    let pairs = generate_pairs(&chunks, target_pairs)?;

    let adjacent_count = pairs.iter().filter(|(t, _, _)| *t == PairType::Adjacent).count();
    let same_topic_count = pairs.iter().filter(|(t, _, _)| *t == PairType::SameTopic).count();
    let random_count = pairs.iter().filter(|(t, _, _)| *t == PairType::Random).count();

    println!("  Generated {} pairs:", pairs.len());
    println!("    - Adjacent: {}", adjacent_count);
    println!("    - Same Topic: {}", same_topic_count);
    println!("    - Random: {}", random_count);

    // Initialize LLM (zero-shot for speed on large benchmark)
    let config = LlmConfig {
        model_path: model_dir.join("Hermes-2-Pro-Mistral-7B.Q5_K_M.gguf"),
        causal_grammar_path: model_dir.join("causal_analysis.gbnf"),
        graph_grammar_path: model_dir.join("graph_relationship.gbnf"),
        validation_grammar_path: model_dir.join("validation.gbnf"),
        n_gpu_layers: u32::MAX,
        temperature: 0.0,
        max_tokens: 256,
        use_few_shot: false, // Zero-shot for speed
        ..Default::default()
    };

    let llm = CausalDiscoveryLLM::with_config(config)?;

    println!("\nLoading model...");
    let model_load_start = Instant::now();
    llm.load().await?;
    let model_load_time = model_load_start.elapsed();
    println!("✓ Model loaded in {:.2?}", model_load_time);

    // Run benchmark
    println!("\nRunning benchmark on {} pairs...\n", pairs.len());

    let benchmark_start = Instant::now();
    let mut results: Vec<PairResult> = Vec::with_capacity(pairs.len());

    // Progress tracking
    let progress_interval = (pairs.len() / 20).max(1);
    let mut last_progress = 0;

    for (i, (pair_type, chunk_a, chunk_b)) in pairs.iter().enumerate() {
        let text_a = truncate(&chunk_a.text, 750);
        let text_b = truncate(&chunk_b.text, 750);

        let start = Instant::now();
        let result = llm.analyze_causal_relationship(&text_a, &text_b).await;
        let elapsed = start.elapsed();

        match result {
            Ok(analysis) => {
                results.push(PairResult {
                    pair_type: *pair_type,
                    chunk_a_id: chunk_a.id.clone(),
                    chunk_b_id: chunk_b.id.clone(),
                    has_causal_link: analysis.has_causal_link,
                    direction: format!("{:?}", analysis.direction),
                    confidence: analysis.confidence,
                    mechanism_type: analysis.mechanism_type.map(|m| m.as_str().to_string()),
                    inference_time_ms: elapsed.as_millis() as u64,
                });
            }
            Err(e) => {
                eprintln!("  Error on pair {}: {:?}", i, e);
            }
        }

        // Progress update
        if i - last_progress >= progress_interval || i == pairs.len() - 1 {
            let elapsed = benchmark_start.elapsed();
            let rate = (i + 1) as f64 / elapsed.as_secs_f64();
            let eta = (pairs.len() - i - 1) as f64 / rate;
            print!("\r  Progress: {}/{} ({:.1}%) | {:.1} pairs/sec | ETA: {:.0}s    ",
                   i + 1, pairs.len(),
                   (i + 1) as f64 / pairs.len() as f64 * 100.0,
                   rate, eta);
            std::io::stdout().flush()?;
            last_progress = i;
        }
    }
    println!();

    let total_time = benchmark_start.elapsed();

    // Calculate statistics by pair type
    let adjacent_stats = calculate_stats(&results, PairType::Adjacent);
    let same_topic_stats = calculate_stats(&results, PairType::SameTopic);
    let random_stats = calculate_stats(&results, PairType::Random);

    // Overall stats
    let overall_causal = results.iter().filter(|r| r.has_causal_link).count();
    let overall_causal_rate = overall_causal as f64 / results.len() as f64;
    let overall_avg_confidence = results.iter().map(|r| r.confidence).sum::<f32>() / results.len() as f32;
    let overall_avg_time = results.iter().map(|r| r.inference_time_ms).sum::<u64>() as f64 / results.len() as f64;

    // Print results
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    BENCHMARK RESULTS                          ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ Total Pairs: {:>48} ║", results.len());
    println!("║ Total Time: {:>46.1}s ║", total_time.as_secs_f64());
    println!("║ Throughput: {:>42.2} pairs/sec ║", results.len() as f64 / total_time.as_secs_f64());
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ RESULTS BY PAIR TYPE:                                         ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    print_pair_type_stats("Adjacent (same doc)", &adjacent_stats);
    print_pair_type_stats("Same Topic", &same_topic_stats);
    print_pair_type_stats("Random (baseline)", &random_stats);
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║ OVERALL:                                                      ║");
    println!("║   Causal Rate: {:>45.1}% ║", overall_causal_rate * 100.0);
    println!("║   Avg Confidence: {:>42.2} ║", overall_avg_confidence);
    println!("║   Avg Inference Time: {:>36.0}ms ║", overall_avg_time);
    println!("╚═══════════════════════════════════════════════════════════════╝");

    // Save results
    let benchmark_results = LargeBenchmarkResults {
        timestamp: chrono::Utc::now().to_rfc3339(),
        dataset: dataset_name.to_string(),
        total_pairs: results.len(),
        total_time_sec: total_time.as_secs_f64(),
        throughput_pairs_per_sec: results.len() as f64 / total_time.as_secs_f64(),
        model_load_time_sec: model_load_time.as_secs_f64(),
        adjacent_stats,
        same_topic_stats,
        random_stats,
        overall_causal_rate,
        overall_avg_confidence,
        overall_avg_inference_time_ms: overall_avg_time,
    };

    let output_path = workspace_root.join("benchmark_results/causal_benchmark_large.json");
    std::fs::create_dir_all(output_path.parent().unwrap())?;
    let output_file = File::create(&output_path)?;
    serde_json::to_writer_pretty(output_file, &benchmark_results)?;
    println!("\nResults saved to: {:?}", output_path);

    Ok(())
}

fn load_chunks(path: &PathBuf) -> Result<Vec<Chunk>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut chunks = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if !line.is_empty() {
            match serde_json::from_str::<Chunk>(&line) {
                Ok(chunk) => chunks.push(chunk),
                Err(_) => continue, // Skip malformed lines
            }
        }
    }

    Ok(chunks)
}

fn generate_pairs(chunks: &[Chunk], target_count: usize) -> Result<Vec<(PairType, Chunk, Chunk)>, Box<dyn std::error::Error>> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(42); // Reproducible
    let mut pairs = Vec::new();

    // Group chunks by document
    let mut by_doc: HashMap<String, Vec<&Chunk>> = HashMap::new();
    for chunk in chunks {
        by_doc.entry(chunk.doc_id.clone()).or_default().push(chunk);
    }

    // Sort chunks within each document by chunk_idx
    for doc_chunks in by_doc.values_mut() {
        doc_chunks.sort_by_key(|c| c.chunk_idx);
    }

    // Group chunks by topic
    let mut by_topic: HashMap<String, Vec<&Chunk>> = HashMap::new();
    for chunk in chunks {
        let topic = chunk.topic_hint.clone().unwrap_or_else(|| chunk.doc_id.clone());
        by_topic.entry(topic).or_default().push(chunk);
    }

    // Target distribution: 40% adjacent, 30% same topic, 30% random
    let adjacent_target = target_count * 40 / 100;
    let same_topic_target = target_count * 30 / 100;
    let random_target = target_count - adjacent_target - same_topic_target;

    // Generate adjacent pairs (consecutive chunks in same document)
    let mut adjacent_candidates: Vec<(Chunk, Chunk)> = Vec::new();
    for doc_chunks in by_doc.values() {
        for window in doc_chunks.windows(2) {
            adjacent_candidates.push((window[0].clone(), window[1].clone()));
        }
    }
    adjacent_candidates.shuffle(&mut rng);
    for (a, b) in adjacent_candidates.into_iter().take(adjacent_target) {
        pairs.push((PairType::Adjacent, a, b));
    }

    // Generate same-topic pairs (different documents, same topic)
    let mut same_topic_candidates: Vec<(Chunk, Chunk)> = Vec::new();
    for topic_chunks in by_topic.values() {
        if topic_chunks.len() >= 2 {
            // Sample pairs from same topic
            for i in 0..topic_chunks.len().min(10) {
                for j in (i + 1)..topic_chunks.len().min(10) {
                    if topic_chunks[i].doc_id != topic_chunks[j].doc_id {
                        same_topic_candidates.push((topic_chunks[i].clone(), topic_chunks[j].clone()));
                    }
                }
            }
        }
    }
    same_topic_candidates.shuffle(&mut rng);
    for (a, b) in same_topic_candidates.into_iter().take(same_topic_target) {
        pairs.push((PairType::SameTopic, a, b));
    }

    // Generate random pairs
    let mut indices: Vec<usize> = (0..chunks.len()).collect();
    indices.shuffle(&mut rng);
    let mut random_count = 0;
    let mut idx = 0;
    while random_count < random_target && idx + 1 < indices.len() {
        let a = &chunks[indices[idx]];
        let b = &chunks[indices[idx + 1]];
        // Ensure different documents
        if a.doc_id != b.doc_id {
            pairs.push((PairType::Random, a.clone(), b.clone()));
            random_count += 1;
        }
        idx += 2;
    }

    // Shuffle all pairs
    pairs.shuffle(&mut rng);

    Ok(pairs)
}

fn calculate_stats(results: &[PairResult], pair_type: PairType) -> PairTypeStats {
    let filtered: Vec<_> = results.iter().filter(|r| r.pair_type == pair_type).collect();

    if filtered.is_empty() {
        return PairTypeStats {
            pair_type: format!("{:?}", pair_type),
            total: 0,
            causal_links: 0,
            causal_rate: 0.0,
            avg_confidence: 0.0,
            avg_inference_time_ms: 0.0,
            direction_distribution: HashMap::new(),
            mechanism_type_distribution: HashMap::new(),
        };
    }

    let causal_count = filtered.iter().filter(|r| r.has_causal_link).count();
    let avg_conf = filtered.iter().map(|r| r.confidence).sum::<f32>() / filtered.len() as f32;
    let avg_time = filtered.iter().map(|r| r.inference_time_ms).sum::<u64>() as f64 / filtered.len() as f64;

    let mut direction_dist: HashMap<String, usize> = HashMap::new();
    let mut mech_type_dist: HashMap<String, usize> = HashMap::new();

    for r in &filtered {
        *direction_dist.entry(r.direction.clone()).or_insert(0) += 1;
        if let Some(ref mt) = r.mechanism_type {
            *mech_type_dist.entry(mt.clone()).or_insert(0) += 1;
        }
    }

    PairTypeStats {
        pair_type: format!("{:?}", pair_type),
        total: filtered.len(),
        causal_links: causal_count,
        causal_rate: causal_count as f64 / filtered.len() as f64,
        avg_confidence: avg_conf,
        avg_inference_time_ms: avg_time,
        direction_distribution: direction_dist,
        mechanism_type_distribution: mech_type_dist,
    }
}

fn print_pair_type_stats(name: &str, stats: &PairTypeStats) {
    println!("║ {:20} ({:>4} pairs):                         ║", name, stats.total);
    println!("║   Causal Rate: {:>45.1}% ║", stats.causal_rate * 100.0);
    println!("║   Avg Confidence: {:>42.2} ║", stats.avg_confidence);
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        // Find a safe truncation point (char boundary)
        let truncated: String = s.chars().take(max_len).collect();
        // Find last space for word boundary
        if let Some(last_space) = truncated.rfind(' ') {
            format!("{}...", &truncated[..last_space])
        } else {
            format!("{}...", truncated)
        }
    }
}
