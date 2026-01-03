---
id: "M04-T13"
title: "Implement GraphStorage Backend"
description: |
  Implement GraphStorage struct wrapping RocksDB for graph data.
  Methods: open(path, config), get_hyperbolic(node_id), put_hyperbolic(node_id, point),
  get_cone(node_id), put_cone(node_id, cone), get_adjacency(node_id), put_adjacency(node_id, edges).
  Use Arc<DB> for thread-safe sharing.
layer: "logic"
status: "pending"
priority: "critical"
estimated_hours: 4
sequence: 17
depends_on:
  - "M04-T04"
  - "M04-T06"
  - "M04-T12"
spec_refs:
  - "TECH-GRAPH-004 Section 4.2"
files_to_create:
  - path: "crates/context-graph-graph/src/storage/rocksdb.rs"
    description: "GraphStorage implementation wrapping RocksDB"
files_to_modify:
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "Add rocksdb module and re-exports"
test_file: "crates/context-graph-graph/tests/storage_tests.rs"
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
- Column family definitions (see M04-T12)
- Schema migrations (see M04-T13a)
- Edge types and structs (see M04-T15)

## Definition of Done

### Signatures

```rust
// In crates/context-graph-graph/src/storage/rocksdb.rs

use std::path::Path;
use std::sync::Arc;

use rocksdb::{DB, ColumnFamily, WriteBatch};

use crate::error::{GraphError, GraphResult};
use crate::hyperbolic::PoincarePoint;
use crate::entailment::EntailmentCone;
use super::{
    StorageConfig, CF_ADJACENCY, CF_HYPERBOLIC, CF_CONES, CF_METADATA,
    get_column_family_descriptors, get_db_options,
};
use super::edges::GraphEdge;

/// Node ID type (8 bytes)
pub type NodeId = i64;

/// Graph storage backed by RocksDB
#[derive(Clone)]
pub struct GraphStorage {
    db: Arc<DB>,
}

impl GraphStorage {
    /// Open graph storage at the given path
    ///
    /// # Arguments
    /// * `path` - Directory path for RocksDB database
    /// * `config` - Storage configuration
    ///
    /// # Returns
    /// * `GraphResult<Self>` - Storage instance or error
    pub fn open<P: AsRef<Path>>(path: P, config: StorageConfig) -> GraphResult<Self> {
        let db_opts = get_db_options();
        let cf_descriptors = get_column_family_descriptors(&config);

        let db = DB::open_cf_descriptors(&db_opts, path.as_ref(), cf_descriptors)
            .map_err(|e| GraphError::StorageOpen {
                path: path.as_ref().to_string_lossy().into_owned(),
                cause: e.to_string(),
            })?;

        Ok(Self { db: Arc::new(db) })
    }

    /// Open with default configuration
    pub fn open_default<P: AsRef<Path>>(path: P) -> GraphResult<Self> {
        Self::open(path, StorageConfig::default())
    }

    // ========== Hyperbolic Point Operations ==========

    /// Get hyperbolic coordinates for a node
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    ///
    /// # Returns
    /// * `Option<PoincarePoint>` - Point if exists, None otherwise
    pub fn get_hyperbolic(&self, node_id: NodeId) -> GraphResult<Option<PoincarePoint>> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let point = Self::deserialize_point(&bytes)?;
                Ok(Some(point))
            }
            None => Ok(None),
        }
    }

    /// Store hyperbolic coordinates for a node
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    /// * `point` - Poincare ball coordinates
    pub fn put_hyperbolic(&self, node_id: NodeId, point: &PoincarePoint) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_point(point);

        self.db.put_cf(cf, key, value)?;
        Ok(())
    }

    /// Delete hyperbolic coordinates for a node
    pub fn delete_hyperbolic(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        Ok(())
    }

    // ========== Entailment Cone Operations ==========

    /// Get entailment cone for a node
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    ///
    /// # Returns
    /// * `Option<EntailmentCone>` - Cone if exists, None otherwise
    pub fn get_cone(&self, node_id: NodeId) -> GraphResult<Option<EntailmentCone>> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let cone = Self::deserialize_cone(&bytes)?;
                Ok(Some(cone))
            }
            None => Ok(None),
        }
    }

    /// Store entailment cone for a node
    ///
    /// # Arguments
    /// * `node_id` - Node identifier
    /// * `cone` - Entailment cone data
    pub fn put_cone(&self, node_id: NodeId, cone: &EntailmentCone) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_cone(cone);

        self.db.put_cf(cf, key, value)?;
        Ok(())
    }

    /// Delete entailment cone for a node
    pub fn delete_cone(&self, node_id: NodeId) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();

        self.db.delete_cf(cf, key)?;
        Ok(())
    }

    // ========== Adjacency List Operations ==========

    /// Get edges for a node
    ///
    /// # Arguments
    /// * `node_id` - Source node identifier
    ///
    /// # Returns
    /// * `Vec<GraphEdge>` - List of outgoing edges (empty if node not found)
    pub fn get_adjacency(&self, node_id: NodeId) -> GraphResult<Vec<GraphEdge>> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();

        match self.db.get_cf(cf, key)? {
            Some(bytes) => {
                let edges: Vec<GraphEdge> = bincode::deserialize(&bytes)
                    .map_err(|e| GraphError::CorruptedData(
                        format!("Failed to deserialize edges for node {}: {}", node_id, e)
                    ))?;
                Ok(edges)
            }
            None => Ok(Vec::new()),
        }
    }

    /// Store edges for a node
    ///
    /// # Arguments
    /// * `node_id` - Source node identifier
    /// * `edges` - List of outgoing edges
    pub fn put_adjacency(&self, node_id: NodeId, edges: &[GraphEdge]) -> GraphResult<()> {
        let cf = self.cf_adjacency()?;
        let key = node_id.to_le_bytes();
        let value = bincode::serialize(edges)
            .map_err(|e| GraphError::Serialization(format!("Failed to serialize edges: {}", e)))?;

        self.db.put_cf(cf, key, value)?;
        Ok(())
    }

    /// Add a single edge (reads existing, appends, writes back)
    pub fn add_edge(&self, source: NodeId, edge: GraphEdge) -> GraphResult<()> {
        let mut edges = self.get_adjacency(source)?;
        edges.push(edge);
        self.put_adjacency(source, &edges)
    }

    /// Remove an edge by target
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

    /// Perform multiple operations atomically
    pub fn write_batch(&self, batch: WriteBatch) -> GraphResult<()> {
        self.db.write(batch)?;
        Ok(())
    }

    /// Create a new write batch
    pub fn new_batch(&self) -> WriteBatch {
        WriteBatch::default()
    }

    /// Add hyperbolic point to batch
    pub fn batch_put_hyperbolic(&self, batch: &mut WriteBatch, node_id: NodeId, point: &PoincarePoint) -> GraphResult<()> {
        let cf = self.cf_hyperbolic()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_point(point);
        batch.put_cf(cf, key, value);
        Ok(())
    }

    /// Add cone to batch
    pub fn batch_put_cone(&self, batch: &mut WriteBatch, node_id: NodeId, cone: &EntailmentCone) -> GraphResult<()> {
        let cf = self.cf_cones()?;
        let key = node_id.to_le_bytes();
        let value = Self::serialize_cone(cone);
        batch.put_cf(cf, key, value);
        Ok(())
    }

    // ========== Iteration ==========

    /// Iterate over all hyperbolic points
    pub fn iter_hyperbolic(&self) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, PoincarePoint)>> + '_> {
        let cf = self.cf_hyperbolic()?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(key[..8].try_into().unwrap());
            let point = Self::deserialize_point(&value)?;
            Ok((node_id, point))
        }))
    }

    /// Iterate over all cones
    pub fn iter_cones(&self) -> GraphResult<impl Iterator<Item = GraphResult<(NodeId, EntailmentCone)>> + '_> {
        let cf = self.cf_cones()?;
        let iter = self.db.iterator_cf(cf, rocksdb::IteratorMode::Start);

        Ok(iter.map(|result| {
            let (key, value) = result.map_err(GraphError::from)?;
            let node_id = NodeId::from_le_bytes(key[..8].try_into().unwrap());
            let cone = Self::deserialize_cone(&value)?;
            Ok((node_id, cone))
        }))
    }

    // ========== Statistics ==========

    /// Get count of hyperbolic points stored
    pub fn hyperbolic_count(&self) -> GraphResult<usize> {
        let cf = self.cf_hyperbolic()?;
        Ok(self.db.iterator_cf(cf, rocksdb::IteratorMode::Start).count())
    }

    /// Get count of cones stored
    pub fn cone_count(&self) -> GraphResult<usize> {
        let cf = self.cf_cones()?;
        Ok(self.db.iterator_cf(cf, rocksdb::IteratorMode::Start).count())
    }

    /// Get count of nodes with adjacency lists
    pub fn adjacency_count(&self) -> GraphResult<usize> {
        let cf = self.cf_adjacency()?;
        Ok(self.db.iterator_cf(cf, rocksdb::IteratorMode::Start).count())
    }

    // ========== Internal Helpers ==========

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

    fn cf_metadata(&self) -> GraphResult<&ColumnFamily> {
        self.db.cf_handle(CF_METADATA)
            .ok_or_else(|| GraphError::ColumnFamilyNotFound(CF_METADATA.to_string()))
    }

    /// Serialize PoincarePoint to 256 bytes
    fn serialize_point(point: &PoincarePoint) -> Vec<u8> {
        // [f32; 64] = 64 * 4 = 256 bytes
        let mut bytes = Vec::with_capacity(256);
        for coord in &point.coords {
            bytes.extend_from_slice(&coord.to_le_bytes());
        }
        bytes
    }

    /// Deserialize PoincarePoint from 256 bytes
    fn deserialize_point(bytes: &[u8]) -> GraphResult<PoincarePoint> {
        if bytes.len() != 256 {
            return Err(GraphError::CorruptedData(
                format!("Expected 256 bytes for PoincarePoint, got {}", bytes.len())
            ));
        }

        let mut coords = [0.0f32; 64];
        for (i, chunk) in bytes.chunks_exact(4).enumerate() {
            coords[i] = f32::from_le_bytes(chunk.try_into().unwrap());
        }

        Ok(PoincarePoint { coords })
    }

    /// Serialize EntailmentCone to 268 bytes
    fn serialize_cone(cone: &EntailmentCone) -> Vec<u8> {
        // 256 (apex) + 4 (aperture) + 4 (aperture_factor) + 4 (depth) = 268 bytes
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

        bytes
    }

    /// Deserialize EntailmentCone from 268 bytes
    fn deserialize_cone(bytes: &[u8]) -> GraphResult<EntailmentCone> {
        if bytes.len() != 268 {
            return Err(GraphError::CorruptedData(
                format!("Expected 268 bytes for EntailmentCone, got {}", bytes.len())
            ));
        }

        // Apex (256 bytes)
        let apex_point = Self::deserialize_point(&bytes[..256])?;

        // Aperture (4 bytes)
        let aperture = f32::from_le_bytes(bytes[256..260].try_into().unwrap());

        // Aperture factor (4 bytes)
        let aperture_factor = f32::from_le_bytes(bytes[260..264].try_into().unwrap());

        // Depth (4 bytes)
        let depth = u32::from_le_bytes(bytes[264..268].try_into().unwrap());

        Ok(EntailmentCone {
            apex: apex_point,
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

### Constraints
- PoincarePoint serialized to exactly 256 bytes (64 * 4)
- EntailmentCone serialized to exactly 268 bytes (256 + 4 + 4 + 4)
- NodeId is i64 (8 bytes, little-endian)
- Use bincode for edge serialization
- Arc<DB> for thread-safe sharing

### Acceptance Criteria
- [ ] open() creates DB with all 4 CFs
- [ ] get_hyperbolic() deserializes 256 bytes to PoincarePoint
- [ ] put_hyperbolic() serializes point to 256 bytes
- [ ] get_cone() deserializes 268 bytes to EntailmentCone
- [ ] Proper error handling with GraphError variants
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. open(path, config):
   - Create DB options
   - Get CF descriptors
   - Open DB with all CFs
   - Wrap in Arc<DB>

2. get_hyperbolic(node_id):
   - Convert node_id to bytes
   - Get from CF_HYPERBOLIC
   - Deserialize 256 bytes to PoincarePoint

3. serialize_point(point):
   - Iterate 64 f32 coords
   - Write each as 4 little-endian bytes
   - Total: 256 bytes

4. serialize_cone(cone):
   - Write apex (256 bytes)
   - Write aperture (4 bytes)
   - Write aperture_factor (4 bytes)
   - Write depth (4 bytes)
   - Total: 268 bytes

### Edge Cases
- Corrupted data: Return CorruptedData error
- Missing CF: Return ColumnFamilyNotFound
- Empty adjacency: Return empty Vec
- Concurrent writes: RocksDB handles internally

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph storage
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] Database opens successfully
- [ ] Point roundtrip preserves data
- [ ] Cone roundtrip preserves data
- [ ] Edge list roundtrip preserves data

### Test Cases

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_open_storage() {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();
        assert_eq!(storage.hyperbolic_count().unwrap(), 0);
    }

    #[test]
    fn test_hyperbolic_roundtrip() {
        let dir = tempdir().unwrap();
        let storage = GraphStorage::open_default(dir.path()).unwrap();

        let point = PoincarePoint::origin();
        storage.put_hyperbolic(42, &point).unwrap();

        let loaded = storage.get_hyperbolic(42).unwrap().unwrap();
        assert_eq!(loaded.coords, point.coords);
    }

    #[test]
    fn test_point_serialization_size() {
        let point = PoincarePoint::origin();
        let bytes = GraphStorage::serialize_point(&point);
        assert_eq!(bytes.len(), 256);
    }

    #[test]
    fn test_cone_serialization_size() {
        let cone = EntailmentCone {
            apex: PoincarePoint::origin(),
            aperture: 1.0,
            aperture_factor: 1.0,
            depth: 0,
        };
        let bytes = GraphStorage::serialize_cone(&cone);
        assert_eq!(bytes.len(), 268);
    }
}
```
