---
namespace: handoff
agent_id: agent-00-coordinator
timestamp: 2026-04-14T23:20:00Z
status: active
tags: handoff, typed-edges, implementation-spec, agent-01-to-02
---

# Implementation Spec: Typed-Edge Training-Data Factory

This is the concrete file-by-file plan. Every downstream agent (02 through 05) works from this spec. If you find anything wrong with this spec while implementing, STOP and write a blocker — do not silently deviate.

## Executive summary

Four coherent features. Land them in a single coding pass because they share types (`edge_type_distribution` depends on typed-edge iteration; Feature 2 produces `ContrastivePair` records the same way the existing miner does; Feature 4 adds a secondary label to Feature 1's export).

- **F1 — `export_typed_edges_corpus`**: new MCP tool, new CF `CF_TYPED_EDGE_RECORDS`, new encode/decode functions. Emits per-edge records joined with content + source_metadata + (optional) LLM validation from F4.
- **F2 — `derive_anomalies_from_edges`**: new MCP tool. Walks existing `CF_TYPED_EDGES` once. Classifies patterns into 5 of 6 AnomalyKinds. Writes into existing `CF_CONTRASTIVE_PAIRS` + secondary indexes. No new storage schema.
- **F3 — `edge_type_distribution: [u32; 8]`** on `TrainingRecord`: bump `TRAINING_RECORD_VERSION` 1 → 2. NO backwards compatibility. `build_training_record` computes the distribution from the outgoing-edges iterator it already walks.
- **F4 — `validate_low_confidence_edges`**: new MCP tool, new CF `CF_TYPED_EDGE_VALIDATIONS`. Calls the existing LLM-backed `validate_graph_link` tool path, persists the verdict. F1 exporter joins on this CF and includes both embedder-ensemble and LLM verdicts in each exported record.

## Identified ground truth (verified against code, not memory)

| Symbol | Path | Notes |
|---|---|---|
| `TrainingRecord` | `crates/context-graph-core/src/training/mod.rs` line 69 | Add `edge_type_distribution: [u32; 8]` at end of struct (bincode append-at-end rule). |
| `TRAINING_RECORD_VERSION` | `crates/context-graph-core/src/training/mod.rs` line 55 | Currently `1`. Bump to `2`. |
| `GraphLinkEdgeType` | `crates/context-graph-core/src/graph_linking/edge_type.rs` line 48 | 8 variants, u8 repr. `COUNT = 8`. Use `.as_u8()` to index into the 8-dim array. |
| `TypedEdge` | `crates/context-graph-core/src/graph_linking/typed_edge.rs` line 49 | Has `source()`, `target()`, `edge_type()`, `weight()`, `direction()`, `embedder_scores()` (array ref), `agreement_count()`, `agreeing_embedders()`. All public accessors. |
| `AnomalyKind` | `crates/context-graph-core/src/contrastive/types.rs` line 47 | 6 variants. `as_u8()` returns 0-5. `as_str()` returns snake_case. |
| `ContrastivePair` | `crates/context-graph-core/src/contrastive/types.rs` line 143 | Carries `anchor_id`, `negative_id`, `similarity_profile: [f32; 13]`, `high_embedders`, `low_embedders`, `disagreement_magnitude`, `anomaly_kind`, `mined_at`, `generator: String`. |
| `CF_TRAINING_RECORDS` | `crates/context-graph-storage/src/teleological/column_families.rs:399` | Existing. |
| `CF_CONTRASTIVE_PAIRS` | `crates/context-graph-storage/src/teleological/column_families.rs:453` | Existing. Write through existing `store_contrastive_pair` (atomic 3-CF WriteBatch). |
| `TELEOLOGICAL_CFS` | `crates/context-graph-storage/src/teleological/column_families.rs:482-509` | 26-entry array. Append 2 new entries (order doesn't matter logically but add at end for minimal diff). |
| `TELEOLOGICAL_CF_COUNT` | `crates/context-graph-storage/src/teleological/column_families.rs:512` | Currently `26`. Bump to `28`. |
| `TOTAL_COLUMN_FAMILIES` | `crates/context-graph-storage/src/column_families.rs:396` | Currently `57`. Bump to `59`. |
| `get_all_teleological_cf_descriptors` | `crates/context-graph-storage/src/teleological/column_families.rs` (search exact location) | Add 2 new `ColumnFamilyDescriptor` rows using new options functions. |
| `uuid_prefixed_cf_options` | `crates/context-graph-storage/src/teleological/column_families.rs:43` | Reusable helper for UUID-keyed 16-byte prefix CFs. |
| `apply_write_buffer_limits` | (same file) | Reusable helper. |
| `store_training_record` / `encode_training_record` / `decode_training_record` | `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs` | Pattern to mirror for new encoders. |
| `iter_all_typed_edges` | `crates/context-graph-storage/src/graph_edges/repository.rs:460` | Use to walk `CF_TYPED_EDGES` in F1/F2/F3. |
| `get_typed_edges_from` | line 312 | Use for per-memory edge distribution computation (F3). |
| `tool_dispatch!` macro | `crates/context-graph-mcp/src/handlers/tools/dispatch.rs:87` | Register new tool names here. |
| `tools/names.rs` | `crates/context-graph-mcp/src/tools/names.rs` | Add 4 new `pub const` name constants. |
| `store_contrastive_pair` | `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs` (search for exact fn) | Atomic 3-CF WriteBatch. Reuse verbatim for F2. |

## Deliverables — phase by phase

### Phase 2: Core types (`context-graph-core`)

**File: `crates/context-graph-core/src/training/mod.rs`**

- Bump `pub const TRAINING_RECORD_VERSION: u8 = 1;` → `pub const TRAINING_RECORD_VERSION: u8 = 2;`
- Add new constant right after: `pub const NUM_EDGE_TYPE_DISTRIBUTION: usize = 8;`
- Add field `pub edge_type_distribution: [u32; NUM_EDGE_TYPE_DISTRIBUTION]` at the END of `TrainingRecord` struct (after `tucker_core` — bincode requires append-only).
- Update docstring at top of file to note v2 bump and new field semantics. Also update the doc comment on `TrainingRecord` to mention `edge_type_distribution`.

**File: `crates/context-graph-core/src/training/tests.rs`**

- Update any test constructors that build `TrainingRecord` to include `edge_type_distribution: [0u32; 8]` (or specific values when the test asserts distribution behavior).
- Add new test `edge_type_distribution_sum_matches_outgoing_edges` — build a record with 5 outgoing edges where 2 are SemSim, 2 are CodeRelated, 1 is CausalChain. Build distribution manually, assert `[2, 2, 0, 1, 0, 0, 0, 0]` and `sum == 5`.
- Add new test `edge_type_distribution_bincode_roundtrip` — bincode encode/decode a TrainingRecord with non-zero distribution, assert field equality post-roundtrip.

**File: NEW `crates/context-graph-core/src/training/edge_signature.rs`**

```rust
//! Compute the 8-dim edge_type_distribution for a memory from an iterator of its
//! outgoing (or combined in+out) TypedEdges.

use crate::graph_linking::{GraphLinkEdgeType, TypedEdge};

/// Length of the edge-type distribution vector (matches `GraphLinkEdgeType::COUNT`).
pub const NUM_EDGE_TYPE_DISTRIBUTION: usize = 8;

/// Count each outgoing edge by its `edge_type`. Returns an 8-vector indexed by
/// `GraphLinkEdgeType::as_u8()`.
pub fn compute_edge_type_distribution<'a, I>(edges: I) -> [u32; NUM_EDGE_TYPE_DISTRIBUTION]
where
    I: IntoIterator<Item = &'a TypedEdge>,
{
    let mut dist = [0u32; NUM_EDGE_TYPE_DISTRIBUTION];
    for edge in edges {
        let idx = edge.edge_type().as_u8() as usize;
        debug_assert!(idx < NUM_EDGE_TYPE_DISTRIBUTION, "edge_type u8 must be 0..8");
        dist[idx] = dist[idx].saturating_add(1);
    }
    dist
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_linking::{DirectedRelation, TypedEdge};
    use uuid::Uuid;

    fn mk_edge(source: Uuid, target: Uuid, et: GraphLinkEdgeType, weight: f32) -> TypedEdge {
        let mut scores = [0f32; 13];
        if let Some(i) = et.primary_embedder_index() {
            scores[i] = weight;
        } else {
            // MultiAgreement — set 3 arbitrary slots
            scores[0] = weight; scores[5] = weight; scores[9] = weight;
        }
        let dir = if et.is_asymmetric() { DirectedRelation::Forward } else { DirectedRelation::Symmetric };
        let (count, bits) = if matches!(et, GraphLinkEdgeType::MultiAgreement) {
            (3u8, 0b0010_0010_0001u16) // E1 + E6 + E10
        } else if let Some(i) = et.primary_embedder_index() {
            (1u8, 1u16 << i)
        } else {
            (0, 0)
        };
        TypedEdge::new(source, target, et, weight, dir, scores, count, bits).expect("build edge")
    }

    #[test]
    fn empty_iterator_yields_zero_vector() {
        let edges: Vec<TypedEdge> = Vec::new();
        assert_eq!(compute_edge_type_distribution(&edges), [0u32; 8]);
    }

    #[test]
    fn single_edge_single_slot() {
        let s = Uuid::new_v4(); let t = Uuid::new_v4();
        let edges = vec![mk_edge(s, t, GraphLinkEdgeType::CausalChain, 0.7)];
        let dist = compute_edge_type_distribution(&edges);
        assert_eq!(dist[GraphLinkEdgeType::CausalChain.as_u8() as usize], 1);
        assert_eq!(dist.iter().sum::<u32>(), 1);
    }

    #[test]
    fn mixed_distribution_sums_correctly() {
        let s = Uuid::new_v4();
        let edges = vec![
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::SemanticSimilar, 0.9),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::SemanticSimilar, 0.85),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::CodeRelated, 0.75),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::CodeRelated, 0.72),
            mk_edge(s, Uuid::new_v4(), GraphLinkEdgeType::CausalChain, 0.65),
        ];
        let dist = compute_edge_type_distribution(&edges);
        let mut expected = [0u32; 8];
        expected[GraphLinkEdgeType::SemanticSimilar.as_u8() as usize] = 2;
        expected[GraphLinkEdgeType::CodeRelated.as_u8() as usize] = 2;
        expected[GraphLinkEdgeType::CausalChain.as_u8() as usize] = 1;
        assert_eq!(dist, expected);
        assert_eq!(dist.iter().sum::<u32>(), 5);
    }
}
```

Export `compute_edge_type_distribution` and `NUM_EDGE_TYPE_DISTRIBUTION` from `training/mod.rs`.

**File: NEW `crates/context-graph-core/src/typed_edge_export/mod.rs`**

New module (add `pub mod typed_edge_export;` to `core/src/lib.rs`).

Defines:

```rust
//! Wire types for `export_typed_edges_corpus` (F1). Each record carries one
//! (source, target, edge_type) observation plus full embedder scores and
//! (optionally) the LLM validation from F4.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::teleological::types::NUM_EMBEDDERS;

/// On-disk version byte for `TypedEdgeTrainingRecord`.
pub const TYPED_EDGE_RECORD_VERSION: u8 = 1;

/// One row of the exported typed-edge corpus.
///
/// Serialized with bincode (positional). New fields go at the end only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypedEdgeTrainingRecord {
    pub source_memory_id: Uuid,
    pub target_memory_id: Uuid,
    /// Edge type as `GraphLinkEdgeType::as_u8()` (0..=7).
    pub edge_type: u8,
    pub edge_type_name: String,     // snake_case; lets consumers filter without rebuilding the enum
    pub weight: f32,
    /// Direction: 0=Symmetric, 1=Forward, 2=Backward.
    pub direction: u8,
    pub embedder_scores: [f32; NUM_EMBEDDERS],
    pub agreement_count: u8,
    pub agreeing_embedders: u16,

    // Content join — optional; all empty String / None when caller asks to skip.
    pub source_content: String,
    pub target_content: String,
    pub source_session_id: Option<String>,
    pub target_session_id: Option<String>,
    pub source_type: Option<String>, // SourceMetadata::source_type joined from CF_SOURCE_METADATA
    pub target_type: Option<String>,

    // Mechanism sub-type for CausalChain edges (joined from CF_CAUSAL_RELATIONSHIPS when resolvable).
    pub mechanism_type: Option<String>,

    // LLM validation join — `None` when no validation exists for this edge.
    pub llm_validation: Option<LLMValidationSummary>,

    pub exported_at: DateTime<Utc>,
    pub exporter_version: String, // e.g. "typed_edge_export_v1"
}

/// Compact view of the LLM verdict for embedding inside a TypedEdgeTrainingRecord.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LLMValidationSummary {
    pub validated_at: DateTime<Utc>,
    pub verdict: LLMVerdict,
    pub confidence: f32,
    pub rationale: String,
    pub validator_version: String,
}

/// LLM verdict on a typed edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LLMVerdict {
    Valid,
    Invalid,
    /// The LLM accepted the link but classified it under a different edge type.
    Reclassify { new_edge_type: u8 },
}

impl LLMVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            LLMVerdict::Valid => "valid",
            LLMVerdict::Invalid => "invalid",
            LLMVerdict::Reclassify { .. } => "reclassify",
        }
    }
}

#[cfg(test)]
mod tests;
```

Tests file `crates/context-graph-core/src/typed_edge_export/tests.rs`:

```rust
use super::*;

#[test]
fn verdict_round_trips_through_bincode() {
    for v in [
        LLMVerdict::Valid,
        LLMVerdict::Invalid,
        LLMVerdict::Reclassify { new_edge_type: 3 },
    ] {
        let bytes = bincode::serialize(&v).unwrap();
        let back: LLMVerdict = bincode::deserialize(&bytes).unwrap();
        assert_eq!(v, back);
    }
}

#[test]
fn record_round_trips_with_full_content() {
    use uuid::Uuid;
    let rec = TypedEdgeTrainingRecord {
        source_memory_id: Uuid::new_v4(),
        target_memory_id: Uuid::new_v4(),
        edge_type: 3,
        edge_type_name: "causal_chain".into(),
        weight: 0.72,
        direction: 1,
        embedder_scores: {
            let mut s = [0f32; 13];
            s[0] = 0.8; s[4] = 0.72; s
        },
        agreement_count: 1,
        agreeing_embedders: 1 << 4,
        source_content: "A causes B.".into(),
        target_content: "B happens after A.".into(),
        source_session_id: Some("sess-1".into()),
        target_session_id: Some("sess-1".into()),
        source_type: Some("HookDescription".into()),
        target_type: Some("HookDescription".into()),
        mechanism_type: Some("direct".into()),
        llm_validation: Some(LLMValidationSummary {
            validated_at: chrono::Utc::now(),
            verdict: LLMVerdict::Valid,
            confidence: 0.9,
            rationale: "Direct cause-effect stated.".into(),
            validator_version: "hermes-2-pro-mistral-7b-q5-km@2026-04".into(),
        }),
        exported_at: chrono::Utc::now(),
        exporter_version: "typed_edge_export_v1".into(),
    };
    let bytes = bincode::serialize(&rec).unwrap();
    let back: TypedEdgeTrainingRecord = bincode::deserialize(&bytes).unwrap();
    assert_eq!(rec, back);
}

#[test]
fn record_round_trips_without_content_or_validation() {
    use uuid::Uuid;
    let rec = TypedEdgeTrainingRecord {
        source_memory_id: Uuid::new_v4(),
        target_memory_id: Uuid::new_v4(),
        edge_type: 0,
        edge_type_name: "semantic_similar".into(),
        weight: 0.8,
        direction: 0,
        embedder_scores: [0.0; 13],
        agreement_count: 1,
        agreeing_embedders: 1,
        source_content: String::new(),
        target_content: String::new(),
        source_session_id: None,
        target_session_id: None,
        source_type: None,
        target_type: None,
        mechanism_type: None,
        llm_validation: None,
        exported_at: chrono::Utc::now(),
        exporter_version: "typed_edge_export_v1".into(),
    };
    let bytes = bincode::serialize(&rec).unwrap();
    let back: TypedEdgeTrainingRecord = bincode::deserialize(&bytes).unwrap();
    assert_eq!(rec, back);
}
```

**File: NEW `crates/context-graph-core/src/llm_edge_validation/mod.rs`**

```rust
//! Persisted LLM verdict on one typed edge (F4).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::typed_edge_export::LLMVerdict;

/// On-disk version byte for `LLMEdgeValidation`.
pub const LLM_EDGE_VALIDATION_VERSION: u8 = 1;

/// An LLM-issued verdict on a single typed edge, persisted to
/// `CF_TYPED_EDGE_VALIDATIONS` keyed by `[source:16][target:16][edge_type:u8]`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LLMEdgeValidation {
    pub validated_at: DateTime<Utc>,
    pub verdict: LLMVerdict,
    /// `[0.0, 1.0]`.
    pub confidence: f32,
    /// 1-3 sentence justification from the grammar-constrained LLM output.
    pub rationale: String,
    /// Edge weight at the moment of validation (so drift can be audited).
    pub auto_derived_weight: f32,
    /// e.g. `"Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"`.
    pub validator_version: String,
    /// SHA-256 of the prompt actually sent to the LLM. Reproducibility.
    pub prompt_hash: [u8; 32],
}

#[cfg(test)]
mod tests;
```

Tests file covers bincode roundtrip happy + boundary cases.

Add `pub mod llm_edge_validation;` to `core/src/lib.rs`.

---

### Phase 3: Storage (`context-graph-storage`)

**File: `crates/context-graph-storage/src/teleological/column_families.rs`**

1. After `CF_CONTRASTIVE_BY_ANCHOR` const, add:

```rust
/// Column family for exported typed-edge training records (F1).
///
/// Populated by the `export_typed_edges_corpus` MCP tool. Each record is one
/// (source, target, edge_type) observation joined with content, source
/// metadata, optional mechanism_type, and (if present) an LLM validation.
///
/// Key: `[source_uuid: 16B][target_uuid: 16B][edge_type: u8]` (33 bytes). The
/// edge-type suffix lets the same `(source, target)` pair be exported under
/// multiple classifications when an LLM reclassification exists.
///
/// Value: `[version: u8 = TYPED_EDGE_RECORD_VERSION][bincode-encoded
/// TypedEdgeTrainingRecord]`.
///
/// # Storage Details
/// - LZ4 compression
/// - Bloom filter for fast composite-key lookups
/// - 33-byte prefix extractor so the whole composite key is a stable prefix
/// - 4MB write buffer (records are typically 2-10 KB each)
pub const CF_TYPED_EDGE_RECORDS: &str = "typed_edge_records";

/// Column family for LLM-validated typed edges (F4).
///
/// Written by `validate_low_confidence_edges` after an LLM call confirms,
/// rejects, or reclassifies an auto-derived edge.
///
/// Key: `[source_uuid: 16B][target_uuid: 16B][edge_type: u8]` (33 bytes) —
/// matches the primary-key shape of the edge being validated.
/// Value: `[version: u8 = LLM_EDGE_VALIDATION_VERSION][bincode-encoded
/// LLMEdgeValidation]`.
///
/// # Storage Details
/// - LZ4 compression (rationale text compresses)
/// - Bloom filter for fast existence checks
/// - 33-byte prefix extractor
/// - 2MB write buffer (append-light)
pub const CF_TYPED_EDGE_VALIDATIONS: &str = "typed_edge_validations";
```

2. Append both to `TELEOLOGICAL_CFS` array.
3. Bump `TELEOLOGICAL_CF_COUNT` 26 → 28.
4. Add `typed_edge_records_cf_options(cache)` and `typed_edge_validations_cf_options(cache)` functions mirroring `contrastive_pairs_cf_options` (33-byte prefix for records, same for validations; 4MB / 2MB write buffers respectively).
5. Locate the function that builds teleological-only descriptors (probably `get_all_teleological_cf_descriptors`, search for `ColumnFamilyDescriptor::new(CF_CONTRASTIVE_BY_ANCHOR,`). Append two more rows for the new CFs.

**File: `crates/context-graph-storage/src/column_families.rs`**

- Bump `pub const TOTAL_COLUMN_FAMILIES: usize = 57;` → `= 59;`.
- Update the comment above it to reflect 28 teleological instead of 26.

**File: NEW `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_export.rs`**

Implements the CRUD for `CF_TYPED_EDGE_RECORDS` and the exporter:

```rust
use context_graph_core::error::{CoreError, CoreResult};
use context_graph_core::typed_edge_export::{TypedEdgeTrainingRecord, TYPED_EDGE_RECORD_VERSION};
use rocksdb::{ColumnFamily, IteratorMode, WriteBatch};
use tracing::{debug, error};
use uuid::Uuid;

use crate::teleological::column_families::CF_TYPED_EDGE_RECORDS;
use super::store::RocksDbTeleologicalStore;
use super::types::TeleologicalStoreError;

impl RocksDbTeleologicalStore {
    #[inline]
    fn cf_typed_edge_records(&self) -> &ColumnFamily {
        self.db.cf_handle(CF_TYPED_EDGE_RECORDS)
            .expect("CF_TYPED_EDGE_RECORDS must exist")
    }

    /// Build the 33-byte composite key.
    #[inline]
    fn typed_edge_record_key(source: Uuid, target: Uuid, edge_type: u8) -> [u8; 33] {
        let mut k = [0u8; 33];
        k[..16].copy_from_slice(source.as_bytes());
        k[16..32].copy_from_slice(target.as_bytes());
        k[32] = edge_type;
        k
    }

    pub async fn store_typed_edge_record(&self, r: &TypedEdgeTrainingRecord) -> CoreResult<()> {
        let payload = encode_typed_edge_record(r)?;
        let key = Self::typed_edge_record_key(r.source_memory_id, r.target_memory_id, r.edge_type);
        let cf = self.cf_typed_edge_records();
        self.db.put_cf(cf, &key, &payload).map_err(|e| {
            error!(src=%r.source_memory_id, tgt=%r.target_memory_id, et=r.edge_type, error=%e,
                "ROCKSDB ERROR: Failed to store typed-edge record");
            TeleologicalStoreError::rocksdb_op("put_typed_edge_record", CF_TYPED_EDGE_RECORDS,
                Some(r.source_memory_id), e)
        })?;
        debug!(key_len=key.len(), "stored typed-edge record");
        Ok(())
    }

    pub async fn get_typed_edge_record(
        &self, source: Uuid, target: Uuid, edge_type: u8,
    ) -> CoreResult<Option<TypedEdgeTrainingRecord>> {
        let key = Self::typed_edge_record_key(source, target, edge_type);
        let cf = self.cf_typed_edge_records();
        match self.db.get_cf(cf, &key) {
            Ok(Some(bytes)) => decode_typed_edge_record(&bytes).map(Some),
            Ok(None) => Ok(None),
            Err(e) => {
                error!(src=%source, tgt=%target, et=edge_type, error=%e,
                    "ROCKSDB ERROR: Failed to read typed-edge record");
                Err(TeleologicalStoreError::rocksdb_op(
                    "get_typed_edge_record", CF_TYPED_EDGE_RECORDS, Some(source), e).into())
            }
        }
    }

    pub async fn list_typed_edge_record_keys(&self) -> CoreResult<Vec<(Uuid, Uuid, u8)>> {
        let cf = self.cf_typed_edge_records();
        let iter = self.db.iterator_cf(cf, IteratorMode::Start);
        let mut out = Vec::new();
        for item in iter {
            let (key, _value) = item.map_err(|e| {
                error!(error=%e, "ROCKSDB ERROR: iteration failed in list_typed_edge_record_keys");
                TeleologicalStoreError::rocksdb_op(
                    "iterate_typed_edge_records", CF_TYPED_EDGE_RECORDS, None, e)
            })?;
            if key.len() != 33 {
                return Err(CoreError::SerializationError(format!(
                    "CF_TYPED_EDGE_RECORDS key length {}, expected 33", key.len()
                )));
            }
            let mut s = [0u8;16]; s.copy_from_slice(&key[..16]);
            let mut t = [0u8;16]; t.copy_from_slice(&key[16..32]);
            out.push((Uuid::from_bytes(s), Uuid::from_bytes(t), key[32]));
        }
        Ok(out)
    }

    pub async fn count_typed_edge_records(&self) -> CoreResult<usize> {
        let cf = self.cf_typed_edge_records();
        Ok(self.db.iterator_cf(cf, IteratorMode::Start).count())
    }

    pub async fn clear_all_typed_edge_records(&self) -> CoreResult<usize> {
        let keys = self.list_typed_edge_record_keys().await?;
        let cf = self.cf_typed_edge_records();
        let mut batch = WriteBatch::default();
        for (s, t, et) in &keys {
            batch.delete_cf(cf, Self::typed_edge_record_key(*s, *t, *et));
        }
        self.db.write(batch).map_err(|e| {
            error!(error=%e, "ROCKSDB ERROR: batch clear of typed-edge records failed");
            TeleologicalStoreError::rocksdb_op(
                "clear_typed_edge_records", CF_TYPED_EDGE_RECORDS, None, e)
        })?;
        Ok(keys.len())
    }
}

pub fn encode_typed_edge_record(r: &TypedEdgeTrainingRecord) -> CoreResult<Vec<u8>> {
    let mut body = bincode::serialize(r).map_err(|e| {
        CoreError::SerializationError(format!("bincode serialize TypedEdgeTrainingRecord: {}", e))
    })?;
    let mut out = Vec::with_capacity(1 + body.len());
    out.push(TYPED_EDGE_RECORD_VERSION);
    out.append(&mut body);
    Ok(out)
}

pub fn decode_typed_edge_record(bytes: &[u8]) -> CoreResult<TypedEdgeTrainingRecord> {
    if bytes.is_empty() {
        return Err(CoreError::SerializationError(
            "typed-edge record payload is empty".into()));
    }
    let v = bytes[0];
    if v != TYPED_EDGE_RECORD_VERSION {
        return Err(CoreError::SerializationError(format!(
            "typed-edge record version mismatch: got {}, expected {}. \
             No automatic migration — re-run export_typed_edges_corpus.",
            v, TYPED_EDGE_RECORD_VERSION
        )));
    }
    bincode::deserialize(&bytes[1..]).map_err(|e| {
        CoreError::SerializationError(format!("bincode deserialize TypedEdgeTrainingRecord: {}", e))
    })
}
```

**File: NEW `crates/context-graph-storage/src/teleological/rocksdb_store/llm_validation.rs`**

Same pattern — `CF_TYPED_EDGE_VALIDATIONS` CRUD + encode/decode with `LLM_EDGE_VALIDATION_VERSION`. Key shape is the 33-byte composite key identical to typed_edge_records (share the helper).

**File: NEW `crates/context-graph-storage/src/teleological/rocksdb_store/anomaly_derivation.rs`**

Implements F2 — the pattern-matcher that walks `CF_TYPED_EDGES` and writes into `CF_CONTRASTIVE_PAIRS`:

```rust
//! F2: derive anomaly pairs directly from typed-edge combinations.
//!
//! Rather than re-scanning all memories and re-computing similarities, we
//! walk CF_TYPED_EDGES (already the output of the embedder ensemble) and
//! classify distinctive (source, target) pairs into five AnomalyKinds.
//!
//! The sixth kind, `Other`, is left to the offline miner since it requires
//! full per-embedder re-scoring.

use std::collections::HashMap;
use context_graph_core::contrastive::types::{AnomalyKind, ContrastivePair};
use context_graph_core::error::CoreResult;
use context_graph_core::graph_linking::{GraphLinkEdgeType, TypedEdge};
use chrono::Utc;
use tracing::{debug, info, warn};
use uuid::Uuid;

use super::store::RocksDbTeleologicalStore;

/// Derivation-time tunables.
#[derive(Debug, Clone)]
pub struct AnomalyDerivationConfig {
    /// Edge weight above which we treat an embedder as "high" for a pair.
    pub high_threshold: f32,
    /// Score below which we treat an embedder as "low" for a pair.
    /// Applied to the opposing-embedder slot inside `embedder_scores`.
    pub low_threshold: f32,
    /// Hard cap on total pairs written in one run.
    pub max_pairs: usize,
    /// Minimum `high - low` gap required to keep a pair (echoes the offline
    /// miner's `min_disagreement`).
    pub min_disagreement: f32,
}

impl Default for AnomalyDerivationConfig {
    fn default() -> Self {
        Self {
            high_threshold: 0.60,
            low_threshold: 0.30,
            max_pairs: 10_000,
            min_disagreement: 0.30,
        }
    }
}

/// Summary of one derivation pass.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnomalyDerivationSummary {
    pub edges_scanned: usize,
    pub pairs_written: usize,
    pub skipped_below_threshold: usize,
    pub per_kind_counts: HashMap<AnomalyKind, usize>,
    pub duration_ms: u64,
}

impl RocksDbTeleologicalStore {
    /// Walk CF_TYPED_EDGES once, classify pairs against the five expressible
    /// AnomalyKinds, and atomically write each hit into CF_CONTRASTIVE_PAIRS.
    ///
    /// Per-pair writes go through `store_contrastive_pair` to keep the
    /// secondary indexes (CF_CONTRASTIVE_BY_KIND, CF_CONTRASTIVE_BY_ANCHOR)
    /// in sync. Runs are idempotent because the composite primary key is
    /// (anchor, negative) — re-derivation just overwrites.
    pub async fn derive_anomalies_from_edges(
        &self,
        edge_repository: &crate::graph_edges::EdgeRepository,
        cfg: &AnomalyDerivationConfig,
    ) -> CoreResult<AnomalyDerivationSummary> {
        let t0 = std::time::Instant::now();
        let edges = edge_repository.iter_all_typed_edges()
            .map_err(|e| context_graph_core::error::CoreError::StorageError(
                format!("iter_all_typed_edges failed: {}", e)))?;

        let mut summary = AnomalyDerivationSummary::default();
        summary.edges_scanned = edges.len();

        for edge in edges.iter() {
            if summary.pairs_written >= cfg.max_pairs {
                info!(cap = cfg.max_pairs, "derive_anomalies_from_edges: max_pairs reached");
                break;
            }
            let Some((kind, high_embedders, low_embedders, disagreement)) =
                classify_edge_as_anomaly(edge, cfg) else { continue };
            if disagreement < cfg.min_disagreement {
                summary.skipped_below_threshold += 1;
                continue;
            }

            let pair = ContrastivePair {
                anchor_id: edge.source(),
                negative_id: edge.target(),
                // Content is looked up inside the write path via
                // multi_get_content to avoid re-scan cost.
                anchor_text: self.get_content(edge.source()).await?.unwrap_or_default(),
                negative_text: self.get_content(edge.target()).await?.unwrap_or_default(),
                similarity_profile: *edge.embedder_scores(),
                high_embedders,
                low_embedders,
                disagreement_magnitude: disagreement,
                anomaly_kind: kind,
                mined_at: Utc::now(),
                generator: "typed_edge_anomaly_derivation_v1".to_string(),
            };

            self.store_contrastive_pair(&pair).await?;
            summary.pairs_written += 1;
            *summary.per_kind_counts.entry(kind).or_insert(0) += 1;
        }
        summary.duration_ms = t0.elapsed().as_millis() as u64;
        debug!(scanned = summary.edges_scanned, written = summary.pairs_written,
            "derive_anomalies_from_edges complete");
        Ok(summary)
    }
}

/// Pure-function classifier used by `derive_anomalies_from_edges`.
/// Returns `None` when the edge does not fit any expressible kind.
fn classify_edge_as_anomaly(edge: &TypedEdge, cfg: &AnomalyDerivationConfig)
    -> Option<(AnomalyKind, Vec<u8>, Vec<u8>, f32)>
{
    let scores = edge.embedder_scores();
    // Helper
    let is_high = |i: usize| scores[i] >= cfg.high_threshold;
    let is_low  = |i: usize| scores[i] <= cfg.low_threshold;

    // E1=0, E5=4, E6=5, E7=6, E8=7, E9=8, E10=9, E11=10, E13=12
    match edge.edge_type() {
        GraphLinkEdgeType::SemanticSimilar => {
            // SemanticButNotCausal: E1 high, E5 low
            if is_high(0) && is_low(4) {
                return Some((AnomalyKind::SemanticButNotCausal, vec![0], vec![4], scores[0]-scores[4]));
            }
        }
        GraphLinkEdgeType::KeywordOverlap => {
            // KeywordButNotParaphrase: E6 or E13 high, E10 low
            let hi_idx = if is_high(5) { Some(5) } else if is_high(12) { Some(12) } else { None };
            if let Some(hi) = hi_idx {
                if is_low(9) {
                    return Some((AnomalyKind::KeywordButNotParaphrase,
                                 vec![hi as u8], vec![9], scores[hi]-scores[9]));
                }
            }
        }
        GraphLinkEdgeType::CodeRelated => {
            // CodeShapeButDifferentIntent: E7 high, E1 low
            if is_high(6) && is_low(0) {
                return Some((AnomalyKind::CodeShapeButDifferentIntent, vec![6], vec![0],
                             scores[6]-scores[0]));
            }
        }
        GraphLinkEdgeType::EntityShared => {
            // EntitySharedButDifferentStructure: E11 high, E8 low
            if is_high(10) && is_low(7) {
                return Some((AnomalyKind::EntitySharedButDifferentStructure,
                             vec![10], vec![7], scores[10]-scores[7]));
            }
        }
        _ => {
            // HdcRobustButSemanticDifferent — special: high E9, low E1, regardless of edge_type
            if is_high(8) && is_low(0) {
                return Some((AnomalyKind::HdcRobustButSemanticDifferent,
                             vec![8], vec![0], scores[8]-scores[0]));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::graph_linking::{DirectedRelation, GraphLinkEdgeType};
    use uuid::Uuid;

    fn edge_with(scores: [f32; 13], et: GraphLinkEdgeType) -> TypedEdge {
        let dir = if et.is_asymmetric() { DirectedRelation::Forward } else { DirectedRelation::Symmetric };
        // Set the primary embedder weight above the agreement threshold (0.5 for tests).
        let mut s = scores;
        if let Some(i) = et.primary_embedder_index() { if s[i] < 0.5 { s[i] = 0.8; } }
        // Determine bitset from scores >= 0.5 ignoring temporal.
        let mut bits = 0u16; let mut count = 0u8;
        for (i,x) in s.iter().enumerate() {
            if matches!(i, 1..=3) { continue; }
            if *x >= 0.5 { bits |= 1 << i; count += 1; }
        }
        let weight = if let Some(i) = et.primary_embedder_index() { s[i] } else { 0.8 };
        TypedEdge::new(Uuid::new_v4(), Uuid::new_v4(), et, weight, dir, s, count, bits).unwrap()
    }

    #[test]
    fn classifies_semantic_but_not_causal() {
        let mut s = [0f32; 13]; s[0] = 0.9; s[4] = 0.1;
        let e = edge_with(s, GraphLinkEdgeType::SemanticSimilar);
        let cfg = AnomalyDerivationConfig::default();
        let got = classify_edge_as_anomaly(&e, &cfg).unwrap();
        assert_eq!(got.0, AnomalyKind::SemanticButNotCausal);
        assert_eq!(got.1, vec![0]); assert_eq!(got.2, vec![4]);
        assert!((got.3 - 0.8).abs() < 1e-3);
    }

    #[test]
    fn classifies_code_shape_but_different_intent() {
        let mut s = [0f32; 13]; s[6] = 0.85; s[0] = 0.05;
        let e = edge_with(s, GraphLinkEdgeType::CodeRelated);
        let got = classify_edge_as_anomaly(&e, &AnomalyDerivationConfig::default()).unwrap();
        assert_eq!(got.0, AnomalyKind::CodeShapeButDifferentIntent);
    }

    #[test]
    fn classifies_hdc_cross_type() {
        // Edge type is ParaphraseAligned but pattern matches HDC anomaly.
        let mut s = [0f32; 13]; s[9] = 0.8; s[8] = 0.95; s[0] = 0.10;
        let e = edge_with(s, GraphLinkEdgeType::ParaphraseAligned);
        let got = classify_edge_as_anomaly(&e, &AnomalyDerivationConfig::default()).unwrap();
        assert_eq!(got.0, AnomalyKind::HdcRobustButSemanticDifferent);
    }

    #[test]
    fn no_classification_when_thresholds_not_met() {
        let s = [0.4f32; 13];
        let e = edge_with(s, GraphLinkEdgeType::SemanticSimilar);
        assert!(classify_edge_as_anomaly(&e, &AnomalyDerivationConfig::default()).is_none());
    }
}
```

**File: modify `crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs`**

Register the three new submodules:

```rust
pub mod typed_edge_export;
pub mod llm_validation;
pub mod anomaly_derivation;
```

**File: modify `crates/context-graph-storage/src/teleological/rocksdb_store/export.rs`**

- Inside `build_training_record` (or the function that walks outgoing edges to populate `outgoing_edges`), compute the distribution. Since we already walk outgoing edges, add one line: `let edge_type_distribution = context_graph_core::training::compute_edge_type_distribution(outgoing_edges.iter().map(…));`
- Hook that into the constructed `TrainingRecord` at the end of the function.
- If the function takes `&[TrainingEdge]` rather than `&[TypedEdge]`, compute from `TrainingEdge::edge_type` directly (it's already the u8 index).

Actually: the existing code builds `TrainingEdge` first (with `edge_type: u8` already set), so we compute `edge_type_distribution` from the built-up `Vec<TrainingEdge>` of outgoing edges. Add a helper:

```rust
fn edge_type_distribution_from_training_edges(edges: &[TrainingEdge]) -> [u32; 8] {
    let mut dist = [0u32; 8];
    for e in edges {
        if (e.edge_type as usize) < 8 {
            dist[e.edge_type as usize] = dist[e.edge_type as usize].saturating_add(1);
        }
    }
    dist
}
```

Use this inside `build_training_record` and set `record.edge_type_distribution = ...`.

---

### Phase 4: MCP Tools (`context-graph-mcp`)

**File: `crates/context-graph-mcp/src/tools/names.rs`**

Add four new constants:

```rust
pub const EXPORT_TYPED_EDGES_CORPUS: &str = "export_typed_edges_corpus";
pub const DERIVE_ANOMALIES_FROM_EDGES: &str = "derive_anomalies_from_edges";
pub const VALIDATE_LOW_CONFIDENCE_EDGES: &str = "validate_low_confidence_edges";
pub const LIST_TYPED_EDGE_RECORDS: &str = "list_typed_edge_records";
```

**File: NEW `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs`**

Four JSON schema definitions. Models:

| Tool | Schema surface |
|---|---|
| `export_typed_edges_corpus` | `{ maxEdges?: int default 10000, includeContent?: bool default true, includeSourceMetadata?: bool default true, includeMechanismType?: bool default true, joinLLMValidation?: bool default true, clearExisting?: bool default false }` |
| `derive_anomalies_from_edges` | `{ highThreshold?: float default 0.60, lowThreshold?: float default 0.30, minDisagreement?: float default 0.30, maxPairs?: int default 10000, kinds?: array<string> }` |
| `validate_low_confidence_edges` | `{ weightCeiling?: float default 0.60, maxEdges?: int default 100, sessionFilter?: string }` |
| `list_typed_edge_records` | `{ limit?: int default 100, offset?: int default 0, includeFull?: bool default false }` |

Each has `additionalProperties: false`.

**File: NEW `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs`**

Implements the four handlers. Each one:
1. Downcasts `teleological_store` to `RocksDbTeleologicalStore` (fail fast if not RocksDB).
2. Calls the storage methods we just wrote.
3. Returns structured JSON with metrics (count, duration_ms, etc.).

Outline:

```rust
use context_graph_core::graph_linking::GraphLinkEdgeType;
use context_graph_core::typed_edge_export::{TypedEdgeTrainingRecord, LLMValidationSummary};
use context_graph_storage::graph_edges::EdgeRepository;
use context_graph_storage::teleological::rocksdb_store::anomaly_derivation::AnomalyDerivationConfig;
use serde_json::{json, Value};
use tracing::{error, info};

use crate::handlers::core::Handlers;
use crate::protocol::McpError;

impl Handlers {
    pub async fn handle_export_typed_edges_corpus(&self, params: Value) -> Result<Value, McpError> {
        // Pull params (max_edges, include_content, include_source_metadata,
        // include_mechanism_type, join_llm_validation, clear_existing).
        // Downcast store.
        // If clear_existing: call store.clear_all_typed_edge_records().await.
        // Get edge_repo (self.edge_repository).
        // Walk edge_repo.iter_all_typed_edges().
        // For each edge:
        //   - optionally fetch source/target content, source_metadata.
        //   - optionally fetch mechanism_type (from CF_CAUSAL_RELATIONSHIPS join if edge.edge_type == CausalChain).
        //   - optionally fetch LLM validation (store.get_llm_edge_validation(…)).
        //   - build TypedEdgeTrainingRecord.
        //   - store_typed_edge_record(&rec).await.
        // Return { status: "exported", exported, skipped, duration_ms, storage: {...} }.
        todo!("implement per spec above")
    }

    // ... three more handlers ...
}
```

Flesh out in the coder step. Each handler < 200 lines.

**File: `crates/context-graph-mcp/src/handlers/tools/dispatch.rs`**

Add four arms in `tool_dispatch!` (after the contrastive ones):

```rust
EXPORT_TYPED_EDGES_CORPUS => self.handle_export_typed_edges_corpus(arguments).await;
DERIVE_ANOMALIES_FROM_EDGES => self.handle_derive_anomalies_from_edges(arguments).await;
VALIDATE_LOW_CONFIDENCE_EDGES => self.handle_validate_low_confidence_edges(arguments).await;
LIST_TYPED_EDGE_RECORDS => self.handle_list_typed_edge_records(arguments).await;
```

**File: `crates/context-graph-mcp/src/tools/definitions/mod.rs`**

Register the new submodule `pub mod typed_edges;` and include its four tool definitions in `ALL_TOOLS` (or however the composite list is constructed — check file).

---

### Phase 5: Integration Tests (NO MOCKS)

**File: NEW `crates/context-graph-storage/tests/typed_edge_export_integration.rs`**

Real RocksDB via `TempDir`. Build 5 synthetic fingerprints, use `EdgeRepository::store_typed_edges_batch` to seed 8 known edges covering every edge type, then call `store_typed_edge_record` with each, then assert:
- `count_typed_edge_records() == 8`.
- `list_typed_edge_record_keys()` returns the 8 keys in iteration order.
- Roundtripping one record via store/get returns byte-identical struct.
- Decoding a record with wrong version byte returns structured error.

**File: NEW `crates/context-graph-storage/tests/llm_validation_integration.rs`**

Same shape for `CF_TYPED_EDGE_VALIDATIONS`. No LLM call is issued; the test writes `LLMEdgeValidation` directly (the LLM is a different module's responsibility — this test only verifies storage).

**File: NEW `crates/context-graph-storage/tests/anomaly_derivation_integration.rs`**

Build 6 known edges: one matching each of the 5 anomaly patterns + one that matches none. Run `derive_anomalies_from_edges`. Assert:
- Summary `edges_scanned == 6`, `pairs_written == 5`.
- `per_kind_counts` has exactly one entry per named kind (except `Other`).
- `CF_CONTRASTIVE_PAIRS` count increased by 5.
- Each written `ContrastivePair` has `generator == "typed_edge_anomaly_derivation_v1"` so the origin is traceable.

**File: extend `crates/context-graph-core/src/training/tests.rs`**

New tests already spec'd above (distribution sum == outgoing edge count; bincode roundtrip).

**File: NEW `crates/context-graph-mcp/src/handlers/tests/typed_edges_tools.rs`**

E2E through the handler dispatch with real store:
- Seed 10 memories, 15 typed edges.
- Invoke `export_typed_edges_corpus` → verify response JSON structure + `count_typed_edge_records() == 15`.
- Invoke `derive_anomalies_from_edges` with synthetic edges engineered to match kinds → verify `CF_CONTRASTIVE_PAIRS` count.
- Invoke `list_typed_edge_records` → verify paginated response.
- `validate_low_confidence_edges` test is LLM-gated: only run under `cfg(feature = "llm")` and mark as `#[ignore]` unless Hermes is up. Non-LLM path: storage-only roundtrip is covered by the dedicated integration test.

---

## Canonical synthetic fixture for F6 manual FSV

The FSV pass (Phase 6, me) uses this exact fixture so the numbers are predictable:

- 6 memories: M1..M6.
- Edges seeded by hand (bypass the NN-Descent scanner, write directly via `store_typed_edges_batch`):

| Source | Target | edge_type | weight | notable embedder_scores |
|---|---|---|---|---|
| M1 | M2 | SemanticSimilar | 0.88 | e1=0.88, e5=0.10, rest~0 |
| M1 | M3 | CodeRelated | 0.80 | e7=0.80, e1=0.05, rest~0 |
| M2 | M3 | CausalChain | 0.65 | e5=0.65, rest~0 |
| M2 | M4 | EntityShared | 0.72 | e11=0.72, e8=0.05, rest~0 |
| M3 | M5 | KeywordOverlap | 0.55 | e6=0.55, e10=0.10, rest~0 |
| M4 | M5 | ParaphraseAligned | 0.75 | e10=0.75, e1=0.20, rest~0 |
| M4 | M6 | GraphConnected | 0.68 | e8=0.68, rest~0 |
| M5 | M6 | MultiAgreement | 0.70 | e1=0.70, e6=0.65, e10=0.68 |

After seeding:
1. **F3 expected**: `TrainingRecord.edge_type_distribution` for M1 = `[1,1,0,0,0,0,0,0]` (one SemSim outgoing + one CodeRelated outgoing).
2. **F1 expected**: `count_typed_edge_records() == 8` after `export_typed_edges_corpus`. One record per edge, with full content and source_metadata if requested.
3. **F2 expected**: `derive_anomalies_from_edges` writes:
   - (M1,M2) → SemanticButNotCausal (e1=0.88 high, e5=0.10 low, gap 0.78)
   - (M1,M3) → CodeShapeButDifferentIntent (e7=0.80 high, e1=0.05 low, gap 0.75)
   - (M2,M4) → EntitySharedButDifferentStructure (e11=0.72 high, e8=0.05 low, gap 0.67)
   - (M3,M5) → KeywordButNotParaphrase (e6=0.55 high, e10=0.10 low, gap 0.45)
   - Total pairs written: **4** (no E9-high pattern in this fixture → no HdcRobustButSemanticDifferent).
4. **F4 expected** (with LLM enabled): calling `validate_low_confidence_edges(weight_ceiling=0.60)` validates M3→M5 (weight=0.55) — the only edge below ceiling. If LLM disabled, tool returns `{ status: "skipped", reason: "llm feature not enabled" }`.

These exact numbers are what FSV will check.

## Error taxonomy (fail-fast guarantees)

| Condition | Behavior |
|---|---|
| Store is not `RocksDbTeleologicalStore` | Handler returns `McpError::ToolError { code: -32001, message: "... requires RocksDbTeleologicalStore" }`. |
| `CF_TYPED_EDGE_RECORDS` key length ≠ 33 during iteration | Return `CoreError::SerializationError` with length + expected. |
| Version byte mismatch on `decode_*` | Return `CoreError::SerializationError` with version info + re-export hint. |
| bincode serialize/deserialize failure | Wrap in `CoreError::SerializationError` with underlying error text. |
| RocksDB op failure | `TeleologicalStoreError::rocksdb_op(op_name, cf_name, key_uuid, e)`. |
| LLM tool call failure | Propagate the graph-agent's error; do NOT silently swallow and retry. |
| `GraphLinkEdgeType::from_u8` returns None (corrupted data) | Treat as hard error, not warn-and-skip. The edge shouldn't exist. |

## Post-implementation verification checklist (me)

1. `env CUDA_HOME=/usr/local/cuda cargo check --workspace` — must pass with zero warnings.
2. `env CUDA_HOME=/usr/local/cuda cargo test --workspace -- --test-threads=4 --nocapture` — all tests green (new + old).
3. Diff every file touched, skim for any fallback/workaround code.
4. `grep -n "TODO\|FIXME\|unwrap_or_default\|unimplemented" <new files>` — zero hits.
5. Dump `TOTAL_COLUMN_FAMILIES` — confirm 59.
6. Start MCP (stdio), send `tools/list`, confirm 4 new tool names present, total = 75.
7. Manual FSV with the synthetic 6-memory fixture above. Check assertions one by one. Store numbers in `tasks/typed_edges_fsv_report.md`.
