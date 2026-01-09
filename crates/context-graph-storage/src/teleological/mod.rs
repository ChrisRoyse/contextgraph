//! Teleological fingerprint storage extensions.
//!
//! Adds 20 column families for TeleologicalFingerprint storage:
//! - 7 core teleological CFs (~63KB fingerprints, purpose vectors, indexes, teleological vectors)
//! - 13 quantized embedder CFs (per-embedder quantized storage)
//!
//! # Column Families (20 new, 32 total with base 12)
//!
//! ## Core Teleological (7 CFs)
//! | Name | Purpose | Key Format | Value Size |
//! |------|---------|------------|------------|
//! | fingerprints | Primary ~63KB TeleologicalFingerprints | UUID (16 bytes) | ~63KB |
//! | purpose_vectors | 13D purpose vectors | UUID (16 bytes) | 52 bytes |
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
//! | emb_7 | E8_Graph quantized (Float8) | UUID (16 bytes) | ~384 bytes |
//! | emb_8 | E9_HDC quantized (Binary) | UUID (16 bytes) | ~1250 bytes |
//! | emb_9 | E10_Multimodal quantized (PQ-8) | UUID (16 bytes) | ~8 bytes |
//! | emb_10 | E11_Entity quantized (Float8) | UUID (16 bytes) | ~384 bytes |
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
pub mod serialization;

#[cfg(test)]
mod tests;

// Re-export column family types
pub use column_families::{
    e13_splade_inverted_cf_options,
    e1_matryoshka_128_cf_options,
    fingerprint_cf_options,
    get_all_teleological_cf_descriptors,
    get_quantized_embedder_cf_descriptors,
    get_teleological_cf_descriptors,
    purpose_vector_cf_options,
    // Quantized embedder column families (TASK-EMB-022)
    quantized_embedder_cf_options,
    synergy_matrix_cf_options,
    teleological_profiles_cf_options,
    teleological_vectors_cf_options,
    CF_E13_SPLADE_INVERTED,
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
    CF_PURPOSE_VECTORS,
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
    e1_matryoshka_128_key,
    fingerprint_key,
    parse_e13_splade_key,
    parse_e1_matryoshka_key,
    parse_fingerprint_key,
    parse_purpose_vector_key,
    parse_teleological_profile_key,
    parse_teleological_vector_key,
    purpose_vector_key,
    teleological_profile_key,
    teleological_vector_key,
    // TASK-TELEO-006: New key format functions
    SYNERGY_MATRIX_KEY,
};

// Re-export serialization types
pub use serialization::{
    deserialize_e1_matryoshka_128, deserialize_memory_id_list, deserialize_purpose_vector,
    deserialize_teleological_fingerprint, serialize_e1_matryoshka_128, serialize_memory_id_list,
    serialize_purpose_vector, serialize_teleological_fingerprint, TELEOLOGICAL_VERSION,
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
    PURPOSE_VECTOR_DIM,
};

// Re-export RocksDB teleological store (TASK: RocksDbTeleologicalStore)
pub use rocksdb_store::{
    RocksDbTeleologicalStore, TeleologicalStoreConfig, TeleologicalStoreError,
    TeleologicalStoreResult,
};
