//! Comprehensive integration tests for the Purpose Pattern Index.
//!
//! # Test Philosophy
//!
//! - NO mock data - all tests use real values
//! - Every test prints "[VERIFIED]" on success
//! - Tests verify fail-fast semantics
//! - Full State Verification after complex operations
//!
//! # CRITICAL: NO FALLBACKS
//!
//! All operations are fail-fast. Missing entries cause immediate errors.
//! Invalid queries rejected at construction time.
//!
//! # Test Categories
//!
//! 1. Error Tests - All PurposeIndexError variants
//! 2. Entry Tests - PurposeMetadata and PurposeIndexEntry
//! 3. Query Tests - PurposeQuery builder and validation
//! 4. Clustering Tests - K-means with real 13D vectors
//! 5. HNSW Index Tests - Insert/remove/search cycle
//! 6. Full State Verification Tests - Complete workflow

#[cfg(test)]
mod tests {
    use super::super::clustering::{KMeansConfig, KMeansPurposeClustering, StandardKMeans};
    use super::super::entry::{PurposeIndexEntry, PurposeMetadata};
    use super::super::error::{PurposeIndexError, PurposeIndexResult};
    use super::super::hnsw_purpose::{HnswPurposeIndex, PurposeIndexOps};
    use super::super::query::{PurposeQuery, PurposeQueryTarget, PurposeSearchResult};

    use super::super::entry::GoalId;
    use crate::index::config::{DistanceMetric, HnswConfig, PURPOSE_VECTOR_DIM};
    use crate::index::error::IndexError;
    use crate::types::fingerprint::PurposeVector;
    use crate::types::JohariQuadrant;

    use std::collections::HashSet;
    use std::time::{Duration, SystemTime};
    use uuid::Uuid;

    // =========================================================================
    // HELPER FUNCTIONS - REAL DATA ONLY (NO MOCKS)
    // =========================================================================

    /// Create a purpose vector with deterministic values based on base and variation.
    /// Uses REAL alignment values in range [0.0, 1.0].
    fn create_purpose_vector(base: f32, variation: f32) -> PurposeVector {
        let mut alignments = [0.0f32; PURPOSE_VECTOR_DIM];
        for (i, alignment) in alignments.iter_mut().enumerate() {
            *alignment = (base + (i as f32 * variation)).clamp(0.0, 1.0);
        }
        PurposeVector::new(alignments)
    }

    /// Create metadata with a specific goal and quadrant.
    fn create_metadata(goal: &str, quadrant: JohariQuadrant) -> PurposeMetadata {
        PurposeMetadata::new(GoalId::new(goal), 0.85, quadrant).unwrap()
    }

    /// Create a complete purpose index entry with real data.
    fn create_entry(base: f32, goal: &str, quadrant: JohariQuadrant) -> PurposeIndexEntry {
        let pv = create_purpose_vector(base, 0.02);
        let metadata = create_metadata(goal, quadrant);
        PurposeIndexEntry::new(Uuid::new_v4(), pv, metadata)
    }

    /// Create a purpose index entry with a specific memory ID.
    fn create_entry_with_id(
        memory_id: Uuid,
        base: f32,
        goal: &str,
        quadrant: JohariQuadrant,
    ) -> PurposeIndexEntry {
        let pv = create_purpose_vector(base, 0.02);
        let metadata = create_metadata(goal, quadrant);
        PurposeIndexEntry::new(memory_id, pv, metadata)
    }

    /// Create a default HNSW config for purpose vectors.
    fn purpose_config() -> HnswConfig {
        HnswConfig::new(16, 200, 100, DistanceMetric::Cosine, PURPOSE_VECTOR_DIM)
    }

    /// Create entries forming distinct clusters for clustering tests.
    fn create_clustered_entries() -> Vec<PurposeIndexEntry> {
        let mut entries = Vec::new();

        // Cluster 1: low values (base around 0.2)
        for i in 0..5 {
            entries.push(create_entry(
                0.15 + i as f32 * 0.02,
                "goal_low",
                JohariQuadrant::Open,
            ));
        }

        // Cluster 2: medium values (base around 0.5)
        for i in 0..5 {
            entries.push(create_entry(
                0.45 + i as f32 * 0.02,
                "goal_mid",
                JohariQuadrant::Hidden,
            ));
        }

        // Cluster 3: high values (base around 0.8)
        for i in 0..5 {
            entries.push(create_entry(
                0.75 + i as f32 * 0.02,
                "goal_high",
                JohariQuadrant::Blind,
            ));
        }

        entries
    }

    // =========================================================================
    // ERROR TESTS - All PurposeIndexError variants
    // =========================================================================

    #[test]
    fn test_error_not_found_has_descriptive_message() {
        let id = Uuid::new_v4();
        let err = PurposeIndexError::not_found(id);
        let msg = err.to_string();

        assert!(
            msg.contains(&id.to_string()),
            "Error should contain memory ID"
        );
        assert!(
            msg.contains("not found"),
            "Error should contain 'not found'"
        );

        println!("[VERIFIED] NotFound error contains memory ID: {}", msg);
    }

    #[test]
    fn test_error_invalid_confidence_has_descriptive_message() {
        let err = PurposeIndexError::invalid_confidence(1.5, "test context");
        let msg = err.to_string();

        assert!(msg.contains("1.5"), "Error should contain invalid value");
        assert!(msg.contains("test context"), "Error should contain context");

        println!(
            "[VERIFIED] InvalidConfidence error contains value and context: {}",
            msg
        );
    }

    #[test]
    fn test_error_invalid_query_has_descriptive_message() {
        let err = PurposeIndexError::invalid_query("limit must be positive");
        let msg = err.to_string();

        assert!(
            msg.contains("limit must be positive"),
            "Error should contain reason"
        );

        println!("[VERIFIED] InvalidQuery error contains reason: {}", msg);
    }

    #[test]
    fn test_error_dimension_mismatch_has_descriptive_message() {
        let err = PurposeIndexError::dimension_mismatch(13, 10);
        let msg = err.to_string();

        assert!(
            msg.contains("13"),
            "Error should contain expected dimension"
        );
        assert!(msg.contains("10"), "Error should contain actual dimension");

        println!(
            "[VERIFIED] DimensionMismatch error contains dimensions: {}",
            msg
        );
    }

    #[test]
    fn test_error_clustering_has_descriptive_message() {
        let err = PurposeIndexError::clustering("insufficient data points");
        let msg = err.to_string();

        assert!(
            msg.contains("insufficient data points"),
            "Error should contain reason"
        );

        println!("[VERIFIED] ClusteringError contains reason: {}", msg);
    }

    #[test]
    fn test_error_hnsw_wrapping() {
        let index_err = IndexError::NotFound {
            memory_id: Uuid::new_v4(),
        };
        let purpose_err: PurposeIndexError = index_err.into();
        let msg = purpose_err.to_string();

        assert!(msg.contains("HNSW"), "Error should indicate HNSW source");

        println!("[VERIFIED] HnswError wraps IndexError correctly: {}", msg);
    }

    #[test]
    fn test_error_persistence_has_descriptive_message() {
        let err = PurposeIndexError::persistence("saving index", "disk full");
        let msg = err.to_string();

        assert!(msg.contains("saving index"), "Error should contain context");
        assert!(msg.contains("disk full"), "Error should contain message");

        println!(
            "[VERIFIED] PersistenceError contains context and message: {}",
            msg
        );
    }

    #[test]
    fn test_error_propagation_fail_fast() {
        // Test that errors propagate properly through the result type
        fn inner_operation() -> PurposeIndexResult<()> {
            Err(PurposeIndexError::not_found(Uuid::new_v4()))
        }

        fn outer_operation() -> PurposeIndexResult<String> {
            inner_operation()?; // Should propagate
            Ok("success".to_string())
        }

        let result = outer_operation();
        assert!(result.is_err());

        println!("[VERIFIED] Errors propagate through Result (fail-fast semantics)");
    }

    // =========================================================================
    // ENTRY TESTS - PurposeMetadata and PurposeIndexEntry
    // =========================================================================

    #[test]
    fn test_purpose_metadata_with_real_goal_and_quadrant() {
        let goal = GoalId::new("master_machine_learning");
        let quadrant = JohariQuadrant::Open;

        let metadata = PurposeMetadata::new(goal.clone(), 0.85, quadrant).unwrap();

        assert_eq!(metadata.primary_goal.as_str(), "master_machine_learning");
        assert!((metadata.confidence - 0.85).abs() < f32::EPSILON);
        assert_eq!(metadata.dominant_quadrant, JohariQuadrant::Open);

        // Verify timestamp is recent
        let elapsed = metadata.computed_at.elapsed().unwrap();
        assert!(elapsed < Duration::from_secs(1));

        println!("[VERIFIED] PurposeMetadata construction with real GoalId and JohariQuadrant");
    }

    #[test]
    fn test_purpose_metadata_all_quadrants() {
        for quadrant in JohariQuadrant::all() {
            let metadata = PurposeMetadata::new(GoalId::new("test_goal"), 0.75, quadrant).unwrap();

            assert_eq!(metadata.dominant_quadrant, quadrant);
        }

        println!("[VERIFIED] PurposeMetadata works with all JohariQuadrant variants");
    }

    #[test]
    fn test_purpose_index_entry_with_real_purpose_vector() {
        let alignments = [
            0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76,
        ];
        let pv = PurposeVector::new(alignments);
        let metadata =
            PurposeMetadata::new(GoalId::new("learn_pytorch"), 0.9, JohariQuadrant::Hidden)
                .unwrap();

        let memory_id = Uuid::new_v4();
        let entry = PurposeIndexEntry::new(memory_id, pv, metadata);

        assert_eq!(entry.memory_id, memory_id);
        assert_eq!(entry.purpose_vector.alignments, alignments);
        assert_eq!(entry.metadata.primary_goal.as_str(), "learn_pytorch");

        println!("[VERIFIED] PurposeIndexEntry with real PurposeVector and metadata");
    }

    #[test]
    fn test_entry_dimension_matches_purpose_vector_dim() {
        let entry = create_entry(0.5, "test", JohariQuadrant::Open);

        assert_eq!(entry.get_alignments().len(), PURPOSE_VECTOR_DIM);
        assert_eq!(entry.get_alignments().len(), 13);

        println!(
            "[VERIFIED] Entry alignments have dimension {}",
            PURPOSE_VECTOR_DIM
        );
    }

    #[test]
    fn test_entry_aggregate_alignment_computed_correctly() {
        let uniform = PurposeVector::new([0.75; PURPOSE_VECTOR_DIM]);
        let metadata = create_metadata("test", JohariQuadrant::Open);
        let entry = PurposeIndexEntry::new(Uuid::new_v4(), uniform, metadata);

        let aggregate = entry.aggregate_alignment();
        assert!((aggregate - 0.75).abs() < f32::EPSILON);

        println!(
            "[VERIFIED] Entry aggregate_alignment returns correct mean: {:.4}",
            aggregate
        );
    }

    #[test]
    fn test_entry_validation_rejects_invalid_confidence() {
        // Test confidence > 1.0
        let result = PurposeMetadata::new(GoalId::new("test"), 1.5, JohariQuadrant::Open);
        assert!(result.is_err());

        // Test confidence < 0.0
        let result = PurposeMetadata::new(GoalId::new("test"), -0.1, JohariQuadrant::Open);
        assert!(result.is_err());

        // Test NaN
        let result = PurposeMetadata::new(GoalId::new("test"), f32::NAN, JohariQuadrant::Open);
        assert!(result.is_err());

        println!("[VERIFIED] Entry validation detects invalid confidence values");
    }

    // =========================================================================
    // QUERY TESTS - PurposeQuery builder and validation
    // =========================================================================

    #[test]
    fn test_query_builder_with_validation() {
        let pv = create_purpose_vector(0.7, 0.02);

        let query = PurposeQuery::builder()
            .target(PurposeQueryTarget::Vector(pv))
            .limit(10)
            .min_similarity(0.5)
            .build()
            .unwrap();

        assert_eq!(query.limit, 10);
        assert!((query.min_similarity - 0.5).abs() < f32::EPSILON);

        println!("[VERIFIED] PurposeQuery builder creates valid query");
    }

    #[test]
    fn test_query_all_target_variants() {
        let pv = create_purpose_vector(0.5, 0.02);

        // Vector target
        let target = PurposeQueryTarget::vector(pv.clone());
        assert!(!target.requires_memory_lookup());

        // Pattern target
        let target = PurposeQueryTarget::pattern(5, 0.7).unwrap();
        assert!(!target.requires_memory_lookup());

        // FromMemory target
        let target = PurposeQueryTarget::from_memory(Uuid::new_v4());
        assert!(target.requires_memory_lookup());

        println!("[VERIFIED] All PurposeQueryTarget variants work correctly");
    }

    #[test]
    fn test_query_filter_combinations() {
        let pv = create_purpose_vector(0.5, 0.02);

        // No filters
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, 0.0).unwrap();
        assert!(!query.has_filters());
        assert_eq!(query.filter_count(), 0);

        // Goal filter only
        let query = query.with_goal_filter(GoalId::new("test_goal"));
        assert!(query.has_filters());
        assert_eq!(query.filter_count(), 1);

        // Both filters
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, 0.0)
            .unwrap()
            .with_goal_filter(GoalId::new("goal_a"))
            .with_quadrant_filter(JohariQuadrant::Hidden);
        assert!(query.has_filters());
        assert_eq!(query.filter_count(), 2);

        println!("[VERIFIED] Query filter combinations work correctly");
    }

    #[test]
    fn test_query_rejects_invalid_min_similarity_out_of_range() {
        let pv = create_purpose_vector(0.5, 0.02);

        // Over 1.0
        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, 1.5);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("min_similarity"));

        // Under 0.0
        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 10, -0.1);
        assert!(result.is_err());

        println!(
            "[VERIFIED] FAIL FAST: Query rejects invalid min_similarity: {}",
            msg
        );
    }

    #[test]
    fn test_query_rejects_limit_zero() {
        let pv = create_purpose_vector(0.5, 0.02);

        let result = PurposeQuery::new(PurposeQueryTarget::Vector(pv), 0, 0.5);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("limit"));

        println!("[VERIFIED] FAIL FAST: Query rejects limit=0: {}", msg);
    }

    #[test]
    fn test_query_pattern_rejects_invalid_coherence() {
        // Over 1.0
        let result = PurposeQueryTarget::pattern(5, 1.5);
        assert!(result.is_err());

        // Under 0.0
        let result = PurposeQueryTarget::pattern(5, -0.1);
        assert!(result.is_err());

        println!("[VERIFIED] FAIL FAST: Pattern target rejects invalid coherence_threshold");
    }

    #[test]
    fn test_query_pattern_rejects_zero_cluster_size() {
        let result = PurposeQueryTarget::pattern(0, 0.7);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("min_cluster_size"));

        println!(
            "[VERIFIED] FAIL FAST: Pattern target rejects min_cluster_size=0: {}",
            msg
        );
    }

    // =========================================================================
    // CLUSTERING TESTS - K-means with real 13D vectors
    // =========================================================================

    #[test]
    fn test_kmeans_with_real_13d_purpose_vectors() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let config = KMeansConfig::new(3, 100, 1e-6).unwrap();

        println!("[BEFORE] entries={}, k={}", entries.len(), config.k);

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        println!(
            "[AFTER] clusters={}, iterations={}, WCSS={:.4}",
            result.num_clusters(),
            result.iterations,
            result.wcss
        );

        assert_eq!(result.num_clusters(), 3);
        assert_eq!(result.total_points(), 15);

        for (i, cluster) in result.clusters.iter().enumerate() {
            assert!(!cluster.is_empty(), "Cluster {} should not be empty", i);
            println!(
                "  Cluster {}: {} members, coherence={:.4}",
                i,
                cluster.len(),
                cluster.coherence
            );
        }

        println!("[VERIFIED] K-means with real 13D purpose vectors produces 3 clusters");
    }

    #[test]
    fn test_kmeans_convergence_detection() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let config = KMeansConfig::new(3, 500, 1e-6).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        assert!(
            result.converged,
            "Should converge with well-separated clusters"
        );
        assert!(
            result.iterations < config.max_iterations,
            "Should converge before max_iterations"
        );

        println!(
            "[VERIFIED] Convergence detected at iteration {} < max {}",
            result.iterations, config.max_iterations
        );
    }

    #[test]
    fn test_cluster_coherence_calculation() {
        let clusterer = StandardKMeans::new();

        // Create tightly grouped entries (high coherence expected)
        let entries: Vec<PurposeIndexEntry> = (0..10)
            .map(|i| create_entry(0.5 + i as f32 * 0.005, "tight", JohariQuadrant::Open))
            .collect();

        let config = KMeansConfig::new(1, 100, 1e-6).unwrap();
        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        let coherence = result.clusters[0].coherence;
        assert!(coherence > 0.9, "Tight cluster should have high coherence");

        println!(
            "[VERIFIED] Cluster coherence calculation: {:.4} > 0.9",
            coherence
        );
    }

    #[test]
    fn test_clustering_single_point_edge_case() {
        let clusterer = StandardKMeans::new();
        let entries = vec![create_entry(0.5, "single", JohariQuadrant::Open)];
        let config = KMeansConfig::new(1, 100, 1e-6).unwrap();

        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        assert_eq!(result.num_clusters(), 1);
        assert_eq!(result.total_points(), 1);
        assert!(result.converged);

        println!("[VERIFIED] Single point clustering works (edge case)");
    }

    #[test]
    fn test_clustering_k_greater_than_n_rejected() {
        let clusterer = StandardKMeans::new();
        let entries = vec![create_entry(0.5, "single", JohariQuadrant::Open)];
        let config = KMeansConfig::new(5, 100, 1e-6).unwrap(); // k=5 but only 1 entry

        let result = clusterer.cluster_purposes(&entries, &config);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("k (5)"));

        println!("[VERIFIED] FAIL FAST: Clustering rejects k > n: {}", msg);
    }

    // =========================================================================
    // HNSW INDEX TESTS - Insert/remove/search cycle
    // =========================================================================

    #[test]
    fn test_hnsw_insert_remove_get_cycle() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let entry = create_entry(0.7, "test_goal", JohariQuadrant::Open);
        let memory_id = entry.memory_id;

        println!(
            "[BEFORE] len={}, contains({})={}",
            index.len(),
            memory_id,
            index.contains(memory_id)
        );

        // Insert
        index.insert(entry.clone()).unwrap();
        assert_eq!(index.len(), 1);
        assert!(index.contains(memory_id));

        println!(
            "[AFTER INSERT] len={}, contains({})={}",
            index.len(),
            memory_id,
            index.contains(memory_id)
        );

        // Get
        let retrieved = index.get(memory_id).unwrap();
        assert_eq!(retrieved.memory_id, memory_id);
        assert_eq!(retrieved.metadata.primary_goal.as_str(), "test_goal");

        // Remove
        index.remove(memory_id).unwrap();
        assert_eq!(index.len(), 0);
        assert!(!index.contains(memory_id));

        println!(
            "[AFTER REMOVE] len={}, contains({})={}",
            index.len(),
            memory_id,
            index.contains(memory_id)
        );

        println!("[VERIFIED] Insert/remove/get cycle works correctly");
    }

    #[test]
    fn test_hnsw_search_with_vector_target() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries with varying purpose vectors
        for i in 0..10 {
            let entry = create_entry(0.3 + i as f32 * 0.05, "goal", JohariQuadrant::Open);
            index.insert(entry).unwrap();
        }

        // Search with vector similar to highest entry
        let query_vector = create_purpose_vector(0.75, 0.02);
        let query = PurposeQuery::new(PurposeQueryTarget::Vector(query_vector), 5, 0.0).unwrap();

        let results = index.search(&query).unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 5);

        // Results should be sorted by similarity descending
        for i in 1..results.len() {
            assert!(results[i - 1].purpose_similarity >= results[i].purpose_similarity);
        }

        println!("[VERIFIED] Search with Vector target returns sorted results");
    }

    #[test]
    fn test_hnsw_search_with_pattern_target() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert clustered entries
        let entries = create_clustered_entries();
        for entry in entries {
            index.insert(entry).unwrap();
        }

        let target = PurposeQueryTarget::pattern(2, 0.5).unwrap();
        let query = PurposeQuery::new(target, 20, 0.0).unwrap();

        let results = index.search(&query).unwrap();

        // Pattern search should return some results
        println!("[RESULT] Pattern search found {} results", results.len());

        println!("[VERIFIED] Search with Pattern target executes");
    }

    #[test]
    fn test_hnsw_search_with_from_memory_target() {
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
        // Should find the source itself
        assert!(results.iter().any(|r| r.memory_id == source_id));

        println!("[VERIFIED] Search with FromMemory target finds source memory");
    }

    #[test]
    fn test_hnsw_search_with_goal_filtering() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries with different goals
        for i in 0..10 {
            let goal = if i % 2 == 0 { "goal_a" } else { "goal_b" };
            let entry = create_entry(0.5 + i as f32 * 0.02, goal, JohariQuadrant::Open);
            index.insert(entry).unwrap();
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.55, 0.02)),
            10,
            0.0,
        )
        .unwrap()
        .with_goal_filter(GoalId::new("goal_a"));

        let results = index.search(&query).unwrap();

        for result in &results {
            assert_eq!(result.metadata.primary_goal.as_str(), "goal_a");
        }

        println!(
            "[VERIFIED] Goal filtering returns only matching entries ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_hnsw_search_with_quadrant_filtering() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries in different quadrants
        for quadrant in JohariQuadrant::all() {
            for i in 0..3 {
                let entry = create_entry(0.5 + i as f32 * 0.05, "goal", quadrant);
                index.insert(entry).unwrap();
            }
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.55, 0.02)),
            10,
            0.0,
        )
        .unwrap()
        .with_quadrant_filter(JohariQuadrant::Hidden);

        let results = index.search(&query).unwrap();

        for result in &results {
            assert_eq!(result.metadata.dominant_quadrant, JohariQuadrant::Hidden);
        }

        println!(
            "[VERIFIED] Quadrant filtering returns only matching entries ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_hnsw_search_min_similarity_threshold() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..10 {
            let entry = create_entry(0.1 + i as f32 * 0.08, "goal", JohariQuadrant::Open);
            index.insert(entry).unwrap();
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.9, 0.01)),
            10,
            0.8, // High min_similarity
        )
        .unwrap();

        let results = index.search(&query).unwrap();

        for result in &results {
            assert!(
                result.purpose_similarity >= 0.8,
                "Similarity {} should be >= 0.8",
                result.purpose_similarity
            );
        }

        println!(
            "[VERIFIED] min_similarity threshold filters results ({} passed)",
            results.len()
        );
    }

    #[test]
    fn test_hnsw_search_empty_index() {
        let index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.5, 0.02)),
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
    fn test_hnsw_not_found_error_on_missing_memory() {
        let index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let non_existent = Uuid::new_v4();

        // Get fails on missing
        let result = index.get(non_existent);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not found"));

        println!(
            "[VERIFIED] FAIL FAST: Get fails for non-existent memory: {}",
            msg
        );
    }

    #[test]
    fn test_hnsw_remove_non_existent_fails() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let non_existent = Uuid::new_v4();

        let result = index.remove(non_existent);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not found"));

        println!(
            "[VERIFIED] FAIL FAST: Remove fails for non-existent memory: {}",
            msg
        );
    }

    #[test]
    fn test_hnsw_duplicate_handling_updates_entry() {
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

        // Second insert with same ID (update)
        let entry2 = PurposeIndexEntry::new(
            memory_id,
            create_purpose_vector(0.8, 0.01),
            create_metadata("goal2", JohariQuadrant::Hidden),
        );
        index.insert(entry2).unwrap();

        // Should still have 1 entry, updated
        assert_eq!(index.len(), 1);
        let retrieved = index.get(memory_id).unwrap();
        assert_eq!(retrieved.metadata.primary_goal.as_str(), "goal2");
        assert_eq!(retrieved.metadata.dominant_quadrant, JohariQuadrant::Hidden);

        println!("[VERIFIED] Duplicate handling updates existing entry");
    }

    #[test]
    fn test_hnsw_search_from_memory_non_existent_fails() {
        let index = HnswPurposeIndex::new(purpose_config()).unwrap();
        let non_existent = Uuid::new_v4();

        let query =
            PurposeQuery::new(PurposeQueryTarget::from_memory(non_existent), 10, 0.0).unwrap();
        let result = index.search(&query);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not found"));

        println!(
            "[VERIFIED] FAIL FAST: FromMemory search fails for non-existent memory: {}",
            msg
        );
    }

    // =========================================================================
    // FULL STATE VERIFICATION TESTS
    // =========================================================================

    #[test]
    fn test_full_state_verification_complete_workflow() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Initial state
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.goal_count(), 0);
        println!(
            "[STATE] Initial: empty={}, len={}, goals={}",
            index.is_empty(),
            index.len(),
            index.goal_count()
        );

        // Step 1: Insert multiple entries with diverse goals/quadrants
        let entries: Vec<PurposeIndexEntry> = vec![
            create_entry_with_id(Uuid::new_v4(), 0.3, "goal_alpha", JohariQuadrant::Open),
            create_entry_with_id(Uuid::new_v4(), 0.4, "goal_alpha", JohariQuadrant::Hidden),
            create_entry_with_id(Uuid::new_v4(), 0.5, "goal_beta", JohariQuadrant::Open),
            create_entry_with_id(Uuid::new_v4(), 0.6, "goal_beta", JohariQuadrant::Blind),
            create_entry_with_id(Uuid::new_v4(), 0.7, "goal_gamma", JohariQuadrant::Unknown),
        ];

        let ids: Vec<Uuid> = entries.iter().map(|e| e.memory_id).collect();

        for entry in &entries {
            index.insert(entry.clone()).unwrap();
        }

        // Verify after inserts
        assert_eq!(index.len(), 5);
        assert_eq!(index.goal_count(), 3); // alpha, beta, gamma
        for id in &ids {
            assert!(index.contains(*id));
        }
        println!(
            "[STATE] After 5 inserts: len={}, goals={}",
            index.len(),
            index.goal_count()
        );

        // Step 2: Verify secondary indexes updated
        let alpha_set = index.get_by_goal(&GoalId::new("goal_alpha")).unwrap();
        assert_eq!(alpha_set.len(), 2);
        let open_set = index.get_by_quadrant(JohariQuadrant::Open).unwrap();
        assert_eq!(open_set.len(), 2);
        println!(
            "[STATE] Secondary indexes: goal_alpha={}, quadrant_open={}",
            alpha_set.len(),
            open_set.len()
        );

        // Step 3: Search with each query type
        // Vector search
        let vector_query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.5, 0.02)),
            5,
            0.0,
        )
        .unwrap();
        let vector_results = index.search(&vector_query).unwrap();
        assert_eq!(vector_results.len(), 5);
        println!("[STATE] Vector search: {} results", vector_results.len());

        // FromMemory search
        let from_memory_query =
            PurposeQuery::new(PurposeQueryTarget::from_memory(ids[2]), 3, 0.0).unwrap();
        let from_memory_results = index.search(&from_memory_query).unwrap();
        assert!(!from_memory_results.is_empty());
        println!(
            "[STATE] FromMemory search: {} results",
            from_memory_results.len()
        );

        // Filtered search
        let filtered_query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.5, 0.02)),
            10,
            0.0,
        )
        .unwrap()
        .with_goal_filter(GoalId::new("goal_beta"));
        let filtered_results = index.search(&filtered_query).unwrap();
        assert_eq!(filtered_results.len(), 2);
        println!(
            "[STATE] Filtered search (goal_beta): {} results",
            filtered_results.len()
        );

        // Step 4: Remove entries and verify cleanup
        let to_remove = ids[0]; // goal_alpha, Open
        index.remove(to_remove).unwrap();

        assert_eq!(index.len(), 4);
        assert!(!index.contains(to_remove));
        let alpha_set = index.get_by_goal(&GoalId::new("goal_alpha")).unwrap();
        assert_eq!(alpha_set.len(), 1); // Reduced from 2 to 1
        let open_set = index.get_by_quadrant(JohariQuadrant::Open).unwrap();
        assert_eq!(open_set.len(), 1); // Reduced from 2 to 1
        println!(
            "[STATE] After remove: len={}, goal_alpha={}, quadrant_open={}",
            index.len(),
            alpha_set.len(),
            open_set.len()
        );

        // Step 5: Clear and verify
        index.clear();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert_eq!(index.goal_count(), 0);
        println!(
            "[STATE] After clear: empty={}, len={}, goals={}",
            index.is_empty(),
            index.len(),
            index.goal_count()
        );

        println!("[VERIFIED] Full state verification complete - all data structures consistent");
    }

    #[test]
    fn test_full_state_secondary_indexes_consistency() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert entries
        let entries = vec![
            create_entry_with_id(Uuid::new_v4(), 0.3, "shared_goal", JohariQuadrant::Open),
            create_entry_with_id(Uuid::new_v4(), 0.5, "shared_goal", JohariQuadrant::Open),
            create_entry_with_id(Uuid::new_v4(), 0.7, "unique_goal", JohariQuadrant::Unknown),
        ];

        let ids: Vec<Uuid> = entries.iter().map(|e| e.memory_id).collect();
        for entry in entries {
            index.insert(entry).unwrap();
        }

        // Verify initial state
        assert_eq!(index.goal_count(), 2);
        assert!(index.get_by_goal(&GoalId::new("shared_goal")).is_some());
        assert!(index.get_by_goal(&GoalId::new("unique_goal")).is_some());

        // Remove one shared_goal entry
        index.remove(ids[0]).unwrap();
        let shared_set = index.get_by_goal(&GoalId::new("shared_goal")).unwrap();
        assert_eq!(shared_set.len(), 1); // Still exists with 1 member

        // Remove the unique_goal entry
        index.remove(ids[2]).unwrap();
        assert!(index.get_by_goal(&GoalId::new("unique_goal")).is_none()); // Empty set removed
        assert_eq!(index.goal_count(), 1);

        // Remove the last shared_goal entry
        index.remove(ids[1]).unwrap();
        assert!(index.get_by_goal(&GoalId::new("shared_goal")).is_none()); // Empty set removed
        assert_eq!(index.goal_count(), 0);

        println!("[VERIFIED] Secondary indexes cleaned up correctly on removal");
    }

    #[test]
    fn test_full_state_results_contain_complete_data() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let alignments = [
            0.8, 0.7, 0.9, 0.6, 0.75, 0.65, 0.85, 0.72, 0.78, 0.68, 0.82, 0.71, 0.76,
        ];
        let pv = PurposeVector::new(alignments);
        let metadata =
            PurposeMetadata::new(GoalId::new("complete_test"), 0.95, JohariQuadrant::Blind)
                .unwrap();
        let entry = PurposeIndexEntry::new(Uuid::new_v4(), pv.clone(), metadata);
        let memory_id = entry.memory_id;

        index.insert(entry).unwrap();

        let query = PurposeQuery::new(PurposeQueryTarget::Vector(pv.clone()), 1, 0.0).unwrap();

        let results = index.search(&query).unwrap();
        assert_eq!(results.len(), 1);

        let result = &results[0];
        assert_eq!(result.memory_id, memory_id);
        assert_eq!(result.purpose_vector.alignments, alignments);
        assert_eq!(result.metadata.primary_goal.as_str(), "complete_test");
        assert!((result.metadata.confidence - 0.95).abs() < f32::EPSILON);
        assert_eq!(result.metadata.dominant_quadrant, JohariQuadrant::Blind);

        println!("[VERIFIED] Search results contain complete entry data");
    }

    // =========================================================================
    // ADDITIONAL EDGE CASE TESTS
    // =========================================================================

    #[test]
    fn test_index_with_wrong_dimension_config_fails() {
        let wrong_config = HnswConfig::new(16, 200, 100, DistanceMetric::Cosine, 100);
        let result = HnswPurposeIndex::new(wrong_config);

        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("13"));
        assert!(msg.contains("100"));

        println!(
            "[VERIFIED] FAIL FAST: Index rejects wrong dimension config: {}",
            msg
        );
    }

    #[test]
    fn test_search_results_sorted_by_similarity_descending() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..10 {
            let entry = create_entry(0.1 * i as f32, "goal", JohariQuadrant::Open);
            index.insert(entry).unwrap();
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.5, 0.01)),
            10,
            0.0,
        )
        .unwrap();

        let results = index.search(&query).unwrap();

        for i in 1..results.len() {
            assert!(
                results[i - 1].purpose_similarity >= results[i].purpose_similarity,
                "Results should be sorted by similarity descending"
            );
        }

        println!("[VERIFIED] Search results sorted by similarity descending");
    }

    #[test]
    fn test_filter_returns_empty_when_no_matches() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        let entry = create_entry(0.5, "existing_goal", JohariQuadrant::Open);
        index.insert(entry).unwrap();

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.5, 0.02)),
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
    fn test_search_respects_limit() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        for i in 0..20 {
            let entry = create_entry(0.3 + i as f32 * 0.03, "goal", JohariQuadrant::Open);
            index.insert(entry).unwrap();
        }

        let query = PurposeQuery::new(
            PurposeQueryTarget::Vector(create_purpose_vector(0.5, 0.02)),
            5, // Limit to 5
            0.0,
        )
        .unwrap();

        let results = index.search(&query).unwrap();

        assert!(results.len() <= 5);

        println!(
            "[VERIFIED] Search respects limit parameter ({} results)",
            results.len()
        );
    }

    #[test]
    fn test_goals_list_returns_all_goals() {
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
        let goal_strs: HashSet<&str> = goals.iter().map(|g| g.as_str()).collect();
        assert!(goal_strs.contains("alpha"));
        assert!(goal_strs.contains("beta"));
        assert!(goal_strs.contains("gamma"));

        println!("[VERIFIED] goals() returns all distinct goals");
    }

    #[test]
    fn test_kmeans_config_boundary_values() {
        // Valid boundary k=1
        let config = KMeansConfig::with_k(1).unwrap();
        assert_eq!(config.k, 1);

        // Valid large max_iterations
        let config = KMeansConfig::new(5, 10000, 1e-6).unwrap();
        assert_eq!(config.max_iterations, 10000);

        // Valid small convergence_threshold
        let config = KMeansConfig::new(5, 100, 1e-10).unwrap();
        assert!(config.convergence_threshold > 0.0);

        println!("[VERIFIED] KMeansConfig accepts valid boundary values");
    }

    #[test]
    fn test_clustering_preserves_all_memory_ids() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();
        let original_ids: HashSet<Uuid> = entries.iter().map(|e| e.memory_id).collect();

        let config = KMeansConfig::new(3, 100, 1e-6).unwrap();
        let result = clusterer.cluster_purposes(&entries, &config).unwrap();

        let clustered_ids: HashSet<Uuid> = result
            .clusters
            .iter()
            .flat_map(|c| c.members.iter().copied())
            .collect();

        assert_eq!(original_ids, clustered_ids);

        println!("[VERIFIED] Clustering preserves all memory IDs");
    }

    #[test]
    fn test_purpose_search_result_matches_methods() {
        let pv = create_purpose_vector(0.8, 0.02);
        let metadata = create_metadata("test_goal", JohariQuadrant::Hidden);
        let result = PurposeSearchResult::new(Uuid::new_v4(), 0.95, pv, metadata);

        assert!(result.matches_goal(&GoalId::new("test_goal")));
        assert!(!result.matches_goal(&GoalId::new("other_goal")));
        assert!(result.matches_quadrant(JohariQuadrant::Hidden));
        assert!(!result.matches_quadrant(JohariQuadrant::Open));

        println!("[VERIFIED] PurposeSearchResult matches_* methods work correctly");
    }

    #[test]
    fn test_entry_stale_detection() {
        let past = SystemTime::now() - Duration::from_secs(3600);
        let metadata =
            PurposeMetadata::with_timestamp(GoalId::new("test"), 0.75, past, JohariQuadrant::Open)
                .unwrap();

        let entry = PurposeIndexEntry::new(Uuid::new_v4(), PurposeVector::default(), metadata);

        // Entry is 1 hour old
        assert!(entry.is_stale(Duration::from_secs(1800))); // 30 min threshold
        assert!(!entry.is_stale(Duration::from_secs(7200))); // 2 hour threshold

        println!("[VERIFIED] Entry stale detection works correctly");
    }

    #[test]
    fn test_multiple_inserts_and_removes() {
        let mut index = HnswPurposeIndex::new(purpose_config()).unwrap();

        // Insert 20 entries
        let mut ids: Vec<Uuid> = Vec::new();
        for i in 0..20 {
            let entry = create_entry(0.3 + i as f32 * 0.03, "goal", JohariQuadrant::Open);
            ids.push(entry.memory_id);
            index.insert(entry).unwrap();
        }
        assert_eq!(index.len(), 20);

        // Remove every other entry
        for i in (0..20).step_by(2) {
            index.remove(ids[i]).unwrap();
        }
        assert_eq!(index.len(), 10);

        // Verify remaining entries exist
        for i in (1..20).step_by(2) {
            assert!(index.contains(ids[i]));
        }

        // Verify removed entries don't exist
        for i in (0..20).step_by(2) {
            assert!(!index.contains(ids[i]));
        }

        println!("[VERIFIED] Multiple inserts and removes maintain consistency");
    }

    #[test]
    fn test_clustering_wcss_decreases_with_more_clusters() {
        let clusterer = StandardKMeans::new();
        let entries = create_clustered_entries();

        let result_k1 = clusterer
            .cluster_purposes(&entries, &KMeansConfig::with_k(1).unwrap())
            .unwrap();

        let result_k2 = clusterer
            .cluster_purposes(&entries, &KMeansConfig::with_k(2).unwrap())
            .unwrap();

        let result_k3 = clusterer
            .cluster_purposes(&entries, &KMeansConfig::with_k(3).unwrap())
            .unwrap();

        // WCSS should decrease or stay same as k increases
        assert!(result_k1.wcss >= result_k2.wcss);
        assert!(result_k2.wcss >= result_k3.wcss);

        println!(
            "[VERIFIED] WCSS decreases with increasing k: k1={:.4}, k2={:.4}, k3={:.4}",
            result_k1.wcss, result_k2.wcss, result_k3.wcss
        );
    }
}
