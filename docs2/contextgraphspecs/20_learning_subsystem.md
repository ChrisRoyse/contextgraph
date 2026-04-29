# Learning Subsystem (Learning-as-UTL)

**Source files covered:**
- `crates/context-graph-core/src/learner/mod.rs`
- `crates/context-graph-core/src/learning/mod.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/learner*.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/learning_events*.rs`
- `crates/context-graph-mcp/src/tools/definitions/learning.rs`
- `crates/context-graph-mcp/src/tools/definitions/utl.rs`
- `crates/context-graph-cli/src/commands/learning.rs`
- `crates/context-graph-cli/src/commands/utl.rs`

## 1. Scope
Learner / UTL records are **separate from content fingerprints**. The subsystem stores: learner profiles, observations, state history, goal states, retrieval logs, K_sleep, delta logs, audits, M(t) traces, learner constellations, learner training datasets, and Learning-as-UTL events.

Content embedders are E1-E14 (always); learner-state slots E15-E21 carry signal embeddings used for state estimation only — they never enter content fusion.

## 2. Constants
| Constant | Value | Source |
| --- | --- | --- |
| LEARNER_RECORD_VERSION | 1 | learner/mod.rs |
| LEARNER_MODALITY_COUNT | 7 | learner/mod.rs (validation permits +1) |
| MAX_LEARNER_STATE_VECTOR_VALUES | 16384 | learner/mod.rs |
| MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION | 16 | learner/mod.rs |
| MAX_RECENT_SCORE_WINDOW | 1024 | learner/mod.rs |
| MAX_LEARNER_LABEL_CHARS | 256 | learner/mod.rs |
| LEARNING_EVENT_VERSION | 1 | learning/mod.rs |
| MAX_LEARNING_EVENT_MEMORY_IDS | 1024 | learning/mod.rs |
| MAX_LEARNING_EVENT_TEXT_CHARS | 65536 | learning/mod.rs |
| MAX_LEARNING_EVENT_LABEL_CHARS | 256 | learning/mod.rs |

## 3. Modalities (`LearnerModality` enum, 14 variants)
| Variant | Wire string | Group |
| --- | --- | --- |
| AffectSpeech | affect_speech | affect |
| AffectFace | affect_face | affect |
| AffectText | affect_text | affect |
| Ppg | ppg | physiological |
| Eda | eda | physiological |
| Eeg | eeg | physiological |
| EegArtifactRobust | eeg_artifact_robust | physiological |
| SelfReport | self_report | self-report |
| Confused | confused | regulated state label |
| Optimal | optimal | regulated state label |
| Boring | boring | regulated state label |
| Stuck | stuck | regulated state label |
| Dysregulated | dysregulated | regulated state label |
| Dissipating | dissipating | regulated state label |

## 4. Learner-State Embedders (E15-E21)
| Slot | Source | Purpose |
| --- | --- | --- |
| E15 | OpenFace 3.0 MTL | Affect / facial action units |
| E16 | speech embedder | Affect / paralinguistic |
| E17 | PaPaGei (Zenodo) | PPG |
| E18 | EDA model | Galvanic skin response |
| E19 | WESAD stress head | Stress regulation |
| E20 | EEG model | EEG |
| E21 | EEG artifact-robust | EEG (motion-robust) |

`preflight_learner_assets` verifies cached weights + calibration data; fails closed unless `allowMissing=true`.

## 5. Data Models
| Model | Fields |
| --- | --- |
| ObservationEnvelope | observation_id, learner_id, session_ts, modality, consent / raw / version parents, created_at |
| ComputationEnvelope | computation_id, learner_id, session_ts, parents, threshold_version, output_sha256, created_at |
| LearnerProfile | learner_id, handle, consent_state, modalities_enabled, calibration_session_ts, created_at |
| LearnerStateComponents | plasticity_window, hrv_coherence, valence, arousal, stress_floor, k_sleep |
| LearnerStateVector | per-modality vectors + scalars |
| ModalityEmbedding | modality, vector, scalar, source_observation_id |
| LearningStateSnapshot | 14D topic_profile, 91D cross_correlations, rank, scores, contradiction / integration / stability / domain / transfer |
| LearningFeatures | delta_e, rank shift, disagreement, surprise, coherence, consolidation, transfer, attribution |
| LearningSignal | signal id, vector, scalar, label, attribution |
| LearnerKSleep | sleep-gated consolidation multiplier |
| LearnerRetrievalLog | trace_id, ts, correct, score, state vector |
| GoalCentroid | per (skill, modality), expert vector |
| LearnerGoalState | distance to centroid, persisted snapshot |
| LearnerDeltaLog | ΔS, ΔC, ΔE, L per session |
| LearnerAuditEntry | audit_id, action, ts, operator |
| LearnerTrainingDataset | row-major f32 matrix (versioned) |
| LearningEvent | before/after snapshots, outcome label, signals, metadata |

## 6. Formulas
| Formula | Behavior |
| --- | --- |
| ΔS | Bag-of-words cosine distance(s); γ-weighted simulated distance; exploration boost; clamp [0, 1] |
| ΔC | Recent-score stability + gradient + HRV + panel agreement + contradiction; window capped at 1024 |
| ΔE | Plasticity, valence/arousal, stress_floor exponents; `k_state = sqrt(plasticity) · k_sleep` |
| L | `ΔS · ΔC · ΔE` |
| M(t) update | Exponential decay + `L / ΔE / k_sleep` contribution; retrieval correctness adjusts decay |
| Next review | Solves for log decay so M(t_next) = 0.7 |

## 7. RocksDB Layout (key formats)
| CF | Key | Value |
| --- | --- | --- |
| CF_LEARNER_PROFILE | `[learner_id:16]` | `[ver:u8][bincode LearnerProfile]` |
| CF_FINGERPRINTS_LEARNER | `[learner_id:16][session_ts:u64 BE]` | `[ver][bincode LearnerFingerprint]` |
| CF_LEARNER_STATE_HISTORY | `[learner_id:16][session_ts:u64 BE]` | `[ver][bincode LearnerStateVector]` |
| CF_LEARNER_M_PER_TRACE | `[learner_id:16][trace_id:16]` | `[ver][bincode LearnerMTrace]` |
| CF_LEARNER_GOAL_STATES | `[learner_id:16][skill_id:16]` | `[ver][bincode LearnerGoalState]` |
| CF_LEARNER_RETRIEVAL_LOG | `[learner_id:16][trace_id:16][ts:u64 BE]` | `[ver][bincode LearnerRetrievalLog]` |
| CF_LEARNER_K_SLEEP | `[learner_id:16][session_ts:u64 BE]` | `[ver][bincode LearnerKSleep]` |
| CF_GOAL_CENTROIDS | `[skill_id:16][modality:u8]` | `[ver][bincode GoalCentroid]` |
| CF_LEARNER_DELTA_LOG | `[learner_id:16][session_ts:u64 BE]` | `[ver][bincode LearnerDeltaLog]` |
| CF_LEARNER_AUDIT | `[learner_id:16][ts:u64 BE][audit_id:16]` | `[ver][bincode LearnerAuditEntry]` |
| CF_LEARNER_CONSTELLATIONS | `[learner_id:16][selector_kind:u8]` | `[ver][bincode]` |
| CF_LEARNER_TRAINING_DATASETS | `[dataset_id:16]` | `[ver][bincode LearnerTrainingDataset]` |
| CF_LEARNING_EVENTS | event UUID (16) | `[ver:u8][bincode LearningEvent]` |

## 8. MCP Tools
**Learning (12)**: record_learning_event, list_learning_events, get_learning_event, count_learning_events, list_learning_signal_embedders, compute_learning_signals, embed_learning_event_signals, estimate_learning_outcome, export_learner_training_dataset, list_learner_training_datasets, get_learner_training_dataset, count_learner_training_datasets.

**UTL (18)**: register_learner, record_session_observation, compute_delta_s, compute_delta_c, compute_delta_e, compute_L, get_learner_M, next_review_for_trace, list_learner_embedders, preflight_learner_assets, count_learner_state, get_learner_state, record_learner_k_sleep, record_learner_retrieval, upsert_goal_centroid, get_goal_distance, compile_learner_constellation, resolve_learner_retrieval_policy.

## 9. State-Conditioned Retrieval Policy
`resolve_learner_retrieval_policy` resolves a content weight profile from persisted learner state (does NOT inject E15-E21 into content fusion):

| Condition (LearnerStateComponents) | Selected profile |
| --- | --- |
| `stress_floor >= 0.80` OR (`stress_floor >= 0.60` AND `arousal >= 0.65`) | `affect_neutral` |
| `stress_floor >= 0.60` | `affect_repair` |
| `valence <= -0.30` | `affect_priming` |
| else | `base_profile` (default `semantic_search`) |

Fail-fast on unknown base profile or invalid state.

## 10. CLI
- `context-graph-cli learning {record|list|get|count|export-dataset|...}` — read/write events, export datasets.
- `context-graph-cli utl {preflight|register|record-observation|compute-l|...}` — learner-state ops.

## 11. Source Notes
- `LEARNER_MODALITY_COUNT = 7` (physiological + affect groups); enum has 14 variants total (includes regulated-state labels). Validation permits `LEARNER_MODALITY_COUNT + 1` modalities per profile.
- E15-E21 are signal embedders; they back goal-distance computations and state vectors but never participate in content RRF/fusion.
- Phase 6 added `compute_learning_signals` and `embed_learning_event_signals` for offline learner-conditioned DPO mining (see [19](19_camd_dpo_subsystem.md)).
