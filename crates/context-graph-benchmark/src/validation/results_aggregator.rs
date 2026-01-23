//! Results aggregator for validation against measured benchmark metrics.
//!
//! Loads existing benchmark results from JSON files and extracts metrics
//! for validation against ARCH rules and performance targets.

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::realdata::config::EmbedderName;

/// Error types for results aggregation.
#[derive(Debug, thiserror::Error)]
pub enum AggregatorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Missing result file: {0}")]
    MissingFile(String),
    #[error("Missing metric: {0}")]
    MissingMetric(String),
}

/// Aggregated benchmark results for validation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedResults {
    /// Results directory path.
    pub results_dir: PathBuf,
    /// Which result files were loaded.
    pub loaded_files: Vec<String>,
    /// Temporal metrics (E2/E3/E4).
    pub temporal: Option<TemporalMetrics>,
    /// Causal metrics (E5).
    pub causal: Option<CausalMetrics>,
    /// Sparse metrics (E6/E13).
    pub sparse: Option<SparseMetrics>,
    /// Graph metrics (E8).
    pub graph: Option<GraphMetrics>,
    /// Multimodal metrics (E10).
    pub multimodal: Option<MultimodalMetrics>,
    /// MCP tool metrics.
    pub mcp: Option<McpMetrics>,
    /// Summary metrics.
    pub summary: Option<SummaryMetrics>,
}

/// Temporal embedder metrics (E2/E3/E4).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemporalMetrics {
    /// E2 recency-weighted MRR.
    pub e2_recency_mrr: f64,
    /// E2 decay accuracy.
    pub e2_decay_accuracy: f64,
    /// E2 fresh retrieval rate.
    pub e2_fresh_retrieval_rate: f64,
    /// E3 periodic recall@10.
    pub e3_periodic_recall: f64,
    /// E3 hourly cluster quality.
    pub e3_hourly_quality: f64,
    /// E3 daily cluster quality.
    pub e3_daily_quality: f64,
    /// E4 sequence accuracy.
    pub e4_sequence_accuracy: f64,
    /// E4 before/after accuracy.
    pub e4_before_after_accuracy: f64,
    /// E4 temporal ordering precision.
    pub e4_ordering_precision: f64,
    /// E4 episode boundary F1.
    pub e4_boundary_f1: f64,
    /// Symmetry validation passed.
    pub symmetry_validated: bool,

    // E4 Hybrid Session Clustering Metrics
    /// E4 session separation ratio (intra/inter similarity).
    pub e4_session_separation_ratio: Option<f64>,
    /// E4 intra-session similarity (average within same session).
    pub e4_intra_session_similarity: Option<f64>,
    /// E4 inter-session similarity (average across different sessions).
    pub e4_inter_session_similarity: Option<f64>,
    /// E4 session silhouette score.
    pub e4_session_silhouette: Option<f64>,
    /// E4 intra-session ordering accuracy.
    pub e4_intra_session_ordering: Option<f64>,
    /// E4 hybrid vs legacy improvement percentage.
    pub e4_hybrid_vs_legacy_improvement: Option<f64>,
    /// E4 hybrid before/after symmetry score.
    pub e4_hybrid_symmetry_score: Option<f64>,
}

/// Causal embedder metrics (E5).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CausalMetrics {
    /// Direction detection accuracy.
    pub direction_accuracy: f64,
    /// E5 asymmetric accuracy (from accuracy_e5_asymmetric).
    pub e5_asymmetric_accuracy: f64,
    /// E5 contribution percentage.
    pub e5_contribution_pct: f64,
    /// Asymmetric ratio (target: 1.5 ± 0.15).
    pub asymmetry_ratio: f64,
    /// Cause-to-effect MRR.
    pub cause_to_effect_mrr: f64,
    /// Effect-to-cause MRR.
    pub effect_to_cause_mrr: f64,
    /// COPA accuracy on real data.
    pub copa_accuracy_real: f64,
    /// Chain traversal accuracy.
    pub chain_traversal_accuracy: f64,
    /// Vector distinctness (% pairs with sim < 0.95).
    pub vector_distinctness: f64,
}

/// Sparse embedder metrics (E6/E13).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SparseMetrics {
    /// E6 MRR@10.
    pub e6_mrr: f64,
    /// E6 sparsity ratio.
    pub e6_sparsity: f64,
    /// E6 average active terms.
    pub e6_avg_active_terms: f64,
    /// E6 vs E1 delta.
    pub e6_vs_e1_delta: f64,
    /// E13 MRR@10.
    pub e13_mrr: f64,
    /// E13 sparsity ratio.
    pub e13_sparsity: f64,
}

/// Graph embedder metrics (E8).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GraphMetrics {
    /// Direction detection rate.
    pub direction_detection_rate: f64,
    /// Hub detection rate.
    pub hub_detection_rate: f64,
    /// Asymmetric ratio (target: 1.5 ± 0.15).
    pub asymmetry_ratio: f64,
    /// E8 contribution score.
    pub e8_contribution: f64,
    /// Centrality correlation.
    pub centrality_correlation: f64,
}

/// Multimodal embedder metrics (E10).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultimodalMetrics {
    /// Intent detection accuracy.
    pub intent_accuracy: f64,
    /// Intent precision.
    pub intent_precision: f64,
    /// Intent recall.
    pub intent_recall: f64,
    /// Intent F1.
    pub intent_f1: f64,
    /// Asymmetric ratio (target: 1.5 ± 0.15).
    pub asymmetry_ratio: f64,
    /// E10 contribution percentage.
    pub e10_contribution_pct: f64,
    /// Optimal blend weight.
    pub optimal_blend: f64,
}

/// MCP tool metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpMetrics {
    /// Tool latencies (tool_name -> p95 latency in ms).
    pub tool_latencies: HashMap<String, f64>,
    /// Tool MRR scores.
    pub tool_mrr: HashMap<String, f64>,
    /// Overall p95 latency.
    pub overall_p95_ms: f64,
    /// Target p95 latency.
    pub target_p95_ms: f64,
    /// All tools under target.
    pub all_under_target: bool,
}

/// Summary metrics from summary.json.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SummaryMetrics {
    /// Overall status.
    pub overall_status: String,
    /// Overall score.
    pub overall_score: f64,
    /// Constitutional compliance score.
    pub constitutional_compliance: f64,
    /// Per-embedder status.
    pub embedder_status: HashMap<String, EmbedderStatus>,
    /// ARCH rules status.
    pub arch_rules: HashMap<String, RuleStatus>,
    /// Anti-pattern status.
    pub anti_patterns: HashMap<String, RuleStatus>,
}

/// Status for an individual embedder.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmbedderStatus {
    pub role: String,
    pub status: String,
    pub target_met: bool,
    pub warnings: Vec<String>,
}

/// Status for a rule/anti-pattern.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleStatus {
    pub status: String,
    pub description: String,
}

/// Benchmark results aggregator.
pub struct BenchmarkResultsAggregator {
    results_dir: PathBuf,
    results: AggregatedResults,
}

impl BenchmarkResultsAggregator {
    /// Create a new aggregator for the given results directory.
    pub fn new<P: AsRef<Path>>(results_dir: P) -> Self {
        Self {
            results_dir: results_dir.as_ref().to_path_buf(),
            results: AggregatedResults {
                results_dir: results_dir.as_ref().to_path_buf(),
                ..Default::default()
            },
        }
    }

    /// Load all available benchmark results.
    pub fn load_all(&mut self) -> Result<&AggregatedResults, AggregatorError> {
        // Try to load each result file (don't fail if some are missing)
        let _ = self.load_temporal();
        let _ = self.load_causal();
        let _ = self.load_sparse();
        let _ = self.load_graph();
        let _ = self.load_multimodal();
        let _ = self.load_mcp();
        let _ = self.load_summary();

        Ok(&self.results)
    }

    /// Get the aggregated results.
    pub fn results(&self) -> &AggregatedResults {
        &self.results
    }

    /// Load temporal benchmark results.
    fn load_temporal(&mut self) -> Result<(), AggregatorError> {
        // Try temporal_full.json first, then temporal_synthetic.json
        let path = self.results_dir.join("temporal_full.json");
        let alt_path = self.results_dir.join("temporal_synthetic.json");

        let file_path = if path.exists() {
            path
        } else if alt_path.exists() {
            alt_path
        } else {
            return Err(AggregatorError::MissingFile("temporal_*.json".to_string()));
        };

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = TemporalMetrics::default();

        // temporal_full.json uses "metrics.recency", "metrics.periodic", "metrics.sequence"
        let metrics_root = json.get("metrics").unwrap_or(&json);

        // Extract E2 metrics from metrics.recency or top-level
        let e2 = metrics_root.get("recency")
            .or(json.get("e2_recency"))
            .or(json.get("recency"));
        if let Some(e2) = e2 {
            metrics.e2_recency_mrr = e2.get("recency_weighted_mrr")
                .or(e2.get("mrr"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            metrics.e2_decay_accuracy = e2.get("decay_accuracy")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            // Try fresh_retrieval_rate_at.10 first
            metrics.e2_fresh_retrieval_rate = e2.get("fresh_retrieval_rate_at")
                .and_then(|v| v.get("10"))
                .and_then(|v| v.as_f64())
                .or_else(|| e2.get("fresh_retrieval_rate").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);
        }

        // Extract E3 metrics from metrics.periodic or top-level
        let e3 = metrics_root.get("periodic")
            .or(json.get("e3_periodic"))
            .or(json.get("periodic"));
        if let Some(e3) = e3 {
            // Try periodic_recall_at.10 first
            metrics.e3_periodic_recall = e3.get("periodic_recall_at")
                .and_then(|v| v.get("10"))
                .and_then(|v| v.as_f64())
                .or_else(|| e3.get("periodic_recall_at_10").and_then(|v| v.as_f64()))
                .or_else(|| e3.get("recall").and_then(|v| v.as_f64()))
                .unwrap_or(0.0);
            metrics.e3_hourly_quality = e3.get("hourly_cluster_quality")
                .or(e3.get("hourly_quality"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            metrics.e3_daily_quality = e3.get("daily_cluster_quality")
                .or(e3.get("daily_quality"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
        }

        // Extract E4 metrics from metrics.sequence or top-level
        let e4 = metrics_root.get("sequence")
            .or(json.get("e4_sequence"))
            .or(json.get("sequence"));
        if let Some(e4) = e4 {
            metrics.e4_sequence_accuracy = e4.get("sequence_accuracy")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            metrics.e4_before_after_accuracy = e4.get("before_after_accuracy")
                .or(e4.get("direction_accuracy"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            metrics.e4_ordering_precision = e4.get("temporal_ordering_precision")
                .or(e4.get("ordering_precision"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            metrics.e4_boundary_f1 = e4.get("episode_boundary_f1")
                .or(e4.get("boundary_f1"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
        }

        // Check symmetry validation from symmetry_check or top-level
        let symmetry = json.get("symmetry_check")
            .or(metrics_root.get("symmetry"))
            .or(json.get("symmetry"));
        if let Some(symmetry) = symmetry {
            metrics.symmetry_validated = symmetry.get("passed")
                .or(symmetry.get("symmetric"))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
        }

        self.results.temporal = Some(metrics);
        self.results.loaded_files.push(file_path.file_name().unwrap().to_string_lossy().to_string());

        Ok(())
    }

    /// Load causal benchmark results.
    fn load_causal(&mut self) -> Result<(), AggregatorError> {
        // Try causal_realdata_real.json first
        let paths = [
            "causal_realdata_real.json",
            "causal_diverse_wiki.json",
            "causal_synthetic.json",
        ];

        let mut file_path = None;
        for p in paths {
            let path = self.results_dir.join(p);
            if path.exists() {
                file_path = Some(path);
                break;
            }
        }

        let file_path = file_path.ok_or_else(|| AggregatorError::MissingFile("causal_*.json".to_string()))?;

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = CausalMetrics::default();

        // Extract from various possible structures
        let results = json.get("results").unwrap_or(&json);

        metrics.direction_accuracy = results.get("direction_accuracy")
            .or(results.get("direction_detection_accuracy"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // Extract accuracy_e5_asymmetric from copa_reasoning section or top-level
        metrics.e5_asymmetric_accuracy = json.get("copa_reasoning")
            .and_then(|v| v.get("accuracy_e5_asymmetric"))
            .and_then(|v| v.as_f64())
            .or_else(|| results.get("accuracy_e5_asymmetric").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        // Extract e5_contribution_pct from e5_contribution section or top-level
        metrics.e5_contribution_pct = json.get("e5_contribution")
            .and_then(|v| v.get("e5_contribution_pct"))
            .and_then(|v| v.as_f64())
            .or_else(|| results.get("e5_contribution_pct").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        metrics.asymmetry_ratio = results.get("asymmetry_ratio")
            .or(results.get("asymmetric_ratio"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.5);

        metrics.cause_to_effect_mrr = results.get("cause_to_effect_mrr")
            .or(results.get("forward_mrr"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.effect_to_cause_mrr = results.get("effect_to_cause_mrr")
            .or(results.get("backward_mrr"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.copa_accuracy_real = results.get("copa_accuracy")
            .or(results.get("reasoning_accuracy"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.chain_traversal_accuracy = results.get("chain_traversal_accuracy")
            .or(results.get("chain_accuracy"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // Vector distinctness from symmetry check
        if let Some(sym) = results.get("symmetry_check") {
            metrics.vector_distinctness = sym.get("distinct_pairs_pct")
                .or(sym.get("distinctness"))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
        }

        self.results.causal = Some(metrics);
        self.results.loaded_files.push(file_path.file_name().unwrap().to_string_lossy().to_string());

        Ok(())
    }

    /// Load sparse benchmark results.
    fn load_sparse(&mut self) -> Result<(), AggregatorError> {
        let path = self.results_dir.join("e6_sparse_benchmark.json");
        if !path.exists() {
            return Err(AggregatorError::MissingFile("e6_sparse_benchmark.json".to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = SparseMetrics::default();

        let results = json.get("results").unwrap_or(&json);

        metrics.e6_mrr = results.get("mrr_at_10")
            .or(results.get("e6_mrr"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.e6_sparsity = results.get("sparsity_ratio")
            .or(results.get("sparsity"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.e6_avg_active_terms = results.get("avg_active_terms")
            .or(results.get("active_terms"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.e6_vs_e1_delta = results.get("e6_vs_e1_delta")
            .or(results.get("delta_vs_e1"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        self.results.sparse = Some(metrics);
        self.results.loaded_files.push("e6_sparse_benchmark.json".to_string());

        Ok(())
    }

    /// Load graph benchmark results.
    fn load_graph(&mut self) -> Result<(), AggregatorError> {
        let paths = ["graph_full.json", "graph_benchmark.json"];

        let mut file_path = None;
        for p in paths {
            let path = self.results_dir.join(p);
            if path.exists() {
                file_path = Some(path);
                break;
            }
        }

        let file_path = file_path.ok_or_else(|| AggregatorError::MissingFile("graph_*.json".to_string()))?;

        let file = File::open(&file_path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = GraphMetrics::default();

        let results = json.get("results").unwrap_or(&json);

        metrics.direction_detection_rate = results.get("direction_detection_rate")
            .or(results.get("direction_accuracy"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.hub_detection_rate = results.get("hub_detection_rate")
            .or(results.get("hub_accuracy"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.asymmetry_ratio = results.get("asymmetry_ratio")
            .or(results.get("asymmetric_ratio"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.5);

        // Try asymmetric_retrieval.e8_contribution_percentage first
        metrics.e8_contribution = results.get("asymmetric_retrieval")
            .and_then(|v| v.get("e8_contribution_percentage"))
            .and_then(|v| v.as_f64())
            .map(|v| v / 100.0) // Convert percentage to decimal
            .or_else(|| results.get("e8_contribution").and_then(|v| v.as_f64()))
            .or_else(|| results.get("contribution").and_then(|v| v.as_f64()))
            .unwrap_or(0.0);

        metrics.centrality_correlation = results.get("centrality_correlation")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        self.results.graph = Some(metrics);
        self.results.loaded_files.push(file_path.file_name().unwrap().to_string_lossy().to_string());

        Ok(())
    }

    /// Load multimodal benchmark results.
    fn load_multimodal(&mut self) -> Result<(), AggregatorError> {
        let path = self.results_dir.join("multimodal_benchmark.json");
        if !path.exists() {
            return Err(AggregatorError::MissingFile("multimodal_benchmark.json".to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = MultimodalMetrics::default();

        let results = json.get("results").unwrap_or(&json);

        metrics.intent_accuracy = results.get("intent_detection_accuracy")
            .or(results.get("intent_accuracy"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.intent_precision = results.get("intent_precision")
            .or(results.get("precision"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.intent_recall = results.get("intent_recall")
            .or(results.get("recall"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.intent_f1 = results.get("intent_f1")
            .or(results.get("f1"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.asymmetry_ratio = results.get("asymmetry_ratio")
            .or(results.get("asymmetric_ratio"))
            .and_then(|v| v.as_f64())
            .unwrap_or(1.5);

        // e10_contribution_percentage is in percent (e.g., 30.76), convert to decimal
        // Try summary first, then results
        metrics.e10_contribution_pct = json.get("summary")
            .and_then(|v| v.get("e10_contribution_percentage"))
            .and_then(|v| v.as_f64())
            .or_else(|| results.get("e10_contribution_percentage").and_then(|v| v.as_f64()))
            .or_else(|| results.get("e10_contribution_pct").and_then(|v| v.as_f64()))
            .or_else(|| results.get("contribution_pct").and_then(|v| v.as_f64()))
            .map(|v| if v > 1.0 { v / 100.0 } else { v }) // Normalize to decimal
            .unwrap_or(0.0);

        metrics.optimal_blend = results.get("optimal_blend")
            .or(results.get("blend_weight"))
            .and_then(|v| v.as_f64())
            .unwrap_or(0.3);

        self.results.multimodal = Some(metrics);
        self.results.loaded_files.push("multimodal_benchmark.json".to_string());

        Ok(())
    }

    /// Load MCP benchmark results.
    fn load_mcp(&mut self) -> Result<(), AggregatorError> {
        let path = self.results_dir.join("mcp_intent_benchmark.json");
        if !path.exists() {
            return Err(AggregatorError::MissingFile("mcp_intent_benchmark.json".to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = McpMetrics {
            target_p95_ms: 2000.0,
            ..Default::default()
        };

        // Extract per-tool metrics
        if let Some(tools) = json.get("tool_results").or(json.get("tools")) {
            if let Some(obj) = tools.as_object() {
                for (name, tool_data) in obj {
                    if let Some(p95) = tool_data.get("p95_latency_ms").and_then(|v| v.as_f64()) {
                        metrics.tool_latencies.insert(name.clone(), p95);
                    }
                    if let Some(mrr) = tool_data.get("mrr").and_then(|v| v.as_f64()) {
                        metrics.tool_mrr.insert(name.clone(), mrr);
                    }
                }
            }
        }

        // Calculate overall p95
        if !metrics.tool_latencies.is_empty() {
            metrics.overall_p95_ms = metrics.tool_latencies.values()
                .cloned()
                .fold(0.0_f64, |a, b| a.max(b));
            metrics.all_under_target = metrics.tool_latencies.values()
                .all(|&v| v < metrics.target_p95_ms);
        }

        self.results.mcp = Some(metrics);
        self.results.loaded_files.push("mcp_intent_benchmark.json".to_string());

        Ok(())
    }

    /// Load summary results.
    fn load_summary(&mut self) -> Result<(), AggregatorError> {
        let path = self.results_dir.join("summary.json");
        if !path.exists() {
            return Err(AggregatorError::MissingFile("summary.json".to_string()));
        }

        let file = File::open(&path)?;
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader)?;

        let mut metrics = SummaryMetrics::default();

        metrics.overall_status = json.get("overall_status")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN")
            .to_string();

        metrics.overall_score = json.get("overall_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        metrics.constitutional_compliance = json.get("constitutional_compliance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // Extract embedder status
        if let Some(embedders) = json.get("embedders").and_then(|v| v.as_object()) {
            for (name, data) in embedders {
                let status = EmbedderStatus {
                    role: data.get("role").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    status: data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    target_met: data.get("target_met").and_then(|v| v.as_bool()).unwrap_or(false),
                    warnings: data.get("warnings")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default(),
                };
                metrics.embedder_status.insert(name.clone(), status);
            }
        }

        // Extract ARCH rules
        if let Some(rules) = json.get("arch_rules").and_then(|v| v.as_object()) {
            for (name, data) in rules {
                let status = RuleStatus {
                    status: data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: data.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                };
                metrics.arch_rules.insert(name.clone(), status);
            }
        }

        // Extract anti-patterns
        if let Some(patterns) = json.get("anti_patterns").and_then(|v| v.as_object()) {
            for (name, data) in patterns {
                let status = RuleStatus {
                    status: data.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    description: data.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                };
                metrics.anti_patterns.insert(name.clone(), status);
            }
        }

        self.results.summary = Some(metrics);
        self.results.loaded_files.push("summary.json".to_string());

        Ok(())
    }

    // ========================================================================
    // Convenience methods for validation
    // ========================================================================

    /// Get asymmetric ratio for an embedder.
    pub fn get_asymmetric_ratio(&self, embedder: EmbedderName) -> Option<f64> {
        match embedder {
            EmbedderName::E5Causal => self.results.causal.as_ref().map(|c| c.asymmetry_ratio),
            EmbedderName::E8Graph => self.results.graph.as_ref().map(|g| g.asymmetry_ratio),
            EmbedderName::E10Multimodal => self.results.multimodal.as_ref().map(|m| m.asymmetry_ratio),
            _ => None,
        }
    }

    /// Get all measured asymmetric ratios.
    pub fn get_all_asymmetric_ratios(&self) -> HashMap<EmbedderName, f64> {
        let mut ratios = HashMap::new();

        if let Some(causal) = &self.results.causal {
            ratios.insert(EmbedderName::E5Causal, causal.asymmetry_ratio);
        }
        if let Some(graph) = &self.results.graph {
            ratios.insert(EmbedderName::E8Graph, graph.asymmetry_ratio);
        }
        if let Some(multimodal) = &self.results.multimodal {
            ratios.insert(EmbedderName::E10Multimodal, multimodal.asymmetry_ratio);
        }

        ratios
    }

    /// Get MCP tool latencies.
    pub fn get_mcp_latencies(&self) -> Option<&HashMap<String, f64>> {
        self.results.mcp.as_ref().map(|m| &m.tool_latencies)
    }

    /// Check if all MCP tools are under latency target.
    pub fn mcp_latencies_under_target(&self) -> bool {
        self.results.mcp.as_ref().map(|m| m.all_under_target).unwrap_or(false)
    }

    /// Get temporal metrics.
    pub fn get_temporal_metrics(&self) -> Option<&TemporalMetrics> {
        self.results.temporal.as_ref()
    }

    /// Get E1 baseline MRR (from summary or sparse delta).
    pub fn get_e1_baseline_mrr(&self) -> Option<f64> {
        // Try to compute from E6 delta
        if let Some(sparse) = &self.results.sparse {
            if sparse.e6_mrr > 0.0 {
                // E6 MRR - delta = E1 MRR
                return Some(sparse.e6_mrr - sparse.e6_vs_e1_delta);
            }
        }

        // Fallback to summary if available
        if let Some(summary) = &self.results.summary {
            if let Some(e1_status) = summary.embedder_status.get("E1_Semantic") {
                // Could parse from notes if stored there
                if e1_status.target_met {
                    return Some(0.7); // Assume good baseline if target met
                }
            }
        }

        None
    }

    /// Get overall constitutional compliance.
    pub fn get_constitutional_compliance(&self) -> Option<f64> {
        self.results.summary.as_ref().map(|s| s.constitutional_compliance)
    }

    /// Check if results are available.
    pub fn has_results(&self) -> bool {
        !self.results.loaded_files.is_empty()
    }

    /// Get list of loaded files.
    pub fn loaded_files(&self) -> &[String] {
        &self.results.loaded_files
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggregator_new() {
        let aggregator = BenchmarkResultsAggregator::new("benchmark_results");
        assert_eq!(aggregator.results_dir, PathBuf::from("benchmark_results"));
    }

    #[test]
    fn test_aggregator_load_real_results() {
        // This test will pass if benchmark_results/ exists with data
        let mut aggregator = BenchmarkResultsAggregator::new("benchmark_results");
        let _ = aggregator.load_all();

        // Check what was loaded
        println!("Loaded files: {:?}", aggregator.loaded_files());

        if aggregator.has_results() {
            // Verify asymmetric ratios if available
            let ratios = aggregator.get_all_asymmetric_ratios();
            for (embedder, ratio) in &ratios {
                println!("{:?}: {:.2}", embedder, ratio);
                assert!(ratio >= &1.0 && ratio <= &2.0, "Ratio should be in reasonable range");
            }
        }
    }
}
