//! Markdown report generation for documentation.
//!
//! Produces human-readable markdown tables and analysis summaries.

use crate::runners::BenchmarkResults;

/// Generate Markdown report from benchmark results.
pub fn generate_markdown(results: &BenchmarkResults) -> String {
    let mut md = String::new();

    // Title and summary
    md.push_str("# Benchmark Results: 13-Embedder Multi-Space vs Single-Embedder RAG\n\n");
    md.push_str(&format!(
        "**Generated:** {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));

    // Executive summary
    md.push_str("## Executive Summary\n\n");
    md.push_str(&format!("**Winner:** {}\n\n", results.comparison.winner));

    if let Some(saf) = results.comparison.scaling_advantage_factor {
        md.push_str(&format!("**Scaling Advantage Factor:** {:.2}x\n\n", saf));
    }

    md.push_str(&format!(
        "**Overall Improvement:** {:.1}%\n\n",
        results.comparison.overall_improvement * 100.0
    ));

    if let Some(mem_overhead) = results.comparison.memory_overhead_factor {
        md.push_str(&format!("**Memory Overhead:** {:.2}x\n\n", mem_overhead));
    }

    // Key findings
    md.push_str("### Key Findings\n\n");
    write_key_findings(&mut md, results);

    // Detailed results table
    md.push_str("## Detailed Results by Tier\n\n");
    write_tier_table(&mut md, results);

    // Retrieval metrics comparison
    md.push_str("## Retrieval Quality Metrics\n\n");
    write_retrieval_table(&mut md, results);

    // Clustering metrics comparison
    md.push_str("## Clustering Quality Metrics\n\n");
    write_clustering_table(&mut md, results);

    // Scaling analysis
    md.push_str("## Scaling Analysis\n\n");
    write_scaling_analysis(&mut md, results);

    // Breaking points
    md.push_str("## Breaking Points\n\n");
    write_breaking_points(&mut md, results);

    // Recommendations
    md.push_str("## Recommendations\n\n");
    write_recommendations(&mut md, results);

    // Methodology
    md.push_str("## Methodology\n\n");
    write_methodology(&mut md, results);

    md
}

fn write_key_findings(md: &mut String, results: &BenchmarkResults) {
    let tier_map = results.tier_results_map();

    // Calculate average improvements
    let mut avg_p10 = 0.0;
    let mut avg_r10 = 0.0;
    let mut avg_mrr = 0.0;
    let mut count = 0;

    for summary in tier_map.values() {
        avg_p10 += summary.improvement.precision_10;
        avg_r10 += summary.improvement.recall_10;
        avg_mrr += summary.improvement.mrr;
        count += 1;
    }

    if count > 0 {
        avg_p10 /= count as f64;
        avg_r10 /= count as f64;
        avg_mrr /= count as f64;
    }

    md.push_str(&format!(
        "1. Multi-space provides **{:.1}%** average improvement in Precision@10\n",
        avg_p10
    ));
    md.push_str(&format!(
        "2. Multi-space provides **{:.1}%** average improvement in Recall@10\n",
        avg_r10
    ));
    md.push_str(&format!(
        "3. Multi-space provides **{:.1}%** average improvement in MRR\n",
        avg_mrr
    ));

    if let Some(saf) = results.comparison.scaling_advantage_factor {
        if saf > 1.0 {
            md.push_str(&format!(
                "4. Multi-space maintains accuracy **{:.1}x longer** as corpus grows\n",
                saf
            ));
        }
    }

    md.push('\n');
}

fn write_tier_table(md: &mut String, results: &BenchmarkResults) {
    md.push_str("| Tier | Corpus Size | Topics | Single P@10 | Multi P@10 | Improvement |\n");
    md.push_str("|------|-------------|--------|-------------|------------|-------------|\n");

    let tier_map = results.tier_results_map();
    let mut tiers: Vec<_> = tier_map.into_iter().collect();
    tiers.sort_by_key(|(name, _)| name.clone());

    for (tier_name, summary) in tiers {
        let tier = match tier_name.as_str() {
            "Tier0" => crate::config::Tier::Tier0,
            "Tier1" => crate::config::Tier::Tier1,
            "Tier2" => crate::config::Tier::Tier2,
            "Tier3" => crate::config::Tier::Tier3,
            "Tier4" => crate::config::Tier::Tier4,
            "Tier5" => crate::config::Tier::Tier5,
            _ => crate::config::Tier::Tier0,
        };
        let config = crate::config::TierConfig::for_tier(tier);

        md.push_str(&format!(
            "| {} | {} | {} | {:.3} | {:.3} | {:+.1}% |\n",
            tier_name,
            format_number(config.memory_count),
            config.topic_count,
            summary.single_embedder.precision_10,
            summary.multi_space.precision_10,
            summary.improvement.precision_10,
        ));
    }

    md.push('\n');
}

fn write_retrieval_table(md: &mut String, results: &BenchmarkResults) {
    md.push_str("### Single-Embedder Baseline\n\n");
    md.push_str("| Tier | P@10 | R@10 | MRR | NDCG@10 |\n");
    md.push_str("|------|------|------|-----|----------|\n");

    let tier_map = results.tier_results_map();
    let mut tiers: Vec<_> = tier_map.iter().collect();
    tiers.sort_by_key(|(name, _)| (*name).clone());

    for (tier_name, summary) in &tiers {
        md.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            tier_name,
            summary.single_embedder.precision_10,
            summary.single_embedder.recall_10,
            summary.single_embedder.mrr,
            summary.single_embedder.ndcg_10,
        ));
    }

    md.push_str("\n### Multi-Space (13 Embedders)\n\n");
    md.push_str("| Tier | P@10 | R@10 | MRR | NDCG@10 |\n");
    md.push_str("|------|------|------|-----|----------|\n");

    for (tier_name, summary) in &tiers {
        md.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            tier_name,
            summary.multi_space.precision_10,
            summary.multi_space.recall_10,
            summary.multi_space.mrr,
            summary.multi_space.ndcg_10,
        ));
    }

    md.push('\n');
}

fn write_clustering_table(md: &mut String, results: &BenchmarkResults) {
    md.push_str("| Tier | Single Purity | Multi Purity | Single NMI | Multi NMI |\n");
    md.push_str("|------|---------------|--------------|------------|----------|\n");

    let tier_map = results.tier_results_map();
    let mut tiers: Vec<_> = tier_map.into_iter().collect();
    tiers.sort_by_key(|(name, _)| name.clone());

    for (tier_name, summary) in tiers {
        md.push_str(&format!(
            "| {} | {:.3} | {:.3} | {:.3} | {:.3} |\n",
            tier_name,
            summary.single_embedder.purity,
            summary.multi_space.purity,
            summary.single_embedder.nmi,
            summary.multi_space.nmi,
        ));
    }

    md.push('\n');
}

fn write_scaling_analysis(md: &mut String, results: &BenchmarkResults) {
    md.push_str("### Degradation Rates\n\n");
    md.push_str("Performance degradation per 10x corpus increase:\n\n");

    if let Some(ref deg) = results.single_embedder.degradation {
        md.push_str("**Single-Embedder:**\n");
        md.push_str(&format!(
            "- Precision@10: {:.1}% drop per 10x\n",
            deg.degradation_rate.precision_10_per_10x * 100.0
        ));
        md.push_str(&format!(
            "- Recall@10: {:.1}% drop per 10x\n",
            deg.degradation_rate.recall_10_per_10x * 100.0
        ));
        md.push_str(&format!(
            "- MRR: {:.1}% drop per 10x\n\n",
            deg.degradation_rate.mrr_per_10x * 100.0
        ));
    }

    if let Some(ref deg) = results.multi_space.degradation {
        md.push_str("**Multi-Space:**\n");
        md.push_str(&format!(
            "- Precision@10: {:.1}% drop per 10x\n",
            deg.degradation_rate.precision_10_per_10x * 100.0
        ));
        md.push_str(&format!(
            "- Recall@10: {:.1}% drop per 10x\n",
            deg.degradation_rate.recall_10_per_10x * 100.0
        ));
        md.push_str(&format!(
            "- MRR: {:.1}% drop per 10x\n\n",
            deg.degradation_rate.mrr_per_10x * 100.0
        ));
    }
}

fn write_breaking_points(md: &mut String, results: &BenchmarkResults) {
    md.push_str("Corpus size where performance drops below 80% of baseline:\n\n");

    md.push_str("| Metric | Single-Embedder | Multi-Space |\n");
    md.push_str("|--------|-----------------|-------------|\n");

    let single_bp = results
        .single_embedder
        .degradation
        .as_ref()
        .map(|d| &d.limits);
    let multi_bp = results.multi_space.degradation.as_ref().map(|d| &d.limits);

    let format_bp = |bp: Option<usize>| -> String {
        bp.map(format_number)
            .unwrap_or_else(|| "No limit".to_string())
    };

    md.push_str(&format!(
        "| P@10 | {} | {} |\n",
        format_bp(single_bp.and_then(|b| b.precision_10_limit)),
        format_bp(multi_bp.and_then(|b| b.precision_10_limit)),
    ));

    md.push_str(&format!(
        "| R@10 | {} | {} |\n",
        format_bp(single_bp.and_then(|b| b.recall_10_limit)),
        format_bp(multi_bp.and_then(|b| b.recall_10_limit)),
    ));

    md.push_str(&format!(
        "| MRR | {} | {} |\n",
        format_bp(single_bp.and_then(|b| b.mrr_limit)),
        format_bp(multi_bp.and_then(|b| b.mrr_limit)),
    ));

    md.push_str(&format!(
        "| **Overall** | {} | {} |\n",
        format_bp(single_bp.and_then(|b| b.overall_limit)),
        format_bp(multi_bp.and_then(|b| b.overall_limit)),
    ));

    md.push('\n');
}

fn write_recommendations(md: &mut String, results: &BenchmarkResults) {
    if results.multi_space_wins() {
        md.push_str("Based on the benchmark results, **multi-space fingerprinting is recommended** for:\n\n");
        md.push_str("1. Applications requiring high retrieval accuracy\n");
        md.push_str("2. Large-scale deployments where accuracy must be maintained\n");
        md.push_str("3. Topic-sensitive retrieval where cluster separation matters\n\n");

        md.push_str("**Consider single-embedder** when:\n\n");
        md.push_str("- Memory constraints are severe\n");
        md.push_str("- Corpus size is small (<1,000 documents)\n");
        md.push_str("- Embedding latency is critical\n");
    } else {
        md.push_str(
            "Based on the benchmark results, the systems perform comparably. Consider:\n\n",
        );
        md.push_str("- Single-embedder for simpler deployment\n");
        md.push_str("- Multi-space for additional robustness at scale\n");
    }

    md.push('\n');
}

/// Format a number with thousands separators.
fn format_number(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(c);
    }
    result.chars().rev().collect()
}

fn write_methodology(md: &mut String, results: &BenchmarkResults) {
    md.push_str("### Configuration\n\n");
    md.push_str(&format!("- **Random Seed:** {}\n", results.config.seed));
    md.push_str(&format!(
        "- **Tiers Tested:** {}\n",
        results
            .config
            .tiers
            .iter()
            .map(|t| t.tier.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    md.push_str(&format!("- **K Values:** {:?}\n", results.config.k_values));
    md.push_str(&format!(
        "- **Warmup Iterations:** {}\n",
        results.config.warmup_iterations
    ));

    md.push_str("\n### Synthetic Data Generation\n\n");
    md.push_str("- Topics generated with controlled inter-topic distance\n");
    md.push_str("- Documents sampled from topic centroids with Gaussian noise\n");
    md.push_str("- Ground truth labels preserve topic assignment\n");
    md.push_str("- Queries generated with known relevant document sets\n\n");

    md.push_str("### Metrics\n\n");
    md.push_str("- **P@K:** Precision at position K\n");
    md.push_str("- **R@K:** Recall at position K\n");
    md.push_str("- **MRR:** Mean Reciprocal Rank\n");
    md.push_str("- **NDCG@K:** Normalized Discounted Cumulative Gain at K\n");
    md.push_str("- **Purity:** Cluster purity (max class per cluster)\n");
    md.push_str("- **NMI:** Normalized Mutual Information\n");
}
