//! Unified benchmark configuration for real data benchmarks.
//!
//! This module provides a centralized configuration structure for running
//! comprehensive benchmarks across all 13 embedders using real HuggingFace data.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Names of the 13 embedders in the fingerprint system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmbedderName {
    /// E1 - Foundation semantic embedder (V_meaning)
    E1Semantic,
    /// E2 - Temporal recency (V_freshness) - POST-RETRIEVAL ONLY
    E2Recency,
    /// E3 - Temporal periodic (V_periodicity) - POST-RETRIEVAL ONLY
    E3Periodic,
    /// E4 - Temporal sequence (V_ordering) - POST-RETRIEVAL ONLY
    E4Sequence,
    /// E5 - Causal reasoning (V_causality) - Asymmetric
    E5Causal,
    /// E6 - Sparse keyword (V_selectivity)
    E6Sparse,
    /// E7 - Code correctness (V_correctness)
    E7Code,
    /// E8 - Graph connectivity (V_connectivity) - Asymmetric
    E8Graph,
    /// E9 - Hyperdimensional robustness (V_robustness)
    E9HDC,
    /// E10 - Multimodal intent (V_multimodality) - Asymmetric
    E10Multimodal,
    /// E11 - Entity factuality (V_factuality)
    E11Entity,
    /// E12 - Late interaction precision (V_precision) - RERANK ONLY
    E12LateInteraction,
    /// E13 - SPLADE expansion (V_keyword_precision) - RECALL ONLY
    E13SPLADE,
}

impl EmbedderName {
    /// All 13 embedders.
    pub fn all() -> Vec<Self> {
        vec![
            Self::E1Semantic,
            Self::E2Recency,
            Self::E3Periodic,
            Self::E4Sequence,
            Self::E5Causal,
            Self::E6Sparse,
            Self::E7Code,
            Self::E8Graph,
            Self::E9HDC,
            Self::E10Multimodal,
            Self::E11Entity,
            Self::E12LateInteraction,
            Self::E13SPLADE,
        ]
    }

    /// Semantic embedders (contribute to topic detection with weight 1.0).
    pub fn semantic() -> Vec<Self> {
        vec![
            Self::E1Semantic,
            Self::E5Causal,
            Self::E6Sparse,
            Self::E7Code,
            Self::E10Multimodal,
            Self::E12LateInteraction,
            Self::E13SPLADE,
        ]
    }

    /// Temporal embedders (never in similarity fusion, POST-RETRIEVAL only).
    pub fn temporal() -> Vec<Self> {
        vec![Self::E2Recency, Self::E3Periodic, Self::E4Sequence]
    }

    /// Relational embedders (weight 0.5 for topic detection).
    pub fn relational() -> Vec<Self> {
        vec![Self::E8Graph, Self::E11Entity]
    }

    /// Structural embedders (weight 0.5 for topic detection).
    pub fn structural() -> Vec<Self> {
        vec![Self::E9HDC]
    }

    /// Asymmetric embedders (require directional similarity).
    pub fn asymmetric() -> Vec<Self> {
        vec![Self::E5Causal, Self::E8Graph, Self::E10Multimodal]
    }

    /// Get the topic weight for this embedder (per CLAUDE.md ARCH-09).
    pub fn topic_weight(&self) -> f64 {
        match self {
            // Semantic: 1.0
            Self::E1Semantic
            | Self::E5Causal
            | Self::E6Sparse
            | Self::E7Code
            | Self::E10Multimodal
            | Self::E12LateInteraction
            | Self::E13SPLADE => 1.0,
            // Temporal: 0.0 (excluded from topic detection)
            Self::E2Recency | Self::E3Periodic | Self::E4Sequence => 0.0,
            // Relational/Structural: 0.5
            Self::E8Graph | Self::E9HDC | Self::E11Entity => 0.5,
        }
    }

    /// Check if this embedder is used for divergence detection.
    pub fn used_for_divergence(&self) -> bool {
        matches!(
            self,
            Self::E1Semantic
                | Self::E5Causal
                | Self::E6Sparse
                | Self::E7Code
                | Self::E10Multimodal
                | Self::E12LateInteraction
                | Self::E13SPLADE
        )
    }

    /// Get the embedder index (0-12).
    pub fn index(&self) -> usize {
        match self {
            Self::E1Semantic => 0,
            Self::E2Recency => 1,
            Self::E3Periodic => 2,
            Self::E4Sequence => 3,
            Self::E5Causal => 4,
            Self::E6Sparse => 5,
            Self::E7Code => 6,
            Self::E8Graph => 7,
            Self::E9HDC => 8,
            Self::E10Multimodal => 9,
            Self::E11Entity => 10,
            Self::E12LateInteraction => 11,
            Self::E13SPLADE => 12,
        }
    }

    /// Get embedder name as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::E1Semantic => "E1_Semantic",
            Self::E2Recency => "E2_Recency",
            Self::E3Periodic => "E3_Periodic",
            Self::E4Sequence => "E4_Sequence",
            Self::E5Causal => "E5_Causal",
            Self::E6Sparse => "E6_Sparse",
            Self::E7Code => "E7_Code",
            Self::E8Graph => "E8_Graph",
            Self::E9HDC => "E9_HDC",
            Self::E10Multimodal => "E10_Multimodal",
            Self::E11Entity => "E11_Entity",
            Self::E12LateInteraction => "E12_LateInteraction",
            Self::E13SPLADE => "E13_SPLADE",
        }
    }

    /// Parse from string.
    pub fn parse_from(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "E1" | "E1_SEMANTIC" | "SEMANTIC" => Some(Self::E1Semantic),
            "E2" | "E2_RECENCY" | "RECENCY" => Some(Self::E2Recency),
            "E3" | "E3_PERIODIC" | "PERIODIC" => Some(Self::E3Periodic),
            "E4" | "E4_SEQUENCE" | "SEQUENCE" => Some(Self::E4Sequence),
            "E5" | "E5_CAUSAL" | "CAUSAL" => Some(Self::E5Causal),
            "E6" | "E6_SPARSE" | "SPARSE" => Some(Self::E6Sparse),
            "E7" | "E7_CODE" | "CODE" => Some(Self::E7Code),
            "E8" | "E8_GRAPH" | "GRAPH" => Some(Self::E8Graph),
            "E9" | "E9_HDC" | "HDC" => Some(Self::E9HDC),
            "E10" | "E10_MULTIMODAL" | "MULTIMODAL" => Some(Self::E10Multimodal),
            "E11" | "E11_ENTITY" | "ENTITY" => Some(Self::E11Entity),
            "E12" | "E12_LATEINTERACTION" | "LATEINTERACTION" => Some(Self::E12LateInteraction),
            "E13" | "E13_SPLADE" | "SPLADE" => Some(Self::E13SPLADE),
            _ => None,
        }
    }
}

impl std::fmt::Display for EmbedderName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Temporal metadata injection configuration.
///
/// Wikipedia data lacks timestamps, so we inject synthetic but realistic temporal metadata
/// for E2/E3/E4 benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalInjectionConfig {
    /// Base timestamp for the dataset (Unix epoch ms).
    pub base_timestamp_ms: i64,

    /// E2 Recency: Time span in days to distribute documents over.
    pub recency_span_days: u32,

    /// E2 Recency: Decay half-life in seconds.
    pub recency_half_life_secs: u64,

    /// E3 Periodic: Assign time-of-day patterns based on topic clusters.
    pub periodic_enabled: bool,

    /// E3 Periodic: Number of hour clusters to create.
    pub periodic_hour_clusters: usize,

    /// E4 Sequence: Number of synthetic sessions to create.
    pub sequence_num_sessions: usize,

    /// E4 Sequence: Chunks per session.
    pub sequence_chunks_per_session: usize,
}

impl Default for TemporalInjectionConfig {
    fn default() -> Self {
        Self {
            base_timestamp_ms: chrono::Utc::now().timestamp_millis(),
            recency_span_days: 365,        // 1 year span
            recency_half_life_secs: 86400, // 1 day half-life
            periodic_enabled: true,
            periodic_hour_clusters: 8, // 3-hour clusters
            sequence_num_sessions: 100,
            sequence_chunks_per_session: 20,
        }
    }
}

/// Fusion strategy for multi-space retrieval.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FusionStrategy {
    /// E1 only (baseline).
    #[default]
    E1Only,
    /// E1 + enhancement embedders via weighted RRF.
    MultiSpace,
    /// E13 recall -> E1 dense -> E12 rerank (3-stage pipeline).
    Pipeline,
    /// Custom embedder combination.
    Custom,
}

/// Unified benchmark configuration for real data benchmarks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedBenchmarkConfig {
    /// Directory containing chunks.jsonl and metadata.json.
    pub data_dir: PathBuf,

    /// Maximum chunks to load (0 = unlimited).
    pub max_chunks: usize,

    /// Directory for embedding checkpoints.
    pub checkpoint_dir: Option<PathBuf>,

    /// Checkpoint interval (save every N embeddings).
    pub checkpoint_interval: usize,

    /// Number of queries to generate for evaluation.
    pub num_queries: usize,

    /// K values for P@K, R@K metrics.
    pub k_values: Vec<usize>,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Embedders to evaluate (default: all 13).
    pub embedders: Vec<EmbedderName>,

    /// Output directory for results.
    pub output_dir: PathBuf,

    /// Temporal metadata injection configuration.
    pub temporal_config: TemporalInjectionConfig,

    /// Whether to run ablation studies.
    pub run_ablation: bool,

    /// Whether to run fusion strategy comparison.
    pub run_fusion_comparison: bool,

    /// Whether to run cross-embedder correlation analysis.
    pub run_correlation_analysis: bool,

    /// Batch size for GPU embedding.
    pub batch_size: usize,

    /// Show progress during embedding.
    pub show_progress: bool,
}

impl Default for UnifiedBenchmarkConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::from("data/hf_benchmark_diverse"),
            max_chunks: 10000,
            checkpoint_dir: Some(PathBuf::from("data/hf_benchmark_diverse/checkpoints")),
            checkpoint_interval: 1000,
            num_queries: 100,
            k_values: vec![1, 5, 10, 20],
            seed: 42,
            embedders: EmbedderName::all(),
            output_dir: PathBuf::from("benchmark_results"),
            temporal_config: TemporalInjectionConfig::default(),
            run_ablation: true,
            run_fusion_comparison: true,
            run_correlation_analysis: true,
            batch_size: 32,
            show_progress: true,
        }
    }
}

impl UnifiedBenchmarkConfig {
    /// Create config for quick testing with limited data.
    pub fn quick_test() -> Self {
        Self {
            max_chunks: 500,
            num_queries: 20,
            k_values: vec![5, 10],
            run_ablation: false,
            run_fusion_comparison: false,
            run_correlation_analysis: false,
            checkpoint_dir: None,
            ..Default::default()
        }
    }

    /// Create config for full benchmark.
    pub fn full_benchmark() -> Self {
        Self {
            max_chunks: 0, // unlimited
            num_queries: 500,
            k_values: vec![1, 5, 10, 20, 50, 100],
            run_ablation: true,
            run_fusion_comparison: true,
            run_correlation_analysis: true,
            ..Default::default()
        }
    }

    /// Get only semantic embedders.
    pub fn with_semantic_only(mut self) -> Self {
        self.embedders = EmbedderName::semantic();
        self
    }

    /// Set specific embedders.
    pub fn with_embedders(mut self, embedders: Vec<EmbedderName>) -> Self {
        self.embedders = embedders;
        self
    }

    /// Set data directory.
    pub fn with_data_dir(mut self, path: PathBuf) -> Self {
        self.data_dir = path;
        self
    }

    /// Set output directory.
    pub fn with_output_dir(mut self, path: PathBuf) -> Self {
        self.output_dir = path;
        self
    }

    /// Set max chunks.
    pub fn with_max_chunks(mut self, n: usize) -> Self {
        self.max_chunks = n;
        self
    }

    /// Disable checkpointing.
    pub fn without_checkpoints(mut self) -> Self {
        self.checkpoint_dir = None;
        self
    }

    /// Validate configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Check data directory exists
        if !self.data_dir.exists() {
            return Err(ConfigError::DataDirNotFound(self.data_dir.clone()));
        }

        // Check chunks.jsonl exists
        let chunks_path = self.data_dir.join("chunks.jsonl");
        if !chunks_path.exists() {
            return Err(ConfigError::ChunksFileNotFound(chunks_path));
        }

        // Check metadata.json exists
        let metadata_path = self.data_dir.join("metadata.json");
        if !metadata_path.exists() {
            return Err(ConfigError::MetadataFileNotFound(metadata_path));
        }

        // Check k_values are valid
        if self.k_values.is_empty() {
            return Err(ConfigError::EmptyKValues);
        }

        // Check num_queries is valid
        if self.num_queries == 0 {
            return Err(ConfigError::ZeroQueries);
        }

        Ok(())
    }
}

/// Configuration errors.
#[derive(Debug)]
pub enum ConfigError {
    DataDirNotFound(PathBuf),
    ChunksFileNotFound(PathBuf),
    MetadataFileNotFound(PathBuf),
    EmptyKValues,
    ZeroQueries,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataDirNotFound(p) => write!(f, "Data directory not found: {}", p.display()),
            Self::ChunksFileNotFound(p) => write!(f, "chunks.jsonl not found: {}", p.display()),
            Self::MetadataFileNotFound(p) => write!(f, "metadata.json not found: {}", p.display()),
            Self::EmptyKValues => write!(f, "k_values cannot be empty"),
            Self::ZeroQueries => write!(f, "num_queries must be > 0"),
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_categories() {
        // Semantic embedders should have weight 1.0
        for e in EmbedderName::semantic() {
            assert_eq!(e.topic_weight(), 1.0, "{} should have weight 1.0", e);
        }

        // Temporal embedders should have weight 0.0
        for e in EmbedderName::temporal() {
            assert_eq!(e.topic_weight(), 0.0, "{} should have weight 0.0", e);
        }

        // Relational/structural should have weight 0.5
        for e in EmbedderName::relational() {
            assert_eq!(e.topic_weight(), 0.5, "{} should have weight 0.5", e);
        }
        for e in EmbedderName::structural() {
            assert_eq!(e.topic_weight(), 0.5, "{} should have weight 0.5", e);
        }
    }

    #[test]
    fn test_embedder_count() {
        assert_eq!(EmbedderName::all().len(), 13);
        assert_eq!(EmbedderName::semantic().len(), 7);
        assert_eq!(EmbedderName::temporal().len(), 3);
        assert_eq!(EmbedderName::relational().len(), 2);
        assert_eq!(EmbedderName::structural().len(), 1);
        assert_eq!(EmbedderName::asymmetric().len(), 3);
    }

    #[test]
    fn test_embedder_parsing() {
        assert_eq!(
            EmbedderName::parse_from("E1"),
            Some(EmbedderName::E1Semantic)
        );
        assert_eq!(EmbedderName::parse_from("e5"), Some(EmbedderName::E5Causal));
        assert_eq!(
            EmbedderName::parse_from("CAUSAL"),
            Some(EmbedderName::E5Causal)
        );
        assert_eq!(EmbedderName::parse_from("invalid"), None);
    }

    #[test]
    fn test_default_config() {
        let config = UnifiedBenchmarkConfig::default();
        assert_eq!(config.embedders.len(), 13);
        assert_eq!(config.k_values, vec![1, 5, 10, 20]);
        assert_eq!(config.max_chunks, 10000);
    }

    #[test]
    fn test_quick_test_config() {
        let config = UnifiedBenchmarkConfig::quick_test();
        assert_eq!(config.max_chunks, 500);
        assert!(!config.run_ablation);
        assert!(config.checkpoint_dir.is_none());
    }
}
