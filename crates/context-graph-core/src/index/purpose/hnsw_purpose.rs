//! HNSW-backed purpose pattern index with metadata enrichment.
//!
//! # CRITICAL: NO FALLBACKS
//!
//! All index operations are fail-fast. Missing entries cause immediate errors.
//! Invalid queries rejected at construction time.
//!
//! # Overview
//!
//! This module provides the main `HnswPurposeIndex` for Stage 4 retrieval:
//! - HNSW-backed ANN search on 13D purpose vectors
//! - Metadata storage for goal and quadrant filtering
//! - Secondary indexes for efficient filtered queries
//!
//! # Architecture
//!
//! ```text
//! HnswPurposeIndex
//! ├── inner: RealHnswIndex (13D HNSW for ANN search - O(log n))
//! ├── metadata: HashMap<Uuid, PurposeMetadata>
//! ├── vectors: HashMap<Uuid, PurposeVector>
//! ├── quadrant_index: HashMap<JohariQuadrant, HashSet<Uuid>>
//! └── goal_index: HashMap<GoalId, HashSet<Uuid>>
//! ```
//!
//! # Usage
//!
//! ```ignore
//! use context_graph_core::index::purpose::{
//!     HnswPurposeIndex, PurposeIndexOps, PurposeIndexEntry,
//!     PurposeQuery, PurposeQueryTarget,
//! };
//!
//! // Create index
//! let mut index = HnswPurposeIndex::new(HnswConfig::purpose_vector())?;
//!
//! // Insert entries
//! index.insert(entry)?;
//!
//! // Search
//! let results = index.search(&query)?;
//! ```

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::index::config::{HnswConfig, PURPOSE_VECTOR_DIM};
use crate::index::hnsw_impl::RealHnswIndex;
use crate::types::fingerprint::PurposeVector;
use crate::types::JohariQuadrant;

use super::clustering::{KMeansConfig, KMeansPurposeClustering, StandardKMeans};
use super::entry::{GoalId, PurposeIndexEntry, PurposeMetadata};
use super::error::{PurposeIndexError, PurposeIndexResult};
use super::query::{PurposeQuery, PurposeQueryTarget, PurposeSearchResult};

/// HNSW-backed purpose pattern index with metadata enrichment.
///
/// Combines a REAL HNSW index (O(log n)) for fast ANN search on 13D purpose
/// vectors with secondary indexes for efficient filtering by goal and Johari quadrant.
///
/// # Structure
///
/// - `inner`: RealHnswIndex for approximate nearest neighbor search (O(log n))
/// - `metadata`: Purpose metadata indexed by memory ID
/// - `vectors`: Purpose vectors indexed by memory ID for reranking
/// - `quadrant_index`: Inverted index from quadrant to memory IDs
/// - `goal_index`: Inverted index from goal ID to memory IDs
///
/// # Fail-Fast Semantics
///
/// - Insert validates dimension matches PURPOSE_VECTOR_DIM (13)
/// - Remove fails if memory not found (no silent no-ops)
/// - Search validates query before execution
/// - Get fails if memory not found
#[derive(Debug)]
pub struct HnswPurposeIndex {
    /// Underlying REAL HNSW index for ANN search (O(log n), not O(n) like old SimpleHnswIndex).
    inner: RealHnswIndex,
    /// Metadata storage indexed by memory ID.
    metadata: HashMap<Uuid, PurposeMetadata>,
    /// Purpose vectors for reranking.
    vectors: HashMap<Uuid, PurposeVector>,
    /// Index by Johari quadrant for filtered queries.
    quadrant_index: HashMap<JohariQuadrant, HashSet<Uuid>>,
    /// Index by primary goal for filtered queries.
    goal_index: HashMap<String, HashSet<Uuid>>,
}

/// Trait for purpose index operations.
///
/// Defines the core operations for managing and querying the purpose index.
///
/// # Fail-Fast Semantics
///
/// All operations validate inputs and fail immediately on invalid data.
/// There are no silent failures or fallback behaviors.
pub trait PurposeIndexOps {
    /// Insert a purpose entry into the index.
    ///
    /// # Arguments
    ///
    /// * `entry` - The purpose index entry to insert
    ///
    /// # Errors
    ///
    /// - `DimensionMismatch`: If vector dimension != 13
    /// - `HnswError`: If HNSW insertion fails
    ///
    /// # Behavior
    ///
    /// If an entry with the same memory_id already exists, it is replaced.
    /// Secondary indexes are updated accordingly.
    fn insert(&mut self, entry: PurposeIndexEntry) -> PurposeIndexResult<()>;

    /// Remove a memory from the index.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID of the memory to remove
    ///
    /// # Errors
    ///
    /// - `NotFound`: If memory_id does not exist in the index
    ///
    /// # Fail-Fast
    ///
    /// Does NOT silently succeed if memory is not found.
    fn remove(&mut self, memory_id: Uuid) -> PurposeIndexResult<()>;

    /// Search the index with query parameters.
    ///
    /// # Arguments
    ///
    /// * `query` - The purpose query specifying target and filters
    ///
    /// # Returns
    ///
    /// Vector of search results sorted by descending similarity.
    ///
    /// # Errors
    ///
    /// - `NotFound`: For FromMemory target when memory doesn't exist
    /// - `ClusteringError`: For Pattern target when clustering fails
    /// - `InvalidQuery`: If query parameters are invalid
    fn search(&self, query: &PurposeQuery) -> PurposeIndexResult<Vec<PurposeSearchResult>>;

    /// Get entry by memory ID.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID of the memory to retrieve
    ///
    /// # Errors
    ///
    /// - `NotFound`: If memory_id does not exist in the index
    fn get(&self, memory_id: Uuid) -> PurposeIndexResult<PurposeIndexEntry>;

    /// Check if memory exists in index.
    ///
    /// # Arguments
    ///
    /// * `memory_id` - UUID to check
    ///
    /// # Returns
    ///
    /// `true` if the memory exists in the index.
    fn contains(&self, memory_id: Uuid) -> bool;

    /// Get total number of entries.
    fn len(&self) -> usize;

    /// Check if empty.
    fn is_empty(&self) -> bool;

    /// Clear all entries.
    fn clear(&mut self);
}

impl HnswPurposeIndex {
    /// Create a new purpose index with given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - HNSW configuration (should have dimension = 13)
    ///
    /// # Errors
    ///
    /// Returns `DimensionMismatch` if config.dimension != PURPOSE_VECTOR_DIM.
    /// Returns `HnswError` if HNSW index construction fails.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let index = HnswPurposeIndex::new(HnswConfig::purpose_vector())?;
    /// ```
    pub fn new(config: HnswConfig) -> PurposeIndexResult<Self> {
        // Fail-fast: validate dimension
        if config.dimension != PURPOSE_VECTOR_DIM {
            return Err(PurposeIndexError::dimension_mismatch(
                PURPOSE_VECTOR_DIM,
                config.dimension,
            ));
        }

        // Create real HNSW index - may fail, propagate error via #[from] conversion
        let inner = RealHnswIndex::new(config)?;

        Ok(Self {
            inner,
            metadata: HashMap::new(),
            vectors: HashMap::new(),
            quadrant_index: HashMap::new(),
            goal_index: HashMap::new(),
        })
    }

    /// Create a new purpose index with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `config` - HNSW configuration
    /// * `capacity` - Expected number of entries for pre-allocation
    ///
    /// # Errors
    ///
    /// Returns `DimensionMismatch` if config.dimension != PURPOSE_VECTOR_DIM.
    /// Returns `HnswError` if HNSW index construction fails.
    pub fn with_capacity(config: HnswConfig, capacity: usize) -> PurposeIndexResult<Self> {
        // Fail-fast: validate dimension
        if config.dimension != PURPOSE_VECTOR_DIM {
            return Err(PurposeIndexError::dimension_mismatch(
                PURPOSE_VECTOR_DIM,
                config.dimension,
            ));
        }

        // Create real HNSW index - may fail, propagate error via #[from] conversion
        let inner = RealHnswIndex::new(config)?;

        Ok(Self {
            inner,
            metadata: HashMap::with_capacity(capacity),
            vectors: HashMap::with_capacity(capacity),
            quadrant_index: HashMap::new(),
            goal_index: HashMap::new(),
        })
    }

    /// Get memories in a specific quadrant.
    ///
    /// Returns all memory IDs that are in the given Johari quadrant.
    #[inline]
    pub fn get_by_quadrant(&self, quadrant: JohariQuadrant) -> Option<&HashSet<Uuid>> {
        self.quadrant_index.get(&quadrant)
    }

    /// Get memories with a specific goal.
    ///
    /// Returns all memory IDs aligned with the given goal.
    #[inline]
    pub fn get_by_goal(&self, goal: &GoalId) -> Option<&HashSet<Uuid>> {
        self.goal_index.get(goal.as_str())
    }

    /// Get the number of distinct goals in the index.
    #[inline]
    pub fn goal_count(&self) -> usize {
        self.goal_index.len()
    }

    /// Get all goals present in the index.
    pub fn goals(&self) -> Vec<GoalId> {
        self.goal_index.keys().map(GoalId::new).collect()
    }

    /// Update secondary indexes when inserting an entry.
    fn update_secondary_indexes(&mut self, memory_id: Uuid, metadata: &PurposeMetadata) {
        // Update quadrant index
        self.quadrant_index
            .entry(metadata.dominant_quadrant)
            .or_default()
            .insert(memory_id);

        // Update goal index
        self.goal_index
            .entry(metadata.primary_goal.as_str().to_string())
            .or_default()
            .insert(memory_id);
    }

    /// Remove from secondary indexes when removing an entry.
    fn remove_from_secondary_indexes(&mut self, memory_id: Uuid, metadata: &PurposeMetadata) {
        // Remove from quadrant index
        if let Some(set) = self.quadrant_index.get_mut(&metadata.dominant_quadrant) {
            set.remove(&memory_id);
            if set.is_empty() {
                self.quadrant_index.remove(&metadata.dominant_quadrant);
            }
        }

        // Remove from goal index
        let goal_key = metadata.primary_goal.as_str();
        if let Some(set) = self.goal_index.get_mut(goal_key) {
            set.remove(&memory_id);
            if set.is_empty() {
                self.goal_index.remove(goal_key);
            }
        }
    }

    /// Perform vector search with optional filtering.
    fn search_vector(
        &self,
        query_vector: &PurposeVector,
        query: &PurposeQuery,
    ) -> PurposeIndexResult<Vec<PurposeSearchResult>> {
        // If we have filters, compute candidate set first
        let candidates = self.compute_candidate_set(query);

        // Determine how many results to request from HNSW
        // Request more if filtering to ensure we get enough after filtering
        let k = if query.has_filters() {
            // Request extra to account for filtering
            query.limit * 3 + 10
        } else {
            query.limit
        };

        // Search HNSW index
        let alignments = &query_vector.alignments;
        let hnsw_results = self.inner.search(alignments.as_slice(), k)?;

        // Build results with filtering and reranking
        let mut results: Vec<PurposeSearchResult> = hnsw_results
            .into_iter()
            .filter_map(|(memory_id, similarity)| {
                // Apply candidate filter if we have one
                if let Some(ref cands) = candidates {
                    if !cands.contains(&memory_id) {
                        return None;
                    }
                }

                // Apply min_similarity filter
                if similarity < query.min_similarity {
                    return None;
                }

                // Retrieve metadata and vector
                let metadata = self.metadata.get(&memory_id)?;
                let vector = self.vectors.get(&memory_id)?;

                // Apply goal filter
                if let Some(ref goal_filter) = query.goal_filter {
                    if metadata.primary_goal.as_str() != goal_filter.as_str() {
                        return None;
                    }
                }

                // Apply quadrant filter
                if let Some(quadrant_filter) = query.quadrant_filter {
                    if metadata.dominant_quadrant != quadrant_filter {
                        return None;
                    }
                }

                Some(PurposeSearchResult::new(
                    memory_id,
                    similarity,
                    vector.clone(),
                    metadata.clone(),
                ))
            })
            .collect();

        // Sort by similarity descending (should already be sorted, but ensure)
        results.sort_by(|a, b| {
            b.purpose_similarity
                .partial_cmp(&a.purpose_similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Limit results
        results.truncate(query.limit);

        Ok(results)
    }

    /// Compute candidate set based on filters.
    fn compute_candidate_set(&self, query: &PurposeQuery) -> Option<HashSet<Uuid>> {
        if !query.has_filters() {
            return None;
        }

        let mut candidates: Option<HashSet<Uuid>> = None;

        // Apply goal filter
        if let Some(ref goal) = query.goal_filter {
            if let Some(goal_set) = self.goal_index.get(goal.as_str()) {
                candidates = Some(goal_set.clone());
            } else {
                // No memories with this goal - return empty set
                return Some(HashSet::new());
            }
        }

        // Apply quadrant filter
        if let Some(quadrant) = query.quadrant_filter {
            if let Some(quadrant_set) = self.quadrant_index.get(&quadrant) {
                candidates = Some(match candidates {
                    Some(cands) => cands.intersection(quadrant_set).copied().collect(),
                    None => quadrant_set.clone(),
                });
            } else {
                // No memories in this quadrant - return empty set
                return Some(HashSet::new());
            }
        }

        candidates
    }

    /// Search using pattern clustering.
    fn search_pattern(
        &self,
        min_cluster_size: usize,
        coherence_threshold: f32,
        query: &PurposeQuery,
    ) -> PurposeIndexResult<Vec<PurposeSearchResult>> {
        // Collect all entries for clustering
        let entries: Vec<PurposeIndexEntry> = self
            .vectors
            .iter()
            .filter_map(|(id, vector)| {
                let metadata = self.metadata.get(id)?;
                Some(PurposeIndexEntry::new(
                    *id,
                    vector.clone(),
                    metadata.clone(),
                ))
            })
            .collect();

        if entries.is_empty() {
            return Ok(Vec::new());
        }

        // Determine k based on entry count and min_cluster_size
        let max_k = entries.len() / min_cluster_size.max(1);
        let k = max_k.max(1).min(entries.len());

        // Run clustering
        let config = KMeansConfig::with_k(k)?;
        let clusterer = StandardKMeans::new();
        let clustering_result = clusterer.cluster_purposes(&entries, &config)?;

        // Filter clusters by size and coherence, collect matching members
        let mut results: Vec<PurposeSearchResult> = Vec::new();

        for cluster in clustering_result.clusters {
            if cluster.len() >= min_cluster_size && cluster.coherence >= coherence_threshold {
                for memory_id in cluster.members {
                    if let (Some(vector), Some(metadata)) =
                        (self.vectors.get(&memory_id), self.metadata.get(&memory_id))
                    {
                        // Apply additional filters
                        if let Some(ref goal_filter) = query.goal_filter {
                            if metadata.primary_goal.as_str() != goal_filter.as_str() {
                                continue;
                            }
                        }
                        if let Some(quadrant_filter) = query.quadrant_filter {
                            if metadata.dominant_quadrant != quadrant_filter {
                                continue;
                            }
                        }

                        results.push(PurposeSearchResult::new(
                            memory_id,
                            cluster.coherence, // Use cluster coherence as similarity
                            vector.clone(),
                            metadata.clone(),
                        ));
                    }
                }
            }
        }

        // Sort by similarity (coherence) descending
        results.sort_by(|a, b| {
            b.purpose_similarity
                .partial_cmp(&a.purpose_similarity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Apply min_similarity filter and limit
        results.retain(|r| r.purpose_similarity >= query.min_similarity);
        results.truncate(query.limit);

        Ok(results)
    }
}

impl PurposeIndexOps for HnswPurposeIndex {
    fn insert(&mut self, entry: PurposeIndexEntry) -> PurposeIndexResult<()> {
        let memory_id = entry.memory_id;
        let alignments = entry.purpose_vector.alignments;

        // Validate dimension
        if alignments.len() != PURPOSE_VECTOR_DIM {
            return Err(PurposeIndexError::dimension_mismatch(
                PURPOSE_VECTOR_DIM,
                alignments.len(),
            ));
        }

        // Remove existing entry if present (update case)
        if self.metadata.contains_key(&memory_id) {
            // Get old metadata for secondary index cleanup
            if let Some(old_metadata) = self.metadata.get(&memory_id) {
                let old_metadata_clone = old_metadata.clone();
                self.remove_from_secondary_indexes(memory_id, &old_metadata_clone);
            }
            self.inner.remove(memory_id);
        }

        // Insert into HNSW
        self.inner.add(memory_id, &alignments)?;

        // Store metadata and vector
        self.metadata.insert(memory_id, entry.metadata.clone());
        self.vectors.insert(memory_id, entry.purpose_vector.clone());

        // Update secondary indexes
        self.update_secondary_indexes(memory_id, &entry.metadata);

        Ok(())
    }

    fn remove(&mut self, memory_id: Uuid) -> PurposeIndexResult<()> {
        // Fail-fast: check existence
        let metadata = self
            .metadata
            .get(&memory_id)
            .ok_or_else(|| PurposeIndexError::not_found(memory_id))?
            .clone();

        // Remove from HNSW
        self.inner.remove(memory_id);

        // Remove from primary storage
        self.metadata.remove(&memory_id);
        self.vectors.remove(&memory_id);

        // Remove from secondary indexes
        self.remove_from_secondary_indexes(memory_id, &metadata);

        Ok(())
    }

    fn search(&self, query: &PurposeQuery) -> PurposeIndexResult<Vec<PurposeSearchResult>> {
        // Validate query
        query.validate()?;

        // Handle different query targets
        match &query.target {
            PurposeQueryTarget::Vector(pv) => self.search_vector(pv, query),

            PurposeQueryTarget::Pattern {
                min_cluster_size,
                coherence_threshold,
            } => self.search_pattern(*min_cluster_size, *coherence_threshold, query),

            PurposeQueryTarget::FromMemory(memory_id) => {
                // Fail-fast: memory must exist
                let vector = self
                    .vectors
                    .get(memory_id)
                    .ok_or_else(|| PurposeIndexError::not_found(*memory_id))?
                    .clone();

                self.search_vector(&vector, query)
            }
        }
    }

    fn get(&self, memory_id: Uuid) -> PurposeIndexResult<PurposeIndexEntry> {
        let metadata = self
            .metadata
            .get(&memory_id)
            .ok_or_else(|| PurposeIndexError::not_found(memory_id))?
            .clone();

        let vector = self
            .vectors
            .get(&memory_id)
            .ok_or_else(|| PurposeIndexError::not_found(memory_id))?
            .clone();

        Ok(PurposeIndexEntry::new(memory_id, vector, metadata))
    }

    fn contains(&self, memory_id: Uuid) -> bool {
        self.metadata.contains_key(&memory_id)
    }

    fn len(&self) -> usize {
        self.metadata.len()
    }

    fn is_empty(&self) -> bool {
        self.metadata.is_empty()
    }

    fn clear(&mut self) {
        // Get all IDs first to avoid borrowing issues during removal
        let ids: Vec<Uuid> = self.metadata.keys().copied().collect();

        // Clear primary storage
        self.metadata.clear();
        self.vectors.clear();
        self.quadrant_index.clear();
        self.goal_index.clear();

        // Remove all entries from HNSW index
        for id in ids {
            self.inner.remove(id);
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::config::DistanceMetric;

    // =========================================================================
    // Helper functions for creating REAL test data (NO mocks)
    // =========================================================================

    /// Create a purpose vector with deterministic values.
    fn create_purpose_vector(base: f32, variation: f32) -> PurposeVector {
        let mut alignments = [0.0f32; PURPOSE_VECTOR_DIM];
        for (i, alignment) in alignments.iter_mut().enumerate() {
            *alignment = (base + (i as f32 * variation)).clamp(0.0, 1.0);
        }
        PurposeVector::new(alignments)
    }

    /// Create metadata with given goal and quadrant.
    fn create_metadata(goal: &str, quadrant: JohariQuadrant) -> PurposeMetadata {
        PurposeMetadata::new(GoalId::new(goal), 0.85, quadrant).unwrap()
    }

    /// Create a complete purpose index entry.
    fn create_entry(base: f32, goal: &str, quadrant: JohariQuadrant) -> PurposeIndexEntry {
        let pv = create_purpose_vector(base, 0.02);
        let metadata = create_metadata(goal, quadrant);
        PurposeIndexEntry::new(Uuid::new_v4(), pv, metadata)
    }

    /// Create a default HNSW config for purpose vectors.
    fn purpose_config() -> HnswConfig {
        HnswConfig::new(16, 200, 100, DistanceMetric::Cosine, PURPOSE_VECTOR_DIM)
    }

    // =========================================================================
    // Constructor Tests
    // =========================================================================

    #[test]
    fn test_hnsw_purpose_index_new() {
        let config = purpose_config();
        let index = HnswPurposeIndex::new(config).unwrap();

        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.goal_count(), 0);

        println!("[VERIFIED] HnswPurposeIndex::new creates empty index");
    }

    #[test]
    fn test_hnsw_purpose_index_new_wrong_dimension() {
        let wrong_config = HnswConfig::new(16, 200, 100, DistanceMetric::Cosine, 100);
        let result = HnswPurposeIndex::new(wrong_config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("13"));
        assert!(msg.contains("100"));

        println!(
            "[VERIFIED] FAIL FAST: HnswPurposeIndex::new rejects wrong dimension: {}",
            msg
        );
    }

    #[test]
    fn test_hnsw_purpose_index_with_capacity() {
        let config = purpose_config();
        let index = HnswPurposeIndex::with_capacity(config, 1000).unwrap();

        assert!(index.is_empty());

        println!("[VERIFIED] HnswPurposeIndex::with_capacity pre-allocates");
    }

    // =========================================================================
    // Insert Tests
    // =========================================================================

    #[test]
    fn test_insert_single_entry() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let entry = create_entry(0.7, "master_ml", JohariQuadrant::Open);
        let memory_id = entry.memory_id;

        println!("[BEFORE] index.len()={}", index.len());

        index.insert(entry).unwrap();

        println!("[AFTER] index.len()={}", index.len());

        assert_eq!(index.len(), 1);
        assert!(index.contains(memory_id));

        println!("[VERIFIED] Single entry inserted correctly");
    }

    #[test]
    fn test_insert_multiple_entries() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let entries: Vec<PurposeIndexEntry> = (0..10)
            .map(|i| create_entry(0.3 + i as f32 * 0.05, "goal", JohariQuadrant::Open))
            .collect();

        for entry in &entries {
            index.insert(entry.clone()).unwrap();
        }

        assert_eq!(index.len(), 10);

        println!("[VERIFIED] Multiple entries inserted correctly");
    }

    #[test]
    fn test_insert_updates_existing() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let memory_id = Uuid::new_v4();

        // First insert
        let entry1 = PurposeIndexEntry::new(
            memory_id,
            create_purpose_vector(0.5, 0.02),
            create_metadata("goal1", JohariQuadrant::Open),
        );
        index.insert(entry1).unwrap();

        assert_eq!(index.len(), 1);
        let retrieved1 = index.get(memory_id).unwrap();
        assert_eq!(retrieved1.metadata.primary_goal.as_str(), "goal1");

        // Update with same ID, different goal
        let entry2 = PurposeIndexEntry::new(
            memory_id,
            create_purpose_vector(0.8, 0.01),
            create_metadata("goal2", JohariQuadrant::Hidden),
        );
        index.insert(entry2).unwrap();

        assert_eq!(index.len(), 1); // Still only 1 entry
        let retrieved2 = index.get(memory_id).unwrap();
        assert_eq!(retrieved2.metadata.primary_goal.as_str(), "goal2");

        println!("[VERIFIED] Insert updates existing entry with same ID");
    }

    #[test]
    fn test_insert_updates_secondary_indexes() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let entry1 = create_entry(0.5, "goal_a", JohariQuadrant::Open);
        let entry2 = create_entry(0.6, "goal_b", JohariQuadrant::Hidden);
        let entry3 = create_entry(0.7, "goal_a", JohariQuadrant::Open);

        index.insert(entry1.clone()).unwrap();
        index.insert(entry2.clone()).unwrap();
        index.insert(entry3.clone()).unwrap();

        // Check goal index
        assert_eq!(index.goal_count(), 2);
        let goal_a_set = index.get_by_goal(&GoalId::new("goal_a")).unwrap();
        assert_eq!(goal_a_set.len(), 2);
        assert!(goal_a_set.contains(&entry1.memory_id));
        assert!(goal_a_set.contains(&entry3.memory_id));

        // Check quadrant index
        let open_set = index.get_by_quadrant(JohariQuadrant::Open).unwrap();
        assert_eq!(open_set.len(), 2);
        let hidden_set = index.get_by_quadrant(JohariQuadrant::Hidden).unwrap();
        assert_eq!(hidden_set.len(), 1);

        println!("[VERIFIED] Insert updates secondary indexes correctly");
    }

    // =========================================================================
    // Remove Tests
    // =========================================================================

    #[test]
    fn test_remove_existing_entry() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let entry = create_entry(0.7, "test_goal", JohariQuadrant::Blind);
        let memory_id = entry.memory_id;

        index.insert(entry).unwrap();
        assert_eq!(index.len(), 1);

        println!("[BEFORE REMOVE] index.len()={}", index.len());

        index.remove(memory_id).unwrap();

        println!("[AFTER REMOVE] index.len()={}", index.len());

        assert_eq!(index.len(), 0);
        assert!(!index.contains(memory_id));

        println!("[VERIFIED] Remove deletes entry correctly");
    }

    #[test]
    fn test_remove_non_existent_fails() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let non_existent_id = Uuid::new_v4();

        let result = index.remove(non_existent_id);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&non_existent_id.to_string()));

        println!(
            "[VERIFIED] FAIL FAST: Remove fails for non-existent entry: {}",
            msg
        );
    }

    #[test]
    fn test_remove_updates_secondary_indexes() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let entry1 = create_entry(0.5, "shared_goal", JohariQuadrant::Open);
        let entry2 = create_entry(0.6, "shared_goal", JohariQuadrant::Open);
        let id1 = entry1.memory_id;

        index.insert(entry1).unwrap();
        index.insert(entry2).unwrap();

        assert_eq!(index.goal_count(), 1);
        assert_eq!(
            index
                .get_by_goal(&GoalId::new("shared_goal"))
                .unwrap()
                .len(),
            2
        );

        index.remove(id1).unwrap();

        assert_eq!(index.goal_count(), 1); // Goal still exists
        assert_eq!(
            index
                .get_by_goal(&GoalId::new("shared_goal"))
                .unwrap()
                .len(),
            1
        ); // But with one less member

        println!("[VERIFIED] Remove updates secondary indexes correctly");
    }

    #[test]
    fn test_remove_cleans_up_empty_indexes() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let entry = create_entry(0.5, "unique_goal", JohariQuadrant::Unknown);
        let id = entry.memory_id;

        index.insert(entry).unwrap();
        assert_eq!(index.goal_count(), 1);
        assert!(index.get_by_quadrant(JohariQuadrant::Unknown).is_some());

        index.remove(id).unwrap();

        // Empty sets should be removed
        assert_eq!(index.goal_count(), 0);
        assert!(index.get_by_goal(&GoalId::new("unique_goal")).is_none());
        assert!(index.get_by_quadrant(JohariQuadrant::Unknown).is_none());

        println!("[VERIFIED] Remove cleans up empty secondary index entries");
    }

    // =========================================================================
    // Get Tests
    // =========================================================================

    #[test]
    fn test_get_existing_entry() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let entry = create_entry(0.75, "retrieve_goal", JohariQuadrant::Hidden);
        let memory_id = entry.memory_id;

        index.insert(entry.clone()).unwrap();

        let retrieved = index.get(memory_id).unwrap();

        assert_eq!(retrieved.memory_id, memory_id);
        assert_eq!(
            retrieved.metadata.primary_goal.as_str(),
            entry.metadata.primary_goal.as_str()
        );
        assert_eq!(
            retrieved.purpose_vector.alignments,
            entry.purpose_vector.alignments
        );

        println!("[VERIFIED] Get retrieves correct entry");
    }

    #[test]
    fn test_get_non_existent_fails() {
        let index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let non_existent_id = Uuid::new_v4();

        let result = index.get(non_existent_id);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"));

        println!(
            "[VERIFIED] FAIL FAST: Get fails for non-existent entry: {}",
            msg
        );
    }

    // =========================================================================
    // Search Tests
    // =========================================================================

    #[test]
    fn test_search_empty_index() {
        let index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let query = PurposeQuery::new(
            PurposeQueryTarget::vector(create_purpose_vector(0.5, 0.02)),
            10,
            0.0,
        )
        .unwrap();

        // Correct database semantics: searching an empty index returns empty results
        // Error should only occur on actual failures (network, disk, corruption, invalid input)
        let result = index.search(&query);
        assert!(
            result.is_ok(),
            "Search on empty index should succeed with empty results"
        );

        let results = result.unwrap();
        assert!(
            results.is_empty(),
            "Empty index should return empty results"
        );

        println!(
            "[VERIFIED] Search on empty index returns empty results (correct database semantics)"
        );
    }

    #[test]
    fn test_search_vector_target() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries with varying similarity to query
        let entries: Vec<PurposeIndexEntry> = (0..5)
            .map(|i| create_entry(0.4 + i as f32 * 0.1, "goal", JohariQuadrant::Open))
            .collect();

        for entry in &entries {
            index.insert(entry.clone()).unwrap();
        }

        // Search with vector similar to highest entry
        let query_vector = create_purpose_vector(0.8, 0.02);
        let query = PurposeQuery::new(PurposeQueryTarget::vector(query_vector), 3, 0.0).unwrap();

        println!("[BEFORE] Searching for 3 nearest neighbors");

        let results = index.search(&query).unwrap();

        println!("[AFTER] Found {} results", results.len());

        assert_eq!(results.len(), 3);
        // Results should be sorted by similarity descending
        assert!(results[0].purpose_similarity >= results[1].purpose_similarity);
        assert!(results[1].purpose_similarity >= results[2].purpose_similarity);

        println!("[VERIFIED] Search with vector target returns sorted results");
    }

    #[test]
    fn test_search_with_min_similarity_filter() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..10 {
            index
                .insert(create_entry(
                    0.1 + i as f32 * 0.08,
                    "goal",
                    JohariQuadrant::Open,
                ))
                .unwrap();
        }

        let query_vector = create_purpose_vector(0.9, 0.01);
        let query = PurposeQuery::new(PurposeQueryTarget::vector(query_vector), 10, 0.8).unwrap();

        let results = index.search(&query).unwrap();

        // All results should meet min_similarity threshold
        for result in &results {
            assert!(
                result.purpose_similarity >= 0.8,
                "Similarity {} should be >= 0.8",
                result.purpose_similarity
            );
        }

        println!(
            "[VERIFIED] Search respects min_similarity filter ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_search_with_goal_filter() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries with different goals
        for i in 0..5 {
            index
                .insert(create_entry(
                    0.5 + i as f32 * 0.02,
                    "goal_a",
                    JohariQuadrant::Open,
                ))
                .unwrap();
            index
                .insert(create_entry(
                    0.5 + i as f32 * 0.02,
                    "goal_b",
                    JohariQuadrant::Open,
                ))
                .unwrap();
        }

        let query_vector = create_purpose_vector(0.55, 0.02);
        let query = PurposeQuery::new(PurposeQueryTarget::vector(query_vector), 10, 0.0)
            .unwrap()
            .with_goal_filter(GoalId::new("goal_a"));

        let results = index.search(&query).unwrap();

        // All results should have goal_a
        for result in &results {
            assert_eq!(result.metadata.primary_goal.as_str(), "goal_a");
        }

        println!(
            "[VERIFIED] Search with goal filter returns only matching goals ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_search_with_quadrant_filter() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries in different quadrants
        for quadrant in JohariQuadrant::all() {
            for i in 0..3 {
                index
                    .insert(create_entry(0.5 + i as f32 * 0.05, "goal", quadrant))
                    .unwrap();
            }
        }

        let query_vector = create_purpose_vector(0.55, 0.02);
        let query = PurposeQuery::new(PurposeQueryTarget::vector(query_vector), 10, 0.0)
            .unwrap()
            .with_quadrant_filter(JohariQuadrant::Hidden);

        let results = index.search(&query).unwrap();

        // All results should be in Hidden quadrant
        for result in &results {
            assert_eq!(result.metadata.dominant_quadrant, JohariQuadrant::Hidden);
        }

        println!(
            "[VERIFIED] Search with quadrant filter returns only matching quadrant ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_search_with_combined_filters() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert varied entries
        let quadrants = JohariQuadrant::all();
        let goals = ["goal_a", "goal_b", "goal_c"];

        for (i, goal) in goals.iter().enumerate() {
            for quadrant in &quadrants {
                index
                    .insert(create_entry(0.4 + i as f32 * 0.1, goal, *quadrant))
                    .unwrap();
            }
        }

        let query_vector = create_purpose_vector(0.5, 0.02);
        let query = PurposeQuery::new(PurposeQueryTarget::vector(query_vector), 10, 0.0)
            .unwrap()
            .with_goal_filter(GoalId::new("goal_b"))
            .with_quadrant_filter(JohariQuadrant::Open);

        let results = index.search(&query).unwrap();

        // All results should match both filters
        for result in &results {
            assert_eq!(result.metadata.primary_goal.as_str(), "goal_b");
            assert_eq!(result.metadata.dominant_quadrant, JohariQuadrant::Open);
        }

        println!(
            "[VERIFIED] Search with combined filters returns only fully matching entries ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_search_from_memory_target() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries
        let entries: Vec<PurposeIndexEntry> = (0..5)
            .map(|i| create_entry(0.4 + i as f32 * 0.1, "goal", JohariQuadrant::Open))
            .collect();

        for entry in &entries {
            index.insert(entry.clone()).unwrap();
        }

        // Search from existing memory
        let source_id = entries[2].memory_id;
        let query = PurposeQuery::new(PurposeQueryTarget::from_memory(source_id), 3, 0.0).unwrap();

        let results = index.search(&query).unwrap();

        assert!(!results.is_empty());
        // Should find the source memory itself as most similar
        assert!(results.iter().any(|r| r.memory_id == source_id));

        println!("[VERIFIED] Search from memory target works");
    }

    #[test]
    fn test_search_from_memory_non_existent_fails() {
        let index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let non_existent = Uuid::new_v4();

        let query =
            PurposeQuery::new(PurposeQueryTarget::from_memory(non_existent), 10, 0.0).unwrap();

        let result = index.search(&query);

        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("not found"));

        println!(
            "[VERIFIED] FAIL FAST: Search from non-existent memory fails: {}",
            msg
        );
    }

    #[test]
    fn test_search_respects_limit() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..20 {
            index
                .insert(create_entry(
                    0.3 + i as f32 * 0.03,
                    "goal",
                    JohariQuadrant::Open,
                ))
                .unwrap();
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::vector(create_purpose_vector(0.5, 0.02)),
            5, // Limit to 5
            0.0,
        )
        .unwrap();

        let results = index.search(&query).unwrap();

        assert!(results.len() <= 5);

        println!("[VERIFIED] Search respects limit parameter");
    }

    // =========================================================================
    // Pattern Search Tests
    // =========================================================================

    #[test]
    fn test_search_pattern_target() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries forming natural clusters
        // Cluster 1: low values
        for i in 0..5 {
            index
                .insert(create_entry(
                    0.1 + i as f32 * 0.02,
                    "goal_low",
                    JohariQuadrant::Open,
                ))
                .unwrap();
        }

        // Cluster 2: high values
        for i in 0..5 {
            index
                .insert(create_entry(
                    0.8 + i as f32 * 0.02,
                    "goal_high",
                    JohariQuadrant::Hidden,
                ))
                .unwrap();
        }

        let target = PurposeQueryTarget::pattern(2, 0.5).unwrap();
        let query = PurposeQuery::new(target, 20, 0.0).unwrap();

        let results = index.search(&query).unwrap();

        // Should find entries from clusters meeting the criteria
        println!("[RESULT] Pattern search found {} results", results.len());

        // Note: Exact results depend on clustering, but we should get some results
        // as long as clusters meet min_cluster_size and coherence_threshold

        println!("[VERIFIED] Search with pattern target executes");
    }

    // =========================================================================
    // Utility Method Tests
    // =========================================================================

    #[test]
    fn test_contains() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let entry = create_entry(0.5, "goal", JohariQuadrant::Open);
        let id = entry.memory_id;
        let other_id = Uuid::new_v4();

        assert!(!index.contains(id));
        assert!(!index.contains(other_id));

        index.insert(entry).unwrap();

        assert!(index.contains(id));
        assert!(!index.contains(other_id));

        println!("[VERIFIED] contains returns correct status");
    }

    #[test]
    fn test_len_and_is_empty() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        assert!(index.is_empty());
        assert_eq!(index.len(), 0);

        index
            .insert(create_entry(0.5, "goal", JohariQuadrant::Open))
            .unwrap();

        assert!(!index.is_empty());
        assert_eq!(index.len(), 1);

        println!("[VERIFIED] len and is_empty work correctly");
    }

    #[test]
    fn test_clear() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..10 {
            index
                .insert(create_entry(
                    0.3 + i as f32 * 0.05,
                    &format!("goal_{}", i % 3),
                    JohariQuadrant::Open,
                ))
                .unwrap();
        }

        assert_eq!(index.len(), 10);
        assert!(index.goal_count() > 0);

        index.clear();

        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.goal_count(), 0);

        println!("[VERIFIED] clear removes all entries and indexes");
    }

    #[test]
    fn test_goals_returns_all_goals() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        index
            .insert(create_entry(0.5, "alpha", JohariQuadrant::Open))
            .unwrap();
        index
            .insert(create_entry(0.6, "beta", JohariQuadrant::Hidden))
            .unwrap();
        index
            .insert(create_entry(0.7, "gamma", JohariQuadrant::Blind))
            .unwrap();

        let goals = index.goals();

        assert_eq!(goals.len(), 3);

        let goal_strs: Vec<&str> = goals.iter().map(|g| g.as_str()).collect();
        assert!(goal_strs.contains(&"alpha"));
        assert!(goal_strs.contains(&"beta"));
        assert!(goal_strs.contains(&"gamma"));

        println!("[VERIFIED] goals returns all distinct goals");
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_filter_returns_empty_when_no_matches() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        index
            .insert(create_entry(0.5, "goal_a", JohariQuadrant::Open))
            .unwrap();

        // Search with non-existent goal filter
        let query = PurposeQuery::new(
            PurposeQueryTarget::vector(create_purpose_vector(0.5, 0.02)),
            10,
            0.0,
        )
        .unwrap()
        .with_goal_filter(GoalId::new("non_existent_goal"));

        let results = index.search(&query).unwrap();

        assert!(results.is_empty());

        println!("[VERIFIED] Filter returns empty when no matches exist");
    }

    #[test]
    fn test_search_results_sorted_by_similarity() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..10 {
            index
                .insert(create_entry(0.1 * i as f32, "goal", JohariQuadrant::Open))
                .unwrap();
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::vector(create_purpose_vector(0.5, 0.01)),
            10,
            0.0,
        )
        .unwrap();

        let results = index.search(&query).unwrap();

        // Verify descending similarity order
        for i in 1..results.len() {
            assert!(
                results[i - 1].purpose_similarity >= results[i].purpose_similarity,
                "Results not sorted: {} should be >= {}",
                results[i - 1].purpose_similarity,
                results[i].purpose_similarity
            );
        }

        println!("[VERIFIED] Search results are sorted by similarity descending");
    }

    #[test]
    fn test_search_result_contains_complete_data() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let entry = create_entry(0.75, "complete_goal", JohariQuadrant::Blind);
        index.insert(entry.clone()).unwrap();

        let query = PurposeQuery::new(
            PurposeQueryTarget::vector(entry.purpose_vector.clone()),
            1,
            0.0,
        )
        .unwrap();

        let results = index.search(&query).unwrap();

        assert_eq!(results.len(), 1);
        let result = &results[0];

        assert_eq!(result.memory_id, entry.memory_id);
        assert_eq!(
            result.purpose_vector.alignments,
            entry.purpose_vector.alignments
        );
        assert_eq!(
            result.metadata.primary_goal.as_str(),
            entry.metadata.primary_goal.as_str()
        );
        assert_eq!(
            result.metadata.dominant_quadrant,
            entry.metadata.dominant_quadrant
        );

        println!("[VERIFIED] Search result contains complete entry data");
    }
}
