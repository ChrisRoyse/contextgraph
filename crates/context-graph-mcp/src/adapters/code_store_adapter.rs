//! CodeStore adapter for MCP integration.
//!
//! This adapter wraps `CodeStore` from `context-graph-storage` to implement
//! the `CodeStorage` trait from `context-graph-core`.
//!
//! # Architecture
//!
//! The adapter bridges:
//! - `CodeStore` from `context-graph-storage` (RocksDB-backed code entity storage)
//! - `CodeStorage` trait from `context-graph-core` (for code capture pipeline)
//!
//! # Constitution Compliance
//!
//! - ARCH-01: "TeleologicalArray is atomic - all 13 embeddings or nothing"
//! - ARCH-05: "All 13 embedders required - missing = fatal"
//! - Code entities stored separately but with full SemanticFingerprint
//!
//! # Thread Safety
//!
//! `CodeStoreAdapter` is `Send + Sync` and can be safely shared across threads.

#![allow(dead_code)] // E7-WIRING: Prepared for code pipeline integration

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, instrument};
use uuid::Uuid;

use context_graph_core::memory::CodeStorage;
use context_graph_core::types::fingerprint::SemanticFingerprint;
use context_graph_core::types::CodeEntity;
use context_graph_storage::code::CodeStore;

/// Adapter wrapping `CodeStore` to implement `CodeStorage` trait.
///
/// This adapter provides an async interface to the synchronous `CodeStore`
/// operations by wrapping them appropriately.
///
/// # Constitution Compliance
///
/// All store operations require complete SemanticFingerprint (all 13 embeddings)
/// per ARCH-01 and ARCH-05.
pub struct CodeStoreAdapter {
    /// The underlying RocksDB-backed code store.
    store: Arc<CodeStore>,
}

impl CodeStoreAdapter {
    /// Create a new CodeStoreAdapter wrapping the given store.
    ///
    /// # Arguments
    /// * `store` - Arc-wrapped CodeStore instance
    pub fn new(store: Arc<CodeStore>) -> Self {
        Self { store }
    }

    /// Get a reference to the underlying store.
    ///
    /// Useful for direct access to CodeStore-specific operations.
    pub fn inner(&self) -> &Arc<CodeStore> {
        &self.store
    }
}

#[async_trait]
impl CodeStorage for CodeStoreAdapter {
    /// Store a code entity with its full SemanticFingerprint.
    ///
    /// # Arguments
    /// * `entity` - The code entity to store
    /// * `fingerprint` - Complete 13-embedding fingerprint
    ///
    /// # Constitution Compliance
    /// - ARCH-01: Stores all 13 embeddings atomically
    /// - ARCH-05: Validates fingerprint completeness
    #[instrument(skip(self, entity, fingerprint), fields(id = %entity.id, name = %entity.name))]
    async fn store(&self, entity: &CodeEntity, fingerprint: &SemanticFingerprint) -> Result<(), String> {
        // CodeStore::store is synchronous, wrap it
        self.store.store(entity, fingerprint).map_err(|e| {
            let msg = format!("CodeStore store failed: {}", e);
            tracing::error!(error = %e, "CodeStoreAdapter: store failed");
            msg
        })?;

        debug!(
            id = %entity.id,
            name = %entity.name,
            "CodeStoreAdapter: stored code entity with 13-embedding fingerprint"
        );

        Ok(())
    }

    /// Get a code entity by ID.
    ///
    /// # Arguments
    /// * `id` - Entity UUID
    async fn get(&self, id: Uuid) -> Result<Option<CodeEntity>, String> {
        self.store.get(id).map_err(|e| {
            let msg = format!("CodeStore get failed: {}", e);
            tracing::error!(error = %e, id = %id, "CodeStoreAdapter: get failed");
            msg
        })
    }

    /// Get all entities for a file path.
    ///
    /// # Arguments
    /// * `file_path` - Absolute path to the file
    async fn get_by_file(&self, file_path: &str) -> Result<Vec<CodeEntity>, String> {
        self.store.get_by_file(file_path).map_err(|e| {
            let msg = format!("CodeStore get_by_file failed: {}", e);
            tracing::error!(error = %e, file_path = %file_path, "CodeStoreAdapter: get_by_file failed");
            msg
        })
    }

    /// Delete all entities for a file.
    ///
    /// # Arguments
    /// * `file_path` - Absolute path to the file
    ///
    /// # Returns
    /// Number of entities deleted.
    #[instrument(skip(self), fields(file = %file_path))]
    async fn delete_file(&self, file_path: &str) -> Result<usize, String> {
        let deleted = self.store.delete_file(file_path).map_err(|e| {
            let msg = format!("CodeStore delete_file failed: {}", e);
            tracing::error!(error = %e, file_path = %file_path, "CodeStoreAdapter: delete_file failed");
            msg
        })?;

        debug!(file_path = %file_path, deleted = deleted, "CodeStoreAdapter: deleted file entities");
        Ok(deleted)
    }

    /// Get the full SemanticFingerprint for an entity.
    ///
    /// # Arguments
    /// * `id` - Entity UUID
    ///
    /// # Returns
    /// Complete 13-embedding fingerprint if found.
    async fn get_fingerprint(&self, id: Uuid) -> Result<Option<SemanticFingerprint>, String> {
        self.store.get_fingerprint(id).map_err(|e| {
            let msg = format!("CodeStore get_fingerprint failed: {}", e);
            tracing::error!(error = %e, id = %id, "CodeStoreAdapter: get_fingerprint failed");
            msg
        })
    }

    /// Search entities by fingerprint similarity.
    ///
    /// # Arguments
    /// * `query_fingerprint` - Query fingerprint (all 13 embeddings)
    /// * `top_k` - Maximum results to return
    /// * `min_similarity` - Minimum similarity threshold [0.0, 1.0]
    /// * `use_e7_primary` - If true, use E7 (code) for scoring; else use E1 (semantic)
    ///
    /// # Returns
    /// Vector of (entity, similarity_score) pairs sorted by decreasing similarity.
    #[instrument(skip(self, query_fingerprint), fields(top_k = top_k, use_e7 = use_e7_primary))]
    async fn search_by_fingerprint(
        &self,
        query_fingerprint: &SemanticFingerprint,
        top_k: usize,
        min_similarity: f32,
        use_e7_primary: bool,
    ) -> Result<Vec<(CodeEntity, f32)>, String> {
        self.store
            .search_by_fingerprint_with_entities(query_fingerprint, top_k, min_similarity, use_e7_primary)
            .map_err(|e| {
                let msg = format!("CodeStore search_by_fingerprint failed: {}", e);
                tracing::error!(error = %e, "CodeStoreAdapter: search_by_fingerprint failed");
                msg
            })
    }
}

#[cfg(test)]
mod tests {
    // Integration tests would require a RocksDB instance
    // Unit tests for the adapter logic

    #[test]
    fn test_adapter_creation() {
        // Placeholder - full tests require storage setup
    }
}
