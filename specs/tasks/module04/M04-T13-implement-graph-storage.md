---
id: "M04-T13"
title: "Implement GraphStorage Backend"
description: |
  Implement GraphStorage struct wrapping RocksDB for graph data.
  Methods: open(path, config), get_hyperbolic(node_id), put_hyperbolic(node_id, point),
  get_cone(node_id), put_cone(node_id, cone), get_adjacency(node_id), put_adjacency(node_id, edges).
  Use Arc<DB> for thread-safe sharing.
layer: "logic"
status: "completed"
priority: "critical"
estimated_hours: 4
sequence: 17
depends_on:
  - "M04-T04"  # PoincarePoint type (64D hyperbolic coordinates)
  - "M04-T06"  # EntailmentCone type (apex + aperture + factor + depth)
  - "M04-T12"  # Column family definitions (COMPLETED)
spec_refs:
  - "TECH-GRAPH-004 Section 4.2"
files_created:
  - path: "crates/context-graph-graph/src/storage/storage_impl.rs"
    description: "GraphStorage implementation wrapping RocksDB (844 lines)"
files_modified:
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "Added storage_impl module and re-exports"
test_file: "crates/context-graph-graph/tests/storage_tests.rs"
sherlock_verified: "2026-01-03"
sherlock_verdict: "VERIFIED"
tests_passing: 76
---

## CRITICAL: Read Before Starting

### Constitution Reference (docs2/constitution.yaml)

```yaml
rules:
  - AP-001: "Never unwrap() in prod - use expect() with context"
  - SEC-06: "Soft delete 30-day recovery"

tech:
  lang: "Rust 1.75+, edition 2021"
  db:
    storage: "RocksDB 0.22"
    vector: "faiss_gpu"
  gpu: "RTX 5090, 32GB VRAM, CUDA 13.1"

performance:
  faiss_1M_k100: "< 2ms"
  entailment_check: "< 1ms"
```

### MANDATORY RULES

1. **NO BACKWARDS COMPATIBILITY** - Fail fast with robust error logging
2. **NO MOCK DATA IN TESTS** - Use real RocksDB instances only
3. **NO unwrap()** - Use `?` operator or `expect("context")`
4. **Result<T, GraphError>** for all fallible operations

---

## Current Codebase State

### M04-T12 Column Families (COMPLETED)

The following are already implemented in `crates/context-graph-graph/src/storage/mod.rs`:

```rust
// Column family names - ALREADY DEFINED
pub const CF_ADJACENCY: &str = "adjacency";        // Edge lists (bincode serialized)
pub const CF_HYPERBOLIC: &str = "hyperbolic";      // Poincaré coordinates (256 bytes)
pub const CF_CONES: &str = "entailment_cones";     // Entailment cones (268 bytes)
pub const CF_FAISS_IDS: &str = "faiss_ids";        // FAISS ID mappings (8 bytes)
pub const CF_NODES: &str = "nodes";                // Node metadata (JSON)
pub const CF_METADATA: &str = "metadata";          // Schema version, stats

pub const ALL_COLUMN_FAMILIES: &[&str] = &[
    CF_ADJACENCY, CF_HYPERBOLIC, CF_CONES, CF_FAISS_IDS, CF_NODES, CF_METADATA
];

// StorageConfig - ALREADY DEFINED with Default, read_optimized(), write_optimized(), validate()
// get_column_family_descriptors(&StorageConfig) - ALREADY DEFINED
// get_db_options() - ALREADY DEFINED
```

### Error Types (crates/context-graph-graph/src/error.rs)

The following error variants are available for use:

```rust
pub enum GraphError {
    // Storage errors - USE THESE
    StorageOpen { path: String, cause: String },  // DB open failed
    Storage(String),                              // General RocksDB error
    ColumnFamilyNotFound(String),                 // CF missing
    CorruptedData { location: String, details: String },  // Deserialization failed

    // Validation errors
    DimensionMismatch { expected: usize, actual: usize },
    InvalidConfig(String),
    NodeNotFound(String),
    EdgeNotFound(String, String),

    // Serialization
    Serialization(String),
    Deserialization(String),
}

// Already implemented:
impl From<rocksdb::Error> for GraphError  // Converts to GraphError::Storage
impl From<bincode::Error> for GraphError  // Converts to GraphError::Deserialization
impl From<serde_json::Error> for GraphError  // Converts to GraphError::Serialization
```

### Existing Storage Tests (tests/storage_tests.rs)

Tests already exist for M04-T12 that verify:
- Column family names and count
- StorageConfig defaults and validation
- Real RocksDB open/read/write operations
- Hyperbolic coordinate storage (256 bytes)
- Entailment cone storage (268 bytes)
- FAISS ID mapping (8 bytes)
- Prefix scan for adjacency lists
- Data persistence across reopen

### Dependencies NOT YET Implemented

**IMPORTANT:** These types are NOT YET DEFINED - they come from M04-T04 and M04-T06:

- `PoincarePoint` - 64D hyperbolic coordinates (M04-T04)
- `EntailmentCone` - Cone with apex, aperture, factor, depth (M04-T06)
- `GraphEdge` - Edge with target, type, NT weights (M04-T15)

For M04-T13, create placeholder types if dependencies are not complete:

```rust
// Placeholder if M04-T04 not complete
#[derive(Debug, Clone, PartialEq)]
pub struct PoincarePoint {
    pub coords: [f32; 64],  // 64 * 4 = 256 bytes
}

impl PoincarePoint {
    pub fn origin() -> Self {
        Self { coords: [0.0; 64] }
    }
}

// Placeholder if M04-T06 not complete
#[derive(Debug, Clone)]
pub struct EntailmentCone {
    pub apex: PoincarePoint,   // 256 bytes
    pub aperture: f32,         // 4 bytes
    pub aperture_factor: f32,  // 4 bytes
    pub depth: u32,            // 4 bytes
}  // Total: 268 bytes
```

---

## Context

GraphStorage provides a type-safe Rust interface to RocksDB for persisting knowledge graph data. It handles serialization of PoincarePoint (256 bytes), EntailmentCone (268 bytes), and edge lists. Thread-safe sharing via Arc<DB> enables concurrent read operations while RocksDB handles write concurrency internally.

## Scope

### In Scope
- GraphStorage struct with Arc<DB>
- open() with path and StorageConfig
- Hyperbolic point CRUD operations
- Entailment cone CRUD operations
- Adjacency list CRUD operations
- Proper error handling with GraphError

### Out of Scope
- Column family definitions (M04-T12 - COMPLETED)
- Schema migrations (M04-T13a)
- Edge types and structs (M04-T15)

---

## Definition of Done

### File to Create: `crates/context-graph-graph/src/storage/rocksdb.rs`

```rust
//! GraphStorage backend wrapping RocksDB.
//!
//! Provides type-safe persistence for hyperbolic coordinates, entailment cones,
//! and edge adjacency lists.
//!
//! # Constitution Reference
//!
//! - AP-001: Never unwrap() in prod - all errors properly typed
//! - rules: Result<T,E> for fallible ops, thiserror for derivation
//!
//! # Binary Formats
//!
//! - PoincarePoint: 256 bytes (64 f32 little-endian)
//! - EntailmentCone: 268 bytes (256 apex + 4 aperture + 4 factor + 4 depth)
//! - NodeId: 8 bytes (i64 little-endian)
//! - Edges: bincode serialized Vec<GraphEdge>

use std::path::Path;
use std::sync::Arc;

use rocksdb::{DB, ColumnFamily, WriteBatch, IteratorMode};

use crate::error::{GraphError, GraphResult};
use super::{
    StorageConfig, CF_ADJACENCY, CF_HYPERBOLIC, CF_CONES, CF_METADATA,
    get_column_family_descriptors, get_db_options,
};

// ========== Type Aliases ==========

/// Node ID type (8 bytes, little-endian)
pub type NodeId = i64;

// ========== Placeholder Types (until M04-T04, M04-T06 complete) ==========

/// 64D Poincaré ball coordinates
#[derive(Debug, Clone, PartialEq)]
pub struct PoincarePoint {
    pub coords: [f32; 64],  // 256 bytes
}

impl PoincarePoint {
    pub fn origin() -> Self {
        Self { coords: [0.0; 64] }
    }
}

/// Entailment cone for hierarchical reasoning
#[derive(Debug, Clone)]
pub struct EntailmentCone {
    pub apex: PoincarePoint,   // 256 bytes
    pub aperture: f32,         // 4 bytes
    pub aperture_factor: f32,  // 4 bytes
    pub depth: u32,            // 4 bytes
}  // Total: 268 bytes

/// Graph edge (placeholder until M04-T15)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphEdge {
    pub target: NodeId,
    pub edge_type: u8,
    // NT weights to be added in M04-T15
}

// ========== GraphStorage ==========

/// Graph storage backed by RocksDB
///
/// Thread-safe via Arc<DB>. Clone is cheap (Arc clone).
#[derive(Clone)]
pub struct GraphStorage {
    db: Arc<DB>,
}

impl GraphStorage {
    /// Open graph storage at the given path.
    ///
    /// # Arguments
    /// * `path` - Directory path for RocksDB database
    /// * `config` - Storage configuration (use StorageConfig::default())
    ///
    /// # Errors
    /// * `GraphError::StorageOpen` - Failed to open database
    /// * `GraphError::InvalidConfig` - Invalid configuration
    ///
    /// # Example
    /// ```rust,ignore
    /// let storage = GraphStorage::open("/data/graph.db", StorageConfig::default())?;
    /// ```
    pub fn open<P: AsRef<Path>>(path: P, config: StorageConfig) -> GraphResult<Self> {
        let db_opts = get_db_options();
        let cf_descriptors = get_column_family_descriptors(&config)?;

        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), cf_descriptors)
            .map_err(|e| GraphError::StorageOpen {
                path: path.as_ref().to_string_lossy().into_owned(),
                cause: e.to_string(),
            })?;

        log::info!("GraphStorage opened at {:?}", path.as_ref());

        Ok(Self { db: Arc::new(db) })
    }

    /// Open with default configuration.
    pub fn open_default<P: AsRef<Path>>(path: P) -> GraphResult<Self> {
        Self::open(path, StorageConfig::default())
    }

    // ========== Hyperbolic Point Operations ==========

    /// Get hyperbolic coordinates for a node.
    ///
    /// # Returns
    /// * `Ok(Some(point))` - Point exists
    /// * `Ok(None)` - Node not found
    /// * `Err(GraphError::CorruptedData)` - Invalid data in storage
    pub fn get_hyperbolic(&self, node_id: NodeId) -> GraphResult<Option<PoincarePoint>> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let point = Self::deserialize_point(&bytes).map_err(|e| {
                    log::error!("CORRUPTED: hyperbolic node_id={}: {}", node_id, e);
                    e
                })?;
                Ok(Some(point))
            }
            None => Ok(None),
        }
    }

    /// Store hyperbolic coordinates for a node.
    ///
    /// Overwrites existing coordinates if present.
    pub fn put_hyperbolic(&self, node_id: NodeId, point: &PoincarePoint) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_point(point);

        self.db.put_cf(cf, key, value)?;
        log::trace!("PUT hyperbolic node_id={}", node_id);
        Ok(())
    }

    /// Delete hyperbolic coordinates for a node.
    pub fn delete_hyperbolic(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        log::trace!("DELETE hyperbolic node_id={}", node_id);
        Ok(())
    }

    // ========== Entailment Cone Operations ==========

    /// Get entailment cone for a node.
    pub fn get_cone(&self, node_id: NodeId) -> GraphResult<Option<EntailmentCone>> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let cone = Self::deserialize_cone(&bytes).map_err(|e| {
                    log::error!("CORRUPTED: cone node_id={}: {}", node_id, e);
                    e
                })?;
                Ok(Some(cone))
            }
            None => Ok(None),
        }
    }

    /// Store entailment cone for a node.
    pub fn put_cone(&self, node_id: NodeId, cone: &EntailmentCone) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_cone(cone);

        self.db.put_cf(cf, key, value)?;
        log::trace!("PUT cone node_id={}", node_id);
        Ok(())
    }

    /// Delete entailment cone for a node.
    pub fn delete_cone(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        log::trace!("DELETE cone node_id={}", node_id);
        Ok(())
    }

    // ========== Adjacency List Operations ==========

    /// Get edges for a node.
    ///
    /// Returns empty Vec if node has no edges.
    pub fn get_adjacency(&self, node_id: NodeId) -> GraphResult<Vec<GraphEdge>> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let edges: Vec<GraphEdge> = bincode::deserialize(&bytes)
                    .map_err(|e| GraphError::CorruptedData {
                        location: format!("adjacency node_id={}", node_id),
                        details: e.to_string(),
                    })?;
                Ok(edges)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Store edges for a node.
    pub fn put_adjacency(&self, node_id: NodeId, edges: &[GraphEdge]) -> GraphResult<()> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();
        let value = bincode::serialize(edges)?;

        self.db.put_cf(cf, key, value)?;
        log::trace!("PUT adjacency node_id={} edges={}", node_id, edges.len());
        Ok(())
    }

    /// Add a single edge (reads existing, appends, writes back).
    pub fn add_edge(&self, source: NodeId, edge: GraphEdge) -> GraphResult<()> {
        let mut edges = self.get_adjacency(source)?;
        edges.push(edge);
        self.put_adjacency(source, &edges)
    }

    /// Remove an edge by target node.
    pub fn remove_edge(&self, source: NodeId, target: NodeId) -> GraphResult<bool> {
        let mut edges = self.get_adjacency(source)?;
        let original_len = edges.len();
        edges.retain(|e| e.target != target);

        if edges.len() < original_len {
            self.put_adjacency(source, &edges)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // ========== Batch Operations ==========

    /// Perform multiple operations atomically.
    pub fn write_batch(&self, batch: WriteBatch) -> GraphResult<()> {
        self.db.write(batch)?;
        Ok(())
    }

    /// Create a new write batch.
    pub fn new_batch(&self) -> WriteBatch {
        WriteBatch::default()
    }

    /// Add hyperbolic point to batch.
    pub fn batch_put_hyperbolic(
        &self,
        batch: &mut WriteBatch,
        node_id: NodeId,
        point: &PoincarePoint,
    ) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_point(point);
        batch.put_cf(cf, key, value);
        Ok(())
    }

    /// Add cone to batch.
    pub fn batch_put_cone(
        &self,
        batch: &mut WriteBatch,
        node_id: NodeId,
        cone: &EntailmentCone,
    ) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_cone(cone);
        batch.put_cf(cf, key, value);
        Ok(())
    }

    // ========== Iteration ==========

    /// Iterate over all hyperbolic points.
    pub fn iter_hyperbolic(&self) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, PoincarePoint)>> + '_> {
        let cf = self.cf_hyperbolic()?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(
                key[..8].try_into().expect("NodeId key must be 8 bytes")
            );
            let point = Self::deserialize_point(&value)?;
            Ok((node_id, point))
        }))
    }

    /// Iterate over all cones.
    pub fn iter_cones(&self) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, EntailmentCone)>> + '_> {
        let cf = self.cf_cones()?;
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(
                key[..8].try_into().expect("NodeId key must be 8 bytes")
            );
            let cone = Self::deserialize_cone(&value)?;
            Ok((node_id, cone))
        }))
    }

    // ========== Statistics ==========

    /// Get count of hyperbolic points stored.
    pub fn hyperbolic_count(&self) -> GraphResult<usize> {
        let cf = self.cf_hyperbolic()?;
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Get count of cones stored.
    pub fn cone_count(&self) -> GraphResult<usize> {
        let cf = self.cf_cones()?;
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    /// Get count of nodes with adjacency lists.
    pub fn adjacency_count(&self) -> GraphResult<usize> {
        let cf = self.cf_adjacency()?;
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    // ========== Column Family Helpers ==========

    fn cf_hyperbolic(&self) -> GraphResult<&ColumnFamily> {
        self.db.cf_handle(CF_HYPERBOLIC)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_HYPERBOLIC.to_string()))
    }

    fn cf_cones(&self) -> GraphResult<&ColumnFamily> {
        self.db.cf_handle(CF_CONES)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_CONES.to_string()))
    }

    fn cf_adjacency(&self) -> GraphResult<&ColumnFamily> {
        self.db.cf_handle(CF_ADJACENCY)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_ADJACENCY.to_string()))
    }

    pub(crate) fn cf_metadata(&self) -> GraphResult<&ColumnFamily> {
        self.db.cf_handle(CF_METADATA)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_METADATA.to_string()))
    }

    // ========== Serialization ==========

    /// Serialize PoincarePoint to exactly 256 bytes.
    fn serialize_point(point: &PoincarePoint) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(256);
        for coord in &point.coords {
            bytes.extend_from_slice(&coord.to_le_bytes());
        }
        debug_assert_eq!(bytes.len(), 256);
        bytes
    }

    /// Deserialize PoincarePoint from 256 bytes.
    fn deserialize_point(bytes: &[u8]) -> GraphResult<PoincarePoint> {
        if bytes.len() != 256 {
            return Err(GraphError::CorruptedData {
                location: "PoincarePoint".to_string(),
                details: format!("Expected 256 bytes, got {}", bytes.len()),
            });
        }

        let mut coords = [0.0f32; 64];
        for (i, chunk) in bytes.chunks_exact(4).enumerate() {
            coords[i] = f32::from_le_bytes(
                chunk.try_into().expect("chunk is exactly 4 bytes")
            );
        }

        Ok(PoincarePoint { coords })
    }

    /// Serialize EntailmentCone to exactly 268 bytes.
    fn serialize_cone(cone: &EntailmentCone) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(268);

        // Apex coordinates (256 bytes)
        for coord in &cone.apex.coords {
            bytes.extend_from_slice(&coord.to_le_bytes());
        }

        // Aperture (4 bytes)
        bytes.extend_from_slice(&cone.aperture.to_le_bytes());

        // Aperture factor (4 bytes)
        bytes.extend_from_slice(&cone.aperture_factor.to_le_bytes());

        // Depth (4 bytes)
        bytes.extend_from_slice(&cone.depth.to_le_bytes());

        debug_assert_eq!(bytes.len(), 268);
        bytes
    }

    /// Deserialize EntailmentCone from 268 bytes.
    fn deserialize_cone(bytes: &[u8]) -> GraphResult<EntailmentCone> {
        if bytes.len() != 268 {
            return Err(GraphError::CorruptedData {
                location: "EntailmentCone".to_string(),
                details: format!("Expected 268 bytes, got {}", bytes.len()),
            });
        }

        let apex = Self::deserialize_point(&bytes[..256])?;
        let aperture = f32::from_le_bytes(bytes[256..260].try_into().expect("4 bytes"));
        let aperture_factor = f32::from_le_bytes(bytes[260..264].try_into().expect("4 bytes"));
        let depth = u32::from_le_bytes(bytes[264..268].try_into().expect("4 bytes"));

        Ok(EntailmentCone {
            apex,
            aperture,
            aperture_factor,
            depth,
        })
    }
}

impl std::fmt::Debug for GraphStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GraphStorage")
            .field("hyperbolic_count", &self.hyperbolic_count().unwrap_or(0))
            .field("cone_count", &self.cone_count().unwrap_or(0))
            .field("adjacency_count", &self.adjacency_count().unwrap_or(0))
            .finish()
    }
}
```

### File to Modify: `crates/context-graph-graph/src/storage/mod.rs`

Add at the end of the file:

```rust
// Add this module declaration
pub mod rocksdb;

// Re-export GraphStorage and types
pub use rocksdb::{GraphStorage, NodeId, PoincarePoint, EntailmentCone, GraphEdge};
```

---

## Constraints

| Constraint | Value | Enforcement |
|------------|-------|-------------|
| PoincarePoint size | 256 bytes | `debug_assert_eq!` in serialize |
| EntailmentCone size | 268 bytes | `debug_assert_eq!` in serialize |
| NodeId type | i64 (8 bytes LE) | Type alias |
| Edge serialization | bincode | `bincode::serialize/deserialize` |
| Thread safety | Arc<DB> | Clone trait |
| Error handling | GraphError | No unwrap() in prod |

---

## Acceptance Criteria

### Build & Test
- [ ] `cargo build -p context-graph-graph` compiles without errors
- [ ] `cargo test -p context-graph-graph storage` passes all tests
- [ ] `cargo clippy -p context-graph-graph -- -D warnings` passes

### Functional
- [ ] `GraphStorage::open()` creates DB with all 6 CFs
- [ ] `get_hyperbolic()` deserializes 256 bytes to PoincarePoint
- [ ] `put_hyperbolic()` serializes point to 256 bytes
- [ ] `get_cone()` deserializes 268 bytes to EntailmentCone
- [ ] `put_cone()` serializes cone to 268 bytes
- [ ] `get_adjacency()` returns empty Vec for missing node
- [ ] `put_adjacency()` stores edges with bincode
- [ ] Data persists across database reopen

---

## Full State Verification Requirements

### 1. Define Source of Truth

| Data Type | Source of Truth | Verification Method |
|-----------|-----------------|---------------------|
| Hyperbolic Point | RocksDB `hyperbolic` CF | `db.get_cf(hyperbolic_cf, node_id_bytes)` |
| Entailment Cone | RocksDB `entailment_cones` CF | `db.get_cf(cones_cf, node_id_bytes)` |
| Adjacency List | RocksDB `adjacency` CF | `db.get_cf(adjacency_cf, node_id_bytes)` |
| Schema Version | RocksDB `metadata` CF | `db.get_cf(metadata_cf, b"schema_version")` |

### 2. Execute & Inspect Pattern

Every write operation MUST be followed by a separate read to verify:

```rust
// CORRECT: Write then read back
storage.put_hyperbolic(node_id, &point)?;
let stored = storage.get_hyperbolic(node_id)?;
assert_eq!(stored, Some(point), "Hyperbolic point not stored correctly");

// WRONG: Assume write succeeded
storage.put_hyperbolic(node_id, &point)?;  // No verification!
```

### 3. Required Edge Case Tests (3 minimum with BEFORE/AFTER logging)

```rust
#[test]
fn test_edge_case_empty_adjacency() {
    // Edge case: Node with no edges
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: Querying adjacency for non-existent node 999");
    let edges = storage.get_adjacency(999).unwrap();
    println!("AFTER: Got {} edges (expected 0)", edges.len());

    assert!(edges.is_empty(), "Non-existent node should have empty adjacency");
}

#[test]
fn test_edge_case_overwrite_hyperbolic() {
    // Edge case: Overwriting existing coordinates
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let point1 = PoincarePoint::origin();
    let mut point2 = PoincarePoint::origin();
    point2.coords[0] = 0.5;

    println!("BEFORE: Writing initial point for node 1");
    storage.put_hyperbolic(1, &point1).unwrap();
    let stored1 = storage.get_hyperbolic(1).unwrap().unwrap();
    println!("AFTER WRITE 1: coords[0] = {}", stored1.coords[0]);

    println!("BEFORE: Overwriting point for node 1");
    storage.put_hyperbolic(1, &point2).unwrap();
    let stored2 = storage.get_hyperbolic(1).unwrap().unwrap();
    println!("AFTER WRITE 2: coords[0] = {}", stored2.coords[0]);

    assert!((stored2.coords[0] - 0.5).abs() < 0.0001, "Overwrite must update value");
}

#[test]
fn test_edge_case_corrupted_point_data() {
    // Edge case: Corrupted data detection
    let temp_dir = tempfile::tempdir().unwrap();
    let db_opts = get_db_options();
    let config = StorageConfig::default();
    let cf_descriptors = get_column_family_descriptors(&config).unwrap();

    let db = rocksdb::DB::open_cf_descriptors(&db_opts, temp_dir.path(), cf_descriptors).unwrap();

    // Write invalid data directly (only 100 bytes instead of 256)
    let hyperbolic_cf = db.cf_handle(CF_HYPERBOLIC).unwrap();
    let invalid_data: Vec<u8> = vec![0u8; 100];
    let node_id: i64 = 42;

    println!("BEFORE: Writing 100 bytes (invalid) for node 42");
    db.put_cf(hyperbolic_cf, node_id.to_le_bytes(), &invalid_data).unwrap();
    drop(db);

    // Reopen via GraphStorage
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("BEFORE: Attempting to read corrupted data");
    let result = storage.get_hyperbolic(42);
    println!("AFTER: Result = {:?}", result.is_err());

    assert!(result.is_err(), "Must detect corrupted data");
    match result {
        Err(GraphError::CorruptedData { location, details }) => {
            assert!(location.contains("PoincarePoint") || details.contains("256"));
        }
        _ => panic!("Expected CorruptedData error"),
    }
}
```

### 4. Evidence of Success

Test output MUST show:
```
BEFORE: Opening RocksDB at /tmp/xxx/test.db
AFTER: RocksDB opened successfully
BEFORE: Writing hyperbolic point for node 1
AFTER: Stored 256 bytes, verified read matches write
BEFORE: Testing persistence across reopen
AFTER: Data persisted correctly (value: [0.0, 0.0, ...])
```

---

## Test Cases (NO MOCKS - Real RocksDB Only)

Add to `crates/context-graph-graph/tests/storage_tests.rs`:

```rust
// ========== M04-T13: GraphStorage Tests ==========

use context_graph_graph::storage::{GraphStorage, NodeId, PoincarePoint, EntailmentCone, GraphEdge};

#[test]
fn test_graph_storage_open() {
    let temp_dir = tempfile::tempdir().unwrap();
    println!("BEFORE: Opening GraphStorage at {:?}", temp_dir.path());

    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    println!("AFTER: GraphStorage opened successfully");
    assert_eq!(storage.hyperbolic_count().unwrap(), 0);
    assert_eq!(storage.cone_count().unwrap(), 0);
    assert_eq!(storage.adjacency_count().unwrap(), 0);
}

#[test]
fn test_hyperbolic_roundtrip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let point = PoincarePoint::origin();

    println!("BEFORE: Storing hyperbolic point for node 42");
    storage.put_hyperbolic(42, &point).unwrap();

    // VERIFY: Read back and compare
    let loaded = storage.get_hyperbolic(42).unwrap();
    println!("AFTER: Loaded point = {:?}", loaded.is_some());

    assert!(loaded.is_some(), "Point must exist after put");
    assert_eq!(loaded.unwrap().coords, point.coords, "Coords must match");
}

#[test]
fn test_cone_roundtrip() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let cone = EntailmentCone {
        apex: PoincarePoint::origin(),
        aperture: 0.5,
        aperture_factor: 1.0,
        depth: 3,
    };

    println!("BEFORE: Storing entailment cone for node 100");
    storage.put_cone(100, &cone).unwrap();

    // VERIFY: Read back and compare
    let loaded = storage.get_cone(100).unwrap().unwrap();
    println!("AFTER: aperture={}, depth={}", loaded.aperture, loaded.depth);

    assert!((loaded.aperture - 0.5).abs() < 0.0001);
    assert_eq!(loaded.depth, 3);
}

#[test]
fn test_adjacency_operations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let edge1 = GraphEdge { target: 2, edge_type: 1 };
    let edge2 = GraphEdge { target: 3, edge_type: 2 };

    println!("BEFORE: Adding edges to node 1");
    storage.add_edge(1, edge1).unwrap();
    storage.add_edge(1, edge2).unwrap();

    // VERIFY: Read back
    let edges = storage.get_adjacency(1).unwrap();
    println!("AFTER: Found {} edges", edges.len());

    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0].target, 2);
    assert_eq!(edges[1].target, 3);
}

#[test]
fn test_data_persists_across_reopen() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().to_path_buf();

    // First open: write data
    {
        let storage = GraphStorage::open_default(&db_path).unwrap();
        storage.put_hyperbolic(1, &PoincarePoint::origin()).unwrap();
        println!("BEFORE CLOSE: Wrote hyperbolic point for node 1");
    }

    // Second open: verify data persisted
    {
        let storage = GraphStorage::open_default(&db_path).unwrap();
        let point = storage.get_hyperbolic(1).unwrap();
        println!("AFTER REOPEN: Point exists = {}", point.is_some());

        assert!(point.is_some(), "Data must persist across reopen");
    }
}

#[test]
fn test_batch_operations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage = GraphStorage::open_default(temp_dir.path()).unwrap();

    let mut batch = storage.new_batch();

    let point1 = PoincarePoint::origin();
    let point2 = PoincarePoint::origin();
    let cone = EntailmentCone {
        apex: PoincarePoint::origin(),
        aperture: 1.0,
        aperture_factor: 1.0,
        depth: 0,
    };

    println!("BEFORE: Adding 3 operations to batch");
    storage.batch_put_hyperbolic(&mut batch, 1, &point1).unwrap();
    storage.batch_put_hyperbolic(&mut batch, 2, &point2).unwrap();
    storage.batch_put_cone(&mut batch, 1, &cone).unwrap();

    storage.write_batch(batch).unwrap();
    println!("AFTER: Batch committed");

    // VERIFY: All data written
    assert_eq!(storage.hyperbolic_count().unwrap(), 2);
    assert_eq!(storage.cone_count().unwrap(), 1);
}
```

---

## Verification Commands

```bash
# Build
cargo build -p context-graph-graph

# Run all storage tests
cargo test -p context-graph-graph storage -- --nocapture

# Run specific M04-T13 tests
cargo test -p context-graph-graph graph_storage -- --nocapture

# Clippy (must pass with no warnings)
cargo clippy -p context-graph-graph -- -D warnings
```

---

## Sherlock-Holmes Final Verification

**YOU MUST USE THE sherlock-holmes SUBAGENT TO VERIFY THIS TASK IS COMPLETE.**

### Verification Checklist for Sherlock

1. **File Exists**: `crates/context-graph-graph/src/storage/rocksdb.rs` exists and is not empty
2. **Module Exported**: `storage/mod.rs` contains `pub mod rocksdb;` and re-exports
3. **Compiles**: `cargo build -p context-graph-graph` succeeds
4. **Tests Pass**: `cargo test -p context-graph-graph storage` all pass
5. **No Clippy Warnings**: `cargo clippy -p context-graph-graph -- -D warnings` passes
6. **Physical Verification**:
   - Create temp RocksDB database
   - Write hyperbolic point, cone, and edges
   - Read back and verify data matches
   - Reopen database and verify persistence
7. **Edge Cases Tested**:
   - Empty adjacency returns empty Vec
   - Corrupted data returns CorruptedData error
   - Overwrite updates value correctly

### Sherlock Invocation

```
Use Task tool with subagent_type="sherlock-holmes" and prompt:

"Forensically verify M04-T13 GraphStorage implementation:
1. Verify rocksdb.rs exists with GraphStorage struct
2. Verify mod.rs exports the module
3. Run cargo build and cargo test
4. Execute tests with --nocapture to see BEFORE/AFTER logs
5. Verify all 3 edge case tests exist and pass
6. Check that no unwrap() exists in production code (only in tests)
7. Report: VERIFIED or FAILED with evidence"
```

---

## Implementation Order

1. Create `storage/rocksdb.rs` with placeholder types
2. Add module to `storage/mod.rs`
3. Implement `GraphStorage::open()` and verify DB opens with all CFs
4. Implement hyperbolic operations with serialization
5. Implement cone operations with serialization
6. Implement adjacency operations with bincode
7. Add batch operations
8. Add iteration and statistics
9. Write tests with BEFORE/AFTER logging
10. Run sherlock-holmes verification
