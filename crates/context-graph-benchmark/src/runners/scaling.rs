//! Scaling analysis benchmark runner.
//!
//! Runs benchmarks across multiple tiers to analyze scaling behavior.

use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::baseline::SingleEmbedderBaseline;
use crate::config::{BenchmarkConfig, Tier, TierConfig};
use crate::datasets::{DatasetGenerator, GeneratorConfig};
use crate::metrics::ScalingMetrics;
use crate::scaling::{DegradationAnalysis, MemoryProfile, MemoryProfiler};

use super::retrieval::RetrievalRunner;
use super::topic::TopicRunner;

/// Scaling analysis runner.
pub struct ScalingRunner {
    config: BenchmarkConfig,
}

impl ScalingRunner {
    /// Create a new runner with config.
    pub fn new(config: BenchmarkConfig) -> Self {
        Self { config }
    }

    /// Create with default CI config.
    pub fn ci() -> Self {
        Self::new(BenchmarkConfig::ci())
    }

    /// Create with full config (all tiers).
    pub fn full() -> Self {
        Self::new(BenchmarkConfig::full())
    }

    /// Run scaling analysis on single-embedder baseline.
    pub fn run_single_embedder(&self) -> ScalingAnalysisResults {
        let mut results = ScalingAnalysisResults::new("single-embedder");
        let mut memory_profiler = MemoryProfiler::new();

        // Generate and benchmark each tier
        for tier_config in &self.config.tiers {
            let tier_results = self.run_tier_single_embedder(tier_config);
            results.add_tier_results(tier_config.tier, tier_results.metrics.clone());

            // Record memory profile
            memory_profiler.record(MemoryProfile {
                tier: tier_config.tier,
                corpus_size: tier_config.memory_count,
                total_bytes: tier_results.memory_bytes,
                components: crate::scaling::memory_profiler::estimate_single_embedder_memory(
                    tier_config.memory_count,
                ),
                per_document_bytes: tier_results.memory_bytes as f64
                    / tier_config.memory_count as f64,
            });
        }

        results.memory_report = Some(memory_profiler.report());
        results.finalize_degradation();
        results
    }

    /// Run scaling analysis on multi-space system.
    pub fn run_multi_space(&self) -> ScalingAnalysisResults {
        let mut results = ScalingAnalysisResults::new("multi-space");
        let mut memory_profiler = MemoryProfiler::new();

        // Generate and benchmark each tier
        for tier_config in &self.config.tiers {
            let tier_results = self.run_tier_multi_space(tier_config);
            results.add_tier_results(tier_config.tier, tier_results.metrics.clone());

            // Record memory profile
            memory_profiler.record(MemoryProfile {
                tier: tier_config.tier,
                corpus_size: tier_config.memory_count,
                total_bytes: tier_results.memory_bytes,
                components: crate::scaling::memory_profiler::estimate_multispace_memory(
                    tier_config.memory_count,
                ),
                per_document_bytes: tier_results.memory_bytes as f64
                    / tier_config.memory_count as f64,
            });
        }

        results.memory_report = Some(memory_profiler.report());
        results.finalize_degradation();
        results
    }

    /// Run a single tier for single-embedder.
    fn run_tier_single_embedder(&self, tier_config: &TierConfig) -> TierResults {
        // Generate dataset
        let gen_config = GeneratorConfig {
            seed: self.config.seed,
            ..Default::default()
        };
        let mut generator = DatasetGenerator::with_config(gen_config);
        let dataset = generator.generate_dataset(tier_config);

        // Build baseline
        let start = Instant::now();
        let mut baseline = SingleEmbedderBaseline::new();
        for (id, fp) in &dataset.fingerprints {
            baseline.insert(*id, &fp.e1_semantic);
        }
        let build_time = start.elapsed();

        // Run retrieval benchmark
        let retrieval_runner = RetrievalRunner::new()
            .with_k_values(self.config.k_values.clone())
            .with_warmup(self.config.warmup_iterations);
        let retrieval_results = retrieval_runner.run_single_embedder(&baseline, &dataset);

        // Run topic detection benchmark
        let topic_runner = TopicRunner::new().with_expected_topics(tier_config.topic_count);
        let topic_results = topic_runner.run_single_embedder(&baseline, &dataset);

        // Estimate memory
        let memory_bytes = crate::scaling::memory_profiler::estimate_single_embedder_memory(
            tier_config.memory_count,
        )
        .total();

        TierResults {
            metrics: ScalingMetrics {
                retrieval: retrieval_results.metrics,
                clustering: topic_results.metrics,
                divergence: Default::default(),
                performance: retrieval_results.performance,
            },
            build_time_ms: build_time.as_millis() as u64,
            memory_bytes,
        }
    }

    /// Run a single tier for multi-space.
    fn run_tier_multi_space(&self, tier_config: &TierConfig) -> TierResults {
        // Generate dataset
        let gen_config = GeneratorConfig {
            seed: self.config.seed,
            ..Default::default()
        };
        let mut generator = DatasetGenerator::with_config(gen_config);
        let dataset = generator.generate_dataset(tier_config);

        let start = Instant::now();
        // No explicit build needed for multi-space (uses dataset directly)
        let build_time = start.elapsed();

        // Run retrieval benchmark
        let retrieval_runner = RetrievalRunner::new()
            .with_k_values(self.config.k_values.clone())
            .with_warmup(self.config.warmup_iterations);
        let retrieval_results = retrieval_runner.run_multi_space(&dataset);

        // Run topic detection benchmark
        let topic_runner = TopicRunner::new().with_expected_topics(tier_config.topic_count);
        let topic_results = topic_runner.run_multi_space(&dataset);

        // Estimate memory
        let memory_bytes =
            crate::scaling::memory_profiler::estimate_multispace_memory(tier_config.memory_count)
                .total();

        TierResults {
            metrics: ScalingMetrics {
                retrieval: retrieval_results.metrics,
                clustering: topic_results.metrics,
                divergence: Default::default(),
                performance: retrieval_results.performance,
            },
            build_time_ms: build_time.as_millis() as u64,
            memory_bytes,
        }
    }
}

/// Results from a single tier benchmark.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TierResults {
    metrics: ScalingMetrics,
    build_time_ms: u64,
    memory_bytes: usize,
}

/// Complete scaling analysis results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScalingAnalysisResults {
    /// System name.
    pub system_name: String,
    /// Results per tier.
    pub tier_results: Vec<(Tier, ScalingMetrics)>,
    /// Degradation analysis.
    pub degradation: Option<DegradationAnalysis>,
    /// Memory scaling report.
    pub memory_report: Option<crate::scaling::memory_profiler::MemoryScalingReport>,
}

impl ScalingAnalysisResults {
    /// Create new results.
    fn new(system_name: &str) -> Self {
        Self {
            system_name: system_name.to_string(),
            tier_results: Vec::new(),
            degradation: None,
            memory_report: None,
        }
    }

    /// Add results for a tier.
    fn add_tier_results(&mut self, tier: Tier, metrics: ScalingMetrics) {
        self.tier_results.push((tier, metrics));
    }

    /// Finalize degradation analysis.
    fn finalize_degradation(&mut self) {
        if self.tier_results.is_empty() {
            return;
        }

        // Use first tier as baseline
        let baseline = self.tier_results[0].1.clone();
        let mut analysis = DegradationAnalysis::new(&self.system_name, baseline);

        for (tier, metrics) in &self.tier_results {
            let config = TierConfig::for_tier(*tier);
            analysis.add_point(*tier, config.memory_count, metrics.clone());
        }

        analysis.finalize();
        self.degradation = Some(analysis);
    }

    /// Get metrics for a specific tier.
    pub fn get_tier_metrics(&self, tier: Tier) -> Option<&ScalingMetrics> {
        self.tier_results
            .iter()
            .find(|(t, _)| *t == tier)
            .map(|(_, m)| m)
    }

    /// Check if system maintained performance across all tiers.
    pub fn maintained_performance(&self, _threshold: f64) -> bool {
        if let Some(ref degradation) = self.degradation {
            if let Some(limit) = degradation.limits.overall_limit {
                // If there's a limit, check if it's beyond our tested tiers
                let max_tested = self
                    .tier_results
                    .last()
                    .map(|(t, _)| TierConfig::for_tier(*t).memory_count)
                    .unwrap_or(0);
                return limit > max_tested;
            }
            true
        } else {
            true
        }
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_scaling_runner_single_tier() {
        let config = BenchmarkConfig::single_tier(Tier::Tier0);
        let runner = ScalingRunner::new(config);

        let results = runner.run_single_embedder();
        assert_eq!(results.tier_results.len(), 1);
        assert!(results.degradation.is_some());
    }

    #[test]
    fn test_scaling_comparison() {
        let config = BenchmarkConfig::single_tier(Tier::Tier0);
        let runner = ScalingRunner::new(config);

        let single_results = runner.run_single_embedder();
        let multi_results = runner.run_multi_space();

        // Both should have results
        assert!(!single_results.tier_results.is_empty());
        assert!(!multi_results.tier_results.is_empty());
    }
}
