# Teleological Subsystem

**Source files covered:**
- `crates/context-graph-core/src/teleological/{mod,types,embedder,groups,resolution,vector,meaning,synergy_matrix,matrix_search,comparison_error,comparator,tucker}.rs`
- `crates/context-graph-core/src/teleological/profile/{mod,task_type,fusion_strategy,metrics,teleological}.rs`
- `crates/context-graph-core/src/teleological/services/{mod,correlation_extractor,fusion_engine,group_aggregator,meaning_pipeline,multi_space_retriever,profile_manager,synergy_service,tucker_decomposer}.rs`
- `crates/context-graph-core/src/types/fingerprint/**/*.rs` — `SemanticFingerprint`, `TeleologicalFingerprint`
- `crates/context-graph-core/src/learner/mod.rs` — learner-state UTL primitives (modalities, fingerprints, goal centroids)
- `crates/context-graph-storage/src/teleological/rocksdb_store/{learner,anomaly_derivation,constellation,store,trait_impl}.rs`
- `crates/context-graph-mcp/src/handlers/tools/{utl_tools,capability_tools,typed_edges_tools,constellation_tools}.rs`

Cross-references: see [04](04_database_schema.md) for the 41 teleological / learner / training column families, [05](05_embedding_subsystem.md) for the 14-embedder fingerprint shape, [17](17_constellation_subsystem.md) for constellation aggregation, [18](18_contrastive_and_typed_edges.md) for anomaly derivation outputs, [20](20_learning_subsystem.md) for learner training datasets.

## 1. Two layers under one name

The "teleological subsystem" covers two superimposed concerns sharing the prefix and storage path:

| Layer | Scope | Persistence root | Reader tools |
|---|---|---|---|
| **Content teleology** | Cross-embedder synergy / fusion / topic profile / meaning extraction over content `TeleologicalFingerprint` rows | `CF_FINGERPRINTS`, `CF_TOPIC_PROFILES`, `CF_TOPIC_PORTFOLIO` | `search_graph`, `get_topic_portfolio`, `get_topic_stability`, `compile_constellation`, `score_against_constellation` |
| **Learner-state UTL (E15–E21)** | Per-learner observation envelopes, state vectors, goal centroids, retrieval logs, audits | `CF_LEARNER_PROFILE`, `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_STATE_HISTORY`, `CF_GOAL_CENTROIDS`, `CF_LEARNER_GOAL_STATES`, `CF_LEARNER_CONSTELLATIONS`, ... | `register_learner`, `record_session_observation`, `upsert_goal_centroid`, `get_goal_distance`, `count_learner_state`, `get_learner_state`, `compile_learner_constellation`, `resolve_learner_retrieval_policy` |

The **content layer** is described in detail across [05](05_embedding_subsystem.md), [06](06_search_and_fusion.md), [12](12_clustering_and_topics.md), [17](17_constellation_subsystem.md). This document concentrates on the **learner UTL layer**, the goal-distance machinery, and anomaly derivation — the parts that turn the system from a passive store into a goal-oriented memory with feedback into training.

## 2. Constants

### 2.1 Content layer (`teleological/types.rs`, `teleological/synergy_matrix.rs`)

| Constant | Value | File |
|---|---|---|
| `NUM_EMBEDDERS` | 14 | `types.rs:217` |
| `EMBEDDING_DIM` (canonical fusion-tensor depth) | 1024 | `types.rs:214` |
| `SYNERGY_DIM` | 14 | `synergy_matrix.rs` (one row/col per embedder) |
| `CROSS_CORRELATION_COUNT` | 91 | `synergy_matrix.rs` (= C(14, 2)) |
| Tucker default ranks | (4, 4, 128) | `types.rs:94` (`TuckerCore::DEFAULT_RANKS`) |
| `MAX_WEIGHTED_AGREEMENT` | 9.5 | `graph_link_tools.rs:1419-1434` (sum of topic weights) |

### 2.2 Learner layer (`learner/mod.rs`)

| Constant | Value | File |
|---|---|---|
| `LEARNER_RECORD_VERSION` | 1 | `mod.rs:17` |
| `LEARNER_MODALITY_COUNT` | 7 | `mod.rs:20` |
| `LEARNER_BASELINE_SELECTOR_REGULATED` | 1 | `mod.rs:23` |
| `MAX_LEARNER_STATE_VECTOR_VALUES` | 16_384 | `mod.rs:26` |
| `MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION` | 16 | `mod.rs:29` |
| `MAX_RECENT_SCORE_WINDOW` | 1024 | `mod.rs:32` |
| `MAX_LEARNER_LABEL_CHARS` | 256 | `mod.rs:35` |

## 3. Learner modalities (E15–E21)

`LearnerModality` (`learner/mod.rs:39-80`) maps to embedder slots E15–E21 in the planning sense (no on-disk per-embedder CFs yet):

| Variant | Slot | String form |
|---|---|---|
| `AffectSpeech` | E15 | `affect_speech` |
| `AffectFace` | E16 | `affect_face` |
| `AffectText` | E17 | `affect_text` |
| `Ppg` | E18 | `ppg` |
| `Eda` | E19 | `eda` |
| `Eeg` | E20 | `eeg` |
| `EegArtifactRobust` | (E20-prime) | `eeg_artifact_robust` |
| `SelfReport` | external | `self_report` (rejected as a centroid modality, see §6) |

`LearnerModality::parse(&str)` is the canonical string→enum entry-point used by every UTL tool.

## 4. Core learner records

All persisted via `RocksDbTeleologicalStore` (`storage/teleological/rocksdb_store/learner.rs`). Every record has a `validate()` returning `CoreError::ValidationError`.

| Type | Fields (selected) | CF | Key | File |
|---|---|---|---|---|
| `ObservationEnvelope` | `observation_id`, `learner_id`, `session_ts`, `modality`, `consent_state`, `raw_sha256`, `preprocessing_version`, `embedder_version`, `threshold_version`, `parent_observation_ids`, `created_at` | (in-fingerprint) | — | `learner/mod.rs:84-140` |
| `ComputationEnvelope` | `computation_id`, `learner_id`, `session_ts`, `parent_observation_ids`, `threshold_version`, `output_sha256`, `created_at` | (in-fingerprint) | — | `learner/mod.rs:144-181` |
| `LearnerProfile` | `learner_id`, `handle`, `consent_state`, `modalities_enabled`, `calibration_session_ts`, `created_at` | `learner_profile` | learner_id | `learner/mod.rs:185-225` |
| `ModalityEmbedding` | `modality`, `vector`, `scalar`, `source_observation_id` | (in-fingerprint) | — | `learner/mod.rs:229-244` |
| `LearnerStateComponents` | `plasticity_window`, `hrv_coherence`, `valence`, `arousal`, `stress_floor`, `k_sleep` (range [0, 5]) | (embedded) | — | `learner/mod.rs:248-272` |
| `LearnerStateVector` | `learner_id`, `session_ts`, `values`, `components`, `context: BTreeMap<String, String>` | `learner_state_history` | `[learner_id:16][session_ts:u64 BE]` | `learner/mod.rs:276-294` |
| `LearnerFingerprint` | `learner_id`, `session_ts`, `observation_envelopes`, `modality_embeddings`, `state_vector` | `fingerprints_learner` | same composite | `learner/mod.rs:298-343` |
| `LearnerDeltaLog` | per-session ΔS / ΔC / ΔE breakdown rows | `learner_delta_log` | `[learner_id:16][session_ts:u64 BE]` | `learner/mod.rs` (DeltaLog block) |
| `LearnerMTrace` | per-trace M(L) memory trace | `learner_m_per_trace` | `[learner_id:16][trace_id:16]` | `learner/mod.rs` |
| `LearnerKSleep` | recovery / sleep-pressure scalar history | `learner_k_sleep` | `[learner_id:16][session_ts:u64 BE]` | `learner/mod.rs` |
| `LearnerRetrievalLog` | per-trace retrieval outcomes (correct, score, state vector) | `learner_retrieval_log` | `[learner_id:16][trace_id:16][ts:u64 BE]` | `learner/mod.rs` |
| `LearnerGoalState` | `learner_id`, `skill_id`, `state_vector` | `learner_goal_states` | `[learner_id:16][skill_id:16]` | `learner/mod.rs:603-615` |
| `GoalCentroid` | `skill_id`, `modality`, `vector` (per-skill expert-answer centroid) | `goal_centroids` | `[skill_id:16][modality:u8]` | `learner/mod.rs:712-724` |
| `LearnerModalityCentroid` | `modality`, `vector`, `scalar_mean`, `sample_count` | (in-constellation) | — | `learner/mod.rs:618-640` |
| `LearnerConstellation` | regulated-state baseline; `selector_kind`, modality centroids, `state_centroid`, `session_ts_start/end` | `learner_constellations` | `[learner_id:16][selector_kind:u8]` | `learner/mod.rs:643-710`; cross-ref [17](17_constellation_subsystem.md) |
| `LearnerAuditEntry` | `audit_id`, `learner_id`, `ts`, `action`, `target_cf`, `result_sha256`, `parent_audit_id` | `learner_audit` | `[learner_id:16][ts:u64 BE][audit_id:16]` | `learner/mod.rs:727-745` |
| `LearnerTrainingDataset` | row-major f32 matrix exported per learner | `learner_training_datasets` | dataset_id | see [20](20_learning_subsystem.md) |

All learner CF row keys preserve `[learner_id]` as a 16-byte prefix so prefix scans yield single-learner views. `LearnerStateVector::values` is the freeform learner-state vector (capacity capped at 16 384 f32) used for goal-distance computation.

## 5. Goal centroids and goal-distance

`GoalCentroid` (`learner/mod.rs:714-724`) is a per-skill expert-answer centroid stored in `CF_GOAL_CENTROIDS` keyed by `[skill_id:16][modality:u8]`. It is **per-modality** so the same skill can hold different centroids for, e.g., affect-text vs. EEG.

### 5.1 `upsert_goal_centroid`

Handler: `utl_tools.rs:1191`. Args (`GoalCentroidArgs`, `utl_tools.rs:232`):

| Field | JSON key | Type | Required |
|---|---|---|---|
| skill_id | `skillId` | UUID | yes |
| modality | `modality` | string (one of E15–E21 strings) | yes |
| vector | `vector` | f32[] | yes |

Validation: `GoalCentroid::validate` runs `validate_vector` (non-empty, finite, length-bounded). Persists via `RocksDbTeleologicalStore::store_goal_centroid` (`learner.rs:236`). `count_goal_centroids` (`learner.rs:299`) is exposed via `get_capability_matrix.runtime.learnerState.goalCentroids`.

### 5.2 `get_goal_distance`

Handler: `utl_tools.rs:1233`. Args (`GoalDistanceArgs`, `utl_tools.rs:240`):

| Field | JSON key | Type | Required | Default |
|---|---|---|---|---|
| learner_id | `learnerId` | UUID | yes | — |
| skill_id | `skillId` | UUID | yes | — |
| session_ts | `sessionTs` | u64 | yes | — |
| modality | `modality` | string | yes | — |
| persist_goal_state | `persistGoalState` | bool | no | `false` |

Algorithm (`utl_tools.rs:1248-1322`):
1. Load `GoalCentroid` for `(skill_id, modality)` → 404 if missing.
2. Load `LearnerFingerprint` for `(learner_id, session_ts)` → 404 if missing.
3. Locate the `ModalityEmbedding` matching `modality` inside the fingerprint → error if absent.
4. Validate dimensions match.
5. Compute three metrics:

   | Metric | Formula | Source |
   |---|---|---|
   | `cosine_similarity` | `dot / (|a|·|b|)` | `cosine_similarity(...)` helper |
   | `cosine_distance` | `1 - cosine_similarity` | derived |
   | `euclidean_distance` | `sqrt(Σ(a_i - b_i)²)` | helper |
   | `transfer_t_lambda_1` | UTL transfer T at λ=1.0 | `compute_transfer_t(emb, centroid, 1.0)` |

6. If `persistGoalState=true`: write `LearnerGoalState { learner_id, skill_id, state_vector: fingerprint.state_vector }` to `CF_LEARNER_GOAL_STATES` and append a `LearnerAuditEntry` with `action="get_goal_distance"`, `target_cf="learner_goal_states"` (`utl_tools.rs:1286-1308`).

The `source_of_truth` field on the response is the canonical list `["goal_centroids", "fingerprints_learner", "learner_goal_states", "learner_audit"]`.

## 6. UTL tool catalogue (17 tools)

`crates/context-graph-mcp/src/tools/definitions/utl.rs` registers exactly 17 learner-state tool definitions (`utl.rs:6-7`). Names below are the canonical MCP names; handlers live in `handlers/tools/utl_tools.rs`.

| Tool | Args (camelCase) | Persists to / reads | Notes |
|---|---|---|---|
| `register_learner` | `learnerId?`, `handle`, `consentState`, `modalitiesEnabled`, `calibrationSessionTs?` | `learner_profile`, `learner_audit` | rejects unknown modality strings; emits audit row |
| `record_session_observation` | `learnerId`, `sessionTs`, `signals?` or `components{plasticityWindow, hrvCoherence, valence, arousal, stressFloor, kSleep}`, `modalityEmbeddings`, `mode`, `consentState`, … | `fingerprints_learner`, `learner_state_history`, `learner_audit` | refuses writes when `consentState=revoked`; supports `mode ∈ {signals, components, both}` |
| `compute_delta_s` | predicted/actual/simulated text, `explorationRate`, `gamma?` | (pure / optional persist via `record_*`) | bag-of-words cosine proxy for ΔS |
| `compute_delta_c` | learner_id, session window | `learner_delta_log` | ΔC retention/recall metric |
| `compute_delta_e` | learner_id, session window | `learner_delta_log` | ΔE engagement metric |
| `compute_L` | learner_id, trace_id | (pure) | M(L) computation result |
| `get_learner_M` | learner_id, trace_id | reads `learner_m_per_trace` | M trace lookup |
| `next_review_for_trace` | learner_id, trace_id | reads `learner_m_per_trace` + recent retrievals | spaced-repetition next-review timestamp |
| `list_learner_embedders` | none | runtime model registry | lists E15–E21 specs from `learner_embedder_specs` |
| `preflight_learner_assets` | `modelsRoot`, `calibrationRoot` | filesystem | reports model/calibration availability per modality |
| `count_learner_state` | none | per-CF counts | aggregates `count_learner_*` from `learner.rs:263-307` |
| `get_learner_state` | `learnerId`, `sessionTs` | reads `learner_state_history` | returns `LearnerStateVector` or 404 |
| `record_learner_k_sleep` | `learnerId`, `sessionTs`, value | `learner_k_sleep`, `learner_audit` | sleep-pressure scalar |
| `record_learner_retrieval` | `learnerId`, `traceId`, `ts`, `correct`, `score`, `stateVector?`, `components?`, `sessionTs?` | `learner_retrieval_log`, `learner_audit` | feeds ΔE / spaced-repetition |
| `upsert_goal_centroid` | `skillId`, `modality`, `vector` | `goal_centroids` | see §5.1 |
| `get_goal_distance` | `learnerId`, `skillId`, `sessionTs`, `modality`, `persistGoalState?` | `goal_centroids`, `fingerprints_learner`, optionally `learner_goal_states`, `learner_audit` | see §5.2 |
| `compile_learner_constellation` | `learnerId`, `sessionTsList[]`, `selectorKind?`, `label?` | `learner_constellations` | aggregates per-modality centroids; see [17](17_constellation_subsystem.md) §7 |
| `resolve_learner_retrieval_policy` | `learnerId`, `sessionTs`, `baseWeightProfile?`, `includeWeights?` | reads multiple learner CFs | returns a per-modality retrieval-weight vector |

Plus the cross-cutting `get_capability_matrix` (in `capability_tools.rs`) which exposes per-CF live counts for the entire learner subsystem.

### 6.1 `get_capability_matrix.runtime.learnerState`

Live counters (`capability_tools.rs:146-158`) exposing the size of every learner CF:

```
learnerProfile, learnerConstellations, fingerprintsLearner,
learnerMPerTrace, learnerStateHistory, learnerGoalStates,
learnerRetrievalLog, learnerKSleep, goalCentroids,
learnerDeltaLog, learnerAudit
```

Each value comes from a `count_learner_*` method on `RocksDbTeleologicalStore` (`learner.rs:263-307`). This is the **only** sanctioned way to size the learner store at runtime — it is used as the source-of-truth probe for `record_session_observation` workflows.

## 7. Anomaly derivation from edges

`derive_anomalies_from_edges` (handler `typed_edges_tools.rs:338`, store `anomaly_derivation.rs`) is the bridge from the **typed-edge graph** ([07](07_graph_and_causal.md)) into **contrastive training corpora** ([18](18_contrastive_and_typed_edges.md)). It belongs here because the classifier is a pure function over the teleological signature attached to each edge.

### 7.1 `AnomalyDerivationConfig` defaults

Mirrored from the offline contrastive-pair miner so both pipelines stay comparable (`anomaly_derivation.rs:31-55`).

| Field | Default |
|---|---|
| `high_threshold` | 0.60 |
| `low_threshold` | 0.30 |
| `min_disagreement` | 0.30 |
| `max_pairs` | 10_000 |

### 7.2 `classify_edge_as_anomaly` rules

Pure function over `TypedEdge.embedder_scores` (`anomaly_derivation.rs:203-279`). Embedder index map: E1=0, E5=4, E6=5, E7=6, E8=7, E9=8, E10=9, E11=10, E13=12.

| `edge_type` | Pattern | Resulting `AnomalyKind` | High slots | Low slots |
|---|---|---|---|---|
| `SemanticSimilar` | E1 high, E5 low | `SemanticButNotCausal` | [0] | [4] |
| `KeywordOverlap` | (E6 ∨ E13) high, E10 low | `KeywordButNotParaphrase` | [5] or [12] | [9] |
| `CodeRelated` | E7 high, E1 low | `CodeShapeButDifferentIntent` | [6] | [0] |
| `EntityShared` | E11 high, E8 low | `EntitySharedButDifferentStructure` | [10] | [7] |
| (any other type) | E9 high, E1 low | `HdcRobustButSemanticDifferent` (cross-type fallback) | [8] | [0] |

Rows below `min_disagreement` are dropped. Source/target with missing content are skipped (so anchors and negatives always have full text). Surviving rows are written via the standard `store_contrastive_pair` write path with `generator = "typed_edge_anomaly_derivation_v1"` — the secondary indexes (`CF_CONTRASTIVE_BY_KIND`, `CF_CONTRASTIVE_BY_ANCHOR`) stay in sync.

### 7.3 `AnomalyDerivationSummary`

Returned to the MCP client (`anomaly_derivation.rs:60-77`):

| Field | Meaning |
|---|---|
| `edges_scanned` | Number of `CF_TYPED_EDGES` rows walked |
| `pairs_written` | Pairs persisted to `CF_CONTRASTIVE_PAIRS` |
| `skipped_below_threshold` | Classified but dropped on `min_disagreement` |
| `skipped_missing_content` | Classified but dropped due to missing source/target text |
| `per_kind_counts` | `HashMap<AnomalyKind, usize>` |
| `duration_ms` | Wall-clock |

Idempotent on `(anchor, negative)` — re-derivation overwrites prior rows.

## 8. Content-layer fingerprint shape (recap)

Detailed in [05](05_embedding_subsystem.md). For completeness, the records this layer ingests:

| Type | Fields (selected) | Notes |
|---|---|---|
| `SemanticFingerprint` | 14 production embedding slots (E1–E14, dual cause/effect for E5, dual source/target for E8, dual paraphrase/context for E10) | shape constants live in `types/fingerprint/mod.rs`; `NUM_EMBEDDERS = 14` |
| `TeleologicalFingerprint` | `id`, `semantic`, `content_hash`, `created_at`, `last_accessed_at`, `access_count`, `importance` (clamped [0, 1]), optional `e6_sparse` | `record_access` mutates timestamps + count; `with_importance` clamps |
| `TopicProfile` | `[f32; 14]` alignments | rendered into the 13-entry compact vector by `teleological_vector` (see §10) |
| `Constellation` | per-embedder centroid + spread for a member set | see [17](17_constellation_subsystem.md) |

## 9. Synergy / fusion / meaning machinery

| Component | File | Purpose |
|---|---|---|
| `SynergyMatrix` | `teleological/synergy_matrix.rs` | 14×14 cross-embedding synergy weights (`SYNERGY_DIM=14`); `with_base_synergies()` is the canonical instantiation used by every constellation compile |
| `GroupAlignments` / `GroupType` | `teleological/groups.rs:266+` | 6D (Factual, Temporal, Causal, Relational, Qualitative, Implementation); `dominant_group` / `weakest_group` over alignments |
| `MultiResolutionHierarchy` | `teleological/resolution.rs` | 14D → 6D group → 3D core (What/How/Why) → 1D scalar; `quick_score()` = level-3 |
| `TeleologicalVector` | `teleological/vector.rs` | Fused per-task representation; constructed from synergy + topic profile |
| `MeaningExtractionConfig`, `ExtractedMeaning`, `FusionMethod`, `NuanceDimension` | `teleological/meaning.rs` | Configuration for `services/meaning_pipeline.rs` |
| `TaskType` (8 variants) | `teleological/profile/task_type.rs:19-78` | `CodeSearch`, `SemanticSearch`, `TemporalSearch`, `CausalSearch`, `FactualSearch`, `SocialSearch`, `AbstractSearch`, `General` — each lists its primary embedder indices |
| `FusionStrategy` | `teleological/profile/fusion_strategy.rs` | enumerated fusion strategies |
| `TeleologicalProfile` | `teleological/profile/teleological.rs` | Task-specific weight + fusion config |
| `MatrixSearchConfig`, `SearchStrategy`, `SimilarityBreakdown`, `ComprehensiveComparison` | `teleological/matrix_search.rs` | Cross-correlation search across all 14 embedders; `embedder_names()` is the canonical name table |
| `TucKerCompressor` (`CpuTuckerCompressor`) | `teleological/tucker.rs` | Streaming HOSVD compression to `TuckerCore`; default ranks (4, 4, 128) |
| `TeleologicalComparator` / `BatchComparator` / `ComparisonResult` / `ComparisonValidationError` | `teleological/comparator.rs`, `comparison_error.rs` | Validation + multi-fingerprint comparison entry-point |
| `services/*.rs` | — | One module per service: correlation extractor, fusion engine, group aggregator, meaning pipeline, multi-space retriever, profile manager, synergy service, tucker decomposer |

## 10. Compact 13-entry teleological vector

Although `NUM_EMBEDDERS = 14`, the historical `teleological_vector` field is **13D** because E14 (BGE-M3 Dense) is tracked separately in training records (the dense norm overlaps E1's information content). Layout:

| Idx | Source |
|---|---|
| 1–9 | Dense L2 norms of E1, E2, E3, E4, E5, E7, E8, E9, E10 |
| 10 | `e6_sparse_nnz / 1000` |
| 11 | E11 dense L2 norm |
| 12 | E12 average per-token norm |
| 13 | `e13_sparse_nnz / 1000` |

`TopicProfile.aggregate_alignment()` divides by `NUM_EMBEDDERS = 14`, so a topic-profile mean over the 14D alignment array maps back to the same scale as the 13-entry vector. Topic-confidence reporting uses `weighted_agreement / 9.5` (`MAX_WEIGHTED_AGREEMENT`).

## 11. Validation invariants

The learner layer is validation-heavy and FAIL FAST:

- `validate_vector(vec, field)` rejects empty, non-finite values, or length > `MAX_LEARNER_STATE_VECTOR_VALUES`.
- `validate_unit(value, field)` enforces `[0, 1]`.
- `validate_signed_unit(value, field)` enforces `[-1, 1]`.
- `validate_label(s, field)` enforces non-empty + length ≤ `MAX_LEARNER_LABEL_CHARS`.
- `validate_hash(s, field)` enforces lowercase hex of expected length.
- `LearnerProfile::validate` caps modalities at `LEARNER_MODALITY_COUNT + 1` (the +1 is `SelfReport`, allowed in profile but not as a centroid).
- `LearnerConstellation::validate` rejects `SelfReport` centroids and duplicate modalities.
- `LearnerStateComponents::validate` enforces `k_sleep` finite in [0, 5]; valence/arousal in [-1, 1]; everything else in [0, 1].
- All envelope fields (`raw_sha256`, `output_sha256`) must be SHA-256 hex.
- All store writes wrap mutations in a `RocksDB WriteBatch` for atomicity and emit a parent `LearnerAuditEntry` (lineage via `parent_audit_id`).

## 12. What is NOT covered here

- The 13×13 cross-correlation centroid math used in `Constellation` — see [17](17_constellation_subsystem.md).
- The compile / score / derive operations on constellations — see [17](17_constellation_subsystem.md).
- ΔS / ΔC / ΔE math, M-trace, and spaced-repetition formulas — see [20](20_learning_subsystem.md).
- CAMD-DPO consumption of contrastive pairs derived in §7 — see [19](19_camd_dpo_subsystem.md).
- The four search strategies (`e1_only`, `multi_space`, `pipeline`, `embedder_first`) and their weight profiles — see [06](06_search_and_fusion.md).
