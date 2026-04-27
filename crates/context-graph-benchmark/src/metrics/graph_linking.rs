//! Graph linking benchmark metrics.
//!
//! Metrics types for all graph linking operations:
//! - NN-Descent K-NN graph construction
//! - EdgeBuilder edge creation
//! - BackgroundGraphBuilder batch processing
//! - Graph expansion in retrieval pipeline
//! - Weight projection inference
//! - R-GCN GNN inference

use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::datasets::graph_linking::ScaleTier;

/// Latency statistics for a benchmark.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyStats {
    /// Mean latency in microseconds.
    pub mean_us: u64,
    /// Median (p50) latency in microseconds.
    pub p50_us: u64,
    /// 95th percentile latency in microseconds.
    pub p95_us: u64,
    /// 99th percentile latency in microseconds.
    pub p99_us: u64,
    /// Minimum latency in microseconds.
    pub min_us: u64,
    /// Maximum latency in microseconds.
    pub max_us: u64,
    /// Standard deviation in microseconds.
    pub std_dev_us: u64,
    /// Number of samples.
    pub count: usize,
}

impl LatencyStats {
    /// Compute stats from a list of durations.
    pub fn from_durations(durations: &[Duration]) -> Self {
        if durations.is_empty() {
            return Self::default();
        }

        let mut micros: Vec<u64> = durations.iter().map(|d| d.as_micros() as u64).collect();
        micros.sort_unstable();

        let count = micros.len();
        let sum: u64 = micros.iter().sum();
        let mean_us = sum / count as u64;

        // Calculate standard deviation
        let variance: f64 = micros
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean_us as f64;
                diff * diff
            })
            .sum::<f64>()
            / count as f64;
        let std_dev_us = variance.sqrt() as u64;

        Self {
            mean_us,
            p50_us: micros[count / 2],
            p95_us: micros[(count as f64 * 0.95) as usize],
            p99_us: micros[(count as f64 * 0.99).min(count as f64 - 1.0) as usize],
            min_us: micros[0],
            max_us: micros[count - 1],
            std_dev_us,
            count,
        }
    }

    /// Get mean latency in milliseconds.
    pub fn mean_ms(&self) -> f64 {
        self.mean_us as f64 / 1000.0
    }

    /// Get p99 latency in milliseconds.
    pub fn p99_ms(&self) -> f64 {
        self.p99_us as f64 / 1000.0
    }
}

impl std::fmt::Display for LatencyStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "p50={:.2}ms p95={:.2}ms p99={:.2}ms (n={})",
            self.p50_us as f64 / 1000.0,
            self.p95_us as f64 / 1000.0,
            self.p99_us as f64 / 1000.0,
            self.count
        )
    }
}

/// Memory usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Peak memory usage in bytes.
    pub peak_bytes: u64,
    /// Allocated memory at end in bytes.
    pub allocated_bytes: u64,
    /// Memory per item (for scaling analysis).
    pub bytes_per_item: f64,
}

/// NN-Descent algorithm metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NNDescentMetrics {
    /// Number of iterations until convergence.
    pub iterations: usize,
    /// Updates performed per iteration.
    pub updates_per_iteration: Vec<usize>,
    /// Convergence rate (updates[last] / updates[first]).
    pub convergence_rate: f64,
    /// Total distance comparisons performed.
    pub total_comparisons: u64,
    /// Total time for graph construction.
    pub latency: LatencyStats,
    /// Memory usage.
    pub memory: MemoryStats,
    /// Number of nodes in graph.
    pub num_nodes: usize,
    /// K neighbors per node.
    pub k: usize,
    /// Edges per second throughput.
    pub edges_per_sec: f64,
}

/// EdgeBuilder metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EdgeBuilderMetrics {
    /// Number of memory pairs processed.
    pub pairs_processed: usize,
    /// Total edges created.
    pub edges_created: usize,
    /// Distribution of edges by type.
    pub edge_type_distribution: HashMap<String, usize>,
    /// Histogram of weighted_agreement values (binned).
    pub agreement_histogram: Vec<usize>,
    /// Latency per pair processing.
    pub latency_per_pair: LatencyStats,
    /// Total processing latency.
    pub total_latency: LatencyStats,
    /// Edges created per second.
    pub edges_per_sec: f64,
    /// Pairs per second throughput.
    pub pairs_per_sec: f64,
}

/// BackgroundGraphBuilder metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackgroundBuilderMetrics {
    /// Number of batches processed.
    pub batches_processed: usize,
    /// Fingerprints processed per batch.
    pub fingerprints_per_batch: LatencyStats,
    /// Queue wait time before processing.
    pub queue_wait_time: LatencyStats,
    /// Batch processing latency.
    pub batch_processing_latency: LatencyStats,
    /// Edges created per batch.
    pub edges_created_per_batch: Vec<usize>,
    /// Total fingerprints processed.
    pub total_fingerprints: usize,
    /// Total edges created.
    pub total_edges: usize,
}

/// Graph expansion stage metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphExpansionMetrics {
    /// Candidates entering the stage.
    pub candidates_in: usize,
    /// Candidates after expansion.
    pub candidates_out: usize,
    /// Expansion ratio (out / in).
    pub expansion_ratio: f64,
    /// Total edges traversed during expansion.
    pub edges_traversed: usize,
    /// Stage latency.
    pub latency: LatencyStats,
    /// Edges followed per candidate.
    pub edges_per_candidate: f64,
}

/// Weight projection (learned edge weights) metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeightProjectionMetrics {
    /// Number of inference calls.
    pub inferences: usize,
    /// Batch size used.
    pub batch_size: usize,
    /// Latency per single inference.
    pub latency_per_inference: LatencyStats,
    /// Throughput (inferences per second).
    pub throughput_per_sec: f64,
    /// Comparison with heuristic weights.
    pub heuristic_comparison: Option<WeightComparisonMetrics>,
}

/// Comparison between learned and heuristic edge weights.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WeightComparisonMetrics {
    /// Mean absolute difference.
    pub mean_abs_diff: f64,
    /// Correlation coefficient.
    pub correlation: f64,
    /// Percentage of edges where learned > heuristic.
    pub learned_higher_pct: f64,
}

/// R-GCN GNN inference metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RGCNMetrics {
    /// Number of nodes in subgraph.
    pub nodes: usize,
    /// Number of edges in subgraph.
    pub edges: usize,
    /// Layer 1 forward pass latency.
    pub layer1_latency: LatencyStats,
    /// Layer 2 forward pass latency.
    pub layer2_latency: LatencyStats,
    /// Total forward pass latency.
    pub total_latency: LatencyStats,
    /// Peak memory during inference.
    pub memory_peak: u64,
    /// Nodes processed per second.
    pub nodes_per_sec: f64,
    /// Edges processed per second.
    pub edges_per_sec: f64,
}

/// Combined report for all graph linking benchmarks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphLinkingReport {
    /// NN-Descent algorithm metrics.
    pub nn_descent: NNDescentMetrics,
    /// EdgeBuilder metrics.
    pub edge_builder: EdgeBuilderMetrics,
    /// BackgroundGraphBuilder metrics.
    pub background_builder: BackgroundBuilderMetrics,
    /// Graph expansion stage metrics.
    pub graph_expansion: GraphExpansionMetrics,
    /// Weight projection metrics.
    pub weight_projection: WeightProjectionMetrics,
    /// R-GCN metrics.
    pub rgcn: RGCNMetrics,
    /// Scale tier used for benchmarking.
    pub tier: Option<String>,
    /// Timestamp of benchmark run.
    pub timestamp: String,
    /// Total benchmark duration in seconds.
    pub total_duration_secs: f64,
}

impl GraphLinkingReport {
    /// Create a new empty report.
    pub fn new() -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            ..Default::default()
        }
    }

    /// Set the tier.
    pub fn with_tier(mut self, tier: ScaleTier) -> Self {
        self.tier = Some(tier.name().to_string());
        self
    }

    /// Convert to JSON.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// Generate markdown report.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();

        md.push_str("# Graph Linking Benchmark Report\n\n");

        if let Some(ref tier) = self.tier {
            md.push_str(&format!("**Tier**: {} memories\n\n", tier));
        }
        md.push_str(&format!("**Timestamp**: {}\n\n", self.timestamp));
        md.push_str(&format!(
            "**Total Duration**: {:.2}s\n\n",
            self.total_duration_secs
        ));

        // NN-Descent
        md.push_str("## NN-Descent\n\n");
        md.push_str(&format!("- Nodes: {}\n", self.nn_descent.num_nodes));
        md.push_str(&format!("- K: {}\n", self.nn_descent.k));
        md.push_str(&format!("- Iterations: {}\n", self.nn_descent.iterations));
        md.push_str(&format!(
            "- Convergence Rate: {:.4}\n",
            self.nn_descent.convergence_rate
        ));
        md.push_str(&format!("- Latency: {}\n", self.nn_descent.latency));
        md.push_str(&format!(
            "- Throughput: {:.0} edges/sec\n\n",
            self.nn_descent.edges_per_sec
        ));

        // EdgeBuilder
        md.push_str("## EdgeBuilder\n\n");
        md.push_str(&format!(
            "- Pairs Processed: {}\n",
            self.edge_builder.pairs_processed
        ));
        md.push_str(&format!(
            "- Edges Created: {}\n",
            self.edge_builder.edges_created
        ));
        md.push_str(&format!(
            "- Latency/Pair: {}\n",
            self.edge_builder.latency_per_pair
        ));
        md.push_str(&format!(
            "- Throughput: {:.0} pairs/sec\n\n",
            self.edge_builder.pairs_per_sec
        ));

        // Graph Expansion
        md.push_str("## Graph Expansion\n\n");
        md.push_str(&format!(
            "- Expansion Ratio: {:.2}x\n",
            self.graph_expansion.expansion_ratio
        ));
        md.push_str(&format!(
            "- Edges/Candidate: {:.1}\n",
            self.graph_expansion.edges_per_candidate
        ));
        md.push_str(&format!("- Latency: {}\n\n", self.graph_expansion.latency));

        // Weight Projection
        md.push_str("## Weight Projection\n\n");
        md.push_str(&format!(
            "- Inferences: {}\n",
            self.weight_projection.inferences
        ));
        md.push_str(&format!(
            "- Latency: {}\n",
            self.weight_projection.latency_per_inference
        ));
        md.push_str(&format!(
            "- Throughput: {:.0} inf/sec\n\n",
            self.weight_projection.throughput_per_sec
        ));

        // R-GCN
        md.push_str("## R-GCN\n\n");
        md.push_str(&format!(
            "- Nodes: {}, Edges: {}\n",
            self.rgcn.nodes, self.rgcn.edges
        ));
        md.push_str(&format!("- Latency: {}\n", self.rgcn.total_latency));
        md.push_str(&format!(
            "- Throughput: {:.0} nodes/sec\n\n",
            self.rgcn.nodes_per_sec
        ));

        md
    }

    /// Print formatted table to stdout.
    pub fn print_table(&self) {
        println!("\n=== Graph Linking Benchmark Results ===");
        if let Some(ref tier) = self.tier {
            println!("Tier: {} memories", tier);
        }
        println!("Timestamp: {}\n", self.timestamp);

        println!("NN-Descent:");
        println!("  Nodes: {}", self.nn_descent.num_nodes);
        println!("  Iterations: {}", self.nn_descent.iterations);
        println!("  Latency: {}", self.nn_descent.latency);

        println!("\nEdgeBuilder:");
        println!("  Pairs: {}", self.edge_builder.pairs_processed);
        println!("  Edges: {}", self.edge_builder.edges_created);
        println!("  Latency/pair: {}", self.edge_builder.latency_per_pair);

        println!("\nWeight Projection:");
        println!(
            "  Throughput: {:.0} inf/sec",
            self.weight_projection.throughput_per_sec
        );
        println!(
            "  Latency: {}",
            self.weight_projection.latency_per_inference
        );

        println!("\nR-GCN:");
        println!("  Nodes: {}, Edges: {}", self.rgcn.nodes, self.rgcn.edges);
        println!("  Latency: {}", self.rgcn.total_latency);

        println!("\nGraph Expansion:");
        println!(
            "  Expansion ratio: {:.2}x",
            self.graph_expansion.expansion_ratio
        );
        println!("  Latency: {}", self.graph_expansion.latency);
    }
}

/// Latency tracker for collecting measurements.
#[derive(Debug, Default)]
pub struct LatencyTracker {
    samples: Vec<Duration>,
}

impl LatencyTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
        }
    }

    /// Record a duration sample.
    pub fn record(&mut self, duration: Duration) {
        self.samples.push(duration);
    }

    /// Record elapsed time from an instant.
    pub fn record_since(&mut self, start: std::time::Instant) {
        self.samples.push(start.elapsed());
    }

    /// Get computed statistics.
    pub fn stats(&self) -> LatencyStats {
        LatencyStats::from_durations(&self.samples)
    }

    /// Get number of samples.
    pub fn count(&self) -> usize {
        self.samples.len()
    }

    /// Clear all samples.
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_stats_from_durations() {
        let durations = vec![
            Duration::from_micros(100),
            Duration::from_micros(200),
            Duration::from_micros(150),
            Duration::from_micros(300),
            Duration::from_micros(250),
        ];

        let stats = LatencyStats::from_durations(&durations);

        assert_eq!(stats.count, 5);
        assert_eq!(stats.min_us, 100);
        assert_eq!(stats.max_us, 300);
        assert_eq!(stats.mean_us, 200);
        assert_eq!(stats.p50_us, 200);
    }

    #[test]
    fn test_latency_tracker() {
        let mut tracker = LatencyTracker::new();

        tracker.record(Duration::from_millis(10));
        tracker.record(Duration::from_millis(20));
        tracker.record(Duration::from_millis(15));

        assert_eq!(tracker.count(), 3);

        let stats = tracker.stats();
        assert_eq!(stats.count, 3);
        assert!(stats.mean_us >= 10_000);
    }

    #[test]
    fn test_report_to_markdown() {
        let report = GraphLinkingReport::new();
        let md = report.to_markdown();

        assert!(md.contains("Graph Linking Benchmark Report"));
        assert!(md.contains("NN-Descent"));
        assert!(md.contains("EdgeBuilder"));
    }

    #[test]
    fn test_report_to_json() {
        let mut report = GraphLinkingReport::new();
        report.nn_descent.iterations = 10;

        let json = report.to_json();
        assert!(json.is_object());
        assert_eq!(json["nn_descent"]["iterations"], 10);
    }
}
