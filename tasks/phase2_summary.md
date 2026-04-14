# Phase 2 — Agent B summary

## Build status
cargo build --release -p context-graph-mcp: PASS
- errors: 0
- warnings: 0 (the `warning:` lines are NVCC PTX build-script info notes from `context-graph-cuda`, not Rust compiler warnings)
- elapsed: 1m13s (final MCP crate compile)
- env: `CUDARC_CUDA_VERSION=13000 CUDA_HOME=/usr/local/cuda-13.1 PATH=/usr/local/cuda-13.1/bin:$PATH`

## Tests

context-graph-core `constellation::`: PASS
- passed: 19 / 19
- covers: welford stats (scalar + vector), reservoir sampling (uniform-convergence, non-finite handling), compiler rejection of too-few-members, identical-members-coherence-one, two-clusters intermediate coherence, own-member high score, unrelated-memory low score, 13-embedder shape, purity (topic selector), selector canonical form stability

context-graph-storage `constellation_integration`: PASS
- passed: 10 / 10
- covers: field-by-field roundtrip (all 13 EmbedderStats), find-by-selector lookup, delete cleans both CFs + idempotency, duplicate-members coherence ≥ 0.99, two-clusters intermediate p50, too-few-members error, own-member in-spread-p95 true, unrelated (orthogonal E1) in-spread-p95 false, list/count/multi_get with aligned slices

context-graph-storage `teleological::rocksdb_store::constellation::tests` (inline unit tests): PASS
- passed: 5 / 5
- covers: encode/decode roundtrip, decode rejects empty, decode rejects wrong version byte, selector index key is stable, selector index keys distinct by kind

context-graph-mcp `tools::definitions::`: PASS
- passed: 31 / 31
- key assertions: `test_total_tool_count_and_no_duplicates` (63 w/o LLM, 67 w/ LLM) and `test_submodule_counts` (`constellation::definitions().len() == 5`) both green

Adjacent sanity checks (all green):
- `column_families::tests::test_total_column_families_constant` → 54 (1 passed)
- `teleological::tests::column_family::*` → 14 passed (including `test_get_all_teleological_cf_descriptors_returns_36` and `test_teleological_cf_names_count` → 23)
- Phase 1 `context-graph-core training::` → 18 passed
- Phase 1 `context-graph-storage training_records_integration` → 13 passed

## Files created
- /home/cabdru/contextgraph/crates/context-graph-core/src/constellation/mod.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/constellation/types.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/constellation/welford.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/constellation/reservoir.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/constellation/compiler.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/constellation/tests.rs
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store/constellation.rs
- /home/cabdru/contextgraph/crates/context-graph-storage/tests/constellation_integration.rs
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/constellation.rs
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/constellation_tools.rs

## Files modified
- /home/cabdru/contextgraph/crates/context-graph-core/src/lib.rs — added `pub mod constellation`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/column_families.rs — added `CF_CONSTELLATIONS` + `CF_CONSTELLATION_BY_SELECTOR` constants, `constellations_cf_options` + `constellation_by_selector_cf_options`, appended both to `TELEOLOGICAL_CFS`, bumped `TELEOLOGICAL_CF_COUNT` 21 → 23, added both descriptors to `get_teleological_cf_descriptors`, updated 34 → 36 docstring in `get_all_teleological_cf_descriptors`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/column_families.rs — bumped `TOTAL_COLUMN_FAMILIES` 52 → 54 and updated the arithmetic comment; updated `test_total_column_families_constant` assertion
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/mod.rs — re-exported `CF_CONSTELLATIONS`, `CF_CONSTELLATION_BY_SELECTOR`, `constellations_cf_options`, `constellation_by_selector_cf_options`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs — added `mod constellation;`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/tests/column_family.rs — updated `test_teleological_cf_names_count` (21 → 23), renamed `test_get_all_teleological_cf_descriptors_returns_34` → `_36` and updated body
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs — added 5 constellation tool name constants
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/mod.rs — registered `pub(crate) mod constellation`, added `tools.extend(constellation::definitions())`, bumped tool count 58 → 63 (no LLM) / 62 → 67 (with LLM), added `constellation::definitions().len() == 5` submodule assertion
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs — added `mod constellation_tools;`
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/dispatch.rs — added 5 dispatch arms

## Key signatures added

Core (`context_graph_core::constellation`):
- `pub const CONSTELLATION_VERSION: u8 = 1;`
- `pub const RESERVOIR_SAMPLE_SIZE: usize = 1024;`
- `pub const MIN_CONSTELLATION_MEMBERS: usize = 3;`
- `pub const NUM_CONSTELLATION_EMBEDDERS: usize = 13;`
- `pub const TOPIC_PROFILE_CENTROID_DIM: usize = 13;`
- `pub const GROUP_ALIGNMENT_CENTROID_DIM: usize = 6;`
- `pub const CROSS_CORRELATION_CENTROID_DIM: usize = 78;`
- `pub const DEFAULT_MAX_MEMBERS: usize = 50_000;`
- `pub struct Constellation { id, label, created_at, selector, member_ids, member_count, per_embedder: Vec<EmbedderStats>, topic_profile_centroid: [f32;13], group_alignment_centroid: [f32;6], cross_correlation_centroid: Vec<f32>, coherence, purity: Option<f32> }`
- `pub enum ConstellationSelector { Topic { topic_id }, Session { session_id }, Tag { tag }, TimeRange { start, end }, ExplicitIds { rationale, ids: Vec<Uuid> } }` + `canonical_form()` + `kind_byte()` accessors
- `pub enum VectorKind { Dense, Sparse, TokenLevel, Asymmetric }`
- `pub struct EmbedderStats { embedder_index, dimension, vector_kind, centroid, sparse_top_terms: Vec<(u16,f32)>, mean_token_count: Option<f32>, pooled_token_centroid: Vec<f32>, mean_l2, stddev_l2, cosine_spread_p50/p95, min/max_cosine, coverage }`
- `pub struct ConstellationScoringResult { constellation_id, memory_id, per_embedder_cosine: [f32;13], combined_score, in_spread_p95 }`
- `pub enum ConstellationError { TooFewMembers, TooManyMembers, DimensionMismatch, DegenerateCentroid }`
- `pub struct WelfordStats` (scalar) + `pub struct WelfordVector` (per-coordinate, panics on dim mismatch, skips NaN/Inf)
- `pub struct ReservoirSample` (seeded Algorithm R, f32-only, drops non-finite)
- `pub struct ConstellationAccumulator` with `new`, `with_max_members`, `observe`, `observe_topic_match`, `finalize`
- `pub fn compile_constellation(selector, label, iterator_of (id, fp, stored_profile), &synergy) -> Result<Constellation, ConstellationError>`
- `pub fn score_memory_against_constellation(&Constellation, Uuid, &SemanticFingerprint) -> ConstellationScoringResult`

Storage (`RocksDbTeleologicalStore` on `context_graph_storage::teleological`):
- `pub async fn store_constellation(&self, c: &Constellation) -> CoreResult<()>` — atomic `WriteBatch` over primary + secondary index CFs
- `pub async fn get_constellation(&self, id: Uuid) -> CoreResult<Option<Constellation>>`
- `pub async fn list_constellation_ids(&self) -> CoreResult<Vec<Uuid>>`
- `pub async fn count_constellations(&self) -> CoreResult<usize>`
- `pub async fn delete_constellation(&self, id: Uuid) -> CoreResult<bool>` — atomic, `Send`-safe (resolves selector before acquiring CF handles)
- `pub async fn find_constellation_by_selector(&self, selector: &ConstellationSelector) -> CoreResult<Option<Uuid>>`
- `pub async fn multi_get_constellations(&self, ids: &[Uuid]) -> CoreResult<Vec<Option<Constellation>>>` — batched via `multi_get_cf`

Storage CFs:
- `pub const CF_CONSTELLATIONS: &str = "constellations";` — primary UUID-keyed store, `uuid_prefixed_cf_options(cache, 64, 8)`
- `pub const CF_CONSTELLATION_BY_SELECTOR: &str = "constellation_by_selector";` — secondary index, `[kind_byte: u8][sha256(canonical_form)[..16]: 16B]` → UUID, 1-byte prefix extractor, 2MB write buffer

MCP (5 tools, registered in `tools/definitions/constellation.rs`, `handlers/tools/constellation_tools.rs`, `dispatch.rs`):
- `compile_constellation` — selectors: topic, session, tag, time_range, explicit_ids. Required: `selector`, `label`. Optional: `topicId`/`sessionId`/`tag`/`startIso`+`endIso`/`memoryIds`/`rationale`, `maxMembers` (default 50000), `rebuildIfExists` (default false)
- `list_constellations` — `limit` (default 50, max 1000), `offset` (default 0), `includeCentroids` (default false)
- `get_constellation` — required `constellationId`, optional `includeCentroids` (default **true**)
- `score_against_constellation` — required `constellationId` + `memoryId`
- `delete_constellation` — required `constellationId`

Every MCP schema has `additionalProperties: false`.

## Plan §4.6 FSV status

- **FSV-7** — Constellation from 5 synthetic memories that are duplicates → coherence ≈ 1.0 on every dense embedder, p50 ≈ 1.0: **PASS**
  - Covered by `constellation_integration::compile_with_duplicate_members_yields_coherence_one` (uses 10 duplicates; asserts coherence ≥ 0.99, E1 p50 ≥ 0.99, then roundtrips through RocksDB and re-asserts coherence preserved to 1e-6).
  - Also covered by `core::constellation::tests::identical_members_give_coherence_near_one` which verifies every one of the 13 embedders independently.

- **FSV-8** — Constellation from 2 clusters of identical memories → E1 coherence ≠ 1.0, indicating a non-degenerate centroid: **PASS**
  - Covered by `constellation_integration::compile_with_two_clusters_yields_mid_coherence` (E1 p50 in [0.3, 0.9999], *not* pinned to 1.0).
  - Note: synthetic fp seeds produce vectors that are correlated but not collinear; the explicit bound is a "not degenerate" check, not a narrow numerical expectation. Real-world E1 coherence for 2 distinct topics drops to ~0.4–0.7 as the plan predicts, but synthetic data doesn't reach that spread without the real embedding model.

- **FSV-9** — Score a known-member vs an unrelated memory → in_spread_p95 true / false: **PASS**
  - Covered by `constellation_integration::score_own_member_matches_centroid_highly` (own member: combined_score ≥ 0.99, in_spread_p95 == true) and `score_unrelated_memory_low` (orthogonal E1: cosine ≈ 0, in_spread_p95 == false).

- **FSV-10** — Roundtrip the constellation through RocksDB and assert every field survives: **PASS**
  - Covered by `constellation_integration::roundtrip_constellation_preserves_all_fields` (hand-built Constellation with every VectorKind represented; after `store` + `get` we compare all 13 EmbedderStats fields individually plus the three centroid arrays, coherence, purity, member_ids, and label).

## Known limitations / deferred items
- **Tag selector semantics**: `ConstellationSelector::Tag { tag }` matches against `SourceMetadata.tool_name`. `SourceMetadata` has no literal `tags: Vec<String>` field today. The handler's description documents the mapping; a proper tag dimension would need a new SourceMetadata field + a new CF for per-tag secondary indexing. Flagged; out of scope for Phase 2.
- **Tag compile path**: because "tag" maps to `tool_name`, compiling a Tag constellation on a database where no memories have that tool_name yields zero members and returns an MCP tool error ("Selector resolved zero members — nothing to compile.").
- **Topic selector resolution**: a Topic constellation currently *includes* every fingerprint in the scan and uses `observe_topic_match` to compute purity (the fraction actually tagged with the topic in the loaded portfolio). This matches the plan's intent ("purity = topic match rate") but means Topic constellations contain non-topic memories until a membership-aware filter is wired. To restrict to *only* topic members, pass `selector=explicit_ids` with the topic's `member_memories` instead. Document in §7 of the caller's README.
- **Streaming compile**: the accumulator buffers all member observations in memory through pass 2 (needed for cosine-against-centroid sampling). A 50 000-member × 13-dense-embedder constellation can transiently reach ≈ 2.5 GB. For larger sets, the caller must lower `maxMembers` or pre-filter; we intentionally fail fast rather than paging to disk. The two-pass design is documented in the `compiler.rs` module header.
- **Coherence formula**: we report `cosine_spread_p50` of E1 as `coherence`, not the arithmetic mean of cosines. The reservoir keeps 1024 samples so mean would require a second accumulator; median is a robust proxy and is what the FSV thresholds assert.
- **ColBERT E12 pooling**: per-member tokens are mean-pooled into one 128D vector before centroid aggregation. This is simpler than the full MaxSim-style aggregation used in retrieval but matches the "pooled 128D" contract in `EmbedderStats::pooled_token_centroid`.
- **`rebuildIfExists=false` behavior**: when an existing constellation matches the selector, `compile_constellation` returns `{ already_exists: true, constellation_id }` without rebuilding. To force a refresh pass `rebuildIfExists=true`.
- **No Phase 1 modifications**: no files under `context-graph-core/src/training/` or `context-graph-storage/.../rocksdb_store/export.rs` or `handlers/tools/training_tools.rs` were touched. No Phase 1 bugs were encountered while building on top of Phase 1 outputs.

## Memory for next agent (use verbatim)

Title: IMPL-PHASE-2-DONE

Content:
Phase 2 (Constellation compiler) is complete and green.

CFs: two new — `CF_CONSTELLATIONS` (primary, UUID-keyed, uuid_prefixed options 64KB/8MB) and `CF_CONSTELLATION_BY_SELECTOR` (secondary index, `[kind:u8][sha256[..16]:16B]`→UUID, 1-byte prefix extractor, 2MB buffer). Totals bumped: 21→23 teleological, 52→54 overall, 34→36 teleological+quantized.

Types (crate context_graph_core::constellation, all bincode-safe, no skip_serializing_if):
- `Constellation` with `id`, `label`, `created_at`, `selector`, `member_ids/count`, `per_embedder: Vec<EmbedderStats>` (length-13), `topic_profile_centroid: [f32;13]`, `group_alignment_centroid: [f32;6]`, `cross_correlation_centroid: Vec<f32>` (78), `coherence: f32`, `purity: Option<f32>`.
- `ConstellationSelector::{Topic, Session, Tag, TimeRange, ExplicitIds}` with `canonical_form() -> String` (deterministic, used for SHA-256 index hash; sorts+dedups `ExplicitIds.ids`; `rationale` is NOT hashed) and `kind_byte() -> u8` (0..4).
- `VectorKind::{Dense, Sparse, TokenLevel, Asymmetric}`.
- `EmbedderStats { embedder_index, dimension, vector_kind, centroid, sparse_top_terms: Vec<(u16,f32)>, mean_token_count: Option<f32>, pooled_token_centroid, mean_l2, stddev_l2, cosine_spread_p50/p95, min/max_cosine, coverage }`.
- `ConstellationScoringResult { constellation_id, memory_id, per_embedder_cosine: [f32;13], combined_score, in_spread_p95 }`.
- `ConstellationError` (thiserror) variants: `TooFewMembers`, `TooManyMembers`, `DimensionMismatch`, `DegenerateCentroid`.

Compiler:
- `ConstellationAccumulator::new(selector, label)` and `with_max_members(selector, label, max)`. Default `max_members = 50_000`.
- `observe(id, fp, stored_topic_profile: Option<[f32;13]>, &synergy)` — two-pass internally (pass 1 accumulates + buffers, pass 2 in `finalize` computes cosine-to-centroid into reservoirs).
- `observe_topic_match(bool)` — optional, used only for `Topic` selectors to compute `purity`.
- `finalize()` — returns `Err(TooFewMembers)` below `MIN_CONSTELLATION_MEMBERS=3`.
- `compile_constellation(selector, label, members_iter, &synergy)` wrapper for the common case.
- `score_memory_against_constellation(&Constellation, Uuid, &SemanticFingerprint)` — used by the score_against_constellation MCP tool.

Key correctness properties:
- All asymmetric embedders use the cause/source/paraphrase side: E5 `e5_causal_as_cause`, E8 `e8_graph_as_source`, E10 `e10_multimodal_paraphrase`.
- Dense embedder dimensions hardcoded: E1=1024, E2/3/4=512, E5=768, E7=1536, E8=1024, E9=1024, E10=768, E11=768. E12 is pooled to 128D. E6/E13 are sparse (dimension=0).
- Zero vectors and non-finite vectors are dropped (coverage decreases) rather than poisoning the centroid.
- Welford per-coordinate aggregation + 1024-sample reservoir for percentiles + min/max. Seeded RNG = deterministic test behavior.
- `coherence` = median cosine of E1 members vs E1 centroid (p50 from reservoir).

Storage API on `RocksDbTeleologicalStore` (concrete impl, downcast pattern required):
- `store_constellation` (atomic WriteBatch, primary + secondary index)
- `get_constellation`
- `list_constellation_ids` / `count_constellations`
- `delete_constellation` (atomic, returns `false` when missing; preserves secondary index if it now points to a different constellation UUID; Send-safe by resolving the selector *before* acquiring CF handles)
- `find_constellation_by_selector`
- `multi_get_constellations` (batched via `multi_get_cf`)

Encoding: `[CONSTELLATION_VERSION:u8 = 1][bincode(Constellation)]`. Mismatched version bytes return `CoreError::SerializationError`; no migration.

MCP surface (5 tools):
- `compile_constellation` — args: `selector` (topic/session/tag/time_range/explicit_ids), selector-specific fields, `label`, `maxMembers` (default 50000), `rebuildIfExists` (default false; when false and a constellation already exists for this selector, returns the existing id).
- `list_constellations` — `limit` (default 50, max 1000), `offset`, `includeCentroids` (default false).
- `get_constellation` — required `constellationId`, optional `includeCentroids` (default **true**).
- `score_against_constellation` — required `constellationId` + `memoryId`.
- `delete_constellation` — required `constellationId`.

All five MCP schemas have `additionalProperties: false`.

What Phase 3/4/6/7 agents can rely on:
- `CONSTELLATION_VERSION` bump requires new migration code.
- `per_embedder` is ALWAYS 13 entries (one per embedder index 0..13) regardless of coverage; check `coverage > 0.0` before using stats.
- Sparse embedders (E6 = idx 5, E13 = idx 12) use `sparse_top_terms` and leave `centroid` empty.
- Token-level embedder (E12 = idx 11) uses `pooled_token_centroid` + `mean_token_count` and leaves `centroid` empty.
- Dense/asymmetric embedders populate `centroid` with length = `dimension`.
- Selector's canonical form is the stable byte input to the secondary-index hash. Do NOT add fields to `ConstellationSelector` without bumping `CONSTELLATION_VERSION` AND preserving the canonical_form mapping for existing variants.
- Every MCP tool call downcasts to `RocksDbTeleologicalStore`; non-RocksDB backends return a tool error. Training tools use the same pattern.
- The tag selector matches `SourceMetadata.tool_name`; a proper tag column is deferred.
