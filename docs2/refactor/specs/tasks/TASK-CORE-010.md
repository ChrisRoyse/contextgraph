# TASK-CORE-010: Module Integration

```xml
<task_spec id="TASK-CORE-010" version="1.0">
<metadata>
  <title>Integrate Teleology Modules into Crate Structure</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>10</sequence>
  <implements>
    <requirement_ref>REQ-MODULE-STRUCTURE-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-005</task_ref>
    <task_ref>TASK-CORE-008</task_ref>
    <task_ref>TASK-CORE-009</task_ref>
  </depends_on>
  <estimated_complexity>low</estimated_complexity>
  <estimated_days>1</estimated_days>
</metadata>

<context>
Final foundation task that integrates all new teleology modules into the crate
structure. Creates proper module files, updates lib.rs exports, and ensures
workspace dependencies are correct.
</context>

<objective>
Create teleology module files in both context-graph-core and context-graph-storage,
export all public types, and verify the complete foundation layer compiles and
is accessible from crate roots.
</objective>

<rationale>
Proper module organization:
1. Enables clean imports from dependent crates
2. Establishes public API surface
3. Validates all foundation work integrates correctly
4. Sets up for logic layer implementation
</rationale>

<input_context_files>
  <file purpose="core_lib">crates/context-graph-core/src/lib.rs</file>
  <file purpose="storage_lib">crates/context-graph-storage/src/lib.rs</file>
  <file purpose="teleology_mod">crates/context-graph-core/src/teleology/mod.rs</file>
  <file purpose="storage_teleological_mod">crates/context-graph-storage/src/teleological/mod.rs</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-005 complete (GoalNode updated)</check>
  <check>TASK-CORE-008 complete (RocksDB store implemented)</check>
  <check>TASK-CORE-009 complete (projection code removed)</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create teleology/mod.rs in context-graph-core</item>
    <item>Create teleological/mod.rs in context-graph-storage</item>
    <item>Export all public types from crate lib.rs</item>
    <item>Update workspace Cargo.toml dependencies</item>
    <item>Verify cross-crate imports work</item>
    <item>Run full workspace build</item>
  </in_scope>
  <out_of_scope>
    <item>New logic implementation (Layer 2 tasks)</item>
    <item>MCP handler updates (Layer 3 tasks)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/mod.rs">
      //! Teleological array types for multi-embedder memory representation.

      pub mod array;
      pub mod comparison;
      pub mod embedder;

      // Re-exports for convenience
      pub use array::{TeleologicalArray, EmbedderOutput, SparseVector, TokenEmbeddings};
      pub use comparison::{ComparisonType, ComparisonResult, SearchMatrix, EmbedderWeights};
      pub use embedder::{Embedder, EmbedderDims, EmbedderMask, EmbedderGroup};
    </signature>
    <signature file="crates/context-graph-core/src/lib.rs">
      pub mod teleology;
      pub mod purpose;
      pub mod alignment;

      // Public re-exports
      pub use teleology::{TeleologicalArray, Embedder, ComparisonType};
      pub use purpose::{GoalNode, GoalLevel};
    </signature>
    <signature file="crates/context-graph-storage/src/teleological/mod.rs">
      //! Storage layer for teleological arrays.

      pub mod store;
      pub mod index;
      pub mod rocksdb_store;

      // Re-exports
      pub use store::{TeleologicalArrayStore, IndexedTeleologicalStore, StorageResult, StorageError};
      pub use index::{EmbedderIndex, IndexType, EmbedderIndexConfig};
      pub use rocksdb_store::RocksDbTeleologicalStore;
    </signature>
    <signature file="crates/context-graph-storage/src/lib.rs">
      pub mod teleological;

      pub use teleological::{TeleologicalArrayStore, RocksDbTeleologicalStore};
    </signature>
  </signatures>

  <constraints>
    <constraint>All modules compile without errors</constraint>
    <constraint>Public types accessible from crate root</constraint>
    <constraint>Cross-crate dependencies work</constraint>
    <constraint>cargo doc generates valid documentation</constraint>
  </constraints>

  <verification>
    <command>cargo build --workspace</command>
    <command>cargo test --workspace</command>
    <command>cargo doc --workspace --no-deps</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/mod.rs

//! Teleological array types for multi-embedder memory representation.
//!
//! This module provides the core types for the teleological array system:
//! - [`TeleologicalArray`]: The 13-embedder array struct
//! - [`Embedder`]: Enum of the 13 embedding types
//! - [`ComparisonType`]: Strategies for comparing arrays
//! - [`SearchMatrix`]: 13x13 weight matrix for search

pub mod array;
pub mod comparison;
pub mod embedder;

pub use array::{
    TeleologicalArray,
    EmbedderOutput,
    SparseVector,
    TokenEmbeddings,
    ArrayMetadata,
};

pub use comparison::{
    ComparisonType,
    ComparisonResult,
    SearchMatrix,
    EmbedderWeights,
    ValidationError,
};

pub use embedder::{
    Embedder,
    EmbedderDims,
    EmbedderMask,
    EmbedderGroup,
};


// crates/context-graph-core/src/lib.rs

//! ContextGraph Core - Teleological Memory System
//!
//! Core types for the 13-embedder teleological array system.

pub mod teleology;
pub mod purpose;
pub mod alignment;

// Convenient re-exports
pub use teleology::{TeleologicalArray, Embedder, ComparisonType};
pub use purpose::{GoalNode, GoalLevel};


// crates/context-graph-storage/src/teleological/mod.rs

//! Storage layer for teleological arrays.
//!
//! Provides traits and implementations for persisting and searching
//! teleological arrays with per-embedder indexing.

pub mod store;
pub mod index;
pub mod rocksdb_store;

pub use store::{
    TeleologicalArrayStore,
    IndexedTeleologicalStore,
    StorageResult,
    StorageError,
    SearchResult,
    SearchFilter,
    StorageStats,
    IndexStats,
};

pub use index::{
    EmbedderIndex,
    IndexType,
    EmbedderIndexConfig,
    HnswConfig,
    InvertedIndexConfig,
};

pub use rocksdb_store::{
    RocksDbTeleologicalStore,
    StoreConfig,
    CF_ARRAYS,
    CF_METADATA,
};


// crates/context-graph-storage/src/lib.rs

//! ContextGraph Storage - Persistent Storage Layer

pub mod teleological;

pub use teleological::{
    TeleologicalArrayStore,
    RocksDbTeleologicalStore,
    StorageError,
    StorageResult,
};
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/mod.rs">
    Module file with re-exports (if not already created)
  </file>
  <file path="crates/context-graph-storage/src/teleological/mod.rs">
    Module file with re-exports (if not already created)
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/lib.rs">
    Add pub mod teleology and re-exports
  </file>
  <file path="crates/context-graph-storage/src/lib.rs">
    Add pub mod teleological and re-exports
  </file>
  <file path="Cargo.toml">
    Verify workspace member dependencies
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>cargo build --workspace succeeds</criterion>
  <criterion>cargo test --workspace passes</criterion>
  <criterion>cargo doc --workspace generates docs</criterion>
  <criterion>TeleologicalArray importable from core crate</criterion>
  <criterion>RocksDbTeleologicalStore importable from storage crate</criterion>
</validation_criteria>

<test_commands>
  <command>cargo build --workspace</command>
  <command>cargo test --workspace</command>
  <command>cargo doc --workspace --no-deps</command>
</test_commands>

<rollback_plan>
  This is an integration task. If issues, review individual component tasks.
</rollback_plan>
</task_spec>
```
