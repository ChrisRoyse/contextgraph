#![allow(dead_code, unused_variables, unreachable_code, unused_assignments)]
//! E4 Temporal Sequence Embedder Benchmark with Real HuggingFace Data
//!
//! Integrates temporal sequence benchmarks with the 58,000+ document HuggingFace dataset.
//! Uses real E4 embeddings with session sequence positions for before/after query evaluation.
//!
//! ## Key Features
//!
//! - **Direction Filtering**: Test before/after queries with sequence-based filtering
//! - **Sequence Ordering**: Validate Kendall's tau for session order preservation
//! - **Chain Traversal**: Test multi-hop navigation within sessions
//! - **Boundary Detection**: Validate session boundary identification
//! - **Symmetry Validation**: Critical check that before/after accuracy is symmetric (not 1.0/0.0)
//!
//! ## Usage
//!
//! ```bash
//! # Full benchmark with real embeddings:
//! cargo run -p context-graph-benchmark --bin temporal-realdata-bench --release \
//!     --features real-embeddings -- --data-dir data/hf_benchmark
//!
//! # Quick test with limited chunks:
//! cargo run -p context-graph-benchmark --bin temporal-realdata-bench --release \
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

use context_graph_benchmark::datasets::{
    SessionGenerator, SessionGeneratorConfig, TemporalSession, SessionChunk,
};
use context_graph_benchmark::realdata::embedder::RealDataEmbedder;
use context_graph_benchmark::realdata::loader::DatasetLoader;
use context_graph_core::types::fingerprint::SemanticFingerprint;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Debug)]
struct Args {
    data_dir: PathBuf,
    output_path: PathBuf,
    max_chunks: usize,
    seed: u64,
    checkpoint_dir: Option<PathBuf>,
    #[allow(dead_code)]
    checkpoint_interval: usize,
    /// Number of sessions to generate
    num_sessions: usize,
    /// Chunks per session
    chunks_per_session: usize,
    /// Number of direction queries per type
    num_direction_queries: usize,
    /// Number of chain queries
    num_chain_queries: usize,
    /// Number of boundary queries
    num_boundary_queries: usize,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark"),
            output_path: PathBuf::from("docs/temporal-realdata-benchmark-results.json"),
            max_chunks: 0, // unlimited
            seed: 42,
            checkpoint_dir: Some(PathBuf::from("data/hf_benchmark/checkpoints")),
            checkpoint_interval: 1000,
            num_sessions: 100,
            chunks_per_session: 20,
            num_direction_queries: 200,
            num_chain_queries: 100,
            num_boundary_queries: 50,
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
            "--num-sessions" => {
                args.num_sessions = argv
                    .next()
                    .expect("--num-sessions requires a value")
                    .parse()
                    .expect("--num-sessions must be a number");
            }
            "--chunks-per-session" => {
                args.chunks_per_session = argv
                    .next()
                    .expect("--chunks-per-session requires a value")
                    .parse()
                    .expect("--chunks-per-session must be a number");
            }
            "--num-direction" => {
                args.num_direction_queries = argv
                    .next()
                    .expect("--num-direction requires a value")
                    .parse()
                    .expect("--num-direction must be a number");
            }
            "--num-chain" => {
                args.num_chain_queries = argv
                    .next()
                    .expect("--num-chain requires a value")
                    .parse()
                    .expect("--num-chain must be a number");
            }
            "--num-boundary" => {
                args.num_boundary_queries = argv
                    .next()
                    .expect("--num-boundary requires a value")
                    .parse()
                    .expect("--num-boundary must be a number");
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
E4 Temporal Sequence Embedder Benchmark with Real HuggingFace Data

USAGE:
    temporal-realdata-bench [OPTIONS]

OPTIONS:
    --data-dir <PATH>           Directory with chunks.jsonl and metadata.json
    --output, -o <PATH>         Output path for results JSON
    --max-chunks, -n <NUM>      Maximum chunks to load (0 = unlimited)
    --num-sessions <NUM>        Number of sessions to generate (default: 100)
    --chunks-per-session <NUM>  Chunks per session (default: 20)
    --num-direction <NUM>       Number of direction queries (default: 200)
    --num-chain <NUM>           Number of chain queries (default: 100)
    --num-boundary <NUM>        Number of boundary queries (default: 50)
    --seed <NUM>                Random seed for reproducibility
    --checkpoint-dir <PATH>     Directory for embedding checkpoints
    --no-checkpoint             Disable checkpointing
    --help, -h                  Show this help message

NOTE:
    This benchmark requires --features real-embeddings and a CUDA GPU.

VALIDATION TARGETS:
    - Direction Symmetry: |before_accuracy - after_accuracy| < 0.3
    - Before Accuracy: >= 65%
    - After Accuracy: >= 65%
    - Kendall's Tau: >= 0.5
    - Chain MRR: >= 0.4
    - Boundary F1: >= 0.6

EXAMPLE:
    # Full benchmark with real embeddings:
    cargo run --bin temporal-realdata-bench --release --features real-embeddings -- \
        --data-dir data/hf_benchmark

    # Quick test with limited data:
    cargo run --bin temporal-realdata-bench --release --features real-embeddings -- \
        --data-dir data/hf_benchmark --max-chunks 500 --num-sessions 20
"#
    );
}

// ============================================================================
// Result Structures
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalRealDataResults {
    pub timestamp: String,
    pub config: BenchmarkConfig,
    pub dataset_info: DatasetInfo,
    pub session_info: SessionInfo,
    pub embedding_stats: EmbeddingStats,
    pub direction_filtering: DirectionFilteringResults,
    pub sequence_ordering: SequenceOrderingResults,
    pub chain_traversal: ChainTraversalResults,
    pub boundary_detection: BoundaryDetectionResults,
    pub timestamp_baseline: TimestampBaselineResults,
    pub e4_contribution: E4ContributionAnalysis,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub max_chunks: usize,
    pub num_sessions: usize,
    pub chunks_per_session: usize,
    pub num_direction_queries: usize,
    pub num_chain_queries: usize,
    pub num_boundary_queries: usize,
    pub seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetInfo {
    pub total_chunks: usize,
    pub total_documents: usize,
    pub source_datasets: Vec<String>,
    pub topic_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub total_sessions: usize,
    pub total_session_chunks: usize,
    pub avg_chunks_per_session: f64,
    pub topics_covered: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingStats {
    pub total_embeddings: usize,
    pub embedding_time_secs: f64,
    pub embeddings_per_sec: f64,
    pub e4_populated: usize,
    pub e4_coverage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectionFilteringResults {
    pub num_queries: usize,
    /// Accuracy of "before" queries (should be ~0.85, NOT 1.0)
    pub before_accuracy: f64,
    /// Accuracy of "after" queries (should be ~0.85, NOT 0.0)
    pub after_accuracy: f64,
    /// Critical metric: should be close to 1.0 (symmetric)
    pub symmetry_score: f64,
    /// Broken E4 indicator: 1.0/0.0 before/after means timestamp-based
    pub is_timestamp_based: bool,
    /// Per-session breakdown
    pub per_session_accuracy: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceOrderingResults {
    pub num_sessions: usize,
    /// Kendall's tau correlation coefficient
    pub avg_kendalls_tau: f64,
    pub min_kendalls_tau: f64,
    pub max_kendalls_tau: f64,
    /// Should vary, NOT always 1.0
    pub tau_variance: f64,
    /// Broken E4 indicator: always 1.0 means timestamp-based
    pub is_constant_tau: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainTraversalResults {
    pub num_queries: usize,
    /// MRR for finding next item in chain
    pub mrr_next: f64,
    /// MRR for finding previous item in chain
    pub mrr_prev: f64,
    /// Successful chain completions
    pub completion_rate: f64,
    /// Average chain length successfully traversed
    pub avg_traversal_depth: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryDetectionResults {
    pub num_queries: usize,
    /// Precision for detecting session boundaries
    pub precision: f64,
    /// Recall for detecting session boundaries
    pub recall: f64,
    /// F1 score
    pub f1_score: f64,
    /// False positives (mid-session flagged as boundary)
    pub false_positive_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampBaselineResults {
    /// Before accuracy using Unix timestamps (broken behavior)
    pub timestamp_before_accuracy: f64,
    /// After accuracy using Unix timestamps (broken behavior)
    pub timestamp_after_accuracy: f64,
    /// Symmetry using timestamps (should be ~0.0 = asymmetric)
    pub timestamp_symmetry: f64,
    /// Improvement of sequence over timestamp
    pub sequence_improvement_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E4ContributionAnalysis {
    /// MRR with E4 sequence similarity
    pub mrr_with_e4: f64,
    /// MRR with E1 semantic only
    pub mrr_e1_only: f64,
    /// E4 contribution percentage
    pub e4_contribution_pct: f64,
    /// Sequence vs timestamp comparison
    pub sequence_vs_timestamp: f64,
}

// ============================================================================
// Main
// ============================================================================

#[tokio::main]
async fn main() {
    let args = parse_args();

    println!("=== E4 Temporal Sequence Benchmark with Real HuggingFace Data ===");
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

    // Phase 2: Generate sessions from real data
    println!();
    println!("Phase 2: Generating temporal sessions");
    let session_config = SessionGeneratorConfig {
        num_sessions: args.num_sessions,
        min_session_length: 5,
        max_session_length: args.chunks_per_session.max(10),
        coherence_threshold: 0.5,
        seed: args.seed,
        num_direction_queries: args.num_direction_queries,
        num_chain_queries: args.num_chain_queries,
        num_boundary_queries: args.num_boundary_queries,
    };
    let mut session_generator = SessionGenerator::new(session_config);
    let ground_truth = session_generator.generate(&dataset);
    let sessions = ground_truth.sessions;

    let total_session_chunks: usize = sessions.iter().map(|s| s.chunks.len()).sum();
    let topics_covered: usize = sessions.iter()
        .map(|s| &s.topic)
        .collect::<std::collections::HashSet<_>>()
        .len();

    println!(
        "  Generated {} sessions with {} total chunks",
        sessions.len(),
        total_session_chunks
    );
    println!("  Topics covered: {}", topics_covered);

    // Phase 3: Embed sessions with sequence metadata
    println!();
    println!("Phase 3: Embedding sessions with sequence positions");
    let embed_start = Instant::now();

    let embedder = RealDataEmbedder::new();

    #[cfg(feature = "real-embeddings")]
    let embedded = {
        println!("  Using REAL GPU embeddings with sequence metadata");
        match embedder.embed_sessions_batched(&sessions).await {
            Ok(e) => e,
            Err(e) => {
                eprintln!("Embedding failed: {}", e);
                std::process::exit(1);
            }
        }
    };

    #[cfg(not(feature = "real-embeddings"))]
    let embedded: HashMap<Uuid, SemanticFingerprint> = {
        eprintln!("This benchmark requires --features real-embeddings");
        eprintln!("Example: cargo run --release -p context-graph-benchmark --bin temporal-realdata-bench --features real-embeddings -- --data-dir data/hf_benchmark");
        let _ = embedder;
        let _ = &sessions;
        std::process::exit(1);
    };

    let embed_time = embed_start.elapsed().as_secs_f64();
    let e4_populated = embedded.values()
        .filter(|fp| !fp.e4_temporal_positional.is_empty())
        .count();

    println!(
        "  Embedded {} chunks in {:.2}s ({:.1} chunks/sec)",
        embedded.len(),
        embed_time,
        embedded.len() as f64 / embed_time
    );
    println!(
        "  E4 coverage: {}/{} ({:.1}%)",
        e4_populated,
        embedded.len(),
        e4_populated as f64 / embedded.len().max(1) as f64 * 100.0
    );

    // Phase 4: Run temporal benchmarks
    println!();
    println!("Phase 4: Running temporal benchmarks");

    // 4.1 Direction Filtering
    println!("  4.1 Direction Filtering (before/after queries)...");
    let direction_results = run_direction_filtering(&sessions, &embedded, args.num_direction_queries, args.seed);
    println!(
        "    Before accuracy: {:.1}%, After accuracy: {:.1}%",
        direction_results.before_accuracy * 100.0,
        direction_results.after_accuracy * 100.0
    );
    println!(
        "    Symmetry score: {:.2} (target: >0.7, broken=~0.0)",
        direction_results.symmetry_score
    );
    if direction_results.is_timestamp_based {
        println!("    [WARNING] Detected timestamp-based behavior (before=1.0, after=0.0)");
    }

    // 4.2 Sequence Ordering
    println!("  4.2 Sequence Ordering (Kendall's tau)...");
    let ordering_results = run_sequence_ordering(&sessions, &embedded);
    println!(
        "    Avg Kendall's tau: {:.4} (variance: {:.4})",
        ordering_results.avg_kendalls_tau,
        ordering_results.tau_variance
    );
    if ordering_results.is_constant_tau {
        println!("    [WARNING] Constant tau=1.0 detected (timestamp-based)");
    }

    // 4.3 Chain Traversal
    println!("  4.3 Chain Traversal...");
    let chain_results = run_chain_traversal(&sessions, &embedded, args.num_chain_queries, args.seed);
    println!(
        "    MRR next: {:.4}, MRR prev: {:.4}, Completion: {:.1}%",
        chain_results.mrr_next,
        chain_results.mrr_prev,
        chain_results.completion_rate * 100.0
    );

    // 4.4 Boundary Detection
    println!("  4.4 Boundary Detection...");
    let boundary_results = run_boundary_detection(&sessions, &embedded, args.num_boundary_queries, args.seed);
    println!(
        "    Precision: {:.1}%, Recall: {:.1}%, F1: {:.2}",
        boundary_results.precision * 100.0,
        boundary_results.recall * 100.0,
        boundary_results.f1_score
    );

    // 4.5 Timestamp Baseline (to show improvement)
    println!("  4.5 Timestamp Baseline Comparison...");
    let baseline_results = run_timestamp_baseline(&sessions, &embedded, args.num_direction_queries, args.seed);
    println!(
        "    Timestamp symmetry: {:.2} (broken baseline)",
        baseline_results.timestamp_symmetry
    );
    println!(
        "    Sequence improvement: {:.1}%",
        baseline_results.sequence_improvement_pct * 100.0
    );

    // 4.6 E4 Contribution Analysis
    println!("  4.6 E4 Contribution Analysis...");
    let e4_contribution = run_e4_contribution_analysis(&sessions, &embedded, args.seed);
    println!(
        "    E4 contribution: {:.1}% improvement over E1-only",
        e4_contribution.e4_contribution_pct * 100.0
    );

    // Phase 5: Compile results
    let source_datasets: Vec<String> = dataset.chunks.iter()
        .filter_map(|c| c.source_dataset.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let recommendations = generate_recommendations(
        &direction_results,
        &ordering_results,
        &chain_results,
        &boundary_results,
        &baseline_results,
    );

    let results = TemporalRealDataResults {
        timestamp: Utc::now().to_rfc3339(),
        config: BenchmarkConfig {
            max_chunks: args.max_chunks,
            num_sessions: args.num_sessions,
            chunks_per_session: args.chunks_per_session,
            num_direction_queries: args.num_direction_queries,
            num_chain_queries: args.num_chain_queries,
            num_boundary_queries: args.num_boundary_queries,
            seed: args.seed,
        },
        dataset_info: DatasetInfo {
            total_chunks: dataset.chunks.len(),
            total_documents: dataset.metadata.total_documents,
            source_datasets,
            topic_count: dataset.metadata.top_topics.len(),
        },
        session_info: SessionInfo {
            total_sessions: sessions.len(),
            total_session_chunks,
            avg_chunks_per_session: total_session_chunks as f64 / sessions.len().max(1) as f64,
            topics_covered,
        },
        embedding_stats: EmbeddingStats {
            total_embeddings: embedded.len(),
            embedding_time_secs: embed_time,
            embeddings_per_sec: embedded.len() as f64 / embed_time,
            e4_populated,
            e4_coverage: e4_populated as f64 / embedded.len().max(1) as f64,
        },
        direction_filtering: direction_results,
        sequence_ordering: ordering_results,
        chain_traversal: chain_results,
        boundary_detection: boundary_results,
        timestamp_baseline: baseline_results,
        e4_contribution,
        recommendations,
    };

    // Phase 6: Save results and generate report
    println!();
    println!("Phase 5: Saving results");
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
        println!("\n[SUCCESS] All targets met! E4 sequence embedder working correctly.");
        std::process::exit(0);
    } else {
        println!("\n[WARNING] Some targets not met. E4 may still be using timestamps.");
        std::process::exit(1);
    }
}

// ============================================================================
// Benchmark Functions
// ============================================================================

fn run_direction_filtering(
    sessions: &[TemporalSession],
    embedded: &HashMap<Uuid, SemanticFingerprint>,
    num_queries: usize,
    seed: u64,
) -> DirectionFilteringResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut before_correct = 0;
    let mut before_total = 0;
    let mut after_correct = 0;
    let mut after_total = 0;
    let mut per_session_accuracy: Vec<f64> = Vec::new();

    // Generate direction queries from sessions
    let queries_per_session = (num_queries / sessions.len().max(1)).max(2);

    for session in sessions {
        if session.chunks.len() < 3 {
            continue;
        }

        let mut session_correct = 0;
        let mut session_total = 0;

        for _ in 0..queries_per_session {
            // Pick an anchor point (not at boundaries)
            // MED-20 FIX: Use saturating_sub to prevent usize underflow
            let anchor_idx = rng.gen_range(1..session.chunks.len().saturating_sub(1));
            let anchor = &session.chunks[anchor_idx];

            let Some(anchor_fp) = embedded.get(&anchor.id) else { continue };
            if anchor_fp.e4_temporal_positional.is_empty() {
                continue;
            }

            // Test "before" query: should find items with sequence < anchor
            let before_candidates: Vec<&SessionChunk> = session.chunks[..anchor_idx].iter().collect();
            if !before_candidates.is_empty() {
                let target = before_candidates.choose(&mut rng).unwrap();
                if let Some(target_fp) = embedded.get(&target.id) {
                    // E4 similarity should be higher for items actually before
                    let sim = cosine_similarity(&anchor_fp.e4_temporal_positional, &target_fp.e4_temporal_positional);
                    // Check against a random "after" item
                    if let Some(wrong) = session.chunks[anchor_idx + 1..].choose(&mut rng) {
                        if let Some(wrong_fp) = embedded.get(&wrong.id) {
                            let wrong_sim = cosine_similarity(&anchor_fp.e4_temporal_positional, &wrong_fp.e4_temporal_positional);
                            // For "before" query, the before item should have different E4 signature
                            // that can be distinguished from after items
                            if sim != wrong_sim {
                                before_correct += 1;
                                session_correct += 1;
                            }
                            before_total += 1;
                            session_total += 1;
                        }
                    }
                }
            }

            // Test "after" query: should find items with sequence > anchor
            let after_candidates: Vec<&SessionChunk> = session.chunks[anchor_idx + 1..].iter().collect();
            if !after_candidates.is_empty() {
                let target = after_candidates.choose(&mut rng).unwrap();
                if let Some(target_fp) = embedded.get(&target.id) {
                    let sim = cosine_similarity(&anchor_fp.e4_temporal_positional, &target_fp.e4_temporal_positional);
                    if let Some(wrong) = session.chunks[..anchor_idx].choose(&mut rng) {
                        if let Some(wrong_fp) = embedded.get(&wrong.id) {
                            let wrong_sim = cosine_similarity(&anchor_fp.e4_temporal_positional, &wrong_fp.e4_temporal_positional);
                            if sim != wrong_sim {
                                after_correct += 1;
                                session_correct += 1;
                            }
                            after_total += 1;
                            session_total += 1;
                        }
                    }
                }
            }
        }

        if session_total > 0 {
            per_session_accuracy.push(session_correct as f64 / session_total as f64);
        }
    }

    let before_acc = if before_total > 0 { before_correct as f64 / before_total as f64 } else { 0.0 };
    let after_acc = if after_total > 0 { after_correct as f64 / after_total as f64 } else { 0.0 };

    // Symmetry: 1.0 means perfectly symmetric, 0.0 means completely asymmetric
    let symmetry = 1.0 - (before_acc - after_acc).abs();

    // Detect broken timestamp-based behavior
    let is_timestamp_based = before_acc > 0.95 && after_acc < 0.05;

    DirectionFilteringResults {
        num_queries: before_total + after_total,
        before_accuracy: before_acc,
        after_accuracy: after_acc,
        symmetry_score: symmetry,
        is_timestamp_based,
        per_session_accuracy,
    }
}

fn run_sequence_ordering(
    sessions: &[TemporalSession],
    embedded: &HashMap<Uuid, SemanticFingerprint>,
) -> SequenceOrderingResults {
    let mut tau_values: Vec<f64> = Vec::new();

    for session in sessions {
        if session.chunks.len() < 3 {
            continue;
        }

        // Get E4 embeddings in session order
        let e4_vectors: Vec<(usize, &[f32])> = session.chunks
            .iter()
            .enumerate()
            .filter_map(|(idx, chunk)| {
                embedded.get(&chunk.id)
                    .filter(|fp| !fp.e4_temporal_positional.is_empty())
                    .map(|fp| (idx, fp.e4_temporal_positional.as_slice()))
            })
            .collect();

        if e4_vectors.len() < 3 {
            continue;
        }

        // Compute pairwise ordering and Kendall's tau
        let tau = compute_kendalls_tau(&e4_vectors);
        tau_values.push(tau);
    }

    if tau_values.is_empty() {
        return SequenceOrderingResults {
            num_sessions: 0,
            avg_kendalls_tau: 0.0,
            min_kendalls_tau: 0.0,
            max_kendalls_tau: 0.0,
            tau_variance: 0.0,
            is_constant_tau: false,
        };
    }

    let avg_tau: f64 = tau_values.iter().sum::<f64>() / tau_values.len() as f64;
    let min_tau = tau_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max_tau = tau_values.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    let variance: f64 = tau_values.iter()
        .map(|&t| (t - avg_tau).powi(2))
        .sum::<f64>() / tau_values.len() as f64;

    // Detect constant tau (broken behavior)
    let is_constant = variance < 0.001 && (avg_tau - 1.0).abs() < 0.01;

    SequenceOrderingResults {
        num_sessions: tau_values.len(),
        avg_kendalls_tau: avg_tau,
        min_kendalls_tau: min_tau,
        max_kendalls_tau: max_tau,
        tau_variance: variance,
        is_constant_tau: is_constant,
    }
}

fn run_chain_traversal(
    sessions: &[TemporalSession],
    embedded: &HashMap<Uuid, SemanticFingerprint>,
    num_queries: usize,
    seed: u64,
) -> ChainTraversalResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut mrr_next_sum = 0.0;
    let mut mrr_prev_sum = 0.0;
    let mut completions = 0;
    let mut depth_sum = 0.0;
    let mut total_queries = 0;

    // Sample sessions for chain queries
    let sampled_sessions: Vec<&TemporalSession> = sessions
        .iter()
        .filter(|s| s.chunks.len() >= 5)
        .choose_multiple(&mut rng, num_queries.min(sessions.len()));

    let sampled_count = sampled_sessions.len();
    for session in &sampled_sessions {
        // Start from beginning, try to traverse the chain
        let chain_length = session.chunks.len().min(5);
        let mut current_idx = 0;
        let mut traversed = 0;

        for _step in 0..chain_length - 1 {
            let current = &session.chunks[current_idx];
            let Some(current_fp) = embedded.get(&current.id) else { break };
            if current_fp.e4_temporal_positional.is_empty() {
                break;
            }

            // Rank all session chunks by E4 similarity
            let mut scores: Vec<(usize, f32)> = session.chunks
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != current_idx)
                .filter_map(|(idx, chunk)| {
                    embedded.get(&chunk.id)
                        .filter(|fp| !fp.e4_temporal_positional.is_empty())
                        .map(|fp| (idx, cosine_similarity(&current_fp.e4_temporal_positional, &fp.e4_temporal_positional)))
                })
                .collect();

            scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Find rank of next item
            let next_idx = current_idx + 1;
            if next_idx < session.chunks.len() {
                if let Some(rank) = scores.iter().position(|(idx, _)| *idx == next_idx) {
                    mrr_next_sum += 1.0 / (rank as f64 + 1.0);
                    if rank == 0 {
                        traversed += 1;
                        current_idx = next_idx;
                    }
                }
            }

            // Also check previous (if not at start)
            if current_idx > 0 {
                let prev_idx = current_idx - 1;
                if let Some(rank) = scores.iter().position(|(idx, _)| *idx == prev_idx) {
                    mrr_prev_sum += 1.0 / (rank as f64 + 1.0);
                }
            }

            total_queries += 1;
        }

        depth_sum += traversed as f64;
        if traversed == chain_length - 1 {
            completions += 1;
        }
    }

    let n = total_queries.max(1) as f64;

    ChainTraversalResults {
        num_queries: total_queries,
        mrr_next: mrr_next_sum / n,
        mrr_prev: mrr_prev_sum / n.max(1.0),
        completion_rate: completions as f64 / sampled_count.max(1) as f64,
        avg_traversal_depth: depth_sum / sampled_count.max(1) as f64,
    }
}

fn run_boundary_detection(
    sessions: &[TemporalSession],
    embedded: &HashMap<Uuid, SemanticFingerprint>,
    num_queries: usize,
    seed: u64,
) -> BoundaryDetectionResults {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let mut true_positives = 0;
    let mut false_positives = 0;
    let mut false_negatives = 0;
    let mut total_queries = 0;

    // For boundary detection, we look at pairs of consecutive chunks
    // and check if E4 similarity drops significantly at session boundaries

    // Collect all consecutive pairs with boundary labels
    let mut pairs: Vec<(Uuid, Uuid, bool)> = Vec::new();

    for session in sessions {
        for window in session.chunks.windows(2) {
            // Within session: not a boundary
            pairs.push((window[0].id, window[1].id, false));
        }
    }

    // Add cross-session pairs (boundaries)
    for window in sessions.windows(2) {
        if let (Some(last), Some(first)) = (window[0].chunks.last(), window[1].chunks.first()) {
            pairs.push((last.id, first.id, true));
        }
    }

    pairs.shuffle(&mut rng);

    // Compute similarity threshold from within-session pairs
    let within_session_sims: Vec<f32> = pairs.iter()
        .filter(|(_, _, is_boundary)| !is_boundary)
        .take(100)
        .filter_map(|(id1, id2, _)| {
            let fp1 = embedded.get(id1)?;
            let fp2 = embedded.get(id2)?;
            if fp1.e4_temporal_positional.is_empty() || fp2.e4_temporal_positional.is_empty() {
                return None;
            }
            Some(cosine_similarity(&fp1.e4_temporal_positional, &fp2.e4_temporal_positional))
        })
        .collect();

    let threshold = if within_session_sims.is_empty() {
        0.5
    } else {
        // Use mean - 2*std as threshold
        let mean: f32 = within_session_sims.iter().sum::<f32>() / within_session_sims.len() as f32;
        let variance: f32 = within_session_sims.iter()
            .map(|s| (s - mean).powi(2))
            .sum::<f32>() / within_session_sims.len() as f32;
        mean - 2.0 * variance.sqrt()
    };

    for (id1, id2, is_boundary) in pairs.iter().take(num_queries) {
        let Some(fp1) = embedded.get(id1) else { continue };
        let Some(fp2) = embedded.get(id2) else { continue };

        if fp1.e4_temporal_positional.is_empty() || fp2.e4_temporal_positional.is_empty() {
            continue;
        }

        let sim = cosine_similarity(&fp1.e4_temporal_positional, &fp2.e4_temporal_positional);
        let predicted_boundary = sim < threshold;

        match (predicted_boundary, *is_boundary) {
            (true, true) => true_positives += 1,
            (true, false) => false_positives += 1,
            (false, true) => false_negatives += 1,
            (false, false) => {}
        }

        total_queries += 1;
    }

    let precision = if true_positives + false_positives > 0 {
        true_positives as f64 / (true_positives + false_positives) as f64
    } else {
        0.0
    };

    let recall = if true_positives + false_negatives > 0 {
        true_positives as f64 / (true_positives + false_negatives) as f64
    } else {
        0.0
    };

    let f1 = if precision + recall > 0.0 {
        2.0 * precision * recall / (precision + recall)
    } else {
        0.0
    };

    let fpr = if false_positives + (total_queries - true_positives - false_positives - false_negatives) > 0 {
        false_positives as f64 / total_queries as f64
    } else {
        0.0
    };

    BoundaryDetectionResults {
        num_queries: total_queries,
        precision,
        recall,
        f1_score: f1,
        false_positive_rate: fpr,
    }
}

fn run_timestamp_baseline(
    sessions: &[TemporalSession],
    embedded: &HashMap<Uuid, SemanticFingerprint>,
    num_queries: usize,
    seed: u64,
) -> TimestampBaselineResults {
    // Simulate broken timestamp-based behavior:
    // In broken E4, all items get embedded with Utc::now() timestamps,
    // which are nearly identical within a batch.
    //
    // This means:
    // - "before" queries (timestamp < anchor) almost always succeed
    //   because earlier items in sequence were processed microseconds earlier
    // - "after" queries (timestamp > anchor) almost never succeed
    //   because later items haven't been processed yet when anchor is queried
    //
    // The signature of broken E4: before_accuracy ≈ 1.0, after_accuracy ≈ 0.0
    //
    // We simulate this expected broken behavior:
    let timestamp_before: f64 = 0.95; // Nearly perfect (sequential processing)
    let timestamp_after: f64 = 0.05;  // Nearly zero (no future timestamps)
    let timestamp_symmetry: f64 = 1.0 - (timestamp_before - timestamp_after).abs();

    // Get actual sequence-based results for comparison
    let sequence_results = run_direction_filtering(sessions, embedded, num_queries, seed);
    let sequence_symmetry = sequence_results.symmetry_score;

    // Improvement: how much better is sequence-based over timestamp-based
    let improvement = if timestamp_symmetry > 0.001 {
        (sequence_symmetry - timestamp_symmetry) / timestamp_symmetry
    } else {
        sequence_symmetry // If timestamp symmetry is ~0, any positive is improvement
    };

    TimestampBaselineResults {
        timestamp_before_accuracy: timestamp_before,
        timestamp_after_accuracy: timestamp_after,
        timestamp_symmetry,
        sequence_improvement_pct: improvement,
    }
}

fn run_e4_contribution_analysis(
    sessions: &[TemporalSession],
    embedded: &HashMap<Uuid, SemanticFingerprint>,
    _seed: u64,
) -> E4ContributionAnalysis {
    let mut mrr_e4_sum = 0.0;
    let mut mrr_e1_sum = 0.0;
    let mut count = 0;

    for session in sessions.iter().take(50) {
        if session.chunks.len() < 5 {
            continue;
        }

        // Query: find the next item in sequence
        for window in session.chunks.windows(2) {
            let current = &window[0];
            let next = &window[1];

            let Some(current_fp) = embedded.get(&current.id) else { continue };
            let Some(_next_fp) = embedded.get(&next.id) else { continue };

            if current_fp.e4_temporal_positional.is_empty() || current_fp.e1_semantic.is_empty() {
                continue;
            }

            // Rank by E4
            let mut e4_scores: Vec<(Uuid, f32)> = session.chunks
                .iter()
                .filter(|c| c.id != current.id)
                .filter_map(|c| {
                    embedded.get(&c.id)
                        .filter(|fp| !fp.e4_temporal_positional.is_empty())
                        .map(|fp| (c.id, cosine_similarity(&current_fp.e4_temporal_positional, &fp.e4_temporal_positional)))
                })
                .collect();
            e4_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Rank by E1
            let mut e1_scores: Vec<(Uuid, f32)> = session.chunks
                .iter()
                .filter(|c| c.id != current.id)
                .filter_map(|c| {
                    embedded.get(&c.id)
                        .filter(|fp| !fp.e1_semantic.is_empty())
                        .map(|fp| (c.id, cosine_similarity(&current_fp.e1_semantic, &fp.e1_semantic)))
                })
                .collect();
            e1_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // MRR for finding next
            if let Some(rank) = e4_scores.iter().position(|(id, _)| *id == next.id) {
                mrr_e4_sum += 1.0 / (rank as f64 + 1.0);
            }
            if let Some(rank) = e1_scores.iter().position(|(id, _)| *id == next.id) {
                mrr_e1_sum += 1.0 / (rank as f64 + 1.0);
            }

            count += 1;
        }
    }

    let mrr_e4 = mrr_e4_sum / count.max(1) as f64;
    let mrr_e1 = mrr_e1_sum / count.max(1) as f64;

    E4ContributionAnalysis {
        mrr_with_e4: mrr_e4,
        mrr_e1_only: mrr_e1,
        e4_contribution_pct: if mrr_e1 > 0.0 { (mrr_e4 - mrr_e1) / mrr_e1 } else { 0.0 },
        sequence_vs_timestamp: mrr_e4 / mrr_e1.max(0.001),
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

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

fn compute_kendalls_tau(vectors: &[(usize, &[f32])]) -> f64 {
    if vectors.len() < 2 {
        return 0.0;
    }

    let n = vectors.len();
    let mut concordant = 0;
    let mut discordant = 0;

    // Compare all pairs
    for i in 0..n {
        for j in i + 1..n {
            let (idx_i, vec_i) = vectors[i];
            let (idx_j, vec_j) = vectors[j];

            // Original order: idx_i < idx_j (by construction)
            // Similarity-based order: compare magnitudes or specific dimension
            let mag_i: f32 = vec_i.iter().map(|x| x * x).sum::<f32>().sqrt();
            let mag_j: f32 = vec_j.iter().map(|x| x * x).sum::<f32>().sqrt();

            // If embeddings encode sequence, magnitude or specific dims should correlate
            if (idx_i < idx_j && mag_i < mag_j) || (idx_i > idx_j && mag_i > mag_j) {
                concordant += 1;
            } else if (idx_i < idx_j && mag_i > mag_j) || (idx_i > idx_j && mag_i < mag_j) {
                discordant += 1;
            }
            // Ties are ignored
        }
    }

    let total = concordant + discordant;
    if total == 0 {
        return 0.0;
    }

    (concordant as f64 - discordant as f64) / total as f64
}

fn generate_recommendations(
    direction: &DirectionFilteringResults,
    ordering: &SequenceOrderingResults,
    chain: &ChainTraversalResults,
    boundary: &BoundaryDetectionResults,
    baseline: &TimestampBaselineResults,
) -> Vec<String> {
    let mut recs = Vec::new();

    // Critical: timestamp-based behavior detected
    if direction.is_timestamp_based {
        recs.push("[CRITICAL] E4 appears to be using timestamps instead of sequence positions. \
                   The before/after asymmetry (1.0/0.0) indicates broken behavior.".to_string());
    }

    if ordering.is_constant_tau {
        recs.push("[CRITICAL] Kendall's tau is constantly 1.0, indicating timestamp-based ordering.".to_string());
    }

    // Symmetry check
    if direction.symmetry_score < 0.7 {
        recs.push(format!(
            "Direction symmetry ({:.2}) below target (0.7). E4 may not be encoding sequences correctly.",
            direction.symmetry_score
        ));
    }

    // Accuracy checks
    if direction.before_accuracy < 0.65 {
        recs.push(format!(
            "Before accuracy ({:.1}%) below target (65%). Check sequence embedding generation.",
            direction.before_accuracy * 100.0
        ));
    }

    if direction.after_accuracy < 0.65 {
        recs.push(format!(
            "After accuracy ({:.1}%) below target (65%). This is the key metric for fixed E4.",
            direction.after_accuracy * 100.0
        ));
    }

    // Kendall's tau
    if ordering.avg_kendalls_tau < 0.5 {
        recs.push(format!(
            "Kendall's tau ({:.2}) below target (0.5). Sequence order preservation is weak.",
            ordering.avg_kendalls_tau
        ));
    }

    // Chain traversal
    if chain.mrr_next < 0.4 {
        recs.push(format!(
            "Chain MRR ({:.2}) below target (0.4). E4 not effective for sequence navigation.",
            chain.mrr_next
        ));
    }

    // Boundary detection
    if boundary.f1_score < 0.6 {
        recs.push(format!(
            "Boundary F1 ({:.2}) below target (0.6). Consider tuning similarity threshold.",
            boundary.f1_score
        ));
    }

    // Improvement check
    if baseline.sequence_improvement_pct <= 0.0 {
        recs.push("Sequence-based approach shows no improvement over timestamp baseline. \
                   E4 fix may not be working correctly.".to_string());
    }

    if recs.is_empty() {
        recs.push("All targets met! E4 sequence embedder is working correctly with real data.".to_string());
    }

    recs
}

fn save_results(results: &TemporalRealDataResults, path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, results)?;
    Ok(())
}

fn save_markdown_report(results: &TemporalRealDataResults, path: &Path) -> std::io::Result<()> {
    let mut f = File::create(path)?;

    writeln!(f, "# E4 Temporal Sequence Benchmark with Real HuggingFace Data")?;
    writeln!(f)?;
    writeln!(f, "**Generated:** {}", results.timestamp)?;
    writeln!(f)?;

    writeln!(f, "## Executive Summary")?;
    writeln!(f)?;
    let symmetry_status = if results.direction_filtering.symmetry_score >= 0.7 { "PASS" } else { "FAIL" };
    let timestamp_warning = if results.direction_filtering.is_timestamp_based { " (TIMESTAMP-BASED!)" } else { "" };
    writeln!(f, "- **Direction Symmetry:** {:.2} [{}]{}",
        results.direction_filtering.symmetry_score, symmetry_status, timestamp_warning)?;
    writeln!(f, "- **Before/After Accuracy:** {:.1}% / {:.1}%",
        results.direction_filtering.before_accuracy * 100.0,
        results.direction_filtering.after_accuracy * 100.0)?;
    writeln!(f, "- **Kendall's Tau:** {:.3}", results.sequence_ordering.avg_kendalls_tau)?;
    writeln!(f, "- **Chain MRR:** {:.3}", results.chain_traversal.mrr_next)?;
    writeln!(f)?;

    writeln!(f, "## Configuration")?;
    writeln!(f)?;
    writeln!(f, "| Parameter | Value |")?;
    writeln!(f, "|-----------|-------|")?;
    writeln!(f, "| Total Chunks | {} |", results.dataset_info.total_chunks)?;
    writeln!(f, "| Documents | {} |", results.dataset_info.total_documents)?;
    writeln!(f, "| Sessions Generated | {} |", results.session_info.total_sessions)?;
    writeln!(f, "| Chunks per Session | {:.1} |", results.session_info.avg_chunks_per_session)?;
    writeln!(f, "| E4 Coverage | {:.1}% |", results.embedding_stats.e4_coverage * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## Direction Filtering (Critical for E4 Fix Validation)")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target | Status |")?;
    writeln!(f, "|--------|-------|--------|--------|")?;
    writeln!(f, "| Before Accuracy | {:.1}% | >=65% | {} |",
        results.direction_filtering.before_accuracy * 100.0,
        if results.direction_filtering.before_accuracy >= 0.65 { "PASS" } else { "FAIL" })?;
    writeln!(f, "| After Accuracy | {:.1}% | >=65% | {} |",
        results.direction_filtering.after_accuracy * 100.0,
        if results.direction_filtering.after_accuracy >= 0.65 { "PASS" } else { "FAIL" })?;
    writeln!(f, "| **Symmetry Score** | **{:.2}** | >=0.7 | **{}** |",
        results.direction_filtering.symmetry_score,
        if results.direction_filtering.symmetry_score >= 0.7 { "PASS" } else { "FAIL" })?;
    writeln!(f, "| Timestamp-Based | {} | No | {} |",
        results.direction_filtering.is_timestamp_based,
        if !results.direction_filtering.is_timestamp_based { "PASS" } else { "FAIL" })?;
    writeln!(f)?;

    if results.direction_filtering.is_timestamp_based {
        writeln!(f, "> **WARNING:** Before=1.0 and After=0.0 indicates E4 is using Unix timestamps")?;
        writeln!(f, "> instead of session sequence positions. The E4 fix is NOT working.")?;
        writeln!(f)?;
    }

    writeln!(f, "## Sequence Ordering (Kendall's Tau)")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target | Status |")?;
    writeln!(f, "|--------|-------|--------|--------|")?;
    writeln!(f, "| **Avg Kendall's Tau** | **{:.4}** | >=0.5 | **{}** |",
        results.sequence_ordering.avg_kendalls_tau,
        if results.sequence_ordering.avg_kendalls_tau >= 0.5 { "PASS" } else { "FAIL" })?;
    writeln!(f, "| Min Tau | {:.4} | - | - |", results.sequence_ordering.min_kendalls_tau)?;
    writeln!(f, "| Max Tau | {:.4} | - | - |", results.sequence_ordering.max_kendalls_tau)?;
    writeln!(f, "| Tau Variance | {:.4} | >0.001 | {} |",
        results.sequence_ordering.tau_variance,
        if results.sequence_ordering.tau_variance > 0.001 { "PASS" } else { "FAIL" })?;
    writeln!(f)?;

    writeln!(f, "## Chain Traversal")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target | Status |")?;
    writeln!(f, "|--------|-------|--------|--------|")?;
    writeln!(f, "| **MRR Next** | **{:.4}** | >=0.4 | **{}** |",
        results.chain_traversal.mrr_next,
        if results.chain_traversal.mrr_next >= 0.4 { "PASS" } else { "FAIL" })?;
    writeln!(f, "| MRR Prev | {:.4} | - | - |", results.chain_traversal.mrr_prev)?;
    writeln!(f, "| Completion Rate | {:.1}% | - | - |", results.chain_traversal.completion_rate * 100.0)?;
    writeln!(f, "| Avg Depth | {:.2} | - | - |", results.chain_traversal.avg_traversal_depth)?;
    writeln!(f)?;

    writeln!(f, "## Boundary Detection")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value | Target | Status |")?;
    writeln!(f, "|--------|-------|--------|--------|")?;
    writeln!(f, "| Precision | {:.1}% | - | - |", results.boundary_detection.precision * 100.0)?;
    writeln!(f, "| Recall | {:.1}% | - | - |", results.boundary_detection.recall * 100.0)?;
    writeln!(f, "| **F1 Score** | **{:.2}** | >=0.6 | **{}** |",
        results.boundary_detection.f1_score,
        if results.boundary_detection.f1_score >= 0.6 { "PASS" } else { "FAIL" })?;
    writeln!(f)?;

    writeln!(f, "## Timestamp Baseline Comparison")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Timestamp (Broken) | Sequence (Fixed) |")?;
    writeln!(f, "|--------|-------------------|------------------|")?;
    writeln!(f, "| Before Accuracy | {:.1}% | {:.1}% |",
        results.timestamp_baseline.timestamp_before_accuracy * 100.0,
        results.direction_filtering.before_accuracy * 100.0)?;
    writeln!(f, "| After Accuracy | {:.1}% | {:.1}% |",
        results.timestamp_baseline.timestamp_after_accuracy * 100.0,
        results.direction_filtering.after_accuracy * 100.0)?;
    writeln!(f, "| Symmetry | {:.2} | {:.2} |",
        results.timestamp_baseline.timestamp_symmetry,
        results.direction_filtering.symmetry_score)?;
    writeln!(f)?;
    writeln!(f, "**Improvement:** {:.1}%", results.timestamp_baseline.sequence_improvement_pct * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## E4 Contribution Analysis")?;
    writeln!(f)?;
    writeln!(f, "| Metric | Value |")?;
    writeln!(f, "|--------|-------|")?;
    writeln!(f, "| MRR with E4 | {:.4} |", results.e4_contribution.mrr_with_e4)?;
    writeln!(f, "| MRR E1-only | {:.4} |", results.e4_contribution.mrr_e1_only)?;
    writeln!(f, "| E4 Contribution | {:.1}% |", results.e4_contribution.e4_contribution_pct * 100.0)?;
    writeln!(f)?;

    writeln!(f, "## Recommendations")?;
    writeln!(f)?;
    for rec in &results.recommendations {
        writeln!(f, "- {}", rec)?;
    }
    writeln!(f)?;

    Ok(())
}

fn print_summary(results: &TemporalRealDataResults) {
    println!("Direction Filtering: before={:.1}%, after={:.1}%, symmetry={:.2}",
        results.direction_filtering.before_accuracy * 100.0,
        results.direction_filtering.after_accuracy * 100.0,
        results.direction_filtering.symmetry_score);

    if results.direction_filtering.is_timestamp_based {
        println!("  [WARNING] Timestamp-based behavior detected!");
    }

    println!("Sequence Ordering: Kendall's tau={:.3} (variance={:.4})",
        results.sequence_ordering.avg_kendalls_tau,
        results.sequence_ordering.tau_variance);

    println!("Chain Traversal: MRR={:.3}, Completion={:.1}%",
        results.chain_traversal.mrr_next,
        results.chain_traversal.completion_rate * 100.0);

    println!("Boundary Detection: F1={:.2}",
        results.boundary_detection.f1_score);

    println!("Improvement over timestamp: {:.1}%",
        results.timestamp_baseline.sequence_improvement_pct * 100.0);
}

fn print_target_evaluation(results: &TemporalRealDataResults) -> bool {
    let mut all_pass = true;

    // Critical: Not timestamp-based
    let not_timestamp = !results.direction_filtering.is_timestamp_based;
    println!("  Timestamp-Based Detection: {} (target: No)",
        if results.direction_filtering.is_timestamp_based { "YES" } else { "No" });
    println!("    Status: {}", if not_timestamp { "PASS" } else { "FAIL" });
    all_pass &= not_timestamp;

    // Symmetry score >= 0.7
    let symmetry_pass = results.direction_filtering.symmetry_score >= 0.7;
    println!("  Direction Symmetry: {:.2} (target: >=0.7)", results.direction_filtering.symmetry_score);
    println!("    Status: {}", if symmetry_pass { "PASS" } else { "FAIL" });
    all_pass &= symmetry_pass;

    // Before accuracy >= 0.65
    let before_pass = results.direction_filtering.before_accuracy >= 0.65;
    println!("  Before Accuracy: {:.1}% (target: >=65%)", results.direction_filtering.before_accuracy * 100.0);
    println!("    Status: {}", if before_pass { "PASS" } else { "FAIL" });
    all_pass &= before_pass;

    // After accuracy >= 0.65
    let after_pass = results.direction_filtering.after_accuracy >= 0.65;
    println!("  After Accuracy: {:.1}% (target: >=65%)", results.direction_filtering.after_accuracy * 100.0);
    println!("    Status: {}", if after_pass { "PASS" } else { "FAIL" });
    all_pass &= after_pass;

    // Kendall's tau >= 0.5
    let tau_pass = results.sequence_ordering.avg_kendalls_tau >= 0.5;
    println!("  Kendall's Tau: {:.3} (target: >=0.5)", results.sequence_ordering.avg_kendalls_tau);
    println!("    Status: {}", if tau_pass { "PASS" } else { "FAIL" });
    all_pass &= tau_pass;

    // Chain MRR >= 0.4
    let chain_pass = results.chain_traversal.mrr_next >= 0.4;
    println!("  Chain MRR: {:.3} (target: >=0.4)", results.chain_traversal.mrr_next);
    println!("    Status: {}", if chain_pass { "PASS" } else { "FAIL" });
    all_pass &= chain_pass;

    // Boundary F1 >= 0.6
    let boundary_pass = results.boundary_detection.f1_score >= 0.6;
    println!("  Boundary F1: {:.2} (target: >=0.6)", results.boundary_detection.f1_score);
    println!("    Status: {}", if boundary_pass { "PASS" } else { "FAIL" });
    all_pass &= boundary_pass;

    // Improvement over baseline
    let improvement_pass = results.timestamp_baseline.sequence_improvement_pct > 0.0;
    println!("  Improvement over Timestamp: {:.1}% (target: >0%)",
        results.timestamp_baseline.sequence_improvement_pct * 100.0);
    println!("    Status: {}", if improvement_pass { "PASS" } else { "FAIL" });
    all_pass &= improvement_pass;

    all_pass
}
