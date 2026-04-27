//! Learning-as-UTL event types and deterministic signal computation.
//!
//! UTL learning signals are kept separate from the E1-E14 production embedder
//! slots. They describe event-level state transitions and are persisted through
//! the storage layer as `LearningEvent` records.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, CoreResult};
use crate::similarity::MultiUtlParams;
use crate::training::NUM_CROSS_CORRELATIONS;
use crate::types::fingerprint::NUM_EMBEDDERS;

/// Current on-disk version byte for `LearningEvent`.
pub const LEARNING_EVENT_VERSION: u8 = 1;

/// Maximum number of memory ids linked to one learning event.
pub const MAX_LEARNING_EVENT_MEMORY_IDS: usize = 1024;

/// Maximum length for event text fields such as query and response.
pub const MAX_LEARNING_EVENT_TEXT_CHARS: usize = 65_536;

/// Maximum length for label-like free text fields.
pub const MAX_LEARNING_EVENT_LABEL_CHARS: usize = 256;

/// Snapshot of the state used to explain a UTL event before or after an action.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningStateSnapshot {
    /// 14D topic/teleological profile at this point in the interaction.
    pub topic_profile: [f32; NUM_EMBEDDERS],
    /// 91D pairwise cross-correlation vector for the same topic profile.
    pub cross_correlations: Vec<f32>,
    /// Rank of the relevant result. Lower is better. `None` means unknown.
    pub retrieval_rank: Option<u32>,
    /// Per-embedder scores that produced or explained the state.
    pub embedder_scores: [f32; NUM_EMBEDDERS],
    /// Contradiction pressure in [0, 1].
    pub contradiction_pressure: f32,
    /// Integration confidence in [0, 1].
    pub integration_confidence: f32,
    /// How often the same memory/structure has recurred.
    pub recurrence_count: u32,
    /// Topic/structure stability in [0, 1].
    pub stability_score: f32,
    /// Domain tag used for transfer detection.
    pub domain: Option<String>,
    /// Count of known successful cross-domain reuses.
    pub successful_transfer_count: u32,
}

impl LearningStateSnapshot {
    /// Build a snapshot with exact shape requirements.
    pub fn new(
        topic_profile: [f32; NUM_EMBEDDERS],
        cross_correlations: Vec<f32>,
    ) -> CoreResult<Self> {
        let state = Self {
            topic_profile,
            cross_correlations,
            retrieval_rank: None,
            embedder_scores: [0.0; NUM_EMBEDDERS],
            contradiction_pressure: 0.0,
            integration_confidence: 0.0,
            recurrence_count: 0,
            stability_score: 0.0,
            domain: None,
            successful_transfer_count: 0,
        };
        state.validate("state")?;
        Ok(state)
    }

    /// Validate all persisted dimensions and scalar ranges.
    pub fn validate(&self, field_prefix: &str) -> CoreResult<()> {
        validate_profile(
            &self.topic_profile,
            &format!("{field_prefix}.topic_profile"),
        )?;
        validate_cross_correlations(
            &self.cross_correlations,
            &format!("{field_prefix}.cross_correlations"),
        )?;
        validate_unit_array(
            &self.embedder_scores,
            &format!("{field_prefix}.embedder_scores"),
        )?;
        validate_unit_scalar(
            self.contradiction_pressure,
            &format!("{field_prefix}.contradiction_pressure"),
        )?;
        validate_unit_scalar(
            self.integration_confidence,
            &format!("{field_prefix}.integration_confidence"),
        )?;
        validate_unit_scalar(
            self.stability_score,
            &format!("{field_prefix}.stability_score"),
        )?;
        if let Some(domain) = self.domain.as_ref() {
            validate_label_len(domain, &format!("{field_prefix}.domain"))?;
        }
        Ok(())
    }
}

/// Human/agent outcome label for a learning event.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LearningOutcomeLabel {
    Useful,
    Neutral,
    Harmful,
    NoLearning,
}

/// Outcome labels and external observations attached to a learning event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningOutcome {
    pub label: LearningOutcomeLabel,
    /// Signed utility delta in [-1, 1].
    pub utility_delta: f32,
    pub correction_required: bool,
    pub reuse_observed: bool,
}

impl LearningOutcome {
    pub fn validate(&self) -> CoreResult<()> {
        if !self.utility_delta.is_finite() || !(-1.0..=1.0).contains(&self.utility_delta) {
            return Err(CoreError::ValidationError {
                field: "outcome.utility_delta".into(),
                message: "must be finite and in [-1, 1]".into(),
            });
        }
        Ok(())
    }
}

/// Deterministically-computed UTL feature bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningFeatures {
    pub delta_e_vector: [f32; NUM_EMBEDDERS],
    pub delta_e_scalar: f32,
    pub retrieval_rank_shift: f32,
    pub embedder_disagreement: f32,
    pub surprise_score: f32,
    pub productive_surprise: bool,
    pub coherence_delta: f32,
    pub contradiction_delta: f32,
    pub consolidation_readiness: f32,
    pub transfer_score: f32,
    pub multi_utl_score: f32,
    pub attribution: [f32; NUM_EMBEDDERS],
}

impl Default for LearningFeatures {
    fn default() -> Self {
        Self {
            delta_e_vector: [0.0; NUM_EMBEDDERS],
            delta_e_scalar: 0.0,
            retrieval_rank_shift: 0.0,
            embedder_disagreement: 0.0,
            surprise_score: 0.0,
            productive_surprise: false,
            coherence_delta: 0.0,
            contradiction_delta: 0.0,
            consolidation_readiness: 0.0,
            transfer_score: 0.0,
            multi_utl_score: 0.5,
            attribution: [0.0; NUM_EMBEDDERS],
        }
    }
}

/// Stable identifiers for UTL learning signal families.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LearningSignalId {
    DeltaE,
    Surprise,
    Coherence,
    Consolidation,
    Transfer,
}

impl LearningSignalId {
    pub fn as_str(self) -> &'static str {
        match self {
            LearningSignalId::DeltaE => "delta_e",
            LearningSignalId::Surprise => "surprise",
            LearningSignalId::Coherence => "coherence",
            LearningSignalId::Consolidation => "consolidation",
            LearningSignalId::Transfer => "transfer",
        }
    }

    pub fn dimension(self) -> usize {
        match self {
            LearningSignalId::DeltaE => NUM_EMBEDDERS,
            LearningSignalId::Surprise
            | LearningSignalId::Coherence
            | LearningSignalId::Consolidation
            | LearningSignalId::Transfer => 3,
        }
    }
}

/// A computed event-level signal. These are not E1-E14 embedders.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningSignal {
    pub signal_id: LearningSignalId,
    pub vector: Vec<f32>,
    pub scalar: f32,
    pub label: Option<String>,
    pub attribution: [f32; NUM_EMBEDDERS],
}

impl LearningSignal {
    pub fn validate(&self) -> CoreResult<()> {
        let expected = self.signal_id.dimension();
        if self.vector.len() != expected {
            return Err(CoreError::DimensionMismatch {
                expected,
                actual: self.vector.len(),
            });
        }
        for (idx, value) in self.vector.iter().enumerate() {
            if !value.is_finite() {
                return Err(CoreError::ValidationError {
                    field: format!("signal.{}.vector[{idx}]", self.signal_id.as_str()),
                    message: "must be finite".into(),
                });
            }
        }
        if !self.scalar.is_finite() || !(-1.0..=1.0).contains(&self.scalar) {
            return Err(CoreError::ValidationError {
                field: format!("signal.{}.scalar", self.signal_id.as_str()),
                message: "must be finite and in [-1, 1]".into(),
            });
        }
        validate_unit_array(
            &self.attribution,
            &format!("signal.{}.attribution", self.signal_id.as_str()),
        )?;
        if let Some(label) = self.label.as_ref() {
            validate_label_len(label, "signal.label")?;
        }
        Ok(())
    }
}

/// Event persisted in `CF_LEARNING_EVENTS`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningEvent {
    pub event_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub memory_ids: Vec<Uuid>,
    pub session_id: Option<String>,
    pub response_id: Option<String>,
    pub task_id: Option<String>,
    pub query: String,
    pub retrieved_context: String,
    pub assistant_response: String,
    pub before: LearningStateSnapshot,
    pub after: LearningStateSnapshot,
    pub outcome: LearningOutcome,
    pub features: LearningFeatures,
    pub signals: Vec<LearningSignal>,
}

impl LearningEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: Uuid,
        memory_ids: Vec<Uuid>,
        session_id: Option<String>,
        response_id: Option<String>,
        task_id: Option<String>,
        query: String,
        retrieved_context: String,
        assistant_response: String,
        before: LearningStateSnapshot,
        after: LearningStateSnapshot,
        outcome: LearningOutcome,
    ) -> CoreResult<Self> {
        let mut event = Self {
            event_id,
            created_at: Utc::now(),
            memory_ids,
            session_id,
            response_id,
            task_id,
            query,
            retrieved_context,
            assistant_response,
            before,
            after,
            outcome,
            features: LearningFeatures::default(),
            signals: Vec::new(),
        };
        event.recompute_features()?;
        Ok(event)
    }

    /// Validate persisted shape and all scalar bounds.
    pub fn validate(&self) -> CoreResult<()> {
        if self.memory_ids.len() > MAX_LEARNING_EVENT_MEMORY_IDS {
            return Err(CoreError::ValidationError {
                field: "memory_ids".into(),
                message: format!(
                    "len {} exceeds max {}",
                    self.memory_ids.len(),
                    MAX_LEARNING_EVENT_MEMORY_IDS
                ),
            });
        }
        validate_text_len(&self.query, "query")?;
        validate_text_len(&self.retrieved_context, "retrieved_context")?;
        validate_text_len(&self.assistant_response, "assistant_response")?;
        if let Some(session_id) = self.session_id.as_ref() {
            validate_label_len(session_id, "session_id")?;
        }
        if let Some(response_id) = self.response_id.as_ref() {
            validate_label_len(response_id, "response_id")?;
        }
        if let Some(task_id) = self.task_id.as_ref() {
            validate_label_len(task_id, "task_id")?;
        }
        self.before.validate("before")?;
        self.after.validate("after")?;
        self.outcome.validate()?;
        validate_features(&self.features)?;
        for signal in &self.signals {
            signal.validate()?;
        }
        Ok(())
    }

    /// Recompute features and baseline signals from persisted input fields.
    pub fn recompute_features(&mut self) -> CoreResult<()> {
        self.before.validate("before")?;
        self.after.validate("after")?;
        self.outcome.validate()?;
        let features = compute_learning_features(&self.before, &self.after, &self.outcome)?;
        let signals = compute_baseline_learning_signals(&features, &self.before, &self.after);
        self.features = features;
        self.signals = signals;
        self.validate()
    }
}

/// Embed a learning event into one deterministic UTL signal family.
#[async_trait]
pub trait LearningSignalEmbedder: Send + Sync {
    fn signal_id(&self) -> LearningSignalId;
    fn dimension(&self) -> usize {
        self.signal_id().dimension()
    }
    async fn embed_event(&self, event: &LearningEvent) -> CoreResult<LearningSignal>;
}

/// Deterministic baseline embedder for the five UTL signal families.
#[derive(Debug, Clone, Copy)]
pub struct DeterministicLearningSignalEmbedder {
    signal_id: LearningSignalId,
}

impl DeterministicLearningSignalEmbedder {
    pub fn new(signal_id: LearningSignalId) -> Self {
        Self { signal_id }
    }
}

#[async_trait]
impl LearningSignalEmbedder for DeterministicLearningSignalEmbedder {
    fn signal_id(&self) -> LearningSignalId {
        self.signal_id
    }

    async fn embed_event(&self, event: &LearningEvent) -> CoreResult<LearningSignal> {
        event.validate()?;
        let signal = compute_signal(self.signal_id, &event.features, &event.before, &event.after);
        signal.validate()?;
        Ok(signal)
    }
}

/// Compute the deterministic UTL feature bundle for a before/after state pair.
pub fn compute_learning_features(
    before: &LearningStateSnapshot,
    after: &LearningStateSnapshot,
    outcome: &LearningOutcome,
) -> CoreResult<LearningFeatures> {
    before.validate("before")?;
    after.validate("after")?;
    outcome.validate()?;

    let mut delta_e_vector = [0.0f32; NUM_EMBEDDERS];
    for idx in 0..NUM_EMBEDDERS {
        delta_e_vector[idx] = after.topic_profile[idx] - before.topic_profile[idx];
    }
    let delta_e_scalar =
        (delta_e_vector.iter().sum::<f32>() / NUM_EMBEDDERS as f32).clamp(-1.0, 1.0);

    let retrieval_rank_shift = compute_rank_shift(before.retrieval_rank, after.retrieval_rank);
    let embedder_disagreement = score_range(&after.embedder_scores);
    let surprise_score =
        (0.5 * retrieval_rank_shift.max(0.0) + 0.5 * embedder_disagreement).clamp(0.0, 1.0);
    let productive_surprise = surprise_score >= 0.15 && outcome.utility_delta >= 0.0;

    let contradiction_delta = after.contradiction_pressure - before.contradiction_pressure;
    let integration_delta = after.integration_confidence - before.integration_confidence;
    let coherence_delta = (integration_delta - contradiction_delta.max(0.0)).clamp(-1.0, 1.0);

    let consolidation_readiness = consolidation_readiness(after);
    let transfer_score = compute_transfer_score(before, after, outcome);
    let attribution = compute_attribution(&delta_e_vector, &after.embedder_scores);
    let multi_utl_score = compute_multi_utl_score(&delta_e_vector, coherence_delta, after);

    Ok(LearningFeatures {
        delta_e_vector,
        delta_e_scalar,
        retrieval_rank_shift,
        embedder_disagreement,
        surprise_score,
        productive_surprise,
        coherence_delta,
        contradiction_delta,
        consolidation_readiness,
        transfer_score,
        multi_utl_score,
        attribution,
    })
}

/// Compute all five deterministic baseline learning signals.
pub fn compute_baseline_learning_signals(
    features: &LearningFeatures,
    before: &LearningStateSnapshot,
    after: &LearningStateSnapshot,
) -> Vec<LearningSignal> {
    [
        LearningSignalId::DeltaE,
        LearningSignalId::Surprise,
        LearningSignalId::Coherence,
        LearningSignalId::Consolidation,
        LearningSignalId::Transfer,
    ]
    .into_iter()
    .map(|id| compute_signal(id, features, before, after))
    .collect()
}

fn compute_signal(
    signal_id: LearningSignalId,
    features: &LearningFeatures,
    before: &LearningStateSnapshot,
    after: &LearningStateSnapshot,
) -> LearningSignal {
    match signal_id {
        LearningSignalId::DeltaE => LearningSignal {
            signal_id,
            vector: features.delta_e_vector.to_vec(),
            scalar: features.delta_e_scalar,
            label: Some(if features.delta_e_scalar > 0.0 {
                "positive_delta".into()
            } else if features.delta_e_scalar < 0.0 {
                "negative_delta".into()
            } else {
                "no_delta".into()
            }),
            attribution: features.attribution,
        },
        LearningSignalId::Surprise => LearningSignal {
            signal_id,
            vector: vec![
                features.retrieval_rank_shift,
                features.embedder_disagreement,
                features.surprise_score,
            ],
            scalar: features.surprise_score,
            label: Some(if features.productive_surprise {
                "productive_surprise".into()
            } else {
                "low_or_noisy_surprise".into()
            }),
            attribution: features.attribution,
        },
        LearningSignalId::Coherence => LearningSignal {
            signal_id,
            vector: vec![
                features.coherence_delta,
                after.contradiction_pressure,
                after.integration_confidence,
            ],
            scalar: features.coherence_delta,
            label: Some(if features.coherence_delta >= 0.0 {
                "coherence_improved".into()
            } else {
                "coherence_reduced".into()
            }),
            attribution: features.attribution,
        },
        LearningSignalId::Consolidation => LearningSignal {
            signal_id,
            vector: vec![
                (after.recurrence_count.min(10) as f32) / 10.0,
                after.stability_score,
                features.consolidation_readiness,
            ],
            scalar: features.consolidation_readiness,
            label: Some(if features.consolidation_readiness >= 0.7 {
                "ready".into()
            } else {
                "not_ready".into()
            }),
            attribution: features.attribution,
        },
        LearningSignalId::Transfer => {
            let cross_domain = domains_differ(before.domain.as_deref(), after.domain.as_deref());
            let count_delta = after
                .successful_transfer_count
                .saturating_sub(before.successful_transfer_count)
                .min(10) as f32
                / 10.0;
            LearningSignal {
                signal_id,
                vector: vec![
                    if cross_domain { 1.0 } else { 0.0 },
                    count_delta,
                    features.transfer_score,
                ],
                scalar: features.transfer_score,
                label: Some(if features.transfer_score > 0.0 {
                    "positive_transfer".into()
                } else {
                    "no_transfer".into()
                }),
                attribution: features.attribution,
            }
        }
    }
}

fn compute_rank_shift(before: Option<u32>, after: Option<u32>) -> f32 {
    match (before, after) {
        (Some(b), Some(a)) if b > 0 => ((b as f32 - a as f32) / b.max(1) as f32).clamp(-1.0, 1.0),
        _ => 0.0,
    }
}

fn score_range(scores: &[f32; NUM_EMBEDDERS]) -> f32 {
    let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
    for value in scores {
        min = min.min(*value);
        max = max.max(*value);
    }
    (max - min).clamp(0.0, 1.0)
}

fn consolidation_readiness(state: &LearningStateSnapshot) -> f32 {
    let recurrence = (state.recurrence_count.min(10) as f32) / 10.0;
    (0.5 * recurrence + 0.5 * state.stability_score).clamp(0.0, 1.0)
}

fn compute_transfer_score(
    before: &LearningStateSnapshot,
    after: &LearningStateSnapshot,
    outcome: &LearningOutcome,
) -> f32 {
    if !domains_differ(before.domain.as_deref(), after.domain.as_deref()) {
        return 0.0;
    }
    let count_delta = after
        .successful_transfer_count
        .saturating_sub(before.successful_transfer_count)
        .min(10) as f32
        / 10.0;
    (0.5 * outcome.utility_delta.max(0.0) + 0.5 * count_delta).clamp(0.0, 1.0)
}

fn domains_differ(before: Option<&str>, after: Option<&str>) -> bool {
    match (before, after) {
        (Some(a), Some(b)) => !a.is_empty() && !b.is_empty() && a != b,
        _ => false,
    }
}

fn compute_attribution(
    delta_e_vector: &[f32; NUM_EMBEDDERS],
    embedder_scores: &[f32; NUM_EMBEDDERS],
) -> [f32; NUM_EMBEDDERS] {
    let mut out = [0.0f32; NUM_EMBEDDERS];
    let delta_sum: f32 = delta_e_vector.iter().map(|v| v.abs()).sum();
    if delta_sum > 0.0 {
        for idx in 0..NUM_EMBEDDERS {
            out[idx] = (delta_e_vector[idx].abs() / delta_sum).clamp(0.0, 1.0);
        }
        return out;
    }

    let score_sum: f32 = embedder_scores.iter().sum();
    if score_sum > 0.0 {
        for idx in 0..NUM_EMBEDDERS {
            out[idx] = (embedder_scores[idx] / score_sum).clamp(0.0, 1.0);
        }
    }
    out
}

fn compute_multi_utl_score(
    delta_e_vector: &[f32; NUM_EMBEDDERS],
    coherence_delta: f32,
    after: &LearningStateSnapshot,
) -> f32 {
    let semantic_deltas = delta_e_vector.map(|v| v.abs().clamp(0.0, 1.0));
    let coherence = coherence_delta.max(0.0).clamp(0.0, 1.0);
    let coherence_deltas = [coherence; NUM_EMBEDDERS];

    let semantic_sum: f32 = semantic_deltas.iter().sum();
    let coherence_sum: f32 = coherence_deltas.iter().sum();
    if semantic_sum <= 0.0 && coherence_sum <= 0.0 {
        return 0.5;
    }

    MultiUtlParams {
        semantic_deltas,
        coherence_deltas,
        tau_weights: after.topic_profile,
        lambda_s: 1.0,
        lambda_c: 1.0,
        w_e: 1.0,
        phi: 0.0,
    }
    .compute()
}

fn validate_features(features: &LearningFeatures) -> CoreResult<()> {
    validate_finite_array(&features.delta_e_vector, "features.delta_e_vector")?;
    validate_unit_array(&features.attribution, "features.attribution")?;
    validate_signed_scalar(features.delta_e_scalar, "features.delta_e_scalar")?;
    validate_signed_scalar(
        features.retrieval_rank_shift,
        "features.retrieval_rank_shift",
    )?;
    validate_unit_scalar(
        features.embedder_disagreement,
        "features.embedder_disagreement",
    )?;
    validate_unit_scalar(features.surprise_score, "features.surprise_score")?;
    validate_signed_scalar(features.coherence_delta, "features.coherence_delta")?;
    validate_signed_scalar(features.contradiction_delta, "features.contradiction_delta")?;
    validate_unit_scalar(
        features.consolidation_readiness,
        "features.consolidation_readiness",
    )?;
    validate_unit_scalar(features.transfer_score, "features.transfer_score")?;
    validate_unit_scalar(features.multi_utl_score, "features.multi_utl_score")?;
    Ok(())
}

fn validate_profile(profile: &[f32; NUM_EMBEDDERS], field: &str) -> CoreResult<()> {
    validate_unit_array(profile, field)
}

fn validate_cross_correlations(values: &[f32], field: &str) -> CoreResult<()> {
    if values.len() != NUM_CROSS_CORRELATIONS {
        return Err(CoreError::DimensionMismatch {
            expected: NUM_CROSS_CORRELATIONS,
            actual: values.len(),
        });
    }
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return Err(CoreError::ValidationError {
                field: format!("{field}[{idx}]"),
                message: "must be finite and in [0, 1]".into(),
            });
        }
    }
    Ok(())
}

fn validate_unit_array(values: &[f32; NUM_EMBEDDERS], field: &str) -> CoreResult<()> {
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() || !(0.0..=1.0).contains(value) {
            return Err(CoreError::ValidationError {
                field: format!("{field}[{idx}]"),
                message: "must be finite and in [0, 1]".into(),
            });
        }
    }
    Ok(())
}

fn validate_finite_array(values: &[f32; NUM_EMBEDDERS], field: &str) -> CoreResult<()> {
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

fn validate_unit_scalar(value: f32, field: &str) -> CoreResult<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be finite and in [0, 1]".into(),
        });
    }
    Ok(())
}

fn validate_signed_scalar(value: f32, field: &str) -> CoreResult<()> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: "must be finite and in [-1, 1]".into(),
        });
    }
    Ok(())
}

fn validate_text_len(value: &str, field: &str) -> CoreResult<()> {
    let len = value.chars().count();
    if len > MAX_LEARNING_EVENT_TEXT_CHARS {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: format!("len {len} exceeds max {MAX_LEARNING_EVENT_TEXT_CHARS}"),
        });
    }
    Ok(())
}

fn validate_label_len(value: &str, field: &str) -> CoreResult<()> {
    let len = value.chars().count();
    if len > MAX_LEARNING_EVENT_LABEL_CHARS {
        return Err(CoreError::ValidationError {
            field: field.into(),
            message: format!("len {len} exceeds max {MAX_LEARNING_EVENT_LABEL_CHARS}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn correlations(value: f32) -> Vec<f32> {
        vec![value; NUM_CROSS_CORRELATIONS]
    }

    fn state(topic: f32) -> LearningStateSnapshot {
        LearningStateSnapshot {
            topic_profile: [topic; NUM_EMBEDDERS],
            cross_correlations: correlations(0.1),
            retrieval_rank: Some(10),
            embedder_scores: [0.2; NUM_EMBEDDERS],
            contradiction_pressure: 0.0,
            integration_confidence: 0.4,
            recurrence_count: 0,
            stability_score: 0.4,
            domain: Some("source".into()),
            successful_transfer_count: 0,
        }
    }

    fn neutral_outcome() -> LearningOutcome {
        LearningOutcome {
            label: LearningOutcomeLabel::Neutral,
            utility_delta: 0.0,
            correction_required: false,
            reuse_observed: false,
        }
    }

    #[test]
    fn no_state_change_has_zero_delta() {
        let before = state(0.5);
        let after = before.clone();
        let features = compute_learning_features(&before, &after, &neutral_outcome()).unwrap();
        println!(
            "BEFORE topic[0]={}, AFTER topic[0]={}, delta_e_scalar={}",
            before.topic_profile[0], after.topic_profile[0], features.delta_e_scalar
        );
        assert_eq!(features.delta_e_vector, [0.0; NUM_EMBEDDERS]);
        assert_eq!(features.delta_e_scalar, 0.0);
    }

    #[test]
    fn contradiction_pressure_reduces_coherence() {
        let before = state(0.5);
        let mut after = state(0.5);
        after.contradiction_pressure = 0.7;
        after.integration_confidence = 0.4;
        let features = compute_learning_features(&before, &after, &neutral_outcome()).unwrap();
        println!(
            "BEFORE contradiction={}, AFTER contradiction={}, coherence_delta={}",
            before.contradiction_pressure, after.contradiction_pressure, features.coherence_delta
        );
        assert!(features.coherence_delta < 0.0);
        assert!((features.coherence_delta + 0.7).abs() < 1e-6);
    }

    #[test]
    fn repeated_stable_memory_has_high_consolidation() {
        let before = state(0.5);
        let mut after = state(0.5);
        after.recurrence_count = 8;
        after.stability_score = 0.9;
        let features = compute_learning_features(&before, &after, &neutral_outcome()).unwrap();
        println!(
            "AFTER recurrence={}, stability={}, readiness={}",
            after.recurrence_count, after.stability_score, features.consolidation_readiness
        );
        assert!((features.consolidation_readiness - 0.85).abs() < 1e-6);
    }

    #[test]
    fn cross_domain_success_has_positive_transfer() {
        let before = state(0.5);
        let mut after = state(0.5);
        after.domain = Some("target".into());
        after.successful_transfer_count = 3;
        let mut outcome = neutral_outcome();
        outcome.utility_delta = 0.4;
        let features = compute_learning_features(&before, &after, &outcome).unwrap();
        println!(
            "BEFORE domain={:?}, AFTER domain={:?}, transfer_score={}",
            before.domain, after.domain, features.transfer_score
        );
        assert!((features.transfer_score - 0.35).abs() < 1e-6);
    }

    #[test]
    fn invalid_cross_correlation_dimension_fails() {
        let mut before = state(0.5);
        before.cross_correlations.pop();
        let after = state(0.5);
        let err = compute_learning_features(&before, &after, &neutral_outcome()).unwrap_err();
        println!("invalid dimension error={err}");
        assert!(
            matches!(err, CoreError::DimensionMismatch { expected, actual } if expected == NUM_CROSS_CORRELATIONS && actual == NUM_CROSS_CORRELATIONS - 1)
        );
    }

    #[tokio::test]
    async fn deterministic_signal_embedder_emits_expected_dimension() {
        let before = state(0.4);
        let mut after = state(0.6);
        after.retrieval_rank = Some(2);
        let event = LearningEvent::new(
            Uuid::new_v4(),
            Vec::new(),
            Some("session".into()),
            None,
            None,
            "query".into(),
            "context".into(),
            "response".into(),
            before,
            after,
            neutral_outcome(),
        )
        .unwrap();
        let embedder = DeterministicLearningSignalEmbedder::new(LearningSignalId::DeltaE);
        let signal = embedder.embed_event(&event).await.unwrap();
        println!(
            "signal_id={}, vector_len={}, scalar={}",
            signal.signal_id.as_str(),
            signal.vector.len(),
            signal.scalar
        );
        assert_eq!(signal.vector.len(), NUM_EMBEDDERS);
    }

    #[test]
    fn maximum_boundary_event_validates() {
        let before = state(0.1);
        let after = state(0.2);
        let event = LearningEvent::new(
            Uuid::new_v4(),
            (0..MAX_LEARNING_EVENT_MEMORY_IDS)
                .map(|_| Uuid::new_v4())
                .collect(),
            Some("s".repeat(MAX_LEARNING_EVENT_LABEL_CHARS)),
            Some("r".repeat(MAX_LEARNING_EVENT_LABEL_CHARS)),
            Some("t".repeat(MAX_LEARNING_EVENT_LABEL_CHARS)),
            "q".repeat(MAX_LEARNING_EVENT_TEXT_CHARS),
            String::new(),
            String::new(),
            before,
            after,
            neutral_outcome(),
        )
        .unwrap();
        println!(
            "maximum boundary memory_ids={}, query_chars={}",
            event.memory_ids.len(),
            event.query.chars().count()
        );
        assert!(event.validate().is_ok());
    }
}
