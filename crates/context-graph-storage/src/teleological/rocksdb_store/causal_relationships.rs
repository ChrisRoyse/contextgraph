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

        info!(
            causal_id = %relationship.id,
            source_id = %relationship.source_fingerprint_id,
            direction = %relationship.direction,
            desc_len = relationship.description.len(),
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

            // Apply direction filter if specified
            if let Some(filter) = direction_filter {
                if filter != "all" && relationship.normalized_direction() != filter {
                    continue;
                }
            }

            // Compute similarity
            let similarity =
                compute_cosine_similarity(query_embedding, &relationship.description_embedding);

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

        info!(
            causal_id = %id,
            source_id = %relationship.source_fingerprint_id,
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

    /// Direction values for rotating through test relationships.
    const DIRECTIONS: &[&str] = &["cause", "effect", "bidirectional"];

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
    fn create_test_relationship(source_id: Uuid, direction: &str) -> CausalRelationship {
        CausalRelationship::new(
            format!(
                "This causal relationship describes how chronic stress leads to elevated \
                 cortisol levels. The mechanism involves sustained activation of the HPA axis. \
                 Direction: {}",
                direction
            ),
            vec![0.1f32; 1024], // E1 1024D embedding
            "Studies show stress hormones damage hippocampal neurons over time.".to_string(),
            source_id,
            direction.to_string(),
            0.85,
            vec!["causes".to_string(), "leads to".to_string()],
        )
    }

    #[test]
    fn test_full_state_verification_store_and_get() {
        let h = TestHarness::new();

        let source_id = Uuid::new_v4();
        let rel = create_test_relationship(source_id, "cause");
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
        assert_eq!(retrieved.direction, "cause", "Direction mismatch");
        assert!((retrieved.confidence - 0.85).abs() < 0.001, "Confidence mismatch");
        assert_eq!(retrieved.description_embedding.len(), 1024, "Embedding dim mismatch");

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
        let relationships: Vec<_> = DIRECTIONS
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

        assert_eq!(by_source.len(), 3, "Should find 3 relationships");

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
                let mut rel = create_test_relationship(source_id, "cause");
                rel.description_embedding[0] = 0.1 + (i as f32 * 0.01);
                h.store.store_causal_relationship(&rel).await.unwrap();
            }
        });

        let query_embedding = vec![0.1f32; 1024];

        // Search without filter
        let results = h
            .rt
            .block_on(h.store.search_causal_relationships(&query_embedding, 5, None))
            .expect("Search failed");

        assert!(!results.is_empty(), "Search should return results");
        assert!(results.len() <= 5, "Should return at most top_k results");

        // Search with direction filter
        let cause_results = h
            .rt
            .block_on(h.store.search_causal_relationships(&query_embedding, 5, Some("cause")))
            .expect("Search failed");

        assert!(!cause_results.is_empty(), "Filtered search should return results");
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
                let direction = DIRECTIONS[i % DIRECTIONS.len()];
                let rel = create_test_relationship(source_id, direction);
                h.store.store_causal_relationship(&rel).await.unwrap();
            }
        });

        let count = h.rt.block_on(h.store.count_causal_relationships()).expect("Count failed");
        assert_eq!(count, BATCH_SIZE, "Count should match batch size");
    }
}
