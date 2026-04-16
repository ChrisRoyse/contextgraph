# 18. Contrastive Pair Mining, Tucker Decomposition, and Typed-Edge Export Pipeline

## Source Files Covered

| # | Path | Subsystem |
|---|------|-----------|
| 1 | `crates/context-graph-core/src/contrastive/mod.rs` | Contrastive module root |
| 2 | `crates/context-graph-core/src/contrastive/mining.rs` | Mining pure functions |
| 3 | `crates/context-graph-core/src/contrastive/types.rs` | Contrastive types |
| 4 | `crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs` | Contrastive MCP handlers |
| 5 | `crates/context-graph-mcp/src/tools/definitions/contrastive.rs` | Contrastive tool schemas |
| 6 | `crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs` | Contrastive RocksDB persistence |
| 7 | `crates/context-graph-core/src/teleological/tucker.rs` | Tucker decomposition (HOSVD) |
| 8 | `crates/context-graph-core/src/typed_edge_export/mod.rs` | Typed-edge export types |
| 9 | `crates/context-graph-core/src/llm_edge_validation/mod.rs` | LLM edge validation types |
| 10 | `crates/context-graph-mcp/src/handlers/tools/typed_edges_tools.rs` | Typed-edge MCP handlers |
| 11 | `crates/context-graph-mcp/src/tools/definitions/typed_edges.rs` | Typed-edge tool schemas |
| 12 | `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_export.rs` | Typed-edge record persistence |
| 13 | `crates/context-graph-storage/src/teleological/rocksdb_store/typed_edge_keys.rs` | 33-byte composite key helpers |
| 14 | `crates/context-graph-storage/src/teleological/rocksdb_store/llm_validation.rs` | LLM validation persistence |
| 15 | `crates/context-graph-storage/src/teleological/rocksdb_store/anomaly_derivation.rs` | Anomaly derivation from edges |
| 16 | `crates/context-graph-storage/src/teleological/rocksdb_store/versioned_bincode.rs` | Shared versioned encode/decode |

---

## Table of Contents

1. [Overview](#1-overview)
2. [Constants](#2-constants)
3. [Contrastive Pair Mining (Phase 3)](#3-contrastive-pair-mining-phase-3)
4. [Tucker Decomposition (Phase 4)](#4-tucker-decomposition-phase-4)
5. [Typed-Edge Export Pipeline](#5-typed-edge-export-pipeline)
6. [MCP Tool Reference](#6-mcp-tool-reference)
7. [Storage Layer](#7-storage-layer)
8. [Versioned Bincode Encoding](#8-versioned-bincode-encoding)
9. [Error Handling](#9-error-handling)
10. [Cross-References](#10-cross-references)

---

## 1. Overview

Three related subsystems produce structured training data from the memory corpus:

- **Phase 3 -- Contrastive Pair Mining**: Discovers `(anchor, negative)` memory pairs exhibiting cross-embedder anomalies -- high similarity on one embedder axis, low on another. These pairs feed downstream contrastive/metric-learning training.
- **Phase 4 -- Tucker Decomposition**: Compresses the 13 x 13 x 1024 embedding interaction tensor of a single `SemanticFingerprint` into a low-rank Tucker-1 core via streaming HOSVD. CPU-only, no GPU dependency.
- **Typed-Edge Export Pipeline**: Four features (F1-F4) that export, annotate, classify, and LLM-validate the typed edges stored in `CF_TYPED_EDGES`.

All subsystems store results in RocksDB column families with versioned-bincode wire format. The contrastive pair and anomaly-derivation pipelines share the same `CF_CONTRASTIVE_PAIRS` write path.

---

## 2. Constants

### Contrastive Mining Constants

Defined in `contrastive/types.rs` and `contrastive/mod.rs`:

| Constant | Value | Source |
|----------|-------|--------|
| `CONTRASTIVE_PAIR_VERSION` | `1u8` | `contrastive/mod.rs:49` |
| `NUM_ANOMALY_KINDS` | `6u8` | `contrastive/types.rs:21` |
| `DEFAULT_MAX_PAIRS` | `10_000usize` | `contrastive/types.rs:24` |
| `DEFAULT_MIN_DISAGREEMENT` | `0.3f32` | `contrastive/types.rs:27` |
| `DEFAULT_HIGH_THRESHOLD` | `0.6f32` | `contrastive/types.rs:30` |
| `DEFAULT_LOW_THRESHOLD` | `0.3f32` | `contrastive/types.rs:33` |
| `DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR` | `10usize` | `contrastive/types.rs:36` |
| `GENERATOR_TAG` | `"cross_embedder_anomaly_v1"` | `contrastive/mining.rs:31` |

### Contrastive Handler Constants

Defined in `contrastive_tools.rs`:

| Constant | Value | Source |
|----------|-------|--------|
| `DEFAULT_CANDIDATE_POOL_SIZE` | `500usize` | `contrastive_tools.rs:57` |
| `HARD_MAX_CANDIDATE_POOL` | `50_000usize` | `contrastive_tools.rs:60` |
| `HARD_MAX_PAIRS` | `100_000usize` | `contrastive_tools.rs:63` |
| `HARD_MAX_TOP_K` | `500usize` | `contrastive_tools.rs:66` |

### Contrastive Storage Constants

Defined in `rocksdb_store/contrastive.rs`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `PAIR_KEY_LEN` | `32` bytes | `[anchor:16][neg:16]` composite primary key |
| `BY_KIND_KEY_LEN` | `33` bytes | `[kind:1][anchor:16][neg:16]` secondary index key |

### Tucker Constants

Defined in `teleological/tucker.rs`:

| Constant | Value | Source |
|----------|-------|--------|
| `REFERENCE_EMBEDDING_DIM` | `1024usize` | `tucker.rs:72` |
| `DEFAULT_RANKS` | `(4, 4, 128)` | `tucker.rs:76` |
| `EMPTY_ENERGY_THRESHOLD` | `1e-24f64` | `tucker.rs:81` |
| `JACOBI_MAX_SWEEPS` | `50usize` | `tucker.rs:85` |
| `JACOBI_TOL` | `1e-14f64` | `tucker.rs:89` |
| `POWER_ITER_MAX` | `500usize` | `tucker.rs:92` |
| `POWER_ITER_TOL` | `1e-5f64` | `tucker.rs:102` |

### Typed-Edge Export Constants

| Constant | Value | Source |
|----------|-------|--------|
| `TYPED_EDGE_RECORD_VERSION` | `1u8` | `typed_edge_export/mod.rs:32` |
| `LLM_EDGE_VALIDATION_VERSION` | `1u8` | `llm_edge_validation/mod.rs:25` |
| `TYPED_EDGE_KEY_LEN` | `33` bytes | `typed_edge_keys.rs:14` |
| `EXPORTER_VERSION` | `"typed_edge_export_v1"` | `typed_edges_tools.rs:51` |

### Typed-Edge Handler Constants

Defined in `typed_edges_tools.rs`:

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_EXPORT_MAX_EDGES` | `10_000` | Default max edges per export run |
| `HARD_MAX_EXPORT_EDGES` | `1_000_000` | Absolute cap on exported edges |
| `HARD_MAX_DERIVE_PAIRS` | `1_000_000` | Absolute cap on derived anomaly pairs |
| `DEFAULT_VALIDATE_MAX_EDGES` | `100` | Default max edges per LLM validation run |
| `HARD_MAX_VALIDATE_EDGES` | `10_000` | Absolute cap on LLM validations per run |
| `DEFAULT_LIST_LIMIT` | `100` | Default page size for list queries |
| `HARD_MAX_LIST_LIMIT` | `1_000` | Absolute cap on list page size |

---

## 3. Contrastive Pair Mining (Phase 3)

### 3.1 Public Types

#### `AnomalyKind` (enum)

Source: `contrastive/types.rs:48-66`

| Variant | `as_u8()` | `as_str()` | Rule |
|---------|-----------|------------|------|
| `SemanticButNotCausal` | 0 | `semantic_but_not_causal` | High E1, low E5 |
| `KeywordButNotParaphrase` | 1 | `keyword_but_not_paraphrase` | High E6 or E13, low E10 |
| `CodeShapeButDifferentIntent` | 2 | `code_shape_but_different_intent` | High E7, low E1 |
| `EntitySharedButDifferentStructure` | 3 | `entity_shared_but_different_structure` | High E11, low E8 |
| `HdcRobustButSemanticDifferent` | 4 | `hdc_robust_but_semantic_different` | High E9, low E1 |
| `Other` | 5 | `other` | Any other high/low combination |

The `as_u8()` ordering is an on-disk contract; reordering requires a `CONTRASTIVE_PAIR_VERSION` bump (`contrastive/types.rs:73`).

Methods: `as_u8()`, `from_u8(v: u8) -> Option<Self>`, `all() -> [AnomalyKind; 6]`, `as_str() -> &'static str`, `parse(s: &str) -> Option<Self>`.

#### `ContrastivePair` (struct)

Source: `contrastive/types.rs:143-171`. Derives `Debug, Clone, Serialize, Deserialize, PartialEq`. No `skip_serializing_if` (bincode positional contract).

| Field | Type | Description |
|-------|------|-------------|
| `anchor_id` | `Uuid` | Anchor memory UUID |
| `negative_id` | `Uuid` | Negative memory UUID |
| `anchor_text` | `String` | Anchor content text |
| `negative_text` | `String` | Negative content text |
| `similarity_profile` | `[f32; 13]` | Per-embedder similarity in `[0, 1]` SRC-3 convention |
| `high_embedders` | `Vec<u8>` | Indices of embedders above `high_threshold` |
| `low_embedders` | `Vec<u8>` | Indices of embedders below `low_threshold` |
| `disagreement_magnitude` | `f32` | `max(high_sim) - min(low_sim)`, always >= 0 |
| `anomaly_kind` | `AnomalyKind` | Classified anomaly type |
| `mined_at` | `DateTime<Utc>` | Mining timestamp |
| `generator` | `String` | Generator tag, e.g. `"cross_embedder_anomaly_v1"` |

On-disk encoding: `[CONTRASTIVE_PAIR_VERSION: u8][bincode(ContrastivePair)]`.

#### `MiningConfig` (struct)

Source: `contrastive/types.rs:177-194`. Not serialized; used in-process only.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `max_pairs` | `usize` | 10,000 | Cap on pairs mined per run |
| `kinds` | `Option<Vec<AnomalyKind>>` | `None` | Optional filter; `None` accepts all |
| `min_disagreement` | `f32` | 0.3 | Minimum high-low gap |
| `high_threshold` | `f32` | 0.6 | Similarity above this = "high" |
| `low_threshold` | `f32` | 0.3 | Similarity below this = "low" |
| `top_k_candidates_per_anchor` | `usize` | 10 | Candidates scored per anchor |
| `session_filter` | `Option<String>` | `None` | Restrict to matching session_id |

Validation via `MiningConfig::validate()`: rejects `min_disagreement` outside `[0, 1]`, rejects `low_threshold >= high_threshold` (`contrastive/types.rs:252-269`).

#### `MiningSummary` (struct)

Source: `contrastive/types.rs:211-227`. Derives `Debug, Clone, Default, Serialize, Deserialize, PartialEq`.

| Field | Type | Description |
|-------|------|-------------|
| `pairs_stored` | `usize` | Pairs successfully written |
| `pairs_skipped_below_threshold` | `usize` | Dropped for insufficient disagreement |
| `pairs_skipped_kind_filter` | `usize` | Dropped by kind filter |
| `anchors_scanned` | `usize` | Distinct anchors visited |
| `anchors_no_candidate` | `usize` | Anchors producing zero candidates |
| `duration_ms` | `u64` | Wall-clock run duration |

#### `ContrastiveError` (enum)

Source: `contrastive/types.rs:233-247`. Derives `Error, Debug`.

| Variant | Description |
|---------|-------------|
| `InvalidDisagreement(f32)` | `min_disagreement` outside `[0, 1]` |
| `InvalidThresholds(f32, f32)` | `low_threshold >= high_threshold` |
| `Storage(String)` | Storage backend failure (formatted string) |

### 3.2 Mining Algorithm

Source: `contrastive/mining.rs`. All functions are pure -- no I/O.

#### Step 1: Compute Similarity Profile

`similarity_profile(anchor: &SemanticFingerprint, negative: &SemanticFingerprint) -> [f32; 13]` (`mining.rs:42-91`)

For each of the 13 embedder slots:

| Slot | Embedder | Method |
|------|----------|--------|
| 0 | E1 semantic | SRC-3 cosine (symmetric) |
| 1 | E2 temporal recent | SRC-3 cosine (symmetric) |
| 2 | E3 temporal periodic | SRC-3 cosine (symmetric) |
| 3 | E4 temporal positional | SRC-3 cosine (symmetric) |
| 4 | E5 causal | SRC-3 cosine on **cause side** of both |
| 5 | E6 sparse keyword | Jaccard similarity (already `[0, 1]`) |
| 6 | E7 code | SRC-3 cosine (symmetric) |
| 7 | E8 graph | SRC-3 cosine on **source side** of both |
| 8 | E9 HDC | SRC-3 cosine (symmetric) |
| 9 | E10 multimodal | SRC-3 cosine on **paraphrase side** of both |
| 10 | E11 entity | SRC-3 cosine (symmetric) |
| 11 | E12 late interaction | SRC-3 cosine on **mean-pooled** tokens |
| 12 | E13 SPLADE | Jaccard similarity (already `[0, 1]`) |

SRC-3 convention: raw cosine `c in [-1, 1]` mapped via `(c + 1) / 2` to `[0, 1]` (`mining.rs:220-239`). Empty, dimension-mismatched, or zero-norm inputs return `0.0`. Non-finite results clamped to `[0, 1]` (`mining.rs:83-89`).

#### Step 2: Classify Anomaly

`classify_anomaly(profile: &[f32; 13], high_threshold: f32, low_threshold: f32) -> AnomalyKind` (`mining.rs:104-134`)

Priority-ordered rules (first match wins):

| Priority | Rule | Condition | Result |
|----------|------|-----------|--------|
| 1 | Code shape | `profile[6] > high` AND `profile[0] < low` | `CodeShapeButDifferentIntent` |
| 2 | Entity/structure | `profile[10] > high` AND `profile[7] < low` | `EntitySharedButDifferentStructure` |
| 3 | HDC/semantic | `profile[8] > high` AND `profile[0] < low` | `HdcRobustButSemanticDifferent` |
| 4 | Keyword/paraphrase | `(profile[5] > high` OR `profile[12] > high)` AND `profile[9] < low` | `KeywordButNotParaphrase` |
| 5 | Semantic/causal | `profile[0] > high` AND `profile[4] < low` | `SemanticButNotCausal` |
| 6 | Fallback | No rule matched | `Other` |

Code-shape is checked first to avoid false `SemanticButNotCausal` for code that carries an E1 signal (`mining.rs:113-116`).

#### Step 3: Mine Pair

`mine_pair_from_candidate(anchor_id, anchor_text, anchor_fp, negative_id, negative_text, negative_fp, cfg) -> Option<ContrastivePair>` (`mining.rs:146-210`)

Returns `None` when:
- `anchor_id == negative_id`
- No embedder clears `high_threshold` OR no embedder falls below `low_threshold` (no disagreement signal)
- `disagreement_magnitude < cfg.min_disagreement`
- `cfg.kinds` is `Some(list)` and the classified kind is not in `list`

### 3.3 MCP Handler Mining Loop

Source: `contrastive_tools.rs:70-397`

1. Parse args with hard-cap enforcement (e.g. `maxPairs` capped at `HARD_MAX_PAIRS=100_000`)
2. Validate `MiningConfig`; reject invalid thresholds
3. Downcast to `RocksDbTeleologicalStore`; error if unavailable
4. Load candidate pool: `list_fingerprints_unbiased(candidate_pool_size)` -- one unbiased scan shared across all anchors
5. Batch-fetch content for all pool members via `get_content_batch`
6. Optionally pre-fetch session metadata for session filtering
7. For each anchor in pool (respecting session mask and `max_pairs` cap):
   - Score every other pool member: compute 13-slot similarity profile, pre-filter by `high_low_bounds`
   - Sort scored candidates by disagreement descending, truncate to `top_k_candidates_per_anchor`
   - For each top-K candidate, call `mine_pair_from_candidate`; persist surviving pairs via `store_contrastive_pair`
8. Return `MiningSummary` JSON

---

## 4. Tucker Decomposition (Phase 4)

### 4.1 Public Types

#### `TuckerCore` (struct)

Source: `teleological/types.rs:73-90`. Derives `Serialize, Deserialize`.

| Field | Type | Description |
|-------|------|-------------|
| `ranks` | `(usize, usize, usize)` | Core tensor dimensions `(rank_1, rank_2, rank_3)`. Default `(4, 4, 128)` |
| `data` | `Vec<f32>` | Flattened core tensor. Size: `rank_1 * rank_2 * rank_3` |
| `u1` | `Vec<f32>` | Factor matrix U1, row-major `13 x rank_1` (embedding space) |
| `u2` | `Vec<f32>` | Factor matrix U2, row-major `13 x rank_2` (embedding space) |
| `u3` | `Vec<f32>` | Factor matrix U3, row-major `1024 x rank_3` (dimension compression) |

#### `TuckerError` (enum)

Source: `tucker.rs:106-133`. Derives `Error, Debug`.

| Variant | Fields | Description |
|---------|--------|-------------|
| `InvalidRank` | `r1, r2, r3, d1, d2, d3: usize` | Requested rank outside valid `1..=dim` range |
| `EmptyFingerprint` | -- | All reference-frame vectors are numerically zero |
| `Numerical(String)` | message | Power iteration or Jacobi convergence failure |

#### `TuckerCompressor` (trait)

Source: `tucker.rs:137-144`

```rust
pub trait TuckerCompressor {
    fn compress(
        &self,
        fingerprint: &SemanticFingerprint,
        ranks: (usize, usize, usize),
    ) -> Result<TuckerCore, TuckerError>;
}
```

Single implementation: `CpuTuckerCompressor` (`tucker.rs:147`).

### 4.2 Reference Frame Extraction

`extract_reference_view(fp: &SemanticFingerprint) -> Box<[[f32; 1024]; 13]>` (`tucker.rs:292-316`)

All 13 embeddings are projected onto a shared 1024-D reference frame. Allocated on the heap (52 KB).

| Slot | Embedder | Original Dim | Mapping |
|------|----------|-------------|---------|
| 0 | E1 semantic | 1024 | Used as-is |
| 1 | E2 temporal recent | 512 | Zero-padded to 1024 |
| 2 | E3 temporal periodic | 512 | Zero-padded to 1024 |
| 3 | E4 temporal positional | 512 | Zero-padded to 1024 |
| 4 | E5 causal (cause side) | 768 | Zero-padded to 1024 |
| 5 | E6 sparse keyword | sparse 30K | **Zero row** (not in dense space) |
| 6 | E7 code | 1536 | **Truncated** to first 1024 entries |
| 7 | E8 graph (source side) | 1024 | Used as-is |
| 8 | E9 HDC | 1024 | Used as-is |
| 9 | E10 multimodal (paraphrase) | 768 | Zero-padded to 1024 |
| 10 | E11 entity | 768 | Zero-padded to 1024 |
| 11 | E12 late interaction | var tokens x 128 | Mean-pooled tokens, zero-padded to 1024 |
| 12 | E13 SPLADE | sparse 30K | **Zero row** (not in dense space) |

Non-finite entries coerced to `0.0` (`tucker.rs:325`).

### 4.3 HOSVD Algorithm Steps

Source: `tucker.rs:149-278`. CPU-only. No threading, no GPU.

The conceptual interaction tensor is `T[i][j][d] = v_i[d] * v_j[d]` for `i, j in 0..13`, `d in 0..1024`. This 173,056-float tensor is **never materialized**.

**Step 1: Validate ranks and check energy**

Ranks must satisfy `1 <= r1,r2 <= 13` and `1 <= r3 <= 1024`. Total squared energy across all reference-frame entries must exceed `EMPTY_ENERGY_THRESHOLD = 1e-24` (`tucker.rs:176-184`).

**Step 2: Compute Mode-1/Mode-2 Gram matrix G_1 (13 x 13)**

```
G_1[a, b] = sum_d (v_a[d] * v_b[d] * w[d])
where w[d] = sum_j v_j[d]^2
```

By symmetry in `i <-> j`, `G_2 == G_1` (`tucker.rs:188-205`). Both are symmetric.

**Step 3: Jacobi eigendecomposition on G_1 (13 x 13)**

`jacobi_eigen_13()` (`tucker.rs:374-481`): Classical Jacobi rotation sweeps on the 13x13 symmetric matrix. Convergence when off-diagonal Frobenius mass < `JACOBI_TOL * (diagonal_mass + 1)`. Max `JACOBI_MAX_SWEEPS=50` sweeps. Returns top-k eigenvalues (sorted by absolute magnitude descending) and their eigenvectors.

- `U1` = top `rank_1` eigenvectors of `G_1`, shape `13 x rank_1`
- `U2` = top `rank_2` eigenvectors of `G_1` (separate decomposition for potentially different rank), shape `13 x rank_2`

**Step 4: Mode-3 Gram matrix G_3 (1024 x 1024) via power iteration**

`G_3[p,q] = (M[p,q])^2` where `M = V^T V` (1024 x 1024, ~52 MB f32). `M` is materialized explicitly; `G_3` is never materialized. Instead, `G_3 @ x` is computed lazily during power iteration as `sum_q (M[p,q])^2 * x[q]` (`g3_apply()`, `tucker.rs:634-646`).

`power_iteration_top_k()` (`tucker.rs:496-613`): Extracts top `rank_3` eigenvectors with deflation.

- Deterministic seed vector (no RNG): alternating-sign pattern seeded by eigenpair index (`tucker.rs:508-518`)
- Gram-Schmidt reorthogonalization against previously-extracted eigenvectors (two passes for numerical stability, `tucker.rs:618-630`)
- Convergence: direction delta `< POWER_ITER_TOL` AND Rayleigh quotient delta `< POWER_ITER_TOL` (`tucker.rs:594`)
- If residual subspace exhausted (near-zero norm), emit zero eigenvector (`tucker.rs:551-558`)
- Max `POWER_ITER_MAX=500` iterations per eigenpair

**Step 5: Core tensor computation (no T materialization)**

```
core[r1, r2, r3] = sum_d U3[d, r3] * A[r1, d] * B[r2, d]
A[r1, d] = sum_i U1[i, r1] * v_i[d]       (rank_1 x 1024)
B[r2, d] = sum_j U2[j, r2] * v_j[d]       (rank_2 x 1024)
```

Total FMA for default ranks `(4, 4, 128)`: `4 * 4 * 128 * 1024 = 2,097,152` (`tucker.rs:60-63`).

**Step 6: Flatten factor matrices**

`flatten_columns()` converts the column-of-eigenvectors representation into row-major `TuckerCore` packing: `u1[i * rank_1 + r1] = U1[i, r1]` (`tucker.rs:701-711`).

### 4.4 Test/Verification Helpers

| Function | Signature | Purpose |
|----------|-----------|---------|
| `reconstruct_tensor` | `(core: &TuckerCore) -> Vec<f32>` | Reconstructs the full 173,056-float tensor for Frobenius error verification |
| `relative_frobenius_error` | `(a: &[f32], b: &[f32]) -> f32` | `||a - b||_F / ||a||_F` relative reconstruction error |

Both are `pub` for downstream FSV use (`tucker.rs:722, 765`).

---

## 5. Typed-Edge Export Pipeline

Four features (F1-F4) constitute the typed-edges training-data factory:

| Feature | MCP Tool | Writes To | Description |
|---------|----------|-----------|-------------|
| F1 | `export_typed_edges_corpus` | `CF_TYPED_EDGE_RECORDS` | Walk typed edges, join content/metadata/LLM verdicts, persist records |
| F2 | `derive_anomalies_from_edges` | `CF_CONTRASTIVE_PAIRS` | Classify typed edges into anomaly kinds, persist as contrastive pairs |
| F3 | (edge signatures) | N/A | Structural output of the 13-slot `embedder_scores` per edge; produced implicitly by F1 |
| F4 | `validate_low_confidence_edges` | `CF_TYPED_EDGE_VALIDATIONS` | LLM-gated validation of low-weight edges |

### 5.1 Public Types

#### `TypedEdgeTrainingRecord` (struct)

Source: `typed_edge_export/mod.rs:38-87`. Derives `Debug, Clone, Serialize, Deserialize, PartialEq`.

| Field | Type | Description |
|-------|------|-------------|
| `source_memory_id` | `Uuid` | Source memory UUID (anchor end) |
| `target_memory_id` | `Uuid` | Target memory UUID (peer end) |
| `edge_type` | `u8` | `GraphLinkEdgeType::as_u8()` (0..=7) |
| `edge_type_name` | `String` | Snake-case enum name for consumer convenience |
| `weight` | `f32` | Computed weight in `[0.0, 1.0]` |
| `direction` | `u8` | `0=Symmetric, 1=Forward, 2=Backward` |
| `embedder_scores` | `[f32; 13]` | Per-embedder similarity profile (SRC-3 `[0, 1]`) |
| `agreement_count` | `u8` | Number of embedders that agreed |
| `agreeing_embedders` | `u16` | Bitset: bit 0 = E1, bit 12 = E13 |
| `source_content` | `String` | Source text (empty when suppressed) |
| `target_content` | `String` | Target text (empty when suppressed) |
| `source_session_id` | `Option<String>` | Source session id from `SourceMetadata` |
| `target_session_id` | `Option<String>` | Target session id from `SourceMetadata` |
| `source_type` | `Option<String>` | Source `SourceMetadata::source_type` |
| `target_type` | `Option<String>` | Target `SourceMetadata::source_type` |
| `mechanism_type` | `Option<String>` | CausalChain mechanism sub-type (from `CF_CAUSAL_RELATIONSHIPS`) |
| `llm_validation` | `Option<LLMValidationSummary>` | LLM verdict (joined from `CF_TYPED_EDGE_VALIDATIONS`) |
| `exported_at` | `DateTime<Utc>` | Export timestamp |
| `exporter_version` | `String` | Generator tag (`"typed_edge_export_v1"`) |

On-disk encoding: `[TYPED_EDGE_RECORD_VERSION: u8][bincode(TypedEdgeTrainingRecord)]`.

#### `LLMValidationSummary` (struct)

Source: `typed_edge_export/mod.rs:94-105`. Embedded inside `TypedEdgeTrainingRecord`.

| Field | Type | Description |
|-------|------|-------------|
| `validated_at` | `DateTime<Utc>` | LLM validation timestamp |
| `verdict` | `LLMVerdict` | Valid / Invalid / Reclassify |
| `confidence` | `f32` | LLM confidence in `[0.0, 1.0]` |
| `rationale` | `String` | 1-3 sentence LLM justification |
| `validator_version` | `String` | Model tag, e.g. `"Hermes-2-Pro-Mistral-7B-Q5_K_M@2026-04-14"` |

#### `LLMVerdict` (enum)

Source: `typed_edge_export/mod.rs:113-125`. Derives `Debug, Clone, Serialize, Deserialize, PartialEq`.

| Variant | Payload | `as_str()` | Description |
|---------|---------|------------|-------------|
| `Valid` | -- | `"valid"` | LLM confirmed the edge |
| `Invalid` | -- | `"invalid"` | LLM rejected the edge |
| `Reclassify` | `new_edge_type: u8` | `"reclassify"` | LLM suggested a different edge type |

Variant reordering/addition requires bumping both `LLM_EDGE_VALIDATION_VERSION` and `TYPED_EDGE_RECORD_VERSION`.

#### `LLMEdgeValidation` (struct)

Source: `llm_edge_validation/mod.rs:35-53`. Standalone persisted row in `CF_TYPED_EDGE_VALIDATIONS`.

| Field | Type | Description |
|-------|------|-------------|
| `validated_at` | `DateTime<Utc>` | Wall-clock validation time |
| `verdict` | `LLMVerdict` | Valid / Invalid / Reclassify |
| `confidence` | `f32` | LLM confidence `[0.0, 1.0]` |
| `rationale` | `String` | 1-3 sentence justification |
| `auto_derived_weight` | `f32` | Edge weight at moment of validation (drift audit) |
| `validator_version` | `String` | Model tag |
| `prompt_hash` | `[u8; 32]` | SHA-256 of the prompt sent to the LLM (reproducibility hook) |

On-disk encoding: `[LLM_EDGE_VALIDATION_VERSION: u8][bincode(LLMEdgeValidation)]`.

#### `AnomalyDerivationConfig` (struct)

Source: `anomaly_derivation.rs:32-44`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `high_threshold` | `f32` | 0.60 | "High" embedder score threshold |
| `low_threshold` | `f32` | 0.30 | "Low" embedder score threshold |
| `max_pairs` | `usize` | 10,000 | Hard cap on pairs per run |
| `min_disagreement` | `f32` | 0.30 | Minimum high-low gap |

Defaults deliberately mirror the offline miner for comparable output (`anomaly_derivation.rs:46-55`).

#### `AnomalyDerivationSummary` (struct)

Source: `anomaly_derivation.rs:59-77`. Derives `Debug, Clone, Default, PartialEq`.

| Field | Type | Description |
|-------|------|-------------|
| `edges_scanned` | `usize` | Total edges walked from `CF_TYPED_EDGES` |
| `pairs_written` | `usize` | Rows written to `CF_CONTRASTIVE_PAIRS` |
| `skipped_below_threshold` | `usize` | Edges below `min_disagreement` |
| `skipped_missing_content` | `usize` | Edges with missing source/target content |
| `per_kind_counts` | `HashMap<AnomalyKind, usize>` | Per-kind counts of written pairs |
| `duration_ms` | `u64` | Wall-clock duration |

### 5.2 F1 -- Export Typed-Edges Corpus

Source: `typed_edges_tools.rs:60-329`

Walk every typed edge in `CF_TYPED_EDGES`, assemble a `TypedEdgeTrainingRecord` with optional joins, and persist to `CF_TYPED_EDGE_RECORDS`.

Optional joins (all default to `true`):
- **Content join**: `get_content(source)` and `get_content(target)`
- **Source metadata join**: `get_source_metadata(source)` and `get_source_metadata(target)` for session_id and source_type
- **Mechanism-type join**: For `CausalChain` edges only, resolves mechanism_type from `CF_CAUSAL_RELATIONSHIPS` via `get_causal_relationships_by_source` (takes first non-empty mechanism_type)
- **LLM validation join**: Fetches from `CF_TYPED_EDGE_VALIDATIONS` by composite key

Idempotent on the `(source, target, edge_type)` composite key. `clearExisting=true` wipes the CF before writing.

### 5.3 F2 -- Derive Anomalies from Edges

Source: `anomaly_derivation.rs:89-179`

Rather than re-scanning the full corpus, F2 walks `CF_TYPED_EDGES` (already output of the embedder ensemble) and classifies each `(source, target)` pair. Each classified hit is written to `CF_CONTRASTIVE_PAIRS` + secondary indexes via the standard `store_contrastive_pair` write path.

Generator tag: `"typed_edge_anomaly_derivation_v1"` (`anomaly_derivation.rs:161`).

#### Edge-Type-Aware Anomaly Classification

`classify_edge_as_anomaly(edge: &TypedEdge, cfg: &AnomalyDerivationConfig) -> Option<(AnomalyKind, Vec<u8>, Vec<u8>, f32)>` (`anomaly_derivation.rs:201-277`)

Unlike the offline miner's priority-ordered rules (Section 3.2), F2 uses edge-type dispatching:

| Edge Type | Pattern | Condition | AnomalyKind |
|-----------|---------|-----------|-------------|
| `SemanticSimilar` | E1 high, E5 low | `scores[0] >= high` AND `scores[4] <= low` | `SemanticButNotCausal` |
| `KeywordOverlap` | E6 or E13 high, E10 low | `(scores[5] >= high` OR `scores[12] >= high)` AND `scores[9] <= low` | `KeywordButNotParaphrase` |
| `CodeRelated` | E7 high, E1 low | `scores[6] >= high` AND `scores[0] <= low` | `CodeShapeButDifferentIntent` |
| `EntityShared` | E11 high, E8 low | `scores[10] >= high` AND `scores[7] <= low` | `EntitySharedButDifferentStructure` |
| Any other | E9 high, E1 low | `scores[8] >= high` AND `scores[0] <= low` | `HdcRobustButSemanticDifferent` |

Note: Uses `>=`/`<=` for thresholds (inclusive), unlike the offline miner which uses `>`/`<` (exclusive). The `Other` kind is not emitted by F2 -- it is the responsibility of the offline miner (`anomaly_derivation.rs:6-8`).

### 5.4 F4 -- LLM Validation of Low-Confidence Edges

Source: `typed_edges_tools.rs:447-747`

Compile-time gated by `#[cfg(feature = "llm")]`. Without the feature, returns `{status:"skipped", reason:"llm feature not enabled"}` (`typed_edges_tools.rs:477-499`).

Requirements:
- `RocksDbTeleologicalStore` backend
- `EdgeRepository` available
- `graph_discovery_service` wired
- LLM loaded (`llm.is_loaded()`)

Algorithm:
1. Walk all typed edges
2. Skip edges with `weight >= weightCeiling` (default 0.60)
3. Apply optional session filter on source's `SourceMetadata.session_id`
4. Skip edges that already have a validation in `CF_TYPED_EDGE_VALIDATIONS`
5. Fetch source and target content (required for LLM prompt)
6. Compute SHA-256 prompt hash over `graph_agent::analyze_relationship::v1` prefix + length-prefixed source + target content (`typed_edges_tools.rs:913-925`)
7. Call `llm.analyze_relationship(&source_content, &target_content)`
8. Convert analysis to `LLMEdgeValidation`: `has_connection=true` => `Valid`, otherwise `Invalid` (no `Reclassify` emitted by current implementation)
9. Persist to `CF_TYPED_EDGE_VALIDATIONS`

Validator version string: `"Hermes-2-Pro-Mistral-7B@2026-04"` (`typed_edges_tools.rs:702`).

---

## 6. MCP Tool Reference

### 6.1 Contrastive Pair Tools (4 tools)

Source: `tools/definitions/contrastive.rs`

#### `mine_contrastive_pairs`

| Parameter | Type | Required | Default | Range | Description |
|-----------|------|----------|---------|-------|-------------|
| `maxPairs` | integer | No | 1000 | 1--100,000 | Hard cap on pairs persisted |
| `minDisagreement` | number | No | 0.3 | 0.0--1.0 | Minimum high-low gap |
| `highThreshold` | number | No | 0.6 | 0.0--1.0 | "High" similarity threshold |
| `lowThreshold` | number | No | 0.3 | 0.0--1.0 | "Low" similarity threshold; must be < `highThreshold` |
| `kinds` | array[string] | No | -- | 6 enum values | Optional AnomalyKind filter |
| `sessionFilter` | string | No | -- | -- | Restrict anchors by session_id |
| `topKCandidatesPerAnchor` | integer | No | 10 | 1--500 | Candidates scored per anchor |
| `candidatePoolSize` | integer | No | 500 | 10--50,000 | Peers sampled per run before top-K |

#### `list_contrastive_pairs`

| Parameter | Type | Required | Default | Range | Description |
|-----------|------|----------|---------|-------|-------------|
| `limit` | integer | No | 100 | 1--10,000 | Max pairs returned |
| `offset` | integer | No | 0 | >= 0 | Pagination offset |
| `kind` | string | No | -- | 6 enum values | AnomalyKind filter (uses `CF_CONTRASTIVE_BY_KIND` prefix scan) |
| `anchorId` | string (uuid) | No | -- | -- | Anchor UUID filter (uses `CF_CONTRASTIVE_BY_ANCHOR`) |
| `includeFull` | boolean | No | false | -- | Hydrate full ContrastivePair records |

#### `get_contrastive_pair`

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `anchorId` | string (uuid) | **Yes** | -- | Anchor memory UUID |
| `negativeId` | string (uuid) | **Yes** | -- | Negative memory UUID |

#### `count_contrastive_pairs`

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `kind` | string | No | -- | AnomalyKind filter; omit for total count |

### 6.2 Typed-Edge Tools (4 tools)

Source: `tools/definitions/typed_edges.rs`

#### `export_typed_edges_corpus`

| Parameter | Type | Required | Default | Range | Description |
|-----------|------|----------|---------|-------|-------------|
| `maxEdges` | integer | No | 10,000 | 1--1,000,000 | Upper bound on edges exported |
| `includeContent` | boolean | No | true | -- | Join source/target content text |
| `includeSourceMetadata` | boolean | No | true | -- | Join session_id and source_type |
| `includeMechanismType` | boolean | No | true | -- | Join mechanism_type for CausalChain edges |
| `joinLLMValidation` | boolean | No | true | -- | Attach LLMValidationSummary if available |
| `clearExisting` | boolean | No | false | -- | Wipe CF_TYPED_EDGE_RECORDS before writing |

#### `derive_anomalies_from_edges`

| Parameter | Type | Required | Default | Range | Description |
|-----------|------|----------|---------|-------|-------------|
| `highThreshold` | number | No | 0.60 | 0.0--1.0 | "High" embedder score threshold |
| `lowThreshold` | number | No | 0.30 | 0.0--1.0 | "Low" embedder score threshold |
| `minDisagreement` | number | No | 0.30 | 0.0--1.0 | Minimum high-low gap |
| `maxPairs` | integer | No | 10,000 | 1--1,000,000 | Hard cap on written pairs |
| `kinds` | array[string] | No | -- | -- | **NOT YET SUPPORTED** -- returns `InvalidParams` |

#### `validate_low_confidence_edges`

| Parameter | Type | Required | Default | Range | Description |
|-----------|------|----------|---------|-------|-------------|
| `weightCeiling` | number | No | 0.60 | 0.0--1.0 | Only edges with weight < ceiling are eligible |
| `maxEdges` | integer | No | 100 | 1--10,000 | Hard cap on edges validated per run |
| `sessionFilter` | string | No | -- | -- | Restrict to matching source session_id |

#### `list_typed_edge_records`

| Parameter | Type | Required | Default | Range | Description |
|-----------|------|----------|---------|-------|-------------|
| `limit` | integer | No | 100 | 1--1,000 | Max rows returned |
| `offset` | integer | No | 0 | >= 0 | Pagination offset |
| `includeFull` | boolean | No | false | -- | Hydrate full TypedEdgeTrainingRecord payload |

All 8 tools use `additionalProperties: false` to reject unknown parameters.

---

## 7. Storage Layer

### 7.1 Column Families

Source: `teleological/column_families.rs`

| CF Name | Constant | Phase | Key Format | Value Format |
|---------|----------|-------|------------|--------------|
| `contrastive_pairs` | `CF_CONTRASTIVE_PAIRS` | Phase 3 | `[anchor:16][neg:16]` = 32 bytes | `[version:1][bincode(ContrastivePair)]` |
| `contrastive_by_kind` | `CF_CONTRASTIVE_BY_KIND` | Phase 3 | `[kind:1][anchor:16][neg:16]` = 33 bytes | Empty (index-only) |
| `contrastive_by_anchor` | `CF_CONTRASTIVE_BY_ANCHOR` | Phase 3 | `[anchor:16]` = 16 bytes | `bincode(Vec<Uuid>)` |
| `typed_edge_records` | `CF_TYPED_EDGE_RECORDS` | Phase 4 F1 | `[source:16][target:16][edge_type:1]` = 33 bytes | `[version:1][bincode(TypedEdgeTrainingRecord)]` |
| `typed_edge_validations` | `CF_TYPED_EDGE_VALIDATIONS` | Phase 4 F4 | `[source:16][target:16][edge_type:1]` = 33 bytes | `[version:1][bincode(LLMEdgeValidation)]` |

### 7.2 Contrastive Storage Operations

Source: `rocksdb_store/contrastive.rs`

All writes are atomic via `WriteBatch`. CF handles use `.expect()` (fail-fast).

| Method | Signature | Description |
|--------|-----------|-------------|
| `store_contrastive_pair` | `(&self, pair: &ContrastivePair) -> CoreResult<()>` | Atomic write to 3 CFs; idempotent on composite key. Deduplication of `CF_CONTRASTIVE_BY_ANCHOR` is write-time skip + read-time dedup |
| `get_contrastive_pair` | `(&self, anchor: Uuid, negative: Uuid) -> CoreResult<Option<ContrastivePair>>` | Point lookup by primary key |
| `delete_contrastive_pair` | `(&self, anchor: Uuid, negative: Uuid) -> CoreResult<bool>` | Atomic delete from 3 CFs; returns `true` if existed |
| `list_contrastive_pair_keys` | `(&self) -> CoreResult<Vec<(Uuid, Uuid)>>` | Full scan of primary CF keys |
| `count_contrastive_pairs` | `(&self) -> CoreResult<usize>` | O(n) primary CF row count |
| `count_contrastive_pairs_by_kind` | `(&self, kind: AnomalyKind) -> CoreResult<usize>` | Prefix scan of `CF_CONTRASTIVE_BY_KIND` |
| `pairs_for_anchor` | `(&self, anchor: Uuid) -> CoreResult<Vec<Uuid>>` | Read + dedup of `CF_CONTRASTIVE_BY_ANCHOR` |
| `list_pairs_by_kind` | `(&self, kind: AnomalyKind, limit: usize) -> CoreResult<Vec<(Uuid, Uuid)>>` | Prefix scan by kind byte |
| `clear_all_contrastive_pairs` | `(&self) -> CoreResult<usize>` | Wipe all 3 CFs; returns primary-CF count |

#### Key Encoding Details

Primary key (`contrastive.rs:489-494`):
```
[anchor_uuid bytes: 16][negative_uuid bytes: 16] = 32 bytes total
```

By-kind secondary index key (`contrastive.rs:507-517`):
```
[kind_byte: 1][anchor_uuid bytes: 16][negative_uuid bytes: 16] = 33 bytes total
```

The kind byte is strictly increasing per `AnomalyKind::as_u8()` ordering, enabling prefix-scan by kind.

By-anchor secondary index (`contrastive.rs:519-535`):
- Key: `anchor_uuid` (16 bytes)
- Value: `bincode(Vec<Uuid>)` of negative UUIDs
- Append-on-write; duplicates deduped on read via `dedupe_uuids()` (`contrastive.rs:548-551`)

### 7.3 Typed-Edge Key Format

Source: `typed_edge_keys.rs`

Shared by both `CF_TYPED_EDGE_RECORDS` and `CF_TYPED_EDGE_VALIDATIONS`:

```
[source_uuid: 16 bytes][target_uuid: 16 bytes][edge_type: 1 byte] = 33 bytes
```

Functions:
- `typed_edge_record_key(source: Uuid, target: Uuid, edge_type: u8) -> [u8; 33]`
- `parse_typed_edge_record_key(key: &[u8]) -> Option<(Uuid, Uuid, u8)>` -- returns `None` on wrong length

The shared key module ensures byte-for-byte compatibility between the two CFs (`typed_edge_keys.rs:1-7`).

### 7.4 Typed-Edge Record Storage

Source: `rocksdb_store/typed_edge_export.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `store_typed_edge_record` | `(&self, record: &TypedEdgeTrainingRecord) -> CoreResult<()>` | Put by 33-byte composite key; idempotent |
| `get_typed_edge_record` | `(&self, source: Uuid, target: Uuid, edge_type: u8) -> CoreResult<Option<TypedEdgeTrainingRecord>>` | Point lookup |
| `list_typed_edge_record_keys` | `(&self) -> CoreResult<Vec<(Uuid, Uuid, u8)>>` | Full-scan key enumeration; malformed keys trigger `SerializationError` |
| `count_typed_edge_records` | `(&self) -> CoreResult<usize>` | O(n) row count |
| `clear_all_typed_edge_records` | `(&self) -> CoreResult<usize>` | Batch delete; returns count |

### 7.5 LLM Validation Storage

Source: `rocksdb_store/llm_validation.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `store_llm_edge_validation` | `(&self, source: Uuid, target: Uuid, edge_type: u8, validation: &LLMEdgeValidation) -> CoreResult<()>` | Put by 33-byte key; idempotent |
| `get_llm_edge_validation` | `(&self, source: Uuid, target: Uuid, edge_type: u8) -> CoreResult<Option<LLMEdgeValidation>>` | Point lookup |
| `list_llm_edge_validation_keys` | `(&self) -> CoreResult<Vec<(Uuid, Uuid, u8)>>` | Full-scan key enumeration |
| `count_llm_edge_validations` | `(&self) -> CoreResult<usize>` | O(n) row count |
| `clear_all_llm_edge_validations` | `(&self) -> CoreResult<usize>` | Batch delete; returns count |

### 7.6 Anomaly Derivation

Source: `rocksdb_store/anomaly_derivation.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `derive_anomalies_from_edges` | `(&self, edge_repository: &EdgeRepository, cfg: &AnomalyDerivationConfig) -> CoreResult<AnomalyDerivationSummary>` | Walk edges, classify, write pairs via `store_contrastive_pair` |

Pure classifier: `classify_edge_as_anomaly(edge: &TypedEdge, cfg: &AnomalyDerivationConfig) -> Option<(AnomalyKind, Vec<u8>, Vec<u8>, f32)>` (returns `None` when no pattern matches).

---

## 8. Versioned Bincode Encoding

Source: `rocksdb_store/versioned_bincode.rs`

Shared wire format for `CF_TYPED_EDGE_RECORDS` and `CF_TYPED_EDGE_VALIDATIONS`. Two generic functions:

### `encode_versioned<T: Serialize>`

```rust
pub(super) fn encode_versioned<T: Serialize>(
    value: &T,
    version: u8,
    type_name: &'static str,
) -> CoreResult<Vec<u8>>
```

Produces `[version: u8][bincode(value)]`. Errors include `type_name` for self-describing failure messages.

### `decode_versioned<T: DeserializeOwned>`

```rust
pub(super) fn decode_versioned<T: DeserializeOwned>(
    bytes: &[u8],
    expected_version: u8,
    type_name: &'static str,
    retry_hint: &'static str,
) -> CoreResult<T>
```

Rejects:
- Empty payloads: `"{type_name} payload is empty (missing version byte)"`
- Version mismatch: `"{type_name} version mismatch: got {v}, expected {expected}. No automatic migration is supported -- {retry_hint}."`
- Bincode decode failure: `"bincode deserialize {type_name}: {error}"`

The contrastive pair CF uses its own encode/decode (`rocksdb_store/contrastive.rs:458-487`) with identical logic but not the shared module (predates versioned_bincode.rs). Both produce the same `[version][bincode]` wire layout.

---

## 9. Error Handling

### Contrastive Errors

| Error Type | Source | When |
|------------|--------|------|
| `ContrastiveError::InvalidDisagreement(f32)` | `types.rs:236` | `min_disagreement` outside `[0, 1]` |
| `ContrastiveError::InvalidThresholds(f32, f32)` | `types.rs:240` | `low >= high` or non-finite |
| `ContrastiveError::Storage(String)` | `types.rs:245` | Backend failure (formatted) |
| `CoreError::SerializationError` | `contrastive.rs:462,478,484` | Bincode encode/decode failure; version mismatch; empty payload |
| `TeleologicalStoreError::rocksdb_op` | `contrastive.rs` (multiple) | RocksDB I/O failure; carries operation name + CF name + key context |

### Tucker Errors

| Error Type | Source | When |
|------------|--------|------|
| `TuckerError::InvalidRank` | `tucker.rs:108` | Rank outside `1..=dim` for any mode |
| `TuckerError::EmptyFingerprint` | `tucker.rs:127` | Total squared energy < `1e-24` |
| `TuckerError::Numerical(String)` | `tucker.rs:131` | Jacobi non-convergence; power iteration non-convergence; non-finite norm |

### Typed-Edge Errors

| Error Type | Source | When |
|------------|--------|------|
| `CoreError::SerializationError` | `versioned_bincode.rs` | Version mismatch; empty payload; bincode failure |
| `CoreError::SerializationError` | `typed_edge_export.rs:151-155` | Malformed key length in CF iteration |
| `TeleologicalStoreError::rocksdb_op` | `typed_edge_export.rs`, `llm_validation.rs` | RocksDB I/O failure |
| `CoreError::StorageError` | `anomaly_derivation.rs:97` | `iter_all_typed_edges` failure |

All MCP handlers return structured JSON errors via `self.tool_error(id, message)`. The `validate_low_confidence_edges` handler returns `{status:"skipped"}` (not an error) when the LLM is unavailable or not loaded.

---

## 10. Cross-References

| Topic | Document |
|-------|----------|
| 13-embedder architecture (E1-E13 dims, SRC-3 convention) | `docs2/contextgraphspecs/05_embedding_subsystem.md` |
| Typed edges, `GraphLinkEdgeType`, `TypedEdge`, `EdgeRepository` | `docs2/contextgraphspecs/07_graph_and_causal.md` |
| RocksDB column family configuration, CF options | `docs2/contextgraphspecs/09_storage_layer.md` |
| MCP tool dispatch, JSON-RPC protocol, `Handlers` struct | `docs2/contextgraphspecs/08_mcp_tools_reference.md` |
| `SemanticFingerprint` fields, asymmetric dual-vector layout | `docs2/contextgraphspecs/05_embedding_subsystem.md` |
| Teleological subsystem, `TeleologicalFingerprint`, `CF_TYPED_EDGES` | `docs2/contextgraphspecs/13_teleological_subsystem.md` |
| LLM graph-agent (`Hermes-2-Pro-Mistral-7B`), `analyze_relationship` | `docs2/contextgraphspecs/07_graph_and_causal.md` |
| `SourceMetadata`, session filtering | `docs2/contextgraphspecs/04_database_schema.md` |
| Jaccard similarity (`jaccard_similarity`) | `crates/context-graph-core/src/similarity/` |
| `NUM_EMBEDDERS = 13` | `crates/context-graph-core/src/teleological/types.rs:217` |
| `GraphLinkEdgeType` (8 variants, 0..=7) | `crates/context-graph-core/src/graph_linking/edge_type.rs:48-81` |
