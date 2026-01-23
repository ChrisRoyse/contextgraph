//! E4 Hybrid Session Clustering Benchmark Runner.
//!
//! This module provides the benchmark runner for evaluating the E4 hybrid
//! session+position encoding implementation. It runs clustering, ordering,
//! and effectiveness benchmarks.
//!
//! ## Usage
//!
//! ```ignore
//! let config = E4HybridSessionBenchmarkConfig::default();
//! let runner = E4HybridSessionBenchmarkRunner::new(config);
//! let results = runner.run(&sessions, &embeddings);
//! ```

use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use context_graph_core::types::fingerprint::SemanticFingerprint;

use crate::datasets::temporal_sessions::{SessionGeneratorConfig, TemporalSession};
use crate::metrics::e4_hybrid_session::{
    compute_hybrid_effectiveness, compute_intra_session_ordering,
    compute_session_clustering_metrics, E4HybridSessionMetrics,
    IntraSessionOrderingMetrics, SessionClusteringMetrics,
};

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for the E4 Hybrid Session Benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E4HybridSessionBenchmarkConfig {
    /// Session generator configuration.
    pub session_config: SessionGeneratorConfig,
    /// K values for retrieval metrics.
    pub k_values: Vec<usize>,
    /// Run legacy E4 comparison benchmark.
    pub run_legacy_comparison: bool,
    /// Run timestamp baseline comparison.
    pub run_timestamp_baseline: bool,
    /// Number of session pairs for clustering analysis.
    pub num_clustering_pairs: usize,
    /// Show progress during embedding.
    pub show_progress: bool,
    /// Random seed for reproducibility.
    pub seed: u64,
}

impl Default for E4HybridSessionBenchmarkConfig {
    fn default() -> Self {
        Self {
            session_config: SessionGeneratorConfig::default(),
            k_values: vec![1, 5, 10, 20],
            run_legacy_comparison: true,
            run_timestamp_baseline: true,
            num_clustering_pairs: 5000,
            show_progress: true,
            seed: 42,
        }
    }
}

// ============================================================================
// Results Structures
// ============================================================================

/// Results from the E4 Hybrid Session Benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E4HybridSessionBenchmarkResults {
    /// Core metrics.
    pub metrics: E4HybridSessionMetrics,
    /// Legacy E4 comparison results (if run).
    pub legacy_comparison: Option<LegacyComparisonResults>,
    /// Timestamp baseline comparison results (if run).
    pub timestamp_baseline: Option<TimestampBaselineResults>,
    /// Performance timings.
    pub timings: E4HybridBenchmarkTimings,
    /// Configuration used.
    pub config: E4HybridSessionBenchmarkConfig,
    /// Dataset statistics.
    pub dataset_stats: E4HybridDatasetStats,
    /// Validation summary.
    pub validation: ValidationSummary,
}

impl E4HybridSessionBenchmarkResults {
    /// Check if all targets are met.
    pub fn all_targets_met(&self) -> bool {
        self.validation.all_passed
    }

    /// Get the overall score.
    pub fn overall_score(&self) -> f64 {
        self.metrics.composite.overall_score
    }
}

/// Legacy E4 comparison results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyComparisonResults {
    /// Legacy E4 clustering metrics.
    pub clustering: SessionClusteringMetrics,
    /// Legacy E4 ordering metrics.
    pub ordering: IntraSessionOrderingMetrics,
    /// Improvement of hybrid over legacy.
    pub hybrid_improvement: f64,
    /// Whether hybrid is significantly better.
    pub hybrid_is_better: bool,
}

/// Timestamp baseline comparison results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampBaselineResults {
    /// Timestamp baseline clustering metrics.
    pub clustering: SessionClusteringMetrics,
    /// Improvement of hybrid over timestamp.
    pub hybrid_improvement: f64,
    /// Whether hybrid is significantly better.
    pub hybrid_is_better: bool,
}

/// Benchmark timing information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E4HybridBenchmarkTimings {
    /// Total benchmark time in milliseconds.
    pub total_ms: u64,
    /// Session generation time in milliseconds.
    pub session_generation_ms: u64,
    /// Embedding extraction time in milliseconds.
    pub embedding_extraction_ms: u64,
    /// Clustering benchmark time in milliseconds.
    pub clustering_benchmark_ms: u64,
    /// Ordering benchmark time in milliseconds.
    pub ordering_benchmark_ms: u64,
    /// Legacy comparison time in milliseconds.
    pub legacy_comparison_ms: Option<u64>,
    /// Timestamp baseline time in milliseconds.
    pub timestamp_baseline_ms: Option<u64>,
}

/// Dataset statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct E4HybridDatasetStats {
    /// Number of sessions.
    pub num_sessions: usize,
    /// Total number of chunks.
    pub num_chunks: usize,
    /// Average session length.
    pub avg_session_length: f64,
    /// Min session length.
    pub min_session_length: usize,
    /// Max session length.
    pub max_session_length: usize,
    /// Number of chunks with embeddings.
    pub num_chunks_with_embeddings: usize,
    /// Number of unique topics.
    pub num_topics: usize,
}

impl E4HybridDatasetStats {
    /// Compute statistics from sessions and embeddings.
    pub fn from_sessions(
        sessions: &[TemporalSession],
        embeddings: &HashMap<Uuid, Vec<f32>>,
    ) -> Self {
        let num_sessions = sessions.len();
        let num_chunks: usize = sessions.iter().map(|s| s.len()).sum();

        let avg_session_length = if num_sessions > 0 {
            num_chunks as f64 / num_sessions as f64
        } else {
            0.0
        };

        let min_session_length = sessions.iter().map(|s| s.len()).min().unwrap_or(0);
        let max_session_length = sessions.iter().map(|s| s.len()).max().unwrap_or(0);

        let num_chunks_with_embeddings = sessions
            .iter()
            .flat_map(|s| s.chunks.iter())
            .filter(|c| embeddings.contains_key(&c.id))
            .count();

        let topics: std::collections::HashSet<_> =
            sessions.iter().map(|s| s.topic.clone()).collect();

        Self {
            num_sessions,
            num_chunks,
            avg_session_length,
            min_session_length,
            max_session_length,
            num_chunks_with_embeddings,
            num_topics: topics.len(),
        }
    }
}

/// Validation summary.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationSummary {
    /// All validation checks passed.
    pub all_passed: bool,
    /// Number of checks passed.
    pub checks_passed: usize,
    /// Total number of checks.
    pub checks_total: usize,
    /// Individual check results.
    pub checks: Vec<ValidationCheck>,
}

/// Individual validation check result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationCheck {
    /// Check name.
    pub name: String,
    /// Check description.
    pub description: String,
    /// Whether the check passed.
    pub passed: bool,
    /// Actual value.
    pub actual: String,
    /// Expected value.
    pub expected: String,
}

// ============================================================================
// Benchmark Runner
// ============================================================================

/// E4 Hybrid Session Benchmark Runner.
pub struct E4HybridSessionBenchmarkRunner {
    config: E4HybridSessionBenchmarkConfig,
}

impl E4HybridSessionBenchmarkRunner {
    /// Create a new runner with the given configuration.
    pub fn new(config: E4HybridSessionBenchmarkConfig) -> Self {
        Self { config }
    }

    /// Run the full benchmark suite.
    ///
    /// # Arguments
    ///
    /// * `sessions` - Generated temporal sessions
    /// * `fingerprints` - Semantic fingerprints for each chunk
    ///
    /// # Returns
    ///
    /// Complete benchmark results.
    pub fn run(
        &self,
        sessions: &[TemporalSession],
        fingerprints: &HashMap<Uuid, SemanticFingerprint>,
    ) -> E4HybridSessionBenchmarkResults {
        let total_start = Instant::now();

        // Extract E4 embeddings from fingerprints
        let extract_start = Instant::now();
        let e4_embeddings = self.extract_e4_embeddings(fingerprints);
        let embedding_extraction_ms = extract_start.elapsed().as_millis() as u64;

        // Compute dataset stats
        let dataset_stats = E4HybridDatasetStats::from_sessions(sessions, &e4_embeddings);

        // Run clustering benchmark
        let clustering_start = Instant::now();
        let clustering_metrics = compute_session_clustering_metrics(sessions, &e4_embeddings);
        let clustering_benchmark_ms = clustering_start.elapsed().as_millis() as u64;

        // Run ordering benchmark
        let ordering_start = Instant::now();
        let ordering_metrics = compute_intra_session_ordering(sessions, &e4_embeddings);
        let ordering_benchmark_ms = ordering_start.elapsed().as_millis() as u64;

        // Run legacy comparison if enabled
        let (legacy_comparison, legacy_comparison_ms) = if self.config.run_legacy_comparison {
            let legacy_start = Instant::now();
            let legacy_embeddings = self.simulate_legacy_e4(&e4_embeddings, sessions);
            let legacy_results = self.run_legacy_comparison(sessions, &legacy_embeddings, &clustering_metrics);
            (Some(legacy_results), Some(legacy_start.elapsed().as_millis() as u64))
        } else {
            (None, None)
        };

        // Run timestamp baseline if enabled
        let (timestamp_baseline, timestamp_baseline_ms) = if self.config.run_timestamp_baseline {
            let baseline_start = Instant::now();
            let timestamp_embeddings = self.simulate_timestamp_baseline(&e4_embeddings, sessions);
            let baseline_results = self.run_timestamp_baseline(sessions, &timestamp_embeddings, &clustering_metrics);
            (Some(baseline_results), Some(baseline_start.elapsed().as_millis() as u64))
        } else {
            (None, None)
        };

        // Compute hybrid effectiveness
        let legacy_clustering = legacy_comparison.as_ref().map(|l| &l.clustering);
        let timestamp_clustering = timestamp_baseline.as_ref().map(|t| &t.clustering);
        let hybrid_effectiveness =
            compute_hybrid_effectiveness(&clustering_metrics, legacy_clustering, timestamp_clustering);

        // Build composite metrics
        let composite = crate::metrics::e4_hybrid_session::CompositeE4HybridMetrics::compute(
            &clustering_metrics,
            &ordering_metrics,
            &hybrid_effectiveness,
        );

        let metrics = E4HybridSessionMetrics {
            clustering: clustering_metrics,
            ordering: ordering_metrics,
            hybrid_effectiveness,
            composite,
        };

        // Run validation checks
        let validation = self.run_validation(&metrics);

        let timings = E4HybridBenchmarkTimings {
            total_ms: total_start.elapsed().as_millis() as u64,
            session_generation_ms: 0, // Sessions passed in, not generated here
            embedding_extraction_ms,
            clustering_benchmark_ms,
            ordering_benchmark_ms,
            legacy_comparison_ms,
            timestamp_baseline_ms,
        };

        E4HybridSessionBenchmarkResults {
            metrics,
            legacy_comparison,
            timestamp_baseline,
            timings,
            config: self.config.clone(),
            dataset_stats,
            validation,
        }
    }

    /// Extract E4 embeddings from semantic fingerprints.
    fn extract_e4_embeddings(
        &self,
        fingerprints: &HashMap<Uuid, SemanticFingerprint>,
    ) -> HashMap<Uuid, Vec<f32>> {
        fingerprints
            .iter()
            .map(|(id, fp)| (*id, fp.e4_temporal_positional.clone()))
            .collect()
    }

    /// Simulate legacy E4 (position-only, no session awareness).
    ///
    /// For simulation, we create embeddings that only encode position
    /// without session information.
    fn simulate_legacy_e4(
        &self,
        _hybrid_embeddings: &HashMap<Uuid, Vec<f32>>,
        sessions: &[TemporalSession],
    ) -> HashMap<Uuid, Vec<f32>> {
        let dim = 512; // E4 dimension
        let mut embeddings = HashMap::new();

        for session in sessions {
            for chunk in &session.chunks {
                // Create position-only embedding (sinusoidal PE without session signature)
                let pos = chunk.sequence_position;
                let mut emb = vec![0.0; dim];

                // Classic sinusoidal positional encoding
                for i in 0..dim / 2 {
                    let freq = 1.0 / (10000.0_f32.powf(2.0 * i as f32 / dim as f32));
                    emb[2 * i] = (pos as f32 * freq).sin();
                    emb[2 * i + 1] = (pos as f32 * freq).cos();
                }

                embeddings.insert(chunk.id, emb);
            }
        }

        embeddings
    }

    /// Simulate timestamp baseline (raw timestamp embedding).
    ///
    /// For simulation, we create embeddings based on simulated timestamps.
    fn simulate_timestamp_baseline(
        &self,
        _hybrid_embeddings: &HashMap<Uuid, Vec<f32>>,
        sessions: &[TemporalSession],
    ) -> HashMap<Uuid, Vec<f32>> {
        let dim = 512;
        let mut embeddings = HashMap::new();

        let base_time = 1704067200000_i64; // 2024-01-01 00:00:00 UTC
        let session_gap_ms = 3600000_i64; // 1 hour between sessions
        let chunk_gap_ms = 60000_i64; // 1 minute between chunks

        for (session_idx, session) in sessions.iter().enumerate() {
            let session_start = base_time + (session_idx as i64 * session_gap_ms);

            for chunk in &session.chunks {
                let chunk_time = session_start + (chunk.sequence_position as i64 * chunk_gap_ms);

                // Create timestamp-based embedding
                let mut emb = vec![0.0; dim];

                // Encode timestamp features
                let time_of_day = (chunk_time % 86400000) as f32 / 86400000.0;
                let day_of_week = ((chunk_time / 86400000) % 7) as f32 / 7.0;

                // Fill embedding with time-based features
                for i in 0..dim / 4 {
                    let freq = 1.0 / (10000.0_f32.powf(4.0 * i as f32 / dim as f32));
                    emb[4 * i] = (time_of_day * freq * std::f32::consts::PI * 2.0).sin();
                    emb[4 * i + 1] = (time_of_day * freq * std::f32::consts::PI * 2.0).cos();
                    emb[4 * i + 2] = (day_of_week * freq * std::f32::consts::PI * 2.0).sin();
                    emb[4 * i + 3] = (day_of_week * freq * std::f32::consts::PI * 2.0).cos();
                }

                embeddings.insert(chunk.id, emb);
            }
        }

        embeddings
    }

    /// Run legacy E4 comparison.
    fn run_legacy_comparison(
        &self,
        sessions: &[TemporalSession],
        legacy_embeddings: &HashMap<Uuid, Vec<f32>>,
        hybrid_clustering: &SessionClusteringMetrics,
    ) -> LegacyComparisonResults {
        let clustering = compute_session_clustering_metrics(sessions, legacy_embeddings);
        let ordering = compute_intra_session_ordering(sessions, legacy_embeddings);

        let hybrid_improvement = if clustering.session_separation_ratio > 0.001 {
            (hybrid_clustering.session_separation_ratio - clustering.session_separation_ratio)
                / clustering.session_separation_ratio
        } else {
            0.0
        };

        let hybrid_is_better = hybrid_improvement >= 0.10;

        LegacyComparisonResults {
            clustering,
            ordering,
            hybrid_improvement,
            hybrid_is_better,
        }
    }

    /// Run timestamp baseline comparison.
    fn run_timestamp_baseline(
        &self,
        sessions: &[TemporalSession],
        timestamp_embeddings: &HashMap<Uuid, Vec<f32>>,
        hybrid_clustering: &SessionClusteringMetrics,
    ) -> TimestampBaselineResults {
        let clustering = compute_session_clustering_metrics(sessions, timestamp_embeddings);

        let hybrid_improvement = if clustering.session_separation_ratio > 0.001 {
            (hybrid_clustering.session_separation_ratio - clustering.session_separation_ratio)
                / clustering.session_separation_ratio
        } else {
            0.0
        };

        let hybrid_is_better = hybrid_improvement >= 0.0;

        TimestampBaselineResults {
            clustering,
            hybrid_improvement,
            hybrid_is_better,
        }
    }

    /// Run validation checks.
    fn run_validation(&self, metrics: &E4HybridSessionMetrics) -> ValidationSummary {
        let mut checks = Vec::new();

        // Check 1: Session separation ratio >= 2.0
        checks.push(ValidationCheck {
            name: "session_separation_ratio".to_string(),
            description: "Session separation ratio >= 2.0".to_string(),
            passed: metrics.clustering.session_separation_ratio >= 2.0,
            actual: format!("{:.2}x", metrics.clustering.session_separation_ratio),
            expected: ">= 2.0x".to_string(),
        });

        // Check 2: Intra-session ordering >= 80%
        checks.push(ValidationCheck {
            name: "intra_session_ordering".to_string(),
            description: "Intra-session ordering accuracy >= 80%".to_string(),
            passed: metrics.ordering.ordering_accuracy >= 0.80,
            actual: format!("{:.1}%", metrics.ordering.ordering_accuracy * 100.0),
            expected: ">= 80%".to_string(),
        });

        // Check 3: Before/after symmetry >= 0.8
        checks.push(ValidationCheck {
            name: "before_after_symmetry".to_string(),
            description: "Before/after symmetry >= 0.8".to_string(),
            passed: metrics.ordering.symmetry_score >= 0.80,
            actual: format!("{:.2}", metrics.ordering.symmetry_score),
            expected: ">= 0.8".to_string(),
        });

        // Check 4: Hybrid vs legacy improvement >= +10%
        checks.push(ValidationCheck {
            name: "hybrid_vs_legacy".to_string(),
            description: "Hybrid vs legacy improvement >= +10%".to_string(),
            passed: metrics.hybrid_effectiveness.vs_legacy_improvement >= 0.10,
            actual: format!(
                "{:+.1}%",
                metrics.hybrid_effectiveness.vs_legacy_improvement * 100.0
            ),
            expected: ">= +10%".to_string(),
        });

        let checks_passed = checks.iter().filter(|c| c.passed).count();
        let checks_total = checks.len();
        let all_passed = checks_passed == checks_total;

        ValidationSummary {
            all_passed,
            checks_passed,
            checks_total,
            checks,
        }
    }
}

// ============================================================================
// Convenience Functions
// ============================================================================

/// Run a quick E4 hybrid session benchmark with default settings.
pub fn run_e4_hybrid_benchmark(
    sessions: &[TemporalSession],
    fingerprints: &HashMap<Uuid, SemanticFingerprint>,
) -> E4HybridSessionBenchmarkResults {
    let config = E4HybridSessionBenchmarkConfig::default();
    let runner = E4HybridSessionBenchmarkRunner::new(config);
    runner.run(sessions, fingerprints)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datasets::temporal_sessions::SessionChunk;

    fn create_test_session(id: &str, num_chunks: usize) -> TemporalSession {
        let chunks = (0..num_chunks)
            .map(|i| SessionChunk {
                id: Uuid::new_v4(),
                sequence_position: i,
                text: format!("Chunk {} in session {}", i, id),
                source_doc_id: format!("doc_{}", id),
                original_topic: "test_topic".to_string(),
                source_dataset: "test".to_string(),
            })
            .collect();

        TemporalSession {
            session_id: id.to_string(),
            chunks,
            topic: "test_topic".to_string(),
            coherence_score: 0.8,
            source_datasets: vec!["test".to_string()],
        }
    }

    fn create_test_fingerprints(sessions: &[TemporalSession]) -> HashMap<Uuid, SemanticFingerprint> {
        let mut fingerprints = HashMap::new();

        for (session_idx, session) in sessions.iter().enumerate() {
            for chunk in &session.chunks {
                // Create a mock fingerprint with E4 embedding
                let mut e4_emb = vec![0.0; 512];

                // Session signature: different base for each session
                let session_base = session_idx as f32 * 0.5;
                for i in 0..64 {
                    e4_emb[i] = session_base + (i as f32 * 0.01);
                }

                // Position encoding: sinusoidal
                let pos = chunk.sequence_position;
                for i in 64..512 {
                    let freq = 1.0 / (10000.0_f32.powf((i - 64) as f32 / 448.0));
                    e4_emb[i] = (pos as f32 * freq).sin();
                }

                // Normalize
                let norm: f32 = e4_emb.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in &mut e4_emb {
                        *x /= norm;
                    }
                }

                let mut fp = SemanticFingerprint::zeroed();
                fp.e4_temporal_positional = e4_emb;
                fingerprints.insert(chunk.id, fp);
            }
        }

        fingerprints
    }

    #[test]
    fn test_config_default() {
        let config = E4HybridSessionBenchmarkConfig::default();
        assert!(config.run_legacy_comparison);
        assert!(config.run_timestamp_baseline);
        assert!(!config.k_values.is_empty());
    }

    #[test]
    fn test_dataset_stats() {
        let sessions = vec![
            create_test_session("s1", 5),
            create_test_session("s2", 8),
            create_test_session("s3", 3),
        ];

        let fingerprints = create_test_fingerprints(&sessions);
        let e4_embeddings: HashMap<Uuid, Vec<f32>> = fingerprints
            .iter()
            .map(|(id, fp)| (*id, fp.e4_temporal_positional.clone()))
            .collect();

        let stats = E4HybridDatasetStats::from_sessions(&sessions, &e4_embeddings);

        assert_eq!(stats.num_sessions, 3);
        assert_eq!(stats.num_chunks, 16);
        assert_eq!(stats.min_session_length, 3);
        assert_eq!(stats.max_session_length, 8);
    }

    #[test]
    fn test_runner_basic() {
        let sessions = vec![
            create_test_session("s1", 5),
            create_test_session("s2", 5),
        ];

        let fingerprints = create_test_fingerprints(&sessions);

        let config = E4HybridSessionBenchmarkConfig {
            run_legacy_comparison: true,
            run_timestamp_baseline: true,
            ..Default::default()
        };

        let runner = E4HybridSessionBenchmarkRunner::new(config);
        let results = runner.run(&sessions, &fingerprints);

        // Check that results are populated
        assert!(results.metrics.clustering.num_sessions > 0);
        assert!(results.timings.total_ms > 0);
        assert!(!results.validation.checks.is_empty());

        // Legacy comparison should be present
        assert!(results.legacy_comparison.is_some());
        assert!(results.timestamp_baseline.is_some());
    }

    #[test]
    fn test_validation_checks() {
        let metrics = E4HybridSessionMetrics {
            clustering: SessionClusteringMetrics {
                session_separation_ratio: 2.5,
                ..Default::default()
            },
            ordering: IntraSessionOrderingMetrics {
                ordering_accuracy: 0.85,
                symmetry_score: 0.9,
                ..Default::default()
            },
            hybrid_effectiveness: crate::metrics::e4_hybrid_session::HybridEffectivenessMetrics {
                vs_legacy_improvement: 0.15,
                ..Default::default()
            },
            ..Default::default()
        };

        let config = E4HybridSessionBenchmarkConfig::default();
        let runner = E4HybridSessionBenchmarkRunner::new(config);
        let validation = runner.run_validation(&metrics);

        assert!(validation.all_passed);
        assert_eq!(validation.checks_passed, 4);
    }
}
