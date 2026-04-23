# 13. Teleological Subsystem

## Source Files Covered

| Crate | File | Role |
|---|---|---|
| `context-graph-core` | `src/teleological/mod.rs` | Module root, re-exports |
| `context-graph-core` | `src/teleological/types.rs` | `ProfileId`, `TuckerCore`, `TopicProfile`, `NUM_EMBEDDERS`, `EMBEDDING_DIM` |
| `context-graph-core` | `src/teleological/embedder.rs` | `Embedder` enum, `EmbedderDims`, `EmbedderGroup`, `EmbedderMask` |
| `context-graph-core` | `src/teleological/groups.rs` | `GroupAlignments`, `GroupType`, `group_indices`, `weighted_group_average` |
| `context-graph-core` | `src/teleological/synergy_matrix/` | `SynergyMatrix`, `BASE_SYNERGIES`, `SYNERGY_DIM`, `CROSS_CORRELATION_COUNT` |
| `context-graph-core` | `src/teleological/vector.rs` | `TeleologicalVector` (fused representation) |
| `context-graph-core` | `src/teleological/resolution.rs` | `MultiResolutionHierarchy`, `DomainAlignments`, `DomainType`, `ResolutionView` |
| `context-graph-core` | `src/teleological/meaning.rs` | `MeaningExtractionConfig`, `ExtractedMeaning`, `FusionMethod` |
| `context-graph-core` | `src/teleological/profile/` | `TeleologicalProfile`, `TaskType`, `FusionStrategy`, `ProfileMetrics` |
| `context-graph-core` | `src/teleological/matrix_search/` | `TeleologicalMatrixSearch`, `SearchStrategy`, `SimilarityBreakdown` |
| `context-graph-core` | `src/teleological/comparator/` | `TeleologicalComparator`, `ComparisonResult`, `BatchComparator` |
| `context-graph-core` | `src/teleological/tucker.rs` | `CpuTuckerCompressor`, streaming HOSVD |
| `context-graph-core` | `src/teleological/comparison_error.rs` | `ComparisonValidationError`, `WeightValues` |
| `context-graph-core` | `src/types/fingerprint/semantic/fingerprint.rs` | `SemanticFingerprint`, `EmbeddingRef`, `ValidationError` |
| `context-graph-core` | `src/types/fingerprint/teleological/types.rs` | `TeleologicalFingerprint`, `TELEOLOGICAL_VECTOR_DIM` |
| `context-graph-core` | `src/training/computation.rs` | `topic_profile_or_fallback`, `compute_cross_correlations`, `compute_group_alignments` |
| `context-graph-storage` | `src/teleological/column_families.rs` | Teleological CF constants, `TELEOLOGICAL_CFS`, `TELEOLOGICAL_CF_COUNT` |
| `context-graph-storage` | `src/teleological/rocksdb_store/crud.rs` | `store_async`, `retrieve_async`, `update_async`, `delete_async` |
| `context-graph-storage` | `src/teleological/rocksdb_store/trait_impl.rs` | `TeleologicalMemoryStore` trait dispatch |
| `context-graph-storage` | `src/teleological/serialization.rs` | bincode encode/decode for `TeleologicalFingerprint` |

---

## 1. What "Teleological" Means Here

"Teleological" (Greek *telos* — purpose) denotes **purpose-oriented memory fusion** across 13 distinct embedder views of the same content. The 13 embeddings are not parallel storage — they are 13 knowledge lenses that together yield a representation richer than any single embedding. A single memory's full representation is the [`TeleologicalFingerprint`](#3-teleologicalfingerprint), which bundles the 13-view [`SemanticFingerprint`](#2-semanticfingerprint) with provenance metadata.

Derived cross-features:

| Feature | Shape | Formula | Source |
|---|---|---|---|
| `topic_profile` | `[f32; 13]` | Per-embedder presence/alignment, one slot per embedder | `training/computation.rs:41` |
| `cross_correlations` | `Vec<f32>` (78) | `topic[i] * topic[j] * synergy(i,j)` for all `i < j` | `training/computation.rs:118` |
| `group_alignments` | `[f32; 6]` | Group-weighted average of topic profile | `training/computation.rs:141` |
| `teleological_vector` | `[f32; 13]` | Per-embedder L2 norm of the active vector | `teleological/types.rs:105` |

Cross-references: embedder model IDs in [05_embedding_subsystem.md](05_embedding_subsystem.md); column-family architecture in [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md); constellation use of fingerprints in [17_constellation_subsystem.md](17_constellation_subsystem.md); training-record use in [16_training_subsystem.md](16_training_subsystem.md); CAMD-DPO use in [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md); graph/typed-edge use in [07_graph_and_causal.md](07_graph_and_causal.md).

---

## 2. `SemanticFingerprint`

**Source**: `types/fingerprint/semantic/fingerprint.rs:145`. Type alias `TeleologicalArray = SemanticFingerprint`.

Stores all 13 embeddings without fusion. No `Default` impl. Test-only helpers `zeroed()` and `stub()` gated on `#[cfg(any(test, feature = "test-utils"))]`.

### 2.1 Fields

| Field | Type | Description | Source |
|---|---|---|---|
| `e1_semantic` | `Vec<f32>` | E1 general semantic (e5-large-v2), 1024D | `fingerprint.rs:147` |
| `e2_temporal_recent` | `Vec<f32>` | E2 recency (sinusoidal PE), 512D | `fingerprint.rs:150` |
| `e3_temporal_periodic` | `Vec<f32>` | E3 periodic (Fourier), 512D | `fingerprint.rs:153` |
| `e4_temporal_positional` | `Vec<f32>` | E4 positional (sinusoidal PE), 512D | `fingerprint.rs:156` |
| `e5_causal_as_cause` | `Vec<f32>` | E5 asymmetric cause side (Longformer SCM), 768D | `fingerprint.rs:162` |
| `e5_causal_as_effect` | `Vec<f32>` | E5 asymmetric effect side, 768D | `fingerprint.rs:168` |
| `e5_causal` | `Vec<f32>` | Legacy E5 single vector (backcompat, `#[serde(default)]`) | `fingerprint.rs:175` |
| `e6_sparse` | `SparseVector` | E6 SPLADE sparse lexical, 30522-vocab | `fingerprint.rs:178` |
| `e7_code` | `Vec<f32>` | E7 Qodo-Embed code, 1536D | `fingerprint.rs:181` |
| `e8_graph_as_source` | `Vec<f32>` | E8 asymmetric graph source, 1024D | `fingerprint.rs:188` |
| `e8_graph_as_target` | `Vec<f32>` | E8 asymmetric graph target, 1024D | `fingerprint.rs:195` |
| `e8_graph` | `Vec<f32>` | Legacy E8 single vector (backcompat) | `fingerprint.rs:202` |
| `e9_hdc` | `Vec<f32>` | E9 projected HDC, 1024D | `fingerprint.rs:205` |
| `e10_multimodal_paraphrase` | `Vec<f32>` | E10 asymmetric paraphrase side, 768D | `fingerprint.rs:212` |
| `e10_multimodal_as_context` | `Vec<f32>` | E10 asymmetric context side, 768D | `fingerprint.rs:219` |
| `e11_entity` | `Vec<f32>` | E11 KEPLER entity, 768D | `fingerprint.rs:222` |
| `e12_late_interaction` | `Vec<Vec<f32>>` | E12 ColBERT, 128D per token, variable count | `fingerprint.rs:225` |
| `e13_splade` | `SparseVector` | E13 SPLADE v3 sparse, 30522-vocab | `fingerprint.rs:228` |

### 2.2 Asymmetric Access Helpers

Asymmetric embedders (E5, E8, E10) expose `*_active_vector()` for symmetric ops and side-specific accessors for asymmetric ops:

| Embedder | Active vector | Cause/Source/Paraphrase | Effect/Target/Context |
|---|---|---|---|
| E5 | `e5_active_vector()` | `get_e5_as_cause()` | `get_e5_as_effect()` |
| E8 | `e8_active_vector()` | `get_e8_as_source()` | `get_e8_as_target()` |
| E10 | `e10_active_vector()` | `get_e10_as_paraphrase()` | `get_e10_as_context()` |

E5 and E8 fall back to the legacy `e5_causal` / `e8_graph` field when the asymmetric pair is empty. E10 falls back from `e10_multimodal_paraphrase` to `e10_multimodal_as_context` because it was introduced with dual vectors (no legacy unified field). `migrate_legacy_e8()` copies the legacy `e8_graph` into both sides.

### 2.3 `EmbeddingRef` Enum

**Source**: `fingerprint.rs:40`.

| Variant | Payload | Used for |
|---|---|---|
| `Dense(&[f32])` | Contiguous float slice | E1, E2-E5, E7-E11 |
| `Sparse(&SparseVector)` | Sparse ref | E6, E13 |
| `TokenLevel(&[Vec<f32>])` | Per-token dense slices | E12 |

### 2.4 `ValidationError` Enum

**Source**: `fingerprint.rs:54`.

| Variant | Meaning |
|---|---|
| `DimensionMismatch { embedder, expected, actual }` | Dense embedding wrong size |
| `EmptyDenseEmbedding { embedder, expected }` | Required dense vector empty |
| `TokenDimensionMismatch { embedder, token_index, expected, actual }` | E12 token wrong size |
| `SparseIndexOutOfBounds { embedder, index, vocab_size }` | Sparse index exceeds vocab |
| `SparseIndicesValuesMismatch { embedder, indices_len, values_len }` | Sparse parallel arrays mismatch |

`get_embedding(idx: usize) -> Option<EmbeddingSlice>` dispatches by index (0-12); E5 returns cause side, E8 returns source side, E10 returns paraphrase side.

---

## 3. `TeleologicalFingerprint`

**Source**: `types/fingerprint/teleological/types.rs:32`. Serialized with bincode (positional).

### 3.1 Fields

| Field | Type | Description | Source |
|---|---|---|---|
| `id` | `Uuid` | UUID v4 primary key | `types.rs:34` |
| `semantic` | `SemanticFingerprint` | The 13-embedding bundle | `types.rs:37` |
| `content_hash` | `[u8; 32]` | SHA-256 of source content | `types.rs:40` |
| `created_at` | `DateTime<Utc>` | Creation timestamp | `types.rs:43` |
| `last_updated` | `DateTime<Utc>` | Last update timestamp | `types.rs:46` |
| `access_count` | `u64` | Read counter | `types.rs:49` |
| `importance` | `f32` | Clamped `[0.0, 1.0]`. Default `0.5` | `types.rs:54` |
| `last_accessed_at` | `DateTime<Utc>` | `#[serde(skip)]`; in-memory only, resets to `Utc::now()` on deserialize | `types.rs:66` |
| `e6_sparse` | `Option<SparseVector>` | Original E6 sparse vector for Stage-1 inverted-index recall (`#[serde(default)]`) | `types.rs:81` |

### 3.2 `teleological_vector()` — 13D Compact Signature

**Source**: `types.rs:105`. Returns `[f32; TELEOLOGICAL_VECTOR_DIM]` where `TELEOLOGICAL_VECTOR_DIM = 13`.

Per-slot formula:

| Slot | Embedder | Formula |
|---|---|---|
| 0 | E1 | `l2_norm(e1_semantic)` |
| 1-3 | E2-E4 | `l2_norm(e*_temporal_*)` |
| 4 | E5 | `l2_norm(e5_active_vector())` |
| 5 | E6 | `indices.len() as f32 / 1000.0` |
| 6 | E7 | `l2_norm(e7_code)` |
| 7 | E8 | `l2_norm(e8_active_vector())` |
| 8 | E9 | `l2_norm(e9_hdc)` |
| 9 | E10 | `l2_norm(e10_active_vector())` |
| 10 | E11 | `l2_norm(e11_entity)` |
| 11 | E12 | Average L2 norm across tokens (0 if empty) |
| 12 | E13 | `indices.len() as f32 / 1000.0` |

---

## 4. `Embedder` Enum and Related Types

**Source**: `teleological/embedder.rs`.

### 4.1 `Embedder` Variants

| Variant | Index | Display | Kind | Dimension |
|---|---|---|---|---|
| `Semantic` | 0 | `E1_Semantic` | Dense | 1024 |
| `TemporalRecent` | 1 | `E2_Temporal_Recent` | Dense | 512 |
| `TemporalPeriodic` | 2 | `E3_Temporal_Periodic` | Dense | 512 |
| `TemporalPositional` | 3 | `E4_Temporal_Positional` | Dense | 512 |
| `Causal` | 4 | `E5_Causal` | Dense (asym) | 768 |
| `Sparse` | 5 | `E6_Sparse_Lexical` | Sparse | 30522 vocab |
| `Code` | 6 | `E7_Code` | Dense | 1536 |
| `Graph` | 7 | `E8_Graph` | Dense (asym) | 1024 |
| `Hdc` | 8 | `E9_HDC` | Dense | 1024 |
| `Contextual` | 9 | `E10_Multimodal` | Dense (asym) | 768 |
| `Entity` | 10 | `E11_Entity` | Dense | 768 |
| `LateInteraction` | 11 | `E12_Late_Interaction` | TokenLevel | 128/token |
| `KeywordSplade` | 12 | `E13_SPLADE` | Sparse | 30522 vocab |

### 4.2 `EmbedderDims`

| Variant | Fields |
|---|---|
| `Dense(usize)` | Fixed-length dense dim |
| `Sparse { vocab_size: usize }` | Sparse vocabulary size |
| `TokenLevel { per_token: usize }` | Per-token dense dim |

### 4.3 `EmbedderGroup`

Coarse taxonomic grouping (orthogonal to `GroupType` in §5): `General`, `Temporal`, `Causal`, `Code`, `Graph`, `Entity`, `Multimodal`, `Lexical`.

### 4.4 `EmbedderMask`

`u16` bitset (bit `i` = embedder `i`). Constructors: `all()`, `none()`, `from_indices(&[usize])`. Queries: `contains(Embedder)`, `is_empty()`, `active_count()`, `iter()`.

---

## 5. Hierarchical Groups (6-Way)

**Source**: `teleological/groups.rs`.

Groups partition the 13 embedders into 6 semantic clusters. Constant `NUM_GROUPS = 6`.

### 5.1 `GroupType` and Embedder Assignments

| `GroupType` | Index | Captures | Embedder indices (`group_indices`) |
|---|---|---|---|
| `Factual` | 0 | what IS (semantic, factual, sparse) | `FACTUAL = [0, 11, 12]` (E1, E12, E13) |
| `Temporal` | 1 | when/sequence | `TEMPORAL = [1, 2]` (E2, E3) |
| `Causal` | 2 | why/how | `CAUSAL = [3, 6]` (E4, E7) |
| `Relational` | 3 | like/where/who | `RELATIONAL = [4, 7, 8]` (E5, E8, E9) |
| `Qualitative` | 4 | feel/principle | `QUALITATIVE = [9, 10]` (E10, E11) |
| `Implementation` | 5 | code | `IMPLEMENTATION = [5]` (E6) |

All 13 embeddings are covered by exactly one group (verified by `test_group_indices_coverage`).

### 5.2 `GroupAlignments` Struct

| Field | Type | Description |
|---|---|---|
| `factual` | `f32` | Mean of E1, E12, E13 alignments |
| `temporal` | `f32` | Mean of E2, E3 |
| `causal` | `f32` | Mean of E4, E7 |
| `relational` | `f32` | Mean of E5, E8, E9 |
| `qualitative` | `f32` | Mean of E10, E11 |
| `implementation` | `f32` | E6 only |

### 5.3 Methods

| Method | Signature | Behavior |
|---|---|---|
| `from_alignments` | `(&[f32;13], Option<&[f32;13]>) -> Self` | Weighted per-group average |
| `new` | `(f32, f32, f32, f32, f32, f32) -> Self` | Explicit construction |
| `as_array` | `() -> [f32; 6]` | Returns `[factual, temporal, causal, relational, qualitative, implementation]` |
| `from_array` | `([f32; 6]) -> Self` | Inverse of `as_array` |
| `average` | `() -> f32` | Mean of the 6 group values |
| `std_dev` | `() -> f32` | Population stddev of the 6 values |
| `coherence` | `() -> f32` | `1.0 / (1.0 + std_dev)` — high when all groups agree |
| `dominant_group` | `() -> GroupType` | Argmax |
| `weakest_group` | `() -> GroupType` | Argmin |
| `similarity` | `(&Self) -> f32` | 6D cosine similarity |
| `get` / `set` | `(GroupType) -> f32` / `(GroupType, f32)` | Field accessors |

### 5.4 `weighted_group_average` Helper

**Source**: `groups.rs:240`.

Private function used by `from_alignments`. For a given group's index list:

```
sum_i (alignments[i] * weights[i]) / sum_i weights[i]
```

Returns `0.0` when the group's indices are empty or total weight is `< f32::EPSILON`.

---

## 6. Synergy Matrix (13x13)

**Source**: `teleological/synergy_matrix/`.

Captures cross-embedder relationship strengths. Used to weight `cross_correlations` in training records and in `TeleologicalVector`.

### 6.1 Constants

| Constant | Value | Source |
|---|---|---|
| `SYNERGY_DIM` | `13` | `constants.rs:10` |
| `CROSS_CORRELATION_COUNT` | `78` = `C(13, 2)` | `constants.rs:14` |
| `BASE_SYNERGIES` | `[[f32; 13]; 13]` | `constants.rs:35` |

### 6.2 `SynergyMatrix` Struct

**Source**: `synergy_matrix/types.rs:12`.

| Field | Type | Description |
|---|---|---|
| `values` | `[[f32; 13]; 13]` | Symmetric with diagonal = 1.0; off-diagonal in `[0, 1]` |
| `weights` | `[[f32; 13]; 13]` | Per-cell adaptive-learning weights |
| `computed_at` | `DateTime<Utc>` | Matrix creation/update time |
| `sample_count` | `u64` | Samples used to compute/refine |

### 6.3 Base Synergy Levels

Off-diagonal values drawn from a 3-level palette: weak = 0.3, moderate = 0.6, strong = 0.9. Diagonal is 1.0.

Selected strong pairs (`>= 0.9`): E1↔E5 (semantic-causal), E1↔E11 (semantic-abstract), E1↔E12 (semantic-factual), E2↔E3 (recency-periodic), E2↔E9 (recency-social), E2↔E10 (recency-emotional), E3↔E4 (periodic-causal), E3↔E7 (periodic-procedural), E4↔E6 (causal-code), E4↔E7 (causal-procedural), E4↔E11 (causal-abstract), E4↔E12 (causal-factual), E6↔E7 (code-procedural), E6↔E8 (code-spatial), E6↔E13 (code-sparse), E9↔E10 (social-emotional), E12↔E13 (factual-sparse). Full table in `constants.rs:35-62`.

### 6.4 Constructors

| Method | Returns | Source |
|---|---|---|
| `new()` | Zeroed off-diagonal, diagonal = 1.0 | `constructors.rs` |
| `with_base_synergies()` | `BASE_SYNERGIES` loaded | `constructors.rs` |
| `identity()` | Identity matrix | `constructors.rs` |

Presets (in `presets.rs`): task-specific matrices referenced by `TeleologicalProfile`.

### 6.5 Accessors

| Method | Signature | Behavior |
|---|---|---|
| `get_synergy` | `(i, j) -> f32` | Returns `values[i][j]` |
| `set_synergy` | `(i, j, f32)` | Mirrors to `values[j][i]` (preserves symmetry); rejects `i == j` |
| `get_weight` / `set_weight` | `(i, j) -> f32` / `(i, j, f32)` | Per-cell adaptive weight |
| `active_pairs_above_threshold` | `(f32) -> Vec<(usize, usize, f32)>` | Iterates upper triangle |

`validation.rs` enforces symmetry, diagonal = 1.0, and `[0, 1]` range. `analysis.rs` provides `mean_synergy()`, `max_synergy()`, `min_synergy()`.

---

## 7. Derived Cross-Features

### 7.1 `topic_profile_or_fallback`

**Source**: `training/computation.rs:41`. Signature: `(Option<[f32; 13]>, &SemanticFingerprint) -> [f32; 13]`.

- If caller provides a stored profile from `CF_TOPIC_PROFILES`, returns it verbatim.
- Otherwise derives a **presence indicator** per embedder (dense → clamped L2 norm; sparse → clamped `sqrt(sum_sq)`; token-level → mean of non-zero per-token dense alignments). Asymmetric embedders use the cause/source/paraphrase side.

### 7.2 `compute_cross_correlations`

**Source**: `training/computation.rs:118`. Signature: `(&[f32; 13], &SynergyMatrix) -> Vec<f32>` of length 78.

For each `i < j` in `0..13`:

```
out.push(topic[i].clamp(0,1) * topic[j].clamp(0,1) * synergy(i,j).clamp(0,1))
```

Pair order: `(0,1), (0,2), ..., (0,12), (1,2), ..., (11,12)`.

### 7.3 `compute_group_alignments`

**Source**: `training/computation.rs:141`. Signature: `(&[f32; 13]) -> [f32; 6]`.

Delegates to `GroupAlignments::from_alignments(profile, None).as_array()`.

---

## 8. `TeleologicalVector` (Fused Representation)

**Source**: `teleological/vector.rs:22`.

| Field | Type | Description |
|---|---|---|
| `topic_profile` | `TopicProfile` | 13D per-embedder alignment |
| `cross_correlations` | `Vec<f32>` | 78 synergy-weighted pair interactions (order: `(0,1), (0,2), ..., (11,12)`) |
| `group_alignments` | `GroupAlignments` | 6D group aggregation |
| `tucker_core` | `Option<TuckerCore>` | Compressed tensor representation |
| `profile_id` | `Option<ProfileId>` | Task-profile tag |
| `confidence` | `f32` | `[0.0, 1.0]` fusion confidence |

Methods: `new`, `with_all`, `get_correlation(i, j)`, `set_correlation(i, j, f32)`. `with_all` panics when `cross_correlations.len() != 78`.

---

## 9. Task Profiles, Matrix Search, Comparator, Meaning, Resolution

These submodules layer task-aware dispatch, retrieval, and comparison on top of the core types. Listed here as reference surface only — full details live in their own files.

| Submodule | Key Types | Purpose |
|---|---|---|
| `profile` | `TeleologicalProfile`, `TaskType`, `FusionStrategy`, `ProfileMetrics` | Task-specific weight/synergy configuration (e.g. `code_implementation`, `conceptual_research`) |
| `matrix_search` | `TeleologicalMatrixSearch`, `SearchStrategy`, `ComparisonScope`, `ComponentWeights`, `SimilarityBreakdown`, `MatrixSearchConfig`, `ComprehensiveComparison` | Cross-correlation search across all 13 embedders |
| `comparator` | `TeleologicalComparator`, `ComparisonResult`, `BatchComparator` | Pairwise fingerprint comparison |
| `meaning` | `MeaningExtractionConfig`, `ExtractedMeaning`, `FusionMethod`, `CrossEmbeddingAnalysis`, `NuanceDimension` | Attention/contrast/sparsification tunables |
| `resolution` | `MultiResolutionHierarchy`, `DomainAlignments`, `DomainType`, `ResolutionView` | 4-level profile hierarchy (13D → 6D → 3D → 1D) |
| `tucker` | `CpuTuckerCompressor`, `TuckerCompressor`, `TuckerCore`, `TuckerError` | Streaming HOSVD compressor (CPU only; ranks default `(4, 4, 128)`). Full write-up in [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) §4. |
| `comparison_error` | `ComparisonValidationError`, `WeightValues`, `ComparisonValidationResult` | Validation error surface for comparator |

### 9.1 `TopicProfile`

**Source**: `types.rs:236`.

| Field | Type | Description |
|---|---|---|
| `alignments` | `[f32; 13]` | Per-embedder alignment, `[0, 1]` |

Methods: `new`, `uniform(f32)`, `aggregate_alignment() -> f32` (mean), `similarity(&Self) -> f32` (13D cosine), `get`/`set`, `active_count(threshold) -> usize`.

### 9.2 `TuckerCore`

**Source**: `types.rs:73`.

| Field | Type | Description |
|---|---|---|
| `ranks` | `(usize, usize, usize)` | Default `(4, 4, 128)` |
| `data` | `Vec<f32>` | Flattened core, size `r1 * r2 * r3` |
| `u1` | `Vec<f32>` | Factor matrix `13 × r1` |
| `u2` | `Vec<f32>` | Factor matrix `13 × r2` |
| `u3` | `Vec<f32>` | Factor matrix `1024 × r3` |

Methods: `new(ranks)`, `with_default_ranks()`, `get_core(i,j,k)`, `set_core(i,j,k,v)`, `compressed_size()`, `original_size() = 173_056`, `compression_ratio()`. Constructors panic on any rank `== 0` or out-of-bounds access.

### 9.3 `ProfileId`

**Source**: `types.rs:14`. Newtype wrapper over `String`. `new()` panics on empty. Impl: `Display`, `From<&str>`, `From<String>`, `AsRef<str>`, `Serialize`/`Deserialize`.

---

## 10. Teleological Column Families (28)

**Source**: `context-graph-storage/src/teleological/column_families.rs`. Constant `TELEOLOGICAL_CF_COUNT = 28`; the `TELEOLOGICAL_CFS` slice lists all 28 names.

| # | Constant | CF name | Key | Value | Purpose |
|---|---|---|---|---|---|
| 1 | `CF_FINGERPRINTS` | `fingerprints` | UUID (16B) | `bincode(TeleologicalFingerprint)` | Primary fingerprint storage (~63KB) |
| 2 | `CF_TOPIC_PROFILES` | `topic_profiles` | UUID (16B) | `13 × f32` = 52B | Per-memory 13D topic profile |
| 3 | `CF_E13_SPLADE_INVERTED` | `e13_splade_inverted` | `term_id: u16` (2B) | `bincode(Vec<Uuid>)` | E13 SPLADE inverted index |
| 4 | `CF_E6_SPARSE_INVERTED` | `e6_sparse_inverted` | `term_id: u16` (2B) | `bincode(Vec<Uuid>)` | E6 sparse lexical inverted index |
| 5 | `CF_E1_MATRYOSHKA_128` | `e1_matryoshka_128` | UUID (16B) | `128 × f32` = 512B | E1 truncated matryoshka for coarse filtering |
| 6 | `CF_CONTENT` | `content` | UUID (16B) | UTF-8 text (LZ4) | Original content text (≤1MB) |
| 7 | `CF_SOURCE_METADATA` | `source_metadata` | UUID (16B) | JSON(`SourceMetadata`) | Per-memory provenance |
| 8 | `CF_FILE_INDEX` | `file_index` | file path bytes | `bincode(FileIndexEntry)` | file_path → fingerprint id index |
| 9 | `CF_TOPIC_PORTFOLIO` | `topic_portfolio` | session_id bytes or `"__latest__"` | `JSON(PersistedTopicPortfolio)` | Session-end topic portfolio snapshot |
| 10 | `CF_E12_LATE_INTERACTION` | `e12_late_interaction` | UUID (16B) | `bincode(Vec<Vec<f32>>)` | E12 ColBERT token embeddings |
| 11 | `CF_ENTITY_PROVENANCE` | `entity_provenance` | `{canonical_id}_{uuid}` | `bincode(EntityProvenance)` | Entity extraction sources (reserved) |
| 12 | `CF_AUDIT_LOG` | `audit_log` | `timestamp_be (8) + uuid (16)` | `JSON(AuditRecord)` | Append-only audit log |
| 13 | `CF_AUDIT_BY_TARGET` | `audit_by_target` | `uuid (16) + timestamp_be (8)` | primary key (24B) | Audit secondary index by target |
| 14 | `CF_MERGE_HISTORY` | `merge_history` | `merged_uuid (16) + timestamp_be (8)` | `JSON(MergeRecord)` | Permanent merge lineage |
| 15 | `CF_IMPORTANCE_HISTORY` | `importance_history` | `uuid (16) + timestamp_be (8)` | `JSON(ImportanceChangeRecord)` | Permanent importance change audit |
| 16 | `CF_TOOL_CALL_INDEX` | `tool_call_index` | tool_use_id bytes | `bincode(Vec<Uuid>)` | tool_use_id → memory ids (reserved) |
| 17 | `CF_CONSOLIDATION_RECOMMENDATIONS` | `consolidation_recommendations` | UUID (16B) | `bincode(ConsolidationRecommendation)` | Dream-phase recommendations (reserved) |
| 18 | `CF_EMBEDDING_REGISTRY` | `embedding_registry` | UUID (16B) | `JSON(EmbeddingVersionRecord)` | Per-fingerprint embedder model versions |
| 19 | `CF_CUSTOM_WEIGHT_PROFILES` | `custom_weight_profiles` | profile name bytes | `JSON([f32; 13])` | User-defined search weight profiles |
| 20 | `CF_HNSW_GRAPHS` | `hnsw_graphs` | `graph:{embedder}` / `meta:{embedder}` | usearch bytes / JSON | Persisted HNSW indexes |
| 21 | `CF_TRAINING_RECORDS` | `training_records` | UUID (16B) | `[v:u8=2][bincode(TrainingRecord)]` | Exported training corpus rows |
| 22 | `CF_CONSTELLATIONS` | `constellations` | constellation UUID (16B) | `[v:u8=1][bincode(Constellation)]` | Compiled constellations |
| 23 | `CF_CONSTELLATION_BY_SELECTOR` | `constellation_by_selector` | `[kind:1][sha256_prefix:16]` = 17B | constellation UUID (16B) | Selector → constellation id index |
| 24 | `CF_CONTRASTIVE_PAIRS` | `contrastive_pairs` | `[anchor:16][negative:16]` = 32B | `[v:u8=1][bincode(ContrastivePair)]` | Mined contrastive pairs |
| 25 | `CF_CONTRASTIVE_BY_KIND` | `contrastive_by_kind` | `[kind:1][anchor:16][neg:16]` = 33B | empty | Secondary index by anomaly kind |
| 26 | `CF_CONTRASTIVE_BY_ANCHOR` | `contrastive_by_anchor` | anchor UUID (16B) | `bincode(Vec<Uuid>)` | Secondary index anchor → negatives |
| 27 | `CF_TYPED_EDGE_RECORDS` | `typed_edge_records` | `[source:16][target:16][edge_type:1]` = 33B | `[v:u8=1][bincode(TypedEdgeTrainingRecord)]` | Exported typed-edge training rows |
| 28 | `CF_TYPED_EDGE_VALIDATIONS` | `typed_edge_validations` | `[source:16][target:16][edge_type:1]` = 33B | `[v:u8=1][bincode(LLMEdgeValidation)]` | LLM verdicts on low-confidence edges |

CFs 1-23 carry core teleological data and derived fusion features. CFs 24-28 persist training signals (see [17_constellation_subsystem.md](17_constellation_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md)).

The base RocksDB schema also opens 13 quantized per-embedder CFs (`CF_EMB_0` … `CF_EMB_12`; `QUANTIZED_EMBEDDER_CF_COUNT = 13`) for forward compatibility; these are not yet populated.

---

## 11. Persistence Path — `store_memory`

**Source**: `context-graph-storage/src/teleological/rocksdb_store/crud.rs:50`.

`store_async(fp: TeleologicalFingerprint) -> CoreResult<Uuid>`:

1. Reject if `fp.id` is soft-deleted (returns `ValidationError`).
2. Call `store_fingerprint_internal(&fp, is_new=true)` — a single `WriteBatch` writing to:
   - `CF_FINGERPRINTS` — full bincode payload keyed by UUID.
   - `CF_E1_MATRYOSHKA_128` — 128D truncated E1 vector.
   - `CF_E13_SPLADE_INVERTED` — per-term inverted index updates.
   - `CF_E6_SPARSE_INVERTED` — per-term inverted index updates (if `e6_sparse` populated).
   - `CF_E12_LATE_INTERACTION` — bincode of token vectors (if non-empty).
   - `CF_TOPIC_PROFILES` — 13D topic profile (computed at store time).
   - `CF_SOURCE_METADATA` — JSON metadata (if supplied).
   - Quantized per-embedder CFs — reserved.
3. Call `add_to_indexes(&fp)` to insert into all 15 HNSW indexes (E1, E1Matryoshka128, E2-E4, E5CausalCause, E5CausalEffect, E7-E11, E10MultimodalParaphrase, E10MultimodalContext).
4. On HNSW failure, invoke `delete_async(id, soft=false)` to roll back all CFs (single-phase cleanup).
5. Return `fp.id`.

### Encoding

- `TeleologicalFingerprint` → bincode (positional; new fields append-only).
- `SourceMetadata`, `AuditRecord`, `MergeRecord`, `ImportanceChangeRecord`, `EmbeddingVersionRecord`, custom weight profiles, HNSW metadata, topic-portfolio snapshots → JSON.
- Version-prefixed rows: `TrainingRecord` (`v=2`), `Constellation` (`v=1`), `ContrastivePair` (`v=1`), `TypedEdgeTrainingRecord` (`v=1`), `LLMEdgeValidation` (`v=1`) — single version byte prepended to bincode payload.
- Validation: `serialization.rs` rejects empty payloads and wrong version bytes with `CoreError::SerializationError`.

---

## 12. Query Paths

### 12.1 Retrieve Full Fingerprint

`retrieve_async(id: Uuid) -> CoreResult<Option<TeleologicalFingerprint>>` (`crud.rs:117`):

1. If `is_soft_deleted(id)`, return `Ok(None)`.
2. `get_fingerprint_raw(id)` → bytes from `CF_FINGERPRINTS`.
3. `deserialize_teleological_fingerprint(&data)` → struct.

### 12.2 Retrieve Topic Profile Only

`get_topic_profile(id: Uuid) -> CoreResult<Option<[f32; 13]>>` reads `CF_TOPIC_PROFILES` directly; 52 bytes per row. Used by the training exporter and the constellation compiler to avoid full-fingerprint deserialization.

### 12.3 Compute 13D Signature On-Demand

`TeleologicalFingerprint::teleological_vector()` (§3.2) computes the compact 13-slot signature without storage access.

### 12.4 Search Paths

The `TeleologicalMemoryStore` trait (`trait_impl.rs:32`) exposes:

| Method | Returns | Backed by |
|---|---|---|
| `search_semantic(&SemanticFingerprint, options)` | `Vec<TeleologicalSearchResult>` | MultiSpace / Pipeline fusion across 11 dense HNSWs |
| `search_text(&str, options)` | `Vec<TeleologicalSearchResult>` | Text embedding + fusion |
| `search_sparse(&SparseVector, top_k)` | `Vec<(Uuid, f32)>` | E13 inverted + scoring |
| `search_e6_sparse(&SparseVector, max)` | `Vec<(Uuid, usize)>` | E6 inverted-index recall |
| `store_batch` / `retrieve_batch` | Batched CRUD | `WriteBatch` / `multi_get_cf` |
| `count` / `storage_size_bytes` | Scalar stats | CF iteration / disk stats |
| `flush` | Sync WAL to disk | RocksDB `flush` |

Full-search mechanics are covered in [06_search_and_fusion.md](06_search_and_fusion.md).

### 12.5 MCP Surface

The teleological store is the backing layer for all MCP tools that read/write memories (e.g. `store_memory`, `get_memory_fingerprint`, `search_graph`, `get_unified_neighbors`, `boost_importance`, `merge_concepts`, `forget_concept`). See [08_mcp_tools_reference.md](08_mcp_tools_reference.md).

---

## 13. Cross-References

| Topic | Document |
|---|---|
| Embedder models and dimensions | [05_embedding_subsystem.md](05_embedding_subsystem.md) |
| Search and fusion mechanics | [06_search_and_fusion.md](06_search_and_fusion.md) |
| Graph / causal / typed edges | [07_graph_and_causal.md](07_graph_and_causal.md) |
| MCP tool contract | [08_mcp_tools_reference.md](08_mcp_tools_reference.md) |
| Storage layer architecture | [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md) |
| Topic clustering | [12_clustering_and_topics.md](12_clustering_and_topics.md) |
| Training corpus export (`TrainingRecord`) | [16_training_subsystem.md](16_training_subsystem.md) |
| Constellation compiler | [17_constellation_subsystem.md](17_constellation_subsystem.md) |
| Contrastive pairs, typed-edge export, Tucker | [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) |
| CAMD-DPO training pipeline | [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md) |
