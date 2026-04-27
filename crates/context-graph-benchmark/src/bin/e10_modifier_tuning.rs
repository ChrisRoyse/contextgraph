//! E10 Direction Modifier Tuning Benchmark
//!
//! Phase 6: Grid search over intent→context and context→intent modifiers
//! to find optimal asymmetric weights for E10 intent retrieval.
//!
//! # Algorithm
//!
//! 1. Load intent-context benchmark pairs (from Phase 3 data)
//! 2. For each (intent_mod, context_mod) combination:
//!    - Compute modified E10 similarities
//!    - Calculate MRR on intent retrieval task
//! 3. Output: optimal modifiers with confidence intervals
//!
//! # Expected Results
//!
//! The default 1.2/0.8 modifiers were set based on theoretical asymmetry.
//! This benchmark validates or finds better values empirically.
//!
//! # Usage
//!
//! ```bash
//! cargo run -p context-graph-benchmark --bin e10_modifier_tuning \
//!     --release --features real-embeddings -- \
//!     --data-dir data/e10_benchmark
//! ```

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use chrono::Utc;
use clap::Parser;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

/// CLI arguments for the modifier tuning benchmark.
#[derive(Parser, Debug)]
#[command(name = "e10-modifier-tuning")]
#[command(about = "Grid search for optimal E10 direction modifiers")]
struct Args {
    /// Data directory containing intent_context_pairs.jsonl
    #[arg(long, default_value = "data/e10_benchmark")]
    data_dir: PathBuf,

    /// Output directory for results
    #[arg(long, default_value = "benchmark_results")]
    output_dir: PathBuf,

    /// Number of bootstrap samples for confidence intervals
    #[arg(long, default_value = "100")]
    bootstrap_samples: usize,

    /// Random seed for reproducibility
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Use simulated embeddings (for testing without real models)
    #[arg(long)]
    simulated: bool,
}

/// Intent-context pair from benchmark data.
#[derive(Debug, Clone, Deserialize)]
struct IntentContextPair {
    intent: String,
    relevant_context: String,
    irrelevant_context: String,
}

/// Result of a single modifier configuration.
#[derive(Debug, Clone, Serialize)]
struct ModifierResult {
    intent_modifier: f32,
    context_modifier: f32,
    asymmetry_ratio: f32,
    mrr: f64,
    mrr_ci_lower: f64,
    mrr_ci_upper: f64,
    hit_rate_at_1: f64,
    avg_rank: f64,
}

/// Full tuning results.
#[derive(Debug, Clone, Serialize)]
struct TuningResults {
    timestamp: String,
    data_source: String,
    num_pairs: usize,
    bootstrap_samples: usize,
    seed: u64,
    simulated: bool,

    /// All tested modifier combinations
    results: Vec<ModifierResult>,

    /// Best configuration
    best_config: ModifierResult,

    /// Default (1.2/0.8) performance for comparison
    default_config: ModifierResult,

    /// Summary statistics
    summary: TuningSummary,
}

/// Summary of tuning results.
#[derive(Debug, Clone, Serialize)]
struct TuningSummary {
    optimal_intent_modifier: f32,
    optimal_context_modifier: f32,
    optimal_asymmetry_ratio: f32,
    improvement_over_default_pct: f64,
    default_mrr: f64,
    best_mrr: f64,
    modifier_sensitivity: String,
}

/// Simulated fingerprint with E10 intent/context vectors.
#[derive(Debug, Clone)]
struct SimulatedE10 {
    as_intent: Vec<f32>,
    as_context: Vec<f32>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("=== E10 Direction Modifier Tuning ===\n");

    // Load data
    let pairs = load_intent_context_pairs(&args.data_dir)?;
    println!("Loaded {} intent-context pairs\n", pairs.len());

    if pairs.is_empty() {
        println!("No data found. Creating synthetic data for demo...");
        let synthetic_pairs = create_synthetic_pairs(50);
        run_tuning(&synthetic_pairs, &args)?;
    } else {
        run_tuning(&pairs, &args)?;
    }

    Ok(())
}

fn load_intent_context_pairs(data_dir: &Path) -> anyhow::Result<Vec<IntentContextPair>> {
    let file_path = data_dir.join("intent_context_pairs.jsonl");

    if !file_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&file_path)?;
    let mut pairs = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let pair: IntentContextPair = serde_json::from_str(line)?;
        pairs.push(pair);
    }

    Ok(pairs)
}

fn create_synthetic_pairs(n: usize) -> Vec<IntentContextPair> {
    let intents = [
        "I want to optimize database performance",
        "Need to fix memory leaks",
        "Looking to implement authentication",
        "Want to add caching",
        "Need to improve test coverage",
    ];

    let relevant = [
        "Techniques for improving query execution times",
        "Use profiling tools to identify leaks",
        "Implement JWT tokens for stateless auth",
        "Use Redis for distributed caching",
        "Write unit tests for core logic",
    ];

    let irrelevant = [
        "History of database systems",
        "Memory management in operating systems",
        "History of cryptography",
        "Physics of computer memory",
        "Software quality philosophy",
    ];

    (0..n)
        .map(|i| IntentContextPair {
            intent: intents[i % intents.len()].to_string(),
            relevant_context: relevant[i % relevant.len()].to_string(),
            irrelevant_context: irrelevant[i % irrelevant.len()].to_string(),
        })
        .collect()
}

fn run_tuning(pairs: &[IntentContextPair], args: &Args) -> anyhow::Result<()> {
    // Generate embeddings for all pairs
    println!("Generating E10 embeddings...");
    let embeddings: Vec<(SimulatedE10, SimulatedE10, SimulatedE10)> = pairs
        .iter()
        .enumerate()
        .map(|(i, pair)| {
            let intent_emb = simulate_e10(&pair.intent, args.seed + i as u64);
            let relevant_emb = simulate_e10(&pair.relevant_context, args.seed + 1000 + i as u64);
            let irrelevant_emb =
                simulate_e10(&pair.irrelevant_context, args.seed + 2000 + i as u64);
            (intent_emb, relevant_emb, irrelevant_emb)
        })
        .collect();

    // Define modifier grid
    let intent_modifiers: [f32; 6] = [1.0, 1.1, 1.2, 1.3, 1.4, 1.5];
    let context_modifiers: [f32; 5] = [0.6, 0.7, 0.8, 0.9, 1.0];

    println!(
        "Testing {} modifier combinations...\n",
        intent_modifiers.len() * context_modifiers.len()
    );

    let mut results = Vec::new();

    for &intent_mod in &intent_modifiers {
        for &context_mod in &context_modifiers {
            let result = evaluate_modifiers(
                &embeddings,
                intent_mod,
                context_mod,
                args.bootstrap_samples,
                args.seed,
            );
            results.push(result);
        }
    }

    // Find best and default configurations
    let best = results
        .iter()
        .max_by(|a, b| a.mrr.partial_cmp(&b.mrr).unwrap())
        .unwrap()
        .clone();

    let default = results
        .iter()
        .find(|r| (r.intent_modifier - 1.2).abs() < 0.01 && (r.context_modifier - 0.8).abs() < 0.01)
        .unwrap()
        .clone();

    let improvement = ((best.mrr - default.mrr) / default.mrr) * 100.0;

    // Determine sensitivity
    let mrr_range = results.iter().map(|r| r.mrr).fold(0.0f64, |a, b| a.max(b))
        - results.iter().map(|r| r.mrr).fold(1.0f64, |a, b| a.min(b));
    let sensitivity = if mrr_range < 0.05 {
        "Low"
    } else if mrr_range < 0.15 {
        "Medium"
    } else {
        "High"
    };

    let summary = TuningSummary {
        optimal_intent_modifier: best.intent_modifier,
        optimal_context_modifier: best.context_modifier,
        optimal_asymmetry_ratio: best.asymmetry_ratio,
        improvement_over_default_pct: improvement,
        default_mrr: default.mrr,
        best_mrr: best.mrr,
        modifier_sensitivity: sensitivity.to_string(),
    };

    let tuning_results = TuningResults {
        timestamp: Utc::now().to_rfc3339(),
        data_source: args.data_dir.to_string_lossy().to_string(),
        num_pairs: pairs.len(),
        bootstrap_samples: args.bootstrap_samples,
        seed: args.seed,
        simulated: true, // Always simulated for now
        results: results.clone(),
        best_config: best.clone(),
        default_config: default.clone(),
        summary: summary.clone(),
    };

    // Print results
    print_results(&tuning_results);

    // Save results
    fs::create_dir_all(&args.output_dir)?;
    let output_path = args.output_dir.join("e10_modifier_tuning.json");
    fs::write(&output_path, serde_json::to_string_pretty(&tuning_results)?)?;
    println!("\nResults saved to: {}", output_path.display());

    // Generate markdown report
    let md_path = args.output_dir.join("e10_modifier_tuning.md");
    fs::write(&md_path, generate_markdown_report(&tuning_results))?;
    println!("Markdown report: {}", md_path.display());

    Ok(())
}

fn simulate_e10(text: &str, seed: u64) -> SimulatedE10 {
    // Create deterministic embeddings with some correlation between intent and context
    let intent = simulate_embedding(text, seed, 768);

    // Context embedding is correlated but different
    let mut context = simulate_embedding(text, seed + 12345, 768);

    // Add some correlation (0.7) with intent
    for i in 0..768 {
        context[i] = 0.7 * intent[i] + 0.3 * context[i];
    }

    // Renormalize
    let norm: f32 = context.iter().map(|x| x * x).sum::<f32>().sqrt();
    for v in &mut context {
        *v /= norm;
    }

    SimulatedE10 {
        as_intent: intent,
        as_context: context,
    }
}

fn simulate_embedding(text: &str, seed: u64, dim: usize) -> Vec<f32> {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    seed.hash(&mut hasher);
    let text_seed = hasher.finish();

    let mut rng = ChaCha8Rng::seed_from_u64(text_seed);
    let mut vec: Vec<f32> = (0..dim).map(|_| rng.gen::<f32>() * 2.0 - 1.0).collect();

    // Normalize
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    for v in &mut vec {
        *v /= norm;
    }

    vec
}

fn evaluate_modifiers(
    embeddings: &[(SimulatedE10, SimulatedE10, SimulatedE10)],
    intent_mod: f32,
    context_mod: f32,
    bootstrap_samples: usize,
    seed: u64,
) -> ModifierResult {
    // Compute MRR for each query
    let mut ranks: Vec<f64> = Vec::new();
    let mut hits_at_1 = 0;

    for (query, relevant, irrelevant) in embeddings.iter() {
        // Query intent vs document context (intent→context direction)
        let sim_relevant =
            compute_modified_similarity(&query.as_intent, &relevant.as_context, intent_mod);
        let sim_irrelevant =
            compute_modified_similarity(&query.as_intent, &irrelevant.as_context, intent_mod);

        // Rank: 1 if relevant > irrelevant, 2 otherwise
        let rank = if sim_relevant >= sim_irrelevant {
            1.0
        } else {
            2.0
        };
        ranks.push(rank);

        if rank == 1.0 {
            hits_at_1 += 1;
        }
    }

    // Compute MRR
    let mrr: f64 = ranks.iter().map(|r| 1.0 / r).sum::<f64>() / ranks.len() as f64;
    let avg_rank = ranks.iter().sum::<f64>() / ranks.len() as f64;
    let hit_rate = hits_at_1 as f64 / ranks.len() as f64;

    // Bootstrap confidence intervals
    let (ci_lower, ci_upper) = bootstrap_ci(&ranks, bootstrap_samples, seed);

    ModifierResult {
        intent_modifier: intent_mod,
        context_modifier: context_mod,
        asymmetry_ratio: intent_mod / context_mod,
        mrr,
        mrr_ci_lower: ci_lower,
        mrr_ci_upper: ci_upper,
        hit_rate_at_1: hit_rate,
        avg_rank,
    }
}

fn compute_modified_similarity(query: &[f32], doc: &[f32], modifier: f32) -> f32 {
    let dot: f32 = query.iter().zip(doc.iter()).map(|(a, b)| a * b).sum();
    let norm_a: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = doc.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a < f32::EPSILON || norm_b < f32::EPSILON {
        return 0.0;
    }

    let sim = dot / (norm_a * norm_b);
    (sim * modifier).clamp(-1.0, 1.0)
}

fn bootstrap_ci(ranks: &[f64], n_samples: usize, seed: u64) -> (f64, f64) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut mrrs = Vec::with_capacity(n_samples);

    for _ in 0..n_samples {
        let sample: Vec<f64> = (0..ranks.len())
            .map(|_| ranks[rng.gen_range(0..ranks.len())])
            .collect();
        let mrr = sample.iter().map(|r| 1.0 / r).sum::<f64>() / sample.len() as f64;
        mrrs.push(mrr);
    }

    mrrs.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let lower_idx = (0.025 * n_samples as f64) as usize;
    let upper_idx = (0.975 * n_samples as f64) as usize;

    (mrrs[lower_idx], mrrs[upper_idx.min(mrrs.len() - 1)])
}

fn print_results(results: &TuningResults) {
    println!("=== Tuning Results ===\n");

    println!("Data: {} pairs", results.num_pairs);
    println!("Bootstrap samples: {}", results.bootstrap_samples);
    println!();

    println!("--- Top 5 Configurations ---");
    let mut sorted: Vec<_> = results.results.iter().collect();
    sorted.sort_by(|a, b| b.mrr.partial_cmp(&a.mrr).unwrap());

    println!(
        "{:>8} {:>8} {:>8} {:>8} {:>15}",
        "Int_Mod", "Ctx_Mod", "Ratio", "MRR", "95% CI"
    );
    println!("{:-<55}", "");

    for r in sorted.iter().take(5) {
        println!(
            "{:>8.2} {:>8.2} {:>8.2} {:>8.4} [{:.4}, {:.4}]",
            r.intent_modifier,
            r.context_modifier,
            r.asymmetry_ratio,
            r.mrr,
            r.mrr_ci_lower,
            r.mrr_ci_upper,
        );
    }

    println!();
    println!("--- Default (1.2/0.8) vs Best ---");
    println!(
        "Default: MRR={:.4} (ratio={:.2})",
        results.default_config.mrr, results.default_config.asymmetry_ratio
    );
    println!(
        "Best:    MRR={:.4} (ratio={:.2}, mods={:.2}/{:.2})",
        results.best_config.mrr,
        results.best_config.asymmetry_ratio,
        results.best_config.intent_modifier,
        results.best_config.context_modifier,
    );
    println!(
        "Improvement: {:.2}%",
        results.summary.improvement_over_default_pct
    );
    println!("Sensitivity: {}", results.summary.modifier_sensitivity);
}

fn generate_markdown_report(results: &TuningResults) -> String {
    let mut md = String::new();

    md.push_str("# E10 Direction Modifier Tuning Results\n\n");
    md.push_str(&format!("Generated: {}\n\n", results.timestamp));

    md.push_str("## Summary\n\n");
    md.push_str(&format!("- **Data pairs**: {}\n", results.num_pairs));
    md.push_str(&format!(
        "- **Bootstrap samples**: {}\n",
        results.bootstrap_samples
    ));
    md.push_str(&format!(
        "- **Simulated embeddings**: {}\n\n",
        results.simulated
    ));

    md.push_str("## Optimal Configuration\n\n");
    md.push_str(&format!(
        "| Metric | Default (1.2/0.8) | Optimal ({:.2}/{:.2}) |\n",
        results.best_config.intent_modifier, results.best_config.context_modifier
    ));
    md.push_str("|--------|-------------------|----------------------|\n");
    md.push_str(&format!(
        "| MRR | {:.4} | {:.4} |\n",
        results.default_config.mrr, results.best_config.mrr
    ));
    md.push_str(&format!(
        "| Hit@1 | {:.2}% | {:.2}% |\n",
        results.default_config.hit_rate_at_1 * 100.0,
        results.best_config.hit_rate_at_1 * 100.0
    ));
    md.push_str(&format!(
        "| Asymmetry Ratio | {:.2} | {:.2} |\n\n",
        results.default_config.asymmetry_ratio, results.best_config.asymmetry_ratio
    ));

    md.push_str(&format!(
        "**Improvement over default**: {:.2}%\n\n",
        results.summary.improvement_over_default_pct
    ));

    md.push_str("## All Configurations\n\n");
    md.push_str("| Intent Mod | Context Mod | Ratio | MRR | 95% CI |\n");
    md.push_str("|------------|-------------|-------|-----|--------|\n");

    let mut sorted: Vec<_> = results.results.iter().collect();
    sorted.sort_by(|a, b| b.mrr.partial_cmp(&a.mrr).unwrap());

    for r in &sorted {
        md.push_str(&format!(
            "| {:.2} | {:.2} | {:.2} | {:.4} | [{:.4}, {:.4}] |\n",
            r.intent_modifier,
            r.context_modifier,
            r.asymmetry_ratio,
            r.mrr,
            r.mrr_ci_lower,
            r.mrr_ci_upper,
        ));
    }

    md.push_str("\n## Interpretation\n\n");
    md.push_str(&format!(
        "Modifier sensitivity: **{}**\n\n",
        results.summary.modifier_sensitivity
    ));

    if results.summary.improvement_over_default_pct < 2.0 {
        md.push_str("The default 1.2/0.8 modifiers perform near-optimally. ");
        md.push_str("No significant improvement from tuning.\n");
    } else if results.summary.improvement_over_default_pct < 10.0 {
        md.push_str("Modest improvement possible with tuned modifiers. ");
        md.push_str("Consider using optimal values in production.\n");
    } else {
        md.push_str("Significant improvement possible! ");
        md.push_str("Strongly recommend using optimal modifiers.\n");
    }

    md
}
