//! Teleological fingerprint storage extensions.
//!
//! Adds 24 column families for TeleologicalFingerprint storage:
//! - 11 core teleological CFs (9 active + 2 legacy for backwards compatibility)
//! - 13 quantized embedder CFs (per-embedder quantized storage)
//!
//! # Column Families (24 new, 32 total with base 8)
//!
//! ## Core Teleological (9 CFs)
//! | Name | Purpose | Key Format | Value Size |
//! |------|---------|------------|------------|
//! | fingerprints | Primary ~63KB TeleologicalFingerprints | UUID (16 bytes) | ~63KB |
//! | topic_profiles | 13D topic profiles | UUID (16 bytes) | 52 bytes |
//! | e13_splade_inverted | Inverted index for E13 SPLADE | term_id (2 bytes) | Vec<UUID> |
//! | e1_matryoshka_128 | E1 Matryoshka 128D truncated vectors | UUID (16 bytes) | 512 bytes |
//! | synergy_matrix | Singleton 13x13 synergy matrix | "synergy" (7 bytes) | ~700 bytes |
//! | teleological_profiles | Task-specific profiles | profile_id string | ~200-500 bytes |
//! | teleological_vectors | Per-memory 13D vectors | UUID (16 bytes) | 52 bytes |
//!
//! ## Quantized Embedder (13 CFs)
//! | Name | Purpose | Key Format | Value Size |
//! |------|---------|------------|------------|
//! | emb_0 | E1_Semantic quantized (PQ-8) | UUID (16 bytes) | ~8 bytes |
//! | emb_1 | E2_TemporalRecent quantized (Float8) | UUID (16 bytes) | ~512 bytes |
//! | emb_2 | E3_TemporalPeriodic quantized (Float8) | UUID (16 bytes) | ~512 bytes |
//! | emb_3 | E4_TemporalPositional quantized (Float8) | UUID (16 bytes) | ~512 bytes |
//! | emb_4 | E5_Causal quantized (PQ-8) | UUID (16 bytes) | ~8 bytes |
//! | emb_5 | E6_Sparse quantized (SparseNative) | UUID (16 bytes) | ~2KB |
//! | emb_6 | E7_Code quantized (PQ-8) | UUID (16 bytes) | ~8 bytes |
//! | emb_7 | E8_Graph quantized (Float8) | UUID (16 bytes) | ~1024 bytes |
//! | emb_8 | E9_HDC quantized (Binary) | UUID (16 bytes) | ~1250 bytes |
//! | emb_9 | E10_Multimodal quantized (PQ-8) | UUID (16 bytes) | ~8 bytes |
//! | emb_10 | E11_Entity quantized (PQ8) | UUID (16 bytes) | ~768 bytes |
//! | emb_11 | E12_LateInteraction quantized (TokenPruning) | UUID (16 bytes) | ~2KB |
//! | emb_12 | E13_SPLADE quantized (SparseNative) | UUID (16 bytes) | ~2KB |
//!
//! # Design Philosophy
//!
//! **FAIL FAST. NO FALLBACKS.**
//!
//! All errors panic with full context. No silent fallbacks or default values.
//! This ensures data integrity and makes bugs immediately visible.

pub mod column_families;
pub mod indexes;
pub mod quantized;
pub mod rocksdb_store;
pub mod schema;
pub mod search;
pub mod serialization;

#[cfg(test)]
mod tests;

// Re-export column family types
pub use column_families::{
    // TASK-STORAGE-P2-001: E12 Late Interaction column family
    e12_late_interaction_cf_options,
    e13_splade_inverted_cf_options,
    e1_matryoshka_128_cf_options,
    fingerprint_cf_options,
    get_all_cf_descriptors,
    get_all_teleological_cf_descriptors,
    get_causal_cf_descriptors,
    get_code_cf_descriptors,
    get_quantized_embedder_cf_descriptors,
    get_teleological_cf_descriptors,
    topic_profile_cf_options,
    // Causal relationship column families
    causal_by_source_cf_options,
    causal_relationships_cf_options,
    CF_CAUSAL_BY_SOURCE,
    CF_CAUSAL_RELATIONSHIPS,
    CAUSAL_CFS,
    CAUSAL_CF_COUNT,
    // Quantized embedder column families (TASK-EMB-022)
    quantized_embedder_cf_options,
    synergy_matrix_cf_options,
    teleological_profiles_cf_options,
    teleological_vectors_cf_options,
    // TASK-CONTENT-001: Content column family
    CF_CONTENT,
    // TASK-STORAGE-P2-001: E12 Late Interaction column family constant
    CF_E12_LATE_INTERACTION,
    CF_E13_SPLADE_INVERTED,
    // E6 sparse inverted index for dual Stage 1 recall (e6upgrade.md)
    CF_E6_SPARSE_INVERTED,
    e6_sparse_inverted_cf_options,
    CF_E1_MATRYOSHKA_128,
    CF_EMB_0,
    CF_EMB_1,
    CF_EMB_10,
    CF_EMB_11,
    CF_EMB_12,
    CF_EMB_2,
    CF_EMB_3,
    CF_EMB_4,
    CF_EMB_5,
    CF_EMB_6,
    CF_EMB_7,
    CF_EMB_8,
    CF_EMB_9,
    CF_FINGERPRINTS,
    CF_TOPIC_PROFILES,
    // TASK-TELEO-006: New teleological vector column families
    CF_SYNERGY_MATRIX,
    CF_TELEOLOGICAL_PROFILES,
    CF_TELEOLOGICAL_VECTORS,
    QUANTIZED_EMBEDDER_CFS,
    QUANTIZED_EMBEDDER_CF_COUNT,
    TELEOLOGICAL_CFS,
    TELEOLOGICAL_CF_COUNT,
};

// Re-export quantized storage types (TASK-EMB-022)
pub use quantized::{QuantizedFingerprintStorage, QuantizedStorageError, QuantizedStorageResult};

// Re-export schema types
pub use schema::{
    e13_splade_inverted_key,
    // E6 sparse inverted index key functions (e6upgrade.md)
    e6_sparse_inverted_key,
    parse_e6_sparse_key,
    e1_matryoshka_128_key,
    fingerprint_key,
    parse_e13_splade_key,
    parse_e1_matryoshka_key,
    parse_fingerprint_key,
    parse_teleological_profile_key,
    parse_teleological_vector_key,
    parse_topic_profile_key,
    teleological_profile_key,
    teleological_vector_key,
    topic_profile_key,
    // TASK-TELEO-006: New key format functions
    SYNERGY_MATRIX_KEY,
};

// Re-export serialization types
pub use serialization::{
    deserialize_e1_matryoshka_128,
    deserialize_memory_id_list,
    deserialize_teleological_fingerprint,
    deserialize_topic_profile,
    serialize_e1_matryoshka_128,
    serialize_memory_id_list,
    serialize_teleological_fingerprint,
    serialize_topic_profile,
    TELEOLOGICAL_VERSION,
};

// Re-export index configuration types (TASK-F005)
pub use indexes::{
    // Config functions
    all_hnsw_configs,
    // Metric functions
    compute_distance,
    cosine_similarity,
    distance_to_similarity,
    get_hnsw_config,
    get_inverted_index_config,
    recommended_metric,
    // Enums
    DistanceMetric,
    EmbedderIndex,
    // Structs
    HnswConfig,
    InvertedIndexConfig,
    E10_DIM,
    E11_DIM,
    E12_TOKEN_DIM,
    E13_SPLADE_VOCAB,
    // Dimension constants
    E1_DIM,
    E1_MATRYOSHKA_DIM,
    E2_DIM,
    E3_DIM,
    E4_DIM,
    E5_DIM,
    E6_SPARSE_VOCAB,
    E7_DIM,
    E8_DIM,
    E9_DIM,
    NUM_EMBEDDERS,
    TOPIC_PROFILE_DIM,
};

// Re-export RocksDB teleological store (TASK: RocksDbTeleologicalStore)
pub use rocksdb_store::{
    RocksDbTeleologicalStore, TeleologicalStoreConfig, TeleologicalStoreError,
    TeleologicalStoreResult,
};

// Re-export search types (TASK-LOGIC-005)
pub use search::{
    // MaxSim for E12 ColBERT reranking
    compute_maxsim_direct,
    EmbedderSearchHit, SearchError, SearchResult, SingleEmbedderSearch, SingleEmbedderSearchConfig,
    SingleEmbedderSearchResults,
};
