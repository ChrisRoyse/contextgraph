---
id: "M04-T00"
title: "Create context-graph-graph Crate Structure"
description: |
  CRITICAL BLOCKER: The `context-graph-graph` crate DOES NOT EXIST.
  This task creates the crate with directory structure, Cargo.toml, and module stubs.
  ALL other Module 04 tasks are blocked until this completes.
layer: "foundation"
status: "pending"
priority: "critical"
estimated_hours: 3
sequence: 1
depends_on: []
spec_refs:
  - "All Module 04 specs require this crate"
  - "TECH-GRAPH-004 Section 1"
files_to_create:
  - path: "crates/context-graph-graph/Cargo.toml"
    description: "Crate manifest with dependencies"
  - path: "crates/context-graph-graph/src/lib.rs"
    description: "Root module with module declarations"
  - path: "crates/context-graph-graph/src/config.rs"
    description: "Configuration types (IndexConfig, HyperbolicConfig, ConeConfig)"
  - path: "crates/context-graph-graph/src/error.rs"
    description: "GraphError enum with all error variants"
  - path: "crates/context-graph-graph/src/hyperbolic/mod.rs"
    description: "Hyperbolic geometry module"
  - path: "crates/context-graph-graph/src/entailment/mod.rs"
    description: "Entailment cones module"
  - path: "crates/context-graph-graph/src/index/mod.rs"
    description: "FAISS GPU index module"
  - path: "crates/context-graph-graph/src/storage/mod.rs"
    description: "RocksDB graph storage module"
  - path: "crates/context-graph-graph/src/traversal/mod.rs"
    description: "Graph traversal algorithms (BFS/DFS/A*)"
  - path: "crates/context-graph-graph/src/marblestone/mod.rs"
    description: "Marblestone NT integration module"
  - path: "crates/context-graph-graph/src/query/mod.rs"
    description: "Query operations module"
  - path: "crates/context-graph-graph/tests/.gitkeep"
    description: "Tests directory placeholder"
  - path: "crates/context-graph-graph/benches/.gitkeep"
    description: "Benchmarks directory placeholder"
files_to_modify:
  - path: "Cargo.toml"
    description: "Add context-graph-graph to workspace members"
test_file: "N/A - crate bootstrap"
---

## Current State Analysis (Audited 2026-01-03)

### CONFIRMED: Crate Does Not Exist
```bash
$ ls -la /home/cabdru/contextgraph/crates/
# Output shows: context-graph-core, context-graph-cuda, context-graph-embeddings,
#               context-graph-mcp, context-graph-storage
# NO context-graph-graph directory exists
```

### Existing Crates in Workspace
| Crate | Path | Relevance |
|-------|------|-----------|
| `context-graph-core` | `crates/context-graph-core/` | **Dependency**: Provides `EdgeType`, `Domain`, `NeurotransmitterWeights`, `MemoryNode` |
| `context-graph-cuda` | `crates/context-graph-cuda/` | **Dependency**: Provides GPU operations trait, stub implementations |
| `context-graph-embeddings` | `crates/context-graph-embeddings/` | **Dependency**: Provides `FusedEmbedding` (1536D), GPU tensor ops |
| `context-graph-storage` | `crates/context-graph-storage/` | **Reference**: Uses RocksDB 0.22, has column family patterns |
| `context-graph-mcp` | `crates/context-graph-mcp/` | Not directly relevant |

### Types Available from `context-graph-core`
```rust
// Re-exported at crate root (context-graph-core/src/lib.rs)
pub use marblestone::{Domain, EdgeType, NeurotransmitterWeights};

// From types module
pub type NodeId = Uuid;
pub type EmbeddingVector = Vec<f32>;
pub const DEFAULT_EMBEDDING_DIM: usize = 1536;
```

**CRITICAL**: `EdgeType` exists but is MISSING `Contradicts` variant. EdgeType currently has:
- `Semantic`, `Temporal`, `Causal`, `Hierarchical`

**MISSING**: `Contradicts` variant required by M04-T21 (Contradiction Detection)

### `NeurotransmitterWeights` Already Exists
Located at: `crates/context-graph-core/src/marblestone/neurotransmitter_weights.rs`

Already implements:
- `new(excitatory, inhibitory, modulatory)`
- `for_domain(domain: Domain)`
- `compute_effective_weight(base_weight)` - uses formula similar to but NOT IDENTICAL to constitution
- `validate()` - checks [0,1] range and NaN/Infinity

**FORMULA DISCREPANCY**:
- Constitution: `w_eff = base × (1 + excitatory - inhibitory + 0.5×modulatory)`
- Existing code: `w_eff = ((base * excitatory - base * inhibitory) * (1 + (modulatory - 0.5) * 0.4)).clamp(0,1)`

This is intentional per existing implementation tests. The graph crate should use the existing implementation.

---

## Scope

### In Scope
1. Create directory structure at `crates/context-graph-graph/`
2. Create `Cargo.toml` with correct dependencies
3. Create `src/lib.rs` with module declarations and re-exports
4. Create stub modules for all submodules (empty with TODO comments)
5. Add crate to workspace in root `Cargo.toml`
6. Ensure `cargo build -p context-graph-graph` succeeds
7. Ensure `cargo test -p context-graph-graph` runs (0 tests OK)
8. Ensure `cargo clippy -p context-graph-graph -- -D warnings` passes

### Out of Scope
- Implementing actual functionality (subsequent M04-T01+ tasks)
- Creating test implementations (each task creates own tests)
- Modifying `EdgeType` to add `Contradicts` (see M04-T26)

---

## Definition of Done

### Required Files

#### 1. `Cargo.toml`
```toml
[package]
name = "context-graph-graph"
version = "0.1.0"
edition = "2021"
description = "Knowledge graph with FAISS GPU vector search and hyperbolic geometry"
license = "MIT"

[dependencies]
# Workspace crates
context-graph-core = { path = "../context-graph-core" }
context-graph-cuda = { path = "../context-graph-cuda" }
context-graph-embeddings = { path = "../context-graph-embeddings" }

# Storage
rocksdb = "0.22"

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
bincode = "1.3"

# Error handling
thiserror = "1.0"

# Logging
tracing = "0.1"

# IDs and time
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# Async (for future use)
tokio = { version = "1.35", features = ["sync"] }

[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
tempfile = "3.10"

[[bench]]
name = "benchmark_suite"
harness = false
```

#### 2. `src/lib.rs`
```rust
//! Knowledge Graph with FAISS GPU Vector Search and Hyperbolic Geometry
//!
//! This crate provides the Knowledge Graph layer for the Context Graph system,
//! combining FAISS GPU-accelerated vector similarity search with hyperbolic
//! geometry for hierarchical reasoning.
//!
//! # Architecture
//!
//! - **config**: Index, hyperbolic, and cone configuration types
//! - **error**: Comprehensive error handling with GraphError
//! - **hyperbolic**: Poincare ball model with Mobius operations
//! - **entailment**: Entailment cones for O(1) IS-A queries
//! - **index**: FAISS GPU IVF-PQ index wrapper
//! - **storage**: RocksDB backend for graph persistence
//! - **traversal**: BFS, DFS, and A* graph traversal
//! - **marblestone**: Marblestone NT integration
//! - **query**: High-level query operations
//!
//! # Constitution Reference
//!
//! - TECH-GRAPH-004: Technical specification
//! - edge_model.nt_weights: Neurotransmitter weighting
//! - perf.latency.faiss_1M_k100: <2ms target
//!
//! # Example
//!
//! ```ignore
//! use context_graph_graph::config::IndexConfig;
//! use context_graph_graph::error::GraphResult;
//!
//! fn example() -> GraphResult<()> {
//!     let config = IndexConfig::default();
//!     assert_eq!(config.dimension, 1536);
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod entailment;
pub mod error;
pub mod hyperbolic;
pub mod index;
pub mod marblestone;
pub mod query;
pub mod storage;
pub mod traversal;

// Re-exports for convenience
pub use config::{ConeConfig, HyperbolicConfig, IndexConfig};
pub use error::{GraphError, GraphResult};

// Re-export core types for convenience
pub use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};
pub use context_graph_core::types::{EmbeddingVector, NodeId, DEFAULT_EMBEDDING_DIM};
```

#### 3. `src/config.rs`
```rust
//! Configuration types for Knowledge Graph components.
//!
//! TODO: Implement in M04-T01, M04-T02, M04-T03

/// FAISS IVF-PQ index configuration.
/// TODO: M04-T01
#[derive(Debug, Clone)]
pub struct IndexConfig {
    /// Embedding dimension (default: 1536)
    pub dimension: usize,
    /// Number of inverted lists for IVF (default: 16384)
    pub nlist: usize,
    /// Number of lists to probe during search (default: 128)
    pub nprobe: usize,
    /// Number of PQ subquantizers (default: 64)
    pub pq_segments: usize,
    /// Bits per PQ code (default: 8)
    pub pq_bits: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            dimension: 1536,
            nlist: 16384,
            nprobe: 128,
            pq_segments: 64,
            pq_bits: 8,
        }
    }
}

/// Hyperbolic (Poincare ball) configuration.
/// TODO: M04-T02
#[derive(Debug, Clone)]
pub struct HyperbolicConfig {
    /// Dimension of hyperbolic space (default: 64)
    pub dimension: usize,
    /// Curvature parameter (must be negative, default: -1.0)
    pub curvature: f32,
    /// Maximum norm for points (default: 0.999)
    pub max_norm: f32,
}

impl Default for HyperbolicConfig {
    fn default() -> Self {
        Self {
            dimension: 64,
            curvature: -1.0,
            max_norm: 0.999,
        }
    }
}

/// Entailment cone configuration.
/// TODO: M04-T03
#[derive(Debug, Clone)]
pub struct ConeConfig {
    /// Base aperture angle in radians (default: PI/4)
    pub base_aperture: f32,
    /// Aperture decay factor per depth level (default: 0.9)
    pub aperture_decay: f32,
    /// Minimum aperture angle (default: 0.1)
    pub min_aperture: f32,
}

impl Default for ConeConfig {
    fn default() -> Self {
        Self {
            base_aperture: std::f32::consts::FRAC_PI_4,
            aperture_decay: 0.9,
            min_aperture: 0.1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_config_default() {
        let config = IndexConfig::default();
        assert_eq!(config.dimension, 1536);
        assert_eq!(config.nlist, 16384);
        assert_eq!(config.nprobe, 128);
        assert_eq!(config.pq_segments, 64);
        assert_eq!(config.pq_bits, 8);
    }

    #[test]
    fn test_hyperbolic_config_default() {
        let config = HyperbolicConfig::default();
        assert_eq!(config.dimension, 64);
        assert_eq!(config.curvature, -1.0);
        assert!(config.curvature < 0.0, "Curvature must be negative");
        assert!(config.max_norm < 1.0, "Max norm must be < 1.0");
    }

    #[test]
    fn test_cone_config_default() {
        let config = ConeConfig::default();
        assert!(config.base_aperture > 0.0);
        assert!(config.aperture_decay > 0.0 && config.aperture_decay <= 1.0);
        assert!(config.min_aperture > 0.0);
    }
}
```

#### 4. `src/error.rs`
```rust
//! Error types for Knowledge Graph operations.
//!
//! TODO: Expand variants in M04-T08

use thiserror::Error;

/// Result type alias for graph operations.
pub type GraphResult<T> = Result<T, GraphError>;

/// Comprehensive error type for all graph operations.
///
/// TODO: M04-T08 will add all 17+ error variants
#[derive(Error, Debug)]
pub enum GraphError {
    /// FAISS index creation failed.
    #[error("FAISS index creation failed: {0}")]
    FaissIndexCreation(String),

    /// FAISS training failed.
    #[error("FAISS training failed: {0}")]
    FaissTrainingFailed(String),

    /// FAISS search failed.
    #[error("FAISS search failed: {0}")]
    FaissSearchFailed(String),

    /// Index not trained.
    #[error("Index not trained - must train before search/add")]
    IndexNotTrained,

    /// Insufficient training data.
    #[error("Insufficient training data: need {required}, got {provided}")]
    InsufficientTrainingData { required: usize, provided: usize },

    /// GPU resource allocation failed.
    #[error("GPU resource allocation failed: {0}")]
    GpuResourceAllocation(String),

    /// GPU transfer failed.
    #[error("GPU transfer failed: {0}")]
    GpuTransferFailed(String),

    /// RocksDB storage error.
    #[error("Storage error: {0}")]
    Storage(String),

    /// Column family not found.
    #[error("Column family not found: {0}")]
    ColumnFamilyNotFound(String),

    /// Data corruption detected.
    #[error("Corrupted data: {0}")]
    CorruptedData(String),

    /// Invalid configuration.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// Node not found.
    #[error("Node not found: {0}")]
    NodeNotFound(String),

    /// Edge not found.
    #[error("Edge not found: {0}")]
    EdgeNotFound(String),

    /// Invalid hyperbolic point (norm >= 1.0).
    #[error("Invalid hyperbolic point: norm {norm} >= 1.0")]
    InvalidHyperbolicPoint { norm: f32 },

    /// Vector ID mismatch.
    #[error("Vector ID mismatch: {0}")]
    VectorIdMismatch(String),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// Error conversions will be added in M04-T08a
// impl From<rocksdb::Error> for GraphError { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = GraphError::IndexNotTrained;
        assert!(err.to_string().contains("not trained"));
    }

    #[test]
    fn test_insufficient_training_data() {
        let err = GraphError::InsufficientTrainingData {
            required: 4194304,
            provided: 1000,
        };
        assert!(err.to_string().contains("4194304"));
        assert!(err.to_string().contains("1000"));
    }

    #[test]
    fn test_invalid_hyperbolic_point() {
        let err = GraphError::InvalidHyperbolicPoint { norm: 1.5 };
        assert!(err.to_string().contains("1.5"));
    }
}
```

#### 5. Stub Modules

Each stub module follows this pattern:

**`src/hyperbolic/mod.rs`**
```rust
//! Hyperbolic geometry module using Poincare ball model.
//!
//! TODO: Implement in M04-T04, M04-T05
//!
//! # Components
//! - PoincarePoint: 64D point in hyperbolic space
//! - PoincareBall: Mobius operations (add, distance, exp_map, log_map)

// TODO: M04-T04 - Define PoincarePoint
// TODO: M04-T05 - Implement PoincareBall Mobius operations
```

**`src/entailment/mod.rs`**
```rust
//! Entailment cones for O(1) IS-A hierarchy queries.
//!
//! TODO: Implement in M04-T06, M04-T07
//!
//! # Components
//! - EntailmentCone: Cone with apex, aperture, axis
//! - Containment: O(1) containment check algorithm

// TODO: M04-T06 - Define EntailmentCone struct
// TODO: M04-T07 - Implement containment logic
```

**`src/index/mod.rs`**
```rust
//! FAISS GPU index wrapper for vector similarity search.
//!
//! TODO: Implement in M04-T09, M04-T10, M04-T11
//!
//! # Components
//! - FaissFFI: C bindings to FAISS library
//! - FaissGpuIndex: GPU index wrapper
//! - SearchResult: Query result struct

// TODO: M04-T09 - Define FAISS FFI bindings
// TODO: M04-T10 - Implement FaissGpuIndex
// TODO: M04-T11 - Implement SearchResult
```

**`src/storage/mod.rs`**
```rust
//! RocksDB storage backend for graph data.
//!
//! TODO: Implement in M04-T12, M04-T13
//!
//! # Components
//! - Column families for nodes, edges, hyperbolic, cones
//! - GraphStorage: Main storage interface

// TODO: M04-T12 - Define column families
// TODO: M04-T13 - Implement GraphStorage
```

**`src/traversal/mod.rs`**
```rust
//! Graph traversal algorithms.
//!
//! TODO: Implement in M04-T16, M04-T17, M04-T17a
//!
//! # Components
//! - BFS: Breadth-first search with edge type filtering
//! - DFS: Depth-first search (iterative, not recursive)
//! - AStar: A* with hyperbolic heuristic

// TODO: M04-T16 - Implement BFS traversal
// TODO: M04-T17 - Implement DFS traversal
// TODO: M04-T17a - Implement A* traversal
```

**`src/marblestone/mod.rs`**
```rust
//! Marblestone neurotransmitter integration.
//!
//! Re-exports types from context-graph-core and adds graph-specific operations.
//!
//! TODO: Implement in M04-T19, M04-T22
//!
//! # Components
//! - Domain-aware search
//! - get_modulated_weight function

// Re-export from core for convenience
pub use context_graph_core::marblestone::{Domain, EdgeType, NeurotransmitterWeights};

// TODO: M04-T19 - Implement domain-aware search
// TODO: M04-T22 - Implement get_modulated_weight
```

**`src/query/mod.rs`**
```rust
//! High-level query operations.
//!
//! TODO: Implement in M04-T18, M04-T20, M04-T21
//!
//! # Components
//! - Semantic search
//! - Entailment queries
//! - Contradiction detection

// TODO: M04-T18 - Implement semantic search
// TODO: M04-T20 - Implement entailment query
// TODO: M04-T21 - Implement contradiction detection
```

---

## Workspace Update

Update root `Cargo.toml`:
```toml
[workspace]
resolver = "2"
members = [
    "crates/context-graph-mcp",
    "crates/context-graph-core",
    "crates/context-graph-cuda",
    "crates/context-graph-embeddings",
    "crates/context-graph-storage",
    "crates/context-graph-graph",  # ADD THIS LINE
]
```

---

## Implementation Steps

### Step 1: Create Directory Structure
```bash
mkdir -p crates/context-graph-graph/src/{hyperbolic,entailment,index,storage,traversal,marblestone,query}
mkdir -p crates/context-graph-graph/tests
mkdir -p crates/context-graph-graph/benches
touch crates/context-graph-graph/tests/.gitkeep
touch crates/context-graph-graph/benches/.gitkeep
```

### Step 2: Create Cargo.toml
Write the Cargo.toml as specified above.

### Step 3: Create src/lib.rs
Write the lib.rs as specified above.

### Step 4: Create Module Files
Create all module files with stub implementations as specified above.

### Step 5: Update Workspace
Add `"crates/context-graph-graph"` to workspace members in root Cargo.toml.

### Step 6: Verify Build
```bash
cargo build -p context-graph-graph
cargo test -p context-graph-graph
cargo clippy -p context-graph-graph -- -D warnings
```

---

## Constraints

1. **Directory**: MUST be at `crates/context-graph-graph/`
2. **Edition**: MUST use edition = "2021"
3. **Dependencies**: MUST NOT add dependencies not in workspace (use existing versions)
4. **No Circular Deps**: context-graph-graph depends on core/cuda/embeddings, NOT vice versa
5. **Compile**: All stubs MUST compile (can have TODO comments but no missing types)

---

## Edge Cases

| Scenario | Expected Behavior |
|----------|-------------------|
| Partial directory exists | Remove and recreate clean |
| Missing parent crates | Build will fail - parent crates must exist first |
| Cargo.lock conflicts | Run `cargo update` after adding to workspace |

---

## Verification

### Test Commands (Source of Truth)
```bash
# Build the crate
cargo build -p context-graph-graph 2>&1

# Run tests (should pass with 0 tests or stub tests)
cargo test -p context-graph-graph 2>&1

# Check for warnings
cargo clippy -p context-graph-graph -- -D warnings 2>&1

# Verify dependency tree
cargo tree -p context-graph-graph 2>&1
```

### Manual Verification Checklist
- [ ] `ls crates/context-graph-graph/` shows expected structure
- [ ] `ls crates/context-graph-graph/src/` shows all module files
- [ ] `grep context-graph-graph Cargo.toml` shows workspace member
- [ ] `cargo build -p context-graph-graph` exits with code 0
- [ ] `cargo test -p context-graph-graph` exits with code 0
- [ ] `cargo clippy -p context-graph-graph -- -D warnings` exits with code 0

---

## Full State Verification Protocol

After completing implementation, you MUST perform these verification steps:

### 1. Source of Truth Identification
The source of truth is the filesystem and Cargo build system:
- Directory structure at `crates/context-graph-graph/`
- Workspace membership in `Cargo.toml`
- Successful compilation with `cargo build`

### 2. Execute & Inspect
```bash
# Verify directory exists
ls -la crates/context-graph-graph/

# Verify all expected files
find crates/context-graph-graph -type f -name "*.rs" | sort

# Verify Cargo.toml
cat crates/context-graph-graph/Cargo.toml

# Verify workspace membership
grep -n "context-graph-graph" Cargo.toml

# Build verification
cargo build -p context-graph-graph 2>&1 | tail -20

# Test verification
cargo test -p context-graph-graph 2>&1 | tail -20

# Clippy verification
cargo clippy -p context-graph-graph -- -D warnings 2>&1 | tail -20

# Dependency tree verification
cargo tree -p context-graph-graph --depth 1
```

### 3. Boundary & Edge Case Audit

**Edge Case 1: Empty module compilation**
```bash
# Before: No crate exists
ls crates/context-graph-graph 2>&1 || echo "Expected: directory not found"

# After: Crate compiles with empty modules
cargo build -p context-graph-graph
echo "Exit code: $?"  # Must be 0
```

**Edge Case 2: Dependency resolution**
```bash
# Verify dependencies resolve correctly
cargo tree -p context-graph-graph | grep -E "(context-graph-core|context-graph-cuda|context-graph-embeddings)"
# Should show all three as direct dependencies
```

**Edge Case 3: Re-exports work**
```bash
# Create a test file to verify re-exports compile
cat > /tmp/test_reexports.rs << 'EOF'
use context_graph_graph::{Domain, EdgeType, NeurotransmitterWeights};
use context_graph_graph::{IndexConfig, HyperbolicConfig, ConeConfig};
use context_graph_graph::{GraphError, GraphResult};
fn main() {
    let _ = IndexConfig::default();
    let _ = Domain::Code;
    println!("Re-exports work!");
}
EOF
# This file should compile (but we just verify the crate builds)
```

### 4. Evidence of Success Log

After all steps, provide output showing:
```
=== M04-T00 VERIFICATION LOG ===
Timestamp: [ISO timestamp]

Directory Structure:
[output of find crates/context-graph-graph -type f]

Build Output:
[output of cargo build -p context-graph-graph]

Test Output:
[output of cargo test -p context-graph-graph]

Clippy Output:
[output of cargo clippy -p context-graph-graph -- -D warnings]

Dependency Tree:
[output of cargo tree -p context-graph-graph --depth 1]

RESULT: PASS/FAIL
```

---

## Final Verification: Sherlock-Holmes Agent

**MANDATORY**: After completing all implementation and verification steps, you MUST spawn a `sherlock-holmes` subagent to perform forensic verification of the task completion.

The Sherlock agent will:
1. Verify all files exist at expected paths
2. Verify Cargo.toml is correct
3. Verify workspace membership
4. Run `cargo build`, `cargo test`, `cargo clippy`
5. Verify no clippy warnings
6. Confirm all acceptance criteria are met
7. Report any discrepancies for immediate resolution

If Sherlock identifies any issues, fix them BEFORE marking this task complete.

---

## Acceptance Criteria

- [ ] Directory `crates/context-graph-graph/` exists
- [ ] All 11 files in `files_to_create` exist
- [ ] `Cargo.toml` in root includes `"crates/context-graph-graph"` in members
- [ ] `cargo build -p context-graph-graph` succeeds with exit code 0
- [ ] `cargo test -p context-graph-graph` runs with exit code 0
- [ ] `cargo clippy -p context-graph-graph -- -D warnings` passes with exit code 0
- [ ] `cargo tree -p context-graph-graph` shows expected dependencies
- [ ] All stub modules compile (no missing type errors)
- [ ] Re-exports from `context-graph-core` work
- [ ] Sherlock-Holmes verification passed

---

## Notes for AI Agent

1. **NO BACKWARDS COMPATIBILITY**: If something fails, it should error loudly. No workarounds.
2. **NO MOCK DATA**: When tests are added later, they must use real implementations.
3. **FAIL FAST**: Every error path must have clear error messages.
4. **ASSUME NOTHING**: Verify every file exists before modifying.
5. **This task BLOCKS 32+ other tasks** - get it right the first time.

---

*Generated: 2026-01-03*
*Audited Against: Codebase state at commit a3476e6*
*Module: 04 - Knowledge Graph*
*Task: M04-T00*
