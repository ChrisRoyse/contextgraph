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
//! - `error`: Storage error types and result aliases
//! - `column_families`: Column family definitions
//! - `teleological`: TeleologicalFingerprint storage (RocksDbTeleologicalStore)
//! - `graph_edges`: K-NN graph edges and typed edges
//! - `indexes`: Secondary index operations (tags, temporal, sources)
//! - `code`: Code entity and E7 embedding storage (separate from text)
//!
//! # Column Families
//!
//! Teleological (19): fingerprints, topic_profiles, e13_splade_inverted, e6_sparse_inverted,
//!                    e1_matryoshka_128, content, source_metadata, file_index, topic_portfolio,
//!                    e12_late_interaction, entity_provenance, audit_log, audit_by_target,
//!                    merge_history, importance_history, tool_call_index,
//!                    consolidation_recommendations, embedding_registry, custom_weight_profiles
//! Quantized Embedder (13): emb_0..emb_12
//! Code (5): code_entities, code_e7_embeddings, code_file_index, code_name_index, code_signature_index
//! Causal (2): causal_relationships, causal_by_source

pub mod code;
pub mod column_families;
pub mod dynamicjepa;
pub mod error;
pub mod graph_edges;
pub mod indexes;
pub mod teleological;

// Re-export column family types for storage consumers
pub use column_families::{
    cf_names, edges_options, embeddings_options, get_all_column_family_descriptors,
    get_column_family_descriptors, index_options, nodes_options, system_options,
    TOTAL_COLUMN_FAMILIES,
};

// Re-export storage error types
pub use error::{StorageError, StorageResult};

// Re-export teleological storage types (TASK-F004)
pub use teleological::{
    // HNSW index configuration types (TASK-F005)
    all_hnsw_configs,
    // MaxSim for E12 ColBERT reranking (TASK-STORAGE-P2-001)
    compute_maxsim_direct,
    custom_weight_profiles_cf_options,
    deserialize_e1_matryoshka_128,
    deserialize_memory_id_list,
    deserialize_teleological_fingerprint,
    deserialize_topic_profile,
    e13_splade_inverted_cf_options,
    // Key format functions
    e13_splade_inverted_key,
    e1_matryoshka_128_cf_options,
    e1_matryoshka_128_key,
    fingerprint_cf_options,
    fingerprint_key,
    fingerprints_learner_cf_options,
    get_all_teleological_cf_descriptors,
    get_hnsw_config,
    get_inverted_index_config,
    get_quantized_embedder_cf_descriptors,
    get_teleological_cf_descriptors,
    goal_centroids_cf_options,
    importance_history_cf_options,
    learner_audit_cf_options,
    learner_constellations_cf_options,
    learner_delta_log_cf_options,
    learner_goal_states_cf_options,
    learner_k_sleep_cf_options,
    learner_m_per_trace_cf_options,
    learner_profile_cf_options,
    learner_retrieval_log_cf_options,
    learner_state_history_cf_options,
    learning_events_cf_options,
    merge_history_cf_options,
    parse_e13_splade_key,
    parse_e1_matryoshka_key,
    parse_fingerprint_key,
    parse_topic_profile_key,
    // Quantized embedder column families (TASK-EMB-022)
    quantized_embedder_cf_options,
    serialize_e1_matryoshka_128,
    serialize_memory_id_list,
    // Serialization functions
    serialize_teleological_fingerprint,
    serialize_topic_profile,
    topic_profile_cf_options,
    topic_profile_key,
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
    CF_EMB_13,
    CF_EMB_2,
    CF_EMB_3,
    CF_EMB_4,
    CF_EMB_5,
    CF_EMB_6,
    CF_EMB_7,
    CF_EMB_8,
    CF_EMB_9,
    CF_FINGERPRINTS,
    CF_FINGERPRINTS_LEARNER,
    CF_GOAL_CENTROIDS,
    CF_IMPORTANCE_HISTORY,
    CF_LEARNER_AUDIT,
    CF_LEARNER_CONSTELLATIONS,
    CF_LEARNER_DELTA_LOG,
    CF_LEARNER_GOAL_STATES,
    CF_LEARNER_K_SLEEP,
    CF_LEARNER_M_PER_TRACE,
    CF_LEARNER_PROFILE,
    CF_LEARNER_RETRIEVAL_LOG,
    CF_LEARNER_STATE_HISTORY,
    CF_LEARNER_TRAINING_DATASETS,
    CF_LEARNING_EVENTS,
    // Phase 4 Lifecycle Provenance: Merge + importance history
    CF_MERGE_HISTORY,
    CF_TOPIC_PROFILES,
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
    QUANTIZED_EMBEDDER_CFS,
    QUANTIZED_EMBEDDER_CF_COUNT,
    TELEOLOGICAL_CFS,
    TELEOLOGICAL_CF_COUNT,
    TELEOLOGICAL_VERSION,
    TOPIC_PROFILE_DIM,
};

// Re-export code storage types (CODE-001)
pub use code::{CodeStorageError, CodeStorageResult, CodeStore, E7_CODE_DIM};

// Re-export DynamicJEPA storage data plane (5090jepa Phase 2)
pub use dynamicjepa::*;

// Re-export code column family constants
pub use teleological::column_families::{
    get_code_cf_descriptors, CF_CODE_E7_EMBEDDINGS, CF_CODE_ENTITIES, CF_CODE_FILE_INDEX,
    CF_CODE_NAME_INDEX, CF_CODE_SIGNATURE_INDEX, CODE_CFS, CODE_CF_COUNT,
};

// Re-export code entity types from core
pub use context_graph_core::types::{
    CodeEntity, CodeEntityType, CodeFileIndexEntry, CodeLanguage, CodeStats, Visibility,
};

// Re-export graph edges storage types (TASK-GRAPHLINK)
pub use graph_edges::{
    BackgroundGraphBuilder, BatchBuildResult, BuilderStats, EdgeRepository, GraphBuilderConfig,
    GraphEdgeStats, GraphEdgeStorageError, GraphEdgeStorageResult, RebuildResult,
};
