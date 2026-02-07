//! Complete TeleologicalMemoryStore trait implementation.
//!
//! This module contains the full trait implementation that delegates to
//! the various impl methods in other submodules.

use std::any::Any;
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
        use crate::types::FileIndexEntry;

        // Use file_index as primary source (proper implementation)
        let entries: Vec<FileIndexEntry> = self
            .file_index
            .iter()
            .map(|entry| {
                let mut file_entry = FileIndexEntry::new(entry.key().clone());
                for id in entry.value().iter() {
                    file_entry.add_fingerprint(*id);
                }
                file_entry
            })
            .collect();

        debug!(
            indexed_file_count = entries.len(),
            "Listed indexed files from in-memory store"
        );
        Ok(entries)
    }

    async fn get_fingerprints_for_file(&self, file_path: &str) -> CoreResult<Vec<Uuid>> {
        // Use file_index as primary source (proper implementation)
        let ids = self
            .file_index
            .get(file_path)
            .map(|entry| entry.value().clone())
            .unwrap_or_default();

        debug!(
            file_path = %file_path,
            fingerprint_count = ids.len(),
            "Retrieved fingerprints for file"
        );
        Ok(ids)
    }

    async fn index_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<()> {
        // Proper implementation: maintain file_index DashMap
        let mut entry = self.file_index.entry(file_path.to_string()).or_insert_with(Vec::new);
        if !entry.contains(&fingerprint_id) {
            entry.push(fingerprint_id);
        }
        info!(
            file_path = %file_path,
            fingerprint_id = %fingerprint_id,
            total_count = entry.len(),
            "Indexed fingerprint for file"
        );
        Ok(())
    }

    async fn unindex_file_fingerprint(&self, file_path: &str, fingerprint_id: Uuid) -> CoreResult<bool> {
        // Proper implementation: remove from file_index DashMap
        let removed = if let Some(mut entry) = self.file_index.get_mut(file_path) {
            let before_len = entry.len();
            entry.retain(|&id| id != fingerprint_id);
            let removed = entry.len() < before_len;
            if removed {
                info!(
                    file_path = %file_path,
                    fingerprint_id = %fingerprint_id,
                    remaining = entry.len(),
                    "Unindexed fingerprint from file"
                );
            }
            removed
        } else {
            debug!(
                file_path = %file_path,
                fingerprint_id = %fingerprint_id,
                "No index entry for file, nothing to unindex"
            );
            false
        };

        // Clean up empty entries
        if let Some(entry) = self.file_index.get(file_path) {
            if entry.is_empty() {
                drop(entry);
                self.file_index.remove(file_path);
                debug!(file_path = %file_path, "Removed empty file index entry");
            }
        }

        Ok(removed)
    }

    async fn clear_file_index(&self, file_path: &str) -> CoreResult<usize> {
        // Proper implementation: remove entry from file_index DashMap and return count
        let count = if let Some((_, ids)) = self.file_index.remove(file_path) {
            let count = ids.len();
            info!(
                file_path = %file_path,
                fingerprints_removed = count,
                "Cleared file index"
            );
            count
        } else {
            debug!(file_path = %file_path, "No index entry for file, nothing to clear");
            0
        };
        Ok(count)
    }

    async fn get_file_watcher_stats(&self) -> CoreResult<crate::types::FileWatcherStats> {
        // Use file_index as primary source (proper implementation)
        if self.file_index.is_empty() {
            return Ok(crate::types::FileWatcherStats::default());
        }

        let chunk_counts: Vec<usize> = self
            .file_index
            .iter()
            .map(|entry| entry.value().len())
            .collect();

        let total_files = chunk_counts.len();
        let total_chunks: usize = chunk_counts.iter().sum();
        let min_chunks = *chunk_counts.iter().min().unwrap_or(&0);
        let max_chunks = *chunk_counts.iter().max().unwrap_or(&0);
        let avg_chunks_per_file = total_chunks as f64 / total_files as f64;

        debug!(
            total_files = total_files,
            total_chunks = total_chunks,
            avg_chunks = avg_chunks_per_file,
            "Computed file watcher stats"
        );

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

            // Apply mechanism type filter if specified
            if let Some(filter) = direction_filter {
                if filter != "all" && rel.normalized_mechanism_type() != filter {
                    continue;
                }
            }

            // Compute cosine similarity using E1 semantic embedding
            let similarity = compute_cosine_similarity(query_embedding, &rel.e1_semantic);
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

    async fn search_causal_e5(
        &self,
        query_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for entry in self.causal_relationships.iter() {
            let rel = entry.value();

            // Select appropriate E5 vector based on search mode
            let doc_embedding = if search_causes {
                &rel.e5_as_cause
            } else {
                &rel.e5_as_effect
            };

            // Skip if E5 embeddings are empty (legacy data with placeholder zeros)
            if doc_embedding.iter().all(|&v| v == 0.0) {
                continue;
            }

            let similarity = compute_cosine_similarity(query_embedding, doc_embedding);
            results.push((rel.id, similarity));
        }

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k
        results.truncate(top_k);

        debug!(
            query_dim = query_embedding.len(),
            search_causes = search_causes,
            top_k = top_k,
            results_count = results.len(),
            "Searched causal relationships using E5 in in-memory store"
        );

        Ok(results)
    }

    async fn search_causal_e5_hybrid(
        &self,
        query_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
        source_weight: f32,
        explanation_weight: f32,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for entry in self.causal_relationships.iter() {
            let rel = entry.value();

            // Select appropriate E5 vectors based on search mode
            let (explanation_embedding, source_embedding) = if search_causes {
                (&rel.e5_as_cause, &rel.e5_source_cause)
            } else {
                (&rel.e5_as_effect, &rel.e5_source_effect)
            };

            // Skip if E5 explanation embeddings are empty (legacy data)
            if explanation_embedding.iter().all(|&v| v == 0.0) {
                continue;
            }

            // Compute explanation similarity
            let explanation_sim = compute_cosine_similarity(query_embedding, explanation_embedding);

            // Compute source similarity (if source embeddings exist)
            let source_sim = if source_embedding.is_empty() || source_embedding.iter().all(|&v| v == 0.0) {
                // No source embeddings - fall back to explanation only
                explanation_sim
            } else {
                compute_cosine_similarity(query_embedding, source_embedding)
            };

            // Compute hybrid score
            let hybrid_score = source_weight * source_sim + explanation_weight * explanation_sim;
            results.push((rel.id, hybrid_score));
        }

        // Sort by hybrid score descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k
        results.truncate(top_k);

        debug!(
            query_dim = query_embedding.len(),
            search_causes = search_causes,
            top_k = top_k,
            source_weight = source_weight,
            explanation_weight = explanation_weight,
            results_count = results.len(),
            "Searched causal relationships using E5 hybrid scoring in in-memory store"
        );

        Ok(results)
    }

    async fn search_causal_e8(
        &self,
        query_embedding: &[f32],
        search_sources: bool,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for entry in self.causal_relationships.iter() {
            let rel = entry.value();

            // Select E8 vector based on search mode
            let doc_embedding = if search_sources {
                &rel.e8_graph_source
            } else {
                &rel.e8_graph_target
            };

            // Skip if E8 embeddings are empty
            if doc_embedding.is_empty() || doc_embedding.iter().all(|&v| v == 0.0) {
                continue;
            }

            let similarity = compute_cosine_similarity(query_embedding, doc_embedding);
            results.push((rel.id, similarity));
        }

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k
        results.truncate(top_k);

        debug!(
            query_dim = query_embedding.len(),
            search_sources = search_sources,
            top_k = top_k,
            results_count = results.len(),
            "Searched causal relationships using E8 in in-memory store"
        );

        Ok(results)
    }

    async fn search_causal_e11(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        use crate::types::CausalRelationship;

        // Validate query embedding dimension (E11 KEPLER is 768D)
        if query_embedding.len() != CausalRelationship::E11_DIM {
            error!(
                "E11 query embedding dimension mismatch: expected {}, got {}",
                CausalRelationship::E11_DIM,
                query_embedding.len()
            );
            return Err(CoreError::ValidationError {
                field: "query_embedding".to_string(),
                message: format!(
                    "E11 query embedding must be {}D (KEPLER), got {}D",
                    CausalRelationship::E11_DIM,
                    query_embedding.len()
                ),
            });
        }

        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for entry in self.causal_relationships.iter() {
            let rel = entry.value();

            // Use e11_entity for E11 entity search - ARCH-02: apples-to-apples comparison
            // E11 (KEPLER) finds entity relationships that E1 misses (e.g., "Diesel" = Rust ORM)
            if !rel.has_entity_embedding() {
                // Skip relationships without E11 embeddings
                continue;
            }

            let similarity = compute_cosine_similarity(query_embedding, rel.e11_embedding());
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
            "Searched causal relationships using E11 (KEPLER entity embeddings) in in-memory store"
        );

        Ok(results)
    }

    async fn search_causal_multi_embedder(
        &self,
        e1_embedding: &[f32],
        e5_embedding: &[f32],
        e8_embedding: &[f32],
        e11_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
        config: &crate::types::MultiEmbedderConfig,
    ) -> CoreResult<Vec<crate::types::CausalSearchResult>> {
        use crate::types::CausalSearchResult;

        // Parallel search across all embedders
        let e1_results = self.search_causal_relationships(e1_embedding, top_k * 3, None).await?;
        let e5_results = self.search_causal_e5_hybrid(
            e5_embedding,
            search_causes,
            top_k * 3,
            0.6,
            0.4,
        ).await?;
        let e8_results = self.search_causal_e8(e8_embedding, !search_causes, top_k * 3).await?;
        let e11_results = self.search_causal_e11(e11_embedding, top_k * 3).await?;

        // Build score maps for each embedder
        use std::collections::HashMap;
        let e1_scores: HashMap<Uuid, f32> = e1_results.into_iter().collect();
        let e5_scores: HashMap<Uuid, f32> = e5_results.into_iter().collect();
        let e8_scores: HashMap<Uuid, f32> = e8_results.into_iter().collect();
        let e11_scores: HashMap<Uuid, f32> = e11_results.into_iter().collect();

        // Collect all unique IDs
        let mut all_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
        all_ids.extend(e1_scores.keys().cloned());
        all_ids.extend(e5_scores.keys().cloned());
        all_ids.extend(e8_scores.keys().cloned());
        all_ids.extend(e11_scores.keys().cloned());

        // Compute RRF scores for each candidate
        let k = 60.0f32;
        let mut rrf_scores: Vec<(Uuid, f32)> = Vec::new();

        // Build rank maps
        let e1_ranks: HashMap<Uuid, usize> = e1_scores.iter()
            .enumerate()
            .map(|(rank, (id, _))| (*id, rank))
            .collect();
        let e5_ranks: HashMap<Uuid, usize> = e5_scores.iter()
            .enumerate()
            .map(|(rank, (id, _))| (*id, rank))
            .collect();
        let e8_ranks: HashMap<Uuid, usize> = e8_scores.iter()
            .enumerate()
            .map(|(rank, (id, _))| (*id, rank))
            .collect();
        let e11_ranks: HashMap<Uuid, usize> = e11_scores.iter()
            .enumerate()
            .map(|(rank, (id, _))| (*id, rank))
            .collect();

        for id in all_ids {
            let mut rrf_score = 0.0f32;

            if let Some(&rank) = e1_ranks.get(&id) {
                rrf_score += config.e1_weight / (k + rank as f32 + 1.0);
            }
            if let Some(&rank) = e5_ranks.get(&id) {
                rrf_score += config.e5_weight / (k + rank as f32 + 1.0);
            }
            if let Some(&rank) = e8_ranks.get(&id) {
                rrf_score += config.e8_weight / (k + rank as f32 + 1.0);
            }
            if let Some(&rank) = e11_ranks.get(&id) {
                rrf_score += config.e11_weight / (k + rank as f32 + 1.0);
            }

            rrf_scores.push((id, rrf_score));
        }

        // Sort by RRF score descending
        rrf_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top_k and build results
        let mut results = Vec::with_capacity(top_k.min(rrf_scores.len()));
        for (id, rrf_score) in rrf_scores.into_iter().take(top_k) {
            let relationship = self.causal_relationships.get(&id).map(|r| r.clone());

            let mut result = CausalSearchResult {
                id,
                relationship,
                e1_score: e1_scores.get(&id).cloned().unwrap_or(0.0),
                e5_score: e5_scores.get(&id).cloned().unwrap_or(0.0),
                e8_score: e8_scores.get(&id).cloned().unwrap_or(0.0),
                e11_score: e11_scores.get(&id).cloned().unwrap_or(0.0),
                rrf_score,
                maxsim_score: None,
                transe_confidence: 0.0,
                consensus_score: 0.0,
                direction_confidence: 0.0,
            };

            result.compute_consensus();
            results.push(result);
        }

        debug!(
            top_k = top_k,
            results_count = results.len(),
            "Multi-embedder causal search in in-memory store"
        );

        Ok(results)
    }

    // ==================== Audit Log (Phase 1.1 Stubs) ====================

    async fn append_audit_record(&self, _record: &crate::types::audit::AuditRecord) -> CoreResult<()> {
        // Test stub: no-op for audit log append
        Ok(())
    }

    async fn get_audit_by_target(
        &self,
        _target_id: Uuid,
        _limit: usize,
    ) -> CoreResult<Vec<crate::types::audit::AuditRecord>> {
        // Test stub: always returns empty
        Ok(Vec::new())
    }

    async fn get_audit_by_time_range(
        &self,
        _start: chrono::DateTime<chrono::Utc>,
        _end: chrono::DateTime<chrono::Utc>,
        _limit: usize,
    ) -> CoreResult<Vec<crate::types::audit::AuditRecord>> {
        // Test stub: always returns empty
        Ok(Vec::new())
    }

    async fn count_audit_records(&self) -> CoreResult<usize> {
        // Test stub: always returns 0
        Ok(0)
    }

    // ==================== Merge History (Phase 4 Stubs) ====================

    async fn append_merge_record(&self, _record: &crate::types::audit::MergeRecord) -> CoreResult<()> {
        // Test stub: no-op
        Ok(())
    }

    async fn get_merge_history(
        &self,
        _merged_id: Uuid,
        _limit: usize,
    ) -> CoreResult<Vec<crate::types::audit::MergeRecord>> {
        // Test stub: always returns empty
        Ok(Vec::new())
    }

    // ==================== Importance History (Phase 4 Stubs) ====================

    async fn append_importance_change(&self, _record: &crate::types::audit::ImportanceChangeRecord) -> CoreResult<()> {
        // Test stub: no-op
        Ok(())
    }

    async fn get_importance_history(
        &self,
        _memory_id: Uuid,
        _limit: usize,
    ) -> CoreResult<Vec<crate::types::audit::ImportanceChangeRecord>> {
        // Test stub: always returns empty
        Ok(Vec::new())
    }

    // ==================== Embedding Version Registry (Phase 6 Stubs) ====================

    async fn store_embedding_version(&self, _record: &crate::types::audit::EmbeddingVersionRecord) -> CoreResult<()> {
        // Test stub: no-op
        Ok(())
    }

    async fn get_embedding_version(
        &self,
        _fingerprint_id: Uuid,
    ) -> CoreResult<Option<crate::types::audit::EmbeddingVersionRecord>> {
        // Test stub: always returns None
        Ok(None)
    }

    // ==================== Type Downcasting ====================

    fn as_any(&self) -> &dyn Any {
        self
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
