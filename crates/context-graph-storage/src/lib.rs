#![deny(deprecated)]
#![allow(clippy::module_inception)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::type_complexity)]

//! Context Graph Storage Layer
//!
//! Provides persistent storage for the Context Graph system
//! using RocksDB as the underlying storage engine.
//!
//! # Architecture
//! - `memex`: Storage trait abstraction (Memex = "memory index")
//! - `rocksdb_backend`: RocksDB implementation
//! - `column_families`: Column family definitions (8 base CFs per PRD v6)
//! - `teleological`: TeleologicalFingerprint storage extensions
//! - `serialization`: Bincode serialization utilities
//! - `indexes`: Secondary index operations (tags, temporal, sources)
//! - `code`: Code entity and E7 embedding storage (separate from text)
//!
//! # Column Families (37 total per PRD v6)
//!
//! Base (8): nodes, edges, embeddings, metadata, temporal, tags, sources, system
//! Teleological (15): fingerprints, topic_profiles, e13_splade_inverted, e1_matryoshka_128,
//!                    synergy_matrix, teleological_profiles, teleological_vectors, etc.
//! Quantized Embedder (13): emb_0..emb_12
//! Code (5): code_entities, code_e7_embeddings, code_file_index, code_name_index, code_signature_index
//!
//! # Constitution Reference
//! - db.dev: sqlite (ghost phase), db.prod: postgres16+
//! - db.vector: faiss_gpu (separate from RocksDB node storage)
//! - SEC-06: Soft delete 30-day recovery

pub mod code;
pub mod column_families;
pub mod indexes;
pub mod memex;
pub mod rocksdb_backend;
pub mod serialization;
pub mod teleological;

// Re-export column family types for storage consumers
pub use column_families::{
    cf_names, edges_options, embeddings_options, get_all_column_family_descriptors,
    get_column_family_descriptors, index_options, nodes_options, system_options,
    TOTAL_COLUMN_FAMILIES,
};

// Re-export RocksDB backend types (TASK-M02-016, TASK-M02-025)
pub use rocksdb_backend::{
    RocksDbConfig, RocksDbMemex, StorageError, StorageResult, DEFAULT_CACHE_SIZE,
    DEFAULT_MAX_OPEN_FILES,
};

// Re-export Memex trait and StorageHealth (TASK-M02-026)
pub use memex::{Memex, StorageHealth};

// Re-export core types for storage consumers
pub use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
pub use context_graph_core::types::{
    EdgeId, EmbeddingVector, GraphEdge, MemoryNode, Modality, NodeId, NodeMetadata,
    ValidationError,
};

// Re-export serialization types and functions
pub use serialization::{
    deserialize_edge, deserialize_embedding, deserialize_node, deserialize_uuid, serialize_edge,
    serialize_embedding, serialize_node, serialize_uuid, SerializationError,
};

// Re-export teleological storage types (TASK-F004)
pub use teleological::{
    // HNSW index configuration types (TASK-F005)
    all_hnsw_configs,
    deserialize_e1_matryoshka_128,
    deserialize_memory_id_list,
    deserialize_topic_profile,
    deserialize_teleological_fingerprint,
    e13_splade_inverted_cf_options,
    // Key format functions
    e13_splade_inverted_key,
    e1_matryoshka_128_cf_options,
    e1_matryoshka_128_key,
    fingerprint_cf_options,
    fingerprint_key,
    get_all_teleological_cf_descriptors,
    get_hnsw_config,
    get_inverted_index_config,
    get_quantized_embedder_cf_descriptors,
    get_teleological_cf_descriptors,
    parse_e13_splade_key,
    parse_e1_matryoshka_key,
    parse_fingerprint_key,
    parse_topic_profile_key,
    parse_teleological_profile_key,
    parse_teleological_vector_key,
    topic_profile_cf_options,
    topic_profile_key,
    // Quantized embedder column families (TASK-EMB-022)
    quantized_embedder_cf_options,
    serialize_e1_matryoshka_128,
    serialize_memory_id_list,
    serialize_topic_profile,
    // Serialization functions
    serialize_teleological_fingerprint,
    synergy_matrix_cf_options,
    teleological_profile_key,
    teleological_profiles_cf_options,
    teleological_vector_key,
    teleological_vectors_cf_options,
    DistanceMetric,
    EmbedderIndex,
    HnswConfig,
    InvertedIndexConfig,
    // Quantized fingerprint storage trait (TASK-EMB-022)
    QuantizedFingerprintStorage,
    QuantizedStorageError,
    QuantizedStorageResult,
    // RocksDB teleological store (TASK: test-remediation)
    RocksDbTeleologicalStore,
    TeleologicalStoreConfig,
    TeleologicalStoreError,
    TeleologicalStoreResult,
    CF_E13_SPLADE_INVERTED,
    // Column family names and functions
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
    E10_DIM,
    E11_DIM,
    E12_TOKEN_DIM,
    E13_SPLADE_VOCAB,
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
    QUANTIZED_EMBEDDER_CFS,
    QUANTIZED_EMBEDDER_CF_COUNT,
    // TASK-TELEO-006: New key format functions
    SYNERGY_MATRIX_KEY,
    TELEOLOGICAL_CFS,
    TELEOLOGICAL_CF_COUNT,
    TELEOLOGICAL_VERSION,
};

// Re-export code storage types (CODE-001)
pub use code::{CodeStorageError, CodeStorageResult, CodeStore, E7_CODE_DIM};

// Re-export code column family constants
pub use teleological::column_families::{
    get_code_cf_descriptors, CF_CODE_ENTITIES, CF_CODE_E7_EMBEDDINGS, CF_CODE_FILE_INDEX,
    CF_CODE_NAME_INDEX, CF_CODE_SIGNATURE_INDEX, CODE_CFS, CODE_CF_COUNT,
};

// Re-export code entity types from core
pub use context_graph_core::types::{
    CodeEntity, CodeEntityType, CodeFileIndexEntry, CodeLanguage, CodeStats, Visibility,
};
