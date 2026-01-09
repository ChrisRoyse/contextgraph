# TASK-CORE-003: Define TeleologicalArray Type

```xml
<task_spec id="TASK-CORE-003" version="1.0">
<metadata>
  <title>Define TeleologicalArray Type with 13-Embedder Storage</title>
  <status>todo</status>
  <layer>foundation</layer>
  <sequence>3</sequence>
  <implements>
    <requirement_ref>REQ-TELEOLOGICAL-02</requirement_ref>
    <requirement_ref>REQ-STORAGE-ATOMIC-01</requirement_ref>
  </implements>
  <depends_on>
    <task_ref>TASK-CORE-002</task_ref>
  </depends_on>
  <estimated_complexity>medium</estimated_complexity>
  <estimated_days>2</estimated_days>
</metadata>

<context>
The TeleologicalArray is the fundamental data structure for the refactored system.
It stores 13 embeddings (one per embedder) as an atomic unit. This replaces the
broken pattern of comparing single embeddings to multi-embedder fingerprints.
Depends on TASK-CORE-002 for the Embedder enum.
</context>

<objective>
Create the TeleologicalArray struct that holds 13 embedder outputs as a fixed array,
with support for dense, sparse, and token-level embedding formats.
</objective>

<rationale>
Storing all 13 embeddings together ensures:
1. Atomic storage/retrieval - never partial arrays
2. Apples-to-apples comparison - both arrays have same structure
3. Per-embedder indexing - each dimension searchable independently
4. Efficient serialization - single unit for persistence

The array uses EmbedderOutput enum to handle different embedding types:
- Dense: Fixed-size f32 vectors
- Sparse: Index-value pairs for SPLADE
- TokenLevel: Per-token embeddings for ColBERT
</rationale>

<input_context_files>
  <file purpose="embedder_enum">crates/context-graph-core/src/teleology/embedder.rs</file>
  <file purpose="architecture_reference">docs2/refactor/01-ARCHITECTURE.md</file>
  <file purpose="storage_spec">docs2/refactor/02-STORAGE.md</file>
</input_context_files>

<prerequisites>
  <check>TASK-CORE-002 complete (Embedder enum exists)</check>
  <check>context-graph-core compiles</check>
</prerequisites>

<scope>
  <in_scope>
    <item>Create TeleologicalArray struct with 13-entry fixed array</item>
    <item>Create EmbedderOutput enum (Dense, Sparse, TokenLevel)</item>
    <item>Create SparseVector type for SPLADE embeddings</item>
    <item>Create TokenEmbeddings type for ColBERT</item>
    <item>Implement MessagePack serialization</item>
    <item>Implement bincode serialization</item>
    <item>Add metadata fields (id, created_at, source_content_hash)</item>
    <item>Implement Default, Clone, Debug traits</item>
  </in_scope>
  <out_of_scope>
    <item>Comparison types (TASK-CORE-004)</item>
    <item>Similarity functions (TASK-LOGIC-001 through 003)</item>
    <item>Storage implementation (TASK-CORE-006 through 008)</item>
  </out_of_scope>
</scope>

<definition_of_done>
  <signatures>
    <signature file="crates/context-graph-core/src/teleology/array.rs">
      use crate::teleology::embedder::Embedder;
      use uuid::Uuid;
      use chrono::{DateTime, Utc};

      /// A sparse vector for SPLADE-style embeddings.
      #[derive(Debug, Clone, PartialEq)]
      pub struct SparseVector {
          pub indices: Vec<u32>,
          pub values: Vec<f32>,
      }

      /// Token-level embeddings for ColBERT-style late interaction.
      #[derive(Debug, Clone, PartialEq)]
      pub struct TokenEmbeddings {
          pub embeddings: Vec<Vec<f32>>,
          pub token_count: usize,
          pub dims_per_token: usize,
      }

      /// Output from a single embedder in the teleological array.
      #[derive(Debug, Clone, PartialEq)]
      pub enum EmbedderOutput {
          /// Dense floating-point vector
          Dense(Vec<f32>),
          /// Sparse vector (SPLADE)
          Sparse(SparseVector),
          /// Token-level embeddings (ColBERT)
          TokenLevel(TokenEmbeddings),
          /// Binary vector (HDC)
          Binary(Vec<u8>),
          /// Not yet computed
          Pending,
          /// Failed to compute
          Failed(String),
      }

      /// The fundamental storage unit: 13 embeddings as an atomic array.
      #[derive(Debug, Clone)]
      pub struct TeleologicalArray {
          /// Unique identifier
          pub id: Uuid,
          /// The 13 embedder outputs
          pub embeddings: [EmbedderOutput; 13],
          /// Source content hash for deduplication
          pub source_hash: u64,
          /// Creation timestamp
          pub created_at: DateTime<Utc>,
          /// Optional metadata
          pub metadata: Option<ArrayMetadata>,
      }

      #[derive(Debug, Clone, Default)]
      pub struct ArrayMetadata {
          pub namespace: Option<String>,
          pub memory_type: Option<String>,
          pub tags: Vec<String>,
          pub custom: std::collections::HashMap<String, String>,
      }

      impl TeleologicalArray {
          pub fn new(id: Uuid) -> Self;
          pub fn with_embeddings(id: Uuid, embeddings: [EmbedderOutput; 13]) -> Self;
          pub fn get(&self, embedder: Embedder) -> &EmbedderOutput;
          pub fn set(&mut self, embedder: Embedder, output: EmbedderOutput);
          pub fn is_complete(&self) -> bool;
          pub fn completed_count(&self) -> usize;
          pub fn storage_bytes(&self) -> usize;
      }

      impl SparseVector {
          pub fn new(indices: Vec<u32>, values: Vec<f32>) -> Self;
          pub fn len(&self) -> usize;
          pub fn is_empty(&self) -> bool;
          pub fn active_dimensions(&self) -> usize;
      }

      impl TokenEmbeddings {
          pub fn new(embeddings: Vec<Vec<f32>>, dims_per_token: usize) -> Self;
          pub fn token_count(&self) -> usize;
      }
    </signature>
  </signatures>

  <constraints>
    <constraint>Array size is exactly 13 (compile-time enforced)</constraint>
    <constraint>All embeddings stored together atomically</constraint>
    <constraint>No partial arrays in storage</constraint>
    <constraint>MessagePack serialization must roundtrip correctly</constraint>
    <constraint>Memory layout efficient (no excessive padding)</constraint>
    <constraint>Implements Serialize, Deserialize via serde</constraint>
  </constraints>

  <verification>
    <command>cargo check -p context-graph-core</command>
    <command>cargo test -p context-graph-core array</command>
    <command>cargo test -p context-graph-core serialization</command>
  </verification>
</definition_of_done>

<pseudo_code>
// crates/context-graph-core/src/teleology/array.rs

use crate::teleology::embedder::Embedder;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

impl SparseVector {
    pub fn new(indices: Vec<u32>, values: Vec<f32>) -> Self {
        debug_assert_eq!(indices.len(), values.len());
        Self { indices, values }
    }

    pub fn len(&self) -> usize { self.indices.len() }
    pub fn is_empty(&self) -> bool { self.indices.is_empty() }
    pub fn active_dimensions(&self) -> usize { self.indices.len() }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenEmbeddings {
    pub embeddings: Vec<Vec<f32>>,
    pub token_count: usize,
    pub dims_per_token: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EmbedderOutput {
    Dense(Vec<f32>),
    Sparse(SparseVector),
    TokenLevel(TokenEmbeddings),
    Binary(Vec<u8>),
    Pending,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeleologicalArray {
    pub id: Uuid,
    pub embeddings: [EmbedderOutput; 13],
    pub source_hash: u64,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<ArrayMetadata>,
}

impl TeleologicalArray {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            embeddings: std::array::from_fn(|_| EmbedderOutput::Pending),
            source_hash: 0,
            created_at: Utc::now(),
            metadata: None,
        }
    }

    pub fn get(&self, embedder: Embedder) -> &EmbedderOutput {
        &self.embeddings[embedder.index()]
    }

    pub fn set(&mut self, embedder: Embedder, output: EmbedderOutput) {
        self.embeddings[embedder.index()] = output;
    }

    pub fn is_complete(&self) -> bool {
        self.embeddings.iter().all(|e| !matches!(e, EmbedderOutput::Pending))
    }

    pub fn completed_count(&self) -> usize {
        self.embeddings.iter()
            .filter(|e| !matches!(e, EmbedderOutput::Pending | EmbedderOutput::Failed(_)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_array_size() {
        let arr = TeleologicalArray::new(Uuid::new_v4());
        assert_eq!(arr.embeddings.len(), 13);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let arr = TeleologicalArray::new(Uuid::new_v4());
        let bytes = rmp_serde::to_vec(&arr).unwrap();
        let restored: TeleologicalArray = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(arr.id, restored.id);
    }
}
</pseudo_code>

<files_to_create>
  <file path="crates/context-graph-core/src/teleology/array.rs">
    TeleologicalArray struct and associated types
  </file>
</files_to_create>

<files_to_modify>
  <file path="crates/context-graph-core/src/teleology/mod.rs">
    Add: pub mod array;
  </file>
  <file path="crates/context-graph-core/Cargo.toml">
    Add dependencies: uuid, chrono, rmp-serde
  </file>
</files_to_modify>

<validation_criteria>
  <criterion>TeleologicalArray::embeddings has exactly 13 elements</criterion>
  <criterion>Serialization roundtrip preserves all data</criterion>
  <criterion>is_complete() returns false for new arrays</criterion>
  <criterion>completed_count() correctly counts non-Pending embeddings</criterion>
  <criterion>SparseVector indices and values have same length</criterion>
</validation_criteria>

<test_commands>
  <command>cargo test -p context-graph-core array -- --nocapture</command>
  <command>cargo test -p context-graph-core serialization -- --nocapture</command>
</test_commands>
</task_spec>
```
