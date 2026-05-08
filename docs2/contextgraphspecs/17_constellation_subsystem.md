# Constellation Subsystem

**Source files covered:**

- `crates/context-graph-core/src/constellation/mod.rs` — module root, version constants
- `crates/context-graph-core/src/constellation/types.rs` — `Constellation`, `ConstellationSelector`, `EmbedderStats`, `VectorKind`, `ConstellationScoringResult`
- `crates/context-graph-core/src/constellation/welford.rs` — `WelfordStats` (scalar) and `WelfordVector` (per-coordinate) online mean + variance
- `crates/context-graph-core/src/constellation/reservoir.rs` — `ReservoirSample` Vitter Algorithm-R reservoir
- `crates/context-graph-core/src/constellation/compiler.rs` — `ConstellationAccumulator`, `compile_constellation`, `score_memory_against_constellation`, `ConstellationError`
- `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs` — `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` persistence + atomic-batch invariants
- `crates/context-graph-storage/src/teleological/rocksdb_store/learner.rs` — `CF_LEARNER_CONSTELLATIONS` storage methods (per-learner regulated-state baselines)
- `crates/context-graph-storage/src/teleological/column_families.rs` — `CF_CONSTELLATIONS` (`:511`), `CF_CONSTELLATION_BY_SELECTOR` (`:529`), `CF_LEARNER_CONSTELLATIONS` (`:428`); CF option builders
- `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` — six constellation MCP handlers
- `crates/context-graph-mcp/src/handlers/tools/utl_tools.rs` — `compile_learner_constellation` handler
- `crates/context-graph-mcp/src/tools/definitions/constellation.rs` — six tool schemas
- `crates/context-graph-storage/src/dynamicjepa/constellation_store.rs` — `CF_DJ_CONSTELLATIONS` reader/writer
- `crates/context-graph-storage/src/dynamicjepa/keys.rs` — `constellation_key(...)` 36-byte composite
- `crates/context-graph-core/src/dynamicjepa/meaning_compression.rs` — `ConstellationCentroid` struct + validation
- `crates/context-graph-core/src/dynamicjepa/panel.rs` — `LatentPanel` (per-event slot vectors that DJ centroids compress)

Cross-references: [04](04_database_schema.md) for the full CF catalog, [05](05_embedding_subsystem.md) for the 14-embedder fingerprint shape that members carry, [13](13_teleological_subsystem.md) §3 / §7 for the teleological vector + `SynergyMatrix` that constellation compilation consumes, [12](12_clustering_and_topics.md) for the topic clustering whose output drives `purity`, [18](18_contrastive_and_typed_edges.md) for `derive_constellation anti_pole`'s consumption of fingerprint scores, [20](20_learning_subsystem.md) for the learner-state side, [21](21_dynamicjepa_subsystem.md) for the DJ subsystem and threshold-calibration pipeline that consumes `CF_DJ_CONSTELLATIONS`. The DJ subsystem maintains its own constellation analog under `dj_constellations`; the calibration of `G_tau` thresholds described in §10.5 cross-links to the threshold-calibration table documented there.

## 1. What a constellation is

A **constellation** is a compiled, queryable summary of a set of memories selected by topic, session, tag, time range, or explicit id list. For one selector resolution it captures:

- per-embedder dense centroids and spread statistics (mean L2, stddev L2, p50/p95 cosine, min/max cosine, coverage)
- aggregate 14-D topic-profile centroid, 6-D group-alignment centroid, 91-D cross-correlation centroid
- coherence (median pairwise cosine to the E1 centroid) and topic purity (when the selector is `Topic`)

Three distinct stores share the "constellation" name. They are **not interchangeable** — each has its own record shape, key format, and persistence path:

| Store | CF | Record type | Key | Built by | Reader |
|---|---|---|---|---|---|
| **Content** | `CF_CONSTELLATIONS` + secondary `CF_CONSTELLATION_BY_SELECTOR` | `Constellation` (`core/constellation/types.rs:36-98`) | UUID (16 B) | `compile_constellation`, `derive_constellation` | `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation` |
| **Learner** (regulated-state baseline) | `CF_LEARNER_CONSTELLATIONS` | `LearnerConstellation` (`core/learner/mod.rs:643-710`) | `[learner_id:16][selector_kind:u8]` (17 B) | `compile_learner_constellation` | scope-restricted to a learner; consumed by `resolve_learner_retrieval_policy`, drift detection, training pipelines |
| **DynamicJEPA** (per-domain instrument centroid) | `CF_DJ_CONSTELLATIONS` | `ConstellationCentroid` (`core/dynamicjepa/meaning_compression.rs:43-60`) | `[domain_uuid:16][sha256(subject_id)[..16]:16][modality_id_LE:4]` (36 B) | `put_constellation` / `put_constellations_with_audit_batch` | `get_constellation`, `list_constellations`, `count_constellations`, `inspect_cf` |

This document focuses primarily on the **content store** (§2-§6, §8-§9). The **learner variant** is summarized in §7 and detailed in [13](13_teleological_subsystem.md) §13 and [20](20_learning_subsystem.md). The **DynamicJEPA variant** is documented in §10 because it shares the conceptual name but uses a completely different data model — see [21](21_dynamicjepa_subsystem.md) for the surrounding pipeline.

## 2. Constants

`crates/context-graph-core/src/constellation/mod.rs:48-65` and `types.rs:20-30`:

| Constant | Value | Meaning |
|---|---|---|
| `CONSTELLATION_VERSION` | 2 | First byte of every `CF_CONSTELLATIONS` payload; v2 = E14-aware (14 per-embedder stats, 14-D topic centroid, 91-entry cross-correlation centroid) |
| `RESERVOIR_SAMPLE_SIZE` | 1024 | Bounded reservoir per embedder for percentile estimation |
| `MIN_CONSTELLATION_MEMBERS` | 3 | Below this `finalize` returns `ConstellationError::TooFewMembers` |
| `DEFAULT_MAX_MEMBERS` | 50 000 | Default ceiling on members fed to one accumulator |
| `NUM_CONSTELLATION_EMBEDDERS` | 14 | = `teleological::types::NUM_EMBEDDERS` |
| `TOPIC_PROFILE_CENTROID_DIM` | 14 | Mean topic-profile alignment array |
| `GROUP_ALIGNMENT_CENTROID_DIM` | 6 | Factual / Temporal / Causal / Relational / Qualitative / Implementation |
| `CROSS_CORRELATION_CENTROID_DIM` | 91 | C(14, 2) cross-embedder pairs |
| `SPARSE_TOP_K` | 50 | Max terms kept in `EmbedderStats::sparse_top_terms` (`compiler.rs:49`) |
| `E12_TOKEN_DIM` | 128 | E12 ColBERT pooled dimension (`compiler.rs:52`) |
| `SELECTOR_INDEX_KEY_LEN` | 17 | `[kind:u8][SHA256(canonical)[..16]]` (`storage/.../constellation.rs:41`) |
| `SCAN_CAP` (handler) | 200 000 | Hard cap on memories scanned during selector resolution (`constellation_tools.rs:38`) |

Wire format: `[CONSTELLATION_VERSION:u8][bincode-encoded Constellation]`. Deserialization rejects mismatched versions with `CoreError::SerializationError` — **no automatic migration**. Encoding helpers are private to `rocksdb_store/constellation.rs:343-372`.

## 3. Selectors

`crates/context-graph-core/src/constellation/types.rs:105-166`. Five variants, each with a stable canonical string used for SHA-256 hashing into the secondary index:

| Variant | Canonical form | `kind_byte` |
|---|---|---|
| `Topic { topic_id }` | `topic:{topic_id}` | 0 |
| `Session { session_id }` | `session:{session_id}` | 1 |
| `Tag { tag }` | `tag:{tag}` | 2 |
| `TimeRange { start, end }` | `timerange:{start_rfc3339_nanos}-{end_rfc3339_nanos}` | 3 |
| `ExplicitIds { rationale, ids }` | `explicit:{sorted_dedup_uuids_joined_by_comma}` | 4 |

For `ExplicitIds` the `rationale` is intentionally **not** hashed — only the (sorted, deduplicated) UUID set determines identity. For `TimeRange` timestamps emit RFC-3339 with nanosecond precision so tooling cannot accidentally collide two different windows.

Secondary-index key: `[kind_byte:u8][SHA256(canonical)[..16]]` → 17 B fixed → constellation UUID (16 B value). `compile_constellation` enforces "at most one primary row per selector": when the index pointer already maps to a different UUID (rebuild case), the prior primary row is **deleted in the same `WriteBatch`** as the new put (`storage/.../constellation.rs:80-110`).

The CF option builder for the secondary index uses a 1-byte prefix extractor (`column_families.rs:1036`) so `prefix_iterator_cf(kind_byte)` enumerates every constellation of a given selector kind (e.g. "list all topic-based constellations").

## 4. `Constellation` record

`crates/context-graph-core/src/constellation/types.rs:36-98`:

| Field | Shape | Notes |
|---|---|---|
| `id` | `Uuid` | Fresh per compile; distinct from any member UUID |
| `label` | `String` | Caller-supplied tag (e.g. `"PRD §7 — Case management"`) |
| `created_at` | `DateTime<Utc>` | wall-clock at `finalize()` |
| `selector` | `ConstellationSelector` | one of §3 |
| `member_ids` | `Vec<Uuid>` | bounded by `max_members` accumulator cap |
| `member_count` | `usize` | `member_ids.len()` |
| `per_embedder` | `Vec<EmbedderStats>` | length = `NUM_CONSTELLATION_EMBEDDERS = 14` |
| `topic_profile_centroid` | `[f32; 14]` | Mean topic alignments (silent-zero on fresh DB — see §4.2) |
| `group_alignment_centroid` | `[f32; 6]` | Mean group alignments |
| `cross_correlation_centroid` | `Vec<f32>` length 91 | Mean 14×14 cross-correlation lower-triangle |
| `coherence` | `f32` in `[-1, 1]` | Median (p50) cosine of member E1 vectors against the E1 centroid; **median**, not mean, so robust to outliers |
| `purity` | `Option<f32>` in `[0, 1]` | Populated only for `selector = Topic { .. }`; ratio of members whose dominant topic matches |

### 4.1 `EmbedderStats`

`types.rs:188-229`:

| Field | Type | Meaning |
|---|---|---|
| `embedder_index` | `u8` | 0..14 |
| `dimension` | `u16` | dense dim for `Dense`/`Asymmetric`; 0 for `Sparse`/`TokenLevel` |
| `vector_kind` | `VectorKind` | `Dense`, `Sparse`, `TokenLevel`, `Asymmetric` |
| `centroid` | `Vec<f32>` | mean dense vector; non-empty only for `Dense`/`Asymmetric` |
| `sparse_top_terms` | `Vec<(u16, f32)>` | top 50 term ids by mean weight; sparse-only |
| `mean_token_count` | `Option<f32>` | E12 only |
| `pooled_token_centroid` | `Vec<f32>` 128D | E12 only |
| `mean_l2`, `stddev_l2` | `f32` | mean and stddev of member L2 norms |
| `cosine_spread_p50`, `cosine_spread_p95` | `f32` | reservoir-sampled cosine percentiles vs centroid |
| `min_cosine`, `max_cosine` | `f32` | observed extremes |
| `coverage` | `f32` in `[0, 1]` | fraction of members with a non-zero vector for this embedder |

`VectorKind` (`types.rs:170-183`): `Dense` (E1, E2, E3, E4, E7, E9, E11, E14), `Sparse` (E6, E13 — `centroid` empty, `sparse_top_terms` populated), `TokenLevel` (E12 — `pooled_token_centroid` + `mean_token_count`), `Asymmetric` (E5 cause, E8 source, E10 paraphrase — only the canonical side participates).

### 4.2 Silent-zero invariant on `topic_profile_centroid`

`Constellation::has_topic_profile_signal()` (`types.rs:95-97`) returns `true` iff at least one of the 14 slots is non-zero. **Reason**: the compiler only observes the topic profile when `RocksDbTeleologicalStore::get_topic_profile(member_id)` returns `Some`. On a fresh DB, before the clustering pipeline has populated `CF_TOPIC_PROFILES`, every member yields `None`, the Welford accumulator stays at `count = 0`, and the field serializes as all-zeros — **indistinguishable from "all members had topic-zero alignment"**. Consumers that condition on topic-centroid trustworthiness MUST call `has_topic_profile_signal()` rather than `iter().any(|x| x != 0.0)` directly.

`group_alignment_centroid` and `cross_correlation_centroid` are derived from `topic_profile_centroid` and inherit the same caveat.

## 5. Aggregation algorithms

The compiler is intrinsically two-pass (cosine spread requires the centroid):

1. **Pass 1** (`observe`): feed fingerprints into per-embedder `WelfordVector`s, reservoir samples of L2 norms, sparse term sums, pooled token centroids; buffer the members.
2. **Pass 2** (`finalize`): with centroids in hand, compute cosine of each member vector against the embedder centroid and feed dedicated cosine reservoirs.

Buffering uses O(N × dense_sizes) bytes transiently — at the default `max_members = 50 000` × 14 embedder slots × 1024 f32 worst case that is roughly 2.7 GB. Reduce `max_members` for low-RAM environments.

| Pass | Per-member work | State |
|---|---|---|
| Dense centroids (E1, E2, E3, E4, E7, E9, E11, E14) | add to `WelfordVector` for each populated embedder | online mean (single pass), population variance via Welford |
| Asymmetric (E5 cause / E8 source / E10 paraphrase) | add **only the canonical side** to its dedicated `WelfordVector` | same |
| Sparse (E6, E13) | merge-add term-frequency counts | `HashMap<u16, (sum_weight, occurrences)>` → top 50 by mean |
| Token-level (E12) | mean-pool tokens per member to 128D, then add to `WelfordVector` | pooled token centroid + per-member token count |
| Cosine spread per embedder | sample cosine(member, current_centroid) into a `ReservoirSample` of size 1024 | p50, p95, min, max via interpolation |
| Topic profile | conditional: only when `stored_topic_profile = Some(_)` | Welford over 14D |
| Group alignment | derived from observed topic profile via `GroupAlignments::from_topic_profile` | Welford over 6D |
| Cross-correlation | lower-triangle 91 entries from observed topic profile | Welford over 91D |
| Coherence | reservoir of cosine(member.E1, centroid.E1); **median**, not mean | reservoir of size 1024 |
| Purity (Topic only) | count of members whose dominant topic matches `selector.topic_id` | `u32 / total` |

`ConstellationAccumulator::observe(memory_id, fingerprint, stored_profile, &SynergyMatrix)` is the streaming entry point; `observe_topic_match(bool)` feeds the purity numerator for `Topic` selectors. `finalize()` returns `Result<Constellation, ConstellationError>`; it fails with `TooFewMembers { count, min }` when fewer than `MIN_CONSTELLATION_MEMBERS = 3` were observed.

### 5.1 Reservoir sampling (Vitter Algorithm R)

`reservoir.rs`. Maintains a uniform sample of size `capacity = 1024` from an unbounded stream. Algorithm:

```text
on observe(value):
  if value not finite: drop
  seen += 1
  if samples.len() < capacity: push
  else:
    idx = rng.gen_range(0..seen)
    if idx < capacity: samples[idx] = value
```

Default seed `0x6C_6F_67_69_73_74_69_63` (`"logistic"` in ASCII) makes tests deterministic. Non-finite values are silently dropped (would poison percentile interpolation). `percentile(q)` interpolates linearly on the sorted reservoir; `min`/`max` over the sample only. On a 10 000-sample uniform `[0,1]` stream the test in `reservoir.rs:177-201` asserts `|p50 - 0.5| < 0.03` and `|p95 - 0.95| < 0.03`.

### 5.2 Welford streaming statistics

`welford.rs`. Two parallel implementations:

| Type | Tracks | Update step |
|---|---|---|
| `WelfordStats` (scalar) | `count`, `mean`, `m2` | `delta = v - mean; mean += delta/count; m2 += delta · (v - mean)` |
| `WelfordVector` (per-coordinate) | `count`, `mean: Vec<f32>`, `m2: Vec<f32>` | per-coordinate same recurrence |

Both silently skip non-finite inputs (a single NaN coordinate would contaminate the mean for every subsequent call). `WelfordVector::observe` panics on dimension mismatch (FAIL FAST — silently truncating or zero-padding would quietly corrupt downstream centroid statistics). Population variance (`m2 / count`, not Bessel-corrected) returns 0.0 for fewer than 2 samples. Both agree with naive `mean` / `stddev_population` over any finite stream of finite f32s to within ~1e-5.

### 5.3 Compiler stages

The streaming flow inside `ConstellationAccumulator`:

1. **Construct** — `with_max_members(selector, label, max_members)` initializes 14 `EmbedderAccumulator`s (one per embedder) with `(VectorKind, dimension)` from `default_kind_and_dim`, each owning a Welford accumulator (or sparse / pooled-token equivalent), an L2 reservoir, and a cosine reservoir.
2. **Pass 1: Per-member `observe`** — for each member, append its observation to each embedder's typed buffer (`Dense(Vec<f32>)`, `Sparse{indices, values}`, `Tokens(Vec<Vec<f32>>)`, or `Missing`); update Welford state; feed L2 reservoir; track `non_empty_count` for coverage.
3. **Pass 1.5: Topic profile** — when `stored_topic_profile.is_some()`, feed the 14-D vector into `topic_profile`, derive `group_alignments` (6-D) and `cross_correlations` (91-D), feed both into their Welford accumulators.
4. **Pass 2: `finalize`** — for each embedder with sufficient members, compute the centroid (Welford mean), then iterate buffered observations to feed cosine-against-centroid into the cosine reservoir, deriving p50/p95/min/max.
5. **Materialize** — pack `EmbedderStats` per embedder, attach `topic_profile_centroid`, `group_alignment_centroid`, `cross_correlation_centroid`, compute `coherence` from the E1 cosine reservoir's median, set `purity` when selector is `Topic`. Returns `Constellation` or `ConstellationError`.

## 6. MCP tools (six)

Handlers live in `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs`. All six downcast `Handlers::teleological_store` to `RocksDbTeleologicalStore` and return `"requires RocksDbTeleologicalStore"` otherwise. Function names: `call_compile_constellation`, `call_list_constellations`, `call_get_constellation`, `call_score_against_constellation`, `call_derive_constellation`, `call_delete_constellation`. Tool count is verified at six in `definitions/constellation.rs:21-286`.

| # | Tool | Required args | Optional args | Persists / reads |
|---|---|---|---|---|
| 1 | `compile_constellation` | `selector`, `label` | `topicId`/`sessionId`/`tag`/`startIso`+`endIso`/`memoryIds`+`rationale` (per selector), `maxMembers` (3..100 000, default 50 000), `rebuildIfExists` (default false) | reads `CF_FINGERPRINTS`, `CF_TOPIC_PROFILES`; writes `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` |
| 2 | `list_constellations` | — | `limit` (1..1000, default 50), `offset` (default 0), `includeCentroids` (default false) | reads `CF_CONSTELLATIONS` (paginated) |
| 3 | `get_constellation` | `constellationId` | `includeCentroids` (default true) | reads `CF_CONSTELLATIONS` |
| 4 | `score_against_constellation` | `constellationId`, `memoryId` | — | reads `CF_CONSTELLATIONS` + `CF_FINGERPRINTS` |
| 5 | `derive_constellation` | `operation`, `sourceConstellationId`, `label` | `otherConstellationId`, `alpha` (0..1, default 0.5), `selector`+selector args (`anti_pole`), `maxCandidates` (3..200 000, default 10 000), `selectedMembers` (3..100 000, default 50) | reads source constellation(s) + candidate fingerprints; writes `CF_CONSTELLATIONS` + secondary index |
| 6 | `delete_constellation` | `constellationId` | — | atomic delete from `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` |

Every schema carries `additionalProperties: false` to reject unknown parameters (silent schema drift has bitten this codebase before; lesson recorded in `MEMORY.md`).

### 6.1 `compile_constellation` (`constellation_tools.rs:42-200`)

Pipeline:
1. `parse_selector(args)` → `ConstellationSelector`.
2. If `!rebuildIfExists` → `find_constellation_by_selector(&selector)` short-circuits with prior UUID and `already_exists: true`.
3. `resolve_members(&selector, max_members)` scans the fingerprint set once and post-filters by selector. Hard cap `SCAN_CAP = 200 000`.
4. Build `ConstellationAccumulator::with_max_members`. For each member:
   - `rocksdb_store.get_topic_profile(fp.id).await` — fail-fast on RocksDB error (no `unwrap_or_default()` — code-simplifier H-2 fix).
   - `acc.observe(fp.id, &fp.semantic, stored_profile, &SynergyMatrix::with_base_synergies())`.
   - For `Topic` selectors: `acc.observe_topic_match(self.topic_membership_matches(fp.id, topic_id))`.
5. `acc.finalize()` → `Constellation`.
6. `rocksdb_store.store_constellation(&c)` writes primary + secondary index atomically (and deletes any prior primary row pointing at the selector).

Response: `{ status: "compiled", constellation_id, label, selector_kind, member_count, coherence, purity, per_embedder_coverage }`.

### 6.2 `list_constellations` (`constellation_tools.rs:203`)

`list_constellation_ids().await` enumerates every UUID, paginates by `(offset, limit)`, then `multi_get_constellations(&page).await`. Missing rows return `{ constellation_id, missing: true }`. Response: `{ total, offset, limit, returned, constellations: [...] }`.

### 6.3 `get_constellation` (`constellation_tools.rs:271`)

Point lookup. Returns `render_constellation(&c, include_centroids)` or `{ constellation_id, found: false }`. `includeCentroids` defaults to `true` here (callers of this endpoint typically want the full record).

### 6.4 `score_against_constellation` (`constellation_tools.rs:305`)

Algorithm:
1. `get_constellation(cid)` → 404 if missing.
2. `teleological_store.retrieve(memoryId)` (works on any backend, not just RocksDB) → 404 if missing.
3. `score_memory_against_constellation(&constellation, mid, &fp.semantic)` → `ConstellationScoringResult`.

`ConstellationScoringResult` (`types.rs:232-246`):

| Field | Meaning |
|---|---|
| `constellation_id` | echo |
| `memory_id` | echo |
| `per_embedder_cosine` | `[f32; 14]` — cosine of candidate's vector against each embedder centroid; 0.0 when candidate lacks the vector or centroid is absent |
| `combined_score` | unweighted mean over entries where `coverage > 0` |
| `in_spread_p95` | `combined_score >= cosine_spread_p95` of E1 — candidate is at least as central as the 95th-percentile member |

### 6.5 `derive_constellation` (`constellation_tools.rs:405`)

Four operations sharing the same handler. All operations run a **RocksDB readback** after store and report `before_count` / `after_count` for verification.

| Operation | Behavior | Method |
|---|---|---|
| `interpolate` | `α·source + (1-α)·other` per embedder centroid | `derive_linear_constellation(..., "interpolate", alpha)` |
| `add` | `source + other` per embedder centroid | `derive_linear_constellation(..., "add", _)` |
| `difference` | `source - other` per embedder centroid | `derive_linear_constellation(..., "difference", _)` |
| `anti_pole` | rank candidates from `selector` by `combined_score` against `source`, drop members of source, take the lowest `selectedMembers`, recompile via `compile_constellation` with `selector = Tag { tag: derived_selector_tag(...) }` | `score_memory_against_constellation` for each candidate; rejects non-finite scores; requires ≥ `MIN_CONSTELLATION_MEMBERS` survivors |

For `anti_pole` the resulting selector is **always** a synthetic `Tag` so the secondary index does not collide with the source's selector. The handler returns the list of selected `(memory_id, source_combined_score)` pairs in `derivation.selected_lowest_scores` for caller inspection.

### 6.6 `delete_constellation` (`constellation_tools.rs:367`)

Returns `{ constellation_id, deleted: bool }`. Idempotent: `deleted = false` if not found. Implementation deletes both primary and secondary index entry in one `WriteBatch`. The secondary index entry is only deleted if it still points at this UUID — avoids clobbering a newer constellation that happens to share the selector (`storage/.../constellation.rs:223-248`).

## 7. Learner variant

`compile_learner_constellation` (handler `utl_tools.rs:~1327`) compiles a regulated-state baseline from `CF_FINGERPRINTS_LEARNER` rows.

Pipeline:
1. For each `session_ts` in `sessionTsList`: `store.get_learner_fingerprint(learner_id, session_ts).await` → 404 on any miss (no partial baseline).
2. `compile_constellation_from_fingerprints(learner_id, &args, &fingerprints)` aggregates:
   - per-modality centroids (`LearnerModalityCentroid { modality, vector, scalar_mean, sample_count }`) — **`SelfReport` is rejected** (`learner/mod.rs:684-691`)
   - state centroid (Welford over `state_vector.values`)
3. Persist via `store.store_learner_constellation(&c)` to `CF_LEARNER_CONSTELLATIONS` keyed by `[learner_id:16][selector_kind:u8]` (17 B). One baseline per `(learner_id, selector_kind)` pair.

`LearnerConstellation::validate` (`learner/mod.rs:656-710`) enforces:
- `sample_count > 0`
- `session_ts_end >= session_ts_start`
- at least one centroid, ≤ `LEARNER_MODALITY_COUNT = 7`
- no duplicate modalities
- no `SelfReport` centroid
- `state_centroid.learner_id == constellation.learner_id`

Counts surface via `count_learner_constellations` in `get_capability_matrix.runtime.learnerState.learnerConstellations`. See [13](13_teleological_subsystem.md) §13.1 for the full learner CF catalog.

## 8. Failure modes

| Error | Trigger | Handler response |
|---|---|---|
| `ConstellationError::TooFewMembers { count, min }` | `< 3` members observed before `finalize()` | `Too few members for a constellation (got {count}, min {min})` |
| `ConstellationError::TooManyMembers { count, max }` | observation buffer exceeded `max_members` cap during pass 1 | `Constellation accumulator rejected a member: …` |
| `ConstellationError::DimensionMismatch` | embedder vector length differs from declared dimension | `Constellation accumulator rejected a member: …` |
| `ConstellationError::DegenerateCentroid` | embedder centroid is all-zero on finalize | `finalize failed: …` |
| Selector resolves zero members | `resolve_members` returned empty | `Selector resolved zero members — nothing to compile.` |
| Already exists | `find_constellation_by_selector` hit + `rebuildIfExists = false` | `{ already_exists: true, constellation_id }` (success, not error) |
| `get_topic_profile` RocksDB read failure | RocksDB-level error | `Failed to read CF_TOPIC_PROFILES for {id}: {e}` (fail-fast, no silent fallback) |
| Non-finite anti_pole score | `combined_score.is_nan() || .is_infinite()` | `anti_pole candidate {id} produced non-finite score` |
| Anti-pole survivors `< MIN_CONSTELLATION_MEMBERS` | after dropping source members | `anti_pole requires at least 3 non-source candidates; got {n}` |
| Backend not RocksDB | any constellation tool on `InMemoryTeleologicalStore` | `<tool> requires RocksDbTeleologicalStore` |
| Version mismatch on read | first payload byte != `CONSTELLATION_VERSION = 2` | `CoreError::SerializationError` (no migration) |

## 9. Atomicity guarantees

`store_constellation` (`storage/.../constellation.rs:70-142`):
- Probes `CF_CONSTELLATION_BY_SELECTOR[index_key]` for a prior pointer (returns RocksDB error wrapped in `TeleologicalStoreError::rocksdb_op` on failure).
- If the prior pointer maps to a **different** UUID → schedules `delete_cf(primary_cf, old)` in the same WriteBatch.
- Always writes new primary row + index pointer in the same WriteBatch.
- Single `db.write(batch)` ensures all three operations land atomically.

`delete_constellation` (`storage/.../constellation.rs:209-260`) mirrors the pattern:
- Resolves the selector by reading the primary row first (the only `.await` in the function so CF handles never cross an await boundary — they are `!Sync` and `tokio::spawn` would reject the resulting future).
- Schedules `delete_cf(primary_cf, id_bytes)` in a `WriteBatch`.
- Probes the secondary index; only deletes the index entry when it still points at this UUID.
- Single `db.write(batch)` for atomicity.

`derive_constellation` performs an explicit `get_constellation(derived.id)` readback after store and reports `before_count` and `after_count` from `count_constellations()` so callers can confirm idempotent persistence.

`encode_constellation` / `decode_constellation` (`storage/.../constellation.rs:343-372`) own the wire format `[CONSTELLATION_VERSION:u8][bincode]`. `decode_constellation` rejects empty payloads and version mismatches with `CoreError::SerializationError`.

## 10. DynamicJEPA constellation variant (`CF_DJ_CONSTELLATIONS`)

A separate, namespace-aligned constellation lives inside the DynamicJEPA subsystem — used to capture per-(domain, subject, modality) instrument-reading centroids derived from `LatentPanel` artifacts. See [21](21_dynamicjepa_subsystem.md) for the surrounding pipeline.

### 10.1 Data model

`crates/context-graph-core/src/dynamicjepa/meaning_compression.rs:43-60`:

```text
pub struct ConstellationCentroid {
    pub header: DjRecordHeader,
    pub constellation_id: ConstellationId,
    pub domain_pack_id: DomainPackId,
    pub subject_id: String,                  // <= 256 bytes; trimmed-empty rejected
    pub modality_id: InstrumentId,
    pub centroid: Vec<f32>,                  // L2-normalized, |norm-1| <= 1e-3
    pub instrument_artifact_hash: [u8; 32],  // SHA-256 of the frozen instrument artifact (non-zero)
    pub reference_set_count: u32,            // > 0
    pub kept_count: u32,                     // > 0
    pub dropped_zero_norm: u32,              // kept + dropped <= reference_set_count
    pub loo_stability: f32,                  // [0, 1] cosine LOO stability
    pub calibration_percentile: u8,          // <= 100
    pub calibration_set_size: u32,           // > 0
    pub built_at_unix_ms: u64,
    pub built_by_run_id: Option<Uuid>,       // None or non-nil
}
```

`CONSTELLATION_CENTROID_RECORD_VERSION = 1`. Validation rules live in `impl Validate for ConstellationCentroid` (`meaning_compression.rs:190-277`).

### 10.2 Storage layout

| Aspect | Value | Source |
|---|---|---|
| CF name | `dj_constellations` | `dynamicjepa/column_families.rs:22` |
| CF options | `dj_composite_index_cf_options(cache)` — 8 KB blocks, LZ4, bloom, 16-byte prefix extractor, 4 MB write buffer | `dynamicjepa/column_families.rs:111-113`, `:157` |
| Key | 36 bytes: `[domain_uuid:16][SHA256(subject_id)[..16]:16][modality_id:LE u32:4]` | `dynamicjepa/keys.rs:72-81` |
| Value | `[DYNAMIC_JEPA_STORAGE_VALUE_VERSION:u8 = 1][bincode-encoded ConstellationCentroid]` | `dynamicjepa/encode.rs` |
| Audit | each batch write is paired with a `DjAuditRecord` row in `CF_DJ_AUDIT_LOG` (`audit_key = [ts_BE:8][audit_id:16]` = 24 B) | `dynamicjepa/keys.rs:115-120` |

`threshold_calibration_key(domain, subject, modality, supersede_seq)` extends the constellation key with a 4-byte supersede sequence (`keys.rs:83-93`) — keeping calibrations co-located with their constellation prefix for efficient prefix scans.

### 10.3 Read / write paths

`crates/context-graph-storage/src/dynamicjepa/constellation_store.rs`:

| Function | Behavior |
|---|---|
| `put_constellation(db, domain_uuid, subject_id, modality_id, &record)` | single-row put via `put_record` (validates record header + serializes via `encode_record`) |
| `get_constellation(db, domain_uuid, subject_id, modality_id)` | single-row read; returns `Option<ConstellationCentroid>` |
| `put_constellations_with_audit_batch(db, &records, &audit)` | validates every record + audit, writes them all into one `WriteBatch` together with the `DjAuditRecord`, executes as a single `db.write(batch)` for atomicity |
| `list_constellations(db, limit, offset)` | paginated linear scan over the CF |
| `count_constellations(db)` | O(n) scan via `count_cf` |

### 10.4 Inspection

`crates/context-graph-storage/src/dynamicjepa/inspection.rs:142-144`: `inspect_cf` and `inspect_cf_key` decode any `CF_DJ_CONSTELLATIONS` row to JSON via `decode_core_row::<ConstellationCentroid>(value, "ConstellationCentroid")`. `snapshot_dj_counts` returns `BTreeMap<String, u64>` of every DJ CF including `dj_constellations` for live counters. Unknown CF names raise `DynamicJepaError::StorageInvariantViolation`.

### 10.5 Calibration of `G_tau` thresholds

The `calibration_percentile` and `calibration_set_size` fields on `ConstellationCentroid` calibrate the per-(domain, subject, modality) acceptance threshold `G_tau` consumed downstream by the DJ surprise / acceptance pipeline. Thresholds are persisted in a dedicated CF keyed via `threshold_calibration_key(...)` (4-byte supersede sequence) so old calibrations remain auditable. The full calibration loop (reference-set selection, LOO cosine stability, percentile-based threshold derivation, supersede-on-drift) is documented in [21](21_dynamicjepa_subsystem.md). The relationship to the content store is one-way — DJ centroids never feed back into `CF_CONSTELLATIONS` and vice versa.

### 10.6 Relationship to `LatentPanel`

The DynamicJEPA pipeline materializes `LatentPanel` records (`core/dynamicjepa/panel.rs:13-28`) — per-event slot vectors (state / action / outcome / time) keyed by `panel_id` and stored in `CF_DJ_LATENT_PANELS`. Each panel references multiple `instrument_reading_ids` and `pairwise_reading_ids`. `ConstellationCentroid` rows are derived from the population of instrument readings for a `(domain_pack_id, subject_id, modality_id)` tuple — they compress those readings into a single L2-normalized centroid plus calibration statistics that downstream `ThresholdCalibration` rows consume.

The DJ constellation has **no relationship** to `Constellation` from §4 — they share a name and the conceptual "centroid of a member set" idea but use entirely different keys, value formats, validation rules, and audit logs.

## 11. What is NOT covered here

- The synergy matrix used during compilation — see [13](13_teleological_subsystem.md) §7.
- The 14-embedder fingerprint shape (E1-E14, dual asymmetric vectors) — see [05](05_embedding_subsystem.md).
- How learner constellations feed `resolve_learner_retrieval_policy` and drift detection — see [20](20_learning_subsystem.md).
- How `derive_anomalies_from_edges` produces contrastive pairs from typed edges (often paired with constellation outputs in training pipelines) — see [18](18_contrastive_and_typed_edges.md).
- Topic clustering and stability metrics consumed by `purity` — see [12](12_clustering_and_topics.md).
- DynamicJEPA pipeline overview, panel materialization, instrument/adapter registries, full G_tau calibration loop — see [21](21_dynamicjepa_subsystem.md).
