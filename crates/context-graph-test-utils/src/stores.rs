//! Test store factories for RocksDB-backed teleological stores.
//!
//! Provides consistent store creation with correct initialization
//! (EmbedderIndexRegistry is set up in the constructor).

use context_graph_storage::teleological::RocksDbTeleologicalStore;
use tempfile::TempDir;

/// Create a `RocksDbTeleologicalStore` from a `TempDir` reference.
///
/// The `TempDir` must outlive the store — caller retains ownership.
pub fn create_test_store(temp_dir: &TempDir) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open(temp_dir.path()).expect("Failed to open RocksDB store")
}

/// Create a `RocksDbTeleologicalStore` from an arbitrary path.
///
/// Useful when the caller manages the directory lifetime directly.
pub fn create_initialized_store(path: &std::path::Path) -> RocksDbTeleologicalStore {
    RocksDbTeleologicalStore::open(path).expect("Failed to open store")
}
