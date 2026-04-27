//! Error types and result aliases for graph edge storage.
//!
//! # FAIL FAST Policy
//!
//! **NO FALLBACKS. NO MOCK DATA. ERRORS ARE FATAL.**
//!
//! All errors include full context for debugging:
//! - The operation that failed
//! - The column family involved
//! - The key being accessed
//! - The underlying storage error

use std::fmt;
use thiserror::Error;
use uuid::Uuid;

/// Error type for graph edge storage operations.
///
/// # Design Philosophy
///
/// Fail fast with maximum context. Every error variant captures:
/// - What operation failed
/// - What data was involved
/// - The underlying cause
#[derive(Debug, Error)]
pub enum GraphEdgeStorageError {
    /// RocksDB operation failed.
    #[error("RocksDB error in {operation} on CF '{column_family}': {source}")]
    RocksDb {
        operation: &'static str,
        column_family: &'static str,
        #[source]
        source: rocksdb::Error,
    },

    /// Serialization failed.
    #[error("Serialization error in {operation}: {message}")]
    Serialization {
        operation: &'static str,
        message: String,
    },

    /// Deserialization failed.
    #[error("Deserialization error in {operation}: {message}")]
    Deserialization {
        operation: &'static str,
        message: String,
    },

    /// Column family not found.
    #[error("Column family '{name}' not found - database may need migration")]
    ColumnFamilyNotFound { name: &'static str },

    /// Invalid embedder ID.
    #[error("Invalid embedder ID {embedder_id} - must be 0-13 (14 embedders)")]
    InvalidEmbedderId { embedder_id: u8 },

    /// Key format error.
    #[error("Invalid key format: expected {expected} bytes, got {actual}")]
    InvalidKeyFormat { expected: usize, actual: usize },

    /// Edge not found.
    #[error("Edge not found: source_node={source_node}, target_node={target_node:?}, embedder={embedder:?}")]
    EdgeNotFound {
        source_node: Uuid,
        target_node: Option<Uuid>,
        embedder: Option<u8>,
    },

    /// Database not initialized.
    #[error("Database not initialized - call open() first")]
    NotInitialized,

    /// Graph building error.
    #[error("Graph build error: {message}")]
    GraphBuildError { message: String },
}

impl GraphEdgeStorageError {
    /// Create a RocksDB error with context.
    pub fn rocksdb(
        operation: &'static str,
        column_family: &'static str,
        source: rocksdb::Error,
    ) -> Self {
        Self::RocksDb {
            operation,
            column_family,
            source,
        }
    }

    /// Create a serialization error.
    pub fn serialization(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Serialization {
            operation,
            message: message.into(),
        }
    }

    /// Create a deserialization error.
    pub fn deserialization(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Deserialization {
            operation,
            message: message.into(),
        }
    }

    /// Create an edge not found error.
    pub fn edge_not_found(
        source_node: Uuid,
        target_node: Option<Uuid>,
        embedder: Option<u8>,
    ) -> Self {
        Self::EdgeNotFound {
            source_node,
            target_node,
            embedder,
        }
    }
}

/// Result type alias for graph edge storage operations.
pub type GraphEdgeStorageResult<T> = Result<T, GraphEdgeStorageError>;

/// Statistics for graph edge storage.
#[derive(Debug, Clone, Default)]
pub struct GraphEdgeStats {
    /// Number of K-NN edges per embedder.
    pub embedder_edge_counts: [u64; 14],
    /// Total K-NN edges across all embedders.
    pub total_embedder_edges: u64,
    /// Number of typed edges.
    pub typed_edge_count: u64,
    /// Number of typed edges by type.
    pub typed_edge_by_type_counts: [u64; 8],
    /// Storage size in bytes (approximate).
    pub storage_bytes: u64,
}

impl fmt::Display for GraphEdgeStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Graph Edge Statistics:")?;
        writeln!(f, "  Total K-NN edges: {}", self.total_embedder_edges)?;
        writeln!(f, "  Typed edges: {}", self.typed_edge_count)?;
        writeln!(f, "  Storage: {} bytes", self.storage_bytes)?;
        writeln!(f, "  K-NN edges per embedder:")?;
        for (i, count) in self.embedder_edge_counts.iter().enumerate() {
            if *count > 0 {
                writeln!(f, "    E{}: {}", i + 1, count)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GraphEdgeStorageError::InvalidEmbedderId { embedder_id: 15 };
        assert!(err.to_string().contains("15"));
        assert!(err.to_string().contains("0-13"));
    }

    #[test]
    fn test_edge_not_found_display() {
        let source = Uuid::nil();
        let err = GraphEdgeStorageError::edge_not_found(source, None, Some(0));
        assert!(err.to_string().contains("source_node="));
        assert!(err.to_string().contains("embedder=Some(0)"));
    }

    #[test]
    fn test_stats_display() {
        let mut stats = GraphEdgeStats::default();
        stats.embedder_edge_counts[0] = 100;
        stats.total_embedder_edges = 100;
        stats.typed_edge_count = 50;

        let display = stats.to_string();
        assert!(display.contains("Total K-NN edges: 100"));
        assert!(display.contains("Typed edges: 50"));
        assert!(display.contains("E1: 100"));
    }

    #[test]
    fn test_serialization_error() {
        let err = GraphEdgeStorageError::serialization("store_edge", "bincode failed");
        assert!(err.to_string().contains("store_edge"));
        assert!(err.to_string().contains("bincode failed"));
    }
}
