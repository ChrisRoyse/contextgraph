//! TeleologicalMemoryStore trait implementation for RocksDbTeleologicalStore.
//!
//! This module contains the async trait implementation that delegates to
//! the modular implementation files:
//! - `crud.rs` - CRUD operations
//! - `search.rs` - Search operations
//! - `persistence.rs` - Batch, statistics, persistence, content

use std::any::Any;
use std::path::PathBuf;

use async_trait::async_trait;
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
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

    async fn search_e6_sparse(
        &self,
        sparse_query: &SparseVector,
        max_candidates: usize,
    ) -> CoreResult<Vec<(Uuid, usize)>> {
        // Delegate to the existing e6_sparse_recall method in inverted_index.rs
        self.e6_sparse_recall(sparse_query, max_candidates)
            .map_err(|e| CoreError::IndexError(e.to_string()))
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
        self.checkpoint_sync()
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

    // ==================== Clustering Support ====================

    async fn scan_fingerprints_for_clustering(
        &self,
        limit: Option<usize>,
    ) -> CoreResult<Vec<(Uuid, [Vec<f32>; 13])>> {
        self.scan_fingerprints_for_clustering_async(limit).await
    }

    async fn list_fingerprints_unbiased(
        &self,
        limit: usize,
    ) -> CoreResult<Vec<context_graph_core::types::fingerprint::TeleologicalFingerprint>> {
        self.list_fingerprints_unbiased_async(limit).await
    }

    // ==================== Causal Relationship Storage ====================

    async fn store_causal_relationship(
        &self,
        relationship: &context_graph_core::types::CausalRelationship,
    ) -> CoreResult<Uuid> {
        RocksDbTeleologicalStore::store_causal_relationship(self, relationship).await
    }

    async fn get_causal_relationship(
        &self,
        id: Uuid,
    ) -> CoreResult<Option<context_graph_core::types::CausalRelationship>> {
        RocksDbTeleologicalStore::get_causal_relationship(self, id).await
    }

    async fn get_causal_relationships_by_source(
        &self,
        source_id: Uuid,
    ) -> CoreResult<Vec<context_graph_core::types::CausalRelationship>> {
        RocksDbTeleologicalStore::get_causal_relationships_by_source(self, source_id).await
    }

    async fn count_causal_relationships(&self) -> CoreResult<usize> {
        RocksDbTeleologicalStore::count_causal_relationships(self).await
    }

    async fn search_causal_relationships(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        direction_filter: Option<&str>,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        RocksDbTeleologicalStore::search_causal_relationships(self, query_embedding, top_k, direction_filter).await
    }

    async fn search_causal_e5(
        &self,
        query_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        RocksDbTeleologicalStore::search_causal_e5(self, query_embedding, search_causes, top_k).await
    }

    async fn search_causal_e5_hybrid(
        &self,
        query_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
        source_weight: f32,
        explanation_weight: f32,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        RocksDbTeleologicalStore::search_causal_e5_hybrid(
            self,
            query_embedding,
            search_causes,
            top_k,
            source_weight,
            explanation_weight,
        ).await
    }

    async fn search_causal_e8(
        &self,
        query_embedding: &[f32],
        search_sources: bool,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        RocksDbTeleologicalStore::search_causal_e8(self, query_embedding, search_sources, top_k).await
    }

    async fn search_causal_e11(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        RocksDbTeleologicalStore::search_causal_e11(self, query_embedding, top_k).await
    }

    async fn search_causal_multi_embedder(
        &self,
        e1_embedding: &[f32],
        e5_embedding: &[f32],
        e8_embedding: &[f32],
        e11_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
        config: &context_graph_core::types::MultiEmbedderConfig,
    ) -> CoreResult<Vec<context_graph_core::types::CausalSearchResult>> {
        RocksDbTeleologicalStore::search_causal_multi_embedder(
            self,
            e1_embedding,
            e5_embedding,
            e8_embedding,
            e11_embedding,
            search_causes,
            top_k,
            config,
        ).await
    }

    // ==================== Audit Log (Phase 1.1) ====================

    async fn append_audit_record(&self, record: &context_graph_core::types::audit::AuditRecord) -> CoreResult<()> {
        self.append_audit_record(record).map_err(Into::into)
    }

    async fn get_audit_by_target(
        &self,
        target_id: uuid::Uuid,
        limit: usize,
    ) -> CoreResult<Vec<context_graph_core::types::audit::AuditRecord>> {
        self.get_audit_by_target(target_id, limit).map_err(Into::into)
    }

    async fn get_audit_by_time_range(
        &self,
        start: chrono::DateTime<chrono::Utc>,
        end: chrono::DateTime<chrono::Utc>,
        limit: usize,
    ) -> CoreResult<Vec<context_graph_core::types::audit::AuditRecord>> {
        self.get_audit_by_time_range(start, end, limit).map_err(Into::into)
    }

    async fn count_audit_records(&self) -> CoreResult<usize> {
        self.count_audit_records().map_err(Into::into)
    }

    // ==================== Merge History (Phase 4) ====================

    async fn append_merge_record(&self, record: &context_graph_core::types::audit::MergeRecord) -> CoreResult<()> {
        self.append_merge_record(record).map_err(Into::into)
    }

    async fn get_merge_history(
        &self,
        merged_id: uuid::Uuid,
        limit: usize,
    ) -> CoreResult<Vec<context_graph_core::types::audit::MergeRecord>> {
        self.get_merge_history(merged_id, limit).map_err(Into::into)
    }

    // ==================== Importance History (Phase 4) ====================

    async fn append_importance_change(&self, record: &context_graph_core::types::audit::ImportanceChangeRecord) -> CoreResult<()> {
        self.append_importance_change(record).map_err(Into::into)
    }

    async fn get_importance_history(
        &self,
        memory_id: uuid::Uuid,
        limit: usize,
    ) -> CoreResult<Vec<context_graph_core::types::audit::ImportanceChangeRecord>> {
        self.get_importance_history(memory_id, limit).map_err(Into::into)
    }

    // ==================== Embedding Version Registry (Phase 6) ====================

    async fn store_embedding_version(&self, record: &context_graph_core::types::audit::EmbeddingVersionRecord) -> CoreResult<()> {
        self.store_embedding_version(record).map_err(Into::into)
    }

    async fn get_embedding_version(
        &self,
        fingerprint_id: uuid::Uuid,
    ) -> CoreResult<Option<context_graph_core::types::audit::EmbeddingVersionRecord>> {
        self.get_embedding_version(fingerprint_id).map_err(Into::into)
    }

    // ==================== Custom Weight Profile Persistence ====================

    async fn store_custom_weight_profile(&self, name: &str, weights: &[f32; 13]) -> CoreResult<()> {
        self.store_custom_weight_profile(name, weights).map_err(Into::into)
    }

    async fn get_custom_weight_profile(&self, name: &str) -> CoreResult<Option<[f32; 13]>> {
        self.get_custom_weight_profile(name).map_err(Into::into)
    }

    async fn list_custom_weight_profiles(&self) -> CoreResult<Vec<(String, [f32; 13])>> {
        self.list_custom_weight_profiles().map_err(Into::into)
    }

    async fn delete_custom_weight_profile(&self, name: &str) -> CoreResult<bool> {
        self.delete_custom_weight_profile(name).map_err(Into::into)
    }

    // ==================== Processing Cursor Persistence ====================

    async fn store_processing_cursor(&self, key: &str, data: &[u8]) -> CoreResult<()> {
        self.store_processing_cursor_sync(key, data)
    }

    async fn get_processing_cursor(&self, key: &str) -> CoreResult<Option<Vec<u8>>> {
        self.get_processing_cursor_sync(key)
    }

    // ==================== Type Downcasting ====================

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn persist_hnsw_indexes_if_available(&self) -> CoreResult<()> {
        self.persist_hnsw_indexes().map_err(|e| {
            CoreError::StorageError(format!("HNSW persistence on shutdown failed: {e}"))
        })
    }
}
