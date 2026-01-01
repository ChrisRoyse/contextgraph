//! Context Graph Storage Layer
//!
//! Provides persistent storage for the Context Graph system
//! using RocksDB as the underlying storage engine.
//!
//! # Architecture
//! - `memex`: Storage trait abstraction (Memex = "memory index")
//! - `rocksdb_backend`: RocksDB implementation
//! - `column_families`: Column family definitions per Johari quadrant
//! - `serialization`: Bincode serialization utilities
//! - `indexes`: Secondary index operations (tags, temporal, sources)
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

// Re-export column family types for storage consumers
pub use column_families::{
    cf_names, edges_options, embeddings_options, get_column_family_descriptors, index_options,
    nodes_options, system_options,
};

// Re-export RocksDB backend types (TASK-M02-016)
pub use rocksdb_backend::{
    RocksDbConfig, RocksDbMemex, StorageError, DEFAULT_CACHE_SIZE, DEFAULT_MAX_OPEN_FILES,
};

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
