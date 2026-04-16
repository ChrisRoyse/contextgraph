# Teleological, Code, and Memory Subsystems

Source files covered:

- `crates/context-graph-core/src/teleological/` (mod.rs, types.rs, embedder.rs, vector.rs, meaning.rs, groups.rs, resolution.rs, profile/, synergy_matrix/, comparator/, matrix_search/, services/)
- `crates/context-graph-storage/src/teleological/` (mod.rs, column_families.rs, schema.rs, serialization.rs, rocksdb_store/, indexes/, quantized/, search/)
- `crates/context-graph-core/src/injection/` (mod.rs, pipeline.rs, budget.rs, candidate.rs, priority.rs, result.rs, formatter.rs, temporal_enrichment.rs)
- `crates/context-graph-core/src/code/mod.rs`
- `crates/context-graph-storage/src/code/` (mod.rs, store.rs, error.rs)
- `crates/context-graph-core/src/memory/` (mod.rs, store.rs, source.rs, capture.rs, manager.rs, session.rs, chunker.rs, watcher.rs, code_capture.rs, code_watcher.rs, ast_chunker.rs)
- `crates/context-graph-core/src/quantization/` (mod.rs, types.rs, fingerprint.rs, accuracy.rs, batch.rs, traits.rs)

---

## 1. What "Teleological" Means in Context Graph

The term **teleological** derives from the Greek *telos* (purpose, goal). In the Context Graph system, the teleological subsystem provides **purpose-oriented memory fusion** across 13 distinct embedding spaces.

The core idea: the 13 embeddings are not just parallel storage channels -- they are 13 distinct **knowledge lenses** that, when combined into a teleological vector, create a representation with exponentially richer meaning than any single embedding. Each embedder captures a different semantic dimension of the same content:

| Embedder | ID | Dimension | Purpose |
|----------|----|-----------|---------|
| E1 Semantic | 0 | 1024D dense | General semantic understanding (e5-large-v2) |
| E2 Temporal Recent | 1 | 512D dense | Recency / temporal decay (sinusoidal PE) |
| E3 Temporal Periodic | 2 | 512D dense | Cyclical / periodic patterns (Fourier) |
| E4 Temporal Positional | 3 | 512D dense | Sequence position (sinusoidal PE) |
| E5 Causal | 4 | 768D dense | Causal reasoning (Longformer SCM, asymmetric) |
| E6 Sparse Lexical | 5 | ~30K sparse | Exact keyword matching (SPLADE) |
| E7 Code | 6 | 1536D dense | Code patterns (Qodo-Embed) |
| E8 Graph | 7 | 1024D dense | Connectivity / graph patterns (asymmetric) |
| E9 HDC | 8 | 1024D dense | Typo-robust / hyperdimensional (projected) |
| E10 Contextual | 9 | 768D dense | Contextual paraphrase (e5-base-v2) |
| E11 Entity | 10 | 768D dense | Entity knowledge (KEPLER) |
| E12 Late Interaction | 11 | 128D/token | Per-token ColBERT (MaxSim) |
| E13 Keyword SPLADE | 12 | ~30K sparse | Keyword expansion (SPLADE v3) |

Key constants defined in `types.rs`:

```rust
pub const EMBEDDING_DIM: usize = 1024;  // Reference embedding dimension
pub const NUM_EMBEDDERS: usize = 13;    // Total embedder count
```

Cross-reference: See `03_embedder_architecture.md` for full embedder details and `06_search_pipeline.md` for how embedders participate in search.

---

## 2. The Embedder Enumeration

Defined in `crates/context-graph-core/src/teleological/embedder.rs`, the `Embedder` enum is the **single source of truth** for all embedder types. Variant ordinal values (0-12) are stable and used for array indexing.

### 2.1 Embedder Enum

| Variant | Index | Display Name | Type | Dimension |
|---------|-------|-------------|------|-----------|
| `Semantic` | 0 | E1_Semantic | Dense | 1024 |
| `TemporalRecent` | 1 | E2_Temporal_Recent | Dense | 512 |
| `TemporalPeriodic` | 2 | E3_Temporal_Periodic | Dense | 512 |
| `TemporalPositional` | 3 | E4_Temporal_Positional | Dense | 512 |
| `Causal` | 4 | E5_Causal | Dense | 768 |
| `Sparse` | 5 | E6_Sparse_Lexical | Sparse | 30522 vocab |
| `Code` | 6 | E7_Code | Dense | 1536 |
| `Graph` | 7 | E8_Graph | Dense | 1024 |
| `Hdc` | 8 | E9_HDC | Dense | 1024 |
| `Contextual` | 9 | E10_Multimodal | Dense | 768 |
| `Entity` | 10 | E11_Entity | Dense | 768 |
| `LateInteraction` | 11 | E12_Late_Interaction | TokenLevel | 128/token |
| `KeywordSplade` | 12 | E13_SPLADE | Sparse | 30522 vocab |

### 2.2 EmbedderDims

Describes the dimensional structure of an embedding:

| Variant | Description |
|---------|-------------|
| `Dense(usize)` | Fixed-length dense float vector |
| `Sparse { vocab_size: usize }` | Sparse vector with vocabulary size |
| `TokenLevel { per_token: usize }` | Variable-length per-token embeddings |

### 2.3 EmbedderMask

A bitmask (`u16`) for selecting subsets of embedders. Bits 0-12 correspond to E1-E13.

Methods: `new()`, `all()`, `from_slice()`, `set()`, `unset()`, `contains()`, `iter()`, `count()`, `is_empty()`, `as_u16()`.

### 2.4 EmbedderGroup

Predefined groups of embedders for common operations:

| Group | Embedders | Purpose |
|-------|-----------|---------|
| `Temporal` | E2, E3, E4 | Time-based queries |
| `Relational` | E4, E5, E11 | Relationship queries |
| `Lexical` | E6, E12, E13 | Keyword / sparse queries |
| `Dense` | All except E6, E12, E13 | Dense vector operations (10 embedders) |
| `Factual` | E1, E12, E13 | Factual knowledge retrieval |
| `Implementation` | E7 | Code-focused operations |
| `All` | E1-E13 | All 13 embedders |

Each embedder also has a configurable temperature for confidence calibration (e.g., Causal=1.2, HDC=1.5, Code=0.9).

---

## 3. Teleological Fusion Types

### 3.1 TopicProfile (`types.rs`)

A 13D vector representing per-embedder alignment scores for topic detection.

| Field | Type | Description |
|-------|------|-------------|
| `alignments` | `[f32; 13]` | Per-embedder alignment scores, each in [0.0, 1.0] |

Methods: `new()`, `uniform()`, `aggregate_alignment()`, `similarity()`, `get()`, `set()`, `active_count()`.

### 3.2 TuckerCore (`types.rs`)

Tucker tensor decomposition for compact representation of the full 13x13x1024 teleological tensor.

| Field | Type | Description |
|-------|------|-------------|
| `ranks` | `(usize, usize, usize)` | Core tensor dimensions. Default: (4, 4, 128) |
| `data` | `Vec<f32>` | Flattened core tensor. Size: rank_1 * rank_2 * rank_3 |
| `u1` | `Vec<f32>` | Factor matrix U1 (13 x rank_1) |
| `u2` | `Vec<f32>` | Factor matrix U2 (13 x rank_2) |
| `u3` | `Vec<f32>` | Factor matrix U3 (1024 x rank_3) |

Original size: 13 x 13 x 1024 = 173,056 floats. Default compressed size yields a significant compression ratio.

### 3.3 ProfileId (`types.rs`)

A newtype wrapper around `String` for task-specific profile identifiers. Panics on empty string (fail-fast). Implements `Display`, `Hash`, `Eq`, `Serialize`, `Deserialize`, and conversions from `&str`/`String`.

---

## 4. TeleologicalVector (`vector.rs`)

The fused multi-embedding representation combining all teleological components.

| Field | Type | Description |
|-------|------|-------------|
| `topic_profile` | `TopicProfile` | 13D embedder weights for topic detection |
| `cross_correlations` | `Vec<f32>` | 78 synergy-weighted pair interactions |
| `group_alignments` | `GroupAlignments` | 6D hierarchical aggregation |
| `tucker_core` | `Option<TuckerCore>` | Optional compressed tensor |
| `profile_id` | `Option<ProfileId>` | Source profile for task-specific weighting |
| `confidence` | `f32` | Reliability of fusion [0.0, 1.0] |

The 78 cross-correlations represent all unique pairs from 13 embedders: C(13,2) = 78. Order: (0,1), (0,2), ..., (0,12), (1,2), ..., (11,12).

**Overall alignment** is computed as a weighted combination:
- 40% topic profile aggregate
- 30% group alignment average
- 20% cross-correlation average
- 10% confidence

**Similarity** between two TeleologicalVectors weights:
- 60% topic profile cosine similarity
- 30% cross-correlation cosine similarity
- 10% group alignment cosine similarity

---

## 5. Group Alignments (`groups.rs`)

Six hierarchical groups that capture different aspects of knowledge:

| Group | Embedders (indices) | Captures |
|-------|---------------------|----------|
| **Factual** | E1(0), E12(11), E13(12) | What something IS |
| **Temporal** | E2(1), E3(2) | When / sequence |
| **Causal** | E4(3), E7(6) | Why / how |
| **Relational** | E5(4), E8(7), E9(8) | Like / where / who |
| **Qualitative** | E10(9), E11(10) | Feel / principle |
| **Implementation** | E6(5) | Code |

`GroupAlignments` struct fields:

| Field | Type | Description |
|-------|------|-------------|
| `factual` | `f32` | Factual group alignment |
| `temporal` | `f32` | Temporal group alignment |
| `causal` | `f32` | Causal group alignment |
| `relational` | `f32` | Relational group alignment |
| `qualitative` | `f32` | Qualitative group alignment |
| `implementation` | `f32` | Implementation group alignment |

Constant: `NUM_GROUPS = 6`.

Each group alignment is a weighted average of its member embedder alignments.

---

## 6. Multi-Resolution Hierarchy (`resolution.rs`)

Provides 4 levels of abstraction for topic representation:

| Level | Dimensions | Name | Description |
|-------|-----------|------|-------------|
| 0 | 13D | Raw | Full per-embedder alignments |
| 1 | 6D | Groups | `GroupAlignments` |
| 2 | 3D | Domains | What / How / Why |
| 3 | 1D | Overall | Single alignment score |

### 6.1 DomainAlignments

Three core knowledge domains at Level 2:

| Domain | Mapping | Description |
|--------|---------|-------------|
| `Factual` (What) | (Factual + Relational) / 2 | What something IS |
| `Procedural` (How) | (Causal + Implementation + Temporal) / 3 | How it works |
| `Affective` (Why) | Qualitative | Why it matters |

### 6.2 MultiResolutionHierarchy

| Field | Type | Description |
|-------|------|-------------|
| `raw` | `[f32; 13]` | Level 0: raw 13D alignments |
| `groups` | `GroupAlignments` | Level 1: 6D group alignments |
| `domains` | `DomainAlignments` | Level 2: 3D domain alignments |
| `overall` | `f32` | Level 3: single score |

Methods: `from_raw()`, `from_raw_weighted()`, `at_level()`, `quick_score()`, `standard_vector()`, `precise_vector()`, `similarity_at_level()`, `recompute()`.

### 6.3 ResolutionView

Returned by `at_level()`:

| Variant | Data |
|---------|------|
| `Raw([f32; 13])` | Level 0 |
| `Groups([f32; 6])` | Level 1 |
| `Domains([f32; 3])` | Level 2 |
| `Overall(f32)` | Level 3 |

---

## 7. Synergy Matrix (`synergy_matrix/`)

A 13x13 matrix capturing cross-embedding synergy strengths.

Key constants:
- `SYNERGY_DIM = 13` (matrix side length)
- `CROSS_CORRELATION_COUNT = 78` (unique pairs: C(13,2))
- `BASE_SYNERGIES` array of predefined synergy values: weak (0.3), moderate (0.6), strong (0.9)

The `SynergyMatrix` struct stores a symmetric 13x13 matrix with diagonal always 1.0 and all values in [0.0, 1.0]. It provides:
- `with_base_synergies()` for predefined values
- `get_synergy(i, j)` / `set_synergy(i, j, value)` for element access
- `indices_to_flat(lo, hi)` for mapping 2D indices to flat cross-correlation arrays
- Analysis and validation methods

---

## 8. Teleological Profiles (`profile/`)

Task-specific configurations for embedding fusion, learned from retrieval feedback.

### 8.1 TaskType (`task_type.rs`)

| Variant | Primary Embedders | Description |
|---------|-------------------|-------------|
| `CodeSearch` | E6, E7 | Code implementation tasks |
| `SemanticSearch` | E1, E5 | General semantic similarity |
| `TemporalSearch` | E2, E3 | Temporal/sequence queries |
| `CausalSearch` | E4, E7 | Causal reasoning |
| `FactualSearch` | E12, E13 | Factual knowledge retrieval |
| `SocialSearch` | E9, E10 | Social/interpersonal context |
| `AbstractSearch` | E11, E5 | Abstract/conceptual queries |
| `General` | E1, E4, E12 | Balanced general-purpose |

Each task type has a suggested `FusionStrategy`.

### 8.2 FusionStrategy (`fusion_strategy.rs`)

| Variant | Cost | Description |
|---------|------|-------------|
| `WeightedAverage` (default) | 2 | Simple weighted average |
| `CrossCorrelation` | 3 | Cross-correlation matrix fusion |
| `TuckerDecomposition { ranks }` | 5 | Tucker tensor decomposition |
| `Attention { heads }` | 4 | Attention-weighted combination |
| `Hierarchical` | 2 | Group-then-domain fusion |
| `PrimaryOnly` | 1 | Primary embeddings only (fast) |

Default Tucker ranks: (4, 4, 128). Default attention heads: 4.

### 8.3 TeleologicalProfile (`teleological.rs`)

| Field | Type | Description |
|-------|------|-------------|
| `id` | `ProfileId` | Unique identifier |
| `name` | `String` | Human-readable name |
| `embedding_weights` | `[f32; 13]` | Per-embedder weights [0.0, 1.0] |
| `fusion_strategy` | `FusionStrategy` | How to combine embeddings |
| `task_type` | `TaskType` | What task this profile is for |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `updated_at` | `DateTime<Utc>` | Last update timestamp |
| `sample_count` | `u64` | Number of learning samples |
| `metrics` | `ProfileMetrics` | Performance metrics |
| `is_system` | `bool` | System-defined vs learned |
| `description` | `Option<String>` | Purpose description |

Pre-defined profiles:
- **code_implementation**: weights=[0.05, 0.02, 0.05, 0.15, 0.08, 0.25, 0.18, 0.05, 0.02, 0.02, 0.05, 0.05, 0.03], Attention fusion
- **conceptual_research**: weights=[0.12, 0.05, 0.03, 0.10, 0.15, 0.03, 0.02, 0.05, 0.05, 0.05, 0.20, 0.12, 0.03], Hierarchical fusion

---

## 9. Meaning Extraction (`meaning.rs`)

Per-embedding semantic extraction follows a 4-step pipeline:

1. **Attention Focusing** -- Top-k dimensions with highest activation
2. **Contrast Enhancement** -- Amplify dimensions deviating from corpus mean
3. **Sparsification** -- Zero out low-signal dimensions
4. **Normalization** -- L2 normalize to unit sphere

### 9.1 MeaningExtractionConfig

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `attention_top_k` | `usize` | 128 | Top dimensions to focus on |
| `contrast_alpha` | `f32` | 0.2 | Contrast enhancement factor |
| `sparsity_threshold` | `f32` | 0.1 | Below this value -> zeroed |
| `min_agreement` | `usize` | 6 | Min embeddings that must agree |
| `disagreement_threshold` | `f32` | 0.3 | Variance threshold for disagreement |
| `normalize` | `bool` | true | Enable L2 normalization |
| `agreement_value_threshold` | `f32` | 0.5 | What counts as "agreeing" |

Preset configurations: `code_search()`, `semantic_search()`, `high_precision()`.

### 9.2 ExtractedMeaning

| Field | Type | Description |
|-------|------|-------------|
| `focus_dimensions` | `Vec<(usize, f32)>` | Top-k (index, value) pairs, sorted desc |
| `enhanced` | `Vec<f32>` | Contrast-enhanced embedding |
| `sparse` | `Vec<f32>` | Sparsified embedding |
| `agreement_dimensions` | `Vec<usize>` | Dimensions where embeddings agree |
| `disagreement_dimensions` | `Vec<usize>` | Dimensions where embeddings disagree |
| `agreement_counts` | `Vec<usize>` | Per-dimension agreement counts |
| `dimension_variances` | `Vec<f32>` | Per-dimension variance |

### 9.3 CrossEmbeddingAnalysis

Performs perspective triangulation: if 3+ embeddings agree on a dimension, it is high-confidence. If exactly one disagrees, that is **nuance** (e.g., emotional nuance when only E10 disagrees).

| Field | Type | Description |
|-------|------|-------------|
| `dimension_agreement` | `Vec<Vec<usize>>` | Per-dimension agreeing embedder indices |
| `triangulated_dimensions` | `Vec<usize>` | 3+ embeddings agree |
| `nuance_dimensions` | `Vec<NuanceDimension>` | Single embedder disagrees |
| `coherence_score` | `f32` | Overall coherence [0, 1] |

### 9.4 FusionMethod

| Variant | Description |
|---------|-------------|
| `WeightedAverage` (default) | Simple weighted average |
| `Attention` | Attention-weighted combination |
| `FocusOnly` | Use only top-k focus dimensions |
| `Hierarchical` | Group-then-domain fusion |
| `ConsensusOnly` | High-agreement dimensions only |

---

## 10. Teleological Storage

### 10.1 Column Families Overview

The storage layer uses RocksDB with **59 column families total** (`TOTAL_COLUMN_FAMILIES` in `storage/column_families.rs:397`) organized into five groups: 11 base + 28 teleological + 13 quantized embedder + 5 code + 2 causal. See `15_verification_report.md#43-column-families-breakdown` for the authoritative breakdown.

#### 10.1.1 Teleological Column Families (28 CFs)

| CF Name | Key Format | Value | Purpose |
|---------|-----------|-------|---------|
| `fingerprints` | UUID (16 bytes) | ~63KB bincode | Primary TeleologicalFingerprint storage |
| `topic_profiles` | UUID (16 bytes) | 52 bytes (13 x f32) | Fast profile-only queries |
| `e13_splade_inverted` | term_id (2 bytes, u16 BE) | Vec\<UUID\> | Stage 1 SPLADE inverted index |
| `e6_sparse_inverted` | term_id (2 bytes, u16 BE) | Vec\<UUID\> | Stage 1 E6 dual sparse recall |
| `e1_matryoshka_128` | UUID (16 bytes) | 512 bytes (128 x f32) | Stage 2 approximate ANN |
| `content` | UUID (16 bytes) | UTF-8 text (LZ4) | Original content text |
| `source_metadata` | UUID (16 bytes) | JSON (~100-500 bytes) | Provenance info |
| `file_index` | file_path (UTF-8) | bincode (Vec\<Uuid\> + meta) | File path secondary index |
| `topic_portfolio` | session_id (UTF-8) | JSON (~1-50KB) | Persisted topic portfolio |
| `e12_late_interaction` | UUID (16 bytes) | bincode (Vec\<Vec\<f32\>\>) | ColBERT token embeddings (10-256KB) |
| `entity_provenance` | canonical_id + UUID | bincode (~200-2000 bytes) | Entity extraction provenance (reserved) |
| `audit_log` | timestamp_be + UUID (24 bytes) | JSON (~200-2000 bytes) | Append-only audit log |
| `audit_by_target` | UUID + timestamp_be (24 bytes) | 24 bytes (primary key) | Audit secondary index by target |
| `merge_history` | UUID + timestamp_be (24 bytes) | JSON (~500-5000 bytes) | Permanent merge lineage |
| `importance_history` | UUID + timestamp_be (24 bytes) | JSON (~100-500 bytes) | Permanent importance changes |
| `tool_call_index` | tool_use_id (UTF-8) | bincode Vec\<Uuid\> | Tool call -> memory mapping (reserved) |
| `consolidation_recommendations` | UUID (16 bytes) | bincode (~500-5000 bytes) | Consolidation suggestions (reserved) |
| `embedding_registry` | UUID (16 bytes) | JSON (~200-500 bytes) | Embedder version tracking |
| `custom_weight_profiles` | profile_name (UTF-8) | JSON [f32; 13] (~100-200 bytes) | User weight profiles |
| `hnsw_graphs` | "graph:{embedder}" / "meta:{embedder}" | Binary (1-100MB) / JSON | Persisted HNSW indexes |
| `training_records` | UUID (16 bytes) | version byte + bincode `TrainingRecord` (MB-scale when full fingerprint included) | Exported training corpus (`export_training_corpus` MCP tool; see `CF_TRAINING_RECORDS` in `teleological/column_families.rs:399`) |
| `constellations` | UUID (16 bytes) | version byte + bincode `Constellation` | Compiled centroid+spread summaries per memory group (`compile_constellation` MCP tool; see `CF_CONSTELLATIONS` in `teleological/column_families.rs:418`) |
| `constellation_by_selector` | selector-hash (UTF-8) | UUID (16 bytes) | Secondary index: O(1) selector -> constellation lookup (`CF_CONSTELLATION_BY_SELECTOR` at `teleological/column_families.rs:436`) |
| `contrastive_pairs` | UUID (16 bytes) | bincode `ContrastivePair` | Mined positive/negative training pairs for contrastive learning (`CF_CONTRASTIVE_PAIRS` at `teleological/column_families.rs:453`) |
| `contrastive_by_kind` | kind-tag + pair UUID | pair UUID | Secondary index by pair kind (`CF_CONTRASTIVE_BY_KIND` at `teleological/column_families.rs:466`) |
| `contrastive_by_anchor` | anchor UUID + pair UUID | pair UUID | Secondary index by anchor memory (`CF_CONTRASTIVE_BY_ANCHOR` at `teleological/column_families.rs:479`) |

`TELEOLOGICAL_CF_COUNT = 28` at `teleological/column_families.rs:551` now matches the `TELEOLOGICAL_CFS` array. The 8-CF growth (20 -> 28) is delivered across commits `aec85437` (training_records), `7179bd7c` (constellations, constellation_by_selector), `48f0420d` (contrastive_pairs, contrastive_by_kind, contrastive_by_anchor), and `61934385` (typed_edge_records, typed_edge_validations). The authoritative total CF count in `storage/column_families.rs:397` is 59.

#### 10.1.2 Quantized Embedder Column Families (13 CFs)

One CF per embedder for per-embedder quantized storage:

| CF Name | Embedder | Quantization | Approximate Value Size |
|---------|----------|-------------|----------------------|
| `emb_0` | E1 Semantic | PQ-8 | ~8 bytes |
| `emb_1` | E2 Temporal Recent | Float8 | ~512 bytes |
| `emb_2` | E3 Temporal Periodic | Float8 | ~512 bytes |
| `emb_3` | E4 Temporal Positional | Float8 | ~512 bytes |
| `emb_4` | E5 Causal | PQ-8 | ~8 bytes |
| `emb_5` | E6 Sparse | SparseNative | ~2KB |
| `emb_6` | E7 Code | PQ-8 | ~8 bytes |
| `emb_7` | E8 Graph | Float8 | ~1024 bytes |
| `emb_8` | E9 HDC | Binary | ~1250 bytes |
| `emb_9` | E10 Contextual | PQ-8 | ~8 bytes |
| `emb_10` | E11 Entity | Float8 | ~768 bytes |
| `emb_11` | E12 Late Interaction | TokenPruning | ~2KB |
| `emb_12` | E13 SPLADE | SparseNative | ~2KB |

Key format for all: UUID (16 bytes).

#### 10.1.3 Code Column Families (5 CFs)

| CF Name | Purpose |
|---------|---------|
| `code_entities` | CodeEntity storage |
| `code_e7_embeddings` | E7 1536D embeddings |
| `code_file_index` | File path -> entity IDs |
| `code_name_index` | Entity name -> entity IDs |
| `code_signature_index` | Signature hash -> entity IDs |

#### 10.1.4 Causal Column Families (2 CFs)

| CF Name | Key Format | Purpose |
|---------|-----------|---------|
| `causal_relationships` | UUID (16 bytes) | LLM-generated causal relationships |
| `causal_by_source` | source UUID (16 bytes) | Secondary index by source fingerprint |

### 10.2 Key Schema (`schema.rs`)

All keys use fixed-size formats for efficient range scans. Key functions follow a fail-fast policy -- parsing functions panic on invalid input to ensure data corruption is immediately detected.

| Function | Input | Output | Size |
|----------|-------|--------|------|
| `fingerprint_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |
| `topic_profile_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |
| `e13_splade_inverted_key()` | `u16` | `[u8; 2]` | 2 bytes |
| `e6_sparse_inverted_key()` | `u16` | `[u8; 2]` | 2 bytes |
| `e1_matryoshka_128_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |
| `content_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |
| `e12_late_interaction_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |
| `session_temporal_key()` | `i64` | `Vec<u8>` | 10 bytes ("t:" + 8-byte BE) |
| `causal_relationship_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |
| `causal_by_source_key()` | `&Uuid` | `[u8; 16]` | 16 bytes |

### 10.3 Serialization (`serialization.rs`)

| Function | Format | Typical Size |
|----------|--------|-------------|
| `serialize_teleological_fingerprint()` | Version byte + bincode | 5-150KB (typically ~63KB) |
| `serialize_topic_profile()` | 13 x f32 LE | Exactly 52 bytes |
| `serialize_e1_matryoshka_128()` | 128 x f32 LE | Exactly 512 bytes |
| `serialize_memory_id_list()` | u32 count + N x 16-byte UUIDs | 4 + N*16 bytes |

Serialization version: `TELEOLOGICAL_VERSION = 1`. Version mismatches return errors (no automatic migration).

Important constraint: **bincode + `skip_serializing_if` = silent data corruption**. Bincode is positional and does not encode field names, so optional fields must use JSON column families instead.

Deserialization performs legacy migration for asymmetric embedders:
- `migrate_legacy_e5()` -- converts single-vector E5 to dual cause/effect format
- `migrate_legacy_e8()` -- converts single-vector E8 to dual source/target format

### 10.4 RocksDbTeleologicalStore (`rocksdb_store/`)

The main persistent store implementation with 59 column families (28 teleological + 13 quantized + 5 code + 2 causal + 11 base). It uses:

- **HNSW indexes** protected by `RwLock` for concurrent query access
- **SPLADE inverted indexes** for Stage 1 recall
- **WriteBatch** for atomic multi-CF writes

The store follows a **fail-fast** policy: every RocksDB operation that fails returns a detailed error including the operation, column family, key, and underlying RocksDB error.

Operations are organized into modules:
- `crud` -- create, read, update, delete
- `search` -- search operation implementations
- `index_ops` -- HNSW index add/remove
- `inverted_index` -- SPLADE inverted index operations
- `persistence` -- batch operations, statistics, persistence
- `content` -- content storage
- `source_metadata` -- provenance metadata
- `audit_log` -- append-only audit logging
- `causal_relationships` -- LLM-generated causal links
- `file_index` -- file path indexing
- `fusion` -- weighted RRF fusion with scores

### 10.5 Index Configuration (`indexes/`)

The indexes module provides HNSW and inverted index configurations per embedder:

- `HnswConfig` -- per-embedder HNSW parameters (M, ef_construction, dimension, metric)
- `InvertedIndexConfig` -- for E6 and E13 sparse embedders
- `DistanceMetric` enum -- `Cosine`, `InnerProduct`, `L2`
- Dimension constants re-exported for all 13 embedders

---

## 11. Teleological Data and Semantic Fingerprints

The relationship between teleological data and regular semantic fingerprints:

1. **SemanticFingerprint** (defined in `crates/context-graph-core/src/types/fingerprint.rs`) stores the raw 13-embedder vectors for a single memory. This is the atomic storage unit (ARCH-01). Typical size: ~46KB.

2. **TeleologicalFingerprint** (`core/types/fingerprint/teleological/types.rs:32`) wraps a SemanticFingerprint with metadata. Fields: `id`, `semantic`, `content_hash`, `created_at`, `last_updated`, `access_count`, `importance`, `last_accessed_at`, `e6_sparse`. This is what gets serialized to the `fingerprints` CF (~63KB).
   - `last_accessed_at` (line 66) is marked `#[serde(skip, default = "default_last_accessed_at")]` because the struct is bincode-serialized (positional format); adding a field to the bincode layout would break existing data. The field is runtime-only and resets to `Utc::now()` on deserialization. It is mutated by `record_access()` (see `core/types/fingerprint/teleological/core.rs:140-145`) which also bumps `access_count` and `last_updated`.
   - The training module reads `last_accessed_at` from the in-memory `TeleologicalFingerprint` when available, but because the field does not round-trip through bincode the value observed in exported `TrainingRecord`s after a restart is effectively the deserialization default (now). Phase-5 recency labels instead come from `extract_temporal_labels(fp.created_at, ...)` in `core/src/training/temporal.rs`, producing `e2_recency_norm` from `created_at`.
   - `teleological_vector()` (method at line 105) returns a `[f32; TELEOLOGICAL_VECTOR_DIM]` (13D) of per-embedder L2 norms for fast first-pass candidate filtering. `TELEOLOGICAL_VECTOR_DIM = 13` is defined at line 12.

3. **TeleologicalVector** is a *derived* fusion of the 13 embeddings (13D topic profile + 78 cross-correlations + 6D group alignments), produced by applying a TeleologicalProfile to a SemanticFingerprint. Distinct from the 13D signature vector above.

4. **TopicProfile** (13D) is extracted from the SemanticFingerprint and stored separately in `topic_profiles` for fast queries without deserializing the full fingerprint.

5. **QuantizedSemanticFingerprint** compresses the ~46KB SemanticFingerprint to ~11KB using mixed quantization (see Section 14).

### 11.1 Recently Added Search Options

The `SearchOptions` struct (`core/traits/teleological_memory_store/options.rs`) has gained three additional fields not present in the initial spec generation:

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `synergy_weights` | `Option<[f32; 13]>` | `None` | Per-embedder synergy multipliers for fusion bonus (used by `rocksdb_store/fusion.rs:194`) |
| `use_quantized_prefilter` | `bool` | `false` | Enable quantized-first prefilter stage |
| `enable_teleological_prefilter` | `bool` | `false` | Enable 13D `teleological_vector()` prefilter stage |

Builder methods: `with_synergy_weights([f32; 13])`, `with_use_quantized_prefilter(bool)`, `with_enable_teleological_prefilter(bool)`.

---

## 12. Injection Subsystem (`crates/context-graph-core/src/injection/`)

The injection subsystem orchestrates context injection from memory retrieval to formatted output for Claude Code hooks.

### 12.1 Pipeline Architecture

```
InjectionPipeline
  |-- SimilarityRetriever   (find similar memories, detect divergence)
  |-- MemoryStore            (lookup memory content by ID)
  |-- PriorityRanker         (compute recency/diversity factors)
  |-- TokenBudgetManager     (select within budget)
  `-- ContextFormatter       (format for injection)
```

Pipeline steps:
1. Retrieve similar memories (default limit: 50)
2. Check for divergence (SEMANTIC embedders only, per AP-62)
3. Convert to candidates with content lookup
4. Rank candidates by priority
5. Select within token budget
6. Format as markdown

### 12.2 InjectionCategory

| Category | Priority | Token Budget | Can Overflow? |
|----------|----------|-------------|---------------|
| `DivergenceAlert` | 1 (highest) | 200 tokens | Yes |
| `HighRelevanceCluster` | 2 | 400 tokens | Yes |
| `SingleSpaceMatch` | 3 | 300 tokens | No |
| `RecentSession` | 4 (lowest) | 200 tokens | No |

Categorization thresholds (from constitution.yaml):
- weighted_agreement >= 2.5: `HighRelevanceCluster`
- weighted_agreement >= 1.0: `SingleSpaceMatch`
- weighted_agreement < 1.0: Below injection threshold

### 12.3 InjectionCandidate

| Field | Type | Description |
|-------|------|-------------|
| `memory_id` | `Uuid` | Memory identifier |
| `content` | `String` | Memory content text |
| `relevance_score` | `f32` | Base similarity [0.0, 1.0] |
| `recency_factor` | `f32` | Time-based multiplier [0.8, 1.3] |
| `diversity_bonus` | `f32` | Multi-space agreement bonus [0.8, 1.5] |
| `weighted_agreement` | `f32` | Cross-space agreement [0.0, 8.5] |
| `matching_spaces` | `Vec<Embedder>` | Embedders that exceeded threshold |
| `priority` | `f32` | Final score = relevance x recency x diversity |
| `token_count` | `u32` | Estimated tokens (words x 1.3) |
| `category` | `InjectionCategory` | Budget pool and sort order |
| `created_at` | `DateTime<Utc>` | Memory creation time |

Sorting: by category first (lower priority number first), then by priority descending within the same category.

### 12.4 TokenBudget

| Field | Type | Default |
|-------|------|---------|
| `total` | `u32` | 1200 |
| `divergence_budget` | `u32` | 200 |
| `cluster_budget` | `u32` | 400 |
| `single_space_budget` | `u32` | 300 |
| `session_budget` | `u32` | 200 |
| `reserved` | `u32` | 100 |

Constants: `MIN_BUDGET = 100`, `BRIEF_BUDGET = 200` (for PreToolUse hook).

The `TokenBudgetManager` enforces:
- Total budget is a hard limit (never exceeded)
- High-priority categories (priority <= 2) can overflow into reserved budget
- Low-priority categories (priority >= 3) cannot exceed their allocated budget

### 12.5 InjectionResult

Contains formatted context, included memory IDs, divergence alerts, token count, and categories included.

### 12.6 TemporalEnrichment

Adds temporal badges to injection results indicating temporal relationships (same session, same day, same period, same sequence).

---

## 13. Code Subsystem

### 13.1 Core: Code Query Type Detection (`crates/context-graph-core/src/code/mod.rs`)

Detects whether a query is actual code syntax (Code2Code), natural language about code (Text2Code), or non-code (NonCode) for optimized E7 Code embedder similarity computation.

| QueryType | Description | Similarity Adjustment |
|-----------|-------------|----------------------|
| `Code2Code` | Actual code syntax (e.g., `fn process_batch<T>`) | Sharpens S-curve: >0.75 boosted 1.4x, <0.75 reduced 0.9x |
| `Text2Code` | Natural language about code (e.g., "batch processing function") | Standard similarity (no modification) |
| `NonCode` | Not code-related | Reduced to 50% (0.5x) |

Detection uses language-specific pattern matching for Rust, Python, JavaScript/TypeScript, Go, and C/C++, plus general code syntax indicators and natural language programming terms.

### 13.2 Storage: CodeStore (`crates/context-graph-storage/src/code/store.rs`)

Separate storage for code entities with full SemanticFingerprint (all 13 embeddings). Uses 5 dedicated column families (see Section 10.1.3).

| Operation | Description |
|-----------|-------------|
| `store_entity()` | Store a code entity with full SemanticFingerprint |
| `get_entity()` | Retrieve by UUID |
| `delete_entity()` | Remove entity and all index entries |
| `search_by_name()` | Search via name index |
| `search_by_signature()` | Search via signature hash index |
| `get_entities_for_file()` | All entities from a file path |
| `delete_entities_for_file()` | Cleanup all entities for a file |
| `get_stats()` | Database statistics |

Key architectural point: code entities are stored in a **separate database** from teleological memories, but each entity receives the full 13-embedder treatment (ARCH-01). E7 (Qodo-Embed, 1536D) provides code-specific patterns, while E1 provides semantic meaning, E5 provides causal understanding, etc.

Serialization policy: JSON for CodeEntity metadata and indexes, bincode for SemanticFingerprint dense vectors.

---

## 14. Quantization for Teleological Data (`crates/context-graph-core/src/quantization/`)

### 14.1 Precision Levels (`types.rs`)

| Precision | Bits | Compression | Typical RMSE | Range |
|-----------|------|-------------|-------------|-------|
| `Int4` | 4 | ~8x | < 5% | [-8, 7] |
| `Int8` | 8 | ~4x | < 1% | [-128, 127] |
| `Fp16` | 16 | ~2x | < 0.01% | IEEE 754 binary16 |
| `Fp32` | 32 | 1x | 0% | IEEE 754 binary32 |

### 14.2 QuantizedEmbedding (`types.rs`)

| Field | Type | Description |
|-------|------|-------------|
| `data` | `Vec<u8>` | Quantized data bytes |
| `scale` | `f32` | Scale factor for dequantization |
| `zero_point` | `i32` | Zero point offset |
| `precision` | `Precision` | Quantization precision |
| `original_dim` | `usize` | Original vector dimension |
| `embedding_type` | `String` | Type identifier |
| `token_count` | `Option<usize>` | Token count (for E12) |

Dequantization formula: `original = (quantized - zero_point) * scale`

### 14.3 SemanticFingerprint Quantization (`fingerprint.rs`)

Compresses SemanticFingerprint from ~46KB to ~11KB using mixed quantization:

| Method | Embedders | Description | Compression |
|--------|-----------|-------------|-------------|
| **PQ-8** | E1, E5, E7, E10 | Product quantization, 8-bit codes | ~128x |
| **Float8** | E2-E4, E8, E9, E11 | Min-max scalar quantization (u8 [0,255]) | 4x |
| **TokenPruning** | E12 | Per-token Float8 | 4x |
| **SparseNative** | E6, E13 | u16 indices + Float8 values | ~2.4x |

`QuantizedFloat8` struct:

| Field | Type | Description |
|-------|------|-------------|
| `data` | `Vec<u8>` | Quantized values [0, 255] |
| `min_val` | `f32` | Original minimum (for dequant) |
| `max_val` | `f32` | Original maximum (for dequant) |
| `original_dim` | `usize` | Original dimension |

Float8 formula:
- Quantize: `q = round((v - min) / (max - min) * 255)`
- Dequantize: `v = (q / 255) * (max - min) + min`

### 14.4 Accuracy Verification

`accuracy.rs` provides `compute_rmse()` and `compute_nrmse()` for measuring reconstruction accuracy. An `AccuracyReport` tracks per-embedder reconstruction quality.

### 14.5 Batch Operations

`batch.rs` provides `batch_quantize()` and `batch_dequantize()` for processing multiple embeddings efficiently.

---

## 15. Memory Management (`crates/context-graph-core/src/memory/`)

### 15.1 Memory Struct (`mod.rs`)

The primary data unit for captured memories:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Unique identifier (UUID v4) |
| `content` | `String` | Knowledge text (max 10,000 chars) |
| `source` | `MemorySource` | Discriminated origin type |
| `created_at` | `DateTime<Utc>` | Creation timestamp |
| `session_id` | `String` | Session this memory belongs to |
| `teleological_array` | `TeleologicalArray` | Full 13-embedding array (atomic, ~46KB) |
| `chunk_metadata` | `Option<ChunkMetadata>` | Chunk info for MDFileChunk sources |
| `word_count` | `u32` | Content word count |

Constant: `MAX_CONTENT_LENGTH = 10_000` characters.

Validation checks:
- Content is not empty and <= 10,000 characters
- Session ID is not empty
- TeleologicalArray passes strict validation (all 13 embeddings valid)
- MDFileChunk sources require chunk_metadata
- Word count matches actual content

### 15.2 MemorySource (`source.rs`)

Four discriminated source types:

| Variant | Fields | Description |
|---------|--------|-------------|
| `HookDescription` | `hook_type: HookType`, `tool_name: Option<String>` | From Claude Code hook events |
| `ClaudeResponse` | `response_type: ResponseType` | From session end/stop responses |
| `MDFileChunk` | `file_path: String`, `chunk_index: u32`, `total_chunks: u32` | From markdown file watcher |
| `CausalExplanation` | `source_fingerprint_id: Uuid`, `causal_relationship_id: Uuid`, `mechanism_type: String`, `confidence: f32` | LLM-generated causal knowledge |

### 15.3 HookType

| Variant | Timeout | Description |
|---------|---------|-------------|
| `SessionStart` | 5000ms | Session initialization |
| `UserPromptSubmit` | 2000ms | User sends a prompt |
| `PreToolUse` | 500ms | Before Edit/Write/Bash |
| `PostToolUse` | 3000ms (async) | After any tool execution |
| `Stop` | 3000ms | Claude stops responding |
| `SessionEnd` | 30000ms | Session cleanup |

### 15.4 ResponseType

| Variant | Description |
|---------|-------------|
| `SessionSummary` | Summary captured at session end |
| `StopResponse` | Response at Stop hook |
| `SignificantResponse` | Significant response worth persisting |

### 15.5 ChunkMetadata

| Field | Type | Description |
|-------|------|-------------|
| `file_path` | `String` | Source file path |
| `chunk_index` | `u32` | Zero-based chunk index |
| `total_chunks` | `u32` | Total chunks from file |
| `word_offset` | `u32` | Word offset from file start |
| `char_offset` | `u32` | Character offset from file start |
| `original_file_hash` | `String` | SHA256 of original file |
| `start_line` | `u32` | Starting line (1-based) |
| `end_line` | `u32` | Ending line (1-based, inclusive) |

### 15.6 MemoryStore (`store.rs`)

RocksDB-backed CRUD storage for Memory structs with session-based indexing.

Column families:
- `memories` -- primary storage (key: UUID 16 bytes, value: bincode Memory)
- `session_index` -- secondary index (key: session_id bytes, value: bincode Vec\<Uuid\>)
- `file_index` -- file path secondary index

Operations:
- `store()` -- persist a memory (writes to primary + secondary indexes)
- `get()` -- retrieve by UUID
- `delete()` -- remove by UUID (with index cleanup)
- `list_by_session()` -- retrieve all memories for a session
- `list_all()` -- iterate all memories
- `count()` -- total memory count

Thread safety: uses `Arc<DB>` for RocksDB sharing and `parking_lot::Mutex` for protecting read-modify-write cycles on secondary indexes (prevents lost-update pattern on concurrent writes).

### 15.7 Additional Memory Subsystems

| Module | Purpose |
|--------|---------|
| `capture.rs` | `MemoryCaptureService` -- orchestrates embedding + storage |
| `manager.rs` | `SessionManager` -- session lifecycle (start, end, active tracking) |
| `session.rs` | `Session` struct with `SessionStatus` |
| `chunker.rs` | `TextChunker` -- splits content into overlapping chunks |
| `ast_chunker.rs` | `AstCodeChunker` -- AST-based code chunking by entity type |
| `watcher.rs` | `GitFileWatcher` -- monitors file changes for re-embedding |
| `code_watcher.rs` | `CodeFileWatcher` -- code-specific file monitoring |
| `code_capture.rs` | `CodeCaptureService` -- end-to-end code entity processing |

---

## 15A. Training Data Export (`crates/context-graph-core/src/training/`)

Added in commits `aec85437` and `7179bd7c`. The training module packages a memory's full multi-dimensional label space into a bincode-serializable `TrainingRecord` (v1) written to CF `training_records`.

### 15A.1 Module Layout

| File | Lines | Purpose |
|------|------:|---------|
| `training/mod.rs` | 204 | `TrainingRecord`, `TrainingEdge`, `CausalLabel`, `TopicMembership`, `TRAINING_RECORD_VERSION = 1`, `NUM_CROSS_CORRELATIONS = 78` |
| `training/computation.rs` | 145 | `compute_cross_correlations`, `compute_group_alignments`, `topic_profile_or_fallback` |
| `training/temporal.rs` | 150 | `extract_temporal_labels`, `PeriodicBucket`, `TemporalLabels` (e2_recency_norm, hour/day/month buckets, session position) |
| `training/tests.rs` | 443 | 18 unit tests covering record construction, temporal buckets, fallback behavior, and legacy-fingerprint compatibility |

### 15A.2 Record Contents

Each `TrainingRecord` bundles: identity and provenance (UUID, content, importance, timestamps, source); all 13 embedding vectors (dense, sparse, token-level); a 13D `topic_profile` loaded from `CF_TOPIC_PROFILES` (with per-embedder vector-presence fallback); 78 synergy-weighted cross-correlations; 6D group alignments; outgoing and incoming typed edges with per-embedder scores; K-NN neighbors per embedder; LLM-discovered causal relationships (both directions); topic memberships; `TemporalLabels`; and an optional Phase-4 Tucker core.

### 15A.3 MCP Tools (4)

Wired in `mcp/src/handlers/tools/training_tools.rs` (932 lines) and dispatched at `mcp/src/handlers/tools/dispatch.rs:167-171`:

| Tool | Purpose |
|------|---------|
| `export_training_corpus` | Iterate fingerprints via `list_fingerprints_unbiased`, build records, persist to `CF_TRAINING_RECORDS` |
| `list_training_records` | List UUIDs stored in the CF with basic shape stats |
| `get_training_record` | Retrieve a record by memory UUID |
| `count_training_records` | Count rows in the CF |

Source-of-truth integration tests: 15 `#[tokio::test(flavor = "multi_thread")]` tests in `crates/context-graph-storage/tests/training_records_integration.rs` exercising round-trip, listing, deletion, and clear semantics.

---

## 15B. Constellation Compiler (`crates/context-graph-core/src/constellation/`)

Added in commits `7179bd7c` and `48f0420d`. A **constellation** is a compiled centroid+spread summary of a set of memories selected by topic, session, tag, time range, or explicit id list. Constellations let downstream classifiers answer "does this new memory belong to the same cluster?" without re-running the full retrieval pipeline.

### 15B.1 Module Layout

| File | Lines | Purpose |
|------|------:|---------|
| `constellation/mod.rs` | 61 | `CONSTELLATION_VERSION = 1`, `RESERVOIR_SAMPLE_SIZE = 1024`, `MIN_CONSTELLATION_MEMBERS = 3`, public re-exports |
| `constellation/types.rs` | 218 | `Constellation`, `ConstellationSelector`, `EmbedderStats`, `VectorKind`, `ConstellationScoringResult`, centroid-dim constants |
| `constellation/welford.rs` | 257 | `WelfordStats`, `WelfordVector` — Welford's online mean + variance (numerically stable, O(1) per update) |
| `constellation/reservoir.rs` | 214 | `ReservoirSample` — bounded-memory percentile estimator (uniform random sample, capacity = `RESERVOIR_SAMPLE_SIZE`) |
| `constellation/compiler.rs` | 849 | `ConstellationAccumulator`, `compile_constellation`, `score_memory_against_constellation`, `ConstellationError`, `DEFAULT_MAX_MEMBERS` |
| `constellation/tests.rs` | 346 | 9 compiler tests (reservoir + welford have 4 + 6 tests in their own files, 19 total across the module) |

### 15B.2 Relationship to Clustering and Topics

Constellations are **related but distinct** from topics (see `12_clustering_and_topics.md`):

| Aspect | Topics (clustering subsystem) | Constellations (training subsystem) |
|--------|-------------------------------|-------------------------------------|
| Discovery | Emergent, discovered by cross-space weighted agreement (>= 2.5) | Caller-selected, compiled on demand |
| Storage | `topic_portfolio` CF (per session) and `TopicPortfolio` (live runtime) | `constellations` CF (primary), `constellation_by_selector` CF (secondary) |
| Scope | Spans all 13 embedding spaces, filtered by ARCH-04 / AP-60 (temporal embedders excluded) | Per-embedder `EmbedderStats` (centroid, variance, percentiles) plus aggregate topic-profile, group, and cross-correlation centroids |
| Evolution | Tracked by `TopicStabilityTracker` (churn, entropy, snapshots) | Immutable once compiled; re-compile to refresh |
| Purpose | Online topic detection and divergence alerts | Training-time "does this belong?" scoring |

### 15B.3 MCP Tools (5)

Wired in `mcp/src/handlers/tools/constellation_tools.rs` and dispatched at `mcp/src/handlers/tools/dispatch.rs:172-177`:

`compile_constellation`, `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation`.

---

## 15C. Contrastive Pair Mining (Phase 3)

Added in commit `48f0420d`. Mines positive/negative pairs for contrastive learning over the stored training corpus. Storage lives in three CFs: `contrastive_pairs` (primary, bincode `ContrastivePair`), `contrastive_by_kind` (secondary index by pair kind), and `contrastive_by_anchor` (secondary index by anchor memory).

### 15C.1 MCP Tools (4)

Wired in `mcp/src/handlers/tools/contrastive_tools.rs` and dispatched at `mcp/src/handlers/tools/dispatch.rs:178-182`:

`mine_contrastive_pairs`, `list_contrastive_pairs`, `get_contrastive_pair`, `count_contrastive_pairs`.

---

## 16. Integration with the Main Search Pipeline

The teleological subsystem integrates with search at multiple levels:

1. **Storage writes**: When a memory is stored, its TeleologicalFingerprint is written to the `fingerprints` CF, its topic profile to `topic_profiles`, E1 Matryoshka 128D to `e1_matryoshka_128`, E12 tokens to `e12_late_interaction`, and sparse term entries to `e13_splade_inverted` and `e6_sparse_inverted`. Per-embedder quantized data goes to `emb_0` through `emb_12`.

2. **HNSW indexing**: Dense embedders are indexed via HNSW for approximate nearest neighbor search. The `indexes/` module provides per-embedder HNSW configurations. Persisted HNSW graphs are stored in the `hnsw_graphs` CF.

3. **5-stage retrieval pipeline** (see `06_search_pipeline.md`):
   - **Stage 1 (Recall)**: E13 SPLADE + E6 sparse inverted index
   - **Stage 2 (Semantic ANN)**: E1 Matryoshka 128D HNSW
   - **Stage 3 (Full E1)**: Full 1024D E1 cosine similarity
   - **Stage 4 (Multi-space fusion)**: All dense embedders via weighted RRF
   - **Stage 5 (MaxSim rerank)**: E12 ColBERT late interaction

4. **Profile-guided search**: `TeleologicalProfile` embedding weights determine how each embedder contributes to the final relevance score. The `TaskType` routes queries to appropriate fusion strategies.

5. **Context injection**: The injection pipeline uses search results to generate formatted context for Claude Code hooks, applying priority ranking, token budgeting, and divergence detection.

6. **Topic clustering**: TopicProfile (13D) vectors enable multi-space topic detection with a weighted agreement threshold of >= 2.5 (constitution ARCH-09). Temporal embedders (E2-E4) are excluded from topic detection (AP-60).

Cross-reference: See `06_search_pipeline.md` for the full retrieval pipeline and `08_mcp_tools.md` for the MCP tool interface.
