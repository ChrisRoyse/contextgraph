use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::MejepaInferError;
use crate::types::{validate_probability, ChunkId, DdaSignals};

pub const CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION: u16 = 1;
pub const CONSTELLATION_RELATIONSHIP_PATTERN_SCHEMA_VERSION: u16 = 1;
pub const CONSTELLATION_DISAGREEMENT_ACTIVE_LEARNING_REASON: &str =
    "constellation_high_value_disagreement";

const MAX_SLOT_PAIR_EVIDENCE: usize = 128;
const MAX_PATTERN_SOURCE_BYTES: usize = 1024;
const ACTIVE_EMBEDDER_SLOT_IDS: [&str; 12] = [
    "e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConstellationRelationshipKind {
    Consensus,
    Contradiction,
    Redundancy,
    BlindSpot,
    Novelty,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotPairEvidence {
    pub left_slot_id: String,
    pub right_slot_id: String,
    pub relationship_kind: ConstellationRelationshipKind,
    pub relationship_score: f32,
    pub consensus_score: f32,
    pub contradiction_score: f32,
    pub redundancy_score: f32,
    pub blind_spot_z_score: f32,
    pub novelty_score: f32,
    pub support_rows: u32,
    #[serde(default)]
    pub oracle_failure_lift_over_single_slot: Option<f32>,
    pub source: String,
}

impl SlotPairEvidence {
    pub fn validate(&self, field: &str) -> Result<(), MejepaInferError> {
        validate_text(&format!("{field}.left_slot_id"), &self.left_slot_id, 128)?;
        validate_text(&format!("{field}.right_slot_id"), &self.right_slot_id, 128)?;
        if self.left_slot_id == self.right_slot_id {
            return invalid(field, "slot-pair evidence must use distinct slots");
        }
        validate_probability(
            &format!("{field}.relationship_score"),
            self.relationship_score,
        )?;
        validate_probability(&format!("{field}.consensus_score"), self.consensus_score)?;
        validate_probability(
            &format!("{field}.contradiction_score"),
            self.contradiction_score,
        )?;
        validate_probability(&format!("{field}.redundancy_score"), self.redundancy_score)?;
        validate_probability(
            &format!("{field}.blind_spot_z_score"),
            self.blind_spot_z_score,
        )?;
        validate_probability(&format!("{field}.novelty_score"), self.novelty_score)?;
        if self.support_rows == 0 {
            return invalid(&format!("{field}.support_rows"), "must be non-zero");
        }
        if let Some(lift) = self.oracle_failure_lift_over_single_slot {
            validate_finite(
                &format!("{field}.oracle_failure_lift_over_single_slot"),
                lift,
            )?;
        }
        validate_text(
            &format!("{field}.source"),
            &self.source,
            MAX_PATTERN_SOURCE_BYTES,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationConsequencePressures {
    pub q2_verdict_pressure: f32,
    pub q3_failure_mode_pressure: f32,
    pub q4_risk_pressure: f32,
    pub q5_replay_uncertainty: f32,
}

impl ConstellationConsequencePressures {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_probability(
            "constellation.q2_verdict_pressure",
            self.q2_verdict_pressure,
        )?;
        validate_probability(
            "constellation.q3_failure_mode_pressure",
            self.q3_failure_mode_pressure,
        )?;
        validate_probability("constellation.q4_risk_pressure", self.q4_risk_pressure)?;
        validate_probability(
            "constellation.q5_replay_uncertainty",
            self.q5_replay_uncertainty,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationIntelligenceEvidence {
    pub schema_version: u16,
    pub consensus_score: f32,
    pub contradiction_score: f32,
    pub redundancy_score: f32,
    pub blind_spot_z_score: f32,
    pub novelty_score: f32,
    pub pressures: ConstellationConsequencePressures,
    pub active_learning_recommended: bool,
    #[serde(default)]
    pub relationship_pattern_id: Option<String>,
    pub slot_pair_evidence: Vec<SlotPairEvidence>,
    pub source: String,
}

impl ConstellationIntelligenceEvidence {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION {
            return invalid(
                "constellation_intelligence.schema_version",
                format!(
                    "expected {CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        validate_probability(
            "constellation_intelligence.consensus_score",
            self.consensus_score,
        )?;
        validate_probability(
            "constellation_intelligence.contradiction_score",
            self.contradiction_score,
        )?;
        validate_probability(
            "constellation_intelligence.redundancy_score",
            self.redundancy_score,
        )?;
        validate_probability(
            "constellation_intelligence.blind_spot_z_score",
            self.blind_spot_z_score,
        )?;
        validate_probability(
            "constellation_intelligence.novelty_score",
            self.novelty_score,
        )?;
        self.pressures.validate()?;
        if let Some(pattern_id) = &self.relationship_pattern_id {
            validate_text(
                "constellation_intelligence.relationship_pattern_id",
                pattern_id,
                128,
            )?;
        }
        if self.slot_pair_evidence.len() > MAX_SLOT_PAIR_EVIDENCE {
            return Err(MejepaInferError::DimMismatch {
                expected: MAX_SLOT_PAIR_EVIDENCE,
                actual: self.slot_pair_evidence.len(),
                context: "constellation_intelligence.slot_pair_evidence exceeds cap".to_string(),
            });
        }
        for (idx, evidence) in self.slot_pair_evidence.iter().enumerate() {
            evidence.validate(&format!(
                "constellation_intelligence.slot_pair_evidence[{idx}]"
            ))?;
        }
        validate_text(
            "constellation_intelligence.source",
            &self.source,
            MAX_PATTERN_SOURCE_BYTES,
        )?;
        Ok(())
    }

    pub fn neutral(source: impl Into<String>) -> Result<Self, MejepaInferError> {
        let value = Self {
            schema_version: CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION,
            consensus_score: 1.0,
            contradiction_score: 0.0,
            redundancy_score: 0.0,
            blind_spot_z_score: 0.0,
            novelty_score: 0.0,
            pressures: ConstellationConsequencePressures {
                q2_verdict_pressure: 0.0,
                q3_failure_mode_pressure: 0.0,
                q4_risk_pressure: 0.0,
                q5_replay_uncertainty: 0.0,
            },
            active_learning_recommended: false,
            relationship_pattern_id: None,
            slot_pair_evidence: Vec::new(),
            source: source.into(),
        };
        value.validate()?;
        Ok(value)
    }

    pub fn with_pattern_id(mut self, pattern_id: String) -> Result<Self, MejepaInferError> {
        validate_text(
            "constellation_intelligence.relationship_pattern_id",
            &pattern_id,
            128,
        )?;
        self.relationship_pattern_id = Some(pattern_id);
        self.validate()?;
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationRelationshipPattern {
    pub schema_version: u16,
    pub pattern_id: String,
    pub cell_id: String,
    pub pattern_kind: ConstellationRelationshipKind,
    pub source_artifact: String,
    pub source_row_count: u32,
    pub oracle_failure_count: u32,
    pub relationship_auc: f32,
    pub per_slot_baseline_auc: f32,
    pub lift_over_per_slot_baseline: f32,
    pub evidence: ConstellationIntelligenceEvidence,
    pub created_at_unix_ms: i64,
}

impl ConstellationRelationshipPattern {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        cell_id: impl Into<String>,
        pattern_kind: ConstellationRelationshipKind,
        source_artifact: impl Into<String>,
        source_row_count: u32,
        oracle_failure_count: u32,
        relationship_auc: f32,
        per_slot_baseline_auc: f32,
        evidence: ConstellationIntelligenceEvidence,
        created_at_unix_ms: i64,
    ) -> Result<Self, MejepaInferError> {
        let cell_id = cell_id.into();
        let source_artifact = source_artifact.into();
        let lift = relationship_auc - per_slot_baseline_auc;
        let pattern_id = relationship_pattern_id(
            &cell_id,
            &pattern_kind,
            &source_artifact,
            relationship_auc,
            per_slot_baseline_auc,
            &evidence.slot_pair_evidence,
        );
        let evidence = evidence.with_pattern_id(pattern_id.clone())?;
        let value = Self {
            schema_version: CONSTELLATION_RELATIONSHIP_PATTERN_SCHEMA_VERSION,
            pattern_id,
            cell_id,
            pattern_kind,
            source_artifact,
            source_row_count,
            oracle_failure_count,
            relationship_auc,
            per_slot_baseline_auc,
            lift_over_per_slot_baseline: lift,
            evidence,
            created_at_unix_ms,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != CONSTELLATION_RELATIONSHIP_PATTERN_SCHEMA_VERSION {
            return invalid(
                "constellation_pattern.schema_version",
                format!(
                    "expected {CONSTELLATION_RELATIONSHIP_PATTERN_SCHEMA_VERSION}; got {}",
                    self.schema_version
                ),
            );
        }
        validate_text("constellation_pattern.pattern_id", &self.pattern_id, 128)?;
        validate_text("constellation_pattern.cell_id", &self.cell_id, 256)?;
        validate_text(
            "constellation_pattern.source_artifact",
            &self.source_artifact,
            MAX_PATTERN_SOURCE_BYTES,
        )?;
        if self.source_row_count == 0 {
            return invalid("constellation_pattern.source_row_count", "must be non-zero");
        }
        if self.oracle_failure_count > self.source_row_count {
            return invalid(
                "constellation_pattern.oracle_failure_count",
                "cannot exceed source_row_count",
            );
        }
        validate_probability(
            "constellation_pattern.relationship_auc",
            self.relationship_auc,
        )?;
        validate_probability(
            "constellation_pattern.per_slot_baseline_auc",
            self.per_slot_baseline_auc,
        )?;
        validate_finite(
            "constellation_pattern.lift_over_per_slot_baseline",
            self.lift_over_per_slot_baseline,
        )?;
        if self.lift_over_per_slot_baseline <= 0.0 {
            return invalid(
                "constellation_pattern.lift_over_per_slot_baseline",
                "relationship pattern must beat the per-slot baseline to be promotable",
            );
        }
        self.evidence.validate()?;
        let expected_pattern_id = relationship_pattern_id(
            &self.cell_id,
            &self.pattern_kind,
            &self.source_artifact,
            self.relationship_auc,
            self.per_slot_baseline_auc,
            &self.evidence.slot_pair_evidence,
        );
        if self.pattern_id != expected_pattern_id {
            return invalid(
                "constellation_pattern.pattern_id",
                "pattern_id does not match deterministic relationship hash",
            );
        }
        if self.evidence.relationship_pattern_id.as_deref() != Some(self.pattern_id.as_str()) {
            return invalid(
                "constellation_pattern.evidence.relationship_pattern_id",
                "evidence back-pointer must match pattern_id",
            );
        }
        if self.created_at_unix_ms <= 0 {
            return invalid(
                "constellation_pattern.created_at_unix_ms",
                "must be positive",
            );
        }
        Ok(())
    }
}

pub fn constellation_pattern_key(pattern_id: &str) -> Result<Vec<u8>, MejepaInferError> {
    validate_text("constellation_pattern.pattern_id", pattern_id, 128)?;
    Ok(pattern_id.as_bytes().to_vec())
}

pub fn default_active_embedder_slot_ids(embedder_count: usize) -> Vec<String> {
    if embedder_count == ACTIVE_EMBEDDER_SLOT_IDS.len() {
        ACTIVE_EMBEDDER_SLOT_IDS
            .iter()
            .map(|slot| (*slot).to_string())
            .collect()
    } else {
        (0..embedder_count)
            .map(|idx| format!("slot_{idx:02}"))
            .collect()
    }
}

pub fn summarize_dda_constellation_intelligence(
    slot_ids: &[String],
    rows: &[(ChunkId, DdaSignals)],
    source: impl Into<String>,
) -> Result<ConstellationIntelligenceEvidence, MejepaInferError> {
    let source = source.into();
    validate_text(
        "constellation_intelligence.source",
        &source,
        MAX_PATTERN_SOURCE_BYTES,
    )?;
    if rows.is_empty() {
        return invalid(
            "constellation_intelligence.rows",
            "requires at least one DDA row",
        );
    }
    let embedder_count = rows[0].1.embedder_count();
    if embedder_count == 0 {
        return invalid(
            "constellation_intelligence.embedder_count",
            "requires at least one embedder",
        );
    }
    if slot_ids.len() != embedder_count {
        return Err(MejepaInferError::DimMismatch {
            expected: embedder_count,
            actual: slot_ids.len(),
            context: "slot_ids must match DDA embedder count".to_string(),
        });
    }
    for (idx, slot_id) in slot_ids.iter().enumerate() {
        validate_text(
            &format!("constellation_intelligence.slot_ids[{idx}]"),
            slot_id,
            128,
        )?;
    }

    let pair_count = embedder_count * embedder_count.saturating_sub(1) / 2;
    let mut pair_acc = (0..pair_count)
        .map(|idx| {
            let (left, right) = pair_slots_for_upper_triangle_index(slot_ids, idx)?;
            Ok(PairAccumulator::new(left, right))
        })
        .collect::<Result<Vec<_>, MejepaInferError>>()?;
    let mut per_embedder_sum = 0.0f64;
    let mut per_embedder_count = 0usize;

    for (row_idx, (chunk_id, signals)) in rows.iter().enumerate() {
        chunk_id.validate(&format!(
            "constellation_intelligence.rows[{row_idx}].chunk_id"
        ))?;
        signals.validate()?;
        if signals.embedder_count() != embedder_count {
            return Err(MejepaInferError::DimMismatch {
                expected: embedder_count,
                actual: signals.embedder_count(),
                context: format!("DDA row {} embedder_count mismatch", chunk_id.0),
            });
        }
        for value in &signals.per_embedder_cosine {
            per_embedder_sum += f64::from(*value);
            per_embedder_count += 1;
        }
        for (idx, acc) in pair_acc.iter_mut().enumerate() {
            let mi = signals.pairwise_mi_upper.get(idx).copied().unwrap_or(0.0);
            acc.observe(
                signals.pairwise_cosine_upper[idx],
                mi,
                signals.blind_spot_z_scores[idx],
            )?;
        }
    }

    let mut all_pair_evidence = pair_acc
        .into_iter()
        .map(|acc| acc.finish(&source))
        .collect::<Result<Vec<_>, _>>()?;

    let consensus_from_embedder = cosine_mean_to_unit(per_embedder_sum, per_embedder_count);
    let pair_consensus =
        mean_probability(all_pair_evidence.iter().map(|pair| pair.consensus_score));
    let contradiction_score = max_probability(
        all_pair_evidence
            .iter()
            .map(|pair| pair.contradiction_score),
    );
    let redundancy_score =
        mean_probability(all_pair_evidence.iter().map(|pair| pair.redundancy_score));
    let blind_spot_z_score =
        max_probability(all_pair_evidence.iter().map(|pair| pair.blind_spot_z_score));
    let novelty_score = max_probability(all_pair_evidence.iter().map(|pair| pair.novelty_score));
    let consensus_score = ((consensus_from_embedder + pair_consensus) / 2.0).clamp(0.0, 1.0);
    all_pair_evidence
        .sort_by(|left, right| pair_sort_score(right).total_cmp(&pair_sort_score(left)));
    let mut evidence = all_pair_evidence;
    evidence.truncate(12);
    let pressures = ConstellationConsequencePressures {
        q2_verdict_pressure: contradiction_score,
        q3_failure_mode_pressure: contradiction_score.max(blind_spot_z_score),
        q4_risk_pressure: novelty_score.max(blind_spot_z_score),
        q5_replay_uncertainty: (1.0 - consensus_score).max(contradiction_score),
    };
    let value = ConstellationIntelligenceEvidence {
        schema_version: CONSTELLATION_INTELLIGENCE_SCHEMA_VERSION,
        consensus_score,
        contradiction_score,
        redundancy_score,
        blind_spot_z_score,
        novelty_score,
        active_learning_recommended: contradiction_score >= 0.65
            || blind_spot_z_score >= 0.65
            || novelty_score >= 0.65,
        relationship_pattern_id: None,
        pressures,
        slot_pair_evidence: evidence,
        source,
    };
    value.validate()?;
    Ok(value)
}

pub fn granger_attestations_from_constellation_intelligence(
    evidence: &ConstellationIntelligenceEvidence,
) -> Result<BTreeMap<String, f32>, MejepaInferError> {
    evidence.validate()?;
    Ok(BTreeMap::from([
        (
            "constellation:consensus_score".to_string(),
            evidence.consensus_score,
        ),
        (
            "constellation:contradiction_score".to_string(),
            evidence.contradiction_score,
        ),
        (
            "constellation:redundancy_score".to_string(),
            evidence.redundancy_score,
        ),
        (
            "constellation:blind_spot_z_score".to_string(),
            evidence.blind_spot_z_score,
        ),
        (
            "constellation:novelty_score".to_string(),
            evidence.novelty_score,
        ),
        (
            "constellation:q2_verdict_pressure".to_string(),
            evidence.pressures.q2_verdict_pressure,
        ),
        (
            "constellation:q3_failure_mode_pressure".to_string(),
            evidence.pressures.q3_failure_mode_pressure,
        ),
        (
            "constellation:q4_risk_pressure".to_string(),
            evidence.pressures.q4_risk_pressure,
        ),
        (
            "constellation:q5_replay_uncertainty".to_string(),
            evidence.pressures.q5_replay_uncertainty,
        ),
    ]))
}

fn pair_slots_for_upper_triangle_index(
    slot_ids: &[String],
    target: usize,
) -> Result<(String, String), MejepaInferError> {
    let mut idx = 0usize;
    for left in 0..slot_ids.len() {
        for right in (left + 1)..slot_ids.len() {
            if idx == target {
                return Ok((slot_ids[left].clone(), slot_ids[right].clone()));
            }
            idx += 1;
        }
    }
    Err(MejepaInferError::DimMismatch {
        expected: slot_ids.len() * slot_ids.len().saturating_sub(1) / 2,
        actual: target + 1,
        context: "upper-triangle pair index out of bounds".to_string(),
    })
}

#[derive(Debug, Clone)]
struct PairAccumulator {
    left_slot_id: String,
    right_slot_id: String,
    rows: u32,
    pairwise_cosine_unit_sum: f64,
    disagreement_sum: f64,
    mi_sum: f64,
    max_abs_z: f32,
}

impl PairAccumulator {
    fn new(left_slot_id: String, right_slot_id: String) -> Self {
        Self {
            left_slot_id,
            right_slot_id,
            rows: 0,
            pairwise_cosine_unit_sum: 0.0,
            disagreement_sum: 0.0,
            mi_sum: 0.0,
            max_abs_z: 0.0,
        }
    }

    fn observe(
        &mut self,
        pairwise_cosine: f32,
        mi: f32,
        blind_z: f32,
    ) -> Result<(), MejepaInferError> {
        if !(-1.0..=1.0).contains(&pairwise_cosine) || !pairwise_cosine.is_finite() {
            return invalid(
                "constellation_pair.pairwise_cosine",
                "must be finite in [-1,1]",
            );
        }
        if !mi.is_finite() || mi < 0.0 {
            return invalid("constellation_pair.mi", "must be finite and non-negative");
        }
        if !blind_z.is_finite() {
            return invalid("constellation_pair.blind_z", "must be finite");
        }
        let unit = ((pairwise_cosine + 1.0) / 2.0).clamp(0.0, 1.0);
        self.rows += 1;
        self.pairwise_cosine_unit_sum += f64::from(unit);
        self.disagreement_sum += f64::from(1.0 - unit);
        self.mi_sum += f64::from(mi);
        self.max_abs_z = self.max_abs_z.max(blind_z.abs());
        Ok(())
    }

    fn finish(self, source: &str) -> Result<SlotPairEvidence, MejepaInferError> {
        if self.rows == 0 {
            return invalid("constellation_pair.rows", "must be non-zero");
        }
        let rows = f64::from(self.rows);
        let consensus_score = (self.pairwise_cosine_unit_sum / rows).clamp(0.0, 1.0) as f32;
        let pairwise_disagreement = (self.disagreement_sum / rows).clamp(0.0, 1.0) as f32;
        let blind_spot_z_score = z_to_probability(self.max_abs_z);
        let mi_mean = (self.mi_sum / rows).max(0.0) as f32;
        let redundancy_score = (consensus_score * (mi_mean / (1.0 + mi_mean))).clamp(0.0, 1.0);
        let contradiction_score = pairwise_disagreement.max(blind_spot_z_score);
        let novelty_score =
            blind_spot_z_score.max(pairwise_disagreement * (1.0 - redundancy_score));
        let relationship_kind = if contradiction_score >= 0.65 {
            ConstellationRelationshipKind::Contradiction
        } else if novelty_score >= 0.65 {
            ConstellationRelationshipKind::Novelty
        } else if redundancy_score >= 0.65 {
            ConstellationRelationshipKind::Redundancy
        } else if blind_spot_z_score >= 0.5 {
            ConstellationRelationshipKind::BlindSpot
        } else {
            ConstellationRelationshipKind::Consensus
        };
        let value = SlotPairEvidence {
            left_slot_id: self.left_slot_id,
            right_slot_id: self.right_slot_id,
            relationship_kind,
            relationship_score: contradiction_score
                .max(novelty_score)
                .max(redundancy_score)
                .max(consensus_score),
            consensus_score,
            contradiction_score,
            redundancy_score,
            blind_spot_z_score,
            novelty_score,
            support_rows: self.rows,
            oracle_failure_lift_over_single_slot: None,
            source: source.to_string(),
        };
        value.validate("constellation_pair")?;
        Ok(value)
    }
}

fn relationship_pattern_id(
    cell_id: &str,
    kind: &ConstellationRelationshipKind,
    source_artifact: &str,
    relationship_auc: f32,
    per_slot_baseline_auc: f32,
    pair_evidence: &[SlotPairEvidence],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cell_id.as_bytes());
    hasher.update(format!("{kind:?}").as_bytes());
    hasher.update(source_artifact.as_bytes());
    hasher.update(relationship_auc.to_le_bytes());
    hasher.update(per_slot_baseline_auc.to_le_bytes());
    for pair in pair_evidence {
        hasher.update(pair.left_slot_id.as_bytes());
        hasher.update(pair.right_slot_id.as_bytes());
        hasher.update(pair.relationship_score.to_le_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn pair_sort_score(pair: &SlotPairEvidence) -> f32 {
    pair.contradiction_score
        .max(pair.novelty_score)
        .max(pair.blind_spot_z_score)
        .max(pair.redundancy_score)
}

fn cosine_mean_to_unit(sum: f64, n: usize) -> f32 {
    if n == 0 {
        0.5
    } else {
        (((sum / n as f64) + 1.0) / 2.0).clamp(0.0, 1.0) as f32
    }
}

fn mean_probability(values: impl IntoIterator<Item = f32>) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for value in values {
        sum += value.clamp(0.0, 1.0);
        n += 1;
    }
    if n == 0 {
        0.0
    } else {
        (sum / n as f32).clamp(0.0, 1.0)
    }
}

fn max_probability(values: impl IntoIterator<Item = f32>) -> f32 {
    values
        .into_iter()
        .fold(0.0f32, |acc, value| acc.max(value.clamp(0.0, 1.0)))
}

fn z_to_probability(abs_z: f32) -> f32 {
    (abs_z / (1.0 + abs_z)).clamp(0.0, 1.0)
}

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > max_bytes {
        return Err(MejepaInferError::DimMismatch {
            expected: max_bytes,
            actual: value.len(),
            context: field.to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return invalid(field, "must not contain control characters");
    }
    Ok(())
}

fn validate_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() {
        return Err(MejepaInferError::NanDetected {
            nan_source: field.to_string(),
            detail: format!("value must be finite; got {value}"),
        });
    }
    Ok(())
}

fn invalid<T>(field: &str, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signals(pairwise: Vec<f32>, z: Vec<f32>) -> DdaSignals {
        DdaSignals::try_new(DdaSignals {
            per_embedder_cosine: vec![0.8, 0.7, 0.6],
            pairwise_cosine_upper: pairwise,
            pairwise_mi_upper: vec![0.1, 0.2, 0.1],
            blind_spot_z_scores: z,
        })
        .expect("signals")
    }

    fn signals_for(
        embedder_count: usize,
        pairwise: Vec<f32>,
        mi: Vec<f32>,
        z: Vec<f32>,
    ) -> DdaSignals {
        DdaSignals::try_new(DdaSignals {
            per_embedder_cosine: vec![0.8; embedder_count],
            pairwise_cosine_upper: pairwise,
            pairwise_mi_upper: mi,
            blind_spot_z_scores: z,
        })
        .expect("signals")
    }

    #[test]
    fn disagreement_pair_drives_pressures_and_active_learning() {
        let rows = vec![(
            ChunkId("chunk-a".to_string()),
            signals(vec![-0.9, 0.95, 0.90], vec![4.0, 0.0, 0.0]),
        )];
        let slot_ids = vec!["e1".to_string(), "e2".to_string(), "e3".to_string()];
        let evidence =
            summarize_dda_constellation_intelligence(&slot_ids, &rows, "unit").expect("evidence");
        assert!(evidence.contradiction_score > 0.75);
        assert!(evidence.pressures.q2_verdict_pressure > 0.75);
        assert!(evidence.active_learning_recommended);
        assert_eq!(evidence.slot_pair_evidence[0].left_slot_id, "e1");
        assert_eq!(evidence.slot_pair_evidence[0].right_slot_id, "e2");
    }

    #[test]
    fn relationship_pattern_requires_positive_lift() {
        let evidence = ConstellationIntelligenceEvidence::neutral("unit")
            .expect("neutral")
            .with_pattern_id("sha256:abc".to_string())
            .expect("pattern id");
        let rejected = ConstellationRelationshipPattern::try_new(
            "python:known_good",
            ConstellationRelationshipKind::Consensus,
            "unit",
            10,
            1,
            0.5,
            0.5,
            evidence,
            1,
        );
        assert!(rejected.is_err());
    }

    #[test]
    fn aggregate_pressure_uses_all_pairs_before_display_truncation() {
        let embedder_count = 6;
        let pair_count = embedder_count * (embedder_count - 1) / 2;
        let mut pairwise = vec![1.0; pair_count];
        let mi = vec![100.0; pair_count];
        let mut z = vec![0.0; pair_count];
        pairwise[pair_count - 1] = -0.92;
        z[pair_count - 1] = 0.0;
        let rows = vec![(
            ChunkId("chunk-truncated-contradiction".to_string()),
            signals_for(embedder_count, pairwise, mi, z),
        )];
        let slot_ids = (0..embedder_count)
            .map(|idx| format!("e{idx}"))
            .collect::<Vec<_>>();

        let evidence =
            summarize_dda_constellation_intelligence(&slot_ids, &rows, "unit").expect("evidence");

        assert_eq!(evidence.slot_pair_evidence.len(), 12);
        assert!(
            !evidence
                .slot_pair_evidence
                .iter()
                .any(|pair| pair.contradiction_score > 0.9),
            "the stored display slice intentionally drops the lower-ranked contradiction"
        );
        assert!(
            evidence.pressures.q2_verdict_pressure > 0.9,
            "aggregate pressure must still see all 15 slot pairs"
        );
    }

    #[test]
    fn relationship_pattern_validation_requires_evidence_back_pointer() {
        let rows = vec![(
            ChunkId("chunk-a".to_string()),
            signals(vec![-0.9, 0.95, 0.90], vec![4.0, 0.0, 0.0]),
        )];
        let slot_ids = vec!["e1".to_string(), "e2".to_string(), "e3".to_string()];
        let evidence =
            summarize_dda_constellation_intelligence(&slot_ids, &rows, "unit").expect("evidence");
        let mut pattern = ConstellationRelationshipPattern::try_new(
            "python:wrong_file",
            ConstellationRelationshipKind::Contradiction,
            "unit",
            10,
            2,
            0.8,
            0.6,
            evidence,
            1,
        )
        .expect("pattern");
        pattern.evidence.relationship_pattern_id = Some("sha256:stale".to_string());

        assert!(pattern.validate().is_err());
    }
}
