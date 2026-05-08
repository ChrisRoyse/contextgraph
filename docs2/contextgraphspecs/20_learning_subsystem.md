# Learning Subsystem (Learning-as-UTL)

**Source files covered:**
- `crates/context-graph-core/src/learner/mod.rs` (1322 lines — UTL primitives, ΔS/ΔC/ΔE/L, M(t), constellations, audit)
- `crates/context-graph-core/src/learning/mod.rs` (910 lines — learning events + 5 deterministic UTL signal embedders)
- `crates/context-graph-core/src/learner_training.rs` (413 lines — matrix-shard datasets compiled from learner CFs)
- `crates/context-graph-embeddings/src/learner/mod.rs` (E15-E21 specs, calibration manifest, asset preflight, deterministic Phase-0 extractors)
- `crates/context-graph-embeddings/fixtures/utl_calibration/{dataset_manifest.json,phase0_synthetic_fixture.json}` (E15-E21 dataset registry + offline FSV fixture)
- `crates/context-graph-storage/src/teleological/rocksdb_store/learner.rs` (701 lines), `learning.rs` (268 lines), `learner_training.rs` (164 lines) — CF persistence
- `crates/context-graph-storage/tests/{utl_learner_state_fsv,utl_learner_embedders_fsv}.rs` (full-state verification)
- `crates/context-graph-mcp/src/handlers/tools/utl_tools.rs` (~2238 lines)
- `crates/context-graph-mcp/src/handlers/tools/learning_tools.rs` (~2004 lines)
- `crates/context-graph-mcp/src/handlers/tools/graph_learning_tools.rs` (~1021 lines)
- `crates/context-graph-mcp/src/handlers/tools/capability_tools.rs` (`get_capability_matrix` reports E1-E14 + E15-E21)
- `crates/context-graph-mcp/src/tools/definitions/{learning,utl,capability}.rs`
- `crates/context-graph-cli/src/commands/{learning,utl}.rs`
- `crates/context-graph-cli/tests/learning_jsonl_export_integration.rs`
- `crates/context-graph-core/src/weights/mod.rs::select_state_conditioned_weight_profile` (state-conditioned policy selector)

**Cross-links:** [04_storage_layer.md](04_storage_layer.md), [05_embedding_subsystem.md](05_embedding_subsystem.md), [13_teleological_subsystem.md](13_teleological_subsystem.md), [16_training_subsystem.md](16_training_subsystem.md), [17_constellation_subsystem.md](17_constellation_subsystem.md), [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md), [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md), [21_dynamicjepa_subsystem.md](21_dynamicjepa_subsystem.md).

## 1. Scope

The Learning subsystem persists everything that describes **how a learner is changing over time** while content embedders E1-E14 capture **what is being remembered**. "UTL" stands for **Universal Theory of Learning**; "Learning-as-UTL" is the in-tree rendering of that theory as Rust types and deterministic computations. Two concentric layers:

1. **UTL learner state** in `crates/context-graph-core/src/learner/` — learner profiles, observations, state vectors, ΔS/ΔC/ΔE/L computations, M(t) consolidation traces, K_sleep, retrieval logs, goal centroids, audit, learner constellations.
2. **Learning-as-UTL events** in `crates/context-graph-core/src/learning/` — event-level before/after snapshots with five deterministic learning signals and a feature bundle that is embedded into matrix shards (`CF_LEARNER_TRAINING_DATASETS`).

Content embedders are E1-E14. Learner-state slots E15-E21 carry signal embeddings used for state estimation only — they never enter content fusion / RRF. The full planned namespace is `UTL_PLANNED_TOTAL_EMBEDDERS = 21`, with the actual learner namespace defined by `LEARNER_EMBEDDER_COUNT = 7` (E15-E21) (`crates/context-graph-embeddings/src/learner/mod.rs:25-31`). Content topology details are in [05_embedding_subsystem.md](05_embedding_subsystem.md).

## 2. Constants

| Constant | Value | Source |
| --- | --- | --- |
| `LEARNER_RECORD_VERSION` | 1 | `learner/mod.rs:17` |
| `LEARNER_MODALITY_COUNT` | 7 | `learner/mod.rs:20` (`LearnerProfile::validate` permits +1 = 8 incl. `SelfReport`; `LearnerConstellation::validate` rejects > 7 and refuses `SelfReport`) |
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
| `LEARNER_EMBEDDER_COUNT` | 7 | `embeddings/learner/mod.rs:25` |
| `UTL_PLANNED_TOTAL_EMBEDDERS` | 21 | `embeddings/learner/mod.rs:28` |
| `UTL_CONTENT_EMBEDDERS_BEFORE_LEARNER_STATE` | 14 | `embeddings/learner/mod.rs:31` |
| `NUM_EMBEDDERS` | 14 | `core/types/fingerprint.rs` |
| `NUM_CROSS_CORRELATIONS` | 91 (= C(14,2)) | `core/training/mod.rs` |

## 3. Modalities (`LearnerModality` — 8 variants, `learner/mod.rs:39-48`)

| Variant | Wire string | Group | Embedder slot |
| --- | --- | --- | --- |
| `AffectSpeech` | `affect_speech` | affect | E15 (Wav2Vec2-large-MSP-DIM) |
| `AffectFace` | `affect_face` | affect | E16 (OpenFace 3.0 + MTL emotion/AU/gaze) |
| `AffectText` | `affect_text` | affect | E17 (MiniLM-L6-v2 + EmoBank regressor) |
| `Ppg` | `ppg` | physiological | E18 (PaPaGei PPG foundation) |
| `Eda` | `eda` | physiological | E19 (WESAD chest physiology stress head) |
| `Eeg` | `eeg` | physiological | E20 (LaBraM EEG foundation) |
| `EegArtifactRobust` | `eeg_artifact_robust` | physiological | E21 (EEGPT artifact-robust EEG) |
| `SelfReport` | `self_report` | self-report | (none — not an embedder) |

Note: `LearnerConstellation::validate` rejects `SelfReport` centroids (`learner/mod.rs:686`).

## 4. Diagnostic States (`LearnerDiagnosticState` — 6 variants, `learner/mod.rs:443-450`)

Classification rules in `learner/mod.rs::classify_learner_state` (line 944):

| Variant | Wire string | Trigger |
| --- | --- | --- |
| `Dissipating` | `dissipating` | `L > 0.3` AND `consolidation_gain_ratio < 0.2` (decay-dominated) |
| `Dysregulated` | `dysregulated` | `L < 0.1` AND `recent_l_low_steps ≥ 3` AND `delta_e < 0.4` |
| `Stuck` | `stuck` | `L < 0.1` AND `recent_l_low_steps ≥ 3` (and `delta_e ≥ 0.4`) |
| `Confused` | `confused` | `L < 0.3` (otherwise) |
| `Optimal` | `optimal` | `0.3 ≤ L < 0.7` |
| `Boring` | `boring` | `L ≥ 0.7` |

`UtlComputation { delta_s, delta_c, delta_e, l, diagnostic_state }` is the return type of `compute_utl_l` (`learner/mod.rs:908`).

## 5. Learner-State Embedders (E15-E21)

Authoritative spec: `crates/context-graph-embeddings/src/learner/mod.rs::LearnerEmbedderSlot` and `learner_embedder_specs()` (lines 33-192).

| Slot | Source | Modality | Output dim | Scalar heads |
| --- | --- | --- | --- | --- |
| E15 | Wav2Vec2-large-MSP-DIM | `affect_speech` | 1024 | `arousal`, `dominance`, `valence` |
| E16 | OpenFace 3.0 + MTL emotion/AU/gaze head | `affect_face` | 512 | `valence`, `arousal`, `au_intensity` |
| E17 | MiniLM-L6-v2 + EmoBank regressor | `affect_text` | 384 | `valence`, `arousal` |
| E18 | PaPaGei PPG foundation model | `ppg` | 512 | `hrv_coherence` |
| E19 | WESAD chest physiology stress head | `eda` | 64 | `stress_floor` |
| E20 | LaBraM EEG foundation model | `eeg` | 768 | `plasticity_window` |
| E21 | EEGPT artifact-robust EEG model | `eeg_artifact_robust` | 768 | `plasticity_window` |

Note: the capability_matrix MCP tool (`capability_tools.rs:236-243`) lists slots in the same E15-E21 numerical order with shorter labels (`speech_affect`, `face_affect`, `text_affect`, `ppg_hrv`, `eda_stress`, `eeg_plasticity`, `eeg_robust`).

`preflight_learner_assets` (MCP) verifies cached weights + calibration data exist on disk; fails closed unless `allowMissing=true`. Required file groups per slot (constants `REQ_E15_SPEECH`..`REQ_E21_EEG_ROBUST` in `embeddings/learner/mod.rs:483-499`):

| Slot | Required files |
| --- | --- |
| E15 | `config.json`, `model.safetensors`/`pytorch_model.bin`/`model.bin`/`weights.pt`, `preprocessor_config.json`/`processor_config.json` |
| E16 | OpenFace `interface.py`, MTL model definition `MLT.py`, RetinaFace weights, Landmark weights (98 or 68), MTL backbone weights |
| E17 | `config.json`, weights, tokenizer (`tokenizer.json`/`vocab.txt`/`sentencepiece.bpe.model`) |
| E18 | `config.json`, `papagei_s.pt` |
| E19 | `config.json`, WESAD stress-floor head (`wesad_stress_head.pt`/`wesad_cnn.safetensors`/`wesad_cnn.pt`/`model.safetensors`) |
| E20 | `config.json`, weights |
| E21 | `config.json`, weights |

Calibration data required for FSV (constants `REQ_GOEMOTIONS`, `REQ_WESAD`):
- GoEmotions sample (`go_emotions_sample.jsonl`) + manifest (E17 sanity)
- Official raw WESAD archive (`WESAD.zip`) + manifest (E18/E19 calibration)

## 6. Calibration Datasets (`embeddings/learner/mod.rs:264-335` and `fixtures/utl_calibration/dataset_manifest.json`)

| Dataset | Modality | Used for | Access | License |
| --- | --- | --- | --- | --- |
| WESAD | EDA/ECG/RESP/ACC/TEMP | E18 PPG alignment, E19 EDA stress-floor calibration | research download | research |
| MSP-Podcast | speech | E15 speech valence/arousal/dominance calibration | research download | research |
| EmoBank | text | E17 text valence/arousal regression calibration | public | open |
| GoEmotions | text | E17 text affect category sanity checks | public | open |
| DAiSEE | face video | E16 engagement/affect face-video calibration | research download | research |
| AffWild2 | audio/video affect | E15 and E16 valence/arousal cross-modal calibration | research download | research |
| DEAP | EEG/peripheral | E20 EEG plasticity and affect cross-checks | research download | research |
| MAHNOB-HCI | EEG/PPG/face | E18/E20 multimodal affect-state alignment | research download | research |
| SEED/SEED-IV | EEG | E20/E21 EEG emotion and artifact robustness checks | research download | research |
| Sleep-EDF | polysomnography | sleep-gated k(τ) calibration | public | open |

Local FSV fixture: `phase0_synthetic_fixture.json` is loaded by `synthetic_calibration_fixture()` for offline tests; the `dataset_manifest.json` declares `planned_total_embedders: 21` and per-slot calibration datasets (e.g. E15: MSP-Podcast / AffWild2 / IEMOCAP / MELD; E18: WESAD / MAHNOB-HCI; E21: DEAP / SEED / AMIGOS).

## 7. Learner Calibration Workflow

For each E15-E21 slot the deterministic Phase-0 extractor in `embeddings/learner/mod.rs` exposes:
- `LearnerEmbedderInput::{Text, Samples, Features}` — accepted input shapes per modality.
- `LearnerEmbeddingOutput { slot, modality, vector, scalars: BTreeMap<String,f32>, embedder_version, latency_us }` — output type.
- `embed_calibration_sample(slot, input)` — produce one synthetic-but-deterministic embedding for unit tests / fixture FSV.
- `state_vector_from_outputs(outputs)` — collapse a vector of `LearnerEmbeddingOutput` into a single `LearnerStateVector` (concatenated `values` plus reduced `LearnerStateComponents`).

Real production extractors (e.g. running the WESAD CNN, PaPaGei, LaBraM) are out-of-tree; the in-tree code only ships:
1. Slot metadata (`LearnerEmbedderSlot`, `LearnerEmbedderSpec`).
2. Asset preflight (`preflight_learner_assets`).
3. Deterministic Phase-0 extractors used to seed FSV without real GPUs/sensors.
4. The synthetic fixture `phase0_synthetic_fixture.json` for golden-output tests.

Once real extractors run on disk, calibration consists of:
1. `preflight_learner_assets` to confirm weights + calibration data are present.
2. `record_session_observation` (MCP) writes one `LearnerFingerprint` per session, with envelopes + modality embeddings + state vector.
3. The session vectors accumulate into per-modality centroids via `compile_learner_constellation`, which writes a `LearnerConstellation { selector_kind = LEARNER_BASELINE_SELECTOR_REGULATED, label = "regulated-baseline" }` to `CF_LEARNER_CONSTELLATIONS`.
4. The constellation is later used as a baseline by `resolve_learner_retrieval_policy` to detect drift from regulated state and to gate state-conditioned retrieval profiles.

The Rust trainer math itself (LoRA, projection, AdamW, multitask heads) lives in `crates/context-graph-embeddings/src/training/` and is shared with the E5 causal embedder fine-tuning pipeline. See [16_training_subsystem.md](16_training_subsystem.md) §4 for the full pipeline. Learner-side fine-tuning of E15-E21 models reuses that machinery once real calibration datasets are downloaded.

## 8. UTL Data Models (Rust)

| Model | File:line | Fields |
| --- | --- | --- |
| `ObservationEnvelope` | `learner/mod.rs:84` | observation_id, learner_id, session_ts, modality, consent / raw / preprocessing / embedder / threshold versions, parent_observation_ids, created_at |
| `ComputationEnvelope` | `learner/mod.rs:144` | computation_id, learner_id, session_ts, parent_observation_ids, threshold_version, output_sha256, created_at |
| `LearnerProfile` | `learner/mod.rs:185` | learner_id, handle, consent_state, modalities_enabled, calibration_session_ts, created_at |
| `ModalityEmbedding` | `learner/mod.rs:229` | modality, vector (f32), scalar (Option), source_observation_id |
| `LearnerStateComponents` | `learner/mod.rs:248` | plasticity_window, hrv_coherence, valence (signed), arousal (signed), stress_floor, k_sleep |
| `LearnerStateVector` | `learner/mod.rs:276` | learner_id, session_ts, values (f32), components, context (BTreeMap) |
| `LearnerFingerprint` | `learner/mod.rs:298` | learner_id, session_ts, observation_envelopes, modality_embeddings, state_vector |
| `DeltaSBreakdown` | `learner/mod.rs:346` | predicted_actual_distance, predicted_simulated_distance, exploration_rate, gamma, delta_s |
| `DeltaCBreakdown` | `learner/mod.rs:372` | outcome_stability, coefficient_of_variation, gradient_effectiveness, hrv_coherence, panel_agreement, contradiction, delta_c |
| `DeltaEBreakdown` | `learner/mod.rs:404` | plasticity_window, hrv_coherence, valence, arousal, valence_arousal, stress_floor, k_sleep, k_state, delta_e |
| `UtlComputation` | `learner/mod.rs:467` | delta_s, delta_c, delta_e, l, diagnostic_state |
| `LearnerDeltaLog` | `learner/mod.rs:487` | learner_id, session_ts, computation, provenance |
| `LearnerMTrace` | `learner/mod.rs:512` | learner_id, trace_id, m_value (f64 [0,1]), last_update_ts, decay_rate (f32 [0,1]), num_retrievals, next_review_ts (Option) |
| `LearnerRetrievalLog` | `learner/mod.rs:565` | learner_id, trace_id, ts, correct, score [0,1], state_at_retrieval |
| `LearnerKSleep` | `learner/mod.rs:584` | learner_id, session_ts, k (f32 [0,5]), slow_wave_minutes (u16) |
| `LearnerGoalState` | `learner/mod.rs:605` | learner_id, skill_id, state_vector |
| `LearnerModalityCentroid` | `learner/mod.rs:619` | modality, vector, scalar_mean, sample_count |
| `LearnerConstellation` | `learner/mod.rs:644` | learner_id, selector_kind, label, sample_count, session_ts_start/end, modality_centroids, state_centroid, created_at |
| `GoalCentroid` | `learner/mod.rs:714` | skill_id, modality, vector |
| `LearnerAuditEntry` | `learner/mod.rs:728` | audit_id, learner_id, ts, action, target_cf, result_sha256, parent_audit_id |

## 9. Learning-Event Data Models

| Model | File:line | Fields |
| --- | --- | --- |
| `LearningStateSnapshot` | `learning/mod.rs:31` | 14D `topic_profile`, 91D `cross_correlations`, optional `retrieval_rank`, 14D `embedder_scores`, contradiction_pressure [0,1], integration_confidence [0,1], recurrence_count, stability_score [0,1], optional `domain`, successful_transfer_count |
| `LearningOutcomeLabel` | `learning/mod.rs:111` | enum `Useful` \| `Neutral` \| `Harmful` \| `NoLearning` |
| `LearningOutcome` | `learning/mod.rs:120` | label, utility_delta [-1,1], correction_required, reuse_observed |
| `LearningFeatures` | `learning/mod.rs:142` | 14D delta_e_vector, delta_e_scalar, retrieval_rank_shift, embedder_disagreement, surprise_score, productive_surprise, coherence_delta, contradiction_delta, consolidation_readiness, transfer_score, multi_utl_score, 14D attribution |
| `LearningSignalId` | `learning/mod.rs:178` | enum `DeltaE`(14) \| `Surprise`(3) \| `Coherence`(3) \| `Consolidation`(3) \| `Transfer`(3) |
| `LearningSignal` | `learning/mod.rs:210` | signal_id, vector, scalar [-1,1], optional label, attribution (14D, [0,1]) |
| `LearningEvent` | `learning/mod.rs:254` | event_id, created_at, memory_ids, session_id, response_id, task_id, query, retrieved_context, assistant_response, before, after, outcome, features, signals |

`LearningEvent::recompute_features()` (`learning/mod.rs:341`) is called inside `new()` and re-computes both `features` (deterministic UTL feature bundle) and `signals` (5 baseline signals) before write.

## 10. Learning Signals (Five Families)

Computed by `compute_baseline_learning_signals` (`learning/mod.rs:438`). Trait `LearningSignalEmbedder` (line 354) returns one `LearningSignal` per family per event. `DeterministicLearningSignalEmbedder` (line 365) is the in-tree implementation (no LLM, no neural model).

| Signal | Vector dim | Scalar | Label set |
| --- | --- | --- | --- |
| `delta_e` | 14 | mean of `after.topic_profile - before.topic_profile`, clamp [-1,1] | `positive_delta` / `negative_delta` / `no_delta` |
| `surprise` | 3: [`retrieval_rank_shift`, `embedder_disagreement`, `surprise_score`] | `surprise_score` (`0.5·max(rank_shift,0) + 0.5·disagreement`, clamp [0,1]) | `productive_surprise` (≥ 0.15 ∧ utility ≥ 0) / `low_or_noisy_surprise` |
| `coherence` | 3: [`coherence_delta`, `after.contradiction_pressure`, `after.integration_confidence`] | `coherence_delta` = (`Δintegration` − max(`Δcontradiction`, 0)), clamp [-1,1] | `coherence_improved` / `coherence_reduced` |
| `consolidation` | 3: [`min(recurrence,10)/10`, `after.stability_score`, `consolidation_readiness`] | `consolidation_readiness` = 0.5·recurrence + 0.5·stability | `ready` (≥ 0.7) / `not_ready` |
| `transfer` | 3: [`cross_domain ? 1 : 0`, `count_delta`, `transfer_score`] | `transfer_score` = 0.5·max(utility,0) + 0.5·count_delta when domains differ, else 0 | `positive_transfer` / `no_transfer` |

`compute_attribution` (`learning/mod.rs:590`): if `Σ |Δtopic| > 0`, attribution = normalized abs Δ; else fallback to normalized embedder_scores.

`compute_multi_utl_score` (`learning/mod.rs:612`): combines per-embedder semantic deltas + coherence delta via `MultiUtlParams::compute()` (`lambda_s = lambda_c = 1`, `w_e = 1`, `phi = 0`).

## 11. UTL Formulas (Source-Coded)

| Formula | File:line | Behavior |
| --- | --- | --- |
| ΔS (text) | `learner/mod.rs:748` `compute_delta_s_from_text` | Bag-of-words cosine distance; γ-weighted simulated distance; exploration boost (`+ 0.5·exploration_rate`); clamp [0,1] |
| ΔS (distances) | `learner/mod.rs:771` | `(max(actual, γ·simulated) + 0.5·exploration_rate).clamp(0,1)` |
| ΔC | `learner/mod.rs:803` | `outcome_stability + 0.2·gradient_eff + 0.2·hrv + 0.2·panel_agreement − 0.3·contradiction` clamp [0,1]; window capped at 1024; CV stability = `1/(1+cv)`; gradient = slope · scale (default 4.0) |
| ΔE | `learner/mod.rs:881` | `arousal_01 = (arousal+1)/2`; `inverted_u = 4·a·(1-a)`; `valence_sigmoid = 1/(1+exp(-3·valence))`; `valence_arousal = vs · iu`; `delta_e = plasticity^0.4 · va^0.4 · stress_floor^0.2`; `k_state = sqrt(plasticity) · k_sleep` |
| L = ΔS·ΔC·ΔE | `learner/mod.rs:908` `compute_utl_l` | `(delta_s · delta_c · delta_e).clamp(0,1)`; classifies via `classify_learner_state` |
| M(t) update | `learner/mod.rs:967` `update_m_trace` | `retained = previous_m · exp(-elapsed · decay_rate)`; `contribution = L · ΔE · k_sleep`; `m = (retained + contribution).clamp(0,1)`; on retrieval correct: `decay × 0.5`; incorrect: `(decay / 0.7).min(1.0)` |
| Next review | `learner/mod.rs:1039` `next_review_timestamp` | Solve `m·exp(-Δt·decay) = 0.7`; schedule at `now + 0.9·Δt`; returns `Some(now)` immediately when `m ≤ 0.7` |
| Transfer T | `learner/mod.rs:1008` `compute_transfer_t` | `exp(-distance / lambda)` clamp [0,1]; lambda must be `> 0` |

## 12. RocksDB Layout — 13 Learner-State CFs

Persistence helpers in `crates/context-graph-storage/src/teleological/rocksdb_store/{learner,learning,learner_training}.rs`. CF constant declarations in `crates/context-graph-storage/src/teleological/column_families.rs:399-492`. All values bincode-serialized with a single version byte prefix.

| CF | Const (line) | Key | Value (bincode) |
| --- | --- | --- | --- |
| `learning_events` | `CF_LEARNING_EVENTS` (416) | `[event_id:16]` | `[ver:u8][LearningEvent]` |
| `learner_profile` | `CF_LEARNER_PROFILE` (422) | `[learner_id:16]` | `[ver][LearnerProfile]` |
| `learner_constellations` | `CF_LEARNER_CONSTELLATIONS` (428) | `[learner_id:16][selector_kind:u8]` | `[ver][LearnerConstellation]` |
| `fingerprints_learner` | `CF_FINGERPRINTS_LEARNER` | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerFingerprint]` |
| `learner_m_per_trace` | `CF_LEARNER_M_PER_TRACE` (440) | `[learner_id:16][trace_id:16]` | `[ver][LearnerMTrace]` |
| `learner_state_history` | `CF_LEARNER_STATE_HISTORY` (446) | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerStateVector]` |
| `learner_goal_states` | `CF_LEARNER_GOAL_STATES` (452) | `[learner_id:16][skill_id:16]` | `[ver][LearnerGoalState]` |
| `learner_retrieval_log` | `CF_LEARNER_RETRIEVAL_LOG` (458) | `[learner_id:16][trace_id:16][ts:u64 BE]` | `[ver][LearnerRetrievalLog]` |
| `learner_k_sleep` | `CF_LEARNER_K_SLEEP` (464) | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerKSleep]` |
| `goal_centroids` | `CF_GOAL_CENTROIDS` (470) | `[skill_id:16][modality:u8]` | `[ver][GoalCentroid]` |
| `learner_delta_log` | `CF_LEARNER_DELTA_LOG` (476) | `[learner_id:16][session_ts:u64 BE]` | `[ver][LearnerDeltaLog]` |
| `learner_audit` | `CF_LEARNER_AUDIT` (482) | `[learner_id:16][ts:u64 BE][audit_id:16]` | `[ver][LearnerAuditEntry]` |
| `learner_training_datasets` | `CF_LEARNER_TRAINING_DATASETS` (492) | `[dataset_id:16]` | `[ver][LearnerTrainingDataset]` |

13 learner-state CFs total. The full storage layer (content CFs + indexes) is documented in [04_storage_layer.md](04_storage_layer.md).

## 13. Learner Training Datasets (`learner_training.rs`)

Matrix shards compiled from learner + Learning-as-UTL CFs into row-major `f32` tensors. Six task families:

| Task variant | Wire string |
| --- | --- |
| `RewardModel` | `reward_model` (default) |
| `Reranker` | `reranker` |
| `EmbedderContrastive` | `embedder_contrastive` |
| `DiagnosticClassifier` | `diagnostic_classifier` |
| `Scheduler` | `scheduler` |
| `PersonalPhysiology` | `personal_physiology` |

`learning_event_feature_schema()` (`learner_training.rs:255`) returns a deterministic 149-column schema for `LearningEvent` rows:

| Block | Columns | Count |
| --- | --- | --- |
| `before_topic_e1..e14` | 14 | before topic profile |
| `before_cross_correlation_00..90` | 91 | before correlations |
| `before_embedder_score_e1..e14` | 14 | before embedder scores |
| `delta_topic_e1..e14` | 14 | after − before topic |
| Scalar block | 16 | rank norms, contradiction, integration, stability, surprise, coherence_delta, contradiction_delta, consolidation_readiness, transfer_score, multi_utl_score, embedder_disagreement, retrieval_rank_shift |
| **Total** | **149** | |

`learning_event_label_schema()` returns 4 labels: `utility_delta`, `outcome_label`, `correction_required`, `reuse_observed`.

`LearnerTrainingDataset::validate()` enforces: `feature_schema` non-empty + ≤ `MAX_LEARNER_TRAINING_COLS`; `rows.len() ≤ MAX_LEARNER_TRAINING_ROWS`; `rows_len`/`cols_len` match payload; `row_major.len() == rows × cols`; all values finite; per-row `provenance_sha256` is 64-hex; `row_major_sha256` recomputed at validate() time and compared against the persisted hash.

See [16_training_subsystem.md](16_training_subsystem.md) §5 for the full schema, validation rules, and the `learning_event_feature_vector(event)` helper that produces one row.

## 14. MCP Tools

### 14.a. UTL Learner-State (18 tools, `definitions/utl.rs`)

Verified count: 18 occurrences of `ToolDefinition::new` in `crates/context-graph-mcp/src/tools/definitions/utl.rs`.

| Tool | Purpose |
| --- | --- |
| `register_learner` | Persist `LearnerProfile` to `CF_LEARNER_PROFILE` |
| `record_session_observation` | Persist `LearnerFingerprint` (envelopes + modality embeddings + state vector) to `CF_FINGERPRINTS_LEARNER` and `CF_LEARNER_STATE_HISTORY` |
| `compute_delta_s` | Compute ΔS from predicted/actual/simulated text + exploration rate |
| `compute_delta_c` | Compute ΔC from `recent_scores` window + HRV + panel agreement + contradiction |
| `compute_delta_e` | Compute ΔE from `LearnerStateComponents` or by reading a stored session |
| `compute_L` | Compute L = ΔS·ΔC·ΔE; optional `persist=true` writes `CF_LEARNER_DELTA_LOG` + updates `CF_LEARNER_M_PER_TRACE` |
| `get_learner_M` | Read `LearnerMTrace` |
| `next_review_for_trace` | Return next-review timestamp for a stored trace |
| `list_learner_embedders` | Canonical E1-E21 matrix (E1-E14 content + E15-E21 learner-state) with model names, dimensions, scalar heads |
| `preflight_learner_assets` | Inspect cached E15-E21 weights + calibration data; fails closed unless `allowMissing=true` |
| `count_learner_state` | Row counts for all learner-state CFs |
| `get_learner_state` | Read profile + session fingerprint + state vector + delta log + k_sleep + optional M trace |
| `record_learner_k_sleep` | Persist `LearnerKSleep` |
| `record_learner_retrieval` | Persist `LearnerRetrievalLog` (state from stored session OR explicit vector/components) |
| `upsert_goal_centroid` | Persist `GoalCentroid` for `(skill, modality)` |
| `get_goal_distance` | Cosine distance to centroid; optional `persistGoalState=true` |
| `compile_learner_constellation` | Compile regulated-state baseline from session fingerprints into `CF_LEARNER_CONSTELLATIONS` |
| `resolve_learner_retrieval_policy` | Read stored learner state; resolve content E1-E14 weight profile (see §15) |

### 14.b. Learning Events (14 tools, `definitions/learning.rs`)

Verified count: 14 occurrences of `ToolDefinition::new` in `crates/context-graph-mcp/src/tools/definitions/learning.rs`.

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
| `export_learner_training_dataset` | Compile UTL + learner-state rows into a 149-column matrix shard in `CF_LEARNER_TRAINING_DATASETS` |
| `list_learner_training_datasets` | Dataset UUIDs + optional matrix shape |
| `get_learner_training_dataset` | Dataset by UUID; toggles `includeMatrix`, `previewRows ≤ 100` |
| `count_learner_training_datasets` | Row count |

### 14.c. Capability matrix surface (`capability_tools.rs`)

`get_capability_matrix` (read-only, no storage mutation) reports:
- `embedders.contentCount = 14`, `learnerStateCount = 7`, `totalSlots = 21` (`capability_tools.rs:69-71`).
- Per-slot metadata for E1-E14 and E15-E21 with `primaryTools` mapping (lines 217-244).
- Tool groups: `learner_state_utl` and `learning_events` enumerate the tools above (lines 206-207).
- Capability records include `state_conditioned_retrieval` (status: `available_when_learner_state_exists`), `learner_measurement` (always available), and `closed_loop_training` (status: `available_when_events_exist`) (lines 248-258).
- Source-of-truth CF list for the learnerState domain mirrors §12 above (lines 264-269).
- Runtime counts (when `includeRuntimeState=true`) call `count_learner_*` per CF (lines 146-158).

### 14.d. Learning signal embedder metadata

`list_learning_signal_embedders` returns:

```json
{
  "implementation": "DeterministicLearningSignalEmbedder",
  "count": 5,
  "embedders": [
    { "signal_id": "delta_e", "dimension": 14 },
    { "signal_id": "surprise", "dimension": 3 },
    { "signal_id": "coherence", "dimension": 3 },
    { "signal_id": "consolidation", "dimension": 3 },
    { "signal_id": "transfer", "dimension": 3 }
  ]
}
```

## 15. State-Conditioned Retrieval Policy

`resolve_learner_retrieval_policy` resolves a content weight profile from persisted learner state. **Does NOT inject E15-E21 into content fusion.** The selector lives in `crates/context-graph-core/src/weights/mod.rs::select_state_conditioned_weight_profile` (lines 658-694) and is invoked by both `utl_tools.rs::resolve_learner_retrieval_policy` and the `search_graph` dispatcher in `memory_tools.rs:1736`.

Decision tree on `LearnerStateComponents`:

| Condition | Selected profile |
| --- | --- |
| `stress_floor ≥ 0.80` OR (`stress_floor ≥ 0.60` AND `arousal ≥ 0.65`) | `affect_neutral` |
| `stress_floor ≥ 0.60` | `affect_repair` |
| `valence ≤ -0.30` | `affect_priming` |
| else | `baseWeightProfile` (default `semantic_search`) |

Allowed `baseWeightProfile` enum (20 values): `semantic_search`, `causal_reasoning`, `code_search`, `fact_checking`, `graph_reasoning`, `temporal_navigation`, `sequence_navigation`, `conversation_history`, `category_weighted`, `typo_tolerant`, `pipeline_stage1_recall`, `pipeline_stage2_scoring`, `pipeline_full`, `balanced`, `multilingual_search`, `long_context`, `translation_finder`, `affect_repair`, `affect_priming`, `affect_neutral`. Fail-fast on unknown base profile or invalid state. The selector is FSV-tested at `utl_tools.rs:2174,2208`.

## 16. Graph-Learning Policy (`graph_learning_tools.rs`)

| Constant | Value | Source |
| --- | --- | --- |
| `GRAPH_LEARNING_TASK_PREFIX` | `"graph_learning:"` | `graph_learning_tools.rs:28` |
| `default_graph_learning_max_scan` | 10000 | `graph_learning_tools.rs:1011` |
| `default_graph_learning_rate` | 0.35 | `graph_learning_tools.rs:1015` |

`record_graph_learning_event` requires `RocksDbTeleologicalStore` and a live `EdgeRepository`; it reads real K-NN/typed edges, rejects missing evidence, derives the learning state from observed edge scores, writes the event, and immediately reads the same RocksDB row back for full-state verification.

`resolve_graph_learning_policy` scans persisted events filtered by `policyScope` (and optionally `sourceMemoryId`), validates embedded graph evidence, aggregates into per-embedder weight deltas + edge-type boosts at `learningRate ∈ [0,1]`, and returns the learned policy. **Errors if matched evidence count < `minEvidence`** rather than falling back to the base policy.

Allowed `graphTool` enum: `get_unified_neighbors` (default), `get_memory_neighbors`, `traverse_graph`. Typed-edge schema and contrastive-pair semantics consumed by this policy live in [18_contrastive_and_typed_edges.md](18_contrastive_and_typed_edges.md).

## 17. CLI

`crates/context-graph-cli/src/commands/learning.rs` (`LearningCommands` enum):

| Subcommand | Purpose |
| --- | --- |
| `learning count` | Row count for `CF_LEARNING_EVENTS` |
| `learning list --limit N --offset M` | UUIDs + per-event shape summary |
| `learning get --event-id <UUID> --include-text` | Full event |
| `learning export-events-jsonl --out <path> --limit N --offset M --include-text --overwrite` | Tensor-row JSONL with `record_kind=learning_event_tensor` (input format for `build_learner_dpo_pairs.py`) |
| `learning export-dataset-jsonl --out <path> --dataset-id <UUID> --limit --offset --overwrite` | Persisted matrix-shard rows as JSONL |
| `learning record-events-jsonl --input <path> --clear-existing` | Bulk-import real `LearningEvent` rows from JSONL |
| `learning record-synthetic --event-id <UUID> --outcome-label useful --utility-delta 0.4 --clear-existing` | Deterministic synthetic event for FSV |

`crates/context-graph-cli/src/commands/utl.rs` (`UtlCommands` enum):

| Subcommand | Purpose |
| --- | --- |
| `utl list-embedders` | E15-E21 embedder registry + calibration dataset manifest |
| `utl fixture` | Print the deterministic local calibration fixture summary (`phase0_synthetic_fixture.json`) |
| `utl preflight-assets --models-root models --calibration-root data/utl_calibration --allow-missing` | Inspect required E15-E21 model files + real calibration data |
| `utl count --storage <path>` | Row counts for all learner-state CFs |
| `utl record-synthetic --learner-id <UUID> --trace-id <UUID> --session-ts 1700000000 --clear-existing` | Store a deterministic synthetic learner session and read it back |
| `utl get-delta --learner-id <UUID> --session-ts <ts>` | Fetch one persisted UTL delta log |
| `utl get-m --learner-id <UUID> --trace-id <UUID>` | Fetch one persisted M(t) trace |

All paths default to `--storage` from the `CONTEXT_GRAPH_STORAGE_PATH` env var.

`learning export-events-jsonl` is the input source for the Python learner-conditioned DPO miner — see [19_camd_dpo_subsystem.md](19_camd_dpo_subsystem.md) §7. Integration test: `crates/context-graph-cli/tests/learning_jsonl_export_integration.rs` exercises the JSONL exports against a real RocksDB store with `record_kind=learning_event_tensor` schema validation.

The actual LoRA + projection training math (`Cell<bool>` toggle, Cosine LR schedule, AdamW, multitask heads, per-stage best-checkpoint selection by `score_spread`, hard-negative-scale lessons, prefix matching) lives in [16_training_subsystem.md](16_training_subsystem.md) §4.

## 18. Dream Consolidation (NREM/REM)

The dream-consolidation feature is exposed as the `dream-consolidation` skill in the Claude Code harness layer (NREM replay of high-importance patterns via Hebbian learning; REM blind-spot discovery via hyperbolic random walk in the Poincare ball). The Rust core does not implement an in-process dream loop today — the skill is the caller-side trigger. M(t) sleep gating in §11 is the in-tree consolidation primitive: `next_review_timestamp` solves `m·exp(-Δt·decay) = 0.7` and `update_m_trace` factors `k_sleep` into the consolidation contribution. The `LearnerKSleep { k ∈ [0, 5], slow_wave_minutes: u16 }` record (CF `learner_k_sleep`) is where the slow-wave evidence is persisted.

Operator triggers (recommended thresholds): `entropy > 0.7 AND churn > 0.5`. The skill invokes content-side curation tools to merge / forget / boost memories; learner-side it can drive `record_learner_k_sleep` to seed M(t) decay-rate adjustments.

## 19. Source Notes

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
- The 7-slot E15-E21 dimensions (1024/512/384/512/64/768/768) are NOT uniform — cross-modal aggregation must reconcile per-slot dimensionality before producing a unified state vector. `state_vector_from_outputs` handles this concatenation.
- The `phase0_synthetic_fixture.json` file is the only in-tree mechanism for FSV without real model weights or calibration data; production deployment requires downloading the datasets enumerated in `dataset_manifest.json`.
