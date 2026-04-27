//! Memory profiling utilities for benchmark analysis.
//!
//! Tracks memory usage across different system components and scales.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::config::Tier;

/// Memory profile for a single measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProfile {
    /// Tier being measured.
    pub tier: Tier,
    /// Corpus size (number of documents).
    pub corpus_size: usize,
    /// Total memory usage (bytes).
    pub total_bytes: usize,
    /// Memory breakdown by component.
    pub components: MemoryComponents,
    /// Per-document memory (bytes).
    pub per_document_bytes: f64,
}

/// Memory usage breakdown by component.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryComponents {
    /// Fingerprint storage (bytes).
    pub fingerprints: usize,
    /// Index memory (bytes).
    pub index: usize,
    /// Metadata and auxiliary structures (bytes).
    pub metadata: usize,
    /// Query cache (if any) (bytes).
    pub query_cache: usize,
    /// Other/overhead (bytes).
    pub other: usize,
}

impl MemoryComponents {
    /// Total memory usage.
    pub fn total(&self) -> usize {
        self.fingerprints + self.index + self.metadata + self.query_cache + self.other
    }

    /// Get breakdown as percentages.
    pub fn percentages(&self) -> MemoryPercentages {
        let total = self.total() as f64;
        if total < 1.0 {
            return MemoryPercentages::default();
        }

        MemoryPercentages {
            fingerprints: self.fingerprints as f64 / total * 100.0,
            index: self.index as f64 / total * 100.0,
            metadata: self.metadata as f64 / total * 100.0,
            query_cache: self.query_cache as f64 / total * 100.0,
            other: self.other as f64 / total * 100.0,
        }
    }
}

/// Memory usage as percentages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryPercentages {
    pub fingerprints: f64,
    pub index: f64,
    pub metadata: f64,
    pub query_cache: f64,
    pub other: f64,
}

/// Memory profiler for tracking usage across scales.
pub struct MemoryProfiler {
    /// Profiles collected at each tier.
    profiles: BTreeMap<Tier, MemoryProfile>,
    /// Baseline memory (before any data loaded).
    baseline_bytes: usize,
}

impl MemoryProfiler {
    /// Create a new profiler.
    pub fn new() -> Self {
        Self {
            profiles: BTreeMap::new(),
            baseline_bytes: 0,
        }
    }

    /// Set baseline memory (before loading data).
    pub fn set_baseline(&mut self, bytes: usize) {
        self.baseline_bytes = bytes;
    }

    /// Record a memory profile.
    pub fn record(&mut self, profile: MemoryProfile) {
        self.profiles.insert(profile.tier, profile);
    }

    /// Get profile for a tier.
    pub fn get(&self, tier: Tier) -> Option<&MemoryProfile> {
        self.profiles.get(&tier)
    }

    /// Get all profiles.
    pub fn all_profiles(&self) -> Vec<&MemoryProfile> {
        self.profiles.values().collect()
    }

    /// Compute memory scaling factor (memory increase per 10x corpus increase).
    pub fn memory_scaling_factor(&self) -> Option<f64> {
        let profiles: Vec<&MemoryProfile> = self.profiles.values().collect();
        if profiles.len() < 2 {
            return None;
        }

        // Use linear regression on log-log scale
        let points: Vec<(f64, f64)> = profiles
            .iter()
            .map(|p| {
                (
                    (p.corpus_size as f64).log10(),
                    (p.total_bytes as f64).log10(),
                )
            })
            .collect();

        // Compute slope (memory exponent)
        let n = points.len() as f64;
        let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
        let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();

        let denom = n * sum_xx - sum_x * sum_x;
        if denom.abs() < f64::EPSILON {
            return None;
        }

        let slope = (n * sum_xy - sum_x * sum_y) / denom;

        // Scaling factor is 10^slope (memory multiplier per 10x corpus increase)
        Some(10.0_f64.powf(slope))
    }

    /// Estimate memory for a given corpus size.
    pub fn estimate_memory(&self, corpus_size: usize) -> Option<usize> {
        if self.profiles.is_empty() {
            return None;
        }

        // Use the most recent profile to estimate
        let last_profile = self.profiles.values().last()?;
        let per_doc = last_profile.per_document_bytes;

        Some((per_doc * corpus_size as f64) as usize)
    }

    /// Generate memory scaling report.
    pub fn report(&self) -> MemoryScalingReport {
        let profiles: Vec<MemoryProfile> = self.profiles.values().cloned().collect();
        let scaling_factor = self.memory_scaling_factor();

        // Compute efficiency metrics
        let efficiency: Vec<MemoryEfficiency> = profiles
            .iter()
            .map(|p| MemoryEfficiency {
                tier: p.tier,
                per_document_bytes: p.per_document_bytes,
                index_overhead_pct: p.components.percentages().index,
            })
            .collect();

        MemoryScalingReport {
            profiles,
            scaling_factor,
            baseline_bytes: self.baseline_bytes,
            efficiency,
        }
    }
}

impl Default for MemoryProfiler {
    fn default() -> Self {
        Self::new()
    }
}

/// Memory scaling report.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryScalingReport {
    /// Profiles at each tier.
    pub profiles: Vec<MemoryProfile>,
    /// Memory scaling factor (multiplier per 10x corpus increase).
    pub scaling_factor: Option<f64>,
    /// Baseline memory (bytes).
    pub baseline_bytes: usize,
    /// Efficiency metrics per tier.
    pub efficiency: Vec<MemoryEfficiency>,
}

/// Memory efficiency metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEfficiency {
    /// Tier.
    pub tier: Tier,
    /// Memory per document (bytes).
    pub per_document_bytes: f64,
    /// Index overhead as percentage of total.
    pub index_overhead_pct: f64,
}

/// Estimate memory usage for the multi-space system.
pub fn estimate_multispace_memory(corpus_size: usize) -> MemoryComponents {
    // From constitution: ~46KB per fingerprint
    let fp_bytes = corpus_size * 46_000;

    // HNSW index: ~100 bytes per vector per index, 12 HNSW indexes
    let index_bytes = corpus_size * 100 * 12;

    // Metadata: ~200 bytes per document
    let metadata_bytes = corpus_size * 200;

    MemoryComponents {
        fingerprints: fp_bytes,
        index: index_bytes,
        metadata: metadata_bytes,
        query_cache: 0,
        other: corpus_size * 50, // Overhead
    }
}

/// Estimate memory usage for single-embedder baseline.
pub fn estimate_single_embedder_memory(corpus_size: usize) -> MemoryComponents {
    // Single embedding: 1024 * 4 = 4KB per document
    let fp_bytes = corpus_size * 4_096;

    // Single HNSW index: ~100 bytes per vector
    let index_bytes = corpus_size * 100;

    // Metadata: ~200 bytes per document
    let metadata_bytes = corpus_size * 200;

    MemoryComponents {
        fingerprints: fp_bytes,
        index: index_bytes,
        metadata: metadata_bytes,
        query_cache: 0,
        other: corpus_size * 20, // Less overhead
    }
}

/// Compare memory efficiency between systems.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryComparison {
    /// Multi-space memory (bytes).
    pub multispace_bytes: usize,
    /// Single-embedder memory (bytes).
    pub single_embedder_bytes: usize,
    /// Memory overhead factor (multispace / single).
    pub overhead_factor: f64,
    /// Cost per 1% accuracy improvement estimate.
    pub bytes_per_accuracy_pct: Option<f64>,
}

impl MemoryComparison {
    /// Create comparison for a given corpus size.
    pub fn for_corpus_size(corpus_size: usize) -> Self {
        let multi = estimate_multispace_memory(corpus_size);
        let single = estimate_single_embedder_memory(corpus_size);

        let multi_total = multi.total();
        let single_total = single.total();

        let overhead_factor = if single_total > 0 {
            multi_total as f64 / single_total as f64
        } else {
            1.0
        };

        Self {
            multispace_bytes: multi_total,
            single_embedder_bytes: single_total,
            overhead_factor,
            bytes_per_accuracy_pct: None, // Computed from benchmark results
        }
    }

    /// Set bytes per accuracy percentage (from benchmark results).
    pub fn with_accuracy_improvement(mut self, accuracy_improvement_pct: f64) -> Self {
        if accuracy_improvement_pct > 0.0 {
            let extra_bytes = self
                .multispace_bytes
                .saturating_sub(self.single_embedder_bytes);
            self.bytes_per_accuracy_pct = Some(extra_bytes as f64 / accuracy_improvement_pct);
        }
        self
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_memory_components() {
        let components = MemoryComponents {
            fingerprints: 1000,
            index: 200,
            metadata: 100,
            query_cache: 50,
            other: 50,
        };

        assert_eq!(components.total(), 1400);

        let pcts = components.percentages();
        assert!((pcts.fingerprints - 71.4).abs() < 0.5);
    }

    #[test]
    fn test_memory_estimation() {
        let multi = estimate_multispace_memory(1000);
        let single = estimate_single_embedder_memory(1000);

        // Multi-space should use more memory
        assert!(multi.total() > single.total());

        // But not excessively more (~10x is expected from 46KB vs 4KB)
        assert!(multi.total() < single.total() * 20);
    }

    #[test]
    fn test_memory_comparison() {
        let comparison = MemoryComparison::for_corpus_size(10_000);

        assert!(comparison.overhead_factor > 1.0);
        assert!(comparison.multispace_bytes > comparison.single_embedder_bytes);
    }
}
