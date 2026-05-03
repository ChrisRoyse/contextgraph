# Learning Subsystem (Learning-as-UTL)

**Source files covered:**
- `crates/context-graph-core/src/learner/mod.rs` (1322 lines)
- `crates/context-graph-core/src/learning/mod.rs` (910 lines)
- `crates/context-graph-core/src/learner_training.rs` (413 lines)
- `crates/context-graph-storage/src/teleological/rocksdb_store/learner*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/learning_events*.rs`
- `crates/context-graph-mcp/src/handlers/tools/learning_tools.rs` (2004 lines)
- `crates/context-graph-mcp/src/handlers/tools/graph_learning_tools.rs` (1021 lines)
- `crates/context-graph-mcp/src/tools/definitions/learning.rs`
- `crates/context-graph-mcp/src/tools/definitions/utl.rs`
- `crates/context-graph-cli/src/commands/learning.rs`
- `crates/context-graph-cli/src/commands/utl.rs`

**Cross-links:** [13_teleological_subsystem.md](13_teleological_subsystem.md), [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md), [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md).

## 1. Scope
The Learning subsystem persists everything that describes **how a learner is changing over time** while content embedders E1-E14 capture **what is being remembered**. Two concentric layers:

1. **UTL (Universal Theory of Learning) state** in `crates/context-graph-core/src/learner/` — learner profiles, observations, state vectors, ΔS/ΔC/ΔE/L computations, M(t) consolidation traces, K_sleep, retrieval logs, goal centroids, audit, learner constellations.
2. **Learning-as-UTL events** in `crates/context-graph-core/src/learning/` — event-level before/after snapshots with five deterministic learning signals and a feature bundle that is embedded into matrix shards (CF_LEARNER_TRAINING_DATASETS).

Content embedders are E1-E14. Learner-state slots E15-E21 carry signal embeddings used for state estimation only — they never enter content fusion / RRF.

## 2. Constants
| Constant | Value | Source |
| --- | --- | --- |
| `LEARNER_RECORD_VERSION` | 1 | `learner/mod.rs:17` |
| `LEARNER_MODALITY_COUNT` | 7 | `learner/mod.rs:20` (validation permits +1 = 8 incl. `SelfReport`) |
| `LEARNER_BASELINE_SELECTOR_REGULATED` | 1 | `learner/mod.rs:23` |
| `MAX_LEARNER_STATE_VECTOR_VALUES` | 16384 | `learner/mod.rs:26` |
| `MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION` | 16 | `learner/mod.rs:29` |
| `MAX_RECENT_SCORE_WINDOW` | 1024 | `learner/mod.rs:32` |
| `MAX_LEARNER_LABEL_CHARS` | 256 | `learner/mod.rs:35` |
| `LEARNING_EVENT_VERSION` | 1 | `learning/mod.rs:18` |
| `MAX_LEARNING_EVENT_MEMORY_IDS` | 1024 | `learning/mod.rs:21` |
| `MAX_LEARNING_EVENT_TEXT_CHARS` | 65536 | `learning/mod.rs:24` |
| `MAX_LEARNING_EVENT_LABEL_CHARS` | 256 | `learning/mod.rs:27` |
| `LEARNER_TRAINING_DATASET_VERSION` | 1 | `learner_training.rs:20` |
| `MAX_LEARNER_TRAINING_ROWS` | 1_000_000 | `learner_training.rs:23` |
| `MAX_LEARNER_TRAINING_COLS` | 4096 | `learner_training.rs:27` |
| `NUM_EMBEDDERS` | 14 | `types/fingerprint.rs` |
| `NUM_CROSS_CORRELATIONS` | 91 (= C(14,2)) | `training/mod.rs` |

## 3. Modalities (`LearnerModality` — 8 variants)
| Variant | Wire string | Group | Embedder slot |
| --- | --- | --- | --- |
| `AffectSpeech` | `affect_speech` | affect | E16 (speech) |
| `AffectFace` | `affect_face` | affect | E15 (OpenFace 3.0 MTL) |
| `AffectText` | `affect_text` | affect | (text) |
| `Ppg` | `ppg` | physiological | E17 (PaPaGei) |
| `Eda` | `eda` | physiological | E18 (EDA model) |
| `Eeg` | `eeg` | physiological | E20 (EEG) |
| `EegArtifactRobust` | `eeg_artifact_robust` | physiological | E21 (motion-robust EEG) |
| `SelfReport` | `self_report` | self-report | (none — not an embedder) |

Note: `LearnerConstellation::validate` rejects `SelfReport` centroids (`learner/mod.rs:686`).

## 4. Diagnostic States (`LearnerDiagnosticState` — 6 variants)
Classification rules in `learner/mod.rs::classify_learner_state` (line 944):

| Variant | Wire string | Trigger |
| --- | --- | --- |
| `Dissipating` | `dissipating` | `L > 0.3` AND `consolidation_gain_ratio < 0.2` (decay-dominated) |
| `Dysregulated` | `dysregulated` | `L < 0.1` AND `recent_l_low_steps ≥ 3` AND `delta_e < 0.4` |
| `Stuck` | `stuck` | `L < 0.1` AND `recent_l_low_steps ≥ 3` (with `delta_e ≥ 0.4`) |
| `Confused` | `confused` | `L < 0.3` (otherwise) |
| `Optimal` | `optimal` | `0.3 ≤ L < 0.7` |
| `Boring` | `boring` | `L ≥ 0.7` |

`UtlComputation { delta_s, delta_c, delta_e, l, diagnostic_state }` is returned by `compute_L`.

## 5. Learner-State Embedders (E15-E21)
| Slot | Source | Modality | Purpose |
| --- | --- | --- | --- |
| E15 | OpenFace 3.0 MTL | `affect_face` | Facial action units / valence-arousal |
| E16 | speech embedder | `affect_speech` | Paralinguistic / prosody |
| E17 | PaPaGei (Zenodo) | `ppg` | PPG cardiac coherence |
| E18 | EDA model | `eda` | Galvanic skin response |
| E19 | WESAD stress head | (stress regulation aggregate) | Stress floor estimate |
| E20 | EEG model | `eeg` | EEG bands |
| E21 | EEG artifact-robust | `eeg_artifact_robust` | EEG (motion-robust) |

`preflight_learner_assets` verifies cached weights + calibration data exist on disk; fails closed unless `allowMissing=true`.

## 6. UTL Data Models (Rust)
| Model | File:line | Fields |
| --- | --- | --- |
| `ObservationEnvelope` | `learner/mod.rs:84` | observation_id, learner_id, session_ts, modality, consent / raw / preprocessing / embedder / threshold versions, parent_observation_ids, created_at |
| `ComputationEnvelope` | `learner/mod.rs:144` | computation_id, learner_id, session_ts, parent_observation_ids, threshold_version, output_sha256, created_at |
| `LearnerProfile` | `learner/mod.rs:185` | learner_id, handle, consent_state, modalities_enabled, calibration_session_ts, created_at |
| `ModalityEmbedding` | `learner/mod.rs:229` | modality, vector (f32), scalar, source_observation_id |
| `LearnerStateComponents` | `learner/mod.rs:248` | plasticity_window, hrv_coherence, valence (signed), arousal (signed), stress_floor, k_sleep |
| `LearnerStateVector` | `learner/mod.rs:276` | learner_id, session_ts, values (f32), components, context (BTreeMap) |
| `LearnerFingerprint` | `learner/mod.rs:298` | learner_id, session_ts, observation_envelopes, modality_embeddings, state_vector |
| `DeltaSBreakdown` | `learner/mod.rs:346` | predicted_actual_distance, predicted_simulated_distance, exploration_rate, gamma, delta_s |
| `DeltaCBreakdown` | `learner/mod.rs:372` | outcome_stability, coefficient_of_variation, gradient_effectiveness, hrv_coherence, panel_agreement, contradiction, delta_c |
| `DeltaEBreakdown` | `learner/mod.rs:404` | plasticity_window, hrv_coherence, valence, arousal, valence_arousal, stress_floor, k_sleep, k_state, delta_e |
| `UtlComputation` | `learner/mod.rs:467` | delta_s, delta_c, delta_e, l, diagnostic_state |
| `LearnerDeltaLog` | `learner/mod.rs:487` | learner_id, session_ts, computation, provenance |
| `LearnerMTrace` | `learner/mod.rs:512` | learner_id, trace_id, m_value (f64 [0,1]), last_update_ts, decay_rate (f32 [0,1]), num_retrievals, next_review_ts |
| `LearnerRetrievalLog` | `learner/mod.rs:565` | learner_id, trace_id, ts, correct, score [0,1], state_at_retrieval |
| `LearnerKSleep` | `learner/mod.rs:584` | learner_id, session_ts, k (f32 [0,5]), slow_wave_minutes (u16) |
| `LearnerGoalState` | `learner/mod.rs:605` | learner_id, skill_id, state_vector |
| `LearnerModalityCentroid` | `learner/mod.rs:619` | modality, vector, scalar_mean, sample_count |
| `LearnerConstellation` | `learner/mod.rs:644` | learner_id, selector_kind, label, sample_count, session_ts_start/end, modality_centroids, state_centroid, created_at |
| `GoalCentroid` | `learner/mod.rs:714` | skill_id, modality, vector |
| `LearnerAuditEntry` | `learner/mod.rs:728` | audit_id, learner_id, ts, action, target_cf, result_sha256, parent_audit_id |

## 7. Learning-Event Data Models
| Model | File:line | Fields |
| --- | --- | --- |
| `LearningStateSnapshot` | `learning/mod.rs:31` | 14D `topic_profile`, 91D `cross_correlations`, optional `retrieval_rank`, 14D `embedder_scores`, contradiction_pressure [0,1], integration_confidence [0,1], recurrence_count, stability_score [0,1], optional `domain`, successful_transfer_count |
| `LearningOutcomeLabel` | `learning/mod.rs:111` | enum `Useful` \| `Neutral` \| `Harmful` \| `NoLearning` |
| `LearningOutcome` | `learning/mod.rs:120` | label, utility_delta [-1,1], correction_required, reuse_observed |
| `LearningFeatures` | `learning/mod.rs:142` | 14D delta_e_vector, delta_e_scalar, retrieval_rank_shift, embedder_disagreement, surprise_score, productive_surprise, coherence_delta, contradiction_delta, consolidation_readiness, transfer_score, multi_utl_score, 14D attribution |
| `LearningSignalId` | `learning/mod.rs:178` | enum `DeltaE`(14) \| `Surprise`(3) \| `Coherence`(3) \| `Consolidation`(3) \| `Transfer`(3) |
| `LearningSignal` | `learning/mod.rs:210` | signal_id, vector, scalar [-1,1], optional label, attribution |
| `LearningEvent` | `learning/mod.rs:254` | event_id, created_at, memory_ids, session_id, response_id, task_id, query, retrieved_context, assistant_response, before, after, outcome, features, signals |

## 8. Learning Signals (Five Families)
Computed by `compute_baseline_learning_signals` (`learning/mod.rs:438`). Trait `LearningSignalEmbedder` returns one `LearningSignal` per family per event. `DeterministicLearningSignalEmbedder` is the in-tree implementation (no LLM, no neural model).

| Signal | Vector dim | Scalar | Label set |
| --- | --- | --- | --- |
| `delta_e` | 14 | mean of `after.topic_profile - before.topic_profile`, clamp [-1,1] | `positive_delta` / `negative_delta` / `no_delta` |
| `surprise` | 3: [`retrieval_rank_shift`, `embedder_disagreement`, `surprise_score`] | `surprise_score` (0.5·max(rank_shift,0) + 0.5·disagreement, clamp [0,1]) | `productive_surprise` (≥0.15 ∧ utility ≥ 0) / `low_or_noisy_surprise` |
| `coherence` | 3: [`coherence_delta`, `after.contradiction_pressure`, `after.integration_confidence`] | `coherence_delta` = (`Δintegration` − max(`Δcontradiction`, 0)), clamp [-1,1] | `coherence_improved` / `coherence_reduced` |
| `consolidation` | 3: [`min(recurrence,10)/10`, `after.stability_score`, `consolidation_readiness`] | `consolidation_readiness` = 0.5·recurrence + 0.5·stability | `ready` (≥0.7) / `not_ready` |
| `transfer` | 3: [`cross_domain ? 1 : 0`, `count_delta`, `transfer_score`] | `transfer_score` = 0.5·max(utility,0) + 0.5·count_delta when domains differ, else 0 | `positive_transfer` / `no_transfer` |

`compute_attribution` (`learning/mod.rs:590`): if `Σ |Δtopic| > 0`, attribution = normalized abs Δ; else fallback to normalized embedder_scores.

`compute_multi_utl_score` (`learning/mod.rs:612`): combines per-embedder semantic deltas + coherence delta via `MultiUtlParams::compute()` (lambda_s = lambda_c = 1, w_e = 1, phi = 0).

## 9. UTL Formulas (Source-Coded)
| Formula | File:line | Behavior |
| --- | --- | --- |
| ΔS (text) | `learner/mod.rs:748` `compute_delta_s_from_text` | Bag-of-words cosine distance; γ-weighted simulated distance; exploration boost (`+ 0.5·exploration_rate`); clamp [0,1] |
| ΔS (distances) | `learner/mod.rs:771` | `(max(actual, γ·simulated) + 0.5·exploration_rate).clamp(0,1)` |
| ΔC | `learner/mod.rs:803` | `outcome_stability + 0.2·gradient_eff + 0.2·hrv + 0.2·panel_agreement − 0.3·contradiction` clamp [0,1]; window capped at 1024; CV stability = `1/(1+cv)`; gradient = slope · scale (default 4.0) |
| ΔE | `learner/mod.rs:881` | `arousal_01 = (arousal+1)/2`; `inverted_u = 4·a·(1-a)`; `valence_sigmoid = 1/(1+exp(-3·valence))`; `valence_arousal = vs · iu`; `delta_e = plasticity^0.4 · va^0.4 · stress_floor^0.2`; `k_state = sqrt(plasticity) · k_sleep` |
| L | `learner/mod.rs:908` | `delta_s · delta_c · delta_e` clamp [0,1] |
| M(t) update | `learner/mod.rs:967` `update_m_trace` | `retained = previous_m · exp(-elapsed · decay_rate)`; `contribution = L · ΔE · k_sleep`; `m = (retained + contribution).clamp(0,1)`; on retrieval correct: `decay × 0.5`; incorrect: `decay / 0.7` capped at 1.0 |
| Next review | `learner/mod.rs:1039` `next_review_timestamp` | Solve `m·exp(-Δt·decay) = 0.7`; schedule at `now + 0.9·Δt` |
| Transfer T | `learner/mod.rs:1008` `compute_transfer_t` | `exp(-distance / lambda)` clamp [0,1]; lambda must be `>0` |

## 10. RocksDB Layout (key formats)
| CF | Key | Value (bincode) |
| --- | --- | --- |
| `CF_LEARNER_PROFILE` | `[learner_id:16]` | `[ver:u8][LearnerProfile]` |
| `CF_FINGERPRINTS_LEARNER` | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerFingerprint]` |
| `CF_LEARNER_STATE_HISTORY` | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerStateVector]` |
| `CF_LEARNER_M_PER_TRACE` | `[learner_id:16][trace_id:16]` | `[ver][LearnerMTrace]` |
| `CF_LEARNER_GOAL_STATES` | `[learner_id:16][skill_id:16]` | `[ver][LearnerGoalState]` |
| `CF_LEARNER_RETRIEVAL_LOG` | `[learner_id:16][trace_id:16][ts:u64 BE]` | `[ver][LearnerRetrievalLog]` |
| `CF_LEARNER_K_SLEEP` | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerKSleep]` |
| `CF_GOAL_CENTROIDS` | `[skill_id:16][modality:u8]` | `[ver][GoalCentroid]` |
| `CF_LEARNER_DELTA_LOG` | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerDeltaLog]` |
| `CF_LEARNER_AUDIT` | `[learner_id:16][ts:u64 BE][audit_id:16]` | `[ver][LearnerAuditEntry]` |
| `CF_LEARNER_CONSTELLATIONS` | `[learner_id:16][selector_kind:u8]` | `[ver][LearnerConstellation]` |
| `CF_LEARNER_TRAINING_DATASETS` | `[dataset_id:16]` | `[ver][LearnerTrainingDataset]` |
| `CF_LEARNING_EVENTS` | `[event_id:16]` | `[ver:u8][LearningEvent]` |

## 11. Learner Training Datasets (`learner_training.rs`)
Matrix shards compiled from learner + Learning-as-UTL CFs into row-major `f32` tensors. Six task families:

| Task variant | Wire string |
| --- | --- |
| `RewardModel` | `reward_model` (default) |
| `Reranker` | `reranker` |
| `EmbedderContrastive` | `embedder_contrastive` |
| `DiagnosticClassifier` | `diagnostic_classifier` |
| `Scheduler` | `scheduler` |
| `PersonalPhysiology` | `personal_physiology` |

`learning_event_feature_schema()` (`learner_training.rs:255`) returns a deterministic feature schema for `LearningEvent` rows:

| Block | Columns | Count |
| --- | --- | --- |
| `before_topic_e1..e14` | 14 | before topic profile |
| `before_cross_correlation_00..90` | 91 | before correlations |
| `before_embedder_score_e1..e14` | 14 | before embedder scores |
| `delta_topic_e1..e14` | 14 | after − before topic |
| Scalar block | 16 | rank norms, contradiction, integration, stability, surprise, coherence_delta, contradiction_delta, consolidation_readiness, transfer_score, multi_utl_score, embedder_disagreement, retrieval_rank_shift |
| **Total** | **149** | |

`learning_event_label_schema()` returns 4 labels: `utility_delta`, `outcome_label`, `correction_required`, `reuse_observed`.

`LearnerTrainingDataset` validates: `feature_schema` non-empty + ≤ `MAX_LEARNER_TRAINING_COLS`; `rows.len() ≤ MAX_LEARNER_TRAINING_ROWS`; `row_major.len() == rows × cols`; `row_major_sha256` recomputed at validate() time and compared; per-row `provenance_sha256` is 64-hex.

## 12. MCP Tools

### 12a. Learning Events (12 tools, `definitions/learning.rs`)
| Tool | Purpose |
| --- | --- |
| `record_learning_event` | Persist event to `CF_LEARNING_EVENTS`; recompute features + 5 baseline signals before write |
| `list_learning_events` | UUIDs + optional feature shape summaries |
| `get_learning_event` | Full event by UUID; toggles `includeText`, `includeSignals` |
| `count_learning_events` | Row count |
| `list_learning_signal_embedders` | Returns metadata for the 5 deterministic UTL signal embedders |
| `compute_learning_signals` | Compute signals from inline before/after/outcome (no storage mutation) |
| `embed_learning_event_signals` | Read persisted event, run signal embedders, verify match against persisted signals; reports before/after CF counts |
| `estimate_learning_outcome` | NN case-based reasoning over `CF_LEARNING_EVENTS`; weighted neighbor evidence; no mutation |
| `record_graph_learning_event` | Persist graph-navigation outcome (K-NN/typed-edge selection); reads real edges from `EdgeRepository`, rejects missing evidence; stored as `task_id = graph_learning:<policyScope>` |
| `resolve_graph_learning_policy` | Aggregate stored graph events into learned embedder weights + edge boosts; errors if evidence < `minEvidence` |
| `export_learner_training_dataset` | Compile UTL + learner-state rows to matrix shard in `CF_LEARNER_TRAINING_DATASETS` |
| `list_learner_training_datasets` | Dataset UUIDs + optional matrix shape |
| `get_learner_training_dataset` | Dataset by UUID; toggles `includeMatrix`, `previewRows ≤ 100` |
| `count_learner_training_datasets` | Row count |

### 12b. UTL (18 tools, `definitions/utl.rs`)
| Tool | Purpose |
| --- | --- |
| `register_learner` | Persist `LearnerProfile` to `CF_LEARNER_PROFILE` |
| `record_session_observation` | Persist `LearnerFingerprint` (envelopes + modality embeddings + state vector) |
| `compute_delta_s` | Compute ΔS from predicted/actual/simulated text + exploration rate |
| `compute_delta_c` | Compute ΔC from recent_scores window + HRV + panel agreement + contradiction |
| `compute_delta_e` | Compute ΔE from `LearnerStateComponents` or by reading a stored session |
| `compute_L` | Compute L = ΔS·ΔC·ΔE; optional `persist=true` writes `CF_LEARNER_DELTA_LOG` + updates `CF_LEARNER_M_PER_TRACE` |
| `get_learner_M` | Read `LearnerMTrace` |
| `next_review_for_trace` | Return next-review timestamp for a stored trace |
| `list_learner_embedders` | Canonical E1-E21 matrix (E1-E14 content + E15-E21 learner-state) |
| `preflight_learner_assets` | Inspect cached E15-E21 weights + calibration data; fails closed unless `allowMissing=true` |
| `count_learner_state` | Row counts for all learner-state CFs |
| `get_learner_state` | Read profile + session fingerprint + state vector + delta log + k_sleep + optional M trace |
| `record_learner_k_sleep` | Persist `LearnerKSleep` |
| `record_learner_retrieval` | Persist `LearnerRetrievalLog` (state from stored session OR explicit vector/components) |
| `upsert_goal_centroid` | Persist `GoalCentroid` for `(skill, modality)` |
| `get_goal_distance` | Cosine distance to centroid; optional `persistGoalState=true` |
| `compile_learner_constellation` | Compile regulated-state baseline from session fingerprints |
| `resolve_learner_retrieval_policy` | Read stored learner state; resolve content E1-E14 weight profile |

### 12c. Learning Signal Embedder Metadata
`list_learning_signal_embedders` returns:
```
{ "implementation": "DeterministicLearningSignalEmbedder",
  "count": 5,
  "embedders": [{ "signal_id": "delta_e"|"surprise"|"coherence"|"consolidation"|"transfer",
                  "dimension": 14|3, ... }] }
```

## 13. State-Conditioned Retrieval Policy
`resolve_learner_retrieval_policy` resolves a content weight profile from persisted learner state. **Does NOT inject E15-E21 into content fusion.** Decision tree on `LearnerStateComponents`:

| Condition | Selected profile |
| --- | --- |
| `stress_floor ≥ 0.80` OR (`stress_floor ≥ 0.60` AND `arousal ≥ 0.65`) | `affect_neutral` |
| `stress_floor ≥ 0.60` | `affect_repair` |
| `valence ≤ -0.30` | `affect_priming` |
| else | `baseWeightProfile` (default `semantic_search`) |

Allowed `baseWeightProfile` enum (20 values): `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`, `multilingual_search`, `long_context`, `translation_finder`, `affect_repair`, `affect_priming`, `affect_neutral`. Fail-fast on unknown base profile or invalid state.

## 14. Graph-Learning Policy (`graph_learning_tools.rs`)
| Constant | Value | Source |
| --- | --- | --- |
| `GRAPH_LEARNING_TASK_PREFIX` | `"graph_learning:"` | `graph_learning_tools.rs:28` |
| `default_graph_learning_max_scan` | 10000 | `graph_learning_tools.rs:65` |
| `default_graph_learning_rate` | 0.35 | `graph_learning_tools.rs:67` |

`record_graph_learning_event` requires `RocksDbTeleologicalStore` and a live `EdgeRepository`; it reads real K-NN/typed edges, rejects missing evidence, derives the learning state from observed edge scores, writes the event, and immediately reads the same RocksDB row back for full-state verification.

`resolve_graph_learning_policy` scans persisted events filtered by `policyScope` (and optionally `sourceMemoryId`), validates embedded graph evidence, aggregates into per-embedder weight deltas + edge-type boosts at `learningRate ∈ [0,1]`, and returns the learned policy. **Errors if matched evidence count < `minEvidence`** rather than falling back to the base policy.

Allowed `graphTool` enum: `get_unified_neighbors` (default), `get_memory_neighbors`, `traverse_graph`.

## 15. CLI
- `context-graph-cli learning {record|list|get|count|export-events-jsonl|export-dataset|...}` — read/write events, export learner training datasets, dump JSONL for `build_learner_dpo_pairs.py`.
- `context-graph-cli utl {preflight|register|record-observation|compute-l|...}` — learner-state ops.

`learning export-events-jsonl` is the input source for the Python learner-conditioned DPO miner — see [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md) §9.

## 16. Source Notes
- `LEARNER_MODALITY_COUNT = 7` (physiological + affect, excluding `SelfReport`); enum has 8 variants. `LearnerProfile::validate` permits `LEARNER_MODALITY_COUNT + 1 = 8` modalities. `LearnerConstellation` rejects more than 7 centroids and refuses `SelfReport`.
- E15-E21 are signal embedders. They back goal-distance computations and learner state vectors but never participate in content RRF / fusion.
- `LearningStateSnapshot.cross_correlations` requires exactly 91 values; `compute_learning_features` errors with `DimensionMismatch` otherwise.
- `LearningSignal.attribution` is always 14D `[0,1]`; computed from delta_e (preferred) or fallback embedder_scores.
- `LearnerTrainingDataset.row_major_sha256` is recomputed at every `validate()` call to guarantee the persisted hash matches the persisted payload.
- `update_m_trace` ground state: `decay_rate = 1/3600` (1-hour half-life baseline).
- `next_review_timestamp` returns `Some(now)` immediately when `m_value ≤ 0.7` (already at threshold).
- `LearnerStateComponents.k_sleep` is bounded `[0, 5]` and required-finite. `delta_e_breakdown.k_state` is `sqrt(plasticity) × k_sleep`, also bounded `[0, 5]`.
- Phase 6 added `compute_learning_signals` and `embed_learning_event_signals` for offline learner-conditioned DPO mining (see [19](19_camd_dpo_subsystem.md)).
- Learning events do not feed DynamicJEPA; they are independent provenance for content / LLM training. DynamicJEPA's "surprise" and "guard" pipeline is in [21](21_dynamicjepa_subsystem.md) and uses a different definition.
