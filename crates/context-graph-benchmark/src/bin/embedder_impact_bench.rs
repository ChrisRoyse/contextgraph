//! Embedder Impact Benchmark CLI
//!
//! Measures each embedder's contribution to:
//! - Retrieval quality (MRR, P@K, R@K, NDCG)
//! - Knowledge graph structure
//! - Resource usage
//!
//! # Usage
//!
//! ```bash
//! # Quick benchmark (Tier 1)
//! cargo run -p context-graph-benchmark --bin embedder-impact-bench -- --tier 1
//!
//! # Standard benchmark with markdown output
//! cargo run -p context-graph-benchmark --bin embedder-impact-bench -- \
//!     --tier 2 --format markdown --output docs/EMBEDDER_IMPACT.md
//!
//! # Comprehensive benchmark
//! cargo run -p context-graph-benchmark --bin embedder-impact-bench -- \
//!     --tier 2 --components all --format markdown
//! ```

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use context_graph_benchmark::datasets::graph_linking::ScaleTier;
use context_graph_benchmark::metrics::causal::E5ImpactAnalysis;
use context_graph_benchmark::runners::embedder_impact::{
    EmbedderImpactConfig, EmbedderImpactResults, EmbedderImpactRunner, RecommendationType,
};
use context_graph_storage::teleological::indexes::EmbedderIndex;

/// Embedder Impact Benchmark CLI
#[derive(Parser, Debug)]
#[command(name = "embedder-impact-bench")]
#[command(about = "Benchmark per-embedder contribution to retrieval quality")]
struct Args {
    /// Tier level (1-6): 1=100, 2=1K, 3=10K, 4=100K, 5=1M, 6=10M
    #[arg(short, long, default_value = "1")]
    tier: u8,

    /// Additional tiers to run (comma-separated)
    #[arg(long, value_delimiter = ',')]
    extra_tiers: Option<Vec<u8>>,

    /// Components to benchmark: retrieval, graph, resource, all
    #[arg(short, long, default_value = "retrieval")]
    components: String,

    /// Output format: table, json, markdown
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Output file path (stdout if not specified)
    #[arg(short = 'O', long)]
    output: Option<PathBuf>,

    /// Random seed
    #[arg(short, long, default_value = "42")]
    seed: u64,

    /// Top K for retrieval
    #[arg(short = 'k', long, default_value = "10")]
    top_k: usize,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    // Parse args
    let args = Args::parse();

    // Setup logging
    let filter = if args.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    // Build tier list
    let mut tiers = vec![ScaleTier::from_level(args.tier)?];
    if let Some(extra) = &args.extra_tiers {
        for &level in extra {
            tiers.push(ScaleTier::from_level(level)?);
        }
    }

    // Parse components
    let run_graph = args.components == "graph" || args.components == "all";
    let run_resource = args.components == "resource" || args.components == "all";

    // Build config
    let config = EmbedderImpactConfig {
        tiers,
        queries_per_tier: None,
        k_values: vec![1, 5, 10, 20],
        seed: args.seed,
        run_blind_spot_analysis: true,
        run_graph_impact: run_graph,
        run_resource_analysis: run_resource,
        embedders: context_graph_benchmark::ablation::all_core_embedders(),
        baseline_embedder: EmbedderIndex::E1Semantic,
        min_similarity: 0.0,
        top_k: args.top_k,
    };

    tracing::info!("Starting embedder impact benchmark");
    tracing::info!("Tiers: {:?}", config.tiers);

    // Run benchmark
    let mut runner = EmbedderImpactRunner::new(config);
    let results = runner.run();

    tracing::info!("Benchmark completed in {}ms", results.metadata.duration_ms);

    // Format output
    let output = match args.format.as_str() {
        "json" => format_json(&results)?,
        "markdown" => format_markdown(&results),
        _ => format_table(&results),
    };

    // Write output
    if let Some(path) = args.output {
        let mut file = fs::File::create(&path)?;
        file.write_all(output.as_bytes())?;
        tracing::info!("Results written to {}", path.display());
    } else {
        println!("{}", output);
    }

    // Print compliance summary
    if !results.compliance.is_compliant {
        tracing::warn!("Constitutional compliance issues detected:");
        for note in &results.compliance.notes {
            tracing::warn!("  - {}", note);
        }
    }

    Ok(())
}

fn format_json(results: &EmbedderImpactResults) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(results)?)
}

fn format_table(results: &EmbedderImpactResults) -> String {
    let mut out = String::new();

    out.push_str("\n=== Embedder Impact Benchmark Results ===\n");
    out.push_str(&format!(
        "Duration: {}ms | Tiers: {:?}\n\n",
        results.metadata.duration_ms, results.metadata.config.tiers
    ));

    // Per-tier results
    for (tier, tier_results) in &results.tier_results {
        out.push_str(&format!(
            "--- Tier {} ({}  docs, {} queries) ---\n",
            tier.name(),
            tier_results.corpus_size,
            tier_results.query_count
        ));

        // Baseline vs Multispace
        out.push_str(&format!(
            "Baseline (E1 only):  MRR={:.3}, P@10={:.3}\n",
            tier_results.baseline_metrics.mrr,
            tier_results
                .baseline_metrics
                .precision_at
                .get(&10)
                .unwrap_or(&0.0)
        ));
        out.push_str(&format!(
            "Multispace (all 13): MRR={:.3}, P@10={:.3}\n\n",
            tier_results.multispace_metrics.mrr,
            tier_results
                .multispace_metrics
                .precision_at
                .get(&10)
                .unwrap_or(&0.0)
        ));

        // Ablation table
        out.push_str("Ablation Impact (removing each embedder):\n");
        out.push_str("+------------------+----------+----------+-----------+----------+\n");
        out.push_str("| Embedder         | Δ MRR    | Δ P@10   | Impact %  | Critical |\n");
        out.push_str("+------------------+----------+----------+-----------+----------+\n");

        let mut deltas: Vec<_> = tier_results.ablation_deltas.values().collect();
        deltas.sort_by(|a, b| {
            b.impact_pct
                .partial_cmp(&a.impact_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for delta in deltas {
            let critical = if delta.is_critical { "YES" } else { "no" };
            out.push_str(&format!(
                "| {:16} | {:+7.4} | {:+7.4} | {:8.2}% | {:8} |\n",
                embedder_short_name(delta.embedder),
                delta.mrr_delta,
                delta.p10_delta,
                delta.impact_pct,
                critical
            ));
        }
        out.push_str("+------------------+----------+----------+-----------+----------+\n\n");

        // Enhancement table
        out.push_str("Enhancement Impact (E1 + each embedder):\n");
        out.push_str("+------------------+----------+----------+-----------+\n");
        out.push_str("| Embedder         | Δ MRR    | Δ P@10   | Improve % |\n");
        out.push_str("+------------------+----------+----------+-----------+\n");

        let mut enhancements: Vec<_> = tier_results.enhancement_deltas.values().collect();
        enhancements.sort_by(|a, b| {
            b.improvement_pct
                .partial_cmp(&a.improvement_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for delta in enhancements {
            out.push_str(&format!(
                "| {:16} | {:+7.4} | {:+7.4} | {:8.2}% |\n",
                embedder_short_name(delta.embedder),
                delta.mrr_delta,
                delta.p10_delta,
                delta.improvement_pct
            ));
        }
        out.push_str("+------------------+----------+----------+-----------+\n\n");

        // Blind spots
        if let Some(ref blind) = tier_results.blind_spots {
            out.push_str("Blind Spot Analysis (what E1 missed):\n");
            out.push_str(&format!("  E1 found: {} docs\n", blind.e1_found_count));
            out.push_str(&format!(
                "  E1 blind spot %: {:.1}%\n",
                blind.e1_blind_spot_percentage()
            ));

            let sorted = blind.sorted_by_unique_finds();
            for unique in sorted.iter().take(5) {
                if unique.count > 0 {
                    out.push_str(&format!(
                        "  {} found {} unique relevant ({:.1}% of total)\n",
                        embedder_short_name(unique.embedder),
                        unique.count,
                        unique.pct_of_total
                    ));
                }
            }
            out.push('\n');
        }

        // E5 Causal Embedder Analysis
        if let Some(ref e5) = tier_results.e5_analysis {
            out.push_str(&format_e5_table_section(e5));
        }
    }

    // Recommendations
    out.push_str("=== Recommendations ===\n");
    for rec in &results.recommendations {
        let priority = match rec.priority {
            1 => "[HIGH]",
            2 => "[MEDIUM]",
            _ => "[LOW]",
        };
        let rec_type = match rec.recommendation_type {
            RecommendationType::Essential => "ESSENTIAL",
            RecommendationType::Valuable => "VALUABLE",
            RecommendationType::Redundant => "REDUNDANT",
            RecommendationType::BlindSpotFinder => "BLIND_SPOT",
            RecommendationType::Performance => "PERF",
        };
        out.push_str(&format!(
            "{} {} - {}\n",
            priority, rec_type, rec.description
        ));
    }

    // Compliance
    out.push_str(&format!(
        "\n=== Constitutional Compliance: {} ===\n",
        if results.compliance.is_compliant {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "  ARCH-09 (topic threshold >= 2.5): {}\n",
        if results.compliance.arch09_topic_threshold {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "  ARCH-12 (E1 is foundation): {}\n",
        if results.compliance.arch12_e1_foundation {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "  ARCH-21 (uses Weighted RRF): {}\n",
        if results.compliance.arch21_weighted_rrf {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "  AP-60 (temporal 0 topic impact): {}\n",
        if results.compliance.ap60_temporal_zero_topic {
            "PASS"
        } else {
            "FAIL"
        }
    ));

    out
}

fn format_markdown(results: &EmbedderImpactResults) -> String {
    let mut out = String::new();

    out.push_str("# Embedder Impact Benchmark Report\n\n");
    out.push_str(&format!(
        "**Duration:** {}ms | **Tiers:** {:?}\n\n",
        results.metadata.duration_ms, results.metadata.config.tiers
    ));

    out.push_str("## Summary\n\n");
    out.push_str(
        "This benchmark measures how each of the 13 embedders impacts retrieval quality.\n\n",
    );

    // Per-tier results
    for (tier, tier_results) in &results.tier_results {
        out.push_str(&format!(
            "## Tier {} ({} docs, {} queries)\n\n",
            tier.name(),
            tier_results.corpus_size,
            tier_results.query_count
        ));

        out.push_str("### Baseline vs Multi-Space\n\n");
        out.push_str("| Configuration | MRR | P@10 | R@10 | NDCG@10 |\n");
        out.push_str("|---------------|-----|------|------|--------|\n");
        out.push_str(&format!(
            "| E1 Only | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            tier_results.baseline_metrics.mrr,
            tier_results
                .baseline_metrics
                .precision_at
                .get(&10)
                .unwrap_or(&0.0),
            tier_results
                .baseline_metrics
                .recall_at
                .get(&10)
                .unwrap_or(&0.0),
            tier_results
                .baseline_metrics
                .ndcg_at
                .get(&10)
                .unwrap_or(&0.0)
        ));
        out.push_str(&format!(
            "| All 13 | {:.3} | {:.3} | {:.3} | {:.3} |\n\n",
            tier_results.multispace_metrics.mrr,
            tier_results
                .multispace_metrics
                .precision_at
                .get(&10)
                .unwrap_or(&0.0),
            tier_results
                .multispace_metrics
                .recall_at
                .get(&10)
                .unwrap_or(&0.0),
            tier_results
                .multispace_metrics
                .ndcg_at
                .get(&10)
                .unwrap_or(&0.0)
        ));

        // Ablation table
        out.push_str("### Ablation Impact (removing each embedder)\n\n");
        out.push_str("| Embedder | Δ MRR | Δ P@10 | Impact % | Critical? |\n");
        out.push_str("|----------|-------|--------|----------|----------|\n");

        let mut deltas: Vec<_> = tier_results.ablation_deltas.values().collect();
        deltas.sort_by(|a, b| {
            b.impact_pct
                .partial_cmp(&a.impact_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for delta in deltas {
            let critical = if delta.is_critical { "**YES**" } else { "no" };
            out.push_str(&format!(
                "| {} | {:+.4} | {:+.4} | {:.2}% | {} |\n",
                embedder_short_name(delta.embedder),
                delta.mrr_delta,
                delta.p10_delta,
                delta.impact_pct,
                critical
            ));
        }
        out.push('\n');

        // Enhancement table
        out.push_str("### Enhancement Impact (E1 + each embedder)\n\n");
        out.push_str("| Embedder | Δ MRR | Δ P@10 | Improvement % |\n");
        out.push_str("|----------|-------|--------|---------------|\n");

        let mut enhancements: Vec<_> = tier_results.enhancement_deltas.values().collect();
        enhancements.sort_by(|a, b| {
            b.improvement_pct
                .partial_cmp(&a.improvement_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for delta in enhancements {
            out.push_str(&format!(
                "| {} | {:+.4} | {:+.4} | {:.2}% |\n",
                embedder_short_name(delta.embedder),
                delta.mrr_delta,
                delta.p10_delta,
                delta.improvement_pct
            ));
        }
        out.push('\n');

        // Blind spots
        if let Some(ref blind) = tier_results.blind_spots {
            out.push_str("### Blind Spot Analysis\n\n");
            out.push_str(&format!(
                "E1 would miss **{:.1}%** of relevant documents without enhancers.\n\n",
                blind.e1_blind_spot_percentage()
            ));

            out.push_str("| Embedder | Unique Finds | Avg Relevance | % of Total |\n");
            out.push_str("|----------|--------------|---------------|------------|\n");

            for unique in blind.sorted_by_unique_finds() {
                if unique.count > 0 {
                    out.push_str(&format!(
                        "| {} | {} | {:.2} | {:.1}% |\n",
                        embedder_short_name(unique.embedder),
                        unique.count,
                        unique.avg_relevance,
                        unique.pct_of_total
                    ));
                }
            }
            out.push('\n');
        }

        // E5 Causal Embedder Analysis
        if let Some(ref e5) = tier_results.e5_analysis {
            out.push_str(&format_e5_markdown_section(e5));
        }
    }

    // Recommendations
    out.push_str("## Recommendations\n\n");
    for rec in &results.recommendations {
        let emoji = match rec.priority {
            1 => "🔴",
            2 => "🟡",
            _ => "🟢",
        };
        out.push_str(&format!(
            "{} **{}**: {}\n\n",
            emoji,
            match rec.recommendation_type {
                RecommendationType::Essential => "Essential",
                RecommendationType::Valuable => "Valuable",
                RecommendationType::Redundant => "Redundant",
                RecommendationType::BlindSpotFinder => "Blind Spot Finder",
                RecommendationType::Performance => "Performance",
            },
            rec.description
        ));
    }

    // Compliance
    out.push_str("## Constitutional Compliance\n\n");
    let status = if results.compliance.is_compliant {
        "✅ PASS"
    } else {
        "❌ FAIL"
    };
    out.push_str(&format!("**Overall:** {}\n\n", status));
    out.push_str("| Rule | Status |\n");
    out.push_str("|------|--------|\n");
    out.push_str(&format!(
        "| ARCH-09: Topic threshold >= 2.5 | {} |\n",
        if results.compliance.arch09_topic_threshold {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| ARCH-12: E1 is foundation | {} |\n",
        if results.compliance.arch12_e1_foundation {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| ARCH-21: Uses Weighted RRF | {} |\n",
        if results.compliance.arch21_weighted_rrf {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| AP-60: Temporal 0 topic impact | {} |\n",
        if results.compliance.ap60_temporal_zero_topic {
            "✅"
        } else {
            "❌"
        }
    ));

    if !results.compliance.notes.is_empty() {
        out.push_str("\n**Notes:**\n");
        for note in &results.compliance.notes {
            out.push_str(&format!("- {}\n", note));
        }
    }

    out.push_str(&format!(
        "\n---\n*Generated by embedder-impact-bench at {}*\n",
        results.metadata.started_at
    ));

    out
}

fn embedder_short_name(embedder: EmbedderIndex) -> &'static str {
    match embedder {
        EmbedderIndex::E1Semantic => "E1_Semantic",
        EmbedderIndex::E1Matryoshka128 => "E1_M128",
        EmbedderIndex::E2TemporalRecent => "E2_Recent",
        EmbedderIndex::E3TemporalPeriodic => "E3_Periodic",
        EmbedderIndex::E4TemporalPositional => "E4_Positional",
        EmbedderIndex::E5Causal => "E5_Causal",
        EmbedderIndex::E5CausalCause => "E5_Cause",
        EmbedderIndex::E5CausalEffect => "E5_Effect",
        EmbedderIndex::E6Sparse => "E6_Sparse",
        EmbedderIndex::E7Code => "E7_Code",
        EmbedderIndex::E8Graph => "E8_Graph",
        EmbedderIndex::E9HDC => "E9_HDC",
        EmbedderIndex::E10Multimodal => "E10_Multi",
        EmbedderIndex::E10MultimodalParaphrase => "E10_Paraphrase",
        EmbedderIndex::E10MultimodalContext => "E10_Context",
        EmbedderIndex::E11Entity => "E11_Entity",
        EmbedderIndex::E12LateInteraction => "E12_LateInt",
        EmbedderIndex::E13Splade => "E13_SPLADE",
        EmbedderIndex::E14BgeM3Dense => "E14_BGE_M3",
    }
}

// =============================================================================
// E5 CAUSAL EMBEDDER ANALYSIS OUTPUT (per ARCH-18, AP-77)
// =============================================================================

/// Format E5 analysis section for table output.
fn format_e5_table_section(e5: &E5ImpactAnalysis) -> String {
    let mut out = String::new();

    out.push_str("\n╔═══════════════════════════════════════════════════════════════╗\n");
    out.push_str("║              E5 CAUSAL EMBEDDER ANALYSIS                      ║\n");
    out.push_str("╚═══════════════════════════════════════════════════════════════╝\n\n");

    // Vector Verification
    out.push_str("┌─ Vector Verification ─────────────────────────────────────────┐\n");
    out.push_str(&format!(
        "│ Distinct vectors: {:>6.1}% ({}/{})                         │\n",
        e5.vector_verification.distinct_vector_pct,
        e5.vector_verification.docs_with_distinct_vectors,
        e5.vector_verification.total_docs_checked
    ));
    out.push_str(&format!(
        "│ Avg cause/effect distance: {:>6.3}                          │\n",
        e5.vector_verification.avg_cause_effect_distance
    ));
    out.push_str(&format!(
        "│ Min distance: {:>6.3} (threshold: 0.300)  {}                │\n",
        e5.vector_verification.min_cause_effect_distance,
        if e5.vector_verification.min_cause_effect_distance >= 0.3 {
            "✓"
        } else {
            "✗"
        }
    ));
    out.push_str(&format!(
        "│ Threshold violations: {:>4}                                  │\n",
        e5.vector_verification.threshold_violations
    ));
    out.push_str("└────────────────────────────────────────────────────────────────┘\n\n");

    // Direction Distribution
    out.push_str("┌─ Query Direction Distribution ────────────────────────────────┐\n");
    out.push_str(&format!(
        "│ Cause:   {:>5.1}% (target: 40%)  {}                          │\n",
        e5.direction_distribution.actual_cause_pct,
        if (e5.direction_distribution.actual_cause_pct - 40.0).abs() <= 5.0 {
            "✓"
        } else {
            "✗"
        }
    ));
    out.push_str(&format!(
        "│ Effect:  {:>5.1}% (target: 40%)  {}                          │\n",
        e5.direction_distribution.actual_effect_pct,
        if (e5.direction_distribution.actual_effect_pct - 40.0).abs() <= 5.0 {
            "✓"
        } else {
            "✗"
        }
    ));
    out.push_str(&format!(
        "│ Unknown: {:>5.1}% (target: 20%)  {}                          │\n",
        e5.direction_distribution.actual_unknown_pct,
        if (e5.direction_distribution.actual_unknown_pct - 20.0).abs() <= 5.0 {
            "✓"
        } else {
            "✗"
        }
    ));
    out.push_str("└────────────────────────────────────────────────────────────────┘\n\n");

    // Asymmetric vs Symmetric Comparison
    out.push_str("┌─ Asymmetric vs Symmetric Retrieval ───────────────────────────┐\n");
    out.push_str(&format!(
        "│ Symmetric MRR:  {:>7.4}                                     │\n",
        e5.asymmetric_comparison.symmetric_mrr
    ));
    out.push_str(&format!(
        "│ Asymmetric MRR: {:>7.4}                                     │\n",
        e5.asymmetric_comparison.asymmetric_mrr
    ));
    out.push_str(&format!(
        "│ Improvement:    {:>+6.1}%  {}                                │\n",
        e5.asymmetric_comparison.mrr_improvement_pct,
        if e5.asymmetric_comparison.mrr_improvement_pct > 10.0 {
            "✓"
        } else {
            "✗"
        }
    ));
    out.push_str(&format!(
        "│ Asymmetric wins: {:>5.1}%                                    │\n",
        e5.asymmetric_comparison.asymmetric_wins_pct
    ));
    out.push_str("└────────────────────────────────────────────────────────────────┘\n\n");

    // Asymmetry Ratio
    out.push_str("┌─ Asymmetry Ratio Analysis ────────────────────────────────────┐\n");
    out.push_str(&format!(
        "│ Observed ratio: {:>5.2} (target: 1.50)  {}                   │\n",
        e5.observed_asymmetry_ratio,
        if (e5.observed_asymmetry_ratio - 1.5).abs() <= 0.2 {
            "✓"
        } else {
            "✗"
        }
    ));
    out.push_str(&format!(
        "│ Cause MRR:   {:>7.4}                                        │\n",
        e5.direction_mrr.cause_mrr
    ));
    out.push_str(&format!(
        "│ Effect MRR:  {:>7.4}                                        │\n",
        e5.direction_mrr.effect_mrr
    ));
    out.push_str(&format!(
        "│ Unknown MRR: {:>7.4}                                        │\n",
        e5.direction_mrr.unknown_mrr
    ));
    out.push_str("└────────────────────────────────────────────────────────────────┘\n\n");

    // Constitutional Compliance
    let compliance = e5.verify_compliance();
    out.push_str("┌─ E5 Constitutional Compliance ────────────────────────────────┐\n");
    out.push_str(&format!(
        "│ ARCH-18 (asymmetric similarity):    {}                       │\n",
        if compliance.arch18_asymmetric {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "│ AP-77 (no symmetric alone):         {}                       │\n",
        if compliance.ap77_no_symmetric {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "│ Direction modifiers correct:        {}                       │\n",
        if compliance.direction_modifiers_correct {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "│ Asymmetry ratio valid (1.3-1.7):    {}                       │\n",
        if compliance.asymmetry_ratio_valid {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str(&format!(
        "│ Overall E5 Compliance:              {}                       │\n",
        if compliance.all_pass() {
            "PASS"
        } else {
            "FAIL"
        }
    ));
    out.push_str("└────────────────────────────────────────────────────────────────┘\n");

    out
}

/// Format E5 analysis section for markdown output.
fn format_e5_markdown_section(e5: &E5ImpactAnalysis) -> String {
    let mut out = String::new();

    out.push_str("### E5 Causal Embedder Analysis\n\n");
    out.push_str("Per ARCH-18, AP-77: E5 uses asymmetric similarity with direction modifiers.\n\n");

    // Vector Verification
    out.push_str("#### Vector Verification\n\n");
    out.push_str("| Metric | Value | Status |\n");
    out.push_str("|--------|-------|--------|\n");
    out.push_str(&format!(
        "| Distinct vectors | {:.1}% ({}/{}) | {} |\n",
        e5.vector_verification.distinct_vector_pct,
        e5.vector_verification.docs_with_distinct_vectors,
        e5.vector_verification.total_docs_checked,
        if e5.vector_verification.threshold_violations == 0 {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Avg cause/effect distance | {:.3} | - |\n",
        e5.vector_verification.avg_cause_effect_distance
    ));
    out.push_str(&format!(
        "| Min distance (threshold: 0.3) | {:.3} | {} |\n",
        e5.vector_verification.min_cause_effect_distance,
        if e5.vector_verification.min_cause_effect_distance >= 0.3 {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Threshold violations | {} | {} |\n\n",
        e5.vector_verification.threshold_violations,
        if e5.vector_verification.threshold_violations == 0 {
            "✅"
        } else {
            "❌"
        }
    ));

    // Direction Distribution
    out.push_str("#### Query Direction Distribution (Target: 40/40/20)\n\n");
    out.push_str("| Direction | Actual | Target | Status |\n");
    out.push_str("|-----------|--------|--------|--------|\n");
    out.push_str(&format!(
        "| Cause | {:.1}% | 40% | {} |\n",
        e5.direction_distribution.actual_cause_pct,
        if (e5.direction_distribution.actual_cause_pct - 40.0).abs() <= 5.0 {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Effect | {:.1}% | 40% | {} |\n",
        e5.direction_distribution.actual_effect_pct,
        if (e5.direction_distribution.actual_effect_pct - 40.0).abs() <= 5.0 {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Unknown | {:.1}% | 20% | {} |\n\n",
        e5.direction_distribution.actual_unknown_pct,
        if (e5.direction_distribution.actual_unknown_pct - 20.0).abs() <= 5.0 {
            "✅"
        } else {
            "❌"
        }
    ));

    // Asymmetric vs Symmetric
    out.push_str("#### Asymmetric vs Symmetric Retrieval\n\n");
    out.push_str("| Metric | Value | Target | Status |\n");
    out.push_str("|--------|-------|--------|--------|\n");
    out.push_str(&format!(
        "| Symmetric MRR | {:.4} | - | - |\n",
        e5.asymmetric_comparison.symmetric_mrr
    ));
    out.push_str(&format!(
        "| Asymmetric MRR | {:.4} | - | - |\n",
        e5.asymmetric_comparison.asymmetric_mrr
    ));
    out.push_str(&format!(
        "| MRR Improvement | {:+.1}% | >10% | {} |\n",
        e5.asymmetric_comparison.mrr_improvement_pct,
        if e5.asymmetric_comparison.mrr_improvement_pct > 10.0 {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Asymmetric wins | {:.1}% | >50% | {} |\n\n",
        e5.asymmetric_comparison.asymmetric_wins_pct,
        if e5.asymmetric_comparison.asymmetric_wins_pct > 50.0 {
            "✅"
        } else {
            "❌"
        }
    ));

    // Direction MRR Breakdown
    out.push_str("#### Asymmetry Ratio Analysis\n\n");
    out.push_str(&format!(
        "**Observed ratio: {:.2}** (target: 1.50 ± 0.2) {}\n\n",
        e5.observed_asymmetry_ratio,
        if (e5.observed_asymmetry_ratio - 1.5).abs() <= 0.2 {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str("| Direction | MRR |\n");
    out.push_str("|-----------|-----|\n");
    out.push_str(&format!("| Cause | {:.4} |\n", e5.direction_mrr.cause_mrr));
    out.push_str(&format!(
        "| Effect | {:.4} |\n",
        e5.direction_mrr.effect_mrr
    ));
    out.push_str(&format!(
        "| Unknown | {:.4} |\n\n",
        e5.direction_mrr.unknown_mrr
    ));

    // Constitutional Compliance
    let compliance = e5.verify_compliance();
    out.push_str("#### E5 Constitutional Compliance\n\n");
    out.push_str(&format!(
        "**Overall: {}**\n\n",
        if compliance.all_pass() {
            "✅ PASS"
        } else {
            "❌ FAIL"
        }
    ));
    out.push_str("| Rule | Status |\n");
    out.push_str("|------|--------|\n");
    out.push_str(&format!(
        "| ARCH-18: E5 uses asymmetric similarity | {} |\n",
        if compliance.arch18_asymmetric {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| AP-77: E5 not symmetric alone | {} |\n",
        if compliance.ap77_no_symmetric {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Direction modifiers correct | {} |\n",
        if compliance.direction_modifiers_correct {
            "✅"
        } else {
            "❌"
        }
    ));
    out.push_str(&format!(
        "| Asymmetry ratio valid (1.3-1.7) | {} |\n\n",
        if compliance.asymmetry_ratio_valid {
            "✅"
        } else {
            "❌"
        }
    ));

    out
}
