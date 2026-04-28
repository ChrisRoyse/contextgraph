//! Learner-state UTL primitives.
//!
//! This module is the Phase-0/Phase-1 learner layer described in
//! `docs/07_context_graph_integration.md`. It deliberately keeps learner-state
//! records separate from the content `TeleologicalFingerprint` wire format.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};

/// Current on-disk version byte for learner-layer records.
pub const LEARNER_RECORD_VERSION: u8 = 1;

/// E15-E21 learner modality slots.
pub const LEARNER_MODALITY_COUNT: usize = 7;

/// Selector byte for a learner's regulated-state baseline constellation.
pub const LEARNER_BASELINE_SELECTOR_REGULATED: u8 = 1;

/// Maximum number of f32 values in one learner state vector.
pub const MAX_LEARNER_STATE_VECTOR_VALUES: usize = 16_384;

/// Maximum number of modality embeddings attached to one observation.
pub const MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION: usize = 16;

/// Maximum number of scores accepted for a recent-outcome window.
pub const MAX_RECENT_SCORE_WINDOW: usize = 1024;

/// Maximum length for handle, version, consent, and label fields.
pub const MAX_LEARNER_LABEL_CHARS: usize = 256;

/// Learner modalities mapped to the planned E15-E21 slots.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LearnerModality {
    AffectSpeech,
    AffectFace,
    AffectText,
    Ppg,
    Eda,
    Eeg,
    EegArtifactRobust,
    SelfReport,
}

impl LearnerModality {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AffectSpeech => "affect_speech",
            Self::AffectFace => "affect_face",
            Self::AffectText => "affect_text",
            Self::Ppg => "ppg",
            Self::Eda => "eda",
            Self::Eeg => "eeg",
            Self::EegArtifactRobust => "eeg_artifact_robust",
            Self::SelfReport => "self_report",
        }
    }

    pub fn parse(value: &str) -> CoreResult<Self> {
        match value {
            "affect_speech" => Ok(Self::AffectSpeech),
            "affect_face" => Ok(Self::AffectFace),
            "affect_text" => Ok(Self::AffectText),
            "ppg" => Ok(Self::Ppg),
            "eda" => Ok(Self::Eda),
            "eeg" => Ok(Self::Eeg),
            "eeg_artifact_robust" => Ok(Self::EegArtifactRobust),
            "self_report" => Ok(Self::SelfReport),
            other => Err(CoreError::ValidationError {
                field: "modality".into(),
                message: format!("unknown learner modality: {other}"),
            }),
        }
    }
}

/// Provenance envelope for raw or derived learner observations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObservationEnvelope {
    pub observation_id: Uuid,
    pub learner_id: Uuid,
    pub session_ts: u64,
    pub modality: LearnerModality,
    pub consent_state: String,
    pub raw_sha256: String,
    pub preprocessing_version: String,
    pub embedder_version: String,
    pub threshold_version: String,
    pub parent_observation_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
}

impl ObservationEnvelope {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        observation_id: Uuid,
        learner_id: Uuid,
        session_ts: u64,
        modality: LearnerModality,
        consent_state: String,
        raw_sha256: String,
        preprocessing_version: String,
        embedder_version: String,
        threshold_version: String,
        parent_observation_ids: Vec<Uuid>,
    ) -> CoreResult<Self> {
        let envelope = Self {
            observation_id,
            learner_id,
            session_ts,
            modality,
            consent_state,
            raw_sha256,
            preprocessing_version,
            embedder_version,
            threshold_version,
            parent_observation_ids,
            created_at: Utc::now(),
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn validate(&self) -> CoreResult<()> {
        validate_label(&self.consent_state, "observation.consent_state")?;
        validate_hash(&self.raw_sha256, "observation.raw_sha256")?;
        validate_label(
            &self.preprocessing_version,
            "observation.preprocessing_version",
        )?;
        validate_label(&self.embedder_version, "observation.embedder_version")?;
        validate_label(&self.threshold_version, "observation.threshold_version")?;
        Ok(())
    }
}

/// Provenance envelope for deterministic UTL computations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputationEnvelope {
    pub computation_id: Uuid,
    pub learner_id: Uuid,
    pub session_ts: u64,
    pub parent_observation_ids: Vec<Uuid>,
    pub threshold_version: String,
    pub output_sha256: String,
    pub created_at: DateTime<Utc>,
}

impl ComputationEnvelope {
    pub fn new(
        computation_id: Uuid,
        learner_id: Uuid,
        session_ts: u64,
        parent_observation_ids: Vec<Uuid>,
        threshold_version: String,
        output_sha256: String,
    ) -> CoreResult<Self> {
        let envelope = Self {
            computation_id,
            learner_id,
            session_ts,
            parent_observation_ids,
            threshold_version,
            output_sha256,
            created_at: Utc::now(),
        };
        envelope.validate()?;
        Ok(envelope)
    }

    pub fn validate(&self) -> CoreResult<()> {
        validate_label(&self.threshold_version, "computation.threshold_version")?;
        validate_hash(&self.output_sha256, "computation.output_sha256")?;
        Ok(())
    }
}

/// Learner registry record stored in `CF_LEARNER_PROFILE`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerProfile {
    pub learner_id: Uuid,
    pub handle: String,
    pub consent_state: String,
    pub modalities_enabled: BTreeSet<LearnerModality>,
    pub calibration_session_ts: Option<u64>,
    pub created_at: DateTime<Utc>,
}

impl LearnerProfile {
    pub fn new(
        learner_id: Uuid,
        handle: String,
        consent_state: String,
        modalities_enabled: BTreeSet<LearnerModality>,
        calibration_session_ts: Option<u64>,
    ) -> CoreResult<Self> {
        let profile = Self {
            learner_id,
            handle,
            consent_state,
            modalities_enabled,
            calibration_session_ts,
            created_at: Utc::now(),
        };
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> CoreResult<()> {
        validate_label(&self.handle, "profile.handle")?;
        validate_label(&self.consent_state, "profile.consent_state")?;
        if self.modalities_enabled.len() > LEARNER_MODALITY_COUNT + 1 {
            return Err(CoreError::ValidationError {
                field: "profile.modalities_enabled".into(),
                message: "too many learner modalities".into(),
            });
        }
        Ok(())
    }
}

/// One modality embedding or scalar vector attached to a learner observation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModalityEmbedding {
    pub modality: LearnerModality,
    pub vector: Vec<f32>,
    pub scalar: Option<f32>,
    pub source_observation_id: Uuid,
}

impl ModalityEmbedding {
    pub fn validate(&self) -> CoreResult<()> {
        validate_vector(&self.vector, "modality_embedding.vector")?;
        if let Some(value) = self.scalar {
            validate_unit(value, "modality_embedding.scalar")?;
        }
        Ok(())
    }
}

/// Reduced UTL learner-state subcomponents.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerStateComponents {
    pub plasticity_window: f32,
    pub hrv_coherence: f32,
    pub valence: f32,
    pub arousal: f32,
    pub stress_floor: f32,
    pub k_sleep: f32,
}

impl LearnerStateComponents {
    pub fn validate(&self) -> CoreResult<()> {
        validate_unit(self.plasticity_window, "state.plasticity_window")?;
        validate_unit(self.hrv_coherence, "state.hrv_coherence")?;
        validate_signed_unit(self.valence, "state.valence")?;
        validate_signed_unit(self.arousal, "state.arousal")?;
        validate_unit(self.stress_floor, "state.stress_floor")?;
        if !self.k_sleep.is_finite() || !(0.0..=5.0).contains(&self.k_sleep) {
            return Err(CoreError::ValidationError {
                field: "state.k_sleep".into(),
                message: "must be finite and in [0, 5]".into(),
            });
        }
        Ok(())
    }
}

/// Concatenated learner state vector stored in learner history CFs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerStateVector {
    pub learner_id: Uuid,
    pub session_ts: u64,
    pub values: Vec<f32>,
    pub components: LearnerStateComponents,
    pub context: BTreeMap<String, String>,
}

impl LearnerStateVector {
    pub fn validate(&self) -> CoreResult<()> {
        validate_vector(&self.values, "state_vector.values")?;
        self.components.validate()?;
        for (key, value) in &self.context {
            validate_label(key, "state_vector.context.key")?;
            validate_label(value, "state_vector.context.value")?;
        }
        Ok(())
    }
}

/// Versioned learner fingerprint stored in `CF_FINGERPRINTS_LEARNER`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerFingerprint {
    pub learner_id: Uuid,
    pub session_ts: u64,
    pub observation_envelopes: Vec<ObservationEnvelope>,
    pub modality_embeddings: Vec<ModalityEmbedding>,
    pub state_vector: LearnerStateVector,
}

impl LearnerFingerprint {
    pub fn validate(&self) -> CoreResult<()> {
        if self.observation_envelopes.is_empty() {
            return Err(CoreError::ValidationError {
                field: "fingerprint.observation_envelopes".into(),
                message: "at least one observation envelope is required".into(),
            });
        }
        if self.modality_embeddings.len() > MAX_MODALITY_EMBEDDINGS_PER_OBSERVATION {
            return Err(CoreError::ValidationError {
                field: "fingerprint.modality_embeddings".into(),
                message: "too many modality embeddings".into(),
            });
        }
        for envelope in &self.observation_envelopes {
            envelope.validate()?;
            if envelope.learner_id != self.learner_id || envelope.session_ts != self.session_ts {
                return Err(CoreError::ValidationError {
                    field: "fingerprint.observation_envelopes".into(),
                    message: "observation learner_id/session_ts must match fingerprint".into(),
                });
            }
        }
        for embedding in &self.modality_embeddings {
            embedding.validate()?;
        }
        self.state_vector.validate()?;
        if self.state_vector.learner_id != self.learner_id
            || self.state_vector.session_ts != self.session_ts
        {
            return Err(CoreError::ValidationError {
                field: "fingerprint.state_vector".into(),
                message: "state vector learner_id/session_ts must match fingerprint".into(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeltaSBreakdown {
    pub predicted_actual_distance: f32,
    pub predicted_simulated_distance: f32,
    pub exploration_rate: f32,
    pub gamma: f32,
    pub delta_s: f32,
}

impl DeltaSBreakdown {
    pub fn validate(&self) -> CoreResult<()> {
        validate_unit(
            self.predicted_actual_distance,
            "delta_s.predicted_actual_distance",
        )?;
        validate_unit(
            self.predicted_simulated_distance,
            "delta_s.predicted_simulated_distance",
        )?;
        validate_unit(self.exploration_rate, "delta_s.exploration_rate")?;
        validate_unit(self.gamma, "delta_s.gamma")?;
        validate_unit(self.delta_s, "delta_s.delta_s")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeltaCBreakdown {
    pub outcome_stability: f32,
    pub coefficient_of_variation: f32,
    pub gradient_effectiveness: f32,
    pub hrv_coherence: f32,
    pub panel_agreement: f32,
    pub contradiction: f32,
    pub delta_c: f32,
}

impl DeltaCBreakdown {
    pub fn validate(&self) -> CoreResult<()> {
        validate_unit(self.outcome_stability, "delta_c.outcome_stability")?;
        if !self.coefficient_of_variation.is_finite() || self.coefficient_of_variation < 0.0 {
            return Err(CoreError::ValidationError {
                field: "delta_c.coefficient_of_variation".into(),
                message: "must be finite and >= 0".into(),
            });
        }
        validate_unit(
            self.gradient_effectiveness,
            "delta_c.gradient_effectiveness",
        )?;
        validate_unit(self.hrv_coherence, "delta_c.hrv_coherence")?;
        validate_unit(self.panel_agreement, "delta_c.panel_agreement")?;
        validate_unit(self.contradiction, "delta_c.contradiction")?;
        validate_unit(self.delta_c, "delta_c.delta_c")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeltaEBreakdown {
    pub plasticity_window: f32,
    pub hrv_coherence: f32,
    pub valence: f32,
    pub arousal: f32,
    pub valence_arousal: f32,
    pub stress_floor: f32,
    pub k_sleep: f32,
    pub k_state: f32,
    pub delta_e: f32,
}

impl DeltaEBreakdown {
    pub fn validate(&self) -> CoreResult<()> {
        validate_unit(self.plasticity_window, "delta_e.plasticity_window")?;
        validate_unit(self.hrv_coherence, "delta_e.hrv_coherence")?;
        validate_signed_unit(self.valence, "delta_e.valence")?;
        validate_signed_unit(self.arousal, "delta_e.arousal")?;
        validate_unit(self.valence_arousal, "delta_e.valence_arousal")?;
        validate_unit(self.stress_floor, "delta_e.stress_floor")?;
        if !self.k_sleep.is_finite() || !(0.0..=5.0).contains(&self.k_sleep) {
            return Err(CoreError::ValidationError {
                field: "delta_e.k_sleep".into(),
                message: "must be finite and in [0, 5]".into(),
            });
        }
        if !self.k_state.is_finite() || !(0.0..=5.0).contains(&self.k_state) {
            return Err(CoreError::ValidationError {
                field: "delta_e.k_state".into(),
                message: "must be finite and in [0, 5]".into(),
            });
        }
        validate_unit(self.delta_e, "delta_e.delta_e")?;
        Ok(())
    }
}

/// UTL state class used by dashboards and scheduler decisions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LearnerDiagnosticState {
    Confused,
    Optimal,
    Boring,
    Stuck,
    Dysregulated,
    Dissipating,
}

impl LearnerDiagnosticState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Confused => "confused",
            Self::Optimal => "optimal",
            Self::Boring => "boring",
            Self::Stuck => "stuck",
            Self::Dysregulated => "dysregulated",
            Self::Dissipating => "dissipating",
        }
    }
}

/// Full deterministic UTL computation for one learner session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UtlComputation {
    pub delta_s: DeltaSBreakdown,
    pub delta_c: DeltaCBreakdown,
    pub delta_e: DeltaEBreakdown,
    pub l: f32,
    pub diagnostic_state: LearnerDiagnosticState,
}

impl UtlComputation {
    pub fn validate(&self) -> CoreResult<()> {
        self.delta_s.validate()?;
        self.delta_c.validate()?;
        self.delta_e.validate()?;
        validate_unit(self.l, "utl.l")?;
        Ok(())
    }
}

/// Persisted session diagnostic stored in `CF_LEARNER_DELTA_LOG`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerDeltaLog {
    pub learner_id: Uuid,
    pub session_ts: u64,
    pub computation: UtlComputation,
    pub provenance: ComputationEnvelope,
}

impl LearnerDeltaLog {
    pub fn validate(&self) -> CoreResult<()> {
        self.computation.validate()?;
        self.provenance.validate()?;
        if self.provenance.learner_id != self.learner_id
            || self.provenance.session_ts != self.session_ts
        {
            return Err(CoreError::ValidationError {
                field: "delta_log.provenance".into(),
                message: "provenance learner_id/session_ts must match delta log".into(),
            });
        }
        Ok(())
    }
}

/// Per-trace consolidation record stored in `CF_LEARNER_M_PER_TRACE`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerMTrace {
    pub learner_id: Uuid,
    pub trace_id: Uuid,
    pub m_value: f64,
    pub last_update_ts: u64,
    pub decay_rate: f32,
    pub num_retrievals: u32,
    pub next_review_ts: Option<u64>,
}

impl LearnerMTrace {
    pub fn new(
        learner_id: Uuid,
        trace_id: Uuid,
        m_value: f64,
        last_update_ts: u64,
        decay_rate: f32,
        num_retrievals: u32,
    ) -> CoreResult<Self> {
        let mut trace = Self {
            learner_id,
            trace_id,
            m_value,
            last_update_ts,
            decay_rate,
            num_retrievals,
            next_review_ts: None,
        };
        trace.next_review_ts =
            next_review_timestamp(trace.m_value, trace.decay_rate, last_update_ts);
        trace.validate()?;
        Ok(trace)
    }

    pub fn validate(&self) -> CoreResult<()> {
        if !self.m_value.is_finite() || !(0.0..=1.0).contains(&self.m_value) {
            return Err(CoreError::ValidationError {
                field: "m_trace.m_value".into(),
                message: "must be finite and in [0, 1]".into(),
            });
        }
        if !self.decay_rate.is_finite() || !(0.0..=1.0).contains(&self.decay_rate) {
            return Err(CoreError::ValidationError {
                field: "m_trace.decay_rate".into(),
                message: "must be finite and in [0, 1]".into(),
            });
        }
        Ok(())
    }
}

/// Retrieval-practice event stored in `CF_LEARNER_RETRIEVAL_LOG`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerRetrievalLog {
    pub learner_id: Uuid,
    pub trace_id: Uuid,
    pub ts: u64,
    pub correct: bool,
    pub score: f32,
    pub state_at_retrieval: LearnerStateVector,
}

impl LearnerRetrievalLog {
    pub fn validate(&self) -> CoreResult<()> {
        validate_unit(self.score, "retrieval.score")?;
        self.state_at_retrieval.validate()?;
        Ok(())
    }
}

/// Sleep-derived consolidation multiplier stored in `CF_LEARNER_K_SLEEP`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerKSleep {
    pub learner_id: Uuid,
    pub session_ts: u64,
    pub k: f32,
    pub slow_wave_minutes: u16,
}

impl LearnerKSleep {
    pub fn validate(&self) -> CoreResult<()> {
        if !self.k.is_finite() || !(0.0..=5.0).contains(&self.k) {
            return Err(CoreError::ValidationError {
                field: "k_sleep.k".into(),
                message: "must be finite and in [0, 5]".into(),
            });
        }
        Ok(())
    }
}

/// Per-learner deployment/goal state stored in `CF_LEARNER_GOAL_STATES`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerGoalState {
    pub learner_id: Uuid,
    pub skill_id: Uuid,
    pub state_vector: LearnerStateVector,
}

impl LearnerGoalState {
    pub fn validate(&self) -> CoreResult<()> {
        self.state_vector.validate()
    }
}

/// Per-modality centroid inside a learner regulated-state constellation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerModalityCentroid {
    pub modality: LearnerModality,
    pub vector: Vec<f32>,
    pub scalar_mean: Option<f32>,
    pub sample_count: u32,
}

impl LearnerModalityCentroid {
    pub fn validate(&self) -> CoreResult<()> {
        if self.sample_count == 0 {
            return Err(CoreError::ValidationError {
                field: "learner_constellation.centroid.sample_count".into(),
                message: "must be > 0".into(),
            });
        }
        validate_vector(&self.vector, "learner_constellation.centroid.vector")?;
        if let Some(value) = self.scalar_mean {
            validate_signed_unit(value, "learner_constellation.centroid.scalar_mean")?;
        }
        Ok(())
    }
}

/// Per-learner regulated-state baseline stored in `CF_LEARNER_CONSTELLATIONS`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerConstellation {
    pub learner_id: Uuid,
    pub selector_kind: u8,
    pub label: String,
    pub sample_count: u32,
    pub session_ts_start: u64,
    pub session_ts_end: u64,
    pub modality_centroids: Vec<LearnerModalityCentroid>,
    pub state_centroid: LearnerStateVector,
    pub created_at: DateTime<Utc>,
}

impl LearnerConstellation {
    pub fn validate(&self) -> CoreResult<()> {
        validate_label(&self.label, "learner_constellation.label")?;
        if self.sample_count == 0 {
            return Err(CoreError::ValidationError {
                field: "learner_constellation.sample_count".into(),
                message: "must be > 0".into(),
            });
        }
        if self.session_ts_end < self.session_ts_start {
            return Err(CoreError::ValidationError {
                field: "learner_constellation.session_ts_end".into(),
                message: "must be >= session_ts_start".into(),
            });
        }
        if self.modality_centroids.is_empty() {
            return Err(CoreError::ValidationError {
                field: "learner_constellation.modality_centroids".into(),
                message: "at least one centroid is required".into(),
            });
        }
        if self.modality_centroids.len() > LEARNER_MODALITY_COUNT {
            return Err(CoreError::ValidationError {
                field: "learner_constellation.modality_centroids".into(),
                message: "too many learner modality centroids".into(),
            });
        }

        let mut seen = BTreeSet::new();
        for centroid in &self.modality_centroids {
            if centroid.modality == LearnerModality::SelfReport {
                return Err(CoreError::ValidationError {
                    field: "learner_constellation.modality_centroids".into(),
                    message: "self_report is not an embedder centroid".into(),
                });
            }
            if !seen.insert(centroid.modality) {
                return Err(CoreError::ValidationError {
                    field: "learner_constellation.modality_centroids".into(),
                    message: "duplicate modality centroid".into(),
                });
            }
            centroid.validate()?;
        }

        self.state_centroid.validate()?;
        if self.state_centroid.learner_id != self.learner_id {
            return Err(CoreError::ValidationError {
                field: "learner_constellation.state_centroid".into(),
                message: "state centroid learner_id must match constellation".into(),
            });
        }
        Ok(())
    }
}

/// Per-skill expert-answer centroid stored in `CF_GOAL_CENTROIDS`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoalCentroid {
    pub skill_id: Uuid,
    pub modality: LearnerModality,
    pub vector: Vec<f32>,
}

impl GoalCentroid {
    pub fn validate(&self) -> CoreResult<()> {
        validate_vector(&self.vector, "goal_centroid.vector")
    }
}

/// Learner audit row stored in `CF_LEARNER_AUDIT`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearnerAuditEntry {
    pub audit_id: Uuid,
    pub learner_id: Uuid,
    pub ts: u64,
    pub action: String,
    pub target_cf: String,
    pub result_sha256: String,
    pub parent_audit_id: Option<Uuid>,
}

impl LearnerAuditEntry {
    pub fn validate(&self) -> CoreResult<()> {
        validate_label(&self.action, "audit.action")?;
        validate_label(&self.target_cf, "audit.target_cf")?;
        validate_hash(&self.result_sha256, "audit.result_sha256")?;
        Ok(())
    }
}

/// Compute Delta S using deterministic text bag cosine as the Phase-0 proxy.
pub fn compute_delta_s_from_text(
    predicted: &str,
    actual: &str,
    simulated: Option<&str>,
    exploration_rate: f32,
    gamma: Option<f32>,
) -> CoreResult<DeltaSBreakdown> {
    validate_unit(exploration_rate, "delta_s.exploration_rate")?;
    let gamma = gamma.unwrap_or(0.7);
    validate_unit(gamma, "delta_s.gamma")?;

    let predicted_actual_distance = bag_of_words_cosine_distance(predicted, actual);
    let predicted_simulated_distance = simulated
        .map(|s| bag_of_words_cosine_distance(predicted, s))
        .unwrap_or(0.0);
    compute_delta_s_from_distances(
        predicted_actual_distance,
        predicted_simulated_distance,
        exploration_rate,
        gamma,
    )
}

pub fn compute_delta_s_from_distances(
    predicted_actual_distance: f32,
    predicted_simulated_distance: f32,
    exploration_rate: f32,
    gamma: f32,
) -> CoreResult<DeltaSBreakdown> {
    validate_unit(
        predicted_actual_distance,
        "delta_s.predicted_actual_distance",
    )?;
    validate_unit(
        predicted_simulated_distance,
        "delta_s.predicted_simulated_distance",
    )?;
    validate_unit(exploration_rate, "delta_s.exploration_rate")?;
    validate_unit(gamma, "delta_s.gamma")?;

    let delta_s = (predicted_actual_distance.max(gamma * predicted_simulated_distance)
        + 0.5 * exploration_rate)
        .clamp(0.0, 1.0);
    let out = DeltaSBreakdown {
        predicted_actual_distance,
        predicted_simulated_distance,
        exploration_rate,
        gamma,
        delta_s,
    };
    out.validate()?;
    Ok(out)
}

/// Compute Delta C from the file-03 operational form.
pub fn compute_delta_c(
    recent_scores: &[f32],
    hrv_coherence: f32,
    panel_agreement: f32,
    contradiction: f32,
    gradient_scale: Option<f32>,
) -> CoreResult<DeltaCBreakdown> {
    if recent_scores.len() > MAX_RECENT_SCORE_WINDOW {
        return Err(CoreError::ValidationError {
            field: "delta_c.recent_scores".into(),
            message: format!(
                "len {} exceeds max {}",
                recent_scores.len(),
                MAX_RECENT_SCORE_WINDOW
            ),
        });
    }
    for (idx, value) in recent_scores.iter().enumerate() {
        validate_unit(*value, &format!("delta_c.recent_scores[{idx}]"))?;
    }
    validate_unit(hrv_coherence, "delta_c.hrv_coherence")?;
    validate_unit(panel_agreement, "delta_c.panel_agreement")?;
    validate_unit(contradiction, "delta_c.contradiction")?;

    let (mean, stddev) = mean_stddev(recent_scores);
    let cv = if recent_scores.is_empty() {
        0.0
    } else if mean <= 1e-6 {
        if stddev <= 1e-6 {
            0.0
        } else {
            1.0
        }
    } else {
        stddev / mean
    };
    let outcome_stability = if recent_scores.is_empty() {
        0.0
    } else {
        1.0 / (1.0 + cv)
    };

    let slope = if recent_scores.len() < 2 {
        0.0
    } else {
        (recent_scores[recent_scores.len() - 1] - recent_scores[0])
            / (recent_scores.len() - 1) as f32
    };
    let scale = gradient_scale.unwrap_or(4.0);
    if !scale.is_finite() || scale < 0.0 {
        return Err(CoreError::ValidationError {
            field: "delta_c.gradient_scale".into(),
            message: "must be finite and >= 0".into(),
        });
    }
    let gradient_effectiveness = (slope * scale).clamp(0.0, 1.0);

    let delta_c = (outcome_stability
        + 0.2 * gradient_effectiveness
        + 0.2 * hrv_coherence
        + 0.2 * panel_agreement
        - 0.3 * contradiction)
        .clamp(0.0, 1.0);

    let out = DeltaCBreakdown {
        outcome_stability,
        coefficient_of_variation: cv,
        gradient_effectiveness,
        hrv_coherence,
        panel_agreement,
        contradiction,
        delta_c,
    };
    out.validate()?;
    Ok(out)
}

/// Compute Delta E from the differentiated UTL subcomponents.
pub fn compute_delta_e(components: &LearnerStateComponents) -> CoreResult<DeltaEBreakdown> {
    components.validate()?;
    let arousal_01 = ((components.arousal + 1.0) / 2.0).clamp(0.0, 1.0);
    let inverted_u = (4.0 * arousal_01 * (1.0 - arousal_01)).clamp(0.0, 1.0);
    let valence_sigmoid = 1.0 / (1.0 + (-3.0 * components.valence).exp());
    let valence_arousal = (valence_sigmoid * inverted_u).clamp(0.0, 1.0);
    let delta_e = components.plasticity_window.powf(0.4)
        * valence_arousal.powf(0.4)
        * components.stress_floor.powf(0.2);
    let k_state = components.plasticity_window.sqrt() * components.k_sleep;

    let out = DeltaEBreakdown {
        plasticity_window: components.plasticity_window,
        hrv_coherence: components.hrv_coherence,
        valence: components.valence,
        arousal: components.arousal,
        valence_arousal,
        stress_floor: components.stress_floor,
        k_sleep: components.k_sleep,
        k_state,
        delta_e: delta_e.clamp(0.0, 1.0),
    };
    out.validate()?;
    Ok(out)
}

/// Compute L = DeltaS * DeltaC * DeltaE and classify the session.
pub fn compute_utl_l(
    delta_s: DeltaSBreakdown,
    delta_c: DeltaCBreakdown,
    delta_e: DeltaEBreakdown,
    recent_l_low_steps: u32,
    consolidation_gain_ratio: Option<f32>,
) -> CoreResult<UtlComputation> {
    delta_s.validate()?;
    delta_c.validate()?;
    delta_e.validate()?;
    if let Some(ratio) = consolidation_gain_ratio {
        if !ratio.is_finite() || ratio < 0.0 {
            return Err(CoreError::ValidationError {
                field: "utl.consolidation_gain_ratio".into(),
                message: "must be finite and >= 0".into(),
            });
        }
    }
    let l = (delta_s.delta_s * delta_c.delta_c * delta_e.delta_e).clamp(0.0, 1.0);
    let diagnostic_state = classify_learner_state(
        l,
        delta_e.delta_e,
        recent_l_low_steps,
        consolidation_gain_ratio,
    );
    let out = UtlComputation {
        delta_s,
        delta_c,
        delta_e,
        l,
        diagnostic_state,
    };
    out.validate()?;
    Ok(out)
}

pub fn classify_learner_state(
    l: f32,
    delta_e: f32,
    recent_l_low_steps: u32,
    consolidation_gain_ratio: Option<f32>,
) -> LearnerDiagnosticState {
    if l > 0.3 && consolidation_gain_ratio.is_some_and(|ratio| ratio < 0.2) {
        return LearnerDiagnosticState::Dissipating;
    }
    if l < 0.1 && recent_l_low_steps >= 3 && delta_e < 0.4 {
        LearnerDiagnosticState::Dysregulated
    } else if l < 0.1 && recent_l_low_steps >= 3 {
        LearnerDiagnosticState::Stuck
    } else if l < 0.3 {
        LearnerDiagnosticState::Confused
    } else if l < 0.7 {
        LearnerDiagnosticState::Optimal
    } else {
        LearnerDiagnosticState::Boring
    }
}

/// Update a consolidation trace with one retrieval event.
pub fn update_m_trace(
    previous: Option<&LearnerMTrace>,
    learner_id: Uuid,
    trace_id: Uuid,
    now_ts: u64,
    computation: &UtlComputation,
    retrieval_correct: Option<bool>,
) -> CoreResult<LearnerMTrace> {
    computation.validate()?;
    let (previous_m, previous_ts, previous_decay, previous_count) = previous
        .map(|t| (t.m_value, t.last_update_ts, t.decay_rate, t.num_retrievals))
        .unwrap_or((0.0, now_ts, 1.0 / 3600.0, 0));
    let elapsed = now_ts.saturating_sub(previous_ts) as f64;
    let retained = previous_m * (-(elapsed * previous_decay as f64)).exp();
    let contribution = computation.l as f64
        * computation.delta_e.delta_e as f64
        * computation.delta_e.k_sleep as f64;
    let m_value = (retained + contribution).clamp(0.0, 1.0);

    let mut decay_rate = previous_decay;
    let mut num_retrievals = previous_count;
    if let Some(correct) = retrieval_correct {
        num_retrievals = num_retrievals.saturating_add(1);
        decay_rate = if correct {
            decay_rate * 0.5
        } else {
            (decay_rate / 0.7).min(1.0)
        };
    }

    LearnerMTrace::new(
        learner_id,
        trace_id,
        m_value,
        now_ts,
        decay_rate,
        num_retrievals,
    )
}

/// Compute the transfer term T(train, deploy) = exp(-distance/lambda).
pub fn compute_transfer_t(train: &[f32], deploy: &[f32], lambda: f32) -> CoreResult<f32> {
    if train.len() != deploy.len() {
        return Err(CoreError::DimensionMismatch {
            expected: train.len(),
            actual: deploy.len(),
        });
    }
    if train.is_empty() {
        return Err(CoreError::ValidationError {
            field: "transfer.vector".into(),
            message: "state vectors must not be empty".into(),
        });
    }
    validate_vector(train, "transfer.train")?;
    validate_vector(deploy, "transfer.deploy")?;
    if !lambda.is_finite() || lambda <= 0.0 {
        return Err(CoreError::ValidationError {
            field: "transfer.lambda".into(),
            message: "must be finite and > 0".into(),
        });
    }
    let distance = train
        .iter()
        .zip(deploy.iter())
        .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
        .sum::<f64>()
        .sqrt() as f32;
    Ok((-distance / lambda).exp().clamp(0.0, 1.0))
}

/// Next review time using the Phase-0 M threshold formula.
pub fn next_review_timestamp(m_value: f64, decay_rate: f32, now_ts: u64) -> Option<u64> {
    let target_m = 0.7f64;
    if !m_value.is_finite() || !decay_rate.is_finite() || decay_rate <= 0.0 {
        return None;
    }
    if m_value <= target_m {
        return Some(now_ts);
    }
    let elapsed_to_threshold = (m_value / target_m).ln() / decay_rate as f64;
    if !elapsed_to_threshold.is_finite() || elapsed_to_threshold < 0.0 {
        return None;
    }
    let scheduled = now_ts as f64 + elapsed_to_threshold * 0.9;
    if scheduled > u64::MAX as f64 {
        None
    } else {
        Some(scheduled.round() as u64)
    }
}

/// Build a deterministic SHA-256 hex hash for a serializable object.
pub fn sha256_json<T: Serialize>(value: &T) -> CoreResult<String> {
    let bytes = serde_json::to_vec(value)
        .map_err(|e| CoreError::SerializationError(format!("serialize for sha256: {e}")))?;
    Ok(sha256_bytes(&bytes))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn bag_of_words_cosine_distance(a: &str, b: &str) -> f32 {
    let a_tokens = token_counts(a);
    let b_tokens = token_counts(b);
    if a_tokens.is_empty() && b_tokens.is_empty() {
        return 0.0;
    }
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 1.0;
    }
    let mut dot = 0.0f32;
    for (token, a_count) in &a_tokens {
        if let Some(b_count) = b_tokens.get(token) {
            dot += *a_count as f32 * *b_count as f32;
        }
    }
    let norm_a = a_tokens
        .values()
        .map(|v| (*v as f32).powi(2))
        .sum::<f32>()
        .sqrt();
    let norm_b = b_tokens
        .values()
        .map(|v| (*v as f32).powi(2))
        .sum::<f32>()
        .sqrt();
    if norm_a <= 0.0 || norm_b <= 0.0 {
        1.0
    } else {
        (1.0 - dot / (norm_a * norm_b)).clamp(0.0, 1.0)
    }
}

fn token_counts(value: &str) -> BTreeMap<String, u32> {
    let mut out = BTreeMap::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            *out.entry(std::mem::take(&mut current)).or_insert(0) += 1;
        }
    }
    if !current.is_empty() {
        *out.entry(current).or_insert(0) += 1;
    }
    out
}

fn mean_stddev(values: &[f32]) -> (f32, f32) {
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    let variance = values
        .iter()
        .map(|value| (*value - mean).powi(2))
        .sum::<f32>()
        / values.len() as f32;
    (mean, variance.sqrt())
}

fn validate_vector(values: &[f32], field: &str) -> CoreResult<()> {
    if values.len() > MAX_LEARNER_STATE_VECTOR_VALUES {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: format!(
                "len {} exceeds max {}",
                values.len(),
                MAX_LEARNER_STATE_VECTOR_VALUES
            ),
        });
    }
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(CoreError::ValidationError {
                field: format!("{field}[{idx}]"),
                message: "must be finite".into(),
            });
        }
    }
    Ok(())
}

fn validate_unit(value: f32, field: &str) -> CoreResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be finite and in [0, 1]".into(),
        });
    }
    Ok(())
}

fn validate_signed_unit(value: f32, field: &str) -> CoreResult<()> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be finite and in [-1, 1]".into(),
        });
    }
    Ok(())
}

fn validate_label(value: &str, field: &str) -> CoreResult<()> {
    let len = value.chars().count();
    if len > MAX_LEARNER_LABEL_CHARS {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: format!("len {len} exceeds max {MAX_LEARNER_LABEL_CHARS}"),
        });
    }
    Ok(())
}

fn validate_hash(value: &str, field: &str) -> CoreResult<()> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be a 64-character hex SHA-256".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn components() -> LearnerStateComponents {
        LearnerStateComponents {
            plasticity_window: 0.81,
            hrv_coherence: 0.64,
            valence: 0.2,
            arousal: 0.0,
            stress_floor: 0.9,
            k_sleep: 1.0,
        }
    }

    #[test]
    fn delta_s_happy_path_known_text_distance() {
        let before = "alpha beta gamma";
        let after = "alpha beta delta";
        let result = compute_delta_s_from_text(before, after, None, 0.2, Some(0.7)).unwrap();
        println!(
            "BEFORE predicted='{}' AFTER actual='{}' distance={} delta_s={}",
            before, after, result.predicted_actual_distance, result.delta_s
        );
        assert!(result.predicted_actual_distance > 0.3);
        assert!(result.delta_s > result.predicted_actual_distance);
    }

    #[test]
    fn delta_s_empty_inputs_are_no_surprise_without_exploration() {
        let result = compute_delta_s_from_text("", "", None, 0.0, None).unwrap();
        println!("BEFORE empty strings AFTER delta_s={}", result.delta_s);
        assert_eq!(result.predicted_actual_distance, 0.0);
        assert_eq!(result.delta_s, 0.0);
    }

    #[test]
    fn delta_c_invalid_score_fails() {
        let err = compute_delta_c(&[0.8, 1.2], 0.5, 0.5, 0.0, None).unwrap_err();
        println!("invalid score error={err}");
        assert!(format!("{err}").contains("recent_scores"));
    }

    #[test]
    fn delta_c_stable_scores_clamp_high() {
        let result = compute_delta_c(&[0.7, 0.75, 0.8], 0.6, 0.8, 0.0, None).unwrap();
        println!(
            "AFTER outcome_stability={} gradient={} delta_c={}",
            result.outcome_stability, result.gradient_effectiveness, result.delta_c
        );
        assert!(result.outcome_stability > 0.9);
        assert_eq!(result.delta_c, 1.0);
    }

    #[test]
    fn delta_e_components_match_expected_shape() {
        let result = compute_delta_e(&components()).unwrap();
        println!(
            "AFTER valence_arousal={} k_state={} delta_e={}",
            result.valence_arousal, result.k_state, result.delta_e
        );
        assert!(result.delta_e > 0.0);
        assert!(result.k_state > 0.0);
    }

    #[test]
    fn l_classifies_dysregulated_low_e_after_repeated_low_steps() {
        let delta_s = compute_delta_s_from_distances(0.2, 0.0, 0.0, 0.7).unwrap();
        let delta_c = compute_delta_c(&[0.1, 0.1], 0.1, 0.1, 0.0, None).unwrap();
        let low_e = compute_delta_e(&LearnerStateComponents {
            plasticity_window: 0.1,
            hrv_coherence: 0.1,
            valence: -1.0,
            arousal: 1.0,
            stress_floor: 0.1,
            k_sleep: 1.0,
        })
        .unwrap();
        let result = compute_utl_l(delta_s, delta_c, low_e, 3, None).unwrap();
        println!(
            "AFTER l={} delta_e={} state={}",
            result.l,
            result.delta_e.delta_e,
            result.diagnostic_state.as_str()
        );
        assert_eq!(
            result.diagnostic_state,
            LearnerDiagnosticState::Dysregulated
        );
    }

    #[test]
    fn m_trace_correct_retrieval_extends_half_life() {
        let delta_s = compute_delta_s_from_distances(0.5, 0.0, 0.2, 0.7).unwrap();
        let delta_c = compute_delta_c(&[0.7, 0.8], 0.7, 0.8, 0.0, None).unwrap();
        let delta_e = compute_delta_e(&components()).unwrap();
        let computation = compute_utl_l(delta_s, delta_c, delta_e, 0, None).unwrap();
        let trace_id = Uuid::new_v4();
        let learner_id = Uuid::new_v4();
        let updated = update_m_trace(
            None,
            learner_id,
            trace_id,
            1_700_000_000,
            &computation,
            Some(true),
        )
        .unwrap();
        println!(
            "AFTER m_value={} decay_rate={} num_retrievals={} next_review_ts={:?}",
            updated.m_value, updated.decay_rate, updated.num_retrievals, updated.next_review_ts
        );
        assert!(updated.m_value > 0.0);
        assert_eq!(updated.num_retrievals, 1);
        assert!(updated.decay_rate < 1.0 / 3600.0);
    }

    #[test]
    fn transfer_identical_vectors_is_one() {
        let t = compute_transfer_t(&[0.1, 0.2, 0.3], &[0.1, 0.2, 0.3], 1.5).unwrap();
        println!("AFTER transfer_t={t}");
        assert_eq!(t, 1.0);
    }
}
