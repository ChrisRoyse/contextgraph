//! RocksDB storage backend implementation.
//!
//! Provides persistent storage using RocksDB with column families
//! for efficient indexing.
//!
//! # Performance Targets (constitution.yaml)
//! - inject_context: p95 < 25ms, p99 < 50ms
//! - store_node: < 1ms p95
//! - get_node: < 500μs p95
//!
//! # Column Families
//! Uses CFs defined in `column_families.rs`:
//! - nodes, edges, embeddings, metadata
//! - temporal, tags, sources, system
//!
//! # CRUD Operations (TASK-M02-017)
//! - `store_node()`: Atomic write to nodes, embeddings, temporal, tags, sources CFs
//! - `get_node()`: Retrieve and deserialize MemoryNode by ID
//! - `update_node()`: Update with index maintenance when tags change
//! - `delete_node()`: Soft delete (SEC-06 compliance) or hard delete
//!
//! # Module Structure
//! - `config`: Configuration options (RocksDbConfig)
//! - `error`: Error types (StorageError)
//! - `core`: Main RocksDbMemex struct with open/close/health
//! - `node_ops`: Node CRUD operations
//! - `edge_ops`: Edge CRUD operations
//! - `embedding_ops`: Embedding storage operations (TASK-M02-024)
//! - `index_ops`: Secondary index query operations
//! - `helpers`: Key formatting utilities

mod config;
mod core;
mod edge_ops;
mod embedding_ops;
mod error;
mod helpers;
mod index_ops;
mod memex_impl;
mod node_ops;

#[cfg(test)]
mod tests_core;
#[cfg(test)]
mod tests_edge;
#[cfg(test)]
mod tests_edge_scan;
#[cfg(test)]
mod tests_embedding;
#[cfg(test)]
mod tests_index;
#[cfg(test)]
mod tests_node;
#[cfg(test)]
mod tests_node_lifecycle;

// Re-export configuration
pub use config::{RocksDbConfig, DEFAULT_CACHE_SIZE, DEFAULT_MAX_OPEN_FILES};

// Re-export error types
pub use error::{StorageError, StorageResult};

// Re-export main struct
pub use core::RocksDbMemex;
