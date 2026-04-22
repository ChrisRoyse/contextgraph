# Training Data Export & Downstream Artifact Plan

**Status**: Plan of record. Not yet implemented (Phase 1 had a first cut that was reverted for the bugs listed in §3.3).
**Owner**: core team
**Target**: claude-flow-v3 / context-graph monorepo

## 0. Vision

Context Graph already computes, for every stored memory, a ~46 KB multi-dimensional label space: 13 embedding vectors (dense + sparse + token-level), a 13-D topic profile, 78 synergy-weighted cross-correlations, 6-D group alignments, K-NN neighbors per embedder, typed edges, LLM causal labels, entity extractions, and temporal signatures. All of this sits in RocksDB and is never surfaced to downstream consumers in a form a training pipeline can consume.

This plan turns Context Graph from a retrieval system into a **training-data factory** by building five tightly-scoped, RocksDB-native pipelines:

| # | Pipeline                         | Primary artifact                                            | Consumer                                |
| - | -------------------------------- | ----------------------------------------------------------- | --------------------------------------- |
| 1 | Training corpus export           | `TrainingRecord` per memory in `CF_TRAINING_RECORDS`        | ML training loops (any framework)       |
| 2 | Constellation compiler           | per-embedder centroids + spread stats in `CF_CONSTELLATIONS`| downstream retrieval / classifier heads |
| 3 | Contrastive pair miner           | `(anchor, hard_negative, similarity_profile)` triples in `CF_CONTRASTIVE_PAIRS` | contrastive / metric-learning training  |
| 4 | Tucker-core export               | compact 13×13×1024 interaction fingerprints in `TrainingRecord.tucker_core` | relational representation learning      |
| 5 | Temporal context labels          | recency / periodic / sequence bands already in `TrainingRecord` | pragmatic / discourse-aware training    |

The work is sequenced so every phase is useful on its own and later phases reuse the previous infrastructure.

## 1. Design principles (non-negotiable)

These constraints apply to every phase. Violating any of them is a block on merge.

1. **RocksDB-first**. Every artifact lands in a new or existing column family. No SQLite, no filesystem-sharded files as the source of truth. File-format exports (Parquet / HDF5 / safetensors) are **optional last-mile** dumps produced by a separate CLI command that reads from the CF.
2. **Bincode + one version byte** for dense / positional data. **JSON** for provenance, metadata, and anything with optional fields (constitution rule from PRD; violating it silently corrupts data because bincode is positional).
3. **One version byte per value** (`TRAINING_RECORD_VERSION`, `CONSTELLATION_VERSION`, etc.). Deserialization rejects mismatches with `CoreError::SerializationError`. No automatic migration.
4. **Every CF is observable via MCP**. Writers always come with read-back tools. No feature merges without a `list_*` / `get_*` / `count_*` trio that returns shape stats.
5. **Idempotent**. Every exporter can be re-run safely. Keyed writes overwrite. `clearExisting=true` is a first-class option.
6. **Bounded batches, streaming writes**. Never hold the full corpus in memory. Every loop writes per-record before dropping the record.
7. **Zombie-proof**. Every long-running MCP tool runs under the `prctl(PR_SET_PDEATHSIG, SIGTERM)` + doomsday-watchdog safety net already installed in `main.rs`. No additional threads may be spawned without a shutdown path.
8. **FSV-able**. Every pipeline must be provable via: (a) store synthetic input with known output, (b) read CF back via MCP, (c) assert structural invariants and a known-math property. CLAUDE.md §4 applies.

## 2. Conventions, shared types, shared utilities

These are referenced by multiple phases. Build them once in Phase 1 and reuse.

### 2.1 CF naming

All new CFs live under the teleological namespace: `CF_TRAINING_RECORDS`, `CF_CONSTELLATIONS`, `CF_CONTRASTIVE_PAIRS`, `CF_TUCKER_CORES` (optional, see §6). Each gets:

- a `pub const CF_*: &str` in `crates/context-graph-storage/src/teleological/column_families.rs`
- a `*_cf_options(&Cache) -> Options` helper in the same file
- an entry in the `TELEOLOGICAL_CFS` array
- a descriptor in `get_teleological_cf_descriptors`
- an increment to `TELEOLOGICAL_CF_COUNT` and to the top-level `TOTAL_COLUMN_FAMILIES` constant
- corresponding test updates in `teleological/tests/column_family.rs` and `column_families.rs`

Factor the common UUID-keyed options into a single helper (code-simplifier Issue #11):

```rust
fn uuid_prefixed_cf_options(
    cache: &Cache, block_size_kb: usize, write_buffer_mb: usize
) -> Options
```

### 2.2 Value encoding

```rust
// Always:
//   [version: u8][bincode(Record)]   or   [version: u8][serde_json(Record)]
// Rules:
//   - Dense vectors, fixed-size structs, hot-path records → bincode.
//   - Provenance, optional fields, anything with skip_serializing_if → JSON.
//   - Never mix: no skip_serializing_if with bincode (silent corruption).
fn encode_v1<T: Serialize>(v: u8, r: &T) -> CoreResult<Vec<u8>>;
fn decode_v1<T: DeserializeOwned>(v: u8, bytes: &[u8]) -> CoreResult<T>;
```

Put these in `crates/context-graph-storage/src/teleological/serialization.rs` next to the existing `TELEOLOGICAL_VERSION` constant.

### 2.3 MCP tool shape

Every new tool follows the four-touchpoint pattern already in the repo:

1. `crates/context-graph-mcp/src/tools/names.rs` — `pub const <NAME>: &str = "<snake_case>";`
2. `crates/context-graph-mcp/src/tools/definitions/<category>.rs` — JSON Schema with `additionalProperties: false`
3. `crates/context-graph-mcp/src/handlers/tools/<category>_tools.rs` — `pub(crate) async fn call_<name>(&self, id, args) -> JsonRpcResponse`
4. `crates/context-graph-mcp/src/handlers/tools/dispatch.rs` — one-line dispatch arm

Tool count tests in `definitions/mod.rs::test_submodule_counts` must be bumped in the same commit.

### 2.4 Observability contract

Every CF exposes:

- `list_<records>(limit, offset, includeShape: bool)` — returns UUIDs and (if requested) shape summary (lengths, counts, flags) per record
- `get_<record>(id, includeVectors, includeTokenEmbeddings, includeEdges)` — full record with optional heavy fields
- `count_<records>()` — O(N) iterator count

Downstream consumers (Python / notebooks) can reach the data through these without touching RocksDB directly.

## 3. Phase 1 — Training Corpus Export (v2)

Phase 1 is the foundation. Everything else reuses `TrainingRecord` or its per-record read path. Ship this first.

### 3.1 Goal

For every non-soft-deleted memory, emit one `TrainingRecord` into `CF_TRAINING_RECORDS`. The record bundles the memory's full label space as surfaced by Context Graph. Production data producers (embedding providers, graph builder, causal discovery) are untouched; we only read what they already wrote.

### 3.2 Data model

`TrainingRecord` lives in `crates/context-graph-core/src/training/mod.rs`. Fields:

```
Identity:      memory_id, content, importance, created_at, session_id
Provenance:    source_type, source_path, content_hash
Dense E1-E11:  e1_semantic (1024), e2/3/4 (512), e5 cause+effect (768×2),
               e7_code (1536), e8 source+target (1024×2), e9_hdc (1024),
               e10 paraphrase+context (768×2), e11_entity (768)
Sparse:        e6 (indices u16 + values f32), e13 (indices + values)
Token-level:   e12_late_interaction: Vec<Vec<f32>> (128D per token)
Fusion:        topic_profile[13], cross_correlations[78], group_alignments[6]
Structure:     outgoing_edges, incoming_edges, knn_neighbors[13]
Causal:        causal_effects (this is cause), causal_causes (this is effect)
Clustering:    topic_memberships: Vec<TopicMembership>
Tucker (§6):   tucker_core: Option<TuckerCore>   -- added in Phase 4
Temporal (§7): temporal_labels: TemporalLabels   -- added in Phase 5
```

Supporting types:

- `TrainingEdge { edge_type, peer_id, weight, direction, agreement_count, embedder_scores[13] }`
- `KnnNeighbor { target_id, similarity }`
- `CausalLabel { related_memory_id, rel_id, description, direction, confidence, mechanism_type }`
  - **Both** `related_memory_id` (the peer end when it can be resolved) **and** `rel_id` (always the `CausalRelationship::id`) must be stored. This closes code-simplifier Issue #1.
- `TopicMembership { topic_id, topic_label, topic_profile[13], membership_probability }`

### 3.3 Phase 1 bug carry-overs to fix

These are the refinements from the code-simplifier review of the reverted first cut. None are optional.

1. **(Critical)** `CausalLabel.related_memory_id` must NOT be `rel.source_fingerprint_id`; that is always the current memory itself. Add `rel_id: Uuid` (always `rel.id`) and leave `related_memory_id` as the other-end UUID only when the system can resolve it, else `Uuid::nil()`.
2. **(Medium)** `compute_topic_profile_from_fingerprint` used vector magnitudes and saturated to ~1.0 for the unit-normalized embeddings, destroying signal. Replace with: read the per-memory 13-D topic profile from `CF_TOPIC_PROFILES` (which `RocksDbTeleologicalStore` already maintains). That's the real topic alignment computed at store time, not a post-hoc heuristic. Only fall back to a magnitude-based derivation when `CF_TOPIC_PROFILES` has no row (e.g., legacy data).
3. **(Medium)** Session filter must fetch `get_source_metadata(id)` once per memory, not twice. Hoist into the filter loop and pass into `build_training_record`.
4. **(Medium)** `includeIncomingEdges=true` must build the reverse edge index **once** by iterating `CF_TYPED_EDGES` and bucketing by target UUID, then look up per fingerprint. The current per-memory `get_typed_edges_to` is O(N²). Either implement the one-time scan or keep the flag off by default and update the schema description to be honest about the cost.
5. **(Low)** Fix docstrings on `dense_alignment` / `sparse_alignment` so they describe what the code does (or, preferably, change the code to match the docs — see §3.3.2).
6. **(Low)** `list_training_records` with `includeShape=true` must use `db.multi_get_cf(...)` for batch reads, not N sequential `get_training_record` calls.
7. **(Low)** `arm_doomsday_watchdog` should log and continue if `thread::spawn` fails (EAGAIN under exhaustion). It must not panic on the rescue path.

### 3.4 MCP surface

1. `export_training_corpus` — iterate fingerprints, build records, persist
2. `list_training_records` — paginated UUIDs + shape summary
3. `get_training_record` — single record with include flags
4. `count_training_records` — O(N) scan count

Schemas: unchanged from the reverted first cut except `includeIncomingEdges` now actually maps to the reverse-index implementation and `includeTuckerCore` / `includeTemporalLabels` are added (see §6 / §7).

### 3.5 Algorithm (session filter path)

```
1. Optionally clearExisting=true → clear_all_training_records()
2. If filter='session' && filterId missing → error
3. If includeIncomingEdges=true && edge_repository is Some →
      build HashMap<Uuid, Vec<TrainingEdge>> by iterating CF_TYPED_EDGES once
4. fingerprints = list_fingerprints_unbiased(maxMemories)
5. For each fp in fingerprints:
    a. source = get_source_metadata(fp.id).await   -- single fetch
    b. If filter='session' && source.session_id != filterId → skip
    c. content = get_content(fp.id).await
    d. topic_profile = get_topic_profile(fp.id).await_or_compute_from_fingerprint()
    e. cross_correlations = compute(topic_profile, synergy_matrix)
    f. group_alignments = compute(topic_profile)
    g. outgoing = edge_repo.get_typed_edges_from(fp.id) if include_edges
    h. incoming = incoming_index.get(&fp.id).cloned() if include_incoming
    i. knn[13] = for each active embedder: edge_repo.get_embedder_edges(e, fp.id)
    j. causal_effects = get_causal_relationships_by_source(fp.id)
                           .map(rel -> CausalLabel { rel_id: rel.id, ... })
    k. topic_memberships = topic_portfolio.memberships(fp.id) if loaded
    l. tucker_core = Some(fp.teleological_vector().tucker_decompose(ranks))
                        if includeTuckerCore
    m. temporal_labels = extract(fp.created_at, fp.source_metadata, session)
                            if includeTemporalLabels
    n. Serialize + store_training_record(fp.id, &record)
    o. Drop the record; only its bincode bytes linger in RocksDB
6. Return summary JSON
```

### 3.6 Storage layout

```
CF_TRAINING_RECORDS
  key:   UUID (16 B) — same key as the fingerprint
  value: [TRAINING_RECORD_VERSION: u8][bincode(TrainingRecord)]
```

Uses `uuid_prefixed_cf_options(cache, block_size_kb=64, write_buffer_mb=8)`.

### 3.7 FSV requirements

Per CLAUDE.md §4. Every one is blocking on merge:

- **FSV-1**: synthetic record roundtrip (already in place in `tests/training_records_integration.rs`, keep).
- **FSV-2**: ingest a small synthetic corpus via MCP, export with `filter='session'`, read back, assert 13 / 78 / 6 dimensions on every record.
- **FSV-3**: verify one sample record by mathematical identity:
  `group_alignments[5] == topic_profile[5]` (Implementation = E6 alone). If this ever breaks, group computation drifted.
- **FSV-4**: verify E11-disabled invariant: `topic_profile[10] == 0.0 ⇒ group_alignments[4] == topic_profile[9] / 2` (Qualitative = (E10+E11)/2 with E11=0).
- **FSV-5**: edge matrix with before/after CF counts: clearExisting on empty CF (cleared=0), idempotent re-export, filter no-match (count unchanged), invalid UUID (error), nonexistent UUID (`found: false`).
- **FSV-6**: zombie-prevention tests (`/tmp/test_no_zombies.py`) pass against the freshly-built binary.

### 3.8 Success criteria

- 0 errors / 0 warnings on `cargo build --release -p context-graph-mcp` with `CUDARC_CUDA_VERSION=13000`.
- All unit + integration tests green.
- Exporting a 10 K-memory corpus completes in < 60 s.
- Resident memory during export stays below 500 MB (streaming writes, batched reads).

## 4. Phase 2 — Constellation Compiler

### 4.1 Goal

Given a set of memories (selected by topic, session, tag, or time range), compute a **constellation**: per-embedder centroid + spread statistics. This is the text analog of ClipCannon's TCT bridge. Consumers use a constellation to classify a new memory ("is this in the same space as X?") without running the full 3-stage search pipeline.

### 4.2 Data model

New module `crates/context-graph-core/src/constellation/mod.rs`:

```
pub const CONSTELLATION_VERSION: u8 = 1;

pub struct Constellation {
    pub id: Uuid,                    // constellation UUID (new)
    pub label: String,               // human-readable tag ("PRD §7 - Case management")
    pub created_at: DateTime<Utc>,

    pub selector: ConstellationSelector,  // how this set was chosen
    pub member_ids: Vec<Uuid>,
    pub member_count: usize,

    pub per_embedder: [EmbedderStats; NUM_EMBEDDERS],  // 13 entries
    pub topic_profile_centroid: [f32; 13],
    pub group_alignment_centroid: [f32; 6],
    pub cross_correlation_centroid: Vec<f32>,  // 78

    pub coherence: f32,          // mean cosine of members to centroid (E1)
    pub purity: Option<f32>,     // optional topic purity when selector is topic-based
}

pub enum ConstellationSelector {
    Topic { topic_id: String },
    Session { session_id: String },
    Tag { tag: String },
    TimeRange { start: DateTime<Utc>, end: DateTime<Utc> },
    ExplicitIds { rationale: String },
}

pub struct EmbedderStats {
    pub embedder_index: u8,
    pub dimension: u16,
    pub vector_kind: VectorKind,        // Dense / Sparse / TokenLevel
    pub centroid: Vec<f32>,             // empty for sparse/token-level → see sparse/token fields
    pub sparse_top_terms: Vec<(u16, f32)>,  // populated for E6, E13 (top-K terms by mean weight)
    pub mean_l2: f32,                   // scale stat for the centroid
    pub stddev_l2: f32,                 // spread
    pub cosine_spread_p50: f32,         // median cosine of members to centroid
    pub cosine_spread_p95: f32,
    pub min_cosine: f32,
    pub max_cosine: f32,
    pub coverage: f32,                  // fraction of members with a non-zero vector for this embedder
}
```

For asymmetric embedders (E5, E8, E10) the centroid uses the **cause / source / paraphrase** side only; this matches `compute_topic_profile_from_fingerprint` and is documented in the struct docstring.

For E12 token-level, `centroid` stays empty and we emit a separate `EmbedderStats::mean_token_count` and a pooled 128-D vector.

### 4.3 CF layout

```
CF_CONSTELLATIONS
  key:   constellation_id (16 B UUID)
  value: [CONSTELLATION_VERSION][bincode(Constellation)]

CF_CONSTELLATION_BY_SELECTOR  (secondary index)
  key:   [selector_kind: u8][selector_hash: 16 B]
  value: constellation_id (16 B)
```

Why a secondary index: so "what constellation exists for topic T?" is an O(1) point lookup instead of a linear scan.

### 4.4 Algorithm

```
compile_constellation(selector, label):
    member_ids = resolve(selector)       -- topic members / session fingerprints / tag lookup / time scan
    if member_ids.len() < MIN_MEMBERS (default 3) → error("too few members")

    // Stream member fingerprints; never hold them all in RAM at once
    for each id in member_ids:
        fp = store.retrieve(id)
        for embedder_idx in 0..13:
            update running_mean and running_M2 for that embedder's vector
            update cosine_accumulator (against running centroid)
        accumulate topic_profile, group_alignments, cross_correlations

    finalize stats (Welford for stddev; sort-and-sample for percentiles on a bounded buffer)
    persist to CF_CONSTELLATIONS + update CF_CONSTELLATION_BY_SELECTOR
```

Two guardrails:

- Uses the **incremental / Welford** variance estimator so memory stays O(1) per embedder regardless of member count.
- For percentile stats we keep a **reservoir sample of size 1024** per embedder. Exact p50/p95 at arbitrary member counts is overkill; approximate is fine and bounded.

### 4.5 MCP surface

| Tool                           | Purpose                                                                   |
| ------------------------------ | ------------------------------------------------------------------------- |
| `compile_constellation`        | run the compiler with a selector                                          |
| `list_constellations`          | paginated                                                                 |
| `get_constellation`            | full record by id (optional `includeCentroids`)                           |
| `score_against_constellation`  | classify a new memory: returns per-embedder cosine-to-centroid + verdict |
| `delete_constellation`         | remove                                                                    |

`score_against_constellation` is the main consumer-facing tool. It answers: "given memory X and constellation C, how well does X fit?" Returns `{per_embedder_cosine[13], combined_score, in_spread_p95: bool}`.

### 4.6 FSV requirements

- **FSV-7**: Build a constellation from 5 synthetic memories where all are duplicates of one sentence. Expect coherence ≈ 1.0 on every dense embedder, cosine_spread_p50 ≈ 1.0, cosine_spread_p95 ≈ 1.0.
- **FSV-8**: Build a constellation from 2 clusters of 5 identical memories (10 total). Coherence on E1 should drop — expect around 0.4–0.7 depending on embedding model, definitely not ≈1.0. This catches degenerate-centroid bugs.
- **FSV-9**: Score a known-member memory against its own constellation → expect in_spread_p95 = true. Score an unrelated memory → expect in_spread_p95 = false.
- **FSV-10**: Round-trip the constellation through RocksDB and assert every field survives.

## 5. Phase 3 — Contrastive Pair Miner

### 5.1 Goal

Automated hard-negative mining from existing cross-embedder disagreements. Package each mined pair as `(anchor, hard_negative, similarity_profile)` ready for contrastive-loss training.

### 5.2 Motivation (why this is uniquely valuable)

Cross-embedder anomaly pairs are **structurally interesting negatives**, not random ones. For contrastive training:

- A memory with high E1 but low E5 to another memory is a "looks similar, isn't causally related" pair — gold for causal reasoning training.
- High E6 but low E10: shares keywords but opposite contextual meaning — gold for semantic robustness.
- High E7 but low E1: similar code shape but different intent — gold for code understanding.

Every such pair is already computable by `search_cross_embedder_anomalies`. Phase 3 wraps that logic into a persistent miner.

### 5.3 Data model

```
CF_CONTRASTIVE_PAIRS
  key:   [anchor_uuid: 16 B][neg_uuid: 16 B]
  value: [CONTRASTIVE_PAIR_VERSION: u8][bincode(ContrastivePair)]

pub const CONTRASTIVE_PAIR_VERSION: u8 = 1;

pub struct ContrastivePair {
    pub anchor_id: Uuid,
    pub negative_id: Uuid,
    pub anchor_text: String,
    pub negative_text: String,

    pub similarity_profile: [f32; NUM_EMBEDDERS],  // per-embedder cosine
    pub high_embedders: Vec<u8>,                   // indices where sim > high_threshold
    pub low_embedders: Vec<u8>,                    // indices where sim < low_threshold
    pub disagreement_magnitude: f32,               // max(high_sim) - min(low_sim)
    pub anomaly_kind: AnomalyKind,                 // classification

    pub mined_at: DateTime<Utc>,
    pub generator: String,                         // "cross_embedder_anomaly_v1"
}

pub enum AnomalyKind {
    SemanticButNotCausal,      // high E1, low E5
    KeywordButNotParaphrase,   // high E6/E13, low E10
    CodeShapeButDifferentIntent,  // high E7, low E1
    EntitySharedButDifferentStructure,  // high E11, low E8
    HdcRobustButSemanticDifferent,      // high E9, low E1
    Other,
}
```

### 5.4 CF layout + indexes

```
CF_CONTRASTIVE_PAIRS             primary, key = [anchor:16][neg:16]
CF_CONTRASTIVE_BY_ANCHOR         secondary: key = anchor:16, value = Vec<neg_uuid>
CF_CONTRASTIVE_BY_KIND           secondary: key = [kind:u8][anchor:16][neg:16], value = empty
```

`CF_CONTRASTIVE_BY_KIND` enables: "give me all `SemanticButNotCausal` pairs for training" via prefix scan.

### 5.5 Algorithm

```
mine_contrastive_pairs(max_pairs, kinds: Option<Vec<AnomalyKind>>, min_disagreement: f32):
    // Reuse existing machinery verbatim
    anchors = list_fingerprints_unbiased(batch_size)
    for each anchor in anchors:
        for each (highEmbedder, lowEmbedder) in configured_anomaly_dimensions:
            if kinds is Some && !kinds.contains(classify(highEmbedder, lowEmbedder)): skip
            candidates = search_cross_embedder_anomalies(
                query=anchor.content, highEmbedder, lowEmbedder,
                high=0.6, low=0.3, topK=10
            )
            for each (neg_id, high_sim, low_sim) in candidates:
                disagreement = high_sim - low_sim
                if disagreement < min_disagreement: skip
                pair = build_pair(anchor, neg_id, full_similarity_profile)
                store_pair(pair)   -- keyed [anchor][neg], idempotent
                if total_mined >= max_pairs: return
```

The "full_similarity_profile" step requires a per-embedder cosine computation between anchor and negative. That's 13 dot products per pair. Use `compute_batch_cosine_similarity` from `context-graph-cuda::similarity` to stay on the GPU path already in place.

### 5.6 MCP surface

| Tool                          | Purpose                                                                    |
| ----------------------------- | -------------------------------------------------------------------------- |
| `mine_contrastive_pairs`      | run the miner (maxPairs, kinds filter, minDisagreement, anchorSessionFilter) |
| `list_contrastive_pairs`      | paginated                                                                  |
| `get_contrastive_pair`        | one pair by (anchor, neg)                                                  |
| `count_contrastive_pairs`     | total, optionally by kind                                                  |
| `export_contrastive_dataset`  | dump all pairs in a format that HuggingFace `datasets` can consume (optional Parquet via §6.2) |

### 5.7 FSV requirements

- **FSV-11**: Synthetic. Store A = "cats are mammals", B = "mammals are cats" (same keywords, opposite causal relation). Mine pairs. Expect at least one `SemanticButNotCausal` or `KeywordButNotParaphrase` pair between A and B, with `similarity_profile[E1] > 0.7` and `similarity_profile[E5] < 0.3`.
- **FSV-12**: Mine, then immediately mine again with the same params. Expect count unchanged (idempotent) — identical pairs deduplicate by the composite key.
- **FSV-13**: Request only `kinds=[SemanticButNotCausal]`. Assert `anomaly_kind == SemanticButNotCausal` for every returned pair.
- **FSV-14**: Inspect a mined pair's `similarity_profile` vs the anomaly thresholds. `similarity_profile[high_embedders[0]] > high_threshold` and `similarity_profile[low_embedders[0]] < low_threshold`, else the miner has a labeling bug.

## 6. Phase 4 — Tucker-Core Export

### 6.1 Goal

Populate the already-defined `TeleologicalVector.tucker_core` field and surface it through `TrainingRecord.tucker_core`. A Tucker-decomposed 13×13×1024 interaction tensor captures how the 13 embedding spaces interact for a given memory. Size ≈ `rank_1 × rank_2 × rank_3 = 4 × 4 × 128 = 2048 floats` + three factor matrices, total ≈ 18 KB per record versus 173 056 floats (~676 KB) for the raw tensor.

### 6.2 Data model (reuses existing)

`TuckerCore` already exists in `crates/context-graph-core/src/teleological/types.rs`:

```rust
pub struct TuckerCore {
    pub ranks: (usize, usize, usize),  // default (4, 4, 128)
    pub data: Vec<f32>,                // rank_1 * rank_2 * rank_3
    pub u1: Vec<f32>,                  // 13 × rank_1
    pub u2: Vec<f32>,                  // 13 × rank_2
    pub u3: Vec<f32>,                  // 1024 × rank_3
}
```

Wire it into `TrainingRecord`:

```rust
pub struct TrainingRecord {
    ...
    pub tucker_core: Option<TuckerCore>,  // present when includeTuckerCore=true
}
```

### 6.3 Implementation

Add `include_tucker_core` flag to `export_training_corpus`. When true, each memory:

1. Build the 13×13×1024 interaction tensor `T[i][j][d] = embedding_i[d] * embedding_j[d]` where `embedding_k` is the active vector for embedder k (asymmetric embedders use cause / source / paraphrase side).
2. Apply Tucker-1 decomposition via HOSVD (higher-order SVD) with ranks `(4, 4, 128)`.
3. Store result as `Option<TuckerCore>` on the training record.

HOSVD is built from three standard SVDs (mode-1, mode-2, mode-3 unfolding), which we can do on GPU via `candle`. We cap memory with a streaming mode-unfolding so we never materialize the full 173 K-float tensor — we compute `Cov = T_(n) @ T_(n).T` directly from the 13 vectors without building T explicitly.

Put the compiler in `crates/context-graph-core/src/teleological/tucker.rs` with a trait:

```rust
pub trait TuckerCompressor {
    fn compress(
        fingerprint: &SemanticFingerprint,
        ranks: (usize, usize, usize),
    ) -> CoreResult<TuckerCore>;
}
```

Two implementations: a CPU reference (`CpuTuckerCompressor`) and a GPU fast path (`CandleTuckerCompressor`) behind the `cuda` feature flag.

### 6.4 MCP surface

No new tools. The existing `get_training_record(..., includeTuckerCore=true)` surfaces it. Add a helper tool `score_tucker_similarity` that compares two memories via Tucker-core inner product — useful for finding structurally similar memories even when surface embeddings disagree.

### 6.5 FSV requirements

- **FSV-15**: For a memory whose 13 embeddings are all identical (degenerate synthetic), `u1` and `u2` should be rank-1 (first singular value dominant, others ≈ 0). Assert `u1[0][0] > 0.99` and `u1[0][1..] < 0.01`.
- **FSV-16**: Reconstruction error bound: `reconstruct(tucker) - full_tensor` Frobenius norm / `full_tensor` norm < 0.15 for non-degenerate synthetic data. The exact bound depends on the test data rank structure; pick a threshold that holds robustly.
- **FSV-17**: Round-trip the `TuckerCore` through bincode and assert every field preserves byte-for-byte.
- **FSV-18**: GPU vs CPU parity: both implementations produce reconstructions whose difference is < 1e-4 Frobenius on a fixed seeded input. Catches numerical regressions.

## 7. Phase 5 — Temporal Context Labels

### 7.1 Goal

E2 (recency), E3 (periodic), E4 (positional) are already computed at store time. They're excluded from topic detection per ARCH-04 — rightfully so, because temporal similarity is not topical similarity. But for **training labels**, they are high-signal features:

- "Sentence written at 2 AM" — E3 periodic encoding marks the hour
- "First turn of a conversation vs last turn" — E4 positional encoding marks the sequence
- "Stored yesterday vs stored last year" — E2 recency encoding

These labels enable pragmatic / discourse-aware training without requiring the consumer to re-derive them.

### 7.2 Data model

Add to `TrainingRecord`:

```rust
pub struct TemporalLabels {
    pub stored_at: DateTime<Utc>,
    pub stored_hour_utc: u8,                   // 0..23
    pub stored_day_of_week: u8,                // 0=Sun..6=Sat
    pub stored_month: u8,                      // 1..12
    pub age_seconds_at_export: i64,            // export_time - stored_at

    pub session_sequence: Option<u32>,         // position within session, if session-scoped
    pub session_total: Option<u32>,            // total memories in the session at store time
    pub relative_position: Option<f32>,        // session_sequence / session_total, [0, 1]

    pub periodic_bucket: PeriodicBucket,       // classification of (hour, dow)
    pub e2_recency_norm: f32,                  // L2 norm of E2 vector
    pub e3_periodic_norm: f32,
    pub e4_positional_norm: f32,
}

pub enum PeriodicBucket {
    WeekdayMorning,  WeekdayAfternoon,  WeekdayEvening,  WeekdayNight,
    WeekendMorning,  WeekendAfternoon,  WeekendEvening,  WeekendNight,
}
```

### 7.3 Derivation

Pure function of `fp.created_at` + `fp.source_metadata.session_id` + session cache state. No new model evaluation, no new CF reads beyond what Phase 1 already does. Lives in `crates/context-graph-core/src/training/temporal.rs`.

### 7.4 MCP surface

No new tools. Surfaced via `TrainingRecord.temporal_labels` when `includeTemporalLabels=true`.

### 7.5 FSV requirements

- **FSV-19**: Store a memory at a known UTC timestamp; export; assert `stored_hour_utc` and `stored_day_of_week` match expected.
- **FSV-20**: Create three memories in one session (stored sequentially). Export. Assert `session_sequence` is 1, 2, 3 and `relative_position` is ≈ 0.33, 0.67, 1.0.
- **FSV-21**: Synthetic clock-jump test: a memory stored Saturday 9 AM should land in `WeekendMorning`.
- **FSV-22**: E2/E3/E4 norms must be finite (no NaN / Inf) for every record.

## 8. File-format exports (optional last mile)

RocksDB is primary. Downstream consumers that want flat files get an **export CLI** built on top of the CFs. The MCP side stays RocksDB-only; the CLI is for one-shot dumps.

Location: `crates/context-graph-cli/src/commands/export_training.rs`. Commands:

```
context-graph-cli export training-corpus --format parquet --out ./out/corpus.parquet
context-graph-cli export constellations --format hdf5 --out ./out/constellations.h5
context-graph-cli export contrastive-pairs --format parquet --kind semantic-but-not-causal --out ./out/contrastive.parquet
context-graph-cli export training-corpus --format safetensors --tensors-only --out ./out/tensors/
```

Supported formats:

| Format       | Best for                               | Rust crate        |
| ------------ | -------------------------------------- | ----------------- |
| Parquet      | HuggingFace `datasets`, DuckDB         | `parquet` 52      |
| HDF5         | PyTorch / JAX dataloaders              | `hdf5` 0.8        |
| safetensors  | Tensor-only, fast load, no metadata    | `safetensors` 0.4 |
| JSONL        | Debug / human inspection only — **not** for training | none            |

Note on JSONL: the earlier user direction was "don't use json". We honor that for the primary CF storage. JSONL stays as a debug-only escape hatch with a `--debug` flag that refuses to run without it.

## 9. Testing strategy

### 9.1 Test inventory

| Layer            | Tests                                                                                   |
| ---------------- | --------------------------------------------------------------------------------------- |
| Unit (core)      | `training::tests`, `constellation::tests`, `tucker::tests`, `temporal::tests`           |
| Integration      | `context-graph-storage/tests/training_records_integration.rs` (extend for all CFs)      |
| MCP E2E          | `tests/fsv_harness.py` (hardened version with zombie-proof spawn + stderr drain)        |
| Zombie           | `tests/test_no_zombies.py` — runs before every release                                   |
| Math identities  | Per-phase FSV suite (§3.7, §4.6, §5.7, §6.5, §7.5)                                        |

### 9.2 CI gates

Add to `.github/workflows/ci.yml`:

1. `cargo build --release` with `CUDARC_CUDA_VERSION=13000` — must be 0 errors, 0 warnings.
2. `cargo test --workspace` — all green.
3. `python3 tests/test_no_zombies.py` — all 4 scenarios pass.
4. `python3 tests/fsv_harness.py --quick` — happy path + at least 3 edge cases.

### 9.3 Required discipline from CLAUDE.md §4

- Plan first: `tasks/todo.md` updated before each phase.
- Verify before done: every phase's FSV section is a checklist, not a recommendation.
- Lessons: any correction from the user updates `tasks/lessons.md`.

## 10. Rollout plan

| Week | Phase                                         | Gate                                                                       |
| ---- | --------------------------------------------- | -------------------------------------------------------------------------- |
| 1    | Phase 1 v2 (TrainingRecord, fix critical bug) | FSV-1..FSV-6 green; code-simplifier review pass                            |
| 2    | Phase 2 (Constellation compiler)              | FSV-7..FSV-10 green; compiler runs on live 10 K-memory corpus in < 30 s    |
| 3    | Phase 3 (Contrastive pair miner)              | FSV-11..FSV-14 green; at least 1 K pairs mined from live corpus            |
| 4    | Phase 4 (Tucker core)                         | FSV-15..FSV-18 green; GPU + CPU parity verified                            |
| 5    | Phase 5 (Temporal labels)                     | FSV-19..FSV-22 green                                                       |
| 6    | File-format export CLI                        | Parquet + HDF5 + safetensors dump of a 10 K corpus produces identical data  |
| 7    | Docs, examples, publish training-pipeline SDK | HuggingFace `datasets` recipe working end-to-end                           |

## 11. Open decisions (requires owner input)

1. **Tucker ranks**. Default `(4, 4, 128)` is inherited from `TuckerCore::default_ranks`. Is this the right point on the accuracy vs size curve for your training workload, or do we need `(8, 8, 256)` for richer interaction modeling?
2. **Contrastive pair storage cap**. 100 K pairs at ~2 KB each is 200 MB. A full cross-embedder mine over 10 K memories at top-K=10 per-anchor generates ~100 K candidates. Do we cap at a fixed count, or offer per-kind quotas (say 25 K per `AnomalyKind`)?
3. **Constellation refresh cadence**. Is a constellation a one-shot snapshot (compile once, store, consume), or does it auto-refresh when members change? Auto-refresh implies a background job and staleness tracking; static is simpler. Recommendation: static v1, with a `rebuild_constellation(id)` tool for explicit refresh.
4. **File-format export priority**. Parquet-first (HuggingFace ecosystem) or safetensors-first (tensor-only, minimal metadata)? Both eventually, but which lands first?
5. **Anti-zombie watchdog deadline**. Currently 20 s via `DOOMSDAY_SECS`. If a Tucker export at scale takes 30 s per memory, the watchdog will kill the server mid-export. Either move the watchdog to post-request-timeout-only, or bump the deadline, or split the export into explicit checkpoints.

## 12. Risks and mitigations

| Risk                                                                   | Mitigation                                                                             |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| Topic-profile derivation silently drifts from CF_TOPIC_PROFILES        | Phase 1 §3.3.2 mandates reading the CF directly; fallback path logs a `warn!`          |
| Incoming-edges path eats O(N²) RocksDB scans on opt-in                 | Phase 1 §3.3.4 requires the reverse-index or an honest schema; Phase 3's miner reuses it |
| Tucker HOSVD is numerically unstable on near-degenerate inputs         | Use truncated SVD with condition-number check; fall back to `u1 = identity` on failure |
| Contrastive miner floods CF with near-duplicate pairs                  | Composite key (anchor, neg) is unique; add `min_disagreement` threshold                |
| Temporal labels require session cache; cache misses produce nulls     | `session_sequence: Option<u32>` is already nullable; consumers handle None             |
| File-format export drifts from CF shape over time                      | CLI re-reads from CF via MCP tools (same contract); format tests verify roundtrip      |
| `prctl(PR_SET_PDEATHSIG)` doesn't fire on pid-namespaced container     | Fall through to doomsday watchdog (20 s); add `--allow-orphan` flag for containers     |

## 13. Acceptance criteria (ship when all of these are true)

1. `cargo build --release -p context-graph-mcp` succeeds with 0 warnings under `CUDARC_CUDA_VERSION=13000`.
2. `cargo test --workspace` passes, including all 22 FSV items listed across §3–§7.
3. `python3 tests/test_no_zombies.py` passes 4/4.
4. `python3 tests/fsv_harness.py` passes against a live MCP server.
5. A 10 K-memory training corpus export completes in < 60 s using < 500 MB resident memory.
6. The code-simplifier review of every phase's code returns "APPROVE" (no criticals, no unreviewed mediums).
7. This plan's §11 open decisions have been resolved by the owner and recorded in `tasks/lessons.md`.

---

## Appendix A — Summary of code-simplifier findings carried forward

From the review of the reverted Phase 1 first cut (`context-graph-mcp/src/handlers/tools/training_tools.rs`, `context-graph-core/src/training/mod.rs`, `context-graph-storage/src/teleological/rocksdb_store/export.rs`):

| # | Severity | Fix in phase | Summary                                                                 |
| - | -------- | ------------ | ----------------------------------------------------------------------- |
| 1 | Critical | §3.3.1       | `CausalLabel.related_memory_id` always equals self; store `rel.id` too  |
| 2 | Medium   | §3.3.2       | Topic profile derivation saturates at 1.0; read `CF_TOPIC_PROFILES`     |
| 3 | Medium   | §3.3.3       | Session filter double-fetches `get_source_metadata`                     |
| 4 | Medium   | §3.3.4       | `includeIncomingEdges` is O(N²); build reverse index once               |
| 5 | Low      | §3.3.5       | `dense_alignment` / `sparse_alignment` docstrings don't match code      |
| 6 | Low      | §3.3.6       | `list_training_records` shape loop should use `multi_get_cf`            |
| 7 | Low      | §3.3.7       | `arm_doomsday_watchdog` panics on spawn failure; log and continue       |
| 8 | Low      | §2.1         | Factor `uuid_prefixed_cf_options` helper — 6+ CFs duplicate boilerplate |
| 9 | Low      | FSV suite    | `cross_correlations_ordering` test only checks one pair, not ordering   |

## Appendix B — Dependencies between phases

```
Phase 1 (TrainingRecord)   — foundation, no deps
├─> Phase 2 (Constellation)  reuses TrainingRecord reader, adds new CF
├─> Phase 3 (Contrastive)    reuses TrainingRecord + cross_embedder_anomalies
├─> Phase 4 (Tucker)         extends TrainingRecord, adds no new CF
└─> Phase 5 (Temporal)       extends TrainingRecord, adds no new CF
                                              │
                                              └─> Phase 6 (File-format CLI) reads all CFs
```

Phase 1 is a hard prerequisite. Phases 2–5 can ship in any order once Phase 1 is green. Phase 6 (file-format CLI) requires at least one of 1–5 to be useful; recommend shipping after Phase 3 so the CLI can dump both training records and contrastive pairs in the same release.

## Appendix C — Files touched per phase

**Phase 1**:
- New: `context-graph-core/src/training/{mod,tests,temporal}.rs`
- New: `context-graph-storage/src/teleological/rocksdb_store/export.rs`
- New: `context-graph-storage/tests/training_records_integration.rs`
- New: `context-graph-mcp/src/tools/definitions/training.rs`
- New: `context-graph-mcp/src/handlers/tools/training_tools.rs`
- Modified: `context-graph-core/src/lib.rs`, `context-graph-storage/src/teleological/{column_families,mod,rocksdb_store/mod}.rs`, `context-graph-storage/src/column_families.rs`, `context-graph-mcp/src/tools/{names,definitions/mod}.rs`, `context-graph-mcp/src/handlers/tools/{mod,dispatch}.rs`
- Test updates: `context-graph-storage/src/teleological/tests/column_family.rs`

**Phase 2**:
- New: `context-graph-core/src/constellation/{mod,welford,reservoir,tests}.rs`
- New: `context-graph-storage/tests/constellation_integration.rs`
- New: `context-graph-mcp/src/{tools/definitions/constellation.rs,handlers/tools/constellation_tools.rs}`
- Modified: same set as Phase 1 plus the two new secondary-index CFs

**Phase 3**:
- New: `context-graph-core/src/contrastive/{mod,mining,tests}.rs`
- New: `context-graph-storage/tests/contrastive_integration.rs`
- New: `context-graph-mcp/src/{tools/definitions/contrastive.rs,handlers/tools/contrastive_tools.rs}`

**Phase 4**:
- New: `context-graph-core/src/teleological/tucker.rs`
- Modified: `context-graph-core/src/teleological/types.rs` (no-op; `TuckerCore` already defined)
- Modified: `context-graph-core/src/training/mod.rs` (add `tucker_core` field)

**Phase 5**:
- New: `context-graph-core/src/training/temporal.rs`
- Modified: `context-graph-core/src/training/mod.rs` (add `temporal_labels` field)

**Phase 6 (file-format CLI)**:
- New: `context-graph-cli/src/commands/export_training.rs`
- New: `context-graph-cli/src/commands/export_constellations.rs`
- New: `context-graph-cli/src/commands/export_contrastive.rs`
- Modified: `context-graph-cli/src/commands/mod.rs`, `context-graph-cli/src/main.rs`
- Dependency: add `parquet = "52"` and optionally `hdf5 = "0.8"` (only on non-Windows hosts) to the CLI crate

---

*End of plan. Revise this document in place as phases land; the code-simplifier checkpoint applies at the end of each phase, not at the end of the project.*
