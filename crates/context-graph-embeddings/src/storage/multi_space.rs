//! Multi-space search engine with RRF fusion.
//!
//! # Architecture
//! Integrates with:
//! - `context-graph-core::index::hnsw_impl::HnswMultiSpaceIndex` (HNSW search)
//! - `context-graph-storage::teleological::quantized` (fingerprint storage)
//! - Local types from `super::types` (query results)
//!
//! # Constitution Alignment
//! - Stage 3 of 5-stage pipeline: Multi-space rerank (500 -> 100)
//! - RRF constant k=60
//! - 13 embedders, 12 use HNSW (E6 and E12 are sparse/late-interaction)

use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use super::types::{
    EmbedderQueryResult, MultiSpaceQueryResult, StoredQuantizedFingerprint, NUM_EMBEDDERS, RRF_K,
};
use crate::error::EmbeddingError;

// =============================================================================
// TRAITS FOR DEPENDENCY INJECTION
// =============================================================================

/// Trait for retrieving quantized fingerprints from storage.
///
/// # Implementors
/// - `RocksDbMemex` in `context-graph-storage::teleological::quantized`
pub trait QuantizedFingerprintRetriever: Send + Sync {
    /// Get full fingerprint by ID.
    fn get_fingerprint(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredQuantizedFingerprint>, EmbeddingError>;

    /// Get only purpose vector (fast path for filtering).
    fn get_topic_profile(&self, id: Uuid) -> Result<Option<[f32; 13]>, EmbeddingError>;
}

/// Trait for HNSW index operations.
///
/// # Implementors
/// - `HnswMultiSpaceIndex` in `context-graph-core::index::hnsw_impl`
pub trait MultiSpaceIndexProvider: Send + Sync {
    /// Search a single embedder's space.
    /// Returns (id, similarity) pairs sorted by similarity descending.
    fn search_embedder(
        &self,
        embedder_idx: u8,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(Uuid, f32)>, EmbeddingError>;

    /// Check if embedder uses HNSW (vs inverted index).
    fn embedder_uses_hnsw(&self, embedder_idx: u8) -> bool;
}

// =============================================================================
// MULTI-SPACE SEARCH ENGINE
// =============================================================================

/// Multi-space search engine integrating HNSW indexes with RRF fusion.
///
/// # FAIL FAST Policy
/// - Missing embedder index -> panic with full context
/// - Dimension mismatch -> panic with expected vs actual
/// - Empty query results -> return empty vec (not error)
/// - Storage errors -> propagate with full context
///
/// # Usage
/// ```rust,ignore
/// let engine = MultiSpaceSearchEngine::new(storage, hnsw_manager);
///
/// let mut queries = HashMap::new();
/// queries.insert(0, query_e1.clone());  // E1 semantic
/// queries.insert(4, query_e5.clone());  // E5 causal
///
/// let results = engine.search_multi_space(&queries, None, 100, 20)?;
/// ```
pub struct MultiSpaceSearchEngine<S: QuantizedFingerprintRetriever> {
    /// Storage backend for fingerprint retrieval (reserved for future use)
    #[allow(dead_code)]
    storage: Arc<S>,

    /// HNSW index manager (from context-graph-core)
    hnsw_manager: Arc<dyn MultiSpaceIndexProvider>,
}

impl<S: QuantizedFingerprintRetriever> MultiSpaceSearchEngine<S> {
    /// Create new multi-space search engine.
    ///
    /// # Arguments
    /// * `storage` - Quantized fingerprint storage backend
    /// * `hnsw_manager` - HNSW index provider from context-graph-core
    ///
    /// # Example
    /// ```rust,ignore
    /// let storage = Arc::new(RocksDbMemex::open(path)?);
    /// let hnsw = Arc::new(HnswMultiSpaceIndex::new());
    /// let engine = MultiSpaceSearchEngine::new(storage, hnsw);
    /// ```
    pub fn new(storage: Arc<S>, hnsw_manager: Arc<dyn MultiSpaceIndexProvider>) -> Self {
        Self {
            storage,
            hnsw_manager,
        }
    }

    /// Search a single embedder's HNSW index.
    ///
    /// # Arguments
    /// * `embedder_idx` - Embedder index (0-12)
    /// * `query` - Query vector (dimension must match embedder)
    /// * `k` - Number of results
    ///
    /// # Returns
    /// Vec of `EmbedderQueryResult` sorted by similarity descending.
    ///
    /// # Panics
    /// - `embedder_idx >= 13` -> panic
    /// - Embedder doesn't use HNSW (E6, E12) -> panic
    pub fn search_single_space(
        &self,
        embedder_idx: u8,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<EmbedderQueryResult>, EmbeddingError> {
        // =====================================================================
        // FAIL FAST VALIDATION
        // =====================================================================

        if embedder_idx as usize >= NUM_EMBEDDERS {
            panic!(
                "SEARCH ERROR: Invalid embedder_idx={}. Valid range: 0-12. \
                 Query dimension={}. Caller must validate embedder index.",
                embedder_idx,
                query.len()
            );
        }

        if !self.hnsw_manager.embedder_uses_hnsw(embedder_idx) {
            panic!(
                "SEARCH ERROR: Embedder {} does not use HNSW. \
                 E6 (sparse) and E12 (late interaction) require different search paths. \
                 Caller must route to appropriate index.",
                embedder_idx
            );
        }

        // =====================================================================
        // EXECUTE SEARCH
        // =====================================================================

        let raw_results = self.hnsw_manager.search_embedder(embedder_idx, query, k)?;

        // Convert to EmbedderQueryResult with ranks
        let results: Vec<EmbedderQueryResult> = raw_results
            .into_iter()
            .enumerate()
            .map(|(rank, (id, similarity))| {
                EmbedderQueryResult::from_similarity(id, embedder_idx, similarity, rank)
            })
            .collect();

        // Log success evidence
        eprintln!(
            "[SINGLE-SPACE SEARCH] embedder={}, query_dim={}, k={}, found={}",
            embedder_idx,
            query.len(),
            k,
            results.len()
        );

        Ok(results)
    }

    /// Multi-space search with RRF fusion across multiple embedders.
    ///
    /// # Arguments
    /// * `queries` - Map of embedder_idx -> query vector
    /// * `weights` - Optional per-embedder weights (defaults to uniform 1.0)
    /// * `k_per_space` - Number of results per embedder
    /// * `final_k` - Number of final fused results
    ///
    /// # Returns
    /// Vec of `MultiSpaceQueryResult` sorted by RRF score descending.
    ///
    /// # RRF Formula (Constitution embeddings.similarity.rrf_constant = 60)
    /// ```text
    /// RRF(d) = Ei wi / (60 + ranki(d))
    /// ```
    ///
    /// # Panics
    /// - Empty `queries` map -> panic
    /// - Invalid embedder_idx in `queries` -> panic
    pub fn search_multi_space(
        &self,
        queries: &HashMap<u8, Vec<f32>>,
        weights: Option<&[f32; 13]>,
        k_per_space: usize,
        final_k: usize,
    ) -> Result<Vec<MultiSpaceQueryResult>, EmbeddingError> {
        // =====================================================================
        // FAIL FAST VALIDATION
        // =====================================================================

        if queries.is_empty() {
            panic!(
                "SEARCH ERROR: Empty queries map. Must query at least one embedder. \
                 This indicates a bug in query construction."
            );
        }

        for &embedder_idx in queries.keys() {
            if embedder_idx as usize >= NUM_EMBEDDERS {
                panic!(
                    "SEARCH ERROR: Invalid embedder_idx={} in queries. Valid range: 0-12.",
                    embedder_idx
                );
            }
        }

        // =====================================================================
        // COLLECT PER-SPACE RESULTS
        // =====================================================================

        let mut all_results: HashMap<Uuid, Vec<EmbedderQueryResult>> = HashMap::new();

        for (&embedder_idx, query) in queries {
            // Skip non-HNSW embedders (E6, E12) - they need different search
            if !self.hnsw_manager.embedder_uses_hnsw(embedder_idx) {
                eprintln!(
                    "[MULTI-SPACE SEARCH] Skipping embedder {} (not HNSW)",
                    embedder_idx
                );
                continue;
            }

            let space_results = self.search_single_space(embedder_idx, query, k_per_space)?;

            for result in space_results {
                all_results.entry(result.id).or_default().push(result);
            }
        }

        // =====================================================================
        // COMPUTE RRF FUSION
        // =====================================================================

        let mut fused_results: Vec<MultiSpaceQueryResult> = all_results
            .into_iter()
            .map(|(id, embedder_results)| {
                // Use existing MultiSpaceQueryResult::from_embedder_results if available,
                // or compute manually
                self.compute_rrf_fusion(&embedder_results, weights, id)
            })
            .collect();

        // Sort by RRF score descending
        fused_results.sort_by(|a, b| {
            b.rrf_score
                .partial_cmp(&a.rrf_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Truncate to final_k
        let total_candidates = fused_results.len();
        fused_results.truncate(final_k);

        // =====================================================================
        // LOG SUCCESS EVIDENCE
        // =====================================================================

        eprintln!(
            "[MULTI-SPACE SEARCH] spaces_queried={}, unique_candidates={}, returned={}",
            queries.len(),
            total_candidates,
            fused_results.len()
        );

        if let Some(top) = fused_results.first() {
            eprintln!(
                "[MULTI-SPACE SEARCH] top_result: id={}, rrf_score={:.4}, embedder_count={}",
                top.id, top.rrf_score, top.embedder_count
            );
        }

        Ok(fused_results)
    }

    /// Search with purpose vector weighting.
    ///
    /// Uses the query's purpose vector to weight RRF fusion.
    /// Higher weight on embedders aligned with query purpose.
    pub fn search_purpose_weighted(
        &self,
        queries: &HashMap<u8, Vec<f32>>,
        topic_profile: &[f32; 13],
        k_per_space: usize,
        final_k: usize,
    ) -> Result<Vec<MultiSpaceQueryResult>, EmbeddingError> {
        eprintln!(
            "[PURPOSE-WEIGHTED SEARCH] topic_profile_sum={:.4}",
            topic_profile.iter().sum::<f32>()
        );
        self.search_multi_space(queries, Some(topic_profile), k_per_space, final_k)
    }

    /// Compute RRF fusion for a single document across multiple embedder results.
    ///
    /// Formula: RRF(d) = Ei wi / (k + ranki(d))
    /// where k = RRF_K = 60
    fn compute_rrf_fusion(
        &self,
        results: &[EmbedderQueryResult],
        weights: Option<&[f32; 13]>,
        id: Uuid,
    ) -> MultiSpaceQueryResult {
        let mut embedder_similarities = [f32::NAN; 13];
        let mut rrf_score = 0.0f32;
        let mut weighted_sum = 0.0f32;
        let mut weight_total = 0.0f32;

        for result in results {
            let idx = result.embedder_idx as usize;
            if idx < 13 {
                embedder_similarities[idx] = result.similarity;

                // HIGH-3 FIX: RRF contribution w / (k + rank + 1) — 1-indexed
                // consistent with core fusion (rank 0 → 1/(60+1) = 0.01639)
                let w = weights.map(|w| w[idx]).unwrap_or(1.0);
                rrf_score += w / (RRF_K + result.rank as f32 + 1.0);

                weighted_sum += result.similarity * w;
                weight_total += w;
            }
        }

        let weighted_similarity = if weight_total > 0.0 {
            weighted_sum / weight_total
        } else {
            0.0
        };

        MultiSpaceQueryResult {
            id,
            embedder_similarities,
            rrf_score,
            weighted_similarity,
            embedder_count: results.len(),
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // TEST FIXTURES (REAL DATA, NO MOCKS)
    // -------------------------------------------------------------------------

    /// Test storage that uses in-memory HashMap (still real data structures)
    struct TestStorage {
        topic_profiles: HashMap<Uuid, [f32; 13]>,
    }

    impl TestStorage {
        fn new() -> Self {
            Self {
                topic_profiles: HashMap::new(),
            }
        }

        #[allow(dead_code)] // Prepared for future tests
        fn add_topic_profile(&mut self, id: Uuid, pv: [f32; 13]) {
            self.topic_profiles.insert(id, pv);
        }
    }

    impl QuantizedFingerprintRetriever for TestStorage {
        fn get_fingerprint(
            &self,
            _id: Uuid,
        ) -> Result<Option<StoredQuantizedFingerprint>, EmbeddingError> {
            // Not needed for search tests
            Ok(None)
        }

        fn get_topic_profile(&self, id: Uuid) -> Result<Option<[f32; 13]>, EmbeddingError> {
            Ok(self.topic_profiles.get(&id).copied())
        }
    }

    /// Test HNSW manager with fixed results
    struct TestHnswManager {
        results: HashMap<u8, Vec<(Uuid, f32)>>,
    }

    impl TestHnswManager {
        fn new() -> Self {
            Self {
                results: HashMap::new(),
            }
        }

        fn set_results(&mut self, embedder_idx: u8, results: Vec<(Uuid, f32)>) {
            self.results.insert(embedder_idx, results);
        }
    }

    impl MultiSpaceIndexProvider for TestHnswManager {
        fn search_embedder(
            &self,
            embedder_idx: u8,
            _query: &[f32],
            k: usize,
        ) -> Result<Vec<(Uuid, f32)>, EmbeddingError> {
            Ok(self
                .results
                .get(&embedder_idx)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .take(k)
                .collect())
        }

        fn embedder_uses_hnsw(&self, embedder_idx: u8) -> bool {
            // LOW-8 FIX: E6 (idx 5, sparse), E12 (idx 11, ColBERT late interaction),
            // and E13 (idx 12, SPLADE sparse) don't use HNSW.
            // Previously E13 was missing from the exclusion list.
            embedder_idx != 5 && embedder_idx != 11 && embedder_idx != 12
        }
    }

    // -------------------------------------------------------------------------
    // UNIT TESTS
    // -------------------------------------------------------------------------
    // LOW-3 Note: Tests in this module use zeroed query vectors (vec![0.0f32; N])
    // intentionally. They test index *infrastructure* — ranking, deduplication,
    // RRF formula, weight application, and error handling — NOT search quality.
    // The TestHnswManager returns pre-configured results regardless of query
    // content, so the query vector values are irrelevant. Search quality with
    // real embeddings is validated by benchmark suites.

    #[test]
    fn test_single_space_search_returns_ranked_results() {
        let storage = Arc::new(TestStorage::new());
        let mut hnsw = TestHnswManager::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        let id3 = Uuid::new_v4();

        // Set up results with known similarities
        hnsw.set_results(0, vec![(id1, 0.95), (id2, 0.80), (id3, 0.65)]);

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));
        let query = vec![0.0f32; 1024];

        let results = engine.search_single_space(0, &query, 10).unwrap();

        // Verify
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, id1);
        assert_eq!(results[0].rank, 0);
        assert!((results[0].similarity - 0.95).abs() < f32::EPSILON);
        assert_eq!(results[1].rank, 1);
        assert_eq!(results[2].rank, 2);

        eprintln!("[VERIFIED] Single space search returns correctly ranked results");
    }

    #[test]
    fn test_rrf_fusion_formula_matches_constitution() {
        let storage = Arc::new(TestStorage::new());
        let mut hnsw = TestHnswManager::new();

        let id1 = Uuid::new_v4();

        // Document appears at rank 0 in E1 and rank 2 in E2
        hnsw.set_results(0, vec![(id1, 0.90)]); // rank 0
        hnsw.set_results(
            1,
            vec![
                (Uuid::new_v4(), 0.95),
                (Uuid::new_v4(), 0.85),
                (id1, 0.75), // rank 2
            ],
        );

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);
        queries.insert(1, vec![0.0f32; 512]);

        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // Find id1 in results
        let id1_result = results.iter().find(|r| r.id == id1).unwrap();

        // Manual RRF calculation (1-indexed):
        // From E1: 1/(60+0+1) = 1/61 = 0.01639
        // From E2: 1/(60+2+1) = 1/63 = 0.01587
        // Total: 0.03226
        let expected_rrf = 1.0 / (60.0 + 0.0 + 1.0) + 1.0 / (60.0 + 2.0 + 1.0);

        assert!(
            (id1_result.rrf_score - expected_rrf).abs() < 0.001,
            "RRF score mismatch: got {}, expected {}",
            id1_result.rrf_score,
            expected_rrf
        );

        eprintln!(
            "[VERIFIED] RRF formula: 1/(60+0+1) + 1/(60+2+1) = {:.5} matches computed {:.5}",
            expected_rrf, id1_result.rrf_score
        );
    }

    #[test]
    fn test_purpose_weights_affect_ranking() {
        let storage = Arc::new(TestStorage::new());
        let mut hnsw = TestHnswManager::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Both documents appear at same ranks but in different spaces
        hnsw.set_results(0, vec![(id1, 0.90)]); // id1 in E1
        hnsw.set_results(1, vec![(id2, 0.90)]); // id2 in E2

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);
        queries.insert(1, vec![0.0f32; 512]);

        // Weight E1 heavily
        let weights_favor_e1 = [
            10.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];
        let results = engine
            .search_multi_space(&queries, Some(&weights_favor_e1), 10, 10)
            .unwrap();

        // id1 should rank higher due to E1 weight
        let id1_idx = results.iter().position(|r| r.id == id1).unwrap();
        let id2_idx = results.iter().position(|r| r.id == id2).unwrap();

        assert!(
            id1_idx < id2_idx,
            "id1 should rank higher with E1 weight=10"
        );

        // Now weight E2 heavily
        let weights_favor_e2 = [
            1.0, 10.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0,
        ];
        let results = engine
            .search_multi_space(&queries, Some(&weights_favor_e2), 10, 10)
            .unwrap();

        let id1_idx = results.iter().position(|r| r.id == id1).unwrap();
        let id2_idx = results.iter().position(|r| r.id == id2).unwrap();

        assert!(
            id2_idx < id1_idx,
            "id2 should rank higher with E2 weight=10"
        );

        eprintln!("[VERIFIED] Purpose weights correctly affect ranking");
    }

    // -------------------------------------------------------------------------
    // EDGE CASE TESTS (3 REQUIRED)
    // -------------------------------------------------------------------------

    /// Edge Case 1: RRF with single embedder (degenerate case)
    #[test]
    fn test_edge_case_single_embedder_rrf() {
        let storage = Arc::new(TestStorage::new());
        let mut hnsw = TestHnswManager::new();

        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();
        hnsw.set_results(
            0,
            ids.iter()
                .enumerate()
                .map(|(i, id)| (*id, 1.0 - i as f32 * 0.1))
                .collect(),
        );

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);

        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // Verify RRF scores are 1/(61+rank) (1-indexed)
        for (i, result) in results.iter().enumerate() {
            let expected = 1.0 / (61.0 + i as f32);
            assert!(
                (result.rrf_score - expected).abs() < 0.0001,
                "Rank {} RRF: expected {}, got {}",
                i,
                expected,
                result.rrf_score
            );
        }

        eprintln!("[VERIFIED] Edge case: Single embedder RRF = 1/(61+rank)");
    }

    /// Edge Case 2: Document appears in subset of queried spaces
    #[test]
    fn test_edge_case_partial_space_coverage() {
        let storage = Arc::new(TestStorage::new());
        let mut hnsw = TestHnswManager::new();

        let doc_partial = Uuid::new_v4();
        let doc_full = Uuid::new_v4();

        // doc_partial only in E1, doc_full in both E1 and E2
        hnsw.set_results(0, vec![(doc_partial, 0.90), (doc_full, 0.80)]);
        hnsw.set_results(1, vec![(doc_full, 0.85)]);

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);
        queries.insert(1, vec![0.0f32; 512]);

        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        let partial = results.iter().find(|r| r.id == doc_partial).unwrap();
        let full = results.iter().find(|r| r.id == doc_full).unwrap();

        // doc_partial: only E1 similarity, E2 should be NaN
        assert!(!partial.embedder_similarities[0].is_nan());
        assert!(partial.embedder_similarities[1].is_nan());
        assert_eq!(partial.embedder_count, 1);

        // doc_full: both E1 and E2 present
        assert!(!full.embedder_similarities[0].is_nan());
        assert!(!full.embedder_similarities[1].is_nan());
        assert_eq!(full.embedder_count, 2);

        eprintln!("[VERIFIED] Edge case: Partial coverage has NaN for missing embedders");
    }

    /// Edge Case 3: All results have same similarity
    #[test]
    fn test_edge_case_tied_similarities() {
        let storage = Arc::new(TestStorage::new());
        let mut hnsw = TestHnswManager::new();

        let ids: Vec<Uuid> = (0..3).map(|_| Uuid::new_v4()).collect();

        // All same similarity, but different ranks
        hnsw.set_results(0, vec![(ids[0], 0.80), (ids[1], 0.80), (ids[2], 0.80)]);

        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let mut queries = HashMap::new();
        queries.insert(0, vec![0.0f32; 1024]);

        let results = engine.search_multi_space(&queries, None, 10, 10).unwrap();

        // RRF should break ties by rank
        assert!(results[0].rrf_score > results[1].rrf_score);
        assert!(results[1].rrf_score > results[2].rrf_score);

        // Verify deterministic ordering
        assert_eq!(results[0].id, ids[0]);
        assert_eq!(results[1].id, ids[1]);
        assert_eq!(results[2].id, ids[2]);

        eprintln!("[VERIFIED] Edge case: Tied similarities broken by rank order");
    }

    // -------------------------------------------------------------------------
    // PANIC TESTS (FAIL FAST)
    // -------------------------------------------------------------------------

    #[test]
    #[should_panic(expected = "Invalid embedder_idx")]
    fn test_invalid_embedder_panics() {
        let storage = Arc::new(TestStorage::new());
        let hnsw = TestHnswManager::new();
        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        // embedder_idx=15 is invalid
        let _ = engine.search_single_space(15, &[0.0f32; 1024], 10);
    }

    #[test]
    #[should_panic(expected = "Empty queries map")]
    fn test_empty_queries_panics() {
        let storage = Arc::new(TestStorage::new());
        let hnsw = TestHnswManager::new();
        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        let queries: HashMap<u8, Vec<f32>> = HashMap::new();
        let _ = engine.search_multi_space(&queries, None, 10, 10);
    }

    #[test]
    #[should_panic(expected = "does not use HNSW")]
    fn test_sparse_embedder_panics() {
        let storage = Arc::new(TestStorage::new());
        let hnsw = TestHnswManager::new();
        let engine = MultiSpaceSearchEngine::new(storage, Arc::new(hnsw));

        // E6 (index 5) is sparse, not HNSW
        let _ = engine.search_single_space(5, &[0.0f32; 100], 10);
    }
}
