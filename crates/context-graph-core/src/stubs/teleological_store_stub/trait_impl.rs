//! Complete TeleologicalMemoryStore trait implementation.
//!
//! This module contains the full trait implementation that delegates to
//! the various impl methods in other submodules.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use async_trait::async_trait;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::InMemoryTeleologicalStore;
use crate::error::{CoreError, CoreResult};
use crate::traits::{
    TeleologicalMemoryStore, TeleologicalSearchOptions, TeleologicalSearchResult,
    TeleologicalStorageBackend,
};
use crate::types::fingerprint::{SemanticFingerprint, SparseVector, TeleologicalFingerprint};
use crate::types::SourceMetadata;

#[async_trait]
impl TeleologicalMemoryStore for InMemoryTeleologicalStore {
    async fn store(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<Uuid> {
        let id = fingerprint.id;
        let size = Self::estimate_fingerprint_size(&fingerprint);
        debug!("Storing fingerprint {} ({} bytes)", id, size);
        self.data.insert(id, fingerprint);
        self.size_bytes.fetch_add(size, Ordering::Relaxed);
        Ok(id)
    }

    async fn retrieve(&self, id: Uuid) -> CoreResult<Option<TeleologicalFingerprint>> {
        if self.deleted.contains_key(&id) {
            debug!("Fingerprint {} is soft-deleted", id);
            return Ok(None);
        }
        Ok(self.data.get(&id).map(|r| r.clone()))
    }

    async fn update(&self, fingerprint: TeleologicalFingerprint) -> CoreResult<bool> {
        let id = fingerprint.id;
        if !self.data.contains_key(&id) {
            debug!("Update failed: fingerprint {} not found", id);
            return Ok(false);
        }
        let old_size = self
            .data
            .get(&id)
            .map(|r| Self::estimate_fingerprint_size(&r))
            .unwrap_or(0);
        let new_size = Self::estimate_fingerprint_size(&fingerprint);
        self.data.insert(id, fingerprint);
        if new_size > old_size {
            self.size_bytes
                .fetch_add(new_size - old_size, Ordering::Relaxed);
        } else {
            self.size_bytes
                .fetch_sub(old_size - new_size, Ordering::Relaxed);
        }
        debug!("Updated fingerprint {}", id);
        Ok(true)
    }

    async fn delete(&self, id: Uuid, soft: bool) -> CoreResult<bool> {
        if !self.data.contains_key(&id) {
            debug!("Delete failed: fingerprint {} not found", id);
            return Ok(false);
        }
        if soft {
            self.deleted.insert(id, ());
            debug!("Soft-deleted fingerprint {}", id);
        } else {
            if let Some((_, fp)) = self.data.remove(&id) {
                let size = Self::estimate_fingerprint_size(&fp);
                self.size_bytes.fetch_sub(size, Ordering::Relaxed);
            }
            self.deleted.remove(&id);
            self.content.remove(&id);
            debug!("Hard-deleted fingerprint {} (content also removed)", id);
        }
        Ok(true)
    }

    async fn search_semantic(
        &self,
        query: &SemanticFingerprint,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        self.search_semantic_impl(query, options).await
    }

    async fn search_text(
        &self,
        text: &str,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        self.search_text_impl(text, options).await
    }

    async fn search_sparse(
        &self,
        sparse_query: &SparseVector,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        self.search_sparse_impl(sparse_query, top_k).await
    }

    async fn search_e6_sparse(
        &self,
        sparse_query: &SparseVector,
        max_candidates: usize,
    ) -> CoreResult<Vec<(Uuid, usize)>> {
        self.search_e6_sparse_impl(sparse_query, max_candidates).await
    }

    async fn store_batch(
        &self,
        fingerprints: Vec<TeleologicalFingerprint>,
    ) -> CoreResult<Vec<Uuid>> {
        debug!("Batch storing {} fingerprints", fingerprints.len());
        let mut ids = Vec::with_capacity(fingerprints.len());
        for fp in fingerprints {
            let id = self.store(fp).await?;
            ids.push(id);
        }
        Ok(ids)
    }

    async fn retrieve_batch(
        &self,
        ids: &[Uuid],
    ) -> CoreResult<Vec<Option<TeleologicalFingerprint>>> {
        debug!("Batch retrieving {} fingerprints", ids.len());
        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            results.push(self.retrieve(*id).await?);
        }
        Ok(results)
    }

    async fn count(&self) -> CoreResult<usize> {
        Ok(self.data.len() - self.deleted.len())
    }

    fn storage_size_bytes(&self) -> usize {
        self.size_bytes.load(Ordering::Relaxed)
    }

    fn backend_type(&self) -> TeleologicalStorageBackend {
        TeleologicalStorageBackend::InMemory
    }

    async fn flush(&self) -> CoreResult<()> {
        debug!("Flush called on in-memory store (no-op)");
        Ok(())
    }

    async fn checkpoint(&self) -> CoreResult<PathBuf> {
        warn!("Checkpoint requested but InMemoryTeleologicalStore does not persist data");
        Err(CoreError::FeatureDisabled {
            feature: "checkpoint".to_string(),
        })
    }

    async fn restore(&self, checkpoint_path: &Path) -> CoreResult<()> {
        error!(
            "Restore from {:?} requested but InMemoryTeleologicalStore does not persist data",
            checkpoint_path
        );
        Err(CoreError::FeatureDisabled {
            feature: "restore".to_string(),
        })
    }

    async fn compact(&self) -> CoreResult<()> {
        let deleted_ids: Vec<Uuid> = self.deleted.iter().map(|r| *r.key()).collect();
        for id in deleted_ids {
            if let Some((_, fp)) = self.data.remove(&id) {
                let size = Self::estimate_fingerprint_size(&fp);
                self.size_bytes.fetch_sub(size, Ordering::Relaxed);
            }
            self.deleted.remove(&id);
        }
        info!(
            "Compaction complete: removed {} soft-deleted entries",
            self.deleted.len()
        );
        Ok(())
    }

    async fn store_content(&self, id: Uuid, content: &str) -> CoreResult<()> {
        self.store_content_impl(id, content).await
    }

    async fn get_content(&self, id: Uuid) -> CoreResult<Option<String>> {
        self.get_content_impl(id).await
    }

    async fn get_content_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<String>>> {
        self.get_content_batch_impl(ids).await
    }

    async fn delete_content(&self, id: Uuid) -> CoreResult<bool> {
        self.delete_content_impl(id).await
    }

    // ==================== Source Metadata Storage ====================

    async fn store_source_metadata(&self, id: Uuid, metadata: &SourceMetadata) -> CoreResult<()> {
        debug!("Storing source metadata for {}", id);
        self.source_metadata.insert(id, metadata.clone());
        Ok(())
    }

    async fn get_source_metadata(&self, id: Uuid) -> CoreResult<Option<SourceMetadata>> {
        Ok(self.source_metadata.get(&id).map(|r| r.clone()))
    }

    async fn delete_source_metadata(&self, id: Uuid) -> CoreResult<bool> {
        Ok(self.source_metadata.remove(&id).is_some())
    }

    async fn get_source_metadata_batch(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<SourceMetadata>>> {
        Ok(ids
            .iter()
            .map(|id| self.source_metadata.get(id).map(|r| r.clone()))
            .collect())
    }

    async fn find_fingerprints_by_file_path(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        let mut matching_ids = Vec::new();
        for entry in self.source_metadata.iter() {
            if let Some(ref path) = entry.value().file_path {
                if path == file_path {
                    matching_ids.push(*entry.key());
                }
            }
        }
        debug!("Found {} fingerprints for file_path: {}", matching_ids.len(), file_path);
        Ok(matching_ids)
    }

    // ==================== File Index Storage ====================
    // In-memory stub implementation uses source_metadata scanning as fallback

    async fn list_indexed_files(&self) -> CoreResult<Vec<crate::types::FileIndexEntry>> {
        use std::collections::HashMap;
        use crate::types::FileIndexEntry;

        // Build file entries from source_metadata (fallback scan approach)
        let mut file_map: HashMap<String, Vec<Uuid>> = HashMap::new();
        for entry in self.source_metadata.iter() {
            if let Some(ref path) = entry.value().file_path {
                file_map.entry(path.clone()).or_default().push(*entry.key());
            }
        }

        let entries: Vec<FileIndexEntry> = file_map
            .into_iter()
            .map(|(path, ids)| {
                let mut entry = FileIndexEntry::new(path);
                for id in ids {
                    entry.add_fingerprint(id);
                }
                entry
            })
            .collect();

        debug!("Listed {} indexed files from in-memory store", entries.len());
        Ok(entries)
    }

    async fn get_fingerprints_for_file(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        // Delegate to find_fingerprints_by_file_path (same behavior for stub)
        self.find_fingerprints_by_file_path(file_path).await
    }

    async fn index_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<()> {
        // In-memory stub: No-op, source_metadata is already tracking by file_path
        debug!(
            "index_file_fingerprint called for '{}' with {} (no-op in stub)",
            file_path, fingerprint_id
        );
        Ok(())
    }

    async fn unindex_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<bool> {
        // In-memory stub: No-op, source_metadata will be cleaned up by delete operations
        debug!(
            "unindex_file_fingerprint called for '{}' with {} (no-op in stub)",
            file_path, fingerprint_id
        );
        Ok(false)
    }

    async fn clear_file_index(&self, file_path: &str) -> CoreResult<usize> {
        // In-memory stub: Count matching entries but don't maintain separate index
        let count = self.find_fingerprints_by_file_path(file_path).await?.len();
        debug!(
            "clear_file_index called for '{}': {} entries (no-op in stub)",
            file_path, count
        );
        Ok(count)
    }

    async fn get_file_watcher_stats(&self) -> CoreResult<crate::types::FileWatcherStats> {
        use std::collections::HashMap;

        // Build stats from source_metadata
        let mut file_chunks: HashMap<String, usize> = HashMap::new();
        for entry in self.source_metadata.iter() {
            if let Some(ref path) = entry.value().file_path {
                *file_chunks.entry(path.clone()).or_default() += 1;
            }
        }

        if file_chunks.is_empty() {
            return Ok(crate::types::FileWatcherStats::default());
        }

        let total_files = file_chunks.len();
        let chunk_counts: Vec<usize> = file_chunks.values().cloned().collect();
        let total_chunks: usize = chunk_counts.iter().sum();
        let min_chunks = *chunk_counts.iter().min().unwrap_or(&0);
        let max_chunks = *chunk_counts.iter().max().unwrap_or(&0);
        let avg_chunks_per_file = total_chunks as f64 / total_files as f64;

        Ok(crate::types::FileWatcherStats {
            total_files,
            total_chunks,
            avg_chunks_per_file,
            min_chunks,
            max_chunks,
        })
    }

    // ==================== Topic Portfolio Persistence ====================
    // In-memory stub implementation stores portfolios in memory only

    async fn persist_topic_portfolio(
        &self,
        session_id: &str,
        portfolio: &crate::clustering::PersistedTopicPortfolio,
    ) -> CoreResult<()> {
        debug!(
            session_id = %session_id,
            topic_count = portfolio.topics.len(),
            "Persisting topic portfolio to in-memory store"
        );
        self.topic_portfolios.insert(session_id.to_string(), portfolio.clone());
        // Also store as "__latest__"
        self.topic_portfolios.insert("__latest__".to_string(), portfolio.clone());
        Ok(())
    }

    async fn load_topic_portfolio(
        &self,
        session_id: &str,
    ) -> CoreResult<Option<crate::clustering::PersistedTopicPortfolio>> {
        debug!(session_id = %session_id, "Loading topic portfolio from in-memory store");
        Ok(self.topic_portfolios.get(session_id).map(|r| r.clone()))
    }

    async fn load_latest_topic_portfolio(
        &self,
    ) -> CoreResult<Option<crate::clustering::PersistedTopicPortfolio>> {
        debug!("Loading latest topic portfolio from in-memory store");
        Ok(self.topic_portfolios.get("__latest__").map(|r| r.clone()))
    }

    async fn scan_fingerprints_for_clustering(
        &self,
        limit: Option<usize>,
    ) -> CoreResult<Vec<(uuid::Uuid, [Vec<f32>; 13])>> {
        debug!(limit = ?limit, "Scanning fingerprints for clustering from in-memory store");

        let mut results = Vec::new();

        for entry in self.data.iter() {
            // Skip soft-deleted fingerprints
            if self.deleted.contains_key(entry.key()) {
                continue;
            }

            let fp = entry.value();
            let cluster_array = fp.semantic.to_cluster_array();
            results.push((fp.id, cluster_array));

            // Apply limit if specified
            if let Some(max) = limit {
                if results.len() >= max {
                    break;
                }
            }
        }

        debug!(count = results.len(), "Scanned fingerprints for clustering");
        Ok(results)
    }

    // ==================== Causal Relationship Storage ====================

    async fn store_causal_relationship(
        &self,
        relationship: &crate::types::CausalRelationship,
    ) -> CoreResult<Uuid> {
        let id = relationship.id;
        debug!(causal_id = %id, source_id = %relationship.source_fingerprint_id, "Storing causal relationship");

        // Store in primary map
        self.causal_relationships.insert(id, relationship.clone());

        // Update secondary index
        self.causal_by_source
            .entry(relationship.source_fingerprint_id)
            .or_insert_with(Vec::new)
            .push(id);

        Ok(id)
    }

    async fn get_causal_relationship(
        &self,
        id: Uuid,
    ) -> CoreResult<Option<crate::types::CausalRelationship>> {
        Ok(self.causal_relationships.get(&id).map(|r| r.clone()))
    }

    async fn get_causal_relationships_by_source(
        &self,
        source_id: Uuid,
    ) -> CoreResult<Vec<crate::types::CausalRelationship>> {
        let causal_ids = match self.causal_by_source.get(&source_id) {
            Some(ids) => ids.clone(),
            None => return Ok(Vec::new()),
        };

        let mut results = Vec::with_capacity(causal_ids.len());
        for causal_id in causal_ids {
            if let Some(rel) = self.causal_relationships.get(&causal_id) {
                results.push(rel.clone());
            }
        }

        debug!(source_id = %source_id, count = results.len(), "Retrieved causal relationships by source");
        Ok(results)
    }

    async fn search_causal_relationships(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        direction_filter: Option<&str>,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for entry in self.causal_relationships.iter() {
            let rel = entry.value();

            // Apply direction filter if specified
            if let Some(filter) = direction_filter {
                if filter != "all" && rel.normalized_direction() != filter {
                    continue;
                }
            }

            // Compute cosine similarity
            let similarity = compute_cosine_similarity(query_embedding, &rel.description_embedding);
            results.push((rel.id, similarity));
        }

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k
        results.truncate(top_k);

        debug!(
            query_dim = query_embedding.len(),
            top_k = top_k,
            results_count = results.len(),
            direction_filter = ?direction_filter,
            "Searched causal relationships in in-memory store"
        );

        Ok(results)
    }
}

/// Compute cosine similarity between two vectors.
fn compute_cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    let denom = (norm_a.sqrt()) * (norm_b.sqrt());
    if denom < f32::EPSILON {
        0.0
    } else {
        dot / denom
    }
}
