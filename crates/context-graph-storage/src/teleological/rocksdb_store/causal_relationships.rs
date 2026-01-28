//! Causal relationship storage operations for RocksDbTeleologicalStore.
//!
//! This module provides methods for storing and retrieving LLM-generated
//! causal relationship descriptions with full provenance.
//!
//! # Column Families
//!
//! - `CF_CAUSAL_RELATIONSHIPS`: Primary storage by UUID
//! - `CF_CAUSAL_BY_SOURCE`: Secondary index by source fingerprint ID

use tracing::{debug, error, info, warn};
use uuid::Uuid;

use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::types::CausalRelationship;

use crate::teleological::column_families::{CF_CAUSAL_BY_SOURCE, CF_CAUSAL_RELATIONSHIPS};
use crate::teleological::schema::{causal_by_source_key, causal_relationship_key};

use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

/// Maximum size for a serialized causal relationship: 100KB
/// (description ~2KB + embedding 4KB + source content ~10KB + overhead)
const MAX_CAUSAL_RELATIONSHIP_SIZE: usize = 102_400;

impl RocksDbTeleologicalStore {
    // ========================================================================
    // Column Family Accessors (FAIL FAST)
    // ========================================================================

    /// Get the causal_relationships column family handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_causal_relationships(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_CAUSAL_RELATIONSHIPS)
            .expect("CF_CAUSAL_RELATIONSHIPS must exist - database initialization failed")
    }

    /// Get the causal_by_source column family handle (FAIL FAST on missing).
    #[inline]
    pub(crate) fn cf_causal_by_source(&self) -> &rocksdb::ColumnFamily {
        self.db
            .cf_handle(CF_CAUSAL_BY_SOURCE)
            .expect("CF_CAUSAL_BY_SOURCE must exist - database initialization failed")
    }

    // ========================================================================
    // Store Operations
    // ========================================================================

    /// Store a causal relationship.
    ///
    /// Stores the relationship in CF_CAUSAL_RELATIONSHIPS and updates
    /// the secondary index in CF_CAUSAL_BY_SOURCE.
    ///
    /// # Arguments
    /// * `relationship` - The causal relationship to store
    ///
    /// # Returns
    /// The UUID of the stored relationship
    ///
    /// # Errors
    /// Returns error if serialization or RocksDB operations fail.
    pub async fn store_causal_relationship(
        &self,
        relationship: &CausalRelationship,
    ) -> CoreResult<Uuid> {
        // 1. Serialize the relationship
        let serialized = bincode::serialize(relationship).map_err(|e| {
            error!(
                "CAUSAL ERROR: Failed to serialize CausalRelationship {}: {}",
                relationship.id, e
            );
            CoreError::Internal(format!(
                "Failed to serialize causal relationship {}: {}",
                relationship.id, e
            ))
        })?;

        // 2. Validate size
        if serialized.len() > MAX_CAUSAL_RELATIONSHIP_SIZE {
            error!(
                "CAUSAL ERROR: Serialized size {} bytes exceeds max {} bytes for relationship {}",
                serialized.len(),
                MAX_CAUSAL_RELATIONSHIP_SIZE,
                relationship.id
            );
            return Err(CoreError::Internal(format!(
                "Causal relationship {} exceeds max size: {} > {} bytes",
                relationship.id,
                serialized.len(),
                MAX_CAUSAL_RELATIONSHIP_SIZE
            )));
        }

        // 3. Store in primary CF
        let cf = self.cf_causal_relationships();
        let key = causal_relationship_key(&relationship.id);

        self.db.put_cf(cf, key, &serialized).map_err(|e| {
            error!(
                "ROCKSDB ERROR: Failed to store causal relationship {}: {}",
                relationship.id, e
            );
            TeleologicalStoreError::rocksdb_op(
                "put",
                CF_CAUSAL_RELATIONSHIPS,
                Some(relationship.id),
                e,
            )
        })?;

        // 4. Update secondary index (source_fingerprint_id -> Vec<causal_id>)
        self.update_causal_by_source_index(relationship.source_fingerprint_id, relationship.id)
            .await?;

        // 5. Add to E11 HNSW index if embedding present
        if relationship.has_entity_embedding() {
            if let Err(e) = self
                .causal_e11_index
                .insert(relationship.id, relationship.e11_embedding())
            {
                error!(
                    "Failed to add causal relationship {} to E11 HNSW index: {}",
                    relationship.id, e
                );
                // Non-fatal - relationship is stored, just not indexed
            } else {
                debug!(
                    causal_id = %relationship.id,
                    "Added causal relationship to E11 HNSW index"
                );
            }
        }

        info!(
            causal_id = %relationship.id,
            source_id = %relationship.source_fingerprint_id,
            mechanism_type = %relationship.mechanism_type,
            explanation_len = relationship.explanation.len(),
            "Stored causal relationship"
        );

        Ok(relationship.id)
    }

    /// Update the causal_by_source secondary index.
    ///
    /// Adds the causal_id to the list of relationships for this source.
    async fn update_causal_by_source_index(
        &self,
        source_id: Uuid,
        causal_id: Uuid,
    ) -> CoreResult<()> {
        let cf = self.cf_causal_by_source();
        let key = causal_by_source_key(&source_id);

        // Read existing list (if any)
        let mut causal_ids: Vec<Uuid> = match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).unwrap_or_else(|e| {
                warn!(
                    "CAUSAL WARNING: Failed to deserialize causal_by_source for {}: {}. Starting fresh.",
                    source_id, e
                );
                Vec::new()
            }),
            Ok(None) => Vec::new(),
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to read causal_by_source for {}: {}",
                    source_id, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to read causal_by_source for {}: {}",
                    source_id, e
                )));
            }
        };

        // Add causal_id if not already present
        if !causal_ids.contains(&causal_id) {
            causal_ids.push(causal_id);

            let serialized = bincode::serialize(&causal_ids).map_err(|e| {
                CoreError::Internal(format!("Failed to serialize causal_by_source list: {}", e))
            })?;

            self.db.put_cf(cf, key, &serialized).map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to update causal_by_source for {}: {}",
                    source_id, e
                );
                TeleologicalStoreError::rocksdb_op("put", CF_CAUSAL_BY_SOURCE, Some(source_id), e)
            })?;

            debug!(
                source_id = %source_id,
                causal_id = %causal_id,
                total_count = causal_ids.len(),
                "Updated causal_by_source index"
            );
        }

        Ok(())
    }

    // ========================================================================
    // Retrieve Operations
    // ========================================================================

    /// Retrieve a causal relationship by ID.
    ///
    /// # Arguments
    /// * `id` - The causal relationship UUID
    ///
    /// # Returns
    /// The causal relationship if found, None otherwise.
    pub async fn get_causal_relationship(
        &self,
        id: Uuid,
    ) -> CoreResult<Option<CausalRelationship>> {
        let cf = self.cf_causal_relationships();
        let key = causal_relationship_key(&id);

        match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => {
                let relationship: CausalRelationship = bincode::deserialize(&bytes).map_err(|e| {
                    error!(
                        "CAUSAL ERROR: Failed to deserialize CausalRelationship {}: {}. Data corruption.",
                        id, e
                    );
                    CoreError::Internal(format!(
                        "Failed to deserialize causal relationship {}: {}",
                        id, e
                    ))
                })?;
                Ok(Some(relationship))
            }
            Ok(None) => {
                debug!("No causal relationship found for ID {}", id);
                Ok(None)
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to read causal relationship {}: {}",
                    id, e
                );
                Err(CoreError::StorageError(format!(
                    "Failed to read causal relationship {}: {}",
                    id, e
                )))
            }
        }
    }

    /// Retrieve all causal relationships for a source fingerprint.
    ///
    /// Uses the secondary index for efficient lookup.
    ///
    /// # Arguments
    /// * `source_id` - The source fingerprint UUID
    ///
    /// # Returns
    /// Vector of causal relationships derived from this source.
    pub async fn get_causal_relationships_by_source(
        &self,
        source_id: Uuid,
    ) -> CoreResult<Vec<CausalRelationship>> {
        let cf = self.cf_causal_by_source();
        let key = causal_by_source_key(&source_id);

        // Get list of causal_ids from index
        let causal_ids: Vec<Uuid> = match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).map_err(|e| {
                error!(
                    "CAUSAL ERROR: Failed to deserialize causal_by_source for {}: {}",
                    source_id, e
                );
                CoreError::Internal(format!(
                    "Failed to deserialize causal_by_source for {}: {}",
                    source_id, e
                ))
            })?,
            Ok(None) => {
                debug!("No causal relationships found for source {}", source_id);
                return Ok(Vec::new());
            }
            Err(e) => {
                error!(
                    "ROCKSDB ERROR: Failed to read causal_by_source for {}: {}",
                    source_id, e
                );
                return Err(CoreError::StorageError(format!(
                    "Failed to read causal_by_source for {}: {}",
                    source_id, e
                )));
            }
        };

        // Fetch each causal relationship
        let mut relationships = Vec::with_capacity(causal_ids.len());
        for causal_id in causal_ids {
            match self.get_causal_relationship(causal_id).await {
                Ok(Some(rel)) => relationships.push(rel),
                Ok(None) => {
                    warn!(
                        "CAUSAL WARNING: Index references non-existent relationship {} for source {}",
                        causal_id, source_id
                    );
                }
                Err(e) => {
                    error!(
                        "Failed to fetch causal relationship {} for source {}: {}",
                        causal_id, source_id, e
                    );
                    // Continue to fetch other relationships
                }
            }
        }

        debug!(
            source_id = %source_id,
            count = relationships.len(),
            "Retrieved causal relationships by source"
        );

        Ok(relationships)
    }

    // ========================================================================
    // Search Operations
    // ========================================================================

    /// Search causal relationships by description embedding similarity.
    ///
    /// Performs a brute-force scan (suitable for <10K relationships).
    /// For larger collections, consider adding HNSW index for descriptions.
    ///
    /// # Arguments
    /// * `query_embedding` - E1 1024D query embedding
    /// * `top_k` - Number of results to return
    /// * `direction_filter` - Optional filter: "cause", "effect", or None for all
    ///
    /// # Returns
    /// Vector of (causal_id, similarity_score) tuples, sorted by similarity descending.
    pub async fn search_causal_relationships(
        &self,
        query_embedding: &[f32],
        top_k: usize,
        direction_filter: Option<&str>,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        use crate::teleological::rocksdb_store::helpers::compute_cosine_similarity;

        let cf = self.cf_causal_relationships();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (_key, value) = item.map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to iterate causal_relationships: {}",
                    e
                );
                CoreError::StorageError(format!("Failed to iterate causal_relationships: {}", e))
            })?;

            let relationship: CausalRelationship = match bincode::deserialize(&value) {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "CAUSAL WARNING: Failed to deserialize relationship during search: {}",
                        e
                    );
                    continue;
                }
            };

            // Apply mechanism type filter if specified
            if let Some(filter) = direction_filter {
                if filter != "all" && relationship.normalized_mechanism_type() != filter {
                    continue;
                }
            }

            // Compute similarity using E1 semantic embedding
            let similarity =
                compute_cosine_similarity(query_embedding, &relationship.e1_semantic);

            results.push((relationship.id, similarity));
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
            "Searched causal relationships"
        );

        Ok(results)
    }

    /// Search causal relationships using E5 asymmetric embeddings.
    ///
    /// E5 dual embeddings enable directional causal search:
    /// - "What caused X?" → `search_causes=true`: query is effect, search cause vectors
    /// - "What are effects of X?" → `search_causes=false`: query is cause, search effect vectors
    ///
    /// # Arguments
    /// * `query_embedding` - E5 768D query embedding
    /// * `search_causes` - If true, search e5_as_cause vectors; if false, search e5_as_effect vectors
    /// * `top_k` - Number of results
    ///
    /// # Returns
    /// Vector of (causal_id, similarity) tuples sorted by similarity descending.
    pub async fn search_causal_e5(
        &self,
        query_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        use crate::teleological::rocksdb_store::helpers::compute_cosine_similarity;

        let cf = self.cf_causal_relationships();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (_key, value) = item.map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to iterate causal_relationships: {}",
                    e
                );
                CoreError::StorageError(format!("Failed to iterate causal_relationships: {}", e))
            })?;

            let relationship: CausalRelationship = match bincode::deserialize(&value) {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "CAUSAL WARNING: Failed to deserialize relationship during E5 search: {}",
                        e
                    );
                    continue;
                }
            };

            // Select appropriate E5 vector based on search mode
            let doc_embedding = if search_causes {
                // Searching for causes: compare query (effect) against stored cause vectors
                &relationship.e5_as_cause
            } else {
                // Searching for effects: compare query (cause) against stored effect vectors
                &relationship.e5_as_effect
            };

            // Skip if E5 embeddings are empty (legacy data with placeholder zeros)
            if doc_embedding.iter().all(|&v| v == 0.0) {
                continue;
            }

            let similarity = compute_cosine_similarity(query_embedding, doc_embedding);
            results.push((relationship.id, similarity));
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
            "Searched causal relationships using E5"
        );

        Ok(results)
    }

    /// Search causal relationships using hybrid source + explanation scoring.
    ///
    /// Combines source-anchored embeddings with explanation embeddings to prevent
    /// LLM-generated explanations from clustering together. Source content is unique
    /// per document, providing diversity; explanation provides mechanism detail.
    ///
    /// # Hybrid Scoring
    /// `score = source_weight * source_similarity + explanation_weight * explanation_similarity`
    ///
    /// Default weights: source=0.6, explanation=0.4
    ///
    /// # Arguments
    /// * `query_embedding` - E5 768D query embedding
    /// * `search_causes` - If true, search cause vectors; if false, search effect vectors
    /// * `top_k` - Number of results
    /// * `source_weight` - Weight for source-anchored similarity (default 0.6)
    /// * `explanation_weight` - Weight for explanation similarity (default 0.4)
    ///
    /// # Returns
    /// Vector of (causal_id, hybrid_score) tuples sorted by score descending.
    pub async fn search_causal_e5_hybrid(
        &self,
        query_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
        source_weight: f32,
        explanation_weight: f32,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        use crate::teleological::rocksdb_store::helpers::compute_cosine_similarity;

        let cf = self.cf_causal_relationships();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (_key, value) = item.map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to iterate causal_relationships: {}",
                    e
                );
                CoreError::StorageError(format!("Failed to iterate causal_relationships: {}", e))
            })?;

            let relationship: CausalRelationship = match bincode::deserialize(&value) {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "CAUSAL WARNING: Failed to deserialize relationship during hybrid search: {}",
                        e
                    );
                    continue;
                }
            };

            // Select appropriate E5 vectors based on search mode
            let (explanation_embedding, source_embedding) = if search_causes {
                // Searching for causes: compare query (effect) against stored cause vectors
                (&relationship.e5_as_cause, &relationship.e5_source_cause)
            } else {
                // Searching for effects: compare query (cause) against stored effect vectors
                (&relationship.e5_as_effect, &relationship.e5_source_effect)
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
            results.push((relationship.id, hybrid_score));
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
            "Searched causal relationships using E5 hybrid scoring"
        );

        Ok(results)
    }

    /// Search causal relationships using E8 graph structure embeddings.
    ///
    /// E8 embeddings capture the graph structure of causal relationships.
    /// This enables finding relationships with similar connectivity patterns.
    ///
    /// # Arguments
    /// * `query_embedding` - E8 1024D query embedding
    /// * `search_sources` - If true, query as target, search source vectors
    ///                     If false, query as source, search target vectors
    /// * `top_k` - Number of results
    ///
    /// # Returns
    /// Vector of (causal_id, similarity) tuples sorted by similarity descending.
    pub async fn search_causal_e8(
        &self,
        query_embedding: &[f32],
        search_sources: bool,
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        use crate::teleological::rocksdb_store::helpers::compute_cosine_similarity;

        let cf = self.cf_causal_relationships();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (_key, value) = item.map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to iterate causal_relationships: {}",
                    e
                );
                CoreError::StorageError(format!("Failed to iterate causal_relationships: {}", e))
            })?;

            let relationship: CausalRelationship = match bincode::deserialize(&value) {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "CAUSAL WARNING: Failed to deserialize relationship during E8 search: {}",
                        e
                    );
                    continue;
                }
            };

            // Skip if E8 embeddings are not set
            if !relationship.has_graph_embeddings() {
                continue;
            }

            // Select appropriate E8 vector based on search mode
            let doc_embedding = if search_sources {
                // Searching for sources: compare query (target) against stored source vectors
                relationship.e8_graph_source_embedding()
            } else {
                // Searching for targets: compare query (source) against stored target vectors
                relationship.e8_graph_target_embedding()
            };

            let similarity = compute_cosine_similarity(query_embedding, doc_embedding);
            results.push((relationship.id, similarity));
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
            "Searched causal relationships using E8 graph embeddings"
        );

        Ok(results)
    }

    /// Search causal relationships using E11 KEPLER entity embeddings.
    ///
    /// E11 (KEPLER) embeddings enable entity-based search using TransE knowledge
    /// graph operations. This finds relationships containing similar entities.
    /// KEPLER knows entity relationships that E1 misses (e.g., "Diesel" = Rust ORM).
    ///
    /// # Performance
    ///
    /// Uses HNSW index for O(log n) search instead of O(n) brute-force scan.
    /// Target performance: <5ms for 5000 relationships (vs ~42ms brute-force).
    ///
    /// # Arguments
    /// * `query_embedding` - E11 768D KEPLER query embedding
    /// * `top_k` - Number of results
    ///
    /// # Returns
    /// Vector of (causal_id, similarity) tuples sorted by similarity descending.
    ///
    /// # Note
    /// Only relationships with valid E11 embeddings are indexed. Older relationships
    /// stored without E11 embeddings are not searchable. Use `search_causal_relationships()`
    /// for E1 semantic search as a fallback.
    pub async fn search_causal_e11(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        // Use HNSW index for O(log n) search
        let results = self.causal_e11_index.search(query_embedding, top_k)?;

        debug!(
            query_dim = query_embedding.len(),
            top_k = top_k,
            results_count = results.len(),
            index_size = self.causal_e11_index.len(),
            "Searched causal relationships using E11 HNSW index"
        );

        Ok(results)
    }

    /// Search causal relationships using E11 KEPLER entity embeddings (brute-force).
    ///
    /// This is the legacy O(n) brute-force implementation kept for testing/comparison.
    /// Use `search_causal_e11` for production queries.
    ///
    /// # Arguments
    /// * `query_embedding` - E11 768D KEPLER query embedding
    /// * `top_k` - Number of results
    ///
    /// # Returns
    /// Vector of (causal_id, similarity) tuples sorted by similarity descending.
    #[allow(dead_code)]
    pub async fn search_causal_e11_brute_force(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> CoreResult<Vec<(Uuid, f32)>> {
        use crate::teleological::rocksdb_store::helpers::compute_cosine_similarity;

        let cf = self.cf_causal_relationships();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        let mut results: Vec<(Uuid, f32)> = Vec::new();

        for item in iter {
            let (_key, value) = item.map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to iterate causal_relationships: {}",
                    e
                );
                CoreError::StorageError(format!("Failed to iterate causal_relationships: {}", e))
            })?;

            let relationship: CausalRelationship = match bincode::deserialize(&value) {
                Ok(r) => r,
                Err(e) => {
                    warn!(
                        "CAUSAL WARNING: Failed to deserialize relationship during E11 search: {}",
                        e
                    );
                    continue;
                }
            };

            // Only search relationships that have E11 embeddings
            if !relationship.has_entity_embedding() {
                continue;
            }

            let doc_embedding = relationship.e11_embedding();

            // Validate dimension match (768D)
            if query_embedding.len() != doc_embedding.len() {
                warn!(
                    expected = query_embedding.len(),
                    actual = doc_embedding.len(),
                    relationship_id = %relationship.id,
                    "E11 dimension mismatch - skipping relationship"
                );
                continue;
            }

            let similarity = compute_cosine_similarity(query_embedding, doc_embedding);
            results.push((relationship.id, similarity));
        }

        // Sort by similarity descending
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        debug!(
            query_dim = query_embedding.len(),
            top_k = top_k,
            results_count = results.len(),
            "Searched causal relationships using E11 brute-force (legacy)"
        );

        Ok(results)
    }

    /// Search causal relationships using all 4 embedders for maximum accuracy.
    ///
    /// Implements the 6-stage multi-embedder pipeline:
    /// 1. E1 semantic search (foundation)
    /// 2. E5 causal search (asymmetric, directional)
    /// 3. E8 graph search (connectivity)
    /// 4. E11 entity search (knowledge graph)
    /// 5. Weighted RRF fusion (per ARCH-21)
    /// 6. Consensus scoring and direction confidence
    ///
    /// # Arguments
    /// * `e1_embedding` - E1 1024D semantic embedding
    /// * `e5_embedding` - E5 768D causal embedding (already directional)
    /// * `e8_embedding` - E8 1024D graph embedding
    /// * `e11_embedding` - E11 768D entity embedding
    /// * `search_causes` - If true, searching for causes (query is effect)
    /// * `top_k` - Number of final results
    /// * `config` - Multi-embedder configuration with weights
    ///
    /// # Returns
    /// Vector of CausalSearchResult with per-embedder scores and consensus metrics.
    pub async fn search_causal_multi_embedder(
        &self,
        e1_embedding: &[f32],
        e5_embedding: &[f32],
        e8_embedding: &[f32],
        e11_embedding: &[f32],
        search_causes: bool,
        top_k: usize,
        config: &context_graph_core::types::MultiEmbedderConfig,
    ) -> CoreResult<Vec<context_graph_core::types::CausalSearchResult>> {
        use crate::teleological::rocksdb_store::fusion::{weighted_rrf_fusion_with_scores, RRF_K};
        use context_graph_core::types::CausalSearchResult;

        // Over-fetch factor for RRF (3x to ensure enough candidates)
        let fetch_k = top_k * 3;

        // Stage 1-4: Search all embedders in parallel
        info!(
            top_k = top_k,
            fetch_k = fetch_k,
            search_causes = search_causes,
            "Starting multi-embedder causal search"
        );

        // Run all 4 searches (not using tokio::join! since we're on sync iterator)
        let e1_results = self
            .search_causal_relationships(e1_embedding, fetch_k, None)
            .await?;

        let e5_results = self
            .search_causal_e5_hybrid(
                e5_embedding,
                search_causes,
                fetch_k,
                0.6, // source_weight
                0.4, // explanation_weight
            )
            .await?;

        let e8_results = self
            .search_causal_e8(e8_embedding, search_causes, fetch_k)
            .await?;

        let e11_results = self.search_causal_e11(e11_embedding, fetch_k).await?;

        debug!(
            e1_count = e1_results.len(),
            e5_count = e5_results.len(),
            e8_count = e8_results.len(),
            e11_count = e11_results.len(),
            "Multi-embedder search complete, starting RRF fusion"
        );

        // Stage 5: Weighted RRF fusion
        let fused = weighted_rrf_fusion_with_scores(
            vec![
                (e1_results, config.e1_weight, "e1"),
                (e5_results, config.e5_weight, "e5"),
                (e8_results, config.e8_weight, "e8"),
                (e11_results, config.e11_weight, "e11"),
            ],
            RRF_K as i32,
        );

        // Stage 6: Build results with consensus scoring
        let mut results = Vec::with_capacity(fused.len().min(top_k));

        for (id, rrf_score, embedder_scores) in fused.into_iter().take(top_k) {
            let e1_score = *embedder_scores.get("e1").unwrap_or(&0.0);
            let e5_score = *embedder_scores.get("e5").unwrap_or(&0.0);
            let e8_score = *embedder_scores.get("e8").unwrap_or(&0.0);
            let e11_score = *embedder_scores.get("e11").unwrap_or(&0.0);

            let mut result = CausalSearchResult::new(id, e1_score, e5_score, e8_score, e11_score)
                .with_rrf_score(rrf_score);

            // Compute consensus
            result.compute_consensus();

            // Filter by minimum consensus if configured
            if result.consensus_score < config.min_consensus {
                continue;
            }

            // Fetch full relationship if needed
            if let Ok(Some(rel)) = self.get_causal_relationship(id).await {
                result = result.with_relationship(rel);
            }

            results.push(result);
        }

        info!(
            results_count = results.len(),
            "Multi-embedder causal search complete"
        );

        Ok(results)
    }

    /// Get total count of causal relationships.
    pub async fn count_causal_relationships(&self) -> CoreResult<usize> {
        let cf = self.cf_causal_relationships();
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);
        let count = iter.count();
        Ok(count)
    }

    // ========================================================================
    // Delete Operations
    // ========================================================================

    /// Delete a causal relationship by ID.
    ///
    /// Also removes the relationship from the secondary index.
    ///
    /// # Arguments
    /// * `id` - The causal relationship UUID
    ///
    /// # Returns
    /// True if the relationship was deleted, false if not found.
    pub async fn delete_causal_relationship(&self, id: Uuid) -> CoreResult<bool> {
        // First, get the relationship to find its source_id
        let relationship = match self.get_causal_relationship(id).await? {
            Some(r) => r,
            None => {
                debug!("No causal relationship to delete for ID {}", id);
                return Ok(false);
            }
        };

        // Delete from primary CF
        let cf = self.cf_causal_relationships();
        let key = causal_relationship_key(&id);

        self.db.delete_cf(cf, key).map_err(|e| {
            error!(
                "ROCKSDB ERROR: Failed to delete causal relationship {}: {}",
                id, e
            );
            TeleologicalStoreError::rocksdb_op("delete", CF_CAUSAL_RELATIONSHIPS, Some(id), e)
        })?;

        // Update secondary index
        self.remove_from_causal_by_source_index(relationship.source_fingerprint_id, id)
            .await?;

        // Remove from E11 HNSW index
        let removed_from_hnsw = self.causal_e11_index.remove(id);

        info!(
            causal_id = %id,
            source_id = %relationship.source_fingerprint_id,
            removed_from_hnsw = removed_from_hnsw,
            "Deleted causal relationship"
        );

        Ok(true)
    }

    /// Remove a causal_id from the causal_by_source index.
    async fn remove_from_causal_by_source_index(
        &self,
        source_id: Uuid,
        causal_id: Uuid,
    ) -> CoreResult<()> {
        let cf = self.cf_causal_by_source();
        let key = causal_by_source_key(&source_id);

        // Read existing list
        let mut causal_ids: Vec<Uuid> = match self.db.get_cf(cf, key) {
            Ok(Some(bytes)) => bincode::deserialize(&bytes).unwrap_or_default(),
            Ok(None) => return Ok(()), // Nothing to remove
            Err(e) => {
                warn!(
                    "CAUSAL WARNING: Failed to read causal_by_source for {} during delete: {}",
                    source_id, e
                );
                return Ok(());
            }
        };

        // Remove the causal_id
        causal_ids.retain(|id| *id != causal_id);

        if causal_ids.is_empty() {
            // Delete the index entry entirely
            self.db.delete_cf(cf, key).map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to delete empty causal_by_source for {}: {}",
                    source_id, e
                );
                CoreError::StorageError(format!(
                    "Failed to delete causal_by_source for {}: {}",
                    source_id, e
                ))
            })?;
        } else {
            // Update with remaining IDs
            let serialized = bincode::serialize(&causal_ids).map_err(|e| {
                CoreError::Internal(format!("Failed to serialize causal_by_source list: {}", e))
            })?;

            self.db.put_cf(cf, key, &serialized).map_err(|e| {
                error!(
                    "ROCKSDB ERROR: Failed to update causal_by_source for {}: {}",
                    source_id, e
                );
                CoreError::StorageError(format!(
                    "Failed to update causal_by_source for {}: {}",
                    source_id, e
                ))
            })?;
        }

        Ok(())
    }
}

// ============================================================================
// TESTS - Full State Verification
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::types::CausalRelationship;
    use tempfile::TempDir;

    /// Mechanism type values for rotating through test relationships.
    const MECHANISM_TYPES: &[&str] = &["direct", "mediated", "feedback", "temporal"];

    /// Test harness providing store and runtime.
    struct TestHarness {
        store: RocksDbTeleologicalStore,
        rt: tokio::runtime::Runtime,
        _temp_dir: TempDir, // Keep alive for test duration
    }

    impl TestHarness {
        fn new() -> Self {
            let temp_dir = TempDir::new().expect("Failed to create temp dir");
            let store =
                RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open store");
            let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
            Self { store, rt, _temp_dir: temp_dir }
        }
    }

    /// Create a test relationship with realistic data.
    fn create_test_relationship(source_id: Uuid, mechanism_type: &str) -> CausalRelationship {
        CausalRelationship::new(
            "Chronic stress elevates cortisol levels".to_string(),     // cause_statement
            "Memory impairment and cognitive decline".to_string(),     // effect_statement
            format!(
                "This causal relationship describes how chronic stress leads to elevated \
                 cortisol levels. The mechanism involves sustained activation of the HPA axis. \
                 Mechanism type: {}",
                mechanism_type
            ),                                                          // explanation
            vec![0.1f32; CausalRelationship::E5_DIM],                  // e5_as_cause (768D)
            vec![0.2f32; CausalRelationship::E5_DIM],                  // e5_as_effect (768D)
            vec![0.3f32; CausalRelationship::E1_DIM],                  // e1_semantic (1024D)
            "Studies show stress hormones damage hippocampal neurons over time.".to_string(),
            source_id,
            0.85,
            mechanism_type.to_string(),
        )
    }

    #[test]
    fn test_full_state_verification_store_and_get() {
        let h = TestHarness::new();

        let source_id = Uuid::new_v4();
        let rel = create_test_relationship(source_id, "direct");
        let expected_id = rel.id;

        // Store
        let stored_id = h
            .rt
            .block_on(h.store.store_causal_relationship(&rel))
            .expect("Store failed");
        assert_eq!(stored_id, expected_id, "Store should return the relationship ID");

        // Get and verify
        let retrieved = h
            .rt
            .block_on(h.store.get_causal_relationship(stored_id))
            .expect("Get failed")
            .expect("Relationship not found");

        assert_eq!(retrieved.id, expected_id, "ID mismatch");
        assert_eq!(retrieved.source_fingerprint_id, source_id, "Source ID mismatch");
        assert_eq!(retrieved.mechanism_type, "direct", "Mechanism type mismatch");
        assert!((retrieved.confidence - 0.85).abs() < 0.001, "Confidence mismatch");
        assert_eq!(retrieved.e5_as_cause.len(), CausalRelationship::E5_DIM, "E5 cause dim mismatch");
        assert_eq!(retrieved.e5_as_effect.len(), CausalRelationship::E5_DIM, "E5 effect dim mismatch");
        assert_eq!(retrieved.e1_semantic.len(), CausalRelationship::E1_DIM, "E1 dim mismatch");

        // Count
        let count = h
            .rt
            .block_on(h.store.count_causal_relationships())
            .expect("Count failed");
        assert_eq!(count, 1, "Count should be 1");
    }

    #[test]
    fn test_full_state_verification_secondary_index() {
        let h = TestHarness::new();

        // Create multiple relationships for same source
        let source_id = Uuid::new_v4();
        let relationships: Vec<_> = MECHANISM_TYPES
            .iter()
            .map(|dir| create_test_relationship(source_id, dir))
            .collect();
        let expected_ids: Vec<_> = relationships.iter().map(|r| r.id).collect();

        // Store all
        h.rt.block_on(async {
            for rel in &relationships {
                h.store.store_causal_relationship(rel).await.unwrap();
            }
        });

        // Verify secondary index
        let by_source = h
            .rt
            .block_on(h.store.get_causal_relationships_by_source(source_id))
            .expect("Get by source failed");

        assert_eq!(by_source.len(), 4, "Should find 4 relationships");

        let found_ids: std::collections::HashSet<_> = by_source.iter().map(|r| r.id).collect();
        for id in &expected_ids {
            assert!(found_ids.contains(id), "Expected ID {} missing", id);
        }
    }

    #[test]
    fn test_full_state_verification_search() {
        let h = TestHarness::new();

        // Store 10 relationships with slightly different embeddings
        h.rt.block_on(async {
            for i in 0..10 {
                let source_id = Uuid::new_v4();
                let mut rel = create_test_relationship(source_id, "direct");
                // Modify E1 semantic embedding for search differentiation
                rel.e1_semantic[0] = 0.1 + (i as f32 * 0.01);
                h.store.store_causal_relationship(&rel).await.unwrap();
            }
        });

        let query_embedding = vec![0.3f32; CausalRelationship::E1_DIM];

        // Search without filter
        let results = h
            .rt
            .block_on(h.store.search_causal_relationships(&query_embedding, 5, None))
            .expect("Search failed");

        assert!(!results.is_empty(), "Search should return results");
        assert!(results.len() <= 5, "Should return at most top_k results");

        // Search with mechanism type filter
        let direct_results = h
            .rt
            .block_on(h.store.search_causal_relationships(&query_embedding, 5, Some("direct")))
            .expect("Search failed");

        assert!(!direct_results.is_empty(), "Filtered search should return results");
    }

    #[test]
    fn test_full_state_verification_delete() {
        let h = TestHarness::new();

        let source_id = Uuid::new_v4();
        let rel = create_test_relationship(source_id, "cause");
        let rel_id = rel.id;

        // Store
        h.rt.block_on(h.store.store_causal_relationship(&rel)).unwrap();

        let count_before = h.rt.block_on(h.store.count_causal_relationships()).unwrap();
        assert_eq!(count_before, 1, "Should have 1 relationship");

        // Delete
        let deleted = h
            .rt
            .block_on(h.store.delete_causal_relationship(rel_id))
            .expect("Delete failed");
        assert!(deleted, "Delete should return true");

        // Verify count is zero
        let count_after = h.rt.block_on(h.store.count_causal_relationships()).unwrap();
        assert_eq!(count_after, 0, "Should have 0 relationships");

        // Verify not retrievable
        let retrieved = h
            .rt
            .block_on(h.store.get_causal_relationship(rel_id))
            .expect("Get failed");
        assert!(retrieved.is_none(), "Should not find deleted relationship");

        // Verify secondary index cleared
        let by_source = h
            .rt
            .block_on(h.store.get_causal_relationships_by_source(source_id))
            .expect("Get by source failed");
        assert!(by_source.is_empty(), "Secondary index should be cleared");
    }

    #[test]
    fn test_edge_case_empty_database() {
        let h = TestHarness::new();

        // Get non-existent
        let result = h
            .rt
            .block_on(h.store.get_causal_relationship(Uuid::new_v4()))
            .expect("Get failed");
        assert!(result.is_none(), "Should return None for non-existent ID");

        // Get by source non-existent
        let by_source = h
            .rt
            .block_on(h.store.get_causal_relationships_by_source(Uuid::new_v4()))
            .expect("Get by source failed");
        assert!(by_source.is_empty(), "Should return empty vec for non-existent source");

        // Search empty
        let query = vec![0.1f32; 1024];
        let results = h
            .rt
            .block_on(h.store.search_causal_relationships(&query, 10, None))
            .expect("Search failed");
        assert!(results.is_empty(), "Search should return empty on empty database");

        // Count empty
        let count = h.rt.block_on(h.store.count_causal_relationships()).expect("Count failed");
        assert_eq!(count, 0, "Count should be 0 on empty database");
    }

    #[test]
    fn test_edge_case_large_batch() {
        let h = TestHarness::new();

        const BATCH_SIZE: usize = 100;

        h.rt.block_on(async {
            for i in 0..BATCH_SIZE {
                let source_id = Uuid::new_v4();
                let direction = MECHANISM_TYPES[i % MECHANISM_TYPES.len()];
                let rel = create_test_relationship(source_id, direction);
                h.store.store_causal_relationship(&rel).await.unwrap();
            }
        });

        let count = h.rt.block_on(h.store.count_causal_relationships()).expect("Count failed");
        assert_eq!(count, BATCH_SIZE, "Count should match batch size");
    }
}
