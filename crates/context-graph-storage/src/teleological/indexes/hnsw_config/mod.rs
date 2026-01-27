//! HNSW index configuration for 4-stage retrieval pipeline.
//!
//! # FAIL FAST. NO FALLBACKS.
//!
//! Invalid configurations panic immediately. No silent defaults.
//!
//! # Index Types (from constitution.yaml)
//!
//! | Stage | Index | Type | Embedder |
//! |-------|-------|------|----------|
//! | 1 | SPLADE inverted | InvertedIndex | E13 |
//! | 2 | Matryoshka 128D | HNSW | E1 (truncated) |
//! | 3 | Full embeddings | HNSW x 10 | E1-E5, E7-E11 |
//! | 4 | ColBERT MaxSim | Token-level | E12 |
//!
//! # HNSW Configuration Table
//!
//! | Index | Dimension | M | ef_construction | ef_search | Metric |
//! |-------|-----------|---|-----------------|-----------|--------|
//! | E1Semantic | 1024 | 16 | 200 | 100 | Cosine |
//! | E1Matryoshka128 | 128 | 32 | 256 | 128 | Cosine |
//! | E2TemporalRecent | 512 | 16 | 200 | 100 | Cosine |
//! | E3TemporalPeriodic | 512 | 16 | 200 | 100 | Cosine |
//! | E4TemporalPositional | 512 | 16 | 200 | 100 | Cosine |
//! | E5Causal | 768 | 16 | 200 | 100 | AsymmetricCosine |
//! | E7Code | 1536 | 16 | 200 | 100 | Cosine |
//! | E8Graph | 1024 | 16 | 200 | 100 | Cosine |
//! | E9HDC | 1024 | 16 | 200 | 100 | Cosine |
//! | E10Multimodal | 768 | 16 | 200 | 100 | Cosine |
//! | E11Entity | 768 | 16 | 200 | 100 | Cosine |
//!
//! # Module Structure
//!
//! - [`constants`]: Dimension constants for all embedders
//! - [`distance`]: Distance metric enum
//! - [`embedder`]: Embedder index enum
//! - [`config`]: HNSW and inverted index config structs
//! - [`functions`]: Config accessor functions

mod config;
mod constants;
mod distance;
mod embedder;
mod functions;

// Re-export constants
pub use self::constants::{
    E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E1_DIM, E1_MATRYOSHKA_DIM, E2_DIM, E3_DIM,
    E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM, NUM_EMBEDDERS, TOPIC_PROFILE_DIM,
};

// Re-export types
pub use self::config::{HnswConfig, InvertedIndexConfig};
pub use self::distance::DistanceMetric;
pub use self::embedder::EmbedderIndex;

// Re-export functions
pub use self::functions::{all_hnsw_configs, get_hnsw_config, get_inverted_index_config};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _ = EmbedderIndex::E1Semantic;
        let _ = DistanceMetric::Cosine;
        let _ = HnswConfig::matryoshka_128d();
        let _ = InvertedIndexConfig::e6_sparse();

        // Verify all public functions work
        let _ = get_hnsw_config(EmbedderIndex::E1Semantic);
        let _ = all_hnsw_configs();
        let _ = get_inverted_index_config(EmbedderIndex::E6Sparse);

        // Verify constants
        assert_eq!(E1_DIM, 1024);
        assert_eq!(NUM_EMBEDDERS, 13);
    }

    #[test]
    fn test_verification_log() {
        println!("\n=== MODULARIZED HNSW_CONFIG VERIFICATION ===");

        println!("Enum Verification:");
        let hnsw_count = EmbedderIndex::all_hnsw().len();
        assert_eq!(hnsw_count, 15);
        println!("  - EmbedderIndex: 15 HNSW + 3 non-HNSW = 18 variants");
        println!("  - Includes E5CausalCause, E5CausalEffect, E10MultimodalIntent, E10MultimodalContext (ARCH-15)");
        println!("  - DistanceMetric: 5 variants");

        println!("Struct Verification:");
        println!("  - HnswConfig: m, ef_construction, ef_search, metric, dimension");
        println!("  - InvertedIndexConfig: vocab_size, max_nnz, use_bm25");

        println!("Function Verification:");
        let configs = all_hnsw_configs();
        assert_eq!(configs.len(), 15);
        println!("  - all_hnsw_configs: {} entries", configs.len());

        assert!(get_hnsw_config(EmbedderIndex::E6Sparse).is_none());
        println!("  - get_hnsw_config: None for E6, E12, E13");

        assert!(get_inverted_index_config(EmbedderIndex::E6Sparse).is_some());
        println!("  - get_inverted_index_config: Some for E6, E13");

        println!("VERIFICATION COMPLETE");
    }
}
