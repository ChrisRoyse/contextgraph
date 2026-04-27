//! Markdown report generation for unified benchmark results.
//!
//! Generates human-readable reports from benchmark results.

use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::config::EmbedderName;
use super::results::UnifiedBenchmarkResults;

/// Report generator for unified benchmark results.
pub struct ReportGenerator {
    results: UnifiedBenchmarkResults,
}

impl ReportGenerator {
    /// Create a new report generator.
    pub fn new(results: UnifiedBenchmarkResults) -> Self {
        Self { results }
    }

    /// Generate full markdown report.
    pub fn generate_markdown(&self) -> String {
        let mut report = String::new();

        self.write_header(&mut report);
        self.write_summary(&mut report);
        self.write_dataset_info(&mut report);
        self.write_per_embedder_results(&mut report);
        self.write_fusion_results(&mut report);
        self.write_cross_embedder_analysis(&mut report);
        self.write_ablation_results(&mut report);
        self.write_constitutional_compliance(&mut report);
        self.write_recommendations(&mut report);
        self.write_footer(&mut report);

        report
    }

    /// Write report to file.
    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<()> {
        let report = self.generate_markdown();
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(report.as_bytes())?;
        Ok(())
    }

    fn write_header(&self, out: &mut String) {
        writeln!(out, "# Unified Real Data Benchmark Report").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "**Generated:** {}",
            self.results
                .metadata
                .end_time
                .format("%Y-%m-%d %H:%M:%S UTC")
        )
        .unwrap();
        writeln!(
            out,
            "**Duration:** {:.1}s",
            self.results.metadata.duration_secs
        )
        .unwrap();
        if let Some(ref commit) = self.results.metadata.git_commit {
            writeln!(out, "**Git Commit:** `{}`", &commit[..8.min(commit.len())]).unwrap();
        }
        writeln!(out).unwrap();
    }

    fn write_summary(&self, out: &mut String) {
        writeln!(out, "## Executive Summary").unwrap();
        writeln!(out).unwrap();

        // Find best embedder
        let best = self
            .results
            .per_embedder_results
            .iter()
            .max_by(|a, b| a.1.mrr_at_10.partial_cmp(&b.1.mrr_at_10).unwrap());

        if let Some((embedder, results)) = best {
            writeln!(
                out,
                "- **Best Single Embedder:** {} (MRR@10: {:.3})",
                embedder, results.mrr_at_10
            )
            .unwrap();
        }

        // E1 baseline
        if let Some(e1) = self
            .results
            .per_embedder_results
            .get(&EmbedderName::E1Semantic)
        {
            writeln!(
                out,
                "- **E1 Foundation Baseline:** MRR@10: {:.3}",
                e1.mrr_at_10
            )
            .unwrap();
        }

        // Best fusion
        if let Some(ref fusion) = self.results.fusion_results {
            writeln!(
                out,
                "- **Best Fusion Strategy:** {:?} (MRR@10: {:.3}, +{:.1}% over E1)",
                fusion.best_strategy,
                fusion
                    .by_strategy
                    .get(&fusion.best_strategy)
                    .map(|r| r.mrr_at_10)
                    .unwrap_or(0.0),
                fusion.improvement_over_baseline * 100.0
            )
            .unwrap();
        }

        // Constitutional compliance
        let compliance = &self.results.constitutional_compliance;
        let status = if compliance.all_passed {
            "PASS"
        } else {
            "FAIL"
        };
        let passed_count = compliance.rules.iter().filter(|r| r.passed).count();
        writeln!(
            out,
            "- **Constitutional Compliance:** {} ({}/{} rules)",
            status,
            passed_count,
            compliance.rules.len()
        )
        .unwrap();

        writeln!(out).unwrap();
    }

    fn write_dataset_info(&self, out: &mut String) {
        writeln!(out, "## Dataset Information").unwrap();
        writeln!(out).unwrap();

        let info = &self.results.dataset_info;
        writeln!(out, "| Metric | Value |").unwrap();
        writeln!(out, "|--------|-------|").unwrap();
        writeln!(out, "| Total Chunks | {} |", info.total_chunks).unwrap();
        writeln!(out, "| Chunks Used | {} |", info.chunks_used).unwrap();
        writeln!(out, "| Total Documents | {} |", info.total_documents).unwrap();
        writeln!(out, "| Unique Topics | {} |", info.num_topics).unwrap();
        writeln!(out, "| Queries Generated | {} |", info.queries_generated).unwrap();
        writeln!(
            out,
            "| Source Datasets | {} |",
            info.source_datasets.join(", ")
        )
        .unwrap();
        writeln!(out).unwrap();

        if !info.top_topics.is_empty() {
            writeln!(out, "### Top Topics").unwrap();
            writeln!(out).unwrap();
            writeln!(out, "| Topic | Chunks | Percentage |").unwrap();
            writeln!(out, "|-------|--------|------------|").unwrap();
            for topic in info.top_topics.iter().take(10) {
                writeln!(
                    out,
                    "| {} | {} | {:.1}% |",
                    topic.name, topic.chunk_count, topic.percentage
                )
                .unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    fn write_per_embedder_results(&self, out: &mut String) {
        writeln!(out, "## Per-Embedder Results").unwrap();
        writeln!(out).unwrap();

        // Sort embedders by MRR
        let mut embedders: Vec<_> = self.results.per_embedder_results.iter().collect();
        embedders.sort_by(|a, b| b.1.mrr_at_10.partial_cmp(&a.1.mrr_at_10).unwrap());

        writeln!(out, "### Retrieval Quality").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "| Embedder | Category | MRR@10 | P@5 | P@10 | R@20 | MAP |"
        )
        .unwrap();
        writeln!(
            out,
            "|----------|----------|--------|-----|------|------|-----|"
        )
        .unwrap();

        for (name, results) in &embedders {
            let p5 = results.precision_at_k.get(&5).copied().unwrap_or(0.0);
            let p10 = results.precision_at_k.get(&10).copied().unwrap_or(0.0);
            let r20 = results.recall_at_k.get(&20).copied().unwrap_or(0.0);
            writeln!(
                out,
                "| {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |",
                name, results.category, results.mrr_at_10, p5, p10, r20, results.map
            )
            .unwrap();
        }
        writeln!(out).unwrap();

        // E1 Contribution table
        writeln!(out, "### Contribution vs E1 Baseline").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "| Embedder | MRR Improvement | Is Enhancement? |").unwrap();
        writeln!(out, "|----------|-----------------|-----------------|").unwrap();

        for (name, results) in &embedders {
            if *name == &EmbedderName::E1Semantic {
                continue;
            }
            let improvement = results.contribution_vs_e1;
            let is_enhancement = improvement > 0.0;
            writeln!(
                out,
                "| {} | {:+.1}% | {} |",
                name,
                improvement * 100.0,
                if is_enhancement { "Yes" } else { "No" }
            )
            .unwrap();
        }
        writeln!(out).unwrap();

        // Asymmetric embedders
        let asymmetric_embedders: Vec<_> = embedders
            .iter()
            .filter(|(_, r)| r.asymmetric_ratio.is_some())
            .collect();

        if !asymmetric_embedders.is_empty() {
            writeln!(out, "### Asymmetric Embedder Ratios").unwrap();
            writeln!(out).unwrap();
            writeln!(out, "| Embedder | Asymmetric Ratio | Target | Status |").unwrap();
            writeln!(out, "|----------|------------------|--------|--------|").unwrap();

            for (name, results) in &asymmetric_embedders {
                if let Some(ratio) = results.asymmetric_ratio {
                    let target = 1.5;
                    let tolerance = 0.15;
                    let status = if (ratio - target).abs() <= tolerance {
                        "PASS"
                    } else {
                        "WARN"
                    };
                    writeln!(
                        out,
                        "| {} | {:.3} | {:.2}+/-{:.2} | {} |",
                        name, ratio, target, tolerance, status
                    )
                    .unwrap();
                }
            }
            writeln!(out).unwrap();
        }
    }

    fn write_fusion_results(&self, out: &mut String) {
        let Some(ref fusion) = self.results.fusion_results else {
            return;
        };

        writeln!(out, "## Fusion Strategy Comparison").unwrap();
        writeln!(out).unwrap();

        writeln!(
            out,
            "| Strategy | MRR@10 | P@10 | R@20 | Latency (ms) | Quality/Latency |"
        )
        .unwrap();
        writeln!(
            out,
            "|----------|--------|------|------|--------------|-----------------|"
        )
        .unwrap();

        for (strategy, results) in &fusion.by_strategy {
            let is_best = *strategy == fusion.best_strategy;
            let marker = if is_best { " *" } else { "" };
            writeln!(
                out,
                "| {:?}{} | {:.3} | {:.3} | {:.3} | {:.1} | {:.4} |",
                strategy,
                marker,
                results.mrr_at_10,
                results.precision_at_10,
                results.recall_at_20,
                results.latency_ms,
                results.quality_latency_ratio
            )
            .unwrap();
        }
        writeln!(out).unwrap();

        writeln!(out, "*Best strategy: {:?}*", fusion.best_strategy).unwrap();
        writeln!(out).unwrap();

        if !fusion.recommendations.is_empty() {
            writeln!(out, "### Fusion Recommendations").unwrap();
            writeln!(out).unwrap();
            for rec in &fusion.recommendations {
                writeln!(out, "- {}", rec).unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    fn write_cross_embedder_analysis(&self, out: &mut String) {
        let Some(ref analysis) = self.results.cross_embedder_analysis else {
            return;
        };

        writeln!(out, "## Cross-Embedder Analysis").unwrap();
        writeln!(out).unwrap();

        // Best complementary pairs
        writeln!(out, "### Best Complementary Pairs").unwrap();
        writeln!(out).unwrap();
        writeln!(out, "| Pair | Complementarity Score |").unwrap();
        writeln!(out, "|------|----------------------|").unwrap();

        for (a, b, score) in analysis.best_complementary_pairs.iter().take(10) {
            writeln!(out, "| {} + {} | {:.3} |", a, b, score).unwrap();
        }
        writeln!(out).unwrap();

        // Redundancy warnings
        if !analysis.redundancy_pairs.is_empty() {
            writeln!(out, "### Redundancy Warnings").unwrap();
            writeln!(out).unwrap();
            writeln!(
                out,
                "These pairs have high MRR proximity but limited complementarity:"
            )
            .unwrap();
            writeln!(out).unwrap();

            for (a, b, proximity) in &analysis.redundancy_pairs {
                writeln!(out, "- {} and {} (MRR proximity: {:.3})", a, b, proximity).unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    fn write_ablation_results(&self, out: &mut String) {
        let Some(ref ablation) = self.results.ablation_results else {
            return;
        };

        writeln!(out, "## Ablation Study").unwrap();
        writeln!(out).unwrap();

        // Addition impact
        writeln!(out, "### Impact of Adding Each Embedder to E1").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "| Embedder | MRR Change | P@10 Change | Significant? |"
        )
        .unwrap();
        writeln!(
            out,
            "|----------|------------|-------------|--------------|"
        )
        .unwrap();

        let mut addition_impacts: Vec<_> = ablation.addition_impact.iter().collect();
        addition_impacts.sort_by(|a, b| b.1.mrr_change.partial_cmp(&a.1.mrr_change).unwrap());

        for (name, impact) in &addition_impacts {
            writeln!(
                out,
                "| {} | {:+.1}% | {:+.1}% | {} |",
                name,
                impact.mrr_change * 100.0,
                impact.precision_change * 100.0,
                if impact.is_significant { "Yes" } else { "No" }
            )
            .unwrap();
        }
        writeln!(out).unwrap();

        // Critical embedders
        if !ablation.critical_embedders.is_empty() {
            writeln!(out, "### Critical Embedders").unwrap();
            writeln!(out).unwrap();
            writeln!(out, "Removing these causes >10% degradation:").unwrap();
            for e in &ablation.critical_embedders {
                writeln!(out, "- {}", e).unwrap();
            }
            writeln!(out).unwrap();
        }

        // Redundant embedders
        if !ablation.redundant_embedders.is_empty() {
            writeln!(out, "### Potentially Redundant Embedders").unwrap();
            writeln!(out).unwrap();
            writeln!(out, "Removing these causes <2% change:").unwrap();
            for e in &ablation.redundant_embedders {
                writeln!(out, "- {}", e).unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    fn write_constitutional_compliance(&self, out: &mut String) {
        writeln!(out, "## Constitutional Compliance").unwrap();
        writeln!(out).unwrap();

        let compliance = &self.results.constitutional_compliance;
        let status = if compliance.all_passed {
            "PASSED"
        } else {
            "FAILED"
        };
        writeln!(out, "**Overall Status:** {}", status).unwrap();
        writeln!(out).unwrap();

        writeln!(out, "| Rule | Description | Status | Details |").unwrap();
        writeln!(out, "|------|-------------|--------|---------|").unwrap();

        for rule in &compliance.rules {
            let status_emoji = if rule.passed { "PASS" } else { "FAIL" };
            writeln!(
                out,
                "| {} | {} | {} | {} |",
                rule.rule_id,
                rule.description,
                status_emoji,
                &rule.details[..rule.details.len().min(50)]
            )
            .unwrap();
        }
        writeln!(out).unwrap();

        if !compliance.warnings.is_empty() {
            writeln!(out, "### Warnings").unwrap();
            writeln!(out).unwrap();
            for warning in &compliance.warnings {
                writeln!(out, "- {}", warning).unwrap();
            }
            writeln!(out).unwrap();
        }

        if !compliance.errors.is_empty() {
            writeln!(out, "### Errors").unwrap();
            writeln!(out).unwrap();
            for error in &compliance.errors {
                writeln!(out, "- {}", error).unwrap();
            }
            writeln!(out).unwrap();
        }
    }

    fn write_recommendations(&self, out: &mut String) {
        if self.results.recommendations.is_empty() {
            return;
        }

        writeln!(out, "## Recommendations").unwrap();
        writeln!(out).unwrap();

        for (i, rec) in self.results.recommendations.iter().enumerate() {
            writeln!(out, "{}. {}", i + 1, rec).unwrap();
        }
        writeln!(out).unwrap();
    }

    fn write_footer(&self, out: &mut String) {
        writeln!(out, "---").unwrap();
        writeln!(out).unwrap();
        writeln!(
            out,
            "*Report generated by context-graph-benchmark unified-realdata-bench*"
        )
        .unwrap();
        writeln!(out, "*Version: {}*", self.results.metadata.version).unwrap();
    }
}

/// Generate a concise summary for console output.
pub fn generate_console_summary(results: &UnifiedBenchmarkResults) -> String {
    let mut out = String::new();

    writeln!(out, "\n=== Unified Real Data Benchmark Results ===\n").unwrap();

    // E1 baseline
    if let Some(e1) = results.per_embedder_results.get(&EmbedderName::E1Semantic) {
        writeln!(out, "E1 Foundation: MRR@10={:.3}", e1.mrr_at_10).unwrap();
    }

    // Top 5 embedders
    let mut embedders: Vec<_> = results.per_embedder_results.iter().collect();
    embedders.sort_by(|a, b| b.1.mrr_at_10.partial_cmp(&a.1.mrr_at_10).unwrap());

    writeln!(out, "\nTop 5 Embedders by MRR@10:").unwrap();
    for (name, res) in embedders.iter().take(5) {
        writeln!(out, "  {}: {:.3}", name, res.mrr_at_10).unwrap();
    }

    // Fusion result
    if let Some(ref fusion) = results.fusion_results {
        writeln!(
            out,
            "\nBest Fusion: {:?} (+{:.1}% over E1)",
            fusion.best_strategy,
            fusion.improvement_over_baseline * 100.0
        )
        .unwrap();
    }

    // Compliance
    let passed = results
        .constitutional_compliance
        .rules
        .iter()
        .filter(|r| r.passed)
        .count();
    let total = results.constitutional_compliance.rules.len();
    writeln!(out, "\nConstitutional Compliance: {}/{}", passed, total).unwrap();

    out
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::super::config::UnifiedBenchmarkConfig;
    use super::super::results::*;
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    fn create_test_results() -> UnifiedBenchmarkResults {
        let mut per_embedder = HashMap::new();

        let mut e1 = EmbedderResults::new(EmbedderName::E1Semantic);
        e1.mrr_at_10 = 0.75;
        e1.precision_at_k.insert(5, 0.6);
        e1.precision_at_k.insert(10, 0.5);
        e1.recall_at_k.insert(20, 0.8);
        e1.map = 0.7;
        per_embedder.insert(EmbedderName::E1Semantic, e1);

        let mut e5 = EmbedderResults::new(EmbedderName::E5Causal);
        e5.mrr_at_10 = 0.72;
        e5.contribution_vs_e1 = 0.05;
        e5.asymmetric_ratio = Some(1.48);
        per_embedder.insert(EmbedderName::E5Causal, e5);

        UnifiedBenchmarkResults {
            metadata: BenchmarkMetadata {
                version: "1.0.0".to_string(),
                start_time: Utc::now(),
                end_time: Utc::now(),
                duration_secs: 120.5,
                config: UnifiedBenchmarkConfig::default(),
                git_commit: Some("abc123".to_string()),
                hostname: Some("test".to_string()),
            },
            dataset_info: DatasetInfo {
                total_chunks: 58895,
                total_documents: 2437,
                num_topics: 120,
                top_topics: vec![
                    TopicInfo {
                        name: "science".to_string(),
                        chunk_count: 5000,
                        percentage: 8.5,
                    },
                    TopicInfo {
                        name: "history".to_string(),
                        chunk_count: 4000,
                        percentage: 6.8,
                    },
                ],
                source_datasets: vec!["wikipedia".to_string()],
                chunks_used: 10000,
                queries_generated: 100,
            },
            per_embedder_results: per_embedder,
            fusion_results: None,
            cross_embedder_analysis: None,
            ablation_results: None,
            recommendations: vec!["Use E1+E5+E7 for best results".to_string()],
            constitutional_compliance: ConstitutionalCompliance::new(),
            timings: BenchmarkTimings::default(),
        }
    }

    #[test]
    fn test_generate_markdown() {
        let results = create_test_results();
        let generator = ReportGenerator::new(results);
        let report = generator.generate_markdown();

        assert!(report.contains("# Unified Real Data Benchmark Report"));
        assert!(report.contains("## Executive Summary"));
        assert!(report.contains("## Dataset Information"));
        assert!(report.contains("## Per-Embedder Results"));
    }

    #[test]
    fn test_console_summary() {
        let results = create_test_results();
        let summary = generate_console_summary(&results);

        assert!(summary.contains("E1 Foundation"));
        assert!(summary.contains("Top 5 Embedders"));
    }
}
