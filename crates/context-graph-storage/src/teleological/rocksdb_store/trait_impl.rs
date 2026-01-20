//! TeleologicalMemoryStore trait implementation for RocksDbTeleologicalStore.
//!
//! This module contains the async trait implementation that delegates to
//! the modular implementation files:
//! - `crud.rs` - CRUD operations
//! - `search.rs` - Search operations
//! - `persistence.rs` - Batch, statistics, persistence, content

use std::path::PathBuf;

use async_trait::async_trait;
use uuid::Uuid;

use context_graph_core::error::CoreResult;
use context_graph_core::traits::{
    TeleologicalMemoryStore, TeleologicalSearchOptions, TeleologicalSearchResult,
    TeleologicalStorageBackend,
};
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, TeleologicalFingerprint,
};
use context_graph_core::types::SourceMetadata;

use super::store::RocksDbTeleologicalStore;

// ============================================================================
// TeleologicalMemoryStore Trait Implementation
// ============================================================================

#[async_trait]
impl TeleologicalMemoryStore for RocksDbTeleologicalStore {
    // ==================== CRUD Operations ====================

    async fn store(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<Uuid> {
        self.store_async(fingerprint).await
    }

    async fn retrieve(&self, id: Uuid) -> CoreResult<Option<TeleologicalFingerprint>> {
        self.retrieve_async(id).await
    }

    async fn update(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<bool> {
        self.update_async(fingerprint).await
    }

    async fn delete(&self, id: Uuid, soft: bool) -> CoreResult<bool> {
        self.delete_async(id, soft).await
    }

    // ==================== Search Operations ====================

    async fn search_semantic(
        &self,
        query: &SemanticFingerprint,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        self.search_semantic_async(query, options).await
    }

    async fn search_text(
        &self,
        text: &str,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        self.search_text_async(text, options).await
    }

    async fn search_sparse(
        &self,
        sparse_query: &SparseVector,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        self.search_sparse_async(sparse_query, top_k).await
    }

    // ==================== Batch Operations ====================

    async fn store_batch(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>> {
        self.store_batch_async(fingerprints).await
    }

    async fn retrieve_batch(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
        self.retrieve_batch_async(ids).await
    }

    // ==================== Statistics ====================

    async fn count(&self) -> CoreResult<usize> {
        self.count_async().await
    }

    fn storage_size_bytes(&self) -> usize {
        self.storage_size_bytes_internal()
    }

    fn backend_type(&self) -> TeleologicalStorageBackend {
        self.backend_type_internal()
    }

    // ==================== Persistence ====================

    async fn flush(&self) -> CoreResult<()> {
        self.flush_async().await
    }

    async fn checkpoint(&self) -> CoreResult<PathBuf> {
        self.checkpoint_async().await
    }

    async fn restore(&self, checkpoint_path: &std::path::Path) -> CoreResult<()> {
        self.restore_async(checkpoint_path).await
    }

    async fn compact(&self) -> CoreResult<()> {
        self.compact_async().await
    }

    // ==================== Content Storage ====================

    async fn store_content(&self, id: Uuid, content: &str) -> CoreResult<()> {
        self.store_content_async(id, content).await
    }

    async fn get_content(&self, id: Uuid) -> CoreResult<Option<String>> {
        self.get_content_async(id).await
    }

    async fn get_content_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<String>>> {
        self.get_content_batch_async(ids).await
    }

    async fn delete_content(&self, id: Uuid) -> CoreResult<bool> {
        self.delete_content_async(id).await
    }

    // ==================== Source Metadata Storage ====================

    async fn store_source_metadata(&self, id: Uuid, metadata: &SourceMetadata) -> CoreResult<()> {
        self.store_source_metadata_async(id, metadata).await
    }

    async fn get_source_metadata(&self, id: Uuid) -> CoreResult<Option<SourceMetadata>> {
        self.get_source_metadata_async(id).await
    }

    async fn delete_source_metadata(&self, id: Uuid) -> CoreResult<bool> {
        self.delete_source_metadata_async(id).await
    }

    async fn get_source_metadata_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<SourceMetadata>>> {
        self.get_source_metadata_batch_async(ids).await
    }

    async fn find_fingerprints_by_file_path(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        self.find_fingerprints_by_file_path(file_path).await
    }

    // ==================== File Index Storage ====================

    async fn list_indexed_files(&self) -> CoreResult<Vec<context_graph_core::types::file_index::FileIndexEntry>> {
        self.list_indexed_files_async().await
    }

    async fn get_fingerprints_for_file(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        self.get_fingerprints_for_file_async(file_path).await
    }

    async fn index_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<()> {
        self.index_file_fingerprint_async(file_path, fingerprint_id).await
    }

    async fn unindex_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<bool> {
        self.unindex_file_fingerprint_async(file_path, fingerprint_id).await
    }

    async fn clear_file_index(&self, file_path: &str) -> CoreResult<usize> {
        self.clear_file_index_async(file_path).await
    }

    async fn get_file_watcher_stats(&self) -> CoreResult<context_graph_core::types::file_index::FileWatcherStats> {
        self.get_file_watcher_stats_async().await
    }

    // ==================== Topic Portfolio Persistence ====================

    async fn persist_topic_portfolio(
        &self,
        session_id: &str,
        portfolio: &context_graph_core::clustering::PersistedTopicPortfolio,
    ) -> CoreResult<()> {
        self.persist_topic_portfolio_async(session_id, portfolio).await
    }

    async fn load_topic_portfolio(
        &self,
        session_id: &str,
    ) -> CoreResult<Option<context_graph_core::clustering::PersistedTopicPortfolio>> {
        self.load_topic_portfolio_async(session_id).await
    }

    async fn load_latest_topic_portfolio(
        &self,
    ) -> CoreResult<Option<context_graph_core::clustering::PersistedTopicPortfolio>> {
        self.load_latest_topic_portfolio_async().await
    }
}
