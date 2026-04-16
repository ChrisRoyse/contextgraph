# 16. Training Data Export Subsystem

## Source Files Covered

| Crate | File | Role |
|---|---|---|
| `context-graph-core` | `src/training/mod.rs` | `TrainingRecord`, `TrainingEdge`, `KnnNeighbor`, `CausalLabel`, `TopicMembership` structs; constants; version byte |
| `context-graph-core` | `src/training/computation.rs` | `topic_profile_or_fallback`, `compute_cross_correlations`, `compute_group_alignments` |
| `context-graph-core` | `src/training/temporal.rs` | `extract_temporal_labels`, `PeriodicBucket`, `TemporalLabels` |
| `context-graph-core` | `src/training/edge_signature.rs` | `compute_edge_type_distribution` |
| `context-graph-core` | `src/teleological/groups.rs` | `GroupAlignments`, `GroupType`, `group_indices`, `weighted_group_average` |
| `context-graph-mcp` | `src/handlers/tools/training_tools.rs` | MCP tool handlers for all 4 training tools |
| `context-graph-mcp` | `src/tools/definitions/training.rs` | JSON Schema definitions for 4 MCP tools |
| `context-graph-storage` | `src/teleological/rocksdb_store/export.rs` | `store_training_record`, `get_training_record`, `list_training_record_ids`, `count_training_records`, `clear_all_training_records`, `multi_get_training_records`, `get_topic_profile`; `encode_training_record`, `decode_training_record` |
| `context-graph-cli` | `src/commands/export_training.rs` | `ExportTrainingArgs`, `ExportSummary`, Parquet file export |

---

## 1. Purpose

The training data export subsystem serializes every stored memory into a self-contained `TrainingRecord` that bundles identity, all 13 embedding vectors, derived fusion features (topic profile, cross-correlations, group alignments), graph structure (typed edges, K-NN neighbors), LLM-discovered causal labels, temporal labels, and an optional Tucker-core decomposition. Records are persisted to `CF_TRAINING_RECORDS` in RocksDB via bincode with a version-byte prefix. A separate CLI command exports those records to Parquet for external consumption.

---

## 2. Constants

All defined in `crates/context-graph-core/src/training/mod.rs` unless noted.

| Constant | Value | Source | Description |
|---|---|---|---|
| `TRAINING_RECORD_VERSION` | `2` | `mod.rs:67` | Version byte prepended to every bincode-encoded record on disk. v2 adds `edge_type_distribution`. |
| `NUM_CROSS_CORRELATIONS` | `78` | `mod.rs:70` | C(13,2) = 78 unique pairs |
| `NUM_GROUP_ALIGNMENTS` | `6` | `mod.rs:74` | Factual, Temporal, Causal, Relational, Qualitative, Implementation |
| `NUM_EMBEDDERS` | `13` | `types.rs:217` | Number of embedder slots in the system |
| `NUM_EDGE_TYPE_DISTRIBUTION` | `8` | `edge_signature.rs:12` | Equals `GraphLinkEdgeType::COUNT`; one slot per edge type |
| `SYNERGY_DIM` | `13` | `synergy_matrix/constants.rs:10` | Dimension of the synergy matrix (same as `NUM_EMBEDDERS`) |
| `NUM_GROUPS` | `6` | `groups.rs:18` | Number of hierarchical embedding groups |
| `DEFAULT_MAX_MEMORIES` | `10_000` | `training_tools.rs:57` | Default cap on memories per export call |
| `HARD_MAX_MEMORIES` | `1_000_000` | `training_tools.rs:58` | Absolute ceiling for `maxMemories` parameter |
| `DEFAULT_RANKS` | `(4, 4, 128)` | `tucker.rs:77` | Tucker-1 decomposition ranks (rank_1, rank_2, rank_3) |

---

## 3. Data Structures

### 3.1 TrainingRecord

Defined in `training/mod.rs:80-172`. Serialized with bincode (positional layout). New fields MUST be appended at the end.

| Field | Type | Description |
|---|---|---|
| `memory_id` | `Uuid` | Primary key |
| `content` | `String` | Memory text content |
| `importance` | `f32` | Clamped [0.0, 1.0] importance score |
| `created_at` | `DateTime<Utc>` | Fingerprint creation timestamp |
| `session_id` | `Option<String>` | Source session ID from `SourceMetadata` |
| `source_type` | `Option<String>` | Debug-formatted `SourceType` enum |
| `source_path` | `Option<String>` | File path from `SourceMetadata` |
| `content_hash` | `Option<[u8; 32]>` | SHA-256 of source content from fingerprint |
| `e1_semantic` | `Vec<f32>` | 1024D E1 semantic vector |
| `e2_temporal_recent` | `Vec<f32>` | 512D E2 temporal recency vector |
| `e3_temporal_periodic` | `Vec<f32>` | 512D E3 temporal periodic vector |
| `e4_temporal_positional` | `Vec<f32>` | 512D E4 temporal positional vector |
| `e5_causal_cause` | `Vec<f32>` | 768D E5 asymmetric cause vector |
| `e5_causal_effect` | `Vec<f32>` | 768D E5 asymmetric effect vector |
| `e7_code` | `Vec<f32>` | 1536D E7 code vector |
| `e8_graph_source` | `Vec<f32>` | 1024D E8 asymmetric source vector |
| `e8_graph_target` | `Vec<f32>` | 1024D E8 asymmetric target vector |
| `e9_hdc` | `Vec<f32>` | 1024D E9 HDC vector |
| `e10_paraphrase` | `Vec<f32>` | 768D E10 asymmetric paraphrase vector |
| `e10_context` | `Vec<f32>` | 768D E10 asymmetric context vector |
| `e11_entity` | `Vec<f32>` | 768D E11 entity vector |
| `e6_sparse_indices` | `Vec<u16>` | E6 sparse vector indices |
| `e6_sparse_values` | `Vec<f32>` | E6 sparse vector values |
| `e13_splade_indices` | `Vec<u16>` | E13 SPLADE sparse indices |
| `e13_splade_values` | `Vec<f32>` | E13 SPLADE sparse values |
| `e12_token_embeddings` | `Vec<Vec<f32>>` | E12 ColBERT: variable tokens x 128D per token |
| `topic_profile` | `[f32; 13]` | 13D per-embedder alignment scores |
| `cross_correlations` | `Vec<f32>` | 78 synergy-weighted pairwise interactions |
| `group_alignments` | `[f32; 6]` | 6D group alignment: Factual/Temporal/Causal/Relational/Qualitative/Implementation |
| `outgoing_edges` | `Vec<TrainingEdge>` | Typed edges where this memory is source |
| `incoming_edges` | `Vec<TrainingEdge>` | Typed edges where this memory is target |
| `knn_neighbors` | `Vec<Vec<KnnNeighbor>>` | K-NN neighbors; always 13 outer vecs (one per embedder) |
| `causal_effects` | `Vec<CausalLabel>` | LLM-discovered causal relationships (this = cause) |
| `causal_causes` | `Vec<CausalLabel>` | LLM-discovered causal relationships (this = effect). Always empty in current export. |
| `topic_memberships` | `Vec<TopicMembership>` | Cluster memberships. Always empty in current export. |
| `temporal_labels` | `Option<TemporalLabels>` | Phase-5 temporal features. `None` when `includeTemporalLabels=false`. |
| `tucker_core` | `Option<TuckerCore>` | Phase-4 Tucker-1 decomposition. `None` unless `includeTuckerCore=true`. |
| `edge_type_distribution` | `[u32; 8]` | v2 field: per-`GraphLinkEdgeType` count of outgoing edges |

Dense embedding vectors are empty `Vec` when `includeEmbeddings=false`. Sparse vectors are empty when `includeSparseVectors=false`. Token embeddings are empty when `includeTokenEmbeddings=false`.

### 3.2 TrainingEdge

Defined in `training/mod.rs:178-188`.

| Field | Type | Description |
|---|---|---|
| `edge_type` | `u8` | `GraphLinkEdgeType` repr. 0=SemanticSimilar through 7=MultiAgreement |
| `peer_id` | `Uuid` | Target for outgoing edges; source for incoming edges |
| `weight` | `f32` | Edge weight |
| `direction` | `u8` | `DirectedRelation` repr: 0=Symmetric, 1=Forward, 2=Backward |
| `agreement_count` | `u8` | Number of embedders that agree on this edge |
| `embedder_scores` | `[f32; 13]` | Per-embedder similarity scores for this edge |

### 3.3 KnnNeighbor

Defined in `training/mod.rs:191-195`.

| Field | Type | Description |
|---|---|---|
| `target_id` | `Uuid` | Neighbor memory UUID |
| `similarity` | `f32` | Similarity score in the embedder's space |

### 3.4 CausalLabel

Defined in `training/mod.rs:204-221`.

| Field | Type | Description |
|---|---|---|
| `related_memory_id` | `Uuid` | Peer end of relationship. `Uuid::nil()` when peer is unresolvable. |
| `rel_id` | `Uuid` | `CausalRelationship.id` for joining back against `CF_CAUSAL_RELATIONSHIPS` |
| `description` | `String` | Concatenation: `"{cause_statement} -> {effect_statement}. {explanation}"` |
| `direction` | `String` | `"cause"` when this memory produces the other; `"effect"` otherwise |
| `confidence` | `f32` | LLM confidence score |
| `mechanism_type` | `Option<String>` | Classification: `"direct"` / `"mediated"` / `"feedback"` / `"temporal"` |

### 3.5 TopicMembership

Defined in `training/mod.rs:224-230`.

| Field | Type | Description |
|---|---|---|
| `topic_id` | `String` | Topic identifier |
| `topic_label` | `Option<String>` | Human-readable topic label |
| `topic_profile` | `[f32; 13]` | The topic's 13D alignment profile |
| `membership_probability` | `f32` | Probability of belonging to this topic |

### 3.6 TemporalLabels

Defined in `training/temporal.rs:59-85`.

| Field | Type | Description |
|---|---|---|
| `stored_at` | `DateTime<Utc>` | UTC wall-clock time of memory creation |
| `stored_hour_utc` | `u8` | 0..23 |
| `stored_day_of_week` | `u8` | 1=Monday, 7=Sunday (ISO weekday) |
| `stored_month` | `u8` | 1..12 |
| `age_seconds_at_export` | `i64` | `export_now - stored_at` in seconds |
| `session_sequence` | `Option<u32>` | Position within the creating session |
| `session_total` | `Option<u32>` | Total memories in that session |
| `relative_position` | `Option<f32>` | `session_sequence / session_total` in [0, 1] |
| `periodic_bucket` | `PeriodicBucket` | Coarse weekday/weekend x time-of-day bucket |
| `e2_recency_norm` | `f32` | L2 norm of E2 temporal-recent vector (0.0 when empty) |
| `e3_periodic_norm` | `f32` | L2 norm of E3 temporal-periodic vector |
| `e4_positional_norm` | `f32` | L2 norm of E4 temporal-positional vector |

### 3.7 PeriodicBucket

Defined in `training/temporal.rs:23-33`. An enum with 8 variants.

| Variant | Weekday/Weekend | Hour Range (UTC) |
|---|---|---|
| `WeekdayMorning` | Weekday (Mon-Fri) | 05..11 |
| `WeekdayAfternoon` | Weekday | 12..16 |
| `WeekdayEvening` | Weekday | 17..20 |
| `WeekdayNight` | Weekday | 21..04 |
| `WeekendMorning` | Weekend (Sat-Sun) | 05..11 |
| `WeekendAfternoon` | Weekend | 12..16 |
| `WeekendEvening` | Weekend | 17..20 |
| `WeekendNight` | Weekend | 21..04 |

Derived by `PeriodicBucket::from_dow_hour(dow, hour)` at `temporal.rs:38-50`. Day-of-week uses `chrono::Datelike::iso_weekday().number_from_monday()`: Saturday=6, Sunday=7 are weekend.

### 3.8 GraphLinkEdgeType (Referenced)

Defined in `graph_linking/edge_type.rs:48-81`. 8 variants at `repr(u8)`.

| Value | Variant | Primary Embedder | Asymmetric |
|---|---|---|---|
| 0 | `SemanticSimilar` | E1 (idx 0) | No |
| 1 | `CodeRelated` | E7 (idx 6) | No |
| 2 | `EntityShared` | E11 (idx 10) | No |
| 3 | `CausalChain` | E5 (idx 4) | Yes |
| 4 | `GraphConnected` | E8 (idx 7) | Yes |
| 5 | `ParaphraseAligned` | E10 (idx 9) | No |
| 6 | `KeywordOverlap` | E6 (idx 5) | No |
| 7 | `MultiAgreement` | None (3+ embedders) | No |

### 3.9 TuckerCore (Referenced)

Defined in `teleological/types.rs:73-89`.

| Field | Type | Description |
|---|---|---|
| `ranks` | `(usize, usize, usize)` | Core tensor dimensions. Default `(4, 4, 128)`. |
| `data` | `Vec<f32>` | Flattened core tensor. Size: `rank_1 * rank_2 * rank_3`. |
| `u1` | `Vec<f32>` | Factor matrix U1: `SYNERGY_DIM x rank_1` (13 x 4) |
| `u2` | `Vec<f32>` | Factor matrix U2: `SYNERGY_DIM x rank_2` (13 x 4) |
| `u3` | `Vec<f32>` | Factor matrix U3: `1024 x rank_3` (1024 x 128) |

### 3.10 ExportSummary (CLI)

Defined in `export_training.rs:74-84`.

| Field | Type | Description |
|---|---|---|
| `total_records` | `usize` | Records written to Parquet |
| `bytes_written` | `u64` | On-disk file size in bytes |
| `row_groups` | `usize` | Parquet row groups (one per batch) |
| `elapsed_ms` | `u64` | Wall-clock time for the full export |

---

## 4. Algorithms

### 4.1 Topic Profile Resolution (`topic_profile_or_fallback`)

Source: `computation.rs:41-63`.

Resolves the 13D topic profile for a memory. Two paths:

**Preferred path** -- when a stored profile exists in `CF_TOPIC_PROFILES`, return it directly.

**Fallback path** -- when no stored profile exists, derive a presence indicator per embedder:

1. For each dense embedder slot (E1/E2/E3/E4/E5_cause/E7/E8_source/E9/E10_paraphrase/E11): compute `L2_norm(vector).clamp(0.0, 1.0)`. A unit-normalized vector yields ~1.0; empty yields 0.0.
2. For each sparse embedder slot (E6/E13): compute `sqrt(sum(v_i^2)).clamp(0.0, 1.0)`.
3. For token-level (E12): compute mean of per-token dense alignments, excluding zero-alignment tokens, clamped to [0.0, 1.0].

Asymmetric embedders use the cause/source/paraphrase side only.

The fallback is NOT a real topical alignment; it is a magnitude-presence indicator.

### 4.2 Cross-Correlations (`compute_cross_correlations`)

Source: `computation.rs:118-133`.

Produces 78 values (C(13,2) unique pairs). For each pair (i, j) where i < j in 0..13:

```
cross_corr[k] = clamp(alignment_i, 0, 1) * clamp(alignment_j, 0, 1) * clamp(synergy(i, j), 0, 1)
```

where:
- `alignment_i` = `topic_profile[i]`
- `synergy(i, j)` = the value from the 13x13 `SynergyMatrix` via `synergy.get_synergy(i, j)`

Pair ordering: `(0,1), (0,2), ..., (0,12), (1,2), ..., (11,12)`. All values in [0, 1].

Steps:
1. Initialize output vector with capacity 78.
2. Iterate i from 0 to 12.
3. Iterate j from (i+1) to 12.
4. Clamp `topic_profile[i]` and `topic_profile[j]` to [0, 1].
5. Clamp `synergy.get_synergy(i, j)` to [0, 1].
6. Push the product of the three clamped values.
7. Debug-assert output length equals `NUM_CROSS_CORRELATIONS` (78).

### 4.3 Group Alignments (`compute_group_alignments`)

Source: `computation.rs:141-145` delegates to `GroupAlignments::from_alignments` in `groups.rs:74-89`.

Produces a 6D vector by computing weighted average within each group. Default weights are uniform (1.0 per embedder).

**Group Definitions** (from `groups.rs:51-64`):

| Group | Index | Embedder Slots | Description |
|---|---|---|---|
| Factual | 0 | E1(0), E12(11), E13(12) | What IS |
| Temporal | 1 | E2(1), E3(2) | When/sequence |
| Causal | 2 | E4(3), E7(6) | Why/how |
| Relational | 3 | E5(4), E8(7), E9(8) | Like/where/who |
| Qualitative | 4 | E10(9), E11(10) | Feel/principle |
| Implementation | 5 | E6(5) | Code |

**Formula** (`weighted_group_average` at `groups.rs:244-262`):

```
group_alignment[g] = sum(alignments[idx] * weights[idx] for idx in group_indices[g])
                     / sum(weights[idx] for idx in group_indices[g])
```

Returns 0.0 when the weight sum is below `f32::EPSILON`.

### 4.4 Temporal Labels (`extract_temporal_labels`)

Source: `temporal.rs:101-138`.

Steps:
1. Read `stored_at` from `fingerprint.created_at`.
2. Extract `hour` (0-23), `dow` (1-7 ISO weekday), `month` (1-12) from the UTC representation.
3. Compute `periodic_bucket` via `PeriodicBucket::from_dow_hour(dow, hour)`.
4. Compute `age_seconds = (export_now - stored_at).num_seconds()`.
5. Compute `relative_position = session_sequence / session_total` when both are `Some` and `total > 0`; otherwise `None`.
6. Compute L2 norms of E2, E3, E4 vectors from the fingerprint's semantic data.
7. Return the assembled `TemporalLabels` struct.

The `export_now` parameter is a single `Utc::now()` snapshot shared across the entire export run for consistent age computation.

### 4.5 Edge Type Distribution (`compute_edge_type_distribution`)

Source: `edge_signature.rs:19-34`.

Steps:
1. Initialize `[0u32; 8]` distribution array.
2. For each edge in the iterator: read `edge.edge_type().as_u8()` as index; saturating-add 1 to that slot.
3. Return the array.

Saturates at `u32::MAX` per slot. The sum of the array equals `outgoing_edges.len()` modulo saturation. In the MCP handler (`training_tools.rs:513-520`), an equivalent inline loop derives the same distribution from the `TrainingEdge.edge_type` field.

### 4.6 Incoming Edge Reverse Index

Source: `training_tools.rs:170-189`, `build_incoming_index` at line 735.

When `includeIncomingEdges=true`, the handler builds a `HashMap<Uuid, Vec<TrainingEdge>>` from a single O(N) sweep over all typed edges (`iter_all_typed_edges`). For each edge, the target UUID maps to a `TrainingEdge` whose `peer_id` is set to the edge's source. This is a one-time cost; per-memory lookup is O(1).

### 4.7 Tucker-Core Decomposition

Source: `training_tools.rs:498-508`.

When `includeTuckerCore=true`, `CpuTuckerCompressor.compress(&fp.semantic, DEFAULT_RANKS)` runs a streaming HOSVD on the 13x13x1024 embedding interaction tensor at ranks (4, 4, 128). CPU-only. Adds ~18KB per record and a few ms of compute. Failures are logged and the field stays `None`; the record still persists.

---

## 5. MCP Tools

All 4 tools are defined in `tools/definitions/training.rs:14-175` and handled in `handlers/tools/training_tools.rs`.

### 5.1 `export_training_corpus`

Iterates stored memories, assembles `TrainingRecord` per memory, persists to `CF_TRAINING_RECORDS`.

**Parameters:**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `filter` | `string` enum `["all","session"]` | No | `"all"` | `"all"` scans every non-soft-deleted fingerprint. `"session"` requires `filterId`. |
| `filterId` | `string` | No | -- | Session ID when `filter="session"`. Error if `filter="session"` and empty/missing. |
| `maxMemories` | `integer` | No | `10000` | Upper bound on memories to process. Clamped to [1, 1000000]. |
| `includeEmbeddings` | `boolean` | No | `true` | Include dense vectors (E1-E11). ~46KB/record when true. |
| `includeSparseVectors` | `boolean` | No | `true` | Include E6 and E13 sparse index/value pairs. |
| `includeTokenEmbeddings` | `boolean` | No | `false` | Include E12 ColBERT token embeddings. ~15KB/record. |
| `includeEdges` | `boolean` | No | `true` | Include outgoing typed edges and per-embedder K-NN neighbors. |
| `includeCausalLabels` | `boolean` | No | `true` | Include LLM-discovered causal relationships (effects direction). |
| `includeIncomingEdges` | `boolean` | No | `false` | Include typed edges where this memory is target. O(N) reverse-index scan at start. |
| `includeTemporalLabels` | `boolean` | No | `true` | Include Phase-5 temporal labels. Cheap. |
| `includeTuckerCore` | `boolean` | No | `false` | Phase-4 Tucker-1 decomposition. CPU HOSVD, adds ~18KB/record. |
| `clearExisting` | `boolean` | No | `false` | Delete all existing records in CF_TRAINING_RECORDS before exporting. |

**Response shape** (from `training_tools.rs:283-317`):

```json
{
  "status": "exported",
  "exported": <int>,
  "skipped_filtered": <int>,
  "errors": <int>,
  "cleared_before_export": <int>,
  "fingerprints_scanned": <int>,
  "approx_bytes_written": <int>,
  "elapsed_ms": <int>,
  "storage": { "backend": "rocksdb", "column_family": "training_records", "format": "version_byte + bincode" },
  "record_shape": { ... },
  "filter": { "mode": "...", "filter_id": "..." }
}
```

**Processing flow** (`training_tools.rs:60-318`):

1. Parse all parameters with defaults.
2. Validate: `filter="session"` requires non-empty `filterId`.
3. Downcast store to `RocksDbTeleologicalStore`. Error if not RocksDB backend.
4. If `clearExisting=true`, call `clear_all_training_records()`.
5. Call `list_fingerprints_unbiased(maxMemories)` to gather fingerprints.
6. If `includeIncomingEdges && includeEdges`, build reverse index via `iter_all_typed_edges` (one O(N) scan).
7. Capture `export_now = Utc::now()` for temporal label consistency.
8. For each fingerprint:
   a. Fetch `source_metadata` once.
   b. If `filter="session"`, check `source_metadata.session_id` matches `filterId`; skip if not.
   c. Read `CF_TOPIC_PROFILES` for this memory; fall back to magnitude derivation if missing.
   d. Call `build_training_record(...)` to assemble the record.
   e. Persist via `store_training_record(fp.id, &record)`.
   f. Increment counters.
9. Return JSON summary.

### 5.2 `list_training_records`

Returns UUIDs stored in `CF_TRAINING_RECORDS` with optional per-record shape statistics.

**Parameters:**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `limit` | `integer` | No | `50` | Max UUIDs to return. Clamped to [1, 1000]. |
| `offset` | `integer` | No | `0` | Skip this many UUIDs (pagination). |
| `includeShape` | `boolean` | No | `true` | Include per-record shape summary. Uses `multi_get_training_records` for batched reads. |

**Response shape:**

```json
{
  "total": <int>,
  "offset": <int>,
  "limit": <int>,
  "returned": <int>,
  "records": [ { "memory_id": "...", "content_chars": <int>, ... } ]
}
```

When `includeShape=true`, each record item includes: `content_chars`, `importance`, `session_id`, `source_type`, `topic_profile_len`, `cross_correlations_len`, `group_alignments_len`, `e1_len`, `e5_cause_len`, `e7_len`, `e8_source_len`, `e11_len`, `e6_sparse_nnz`, `e13_sparse_nnz`, `e12_token_count`, `outgoing_edges`, `incoming_edges`, `knn_embedders_populated`, `causal_effects`, `causal_causes`, `temporal_labels_present`.

### 5.3 `get_training_record`

Fetch a single training record by UUID.

**Parameters:**

| Name | Type | Required | Default | Description |
|---|---|---|---|---|
| `memoryId` | `string` (uuid) | **Yes** | -- | UUID of the memory |
| `includeVectors` | `boolean` | No | `false` | Include raw dense/sparse embedding arrays and cross-correlations |
| `includeTokenEmbeddings` | `boolean` | No | `false` | Include E12 token embeddings |
| `includeEdges` | `boolean` | No | `true` | Include outgoing/incoming edges, K-NN, causal labels detail |

Always returns: `memory_id`, `found`, `content`, `importance`, `created_at`, `session_id`, `source_type`, `source_path`, `content_hash` (hex), `topic_profile`, `cross_correlations_len`, `group_alignments`, `temporal_labels_present`, `tucker_core_present`, and a `shape` object with dimension counts for all vectors.

### 5.4 `count_training_records`

Returns the current number of rows in `CF_TRAINING_RECORDS`. No parameters. O(N) count via CF iterator.

**Response:**

```json
{ "count": <int> }
```

---

## 6. CLI Export Command

Source: `export_training.rs`.

### 6.1 Command Flags (`ExportTrainingArgs`)

| Flag | Type | Default | Env Var | Description |
|---|---|---|---|---|
| `--kind` | `string` | `"training-corpus"` | -- | Export target. Only `"training-corpus"` is supported. |
| `--format` | `string` | `"parquet"` | -- | Output format. Only `"parquet"` is supported. |
| `--out` | `PathBuf` | -- (required) | -- | Absolute path for the output Parquet file. Parent directory must exist. |
| `--storage` | `PathBuf` | -- (required) | `CONTEXT_GRAPH_STORAGE_PATH` | Path to the RocksDB data directory. |
| `--batch-size` | `usize` | `500` | -- | Records per batch/row group. Larger = less Parquet overhead, more RAM. |
| `--limit` | `Option<usize>` | None (no cap) | -- | Cap on total records exported. |

### 6.2 Parquet Schema

Two columns:

| Column | Arrow Type | Nullable | Description |
|---|---|---|---|
| `memory_id` | `Utf8` | No | UUID string |
| `record_bytes` | `Binary` | No | Byte-for-byte copy of `[version_byte][bincode]` payload |

Compression: Zstd (default level). One row group per batch (`ArrowWriter::flush()` called after each batch write).

### 6.3 Processing Flow

1. Validate `--kind`, `--format`, `--batch-size > 0`, parent directory of `--out` exists, `--storage` exists.
2. Open `RocksDbTeleologicalStore` at `--storage`.
3. List all training record IDs. Apply `--limit` truncation.
4. Open Parquet file writer with Zstd compression.
5. For each batch of `batch-size` IDs:
   a. `multi_get_training_records(batch)` -- batched RocksDB read.
   b. For each successfully decoded record: re-encode via `encode_training_record` (byte-for-byte identical to RocksDB), add to Arrow arrays.
   c. Missing/undecodable records: log warning, skip.
   d. Write `RecordBatch`, flush (creates one row group per batch).
6. Close writer (finalizes Parquet footer).
7. On any error: remove partially-written file, return error.
8. Return `ExportSummary` and print JSON to stdout.

### 6.4 Exit Code

`summary_to_exit_code` at `export_training.rs:289-314`: returns 0 on success, 1 on failure.

---

## 7. Storage Layer

### 7.1 Column Family

| CF Name | Constant | Source |
|---|---|---|
| `training_records` | `CF_TRAINING_RECORDS` | `column_families.rs:399` |
| `topic_profiles` | `CF_TOPIC_PROFILES` | `column_families.rs:76` |

### 7.2 Key Format

Both column families use 16-byte raw UUID as key: `id.as_bytes()` (big-endian byte order from `uuid::Uuid`).

### 7.3 Value Format

**CF_TRAINING_RECORDS:**

```
[version: u8][bincode-encoded TrainingRecord]
```

- Byte 0: `TRAINING_RECORD_VERSION` (currently `2`).
- Bytes 1..N: bincode serialization of `TrainingRecord` (positional format; no `skip_serializing_if`).

**CF_TOPIC_PROFILES:**

52 bytes = 13 x `f32` in little-endian format. Decoded by reading 4-byte chunks via `f32::from_le_bytes`.

### 7.4 Encoding/Decoding

Both functions are pub and available for downstream use (CLI Parquet export, tests).

**`encode_training_record`** (`export.rs:284-292`):
1. `bincode::serialize(record)` -- positional format.
2. Prepend `TRAINING_RECORD_VERSION` byte.
3. Return `Vec<u8>`.

**`decode_training_record`** (`export.rs:298-315`):
1. Reject empty payload.
2. Check `bytes[0]` equals `TRAINING_RECORD_VERSION`. Reject on mismatch with explicit error message: "No automatic migration is supported -- re-run export_training_corpus."
3. `bincode::deserialize(&bytes[1..])`.

### 7.5 Storage Operations

All methods on `RocksDbTeleologicalStore` in `export.rs`:

| Method | Signature | Description |
|---|---|---|
| `store_training_record` | `async fn(&self, id: Uuid, record: &TrainingRecord) -> CoreResult<()>` | Idempotent put. Re-exports overwrite. |
| `get_training_record` | `async fn(&self, id: Uuid) -> CoreResult<Option<TrainingRecord>>` | Returns `None` if not stored. Error on version mismatch. |
| `list_training_record_ids` | `async fn(&self) -> CoreResult<Vec<Uuid>>` | O(N) iterator scan. Skips non-16-byte keys. |
| `count_training_records` | `async fn(&self) -> CoreResult<usize>` | O(N) iterator count. |
| `delete_training_record` | `async fn(&self, id: Uuid) -> CoreResult<bool>` | Returns true if record existed. |
| `clear_all_training_records` | `async fn(&self) -> CoreResult<usize>` | Delete all. Returns count deleted. |
| `multi_get_training_records` | `async fn(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<TrainingRecord>>>` | Batched fetch. ~30x faster than sequential reads. Undecodable entries return `None`. |
| `get_topic_profile` | `async fn(&self, id: Uuid) -> CoreResult<Option<[f32; 13]>>` | Read from `CF_TOPIC_PROFILES`. Rejects rows not 52 bytes. |

---

## 8. Error Handling

| Error | Source | Trigger |
|---|---|---|
| `CoreError::SerializationError("...empty...")` | `decode_training_record` | Empty payload (0 bytes) |
| `CoreError::SerializationError("...version mismatch...")` | `decode_training_record` | Version byte != `TRAINING_RECORD_VERSION` |
| `CoreError::SerializationError("bincode serialize...")` | `encode_training_record` | Bincode serialization failure |
| `CoreError::SerializationError("bincode deserialize...")` | `decode_training_record` | Bincode deserialization failure |
| `CoreError::SerializationError("CF_TOPIC_PROFILES row...expected 52")` | `get_topic_profile` | Topic profile row is not 52 bytes |
| `TeleologicalStoreError::rocksdb_op(...)` | All storage methods | RocksDB read/write/iterate failures. Propagated with operation name, CF name, and key UUID. |
| MCP tool error | `call_export_training_corpus` | `filter="session"` with empty/missing `filterId` |
| MCP tool error | All MCP handlers | Store is not `RocksDbTeleologicalStore` |
| `anyhow::Error` | CLI `run()` | Missing parent directory, missing storage path, unsupported kind/format, batch_size=0 |

The handler logs per-record build failures at `warn` level and continues (`errors` counter incremented). Store failures on individual records also log and continue. The export never aborts on single-record failures.

The CLI Parquet export removes partially-written files on any failure.

---

## 9. Integration Points

### 9.1 Teleological Store

- `list_fingerprints_unbiased(max)` -- enumerates non-soft-deleted fingerprints up to the given limit.
- `get_content(id)` -- fetches memory text content.
- `get_source_metadata(id)` -- fetches `SourceMetadata` (session_id, file_path, source_type, session_sequence).
- `get_causal_relationships_by_source(id)` -- fetches LLM-discovered causal relationships where this memory is the cause.
- `get_topic_profile(id)` -- reads per-memory 13D topic profile from `CF_TOPIC_PROFILES`.

### 9.2 Edge Repository

- `get_typed_edges_from(source)` -- fetches outgoing typed edges for a memory.
- `iter_all_typed_edges()` -- full scan for building the incoming-edge reverse index.
- `get_embedder_edges(embedder_idx, source)` -- fetches K-NN edges per embedder space.

### 9.3 Synergy Matrix

- `SynergyMatrix::with_base_synergies()` -- instantiated once per export run. Provides the 13x13 synergy weights for cross-correlation computation.

### 9.4 Tucker Compressor

- `CpuTuckerCompressor.compress(&fp.semantic, DEFAULT_RANKS)` -- CPU-only streaming HOSVD. Only called when `includeTuckerCore=true`.

---

## 10. Version History

| Version | Changes |
|---|---|
| v1 | Initial training record format. All fields except `edge_type_distribution`. |
| v2 (current) | Added `edge_type_distribution: [u32; 8]` at the end of `TrainingRecord`. v1 records are unreadable by the v2 decoder; re-export required. |

Deserialization rejects version mismatches. No automatic migration path exists by design.

---

## 11. Cross-References

| Topic | Document |
|---|---|
| Embedding subsystem (E1-E13 details) | [05_embedding_subsystem.md](05_embedding_subsystem.md) |
| Search and fusion (synergy matrix, multi-space) | [06_search_and_fusion.md](06_search_and_fusion.md) |
| Graph linking and causal (typed edges, K-NN) | [07_graph_and_causal.md](07_graph_and_causal.md) |
| MCP tools reference (all tool definitions) | [08_mcp_tools_reference.md](08_mcp_tools_reference.md) |
| Storage layer (RocksDB column families) | [09_storage_layer.md](09_storage_layer.md) |
| CLI reference | [10_cli_reference.md](10_cli_reference.md) |
| Teleological subsystem (topic profiles, groups) | [13_teleological_subsystem.md](13_teleological_subsystem.md) |
| Clustering and topics (topic memberships) | [12_clustering_and_topics.md](12_clustering_and_topics.md) |
