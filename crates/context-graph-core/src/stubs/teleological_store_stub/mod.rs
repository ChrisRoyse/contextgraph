//! In-memory stub implementation of TeleologicalMemoryStore.
//!
//! # WARNING: TEST ONLY - DO NOT USE IN PRODUCTION
//!
//! This module provides `InMemoryTeleologicalStore`, a thread-safe in-memory
//! implementation of the `TeleologicalMemoryStore` trait **for testing only**.
//!
//! ## Critical Limitations
//!
//! - **O(n) search complexity**: All search operations perform full table scans.
//! - **No persistence**: All data is lost when the store is dropped.
//! - **No HNSW indexing**: Unlike production stores, this stub does not use
//!   approximate nearest neighbor search.
//!
//! ## When to Use
//!
//! - Unit tests that need a `TeleologicalMemoryStore` implementation
//! - Integration tests with small datasets (< 1000 fingerprints)
//! - Development/prototyping where persistence is not required
//!
//! ## When NOT to Use
//!
//! - Production systems (use `RocksDbTeleologicalStore` instead)
//! - Benchmarks (O(n) will skew results)
//! - Any scenario requiring data persistence
//! - Datasets larger than ~1000 fingerprints
//!
//! # Design
//!
//! - Uses `DashMap` for concurrent access without external locking
//! - No persistence - data is lost on drop
//! - Full trait implementation with real algorithms (not mocks)
//!
//! # Performance
//!
//! - **O(n) search operations** - full table scan, no indexing
//! - O(1) CRUD operations via HashMap
//! - ~46KB per fingerprint in memory

mod content;
mod search;
mod similarity;
#[cfg(test)]
mod tests;
mod trait_impl;

use std::sync::atomic::AtomicUsize;

use dashmap::DashMap;
use tracing::info;
use uuid::Uuid;

use crate::clustering::PersistedTopicPortfolio;
use crate::traits::TeleologicalStorageBackend;
use crate::types::fingerprint::TeleologicalFingerprint;
use crate::types::SourceMetadata;

/// In-memory implementation of TeleologicalMemoryStore.
///
/// # WARNING: TEST ONLY - DO NOT USE IN PRODUCTION
///
/// For production use, use `RocksDbTeleologicalStore` from `context-graph-storage`.
#[derive(Debug)]
pub struct InMemoryTeleologicalStore {
    /// Main storage: UUID -> TeleologicalFingerprint
    pub(crate) data: DashMap<Uuid, TeleologicalFingerprint>,
    /// Soft-deleted IDs (still in data but marked deleted)
    pub(crate) deleted: DashMap<Uuid, ()>,
    /// Content storage: UUID -> original content text
    pub(crate) content: DashMap<Uuid, String>,
    /// Source metadata storage: UUID -> SourceMetadata
    pub(crate) source_metadata: DashMap<Uuid, SourceMetadata>,
    /// Topic portfolio storage: session_id -> PersistedTopicPortfolio
    pub(crate) topic_portfolios: DashMap<String, PersistedTopicPortfolio>,
    /// Running size estimate in bytes
    pub(crate) size_bytes: AtomicUsize,
}

impl InMemoryTeleologicalStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        info!("Creating new InMemoryTeleologicalStore (TEST ONLY)");
        Self {
            data: DashMap::new(),
            deleted: DashMap::new(),
            content: DashMap::new(),
            source_metadata: DashMap::new(),
            topic_portfolios: DashMap::new(),
            size_bytes: AtomicUsize::new(0),
        }
    }

    /// Create with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        info!(
            "Creating InMemoryTeleologicalStore with capacity {} (TEST ONLY)",
            capacity
        );
        Self {
            data: DashMap::with_capacity(capacity),
            deleted: DashMap::new(),
            content: DashMap::with_capacity(capacity),
            source_metadata: DashMap::with_capacity(capacity),
            topic_portfolios: DashMap::new(),
            size_bytes: AtomicUsize::new(0),
        }
    }

    /// Estimate memory size of a fingerprint.
    pub(crate) fn estimate_fingerprint_size(fp: &TeleologicalFingerprint) -> usize {
        let base = std::mem::size_of::<TeleologicalFingerprint>();
        let semantic = fp.semantic.storage_size();
        // Purpose evolution was removed - now just base + semantic
        base + semantic
    }

    /// Returns the backend type.
    pub fn backend_type(&self) -> TeleologicalStorageBackend {
        TeleologicalStorageBackend::InMemory
    }
}

impl Default for InMemoryTeleologicalStore {
    fn default() -> Self {
        Self::new()
    }
}
