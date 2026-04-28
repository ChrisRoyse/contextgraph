# Learning Subsystem

**Source files covered:**
- `crates/context-graph-core/src/learner/mod.rs`
- `crates/context-graph-core/src/learning/mod.rs`
- `crates/context-graph-storage/src/teleological/rocksdb_store/learner*.rs`
- `crates/context-graph-mcp/src/tools/definitions/learning.rs`
- `crates/context-graph-mcp/src/tools/definitions/utl.rs`
- `crates/context-graph-cli/src/commands/learning.rs`
- `crates/context-graph-cli/src/commands/utl.rs`

## 1. Scope
Learner/UTL records are separate from content fingerprints. The subsystem stores learner profiles, observations, state history, goal states, retrieval logs, K_sleep, delta logs, audits, M traces, learner constellations, and learner training datasets.

## 2. Constants
| Constant | Value |
| --- | --- |
| LEARNER_RECORD_VERSION | 1 |
| LEARNER_MODALITY_COUNT | 7 |
| MAX_LEARNER_STATE_VECTOR_VALUES | 16384 |
| MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION | 16 |
| MAX_RECENT_SCORE_WINDOW | 1024 |
| MAX_LEARNER_LABEL_CHARS | 256 |
| LEARNING_EVENT_VERSION | 1 |
| MAX_LEARNING_EVENT_MEMORY_IDS | 1024 |
| MAX_LEARNING_EVENT_TEXT_CHARS | 65536 |

## 3. Modalities
| Variant | Wire string |
| --- | --- |
| AffectSpeech | affect_speech |
| AffectFace | affect_face |
| AffectText | affect_text |
| Ppg | ppg |
| Eda | eda |
| Eeg | eeg |
| EegArtifactRobust | eeg_artifact_robust |
| SelfReport | self_report |
| Confused | confused |
| Optimal | optimal |
| Boring | boring |
| Stuck | stuck |
| Dysregulated | dysregulated |
| Dissipating | dissipating |

## 4. Data Models
| Model | Fields |
| --- | --- |
| ObservationEnvelope | observation_id, learner_id, session_ts, modality, consent/raw/version parents, created_at |
| ComputationEnvelope | computation_id, learner_id, session_ts, parents, threshold_version, output_sha256, created_at |
| LearnerProfile | learner_id, handle, consent_state, modalities_enabled, calibration_session_ts, created_at |
| ModalityEmbedding | modality, vector, scalar, source_observation_id |
| LearningStateSnapshot | 14D topic_profile, 91D cross_correlations, rank, scores, contradiction/integration/stability/domain/transfer |
| LearningFeatures | delta_e, rank shift, disagreement, surprise, coherence, consolidation, transfer, attribution |
| LearningSignal | signal id, vector, scalar, label, attribution |

## 5. Formulas
| Formula | Behavior |
| --- | --- |
| Delta S | bag-of-words cosine distances, gamma simulated distance, exploration boost, clamp [0,1] |
| Delta C | recent-score stability, gradient, HRV, panel, contradiction; recent scores capped at 1024 |
| Delta E | plasticity, valence/arousal, stress floor exponents; k_state from sqrt(plasticity)*k_sleep |
| L | DeltaS * DeltaC * DeltaE |
| M update | exponential decay plus L/deltaE/k_sleep contribution; retrieval correctness adjusts decay |
| Next review | target M=0.7 and log decay interval |

## 6. Tools
Learning tools: record/list/get/count events and export/list/get/count learner training datasets. UTL tools: register learner, record observation, compute deltas/L, read M/next review, list/preflight embedders, state reads, K_sleep/retrieval/goal/constellation operations.

## 7. Current Source Gap
`LEARNER_MODALITY_COUNT` is 7 while `LearnerModality` has 8 variants; validation permits `LEARNER_MODALITY_COUNT + 1` modalities.
