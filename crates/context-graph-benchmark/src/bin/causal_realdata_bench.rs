#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! E5 Causal Embedder Benchmark with Real HuggingFace Data
//!
//! Integrates causal benchmarks with the 10,000+ document HuggingFace dataset.
//! Uses real E5 embeddings with dual vectors (as_cause, as_effect) for
//! asymmetric similarity evaluation.
//!
//! ## Key Features
//!
//! - **Direction Detection**: Test on real documents, not synthetic patterns
//! - **Asymmetric Retrieval**: Use E5's dual vectors for cause→effect ranking
//! - **COPA-Style Reasoning**: Generate questions from real document pairs
//! - **E5 Contribution Analysis**: Compare E5 asymmetric vs symmetric retrieval
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark with real embeddings:
//! cargo run -p context-graph-benchmark --bin causal-realdata-bench --release \
//!     --features real-embeddings -- --data-dir data/hf_benchmark
//!
//! # Quick test with limited chunks:
//! cargo run -p context-graph-benchmark --bin causal-realdata-bench --release \
//!     --features real-embeddings -- --data-dir data/hf_benchmark --max-chunks 500
//! ```

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_benchmark::realdata::embedder::{EmbeddedDataset, RealDataEmbedder};
use context_graph_benchmark::realdata::loader::{ChunkRecord, DatasetLoader, RealDataset};
use context_graph_core::causal::asymmetric::{detect_causal_query_intent, CausalDirection};

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    data_dir: PathBuf,
    output_path: PathBuf,
    max_chunks: usize,
    num_queries: usize,
    seed: u64,
    checkpoint_dir: Option<PathBuf>,
    checkpoint_interval: usize,
    /// Number of direction detection samples
    num_direction_samples: usize,
    /// Number of asymmetric retrieval queries
    num_asymmetric_queries: usize,
    /// Number of COPA-style questions
    num_copa_questions: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark"),
            output_path: PathBuf::from("docs/causal-realdata-benchmark-results.json"),
            max_chunks: 0, // unlimited
            num_queries: 200,
            seed: 42,
            checkpoint_dir: Some(PathBuf::from("data/hf_benchmark/checkpoints")),
            checkpoint_interval: 1000,
            num_direction_samples: 500,
            num_asymmetric_queries: 200,
            num_copa_questions: 100,
        }
    }
}

fn parse_args() -> Args {
    let mut args = Args::default();
    let mut argv = std::env::args().skip(1);

    while let Some(arg) = argv.next() {
        match arg.as_str() {
            "--data-dir" => {
                args.data_dir = PathBuf::from(argv.next().expect("--data-dir requires a value"));
            }
            "--output" | "-o" => {
                args.output_path = PathBuf::from(argv.next().expect("--output requires a value"));
            }
            "--max-chunks" | "-n" => {
                args.max_chunks = argv
                    .next()
                    .expect("--max-chunks requires a value")
                    .parse()
                    .expect("--max-chunks must be a number");
            }
            "--num-queries" => {
                args.num_queries = argv
                    .next()
                    .expect("--num-queries requires a value")
                    .parse()
                    .expect("--num-queries must be a number");
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
            "--num-copa" => {
                args.num_copa_questions = argv
                    .next()
                    .expect("--num-copa requires a value")
                    .parse()
                    .expect("--num-copa must be a number");
            }
            "--seed" => {
                args.seed = argv
                    .next()
                    .expect("--seed requires a value")
                    .parse()
                    .expect("--seed must be a number");
            }
            "--checkpoint-dir" => {
                args.checkpoint_dir =
                    Some(PathBuf::from(argv.next().expect("--checkpoint-dir requires a value")));
            }
            "--no-checkpoint" => {
                args.checkpoint_dir = None;
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
    eprintln!(
        r#"
E5 Causal Embedder Benchmark with Real HuggingFace Data

USAGE:
    causal-realdata-bench [OPTIONS]

OPTIONS:
    --data-dir <PATH>           Directory with chunks.jsonl and metadata.json
    --output, -o <PATH>         Output path for results JSON
    --max-chunks, -n <NUM>      Maximum chunks to load (0 = unlimited)
    --num-queries <NUM>         Number of general queries
    --num-direction <NUM>       Number of direction detection samples (default: 500)
    --num-asymmetric <NUM>      Number of asymmetric retrieval queries (default: 200)
    --num-copa <NUM>            Number of COPA-style questions (default: 100)
    --seed <NUM>                Random seed for reproducibility
    --checkpoint-dir <PATH>     Directory for embedding checkpoints
    --no-checkpoint             Disable checkpointing
    --help, -h                  Show this help message

NOTE:
    This benchmark requires --features real-embeddings and a CUDA GPU.

EXAMPLE:
    # Full benchmark with real embeddings:
    cargo run --bin causal-realdata-bench --release --features real-embeddings -- \
        --data-dir data/hf_benchmark

    # Quick test with limited data:
    cargo run --bin causal-realdata-bench --release --features real-embeddings -- \
        --data-dir data/hf_benchmark --max-chunks 500 --num-direction 100
"#
    );
}

// ============================================================================
// Result Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalRealDataResults {
    pub timestamp: String,
    pub config: BenchmarkConfig,
    pub dataset_info: DatasetInfo,
    pub embedding_stats: EmbeddingStats,
    pub direction_detection: DirectionDetectionResults,
    pub asymmetric_retrieval: AsymmetricRetrievalResults,
    pub copa_reasoning: CopaReasoningResults,
    pub e5_contribution: E5ContributionAnalysis,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub max_chunks: usize,
    pub num_direction_samples: usize,
    pub num_asymmetric_queries: usize,
    pub num_copa_questions: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub total_chunks: usize,
    pub total_documents: usize,
    pub source_datasets: Vec<String>,
    pub topic_count: usize,
    pub has_asymmetric_e5: bool,
    pub e5_coverage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStats {
    pub total_embeddings: usize,
    pub embedding_time_secs: f64,
    pub embeddings_per_sec: f64,
    pub e5_as_cause_populated: usize,
    pub e5_as_effect_populated: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionDetectionResults {
    pub total_samples: usize,
    pub cause_detected: usize,
    pub effect_detected: usize,
    pub unknown_detected: usize,
    pub detection_rate: f64,
    pub sample_queries: Vec<DirectionSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionSample {
    pub text_snippet: String,
    pub detected_direction: String,
    pub source_dataset: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsymmetricRetrievalResults {
    pub num_queries: usize,
    /// MRR using E5 cause→effect direction (query_as_cause vs doc_as_effect)
    pub mrr_cause_to_effect: f64,
    /// MRR using E5 effect→cause direction (query_as_effect vs doc_as_cause)
    pub mrr_effect_to_cause: f64,
    /// MRR using symmetric E1 (baseline)
    pub mrr_symmetric_e1: f64,
    /// Asymmetry ratio: cause_to_effect / effect_to_cause
    pub asymmetry_ratio: f64,
    /// Improvement over symmetric baseline
    pub improvement_over_symmetric: f64,
    /// Per-direction breakdown
    pub direction_breakdown: DirectionBreakdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionBreakdown {
    pub cause_queries_mrr: f64,
    pub effect_queries_mrr: f64,
    pub neutral_queries_mrr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopaReasoningResults {
    pub num_questions: usize,
    pub accuracy_e5_asymmetric: f64,
    pub accuracy_e1_symmetric: f64,
    pub accuracy_random: f64,
    pub improvement_over_e1: f64,
    pub improvement_over_random: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E5ContributionAnalysis {
    /// MRR with E5 asymmetric scoring
    pub mrr_with_e5: f64,
    /// MRR without E5 (E1 only)
    pub mrr_without_e5: f64,
    /// E5 contribution percentage
    pub e5_contribution_pct: f64,
    /// Asymmetric vs symmetric E5 comparison
    pub asymmetric_vs_symmetric: f64,
    /// Per-source breakdown
    pub per_source_contribution: HashMap<String, f64>,
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let args = parse_args();

    println!("=== E5 Causal Benchmark with Real HuggingFace Data ===");
    println!();

    // Phase 1: Load dataset
    println!("Phase 1: Loading dataset from {:?}", args.data_dir);
    let dataset = match DatasetLoader::new()
        .with_max_chunks(args.max_chunks)
        .load_from_dir(&args.data_dir)
    {
        Ok(ds) => ds,
        Err(e) => {
            eprintln!("Failed to load dataset: {}", e);
            std::process::exit(1);
        }
    };
    println!(
        "  Loaded {} chunks from {} documents",
        dataset.chunks.len(),
        dataset.metadata.total_documents
    );

    // Phase 2: Embed dataset
    println!();
    println!("Phase 2: Embedding dataset with 13 embedders");
    let embed_start = Instant::now();

    let embedder = RealDataEmbedder::new();
    #[cfg(feature = "real-embeddings")]
    let embedded = {
        println!("  Using REAL GPU embeddings");
        match embedder
            .embed_dataset_batched(
                &dataset,
                args.checkpoint_dir.as_deref(),
                args.checkpoint_interval,
            )
            .await
        {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Embedding failed: {}", e);
                std::process::exit(1);
            }
        }
    };
    #[cfg(not(feature = "real-embeddings"))]
    let embedded: EmbeddedDataset = {
        eprintln!("This benchmark requires --features real-embeddings");
        eprintln!("Example: cargo run --release -p context-graph-benchmark --bin causal-realdata-bench --features real-embeddings -- --data-dir data/hf_benchmark");
        let _ = embedder; // suppress unused warning
        let _ = &dataset;
        std::process::exit(1);
    };

    let embed_time = embed_start.elapsed().as_secs_f64();
    println!(
        "  Embedded {} chunks in {:.2}s ({:.1} chunks/sec)",
        embedded.fingerprints.len(),
        embed_time,
        embedded.fingerprints.len() as f64 / embed_time
    );

    // Check E5 coverage
    let e5_as_cause_count = embedded
        .fingerprints
        .values()
        .filter(|fp| !fp.e5_causal_as_cause.is_empty())
        .count();
    let e5_as_effect_count = embedded
        .fingerprints
        .values()
        .filter(|fp| !fp.e5_causal_as_effect.is_empty())
        .count();
    let has_asymmetric_e5 = embedded
        .fingerprints
        .values()
        .any(|fp| fp.has_asymmetric_e5());

    println!(
        "  E5 coverage: as_cause={}, as_effect={}, asymmetric={}",
        e5_as_cause_count, e5_as_effect_count, has_asymmetric_e5
    );

    // Verify E5 cause and effect vectors are actually different (ARCH-15 compliance)
    println!();
    println!("  E5 Vector Asymmetry Verification:");

    let (identical_count, high_similarity_count, total_e5_pairs, similarity_sum) =
        verify_e5_asymmetry(&embedded);

    let vector_distinctness = if total_e5_pairs > 0 {
        100.0 * (1.0 - identical_count as f64 / total_e5_pairs as f64)
    } else {
        0.0
    };

    let avg_cause_effect_similarity = if total_e5_pairs > 0 {
        similarity_sum / total_e5_pairs as f64
    } else {
        0.0
    };

    println!("    Total E5 vector pairs: {}", total_e5_pairs);
    println!("    Identical vectors (sim > 0.999): {}", identical_count);
    println!("    High similarity vectors (sim > 0.95): {}", high_similarity_count);
    println!("    Vector distinctness: {:.1}% different", vector_distinctness);
    println!("    Avg cause-effect similarity: {:.4}", avg_cause_effect_similarity);

    // Fail fast if asymmetry is broken
    if identical_count > total_e5_pairs / 2 {
        eprintln!("\n[CRITICAL] >50% of E5 vectors are identical ({}/{})!",
            identical_count, total_e5_pairs);
        eprintln!("CausalModel.embed_dual() is not producing different embeddings.");
        std::process::exit(1);
    }

    if vector_distinctness < 80.0 && total_e5_pairs > 0 {
        eprintln!("\n[WARNING] E5 vector distinctness is low: {:.1}% (target: >95%)", vector_distinctness);
    }

    // Phase 3: Run causal benchmarks
    println!();
    println!("Phase 3: Running causal benchmarks");

    // 3.1 Direction Detection
    println!("  3.1 Direction Detection...");
    let direction_results = run_direction_detection(&dataset, args.num_direction_samples, args.seed);
    println!(
        "    Detection rate: {:.1}% ({}/{} with direction)",
        direction_results.detection_rate * 100.0,
        direction_results.cause_detected + direction_results.effect_detected,
        direction_results.total_samples
    );

    // 3.2 Asymmetric Retrieval
    println!("  3.2 Asymmetric Retrieval with E5...");
    let asymmetric_results = run_asymmetric_retrieval(&dataset, &embedded, args.num_asymmetric_queries, args.seed);
    println!(
        "    MRR cause→effect: {:.4}, effect→cause: {:.4}",
        asymmetric_results.mrr_cause_to_effect, asymmetric_results.mrr_effect_to_cause
    );
    println!(
        "    Asymmetry ratio: {:.2} (target: ~1.5)",
        asymmetric_results.asymmetry_ratio
    );

    // 3.3 COPA-Style Reasoning
    println!("  3.3 COPA-Style Reasoning...");
    let copa_results = run_copa_reasoning(&dataset, &embedded, args.num_copa_questions, args.seed);
    println!(
        "    E5 asymmetric accuracy: {:.1}%, E1 symmetric: {:.1}%",
        copa_results.accuracy_e5_asymmetric * 100.0,
        copa_results.accuracy_e1_symmetric * 100.0
    );

    // 3.4 E5 Contribution Analysis
    println!("  3.4 E5 Contribution Analysis...");
    let e5_contribution = run_e5_contribution_analysis(&dataset, &embedded, args.num_queries, args.seed);
    println!(
        "    E5 contribution: {:.1}% improvement",
        e5_contribution.e5_contribution_pct * 100.0
    );

    // Phase 4: Compile results
    let source_datasets: Vec<String> = dataset.chunks.iter()
        .filter_map(|c| c.source_dataset.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    // Generate recommendations before moving results into struct
    let recommendations = generate_recommendations(&asymmetric_results, &copa_results, &e5_contribution);

    let results = CausalRealDataResults {
        timestamp: Utc::now().to_rfc3339(),
        config: BenchmarkConfig {
            max_chunks: args.max_chunks,
            num_direction_samples: args.num_direction_samples,
            num_asymmetric_queries: args.num_asymmetric_queries,
            num_copa_questions: args.num_copa_questions,
            seed: args.seed,
        },
        dataset_info: DatasetInfo {
            total_chunks: dataset.chunks.len(),
            total_documents: dataset.metadata.total_documents,
            source_datasets,
            topic_count: embedded.topic_count,
            has_asymmetric_e5,
            e5_coverage: e5_as_cause_count as f64 / embedded.fingerprints.len().max(1) as f64,
        },
        embedding_stats: EmbeddingStats {
            total_embeddings: embedded.fingerprints.len(),
            embedding_time_secs: embed_time,
            embeddings_per_sec: embedded.fingerprints.len() as f64 / embed_time,
            e5_as_cause_populated: e5_as_cause_count,
            e5_as_effect_populated: e5_as_effect_count,
        },
        direction_detection: direction_results,
        asymmetric_retrieval: asymmetric_results,
        copa_reasoning: copa_results,
        e5_contribution,
        recommendations,
    };

    // Phase 5: Save results and generate report
    println!();
    println!("Phase 4: Saving results");
    if let Err(e) = save_results(&results, &args.output_path) {
        eprintln!("Failed to save results: {}", e);
    } else {
        println!("  Results saved to: {:?}", args.output_path);
    }

    // Generate markdown report
    let report_path = args.output_path.with_extension("md");
    if let Err(e) = save_markdown_report(&results, &report_path) {
        eprintln!("Failed to save report: {}", e);
    } else {
        println!("  Report saved to: {:?}", report_path);
    }

    // Print summary
    println!();
    println!("=== Summary ===");
    print_summary(&results);

    // Check targets
    println!();
    println!("=== Target Evaluation ===");
    let success = print_target_evaluation(&results);

    if success {
        println!("\n[SUCCESS] All targets met!");
        std::process::exit(0);
    } else {
        println!("\n[WARNING] Some targets not met. Review results above.");
        std::process::exit(1);
    }
}

// ============================================================================
// Benchmark Functions
// ============================================================================

fn run_direction_detection(
    dataset: &RealDataset,
    num_samples: usize,
    seed: u64,
) -> DirectionDetectionResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Sample random chunks
    let samples: Vec<&ChunkRecord> = dataset
        .chunks
        .choose_multiple(&mut rng, num_samples.min(dataset.chunks.len()))
        .collect();

    let mut cause_count = 0;
    let mut effect_count = 0;
    let mut unknown_count = 0;
    let mut sample_queries = Vec::new();

    for chunk in &samples {
        // Use the chunk text as a pseudo-query to test direction detection
        let direction = detect_causal_query_intent(&chunk.text);

        match direction {
            CausalDirection::Cause => cause_count += 1,
            CausalDirection::Effect => effect_count += 1,
            CausalDirection::Unknown => unknown_count += 1,
        }

        // Save some samples for inspection
        if sample_queries.len() < 20 {
            sample_queries.push(DirectionSample {
                text_snippet: chunk.text.chars().take(100).collect::<String>() + "...",
                detected_direction: format!("{:?}", direction),
                source_dataset: chunk.source_dataset.clone().unwrap_or_default(),
            });
        }
    }

    let total = samples.len();
    let detected = cause_count + effect_count;

    DirectionDetectionResults {
        total_samples: total,
        cause_detected: cause_count,
        effect_detected: effect_count,
        unknown_detected: unknown_count,
        detection_rate: detected as f64 / total.max(1) as f64,
        sample_queries,
    }
}

fn run_asymmetric_retrieval(
    _dataset: &RealDataset,
    embedded: &EmbeddedDataset,
    num_queries: usize,
    seed: u64,
) -> AsymmetricRetrievalResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Get all IDs and fingerprints
    let ids: Vec<Uuid> = embedded.fingerprints.keys().copied().collect();
    if ids.len() < 10 {
        return AsymmetricRetrievalResults {
            num_queries: 0,
            mrr_cause_to_effect: 0.0,
            mrr_effect_to_cause: 0.0,
            mrr_symmetric_e1: 0.0,
            asymmetry_ratio: 1.0,
            improvement_over_symmetric: 0.0,
            direction_breakdown: DirectionBreakdown {
                cause_queries_mrr: 0.0,
                effect_queries_mrr: 0.0,
                neutral_queries_mrr: 0.0,
            },
        };
    }

    // Sample query indices
    let query_ids: Vec<Uuid> = ids.choose_multiple(&mut rng, num_queries.min(ids.len())).copied().collect();

    let mut mrr_c2e_sum = 0.0;
    let mut mrr_e2c_sum = 0.0;
    let mut mrr_e1_sum = 0.0;
    let mut cause_mrr_sum = 0.0;
    let mut effect_mrr_sum = 0.0;
    let mut neutral_mrr_sum = 0.0;
    let mut cause_count = 0usize;
    let mut effect_count = 0usize;
    let mut neutral_count = 0usize;

    for query_id in &query_ids {
        let Some(query_fp) = embedded.fingerprints.get(query_id) else { continue };
        let query_topic = embedded.topic_assignments.get(query_id).copied().unwrap_or(0);

        // Detect direction from query chunk text
        let empty_string = String::new();
        let query_text = embedded.chunk_info.get(query_id)
            .map(|info| &info.topic_hint)
            .unwrap_or(&empty_string);
        let detected_direction = detect_causal_query_intent(query_text);

        // Compute scores for all documents (excluding self)
        let mut scores_c2e: Vec<(Uuid, f32)> = Vec::new();
        let mut scores_e2c: Vec<(Uuid, f32)> = Vec::new();
        let mut scores_e1: Vec<(Uuid, f32)> = Vec::new();

        for (&doc_id, doc_fp) in &embedded.fingerprints {
            if doc_id == *query_id {
                continue;
            }

            // Cause→Effect: query_as_cause vs doc_as_effect
            // This measures: "if query is the cause, how similar is doc as an effect?"
            let sim_c2e = cosine_similarity(query_fp.get_e5_as_cause(), doc_fp.get_e5_as_effect());

            // Effect→Cause: query_as_effect vs doc_as_cause
            // This measures: "if query is the effect, how similar is doc as a cause?"
            let sim_e2c = cosine_similarity(query_fp.get_e5_as_effect(), doc_fp.get_e5_as_cause());

            // Symmetric E1 baseline
            let sim_e1 = cosine_similarity(&query_fp.e1_semantic, &doc_fp.e1_semantic);

            scores_c2e.push((doc_id, sim_c2e));
            scores_e2c.push((doc_id, sim_e2c));
            scores_e1.push((doc_id, sim_e1));
        }

        // Sort by similarity
        scores_c2e.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores_e2c.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores_e1.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Relevant = same topic
        let relevant: std::collections::HashSet<Uuid> = embedded
            .topic_assignments
            .iter()
            .filter(|(id, topic)| **topic == query_topic && **id != *query_id)
            .map(|(id, _)| *id)
            .collect();

        if relevant.is_empty() {
            continue;
        }

        // Compute MRR@10
        let mrr_c2e = compute_mrr(&scores_c2e, &relevant);
        let mrr_e2c = compute_mrr(&scores_e2c, &relevant);
        let mrr_e1 = compute_mrr(&scores_e1, &relevant);

        mrr_c2e_sum += mrr_c2e;
        mrr_e2c_sum += mrr_e2c;
        mrr_e1_sum += mrr_e1;

        // Track per-direction MRR
        match detected_direction {
            CausalDirection::Cause => {
                cause_mrr_sum += mrr_c2e;
                cause_count += 1;
            }
            CausalDirection::Effect => {
                effect_mrr_sum += mrr_e2c;
                effect_count += 1;
            }
            CausalDirection::Unknown => {
                neutral_mrr_sum += (mrr_c2e + mrr_e2c) / 2.0;
                neutral_count += 1;
            }
        }
    }

    let n = query_ids.len() as f64;
    let mrr_c2e = mrr_c2e_sum / n;
    let mrr_e2c = mrr_e2c_sum / n;
    let mrr_e1 = mrr_e1_sum / n;

    AsymmetricRetrievalResults {
        num_queries: query_ids.len(),
        mrr_cause_to_effect: mrr_c2e,
        mrr_effect_to_cause: mrr_e2c,
        mrr_symmetric_e1: mrr_e1,
        asymmetry_ratio: if mrr_e2c > 0.0 { mrr_c2e / mrr_e2c } else { 1.0 },
        improvement_over_symmetric: if mrr_e1 > 0.0 { (mrr_c2e - mrr_e1) / mrr_e1 } else { 0.0 },
        direction_breakdown: DirectionBreakdown {
            cause_queries_mrr: if cause_count > 0 { cause_mrr_sum / cause_count as f64 } else { 0.0 },
            effect_queries_mrr: if effect_count > 0 { effect_mrr_sum / effect_count as f64 } else { 0.0 },
            neutral_queries_mrr: if neutral_count > 0 { neutral_mrr_sum / neutral_count as f64 } else { 0.0 },
        },
    }
}

fn run_copa_reasoning(
    _dataset: &RealDataset,
    embedded: &EmbeddedDataset,
    num_questions: usize,
    seed: u64,
) -> CopaReasoningResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Get chunks by topic for COPA-style questions
    let mut topic_chunks: HashMap<usize, Vec<Uuid>> = HashMap::new();
    for (id, topic) in &embedded.topic_assignments {
        topic_chunks.entry(*topic).or_default().push(*id);
    }

    // Keep only topics with at least 3 chunks
    let valid_topics: Vec<(usize, Vec<Uuid>)> = topic_chunks
        .into_iter()
        .filter(|(_, chunks)| chunks.len() >= 3)
        .collect();

    if valid_topics.is_empty() {
        return CopaReasoningResults {
            num_questions: 0,
            accuracy_e5_asymmetric: 0.0,
            accuracy_e1_symmetric: 0.0,
            accuracy_random: 0.5,
            improvement_over_e1: 0.0,
            improvement_over_random: 0.0,
        };
    }

    let mut correct_e5 = 0;
    let mut correct_e1 = 0;
    let mut total = 0;

    // Generate COPA-style questions: premise + 2 alternatives (1 correct same-topic, 1 distractor)
    for _ in 0..num_questions {
        if total >= num_questions {
            break;
        }

        // Pick a random topic with chunks
        let (topic_idx, topic_chunk_ids) = valid_topics.choose(&mut rng).unwrap();

        // Pick premise and correct answer from same topic
        let mut shuffled_chunks = topic_chunk_ids.clone();
        shuffled_chunks.shuffle(&mut rng);

        if shuffled_chunks.len() < 2 {
            continue;
        }

        let premise_id = shuffled_chunks[0];
        let correct_id = shuffled_chunks[1];

        // Pick a distractor from a different topic
        let distractor_topic = valid_topics
            .iter()
            .filter(|(t, _)| t != topic_idx)
            .choose(&mut rng);

        let distractor_id = match distractor_topic {
            Some((_, chunks)) => *chunks.choose(&mut rng).unwrap(),
            None => continue,
        };

        // Get fingerprints
        let Some(premise_fp) = embedded.fingerprints.get(&premise_id) else { continue };
        let Some(correct_fp) = embedded.fingerprints.get(&correct_id) else { continue };
        let Some(distractor_fp) = embedded.fingerprints.get(&distractor_id) else { continue };

        // E5 asymmetric: use cause→effect similarity
        let sim_e5_correct = cosine_similarity(premise_fp.get_e5_as_cause(), correct_fp.get_e5_as_effect());
        let sim_e5_distractor = cosine_similarity(premise_fp.get_e5_as_cause(), distractor_fp.get_e5_as_effect());

        // E1 symmetric baseline
        let sim_e1_correct = cosine_similarity(&premise_fp.e1_semantic, &correct_fp.e1_semantic);
        let sim_e1_distractor = cosine_similarity(&premise_fp.e1_semantic, &distractor_fp.e1_semantic);

        // Check if correct answer is ranked higher
        if sim_e5_correct > sim_e5_distractor {
            correct_e5 += 1;
        }
        if sim_e1_correct > sim_e1_distractor {
            correct_e1 += 1;
        }

        total += 1;
    }

    let acc_e5 = correct_e5 as f64 / total.max(1) as f64;
    let acc_e1 = correct_e1 as f64 / total.max(1) as f64;

    CopaReasoningResults {
        num_questions: total,
        accuracy_e5_asymmetric: acc_e5,
        accuracy_e1_symmetric: acc_e1,
        accuracy_random: 0.5,
        improvement_over_e1: if acc_e1 > 0.0 { (acc_e5 - acc_e1) / acc_e1 } else { 0.0 },
        improvement_over_random: (acc_e5 - 0.5) / 0.5,
    }
}

fn run_e5_contribution_analysis(
    dataset: &RealDataset,
    embedded: &EmbeddedDataset,
    num_queries: usize,
    seed: u64,
) -> E5ContributionAnalysis {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let ids: Vec<Uuid> = embedded.fingerprints.keys().copied().collect();
    let query_ids: Vec<Uuid> = ids.choose_multiple(&mut rng, num_queries.min(ids.len())).copied().collect();

    let mut mrr_with_e5 = 0.0;
    let mut mrr_without_e5 = 0.0;
    let mut per_source: HashMap<String, (f64, usize)> = HashMap::new();

    for query_id in &query_ids {
        let Some(query_fp) = embedded.fingerprints.get(query_id) else { continue };
        let query_topic = embedded.topic_assignments.get(query_id).copied().unwrap_or(0);
        let query_source = embedded.chunk_info.get(query_id)
            .and_then(|info| dataset.chunks.iter().find(|c| c.title == info.title))
            .and_then(|c| c.source_dataset.clone())
            .unwrap_or_default();

        // Compute scores with E5 asymmetric fusion
        let mut scores_with_e5: Vec<(Uuid, f32)> = Vec::new();
        let mut scores_e1_only: Vec<(Uuid, f32)> = Vec::new();

        for (&doc_id, doc_fp) in &embedded.fingerprints {
            if doc_id == *query_id {
                continue;
            }

            let sim_e1 = cosine_similarity(&query_fp.e1_semantic, &doc_fp.e1_semantic);
            let sim_e5 = cosine_similarity(query_fp.get_e5_as_cause(), doc_fp.get_e5_as_effect());

            // Fusion: 80% E1 + 20% E5 (weighted blend)
            let fused = sim_e1 * 0.8 + sim_e5 * 0.2;

            scores_with_e5.push((doc_id, fused));
            scores_e1_only.push((doc_id, sim_e1));
        }

        scores_with_e5.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores_e1_only.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let relevant: std::collections::HashSet<Uuid> = embedded
            .topic_assignments
            .iter()
            .filter(|(id, topic)| **topic == query_topic && **id != *query_id)
            .map(|(id, _)| *id)
            .collect();

        if relevant.is_empty() {
            continue;
        }

        let mrr_e5 = compute_mrr(&scores_with_e5, &relevant);
        let mrr_e1 = compute_mrr(&scores_e1_only, &relevant);

        mrr_with_e5 += mrr_e5;
        mrr_without_e5 += mrr_e1;

        // Track per-source
        if !query_source.is_empty() {
            let entry = per_source.entry(query_source).or_insert((0.0, 0));
            entry.0 += mrr_e5 - mrr_e1;
            entry.1 += 1;
        }
    }

    let n = query_ids.len() as f64;
    let mrr_e5 = mrr_with_e5 / n;
    let mrr_e1 = mrr_without_e5 / n;

    let per_source_contribution: HashMap<String, f64> = per_source
        .into_iter()
        .map(|(source, (sum, count))| (source, sum / count as f64))
        .collect();

    E5ContributionAnalysis {
        mrr_with_e5: mrr_e5,
        mrr_without_e5: mrr_e1,
        e5_contribution_pct: if mrr_e1 > 0.0 { (mrr_e5 - mrr_e1) / mrr_e1 } else { 0.0 },
        asymmetric_vs_symmetric: mrr_e5 / mrr_e1.max(0.001),
        per_source_contribution,
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Verify E5 asymmetry by comparing cause and effect vectors.
/// Returns (identical_count, high_similarity_count, total_pairs, similarity_sum).
fn verify_e5_asymmetry(embedded: &EmbeddedDataset) -> (usize, usize, usize, f64) {
    let mut identical_count = 0usize;
    let mut high_similarity_count = 0usize;
    let mut total_e5_pairs = 0usize;
    let mut similarity_sum = 0.0f64;

    for fp in embedded.fingerprints.values() {
        let cause = fp.get_e5_as_cause();
        let effect = fp.get_e5_as_effect();

        if cause.is_empty() || effect.is_empty() {
            continue;
        }

        total_e5_pairs += 1;
        let sim = cosine_similarity(cause, effect);
        similarity_sum += sim as f64;

        if sim > 0.999 {
            identical_count += 1;
        }
        if sim > 0.95 {
            high_similarity_count += 1;
        }
    }

    (identical_count, high_similarity_count, total_e5_pairs, similarity_sum)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let norm_product = (norm_a * norm_b).sqrt();
    if norm_product < f32::EPSILON {
        0.0
    } else {
        (dot / norm_product).clamp(-1.0, 1.0)
    }
}

fn compute_mrr(ranked: &[(Uuid, f32)], relevant: &std::collections::HashSet<Uuid>) -> f64 {
    for (i, (id, _)) in ranked.iter().take(10).enumerate() {
        if relevant.contains(id) {
            return 1.0 / (i as f64 + 1.0);
        }
    }
    0.0
}

fn generate_recommendations(
    asymmetric: &AsymmetricRetrievalResults,
    copa: &CopaReasoningResults,
    e5: &E5ContributionAnalysis,
) -> Vec<String> {
    let mut recs = Vec::new();

    if asymmetric.asymmetry_ratio < 1.2 {
        recs.push("Asymmetry ratio below target (1.5). Consider tuning E5 direction modifiers.".to_string());
    }

    if copa.accuracy_e5_asymmetric < 0.7 {
        recs.push("COPA accuracy below 70%. E5 embedder may need domain-specific fine-tuning.".to_string());
    }

    if e5.e5_contribution_pct < 0.05 {
        recs.push("E5 contribution below 5%. Consider increasing E5 weight in fusion formula.".to_string());
    }

    if asymmetric.improvement_over_symmetric < 0.0 {
        recs.push("E5 asymmetric performing worse than E1 symmetric. Check E5 embedding quality.".to_string());
    }

    if recs.is_empty() {
        recs.push("All targets met! E5 causal embedder performing well on real data.".to_string());
    }

    recs
}

fn save_results(results: &CausalRealDataResults, path: &Path) -> std::io::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, results)?;
    Ok(())
}

fn save_markdown_report(results: &CausalRealDataResults, path: &Path) -> std::io::Result<()> {
    let mut f = File::create(path)?;

    writeln!(f, "# E5 Causal Benchmark with Real HuggingFace Data")?;
    writeln!(f)?;
    writeln!(f, "**Generated:** {}", results.timestamp)?;
    writeln!(f)?;

    writeln!(f, "## Configuration")?;
    writeln!(f)?;
    writeln!(f, "| Parameter | Value |")?;
    writeln!(f, "|-----------|-------|")?;
    writeln!(f, "| Total Chunks | {} |", results.dataset_info.total_chunks)?;
    writeln!(f, "| Documents | {} |", results.dataset_info.total_documents)?;
    writeln!(f, "| Topics | {} |", results.dataset_info.topic_count)?;
    writeln!(f, "| E5 Coverage | {:.1}% |", results.dataset_info.e5_coverage * 100.0)?;
    writeln!(f, "| Asymmetric E5 | {} |", results.dataset_info.has_asymmetric_e5)?;
    writeln!(f)?;

    writeln!(f, "## Direction Detection")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value |")?;
    writeln!(f, "|--------|-------|")?;
    writeln!(f, "| Total Samples | {} |", results.direction_detection.total_samples)?;
    writeln!(f, "| Cause Detected | {} |", results.direction_detection.cause_detected)?;
    writeln!(f, "| Effect Detected | {} |", results.direction_detection.effect_detected)?;
    writeln!(f, "| Detection Rate | {:.1}% |", results.direction_detection.detection_rate * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## Asymmetric Retrieval")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target |")?;
    writeln!(f, "|--------|-------|--------|")?;
    writeln!(f, "| MRR Cause→Effect | {:.4} | - |", results.asymmetric_retrieval.mrr_cause_to_effect)?;
    writeln!(f, "| MRR Effect→Cause | {:.4} | - |", results.asymmetric_retrieval.mrr_effect_to_cause)?;
    writeln!(f, "| MRR Symmetric (E1) | {:.4} | - |", results.asymmetric_retrieval.mrr_symmetric_e1)?;
    writeln!(f, "| **Asymmetry Ratio** | **{:.2}** | ~1.5 |", results.asymmetric_retrieval.asymmetry_ratio)?;
    writeln!(f, "| Improvement over E1 | {:.1}% | >0% |", results.asymmetric_retrieval.improvement_over_symmetric * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## COPA-Style Reasoning")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target |")?;
    writeln!(f, "|--------|-------|--------|")?;
    writeln!(f, "| **E5 Asymmetric Accuracy** | **{:.1}%** | >70% |", results.copa_reasoning.accuracy_e5_asymmetric * 100.0)?;
    writeln!(f, "| E1 Symmetric Accuracy | {:.1}% | - |", results.copa_reasoning.accuracy_e1_symmetric * 100.0)?;
    writeln!(f, "| Random Baseline | {:.1}% | - |", results.copa_reasoning.accuracy_random * 100.0)?;
    writeln!(f, "| Improvement over E1 | {:.1}% | >0% |", results.copa_reasoning.improvement_over_e1 * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## E5 Contribution Analysis")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target |")?;
    writeln!(f, "|--------|-------|--------|")?;
    writeln!(f, "| MRR with E5 | {:.4} | - |", results.e5_contribution.mrr_with_e5)?;
    writeln!(f, "| MRR without E5 | {:.4} | - |", results.e5_contribution.mrr_without_e5)?;
    writeln!(f, "| **E5 Contribution** | **{:.1}%** | >5% |", results.e5_contribution.e5_contribution_pct * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## Recommendations")?;
    writeln!(f)?;
    for rec in &results.recommendations {
        writeln!(f, "- {}", rec)?;
    }
    writeln!(f)?;

    Ok(())
}

fn print_summary(results: &CausalRealDataResults) {
    println!("Direction Detection: {:.1}% detection rate", results.direction_detection.detection_rate * 100.0);
    println!("Asymmetry Ratio: {:.2} (target: ~1.5)", results.asymmetric_retrieval.asymmetry_ratio);
    println!("COPA Accuracy: {:.1}% (E5) vs {:.1}% (E1)",
        results.copa_reasoning.accuracy_e5_asymmetric * 100.0,
        results.copa_reasoning.accuracy_e1_symmetric * 100.0);
    println!("E5 Contribution: {:.1}%", results.e5_contribution.e5_contribution_pct * 100.0);
}

fn print_target_evaluation(results: &CausalRealDataResults) -> bool {
    let mut all_pass = true;

    // Asymmetry ratio target: ~1.5
    let asymmetry_pass = results.asymmetric_retrieval.asymmetry_ratio >= 1.2
        && results.asymmetric_retrieval.asymmetry_ratio <= 2.0;
    println!("  Asymmetry Ratio: {:.2} (target: 1.2-2.0)", results.asymmetric_retrieval.asymmetry_ratio);
    println!("    Status: {}", if asymmetry_pass { "PASS" } else { "FAIL" });
    all_pass &= asymmetry_pass;

    // COPA accuracy target: >70%
    let copa_pass = results.copa_reasoning.accuracy_e5_asymmetric >= 0.7;
    println!("  COPA Accuracy: {:.1}% (target: >70%)", results.copa_reasoning.accuracy_e5_asymmetric * 100.0);
    println!("    Status: {}", if copa_pass { "PASS" } else { "FAIL" });
    all_pass &= copa_pass;

    // E5 contribution target: >5%
    let e5_pass = results.e5_contribution.e5_contribution_pct >= 0.05;
    println!("  E5 Contribution: {:.1}% (target: >5%)", results.e5_contribution.e5_contribution_pct * 100.0);
    println!("    Status: {}", if e5_pass { "PASS" } else { "FAIL" });
    all_pass &= e5_pass;

    // Improvement over E1
    let improvement_pass = results.asymmetric_retrieval.improvement_over_symmetric >= 0.0;
    println!("  E5 vs E1 Improvement: {:.1}% (target: >0%)", results.asymmetric_retrieval.improvement_over_symmetric * 100.0);
    println!("    Status: {}", if improvement_pass { "PASS" } else { "FAIL" });
    all_pass &= improvement_pass;

    all_pass
}
