# Module 4: Knowledge Graph - Atomic Tasks

```yaml
metadata:
  module_id: "module-04"
  module_name: "Knowledge Graph"
  version: "2.0.0"
  phase: 2
  total_tasks: 33
  approach: "inside-out-bottom-up"
  created: "2025-12-31"
  updated: "2026-01-02"
  dependencies:
    - module-02-core-infrastructure
    - module-03-embedding-pipeline
  estimated_duration: "5 weeks"
  spec_refs:
    - SPEC-GRAPH-004 (Functional)
    - TECH-GRAPH-004 (Technical)
```

---

## Task Overview

This module implements the Knowledge Graph layer combining FAISS GPU-accelerated vector similarity search with hyperbolic geometry for hierarchical reasoning. Tasks are organized in inside-out, bottom-up order:

1. **Foundation Layer** (Tasks 0-8a, 12 total): Crate structure, core types - HyperbolicPoint, EntailmentCone, IndexConfig
2. **Logic Layer** (Tasks 9-17a, 13 total): FAISS FFI, RocksDB graph storage, Poincare operations
3. **Surface Layer** (Tasks 18-29, 8 total): Query operations, Marblestone integration, traversal, CUDA kernels

### Critical Blockers Identified

| Issue | Severity | Resolution Task |
|-------|----------|-----------------|
| `context-graph-graph` crate DOES NOT EXIST | CRITICAL | M04-T00 |
| No FAISS FFI implementation exists | CRITICAL | M04-T09 |
| No CUDA kernels exist | HIGH | M04-T23, M04-T24 |
| Vector1536 type not re-exported | HIGH | M04-T01a |
| EdgeType::CONTRADICTS missing | HIGH | M04-T26 |
| Formula conflicts in containment | MEDIUM | M04-T27 |

---

## Canonical Formulas (IMPORTANT)

**All tasks MUST use these canonical formulas:**

```rust
// Neurotransmitter modulation (M04-T14, M04-T14a, M04-T22)
net_activation = excitatory - inhibitory + (modulatory * 0.5)

// Edge weight modulation (M04-T15, M04-T19, M04-T22)
w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
// Result clamped to [0.0, 1.0]

// Entailment cone membership score (M04-T07, M04-T27)
// For points within cone: score = 1.0
// For points outside cone: score = exp(-2.0 * (angle - aperture))
```

---

## Foundation Layer: Core Types

```yaml
tasks:
  # ============================================================
  # FOUNDATION: Crate Bootstrap (CRITICAL - MUST BE FIRST)
  # ============================================================

  - id: "M04-T00"
    title: "Create context-graph-graph Crate Structure"
    description: |
      CRITICAL: This crate does not exist and MUST be created before any other task.
      Create the complete crate directory structure with Cargo.toml and module layout.
      Dependencies: context-graph-core, context-graph-cuda, rocksdb, faiss (via FFI).
    layer: "foundation"
    priority: "critical"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/"
    dependencies: []
    acceptance_criteria:
      - "Directory crates/context-graph-graph/ exists"
      - "Cargo.toml with proper dependencies compiles"
      - "src/lib.rs with module declarations compiles"
      - "cargo build succeeds with no errors"
      - "cargo test runs (even with 0 tests)"
      - "No clippy warnings"
    files_to_create:
      - "crates/context-graph-graph/Cargo.toml"
      - "crates/context-graph-graph/src/lib.rs"
      - "crates/context-graph-graph/src/config.rs"
      - "crates/context-graph-graph/src/error.rs"
      - "crates/context-graph-graph/src/hyperbolic/mod.rs"
      - "crates/context-graph-graph/src/entailment/mod.rs"
      - "crates/context-graph-graph/src/index/mod.rs"
      - "crates/context-graph-graph/src/storage/mod.rs"
      - "crates/context-graph-graph/src/traversal/mod.rs"
      - "crates/context-graph-graph/src/marblestone/mod.rs"
    test_file: "N/A - crate bootstrap"
    spec_refs:
      - "All Module 04 specs require this crate"

  # ============================================================
  # FOUNDATION: Configuration Types
  # ============================================================

  - id: "M04-T01"
    title: "Define IndexConfig for FAISS IVF-PQ"
    description: |
      Implement IndexConfig struct for FAISS GPU index configuration.
      Fields: dimension (1536), nlist (16384), nprobe (128), pq_segments (64),
      pq_bits (8), gpu_id (0), use_float16 (true), min_train_vectors (4_194_304).
      Include factory_string() method returning "IVF{nlist},PQ{pq_segments}x{pq_bits}".
    layer: "foundation"
    priority: "critical"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/config.rs"
    dependencies:
      - "M04-T00"
    acceptance_criteria:
      - "IndexConfig struct compiles with all 8 fields"
      - "Default returns nlist=16384, nprobe=128, pq_segments=64, pq_bits=8"
      - "factory_string() returns 'IVF16384,PQ64x8' for defaults"
      - "min_train_vectors = 256 * nlist = 4,194,304"
      - "Serde Serialize/Deserialize implemented"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/config_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 2"
      - "REQ-KG-001 through REQ-KG-005"

  - id: "M04-T01a"
    title: "Re-export Vector1536 from Embeddings Crate"
    description: |
      Re-export the Vector1536 type from context-graph-embeddings for use in the graph crate.
      This ensures type consistency between embedding vectors and graph index vectors.
      Add to crates/context-graph-graph/src/lib.rs.
    layer: "foundation"
    priority: "high"
    estimated_hours: 1
    file_path: "crates/context-graph-graph/src/lib.rs"
    dependencies:
      - "M04-T01"
    acceptance_criteria:
      - "pub use context_graph_embeddings::Vector1536; compiles"
      - "Vector1536 is accessible from crate root"
      - "Cargo.toml has context-graph-embeddings dependency"
      - "Compiles with `cargo build`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/config_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 2"

  - id: "M04-T02"
    title: "Define HyperbolicConfig for Poincare Ball"
    description: |
      Implement HyperbolicConfig struct for 64D Poincare ball model.
      Fields: dim (64), curvature (-1.0), eps (1e-7), max_norm (1.0 - 1e-5).
      NOTE: Curvature MUST be negative (validated in M04-T02a).
    layer: "foundation"
    priority: "critical"
    estimated_hours: 1
    file_path: "crates/context-graph-graph/src/config.rs"
    dependencies:
      - "M04-T00"
    acceptance_criteria:
      - "HyperbolicConfig struct with 4 fields"
      - "Default returns dim=64, curvature=-1.0"
      - "max_norm ensures points stay within ball boundary"
      - "eps prevents numerical instability"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/config_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 5"
      - "REQ-KG-050, REQ-KG-054"

  - id: "M04-T02a"
    title: "Implement Curvature Validation for HyperbolicConfig"
    description: |
      Add validate() method to HyperbolicConfig that ensures curvature < 0.
      Returns Result<(), GraphError::InvalidConfig> if curvature >= 0.
      Also validates: dim > 0, eps > 0, max_norm in (0, 1).
    layer: "foundation"
    priority: "high"
    estimated_hours: 0.5
    file_path: "crates/context-graph-graph/src/config.rs"
    dependencies:
      - "M04-T02"
    acceptance_criteria:
      - "validate() returns Ok(()) for valid config"
      - "validate() returns Err for curvature >= 0"
      - "validate() returns Err for dim == 0"
      - "validate() returns Err for eps <= 0"
      - "validate() returns Err for max_norm not in (0,1)"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/config_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 5"
      - "REQ-KG-054"

  - id: "M04-T03"
    title: "Define ConeConfig for Entailment Cones"
    description: |
      Implement ConeConfig struct for EntailmentCone parameters.
      Fields: min_aperture (0.1 rad), max_aperture (1.5 rad), base_aperture (1.0 rad),
      aperture_decay (0.85 per level), membership_threshold (0.7).
      Include compute_aperture(depth) method.
    layer: "foundation"
    priority: "high"
    estimated_hours: 1.5
    file_path: "crates/context-graph-graph/src/config.rs"
    dependencies:
      - "M04-T00"
    acceptance_criteria:
      - "ConeConfig struct with 5 fields"
      - "compute_aperture(0) returns base_aperture"
      - "compute_aperture(n) = base * decay^n, clamped to [min, max]"
      - "Default values match spec"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/config_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 6"
      - "REQ-KG-052"

  # ============================================================
  # FOUNDATION: Hyperbolic Geometry Types
  # ============================================================

  - id: "M04-T04"
    title: "Define PoincarePoint for 64D Hyperbolic Space"
    description: |
      Implement PoincarePoint struct with coords: [f32; 64].
      Constraint: ||coords|| < 1.0 (strict inequality for Poincare ball).
      Include methods: origin(), norm_squared(), norm(), project(&HyperbolicConfig).
      Use #[repr(C, align(64))] for SIMD optimization.
    layer: "foundation"
    priority: "critical"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/hyperbolic/poincare.rs"
    dependencies:
      - "M04-T02"
    acceptance_criteria:
      - "PoincarePoint struct with [f32; 64] array"
      - "origin() returns all zeros"
      - "norm() computes Euclidean norm"
      - "project() rescales if norm >= max_norm"
      - "Memory alignment 64 bytes for cache efficiency"
      - "Clone, Debug traits implemented"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/poincare_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 5.1"
      - "REQ-KG-050"

  - id: "M04-T05"
    title: "Implement PoincareBall Mobius Operations"
    description: |
      Implement PoincareBall struct with Mobius algebra operations.
      Methods: mobius_add(x, y), distance(x, y), exp_map(x, v), log_map(x, y).
      Distance formula: d(x,y) = (2/sqrt(|c|)) * arctanh(sqrt(|c| * ||x-y||^2 / ((1-|c|||x||^2)(1-|c|||y||^2))))
      Performance target: <10us per distance computation.
    layer: "foundation"
    priority: "critical"
    estimated_hours: 4
    file_path: "crates/context-graph-graph/src/hyperbolic/mobius.rs"
    dependencies:
      - "M04-T04"
    acceptance_criteria:
      - "mobius_add() implements Mobius addition formula correctly"
      - "distance() returns Poincare ball distance in <10us"
      - "exp_map() maps tangent vector to point on manifold"
      - "log_map() returns tangent vector from x to y"
      - "All operations handle boundary cases (norm near 1.0)"
      - "Unit tests verify mathematical properties (symmetry, identity)"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/mobius_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 5.2"
      - "REQ-KG-051"

  - id: "M04-T06"
    title: "Define EntailmentCone Struct"
    description: |
      Implement EntailmentCone struct for O(1) IS-A hierarchy queries.
      Fields: apex (PoincarePoint), aperture (f32), aperture_factor (f32), depth (u32).
      Include methods: new(), effective_aperture(), contains(), membership_score().
      Constraint: aperture in [0, pi/2], aperture_factor in [0.5, 2.0].
    layer: "foundation"
    priority: "critical"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/entailment/cones.rs"
    dependencies:
      - "M04-T03"
      - "M04-T05"
    acceptance_criteria:
      - "EntailmentCone struct with apex, aperture, aperture_factor, depth"
      - "new() computes aperture from depth using ConeConfig"
      - "effective_aperture() = aperture * aperture_factor"
      - "contains() returns bool in <50us"
      - "membership_score() returns soft [0,1] score"
      - "Serde serialization produces 268 bytes (256 coords + 4 aperture + 4 factor + 4 depth)"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/entailment_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 6"
      - "REQ-KG-052, REQ-KG-053"

  - id: "M04-T07"
    title: "Implement EntailmentCone Containment Logic"
    description: |
      Implement EntailmentCone containment check algorithm.
      Algorithm:
      1. Compute tangent = log_map(apex, point)
      2. Compute to_origin = log_map(apex, origin)
      3. angle = arccos(dot(tangent, to_origin) / (||tangent|| * ||to_origin||))
      4. Return angle <= effective_aperture()

      CANONICAL FORMULA for membership_score():
      - If contained: 1.0
      - If not contained: exp(-2.0 * (angle - aperture))

      Include update_aperture() for training.
    layer: "foundation"
    priority: "critical"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/entailment/cones.rs"
    dependencies:
      - "M04-T06"
    acceptance_criteria:
      - "contains() returns true for points within cone"
      - "contains() returns false for points outside cone"
      - "membership_score() uses canonical formula (see above)"
      - "update_aperture() adjusts aperture_factor based on training signal"
      - "Edge cases handled: apex at origin, point at apex, degenerate cones"
      - "Performance: <50us per containment check"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/entailment_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 6"
      - "REQ-KG-053"

  - id: "M04-T08"
    title: "Define GraphError Enum"
    description: |
      Implement comprehensive GraphError enum for knowledge graph operations.
      Variants: FaissIndexCreation, FaissTrainingFailed, FaissSearchFailed, FaissAddFailed,
      IndexNotTrained, InsufficientTrainingData, GpuResourceAllocation, GpuTransferFailed,
      StorageOpen, Storage, ColumnFamilyNotFound, CorruptedData, VectorIdMismatch, InvalidConfig,
      NodeNotFound, EdgeNotFound, InvalidHyperbolicPoint.
      Use thiserror for derivation.
    layer: "foundation"
    priority: "high"
    estimated_hours: 1.5
    file_path: "crates/context-graph-graph/src/error.rs"
    dependencies:
      - "M04-T00"
    acceptance_criteria:
      - "GraphError enum with 17+ variants"
      - "All variants have descriptive #[error()] messages"
      - "Error is Send + Sync"
      - "InsufficientTrainingData includes provided/required counts"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/error_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 9"

  - id: "M04-T08a"
    title: "Implement Error Conversions (From Traits)"
    description: |
      Add From trait implementations for GraphError to convert external errors.
      Required conversions:
      - From<rocksdb::Error> for GraphError
      - From<std::io::Error> for GraphError
      - From<serde_json::Error> for GraphError
      These enable the ? operator in functions returning Result<T, GraphError>.
    layer: "foundation"
    priority: "high"
    estimated_hours: 1
    file_path: "crates/context-graph-graph/src/error.rs"
    dependencies:
      - "M04-T08"
    acceptance_criteria:
      - "From<rocksdb::Error> implemented"
      - "From<std::io::Error> implemented"
      - "From<serde_json::Error> implemented"
      - "? operator works with these error types"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/error_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 9"

  # ============================================================
  # LOGIC LAYER: FAISS GPU Index
  # ============================================================

  - id: "M04-T09"
    title: "Define FAISS FFI Bindings"
    description: |
      Implement faiss_ffi module with C bindings to FAISS library.
      Bindings: faiss_index_factory, faiss_StandardGpuResources_new/free,
      faiss_index_cpu_to_gpu, faiss_Index_train, faiss_Index_is_trained,
      faiss_Index_add_with_ids, faiss_Index_search, faiss_IndexIVF_nprobe_set,
      faiss_Index_ntotal, faiss_write_index, faiss_read_index, faiss_Index_free.
      Include GpuResources RAII wrapper with Send + Sync.
    layer: "logic"
    priority: "critical"
    estimated_hours: 4
    file_path: "crates/context-graph-graph/src/index/faiss_ffi.rs"
    dependencies:
      - "M04-T08a"
    acceptance_criteria:
      - "All extern 'C' declarations compile"
      - "GpuResources wrapper handles allocation/deallocation"
      - "GpuResources is Send + Sync"
      - "MetricType enum with InnerProduct=0, L2=1"
      - "Link directive: #[link(name = 'faiss_c')]"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/faiss_ffi_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 3.1"

  - id: "M04-T10"
    title: "Implement FaissGpuIndex Wrapper"
    description: |
      Implement FaissGpuIndex struct wrapping FAISS GPU index.
      Methods: new(config), train(vectors), search(queries, k), add_with_ids(vectors, ids),
      ntotal(), save(path), load(path).
      Use NonNull for GPU pointer, Arc<GpuResources> for resource sharing.
      Performance: <5ms for k=10 search on 10M vectors.
    layer: "logic"
    priority: "critical"
    estimated_hours: 5
    file_path: "crates/context-graph-graph/src/index/gpu_index.rs"
    dependencies:
      - "M04-T01"
      - "M04-T09"
    acceptance_criteria:
      - "new() creates IVF-PQ index and transfers to GPU"
      - "train() requires min_train_vectors, sets nprobe after training"
      - "search() returns SearchResult with ids and distances"
      - "add_with_ids() adds vectors incrementally (no rebuild)"
      - "Drop impl frees GPU resources correctly"
      - "Send + Sync implemented (unsafe)"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/gpu_index_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 3.2"
      - "REQ-KG-001 through REQ-KG-008"

  - id: "M04-T11"
    title: "Implement SearchResult Struct"
    description: |
      Implement SearchResult struct for FAISS query results.
      Fields: ids (Vec<i64>), distances (Vec<f32>), k (usize), num_queries (usize).
      Include query_results(idx) iterator method for extracting per-query results.
      Handle -1 sentinel IDs (no match found).
    layer: "logic"
    priority: "high"
    estimated_hours: 1.5
    file_path: "crates/context-graph-graph/src/index/gpu_index.rs"
    dependencies:
      - "M04-T10"
    acceptance_criteria:
      - "SearchResult struct with 4 fields"
      - "query_results(idx) returns iterator of (id, distance) pairs"
      - "Filters out -1 sentinel values"
      - "Clone, Debug implemented"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/search_result_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 3.2"

  # ============================================================
  # LOGIC LAYER: RocksDB Graph Storage
  # ============================================================

  - id: "M04-T12"
    title: "Define Graph Storage Column Families"
    description: |
      Define RocksDB column families for knowledge graph storage.
      CFs: adjacency (edge lists), hyperbolic (64D coordinates), entailment_cones (cone data).
      Include get_column_family_descriptors() returning optimized CF options.
      Hyperbolic CF: 256 bytes per point (64 * 4), LZ4 compression.
      Cones CF: 268 bytes per cone, bloom filter enabled.
    layer: "logic"
    priority: "high"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/storage/mod.rs"
    dependencies:
      - "M04-T08a"
    acceptance_criteria:
      - "CF_ADJACENCY, CF_HYPERBOLIC, CF_CONES constants defined"
      - "get_column_family_descriptors() returns 3 CFs with options"
      - "Hyperbolic CF optimized for point lookups"
      - "Adjacency CF optimized for prefix scans"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/storage_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4"

  - id: "M04-T13"
    title: "Implement GraphStorage Backend"
    description: |
      Implement GraphStorage struct wrapping RocksDB for graph data.
      Methods: open(path, config), get_hyperbolic(node_id), put_hyperbolic(node_id, point),
      get_cone(node_id), put_cone(node_id, cone), get_adjacency(node_id), put_adjacency(node_id, edges).
      Use Arc<DB> for thread-safe sharing.
    layer: "logic"
    priority: "critical"
    estimated_hours: 4
    file_path: "crates/context-graph-graph/src/storage/rocksdb.rs"
    dependencies:
      - "M04-T04"
      - "M04-T06"
      - "M04-T12"
    acceptance_criteria:
      - "open() creates DB with all 3 CFs"
      - "get_hyperbolic() deserializes 256 bytes to PoincarePoint"
      - "put_hyperbolic() serializes point to 256 bytes"
      - "get_cone() deserializes 268 bytes to EntailmentCone"
      - "Proper error handling with GraphError variants"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/storage_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4.2"

  - id: "M04-T13a"
    title: "Implement Storage Schema Migrations"
    description: |
      Define migration system from old CF schema to new.
      Add schema version tracking in a metadata CF.
      Migrations needed:
      - v1: Initial schema with adjacency, hyperbolic, entailment_cones CFs
      - Future: Placeholder for v2 migrations
      Include migrate() method that checks version and applies migrations.
    layer: "logic"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/storage/migrations.rs"
    dependencies:
      - "M04-T13"
    acceptance_criteria:
      - "SCHEMA_VERSION constant defined (v1 = 1)"
      - "get_schema_version() reads from metadata CF"
      - "migrate() applies migrations incrementally"
      - "migrate() is idempotent (running twice is safe)"
      - "Migration v1 creates all required CFs"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/storage_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4.2"

  - id: "M04-T14"
    title: "Implement NeurotransmitterWeights for Edges (Marblestone)"
    description: |
      Implement NeurotransmitterWeights struct from Module 2 in graph context.
      Fields: excitatory (f32), inhibitory (f32), modulatory (f32), all in [0,1].
      Include for_domain(Domain) factory with domain-specific profiles.

      CANONICAL FORMULA for net_activation():
      net_activation = excitatory - inhibitory + (modulatory * 0.5)
    layer: "logic"
    priority: "high"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/storage/edges.rs"
    dependencies: []
    acceptance_criteria:
      - "NeurotransmitterWeights struct with 3 f32 fields"
      - "Default: excitatory=0.5, inhibitory=0.5, modulatory=0.0"
      - "for_domain(Code) = {0.7, 0.3, 0.2}"
      - "for_domain(Creative) = {0.8, 0.2, 0.5}"
      - "net_activation() uses CANONICAL formula"
      - "Serde serialization works"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/marblestone_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4.1"
      - "REQ-KG-065"

  - id: "M04-T14a"
    title: "Implement NT Weight Range Validation"
    description: |
      Add validate() method to NeurotransmitterWeights that ensures all weights in [0,1].
      Add net_activation() method implementing the canonical formula.
      Returns Result<(), GraphError::InvalidConfig> if any weight outside range.
    layer: "logic"
    priority: "high"
    estimated_hours: 1
    file_path: "crates/context-graph-graph/src/storage/edges.rs"
    dependencies:
      - "M04-T14"
    acceptance_criteria:
      - "validate() returns Ok(()) for weights in [0,1]"
      - "validate() returns Err for weight < 0"
      - "validate() returns Err for weight > 1"
      - "net_activation() = excitatory - inhibitory + (modulatory * 0.5)"
      - "net_activation() result is in [-1.5, 1.5] range"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/marblestone_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4.1"
      - "REQ-KG-065"

  - id: "M04-T15"
    title: "Implement GraphEdge with Marblestone Fields"
    description: |
      Implement GraphEdge struct with full Marblestone support.
      Fields: id, source, target, edge_type, weight, confidence, domain,
      neurotransmitter_weights, is_amortized_shortcut, steering_reward,
      traversal_count, created_at, last_traversed_at.
      Include get_modulated_weight(query_domain) method.

      CANONICAL FORMULA for get_modulated_weight():
      w_eff = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
      Result clamped to [0.0, 1.0]
    layer: "logic"
    priority: "critical"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/storage/edges.rs"
    dependencies:
      - "M04-T14a"
    acceptance_criteria:
      - "GraphEdge struct with all 13 fields"
      - "new() initializes with domain-appropriate NT weights"
      - "get_modulated_weight() uses CANONICAL formula"
      - "record_traversal() increments count and updates steering_reward with EMA"
      - "EdgeType enum: Semantic, Temporal, Causal, Hierarchical (CONTRADICTS added in M04-T26)"
      - "Domain enum: Code, Legal, Medical, Creative, Research, General"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/edge_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4.1"
      - "REQ-KG-040 through REQ-KG-044, REQ-KG-065"

  # ============================================================
  # LOGIC LAYER: Graph Traversal
  # ============================================================

  - id: "M04-T16"
    title: "Implement BFS Graph Traversal"
    description: |
      Implement bfs_traverse(storage, start, params) function.
      BfsParams: max_depth (6), max_nodes (10000), edge_types (Option<Vec>), domain_filter.
      Returns BfsResult with nodes, edges, depth_counts.
      Use VecDeque for frontier, HashSet for visited tracking.
      Performance: <100ms for depth=6 on 10M node graph.
    layer: "logic"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/traversal/bfs.rs"
    dependencies:
      - "M04-T13"
      - "M04-T15"
    acceptance_criteria:
      - "bfs_traverse() visits nodes level by level"
      - "Respects max_depth and max_nodes limits"
      - "edge_types filter restricts which edges to follow"
      - "domain_filter applies Marblestone domain matching"
      - "No infinite loops on cyclic graphs (visited set)"
      - "depth_counts tracks nodes found at each depth"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/traversal_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 7.1"
      - "REQ-KG-061"

  - id: "M04-T17"
    title: "Implement DFS Graph Traversal"
    description: |
      Implement dfs_traverse(storage, start, max_depth, max_nodes) function.
      Use iterative stack-based approach (not recursive).
      Returns Vec<NodeId> of visited nodes in DFS order.
      Handle cycles via visited set.
    layer: "logic"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/traversal/dfs.rs"
    dependencies:
      - "M04-T13"
    acceptance_criteria:
      - "dfs_traverse() visits nodes in depth-first order"
      - "Uses iterative stack, not recursion"
      - "Respects max_depth and max_nodes"
      - "No stack overflow on deep graphs"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/traversal_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 7.2"

  - id: "M04-T17a"
    title: "Implement A* Hyperbolic Traversal"
    description: |
      Implement astar_traverse(storage, start, goal, heuristic) function.
      Use hyperbolic distance as admissible heuristic for efficient path finding.
      The heuristic uses Poincare ball distance to goal as lower bound.
      Returns Option<Vec<NodeId>> with optimal path if found.
    layer: "logic"
    priority: "medium"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/traversal/astar.rs"
    dependencies:
      - "M04-T17"
    acceptance_criteria:
      - "astar_traverse() finds optimal path using A*"
      - "Uses hyperbolic distance heuristic (admissible)"
      - "Returns None if no path exists"
      - "Returns Some(path) with shortest path"
      - "Uses priority queue for frontier"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/traversal_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 7"
      - "REQ-KG-061"

  # ============================================================
  # SURFACE LAYER: Query Operations
  # ============================================================

  - id: "M04-T18"
    title: "Implement Semantic Search Operation"
    description: |
      Implement semantic_search(query, k, filters) on KnowledgeGraph.
      Uses FAISS GPU index for initial k-NN retrieval.
      Applies SearchFilters: min_importance, johari_quadrants, created_after, agent_id.
      Returns Vec<SearchResult> with node, similarity, distance.
      Performance: <10ms for k=100 on 10M vectors.
    layer: "surface"
    priority: "critical"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/lib.rs"
    dependencies:
      - "M04-T10"
      - "M04-T11"
    acceptance_criteria:
      - "semantic_search() calls FAISS search internally"
      - "Converts L2 distance to cosine similarity"
      - "Applies post-filters to results"
      - "Returns empty vec if index not trained"
      - "Performance meets <10ms target"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/search_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 8"
      - "REQ-KG-060"

  - id: "M04-T19"
    title: "Implement Domain-Aware Search (Marblestone)"
    description: |
      Implement domain_aware_search(query, domain, k) with neurotransmitter modulation.
      Algorithm:
      1. FAISS k-NN search fetching 3x candidates
      2. Apply NeurotransmitterWeights modulation per domain
      3. Re-rank by modulated score
      4. Return top-k results

      CANONICAL FORMULA for modulation:
      modulated_score = base_similarity * (1.0 + net_activation)

      Performance: <10ms for k=10 on 10M vectors.
    layer: "surface"
    priority: "critical"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/marblestone/domain_search.rs"
    dependencies:
      - "M04-T14a"
      - "M04-T15"
      - "M04-T18"
    acceptance_criteria:
      - "domain_aware_search() over-fetches candidates (3x)"
      - "Applies NT modulation using CANONICAL formula"
      - "Re-ranks results by modulated score"
      - "Truncates to requested k"
      - "DomainSearchResult includes base_distance and modulated_score"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/domain_search_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 8"
      - "REQ-KG-065"

  - id: "M04-T20"
    title: "Implement Entailment Query Operation"
    description: |
      Implement entailment_query(node_id, direction, max_depth) function.
      Uses EntailmentCone containment for O(1) IS-A hierarchy checks.
      Direction: Ancestors (concepts that entail this) or Descendants (concepts entailed by this).
      Returns Vec<KnowledgeNode> in hierarchy.
      Performance: <1ms per containment check.
    layer: "surface"
    priority: "critical"
    estimated_hours: 4
    file_path: "crates/context-graph-graph/src/lib.rs"
    dependencies:
      - "M04-T07"
      - "M04-T13"
    acceptance_criteria:
      - "entailment_query() retrieves node's cone from storage"
      - "Checks containment against candidate nodes"
      - "Ancestors: finds cones that contain this node"
      - "Descendants: finds nodes contained by this cone"
      - "Respects max_depth limit"
      - "Performance: <1ms per cone check"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/entailment_query_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 8"
      - "REQ-KG-062"

  - id: "M04-T21"
    title: "Implement Contradiction Detection"
    description: |
      Implement contradiction_detect(node_id, threshold) function.
      Algorithm:
      1. Semantic search for similar nodes (k=50)
      2. Check for CONTRADICTS edges (requires M04-T26)
      3. Compute contradiction confidence based on similarity + edge weight
      4. Return ContradictionResult with node, contradiction_type, confidence.
      ContradictionType: DirectOpposition, LogicalInconsistency, TemporalConflict, CausalConflict.
    layer: "surface"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/lib.rs"
    dependencies:
      - "M04-T18"
      - "M04-T16"
      - "M04-T26"
    acceptance_criteria:
      - "contradiction_detect() finds semantically similar nodes"
      - "Checks for explicit CONTRADICTS edge type"
      - "Computes confidence score in [0,1]"
      - "Filters by threshold"
      - "Classifies contradiction type"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/contradiction_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 8"
      - "REQ-KG-063"

  - id: "M04-T22"
    title: "Implement get_modulated_weight Function (Marblestone)"
    description: |
      Implement get_modulated_weight(edge, domain) standalone function.

      CANONICAL FORMULA:
      net_activation = excitatory - inhibitory + (modulatory * 0.5)
      effective_weight = base_weight * (1.0 + net_activation + domain_bonus) * steering_factor
      Result clamped to [0.0, 1.0]

      Pure function with no side effects.
      Used by traversal and search operations for domain-aware edge weighting.
    layer: "surface"
    priority: "high"
    estimated_hours: 1
    file_path: "crates/context-graph-graph/src/marblestone/mod.rs"
    dependencies:
      - "M04-T15"
    acceptance_criteria:
      - "get_modulated_weight() is pure function"
      - "Applies CANONICAL NT modulation formula"
      - "Result clamped to [0.0, 1.0]"
      - "Domain match adds bonus (0.1 for exact match)"
      - "Unit tests verify edge cases"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/marblestone_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 8"
      - "REQ-KG-065"

  # ============================================================
  # SURFACE LAYER: CUDA Kernels & Integration
  # ============================================================

  - id: "M04-T23"
    title: "Implement Poincare Distance CUDA Kernel"
    description: |
      Implement poincare_distance_batch CUDA kernel for GPU-accelerated hyperbolic distance.
      Input: queries[n_q][64], database[n_db][64], curvature c
      Output: distances[n_q][n_db]
      Use shared memory for query caching.
      Performance: <1ms for 1K x 1K distance matrix.
    layer: "surface"
    priority: "high"
    estimated_hours: 4
    file_path: "crates/context-graph-cuda/kernels/poincare_distance.cu"
    dependencies:
      - "M04-T05"
    acceptance_criteria:
      - "CUDA kernel compiles with nvcc"
      - "Shared memory used for query vectors"
      - "Matches CPU implementation within 1e-5 tolerance"
      - "Performance: <1ms for 1K x 1K"
      - "Handles boundary cases (points near norm=1)"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/cuda_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 10.1"

  - id: "M04-T24"
    title: "Implement Cone Membership CUDA Kernel"
    description: |
      Implement cone_check_batch CUDA kernel for batch entailment cone membership.
      Input: cones[n_cones][65] (64 apex coords + 1 aperture), points[n_pts][64]
      Output: scores[n_cones][n_pts]
      Performance: <2ms for 1K x 1K membership matrix.
    layer: "surface"
    priority: "high"
    estimated_hours: 4
    file_path: "crates/context-graph-cuda/kernels/cone_check.cu"
    dependencies:
      - "M04-T07"
    acceptance_criteria:
      - "CUDA kernel compiles with nvcc"
      - "Shared memory used for cone data"
      - "Matches CPU implementation within 1e-5 tolerance"
      - "Performance: <2ms for 1K x 1K"
      - "Returns soft membership score [0,1]"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/cuda_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 10.2"

  # ============================================================
  # SURFACE LAYER: New Tasks from Analysis
  # ============================================================

  - id: "M04-T26"
    title: "Add EdgeType::CONTRADICTS Variant"
    description: |
      Add CONTRADICTS variant to EdgeType enum.
      This is required for contradiction detection in M04-T21.
      EdgeType should now have: Semantic, Temporal, Causal, Hierarchical, Contradicts.
      Update any match statements to handle the new variant.
    layer: "surface"
    priority: "high"
    estimated_hours: 1
    file_path: "crates/context-graph-graph/src/storage/edges.rs"
    dependencies:
      - "M04-T15"
    acceptance_criteria:
      - "EdgeType::Contradicts variant exists"
      - "All match statements handle Contradicts"
      - "Serde serialization works for Contradicts"
      - "Documentation updated"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/edge_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 4.1"
      - "REQ-KG-063"

  - id: "M04-T27"
    title: "Fix Containment Formula Conflicts"
    description: |
      Ensure consistent containment formula across all implementations.
      Three conflicting formulas were found during analysis.

      CANONICAL FORMULA (use everywhere):
      - Compute angle between point direction and cone axis
      - If angle <= effective_aperture: contained, score = 1.0
      - If angle > effective_aperture: not contained, score = exp(-2.0 * (angle - aperture))

      Update M04-T07 implementation and all test cases to use this formula.
    layer: "surface"
    priority: "medium"
    estimated_hours: 2
    file_path: "crates/context-graph-graph/src/entailment/cones.rs"
    dependencies:
      - "M04-T07"
    acceptance_criteria:
      - "Single canonical formula used in contains()"
      - "Single canonical formula used in membership_score()"
      - "All tests use consistent expected values"
      - "Documentation updated with formula"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/entailment_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 6"
      - "REQ-KG-053"

  - id: "M04-T28"
    title: "Implement GPU Memory Manager"
    description: |
      Implement GpuMemoryManager for VRAM budget tracking and allocation.
      Target: 24GB RTX 5090 with budget:
      - FAISS index: 8GB
      - Hyperbolic coords: 2.5GB
      - Entailment cones: 2.7GB
      - Working memory: 10.8GB
      Methods: allocate(bytes), free(allocation), available(), used(), budget().
      Returns error if allocation exceeds budget.
    layer: "surface"
    priority: "high"
    estimated_hours: 3
    file_path: "crates/context-graph-graph/src/index/gpu_memory.rs"
    dependencies:
      - "M04-T10"
    acceptance_criteria:
      - "GpuMemoryManager struct with budget tracking"
      - "allocate() reserves memory and returns handle"
      - "free() releases memory back to pool"
      - "available() returns remaining budget"
      - "Returns GpuResourceAllocation error if over budget"
      - "Thread-safe (Arc<Mutex<>> or similar)"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/gpu_memory_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 10"
      - "NFR-KG-001"

  - id: "M04-T29"
    title: "Create Performance Benchmark Suite"
    description: |
      Implement comprehensive benchmark suite validating all NFR targets.
      Benchmarks:
      - FAISS k=10 search (<5ms target)
      - FAISS k=100 search (<10ms target)
      - Poincare distance CPU (<10us target)
      - Poincare distance GPU batch (<1ms for 1Kx1K)
      - Cone containment CPU (<50us target)
      - Cone containment GPU batch (<2ms for 1Kx1K)
      - BFS depth=6 (<100ms target)
      - Domain-aware search (<10ms target)
      Use criterion for benchmarking.
    layer: "surface"
    priority: "high"
    estimated_hours: 4
    file_path: "crates/context-graph-graph/benches/benchmark_suite.rs"
    dependencies:
      - "M04-T25"
      - "M04-T28"
    acceptance_criteria:
      - "All NFR targets have benchmarks"
      - "Benchmarks use realistic data sizes (10M vectors where specified)"
      - "Results output in criterion format"
      - "CI can run benchmarks with --no-fail-fast"
      - "GPU benchmarks marked with #[requires_gpu]"
      - "Compiles with `cargo build`"
      - "Benchmarks run with `cargo bench`"
      - "No clippy warnings"
    test_file: "N/A - benchmark, not test"
    spec_refs:
      - "TECH-GRAPH-004 Section 11"
      - "All NFR-KG requirements"

  - id: "M04-T25"
    title: "Create Module Integration Tests"
    description: |
      Implement comprehensive integration tests for Module 4:
      - End-to-end FAISS index lifecycle (train, add, search)
      - Hyperbolic distance computation (CPU vs GPU comparison)
      - Entailment cone containment queries
      - Graph traversal with Marblestone edge modulation
      - Domain-aware search ranking
      - Contradiction detection pipeline
      Performance benchmarks against NFR targets.
    layer: "surface"
    priority: "critical"
    estimated_hours: 6
    file_path: "crates/context-graph-graph/tests/integration_tests.rs"
    dependencies:
      - "M04-T18"
      - "M04-T19"
      - "M04-T20"
      - "M04-T21"
      - "M04-T23"
      - "M04-T24"
      - "M04-T26"
      - "M04-T27"
    acceptance_criteria:
      - "FAISS search returns correct top-k in <10ms"
      - "Hyperbolic distance CPU/GPU match within tolerance"
      - "Entailment query finds correct hierarchy"
      - "BFS traversal respects depth limits"
      - "Domain-aware search re-ranks correctly"
      - "All tests use real FAISS index (no mocks per spec)"
      - "Tests marked #[requires_gpu] for CI skip on non-GPU"
      - "Compiles with `cargo build`"
      - "Tests pass with `cargo test`"
      - "No clippy warnings"
    test_file: "crates/context-graph-graph/tests/integration_tests.rs"
    spec_refs:
      - "TECH-GRAPH-004 Section 11"
      - "All NFR-KG requirements"
```

---

## Dependency Graph

```
M04-T00 (Create Crate) ──────────────────────────────────────────────────┐
    │                                                                     │
    ├──► M04-T01 (IndexConfig) ──► M04-T01a (Vector1536)                 │
    │         │                                                           │
    │         └─────────────────────────────────────────────────────────┐ │
    │                                                                   │ │
    ├──► M04-T02 (HyperbolicConfig) ──► M04-T02a (Curvature Validation) │ │
    │         │                                                         │ │
    │         └──► M04-T04 (PoincarePoint) ──► M04-T05 (PoincareBall)   │ │
    │                                               │                   │ │
    ├──► M04-T03 (ConeConfig) ──────────────────────┼───────────────────┤ │
    │                                               │                   │ │
    │              ┌────────────────────────────────┘                   │ │
    │              │                                                     │ │
    │              ▼                                                     │ │
    │         M04-T06 (EntailmentCone) ──► M04-T07 (Containment)        │ │
    │                                            │                       │ │
    │                                            └──► M04-T27 (Formula)  │ │
    │                                            │                       │ │
    │                                            └──► M04-T24 (Cone CUDA)│ │
    │                                                                    │ │
    └──► M04-T08 (GraphError) ──► M04-T08a (Error Conversions)          │ │
              │                                                          │ │
              │                                                          │ │
              ▼                                                          │ │
         M04-T09 (FAISS FFI) ────────────────────────────────────────────┘ │
              │                                                            │
              └──► M04-T10 (FaissGpuIndex) ◄───────────────────────────────┘
                        │
                        ├──► M04-T11 (SearchResult)
                        │
                        ├──► M04-T28 (GPU Memory Manager)
                        │
                        └──► M04-T18 (Semantic Search)
                                  │
                                  └──► M04-T19 (Domain Search)
                                  │
                                  └──► M04-T21 (Contradiction) ◄── M04-T26 (CONTRADICTS)

M04-T08a ──► M04-T12 (Column Families) ──► M04-T13 (GraphStorage) ──► M04-T13a (Migrations)
                                                 │
                                                 ├──► M04-T16 (BFS)
                                                 │
                                                 ├──► M04-T17 (DFS) ──► M04-T17a (A*)
                                                 │
                                                 └──► M04-T20 (Entailment Query)

M04-T14 (NTWeights) ──► M04-T14a (NT Validation) ──► M04-T15 (GraphEdge)
                                                           │
                                                           ├──► M04-T22 (get_modulated_weight)
                                                           │
                                                           ├──► M04-T26 (CONTRADICTS)
                                                           │
                                                           └──► M04-T16 (BFS)

M04-T05 ──► M04-T23 (Poincare CUDA)

M04-T18 + M04-T19 + M04-T20 + M04-T21 + M04-T23 + M04-T24 + M04-T26 + M04-T27 ──► M04-T25 (Integration)
                                                                                        │
                                                                               M04-T28 ─┴─► M04-T29 (Benchmarks)
```

---

## Implementation Order (Recommended)

### Week 1: Foundation Types (12 tasks)
1. **M04-T00**: Create crate structure (CRITICAL - FIRST)
2. **M04-T14**: NeurotransmitterWeights (can parallel with T00)
3. **M04-T01**: IndexConfig for FAISS
4. **M04-T02**: HyperbolicConfig for Poincare ball
5. **M04-T03**: ConeConfig for entailment
6. **M04-T08**: GraphError enum
7. **M04-T01a**: Vector1536 re-export
8. **M04-T02a**: Curvature validation
9. **M04-T04**: PoincarePoint struct
10. **M04-T08a**: Error conversions
11. **M04-T05**: PoincareBall Mobius operations
12. **M04-T06**: EntailmentCone struct
13. **M04-T07**: Cone containment logic

### Week 2: FAISS and Storage (11 tasks)
14. **M04-T09**: FAISS FFI bindings
15. **M04-T10**: FaissGpuIndex wrapper
16. **M04-T11**: SearchResult struct
17. **M04-T12**: Column family definitions
18. **M04-T13**: GraphStorage backend
19. **M04-T13a**: Storage migrations
20. **M04-T14a**: NT weight validation
21. **M04-T15**: GraphEdge with Marblestone
22. **M04-T26**: EdgeType::CONTRADICTS

### Week 3: Traversal and Query (8 tasks)
23. **M04-T16**: BFS traversal
24. **M04-T17**: DFS traversal
25. **M04-T17a**: A* traversal
26. **M04-T18**: Semantic search
27. **M04-T19**: Domain-aware search (Marblestone)
28. **M04-T20**: Entailment query
29. **M04-T21**: Contradiction detection
30. **M04-T22**: get_modulated_weight function

### Week 4: CUDA Kernels and Fixes (4 tasks)
31. **M04-T23**: Poincare distance CUDA kernel
32. **M04-T24**: Cone membership CUDA kernel
33. **M04-T27**: Fix formula conflicts
34. **M04-T28**: GPU memory manager

### Week 5: Integration (2 tasks)
35. **M04-T25**: Integration tests
36. **M04-T29**: Performance benchmark suite

---

## Quality Gates

| Gate | Criteria | Required For |
|------|----------|--------------|
| **Gate 0: Crate Exists** | M04-T00 complete, `cargo build` succeeds | All other tasks |
| **Gate 1: Foundation Complete** | M04-T00 through M04-T08a pass all tests | Week 2 start |
| **Gate 2: Index Functional** | M04-T09 through M04-T15, M04-T26 pass all tests | Week 3 start |
| **Gate 3: Queries Operational** | M04-T16 through M04-T22 pass all tests | Week 4 start |
| **Gate 4: GPU Kernels Ready** | M04-T23, M04-T24, M04-T27, M04-T28 compile and test | Week 5 start |
| **Gate 5: Module Complete** | All 33 tasks complete, benchmarks pass NFR targets | Module 5 start |

---

## Performance Targets Summary

| Operation | Target | Conditions |
|-----------|--------|------------|
| FAISS k=10 search | <5ms | nprobe=128, 10M vectors |
| FAISS k=100 search | <10ms | nprobe=128, 10M vectors |
| Poincare distance (CPU) | <10us | Single pair |
| Poincare distance (GPU) | <1ms | 1K x 1K batch |
| Cone containment (CPU) | <50us | Single check |
| Cone containment (GPU) | <2ms | 1K x 1K batch |
| BFS depth=6 | <100ms | 10M nodes |
| Domain-aware search | <10ms | k=10, 10M vectors |
| Entailment query | <1ms | Per cone check |

---

## Memory Budget

| Component | Budget |
|-----------|--------|
| FAISS GPU index (10M vectors) | 8GB |
| Hyperbolic coordinates (10M nodes) | 2.5GB |
| Entailment cones (10M nodes) | 2.7GB |
| RocksDB cache | 8GB |
| Working memory | 10.8GB |
| **Total VRAM** | **24GB (RTX 5090)** |

---

## File Structure (Expected)

```
crates/context-graph-graph/
  Cargo.toml
  src/
    lib.rs                           # M04-T00, M04-T01a, M04-T18, M04-T20, M04-T21
    config.rs                        # M04-T01, M04-T02, M04-T02a, M04-T03
    error.rs                         # M04-T08, M04-T08a
    hyperbolic/
      mod.rs
      poincare.rs                    # M04-T04
      mobius.rs                      # M04-T05
    entailment/
      mod.rs
      cones.rs                       # M04-T06, M04-T07, M04-T27
    index/
      mod.rs
      faiss_ffi.rs                   # M04-T09
      gpu_index.rs                   # M04-T10, M04-T11
      gpu_memory.rs                  # M04-T28
    storage/
      mod.rs                         # M04-T12
      rocksdb.rs                     # M04-T13
      migrations.rs                  # M04-T13a
      edges.rs                       # M04-T14, M04-T14a, M04-T15, M04-T26
    traversal/
      mod.rs
      bfs.rs                         # M04-T16
      dfs.rs                         # M04-T17
      astar.rs                       # M04-T17a
    marblestone/
      mod.rs                         # M04-T22
      domain_search.rs               # M04-T19
  tests/
    config_tests.rs                  # M04-T01, M04-T02, M04-T02a, M04-T03
    error_tests.rs                   # M04-T08, M04-T08a
    poincare_tests.rs                # M04-T04
    mobius_tests.rs                  # M04-T05
    entailment_tests.rs              # M04-T06, M04-T07, M04-T27
    faiss_ffi_tests.rs               # M04-T09
    gpu_index_tests.rs               # M04-T10
    gpu_memory_tests.rs              # M04-T28
    search_result_tests.rs           # M04-T11
    storage_tests.rs                 # M04-T12, M04-T13, M04-T13a
    marblestone_tests.rs             # M04-T14, M04-T14a, M04-T22
    edge_tests.rs                    # M04-T15, M04-T26
    traversal_tests.rs               # M04-T16, M04-T17, M04-T17a
    search_tests.rs                  # M04-T18
    domain_search_tests.rs           # M04-T19
    entailment_query_tests.rs        # M04-T20
    contradiction_tests.rs           # M04-T21
    cuda_tests.rs                    # M04-T23, M04-T24
    integration_tests.rs             # M04-T25
  benches/
    benchmark_suite.rs               # M04-T29

crates/context-graph-cuda/kernels/
    poincare_distance.cu             # M04-T23
    cone_check.cu                    # M04-T24
```

---

## Marblestone Integration Summary

Tasks with Marblestone features:
- **M04-T14**: NeurotransmitterWeights struct (excitatory/inhibitory/modulatory)
- **M04-T14a**: NT weight validation and net_activation()
- **M04-T15**: GraphEdge with domain, steering_reward, is_amortized_shortcut
- **M04-T16**: BFS with domain-aware edge filtering
- **M04-T19**: domain_aware_search() with NT modulation
- **M04-T22**: get_modulated_weight() pure function

---

## Critical Constraints

**NO MOCK FAISS**: Per spec REQ-KG-TEST, all tests MUST use real FAISS GPU index.
- Mock implementations are forbidden for vector similarity search
- Tests requiring GPU should be marked `#[requires_gpu]` for CI handling

**NO BACKWARDS COMPATIBILITY**: Fail fast on invalid inputs.
- Do not add compatibility shims for old data formats
- Use migrations (M04-T13a) for schema changes

**Hyperbolic Constraint**: All PoincarePoint instances MUST maintain ||coords|| < 1.0.
- project() must be called after any operation that could push points to boundary

**All Tests Must**:
- Compile with `cargo build`
- Pass with `cargo test`
- Have no clippy warnings

---

## New Tasks Summary (8 Added)

| Task ID | Title | Layer | Justification |
|---------|-------|-------|---------------|
| M04-T00 | Create crate structure | foundation | Crate does not exist (CRITICAL) |
| M04-T01a | Re-export Vector1536 | foundation | Missing type dependency |
| M04-T02a | Curvature validation | foundation | Spec requires c < 0 |
| M04-T08a | Error conversions | foundation | From traits needed for ? operator |
| M04-T13a | Storage migrations | logic | Schema versioning needed |
| M04-T14a | NT weight validation | logic | Range [0,1] not enforced |
| M04-T17a | A* traversal | logic | Hyperbolic heuristics needed |
| M04-T26 | EdgeType::CONTRADICTS | surface | Missing edge type for T21 |
| M04-T27 | Fix formula conflicts | surface | Containment formulas inconsistent |
| M04-T28 | GPU memory manager | surface | VRAM budget tracking |
| M04-T29 | Benchmark suite | surface | Performance validation |

---

*Generated: 2025-12-31*
*Updated: 2026-01-02*
*Module: 04 - Knowledge Graph*
*Version: 2.0.0*
*Total Tasks: 33 (25 original + 8 new)*
