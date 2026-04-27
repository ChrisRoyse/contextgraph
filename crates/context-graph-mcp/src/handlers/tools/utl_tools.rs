//! UTL learner-state tool handlers.

use std::collections::{BTreeMap, BTreeSet};

use context_graph_core::learner::{
    compute_delta_c, compute_delta_e, compute_delta_s_from_text, compute_utl_l, sha256_bytes,
    sha256_json, update_m_trace, ComputationEnvelope, LearnerDeltaLog, LearnerFingerprint,
    LearnerModality, LearnerProfile, LearnerStateComponents, LearnerStateVector, ModalityEmbedding,
    ObservationEnvelope,
};
use context_graph_storage::teleological::RocksDbTeleologicalStore;
use serde::Deserialize;
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
    components: ComponentsArgs,
    #[serde(default)]
    embeddings: Vec<EmbeddingArgs>,
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
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "learner_id": profile.learner_id.to_string(),
                "source_of_truth": source_of_truth(["learner_profile"]),
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
        let modality = match LearnerModality::parse(&parsed.modality) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("{e}")),
        };
        let observation_id = Uuid::new_v4();
        let envelope = match ObservationEnvelope::new(
            observation_id,
            parsed.learner_id,
            parsed.session_ts,
            modality,
            parsed.consent_state,
            sha256_bytes(parsed.raw_text.as_bytes()),
            parsed.preprocessing_version,
            parsed.embedder_version,
            parsed.threshold_version,
            Vec::new(),
        ) {
            Ok(v) => v,
            Err(e) => return self.tool_error(id, &format!("Observation validation failed: {e}")),
        };
        let components: LearnerStateComponents = parsed.components.into();
        let values = if parsed.state_vector.is_empty() {
            vec![
                components.plasticity_window,
                components.hrv_coherence,
                components.valence,
                components.arousal,
                components.stress_floor,
                components.k_sleep,
            ]
        } else {
            parsed.state_vector
        };
        let state_vector = LearnerStateVector {
            learner_id: parsed.learner_id,
            session_ts: parsed.session_ts,
            values,
            components,
            context: BTreeMap::new(),
        };
        let mut embeddings = Vec::with_capacity(parsed.embeddings.len());
        for embedding in parsed.embeddings {
            let modality = match LearnerModality::parse(&embedding.modality) {
                Ok(v) => v,
                Err(e) => return self.tool_error(id, &format!("{e}")),
            };
            embeddings.push(ModalityEmbedding {
                modality,
                vector: embedding.vector,
                scalar: embedding.scalar,
                source_observation_id: observation_id,
            });
        }
        if embeddings.is_empty() {
            embeddings.push(ModalityEmbedding {
                modality,
                vector: state_vector.values.clone(),
                scalar: None,
                source_observation_id: observation_id,
            });
        }
        let fingerprint = LearnerFingerprint {
            learner_id: parsed.learner_id,
            session_ts: parsed.session_ts,
            observation_envelopes: vec![envelope],
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
        if let Err(e) = store.store_learner_fingerprint(&fingerprint).await {
            return self.tool_error(id, &format!("store_learner_fingerprint failed: {e}"));
        }
        self.tool_result(
            id,
            json!({
                "status": "stored",
                "source_of_truth": source_of_truth(["fingerprints_learner", "learner_state_history"]),
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
                "source_of_truth": source_of_truth(["learner_delta_log", "learner_m_per_trace"]),
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

fn source_of_truth<const N: usize>(column_families: [&str; N]) -> serde_json::Value {
    json!({
        "backend": "rocksdb",
        "format": "version_byte + bincode",
        "column_families": column_families.to_vec(),
    })
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

fn one() -> f32 {
    1.0
}
