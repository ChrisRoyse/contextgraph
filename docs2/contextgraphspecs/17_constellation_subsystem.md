# 17. Constellation Subsystem

A **constellation** is a compiled reference anchor — a bundle of per-embedder centroids plus spread statistics — for a caller-selected set of memories. Once compiled, a constellation answers "does this new memory belong to the same cluster?" in O(13) cosine operations without re-running the full retrieval pipeline. Constellations feed style/topic scoring in downstream training (see [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md)).

---

## Source Files Covered

| File | Crate | Role |
|------|-------|------|
| `crates/context-graph-core/src/constellation/mod.rs` | core | Module root, re-exports, version/capacity constants |
| `crates/context-graph-core/src/constellation/compiler.rs` | core | `ConstellationAccumulator`, `compile_constellation`, `score_memory_against_constellation`, `ConstellationError` |
| `crates/context-graph-core/src/constellation/reservoir.rs` | core | `ReservoirSample` (Vitter's Algorithm R) |
| `crates/context-graph-core/src/constellation/welford.rs` | core | `WelfordStats`, `WelfordVector` (online mean + variance) |
| `crates/context-graph-core/src/constellation/types.rs` | core | `Constellation`, `ConstellationSelector`, `EmbedderStats`, `VectorKind`, `ConstellationScoringResult` |
| `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` | mcp | 5 MCP tool handlers |
| `crates/context-graph-mcp/src/tools/definitions/constellation.rs` | mcp | Tool schema definitions |
| `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs` | storage | RocksDB CRUD + secondary index |

---

## Constants

All constants with their defined values and source locations.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `CONSTELLATION_VERSION` | `1` | `mod.rs:52` | On-disk version byte prefix for `CF_CONSTELLATIONS` payloads. Deserialization rejects mismatches with `CoreError::SerializationError`. No automatic migration. |
| `RESERVOIR_SAMPLE_SIZE` | `1024` | `mod.rs:56` | Capacity of each `ReservoirSample` instance used for percentile estimation. |
| `MIN_CONSTELLATION_MEMBERS` | `3` | `mod.rs:61` | `finalize()` returns `ConstellationError::TooFewMembers` below this threshold. |
| `DEFAULT_MAX_MEMBERS` | `50_000` | `compiler.rs:47` | Default cap on in-memory member fingerprints buffered between pass 1 and pass 2. |
| `SPARSE_TOP_K` | `50` | `compiler.rs:49` | Maximum number of sparse terms retained in `EmbedderStats::sparse_top_terms`, sorted by weight descending. |
| `E12_TOKEN_DIM` | `128` | `compiler.rs:52` | ColBERT (E12) pooled token dimension. Each member's tokens are mean-pooled into a 128D vector. |
| `NUM_CONSTELLATION_EMBEDDERS` | `13` (= `NUM_EMBEDDERS`) | `types.rs:20` | Length of the `per_embedder` vector in `Constellation`. One entry per embedder index. |
| `TOPIC_PROFILE_CENTROID_DIM` | `13` (= `NUM_EMBEDDERS`) | `types.rs:23` | Dimension of `topic_profile_centroid`. |
| `GROUP_ALIGNMENT_CENTROID_DIM` | `6` | `types.rs:27` | Dimension of `group_alignment_centroid`. Six groups: Factual, Temporal, Causal, Relational, Qualitative, Implementation. |
| `CROSS_CORRELATION_CENTROID_DIM` | `78` | `types.rs:30` | Dimension of `cross_correlation_centroid`. C(13,2) = 78 pairwise correlation entries. |
| `SCAN_CAP` | `200_000` | `constellation_tools.rs:35` | Hard cap on fingerprints scanned during selector resolution in MCP handlers. |
| `SELECTOR_INDEX_KEY_LEN` | `17` (1 + 16) | `constellation.rs:41` (storage) | Secondary index key length: 1-byte kind discriminant + 16-byte truncated SHA-256 hash. |
| `DEFAULT_SEED` | `0x6C_6F_67_69_73_74_69_63` | `reservoir.rs:18` | Default deterministic seed for `ReservoirSample::new()`. ASCII encoding of "logistic". |

---

## Public Types

### `Constellation`

**Source**: `types.rs:37`

Compiled constellation record persisted to `CF_CONSTELLATIONS`. Encoded on-disk as `[CONSTELLATION_VERSION: u8][bincode]`.

| Field | Type | Description |
|-------|------|-------------|
| `id` | `Uuid` | Fresh UUID per compilation. Distinct from any member memory id. |
| `label` | `String` | Human-readable tag (e.g. "PRD S7 -- Case management"). Caller-supplied. |
| `created_at` | `DateTime<Utc>` | Compilation timestamp. |
| `selector` | `ConstellationSelector` | How the member set was resolved. Hashed into the secondary index. |
| `member_ids` | `Vec<Uuid>` | UUIDs of all member memories. |
| `member_count` | `usize` | Length of `member_ids`. |
| `per_embedder` | `Vec<EmbedderStats>` | Per-embedder centroid + spread stats. Always length 13 (`NUM_CONSTELLATION_EMBEDDERS`). |
| `topic_profile_centroid` | `[f32; 13]` | Mean 13D topic profile across members. All-zero when no member had a stored topic profile (see `has_topic_profile_signal()`). |
| `group_alignment_centroid` | `[f32; 6]` | Mean 6D group alignment across members. Derived from `topic_profile_centroid`. |
| `cross_correlation_centroid` | `Vec<f32>` | Mean 78-entry cross-correlation vector. Length `CROSS_CORRELATION_CENTROID_DIM` (78). |
| `coherence` | `f32` | Median (p50) cosine of members against the E1 centroid. Robust to outliers. |
| `purity` | `Option<f32>` | Topic purity in `[0.0, 1.0]`. Populated only for `Topic` selectors; `None` for all others. |

**Method**: `has_topic_profile_signal() -> bool` -- Returns `true` when at least one slot of `topic_profile_centroid` is non-zero. An all-zero centroid indicates that `stored_topic_profile` was `None` for every member (fresh DB, clustering pipeline not yet run).

### `ConstellationSelector`

**Source**: `types.rs:108`

Enum determining how a constellation's member set was chosen. Each variant serializes to a canonical string form that is SHA-256-hashed into the secondary index key.

| Variant | Fields | Canonical Form | Kind Byte |
|---------|--------|----------------|-----------|
| `Topic` | `topic_id: String` | `"topic:{topic_id}"` | `0` |
| `Session` | `session_id: String` | `"session:{session_id}"` | `1` |
| `Tag` | `tag: String` | `"tag:{tag}"` | `2` |
| `TimeRange` | `start: DateTime<Utc>`, `end: DateTime<Utc>` | `"timerange:{start_rfc3339_nanos}-{end_rfc3339_nanos}"` | `3` |
| `ExplicitIds` | `rationale: String`, `ids: Vec<Uuid>` | `"explicit:{sorted_deduped_ids_joined_by_comma}"` | `4` |

**Key behaviors** (from `canonical_form()` in `types.rs:134`):
- For `ExplicitIds`, the `rationale` field is intentionally NOT part of the canonical form. Only the sorted, deduplicated id set determines identity.
- For `TimeRange`, timestamps are emitted with nanosecond precision via `to_rfc3339_opts(SecondsFormat::Nanos, true)`.

### `EmbedderStats`

**Source**: `types.rs:191`

Per-embedder centroid + spread statistics. One entry per embedder index 0..12 regardless of population. Check `coverage` to determine whether data exists.

| Field | Type | Description |
|-------|------|-------------|
| `embedder_index` | `u8` | Embedder index in 0..12. |
| `dimension` | `u16` | Dense vector dimension (0 for sparse/token-level). |
| `vector_kind` | `VectorKind` | Classification of this embedder's storage. |
| `centroid` | `Vec<f32>` | Mean dense vector. Non-empty only for `Dense`/`Asymmetric`. |
| `sparse_top_terms` | `Vec<(u16, f32)>` | Top terms by mean weight (up to 50). Sorted descending by weight. Empty for non-sparse. |
| `mean_token_count` | `Option<f32>` | Mean tokens per member. Populated only for token-level (E12). |
| `pooled_token_centroid` | `Vec<f32>` | Mean-pooled 128D token centroid for E12. Empty for non-token embedders. |
| `mean_l2` | `f32` | Mean L2 norm of member vectors (dense/asymmetric) or L2 of mean sparse/pooled vector (sparse/token). |
| `stddev_l2` | `f32` | Population stddev of L2 norms across reservoir samples. |
| `cosine_spread_p50` | `f32` | Median cosine similarity of member vectors to the centroid. |
| `cosine_spread_p95` | `f32` | 95th percentile cosine-to-centroid similarity. |
| `min_cosine` | `f32` | Minimum observed cosine-to-centroid. |
| `max_cosine` | `f32` | Maximum observed cosine-to-centroid. |
| `coverage` | `f32` | Fraction of members in `[0.0, 1.0]` with a non-zero vector for this embedder. |

### `VectorKind`

**Source**: `types.rs:173`

| Variant | Embedders | Centroid Strategy |
|---------|-----------|-------------------|
| `Dense` | E1, E2, E3, E4, E7, E9, E11 | `WelfordVector` mean over member vectors. |
| `Sparse` | E6, E13 | Mean sparse weight per term id. `centroid` stays empty; `sparse_top_terms` holds ranked mean-weight terms. |
| `TokenLevel` | E12 | Each member's tokens are mean-pooled to 128D; those pooled vectors are averaged via `WelfordVector`. `centroid` stays empty; `pooled_token_centroid` carries the result. |
| `Asymmetric` | E5 (cause), E8 (source), E10 (paraphrase) | Centroid computed on the production side only (cause/source/paraphrase). |

### `ConstellationScoringResult`

**Source**: `types.rs:234`

| Field | Type | Description |
|-------|------|-------------|
| `constellation_id` | `Uuid` | Constellation scored against. |
| `memory_id` | `Uuid` | Candidate memory id. |
| `per_embedder_cosine` | `[f32; 13]` | Per-embedder cosine of candidate vector against centroid. `0.0` when candidate lacks that vector or centroid is absent. |
| `combined_score` | `f32` | Unweighted mean of `per_embedder_cosine` over entries where `coverage > 0`. |
| `in_spread_p95` | `bool` | True when `per_embedder_cosine[0]` (E1) >= `cosine_spread_p95 - 1e-6` on E1. |

### `ConstellationError`

**Source**: `compiler.rs:56`

| Variant | Fields | Message Template |
|---------|--------|-----------------|
| `TooFewMembers` | `count: usize`, `min: usize` | `"constellation requires at least {min} members (got {count})"` |
| `TooManyMembers` | `count: usize`, `max: usize` | `"constellation exceeded max_members={max}; got {count}"` |
| `DimensionMismatch` | `embedder_index: u8`, `expected: usize`, `actual: usize` | `"embedder {embedder_index} dimension mismatch: expected {expected}, got {actual}"` |
| `DegenerateCentroid` | `embedder_index: u8` | `"embedder {embedder_index} centroid is all-zero; constellation is degenerate"` |

---

## Embedder Index Map

The `default_kind_and_dim()` function in `compiler.rs:421` maps embedder indices to `(VectorKind, dimension)`:

| Index | Embedder | VectorKind | Dimension | Fingerprint Field |
|-------|----------|------------|-----------|-------------------|
| 0 | E1 (semantic) | Dense | 1024 | `e1_semantic` |
| 1 | E2 (temporal recent) | Dense | 512 | `e2_temporal_recent` |
| 2 | E3 (temporal periodic) | Dense | 512 | `e3_temporal_periodic` |
| 3 | E4 (temporal positional) | Dense | 512 | `e4_temporal_positional` |
| 4 | E5 (causal) | Asymmetric | 768 | `e5_causal_as_cause` |
| 5 | E6 (sparse keyword) | Sparse | 0 | `e6_sparse` |
| 6 | E7 (code) | Dense | 1536 | `e7_code` |
| 7 | E8 (graph) | Asymmetric | 1024 | `e8_graph_as_source` |
| 8 | E9 (HDC) | Dense | 1024 | `e9_hdc` |
| 9 | E10 (multimodal) | Asymmetric | 768 | `e10_multimodal_paraphrase` |
| 10 | E11 (entity) | Dense | 768 | `e11_entity` |
| 11 | E12 (ColBERT) | TokenLevel | 0 | `e12_late_interaction` |
| 12 | E13 (SPLADE) | Sparse | 0 | `e13_splade` |

---

## Two-Pass Compiler Algorithm

**Source**: `compiler.rs`

The compiler requires both centroid stats (mean vectors) and per-member cosine-to-centroid spread statistics. Computing cosine-to-centroid requires the centroid, making this intrinsically two-pass work. All member fingerprints are buffered in memory between passes.

### ConstellationAccumulator

**Source**: `compiler.rs:155`

| Field | Type | Purpose |
|-------|------|---------|
| `selector` | `ConstellationSelector` | How members were selected. |
| `label` | `String` | Human-readable tag. |
| `max_members` | `usize` | Cap on buffered members. Default `50_000`. |
| `member_ids` | `Vec<Uuid>` | Accumulated member UUIDs. |
| `e_stats` | `Vec<EmbedderAccumulator>` | One per-embedder accumulator (length 13). |
| `topic_profile` | `WelfordVector` | 13D topic profile centroid accumulator. |
| `group_alignments` | `WelfordVector` | 6D group alignment centroid accumulator. |
| `cross_correlations` | `WelfordVector` | 78D cross-correlation centroid accumulator. |
| `topic_match_count` | `u64` | Numerator for purity (Topic selectors only). |

### Pass 1: `observe(id, fp, stored_topic_profile, synergy)`

**Source**: `compiler.rs:210`

Called once per member fingerprint. Steps:

1. **Cap check**: If `member_ids.len() >= max_members`, return `Err(TooManyMembers)`.
2. **Push member_id**: Append `id` to `member_ids`.
3. **Dense embedders** (indices 0-4, 6-10): Call `observe_dense()` which:
   - Treats empty, wrong-dimension, non-finite, or zero-norm vectors as `Missing`.
   - For valid vectors: feeds the vector into the `WelfordVector` centroid accumulator, observes the L2 norm into the `l2_samples` reservoir, increments `non_empty_count`, and stores a clone as `ObservedVector::Dense(v)`.
4. **Sparse embedders** (indices 5, 12): Call `observe_sparse()` which:
   - Treats empty, mismatched-length, non-finite, or zero-energy sparse vectors as `Missing`.
   - Accumulates per-term weight sums in `sparse_term_sums: HashMap<u16, f32>`.
   - Observes `sqrt(sum_of_squares)` into `l2_samples`.
   - Stores a clone as `ObservedVector::Sparse { indices, values }`.
5. **Token embedder** (index 11): Call `observe_tokens()` which:
   - Mean-pools valid 128D tokens into a single 128D vector.
   - Feeds the pooled vector into the `WelfordVector` token centroid accumulator.
   - Records token count into `WelfordStats` token count accumulator.
   - Stores a clone as `ObservedVector::Tokens(tokens)`.
6. **Topic profile / groups / cross-correlations**: If `stored_topic_profile` is `Some`:
   - Observes the 13D profile into `topic_profile` WelfordVector.
   - Computes 6D group alignments via `compute_group_alignments()` and observes into `group_alignments`.
   - Computes 78D cross-correlations via `compute_cross_correlations(profile, synergy)` and observes into `cross_correlations`.
   - When `stored_topic_profile` is `None`, the member is silently skipped (no zero-imputation).

### Pass 2: `finalize() -> Result<Constellation, ConstellationError>`

**Source**: `compiler.rs:269`

Called once after all members have been observed. Steps:

1. **Minimum check**: If `member_ids.len() < MIN_CONSTELLATION_MEMBERS` (3), return `Err(TooFewMembers)`.
2. **Destructure** the accumulator into its constituent parts.
3. **Per-embedder finalization** (for each of the 13 embedder accumulators):
   - **`finalize_cosine_pass()`** (`compiler.rs:589`): Walks buffered observations and computes cosine of each member vector against the now-known centroid:
     - **Dense/Asymmetric**: Extracts the centroid from `WelfordVector::mean()`. For each `ObservedVector::Dense(v)`, computes `cosine(v, centroid)` and feeds into `cosine_samples` reservoir.
     - **Sparse**: Computes mean sparse vector by dividing `sparse_term_sums` by `non_empty_count`. For each `ObservedVector::Sparse`, computes sparse cosine `dot / (member_norm * mean_norm)` clamped to `[-1, 1]` and feeds into `cosine_samples`.
     - **TokenLevel**: Extracts pooled centroid from `WelfordVector::mean()`. For each `ObservedVector::Tokens`, re-pools tokens to 128D and computes `cosine(pooled, centroid)` into `cosine_samples`.
   - **`build_stats()`** (`compiler.rs:690`): Constructs the `EmbedderStats` record:
     - `centroid` = `WelfordVector::mean()` (dense/asymmetric), empty otherwise.
     - `sparse_top_terms` = sorted-by-weight-descending mean sparse terms, truncated to `SPARSE_TOP_K` (50).
     - `pooled_token_centroid` = `WelfordVector::mean()` for token-level.
     - `mean_token_count` = `WelfordStats::mean()` for token count.
     - `mean_l2` = centroid L2 norm (dense/asymmetric), sparse mean vector L2 (sparse), or pooled centroid L2 (token).
     - `stddev_l2` = population stddev computed from the L2 reservoir samples.
     - `cosine_spread_p50` = `cosine_samples.percentile(0.5)`.
     - `cosine_spread_p95` = `cosine_samples.percentile(0.95)`.
     - `min_cosine`, `max_cosine` = reservoir min/max.
     - `coverage` = `non_empty_count / total_members`.
4. **Topic / group / cross-correlation centroids**: Copy `WelfordVector::mean()` into fixed-size arrays.
5. **Coherence**: Set to `cosine_spread_p50` of E1 (index 0). The median is used as a robust proxy (`mean_cosine()` at `compiler.rs:780`).
6. **Purity**: For `Topic` selectors, `purity = topic_match_count / member_count`. For all other selectors, `purity = None`.
7. **Construct** and return the `Constellation` with a fresh `Uuid::new_v4()` and `Utc::now()` timestamp.

### Convenience function: `compile_constellation()`

**Source**: `compiler.rs:341`

```rust
pub fn compile_constellation<I>(
    selector: ConstellationSelector,
    label: String,
    members: I,                       // Iterator of (Uuid, SemanticFingerprint, Option<[f32; 13]>)
    synergy: &SynergyMatrix,
) -> Result<Constellation, ConstellationError>
```

Creates a `ConstellationAccumulator::new()`, calls `observe()` for each member, then `finalize()`. Intended for callers that already have the full member set.

### Memory Cost

The compiler buffers all member fingerprints in memory between passes. For N members with 13 dense embedders at up to 1536 f32 per vector, worst-case transient memory is O(N x sum_of_all_dims x 4 bytes). For 50,000 members this is roughly 2.5 GB (`compiler.rs:24`).

---

## Reservoir Sampling (Vitter's Algorithm R)

**Source**: `reservoir.rs`

Fixed-capacity bounded-memory reservoir for approximate percentile computation over unbounded streams. Maintains a uniform random sample of size `capacity`.

### ReservoirSample Struct

| Field | Type | Description |
|-------|------|-------------|
| `capacity` | `usize` | Maximum number of samples retained. |
| `samples` | `Vec<f32>` | Current retained samples (length <= capacity). |
| `seen` | `u64` | Total count of finite values observed via `observe()`. |
| `rng` | `StdRng` | Seeded PRNG for replacement decisions. |

### Algorithm Steps (`observe()`, `reservoir.rs:47`)

1. If `value` is non-finite (NaN, +Inf, -Inf) or `capacity == 0`, return immediately without updating `seen`.
2. Increment `seen`.
3. If `samples.len() < capacity`: push `value` and return (filling phase).
4. Otherwise (replacement phase): generate a random index `idx` in `[0, seen)`. If `idx < capacity`, replace `samples[idx]` with `value`.

This guarantees each observed finite value has equal probability `capacity / seen` of being in the reservoir at any point.

### Percentile Computation (`percentile()`, `reservoir.rs:72`)

Linear interpolation on the sorted reservoir:

1. If reservoir is empty, return `0.0`.
2. Clamp quantile `q` to `[0.0, 1.0]`.
3. Sort the reservoir ascending.
4. If only 1 sample, return it.
5. Compute position: `pos = q * (len - 1)`.
6. `lo = floor(pos)`, `hi = ceil(pos)`.
7. If `lo == hi`, return `sorted[lo]`.
8. Otherwise, linear interpolation: `result = sorted[lo] * (1 - frac) + sorted[hi] * frac` where `frac = pos - lo`.

### Public Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(capacity: usize) -> Self` | Default seed (`DEFAULT_SEED`). Capacity 0 is valid (all observes are no-ops). |
| `with_seed` | `fn with_seed(capacity: usize, seed: u64) -> Self` | Custom seed for independent samples in the same process. |
| `observe` | `fn observe(&mut self, value: f32)` | Observe one sample. Non-finite inputs dropped. |
| `sorted` | `fn sorted(&self) -> Vec<f32>` | Fresh ascending-sorted copy of current samples. |
| `percentile` | `fn percentile(&self, q: f32) -> f32` | Approximate quantile via linear interpolation. Returns `0.0` on empty. |
| `min` | `fn min(&self) -> f32` | Minimum sample in reservoir. Returns `0.0` on empty. |
| `max` | `fn max(&self) -> f32` | Maximum sample in reservoir. Returns `0.0` on empty. |
| `seen` | `fn seen(&self) -> u64` | Total finite values observed. |
| `len` | `fn len(&self) -> usize` | Current reservoir occupancy (<= capacity). |
| `is_empty` | `fn is_empty(&self) -> bool` | True when `samples` is empty. |

---

## Welford's Online Mean + Variance Algorithm

**Source**: `welford.rs`

Single-pass numerically stable estimators for mean and variance. Used by the compiler to aggregate embedder statistics without holding the full member set.

### WelfordStats (Scalar)

**Source**: `welford.rs:14`

| Field | Type | Description |
|-------|------|-------------|
| `count` | `u64` | Number of finite values observed. |
| `mean` | `f32` | Running mean. |
| `m2` | `f32` | Running sum of squared deltas from the mean (for variance computation). |

**Update formula** (`observe()`, `welford.rs:31`):

```
count += 1
delta  = value - mean
mean  += delta / count
delta2 = value - mean        // note: uses UPDATED mean
m2    += delta * delta2
```

**Variance** (population, not Bessel-corrected, `welford.rs:55`): `m2 / count`. Returns `0.0` for count < 2.

**Stddev**: `sqrt(variance)`.

Non-finite inputs (NaN, +/-Inf) are silently skipped.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new() -> Self` | Initialize with count=0, mean=0, m2=0. |
| `observe` | `fn observe(&mut self, value: f32)` | Update with one sample. Non-finite skipped. |
| `count` | `fn count(&self) -> u64` | Number of finite samples observed. |
| `mean` | `fn mean(&self) -> f32` | Running mean (0.0 for empty). |
| `variance` | `fn variance(&self) -> f32` | Population variance (0.0 for count < 2). |
| `stddev` | `fn stddev(&self) -> f32` | `sqrt(variance)`. |

### WelfordVector (Per-Coordinate)

**Source**: `welford.rs:87`

| Field | Type | Description |
|-------|------|-------------|
| `count` | `u64` | Number of valid vectors observed. |
| `mean` | `Vec<f32>` | Running per-coordinate mean (the centroid). |
| `m2` | `Vec<f32>` | Running per-coordinate M2 (sum of squared deltas). |

**Update formula** (`observe()`, `welford.rs:103`):

For each coordinate `i`:
```
count += 1
inv    = 1.0 / count
delta  = v[i] - mean[i]
mean[i] += delta * inv
delta2 = v[i] - mean[i]     // uses UPDATED mean
m2[i] += delta * delta2
```

If ANY coordinate in the input vector is non-finite, the entire vector is skipped.

**Panics** on dimension mismatch (input length != accumulator dimension). This is deliberate fail-fast behavior.

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(dim: usize) -> Self` | Initialize with given dimension. |
| `observe` | `fn observe(&mut self, v: &[f32])` | Update with one vector. Panics on dim mismatch. Skips if any coordinate is non-finite. |
| `count` | `fn count(&self) -> u64` | Number of valid vectors observed. |
| `mean` | `fn mean(&self) -> &[f32]` | Running centroid (borrow). |
| `mean_l2` | `fn mean_l2(&self) -> f32` | L2 norm of the centroid vector. |
| `stddev_l2` | `fn stddev_l2(&self) -> f32` | L2 norm of the per-coordinate population stddev vector. 0.0 for count < 2. |

---

## Scoring Algorithm

### `score_memory_against_constellation()`

**Source**: `compiler.rs:369`

```rust
pub fn score_memory_against_constellation(
    constellation: &Constellation,
    memory_id: Uuid,
    fp: &SemanticFingerprint,
) -> ConstellationScoringResult
```

Scores a candidate memory against an already-compiled constellation. Steps:

1. Initialize `per_embedder_cosine = [0.0; 13]`, `active_count = 0`, `summed = 0.0`.
2. For each embedder `i` in 0..12:
   - If `per_embedder[i].coverage <= 0.0`, skip (embedder has no data in the constellation).
   - Call `score_one(stats, fp)` to compute cosine:
     - **Dense/Asymmetric** (`compiler.rs:790`): `cosine(pick_dense(idx, fp), stats.centroid)`. Returns `0.0` if centroid is empty.
     - **Sparse** (`compiler.rs:797`): Builds member vector from the fingerprint's sparse field, computes `dot / (member_norm * centroid_norm)` clamped to `[-1, 1]`. Uses only the `sparse_top_terms` (up to 50 terms) as the centroid approximation.
     - **TokenLevel** (`compiler.rs:814`): Mean-pools the candidate's tokens to 128D, then `cosine(pooled, pooled_token_centroid)`.
   - Store cosine in `per[i]`, accumulate into `summed`, increment `active_count`.
3. `combined_score = summed / active_count` (or `0.0` if no active embedders).
4. `in_spread_p95`: True when E1 (index 0) has `coverage > 0` AND `per[0] >= e1.cosine_spread_p95 - 1e-6`. The epsilon tolerance (`1e-6`) prevents floating-point edge cases from rejecting exact-match members.
5. Return `ConstellationScoringResult`.

### `pick_dense()` mapping (`compiler.rs:824`)

| `emb_idx` | Fingerprint Field |
|-----------|--------------------|
| 0 | `e1_semantic` |
| 1 | `e2_temporal_recent` |
| 2 | `e3_temporal_periodic` |
| 3 | `e4_temporal_positional` |
| 4 | `e5_causal_as_cause` |
| 6 | `e7_code` |
| 7 | `e8_graph_as_source` |
| 8 | `e9_hdc` |
| 9 | `e10_multimodal_paraphrase` |
| 10 | `e11_entity` |
| _ | `&[]` (empty slice) |

### `pick_sparse()` mapping (`compiler.rs:840`)

| `emb_idx` | Fingerprint Field |
|-----------|--------------------|
| 5 | `e6_sparse` (indices + values) |
| 12 | `e13_splade` (indices + values) |
| _ | `None` |

---

## Internal Helper Functions

**Source**: `compiler.rs`

| Function | Location | Purpose |
|----------|----------|---------|
| `default_kind_and_dim(idx: u8)` | `compiler.rs:421` | Returns `(VectorKind, dimension)` for each embedder index. |
| `l2_norm(v: &[f32]) -> f32` | `compiler.rs:440` | `sqrt(sum(x^2))`. |
| `cosine(a: &[f32], b: &[f32]) -> f32` | `compiler.rs:444` | Dense cosine similarity clamped to `[-1, 1]`. Returns `0.0` on empty/mismatched/zero-norm inputs. |
| `sparse_sumsq(s: &SparseVector) -> f32` | `compiler.rs:462` | Sum of squares of sparse values. |
| `sparse_dot(av, bv) -> f32` | `compiler.rs:466` | Sparse inner product via HashMap probe on the smaller side. |
| `sparse_l2(v: &[(u16, f32)]) -> f32` | `compiler.rs:665` | L2 norm of sparse term-value pairs. |
| `pool_tokens_for_cosine(tokens) -> Vec<f32>` | `compiler.rs:669` | Mean-pool valid 128D tokens. Returns zero vector if no valid tokens. |
| `observe_dense(acc, v)` | `compiler.rs:484` | Pass-1 dense observation. Guards against empty, wrong-dim, non-finite, zero-norm vectors. |
| `observe_sparse(acc, s)` | `compiler.rs:516` | Pass-1 sparse observation. Guards against empty, mismatched, non-finite, zero-energy. |
| `observe_tokens(acc, tokens)` | `compiler.rs:543` | Pass-1 token observation. Mean-pools then feeds into WelfordVector. |
| `finalize_cosine_pass(acc, idx)` | `compiler.rs:589` | Pass-2: compute cosine of each buffered observation against centroid. |
| `build_stats(emb_idx, acc, total)` | `compiler.rs:690` | Assemble `EmbedderStats` from accumulator state. |
| `reservoir_stddev(r)` | `compiler.rs:769` | Population stddev computed from sorted reservoir samples. |
| `mean_cosine(stats) -> f32` | `compiler.rs:780` | Returns `cosine_spread_p50` (median as robust proxy). |
| `score_one(stats, fp) -> f32` | `compiler.rs:788` | Per-embedder cosine for scoring. Routes by `VectorKind`. |

---

## MCP Tool Definitions

**Source**: `constellation.rs` (definitions) and `constellation_tools.rs` (handlers)

All five schemas carry `additionalProperties: false` to reject unknown parameters.

### `compile_constellation`

**Handler**: `call_compile_constellation()` (`constellation_tools.rs:39`)

Resolves members from the selector, streams them into a `ConstellationAccumulator`, persists the result.

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `selector` | `string` enum: `topic`, `session`, `tag`, `time_range`, `explicit_ids` | Yes | -- | Resolution strategy for gathering members. |
| `topicId` | `string` | When selector=topic | -- | Matches topic portfolio topic IDs. |
| `sessionId` | `string` | When selector=session | -- | Matches `source_metadata.session_id`. |
| `tag` | `string` | When selector=tag | -- | Matches `source_metadata.tool_name` (closest available tag field). |
| `startIso` | `string` (date-time) | When selector=time_range | -- | RFC-3339 inclusive lower bound on `fingerprint.created_at`. |
| `endIso` | `string` (date-time) | When selector=time_range | -- | RFC-3339 inclusive upper bound. Must be >= `startIso`. |
| `memoryIds` | `array` of UUID strings | When selector=explicit_ids | -- | UUIDs of memories to include. At least one required. |
| `rationale` | `string` | No | `""` | Free-form annotation for explicit_ids. NOT part of selector hash. |
| `label` | `string` (1-256 chars) | Yes | -- | Human-readable tag for the constellation. |
| `maxMembers` | `integer` (3-100000) | No | `50000` | Cap on in-memory members. Clamped to `[3, 100_000]` in handler. |
| `rebuildIfExists` | `boolean` | No | `false` | When true, recompile even if a constellation exists for this selector. When false, return existing id. |

**Handler flow**:

1. Parse `label` (required, non-empty) and `selector` (via `parse_selector()`).
2. Clamp `maxMembers` to `[3, 100_000]`.
3. Downcast `teleological_store` to `RocksDbTeleologicalStore`. Error if not RocksDB.
4. If `rebuildIfExists == false`: check `find_constellation_by_selector()`. If found, return `already_exists: true` with existing id.
5. Resolve members via `resolve_members()` (see below). Error if zero results.
6. Create `ConstellationAccumulator::with_max_members()`.
7. For each member fingerprint:
   - Read topic profile via `rocksdb_store.get_topic_profile(fp.id)`. Error on DB failure (no silent fallback).
   - Call `acc.observe()`.
   - For Topic selectors: call `topic_membership_matches()` then `acc.observe_topic_match()`.
8. Call `acc.finalize()`. Error on `TooFewMembers`.
9. Call `rocksdb_store.store_constellation()`.
10. Return JSON with `constellation_id`, `label`, `selector_kind`, `member_count`, `coherence`, `purity`, `per_embedder_coverage`.

### `list_constellations`

**Handler**: `call_list_constellations()` (`constellation_tools.rs:205`)

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `limit` | `integer` (1-1000) | No | `50` | Max UUIDs to return. |
| `offset` | `integer` (>= 0) | No | `0` | Skip this many UUIDs (pagination). |
| `includeCentroids` | `boolean` | No | `false` | When true, include per-embedder centroid arrays. Can produce large responses. |

**Handler flow**: Lists all constellation IDs via `list_constellation_ids()`, applies offset/limit pagination in-memory, fetches records via `multi_get_constellations()`, renders via `render_constellation()`.

### `get_constellation`

**Handler**: `call_get_constellation()` (`constellation_tools.rs:283`)

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `constellationId` | `string` (UUID) | Yes | -- | UUID returned by `compile_constellation`. |
| `includeCentroids` | `boolean` | No | `true` | Default true (typical caller wants full record). When false, only shape/summary returned. |

**Handler flow**: Point lookup via `get_constellation(cid)`. Returns `found: false` if not present. Renders full record via `render_constellation()`.

### `score_against_constellation`

**Handler**: `call_score_against_constellation()` (`constellation_tools.rs:323`)

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `constellationId` | `string` (UUID) | Yes | -- | UUID of constellation to score against. |
| `memoryId` | `string` (UUID) | Yes | -- | UUID of candidate memory (must exist in fingerprint store). |

**Handler flow**: Fetches constellation, retrieves candidate fingerprint via `teleological_store.retrieve()`, calls `score_memory_against_constellation()`, returns per-embedder cosines, combined score, and in_spread_p95 flag.

### `delete_constellation`

**Handler**: `call_delete_constellation()` (`constellation_tools.rs:390`)

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `constellationId` | `string` (UUID) | Yes | -- | UUID of constellation to delete. |

**Handler flow**: Calls `delete_constellation(cid)`. Returns `{ deleted: bool }`. Idempotent (returns `false` when no record existed).

---

## Member Resolution

**Source**: `constellation_tools.rs:428`

The `resolve_members()` method resolves the member set from a selector. Two paths:

### ExplicitIds Path
Fetches each id directly via `teleological_store.retrieve(id)`. Missing fingerprints are skipped with a debug log. No scan. Capped at `max_members`.

### Scan Path (Topic, Session, Tag, TimeRange)
1. Calls `list_fingerprints_unbiased(SCAN_CAP)` to scan up to 200,000 fingerprints.
2. For each fingerprint, filters by selector kind:
   - **Session**: Reads `source_metadata.session_id` via `get_source_metadata()`. Match requires exact string equality.
   - **Tag**: Reads `source_metadata.tool_name` via `get_source_metadata()`. Match requires exact string equality.
   - **TimeRange**: Checks `fp.created_at >= start && fp.created_at <= end` (inclusive).
   - **Topic**: Includes ALL fingerprints from the scan (filter is broad). Purity is measured later via `observe_topic_match()`.
3. Stops collecting once `max_members` is reached.

---

## Storage Layer

**Source**: `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs`

### Column Families

| CF Name | Constant | Key Format | Value Format | Purpose |
|---------|----------|------------|--------------|---------|
| `"constellations"` | `CF_CONSTELLATIONS` | UUID (16 bytes) | `[version: u8][bincode(Constellation)]` | Primary constellation records. |
| `"constellation_by_selector"` | `CF_CONSTELLATION_BY_SELECTOR` | `[kind_byte: u8][sha256(canonical_form)[..16]: 16B]` = 17 bytes | UUID (16 bytes) | Secondary index: selector -> constellation UUID. |

### CF Configuration

**Source**: `column_families.rs`

| CF | Compression | Bloom Filter | Prefix Extractor | Write Buffer |
|----|------------|--------------|-------------------|-------------|
| `CF_CONSTELLATIONS` | LZ4 (via `uuid_prefixed_cf_options`) | Yes | 16-byte UUID prefix | 8MB |
| `CF_CONSTELLATION_BY_SELECTOR` | LZ4 | Yes (10-bit) | 1-byte (selector kind) | 2MB |

### Encoding / Decoding

**`encode_constellation(c)`** (`constellation.rs:340`):
1. `bincode::serialize(c)` -> `Vec<u8>`.
2. Prepend `CONSTELLATION_VERSION` (1) as a single byte.
3. Return `[version_byte][bincode_bytes]`.

**`decode_constellation(bytes)`** (`constellation.rs:352`):
1. Reject empty input with `CoreError::SerializationError`.
2. Check `bytes[0] == CONSTELLATION_VERSION`. Reject mismatch with error message including got/expected versions.
3. `bincode::deserialize(&bytes[1..])`.

All structs use `#[derive(Serialize, Deserialize)]` with NO `skip_serializing_if` (bincode is positional, `skip_serializing_if` silently corrupts).

### Secondary Index Key Construction

**`selector_index_key(selector)`** (`constellation.rs:375`):
1. Compute `canonical_form()` of the selector (deterministic string).
2. SHA-256 hash the canonical form bytes.
3. Construct 17-byte key: `[kind_byte()][sha256_digest[..16]]`.

### CRUD Operations

| Method | Signature | Atomicity | Behavior |
|--------|-----------|-----------|----------|
| `store_constellation` | `async fn store_constellation(&self, c: &Constellation) -> CoreResult<()>` | Single `WriteBatch` | Probes secondary index for existing pointer. If it points to a different UUID, deletes the old primary row. Writes new primary + index in one atomic batch. |
| `get_constellation` | `async fn get_constellation(&self, id: Uuid) -> CoreResult<Option<Constellation>>` | Point read | Returns `None` if not found. Decodes with version check. |
| `list_constellation_ids` | `async fn list_constellation_ids(&self) -> CoreResult<Vec<Uuid>>` | Full CF scan | Iterates `CF_CONSTELLATIONS`. Skips non-16-byte keys with warning. |
| `count_constellations` | `async fn count_constellations(&self) -> CoreResult<usize>` | O(n) scan | Counts iterator entries. |
| `delete_constellation` | `async fn delete_constellation(&self, id: Uuid) -> CoreResult<bool>` | Single `WriteBatch` | Reads primary record to find selector. Deletes primary + secondary index only if index still points to this UUID (avoids clobbering newer constellation for same selector). Returns `false` if not found. |
| `find_constellation_by_selector` | `async fn find_constellation_by_selector(&self, selector: &ConstellationSelector) -> CoreResult<Option<Uuid>>` | Point read | Looks up secondary index. Rejects non-16-byte values with `CoreError::SerializationError`. |
| `multi_get_constellations` | `async fn multi_get_constellations(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<Constellation>>>` | Batch read | Uses `multi_get_cf`. Output indices align with input. Decode failures yield `None` with warning (not hard error). |

### Orphan Prevention

**Source**: `store_constellation()` at `constellation.rs:70`

When `rebuildIfExists=true` causes a new constellation UUID to be generated for an existing selector, the `store_constellation` method prevents orphaned primary records:
1. Probes the secondary index for the current pointer.
2. If the pointer exists and points to a DIFFERENT UUID than the new one, includes a `delete_cf` for the old primary row in the same `WriteBatch`.
3. All three operations (delete-old-primary, put-new-primary, put-new-index) are atomic.

### Delete Safety

**Source**: `delete_constellation()` at `constellation.rs:206`

The delete operation only removes the secondary index entry if it still points to the UUID being deleted. This prevents clobbering a newer constellation that happens to share the same selector (race condition guard).

---

## Cross-References

| Topic | Document |
|-------|----------|
| Embedder dimensions and model IDs | [05_embedding_subsystem.md](05_embedding_subsystem.md) |
| RocksDB column family architecture | [04_database_schema.md](04_database_schema.md), [09_storage_layer.md](09_storage_layer.md) |
| `SemanticFingerprint`, `SynergyMatrix`, group alignments | [13_teleological_subsystem.md](13_teleological_subsystem.md) |
| Graph and typed edges | [07_graph_and_causal.md](07_graph_and_causal.md) |
| MCP tool protocol | [08_mcp_tools_reference.md](08_mcp_tools_reference.md) |
| Topic clustering and portfolios | [12_clustering_and_topics.md](12_clustering_and_topics.md) |
| Training corpus export (`TrainingRecord`) | [16_training_subsystem.md](16_training_subsystem.md) |
| Constellations as CAMD-DPO style anchor | [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md) |
| Contrastive pairs and typed-edge export | [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md) |
