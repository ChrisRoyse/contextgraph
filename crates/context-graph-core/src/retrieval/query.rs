//! Query types for multi-embedding search.
//!
//! This module provides the core query configuration for searching across
//! the 13 embedding spaces defined in `SemanticFingerprint`.
//!
//! # Performance Targets (constitution.yaml)
//! - Total latency: <60ms @ 1M memories
//! - Single embedding: <30ms
//!
//! # Example
//!
//! ```
//! use context_graph_core::retrieval::{MultiEmbeddingQuery, EmbeddingSpaceMask};
//!
//! let query = MultiEmbeddingQuery {
//!     query_text: "How does memory consolidation work?".to_string(),
//!     active_spaces: EmbeddingSpaceMask::ALL,
//!     final_limit: 10,
//!     ..Default::default()
//! };
//!
//! assert!(query.validate().is_ok());
//! ```

use crate::config::constants::{pipeline, similarity};
use crate::error::{CoreError, CoreResult};
use crate::types::fingerprint::NUM_EMBEDDERS;

use super::aggregation::AggregationStrategy;

/// Query configuration for multi-embedding search.
///
/// # Performance Targets (constitution.yaml)
/// - Total latency: <60ms @ 1M memories
/// - Single embedding: <30ms
///
/// # Fail-Fast Behavior
/// - Empty query_text: Returns `CoreError::ValidationError`
/// - Invalid space indices: Returns `CoreError::ValidationError`
/// - All spaces disabled: Returns `CoreError::ValidationError`
#[derive(Clone, Debug)]
pub struct MultiEmbeddingQuery {
    /// The query text to embed. MUST be non-empty.
    pub query_text: String,

    /// Which embedding spaces to search (bitmask).
    /// Default: ALL (0x1FFF = all 13 spaces)
    pub active_spaces: EmbeddingSpaceMask,

    /// Per-space weight overrides [0.0, 1.0].
    /// None = equal weighting (1.0 for all active spaces)
    pub space_weights: Option<[f32; NUM_EMBEDDERS]>,

    /// Maximum results per space before aggregation.
    /// Default: 100, Range: [1, 1000]
    pub per_space_limit: usize,

    /// Final result limit after aggregation.
    /// Default: 10, Range: [1, 1000]
    pub final_limit: usize,

    /// Minimum similarity threshold per space [0.0, 1.0].
    /// Results below threshold are filtered. Default: 0.0
    pub min_similarity: f32,

    /// Include per-space breakdown in results.
    /// Default: false (reduces response size)
    pub include_space_breakdown: bool,

    /// Pipeline stage configuration.
    /// None = use defaults from PipelineStageConfig::default()
    pub pipeline_config: Option<PipelineStageConfig>,

    /// Aggregation strategy.
    /// Default: RRF with k=60
    pub aggregation: AggregationStrategy,
}

impl Default for MultiEmbeddingQuery {
    fn default() -> Self {
        Self {
            query_text: String::new(),
            active_spaces: EmbeddingSpaceMask::ALL,
            space_weights: None,
            per_space_limit: 100,
            final_limit: 10,
            min_similarity: 0.0,
            include_space_breakdown: false,
            pipeline_config: None,
            aggregation: AggregationStrategy::RRF {
                k: similarity::RRF_K,
            },
        }
    }
}

impl MultiEmbeddingQuery {
    /// Create a new query with the given text.
    ///
    /// Uses default settings for all other parameters.
    pub fn new(query_text: impl Into<String>) -> Self {
        Self {
            query_text: query_text.into(),
            ..Default::default()
        }
    }

    /// Validate query configuration.
    ///
    /// # Errors
    /// - `CoreError::ValidationError` if query_text is empty
    /// - `CoreError::ValidationError` if no spaces are active
    /// - `CoreError::ValidationError` if limits are out of range
    pub fn validate(&self) -> CoreResult<()> {
        if self.query_text.is_empty() {
            return Err(CoreError::ValidationError {
                field: "query_text".to_string(),
                message: "Query text must not be empty".to_string(),
            });
        }

        if self.active_spaces.active_count() == 0 {
            return Err(CoreError::ValidationError {
                field: "active_spaces".to_string(),
                message: "At least one embedding space must be active".to_string(),
            });
        }

        if self.per_space_limit == 0 || self.per_space_limit > 1000 {
            return Err(CoreError::ValidationError {
                field: "per_space_limit".to_string(),
                message: format!(
                    "per_space_limit must be in [1, 1000], got {}",
                    self.per_space_limit
                ),
            });
        }

        if self.final_limit == 0 || self.final_limit > 1000 {
            return Err(CoreError::ValidationError {
                field: "final_limit".to_string(),
                message: format!("final_limit must be in [1, 1000], got {}", self.final_limit),
            });
        }

        if self.min_similarity < 0.0 || self.min_similarity > 1.0 {
            return Err(CoreError::ValidationError {
                field: "min_similarity".to_string(),
                message: format!(
                    "min_similarity must be in [0.0, 1.0], got {}",
                    self.min_similarity
                ),
            });
        }

        Ok(())
    }
}

/// Configuration for 5-stage retrieval pipeline.
///
/// # Stage Targets (constitution.yaml)
/// - Stage 1 (SPLADE): <5ms, 1000 candidates
/// - Stage 2 (Matryoshka): <10ms, 200 candidates
/// - Stage 3 (Full HNSW): <20ms, 100 candidates
/// - Stage 4 (Teleological): <10ms, 50 candidates
/// - Stage 5 (Late Interaction): <15ms, final ranking
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct PipelineStageConfig {
    /// Stage 1: SPLADE sparse retrieval candidate count.
    /// Default: 1000
    pub splade_candidates: usize,

    /// Stage 2: Matryoshka 128D filter count.
    /// Default: 200
    pub matryoshka_128d_limit: usize,

    /// Stage 3: Full 13-space embedding search limit.
    /// Default: 100
    pub full_search_limit: usize,

    /// Stage 4: Teleological alignment filter limit.
    /// Default: 50
    pub teleological_limit: usize,

    /// Stage 5: Late interaction rerank count.
    /// Default: 20
    pub late_interaction_limit: usize,

    /// RRF k parameter.
    ///
    /// Constitution: `embeddings.similarity.rrf_constant`
    /// Default: 60 (via `similarity::RRF_K`)
    /// Formula: RRF(d) = Σᵢ 1/(k + rankᵢ(d))
    pub rrf_k: f32,

    /// Minimum alignment threshold for Stage 4 teleological filter.
    ///
    /// Constitution: `teleological.thresholds.critical`
    /// Default: 0.55 (via `alignment::CRITICAL`)
    pub min_alignment_threshold: f32,
}

impl Default for PipelineStageConfig {
    fn default() -> Self {
        Self {
            splade_candidates: 1000,
            matryoshka_128d_limit: 200,
            full_search_limit: 100,
            teleological_limit: 50,
            late_interaction_limit: 20,
            rrf_k: pipeline::DEFAULT_RRF_K,
            min_alignment_threshold: pipeline::DEFAULT_MIN_ALIGNMENT,
        }
    }
}

impl PipelineStageConfig {
    /// Validate pipeline configuration. FAILS FAST on invalid values.
    ///
    /// # Validation Rules
    ///
    /// 1. All candidate counts must be > 0
    /// 2. rrf_k must be > 0
    /// 3. min_alignment_threshold must be in [0.0, 1.0]
    /// 4. Stage limits should form a decreasing funnel
    ///
    /// # Errors
    ///
    /// Returns `CoreError::ValidationError` if any rule is violated.
    pub fn validate(&self) -> crate::error::CoreResult<()> {
        // Rule 1: All candidate counts must be > 0
        if self.splade_candidates == 0 {
            return Err(crate::error::CoreError::ValidationError {
                field: "splade_candidates".to_string(),
                message: "Stage 1 SPLADE candidates must be > 0".to_string(),
            });
        }

        if self.matryoshka_128d_limit == 0 {
            return Err(crate::error::CoreError::ValidationError {
                field: "matryoshka_128d_limit".to_string(),
                message: "Stage 2 Matryoshka limit must be > 0".to_string(),
            });
        }

        if self.full_search_limit == 0 {
            return Err(crate::error::CoreError::ValidationError {
                field: "full_search_limit".to_string(),
                message: "Stage 3 full search limit must be > 0".to_string(),
            });
        }

        if self.teleological_limit == 0 {
            return Err(crate::error::CoreError::ValidationError {
                field: "teleological_limit".to_string(),
                message: "Stage 4 teleological limit must be > 0".to_string(),
            });
        }

        if self.late_interaction_limit == 0 {
            return Err(crate::error::CoreError::ValidationError {
                field: "late_interaction_limit".to_string(),
                message: "Stage 5 late interaction limit must be > 0".to_string(),
            });
        }

        // Rule 2: rrf_k must be > 0
        if self.rrf_k <= 0.0 {
            return Err(crate::error::CoreError::ValidationError {
                field: "rrf_k".to_string(),
                message: "RRF k parameter must be > 0".to_string(),
            });
        }

        // Rule 3: min_alignment_threshold must be in [0.0, 1.0]
        if !(0.0..=1.0).contains(&self.min_alignment_threshold) {
            return Err(crate::error::CoreError::ValidationError {
                field: "min_alignment_threshold".to_string(),
                message: format!(
                    "Alignment threshold must be in [0.0, 1.0], got {}",
                    self.min_alignment_threshold
                ),
            });
        }

        Ok(())
    }
}

/// Bitmask for active embedding spaces (0-12).
///
/// # Bit Layout
/// - Bit 0: E1 Semantic
/// - Bit 1: E2 Temporal-Recent
/// - Bit 2: E3 Temporal-Periodic
/// - Bit 3: E4 Temporal-Positional
/// - Bit 4: E5 Causal
/// - Bit 5: E6 Sparse
/// - Bit 6: E7 Code
/// - Bit 7: E8 Graph
/// - Bit 8: E9 HDC
/// - Bit 9: E10 Multimodal
/// - Bit 10: E11 Entity
/// - Bit 11: E12 Late-Interaction
/// - Bit 12: E13 SPLADE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbeddingSpaceMask(pub u16);

impl EmbeddingSpaceMask {
    /// All 13 spaces active (bits 0-12).
    pub const ALL: Self = Self(0x1FFF);

    /// All 12 dense spaces (E1-E12, bits 0-11).
    pub const ALL_DENSE: Self = Self(0x0FFF);

    /// E1 semantic only.
    pub const SEMANTIC_ONLY: Self = Self(0x0001);

    /// Text core: E1, E2, E3.
    pub const TEXT_CORE: Self = Self(0x0007);

    /// E13 SPLADE only (bit 12).
    pub const SPLADE_ONLY: Self = Self(0x1000);

    /// Hybrid: E1 semantic + E13 SPLADE.
    pub const HYBRID: Self = Self(0x1001);

    /// Stage 2 fast filter: E1 only (Matryoshka 128D).
    pub const MATRYOSHKA_FILTER: Self = Self(0x0001);

    /// Code-focused: E1, E7 Code, E13 SPLADE.
    pub const CODE_FOCUSED: Self = Self(0x1041);

    /// Check if a specific space is active.
    #[inline]
    pub fn is_active(&self, space_idx: usize) -> bool {
        if space_idx >= NUM_EMBEDDERS {
            return false;
        }
        (self.0 & (1 << space_idx)) != 0
    }

    /// Count number of active spaces.
    #[inline]
    pub fn active_count(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// Check if E13 SPLADE is active (for Stage 1).
    #[inline]
    pub fn includes_splade(&self) -> bool {
        self.is_active(12)
    }

    /// Check if E12 Late-Interaction is active (for Stage 5).
    #[inline]
    pub fn includes_late_interaction(&self) -> bool {
        self.is_active(11)
    }

    /// Get list of active space indices.
    pub fn active_indices(&self) -> Vec<usize> {
        (0..NUM_EMBEDDERS).filter(|&i| self.is_active(i)).collect()
    }

    /// Get embedding space name by index.
    pub const fn space_name(idx: usize) -> &'static str {
        match idx {
            0 => "E1_Semantic",
            1 => "E2_Temporal_Recent",
            2 => "E3_Temporal_Periodic",
            3 => "E4_Temporal_Positional",
            4 => "E5_Causal",
            5 => "E6_Sparse",
            6 => "E7_Code",
            7 => "E8_Graph",
            8 => "E9_HDC",
            9 => "E10_Multimodal",
            10 => "E11_Entity",
            11 => "E12_Late_Interaction",
            12 => "E13_SPLADE",
            _ => "Unknown",
        }
    }
}

impl Default for EmbeddingSpaceMask {
    fn default() -> Self {
        Self::ALL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_space_mask_all() {
        let mask = EmbeddingSpaceMask::ALL;
        assert_eq!(mask.active_count(), 13);
        for i in 0..13 {
            assert!(mask.is_active(i), "Space {} should be active", i);
        }
        println!("[VERIFIED] EmbeddingSpaceMask::ALL has 13 active spaces");
    }

    #[test]
    fn test_embedding_space_mask_presets() {
        assert_eq!(EmbeddingSpaceMask::ALL.active_count(), 13);
        assert_eq!(EmbeddingSpaceMask::ALL_DENSE.active_count(), 12);
        assert_eq!(EmbeddingSpaceMask::SEMANTIC_ONLY.active_count(), 1);
        assert_eq!(EmbeddingSpaceMask::TEXT_CORE.active_count(), 3);
        assert_eq!(EmbeddingSpaceMask::SPLADE_ONLY.active_count(), 1);
        assert_eq!(EmbeddingSpaceMask::HYBRID.active_count(), 2);

        assert!(EmbeddingSpaceMask::ALL.includes_splade());
        assert!(EmbeddingSpaceMask::ALL.includes_late_interaction());
        assert!(!EmbeddingSpaceMask::ALL_DENSE.includes_splade());

        println!("[VERIFIED] All EmbeddingSpaceMask presets have correct counts");
    }

    #[test]
    fn test_embedding_space_mask_active_indices() {
        let mask = EmbeddingSpaceMask::TEXT_CORE;
        let indices = mask.active_indices();
        assert_eq!(indices, vec![0, 1, 2]);
        println!("[VERIFIED] active_indices returns correct list");
    }

    #[test]
    fn test_pipeline_stage_config_defaults() {
        let config = PipelineStageConfig::default();

        assert_eq!(config.splade_candidates, 1000);
        assert_eq!(config.matryoshka_128d_limit, 200);
        assert_eq!(config.full_search_limit, 100);
        assert_eq!(config.teleological_limit, 50);
        assert_eq!(config.late_interaction_limit, 20);
        assert!((config.rrf_k - 60.0).abs() < 0.001);
        assert!((config.min_alignment_threshold - 0.55).abs() < 0.001);

        println!("[VERIFIED] PipelineStageConfig defaults match constitution.yaml");
    }

    #[test]
    fn test_query_validation_empty_text() {
        let query = MultiEmbeddingQuery {
            query_text: "".into(),
            ..Default::default()
        };

        let result = query.validate();
        assert!(result.is_err());

        match result.unwrap_err() {
            CoreError::ValidationError { field, .. } => {
                assert_eq!(field, "query_text");
            }
            _ => panic!("Expected ValidationError"),
        }

        println!("[VERIFIED] Empty query text returns ValidationError");
    }

    #[test]
    fn test_query_validation_no_active_spaces() {
        let query = MultiEmbeddingQuery {
            query_text: "test".into(),
            active_spaces: EmbeddingSpaceMask(0),
            ..Default::default()
        };

        let result = query.validate();
        assert!(result.is_err());

        println!("[VERIFIED] Zero active spaces returns ValidationError");
    }

    #[test]
    fn test_query_validation_invalid_per_space_limit() {
        let query = MultiEmbeddingQuery {
            query_text: "test".into(),
            per_space_limit: 0,
            ..Default::default()
        };

        assert!(query.validate().is_err());

        let query2 = MultiEmbeddingQuery {
            query_text: "test".into(),
            per_space_limit: 1001,
            ..Default::default()
        };

        assert!(query2.validate().is_err());

        println!("[VERIFIED] Invalid per_space_limit returns ValidationError");
    }

    #[test]
    fn test_query_validation_invalid_min_similarity() {
        let query = MultiEmbeddingQuery {
            query_text: "test".into(),
            min_similarity: -0.1,
            ..Default::default()
        };

        assert!(query.validate().is_err());

        let query2 = MultiEmbeddingQuery {
            query_text: "test".into(),
            min_similarity: 1.1,
            ..Default::default()
        };

        assert!(query2.validate().is_err());

        println!("[VERIFIED] Invalid min_similarity returns ValidationError");
    }

    #[test]
    fn test_query_validation_valid() {
        let query = MultiEmbeddingQuery {
            query_text: "How does memory consolidation work?".into(),
            active_spaces: EmbeddingSpaceMask::ALL,
            final_limit: 10,
            ..Default::default()
        };

        assert!(query.validate().is_ok());

        println!("[VERIFIED] Valid query passes validation");
    }

    #[test]
    fn test_space_name() {
        assert_eq!(EmbeddingSpaceMask::space_name(0), "E1_Semantic");
        assert_eq!(EmbeddingSpaceMask::space_name(12), "E13_SPLADE");
        assert_eq!(EmbeddingSpaceMask::space_name(99), "Unknown");

        println!("[VERIFIED] space_name returns correct names");
    }
}
