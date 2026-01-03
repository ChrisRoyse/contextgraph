//! Configuration types for Knowledge Graph components.
//!
//! This module provides configuration structures for:
//! - FAISS IVF-PQ vector index (IndexConfig)
//! - Hyperbolic/Poincare ball geometry (HyperbolicConfig)
//! - Entailment cones for IS-A queries (ConeConfig)
//!
//! # Constitution Reference
//!
//! - perf.latency.faiss_1M_k100: <2ms (drives nlist/nprobe defaults)
//! - embeddings.models.E7_Code: 1536D (default dimension)
//!
//! TODO: Full implementation in M04-T01, M04-T02, M04-T03

use serde::{Deserialize, Serialize};

/// Configuration for FAISS IVF-PQ GPU index.
///
/// Configures the FAISS GPU index for 10M+ vector search with <5ms latency.
///
/// # Performance Targets
/// - 10M vectors, k=10: <5ms latency
/// - 10M vectors, k=100: <10ms latency
/// - Memory: ~8GB VRAM for 10M 1536D vectors with PQ64x8
///
/// # Constitution Reference
/// - perf.latency.faiss_1M_k100: <2ms
/// - stack.deps: faiss@0.12+gpu
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexConfig {
    /// Vector dimension (must match embedding dimension).
    /// Default: 1536 per constitution embeddings.models.E7_Code
    pub dimension: usize,

    /// Number of inverted lists (clusters).
    /// Default: 16384 = 4 * sqrt(10M) for optimal recall/speed tradeoff
    pub nlist: usize,

    /// Number of clusters to probe during search.
    /// Default: 128 balances accuracy vs search time
    pub nprobe: usize,

    /// Number of product quantization segments.
    /// Must evenly divide dimension. Default: 64 (1536/64 = 24 bytes per segment)
    pub pq_segments: usize,

    /// Bits per quantization code.
    /// Valid values: 4, 8, 12, 16. Default: 8
    pub pq_bits: u8,

    /// GPU device ID.
    /// Default: 0 (primary GPU)
    pub gpu_id: i32,

    /// Use float16 for reduced memory.
    /// Default: true (halves VRAM usage)
    pub use_float16: bool,

    /// Minimum vectors required for training (256 * nlist).
    /// Default: 4,194,304 (256 * 16384)
    pub min_train_vectors: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            dimension: 1536,
            nlist: 16384,
            nprobe: 128,
            pq_segments: 64,
            pq_bits: 8,
            gpu_id: 0,
            use_float16: true,
            min_train_vectors: 4_194_304, // 256 * 16384
        }
    }
}

impl IndexConfig {
    /// Generate FAISS factory string for index creation.
    ///
    /// Returns format: "IVF{nlist},PQ{pq_segments}x{pq_bits}"
    ///
    /// # Example
    /// ```
    /// use context_graph_graph::config::IndexConfig;
    /// let config = IndexConfig::default();
    /// assert_eq!(config.factory_string(), "IVF16384,PQ64x8");
    /// ```
    pub fn factory_string(&self) -> String {
        format!("IVF{},PQ{}x{}", self.nlist, self.pq_segments, self.pq_bits)
    }

    /// Calculate minimum training vectors based on nlist.
    ///
    /// FAISS requires at least 256 vectors per cluster for quality training.
    ///
    /// # Returns
    /// 256 * nlist
    ///
    /// # Example
    /// ```
    /// use context_graph_graph::config::IndexConfig;
    /// let config = IndexConfig::default();
    /// assert_eq!(config.calculate_min_train_vectors(), 4_194_304);
    /// ```
    pub fn calculate_min_train_vectors(&self) -> usize {
        256 * self.nlist
    }
}

/// Hyperbolic (Poincare ball) configuration.
///
/// Configures the Poincare ball model for representing hierarchical
/// relationships in hyperbolic space.
///
/// # Mathematics
/// - d(x,y) = arcosh(1 + 2||x-y||² / ((1-||x||²)(1-||y||²)))
/// - Curvature must be negative (typically -1.0)
/// - All points must have norm < 1.0
///
/// TODO: M04-T02 - Add validation for curvature
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperbolicConfig {
    /// Dimension of hyperbolic space (default: 64)
    pub dimension: usize,
    /// Curvature parameter (must be negative, default: -1.0)
    pub curvature: f32,
    /// Maximum norm for points (default: 0.999, must be < 1.0)
    pub max_norm: f32,
}

impl Default for HyperbolicConfig {
    fn default() -> Self {
        Self {
            dimension: 64,
            curvature: -1.0,
            max_norm: 0.999,
        }
    }
}

/// Entailment cone configuration.
///
/// Configures entailment cones for O(1) IS-A hierarchy queries.
/// Cones narrow as depth increases (children have smaller apertures).
///
/// # Constitution Reference
/// - perf.latency.entailment_check: <1ms
///
/// TODO: M04-T03 - Add aperture calculation helpers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConeConfig {
    /// Base aperture angle in radians (default: PI/4 = 45 degrees)
    pub base_aperture: f32,
    /// Aperture decay factor per depth level (default: 0.9)
    pub aperture_decay: f32,
    /// Minimum aperture angle (default: 0.1 radians)
    pub min_aperture: f32,
}

impl Default for ConeConfig {
    fn default() -> Self {
        Self {
            base_aperture: std::f32::consts::FRAC_PI_4,
            aperture_decay: 0.9,
            min_aperture: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_config_default_values() {
        let config = IndexConfig::default();
        assert_eq!(config.dimension, 1536);
        assert_eq!(config.nlist, 16384);
        assert_eq!(config.nprobe, 128);
        assert_eq!(config.pq_segments, 64);
        assert_eq!(config.pq_bits, 8);
        assert_eq!(config.gpu_id, 0);
        assert!(config.use_float16);
        assert_eq!(config.min_train_vectors, 4_194_304);
    }

    #[test]
    fn test_index_config_pq_segments_divides_dimension() {
        let config = IndexConfig::default();
        assert_eq!(
            config.dimension % config.pq_segments,
            0,
            "PQ segments must divide dimension evenly"
        );
    }

    #[test]
    fn test_index_config_min_train_vectors_formula() {
        let config = IndexConfig::default();
        assert_eq!(
            config.min_train_vectors,
            256 * config.nlist,
            "min_train_vectors must equal 256 * nlist"
        );
    }

    #[test]
    fn test_factory_string_default() {
        let config = IndexConfig::default();
        assert_eq!(config.factory_string(), "IVF16384,PQ64x8");
    }

    #[test]
    fn test_factory_string_custom() {
        let config = IndexConfig {
            dimension: 768,
            nlist: 4096,
            nprobe: 64,
            pq_segments: 32,
            pq_bits: 4,
            gpu_id: 1,
            use_float16: false,
            min_train_vectors: 256 * 4096,
        };
        assert_eq!(config.factory_string(), "IVF4096,PQ32x4");
    }

    #[test]
    fn test_calculate_min_train_vectors() {
        let config = IndexConfig::default();
        assert_eq!(config.calculate_min_train_vectors(), 4_194_304);

        let custom = IndexConfig {
            nlist: 1024,
            ..Default::default()
        };
        assert_eq!(custom.calculate_min_train_vectors(), 256 * 1024);
    }

    #[test]
    fn test_index_config_serialization_roundtrip() {
        let config = IndexConfig::default();
        let json = serde_json::to_string(&config).expect("Serialization failed");
        let deserialized: IndexConfig =
            serde_json::from_str(&json).expect("Deserialization failed");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_index_config_json_format() {
        let config = IndexConfig::default();
        let json = serde_json::to_string_pretty(&config).expect("Serialization failed");
        assert!(json.contains("\"dimension\": 1536"));
        assert!(json.contains("\"nlist\": 16384"));
        assert!(json.contains("\"nprobe\": 128"));
        assert!(json.contains("\"pq_segments\": 64"));
        assert!(json.contains("\"pq_bits\": 8"));
        assert!(json.contains("\"gpu_id\": 0"));
        assert!(json.contains("\"use_float16\": true"));
        assert!(json.contains("\"min_train_vectors\": 4194304"));
    }

    #[test]
    fn test_pq_bits_type_is_u8() {
        let config = IndexConfig::default();
        // This is a compile-time check - if pq_bits is not u8, this won't compile
        let _: u8 = config.pq_bits;
    }

    #[test]
    fn test_hyperbolic_config_default() {
        let config = HyperbolicConfig::default();
        assert_eq!(config.dimension, 64);
        assert_eq!(config.curvature, -1.0);
        assert!(config.curvature < 0.0, "Curvature must be negative");
        assert!(config.max_norm < 1.0, "Max norm must be < 1.0");
        assert!(config.max_norm > 0.0, "Max norm must be positive");
    }

    #[test]
    fn test_cone_config_default() {
        let config = ConeConfig::default();
        assert!(config.base_aperture > 0.0);
        assert!(config.base_aperture < std::f32::consts::PI);
        assert!(config.aperture_decay > 0.0 && config.aperture_decay <= 1.0);
        assert!(config.min_aperture > 0.0);
        assert!(config.min_aperture < config.base_aperture);
    }

    #[test]
    fn test_hyperbolic_config_serialization() {
        let config = HyperbolicConfig::default();
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: HyperbolicConfig =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(config.curvature, deserialized.curvature);
    }

    #[test]
    fn test_cone_config_serialization() {
        let config = ConeConfig::default();
        let json = serde_json::to_string(&config).expect("Failed to serialize");
        let deserialized: ConeConfig =
            serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(config.base_aperture, deserialized.base_aperture);
    }
}
