//! UTL learner-state tool handlers.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::Utc;
use context_graph_core::learner::{
    compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_transfer_t, compute_utl_l,
    sha256_bytes, sha256_json, update_m_trace, ComputationEnvelope, GoalCentroid,
    LearnerAuditEntry, LearnerConstellation, LearnerDeltaLog, LearnerFingerprint, LearnerGoalState,
    LearnerKSleep, LearnerModality, LearnerModalityCentroid, LearnerProfile, LearnerRetrievalLog,
    LearnerStateComponents, LearnerStateVector, ModalityEmbedding, ObservationEnvelope,
    LEARNER_BASELINE_SELECTOR_REGULATED,
};
use context_graph_core::weights::{
    get_effective_weight_profile, select_state_conditioned_weight_profile,
    StateConditionedProfileSelection,
};
use context_graph_embeddings::types::ModelId;
use context_graph_embeddings::{
    embed_learner_signal, learner_embedder_specs, preflight_learner_assets,
    state_vector_from_outputs, LearnerEmbedderInput, LearnerEmbedderSlot,
    CALIBRATION_DATASET_MANIFEST, LEARNER_EMBEDDER_COUNT, UTL_PLANNED_TOTAL_EMBEDDERS,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::handlers::Handlers;
use crate::protocol::{JsonRpcId, JsonRpcResponse};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterLearnerArgs {
    learner_id: Option<Uuid>,
    handle: String,
    #[serde(default = "default_consent")]
    consent_state: String,
    #[serde(default = "default_modalities")]
    modalities_enabled: Vec<String>,
    calibration_session_ts: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordObservationArgs {
    learner_id: Uuid,
    session_ts: u64,
    #[serde(default = "default_modality")]
    modality: String,
    #[serde(default)]
    raw_text: String,
    #[serde(default = "default_consent")]
    consent_state: String,
    #[serde(default = "default_preprocess")]
    preprocessing_version: String,
    #[serde(default = "default_embedder")]
    embedder_version: String,
    #[serde(default = "default_threshold")]
    threshold_version: String,
    #[serde(default)]
    state_vector: Vec<f32>,
    components: Option<ComponentsArgs>,
    #[serde(default)]
    embeddings: Vec<EmbeddingArgs>,
    #[serde(default)]
    signals: Vec<SignalArgs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComponentsArgs {
    plasticity_window: f32,
    hrv_coherence: f32,
    valence: f32,
    arousal: f32,
    stress_floor: f32,
    #[serde(default = "one")]
    k_sleep: f32,
}

impl From<ComponentsArgs> for LearnerStateComponents {
    fn from(value: ComponentsArgs) -> Self {
        Self {
            plasticity_window: value.plasticity_window,
            hrv_coherence: value.hrv_coherence,
            valence: value.valence,
            arousal: value.arousal,
            stress_floor: value.stress_floor,
            k_sleep: value.k_sleep,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EmbeddingArgs {
    modality: String,
    vector: Vec<f32>,
    scalar: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalArgs {
    modality: String,
    text: Option<String>,
    #[serde(default)]
    samples: Vec<f32>,
    #[serde(default)]
    features: Vec<f32>,
    sample_rate_hz: Option<u32>,
    channels: Option<u16>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaSArgs {
    #[serde(default)]
    predicted_text: String,
    #[serde(default)]
    actual_text: String,
    simulated_text: Option<String>,
    #[serde(default)]
    exploration_rate: f32,
    gamma: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaCArgs {
    #[serde(default)]
    recent_scores: Vec<f32>,
    #[serde(default)]
    hrv_coherence: f32,
    #[serde(default)]
    panel_agreement: f32,
    #[serde(default)]
    contradiction: f32,
    gradient_scale: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaEArgs {
    learner_id: Option<Uuid>,
    session_ts: Option<u64>,
    components: Option<ComponentsArgs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComputeLArgs {
    learner_id: Option<Uuid>,
    session_ts: Option<u64>,
    trace_id: Option<Uuid>,
    #[serde(default)]
    predicted_text: String,
    #[serde(default)]
    actual_text: String,
    simulated_text: Option<String>,
    #[serde(default)]
    exploration_rate: f32,
    #[serde(default)]
    recent_scores: Vec<f32>,
    #[serde(default)]
    hrv_coherence: f32,
    #[serde(default)]
    panel_agreement: f32,
    #[serde(default)]
    contradiction: f32,
    components: ComponentsArgs,
    #[serde(default)]
    persist: bool,
    retrieval_correct: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TraceArgs {
    learner_id: Uuid,
    trace_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreflightLearnerAssetsArgs {
    #[serde(default = "default_models_root")]
    models_root: PathBuf,
    #[serde(default = "default_calibration_root")]
    calibration_root: PathBuf,
    #[serde(default)]
    allow_missing: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetLearnerStateArgs {
    learner_id: Uuid,
    session_ts: Option<u64>,
    trace_id: Option<Uuid>,
    #[serde(default)]
    include_vectors: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordKSleepArgs {
    learner_id: Uuid,
    session_ts: u64,
    slow_wave_minutes: u16,
    k: Option<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordRetrievalArgs {
    learner_id: Uuid,
    trace_id: Uuid,
    ts: u64,
    session_ts: Option<u64>,
    correct: bool,
    score: f32,
    #[serde(default)]
    state_vector: Vec<f32>,
    components: Option<ComponentsArgs>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoalCentroidArgs {
    skill_id: Uuid,
    modality: String,
    vector: Vec<f32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoalDistanceArgs {
    learner_id: Uuid,
    skill_id: Uuid,
    session_ts: u64,
    modality: String,
    #[serde(default)]
    persist_goal_state: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileConstellationArgs {
    learner_id: Uuid,
    session_ts_list: Vec<u64>,
    selector_kind: Option<u8>,
    #[serde(default = "default_constellation_label")]
    label: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResolveRetrievalPolicyArgs {
    learner_id: Uuid,
    session_ts: u64,
    base_weight_profile: Option<String>,
    #[serde(default = "default_true")]
    include_weights: bool,
}

impl Handlers {
    pub(crate) async fn call_register_learner(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: RegisterLearnerArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid register_learner args: {e}")),
        };
        let mut modalities = BTreeSet::new();
        for raw in parsed.modalities_enabled {
            let modality = match LearnerModality::parse(&raw) {
                Ok(v) => v,
                Err(e) => return self.tool_error(id, &format!("{e}")),
            };
            modalities.insert(modality);
        }
        let profile = match LearnerProfile::new(
            parsed.learner_id.unwrap_or_else(Uuid::new_v4),
            parsed.handle,
            parsed.consent_state,
            modalities,
            parsed.calibration_session_ts,
        ) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Learner profile validation failed: {e}"))
            }
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(id, "register_learner requires RocksDbTeleologicalStore.");
        };
        if let Err(e) = store.store_learner_profile(&profile).await {
            return self.tool_error(id, &format!("store_learner_profile failed: {e}"));
        }
        if let Err(e) = store_learner_audit(
            store,
            profile.learner_id,
            parsed.calibration_session_ts.unwrap_or(0),
            "register_learner",
            "learner_profile",
            &profile,
        )
        .await
        {
            return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "learner_id": profile.learner_id.to_string(),
                "source_of_truth": source_of_truth(["learner_profile", "learner_audit"]),
                "profile": render_profile(&profile),
            }),
        )
    }

    pub(crate) async fn call_record_session_observation(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: RecordObservationArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self
                    .tool_error(id, &format!("Invalid record_session_observation args: {e}"))
            }
        };
        let RecordObservationArgs {
            learner_id,
            session_ts,
            modality,
            raw_text,
            consent_state,
            preprocessing_version,
            embedder_version,
            threshold_version,
            state_vector: state_vector_arg,
            components: components_arg,
            embeddings: embedding_args,
            signals,
        } = parsed;
        if consent_state == "revoked" {
            return self.tool_error(
                id,
                "record_session_observation refuses consentState=revoked; no learner observation was written",
            );
        }
        let mut context = BTreeMap::new();
        let mut embeddings = Vec::with_capacity(embedding_args.len() + signals.len());
        let mut observation_envelopes = Vec::new();
        let state_vector = if signals.is_empty() {
            let modality = match LearnerModality::parse(&modality) {
                Ok(v) => v,
                Err(e) => return self.tool_error(id, &format!("{e}")),
            };
            let observation_id = Uuid::new_v4();
            let envelope = match ObservationEnvelope::new(
                observation_id,
                learner_id,
                session_ts,
                modality,
                consent_state.clone(),
                sha256_bytes(raw_text.as_bytes()),
                preprocessing_version.clone(),
                embedder_version.clone(),
                threshold_version.clone(),
                Vec::new(),
            ) {
                Ok(v) => v,
                Err(e) => {
                    return self.tool_error(id, &format!("Observation validation failed: {e}"))
                }
            };
            observation_envelopes.push(envelope);
            let Some(components) = components_arg else {
                return self.tool_error(
                    id,
                    "record_session_observation requires components when signals are not provided",
                );
            };
            let components: LearnerStateComponents = components.into();
            let values = if state_vector_arg.is_empty() {
                vec![
                    components.plasticity_window,
                    components.hrv_coherence,
                    components.valence,
                    components.arousal,
                    components.stress_floor,
                    components.k_sleep,
                ]
            } else {
                state_vector_arg
            };
            LearnerStateVector {
                learner_id,
                session_ts,
                values,
                components,
                context,
            }
        } else {
            let mut outputs = Vec::with_capacity(signals.len());
            for signal in signals {
                let (slot, input, raw_hash) = match signal_to_embedder_input(signal) {
                    Ok(v) => v,
                    Err(e) => return self.tool_error(id, &e),
                };
                let output = match embed_learner_signal(slot, &input) {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(
                            id,
                            &format!("learner embedder {} failed: {e}", slot.as_str()),
                        )
                    }
                };
                let observation_id = Uuid::new_v4();
                let envelope = match ObservationEnvelope::new(
                    observation_id,
                    learner_id,
                    session_ts,
                    output.modality,
                    consent_state.clone(),
                    raw_hash,
                    preprocessing_version.clone(),
                    output.embedder_version.clone(),
                    threshold_version.clone(),
                    Vec::new(),
                ) {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("Observation validation failed: {e}"))
                    }
                };
                observation_envelopes.push(envelope);
                embeddings.push(output.to_modality_embedding(observation_id));
                outputs.push(output);
            }
            if outputs.len() == LEARNER_EMBEDDER_COUNT {
                context.insert(
                    "record_session_observation_mode".into(),
                    "full_signal_panel".into(),
                );
                match state_vector_from_outputs(learner_id, session_ts, &outputs, context) {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("state vector derivation failed: {e}"))
                    }
                }
            } else {
                let Some(components) = components_arg else {
                    return self.tool_error(
                        id,
                        "partial signal observations require components so a valid learner state vector can be persisted",
                    );
                };
                let components: LearnerStateComponents = components.into();
                let values = if state_vector_arg.is_empty() {
                    vec![
                        components.plasticity_window,
                        components.hrv_coherence,
                        components.valence,
                        components.arousal,
                        components.stress_floor,
                        components.k_sleep,
                    ]
                } else {
                    state_vector_arg
                };
                context.insert(
                    "record_session_observation_mode".into(),
                    "partial_signals_with_components".into(),
                );
                LearnerStateVector {
                    learner_id,
                    session_ts,
                    values,
                    components,
                    context,
                }
            }
        };
        let default_observation_id = observation_envelopes
            .first()
            .map(|envelope| envelope.observation_id)
            .unwrap_or_else(Uuid::new_v4);
        for embedding in embedding_args {
            let modality = match LearnerModality::parse(&embedding.modality) {
                Ok(v) => v,
                Err(e) => return self.tool_error(id, &format!("{e}")),
            };
            embeddings.push(ModalityEmbedding {
                modality,
                vector: embedding.vector,
                scalar: embedding.scalar,
                source_observation_id: default_observation_id,
            });
        }
        if embeddings.is_empty() {
            let observation_id = observation_envelopes
                .first()
                .map(|envelope| envelope.observation_id)
                .unwrap_or_else(Uuid::new_v4);
            let modality = observation_envelopes
                .first()
                .map(|envelope| envelope.modality)
                .unwrap_or(LearnerModality::AffectText);
            embeddings.push(ModalityEmbedding {
                modality,
                vector: state_vector.values.clone(),
                scalar: None,
                source_observation_id: observation_id,
            });
        }
        let fingerprint = LearnerFingerprint {
            learner_id,
            session_ts,
            observation_envelopes,
            modality_embeddings: embeddings,
            state_vector,
        };
        if let Err(e) = fingerprint.validate() {
            return self.tool_error(id, &format!("Learner fingerprint validation failed: {e}"));
        }
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "record_session_observation requires RocksDbTeleologicalStore.",
            );
        };
        match store.get_learner_profile(fingerprint.learner_id).await {
            Ok(Some(profile)) if profile.consent_state == "revoked" => {
                return self.tool_error(
                    id,
                    "record_session_observation refuses writes for learner profile with consent_state=revoked",
                )
            }
            Ok(_) => {}
            Err(e) => return self.tool_error(id, &format!("get_learner_profile failed: {e}")),
        }
        if let Err(e) = store.store_learner_fingerprint(&fingerprint).await {
            return self.tool_error(id, &format!("store_learner_fingerprint failed: {e}"));
        }
        if let Err(e) = store_learner_audit(
            store,
            fingerprint.learner_id,
            fingerprint.session_ts,
            "record_session_observation",
            "fingerprints_learner",
            &fingerprint,
        )
        .await
        {
            return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": source_of_truth(["fingerprints_learner", "learner_state_history", "learner_audit"]),
                "fingerprint": render_fingerprint(&fingerprint),
            }),
        )
    }

    pub(crate) async fn call_compute_delta_s(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: DeltaSArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid compute_delta_s args: {e}")),
        };
        match compute_delta_s_from_text(
            &parsed.predicted_text,
            &parsed.actual_text,
            parsed.simulated_text.as_deref(),
            parsed.exploration_rate,
            parsed.gamma,
        ) {
            Ok(result) => self.tool_result(id, json!({"delta_s": result})),
            Err(e) => self.tool_error(id, &format!("compute_delta_s failed: {e}")),
        }
    }

    pub(crate) async fn call_compute_delta_c(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: DeltaCArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid compute_delta_c args: {e}")),
        };
        match compute_delta_c(
            &parsed.recent_scores,
            parsed.hrv_coherence,
            parsed.panel_agreement,
            parsed.contradiction,
            parsed.gradient_scale,
        ) {
            Ok(result) => self.tool_result(id, json!({"delta_c": result})),
            Err(e) => self.tool_error(id, &format!("compute_delta_c failed: {e}")),
        }
    }

    pub(crate) async fn call_compute_delta_e(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: DeltaEArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid compute_delta_e args: {e}")),
        };
        let components = if let Some(components) = parsed.components {
            components.into()
        } else if let (Some(learner_id), Some(session_ts)) = (parsed.learner_id, parsed.session_ts)
        {
            let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
                return self.tool_error(id, "compute_delta_e requires RocksDbTeleologicalStore.");
            };
            match store.get_learner_state_vector(learner_id, session_ts).await {
                Ok(Some(state)) => state.components,
                Ok(None) => {
                    return self.tool_error(id, "No learner state vector found for learner/session")
                }
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_state_vector failed: {e}"))
                }
            }
        } else {
            return self.tool_error(
                id,
                "compute_delta_e requires either components or learnerId+sessionTs",
            );
        };
        match compute_delta_e(&components) {
            Ok(result) => self.tool_result(id, json!({"delta_e": result})),
            Err(e) => self.tool_error(id, &format!("compute_delta_e failed: {e}")),
        }
    }

    pub(crate) async fn call_compute_l(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: ComputeLArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid compute_L args: {e}")),
        };
        let delta_s = match compute_delta_s_from_text(
            &parsed.predicted_text,
            &parsed.actual_text,
            parsed.simulated_text.as_deref(),
            parsed.exploration_rate,
            Some(0.7),
        ) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("compute_delta_s failed: {e}")),
        };
        let delta_c = match compute_delta_c(
            &parsed.recent_scores,
            parsed.hrv_coherence,
            parsed.panel_agreement,
            parsed.contradiction,
            None,
        ) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("compute_delta_c failed: {e}")),
        };
        let components: LearnerStateComponents = parsed.components.into();
        let delta_e = match compute_delta_e(&components) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("compute_delta_e failed: {e}")),
        };
        let computation = match compute_utl_l(delta_s, delta_c, delta_e, 0, None) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("compute_L failed: {e}")),
        };

        let mut persisted = json!({"delta_log": false, "m_trace": false});
        if parsed.persist {
            let (Some(learner_id), Some(session_ts)) = (parsed.learner_id, parsed.session_ts)
            else {
                return self.tool_error(
                    id,
                    "compute_L persist=true requires learnerId and sessionTs",
                );
            };
            let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
                return self.tool_error(
                    id,
                    "compute_L persist=true requires RocksDbTeleologicalStore.",
                );
            };
            let output_hash = match sha256_json(&computation) {
                Ok(v) => v,
                Err(e) => return self.tool_error(id, &format!("hash computation failed: {e}")),
            };
            let provenance = match ComputationEnvelope::new(
                Uuid::new_v4(),
                learner_id,
                session_ts,
                Vec::new(),
                default_threshold(),
                output_hash,
            ) {
                Ok(v) => v,
                Err(e) => {
                    return self.tool_error(id, &format!("provenance validation failed: {e}"))
                }
            };
            let delta_log = LearnerDeltaLog {
                learner_id,
                session_ts,
                computation: computation.clone(),
                provenance,
            };
            if let Err(e) = store.store_learner_delta_log(&delta_log).await {
                return self.tool_error(id, &format!("store_learner_delta_log failed: {e}"));
            }
            if let Err(e) = store_learner_audit(
                store,
                learner_id,
                session_ts,
                "compute_L",
                "learner_delta_log",
                &delta_log,
            )
            .await
            {
                return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
            }
            persisted["delta_log"] = json!(true);
            if let Some(trace_id) = parsed.trace_id {
                let previous = match store.get_learner_m_trace(learner_id, trace_id).await {
                    Ok(v) => v,
                    Err(e) => {
                        return self.tool_error(id, &format!("get_learner_m_trace failed: {e}"))
                    }
                };
                let m_trace = match update_m_trace(
                    previous.as_ref(),
                    learner_id,
                    trace_id,
                    session_ts,
                    &computation,
                    parsed.retrieval_correct,
                ) {
                    Ok(v) => v,
                    Err(e) => return self.tool_error(id, &format!("update_m_trace failed: {e}")),
                };
                if let Err(e) = store.store_learner_m_trace(&m_trace).await {
                    return self.tool_error(id, &format!("store_learner_m_trace failed: {e}"));
                }
                if let Err(e) = store_learner_audit(
                    store,
                    learner_id,
                    session_ts,
                    "compute_L",
                    "learner_m_per_trace",
                    &m_trace,
                )
                .await
                {
                    return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
                }
                persisted["m_trace"] = json!(true);
                persisted["next_review_ts"] = json!(m_trace.next_review_ts);
            }
        }

        self.tool_result(
            id,
            json!({
                "computation": {
                    "delta_s": computation.delta_s,
                    "delta_c": computation.delta_c,
                    "delta_e": computation.delta_e,
                    "l": computation.l,
                    "diagnostic_state": computation.diagnostic_state.as_str(),
                },
                "persisted": persisted,
                "source_of_truth": source_of_truth(["learner_delta_log", "learner_m_per_trace", "learner_audit"]),
            }),
        )
    }

    pub(crate) async fn call_get_learner_m(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: TraceArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid get_learner_M args: {e}")),
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(id, "get_learner_M requires RocksDbTeleologicalStore.");
        };
        match store
            .get_learner_m_trace(parsed.learner_id, parsed.trace_id)
            .await
        {
            Ok(Some(trace)) => self.tool_result(
                id,
                json!({"found": true, "m_trace": render_m_trace(&trace)}),
            ),
            Ok(None) => self.tool_result(id, json!({"found": false})),
            Err(e) => self.tool_error(id, &format!("get_learner_m_trace failed: {e}")),
        }
    }

    pub(crate) async fn call_next_review_for_trace(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: TraceArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid next_review_for_trace args: {e}"))
            }
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "next_review_for_trace requires RocksDbTeleologicalStore.",
            );
        };
        match store
            .get_learner_m_trace(parsed.learner_id, parsed.trace_id)
            .await
        {
            Ok(Some(trace)) => self.tool_result(
                id,
                json!({
                    "found": true,
                    "learner_id": trace.learner_id.to_string(),
                    "trace_id": trace.trace_id.to_string(),
                    "m_value": trace.m_value,
                    "decay_rate": trace.decay_rate,
                    "next_review_ts": trace.next_review_ts,
                    "source_of_truth": source_of_truth(["learner_m_per_trace"]),
                }),
            ),
            Ok(None) => self.tool_result(id, json!({"found": false})),
            Err(e) => self.tool_error(id, &format!("get_learner_m_trace failed: {e}")),
        }
    }

    pub(crate) async fn call_list_learner_embedders(
        &self,
        id: Option<JsonRpcId>,
    ) -> JsonRpcResponse {
        self.tool_result(
            id,
            json!({
                "source_of_truth": {
                    "content_registry": "context_graph_embeddings::types::ModelId::production",
                    "learner_registry": "context_graph_embeddings::learner::LearnerEmbedderSlot::all",
                    "calibration_manifest": "context_graph_embeddings::learner::CALIBRATION_DATASET_MANIFEST"
                },
                "planned_total_embedders": UTL_PLANNED_TOTAL_EMBEDDERS,
                "content_embedder_count": ModelId::production().len(),
                "learner_embedder_count": learner_embedder_specs().len(),
                "content_embedders": content_embedder_specs_json(),
                "learner_embedders": learner_embedder_specs().iter().map(|spec| json!({
                    "slot": spec.slot_number,
                    "id": spec.slot.as_str(),
                    "modality": spec.modality.as_str(),
                    "model_name": spec.model_name,
                    "model_path": spec.model_path,
                    "output_dimension": spec.output_dimension,
                    "scalar_heads": spec.scalar_heads,
                })).collect::<Vec<_>>(),
                "calibration_datasets": CALIBRATION_DATASET_MANIFEST.iter().map(|dataset| json!({
                    "name": dataset.name,
                    "modality": dataset.modality,
                    "used_for": dataset.used_for,
                    "access": dataset.access,
                    "license": dataset.license,
                })).collect::<Vec<_>>(),
                "legacy_content_variants_not_counted": [{
                    "id": "Entity",
                    "reason": "legacy E11 MiniLM compatibility variant; production E11 is Kepler"
                }]
            }),
        )
    }

    pub(crate) async fn call_preflight_learner_assets(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: PreflightLearnerAssetsArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid preflight_learner_assets args: {e}"))
            }
        };
        let report = match preflight_learner_assets(&parsed.models_root, &parsed.calibration_root) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("preflight_learner_assets failed: {e}")),
        };
        let value = match serde_json::to_value(&report) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("serialize preflight report failed: {e}"))
            }
        };
        if !report.ready && !parsed.allow_missing {
            return self.tool_error(
                id,
                &format!(
                    "UTL learner asset preflight failed: {} missing file groups",
                    report.missing_count
                ),
            );
        }
        self.tool_result(id, value)
    }

    pub(crate) async fn call_count_learner_state(&self, id: Option<JsonRpcId>) -> JsonRpcResponse {
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(id, "count_learner_state requires RocksDbTeleologicalStore.");
        };
        match learner_counts(store).await {
            Ok(counts) => self.tool_result(
                id,
                json!({
                    "source_of_truth": source_of_truth([
                        "learner_profile",
                        "learner_constellations",
                        "fingerprints_learner",
                        "learner_m_per_trace",
                        "learner_state_history",
                        "learner_goal_states",
                        "learner_retrieval_log",
                        "learner_k_sleep",
                        "goal_centroids",
                        "learner_delta_log",
                        "learner_audit"
                    ]),
                    "counts": counts,
                }),
            ),
            Err(e) => self.tool_error(id, &format!("count learner CFs failed: {e}")),
        }
    }

    pub(crate) async fn call_get_learner_state(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: GetLearnerStateArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid get_learner_state args: {e}")),
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(id, "get_learner_state requires RocksDbTeleologicalStore.");
        };
        let profile = match store.get_learner_profile(parsed.learner_id).await {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("get_learner_profile failed: {e}")),
        };
        let mut session = json!(null);
        if let Some(session_ts) = parsed.session_ts {
            let fingerprint = match store
                .get_learner_fingerprint(parsed.learner_id, session_ts)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_fingerprint failed: {e}"))
                }
            };
            let state = match store
                .get_learner_state_vector(parsed.learner_id, session_ts)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_state_vector failed: {e}"))
                }
            };
            let delta = match store
                .get_learner_delta_log(parsed.learner_id, session_ts)
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_delta_log failed: {e}"))
                }
            };
            let k_sleep = match store
                .get_learner_k_sleep(parsed.learner_id, session_ts)
                .await
            {
                Ok(v) => v,
                Err(e) => return self.tool_error(id, &format!("get_learner_k_sleep failed: {e}")),
            };
            session = json!({
                "session_ts": session_ts,
                "fingerprint": fingerprint.as_ref().map(render_fingerprint),
                "state_vector": state.as_ref().map(|state| render_state_vector(state, parsed.include_vectors)),
                "delta_log": delta.as_ref().map(render_delta_log),
                "k_sleep": k_sleep.as_ref().map(render_k_sleep),
            });
        }
        let m_trace = if let Some(trace_id) = parsed.trace_id {
            match store.get_learner_m_trace(parsed.learner_id, trace_id).await {
                Ok(v) => v.map(|trace| render_m_trace(&trace)),
                Err(e) => return self.tool_error(id, &format!("get_learner_m_trace failed: {e}")),
            }
        } else {
            None
        };
        self.tool_result(
            id,
            json!({
                "source_of_truth": source_of_truth([
                    "learner_profile",
                    "fingerprints_learner",
                    "learner_state_history",
                    "learner_delta_log",
                    "learner_k_sleep",
                    "learner_m_per_trace"
                ]),
                "learner_id": parsed.learner_id.to_string(),
                "profile_found": profile.is_some(),
                "profile": profile.as_ref().map(render_profile),
                "session": session,
                "m_trace": m_trace,
            }),
        )
    }

    pub(crate) async fn call_record_learner_k_sleep(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: RecordKSleepArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid record_learner_k_sleep args: {e}"))
            }
        };
        let value = LearnerKSleep {
            learner_id: parsed.learner_id,
            session_ts: parsed.session_ts,
            k: parsed
                .k
                .unwrap_or_else(|| k_sleep_from_slow_wave_minutes(parsed.slow_wave_minutes)),
            slow_wave_minutes: parsed.slow_wave_minutes,
        };
        if let Err(e) = value.validate() {
            return self.tool_error(id, &format!("LearnerKSleep validation failed: {e}"));
        }
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "record_learner_k_sleep requires RocksDbTeleologicalStore.",
            );
        };
        if let Err(e) = store.store_learner_k_sleep(&value).await {
            return self.tool_error(id, &format!("store_learner_k_sleep failed: {e}"));
        }
        if let Err(e) = store_learner_audit(
            store,
            parsed.learner_id,
            parsed.session_ts,
            "record_learner_k_sleep",
            "learner_k_sleep",
            &value,
        )
        .await
        {
            return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": source_of_truth(["learner_k_sleep", "learner_audit"]),
                "k_sleep": render_k_sleep(&value),
            }),
        )
    }

    pub(crate) async fn call_record_learner_retrieval(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: RecordRetrievalArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid record_learner_retrieval args: {e}"))
            }
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "record_learner_retrieval requires RocksDbTeleologicalStore.",
            );
        };
        let state_at_retrieval = if let Some(session_ts) = parsed.session_ts {
            match store
                .get_learner_state_vector(parsed.learner_id, session_ts)
                .await
            {
                Ok(Some(state)) => state,
                Ok(None) => {
                    return self
                        .tool_error(id, "No learner state vector found for learnerId+sessionTs")
                }
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_state_vector failed: {e}"))
                }
            }
        } else {
            let Some(components) = parsed.components else {
                return self.tool_error(
                    id,
                    "record_learner_retrieval requires either sessionTs or stateVector+components",
                );
            };
            LearnerStateVector {
                learner_id: parsed.learner_id,
                session_ts: parsed.ts,
                values: parsed.state_vector,
                components: components.into(),
                context: BTreeMap::from([("source".into(), "record_learner_retrieval".into())]),
            }
        };
        let log = LearnerRetrievalLog {
            learner_id: parsed.learner_id,
            trace_id: parsed.trace_id,
            ts: parsed.ts,
            correct: parsed.correct,
            score: parsed.score,
            state_at_retrieval,
        };
        if let Err(e) = log.validate() {
            return self.tool_error(id, &format!("LearnerRetrievalLog validation failed: {e}"));
        }
        if let Err(e) = store.store_learner_retrieval_log(&log).await {
            return self.tool_error(id, &format!("store_learner_retrieval_log failed: {e}"));
        }
        if let Err(e) = store_learner_audit(
            store,
            parsed.learner_id,
            parsed.ts,
            "record_learner_retrieval",
            "learner_retrieval_log",
            &log,
        )
        .await
        {
            return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": source_of_truth(["learner_retrieval_log", "learner_audit"]),
                "retrieval_log": render_retrieval_log(&log),
            }),
        )
    }

    pub(crate) async fn call_upsert_goal_centroid(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: GoalCentroidArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("Invalid upsert_goal_centroid args: {e}"))
            }
        };
        let modality = match LearnerModality::parse(&parsed.modality) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("{e}")),
        };
        let centroid = GoalCentroid {
            skill_id: parsed.skill_id,
            modality,
            vector: parsed.vector,
        };
        if let Err(e) = centroid.validate() {
            return self.tool_error(id, &format!("GoalCentroid validation failed: {e}"));
        }
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "upsert_goal_centroid requires RocksDbTeleologicalStore.",
            );
        };
        if let Err(e) = store.store_goal_centroid(&centroid).await {
            return self.tool_error(id, &format!("store_goal_centroid failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": source_of_truth(["goal_centroids"]),
                "goal_centroid": render_goal_centroid(&centroid, false),
            }),
        )
    }

    pub(crate) async fn call_get_goal_distance(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: GoalDistanceArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Invalid get_goal_distance args: {e}")),
        };
        let modality = match LearnerModality::parse(&parsed.modality) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("{e}")),
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(id, "get_goal_distance requires RocksDbTeleologicalStore.");
        };
        let centroid = match store.get_goal_centroid(parsed.skill_id, modality).await {
            Ok(Some(v)) => v,
            Ok(None) => return self.tool_error(id, "Goal centroid not found for skill/modality"),
            Err(e) => return self.tool_error(id, &format!("get_goal_centroid failed: {e}")),
        };
        let fingerprint = match store
            .get_learner_fingerprint(parsed.learner_id, parsed.session_ts)
            .await
        {
            Ok(Some(v)) => v,
            Ok(None) => return self.tool_error(id, "Learner fingerprint not found for session"),
            Err(e) => return self.tool_error(id, &format!("get_learner_fingerprint failed: {e}")),
        };
        let Some(embedding) = fingerprint
            .modality_embeddings
            .iter()
            .find(|embedding| embedding.modality == modality)
        else {
            return self.tool_error(id, "Learner fingerprint has no embedding for modality");
        };
        if embedding.vector.len() != centroid.vector.len() {
            return self.tool_error(
                id,
                &format!(
                    "Goal centroid dimension {} does not match learner embedding dimension {}",
                    centroid.vector.len(),
                    embedding.vector.len()
                ),
            );
        }
        let cosine = cosine_similarity(&embedding.vector, &centroid.vector);
        let euclidean = euclidean_distance(&embedding.vector, &centroid.vector);
        let transfer_t = match compute_transfer_t(&embedding.vector, &centroid.vector, 1.0) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("compute_transfer_t failed: {e}")),
        };
        let mut persisted_goal_state = json!(null);
        if parsed.persist_goal_state {
            let goal = LearnerGoalState {
                learner_id: parsed.learner_id,
                skill_id: parsed.skill_id,
                state_vector: fingerprint.state_vector.clone(),
            };
            if let Err(e) = store.store_learner_goal_state(&goal).await {
                return self.tool_error(id, &format!("store_learner_goal_state failed: {e}"));
            }
            if let Err(e) = store_learner_audit(
                store,
                parsed.learner_id,
                parsed.session_ts,
                "get_goal_distance",
                "learner_goal_states",
                &goal,
            )
            .await
            {
                return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
            }
            persisted_goal_state = render_goal_state(&goal);
        }
        self.tool_result(
            id,
            json!({
                "found": true,
                "source_of_truth": source_of_truth(["goal_centroids", "fingerprints_learner", "learner_goal_states", "learner_audit"]),
                "learner_id": parsed.learner_id.to_string(),
                "skill_id": parsed.skill_id.to_string(),
                "session_ts": parsed.session_ts,
                "modality": modality.as_str(),
                "cosine_similarity": cosine,
                "cosine_distance": 1.0 - cosine,
                "euclidean_distance": euclidean,
                "transfer_t_lambda_1": transfer_t,
                "persisted_goal_state": persisted_goal_state,
            }),
        )
    }

    pub(crate) async fn call_compile_learner_constellation(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: CompileConstellationArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("Invalid compile_learner_constellation args: {e}"),
                )
            }
        };
        if parsed.session_ts_list.is_empty() {
            return self.tool_error(
                id,
                "compile_learner_constellation requires at least one session timestamp",
            );
        }
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "compile_learner_constellation requires RocksDbTeleologicalStore.",
            );
        };
        let mut fingerprints = Vec::with_capacity(parsed.session_ts_list.len());
        for session_ts in &parsed.session_ts_list {
            match store
                .get_learner_fingerprint(parsed.learner_id, *session_ts)
                .await
            {
                Ok(Some(fingerprint)) => fingerprints.push(fingerprint),
                Ok(None) => {
                    return self.tool_error(
                        id,
                        &format!("Learner fingerprint not found for sessionTs={session_ts}"),
                    )
                }
                Err(e) => {
                    return self.tool_error(id, &format!("get_learner_fingerprint failed: {e}"))
                }
            }
        }
        let constellation = match compile_constellation_from_fingerprints(
            parsed.learner_id,
            &parsed,
            &fingerprints,
        ) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &e),
        };
        if let Err(e) = store.store_learner_constellation(&constellation).await {
            return self.tool_error(id, &format!("store_learner_constellation failed: {e}"));
        }
        if let Err(e) = store_learner_audit(
            store,
            parsed.learner_id,
            constellation.session_ts_end,
            "compile_learner_constellation",
            "learner_constellations",
            &constellation,
        )
        .await
        {
            return self.tool_error(id, &format!("store_learner_audit_entry failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": source_of_truth(["learner_constellations", "learner_audit"]),
                "constellation": render_constellation(&constellation),
            }),
        )
    }

    pub(crate) async fn call_resolve_learner_retrieval_policy(
        &self,
        id: Option<JsonRpcId>,
        args: serde_json::Value,
    ) -> JsonRpcResponse {
        let parsed: ResolveRetrievalPolicyArgs = match serde_json::from_value(args) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(
                    id,
                    &format!("Invalid resolve_learner_retrieval_policy args: {e}"),
                )
            }
        };
        let Some(store) = self.rocksdb_store_or_error(id.clone()) else {
            return self.tool_error(
                id,
                "resolve_learner_retrieval_policy requires RocksDbTeleologicalStore.",
            );
        };
        match store.get_learner_profile(parsed.learner_id).await {
            Ok(Some(profile)) if profile.consent_state == "revoked" => {
                return self.tool_error(
                    id,
                    "resolve_learner_retrieval_policy refuses to use learner state after consent revocation",
                )
            }
            Ok(_) => {}
            Err(e) => return self.tool_error(id, &format!("get_learner_profile failed: {e}")),
        }
        let state = match store
            .get_learner_state_vector(parsed.learner_id, parsed.session_ts)
            .await
        {
            Ok(Some(state)) => state,
            Ok(None) => return self.tool_error(
                id,
                "No learner state vector found in CF_LEARNER_STATE_HISTORY for learnerId+sessionTs",
            ),
            Err(e) => return self.tool_error(id, &format!("get_learner_state_vector failed: {e}")),
        };
        let base = parsed.base_weight_profile.as_deref();
        let selection = match select_state_conditioned_weight_profile(base, &state.components) {
            Ok(v) => v,
            Err(e) => {
                return self.tool_error(id, &format!("state-conditioned profile failed: {e}"))
            }
        };
        let weights = if parsed.include_weights {
            match get_effective_weight_profile(&selection.selected_profile) {
                Ok(v) => Some(v),
                Err(e) => {
                    return self.tool_error(
                        id,
                        &format!(
                            "get_effective_weight_profile failed for {}: {e}",
                            selection.selected_profile
                        ),
                    )
                }
            }
        } else {
            None
        };
        self.tool_result(
            id,
            render_retrieval_policy(
                parsed.learner_id,
                parsed.session_ts,
                &state,
                &selection,
                weights.as_ref(),
            ),
        )
    }

    fn rocksdb_store_or_error(&self, _id: Option<JsonRpcId>) -> Option<&RocksDbTeleologicalStore> {
        self.teleological_store
            .as_any()
            .downcast_ref::<RocksDbTeleologicalStore>()
    }
}

fn render_profile(profile: &LearnerProfile) -> serde_json::Value {
    json!({
        "learner_id": profile.learner_id.to_string(),
        "handle": profile.handle,
        "consent_state": profile.consent_state,
        "modalities_enabled": profile.modalities_enabled.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
        "calibration_session_ts": profile.calibration_session_ts,
    })
}

fn render_fingerprint(fingerprint: &LearnerFingerprint) -> serde_json::Value {
    json!({
        "learner_id": fingerprint.learner_id.to_string(),
        "session_ts": fingerprint.session_ts,
        "observation_envelopes": fingerprint.observation_envelopes.len(),
        "modality_embeddings": fingerprint.modality_embeddings.iter().map(|m| json!({
            "modality": m.modality.as_str(),
            "vector_len": m.vector.len(),
            "scalar": m.scalar,
        })).collect::<Vec<_>>(),
        "state_vector_len": fingerprint.state_vector.values.len(),
        "components": fingerprint.state_vector.components,
    })
}

fn render_m_trace(trace: &context_graph_core::learner::LearnerMTrace) -> serde_json::Value {
    json!({
        "learner_id": trace.learner_id.to_string(),
        "trace_id": trace.trace_id.to_string(),
        "m_value": trace.m_value,
        "last_update_ts": trace.last_update_ts,
        "decay_rate": trace.decay_rate,
        "num_retrievals": trace.num_retrievals,
        "next_review_ts": trace.next_review_ts,
    })
}

fn render_state_vector(state: &LearnerStateVector, include_values: bool) -> serde_json::Value {
    let mut out = json!({
        "learner_id": state.learner_id.to_string(),
        "session_ts": state.session_ts,
        "values_len": state.values.len(),
        "components": {
            "plasticity_window": state.components.plasticity_window,
            "hrv_coherence": state.components.hrv_coherence,
            "valence": state.components.valence,
            "arousal": state.components.arousal,
            "stress_floor": state.components.stress_floor,
            "k_sleep": state.components.k_sleep,
        },
        "context": state.context,
    });
    if include_values {
        out["values"] = json!(state.values);
    }
    out
}

fn render_delta_log(log: &LearnerDeltaLog) -> serde_json::Value {
    json!({
        "learner_id": log.learner_id.to_string(),
        "session_ts": log.session_ts,
        "l": log.computation.l,
        "diagnostic_state": log.computation.diagnostic_state.as_str(),
        "delta_s": log.computation.delta_s,
        "delta_c": log.computation.delta_c,
        "delta_e": log.computation.delta_e,
        "provenance": {
            "computation_id": log.provenance.computation_id.to_string(),
            "parent_observation_ids": log.provenance.parent_observation_ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
            "threshold_version": log.provenance.threshold_version,
            "output_sha256": log.provenance.output_sha256,
        }
    })
}

fn render_k_sleep(value: &LearnerKSleep) -> serde_json::Value {
    json!({
        "learner_id": value.learner_id.to_string(),
        "session_ts": value.session_ts,
        "k": value.k,
        "slow_wave_minutes": value.slow_wave_minutes,
    })
}

fn render_retrieval_log(log: &LearnerRetrievalLog) -> serde_json::Value {
    json!({
        "learner_id": log.learner_id.to_string(),
        "trace_id": log.trace_id.to_string(),
        "ts": log.ts,
        "correct": log.correct,
        "score": log.score,
        "state_at_retrieval": render_state_vector(&log.state_at_retrieval, false),
    })
}

fn render_goal_centroid(centroid: &GoalCentroid, include_vector: bool) -> serde_json::Value {
    let mut out = json!({
        "skill_id": centroid.skill_id.to_string(),
        "modality": centroid.modality.as_str(),
        "vector_len": centroid.vector.len(),
    });
    if include_vector {
        out["vector"] = json!(centroid.vector);
    }
    out
}

fn render_goal_state(goal: &LearnerGoalState) -> serde_json::Value {
    json!({
        "learner_id": goal.learner_id.to_string(),
        "skill_id": goal.skill_id.to_string(),
        "state_vector": render_state_vector(&goal.state_vector, false),
    })
}

fn render_constellation(constellation: &LearnerConstellation) -> serde_json::Value {
    json!({
        "learner_id": constellation.learner_id.to_string(),
        "selector_kind": constellation.selector_kind,
        "label": constellation.label,
        "sample_count": constellation.sample_count,
        "session_ts_start": constellation.session_ts_start,
        "session_ts_end": constellation.session_ts_end,
        "modality_centroids": constellation.modality_centroids.iter().map(|centroid| json!({
            "modality": centroid.modality.as_str(),
            "vector_len": centroid.vector.len(),
            "scalar_mean": centroid.scalar_mean,
            "sample_count": centroid.sample_count,
        })).collect::<Vec<_>>(),
        "state_centroid": render_state_vector(&constellation.state_centroid, false),
        "created_at": constellation.created_at,
    })
}

fn render_retrieval_policy(
    learner_id: Uuid,
    session_ts: u64,
    state: &LearnerStateVector,
    selection: &StateConditionedProfileSelection,
    weights: Option<&[f32; 14]>,
) -> serde_json::Value {
    let mut out = json!({
        "source_of_truth": source_of_truth(["learner_state_history"]),
        "learner_id": learner_id.to_string(),
        "session_ts": session_ts,
        "state_vector_len": state.values.len(),
        "components": {
            "plasticity_window": state.components.plasticity_window,
            "hrv_coherence": state.components.hrv_coherence,
            "valence": state.components.valence,
            "arousal": state.components.arousal,
            "stress_floor": state.components.stress_floor,
            "k_sleep": state.components.k_sleep,
        },
        "policy": {
            "base_profile": selection.base_profile.as_str(),
            "selected_profile": selection.selected_profile.as_str(),
            "reason": selection.reason,
        },
    });
    if let Some(weights) = weights {
        out["weights"] = json!(weights_by_embedder(weights));
    }
    out
}

fn weights_by_embedder(weights: &[f32; 14]) -> serde_json::Value {
    let items = weights
        .iter()
        .enumerate()
        .map(|(idx, weight)| {
            json!({
                "index": idx,
                "embedder": context_graph_core::weights::space_name(idx),
                "weight": weight,
            })
        })
        .collect::<Vec<_>>();
    json!(items)
}

async fn learner_counts(
    store: &RocksDbTeleologicalStore,
) -> context_graph_core::error::CoreResult<serde_json::Value> {
    Ok(json!({
        "learner_profile": store.count_learner_profiles().await?,
        "learner_constellations": store.count_learner_constellations().await?,
        "fingerprints_learner": store.count_learner_fingerprints().await?,
        "learner_m_per_trace": store.count_learner_m_traces().await?,
        "learner_state_history": store.count_learner_state_history().await?,
        "learner_goal_states": store.count_learner_goal_states().await?,
        "learner_retrieval_log": store.count_learner_retrieval_logs().await?,
        "learner_k_sleep": store.count_learner_k_sleep().await?,
        "goal_centroids": store.count_goal_centroids().await?,
        "learner_delta_log": store.count_learner_delta_logs().await?,
        "learner_audit": store.count_learner_audit_entries().await?,
    }))
}

fn k_sleep_from_slow_wave_minutes(minutes: u16) -> f32 {
    0.5 + (minutes.min(120) as f32 / 120.0)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let mut dot = 0.0f64;
    let mut norm_a = 0.0f64;
    let mut norm_b = 0.0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let x = *x as f64;
        let y = *y as f64;
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 1e-12 || norm_b <= 1e-12 {
        0.0
    } else {
        (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0) as f32
    }
}

fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (*x as f64 - *y as f64).powi(2))
        .sum::<f64>()
        .sqrt() as f32
}

fn compile_constellation_from_fingerprints(
    learner_id: Uuid,
    args: &CompileConstellationArgs,
    fingerprints: &[LearnerFingerprint],
) -> Result<LearnerConstellation, String> {
    if fingerprints.is_empty() {
        return Err("compile_learner_constellation requires at least one fingerprint".into());
    }
    let first_state_len = fingerprints[0].state_vector.values.len();
    let mut state_values = vec![0.0f32; first_state_len];
    let mut components = LearnerStateComponents {
        plasticity_window: 0.0,
        hrv_coherence: 0.0,
        valence: 0.0,
        arousal: 0.0,
        stress_floor: 0.0,
        k_sleep: 0.0,
    };
    let mut session_ts_start = u64::MAX;
    let mut session_ts_end = 0u64;
    let mut by_modality: BTreeMap<LearnerModality, Vec<&ModalityEmbedding>> = BTreeMap::new();

    for fingerprint in fingerprints {
        if fingerprint.learner_id != learner_id {
            return Err("all fingerprints must belong to the requested learner_id".into());
        }
        if fingerprint.state_vector.values.len() != first_state_len {
            return Err("all learner state vectors must have the same dimension".into());
        }
        session_ts_start = session_ts_start.min(fingerprint.session_ts);
        session_ts_end = session_ts_end.max(fingerprint.session_ts);
        for (dst, src) in state_values
            .iter_mut()
            .zip(fingerprint.state_vector.values.iter())
        {
            *dst += *src;
        }
        components.plasticity_window += fingerprint.state_vector.components.plasticity_window;
        components.hrv_coherence += fingerprint.state_vector.components.hrv_coherence;
        components.valence += fingerprint.state_vector.components.valence;
        components.arousal += fingerprint.state_vector.components.arousal;
        components.stress_floor += fingerprint.state_vector.components.stress_floor;
        components.k_sleep += fingerprint.state_vector.components.k_sleep;

        for embedding in &fingerprint.modality_embeddings {
            if embedding.modality != LearnerModality::SelfReport {
                by_modality
                    .entry(embedding.modality)
                    .or_default()
                    .push(embedding);
            }
        }
    }

    let sample_count = fingerprints.len() as f32;
    for value in &mut state_values {
        *value /= sample_count;
    }
    components.plasticity_window /= sample_count;
    components.hrv_coherence /= sample_count;
    components.valence /= sample_count;
    components.arousal /= sample_count;
    components.stress_floor /= sample_count;
    components.k_sleep /= sample_count;

    let mut modality_centroids = Vec::new();
    for (modality, embeddings) in by_modality {
        if embeddings.is_empty() {
            continue;
        }
        let dim = embeddings[0].vector.len();
        if dim == 0 {
            return Err(format!(
                "modality {} has an empty embedding vector",
                modality.as_str()
            ));
        }
        let mut mean = vec![0.0f32; dim];
        let mut kept = 0u32;
        let mut scalar_sum = 0.0f32;
        let mut scalar_count = 0u32;
        for embedding in embeddings {
            if embedding.vector.len() != dim {
                return Err(format!(
                    "modality {} has inconsistent embedding dimensions",
                    modality.as_str()
                ));
            }
            if let Some(unit) = l2_normalize(&embedding.vector) {
                for (dst, src) in mean.iter_mut().zip(unit.iter()) {
                    *dst += *src;
                }
                kept += 1;
            }
            if let Some(scalar) = embedding.scalar {
                scalar_sum += scalar;
                scalar_count += 1;
            }
        }
        if kept == 0 {
            return Err(format!(
                "modality {} has no non-zero vectors for centroid construction",
                modality.as_str()
            ));
        }
        for value in &mut mean {
            *value /= kept as f32;
        }
        let Some(vector) = l2_normalize(&mean) else {
            return Err(format!(
                "modality {} mean vector collapsed to zero",
                modality.as_str()
            ));
        };
        modality_centroids.push(LearnerModalityCentroid {
            modality,
            vector,
            scalar_mean: (scalar_count > 0).then_some(scalar_sum / scalar_count as f32),
            sample_count: kept,
        });
    }

    let state_centroid = LearnerStateVector {
        learner_id,
        session_ts: session_ts_end,
        values: state_values,
        components,
        context: BTreeMap::from([
            ("source".into(), "compile_learner_constellation".into()),
            (
                "aggregation".into(),
                "mean_of_l2_normalized_modalities".into(),
            ),
        ]),
    };
    let constellation = LearnerConstellation {
        learner_id,
        selector_kind: args
            .selector_kind
            .unwrap_or(LEARNER_BASELINE_SELECTOR_REGULATED),
        label: args.label.clone(),
        sample_count: fingerprints.len() as u32,
        session_ts_start,
        session_ts_end,
        modality_centroids,
        state_centroid,
        created_at: Utc::now(),
    };
    constellation
        .validate()
        .map_err(|e| format!("LearnerConstellation validation failed: {e}"))?;
    Ok(constellation)
}

fn l2_normalize(values: &[f32]) -> Option<Vec<f32>> {
    let norm = values
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm < 1e-6 {
        return None;
    }
    Some(values.iter().map(|v| (*v as f64 / norm) as f32).collect())
}

async fn store_learner_audit<T: Serialize>(
    store: &RocksDbTeleologicalStore,
    learner_id: Uuid,
    ts: u64,
    action: &str,
    target_cf: &str,
    result: &T,
) -> context_graph_core::error::CoreResult<()> {
    let entry = LearnerAuditEntry {
        audit_id: Uuid::new_v4(),
        learner_id,
        ts,
        action: action.into(),
        target_cf: target_cf.into(),
        result_sha256: sha256_json(result)?,
        parent_audit_id: None,
    };
    store.store_learner_audit_entry(&entry).await
}

fn content_embedder_specs_json() -> Vec<serde_json::Value> {
    ModelId::production()
        .iter()
        .map(|model| {
            json!({
                "slot": content_slot_number(*model),
                "id": model.as_str(),
                "model_name": content_model_name(*model),
                "model_repo": model.model_repo(),
                "model_path": model.directory_name(),
                "native_dimension": model.dimension(),
                "projected_dimension": model.projected_dimension(),
                "max_tokens": model.max_tokens(),
                "custom": model.is_custom(),
            })
        })
        .collect()
}

fn content_slot_number(model: ModelId) -> u8 {
    match model {
        ModelId::Semantic => 1,
        ModelId::TemporalRecent => 2,
        ModelId::TemporalPeriodic => 3,
        ModelId::TemporalPositional => 4,
        ModelId::Causal => 5,
        ModelId::Sparse => 6,
        ModelId::Code => 7,
        ModelId::Graph => 8,
        ModelId::Hdc => 9,
        ModelId::Contextual => 10,
        ModelId::Kepler => 11,
        ModelId::LateInteraction => 12,
        ModelId::Splade => 13,
        ModelId::BgeM3Dense => 14,
        ModelId::Entity => 11,
    }
}

fn content_model_name(model: ModelId) -> &'static str {
    match model {
        ModelId::Semantic => "intfloat/e5-large-v2",
        ModelId::TemporalRecent => "exponential recency basis",
        ModelId::TemporalPeriodic => "Fourier periodic basis",
        ModelId::TemporalPositional => "sinusoidal positional basis",
        ModelId::Causal => "nomic-ai/nomic-embed-text-v1.5",
        ModelId::Sparse => "naver/splade-cocondenser-ensembledistil",
        ModelId::Code => "Qodo/Qodo-Embed-1-1.5B",
        ModelId::Graph => "intfloat/e5-large-v2 graph/sentence embedding",
        ModelId::Hdc => "hyperdimensional computing encoder",
        ModelId::Contextual => "intfloat/e5-base-v2 contextual paraphrase",
        ModelId::Entity => "legacy sentence-transformers/all-MiniLM-L6-v2",
        ModelId::Kepler => "THU-KEG/KEPLER-Wiki5M-KE",
        ModelId::LateInteraction => "colbert-ir/colbertv2.0",
        ModelId::Splade => "prithivida/Splade_PP_en_v1",
        ModelId::BgeM3Dense => "BAAI/bge-m3 dense head",
    }
}

fn source_of_truth<const N: usize>(column_families: [&str; N]) -> serde_json::Value {
    json!({
        "backend": "rocksdb",
        "format": "version_byte + bincode",
        "column_families": column_families.to_vec(),
    })
}

fn signal_to_embedder_input(
    signal: SignalArgs,
) -> Result<(LearnerEmbedderSlot, LearnerEmbedderInput, String), String> {
    let modality = LearnerModality::parse(&signal.modality).map_err(|e| e.to_string())?;
    let slot = LearnerEmbedderSlot::from_modality(modality).ok_or_else(|| {
        format!(
            "modality {} has no E15-E21 learner embedder",
            modality.as_str()
        )
    })?;
    let (input, raw_hash) = if let Some(text) = signal.text {
        if modality != LearnerModality::AffectText {
            return Err(format!(
                "text signal is only valid for affect_text, got {}",
                modality.as_str()
            ));
        }
        let raw_hash = sha256_bytes(text.as_bytes());
        (LearnerEmbedderInput::Text { content: text }, raw_hash)
    } else if !signal.features.is_empty() {
        let raw_hash = sha256_bytes(format!("{:?}", signal.features).as_bytes());
        (
            LearnerEmbedderInput::Features {
                modality,
                values: signal.features,
            },
            raw_hash,
        )
    } else {
        let raw_hash = sha256_bytes(format!("{:?}", signal.samples).as_bytes());
        (
            LearnerEmbedderInput::Samples {
                modality,
                samples: signal.samples,
                sample_rate_hz: signal.sample_rate_hz.unwrap_or(1),
                channels: signal.channels.unwrap_or(1),
            },
            raw_hash,
        )
    };
    Ok((slot, input, raw_hash))
}

fn default_consent() -> String {
    "consented-local-first".into()
}

fn default_modalities() -> Vec<String> {
    vec!["affect_text".into(), "self_report".into()]
}

fn default_modality() -> String {
    "affect_text".into()
}

fn default_preprocess() -> String {
    "phase0-preprocess-v1".into()
}

fn default_embedder() -> String {
    "phase0-deterministic-v1".into()
}

fn default_threshold() -> String {
    "thresholds-default-pending-calibration-v1".into()
}

fn default_models_root() -> PathBuf {
    PathBuf::from("models")
}

fn default_calibration_root() -> PathBuf {
    PathBuf::from("data/utl_calibration")
}

fn default_constellation_label() -> String {
    "regulated-baseline".into()
}

fn default_true() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn components(
        plasticity_window: f32,
        hrv_coherence: f32,
        valence: f32,
        arousal: f32,
        stress_floor: f32,
    ) -> LearnerStateComponents {
        LearnerStateComponents {
            plasticity_window,
            hrv_coherence,
            valence,
            arousal,
            stress_floor,
            k_sleep: 1.0,
        }
    }

    fn fingerprint(
        learner_id: Uuid,
        session_ts: u64,
        components: LearnerStateComponents,
        vector: Vec<f32>,
    ) -> LearnerFingerprint {
        let observation_id = Uuid::new_v4();
        let raw = format!("policy-fsv:{learner_id}:{session_ts}:{vector:?}");
        let envelope = ObservationEnvelope::new(
            observation_id,
            learner_id,
            session_ts,
            LearnerModality::AffectText,
            "consented-local-first".into(),
            sha256_bytes(raw.as_bytes()),
            "policy-fsv-preprocess-v1".into(),
            "policy-fsv-embedder-v1".into(),
            "policy-fsv-threshold-v1".into(),
            Vec::new(),
        )
        .unwrap();
        let state_vector = LearnerStateVector {
            learner_id,
            session_ts,
            values: vec![
                components.plasticity_window,
                components.hrv_coherence,
                components.valence,
                components.arousal,
                components.stress_floor,
                components.k_sleep,
            ],
            components,
            context: BTreeMap::from([("test_case".into(), "retrieval_policy_fsv".into())]),
        };
        let fingerprint = LearnerFingerprint {
            learner_id,
            session_ts,
            observation_envelopes: vec![envelope],
            modality_embeddings: vec![ModalityEmbedding {
                modality: LearnerModality::AffectText,
                vector,
                scalar: None,
                source_observation_id: observation_id,
            }],
            state_vector,
        };
        fingerprint.validate().unwrap();
        fingerprint
    }

    #[tokio::test]
    async fn retrieval_policy_fsv_happy_path_and_edges() {
        let tempdir = TempDir::new().unwrap();
        let store = RocksDbTeleologicalStore::open(tempdir.path()).unwrap();
        let learner_id = Uuid::from_u128(0x12345678_1234_4234_9234_123456789abc);

        println!("SOURCE OF TRUTH: RocksDB CF_LEARNER_STATE_HISTORY");
        println!(
            "HAPPY BEFORE state_history={} fingerprints={}",
            store.count_learner_state_history().await.unwrap(),
            store.count_learner_fingerprints().await.unwrap()
        );

        let repair_session = 1_900_000_001;
        store
            .store_learner_fingerprint(&fingerprint(
                learner_id,
                repair_session,
                components(0.55, 0.40, 0.10, 0.20, 0.70),
                vec![0.2, 0.4, 0.8],
            ))
            .await
            .unwrap();
        let repair_state = store
            .get_learner_state_vector(learner_id, repair_session)
            .await
            .unwrap()
            .expect("state vector must be physically present after write");
        let repair_selection = select_state_conditioned_weight_profile(
            Some("semantic_search"),
            &repair_state.components,
        )
        .unwrap();
        let repair_weights =
            get_effective_weight_profile(&repair_selection.selected_profile).unwrap();
        println!(
            "HAPPY AFTER state_history={} selected={} reason={} e1={} e10={} e14={}",
            store.count_learner_state_history().await.unwrap(),
            repair_selection.selected_profile,
            repair_selection.reason,
            repair_weights[0],
            repair_weights[9],
            repair_weights[13]
        );
        assert_eq!(repair_selection.selected_profile, "affect_repair");
        assert!((repair_weights.iter().sum::<f32>() - 1.0).abs() < 0.02);

        println!(
            "EDGE LOW_VALENCE BEFORE state_history={}",
            store.count_learner_state_history().await.unwrap()
        );
        let priming_session = 1_900_000_002;
        store
            .store_learner_fingerprint(&fingerprint(
                learner_id,
                priming_session,
                components(0.60, 0.70, -0.50, 0.10, 0.30),
                vec![0.1, 0.3, 0.9],
            ))
            .await
            .unwrap();
        let priming_state = store
            .get_learner_state_vector(learner_id, priming_session)
            .await
            .unwrap()
            .expect("priming state vector must be physically present");
        let priming_selection = select_state_conditioned_weight_profile(
            Some("multilingual_search"),
            &priming_state.components,
        )
        .unwrap();
        println!(
            "EDGE LOW_VALENCE AFTER state_history={} base={} selected={}",
            store.count_learner_state_history().await.unwrap(),
            priming_selection.base_profile,
            priming_selection.selected_profile
        );
        assert_eq!(priming_selection.base_profile, "multilingual_search");
        assert_eq!(priming_selection.selected_profile, "affect_priming");

        let missing_before = store.count_learner_state_history().await.unwrap();
        println!("EDGE MISSING BEFORE state_history={missing_before}");
        let missing = store
            .get_learner_state_vector(learner_id, 9_999_999_999)
            .await
            .unwrap();
        let missing_after = store.count_learner_state_history().await.unwrap();
        println!(
            "EDGE MISSING AFTER state_history={missing_after} found={}",
            missing.is_some()
        );
        assert!(missing.is_none());
        assert_eq!(missing_after, missing_before);

        let invalid_before = store.count_learner_state_history().await.unwrap();
        println!("EDGE INVALID_PROFILE BEFORE state_history={invalid_before}");
        let invalid = select_state_conditioned_weight_profile(
            Some("missing_profile"),
            &repair_state.components,
        );
        let invalid_after = store.count_learner_state_history().await.unwrap();
        println!(
            "EDGE INVALID_PROFILE AFTER state_history={invalid_after} error={:?}",
            invalid.as_ref().err()
        );
        assert!(invalid.is_err());
        assert_eq!(invalid_after, invalid_before);
    }
}
