# Constellation Subsystem

**Source files covered:**
- `crates/context-graph-core/src/constellation/{mod,types,compiler,welford,reservoir}.rs` — types, accumulator, compiler, Welford online stats, reservoir percentiles
- `crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs` — `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` persistence + secondary-index management
- `crates/context-graph-storage/src/teleological/rocksdb_store/learner.rs:124-143, 283-287` — `CF_LEARNER_CONSTELLATIONS` storage methods
- `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs` — six MCP handlers
- `crates/context-graph-mcp/src/handlers/tools/utl_tools.rs:1327+` — `compile_learner_constellation` handler
- `crates/context-graph-mcp/src/tools/definitions/constellation.rs` — schema definitions

Cross-references: see [04](04_database_schema.md) for CF layout, [13](13_teleological_subsystem.md) for the teleological context that feeds constellation compilation, [18](18_contrastive_and_typed_edges.md) for `derive_constellation anti_pole`'s consumption of fingerprint scores, [20](20_learning_subsystem.md) for the learner-state side.

## 1. Scope

A **constellation** is a compiled summary of a set of memories selected by topic, session, tag, time range, or explicit id list. It captures, for one selector resolution:

- per-embedder dense centroids and spread statistics (mean, stddev, p50/p95 cosine, min/max cosine, coverage)
- aggregate 14D topic-profile centroid, 6D group-alignment centroid, 91D cross-correlation centroid
- coherence (median pairwise cosine to E1 centroid) and topic purity (when selector is `Topic`)

Two stores share the design and the `Constellation` shape:

| Store | CF | Key | Built by | Reader |
|---|---|---|---|---|
| **Content** | `CF_CONSTELLATIONS` + secondary index `CF_CONSTELLATION_BY_SELECTOR` | UUID | `compile_constellation`, `derive_constellation` | `list_constellations`, `get_constellation`, `score_against_constellation`, `delete_constellation` |
| **Learner** (regulated-state baseline) | `CF_LEARNER_CONSTELLATIONS` | `[learner_id:16][selector_kind:u8]` (17B) | `compile_learner_constellation` | scope-restricted to a learner; consumed by `resolve_learner_retrieval_policy`, drift detection, and learner training pipelines |

This document focuses on the content store. The learner variant is summarized in §7 and detailed in [13](13_teleological_subsystem.md) §4 and [20](20_learning_subsystem.md).

## 2. Constants

`crates/context-graph-core/src/constellation/mod.rs:48-65` and `types.rs:20-30`:

| Constant | Value | Meaning |
|---|---|---|
| `CONSTELLATION_VERSION` | 2 | First byte of every `CF_CONSTELLATIONS` payload; v2 = E14-aware (14 per-embedder stats, 14D topic centroid, 91-entry cross-correlation centroid) |
| `RESERVOIR_SAMPLE_SIZE` | 1024 | Bounded reservoir per embedder for percentile estimation |
| `MIN_CONSTELLATION_MEMBERS` | 3 | Below this `finalize` returns `ConstellationError::TooFewMembers` |
| `DEFAULT_MAX_MEMBERS` | 50_000 | Default ceiling on members fed to one accumulator (`compiler.rs:46`) |
| `NUM_CONSTELLATION_EMBEDDERS` | 14 | = `teleological::types::NUM_EMBEDDERS` |
| `TOPIC_PROFILE_CENTROID_DIM` | 14 | Mean topic-profile alignment array |
| `GROUP_ALIGNMENT_CENTROID_DIM` | 6 | Factual / Temporal / Causal / Relational / Qualitative / Implementation |
| `CROSS_CORRELATION_CENTROID_DIM` | 91 | C(14, 2) cross-embedder pairs |
| `SELECTOR_INDEX_KEY_LEN` | 17 | `[kind:u8][SHA256(canonical)[..16]:16B]` |

Wire format: `[CONSTELLATION_VERSION:u8][bincode-encoded Constellation]`. Deserialization rejects mismatched versions with `CoreError::SerializationError` — **no automatic migration**.

## 3. Selectors

Five variants, each with a stable canonical string used for SHA-256 hashing into the secondary index (`types.rs:105-166`):

| Variant | Canonical form | `kind_byte` |
|---|---|---|
| `Topic { topic_id }` | `topic:{topic_id}` | 0 |
| `Session { session_id }` | `session:{session_id}` | 1 |
| `Tag { tag }` | `tag:{tag}` | 2 |
| `TimeRange { start, end }` | `timerange:{start_rfc3339_nanos}-{end_rfc3339_nanos}` | 3 |
| `ExplicitIds { rationale, ids }` | `explicit:{sorted_dedup_uuids_joined_by_comma}` | 4 |

For `ExplicitIds` the `rationale` is intentionally **not** hashed — only the (sorted, deduplicated) UUID set determines identity. For `TimeRange` the timestamps emit RFC-3339 with nanosecond precision so tooling cannot accidentally collide two different windows.

Secondary-index key: `[kind_byte:u8][SHA256(canonical)[..16]]` → 17B fixed → constellation UUID (16B value). `compile_constellation` enforces "at most one primary row per selector": when the index pointer already maps to a different UUID (rebuild case), the prior primary row is **deleted in the same WriteBatch** as the new put (`storage/.../constellation.rs:80-110`).

## 4. `Constellation` record

`crates/context-graph-core/src/constellation/types.rs:36-98`:

| Field | Shape | Notes |
|---|---|---|
| `id` | UUID | Fresh per compile; distinct from any member UUID |
| `label` | String | Caller-supplied tag (e.g. `"PRD §7 — Case management"`) |
| `created_at` | `DateTime<Utc>` | wall-clock at `finalize()` |
| `selector` | `ConstellationSelector` | One of §3 |
| `member_ids` | `Vec<Uuid>` | Bounded by `max_members` accumulator cap |
| `member_count` | usize | `member_ids.len()` |
| `per_embedder` | `Vec<EmbedderStats>` | length = `NUM_CONSTELLATION_EMBEDDERS = 14` |
| `topic_profile_centroid` | `[f32; 14]` | Mean topic alignments (silent-zero on fresh DB — see §4.2) |
| `group_alignment_centroid` | `[f32; 6]` | Mean group alignments |
| `cross_correlation_centroid` | `Vec<f32>` length 91 | Mean 14×14 cross-correlation lower-triangle |
| `coherence` | f32 in [-1, 1] | Median (p50) cosine of member E1 vectors against the E1 centroid; **median, not mean**, so robust to outliers |
| `purity` | `Option<f32>` in [0, 1] | Populated only for `selector = Topic { .. }`; ratio of members whose dominant topic matches |

### 4.1 `EmbedderStats`

`types.rs:188-229`:

| Field | Type | Meaning |
|---|---|---|
| `embedder_index` | u8 | 0..14 |
| `dimension` | u16 | Dense dim for `Dense` / `Asymmetric`; 0 for `Sparse` / `TokenLevel` |
| `vector_kind` | `VectorKind` | `Dense`, `Sparse`, `TokenLevel`, `Asymmetric` |
| `centroid` | `Vec<f32>` | Mean dense vector; non-empty only for `Dense` / `Asymmetric` |
| `sparse_top_terms` | `Vec<(u16, f32)>` | Top 50 term ids by mean weight; sparse-only |
| `mean_token_count` | `Option<f32>` | E12 only |
| `pooled_token_centroid` | `Vec<f32>` 128D | E12 only |
| `mean_l2`, `stddev_l2` | f32 | Mean and stddev of member L2 norms |
| `cosine_spread_p50`, `cosine_spread_p95` | f32 | Reservoir-sampled cosine percentiles vs centroid |
| `min_cosine`, `max_cosine` | f32 | Observed extremes |
| `coverage` | f32 in [0, 1] | Fraction of members with a non-zero vector for this embedder |

### 4.2 Silent-zero invariant on topic centroid

`Constellation::has_topic_profile_signal()` returns `true` iff at least one of the 14 slots is non-zero. **Reason** (per the doc-comment on `topic_profile_centroid`): the compiler only observes the topic profile when `RocksDbTeleologicalStore::get_topic_profile(member_id)` returns `Some`. On a fresh DB, before the clustering pipeline has populated `CF_TOPIC_PROFILES`, every member yields `None`, the Welford accumulator stays at `count = 0`, and the field serializes as all-zeros — **indistinguishable from "all members had topic-zero alignment"**. Consumers that condition on topic-centroid trustworthiness MUST call `has_topic_profile_signal()`.

`group_alignment_centroid` and `cross_correlation_centroid` are derived from `topic_profile_centroid` and inherit the same caveat.

## 5. Aggregation algorithms

`crates/context-graph-core/src/constellation/{compiler,welford,reservoir}.rs`:

| Pass | Per-member work | State |
|---|---|---|
| Dense centroids (E1, E2, E3, E4, E7, E8, E9, E11, E14) | Add to `WelfordVector` for each populated embedder | Online mean (single pass), variance via Welford |
| Asymmetric (E5 cause / E8 source / E10 paraphrase) | Add **only the canonical side** to its dedicated `WelfordVector` | Same |
| Sparse (E6, E13) | Merge-add term-frequency counts | `HashMap<term_id, (sum_weight, occurrences)>` → top 50 by mean |
| Token-level (E12) | Mean-pool tokens per member to 128D, then add to `WelfordVector` | Pooled token centroid |
| Cosine spread per embedder | Sample cosine(member, current_centroid) into a `ReservoirSample` of size 1024 | p50, p95, min, max |
| Topic profile | Conditional: only when `stored_topic_profile = Some(_)` from `CF_TOPIC_PROFILES` | Welford over 14D |
| Group alignment | Derived from observed topic profile via `GroupAlignments::from_topic_profile` | Welford over 6D |
| Cross-correlation | Lower-triangle 91 entries from observed topic profile | Welford over 91D |
| Coherence | Reservoir of cosine(member.E1, centroid.E1); **median**, not mean | Reservoir of size 1024 |
| Purity (Topic only) | Count of members whose dominant topic matches `selector.topic_id` | u32 / total |

`ConstellationAccumulator::observe(memory_id, fingerprint, stored_profile, &SynergyMatrix)` is the streaming entry-point. `finalize()` returns `Result<Constellation, ConstellationError>` — fails with `TooFewMembers { count, min }` when fewer than `MIN_CONSTELLATION_MEMBERS = 3` were observed.

## 6. MCP tools (six)

Handlers: `crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs`. All six downcast `Handlers::teleological_store` to `RocksDbTeleologicalStore` and return `"requires RocksDbTeleologicalStore"` otherwise.

### 6.1 `compile_constellation` (`constellation_tools.rs:42`)

| Arg | Type | Required | Default | Notes |
|---|---|---|---|---|
| `label` | string (non-empty) | yes | — | Caller-supplied tag |
| `selector` (one of) | `topicId` / `sessionId` / `tag` / `timeRange{start,end}` / `explicitIds{rationale,ids}` | yes | — | parsed by `parse_selector` |
| `maxMembers` | u64 | no | `DEFAULT_MAX_MEMBERS` | clamped to [3, 100_000] |
| `rebuildIfExists` | bool | no | `false` | When `false` and a constellation already exists for the selector, returns `{ already_exists: true, constellation_id }` |

Pipeline (`constellation_tools.rs:42-200`):
1. `parse_selector(args)` → `ConstellationSelector`.
2. If `!rebuildIfExists` → `find_constellation_by_selector(&selector)` short-circuits with prior UUID.
3. `resolve_members(&selector, max_members)` scans the fingerprint set once, post-filters by selector. Hard cap `SCAN_CAP = 200_000` (defined at top of file).
4. Build `ConstellationAccumulator::with_max_members`. For each member:
   - `rocksdb_store.get_topic_profile(fp.id).await` → fail-fast on RocksDB error (no `unwrap_or_default()` — code-simplifier H-2 fix).
   - `acc.observe(fp.id, &fp.semantic, stored_profile, &SynergyMatrix::with_base_synergies())`.
   - For `Topic` selectors: `acc.observe_topic_match(self.topic_membership_matches(fp.id, topic_id))`.
5. `acc.finalize()` → `Constellation`.
6. `rocksdb_store.store_constellation(&c)` writes primary + secondary index atomically (and deletes any prior primary row pointing at the selector).

Response: `{ status, constellation_id, label, selector_kind, member_count, coherence, purity, per_embedder_coverage }`.

### 6.2 `list_constellations` (`constellation_tools.rs:203`)

| Arg | Type | Default |
|---|---|---|
| `limit` | u64 | 50 (clamped to [1, 1000]) |
| `offset` | u64 | 0 |
| `includeCentroids` | bool | `false` |

Reads `list_constellation_ids().await` → in-order UUID list, paginates, then `multi_get_constellations(&page).await`. Missing rows return `{ constellation_id, missing: true }`. Response: `{ total, offset, limit, returned, constellations: [...] }`.

### 6.3 `get_constellation` (`constellation_tools.rs:271`)

| Arg | Type | Required | Default |
|---|---|---|---|
| `constellationId` | UUID string | yes | — |
| `includeCentroids` | bool | no | `true` |

Returns `render_constellation(&c, include_centroids)` or `{ constellation_id, found: false }`.

### 6.4 `score_against_constellation` (`constellation_tools.rs:305`)

| Arg | Type | Required |
|---|---|---|
| `constellationId` | UUID | yes |
| `memoryId` | UUID | yes |

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
| `combined_score` | Unweighted mean over entries where `coverage > 0` |
| `in_spread_p95` | `combined_score >= cosine_spread_p95` of E1 — candidate is at least as central as the 95th-percentile member |

### 6.5 `derive_constellation` (`constellation_tools.rs:405`)

Four operations sharing the same handler. All operations run a **RocksDB readback** after store and report `before_count`, `after_count` for verification.

| Arg | Type | Required | Notes |
|---|---|---|---|
| `operation` | string | yes | One of `interpolate`, `add`, `difference`, `anti_pole` |
| `label` | string (non-empty) | yes | Label of the new derived constellation |
| `sourceConstellationId` | UUID | yes | Source operand |
| `otherConstellationId` | UUID | for `interpolate` / `add` / `difference` | Other operand |
| `alpha` | f32 in [0, 1] | optional, default 0.5 | Interpolation weight |
| `selector` | (selector args) | for `anti_pole` | Candidate pool selector |
| `maxCandidates` | u64 | optional, default 10_000 | Clamp [`MIN_CONSTELLATION_MEMBERS`, 200_000] |
| `selectedMembers` | u64 | optional, default 50 | Clamp [`MIN_CONSTELLATION_MEMBERS`, 100_000] |

Operations:

| Operation | Behavior | Method |
|---|---|---|
| `interpolate` | `α·source + (1-α)·other` per embedder centroid | `derive_linear_constellation(..., "interpolate", alpha)` |
| `add` | `source + other` per embedder centroid | `derive_linear_constellation(..., "add", _)` |
| `difference` | `source - other` per embedder centroid | `derive_linear_constellation(..., "difference", _)` |
| `anti_pole` | Rank candidates from `selector` by `combined_score` against `source`, **drop** members of source, take the lowest `selectedMembers`, recompile via `compile_constellation` with `selector = Tag { tag: derived_selector_tag(...) }` | `score_memory_against_constellation` for each candidate; rejects non-finite scores; requires ≥ `MIN_CONSTELLATION_MEMBERS` survivors |

For `anti_pole` the resulting selector is **always** a synthetic `Tag` so the secondary index does not collide with the source's selector. The handler returns the list of selected `(memory_id, source_combined_score)` pairs in `derivation.selected_lowest_scores` for caller inspection.

### 6.6 `delete_constellation` (`constellation_tools.rs:367`)

| Arg | Type | Required |
|---|---|---|
| `constellationId` | UUID | yes |

Returns `{ constellation_id, deleted: bool }`. Idempotent: `deleted=false` if not found. Implementation deletes both primary and secondary index entry in one `WriteBatch`.

## 7. Learner variant

`compile_learner_constellation` (handler `utl_tools.rs:1327+`) compiles a regulated-state baseline from `CF_FINGERPRINTS_LEARNER` rows.

| Arg | Type | Required | Default |
|---|---|---|---|
| `learnerId` | UUID | yes | — |
| `sessionTsList` | u64[] (non-empty) | yes | — |
| `selectorKind` | u8 | optional | `LEARNER_BASELINE_SELECTOR_REGULATED = 1` |
| `label` | string | optional | derived |

Pipeline:
1. For each `session_ts` in `sessionTsList`: `store.get_learner_fingerprint(learner_id, session_ts).await` → 404 on any miss (no partial baseline).
2. `compile_constellation_from_fingerprints(learner_id, &args, &fingerprints)` aggregates:
   - per-modality centroids (`LearnerModalityCentroid { modality, vector, scalar_mean, sample_count }`) — **`SelfReport` is rejected** (`learner/mod.rs:684-691`)
   - state centroid (Welford over `state_vector.values`)
3. Persist via `store.store_learner_constellation(&c)` to `CF_LEARNER_CONSTELLATIONS` keyed by `[learner_id:16][selector_kind:u8]` (17B). One baseline per `(learner_id, selector_kind)` pair.

`LearnerConstellation::validate` (`learner/mod.rs:656-710`) enforces:
- `sample_count > 0`
- `session_ts_end >= session_ts_start`
- at least one centroid, ≤ `LEARNER_MODALITY_COUNT = 7`
- no duplicate modalities
- no `SelfReport` centroid
- `state_centroid.learner_id == constellation.learner_id`

Counts surface via `count_learner_constellations` in `get_capability_matrix.runtime.learnerState.learnerConstellations`.

## 8. Failure modes

| Error | Trigger | Handler response |
|---|---|---|
| `ConstellationError::TooFewMembers { count, min }` | `<3` members observed before `finalize()` | `Too few members for a constellation (got {count}, min {min})` |
| Selector resolves zero members | `resolve_members` returned empty | `Selector resolved zero members — nothing to compile.` |
| Already exists | `find_constellation_by_selector` hit + `rebuildIfExists=false` | `{ already_exists: true, constellation_id }` (success, not error) |
| `get_topic_profile` RocksDB read failure | RocksDB-level error | `Failed to read CF_TOPIC_PROFILES for {id}: {e}` (fail-fast, do not silently fall back) |
| Non-finite anti_pole score | `combined_score.is_nan() || .is_infinite()` | `anti_pole candidate {id} produced non-finite score` |
| Anti-pole survivors `< MIN_CONSTELLATION_MEMBERS` | After dropping source members | `anti_pole requires at least 3 non-source candidates; got {n}` |
| Backend not RocksDB | Any constellation tool on `InMemoryTeleologicalStore` | `<tool> requires RocksDbTeleologicalStore` |
| Version mismatch on read | First payload byte != `CONSTELLATION_VERSION = 2` | `CoreError::SerializationError` (no migration) |

## 9. Atomicity guarantees

`store_constellation` (`storage/.../constellation.rs:70-130`):
- Probes `CF_CONSTELLATION_BY_SELECTOR[index_key]` for a prior pointer.
- If the prior pointer maps to a **different** UUID than the new record → schedules `delete_cf(primary_cf, old)` in the same WriteBatch.
- Always writes new primary row + index pointer in the same WriteBatch.
- Single `db.write(batch)` ensures all three operations land atomically.

`delete_constellation` mirrors the pattern (deletes both rows in one WriteBatch).

`derive_constellation` performs an explicit `get_constellation(derived.id)` readback after store and reports `before_count` and `after_count` from `count_constellations()` so callers can confirm idempotent persistence.

## 10. What is NOT covered here

- The synergy matrix used during compilation — see [13](13_teleological_subsystem.md) §9.
- The 14-embedder fingerprint shape (E1–E14, dual asymmetric vectors) — see [05](05_embedding_subsystem.md).
- How learner constellations feed `resolve_learner_retrieval_policy` and drift detection — see [20](20_learning_subsystem.md).
- How `derive_anomalies_from_edges` produces contrastive pairs from typed edges (often paired with constellation outputs in training pipelines) — see [13](13_teleological_subsystem.md) §7 and [18](18_contrastive_and_typed_edges.md).
- Topic clustering and stability metrics consumed by `purity` — see [12](12_clustering_and_topics.md).
