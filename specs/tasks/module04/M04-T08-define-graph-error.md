---
id: "M04-T08"
title: "Define GraphError Enum"
description: |
  Implement comprehensive GraphError enum for knowledge graph operations.
  Variants: FaissIndexCreation, FaissTrainingFailed, FaissSearchFailed, FaissAddFailed,
  IndexNotTrained, InsufficientTrainingData, GpuResourceAllocation, GpuTransferFailed,
  StorageOpen, Storage, ColumnFamilyNotFound, CorruptedData, VectorIdMismatch, InvalidConfig,
  NodeNotFound, EdgeNotFound, InvalidHyperbolicPoint.
  Use thiserror for derivation.
layer: "foundation"
status: "pending"
priority: "high"
estimated_hours: 1.5
sequence: 11
depends_on:
  - "M04-T00"
spec_refs:
  - "TECH-GRAPH-004 Section 9"
files_to_create: []
files_to_modify:
  - path: "crates/context-graph-graph/src/error.rs"
    description: "Define GraphError enum with all variants"
test_file: "crates/context-graph-graph/tests/error_tests.rs"
---

## Context

GraphError provides comprehensive error handling for all knowledge graph operations. Clear, descriptive errors are essential for debugging FAISS GPU operations, RocksDB storage issues, and hyperbolic geometry edge cases. Using thiserror allows automatic Error trait implementation with human-readable messages.

## Scope

### In Scope
- Define GraphError enum with 17+ variants
- Use thiserror derive macro
- Add descriptive error messages with context
- Ensure Error is Send + Sync for async compatibility

### Out of Scope
- From trait implementations (see M04-T08a)
- Error recovery logic

## Definition of Done

### Signatures

```rust
use thiserror::Error;

/// Comprehensive error type for knowledge graph operations
#[derive(Error, Debug)]
pub enum GraphError {
    // ========== FAISS GPU Index Errors ==========

    /// Failed to create FAISS index
    #[error("Failed to create FAISS index: {0}")]
    FaissIndexCreation(String),

    /// Failed to train FAISS index
    #[error("Failed to train FAISS index: {0}")]
    FaissTrainingFailed(String),

    /// Failed to search FAISS index
    #[error("FAISS search failed: {0}")]
    FaissSearchFailed(String),

    /// Failed to add vectors to FAISS index
    #[error("Failed to add vectors to FAISS index: {0}")]
    FaissAddFailed(String),

    /// Index must be trained before search/add operations
    #[error("Index not trained - call train() with sufficient vectors before search/add")]
    IndexNotTrained,

    /// Not enough vectors provided for training
    #[error("Insufficient training data: provided {provided} vectors, required {required}")]
    InsufficientTrainingData {
        provided: usize,
        required: usize,
    },

    // ========== GPU Resource Errors ==========

    /// Failed to allocate GPU resources
    #[error("GPU resource allocation failed: {0}")]
    GpuResourceAllocation(String),

    /// Failed to transfer data to/from GPU
    #[error("GPU data transfer failed: {0}")]
    GpuTransferFailed(String),

    // ========== Storage Errors ==========

    /// Failed to open storage
    #[error("Failed to open storage at {path}: {cause}")]
    StorageOpen {
        path: String,
        cause: String,
    },

    /// General storage operation error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Requested column family does not exist
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),

    /// Data corruption detected
    #[error("Corrupted data in storage: {0}")]
    CorruptedData(String),

    /// Vector ID mismatch between index and storage
    #[error("Vector ID mismatch: index has {index_id}, storage has {storage_id}")]
    VectorIdMismatch {
        index_id: i64,
        storage_id: i64,
    },

    // ========== Configuration Errors ==========

    /// Invalid configuration parameter
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    // ========== Graph Errors ==========

    /// Node not found in graph
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Edge not found in graph
    #[error("Edge not found: source={source}, target={target}")]
    EdgeNotFound {
        source: String,
        target: String,
    },

    // ========== Hyperbolic Geometry Errors ==========

    /// Invalid hyperbolic point (norm >= 1)
    #[error("Invalid hyperbolic point: norm {norm} >= 1.0 (must be strictly inside ball)")]
    InvalidHyperbolicPoint {
        norm: f32,
    },

    // ========== Serialization Errors ==========

    /// JSON serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(String),
}

// Ensure GraphError is Send + Sync for async compatibility
static_assertions::assert_impl_all!(GraphError: Send, Sync);
```

### Constraints
- All variants must have descriptive #[error()] messages
- GraphError must be Send + Sync
- InsufficientTrainingData must include provided/required counts
- Error messages should be actionable (tell user what to do)

### Acceptance Criteria
- [ ] GraphError enum with 17+ variants
- [ ] All variants have descriptive #[error()] messages
- [ ] Error is Send + Sync
- [ ] InsufficientTrainingData includes provided/required counts
- [ ] Compiles with `cargo build`
- [ ] Tests pass with `cargo test`
- [ ] No clippy warnings

## Implementation Approach

### Pseudocode/Algorithm
1. Add thiserror to dependencies
2. Add static_assertions for compile-time checks
3. Define enum with all variants
4. Use structured variants for errors with multiple fields
5. Use String variants for errors with dynamic messages

### Edge Cases
- Long error messages: Keep concise but informative
- Sensitive data in errors: Don't include passwords/tokens
- Chained errors: Use source attribute where needed (M04-T08a)

## Verification

### Test Commands
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph error
cargo clippy -p context-graph-graph -- -D warnings
```

### Manual Verification
- [ ] All variants can be constructed
- [ ] Display trait formats messages correctly
- [ ] Error trait is implemented
- [ ] Send + Sync bounds are satisfied
- [ ] InsufficientTrainingData displays both counts
