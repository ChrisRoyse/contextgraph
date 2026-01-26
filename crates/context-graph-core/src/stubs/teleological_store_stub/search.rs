//! Search operations for the in-memory teleological store.
//!
//! This module implements semantic, text, and sparse search operations.
//! All operations are O(n) full table scans - suitable for testing only.

use std::collections::HashSet;

use tracing::{debug, error};
use uuid::Uuid;

use super::similarity::compute_semantic_scores;
use super::InMemoryTeleologicalStore;
use crate::error::{CoreError, CoreResult};
use crate::traits::{TeleologicalSearchOptions, TeleologicalSearchResult};
use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

impl InMemoryTeleologicalStore {
    /// Search by semantic fingerprint similarity.
    pub async fn search_semantic_impl(
        &self,
        query: &SemanticFingerprint,
        options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        debug!(
            "Semantic search with top_k={}, min_similarity={}",
            options.top_k, options.min_similarity
        );

        let mut results: Vec<TeleologicalSearchResult> = Vec::new();
        let deleted_ids: HashSet<Uuid> = self.deleted.iter().map(|r| *r.key()).collect();

        for entry in self.data.iter() {
            let id = *entry.key();
            let fp = entry.value();

            if !options.include_deleted && deleted_ids.contains(&id) {
                continue;
            }

            let embedder_scores = compute_semantic_scores(query, &fp.semantic);

            let active_scores: Vec<f32> = if options.embedder_indices.is_empty() {
                embedder_scores.to_vec()
            } else {
                options
                    .embedder_indices
                    .iter()
                    .filter_map(|&i| embedder_scores.get(i).copied())
                    .collect()
            };

            let similarity = if active_scores.is_empty() {
                0.0
            } else {
                active_scores.iter().sum::<f32>() / active_scores.len() as f32
            };

            if similarity < options.min_similarity {
                continue;
            }

            results.push(TeleologicalSearchResult::new(fp.clone(), similarity, embedder_scores));
        }

        results.sort_by(|a, b| {
            b.similarity
                .partial_cmp(&a.similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(options.top_k);
        debug!("Semantic search returned {} results", results.len());
        Ok(results)
    }

    /// Search by text query (requires embedding provider).
    pub async fn search_text_impl(
        &self,
        _text: &str,
        _options: TeleologicalSearchOptions,
    ) -> CoreResult<Vec<TeleologicalSearchResult>> {
        error!(
            "search_text not supported in InMemoryTeleologicalStore (requires embedding provider)"
        );
        Err(CoreError::FeatureDisabled {
            feature: "text_search".to_string(),
        })
    }

    /// Search by sparse vector (SPLADE-style).
    pub async fn search_sparse_impl(
        &self,
        sparse_query: &SparseVector,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        debug!(
            "Sparse search with top_k={}, query nnz={}",
            top_k,
            sparse_query.nnz()
        );

        let mut results: Vec<(Uuid, f32)> = Vec::new();
        let deleted_ids: HashSet<Uuid> = self.deleted.iter().map(|r| *r.key()).collect();

        for entry in self.data.iter() {
            let id = *entry.key();
            let fp = entry.value();

            if deleted_ids.contains(&id) {
                continue;
            }

            let score = sparse_query.dot(&fp.semantic.e13_splade);

            if score > 0.0 {
                results.push((id, score));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        debug!("Sparse search returned {} results", results.len());
        Ok(results)
    }

    /// E6 sparse recall for exact keyword matching.
    ///
    /// Returns candidates that share terms with the query, sorted by term overlap count.
    /// This finds exact keyword matches that E1 semantic search might miss.
    pub async fn search_e6_sparse_impl(
        &self,
        sparse_query: &SparseVector,
        max_candidates: usize,
    ) -> CoreResult<Vec<(Uuid, usize)>> {
        debug!(
            "E6 sparse recall with max_candidates={}, query nnz={}",
            max_candidates,
            sparse_query.nnz()
        );

        let mut results: Vec<(Uuid, usize)> = Vec::new();
        let deleted_ids: HashSet<Uuid> = self.deleted.iter().map(|r| *r.key()).collect();

        // Build set of query term indices for fast lookup
        let query_terms: HashSet<u16> = sparse_query.indices.iter().copied().collect();

        for entry in self.data.iter() {
            let id = *entry.key();
            let fp = entry.value();

            if deleted_ids.contains(&id) {
                continue;
            }

            // Count overlapping terms between query and document E6 sparse vectors
            let doc_e6 = &fp.semantic.e6_sparse;
            let overlap_count: usize = doc_e6
                .indices
                .iter()
                .filter(|idx| query_terms.contains(idx))
                .count();

            if overlap_count > 0 {
                results.push((id, overlap_count));
            }
        }

        // Sort by overlap count descending
        results.sort_by(|a, b| b.1.cmp(&a.1));
        results.truncate(max_candidates);
        debug!("E6 sparse recall returned {} candidates", results.len());
        Ok(results)
    }
}
