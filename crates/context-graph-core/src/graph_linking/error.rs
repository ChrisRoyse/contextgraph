//! Fail-fast error types for graph linking operations.
//!
//! # Design Principles
//!
//! Per user requirements: NO backwards compatibility, fail fast with robust error logging.
//!
//! All errors include:
//! - Unique error code (E_GRAPHLINK_XXX)
//! - Descriptive message with context
//! - Actionable information for debugging
//!
//! # Error Codes
//!
//! | Code | Description |
//! |------|-------------|
//! | E_GRAPHLINK_001 | Invalid embedder ID |
//! | E_GRAPHLINK_002 | Symmetric cosine used for asymmetric embedder (AP-77 violation) |
//! | E_GRAPHLINK_003 | Temporal embedder used for edge detection (AP-60 violation) |
//! | E_GRAPHLINK_004 | Invalid edge type |
//! | E_GRAPHLINK_005 | Missing embedder edge |
//! | E_GRAPHLINK_006 | K-NN graph construction failure |
//! | E_GRAPHLINK_007 | NN-Descent convergence failure |
//! | E_GRAPHLINK_008 | Storage key serialization error |
//! | E_GRAPHLINK_009 | Storage key deserialization error |
//! | E_GRAPHLINK_010 | RocksDB operation failure |
//! | E_GRAPHLINK_011 | Invalid similarity score |
//! | E_GRAPHLINK_012 | Edge threshold violation |
//! | E_GRAPHLINK_013 | Insufficient neighbors for K-NN |
//! | E_GRAPHLINK_014 | Direction required for asymmetric edge |
//! | E_GRAPHLINK_015 | Agreement count mismatch |

use thiserror::Error;
use uuid::Uuid;

use super::GraphLinkEdgeType;

/// Result type for graph linking operations.
pub type EdgeResult<T> = Result<T, EdgeError>;

/// Errors that can occur during graph linking operations.
///
/// All errors follow the fail-fast principle with detailed context.
#[derive(Debug, Error)]
pub enum EdgeError {
    /// Invalid embedder ID (not 0-12).
    #[error("E_GRAPHLINK_001: Invalid embedder ID {embedder_id}. Must be 0-12 (E1-E13).")]
    InvalidEmbedderId { embedder_id: u8 },

    /// AP-77 VIOLATION: Attempted to use symmetric cosine for E5 or E8.
    ///
    /// This is a FATAL error per constitution - E5 (causal) and E8 (graph)
    /// MUST use asymmetric similarity.
    #[error("E_GRAPHLINK_002: AP-77 VIOLATION - Symmetric cosine used for asymmetric embedder E{embedder_id}. \
             Embedder {embedder_name} REQUIRES asymmetric similarity. Use DirectedRelation.")]
    SymmetricCosineViolation {
        embedder_id: u8,
        embedder_name: &'static str,
    },

    /// AP-60 VIOLATION: Temporal embedder used for edge detection.
    ///
    /// Temporal embedders (E2, E3, E4) NEVER count toward edge type detection.
    #[error("E_GRAPHLINK_003: AP-60 VIOLATION - Temporal embedder E{embedder_id} ({embedder_name}) \
             used for edge detection. Temporal embedders MUST NOT influence edge types.")]
    TemporalEmbedderViolation {
        embedder_id: u8,
        embedder_name: &'static str,
    },

    /// Invalid edge type value.
    #[error("E_GRAPHLINK_004: Invalid edge type value {value}. Must be 0-7.")]
    InvalidEdgeType { value: u8 },

    /// Missing embedder edge in K-NN graph.
    #[error("E_GRAPHLINK_005: Missing embedder edge for node {node_id} in embedder E{embedder_id}.")]
    MissingEmbedderEdge { node_id: Uuid, embedder_id: u8 },

    /// K-NN graph construction failure.
    #[error("E_GRAPHLINK_006: K-NN graph construction failed for embedder E{embedder_id}: {reason}")]
    KnnConstructionFailed { embedder_id: u8, reason: String },

    /// NN-Descent algorithm failed to converge.
    #[error("E_GRAPHLINK_007: NN-Descent failed to converge after {iterations} iterations for embedder E{embedder_id}. \
             Final delta: {final_delta:.6}, threshold: {threshold:.6}")]
    NnDescentConvergenceFailed {
        embedder_id: u8,
        iterations: usize,
        final_delta: f64,
        threshold: f64,
    },

    /// Storage key serialization error.
    #[error("E_GRAPHLINK_008: Failed to serialize storage key: {reason}")]
    KeySerializationError { reason: String },

    /// Storage key deserialization error.
    #[error("E_GRAPHLINK_009: Failed to deserialize storage key from {bytes_len} bytes: {reason}")]
    KeyDeserializationError { bytes_len: usize, reason: String },

    /// RocksDB operation failure.
    #[error("E_GRAPHLINK_010: RocksDB operation failed in column family '{cf_name}': {reason}")]
    RocksDbError { cf_name: String, reason: String },

    /// Invalid similarity score (must be in [0.0, 1.0] for normalized, [-1.0, 1.0] for cosine).
    #[error("E_GRAPHLINK_011: Invalid similarity score {score:.6}. Expected range: [{min:.1}, {max:.1}]")]
    InvalidSimilarityScore { score: f32, min: f32, max: f32 },

    /// Edge threshold violation - similarity below configured threshold.
    #[error("E_GRAPHLINK_012: Similarity {score:.4} below threshold {threshold:.4} for edge type {edge_type}.")]
    ThresholdViolation {
        score: f32,
        threshold: f32,
        edge_type: GraphLinkEdgeType,
    },

    /// Insufficient neighbors for K-NN (need k neighbors, got fewer).
    #[error("E_GRAPHLINK_013: Insufficient neighbors for node {node_id}. Need {required}, got {actual}.")]
    InsufficientNeighbors {
        node_id: Uuid,
        required: usize,
        actual: usize,
    },

    /// Direction required for asymmetric edge type but not provided.
    #[error("E_GRAPHLINK_014: Direction required for asymmetric edge type {edge_type} but not provided.")]
    DirectionRequired { edge_type: GraphLinkEdgeType },

    /// Agreement count doesn't match agreeing embedders bitset.
    #[error("E_GRAPHLINK_015: Agreement count mismatch. Count: {count}, bitset popcount: {popcount}")]
    AgreementCountMismatch { count: u8, popcount: u8 },
}

impl EdgeError {
    /// Create an InvalidEmbedderId error.
    pub fn invalid_embedder_id(id: u8) -> Self {
        Self::InvalidEmbedderId { embedder_id: id }
    }

    /// Create an AP-77 symmetric cosine violation error.
    ///
    /// This should be called when code attempts to use symmetric cosine
    /// similarity for E5 (Causal) or E8 (Emotional/Graph).
    pub fn symmetric_cosine_violation(embedder_id: u8) -> Self {
        let embedder_name = match embedder_id {
            4 => "Causal",
            7 => "Emotional/Graph",
            _ => "Unknown",
        };
        Self::SymmetricCosineViolation {
            embedder_id,
            embedder_name,
        }
    }

    /// Create an AP-60 temporal embedder violation error.
    ///
    /// This should be called when code attempts to use E2, E3, or E4
    /// for edge type detection (they should only be used for temporal boost).
    pub fn temporal_embedder_violation(embedder_id: u8) -> Self {
        let embedder_name = match embedder_id {
            1 => "TemporalRecent",
            2 => "TemporalPeriodic",
            3 => "TemporalPositional",
            _ => "Unknown",
        };
        Self::TemporalEmbedderViolation {
            embedder_id,
            embedder_name,
        }
    }

    /// Check if this error represents a constitutional violation (AP-xx).
    ///
    /// Constitutional violations are the most severe and should never be ignored.
    pub fn is_constitutional_violation(&self) -> bool {
        matches!(
            self,
            Self::SymmetricCosineViolation { .. } | Self::TemporalEmbedderViolation { .. }
        )
    }

    /// Get the error code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidEmbedderId { .. } => "E_GRAPHLINK_001",
            Self::SymmetricCosineViolation { .. } => "E_GRAPHLINK_002",
            Self::TemporalEmbedderViolation { .. } => "E_GRAPHLINK_003",
            Self::InvalidEdgeType { .. } => "E_GRAPHLINK_004",
            Self::MissingEmbedderEdge { .. } => "E_GRAPHLINK_005",
            Self::KnnConstructionFailed { .. } => "E_GRAPHLINK_006",
            Self::NnDescentConvergenceFailed { .. } => "E_GRAPHLINK_007",
            Self::KeySerializationError { .. } => "E_GRAPHLINK_008",
            Self::KeyDeserializationError { .. } => "E_GRAPHLINK_009",
            Self::RocksDbError { .. } => "E_GRAPHLINK_010",
            Self::InvalidSimilarityScore { .. } => "E_GRAPHLINK_011",
            Self::ThresholdViolation { .. } => "E_GRAPHLINK_012",
            Self::InsufficientNeighbors { .. } => "E_GRAPHLINK_013",
            Self::DirectionRequired { .. } => "E_GRAPHLINK_014",
            Self::AgreementCountMismatch { .. } => "E_GRAPHLINK_015",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_unique() {
        use std::collections::HashSet;

        let errors: Vec<EdgeError> = vec![
            EdgeError::InvalidEmbedderId { embedder_id: 99 },
            EdgeError::SymmetricCosineViolation {
                embedder_id: 4,
                embedder_name: "Causal",
            },
            EdgeError::TemporalEmbedderViolation {
                embedder_id: 1,
                embedder_name: "TemporalRecent",
            },
            EdgeError::InvalidEdgeType { value: 99 },
            EdgeError::MissingEmbedderEdge {
                node_id: Uuid::nil(),
                embedder_id: 0,
            },
            EdgeError::KnnConstructionFailed {
                embedder_id: 0,
                reason: "test".into(),
            },
            EdgeError::NnDescentConvergenceFailed {
                embedder_id: 0,
                iterations: 10,
                final_delta: 0.1,
                threshold: 0.01,
            },
            EdgeError::KeySerializationError {
                reason: "test".into(),
            },
            EdgeError::KeyDeserializationError {
                bytes_len: 0,
                reason: "test".into(),
            },
            EdgeError::RocksDbError {
                cf_name: "test".into(),
                reason: "test".into(),
            },
            EdgeError::InvalidSimilarityScore {
                score: 2.0,
                min: 0.0,
                max: 1.0,
            },
            EdgeError::ThresholdViolation {
                score: 0.5,
                threshold: 0.75,
                edge_type: GraphLinkEdgeType::SemanticSimilar,
            },
            EdgeError::InsufficientNeighbors {
                node_id: Uuid::nil(),
                required: 20,
                actual: 5,
            },
            EdgeError::DirectionRequired {
                edge_type: GraphLinkEdgeType::CausalChain,
            },
            EdgeError::AgreementCountMismatch {
                count: 3,
                popcount: 4,
            },
        ];

        let codes: HashSet<_> = errors.iter().map(|e| e.code()).collect();
        assert_eq!(codes.len(), errors.len(), "All error codes must be unique");
    }

    #[test]
    fn test_symmetric_cosine_violation_e5() {
        let err = EdgeError::symmetric_cosine_violation(4);
        assert!(err.is_constitutional_violation());
        assert_eq!(err.code(), "E_GRAPHLINK_002");
        let msg = format!("{}", err);
        assert!(msg.contains("AP-77"));
        assert!(msg.contains("Causal"));
    }

    #[test]
    fn test_symmetric_cosine_violation_e8() {
        let err = EdgeError::symmetric_cosine_violation(7);
        assert!(err.is_constitutional_violation());
        let msg = format!("{}", err);
        assert!(msg.contains("AP-77"));
        assert!(msg.contains("Emotional"));
    }

    #[test]
    fn test_temporal_embedder_violation() {
        for (id, name) in [(1, "TemporalRecent"), (2, "TemporalPeriodic"), (3, "TemporalPositional")] {
            let err = EdgeError::temporal_embedder_violation(id);
            assert!(err.is_constitutional_violation());
            assert_eq!(err.code(), "E_GRAPHLINK_003");
            let msg = format!("{}", err);
            assert!(msg.contains("AP-60"));
            assert!(msg.contains(name));
        }
    }

    #[test]
    fn test_invalid_embedder_id() {
        let err = EdgeError::invalid_embedder_id(99);
        assert!(!err.is_constitutional_violation());
        assert_eq!(err.code(), "E_GRAPHLINK_001");
        let msg = format!("{}", err);
        assert!(msg.contains("99"));
        assert!(msg.contains("0-12"));
    }

    #[test]
    fn test_direction_required() {
        let err = EdgeError::DirectionRequired {
            edge_type: GraphLinkEdgeType::CausalChain,
        };
        assert!(!err.is_constitutional_violation());
        assert_eq!(err.code(), "E_GRAPHLINK_014");
        let msg = format!("{}", err);
        assert!(msg.contains("Direction required"));
        assert!(msg.contains("causal_chain"));
    }

    #[test]
    fn test_error_display_includes_code() {
        let err = EdgeError::InvalidEdgeType { value: 99 };
        let msg = format!("{}", err);
        assert!(msg.starts_with("E_GRAPHLINK_004"));
    }

    #[test]
    fn test_knn_construction_failed() {
        let err = EdgeError::KnnConstructionFailed {
            embedder_id: 0,
            reason: "insufficient data".to_string(),
        };
        assert_eq!(err.code(), "E_GRAPHLINK_006");
        let msg = format!("{}", err);
        assert!(msg.contains("E0"));
        assert!(msg.contains("insufficient data"));
    }

    #[test]
    fn test_nn_descent_convergence_failed() {
        let err = EdgeError::NnDescentConvergenceFailed {
            embedder_id: 0,
            iterations: 100,
            final_delta: 0.05,
            threshold: 0.01,
        };
        assert_eq!(err.code(), "E_GRAPHLINK_007");
        let msg = format!("{}", err);
        assert!(msg.contains("100 iterations"));
        assert!(msg.contains("0.05"));
    }
}
