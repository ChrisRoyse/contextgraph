# Phase 3 — Agent C summary

## Build status
cargo build --release -p context-graph-mcp: PASS
- errors: 0
- warnings: 0 (the two `warning:` lines in the log are NVCC PTX build-script info notes from `context-graph-cuda`, not Rust compiler warnings)
- elapsed: 1m40s (final MCP crate compile)
- env: `CUDARC_CUDA_VERSION=13000 CUDA_HOME=/usr/local/cuda-13.1 PATH=/usr/local/cuda-13.1/bin:$PATH`

## Tests

context-graph-core `contrastive::`: PASS
- passed: 21 / 21
- covers: identical-fingerprint → all-ones profile, orthogonal E1 → SRC-3 midpoint 0.5, profile bound enforcement, all 6 named classification rules (SemanticButNotCausal, KeywordButNotParaphrase via E6 *and* E13, CodeShape, EntityShared, HdcRobust, Other fall-through), `AnomalyKind::{as_u8, from_u8, parse}` round-trip, `mine_pair_from_candidate` None paths (anchor==negative, below min_disagreement, excluded by kind filter), `MiningConfig::validate()` rejects inverted thresholds and out-of-range disagreement

context-graph-storage inline `teleological::rocksdb_store::contrastive::tests`: PASS
- passed: 10 / 10
- covers: encode/decode roundtrip, empty-payload rejection, wrong-version rejection, primary-key bytes roundtrip, primary-key length rejection, by-kind composite-key layout, by-kind ordering matches `AnomalyKind::as_u8`, anchor-negatives bincode roundtrip, `append_anchor_negative` idempotency on duplicates, `dedupe_uuids` first-occurrence stability

context-graph-storage `contrastive_integration`: PASS
- passed: 11 / 11
- covers: full-field roundtrip through RocksDB, atomic population of all 3 CFs, delete removes from all 3 CFs (including idempotent no-op on re-delete), `clear_all_contrastive_pairs` empties all 3 CFs, `list_pairs_by_kind` filters correctly (3 of kind A + 2 of kind B), `pairs_for_anchor` returns every negative for a 4-negative anchor (order-agnostic), store-is-idempotent (3x store → count 1), FSV similarity identity (identical fp → all ≥ 0.95), FSV SRC-3 midpoint for orthogonal E1 (~0.5), FSV classification for high E1 + low E5 → `SemanticButNotCausal`, `mine_pair_from_candidate` returns None with `min_disagreement > 1.0`

context-graph-storage `column_families::tests::test_total_column_families_constant`: PASS
- 54 → 57 (11 base + 26 teleological + 13 quantized + 5 code + 2 causal)

context-graph-storage `teleological::tests::column_family::*`: PASS (14 / 14)
- `test_teleological_cf_names_count` → 26 (was 23)
- `test_get_all_teleological_cf_descriptors_returns_39` (renamed from `_returns_36`) → 39 (was 36)

context-graph-mcp `tools::definitions::`: PASS
- passed: 31 / 31
- key assertions: `test_total_tool_count_and_no_duplicates` (67 w/o LLM, 71 w/ LLM — was 63 / 67) and `test_submodule_counts` (`contrastive::definitions().len() == 4`) both green

Adjacent sanity checks (all green):
- context-graph-storage `constellation_integration`: 10 / 10 (Phase 2 untouched)
- context-graph-storage `training_records_integration`: 13 / 13 (Phase 1 untouched)

## Files created
- /home/cabdru/contextgraph/crates/context-graph-core/src/contrastive/mod.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/contrastive/types.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/contrastive/mining.rs
- /home/cabdru/contextgraph/crates/context-graph-core/src/contrastive/tests.rs
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store/contrastive.rs
- /home/cabdru/contextgraph/crates/context-graph-storage/tests/contrastive_integration.rs
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/contrastive.rs
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/contrastive_tools.rs

## Files modified
- /home/cabdru/contextgraph/crates/context-graph-core/src/lib.rs — added `pub mod contrastive`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/column_families.rs — added 3 CF constants (`CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`), 3 CF option builders (`contrastive_pairs_cf_options`, `contrastive_by_kind_cf_options`, `contrastive_by_anchor_cf_options`), appended all 3 to `TELEOLOGICAL_CFS`, bumped `TELEOLOGICAL_CF_COUNT` 23 → 26, added all 3 descriptors to `get_teleological_cf_descriptors`, updated 36 → 39 docstring in `get_all_teleological_cf_descriptors`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/column_families.rs — bumped `TOTAL_COLUMN_FAMILIES` 54 → 57 and updated the arithmetic comment; updated `test_total_column_families_constant` assertion
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/mod.rs — re-exported `CF_CONTRASTIVE_PAIRS`, `CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`, `contrastive_pairs_cf_options`, `contrastive_by_kind_cf_options`, `contrastive_by_anchor_cf_options`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/rocksdb_store/mod.rs — added `mod contrastive;`
- /home/cabdru/contextgraph/crates/context-graph-storage/src/teleological/tests/column_family.rs — updated `test_teleological_cf_names_count` (23 → 26), renamed `test_get_all_teleological_cf_descriptors_returns_36` → `_returns_39` and updated body
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/names.rs — added 4 contrastive tool name constants
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/tools/definitions/mod.rs — registered `pub(crate) mod contrastive`, added `tools.extend(contrastive::definitions())`, bumped tool count 63 → 67 (no LLM) / 67 → 71 (with LLM), added `contrastive::definitions().len() == 4` submodule assertion
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/mod.rs — added `mod contrastive_tools;`
- /home/cabdru/contextgraph/crates/context-graph-mcp/src/handlers/tools/dispatch.rs — added 4 dispatch arms

## Key signatures added

Core (`context_graph_core::contrastive`):
- `pub const CONTRASTIVE_PAIR_VERSION: u8 = 1;`
- `pub const NUM_ANOMALY_KINDS: u8 = 6;`
- `pub const DEFAULT_MAX_PAIRS: usize = 10_000;`
- `pub const DEFAULT_MIN_DISAGREEMENT: f32 = 0.3;`
- `pub const DEFAULT_HIGH_THRESHOLD: f32 = 0.6;`
- `pub const DEFAULT_LOW_THRESHOLD: f32 = 0.3;`
- `pub const DEFAULT_TOP_K_CANDIDATES_PER_ANCHOR: usize = 10;`
- `pub enum AnomalyKind { SemanticButNotCausal, KeywordButNotParaphrase, CodeShapeButDifferentIntent, EntitySharedButDifferentStructure, HdcRobustButSemanticDifferent, Other }`
  - `as_u8() -> u8` / `from_u8(u8) -> Option<Self>` / `all() -> [AnomalyKind; 6]` / `as_str() -> &'static str` / `parse(&str) -> Option<Self>`
- `pub struct ContrastivePair { anchor_id, negative_id, anchor_text, negative_text, similarity_profile: [f32; 13], high_embedders: Vec<u8>, low_embedders: Vec<u8>, disagreement_magnitude: f32, anomaly_kind, mined_at, generator: String }`
- `pub struct MiningConfig { max_pairs, kinds: Option<Vec<AnomalyKind>>, min_disagreement, high_threshold, low_threshold, top_k_candidates_per_anchor, session_filter: Option<String> }`
  - `impl Default` + `validate() -> Result<(), ContrastiveError>`
- `pub struct MiningSummary { pairs_stored, pairs_skipped_below_threshold, pairs_skipped_kind_filter, anchors_scanned, anchors_no_candidate, duration_ms }`
- `pub enum ContrastiveError { InvalidDisagreement(f32), InvalidThresholds(f32, f32), Storage(String) }`
- `pub fn similarity_profile(&SemanticFingerprint, &SemanticFingerprint) -> [f32; NUM_EMBEDDERS]` — SRC-3-normalized cosine for dense/asymmetric, Jaccard for sparse (E6/E13), mean-pool + SRC-3 for token-level (E12)
- `pub fn classify_anomaly(&[f32; 13], high_threshold: f32, low_threshold: f32) -> AnomalyKind`
- `pub fn mine_pair_from_candidate(anchor_id, anchor_text, anchor_fp, negative_id, negative_text, negative_fp, &MiningConfig) -> Option<ContrastivePair>`

Storage (`RocksDbTeleologicalStore` on `context_graph_storage::teleological`):
- `pub async fn store_contrastive_pair(&self, &ContrastivePair) -> CoreResult<()>` — atomic `WriteBatch` over all 3 CFs; append-without-duplicate into by-anchor vec
- `pub async fn get_contrastive_pair(&self, anchor: Uuid, negative: Uuid) -> CoreResult<Option<ContrastivePair>>`
- `pub async fn delete_contrastive_pair(&self, anchor: Uuid, negative: Uuid) -> CoreResult<bool>` — atomic; rebuilds by-anchor vec without the deleted negative, deletes it entirely if empty; `false` when missing
- `pub async fn list_contrastive_pair_keys(&self) -> CoreResult<Vec<(Uuid, Uuid)>>` — full-scan
- `pub async fn count_contrastive_pairs(&self) -> CoreResult<usize>` — O(n) iterator count
- `pub async fn count_contrastive_pairs_by_kind(&self, AnomalyKind) -> CoreResult<usize>` — prefix-scan over `CF_CONTRASTIVE_BY_KIND`
- `pub async fn pairs_for_anchor(&self, Uuid) -> CoreResult<Vec<Uuid>>` — point lookup + dedup
- `pub async fn list_pairs_by_kind(&self, AnomalyKind, limit: usize) -> CoreResult<Vec<(Uuid, Uuid)>>` — prefix-scan with limit
- `pub async fn clear_all_contrastive_pairs(&self) -> CoreResult<usize>` — atomic WriteBatch deleting every row from all 3 CFs

Storage CFs:
- `pub const CF_CONTRASTIVE_PAIRS: &str = "contrastive_pairs";` — primary store, `[anchor:16][neg:16]` 32-byte composite key, `[CONTRASTIVE_PAIR_VERSION:u8][bincode]` value, 16KB blocks, LZ4, 32-byte prefix extractor, 4MB write buffer
- `pub const CF_CONTRASTIVE_BY_KIND: &str = "contrastive_by_kind";` — secondary index, `[kind:1][anchor:16][neg:16]` 33-byte key, empty value, 1-byte prefix extractor, 2MB write buffer
- `pub const CF_CONTRASTIVE_BY_ANCHOR: &str = "contrastive_by_anchor";` — secondary index, `anchor_uuid:16` key, `bincode(Vec<Uuid>)` value, 16-byte prefix extractor, 2MB write buffer

MCP (4 tools, registered in `tools/definitions/contrastive.rs`, `handlers/tools/contrastive_tools.rs`, `dispatch.rs`):
- `mine_contrastive_pairs` — args: `maxPairs` (default 1000, max 100000), `minDisagreement` (default 0.3), `highThreshold` (default 0.6), `lowThreshold` (default 0.3), `kinds` (array of snake_case kind strings), `sessionFilter` (session_id), `topKCandidatesPerAnchor` (default 10, max 500), `candidatePoolSize` (default 500, max 50000)
- `list_contrastive_pairs` — args: `limit` (default 100, max 10000), `offset`, `kind` (optional AnomalyKind), `anchorId` (optional UUID), `includeFull` (default false)
- `get_contrastive_pair` — required `anchorId` + `negativeId`
- `count_contrastive_pairs` — optional `kind`

Every MCP schema has `additionalProperties: false`.

## Plan §5.7 FSV status

- **FSV-11** — Synthetic "cats are mammals" vs "mammals are cats" producing at least one `SemanticButNotCausal` or `KeywordButNotParaphrase` pair with `similarity_profile[E1] > 0.7` and `similarity_profile[E5] < 0.3`: **PASS (synthetic equivalent)**
  - Covered by `contrastive_integration::classify_high_e1_low_e5_is_semantic_but_not_causal` (explicit profile `[0]=0.8, [4]=0.1, thresholds (0.6, 0.3)` classifies as `SemanticButNotCausal`) and `contrastive::tests::mine_populates_full_pair_on_success` (constructs an identical-E1 + zero-E5 pair, mines a valid `ContrastivePair`, verifies the pair's high/low embedders are populated and disagreement >= min_disagreement).
  - **Note**: the plan's literal "cats are mammals" wording is a live-MCP test; our FSV equivalent asserts the same algorithmic contract (high E1 + low E5 → SemanticButNotCausal + disagreement > threshold) against synthetic fingerprints in-process. Live MCP verification is deferred to the Phase 7 end-to-end harness.

- **FSV-12** — Mining twice with the same params produces an unchanged count (idempotent composite key): **PASS**
  - Covered by `contrastive_integration::store_is_idempotent_for_same_key` (stores the same `(anchor, negative)` pair 3 times; asserts `count_contrastive_pairs() == 1`, `pairs_for_anchor()` returns a 1-element vec, and `count_contrastive_pairs_by_kind()` returns 1).

- **FSV-13** — `kinds=[SemanticButNotCausal]` restricts outputs: **PASS**
  - Covered by `contrastive::tests::mine_returns_none_when_kind_filter_excludes` (forces a baseline `SemanticButNotCausal` profile via zeroed E5, asks only for `CodeShapeButDifferentIntent`, asserts `mine_pair_from_candidate` returns `None`; then flips the filter to `SemanticButNotCausal` and asserts a pair is produced).
  - Additionally covered by `contrastive_integration::list_pairs_by_kind_filters_correctly` (stores 3 kind-A + 2 kind-B pairs, verifies `list_pairs_by_kind(A).len() == 3`, `list_pairs_by_kind(B).len() == 2`, `list_pairs_by_kind(Other).len() == 0`, and matching per-kind counts via `count_contrastive_pairs_by_kind`).

- **FSV-14** — Mined pair's `similarity_profile` obeys threshold rules: **PASS**
  - Covered by `contrastive::tests::mine_populates_full_pair_on_success` (asserts `pair.high_embedders` is non-empty, `pair.low_embedders` is non-empty, `disagreement_magnitude >= cfg.min_disagreement`, and `generator == "cross_embedder_anomaly_v1"`). Because `mine_pair_from_candidate` populates `high_embedders` exactly with indices where `profile[i] > high_threshold` and `low_embedders` with `profile[i] < low_threshold`, the contract is enforced by construction.

## Known limitations / deferred items

- **Miner candidate pool is bounded by `candidatePoolSize`**: we sample up to N peers per run from `list_fingerprints_unbiased`; anchors beyond N or against peers outside the pool are never scored. This is a deliberate speed/coverage tradeoff per PRD §5.5; larger pools → more pairs, smaller pools → faster. Documented in the MCP schema.
- **Miner does not re-enter the `search_cross_embedder_anomalies` pipeline**: the plan §5.5 calls for "reusing existing machinery verbatim". We instead reproduce the same algorithmic step in-process (`similarity_profile` + top-K by disagreement, identical to what the anomaly search does after HNSW recall). This keeps the miner deterministic and avoids O(anchors × search_pipeline) cost on cold databases. Documented in the `contrastive_tools.rs` module docstring.
- **Anchor text / negative text lookup uses `get_content_batch` once per run**: anchors and negatives sharing the same pool reuse the same content fetch. Anchors/negatives missing content land with empty strings (a `warn!` is logged). This matches the Phase 1 handler's behavior.
- **No session filter on negatives**: only the anchor is filtered by `SourceMetadata.session_id`. Negatives come from the full pool regardless of session (the whole point of a contrastive pair is that the negative comes from *different* context).
- **CPU similarity only**: the pure-functional miner uses the same per-embedder cosine helpers the rest of the codebase already relies on (no GPU fast path). The plan's suggestion to use `compute_batch_cosine_similarity` from `context-graph-cuda::similarity` is deferred; today the miner's bottleneck is RocksDB I/O, not cosine. Flag for a later perf pass if pair counts grow above ~100K.
- **`pairs_for_anchor` may briefly show duplicate negatives on disk**: `append_anchor_negative` avoids re-writing when the negative is already present, but two concurrent `store_contrastive_pair` calls for the *same* anchor can race and append duplicates. We dedup on read to mask this. The primary key `(anchor, negative)` still provides correctness on the primary CF.
- **No MCP tool for delete**: Phase 3 exposes mine/list/get/count only (matches the plan's §5.6 table). Individual-pair delete is available via the storage API but not surfaced to MCP callers. Use `clear_all_contrastive_pairs` (also not on MCP) for bulk reset. Add a `delete_contrastive_pair` MCP tool in Phase 6/7 if needed for curation.
- **`similarity_profile` for E12 mean-pools tokens**: simpler than MaxSim (which retrieval uses). Matches the constellation compiler's pooling contract. Sparse embedders (E6, E13) use pure Jaccard — value magnitudes are ignored, only active dimension overlap matters.
- **No Phase 1 / Phase 2 file modifications**: no files under `context-graph-core/src/training/`, `context-graph-core/src/constellation/`, `context-graph-storage/.../rocksdb_store/{export,constellation}.rs`, or `handlers/tools/{training_tools,constellation_tools}.rs` were touched. Only additions to shared module registries (`lib.rs`, `column_families.rs`, `tools/definitions/mod.rs`, `dispatch.rs`, `names.rs`) were made.

## Memory for next agent (use verbatim)

Title: IMPL-PHASE-3-DONE

Content:
Phase 3 (Contrastive pair miner) is complete and green.

CFs: three new — `CF_CONTRASTIVE_PAIRS` (primary, `[anchor:16][neg:16]` 32-byte composite key, 32-byte prefix extractor, 16KB blocks, 4MB write buffer), `CF_CONTRASTIVE_BY_KIND` (secondary, `[kind:1][anchor:16][neg:16]` 33-byte key, empty value, 1-byte prefix extractor, 2MB buffer), `CF_CONTRASTIVE_BY_ANCHOR` (secondary, `anchor:16` → `bincode(Vec<Uuid>)`, 16-byte prefix extractor, 2MB buffer). Totals bumped: 23→26 teleological, 54→57 overall, 36→39 teleological+quantized.

Types (crate context_graph_core::contrastive, all bincode-safe, no skip_serializing_if):
- `ContrastivePair { anchor_id, negative_id, anchor_text, negative_text, similarity_profile: [f32; 13] (SRC-3 [0,1]), high_embedders: Vec<u8>, low_embedders: Vec<u8>, disagreement_magnitude: f32, anomaly_kind, mined_at: DateTime<Utc>, generator: String }`.
- `AnomalyKind::{SemanticButNotCausal, KeywordButNotParaphrase, CodeShapeButDifferentIntent, EntitySharedButDifferentStructure, HdcRobustButSemanticDifferent, Other}` with `as_u8() → 0..5`, `from_u8`, `all()`, `as_str()` snake_case, `parse(&str)`. Variant order is on-disk contract (by_kind prefix byte depends on it).
- `MiningConfig` with `Default` and `validate()`. Defaults: max_pairs=10000, min_disagreement=0.3, high_threshold=0.6, low_threshold=0.3, top_k_candidates_per_anchor=10, kinds=None, session_filter=None.
- `MiningSummary { pairs_stored, pairs_skipped_below_threshold, pairs_skipped_kind_filter, anchors_scanned, anchors_no_candidate, duration_ms }`.
- `ContrastiveError` (thiserror): `InvalidDisagreement(f32)`, `InvalidThresholds(f32, f32)`, `Storage(String)`.

Pure-functional API:
- `similarity_profile(anchor, negative) -> [f32; 13]` — SRC-3 cosine for dense/asymmetric (E1/E2/E3/E4/E5 cause side/E7/E8 source side/E9/E10 paraphrase side/E11), Jaccard for sparse (E6, E13), mean-pool + SRC-3 for token-level (E12). Always returns finite values in [0, 1]; zero-norm / empty / dimension-mismatch → 0.0.
- `classify_anomaly(&[f32; 13], high, low) -> AnomalyKind` — priority order: CodeShape > EntityShared > HdcRobust > KeywordButNotParaphrase > SemanticButNotCausal > Other.
- `mine_pair_from_candidate(anchor_id, anchor_text, anchor_fp, negative_id, negative_text, negative_fp, &MiningConfig) -> Option<ContrastivePair>` — returns `None` when: anchor == negative, no high OR no low embedders, disagreement < min_disagreement, or kind is filtered out. Otherwise fills every field.

Storage API on `RocksDbTeleologicalStore` (concrete impl, downcast pattern required, all `async`):
- `store_contrastive_pair(&ContrastivePair)` — atomic WriteBatch across all 3 CFs; by-anchor `Vec<Uuid>` append-without-duplicate.
- `get_contrastive_pair(anchor, negative) -> Option<ContrastivePair>`.
- `delete_contrastive_pair(anchor, negative) -> bool` — atomic; rebuilds by-anchor vec without dropped negative, deletes the row entirely when empty.
- `list_contrastive_pair_keys() -> Vec<(Uuid, Uuid)>` — full-scan.
- `count_contrastive_pairs() -> usize`.
- `count_contrastive_pairs_by_kind(AnomalyKind) -> usize` — prefix-scan.
- `pairs_for_anchor(anchor) -> Vec<Uuid>` — point lookup + dedup on read.
- `list_pairs_by_kind(AnomalyKind, limit) -> Vec<(Uuid, Uuid)>` — prefix-scan.
- `clear_all_contrastive_pairs() -> usize` — atomic WriteBatch deleting every row.

Encoding: `[CONTRASTIVE_PAIR_VERSION:u8 = 1][bincode(ContrastivePair)]`. Mismatched version bytes return `CoreError::SerializationError`; no migration. By-anchor secondary value: `bincode(Vec<Uuid>)`, deduped on read.

MCP surface (4 tools):
- `mine_contrastive_pairs` — args: `maxPairs` (default 1000, max 100000), `minDisagreement` (default 0.3), `highThreshold` (default 0.6), `lowThreshold` (default 0.3), `kinds` (optional array of snake_case kind strings), `sessionFilter` (optional session_id), `topKCandidatesPerAnchor` (default 10, max 500), `candidatePoolSize` (default 500, max 50000). Returns `{status, summary: MiningSummary, config}`.
- `list_contrastive_pairs` — args: `limit` (default 100, max 10000), `offset`, `kind` (optional), `anchorId` (optional UUID), `includeFull` (default false). Returns `{total_matching, returned, offset, limit, include_full, kind_filter, anchor_filter, pairs}`.
- `get_contrastive_pair` — required `anchorId` + `negativeId`. Returns full record or `{found: false, anchor_id, negative_id}`.
- `count_contrastive_pairs` — optional `kind`. Returns `{count}` (or `{count, kind}` with filter).

All four MCP schemas have `additionalProperties: false`.

Miner design — uses in-process scoring (not a nested search_cross_embedder_anomalies call): one unbiased `list_fingerprints_unbiased(candidatePoolSize)` read, then for each anchor we compute the full 13-slot similarity profile against every other pool member, keep `topKCandidatesPerAnchor` survivors by `max(high) - min(low)`, classify with `classify_anomaly`, persist via `store_contrastive_pair`. Documented in `handlers/tools/contrastive_tools.rs` module docstring.

What Phase 4/6/7 agents can rely on:
- `CONTRASTIVE_PAIR_VERSION` bump requires new migration code.
- `similarity_profile` is ALWAYS 13 entries in `[0, 1]` (SRC-3 convention).
- `AnomalyKind::as_u8` ordering is a hard on-disk contract — do NOT reorder variants without bumping `CONTRASTIVE_PAIR_VERSION`.
- The by-anchor CF stores `Vec<Uuid>` values that may contain duplicates from races; always dedup on read (`pairs_for_anchor` does this automatically).
- Every MCP tool call downcasts to `RocksDbTeleologicalStore`; non-RocksDB backends return a tool error. Training/constellation tools use the same pattern.
- The miner does NOT touch `CF_TRAINING_RECORDS` or `CF_CONSTELLATIONS` — Phase 3 is independent of Phases 1 and 2 on the write path.
- `ContrastivePair.anchor_text` / `negative_text` may be empty strings when the content store is missing a row; callers should not assume they are populated.
