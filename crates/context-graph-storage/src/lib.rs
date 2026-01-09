#![deny(deprecated)]

//! Context Graph Storage Layer
//!
//! Provides persistent storage for the Context Graph system
//! using RocksDB as the underlying storage engine.
//!
//! # Architecture
//! - `memex`: Storage trait abstraction (Memex = "memory index")
//! - `rocksdb_backend`: RocksDB implementation
//! - `column_families`: Column family definitions per Johari quadrant (12 CFs)
//! - `teleological`: TeleologicalFingerprint storage extensions (20 CFs)
//! - `serialization`: Bincode serialization utilities
//! - `indexes`: Secondary index operations (tags, temporal, sources)
//!
//! # Column Families (39 total)
//!
//! Base (12): nodes, edges, embeddings, metadata, johari_*, temporal, tags, sources, system
//! Teleological (20): fingerprints, purpose_vectors, e13_splade_inverted, e1_matryoshka_128,
//!                    synergy_matrix, teleological_profiles, teleological_vectors, emb_0..emb_12
//! Autonomous (7): autonomous_config, adaptive_threshold_state, drift_history, goal_activity_metrics,
//!                 autonomous_lineage, consolidation_history, memory_curation
//!
//! # Constitution Reference
//! - db.dev: sqlite (ghost phase), db.prod: postgres16+
//! - db.vector: faiss_gpu (separate from RocksDB node storage)
//! - SEC-06: Soft delete 30-day recovery

pub mod autonomous;
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
    EdgeId, EmbeddingVector, GraphEdge, JohariQuadrant, MemoryNode, Modality, NodeId, NodeMetadata,
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
    deserialize_purpose_vector,
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
    parse_purpose_vector_key,
    parse_teleological_profile_key,
    parse_teleological_vector_key,
    purpose_vector_cf_options,
    purpose_vector_key,
    // Quantized embedder column families (TASK-EMB-022)
    quantized_embedder_cf_options,
    serialize_e1_matryoshka_128,
    serialize_memory_id_list,
    serialize_purpose_vector,
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
    CF_PURPOSE_VECTORS,
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
    PURPOSE_VECTOR_DIM,
    QUANTIZED_EMBEDDER_CFS,
    QUANTIZED_EMBEDDER_CF_COUNT,
    // TASK-TELEO-006: New key format functions
    SYNERGY_MATRIX_KEY,
    TELEOLOGICAL_CFS,
    TELEOLOGICAL_CF_COUNT,
    TELEOLOGICAL_VERSION,
};

// Re-export autonomous storage types (TASK-NORTH-007)
pub use autonomous::{
    adaptive_threshold_state_cf_options,
    // CF option builders
    autonomous_config_cf_options,
    autonomous_lineage_cf_options,
    autonomous_lineage_key,
    autonomous_lineage_timestamp_prefix,
    consolidation_history_cf_options,
    consolidation_history_key,
    consolidation_history_timestamp_prefix,
    drift_history_cf_options,
    // Key format functions
    drift_history_key,
    drift_history_timestamp_prefix,
    // Descriptor getter
    get_autonomous_cf_descriptors,
    goal_activity_metrics_cf_options,
    goal_activity_metrics_key,
    memory_curation_cf_options,
    memory_curation_key,
    parse_autonomous_lineage_key,
    parse_consolidation_history_key,
    parse_drift_history_key,
    parse_goal_activity_metrics_key,
    parse_memory_curation_key,
    ADAPTIVE_THRESHOLD_STATE_KEY,
    AUTONOMOUS_CFS,
    AUTONOMOUS_CF_COUNT,
    // Singleton key constants
    AUTONOMOUS_CONFIG_KEY,
    CF_ADAPTIVE_THRESHOLD_STATE,
    // Column family names
    CF_AUTONOMOUS_CONFIG,
    CF_AUTONOMOUS_LINEAGE,
    CF_CONSOLIDATION_HISTORY,
    CF_DRIFT_HISTORY,
    CF_GOAL_ACTIVITY_METRICS,
    CF_MEMORY_CURATION,
};
