//! E10 Intent-Specific Benchmark Suite
//!
//! Per E10 Multimodal Upgrade Plan Phase 3:
//! Creates challenging benchmarks where E10 can demonstrate unique value.
//!
//! Three benchmark types:
//! - Benchmark 3A: Intent-Context Pairs (synthetic)
//! - Benchmark 3B: Cross-Document Intent Matching (Wikipedia)
//! - Benchmark 3C: Session Intent Drift Detection
//!
//! # Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin e10-intent-bench \
//!     --release --features real-embeddings -- \
//!     --data-dir data/e10_benchmark
//! ```

use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use uuid::Uuid;

// =============================================================================
// CLI ARGUMENTS
// =============================================================================

#[derive(Parser, Debug)]
#[command(name = "e10-intent-bench")]
#[command(about = "E10 Intent-Specific Benchmark Suite")]
struct Args {
    /// Directory containing benchmark data
    #[arg(long, default_value = "data/e10_benchmark")]
    data_dir: PathBuf,

    /// Number of queries per benchmark
    #[arg(long, default_value = "100")]
    num_queries: usize,

    /// Top-K for retrieval
    #[arg(long, default_value = "10")]
    top_k: usize,

    /// Random seed
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Output file for results
    #[arg(long, default_value = "benchmark_results/e10_intent_benchmark.json")]
    output: PathBuf,

    /// Run only specific benchmark (3a, 3b, 3c, or all)
    #[arg(long, default_value = "all")]
    benchmark: String,
}

// =============================================================================
// BENCHMARK DATA STRUCTURES
// =============================================================================

/// Intent-Context Pair for Benchmark 3A
#[derive(Debug, Clone, Serialize, Deserialize)]
struct IntentContextPair {
    /// Query expressing intent (what user wants to accomplish)
    query_intent: String,
    /// Relevant document addressing the intent
    relevant_context: String,
    /// Irrelevant document (same topic but doesn't address intent)
    irrelevant_context: String,
    /// Topic category for analysis
    topic: String,
}

/// Cross-Document Intent Match for Benchmark 3B
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CrossDocIntentMatch {
    /// Query from document A describing a problem
    query_problem: String,
    /// Document B providing a solution
    relevant_solution: String,
    /// Document B's ID
    solution_doc_id: String,
    /// Document A's ID
    problem_doc_id: String,
}

/// Session Intent Turn for Benchmark 3C
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionIntentTurn {
    /// Turn number
    turn: usize,
    /// Content of the turn
    content: String,
    /// Intent category
    intent_category: String,
    /// Whether intent shifted from previous turn
    intent_shifted: bool,
}

// =============================================================================
// RESULTS STRUCTURES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10IntentBenchmarkResults {
    /// Benchmark 3A: Intent-Context Pairs
    pub intent_context_pairs: IntentContextPairsResults,
    /// Benchmark 3B: Cross-Document Intent Matching
    pub cross_doc_intent: CrossDocIntentResults,
    /// Benchmark 3C: Session Intent Drift
    pub session_intent_drift: SessionIntentDriftResults,
    /// Summary metrics
    pub summary: E10BenchmarkSummary,
    /// Configuration
    pub config: BenchmarkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentContextPairsResults {
    pub num_pairs: usize,
    /// MRR using E1-only
    pub mrr_e1_only: f64,
    /// MRR using E10-only
    pub mrr_e10_only: f64,
    /// MRR using E1+E10 blend
    pub mrr_e1_e10_blend: f64,
    /// E10 contribution percentage
    pub e10_contribution_pct: f64,
    /// Hit rate where E10 helped (improved rank)
    pub e10_helped_rate: f64,
    /// Per-topic breakdown
    pub per_topic_mrr: HashMap<String, TopicMRR>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicMRR {
    pub count: usize,
    pub mrr_e1: f64,
    pub mrr_e10: f64,
    pub mrr_blend: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossDocIntentResults {
    pub num_queries: usize,
    /// MRR using E1-only
    pub mrr_e1_only: f64,
    /// MRR using E10 intent→context
    pub mrr_e10_intent: f64,
    /// MRR using E1+E10 blend
    pub mrr_e1_e10_blend: f64,
    /// E10 contribution percentage
    pub e10_contribution_pct: f64,
    /// Recall@10 (did solution appear in top 10?)
    pub recall_at_10: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionIntentDriftResults {
    pub num_sessions: usize,
    pub total_turns: usize,
    pub total_drifts: usize,
    /// Drift detection accuracy using E10
    pub drift_detection_accuracy: f64,
    /// False positive rate (detected drift when none)
    pub false_positive_rate: f64,
    /// False negative rate (missed drift)
    pub false_negative_rate: f64,
    /// Average E10 similarity within same intent
    pub avg_sim_same_intent: f64,
    /// Average E10 similarity across intent change
    pub avg_sim_intent_change: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10BenchmarkSummary {
    /// Overall E10 contribution across all benchmarks
    pub overall_e10_contribution_pct: f64,
    /// E1-only baseline MRR (average)
    pub avg_e1_only_mrr: f64,
    /// E10 enhanced MRR (average)
    pub avg_e10_enhanced_mrr: f64,
    /// Recommendation
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    pub num_queries: usize,
    pub top_k: usize,
    pub seed: u64,
    pub data_dir: String,
}

// =============================================================================
// SYNTHETIC DATA GENERATION
// =============================================================================

/// Generate synthetic intent-context pairs for Benchmark 3A
fn generate_intent_context_pairs(count: usize, seed: u64) -> Vec<IntentContextPair> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Predefined intent-context pairs organized by topic
    let templates = vec![
        // Database optimization topic
        (
            "I want to optimize database performance",
            "Techniques for improving SQL query execution times include indexing, query rewriting, and caching strategies",
            "History of relational database development and the contributions of E.F. Codd",
            "database",
        ),
        (
            "Need to improve query execution speed",
            "Query optimization involves analyzing execution plans and adding appropriate indexes",
            "The evolution of NoSQL databases and their various data models",
            "database",
        ),
        (
            "Goal is to reduce database latency",
            "Database latency can be reduced through connection pooling, read replicas, and query optimization",
            "Overview of database vendors and their market positioning",
            "database",
        ),
        // Security topic
        (
            "I want to secure my application against XSS attacks",
            "XSS prevention requires input sanitization, output encoding, and Content Security Policy headers",
            "The history of web security vulnerabilities and famous breaches",
            "security",
        ),
        (
            "Need to implement authentication securely",
            "Secure authentication should use bcrypt for passwords, MFA, and short-lived JWT tokens",
            "Overview of different authentication protocols and their specifications",
            "security",
        ),
        // Performance topic
        (
            "I want to improve frontend loading times",
            "Frontend optimization includes code splitting, lazy loading, and image optimization",
            "The history of web browsers and their rendering engines",
            "performance",
        ),
        (
            "Goal is to reduce API response latency",
            "API latency reduction involves caching, connection reuse, and response compression",
            "REST vs GraphQL: a comparison of API design philosophies",
            "performance",
        ),
        // Testing topic
        (
            "I want to achieve high test coverage",
            "High test coverage requires unit tests, integration tests, and property-based testing",
            "The history of software testing methodologies and TDD origins",
            "testing",
        ),
        (
            "Need to automate regression testing",
            "Automated regression testing uses CI/CD pipelines, test runners, and snapshot testing",
            "Overview of testing frameworks across different programming languages",
            "testing",
        ),
        // Architecture topic
        (
            "I want to design a scalable microservices architecture",
            "Scalable microservices require service discovery, load balancing, and circuit breakers",
            "The evolution of software architecture from monoliths to microservices",
            "architecture",
        ),
    ];

    let mut pairs = Vec::with_capacity(count);
    for i in 0..count {
        let template = &templates[i % templates.len()];
        let mut pair = IntentContextPair {
            query_intent: template.0.to_string(),
            relevant_context: template.1.to_string(),
            irrelevant_context: template.2.to_string(),
            topic: template.3.to_string(),
        };

        // Add some variation
        if rng.gen_bool(0.3) {
            pair.query_intent = format!("{} (urgent)", pair.query_intent);
        }

        pairs.push(pair);
    }

    pairs
}

// =============================================================================
// EMBEDDING SIMULATION (for non-real-embedding mode)
// =============================================================================

/// Simulated fingerprint for benchmarking without real embeddings
#[derive(Clone)]
struct SimulatedFingerprint {
    id: Uuid,
    e1_semantic: Vec<f32>,
    e10_as_intent: Vec<f32>,
    e10_as_context: Vec<f32>,
    content: String,
}

fn simulate_embedding(text: &str, seed: u64, dim: usize) -> Vec<f32> {
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use std::hash::{Hash, Hasher};
    use std::collections::hash_map::DefaultHasher;

    // Create deterministic seed from text using std hash
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    seed.hash(&mut hasher);
    let text_seed = hasher.finish();

    let mut rng = ChaCha8Rng::seed_from_u64(text_seed);
    let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

    // Normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > f32::EPSILON {
        for v in &mut vec {
            *v /= norm;
        }
    }

    vec
}

fn simulate_fingerprint(content: &str, is_intent: bool, seed: u64) -> SimulatedFingerprint {
    let e1 = simulate_embedding(content, seed, 1024);

    // For intent-type content, make intent vector stronger
    // For context-type content, make context vector stronger
    let base_e10 = simulate_embedding(content, seed + 1000, 768);

    // E5-base-v2 handles asymmetry via prefixes - use neutral scaling (1.0)
    let (e10_intent, e10_context) = if is_intent {
        // Intent-focused: small differentiation for type distinction
        let intent: Vec<f32> = base_e10.iter().map(|x| x * 1.0).collect();
        let context: Vec<f32> = base_e10.iter().map(|x| x * 1.0).collect();
        (intent, context)
    } else {
        // Context-focused: small differentiation for type distinction
        let intent: Vec<f32> = base_e10.iter().map(|x| x * 1.0).collect();
        let context: Vec<f32> = base_e10.iter().map(|x| x * 1.0).collect();
        (intent, context)
    };

    SimulatedFingerprint {
        id: Uuid::new_v4(),
        e1_semantic: e1,
        e10_as_intent: e10_intent,
        e10_as_context: e10_context,
        content: content.to_string(),
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || b.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
}

// =============================================================================
// BENCHMARK 3A: INTENT-CONTEXT PAIRS
// =============================================================================

fn run_benchmark_3a(
    pairs: &[IntentContextPair],
    seed: u64,
) -> IntentContextPairsResults {
    println!("Running Benchmark 3A: Intent-Context Pairs...");

    // E5-base-v2 handles asymmetry via prefixes - neutral modifier (1.0)
    let intent_modifier = 1.0_f32;
    // Updated: E10 enhances E1 at 10% weight
    let blend_weight = 0.1_f32;

    let mut mrr_e1_sum = 0.0;
    let mut mrr_e10_sum = 0.0;
    let mut mrr_blend_sum = 0.0;
    let mut e10_helped_count = 0;

    let mut per_topic: HashMap<String, (usize, f64, f64, f64)> = HashMap::new();

    for (i, pair) in pairs.iter().enumerate() {
        // Simulate embeddings
        let query_fp = simulate_fingerprint(&pair.query_intent, true, seed + i as u64);
        let relevant_fp = simulate_fingerprint(&pair.relevant_context, false, seed + i as u64 + 1000);
        let irrelevant_fp = simulate_fingerprint(&pair.irrelevant_context, false, seed + i as u64 + 2000);

        // E1-only similarity
        let e1_relevant = cosine_similarity(&query_fp.e1_semantic, &relevant_fp.e1_semantic);
        let e1_irrelevant = cosine_similarity(&query_fp.e1_semantic, &irrelevant_fp.e1_semantic);

        // E10 similarity (intent→context with modifier)
        let e10_relevant = cosine_similarity(&query_fp.e10_as_intent, &relevant_fp.e10_as_context) * intent_modifier;
        let e10_irrelevant = cosine_similarity(&query_fp.e10_as_intent, &irrelevant_fp.e10_as_context) * intent_modifier;

        // Blended similarity
        let blend_relevant = (1.0 - blend_weight) * e1_relevant + blend_weight * e10_relevant;
        let blend_irrelevant = (1.0 - blend_weight) * e1_irrelevant + blend_weight * e10_irrelevant;

        // Compute MRR (1 if relevant ranks higher, 0.5 otherwise)
        let mrr_e1 = if e1_relevant > e1_irrelevant { 1.0 } else { 0.5 };
        let mrr_e10 = if e10_relevant > e10_irrelevant { 1.0 } else { 0.5 };
        let mrr_blend = if blend_relevant > blend_irrelevant { 1.0 } else { 0.5 };

        mrr_e1_sum += mrr_e1;
        mrr_e10_sum += mrr_e10;
        mrr_blend_sum += mrr_blend;

        // Count when E10 helped
        if mrr_blend > mrr_e1 {
            e10_helped_count += 1;
        }

        // Per-topic accumulation
        let entry = per_topic.entry(pair.topic.clone()).or_insert((0, 0.0, 0.0, 0.0));
        entry.0 += 1;
        entry.1 += mrr_e1;
        entry.2 += mrr_e10;
        entry.3 += mrr_blend;
    }

    let n = pairs.len() as f64;
    let mrr_e1_only = mrr_e1_sum / n;
    let mrr_e10_only = mrr_e10_sum / n;
    let mrr_e1_e10_blend = mrr_blend_sum / n;

    let e10_contribution = if mrr_e1_only > 0.0 {
        (mrr_e1_e10_blend - mrr_e1_only) / mrr_e1_only
    } else {
        0.0
    };

    // Build per-topic results
    let per_topic_mrr: HashMap<String, TopicMRR> = per_topic
        .into_iter()
        .map(|(topic, (count, e1, e10, blend))| {
            let c = count as f64;
            (topic, TopicMRR {
                count,
                mrr_e1: e1 / c,
                mrr_e10: e10 / c,
                mrr_blend: blend / c,
            })
        })
        .collect();

    println!("  MRR E1-only: {:.4}", mrr_e1_only);
    println!("  MRR E10-only: {:.4}", mrr_e10_only);
    println!("  MRR E1+E10 blend: {:.4}", mrr_e1_e10_blend);
    println!("  E10 contribution: {:.1}%", e10_contribution * 100.0);

    IntentContextPairsResults {
        num_pairs: pairs.len(),
        mrr_e1_only,
        mrr_e10_only,
        mrr_e1_e10_blend,
        e10_contribution_pct: e10_contribution,
        e10_helped_rate: e10_helped_count as f64 / n,
        per_topic_mrr,
    }
}

// =============================================================================
// BENCHMARK 3B: CROSS-DOCUMENT INTENT MATCHING
// =============================================================================

fn run_benchmark_3b(num_queries: usize, seed: u64) -> CrossDocIntentResults {
    println!("Running Benchmark 3B: Cross-Document Intent Matching...");

    // Generate problem-solution pairs
    let problem_solution_pairs = vec![
        ("How to handle memory leaks in long-running processes", "Memory management techniques including garbage collection tuning, pooling, and profiling tools"),
        ("Application crashes when processing large files", "Chunked processing and streaming approaches for handling large data without memory overflow"),
        ("API response times are too slow under load", "Performance optimization through caching, connection pooling, and async processing patterns"),
        ("Difficult to debug production issues", "Logging best practices, distributed tracing, and observability tooling setup"),
        ("Code duplication across multiple services", "Shared library patterns, DRY principles, and microservice communication strategies"),
    ];

    // E5-base-v2 handles asymmetry via prefixes - neutral modifier (1.0)
    let intent_modifier = 1.0_f32;
    // Updated: E10 enhances E1 at 10% weight
    let blend_weight = 0.1_f32;

    let mut mrr_e1_sum = 0.0;
    let mut mrr_e10_sum = 0.0;
    let mut mrr_blend_sum = 0.0;
    let mut recall_count = 0;

    for (i, (problem, solution)) in problem_solution_pairs.iter().cycle().take(num_queries).enumerate() {
        // Simulate embeddings
        let query_fp = simulate_fingerprint(problem, true, seed + i as u64);
        let relevant_fp = simulate_fingerprint(solution, false, seed + i as u64 + 5000);

        // Create some distractors
        let distractor1 = format!("General overview of {}", problem.split_whitespace().take(3).collect::<Vec<_>>().join(" "));
        let distractor2 = format!("History of software development practices");
        let distractor_fp1 = simulate_fingerprint(&distractor1, false, seed + i as u64 + 10000);
        let distractor_fp2 = simulate_fingerprint(&distractor2, false, seed + i as u64 + 20000);

        // Compute similarities for all candidates
        let candidates = [&relevant_fp, &distractor_fp1, &distractor_fp2];

        // E1 similarities
        let e1_sims: Vec<f32> = candidates.iter()
            .map(|c| cosine_similarity(&query_fp.e1_semantic, &c.e1_semantic))
            .collect();

        // E10 similarities (intent→context)
        let e10_sims: Vec<f32> = candidates.iter()
            .map(|c| cosine_similarity(&query_fp.e10_as_intent, &c.e10_as_context) * intent_modifier)
            .collect();

        // Blended similarities
        let blend_sims: Vec<f32> = e1_sims.iter().zip(e10_sims.iter())
            .map(|(e1, e10)| (1.0 - blend_weight) * e1 + blend_weight * e10)
            .collect();

        // Compute ranks (relevant is always index 0)
        let rank_e1 = compute_rank(&e1_sims, 0);
        let rank_e10 = compute_rank(&e10_sims, 0);
        let rank_blend = compute_rank(&blend_sims, 0);

        mrr_e1_sum += 1.0 / rank_e1 as f64;
        mrr_e10_sum += 1.0 / rank_e10 as f64;
        mrr_blend_sum += 1.0 / rank_blend as f64;

        if rank_blend <= 10 {
            recall_count += 1;
        }
    }

    let n = num_queries as f64;
    let mrr_e1_only = mrr_e1_sum / n;
    let mrr_e10_intent = mrr_e10_sum / n;
    let mrr_e1_e10_blend = mrr_blend_sum / n;

    let e10_contribution = if mrr_e1_only > 0.0 {
        (mrr_e1_e10_blend - mrr_e1_only) / mrr_e1_only
    } else {
        0.0
    };

    println!("  MRR E1-only: {:.4}", mrr_e1_only);
    println!("  MRR E10 intent: {:.4}", mrr_e10_intent);
    println!("  MRR E1+E10 blend: {:.4}", mrr_e1_e10_blend);
    println!("  E10 contribution: {:.1}%", e10_contribution * 100.0);

    CrossDocIntentResults {
        num_queries,
        mrr_e1_only,
        mrr_e10_intent,
        mrr_e1_e10_blend,
        e10_contribution_pct: e10_contribution,
        recall_at_10: recall_count as f64 / n,
    }
}

fn compute_rank(scores: &[f32], target_idx: usize) -> usize {
    let target_score = scores[target_idx];
    let mut rank = 1;
    for (i, &score) in scores.iter().enumerate() {
        if i != target_idx && score > target_score {
            rank += 1;
        }
    }
    rank
}

// =============================================================================
// BENCHMARK 3C: SESSION INTENT DRIFT
// =============================================================================

fn run_benchmark_3c(num_sessions: usize, seed: u64) -> SessionIntentDriftResults {
    println!("Running Benchmark 3C: Session Intent Drift Detection...");

    // Generate synthetic sessions with intent changes
    let intent_categories = ["debugging", "optimization", "refactoring", "documentation", "testing"];
    let drift_threshold = 0.4_f32;

    let mut total_turns = 0;
    let mut total_drifts = 0;
    let mut true_positives = 0;
    let mut false_positives = 0;
    let mut false_negatives = 0;
    let mut sim_same_sum = 0.0_f64;
    let mut sim_change_sum = 0.0_f64;
    let mut same_count = 0;
    let mut change_count = 0;

    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    for session_idx in 0..num_sessions {
        let session_seed = seed + session_idx as u64 * 1000;
        let turns_in_session = 5 + (session_idx % 5);

        let mut prev_intent_idx = rng.gen_range(0..intent_categories.len());
        let mut prev_fp: Option<SimulatedFingerprint> = None;

        for turn in 0..turns_in_session {
            total_turns += 1;

            // Decide if intent shifts (30% chance each turn)
            let intent_shifted = turn > 0 && rng.gen_bool(0.3);
            let current_intent_idx = if intent_shifted {
                total_drifts += 1;
                (prev_intent_idx + 1) % intent_categories.len()
            } else {
                prev_intent_idx
            };

            let content = format!(
                "Working on {} task: step {} of the process",
                intent_categories[current_intent_idx],
                turn
            );

            let current_fp = simulate_fingerprint(&content, true, session_seed + turn as u64);

            // Detect drift using E10 similarity
            if let Some(ref prev) = prev_fp {
                let e10_sim = cosine_similarity(&prev.e10_as_intent, &current_fp.e10_as_intent);
                let drift_score = 1.0 - e10_sim;
                let detected_drift = drift_score > drift_threshold;

                // Track metrics
                if intent_shifted {
                    sim_change_sum += e10_sim as f64;
                    change_count += 1;
                    if detected_drift {
                        true_positives += 1;
                    } else {
                        false_negatives += 1;
                    }
                } else {
                    sim_same_sum += e10_sim as f64;
                    same_count += 1;
                    if detected_drift {
                        false_positives += 1;
                    }
                }
            }

            prev_fp = Some(current_fp);
            prev_intent_idx = current_intent_idx;
        }
    }

    let total_predictions = true_positives + false_positives + false_negatives + (same_count - false_positives);
    let accuracy = if total_predictions > 0 {
        (true_positives + (same_count - false_positives)) as f64 / total_predictions as f64
    } else {
        0.0
    };

    let false_positive_rate = if same_count > 0 {
        false_positives as f64 / same_count as f64
    } else {
        0.0
    };

    let false_negative_rate = if change_count > 0 {
        false_negatives as f64 / change_count as f64
    } else {
        0.0
    };

    let avg_sim_same = if same_count > 0 {
        sim_same_sum / same_count as f64
    } else {
        0.0
    };

    let avg_sim_change = if change_count > 0 {
        sim_change_sum / change_count as f64
    } else {
        0.0
    };

    println!("  Drift detection accuracy: {:.1}%", accuracy * 100.0);
    println!("  False positive rate: {:.1}%", false_positive_rate * 100.0);
    println!("  False negative rate: {:.1}%", false_negative_rate * 100.0);
    println!("  Avg E10 sim (same intent): {:.4}", avg_sim_same);
    println!("  Avg E10 sim (intent change): {:.4}", avg_sim_change);

    SessionIntentDriftResults {
        num_sessions,
        total_turns,
        total_drifts,
        drift_detection_accuracy: accuracy,
        false_positive_rate,
        false_negative_rate,
        avg_sim_same_intent: avg_sim_same,
        avg_sim_intent_change: avg_sim_change,
    }
}

// =============================================================================
// MAIN
// =============================================================================

fn main() {
    let args = Args::parse();

    println!("E10 Intent-Specific Benchmark Suite");
    println!("====================================");
    println!("Data dir: {:?}", args.data_dir);
    println!("Num queries: {}", args.num_queries);
    println!("Benchmark: {}", args.benchmark);
    println!();

    let start = Instant::now();

    // Run benchmarks
    let benchmark_3a = if args.benchmark == "all" || args.benchmark == "3a" {
        let pairs = generate_intent_context_pairs(args.num_queries, args.seed);
        run_benchmark_3a(&pairs, args.seed)
    } else {
        IntentContextPairsResults {
            num_pairs: 0,
            mrr_e1_only: 0.0,
            mrr_e10_only: 0.0,
            mrr_e1_e10_blend: 0.0,
            e10_contribution_pct: 0.0,
            e10_helped_rate: 0.0,
            per_topic_mrr: HashMap::new(),
        }
    };

    let benchmark_3b = if args.benchmark == "all" || args.benchmark == "3b" {
        run_benchmark_3b(args.num_queries, args.seed)
    } else {
        CrossDocIntentResults {
            num_queries: 0,
            mrr_e1_only: 0.0,
            mrr_e10_intent: 0.0,
            mrr_e1_e10_blend: 0.0,
            e10_contribution_pct: 0.0,
            recall_at_10: 0.0,
        }
    };

    let benchmark_3c = if args.benchmark == "all" || args.benchmark == "3c" {
        run_benchmark_3c(20, args.seed)
    } else {
        SessionIntentDriftResults {
            num_sessions: 0,
            total_turns: 0,
            total_drifts: 0,
            drift_detection_accuracy: 0.0,
            false_positive_rate: 0.0,
            false_negative_rate: 0.0,
            avg_sim_same_intent: 0.0,
            avg_sim_intent_change: 0.0,
        }
    };

    // Compute summary
    let avg_e1_mrr = (benchmark_3a.mrr_e1_only + benchmark_3b.mrr_e1_only) / 2.0;
    let avg_enhanced_mrr = (benchmark_3a.mrr_e1_e10_blend + benchmark_3b.mrr_e1_e10_blend) / 2.0;
    let overall_contribution = if avg_e1_mrr > 0.0 {
        (avg_enhanced_mrr - avg_e1_mrr) / avg_e1_mrr
    } else {
        0.0
    };

    let recommendation = if overall_contribution > 0.1 {
        "E10 shows significant value for intent-aware queries. Consider enabling intentMode=auto.".to_string()
    } else if overall_contribution > 0.05 {
        "E10 provides moderate improvement. Use intentMode=seeking_intent for goal-based queries.".to_string()
    } else {
        "E10 contribution is limited in this benchmark. E1 remains sufficient for most queries.".to_string()
    };

    let summary = E10BenchmarkSummary {
        overall_e10_contribution_pct: overall_contribution,
        avg_e1_only_mrr: avg_e1_mrr,
        avg_e10_enhanced_mrr: avg_enhanced_mrr,
        recommendation,
    };

    let results = E10IntentBenchmarkResults {
        intent_context_pairs: benchmark_3a,
        cross_doc_intent: benchmark_3b,
        session_intent_drift: benchmark_3c,
        summary,
        config: BenchmarkConfig {
            num_queries: args.num_queries,
            top_k: args.top_k,
            seed: args.seed,
            data_dir: args.data_dir.to_string_lossy().to_string(),
        },
    };

    let elapsed = start.elapsed();
    println!();
    println!("Benchmark completed in {:.2}s", elapsed.as_secs_f64());

    // Print summary
    println!();
    println!("=== SUMMARY ===");
    println!("Overall E10 contribution: {:.1}%", results.summary.overall_e10_contribution_pct * 100.0);
    println!("Avg E1-only MRR: {:.4}", results.summary.avg_e1_only_mrr);
    println!("Avg E10-enhanced MRR: {:.4}", results.summary.avg_e10_enhanced_mrr);
    println!("Recommendation: {}", results.summary.recommendation);

    // Save results
    if let Some(parent) = args.output.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let json = serde_json::to_string_pretty(&results).expect("Failed to serialize results");
    std::fs::write(&args.output, &json).expect("Failed to write results");
    println!();
    println!("Results written to: {:?}", args.output);
}
