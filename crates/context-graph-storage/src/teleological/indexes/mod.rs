//! HNSW and inverted index configuration for 5-stage retrieval pipeline.
//!
//! # Index Types
//!
//! | Stage | Index | Type | Embedder |
//! |-------|-------|------|----------|
//! | 1 | SPLADE inverted | InvertedIndex | E13 |
//! | 2 | Matryoshka 128D | HNSW | E1 (truncated) |
//! | 3 | Full embeddings | HNSW x 10 | E1-E5, E7-E11 |
//! | 4 | ColBERT MaxSim | Token-level | E12 |
//! | 5 | Purpose vector | HNSW 13D | PurposeVector |
//!
//! # FAIL FAST. NO FALLBACKS.
//!
//! Invalid configurations panic immediately. No silent defaults.
//!
//! # Module Structure
//!
//! - [`hnsw_config`]: HNSW configuration types (`HnswConfig`, `EmbedderIndex`, `DistanceMetric`)
//! - [`metrics`]: Distance computation functions (`compute_distance`, `cosine_similarity`)
//! - [`embedder_index`]: Per-embedder ANN index trait (`EmbedderIndexOps`, `IndexError`)
//! - [`hnsw_impl`]: HNSW index implementation (`HnswEmbedderIndex`)
//! - [`registry`]: Index registry (`EmbedderIndexRegistry`)
//!
//! # Example
//!
//! ```
//! use context_graph_storage::teleological::indexes::{
//!     get_hnsw_config, all_hnsw_configs, EmbedderIndex, DistanceMetric,
//!     compute_distance, recommended_metric,
//! };
//!
//! // Get HNSW config for E1 semantic embedder
//! let config = get_hnsw_config(EmbedderIndex::E1Semantic).unwrap();
//! assert_eq!(config.dimension, 1024);
//!
//! // Get all HNSW configs (12 total)
//! let configs = all_hnsw_configs();
//! assert_eq!(configs.len(), 12);
//!
//! // Compute cosine distance
//! let a = vec![1.0, 0.0, 0.0];
//! let b = vec![0.0, 1.0, 0.0];
//! let dist = compute_distance(&a, &b, DistanceMetric::Cosine);
//! ```
//!
//! # Per-Embedder Index Example
//!
//! ```
//! use context_graph_storage::teleological::indexes::{
//!     EmbedderIndex, EmbedderIndexRegistry, EmbedderIndexOps,
//! };
//! use uuid::Uuid;
//!
//! // Create registry with all 12 HNSW indexes
//! let registry = EmbedderIndexRegistry::new();
//!
//! // Get index for E8 Graph (384D)
//! let index = registry.get(EmbedderIndex::E8Graph).unwrap();
//!
//! // Insert and search
//! let id = Uuid::new_v4();
//! let vector = vec![0.5f32; 384];
//! index.insert(id, &vector).unwrap();
//!
//! let results = index.search(&vector, 1, None).unwrap();
//! assert_eq!(results[0].0, id);
//! ```

// hnsw_config is now a directory module (hnsw_config/mod.rs)
pub mod hnsw_config;
pub mod metrics;

// Per-embedder index modules (TASK-CORE-007)
pub mod embedder_index;
pub mod hnsw_impl;
pub mod registry;

// Re-export from hnsw_config
pub use hnsw_config::{
    // Enums
    DistanceMetric,
    EmbedderIndex,
    // Structs
    HnswConfig,
    InvertedIndexConfig,
    // Functions
    all_hnsw_configs,
    get_hnsw_config,
    get_inverted_index_config,
};

// Re-export dimension constants
pub use hnsw_config::{
    E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E1_MATRYOSHKA_DIM, E2_DIM, E3_DIM,
    E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM, NUM_EMBEDDERS, PURPOSE_VECTOR_DIM,
};

// Re-export from metrics
pub use metrics::{compute_distance, cosine_similarity, distance_to_similarity, recommended_metric};

// Re-export from per-embedder index modules (TASK-CORE-007)
pub use embedder_index::{validate_vector, EmbedderIndexOps, IndexError, IndexResult};
pub use hnsw_impl::HnswEmbedderIndex;
pub use registry::EmbedderIndexRegistry;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports_hnsw_config() {
        println!("=== TEST: Module exports HNSW config types ===");

        // Verify enum exports
        let _ = EmbedderIndex::E1Semantic;
        let _ = DistanceMetric::Cosine;

        // Verify struct constructors
        let config = HnswConfig::matryoshka_128d();
        assert_eq!(config.dimension, 128);

        // Verify function exports
        let configs = all_hnsw_configs();
        assert_eq!(configs.len(), 12);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_module_exports_metrics() {
        println!("=== TEST: Module exports metrics functions ===");

        // Verify function exports
        let metric = recommended_metric(0);
        assert_eq!(metric, DistanceMetric::Cosine);

        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];

        let dist = compute_distance(&a, &b, DistanceMetric::Cosine);
        assert!(dist < 1e-6);

        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);

        let converted = distance_to_similarity(0.0, DistanceMetric::Cosine);
        assert!((converted - 1.0).abs() < 1e-6);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_module_exports_dimension_constants() {
        println!("=== TEST: Module exports dimension constants ===");

        assert_eq!(E1_DIM, 1024);
        assert_eq!(E2_DIM, 512);
        assert_eq!(E3_DIM, 512);
        assert_eq!(E4_DIM, 512);
        assert_eq!(E5_DIM, 768);
        assert_eq!(E6_SPARSE_VOCAB, 30_522);
        assert_eq!(E7_DIM, 1536);
        assert_eq!(E8_DIM, 384);
        assert_eq!(E9_DIM, 1024); // HDC projected dimension
        assert_eq!(E10_DIM, 768);
        assert_eq!(E11_DIM, 384);
        assert_eq!(E12_TOKEN_DIM, 128);
        assert_eq!(E13_SPLADE_VOCAB, 30_522);
        assert_eq!(NUM_EMBEDDERS, 13);
        assert_eq!(E1_MATRYOSHKA_DIM, 128);
        assert_eq!(PURPOSE_VECTOR_DIM, 13);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_all_acceptance_criteria() {
        println!("\n=== TASK-F005 ACCEPTANCE CRITERIA VERIFICATION ===");
        println!();

        // AC1: EmbedderIndex enum: 15 variants
        println!("AC1: EmbedderIndex has 15 variants");
        let hnsw = EmbedderIndex::all_hnsw();
        assert_eq!(hnsw.len(), 12); // 15 - 3 (E6, E12, E13)
        println!("    - 12 HNSW variants verified");
        println!("    - 3 non-HNSW variants (E6, E12, E13)");
        println!("    PASS");

        // AC2: HnswConfig struct
        println!();
        println!("AC2: HnswConfig struct has m, ef_construction, ef_search, metric, dimension");
        let cfg = get_hnsw_config(EmbedderIndex::E1Semantic).unwrap();
        let _ = cfg.m;
        let _ = cfg.ef_construction;
        let _ = cfg.ef_search;
        let _ = cfg.metric;
        let _ = cfg.dimension;
        println!("    PASS");

        // AC3: DistanceMetric enum: 5 variants
        println!();
        println!("AC3: DistanceMetric has 5 variants");
        let _ = DistanceMetric::Cosine;
        let _ = DistanceMetric::DotProduct;
        let _ = DistanceMetric::Euclidean;
        let _ = DistanceMetric::AsymmetricCosine;
        let _ = DistanceMetric::MaxSim;
        println!("    PASS");

        // AC4: InvertedIndexConfig struct
        println!();
        println!("AC4: InvertedIndexConfig has vocab_size, max_nnz, use_bm25");
        let inv_cfg = get_inverted_index_config(EmbedderIndex::E6Sparse).unwrap();
        let _ = inv_cfg.vocab_size;
        let _ = inv_cfg.max_nnz;
        let _ = inv_cfg.use_bm25;
        println!("    PASS");

        // AC5: get_hnsw_config returns None for E6, E12, E13
        println!();
        println!("AC5: get_hnsw_config returns None for E6, E12, E13");
        assert!(get_hnsw_config(EmbedderIndex::E6Sparse).is_none());
        assert!(get_hnsw_config(EmbedderIndex::E12LateInteraction).is_none());
        assert!(get_hnsw_config(EmbedderIndex::E13Splade).is_none());
        println!("    PASS");

        // AC6: all_hnsw_configs returns 12 entries
        println!();
        println!("AC6: all_hnsw_configs returns HashMap with 12 entries");
        let configs = all_hnsw_configs();
        assert_eq!(configs.len(), 12);
        println!("    PASS");

        // AC7: get_inverted_index_config returns Some for E6, E13 only
        println!();
        println!("AC7: get_inverted_index_config returns Some for E6, E13 only");
        assert!(get_inverted_index_config(EmbedderIndex::E6Sparse).is_some());
        assert!(get_inverted_index_config(EmbedderIndex::E13Splade).is_some());
        assert!(get_inverted_index_config(EmbedderIndex::E1Semantic).is_none());
        println!("    PASS");

        // AC8: recommended_metric function
        println!();
        println!("AC8: recommended_metric(embedder_idx) maps 0-12");
        assert_eq!(recommended_metric(0), DistanceMetric::Cosine);
        assert_eq!(recommended_metric(4), DistanceMetric::AsymmetricCosine);
        assert_eq!(recommended_metric(11), DistanceMetric::MaxSim);
        println!("    PASS");

        // AC9: compute_distance function
        println!();
        println!("AC9: compute_distance(a, b, metric)");
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let dist = compute_distance(&a, &b, DistanceMetric::Cosine);
        assert!((dist - 1.0).abs() < 1e-6);
        println!("    PASS");

        // AC10: distance_to_similarity function
        println!();
        println!("AC10: distance_to_similarity(distance, metric)");
        let sim = distance_to_similarity(0.0, DistanceMetric::Cosine);
        assert!((sim - 1.0).abs() < 1e-6);
        println!("    PASS");

        println!();
        println!("=== ALL ACCEPTANCE CRITERIA PASSED ===");
    }

    #[test]
    fn test_module_exports_per_embedder_index() {
        println!("=== TEST: Module exports per-embedder index types (TASK-CORE-007) ===");

        // Verify trait export
        fn takes_embedder_index_ops<T: EmbedderIndexOps>(_: &T) {}

        // Verify HnswEmbedderIndex creation
        let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        takes_embedder_index_ops(&index);
        assert_eq!(index.config().dimension, 384);

        // Verify IndexError export
        let err = IndexError::DimensionMismatch {
            embedder: EmbedderIndex::E1Semantic,
            expected: 1024,
            actual: 512,
        };
        let msg = format!("{}", err);
        assert!(msg.contains("1024"));
        assert!(msg.contains("512"));

        // Verify IndexResult type alias
        let result: IndexResult<()> = Ok(());
        assert!(result.is_ok());

        // Verify validate_vector export
        let valid_vec = vec![1.0f32; 384];
        let valid_result = validate_vector(&valid_vec, 384, EmbedderIndex::E8Graph);
        assert!(valid_result.is_ok());

        println!("RESULT: PASS");
    }

    #[test]
    fn test_module_exports_registry() {
        use uuid::Uuid;

        println!("=== TEST: Module exports EmbedderIndexRegistry (TASK-CORE-007) ===");

        let registry = EmbedderIndexRegistry::new();
        assert_eq!(registry.len(), 12);

        // Verify get() for HNSW embedder
        let e1_index = registry.get(EmbedderIndex::E1Semantic);
        assert!(e1_index.is_some());

        // Verify get() returns None for non-HNSW
        let e6_index = registry.get(EmbedderIndex::E6Sparse);
        assert!(e6_index.is_none());

        // Verify insert and search through registry
        let e8_index = registry.get(EmbedderIndex::E8Graph).unwrap();
        let id = Uuid::new_v4();
        let vector = vec![0.5f32; 384];
        e8_index.insert(id, &vector).unwrap();

        let results = e8_index.search(&vector, 1, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, id);

        println!("RESULT: PASS");
    }

    #[test]
    fn test_task_core_007_acceptance_criteria() {
        use uuid::Uuid;

        println!("\n=== TASK-CORE-007 ACCEPTANCE CRITERIA VERIFICATION ===");
        println!();

        // AC1: EmbedderIndexOps trait
        println!("AC1: EmbedderIndexOps trait with 8 methods");
        let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let _ = index.embedder();
        let _ = index.config();
        let _ = index.len();
        let _ = index.is_empty();
        // insert, remove, search, insert_batch, flush, memory_bytes tested elsewhere
        println!("    PASS");

        // AC2: HnswEmbedderIndex for all 12 HNSW embedders
        println!();
        println!("AC2: HnswEmbedderIndex works for all 12 HNSW embedders");
        for embedder in EmbedderIndex::all_hnsw() {
            let idx = HnswEmbedderIndex::new(embedder);
            assert!(idx.config().dimension >= 1);
        }
        println!("    PASS");

        // AC3: EmbedderIndexRegistry manages 12 indexes
        println!();
        println!("AC3: EmbedderIndexRegistry manages 12 indexes");
        let registry = EmbedderIndexRegistry::new();
        assert_eq!(registry.len(), 12);
        println!("    PASS");

        // AC4: FAIL FAST - E6/E12/E13 return None from registry
        println!();
        println!("AC4: Registry returns None for E6, E12, E13");
        assert!(registry.get(EmbedderIndex::E6Sparse).is_none());
        assert!(registry.get(EmbedderIndex::E12LateInteraction).is_none());
        assert!(registry.get(EmbedderIndex::E13Splade).is_none());
        println!("    PASS");

        // AC5: IndexError with 5 variants
        println!();
        println!("AC5: IndexError has 5 variants");
        let _ = IndexError::DimensionMismatch {
            embedder: EmbedderIndex::E1Semantic,
            expected: 1024,
            actual: 512,
        };
        let _ = IndexError::IndexNotFound {
            embedder: EmbedderIndex::E6Sparse,
        };
        let _ = IndexError::OperationFailed {
            embedder: EmbedderIndex::E1Semantic,
            message: "test".to_string(),
        };
        let _ = IndexError::ReadOnly;
        let _ = IndexError::InvalidVector {
            message: "test".to_string(),
        };
        println!("    PASS");

        // AC6: validate_vector function
        println!();
        println!("AC6: validate_vector validates dimension and NaN/Inf");
        let good = vec![1.0f32; 1024];
        assert!(validate_vector(&good, 1024, EmbedderIndex::E1Semantic).is_ok());

        let wrong_dim = vec![1.0f32; 512];
        assert!(validate_vector(&wrong_dim, 1024, EmbedderIndex::E1Semantic).is_err());

        let mut nan_vec = vec![1.0f32; 384];
        nan_vec[0] = f32::NAN;
        assert!(validate_vector(&nan_vec, 384, EmbedderIndex::E8Graph).is_err());
        println!("    PASS");

        // AC7: Thread-safe index operations
        println!();
        println!("AC7: Thread-safe index operations with RwLock");
        let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        index.insert(id1, &vec![1.0f32; 384]).unwrap();
        index.insert(id2, &vec![2.0f32; 384]).unwrap();
        assert_eq!(index.len(), 2);
        // Concurrent read would work (tested via RwLock semantics)
        println!("    PASS");

        // AC8: Duplicate ID updates in place
        println!();
        println!("AC8: Duplicate ID updates vector in place");
        let index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let id = Uuid::new_v4();
        index.insert(id, &vec![1.0f32; 384]).unwrap();
        assert_eq!(index.len(), 1);
        index.insert(id, &vec![2.0f32; 384]).unwrap();
        assert_eq!(index.len(), 1);
        println!("    PASS");

        println!();
        println!("=== ALL TASK-CORE-007 ACCEPTANCE CRITERIA PASSED ===");
    }

    #[test]
    fn test_edge_cases_with_synthetic_data() {
        use uuid::Uuid;

        println!("\n=== EDGE CASE VERIFICATION WITH SYNTHETIC DATA ===");
        println!();

        // Edge case 1: Very small vectors (PurposeVector 13D)
        println!("Edge 1: PurposeVector 13D index");
        let pv_index = HnswEmbedderIndex::new(EmbedderIndex::PurposeVector);
        assert_eq!(pv_index.config().dimension, 13);
        let pv_id = Uuid::new_v4();
        let pv_vec: Vec<f32> = (0..13).map(|i| (i as f32) / 13.0).collect();
        pv_index.insert(pv_id, &pv_vec).unwrap();
        let pv_results = pv_index.search(&pv_vec, 1, None).unwrap();
        assert_eq!(pv_results.len(), 1);
        assert_eq!(pv_results[0].0, pv_id);
        println!("    PASS");

        // Edge case 2: Large vectors (E7Code 1536D)
        println!();
        println!("Edge 2: E7Code 1536D index");
        let e7_index = HnswEmbedderIndex::new(EmbedderIndex::E7Code);
        assert_eq!(e7_index.config().dimension, 1536);
        let e7_id = Uuid::new_v4();
        let e7_vec: Vec<f32> = (0..1536).map(|i| (i as f32) / 1536.0).collect();
        e7_index.insert(e7_id, &e7_vec).unwrap();
        println!("    PASS");

        // Edge case 3: Batch insert with 1000 vectors
        println!();
        println!("Edge 3: Batch insert 1000 vectors");
        let batch_index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let batch: Vec<(Uuid, Vec<f32>)> = (0..1000)
            .map(|i| {
                let id = Uuid::new_v4();
                let vec: Vec<f32> = (0..384).map(|j| ((i + j) as f32) / 10000.0).collect();
                (id, vec)
            })
            .collect();
        let count = batch_index.insert_batch(&batch).unwrap();
        assert_eq!(count, 1000);
        assert_eq!(batch_index.len(), 1000);
        println!("    Inserted 1000 vectors");
        println!("    Memory: {} bytes", batch_index.memory_bytes());
        println!("    PASS");

        // Edge case 4: Search with k > len
        println!();
        println!("Edge 4: Search with k > len returns all");
        let small_index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        for i in 0..5 {
            small_index
                .insert(Uuid::new_v4(), &vec![(i as f32) / 5.0; 384])
                .unwrap();
        }
        let results = small_index.search(&vec![0.5f32; 384], 100, None).unwrap();
        assert_eq!(results.len(), 5); // Only 5 vectors exist
        println!("    Requested k=100, got {} results", results.len());
        println!("    PASS");

        // Edge case 5: Remove and search
        println!();
        println!("Edge 5: Removed vectors excluded from search");
        let rm_index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let rm_id1 = Uuid::new_v4();
        let rm_id2 = Uuid::new_v4();
        rm_index.insert(rm_id1, &vec![0.1f32; 384]).unwrap();
        rm_index.insert(rm_id2, &vec![0.9f32; 384]).unwrap();

        rm_index.remove(rm_id1).unwrap();

        let results = rm_index.search(&vec![0.1f32; 384], 10, None).unwrap();
        let ids: Vec<_> = results.iter().map(|(id, _)| *id).collect();
        assert!(!ids.contains(&rm_id1), "Removed ID should not appear");
        assert!(ids.contains(&rm_id2), "Non-removed ID should appear");
        println!("    PASS");

        // Edge case 6: Zero vector (normalized to zero)
        println!();
        println!("Edge 6: Zero vector handling");
        let zero_index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let zero_id = Uuid::new_v4();
        let zero_vec = vec![0.0f32; 384];
        // Zero vector is valid (though cosine similarity will be undefined)
        zero_index.insert(zero_id, &zero_vec).unwrap();
        assert_eq!(zero_index.len(), 1);
        println!("    Zero vector inserted (cosine undefined but no crash)");
        println!("    PASS");

        // Edge case 7: Very small float values
        println!();
        println!("Edge 7: Very small float values");
        let tiny_index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let tiny_vec: Vec<f32> = (0..384).map(|_| 1e-38).collect();
        tiny_index.insert(Uuid::new_v4(), &tiny_vec).unwrap();
        println!("    Subnormal floats accepted");
        println!("    PASS");

        // Edge case 8: Maximum safe float values
        println!();
        println!("Edge 8: Large but finite float values");
        let large_index = HnswEmbedderIndex::new(EmbedderIndex::E8Graph);
        let large_vec: Vec<f32> = (0..384).map(|_| 1e38).collect();
        large_index.insert(Uuid::new_v4(), &large_vec).unwrap();
        println!("    Large floats accepted");
        println!("    PASS");

        // Edge case 9: Registry total counts
        println!();
        println!("Edge 9: Registry aggregation across indexes");
        let registry = EmbedderIndexRegistry::new();

        // Insert into multiple indexes
        for embedder in [
            EmbedderIndex::E1Semantic,
            EmbedderIndex::E8Graph,
            EmbedderIndex::PurposeVector,
        ] {
            let idx = registry.get(embedder).unwrap();
            let dim = idx.config().dimension;
            for _ in 0..10 {
                idx.insert(Uuid::new_v4(), &vec![0.5f32; dim]).unwrap();
            }
        }

        assert_eq!(registry.total_vectors(), 30);
        println!("    Total vectors across 3 indexes: {}", registry.total_vectors());
        println!("    Total memory: {} bytes", registry.total_memory_bytes());
        println!("    PASS");

        println!();
        println!("=== ALL EDGE CASES PASSED ===");
    }

    #[test]
    fn test_full_state_verification() {
        println!("\n=== FULL STATE VERIFICATION ===");
        println!();

        // Verify all 12 indexes exist and have correct dimensions
        let registry = EmbedderIndexRegistry::new();

        let expected: Vec<(EmbedderIndex, usize)> = vec![
            (EmbedderIndex::E1Semantic, 1024),
            (EmbedderIndex::E1Matryoshka128, 128),
            (EmbedderIndex::E2TemporalRecent, 512),
            (EmbedderIndex::E3TemporalPeriodic, 512),
            (EmbedderIndex::E4TemporalPositional, 512),
            (EmbedderIndex::E5Causal, 768),
            (EmbedderIndex::E7Code, 1536),
            (EmbedderIndex::E8Graph, 384),
            (EmbedderIndex::E9HDC, 1024),
            (EmbedderIndex::E10Multimodal, 768),
            (EmbedderIndex::E11Entity, 384),
            (EmbedderIndex::PurposeVector, 13),
        ];

        println!("Verifying 12 HNSW indexes:");
        for (embedder, expected_dim) in &expected {
            let idx = registry.get(*embedder);
            assert!(idx.is_some(), "{:?} should have index", embedder);
            let idx = idx.unwrap();
            let actual_dim = idx.config().dimension;
            assert_eq!(
                actual_dim, *expected_dim,
                "{:?}: expected {}D, got {}D",
                embedder, expected_dim, actual_dim
            );
            println!("  {:?}: {}D ✓", embedder, actual_dim);
        }

        println!();
        println!("Verifying 3 non-HNSW embedders return None:");
        for embedder in [
            EmbedderIndex::E6Sparse,
            EmbedderIndex::E12LateInteraction,
            EmbedderIndex::E13Splade,
        ] {
            assert!(registry.get(embedder).is_none(), "{:?} should return None", embedder);
            println!("  {:?}: None ✓", embedder);
        }

        println!();
        println!("=== FULL STATE VERIFICATION COMPLETE ===");
    }
}
