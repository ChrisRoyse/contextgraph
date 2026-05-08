# Teleological Subsystem

**Source files covered:**

- `crates/context-graph-core/src/teleological/mod.rs` — module root, public re-exports
- `crates/context-graph-core/src/teleological/types.rs` — `NUM_EMBEDDERS`, `EMBEDDING_DIM`, `ProfileId`, `TopicProfile`, `TuckerCore`
- `crates/context-graph-core/src/teleological/embedder.rs` — `Embedder` enum (14 variants), `EmbedderDims`, `EmbedderMask`, `EmbedderGroup`
- `crates/context-graph-core/src/teleological/groups.rs` — `GroupAlignments`, `GroupType`, `NUM_GROUPS = 6`, group index tables
- `crates/context-graph-core/src/teleological/synergy_matrix/{mod,constants,types,constructors,accessors,analysis,presets,validation}.rs` — 14×14 cross-embedding synergy weights
- `crates/context-graph-core/src/teleological/matrix_search/{mod,config,types,strategies,search,weights,embedder_names}.rs` — cross-correlation search engine
- `crates/context-graph-core/src/teleological/profile/{mod,task_type,fusion_strategy,metrics,teleological}.rs` — task-specific weight profiles
- `crates/context-graph-core/src/teleological/comparator/{mod,teleological,strategies,batch,result}.rs` — apples-to-apples fingerprint comparator
- `crates/context-graph-core/src/teleological/{vector,resolution,meaning,comparison_error}.rs` — fused vector + multi-resolution hierarchy + meaning extraction config
- `crates/context-graph-core/src/teleological/services/{correlation_extractor,fusion_engine,group_aggregator,meaning_pipeline,multi_space_retriever,profile_manager,synergy_service,tucker_decomposer}.rs` — 8 logic-layer services
- `crates/context-graph-core/src/teleological/tucker.rs` — streaming HOSVD compressor
- `crates/context-graph-core/src/types/fingerprint/teleological/types.rs` — `TELEOLOGICAL_VECTOR_DIM = 13`, `TeleologicalFingerprint`
- `crates/context-graph-storage/src/teleological/column_families.rs` — `TELEOLOGICAL_CFS` array (41 CFs), `QUANTIZED_EMBEDDER_CF_COUNT = 14`
- `crates/context-graph-storage/src/teleological/rocksdb_store/{store,crud,fusion,search,index_ops,inverted_index,audit_log,...}.rs` — RocksDB-backed storage layer
- `crates/context-graph-mcp/src/handlers/tools/curation_tools.rs` — `boost_importance` handler

Cross-references: [04](04_database_schema.md) for the full CF catalog, [05](05_embedding_subsystem.md) for the 14-embedder fingerprint shape, [06](06_search_and_fusion.md) for the four search strategies, [12](12_clustering_and_topics.md) for HDBSCAN topics that feed `topic_profile_centroid`, [17](17_constellation_subsystem.md) for the constellation aggregation that consumes teleological fingerprints, [18](18_contrastive_and_typed_edges.md) for anomaly derivation, [21](21_dynamicjepa_subsystem.md) for the DJ analog of the teleological pipeline.

## 1. Definitions and scope

The teleological subsystem is the cross-embedder fusion machinery: it owns `TeleologicalFingerprint` (the per-memory record), the 13-dim teleological signature vector, and the comparator/profile/matrix-search machinery that lets the rest of the system treat the 14 embedding spaces as **one** queryable surface rather than 14 independent ones.

| Concern | Owner |
|---|---|
| Per-memory record | `TeleologicalFingerprint` (`types/fingerprint/teleological/types.rs:32`) |
| Compact "shape" of a memory across 13 spaces | `TELEOLOGICAL_VECTOR_DIM = 13` signature vector |
| Cross-embedder synergy weights | `SynergyMatrix` (14×14, `synergy_matrix/types.rs:13`) |
| Apples-to-apples comparison | `TeleologicalComparator` (`comparator/teleological.rs:36`) |
| Cross-correlation search across all embedders | `TeleologicalMatrixSearch` (`matrix_search/`) |
| Task-specific weight profiles | `TeleologicalProfile` (`profile/teleological.rs:17`) |
| Compact tensor representation | `TuckerCore` + `CpuTuckerCompressor` |
| RocksDB persistence | `RocksDbTeleologicalStore` (`rocksdb_store/store.rs`) |

The 14 embedders themselves and their search strategies live in [05](05_embedding_subsystem.md) and [06](06_search_and_fusion.md). The constellation aggregation (centroids over many fingerprints) lives in [17](17_constellation_subsystem.md). The learner-state UTL layer (per-learner observation envelopes) is documented in [20](20_learning_subsystem.md).

## 2. Constants

| Constant | Value | File |
|---|---|---|
| `TELEOLOGICAL_VECTOR_DIM` | 13 | `types/fingerprint/teleological/types.rs:12` |
| `NUM_EMBEDDERS` | 14 | `teleological/types.rs:217` |
| `Embedder::COUNT` | 14 | `teleological/embedder.rs:83` |
| `EMBEDDING_DIM` (canonical fusion dim) | 1024 | `teleological/types.rs:214` |
| `SYNERGY_DIM` | 14 | `teleological/synergy_matrix/constants.rs:10` |
| `CROSS_CORRELATION_COUNT` | 91 (= C(14, 2)) | `synergy_matrix/constants.rs:14` |
| `NUM_GROUPS` | 6 | `teleological/groups.rs:18` |
| `TuckerCore::DEFAULT_RANKS` | (4, 4, 128) | `teleological/types.rs:94` |
| `REFERENCE_EMBEDDING_DIM` (Tucker) | 1024 | `teleological/tucker.rs:73` |
| `TELEOLOGICAL_CF_COUNT` | 41 | `storage/teleological/column_families.rs:657` |
| `QUANTIZED_EMBEDDER_CF_COUNT` | 14 | `column_families.rs:746` |

The split between `TELEOLOGICAL_VECTOR_DIM = 13` and `NUM_EMBEDDERS = 14` is deliberate. The signature vector at `TeleologicalFingerprint::teleological_vector()` is **the compact clustering vector**: one f32 magnitude per embedder, used as a fast first-pass filter before the full multi-embedder search runs. E14 (BGE-M3 Dense) is excluded from this signature because its dense norm overlaps E1's information content; E14 is instead tracked as an additional fusion participant in the 14×14 synergy matrix and the 14-slot per-embedder `EmbedderStats`. The number of HNSW indexes (15: see [05](05_embedding_subsystem.md)) and the number of synergy-matrix entries (14) are both correct — they describe different aspects of the same system.

## 3. The 13-dim teleological signature vector

`TeleologicalFingerprint::teleological_vector() -> [f32; 13]` (`types/fingerprint/teleological/types.rs:105-122`):

| Idx | Source | Embedder | Aggregator |
|---|---|---|---|
| 0 | `s.e1_semantic` | E1 Semantic (1024D) | `l2_norm` |
| 1 | `s.e2_temporal_recent` | E2 Temporal-Recent (512D) | `l2_norm` |
| 2 | `s.e3_temporal_periodic` | E3 Temporal-Periodic (512D) | `l2_norm` |
| 3 | `s.e4_temporal_positional` | E4 Temporal-Positional (512D) | `l2_norm` |
| 4 | `s.e5_active_vector()` | E5 Causal (768D, asymmetric — active side) | `l2_norm` |
| 5 | `s.e6_sparse` | E6 Sparse Lexical (~30K vocab) | `sparse_norm` = `indices.len() / 1000` |
| 6 | `s.e7_code` | E7 Code (1536D) | `l2_norm` |
| 7 | `s.e8_active_vector()` | E8 Graph (1024D, asymmetric — source/target) | `l2_norm` |
| 8 | `s.e9_hdc` | E9 HDC (1024D) | `l2_norm` |
| 9 | `s.e10_active_vector()` | E10 Multimodal/Contextual (768D, asymmetric — paraphrase/context) | `l2_norm` |
| 10 | `s.e11_entity` | E11 Entity (Kepler 768D) | `l2_norm` |
| 11 | `s.e12_late_interaction` | E12 ColBERT (128D/token) | `token_avg_norm` (mean of per-token L2 norms; 0 when no tokens) |
| 12 | `s.e13_splade` | E13 SPLADE (~30K vocab) | `sparse_norm` |

`sparse_norm` divides by 1000 to keep magnitudes comparable to dense L2 norms — typical SPLADE has 200-1500 active terms, yielding 0.2-1.5. For asymmetric embedders (E5, E8, E10), `e{N}_active_vector()` returns the canonical side: cause / source / paraphrase, with legacy fallback when only one side is populated.

## 4. `TeleologicalFingerprint`

`crates/context-graph-core/src/types/fingerprint/teleological/types.rs:32-89`. Stored in `CF_FINGERPRINTS` keyed by 16-byte UUID; values are ~46 KB (~63 KB when E6 sparse is present).

| Field | Type | Notes |
|---|---|---|
| `id` | `Uuid` | UUID v4 |
| `semantic` | `SemanticFingerprint` | 14 embedding slots (E1-E14, dual cause/effect for E5, dual source/target for E8, dual paraphrase/context for E10) |
| `content_hash` | `[u8; 32]` | SHA-256 of source content |
| `created_at`, `last_updated` | `DateTime<Utc>` | wall-clock |
| `access_count` | `u64` | bumped on retrieval |
| `importance` | `f32` clamped `[0, 1]` | default `0.5`; consumed by consolidation, `boost_importance`, dream phases |
| `last_accessed_at` | `DateTime<Utc>` | `#[serde(skip)]` — bincode is positional, so this lives in memory only and resets to `Utc::now()` on deserialization |
| `e6_sparse` | `Option<SparseVector>` | `#[serde(default)]` — original sparse vector for inverted-index recall and exact term matching; ~235 active terms typical (~1.4 KB) |

Why `serde(skip)` for `last_accessed_at`: bincode is a positional format. Adding a field to the bincode layout breaks deserialization of all existing data. For persistent access tracking a separate CF would be required. The on-startup default (`Utc::now()`) is conservative — old memories appear "just accessed" rather than unfairly penalized.

`importance` is clamped to `[0, 1]` only at the **handler** level. The `boost_importance` MCP handler (`curation_tools.rs:147-260`) re-reads the fingerprint, applies the delta, clamps via `BoostImportanceRequest::apply_delta`, writes the updated record, AND appends an `ImportanceChangeRecord` to `CF_IMPORTANCE_HISTORY`. The constructor clamp is bypassed by serde deserialization — this is the locked rule "importance clamping must happen at handler level" recorded in `MEMORY.md`.

## 5. The 14 embedders (`Embedder` enum)

`teleological/embedder.rs:46-79` is the single source of truth for embedder identity. Variant values 0-13 are stable and used for array indexing — never reorder.

| Idx | Variant | Short | Purpose | Dims | Type |
|---|---|---|---|---|---|
| 0 | `Semantic` | E1 | `V_semantic` | 1024 | Dense |
| 1 | `TemporalRecent` | E2 | `V_temporal_recent` | 512 | Dense |
| 2 | `TemporalPeriodic` | E3 | `V_temporal_periodic` | 512 | Dense |
| 3 | `TemporalPositional` | E4 | `V_temporal_positional` | 512 | Dense |
| 4 | `Causal` | E5 | `V_causality` | 768 | Dense (asymmetric cause/effect) |
| 5 | `Sparse` | E6 | `V_sparse` | 30 522 | Sparse |
| 6 | `Code` | E7 | `V_code` | 1536 | Dense |
| 7 | `Graph` | E8 | `V_connectivity` | 1024 | Dense (asymmetric source/target) |
| 8 | `Hdc` | E9 | `V_hdc` | 1024 | Dense (projected from binary) |
| 9 | `Contextual` | E10 | `V_multimodal` | 768 | Dense (asymmetric paraphrase/context) |
| 10 | `Entity` | E11 | `V_entity` | 768 | Dense (KEPLER) |
| 11 | `LateInteraction` | E12 | `V_late_interaction` | 128/token | TokenLevel |
| 12 | `KeywordSplade` | E13 | `V_keyword` | 30 522 | Sparse |
| 13 | `BgeM3Dense` | E14 | `V_style_dense_multilingual` | 1024 | Dense |

`Embedder::default_temperature(self)` calibrates per-embedder confidence: E5 = 1.2 (overconfident), E7 = 0.9 (precision-needed), E9 = 1.5 (noisy), E13 = 1.1, all others = 1.0. `Embedder::from_name` accepts canonical and short forms (case-insensitive); old "Emotional" names for E8 are FAIL-FAST rejected (no backwards compat).

`EmbedderMask` (`embedder.rs:344-409`) is a `u16` bitmask for selecting subsets. `EmbedderGroup` (`:415-467`) defines reusable groups: `Temporal` (E2/E3/E4), `Relational` (E4/E5/E11), `Lexical` (E6/E12/E13), `Dense` (all minus E6/E12/E13), `Factual` (E1/E12/E13), `Implementation` (E7), `All`.

## 6. `TopicProfile` and the multi-resolution hierarchy

`TopicProfile` (`teleological/types.rs:235-329`) is a 14-D alignment vector — one f32 in `[0, 1]` per embedder — used as the per-memory topic-membership signal. Two `TopicProfile`s compose via cosine similarity (`:266`); `aggregate_alignment()` returns the mean, `active_count(threshold)` counts slots above threshold, `get`/`set` panic on out-of-bounds (FAIL FAST).

The `MultiResolutionHierarchy` (`teleological/resolution.rs`) builds a 4-level pyramid from a `TopicProfile`:

| Level | Dim | Type | Computed by |
|---|---|---|---|
| 0 | 14 | full topic profile (per-embedder alignments) | direct |
| 1 | 6 | group alignments (`GroupAlignments`) | `from_alignments` |
| 2 | 3 | `DomainAlignments` (What / How / Why) | derived from groups |
| 3 | 1 | `quick_score()` (one scalar) | mean of level-2 |

`GroupAlignments` (`groups.rs:30`) gives one weighted-average alignment per `GroupType`. The 6 groups and their embedder indices:

| Group | Indices | Captures | Comparator-strategy alias |
|---|---|---|---|
| `Factual` | 0, 11, 12, 13 (E1, E12, E13, E14) | what IS | Factual |
| `Temporal` | 1, 2 (E2, E3) | when/sequence | Temporal |
| `Causal` | 3, 6 (E4, E7) | why/how | Causal |
| `Relational` | 4, 7, 8 (E5, E8, E9) | like/where/who | Relational |
| `Qualitative` | 9, 10 (E10, E11) | feel/principle | Qualitative |
| `Implementation` | 5 (E6) | code | Implementation |

`GroupAlignments::dominant_group()` returns the highest-scoring group; `weakest_group()` the lowest; `coherence()` returns `1 / (1 + std_dev)` over the 6 group scores. Note that the comparator's group map (`comparator/teleological.rs:248-255`) intentionally uses a slightly different grouping for breakdown reporting — the canonical group definitions live in `groups.rs`.

## 7. `SynergyMatrix` — 14×14 cross-embedding weights

`synergy_matrix/types.rs:13`:

```rust
pub struct SynergyMatrix {
    pub values: [[f32; SYNERGY_DIM]; SYNERGY_DIM],   // 14×14
    pub weights: [[f32; SYNERGY_DIM]; SYNERGY_DIM],  // per-cell adaptive weights
    pub computed_at: DateTime<Utc>,
    pub sample_count: u64,
}
```

Invariants enforced by `validation.rs`:
- diagonal always 1.0 (self-synergy)
- matrix is symmetric: `values[i][j] == values[j][i]`
- values in `[0, 1]`

`with_base_synergies()` (`constructors.rs:51`) is the canonical instantiation used by every constellation compile (`constellation_tools.rs:114`) and by every `aggregate_synergy` call in the comparator. The base values are coded in `BASE_SYNERGIES` (`constants.rs:36-65`): three tiers — weak (0.3), moderate (0.6), strong (0.9). Examples: E1↔E5 = 0.9 (semantic-causal strong), E1↔E14 = 0.9 (multilingual semantic), E2↔E3 = 0.9 (episodic-temporal strong), E6↔E13 = 0.9 (sparse↔SPLADE strong).

`CROSS_CORRELATION_COUNT = 91` is the upper-triangle pair count `C(14, 2)`. The `MultiResolutionHierarchy` and the `cross_correlation_centroid` in `Constellation` both use this same count.

## 8. `TeleologicalComparator` — apples-to-apples fingerprint comparison

`comparator/teleological.rs:36-180`. Per the constitution `ARCH-02` rule: each embedder's output is compared **only** with the same embedder's output from the other fingerprint. Cross-embedder comparison is forbidden.

Per-slice routing (`compare_embedder_slices`, `:128-179`):

| Slice variant | Embedders | Function | Normalization |
|---|---|---|---|
| `Dense(_, _)` | E1, E2, E3, E4, E5, E7, E8, E9, E10, E11, E14 | `cosine_similarity` | `(raw + 1) / 2` → `[0, 1]` |
| `Sparse(_, _)` | E6, E13 | `SparseVector::jaccard_similarity` | naturally in `[0, 1]` |
| `TokenLevel(_, _)` | E12 | `max_sim` | `clamp(0, 1)` (raw MaxSim returns `[-1, 1]`) |
| zero-magnitude dense | any | returns `Some(0.0)` | — |
| empty / dim-mismatch | any | returns `None` | — |

After the 14 per-embedder scores are populated, `compare_with_strategy` (`:84-120`) runs `aggregate(scores, strategy)` (the `AggregationStrategies` trait, `comparator/strategies.rs`):

| `SearchStrategy` | Aggregation | File |
|---|---|---|
| `Cosine` (default) | unweighted mean of present scores | `aggregate_mean` |
| `Euclidean` | `1 / (1 + sqrt(Σ(1-s_i)² / n))` | `aggregate_euclidean` |
| `SynergyWeighted` | `Σ(s_i · synergy[i][i]) / Σ(synergy[i][i])` (diagonal weights) | `aggregate_synergy` |
| `GroupHierarchical` | per-group mean → group-weighted mean | `aggregate_hierarchical` |
| `CrossCorrelationDominant` | `Σ((s_i+s_j)/2 · synergy[i][j]) / Σ(synergy[i][j])` over off-diagonal pairs | `aggregate_correlation` |
| `TuckerCompressed` | mean weighted by closeness to score-distribution center | `aggregate_tucker` |
| `Adaptive` | choose by coefficient-of-variation: `<0.1` → mean, `<0.3` → synergy, else Tucker | `aggregate_adaptive` |

`compute_coherence` (`teleological.rs:184-207`) reports `1 / (1 + CoV)` over the per-embedder scores. `find_dominant_embedder` returns the highest-scoring `Embedder`. `generate_breakdown` (`:227-276`) populates a `SimilarityBreakdown { overall, topic_profile, cross_correlations, group_alignments, per_group, per_embedder_topic[14], top_correlation_pairs, strategy_used }`.

`BatchComparator` (`comparator/batch.rs`) runs the same pipeline over a `&[SemanticFingerprint]` for many-vs-many comparisons.

## 9. `TeleologicalMatrixSearch` — cross-correlation search engine

`matrix_search/`. Exposes:

| Type | Purpose | File |
|---|---|---|
| `TeleologicalMatrixSearch` | Search engine entry point | `matrix_search/search/` |
| `MatrixSearchConfig` | Runs strategy + scope + weights + optional `synergy_matrix` | `config.rs` |
| `SearchStrategy` (7 variants) | See §8 | `types.rs:13-30` |
| `ComparisonScope` | What to compare | `types.rs:33-50` |
| `ComponentWeights` | Per-component weight (topic, correlation, group) | `weights.rs` |
| `SimilarityBreakdown` | Full per-component report | `types.rs:53-86` |
| `ComprehensiveComparison` | Multi-scope batched output | `types.rs:89-119` |
| `embedder_names()` | Canonical name table | `embedder_names.rs` |

`ComparisonScope` variants (`types.rs:33-50`):

| Variant | Compares |
|---|---|
| `Full` (default) | topic profile + 91 cross-correlations + group alignments |
| `TopicProfileOnly` | 14D topic profile |
| `CrossCorrelationsOnly` | 91 cross-correlation entries |
| `GroupAlignmentsOnly` | 6D group alignments |
| `SpecificPairs(Vec<(usize, usize)>)` | one or more `(embedder_a, embedder_b)` pairs |
| `SpecificGroups(Vec<GroupType>)` | one or more group families |
| `SingleEmbedderPattern(usize)` | the 12 pairs that one embedder is in |

`SimilarityComputer` and the helper functions in `strategies.rs` (`compute_correlation_similarity`, `compute_purpose_similarity`, `compute_single_embedder_pattern_similarity`, `compute_specific_groups_similarity`, `compute_specific_pairs_similarity`) implement the per-scope math.

## 10. `TeleologicalProfile` — task-specific weight profiles

`profile/teleological.rs:17-246`. Owns:

| Field | Type | Notes |
|---|---|---|
| `id` | `ProfileId` | non-empty (FAIL FAST) |
| `name` | `String` | human-readable |
| `embedding_weights` | `[f32; 14]` | per-embedder weight, normalized to sum-to-1 by `normalize_weights()` |
| `fusion_strategy` | `FusionStrategy` | enumerated (PrimaryOnly, WeightedAverage, Hierarchical, CrossCorrelation, AttentionWeighted, …) |
| `task_type` | `TaskType` | one of 8 (see below) |
| `created_at`, `updated_at` | `DateTime<Utc>` | wall-clock |
| `sample_count` | `u64` | how many samples shaped this profile |
| `metrics` | `ProfileMetrics` | observed retrieval performance |
| `is_system` | `bool` | true for built-in profiles |
| `description` | `Option<String>` | optional |

`TaskType` (`profile/task_type.rs:19-122`): 8 variants. Each lists primary and secondary embedder indices and a suggested fusion strategy.

| TaskType | Primary | Secondary | Suggested strategy |
|---|---|---|---|
| `CodeSearch` | E6, E7 (5, 6) | E4, E12 | `PrimaryOnly` |
| `SemanticSearch` | E1, E5 (0, 4) | E11, E8 | `CrossCorrelation` |
| `TemporalSearch` | E2, E3 (1, 2) | E12, E9 | `Hierarchical` |
| `CausalSearch` | E4, E7 (3, 6) | E12, E9 | `Hierarchical` |
| `FactualSearch` | E12, E13 (11, 12) | E1, E4 | `WeightedAverage` |
| `SocialSearch` | E9, E10 (8, 9) | E1, E2 | attention-weighted |
| `AbstractSearch` | E11, E5 (10, 4) | E1, E4 | `CrossCorrelation` |
| `General` (default) | E1, E4, E12 | E5, E6, E7, E8, E9 | `WeightedAverage` |

Built-in system profiles (factory methods on `TeleologicalProfile`):

| Profile | Weights pattern | File |
|---|---|---|
| `code_implementation()` | `[0.05, 0.02, 0.05, 0.15, 0.08, 0.25, 0.18, 0.05, 0.02, 0.02, 0.05, 0.05, 0.03, 0.0]` (E14 = 0) | `:117-127` |
| `conceptual_research()` | `[0.12, 0.05, 0.03, 0.10, 0.15, 0.03, 0.02, 0.05, 0.05, 0.05, 0.20, 0.12, 0.03, 0.0]` | `:139-149` |
| `system(task_type)` | reset baseline 0.05, primary → 0.20, secondary → 0.10, normalize | `:73-105` |

`set_weight(idx, w)` panics on negative weights or out-of-bounds index (FAIL FAST). `top_embedders(n)` returns the indices of the n highest-weighted embedders. `similarity(other)` returns the cosine similarity of two weight vectors. Profiles are persisted as JSON in `CF_CUSTOM_WEIGHT_PROFILES` (see §13).

## 11. `TuckerCore` and the streaming HOSVD compressor

`teleological/types.rs:73-211` and `teleological/tucker.rs`.

`TuckerCore` is a Tucker-decomposition of the conceptual 14×14×1024 cross-embedding interaction tensor `T[i][j][d] = v_i[d] * v_j[d]`. The full tensor is never materialized; the compressor streams the 14 reference-frame vectors directly into the mode-Gram matrices.

Layout:

| Field | Shape | Bytes (default) |
|---|---|---|
| `ranks: (r1, r2, r3)` | three usizes | ~24 |
| `data` | `r1·r2·r3` f32 (default 4·4·128 = 2048) | 8 192 |
| `u1` | `NUM_EMBEDDERS · r1` f32 (14·4 = 56) | 224 |
| `u2` | `NUM_EMBEDDERS · r2` f32 (14·4 = 56) | 224 |
| `u3` | `EMBEDDING_DIM · r3` f32 (1024·128 = 131 072) | 524 288 |

`compression_ratio()` reports `original / compressed` where `original = NUM_EMBEDDERS² · EMBEDDING_DIM = 200 704` floats. `with_default_ranks()` instantiates with `(4, 4, 128)`. `get_core(i, j, k)` and `set_core(i, j, k, v)` panic on bounds violations (FAIL FAST).

`CpuTuckerCompressor` (`tucker.rs`) implements the streaming HOSVD:
- All 14 embeddings are projected onto a shared 1024-D reference frame: E1/E8-source/E9 used as-is; E2/E3/E4/E5-cause/E10-paraphrase/E11 zero-padded right; E7 truncated to first 1024 entries; E6/E13 sparse → all-zeros; E12 → mean over tokens, zero-padded.
- Mode-1 and mode-2 Gram matrices (14×14) computed via Jacobi eigendecomposition (max 50 sweeps, tolerance 1e-14).
- Mode-3 Gram (1024×1024) factored via power iteration with deflation (max 500 iterations, tolerance 1e-5).
- Total work for default ranks: ≈ 2M FMA per fingerprint.
- `EMPTY_ENERGY_THRESHOLD = 1e-24` — below this the compressor returns `TuckerError::EmptyFingerprint`.

CPU-only by design (per Phase 4 plan §6.3); no GPU path.

## 12. Logic-layer services

`teleological/services/` registers 8 services that orchestrate the above primitives:

| Service | File | Role |
|---|---|---|
| `SynergyService` | `synergy_service.rs` | Cross-embedding synergy computation and refinement |
| `CorrelationExtractor` | `correlation_extractor.rs` | Extract the 91 cross-correlation entries |
| `MeaningPipeline` | `meaning_pipeline.rs` | Full embedding → `ExtractedMeaning` pipeline |
| `TuckerDecomposer` | `tucker_decomposer.rs` | Wraps `CpuTuckerCompressor` |
| `GroupAggregator` | `group_aggregator.rs` | 14D → 6D group aggregation |
| `FusionEngine` | `fusion_engine.rs` | Multi-embedding fusion orchestration |
| `MultiSpaceRetriever` | `multi_space_retriever.rs` | Teleological-aware retrieval |
| `ProfileManager` | `profile_manager.rs` | CRUD for `TeleologicalProfile`s |

`MeaningExtractionConfig`, `ExtractedMeaning`, `FusionMethod`, and `NuanceDimension` (`teleological/meaning.rs`) configure the meaning pipeline. `TeleologicalVector` (`vector.rs`) is the fused per-task representation produced by `FusionEngine`.

## 13. Storage layer

The teleological store is `RocksDbTeleologicalStore` (`crates/context-graph-storage/src/teleological/rocksdb_store/store.rs`). It opens 41 teleological CFs + 14 quantized embedder CFs + 5 code CFs + 2 causal CFs (plus DJ CFs) — 101 total at DB open.

### 13.1 The 41 teleological CFs (`TELEOLOGICAL_CFS`, `column_families.rs:612-654`)

| # | CF name | Purpose | Key | Value |
|---|---|---|---|---|
| 1 | `fingerprints` | Primary `TeleologicalFingerprint` rows | UUID (16 B) | bincode TF (~46-63 KB) |
| 2 | `topic_profiles` | 14D topic-profile vectors per memory | UUID | 14 × f32 = 56 B |
| 3 | `e13_splade_inverted` | SPLADE inverted index for sparse recall | term_id (u16) | `Vec<Uuid>` |
| 4 | `e6_sparse_inverted` | E6 sparse inverted index | term_id (u16) | `Vec<Uuid>` |
| 5 | `e1_matryoshka_128` | E1 truncated 128D vectors | UUID | 128 × f32 = 512 B |
| 6 | `content` | Original UTF-8 text (LZ4) | UUID | up to 1 MB |
| 7 | `source_metadata` | Provenance JSON | UUID | ~100-500 B |
| 8 | `file_index` | path → `FileIndexEntry { Vec<Uuid> }` | UTF-8 path | bincode |
| 9 | `topic_portfolio` | Persisted `PersistedTopicPortfolio` | session_id or `__latest__` | JSON |
| 10 | `e12_late_interaction` | E12 ColBERT token embeddings | UUID | `Vec<Vec<f32>>` bincode (10-256 KB) |
| 11 | `entity_provenance` | Reserved | `{canonical_id}_{uuid}` | bincode |
| 12 | `audit_log` | Append-only `AuditRecord` | `[ts_nanos_BE:8][uuid:16]` | JSON |
| 13 | `audit_by_target` | Audit secondary index | `[uuid:16][ts_nanos_BE:8]` | primary key bytes |
| 14 | `merge_history` | Permanent merge lineage | `[merged_uuid:16][ts_nanos_BE:8]` | JSON `MergeRecord` |
| 15 | `importance_history` | Permanent importance change history | `[uuid:16][ts_nanos_BE:8]` | JSON `ImportanceChangeRecord` |
| 16 | `tool_call_index` | Reserved — `tool_use_id` → `Vec<Uuid>` | tool_use_id | bincode |
| 17 | `consolidation_recommendations` | Reserved | — | — |
| 18 | `embedding_registry` | Embedder registry/version tracking | — | — |
| 19 | `custom_weight_profiles` | `TeleologicalProfile` JSON storage | profile name | JSON |
| 20 | `hnsw_graphs` | Persisted usearch HNSW blobs | `graph:{embedder}` / `meta:{embedder}` | bytes / JSON |
| 21 | `training_records` | Training-data export rows | UUID | `[ver][bincode]` |
| 22 | `learning_events` | Learning event log | UUID | bincode |
| 23 | `learner_profile` | `LearnerProfile` records | learner UUID (16) | bincode |
| 24 | `learner_constellations` | Per-learner regulated-state baselines | `[learner:16][selector_kind:u8]` | bincode |
| 25 | `fingerprints_learner` | `LearnerFingerprint` per session | `[learner:16][session_ts_BE:8]` | bincode |
| 26 | `learner_m_per_trace` | Per-trace M(t) records | `[learner:16][trace:16]` | bincode |
| 27 | `learner_state_history` | `LearnerStateVector` history | `[learner:16][session_ts_BE:8]` | bincode |
| 28 | `learner_goal_states` | `LearnerGoalState` per (learner, skill) | `[learner:16][skill:16]` | bincode |
| 29 | `learner_retrieval_log` | Retrieval-practice outcomes | `[learner:16][trace:16][ts_BE:8]` | bincode |
| 30 | `learner_k_sleep` | Sleep-pressure scalar history | `[learner:16][session_ts_BE:8]` | bincode |
| 31 | `goal_centroids` | Per-(skill, modality) expert centroids | `[skill:16][modality:u8]` | bincode |
| 32 | `learner_delta_log` | Per-session ΔS/ΔC/ΔE | `[learner:16][session_ts_BE:8]` | bincode |
| 33 | `learner_audit` | Per-learner audit trail | `[learner:16][ts_BE:8][audit:16]` | bincode |
| 34 | `learner_training_datasets` | Compiled training matrices per learner | dataset UUID | bincode |
| 35 | `constellations` | `Constellation` records | UUID (16) | `[CONSTELLATION_VERSION:u8][bincode]` — see [17](17_constellation_subsystem.md) |
| 36 | `constellation_by_selector` | Selector → constellation UUID secondary index | `[kind:u8][sha256(canonical)[..16]]` (17 B) | UUID (16) |
| 37 | `contrastive_pairs` | Mined `(anchor, negative)` rows | `[anchor:16][neg:16]` (32 B) | bincode |
| 38 | `contrastive_by_kind` | Anomaly-kind index | `[kind:u8][anchor:16][neg:16]` (33 B) | empty marker |
| 39 | `contrastive_by_anchor` | Anchor → `Vec<Uuid>` index | anchor UUID (16) | append-bincode |
| 40 | `typed_edge_records` | Per-typed-edge training records | `[source:16][target:16][edge_type:u8]` (33 B) | `[ver][bincode]` |
| 41 | `typed_edge_validations` | LLM-validated typed-edge verdicts | same 33-byte composite | `[ver][bincode]` |

### 13.2 The 14 quantized-embedder CFs

`column_families.rs:728-743` allocates one CF per embedder (`emb_0` … `emb_13`) for a `QuantizedEmbedding` rendered via the appropriate scheme: PQ-8 for high-dim dense (E1, E5, E7, E10, E11, E14), Float8E4M3 for mid-dim (E2/E3/E4/E8), Binary for E9 HDC (~1250 B), `SparseNative` for E6/E13, `TokenPruning` for E12. Reserved for forward-compatibility — not yet wired into the production write path.

Critical lessons: PQ-8 must store actual `(scale, bias)` (hardcoding `[-1, 1]` corrupts dequantization when data has different range); for asymmetric embedders both vectors must be quantized independently — cloning cause→effect destroys the dual-vector signal entirely.

### 13.3 Audit log, merge history, importance history

| CF | Behavior | Source |
|---|---|---|
| `audit_log` + `audit_by_target` | Append-only — no update / no delete. Primary key `[ts_nanos_BE:8][uuid:16]` ensures chronological iteration; secondary key `[uuid:16][ts_nanos_BE:8]` enables "all operations on memory X". | `column_families.rs:222-250`; impl in `rocksdb_store/audit_log.rs` |
| `merge_history` | Permanent — never expires, never deleted. Stores `MergeRecord` JSON keyed by `[merged_uuid:16][ts_nanos_BE:8]` for full lineage. | `:259-272` |
| `importance_history` | Permanent. Every change to `TeleologicalFingerprint.importance` writes an `ImportanceChangeRecord` JSON keyed by `[memory_uuid:16][ts_nanos_BE:8]`. Written by the `boost_importance` handler after the clamp. | `:274-286` |

### 13.4 Soft-delete

`crud.rs:33-38`: soft-delete keys live under `soft_deleted::{uuid}` with an i64 millis-since-epoch value. The store maintains an in-memory `soft_deleted: HashMap<Uuid, i64>` mirror but **persists** the keys to RocksDB so the state survives restart (the in-memory map is rebuilt on open). `is_soft_deleted(&id)` is consulted by `get` (`:60`) and `retrieve` (`:124`) — soft-deleted memories are invisible to queries but still recoverable.

`gc_soft_deleted(retention_secs)` (`:493-595`) walks the in-memory map and hard-deletes entries older than the cutoff. After hard-deletes a manual `compact_range_cf` runs to reclaim disk — without it, RocksDB tombstones persist until the next auto-compaction.

### 13.5 HNSW indexes

15 HNSW indexes are built on the dense embedders, declared in `teleological/indexes/`:

`E1, E1Matryoshka128, E2, E3, E4, E5, E5CausalCause, E5CausalEffect, E7, E8, E9, E10, E10MultimodalParaphrase, E10MultimodalContext, E11`.

E6/E13 use inverted indexes (not HNSW); E12 uses MaxSim against the token store. Persisted blobs live in CF `hnsw_graphs`. `try_load_hnsw_indexes()` returns `Ok(true)` when ANY index loads — newly-enabled embedders get empty HNSW until an explicit rebuild fires (lesson recorded in `MEMORY.md`).

### 13.6 Stale-lock detection

`store.rs:145-198` (`open_with_config`) probes the RocksDB LOCK file for diagnostic logging only; it never removes the lock. `inspect_rocksdb_lock` (`:474-559`) uses `fcntl(F_SETLK, F_UNLCK)` to determine whether the lock is held: missing → fine; held by another PID → log warning, leave in place (RocksDB's open will reject); not held → log, leave for RocksDB. `paranoid_checks(true)` is enabled at ~1-3% perf cost to catch SST corruption on open. After open, all `DJ_CF_NAMES` are asserted present (`:200-210`); a missing CF is a hard `TeleologicalStoreError::ColumnFamilyNotFound`.

The MCP daemon has a separate stale-lock workflow at the process level — see `MEMORY.md` for the lessons "alive process ≠ serving process" and "`kill(pid, 0)` returns 0 for zombies".

## 14. Importance signaling and access counting

| Signal | Field | Update path | Persistence |
|---|---|---|---|
| Memory was retrieved | `access_count: u64` | bumped by `consolidation.rs:104, 385` on retrieval | persisted in `CF_FINGERPRINTS` |
| Last read time | `last_accessed_at: DateTime<Utc>` | reset on every deserialize (memory-only) | NOT persisted (`#[serde(skip)]`) |
| Manual + decay-driven priority | `importance: f32` clamped `[0, 1]` | `boost_importance` handler clamps + re-writes + appends `ImportanceChangeRecord` | persisted in `CF_FINGERPRINTS` and CF `importance_history` |
| Effective importance during search | computed | `effective_importance = importance * 0.5^(days_since_access / 30)` (see consolidation) | not stored — recomputed |

The `boost_importance` MCP handler (`curation_tools.rs:147-260`) is the SINGLE legitimate writer of `importance`. Reading the current value, applying the delta, clamping, and writing back **must** be done through this path so the audit row in `importance_history` lands with correct `old`/`new` and the consolidation pipeline sees consistent state.

## 15. Validation invariants

- `ProfileId::new` panics on empty (FAIL FAST).
- `TuckerCore::new` panics on zero rank, `get_core`/`set_core` panic on out-of-bounds.
- `TopicProfile::get`/`set` panic on out-of-bounds embedder index.
- `TeleologicalProfile::set_weight` panics on negative or out-of-bounds.
- `WelfordVector::observe` panics on dimension mismatch (would silently corrupt centroids otherwise).
- `SynergyMatrix::validate` enforces diagonal == 1.0, symmetric, values in `[0, 1]`.
- `ComponentWeights::validate` runs FAIL FAST before any comparator math.

## 16. Cross-cutting lessons

- **bincode + `skip_serializing_if` is BROKEN** — bincode is positional; use JSON for any record that ever has optional fields. `TeleologicalFingerprint.last_accessed_at` works around this with `#[serde(skip)]` + `#[serde(default = ...)]`.
- **Schema defaults must match handler defaults** — `decayFunction` schema said `"exponential"` while the handler defaulted to `linear`; align them in both places.
- **MCP `additionalProperties: false` blocks unlisted params** — handler params NOT in the schema are silently dropped. Audit schema↔DTO alignment after every change.
- **Hard-delete order matters** — HNSW removal before RocksDB commit = data invisible if the RocksDB write fails. Always persist to disk first, then remove from index.
- **Background-task compaction can race with concurrent inserts** — `RocksDbTeleologicalStore` exposes a `compaction_lock: RwLock<()>` (`store.rs:120-127`) so compaction takes an exclusive write lock and stores/deletes a concurrent read lock.
- **Importance clamping must happen at handler level** — the constructor clamp is bypassed by serde deserialization, so `boost_importance` enforces `[0, 1]` before write.
- **`teleological_vector()` is 13-D, the synergy matrix is 14×14** — different concerns (clustering vs. fusion). Don't conflate.

## 17. What is NOT covered here

- The 4 search strategies (`e1_only`, `multi_space`, `pipeline`, `embedder_first`) and how they use the teleological primitives — see [06](06_search_and_fusion.md).
- The 14-embedder fingerprint shape (E1-E14, dual asymmetric vectors) — see [05](05_embedding_subsystem.md).
- Constellation aggregation (centroids over many fingerprints, MCP tools) — see [17](17_constellation_subsystem.md).
- Topic clustering and `topic_profiles` population — see [12](12_clustering_and_topics.md).
- Learner-state UTL records, ΔS/ΔC/ΔE math, M-trace, spaced-repetition — see [20](20_learning_subsystem.md).
- Anomaly derivation that consumes typed-edge per-embedder scores — see [18](18_contrastive_and_typed_edges.md).
- DynamicJEPA's separate teleological analog — see [21](21_dynamicjepa_subsystem.md).
- GPU acceleration for batched cosine and HDBSCAN — see [11](11_cuda_acceleration.md).
