use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::dynamic_embedder::RuntimeEmbedderId;
use crate::embedder_proposal::PendingEmbedderProposal;
use crate::error::MejepaInferError;
use crate::heal::per_cell_promotion::{
    choose_winner_by_phase_e_score, score_holdout, PromotionScore,
};
use crate::heal::promote::{HoldoutEval, ModeWinner};

pub const EMBEDDER_FALSIFICATION_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_EMBEDDER_FALSIFICATION_DELTA_GLOBAL: f32 = 0.005;
pub const DEFAULT_EMBEDDER_FALSIFICATION_DELTA_CELL: f32 = 0.005;
pub const MEJEPA_EMBEDDER_FALSIFICATION_HOLDOUT_OVERLAP: &str =
    "MEJEPA_EMBEDDER_FALSIFICATION_HOLDOUT_OVERLAP";
pub const MEJEPA_EMBEDDER_FALSIFICATION_NO_CANDIDATE_WINNER: &str =
    "MEJEPA_EMBEDDER_FALSIFICATION_NO_CANDIDATE_WINNER";
pub const MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL: &str =
    "MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL";
pub const MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION: &str =
    "MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION";

const MODE_A_LATENCY_MULTIPLIER: f32 = 1.0;
const MODE_B_LATENCY_MULTIPLIER: f32 = 1.05;
const MODE_C_LATENCY_MULTIPLIER: f32 = 1.15;
const FLOAT_EPSILON: f32 = 1e-6;

type CellComparisonResult = (BTreeMap<String, CellFalsificationDelta>, Vec<String>, f32);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderFalsificationGate {
    pub delta_global: f32,
    pub delta_cell: f32,
    pub mode_a_latency_multiplier: f32,
    pub mode_b_latency_multiplier: f32,
    pub mode_c_latency_multiplier: f32,
}

impl Default for EmbedderFalsificationGate {
    fn default() -> Self {
        Self {
            delta_global: DEFAULT_EMBEDDER_FALSIFICATION_DELTA_GLOBAL,
            delta_cell: DEFAULT_EMBEDDER_FALSIFICATION_DELTA_CELL,
            mode_a_latency_multiplier: MODE_A_LATENCY_MULTIPLIER,
            mode_b_latency_multiplier: MODE_B_LATENCY_MULTIPLIER,
            mode_c_latency_multiplier: MODE_C_LATENCY_MULTIPLIER,
        }
    }
}

impl EmbedderFalsificationGate {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_non_negative_finite("gate.delta_global", self.delta_global)?;
        validate_non_negative_finite("gate.delta_cell", self.delta_cell)?;
        validate_positive_finite(
            "gate.mode_a_latency_multiplier",
            self.mode_a_latency_multiplier,
        )?;
        validate_positive_finite(
            "gate.mode_b_latency_multiplier",
            self.mode_b_latency_multiplier,
        )?;
        validate_positive_finite(
            "gate.mode_c_latency_multiplier",
            self.mode_c_latency_multiplier,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderCandidateHoldoutComparison {
    pub schema_version: u32,
    pub proposal_id: [u8; 16],
    pub candidate_id: RuntimeEmbedderId,
    pub candidate_name: String,
    pub candidate_architecture_signature: String,
    pub candidate_artifact_sha256: String,
    pub training_cert_chain_hash: String,
    pub proposal_source_refs: Vec<String>,
    pub proposer_used_window_ids: Vec<String>,
    pub heldout_window_ids: Vec<String>,
    pub mode_a: HoldoutEval,
    pub mode_b: HoldoutEval,
    pub mode_c: HoldoutEval,
    pub created_at_unix_ms: i64,
}

impl EmbedderCandidateHoldoutComparison {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_FALSIFICATION_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {EMBEDDER_FALSIFICATION_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_proposal_id(self.proposal_id)?;
        self.candidate_id.validate().map_err(embed_error)?;
        validate_text("candidate_name", &self.candidate_name, 256)?;
        validate_text(
            "candidate_architecture_signature",
            &self.candidate_architecture_signature,
            512,
        )?;
        validate_sha256("candidate_artifact_sha256", &self.candidate_artifact_sha256)?;
        validate_sha256("training_cert_chain_hash", &self.training_cert_chain_hash)?;
        validate_non_empty_texts("proposal_source_refs", &self.proposal_source_refs, 512)?;
        validate_non_empty_texts(
            "proposer_used_window_ids",
            &self.proposer_used_window_ids,
            256,
        )?;
        validate_non_empty_texts("heldout_window_ids", &self.heldout_window_ids, 256)?;
        validate_holdout_eval("mode_a", &self.mode_a)?;
        validate_holdout_eval("mode_b", &self.mode_b)?;
        validate_holdout_eval("mode_c", &self.mode_c)?;
        if self.created_at_unix_ms <= 0 {
            return invalid("created_at_unix_ms", "must be positive");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CellFalsificationDelta {
    pub before: f32,
    pub after: Option<f32>,
    pub delta: f32,
    pub holds_or_improves: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderFalsificationDecision {
    pub schema_version: u32,
    pub proposal_id: [u8; 16],
    pub candidate_id: RuntimeEmbedderId,
    pub candidate_name: String,
    pub accepted: bool,
    pub winner: ModeWinner,
    pub reason_code: Option<String>,
    pub reason: Option<String>,
    pub global_delta: f32,
    pub min_cell_delta: f32,
    pub mode_a_score: PromotionScore,
    pub mode_b_score: PromotionScore,
    pub mode_c_score: PromotionScore,
    pub compared_cells: BTreeMap<String, CellFalsificationDelta>,
    pub regressing_cells: Vec<String>,
    pub overlapping_window_ids: Vec<String>,
    pub proposer_used_window_ids: Vec<String>,
    pub heldout_window_ids: Vec<String>,
    pub source_of_truth_cf: String,
    pub source_of_truth_key_hex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderProposalRejectionRecord {
    pub schema_version: u32,
    pub rejection_id: [u8; 16],
    pub proposal_id: [u8; 16],
    pub candidate_id: RuntimeEmbedderId,
    pub candidate_name: String,
    pub candidate_architecture_signature: String,
    pub candidate_artifact_sha256: String,
    pub training_cert_chain_hash: String,
    pub proposal_source_refs: Vec<String>,
    pub reason_code: String,
    pub reason: String,
    pub winner: ModeWinner,
    pub global_delta: f32,
    pub min_cell_delta: f32,
    pub mode_a_score: PromotionScore,
    pub mode_b_score: PromotionScore,
    pub mode_c_score: PromotionScore,
    pub compared_cells: BTreeMap<String, CellFalsificationDelta>,
    pub regressing_cells: Vec<String>,
    pub overlapping_window_ids: Vec<String>,
    pub proposer_used_window_ids: Vec<String>,
    pub heldout_window_ids: Vec<String>,
    pub created_at_unix_ms: i64,
    pub source_of_truth_cf: String,
}

impl EmbedderProposalRejectionRecord {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_FALSIFICATION_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {EMBEDDER_FALSIFICATION_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        validate_proposal_id(self.rejection_id)?;
        validate_proposal_id(self.proposal_id)?;
        self.candidate_id.validate().map_err(embed_error)?;
        validate_text("candidate_name", &self.candidate_name, 256)?;
        validate_text(
            "candidate_architecture_signature",
            &self.candidate_architecture_signature,
            512,
        )?;
        validate_sha256("candidate_artifact_sha256", &self.candidate_artifact_sha256)?;
        validate_sha256("training_cert_chain_hash", &self.training_cert_chain_hash)?;
        validate_non_empty_texts("proposal_source_refs", &self.proposal_source_refs, 512)?;
        validate_text("reason_code", &self.reason_code, 128)?;
        validate_text("reason", &self.reason, 1024)?;
        validate_finite("global_delta", self.global_delta)?;
        validate_finite("min_cell_delta", self.min_cell_delta)?;
        validate_cells("compared_cells", &self.compared_cells)?;
        validate_texts("regressing_cells", &self.regressing_cells, 256)?;
        validate_texts("overlapping_window_ids", &self.overlapping_window_ids, 256)?;
        validate_non_empty_texts(
            "proposer_used_window_ids",
            &self.proposer_used_window_ids,
            256,
        )?;
        validate_non_empty_texts("heldout_window_ids", &self.heldout_window_ids, 256)?;
        if self.created_at_unix_ms <= 0 {
            return invalid("created_at_unix_ms", "must be positive");
        }
        if self.source_of_truth_cf != CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS}"),
            );
        }
        Ok(())
    }
}

pub fn proposer_used_window_ids_from_proposal(
    proposal: &PendingEmbedderProposal,
) -> Result<Vec<String>, MejepaInferError> {
    proposal.validate()?;
    let mut windows = BTreeSet::new();
    for source in &proposal.source_signals {
        for window_id in &source.used_window_ids {
            windows.insert(window_id.clone());
        }
    }
    Ok(windows.into_iter().collect())
}

pub fn evaluate_embedder_candidate_falsification(
    comparison: &EmbedderCandidateHoldoutComparison,
    gate: EmbedderFalsificationGate,
) -> Result<EmbedderFalsificationDecision, MejepaInferError> {
    comparison.validate()?;
    gate.validate()?;

    let mode_a_score =
        score_holdout(&comparison.mode_a, gate.mode_a_latency_multiplier).map_err(heal_error)?;
    let mode_b_score =
        score_holdout(&comparison.mode_b, gate.mode_b_latency_multiplier).map_err(heal_error)?;
    let mode_c_score =
        score_holdout(&comparison.mode_c, gate.mode_c_latency_multiplier).map_err(heal_error)?;
    let winner = choose_winner_by_phase_e_score(
        &comparison.mode_a,
        &comparison.mode_b,
        &comparison.mode_c,
        gate.mode_a_latency_multiplier,
        gate.mode_b_latency_multiplier,
        gate.mode_c_latency_multiplier,
    )
    .map_err(heal_error)?;
    let winner_eval = match winner {
        ModeWinner::B => &comparison.mode_b,
        ModeWinner::C => &comparison.mode_c,
        _ => &comparison.mode_a,
    };
    let global_delta = winner_eval.oracle_agreement - comparison.mode_a.oracle_agreement;
    let (compared_cells, mut regressing_cells, min_cell_delta) =
        compare_cells(&comparison.mode_a, winner_eval, gate.delta_cell)?;
    let overlapping_window_ids = heldout_overlap(
        &comparison.proposer_used_window_ids,
        &comparison.heldout_window_ids,
    );
    let (reason_code, reason) = if !overlapping_window_ids.is_empty() {
        (
            Some(MEJEPA_EMBEDDER_FALSIFICATION_HOLDOUT_OVERLAP.to_string()),
            Some(format!(
                "held-out windows were visible to proposer: {}",
                overlapping_window_ids.join(",")
            )),
        )
    } else if global_delta + FLOAT_EPSILON < gate.delta_global {
        (
            Some(MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL.to_string()),
            Some(format!(
                "global oracle-correlation delta {global_delta:.6} below required {:.6}",
                gate.delta_global
            )),
        )
    } else if !winner.is_promoted() {
        (
            Some(MEJEPA_EMBEDDER_FALSIFICATION_NO_CANDIDATE_WINNER.to_string()),
            Some("candidate mode did not beat baseline under Phase-E score".to_string()),
        )
    } else if !regressing_cells.is_empty() {
        regressing_cells.sort();
        (
            Some(MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION.to_string()),
            Some(format!(
                "{} cells regressed beyond delta_cell {:.6}",
                regressing_cells.len(),
                gate.delta_cell
            )),
        )
    } else {
        (None, None)
    };
    let accepted = reason_code.is_none();
    Ok(EmbedderFalsificationDecision {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        proposal_id: comparison.proposal_id,
        candidate_id: comparison.candidate_id.clone(),
        candidate_name: comparison.candidate_name.clone(),
        accepted,
        winner,
        reason_code,
        reason,
        global_delta,
        min_cell_delta,
        mode_a_score,
        mode_b_score,
        mode_c_score,
        compared_cells,
        regressing_cells,
        overlapping_window_ids,
        proposer_used_window_ids: sorted_unique(&comparison.proposer_used_window_ids),
        heldout_window_ids: sorted_unique(&comparison.heldout_window_ids),
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
        source_of_truth_key_hex: None,
    })
}

pub fn evaluate_and_persist_embedder_falsification(
    db: &DB,
    comparison: &EmbedderCandidateHoldoutComparison,
    gate: EmbedderFalsificationGate,
) -> Result<EmbedderFalsificationDecision, MejepaInferError> {
    let mut decision = evaluate_embedder_candidate_falsification(comparison, gate)?;
    if decision.accepted {
        return Ok(decision);
    }
    let record = rejection_record_from_decision(comparison, &decision)?;
    let key = write_embedder_proposal_rejection_sync_readback(db, &record)?;
    decision.source_of_truth_key_hex = Some(hex::encode(key));
    Ok(decision)
}

pub fn write_embedder_proposal_rejection_sync_readback(
    db: &DB,
    record: &EmbedderProposalRejectionRecord,
) -> Result<Vec<u8>, MejepaInferError> {
    record.validate()?;
    let cf = cf(db, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)?;
    let key = embedder_proposal_rejection_key(record.rejection_id);
    let value = bincode::serialize(record)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    db.put_cf_opt(cf, &key, &value, &opts)?;
    db.flush_cf(cf)?;
    let readback = db
        .get_cf(cf, &key)?
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "embedder_proposal_rejection.readback".to_string(),
            detail: "read-after-write could not find persisted rejection".to_string(),
        })?;
    if readback != value {
        return invalid(
            "embedder_proposal_rejection.readback",
            "read-after-write bytes differ from encoded rejection",
        );
    }
    let decoded: EmbedderProposalRejectionRecord = bincode::deserialize(&readback)?;
    decoded.validate()?;
    if decoded != *record {
        return invalid(
            "embedder_proposal_rejection.readback",
            "read-after-write decoded rejection differs from input",
        );
    }
    Ok(key)
}

pub fn read_embedder_proposal_rejection(
    db: &DB,
    rejection_id: [u8; 16],
) -> Result<Option<EmbedderProposalRejectionRecord>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)?;
    let Some(bytes) = db.get_cf(cf, embedder_proposal_rejection_key(rejection_id))? else {
        return Ok(None);
    };
    let record: EmbedderProposalRejectionRecord = bincode::deserialize(&bytes)?;
    record.validate()?;
    Ok(Some(record))
}

pub fn read_embedder_proposal_rejections(
    db: &DB,
) -> Result<Vec<EmbedderProposalRejectionRecord>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let row: EmbedderProposalRejectionRecord = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push(row);
    }
    rows.sort_by(|left, right| {
        left.candidate_architecture_signature
            .cmp(&right.candidate_architecture_signature)
            .then_with(|| left.reason_code.cmp(&right.reason_code))
            .then_with(|| left.candidate_name.cmp(&right.candidate_name))
    });
    Ok(rows)
}

pub fn embedder_architecture_has_rejection(
    db: &DB,
    candidate_architecture_signature: &str,
) -> Result<bool, MejepaInferError> {
    validate_text(
        "candidate_architecture_signature",
        candidate_architecture_signature,
        512,
    )?;
    Ok(read_embedder_proposal_rejections(db)?
        .iter()
        .any(|record| record.candidate_architecture_signature == candidate_architecture_signature))
}

pub fn embedder_proposal_rejection_key(rejection_id: [u8; 16]) -> Vec<u8> {
    rejection_id.to_vec()
}

fn rejection_record_from_decision(
    comparison: &EmbedderCandidateHoldoutComparison,
    decision: &EmbedderFalsificationDecision,
) -> Result<EmbedderProposalRejectionRecord, MejepaInferError> {
    let reason_code =
        decision
            .reason_code
            .clone()
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "falsification_decision.reason_code".to_string(),
                detail: "accepted decisions are not rejection records".to_string(),
            })?;
    let reason = decision
        .reason
        .clone()
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "falsification_decision.reason".to_string(),
            detail: "accepted decisions are not rejection records".to_string(),
        })?;
    let record = EmbedderProposalRejectionRecord {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        rejection_id: rejection_id(comparison, &reason_code),
        proposal_id: comparison.proposal_id,
        candidate_id: comparison.candidate_id.clone(),
        candidate_name: comparison.candidate_name.clone(),
        candidate_architecture_signature: comparison.candidate_architecture_signature.clone(),
        candidate_artifact_sha256: comparison.candidate_artifact_sha256.clone(),
        training_cert_chain_hash: comparison.training_cert_chain_hash.clone(),
        proposal_source_refs: sorted_unique(&comparison.proposal_source_refs),
        reason_code,
        reason,
        winner: decision.winner,
        global_delta: decision.global_delta,
        min_cell_delta: decision.min_cell_delta,
        mode_a_score: decision.mode_a_score.clone(),
        mode_b_score: decision.mode_b_score.clone(),
        mode_c_score: decision.mode_c_score.clone(),
        compared_cells: decision.compared_cells.clone(),
        regressing_cells: decision.regressing_cells.clone(),
        overlapping_window_ids: decision.overlapping_window_ids.clone(),
        proposer_used_window_ids: decision.proposer_used_window_ids.clone(),
        heldout_window_ids: decision.heldout_window_ids.clone(),
        created_at_unix_ms: comparison.created_at_unix_ms,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    };
    record.validate()?;
    Ok(record)
}

fn rejection_id(comparison: &EmbedderCandidateHoldoutComparison, reason_code: &str) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_EMBEDDER_PROPOSAL_REJECTION_V1");
    hasher.update(comparison.proposal_id);
    hasher.update(comparison.candidate_id.slug().as_bytes());
    hasher.update(comparison.candidate_architecture_signature.as_bytes());
    hasher.update(comparison.candidate_artifact_sha256.as_bytes());
    hasher.update(reason_code.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn compare_cells(
    before: &HoldoutEval,
    after: &HoldoutEval,
    tolerance: f32,
) -> Result<CellComparisonResult, MejepaInferError> {
    validate_non_negative_finite("delta_cell", tolerance)?;
    let mut compared = BTreeMap::new();
    let mut regressing = Vec::new();
    let mut min_delta = f32::INFINITY;
    for (cell, before_value) in &before.per_cell_correlation {
        let after_value = after.per_cell_correlation.get(cell).copied();
        let delta = after_value.unwrap_or(0.0) - before_value;
        min_delta = min_delta.min(delta);
        let holds = after_value
            .map(|value| value - before_value + tolerance + FLOAT_EPSILON >= 0.0)
            .unwrap_or(false);
        if !holds {
            regressing.push(cell.clone());
        }
        compared.insert(
            cell.clone(),
            CellFalsificationDelta {
                before: *before_value,
                after: after_value,
                delta,
                holds_or_improves: holds,
            },
        );
    }
    if compared.is_empty() {
        return invalid(
            "holdout_eval.per_cell_correlation",
            "baseline must contain at least one per-cell correlation",
        );
    }
    Ok((compared, regressing, min_delta))
}

fn heldout_overlap(
    proposer_used_window_ids: &[String],
    heldout_window_ids: &[String],
) -> Vec<String> {
    let proposer = proposer_used_window_ids.iter().collect::<BTreeSet<_>>();
    let mut overlap = heldout_window_ids
        .iter()
        .filter(|window| proposer.contains(window))
        .cloned()
        .collect::<Vec<_>>();
    overlap.sort();
    overlap.dedup();
    overlap
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn validate_holdout_eval(field: &str, eval: &HoldoutEval) -> Result<(), MejepaInferError> {
    validate_probability(&format!("{field}.coverage"), eval.coverage)?;
    validate_probability(&format!("{field}.oracle_agreement"), eval.oracle_agreement)?;
    validate_non_negative_finite(
        &format!("{field}.ood_distribution_kl"),
        eval.ood_distribution_kl,
    )?;
    if eval.num_samples == 0 {
        return invalid(format!("{field}.num_samples"), "must be greater than zero");
    }
    validate_cells(
        &format!("{field}.per_cell_correlation"),
        &eval
            .per_cell_correlation
            .iter()
            .map(|(cell, value)| {
                (
                    cell.clone(),
                    CellFalsificationDelta {
                        before: *value,
                        after: Some(*value),
                        delta: 0.0,
                        holds_or_improves: true,
                    },
                )
            })
            .collect(),
    )
}

fn validate_cells(
    field: &str,
    values: &BTreeMap<String, CellFalsificationDelta>,
) -> Result<(), MejepaInferError> {
    if values.is_empty() {
        return invalid(field, "must be non-empty");
    }
    for (cell, delta) in values {
        validate_text(&format!("{field}.{cell}.cell"), cell, 256)?;
        validate_probability(&format!("{field}.{cell}.before"), delta.before)?;
        if let Some(after) = delta.after {
            validate_probability(&format!("{field}.{cell}.after"), after)?;
        }
        validate_finite(&format!("{field}.{cell}.delta"), delta.delta)?;
    }
    Ok(())
}

fn validate_texts(field: &str, values: &[String], max_len: usize) -> Result<(), MejepaInferError> {
    for value in values {
        validate_text(field, value, max_len)?;
    }
    Ok(())
}

fn validate_non_empty_texts(
    field: &str,
    values: &[String],
    max_len: usize,
) -> Result<(), MejepaInferError> {
    if values.is_empty() {
        return invalid(field, "must be non-empty");
    }
    validate_texts(field, values, max_len)
}

fn validate_text(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty()
        || value.trim() != value
        || value.len() > max_len
        || value.bytes().any(|byte| byte == 0 || byte == b'\n')
    {
        return invalid(
            field,
            format!("must be non-empty trimmed single-line text <= {max_len} bytes"),
        );
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), MejepaInferError> {
    validate_text(field, value, 64)?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(field, "must be a 64-character sha256 hex digest");
    }
    Ok(())
}

fn validate_proposal_id(value: [u8; 16]) -> Result<(), MejepaInferError> {
    if value.iter().all(|byte| *byte == 0) {
        return invalid("proposal_id", "must be non-zero");
    }
    Ok(())
}

fn validate_probability(field: &str, value: f32) -> Result<(), MejepaInferError> {
    validate_non_negative_finite(field, value)?;
    if value > 1.0 {
        return invalid(field, "must be <= 1.0");
    }
    Ok(())
}

fn validate_non_negative_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    validate_finite(field, value)?;
    if value < 0.0 {
        return invalid(field, "must be non-negative");
    }
    Ok(())
}

fn validate_positive_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    validate_finite(field, value)?;
    if value <= 0.0 {
        return invalid(field, "must be positive");
    }
    Ok(())
}

fn validate_finite(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() {
        return invalid(field, "must be finite");
    }
    Ok(())
}

fn invalid<T>(field: impl Into<String>, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}

fn heal_error(err: crate::heal::errors::HealError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "embedder_falsification.promotion_gate".to_string(),
        detail: err.to_string(),
    }
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "embedder_falsification.candidate_id".to_string(),
        detail: err.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::open_infer_rocksdb;

    #[test]
    fn accepts_known_positive_candidate() {
        let comparison = comparison_fixture(
            "positive",
            0.906,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.866),
                ("typing:python".to_string(), 0.856),
            ]),
        );
        let decision = evaluate_embedder_candidate_falsification(
            &comparison,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(decision.accepted);
        assert_eq!(decision.winner, ModeWinner::B);
        assert!(decision.global_delta >= DEFAULT_EMBEDDER_FALSIFICATION_DELTA_GLOBAL);
        assert!(decision.regressing_cells.is_empty());
        assert!(decision.reason_code.is_none());
    }

    #[test]
    fn rejects_cell_regression_and_persists_replay_blocker() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path()).unwrap();
        let comparison = comparison_fixture(
            "regression",
            0.908,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.866),
                ("typing:python".to_string(), 0.844),
            ]),
        );

        let decision = evaluate_and_persist_embedder_falsification(
            db.as_ref(),
            &comparison,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!decision.accepted);
        assert_eq!(
            decision.reason_code.as_deref(),
            Some(MEJEPA_EMBEDDER_FALSIFICATION_CELL_REGRESSION)
        );
        assert_eq!(
            read_embedder_proposal_rejections(db.as_ref())
                .unwrap()
                .len(),
            1
        );
        assert!(embedder_architecture_has_rejection(
            db.as_ref(),
            &comparison.candidate_architecture_signature
        )
        .unwrap());
    }

    #[test]
    fn rejects_zero_delta() {
        let comparison = comparison_fixture(
            "zero",
            0.900,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.860),
                ("typing:python".to_string(), 0.850),
            ]),
        );
        let decision = evaluate_embedder_candidate_falsification(
            &comparison,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!decision.accepted);
        assert_eq!(
            decision.reason_code.as_deref(),
            Some(MEJEPA_EMBEDDER_FALSIFICATION_GLOBAL_DELTA_TOO_SMALL)
        );
    }

    #[test]
    fn rejects_seen_holdout_window() {
        let mut comparison = comparison_fixture(
            "overlap",
            0.906,
            BTreeMap::from([
                ("mutation:python".to_string(), 0.866),
                ("typing:python".to_string(), 0.856),
            ]),
        );
        comparison.heldout_window_ids = vec!["proposal-window-a".to_string()];
        let decision = evaluate_embedder_candidate_falsification(
            &comparison,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!decision.accepted);
        assert_eq!(
            decision.reason_code.as_deref(),
            Some(MEJEPA_EMBEDDER_FALSIFICATION_HOLDOUT_OVERLAP)
        );
    }

    fn comparison_fixture(
        suffix: &str,
        mode_b_global: f32,
        mode_b_cells: BTreeMap<String, f32>,
    ) -> EmbedderCandidateHoldoutComparison {
        EmbedderCandidateHoldoutComparison {
            schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
            proposal_id: [7u8; 16],
            candidate_id: RuntimeEmbedderId::dynamic(2, format!("{suffix}_candidate")).unwrap(),
            candidate_name: format!("fixture_{suffix}_candidate"),
            candidate_architecture_signature: format!("fixture-architecture-{suffix}"),
            candidate_artifact_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            training_cert_chain_hash:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            proposal_source_refs: vec!["unknown_cluster:fixture".to_string()],
            proposer_used_window_ids: vec!["proposal-window-a".to_string()],
            heldout_window_ids: vec!["heldout-window-b".to_string()],
            mode_a: eval(
                0.900,
                BTreeMap::from([
                    ("mutation:python".to_string(), 0.860),
                    ("typing:python".to_string(), 0.850),
                ]),
            ),
            mode_b: eval(mode_b_global, mode_b_cells),
            mode_c: eval(
                0.898,
                BTreeMap::from([
                    ("mutation:python".to_string(), 0.858),
                    ("typing:python".to_string(), 0.848),
                ]),
            ),
            created_at_unix_ms: 1_779_100_000_000,
        }
    }

    fn eval(global: f32, cells: BTreeMap<String, f32>) -> HoldoutEval {
        HoldoutEval::try_new_with_cells(0.95, global, 0.01, 256, [9u8; 32], cells).unwrap()
    }
}
