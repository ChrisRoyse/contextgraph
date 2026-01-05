//! Context Graph Storage Layer
//!
//! Provides persistent storage for the Context Graph system
//! using RocksDB as the underlying storage engine.
//!
//! # Architecture
//! - `memex`: Storage trait abstraction (Memex = "memory index")
//! - `rocksdb_backend`: RocksDB implementation
//! - `column_families`: Column family definitions per Johari quadrant (12 CFs)
//! - `teleological`: TeleologicalFingerprint storage extensions (4 CFs)
//! - `serialization`: Bincode serialization utilities
//! - `indexes`: Secondary index operations (tags, temporal, sources)
//!
//! # Column Families (16 total)
//!
//! Base (12): nodes, edges, embeddings, metadata, johari_*, temporal, tags, sources, system
//! Teleological (4): fingerprints, purpose_vectors, e13_splade_inverted, e1_matryoshka_128
//!
//! # Constitution Reference
//! - db.dev: sqlite (ghost phase), db.prod: postgres16+
//! - db.vector: faiss_gpu (separate from RocksDB node storage)
//! - SEC-06: Soft delete 30-day recovery

pub mod column_families;
pub mod indexes;
pub mod memex;
pub mod rocksdb_backend;
pub mod serialization;
pub mod teleological;

// Re-export column family types for storage consumers
pub use column_families::{
    cf_names, edges_options, embeddings_options, get_column_family_descriptors, index_options,
    nodes_options, system_options,
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
    // Column family names and functions
    CF_E1_MATRYOSHKA_128, CF_E13_SPLADE_INVERTED, CF_FINGERPRINTS, CF_PURPOSE_VECTORS,
    TELEOLOGICAL_CFS, get_teleological_cf_descriptors,
    e1_matryoshka_128_cf_options, e13_splade_inverted_cf_options,
    fingerprint_cf_options, purpose_vector_cf_options,
    // Key format functions
    e13_splade_inverted_key, e1_matryoshka_128_key, fingerprint_key,
    parse_e13_splade_key, parse_fingerprint_key, purpose_vector_key,
    parse_purpose_vector_key, parse_e1_matryoshka_key,
    // Serialization functions
    serialize_teleological_fingerprint, deserialize_teleological_fingerprint,
    serialize_purpose_vector, deserialize_purpose_vector,
    serialize_e1_matryoshka_128, deserialize_e1_matryoshka_128,
    serialize_memory_id_list, deserialize_memory_id_list,
    TELEOLOGICAL_VERSION,
};
