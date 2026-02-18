//! From implementations for error type conversions.
//!
//! Connects the unified error hierarchy to existing error types.

use super::sub_errors::{ConfigError, EmbeddingError, IndexError, StorageError};
use super::unified::ContextGraphError;
use crate::teleological::embedder::Embedder;

// ============================================================================
// FROM IMPLEMENTATIONS - Connect to existing error types
// ============================================================================

/// Convert from `context_graph_core::index::error::IndexError`
impl From<crate::index::error::IndexError> for IndexError {
    fn from(e: crate::index::error::IndexError) -> Self {
        use crate::index::error::IndexError as ExistingIndexError;

        match e {
            ExistingIndexError::DimensionMismatch {
                embedder,
                expected,
                actual,
            } => {
                // Convert EmbedderIndex to Embedder
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::Hnsw(format!(
                    "Dimension mismatch for {:?}: expected {}, got {}",
                    embedder, expected, actual
                ))
            }
            ExistingIndexError::InvalidEmbedder { embedder } => {
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::NotFound(embedder)
            }
            ExistingIndexError::NotInitialized { embedder } => {
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::RebuildRequired(embedder)
            }
            ExistingIndexError::IndexEmpty { embedder } => {
                let embedder = embedder_index_to_embedder(embedder);
                IndexError::RebuildRequired(embedder)
            }
            ExistingIndexError::InvalidTermId {
                term_id,
                vocab_size,
            } => IndexError::Inverted(format!(
                "Invalid term_id {} (vocab_size={})",
                term_id, vocab_size
            )),
            ExistingIndexError::ZeroNormVector { memory_id } => IndexError::InsertionFailed {
                memory_id,
                message: "Zero-norm vector".to_string(),
            },
            ExistingIndexError::NotFound { memory_id } => IndexError::InsertionFailed {
                memory_id,
                message: "Memory not found in indexes".to_string(),
            },
            ExistingIndexError::StorageError { context, message } => {
                IndexError::Hnsw(format!("{}: {}", context, message))
            }
            ExistingIndexError::CorruptedIndex { path } => {
                IndexError::Corruption(Embedder::Semantic, format!("Corrupted file: {}", path))
            }
            ExistingIndexError::IoError { context, message } => {
                IndexError::Hnsw(format!("IO error - {}: {}", context, message))
            }
            ExistingIndexError::SerializationError { context, message } => {
                IndexError::Hnsw(format!("Serialization - {}: {}", context, message))
            }
            ExistingIndexError::HnswConstructionFailed {
                dimension, message, ..
            } => IndexError::ConstructionFailed { dimension, message },
            ExistingIndexError::HnswInsertionFailed {
                memory_id, message, ..
            } => IndexError::InsertionFailed { memory_id, message },
            ExistingIndexError::HnswSearchFailed { message, .. } => IndexError::Hnsw(message),
            ExistingIndexError::HnswPersistenceFailed { message, .. } => IndexError::Hnsw(message),
            ExistingIndexError::HnswInternalError { message, .. } => IndexError::Hnsw(message),
            ExistingIndexError::LegacyFormatRejected { path, message } => IndexError::Corruption(
                Embedder::Semantic,
                format!("Legacy: {} - {}", path, message),
            ),
        }
    }
}

/// Helper to convert EmbedderIndex to Embedder
pub(crate) fn embedder_index_to_embedder(idx: crate::index::config::EmbedderIndex) -> Embedder {
    use crate::index::config::EmbedderIndex;

    match idx {
        EmbedderIndex::E1Semantic => Embedder::Semantic,
        EmbedderIndex::E1Matryoshka128 => Embedder::Semantic, // Truncated E1
        EmbedderIndex::E2TemporalRecent => Embedder::TemporalRecent,
        EmbedderIndex::E3TemporalPeriodic => Embedder::TemporalPeriodic,
        EmbedderIndex::E4TemporalPositional => Embedder::TemporalPositional,
        EmbedderIndex::E5Causal => Embedder::Causal,
        EmbedderIndex::E6Sparse => Embedder::Sparse,
        EmbedderIndex::E7Code => Embedder::Code,
        EmbedderIndex::E8Graph => Embedder::Graph,
        EmbedderIndex::E9HDC => Embedder::Hdc,
        EmbedderIndex::E10Multimodal => Embedder::Multimodal,
        EmbedderIndex::E11Entity => Embedder::Entity,
        EmbedderIndex::E12LateInteraction => Embedder::LateInteraction,
        EmbedderIndex::E13Splade => Embedder::KeywordSplade,
    }
}

/// Convert from existing index::error::IndexError to ContextGraphError
impl From<crate::index::error::IndexError> for ContextGraphError {
    fn from(e: crate::index::error::IndexError) -> Self {
        ContextGraphError::Index(e.into())
    }
}

// ============================================================================
// LEGACY COREERROR CONVERSION
// ============================================================================

use super::legacy::CoreError;

/// Convert CoreError to ContextGraphError for interoperability.
impl From<CoreError> for ContextGraphError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::NodeNotFound { id } => {
                ContextGraphError::Storage(StorageError::NotFound(id))
            }
            CoreError::DimensionMismatch { expected, actual } => {
                ContextGraphError::Embedding(EmbeddingError::DimensionMismatch { expected, actual })
            }
            CoreError::ValidationError { field, message } => {
                ContextGraphError::Validation(format!("{}: {}", field, message))
            }
            CoreError::StorageError(msg) => ContextGraphError::Storage(StorageError::Database(msg)),
            CoreError::IndexError(msg) => ContextGraphError::Index(IndexError::Hnsw(msg)),
            CoreError::ConfigError(msg) => ContextGraphError::Config(ConfigError::Missing(msg)),
            CoreError::UtlError(msg) => ContextGraphError::Internal(format!("UTL: {}", msg)),
            CoreError::LayerError { layer, message } => {
                ContextGraphError::Internal(format!("Layer {}: {}", layer, message))
            }
            CoreError::FeatureDisabled { feature } => {
                ContextGraphError::Config(ConfigError::Missing(feature))
            }
            CoreError::SerializationError(msg) => {
                ContextGraphError::Storage(StorageError::Serialization(msg))
            }
            CoreError::Internal(msg) => ContextGraphError::Internal(msg),
            CoreError::Embedding(msg) => {
                ContextGraphError::Embedding(EmbeddingError::GenerationFailed {
                    embedder: Embedder::Semantic, // default
                    reason: msg,
                })
            }
            CoreError::MissingField { field, context } => {
                ContextGraphError::Validation(format!("Missing {}: {}", field, context))
            }
            CoreError::NotImplemented(msg) => {
                ContextGraphError::Internal(format!("Not implemented: {}", msg))
            }
            CoreError::LegacyFormatRejected(msg) => {
                ContextGraphError::Storage(StorageError::Migration(format!("Legacy: {}", msg)))
            }
        }
    }
}
