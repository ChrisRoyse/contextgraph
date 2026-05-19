use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_EMBEDDER_PROPOSALS, CF_MEJEPA_MINCUT_REPORTS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::eval::{ActiveLearningKind, ActiveLearningQueueEntry, ActiveLearningQueueState};
use crate::mincut_panel::MincutReport;
use crate::pairwise_mi::{read_pairwise_mi_matrix, PairwiseMiPersistedMatrix};

pub const EMBEDDER_PROPOSAL_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_EMBEDDER_PROPOSAL_MAX_PROPOSALS: usize = 32;
pub const DEFAULT_EMBEDDER_PROPOSAL_MIN_SIGNAL_MAGNITUDE: f32 = 0.01;
pub const DEFAULT_EMBEDDER_PROPOSAL_MIN_COMPOSITE_SCORE: f32 = 0.0;
pub const DEFAULT_EMBEDDER_PROPOSAL_PAIRWISE_MI_MAX_ROWS: usize = 1_000_000;
pub const MEJEPA_PENDING_EMBEDDER_PROPOSALS_EMPTY_SUBSTRATE: &str =
    "MEJEPA_PENDING_EMBEDDER_PROPOSALS_EMPTY_SUBSTRATE";
pub const MEJEPA_PENDING_EMBEDDER_PROPOSALS_NO_SURVIVING_SIGNAL: &str =
    "MEJEPA_PENDING_EMBEDDER_PROPOSALS_NO_SURVIVING_SIGNAL";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderAbsenceSignalKind {
    MincutStructuralHole,
    UnknownFingerprintCluster,
    PairwiseMiResidual,
    OodScore,
    Curiosity,
    FoundationalityVariance,
}

impl EmbedderAbsenceSignalKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MincutStructuralHole => "mincut_structural_hole",
            Self::UnknownFingerprintCluster => "unknown_fingerprint_cluster",
            Self::PairwiseMiResidual => "pairwise_mi_residual",
            Self::OodScore => "ood_score",
            Self::Curiosity => "curiosity",
            Self::FoundationalityVariance => "foundationality_variance",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderProposalConfig {
    pub max_proposals: usize,
    pub min_signal_magnitude: f32,
    pub min_composite_score: f32,
    pub pairwise_mi_max_rows: usize,
}

impl Default for EmbedderProposalConfig {
    fn default() -> Self {
        Self {
            max_proposals: DEFAULT_EMBEDDER_PROPOSAL_MAX_PROPOSALS,
            min_signal_magnitude: DEFAULT_EMBEDDER_PROPOSAL_MIN_SIGNAL_MAGNITUDE,
            min_composite_score: DEFAULT_EMBEDDER_PROPOSAL_MIN_COMPOSITE_SCORE,
            pairwise_mi_max_rows: DEFAULT_EMBEDDER_PROPOSAL_PAIRWISE_MI_MAX_ROWS,
        }
    }
}

impl EmbedderProposalConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.max_proposals == 0 || self.max_proposals > 1024 {
            return invalid(
                "max_proposals",
                format!("must be in [1, 1024], got {}", self.max_proposals),
            );
        }
        validate_unit("min_signal_magnitude", self.min_signal_magnitude)?;
        validate_unit("min_composite_score", self.min_composite_score)?;
        if self.pairwise_mi_max_rows == 0 {
            return invalid("pairwise_mi_max_rows", "must be greater than zero");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AbsenceShapeDescriptor {
    pub input_modality: String,
    pub suggested_dim: usize,
    pub suggested_objective: String,
    pub reality_channel_for_falsification: String,
    pub signal_magnitude: f32,
}

impl AbsenceShapeDescriptor {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text("absence_shape.input_modality", &self.input_modality, 128)?;
        if self.suggested_dim == 0 || self.suggested_dim > 1_000_000 {
            return invalid(
                "absence_shape.suggested_dim",
                format!("must be in [1, 1000000], got {}", self.suggested_dim),
            );
        }
        validate_text(
            "absence_shape.suggested_objective",
            &self.suggested_objective,
            512,
        )?;
        validate_text(
            "absence_shape.reality_channel_for_falsification",
            &self.reality_channel_for_falsification,
            512,
        )?;
        validate_unit("absence_shape.signal_magnitude", self.signal_magnitude)?;
        Ok(())
    }

    fn dedupe_key(&self) -> String {
        format!(
            "{}|{}|{}|{}",
            normalize_text_key(&self.input_modality),
            self.suggested_dim,
            normalize_text_key(&self.suggested_objective),
            normalize_text_key(&self.reality_channel_for_falsification)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderAbsenceSignal {
    pub kind: EmbedderAbsenceSignalKind,
    pub source_ref: String,
    pub descriptor: AbsenceShapeDescriptor,
    pub predicted_delta_cp_phi: f32,
    pub used_window_ids: Vec<String>,
}

impl EmbedderAbsenceSignal {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text("absence_signal.source_ref", &self.source_ref, 512)?;
        self.descriptor.validate()?;
        validate_unit(
            "absence_signal.predicted_delta_cp_phi",
            self.predicted_delta_cp_phi,
        )?;
        for window_id in &self.used_window_ids {
            validate_text("absence_signal.used_window_id", window_id, 256)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderProposalSourceEvidence {
    pub kind: EmbedderAbsenceSignalKind,
    pub source_ref: String,
    pub signal_magnitude: f32,
    pub predicted_delta_cp_phi: f32,
    pub used_window_ids: Vec<String>,
}

impl EmbedderProposalSourceEvidence {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        validate_text("proposal_source.source_ref", &self.source_ref, 512)?;
        validate_unit("proposal_source.signal_magnitude", self.signal_magnitude)?;
        validate_unit(
            "proposal_source.predicted_delta_cp_phi",
            self.predicted_delta_cp_phi,
        )?;
        for window_id in &self.used_window_ids {
            validate_text("proposal_source.used_window_id", window_id, 256)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingEmbedderProposal {
    pub schema_version: u32,
    pub proposal_id: [u8; 16],
    pub candidate_name: String,
    pub descriptor: AbsenceShapeDescriptor,
    pub composite_score: f32,
    pub predicted_delta_cp_phi: f32,
    pub novelty_vs_existing_proposals: f32,
    pub source_signals: Vec<EmbedderProposalSourceEvidence>,
    pub created_at_unix_ms: i64,
    pub source_of_truth_cf: String,
}

impl PendingEmbedderProposal {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_PROPOSAL_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {EMBEDDER_PROPOSAL_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        if self.proposal_id.iter().all(|byte| *byte == 0) {
            return invalid("proposal_id", "must be non-zero");
        }
        validate_text("candidate_name", &self.candidate_name, 256)?;
        self.descriptor.validate()?;
        validate_unit("composite_score", self.composite_score)?;
        validate_unit("predicted_delta_cp_phi", self.predicted_delta_cp_phi)?;
        validate_unit(
            "novelty_vs_existing_proposals",
            self.novelty_vs_existing_proposals,
        )?;
        if self.source_signals.is_empty() {
            return invalid("source_signals", "must be non-empty");
        }
        for signal in &self.source_signals {
            signal.validate()?;
        }
        if self.created_at_unix_ms <= 0 {
            return invalid("created_at_unix_ms", "must be positive");
        }
        if self.source_of_truth_cf != CF_MEJEPA_EMBEDDER_PROPOSALS {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_EMBEDDER_PROPOSALS}"),
            );
        }
        Ok(())
    }

    fn dedupe_key(&self) -> String {
        self.descriptor.dedupe_key()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderProposalWriteSummary {
    pub rows_written: usize,
    pub byte_identical_readback: bool,
    pub source_of_truth_cf: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EmbedderProposalQueueReport {
    pub schema_version: u32,
    pub proposals: Vec<PendingEmbedderProposal>,
    pub empty_reason: Option<String>,
    pub signals_seen: usize,
    pub duplicates_deduped: usize,
    pub existing_proposal_matches: usize,
    pub source_counts: BTreeMap<String, usize>,
    pub source_of_truth_cf: String,
}

impl EmbedderProposalQueueReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != EMBEDDER_PROPOSAL_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {EMBEDDER_PROPOSAL_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        for proposal in &self.proposals {
            proposal.validate()?;
        }
        if self.proposals.is_empty() && self.empty_reason.is_none() {
            return invalid("empty_reason", "empty queue must explain why it is empty");
        }
        if let Some(reason) = &self.empty_reason {
            validate_text("empty_reason", reason, 256)?;
        }
        if self.source_of_truth_cf != CF_MEJEPA_EMBEDDER_PROPOSALS {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_EMBEDDER_PROPOSALS}"),
            );
        }
        Ok(())
    }
}

pub fn propose_embedder_proposals_from_db(
    db: &DB,
    config: EmbedderProposalConfig,
    created_at_unix_ms: i64,
) -> Result<EmbedderProposalQueueReport, MejepaInferError> {
    let queue = read_active_learning_queue(db)?;
    let mincut_reports = read_all_mincut_reports(db)?;
    let pairwise_mi = read_pairwise_mi_matrix(db, None, None, config.pairwise_mi_max_rows).ok();
    let existing = read_embedder_proposals(db)?;
    let signals = collect_absence_signals_from_sources(
        &queue,
        &mincut_reports,
        pairwise_mi.as_ref(),
        created_at_unix_ms,
    )?;
    propose_embedder_proposals(&signals, &existing, config, created_at_unix_ms)
}

pub fn propose_embedder_proposals(
    signals: &[EmbedderAbsenceSignal],
    existing: &[PendingEmbedderProposal],
    config: EmbedderProposalConfig,
    created_at_unix_ms: i64,
) -> Result<EmbedderProposalQueueReport, MejepaInferError> {
    config.validate()?;
    if created_at_unix_ms <= 0 {
        return invalid("created_at_unix_ms", "must be positive");
    }

    let existing_keys = existing
        .iter()
        .map(|proposal| {
            proposal.validate()?;
            Ok(proposal.dedupe_key())
        })
        .collect::<Result<BTreeSet<_>, MejepaInferError>>()?;
    let mut source_counts = BTreeMap::<String, usize>::new();
    let mut accumulators = BTreeMap::<String, ProposalAccumulator>::new();
    let mut duplicates_deduped = 0usize;
    let mut existing_proposal_matches = 0usize;

    for signal in signals {
        signal.validate()?;
        *source_counts
            .entry(signal.kind.as_str().to_string())
            .or_default() += 1;
        if signal.descriptor.signal_magnitude < config.min_signal_magnitude {
            continue;
        }
        let key = signal.descriptor.dedupe_key();
        if existing_keys.contains(&key) {
            existing_proposal_matches += 1;
            continue;
        }
        match accumulators.get_mut(&key) {
            Some(accumulator) => {
                duplicates_deduped += 1;
                accumulator.push(signal)?;
            }
            None => {
                accumulators.insert(key, ProposalAccumulator::new(signal)?);
            }
        }
    }

    let mut proposals = accumulators
        .into_values()
        .map(|accumulator| accumulator.finish(created_at_unix_ms))
        .collect::<Result<Vec<_>, _>>()?;
    proposals.retain(|proposal| proposal.composite_score >= config.min_composite_score);
    proposals.sort_by(|left, right| {
        right
            .composite_score
            .total_cmp(&left.composite_score)
            .then_with(|| left.candidate_name.cmp(&right.candidate_name))
    });
    proposals.truncate(config.max_proposals);
    let empty_reason = if proposals.is_empty() {
        if signals.is_empty() {
            Some(MEJEPA_PENDING_EMBEDDER_PROPOSALS_EMPTY_SUBSTRATE.to_string())
        } else {
            Some(MEJEPA_PENDING_EMBEDDER_PROPOSALS_NO_SURVIVING_SIGNAL.to_string())
        }
    } else {
        None
    };
    let report = EmbedderProposalQueueReport {
        schema_version: EMBEDDER_PROPOSAL_SCHEMA_VERSION,
        proposals,
        empty_reason,
        signals_seen: signals.len(),
        duplicates_deduped,
        existing_proposal_matches,
        source_counts,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
    };
    report.validate()?;
    Ok(report)
}

pub fn write_embedder_proposals_sync_readback(
    db: &DB,
    proposals: &[PendingEmbedderProposal],
) -> Result<EmbedderProposalWriteSummary, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_EMBEDDER_PROPOSALS)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let mut encoded = Vec::with_capacity(proposals.len());
    for proposal in proposals {
        proposal.validate()?;
        let key = embedder_proposal_key(proposal.proposal_id);
        let value = bincode::serialize(proposal)?;
        db.put_cf_opt(cf, &key, &value, &opts)?;
        encoded.push((key, value));
    }
    db.flush_cf(cf)?;
    for (key, value) in &encoded {
        let readback = db
            .get_cf(cf, key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "embedder_proposal.readback".to_string(),
                detail: "read-after-write could not find persisted proposal".to_string(),
            })?;
        if &readback != value {
            return invalid(
                "embedder_proposal.readback",
                "read-after-write bytes differ from encoded proposal",
            );
        }
    }
    Ok(EmbedderProposalWriteSummary {
        rows_written: proposals.len(),
        byte_identical_readback: true,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
    })
}

pub fn read_embedder_proposal(
    db: &DB,
    proposal_id: [u8; 16],
) -> Result<Option<PendingEmbedderProposal>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_EMBEDDER_PROPOSALS)?;
    let Some(bytes) = db.get_cf(cf, embedder_proposal_key(proposal_id))? else {
        return Ok(None);
    };
    let proposal: PendingEmbedderProposal = bincode::deserialize(&bytes)?;
    proposal.validate()?;
    Ok(Some(proposal))
}

pub fn read_embedder_proposals(db: &DB) -> Result<Vec<PendingEmbedderProposal>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_EMBEDDER_PROPOSALS)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let row: PendingEmbedderProposal = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push(row);
    }
    rows.sort_by(|left, right| {
        right
            .composite_score
            .total_cmp(&left.composite_score)
            .then_with(|| left.candidate_name.cmp(&right.candidate_name))
    });
    Ok(rows)
}

pub fn embedder_proposal_key(proposal_id: [u8; 16]) -> Vec<u8> {
    proposal_id.to_vec()
}

pub fn embedder_proposal_id(descriptor: &AbsenceShapeDescriptor) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_EMBEDDER_PROPOSAL_V1");
    hasher.update(descriptor.dedupe_key().as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

pub fn collect_absence_signals_from_sources(
    queue: &ActiveLearningQueueState,
    mincut_reports: &[MincutReport],
    pairwise_mi: Option<&PairwiseMiPersistedMatrix>,
    created_at_unix_ms: i64,
) -> Result<Vec<EmbedderAbsenceSignal>, MejepaInferError> {
    if created_at_unix_ms <= 0 {
        return invalid("created_at_unix_ms", "must be positive");
    }
    let mut signals = Vec::new();
    for entry in queue.entries.values() {
        signals.extend(signals_from_active_learning_entry(entry)?);
    }
    for report in mincut_reports {
        report.validate()?;
        if let Some(direction) = report.recommended_addition_directions.first() {
            signals.push(signal_from_mincut(report, direction)?);
        }
    }
    if let Some(matrix) = pairwise_mi {
        signals.push(signal_from_pairwise_mi(matrix)?);
    }
    Ok(signals)
}

fn read_active_learning_queue(db: &DB) -> Result<ActiveLearningQueueState, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_ACTIVE_LEARNING_QUEUE)?;
    let Some(bytes) = db.get_cf(cf, b"active")? else {
        return ActiveLearningQueueState::new(1).map_err(eval_error);
    };
    let queue: ActiveLearningQueueState = bincode::deserialize(&bytes)?;
    Ok(queue)
}

fn read_all_mincut_reports(db: &DB) -> Result<Vec<MincutReport>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_MINCUT_REPORTS)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let report: MincutReport = bincode::deserialize(&value)?;
        report.validate()?;
        rows.push(report);
    }
    rows.sort_by_key(|report| report.created_at_unix_ms);
    Ok(rows)
}

fn signals_from_active_learning_entry(
    entry: &ActiveLearningQueueEntry,
) -> Result<Vec<EmbedderAbsenceSignal>, MejepaInferError> {
    validate_active_learning_entry(entry)?;
    let mut signals = Vec::new();
    if entry.ood_score > 0.0 {
        signals.push(EmbedderAbsenceSignal {
            kind: EmbedderAbsenceSignalKind::OodScore,
            source_ref: format!("active_learning:{}", entry.task_id.0),
            descriptor: AbsenceShapeDescriptor {
                input_modality: "prediction_ood_surface".to_string(),
                suggested_dim: default_dim_for_entry(entry),
                suggested_objective: "reduce_out_of_distribution_prediction_mass".to_string(),
                reality_channel_for_falsification: "held_out_ood_recall_and_per_cell_correlation"
                    .to_string(),
                signal_magnitude: entry.ood_score,
            },
            predicted_delta_cp_phi: entry
                .curiosity_score
                .max(entry.ood_score * 0.5)
                .clamp(0.0, 1.0),
            used_window_ids: vec![format!("task:{}", entry.task_id.0)],
        });
    }
    if entry.curiosity_score > 0.0 {
        signals.push(EmbedderAbsenceSignal {
            kind: EmbedderAbsenceSignalKind::Curiosity,
            source_ref: format!("curiosity:{}", entry.task_id.0),
            descriptor: AbsenceShapeDescriptor {
                input_modality: "compression_progress_gap".to_string(),
                suggested_dim: default_dim_for_entry(entry),
                suggested_objective: "maximize_expected_compression_progress".to_string(),
                reality_channel_for_falsification:
                    "held_out_predicted_vs_actual_cp_phi_calibration".to_string(),
                signal_magnitude: entry.curiosity_score,
            },
            predicted_delta_cp_phi: entry.curiosity_score,
            used_window_ids: vec![format!("task:{}", entry.task_id.0)],
        });
    }
    match &entry.kind {
        ActiveLearningKind::UnknownFingerprint { candidate } => {
            candidate.validate().map_err(eval_error)?;
            let candidate_dim = candidate
                .observation_by_embedder
                .values()
                .map(Vec::len)
                .sum::<usize>()
                .max(1);
            let signal_magnitude = ((candidate.ood_score
                + candidate.embedder_disagreement_score.clamp(0.0, 1.0))
                / 2.0)
                .clamp(0.0, 1.0);
            signals.push(EmbedderAbsenceSignal {
                kind: EmbedderAbsenceSignalKind::UnknownFingerprintCluster,
                source_ref: format!(
                    "unknown_fingerprint:{}",
                    hex::encode(candidate.candidate_id)
                ),
                descriptor: AbsenceShapeDescriptor {
                    input_modality: "unknown_fingerprint_cluster".to_string(),
                    suggested_dim: candidate_dim,
                    suggested_objective: "contrast_unknown_cluster_against_known_fingerprints"
                        .to_string(),
                    reality_channel_for_falsification: "held_out_unknown_fingerprint_ood_recall"
                        .to_string(),
                    signal_magnitude,
                },
                predicted_delta_cp_phi: entry.curiosity_score.max(signal_magnitude * 0.5),
                used_window_ids: vec![format!("session:{}", hex::encode(candidate.session_id))],
            });
        }
        ActiveLearningKind::ColdCellTargetedCorpus {
            cell_id,
            abstain_count,
        } => {
            let signal_magnitude = (*abstain_count as f32 / 100.0).clamp(0.0, 1.0);
            signals.push(EmbedderAbsenceSignal {
                kind: EmbedderAbsenceSignalKind::FoundationalityVariance,
                source_ref: format!("cold_cell:{cell_id}"),
                descriptor: AbsenceShapeDescriptor {
                    input_modality: "chunk_foundationality_variance".to_string(),
                    suggested_dim: 64,
                    suggested_objective: "separate_foundational_from_surface_level_chunk_structure"
                        .to_string(),
                    reality_channel_for_falsification:
                        "held_out_foundationality_weighted_cell_correlation".to_string(),
                    signal_magnitude,
                },
                predicted_delta_cp_phi: entry.curiosity_score.max(signal_magnitude * 0.5),
                used_window_ids: vec![format!("cell:{cell_id}")],
            });
        }
        ActiveLearningKind::NovelCluster { candidate } => {
            candidate.validate().map_err(eval_error)?;
            let candidate_dim = candidate
                .observation_by_embedder
                .values()
                .map(Vec::len)
                .sum::<usize>()
                .max(1);
            let signal_magnitude = candidate.novelty_score.clamp(0.0, 1.0);
            signals.push(EmbedderAbsenceSignal {
                kind: EmbedderAbsenceSignalKind::UnknownFingerprintCluster,
                source_ref: format!("novel_cluster:{}", hex::encode(candidate.candidate_id)),
                descriptor: AbsenceShapeDescriptor {
                    input_modality: "novel_constellation_cluster".to_string(),
                    suggested_dim: candidate_dim,
                    suggested_objective: "separate_novel_constellation_cluster".to_string(),
                    reality_channel_for_falsification: "held_out_ontology_growth_audit".to_string(),
                    signal_magnitude,
                },
                predicted_delta_cp_phi: entry.curiosity_score.max(signal_magnitude * 0.5),
                used_window_ids: vec![format!("task:{}", entry.task_id.0)],
            });
        }
        ActiveLearningKind::Uncertainty
        | ActiveLearningKind::OutOfDistribution
        | ActiveLearningKind::OodHarvest { .. }
        | ActiveLearningKind::ConstellationDisagreement { .. }
        | ActiveLearningKind::EwcProtectionViolation { .. }
        | ActiveLearningKind::AgentSurprise { .. } => {}
    }
    Ok(signals)
}

fn signal_from_mincut(
    report: &MincutReport,
    direction: &[f32],
) -> Result<EmbedderAbsenceSignal, MejepaInferError> {
    report.validate()?;
    if direction.is_empty() {
        return invalid("mincut.direction", "must be non-empty");
    }
    let signal_magnitude = structural_gap_score(report);
    Ok(EmbedderAbsenceSignal {
        kind: EmbedderAbsenceSignalKind::MincutStructuralHole,
        source_ref: format!("mincut:{}", report.report_id),
        descriptor: AbsenceShapeDescriptor {
            input_modality: format!("panel_graph:{}", report.graph_source_kind),
            suggested_dim: direction.len(),
            suggested_objective: "targeted_blind_spot_contrastive".to_string(),
            reality_channel_for_falsification: "held_out_mincut_window_per_cell_correlation"
                .to_string(),
            signal_magnitude,
        },
        predicted_delta_cp_phi: signal_magnitude,
        used_window_ids: vec![format!("mincut_window:{}", report.created_at_unix_ms)],
    })
}

fn signal_from_pairwise_mi(
    matrix: &PairwiseMiPersistedMatrix,
) -> Result<EmbedderAbsenceSignal, MejepaInferError> {
    matrix.validate()?;
    let redundancy_gap = matrix
        .health
        .max_off_diagonal
        .max(matrix.health.mean_off_diagonal);
    let signal_magnitude = redundancy_gap.clamp(0.0, 1.0);
    Ok(EmbedderAbsenceSignal {
        kind: EmbedderAbsenceSignalKind::PairwiseMiResidual,
        source_ref: format!(
            "pairwise_mi:{}:{}",
            matrix.corpus_shard_hash, matrix.created_at_unix_ms
        ),
        descriptor: AbsenceShapeDescriptor {
            input_modality: "embedder_pairwise_mi_residual".to_string(),
            suggested_dim: matrix.slots.len().max(1),
            suggested_objective: "reduce_redundant_panel_signal_and_expose_missing_axis"
                .to_string(),
            reality_channel_for_falsification:
                "held_out_pairwise_mi_residual_and_global_correlation".to_string(),
            signal_magnitude,
        },
        predicted_delta_cp_phi: (1.0 - matrix.health.mean_off_diagonal).clamp(0.0, 1.0),
        used_window_ids: vec![format!("pairwise_mi_step:{}", matrix.step)],
    })
}

struct ProposalAccumulator {
    descriptor: AbsenceShapeDescriptor,
    source_signals: Vec<EmbedderProposalSourceEvidence>,
}

impl ProposalAccumulator {
    fn new(signal: &EmbedderAbsenceSignal) -> Result<Self, MejepaInferError> {
        let mut out = Self {
            descriptor: signal.descriptor.clone(),
            source_signals: Vec::new(),
        };
        out.push(signal)?;
        Ok(out)
    }

    fn push(&mut self, signal: &EmbedderAbsenceSignal) -> Result<(), MejepaInferError> {
        signal.validate()?;
        if signal.descriptor.dedupe_key() != self.descriptor.dedupe_key() {
            return invalid(
                "absence_signal.descriptor",
                "cannot merge signals with different absence-shape descriptors",
            );
        }
        self.descriptor.signal_magnitude = self
            .descriptor
            .signal_magnitude
            .max(signal.descriptor.signal_magnitude);
        self.source_signals.push(EmbedderProposalSourceEvidence {
            kind: signal.kind,
            source_ref: signal.source_ref.clone(),
            signal_magnitude: signal.descriptor.signal_magnitude,
            predicted_delta_cp_phi: signal.predicted_delta_cp_phi,
            used_window_ids: signal.used_window_ids.clone(),
        });
        self.source_signals.sort_by(|left, right| {
            left.kind
                .cmp(&right.kind)
                .then_with(|| left.source_ref.cmp(&right.source_ref))
        });
        Ok(())
    }

    fn finish(self, created_at_unix_ms: i64) -> Result<PendingEmbedderProposal, MejepaInferError> {
        let predicted_delta_cp_phi = self
            .source_signals
            .iter()
            .map(|signal| signal.predicted_delta_cp_phi)
            .fold(0.0f32, f32::max);
        let novelty_vs_existing_proposals = 1.0f32;
        let composite_score = (self.descriptor.signal_magnitude
            * predicted_delta_cp_phi
            * novelty_vs_existing_proposals)
            .clamp(0.0, 1.0);
        let proposal_id = embedder_proposal_id(&self.descriptor);
        let candidate_name = format!(
            "embedder_absence_{}_{}",
            self.source_signals
                .first()
                .map(|signal| signal.kind.as_str())
                .unwrap_or("unknown"),
            hex::encode(&proposal_id[..4])
        );
        self.descriptor.validate()?;
        let proposal = PendingEmbedderProposal {
            schema_version: EMBEDDER_PROPOSAL_SCHEMA_VERSION,
            proposal_id,
            candidate_name,
            descriptor: self.descriptor,
            composite_score,
            predicted_delta_cp_phi,
            novelty_vs_existing_proposals,
            source_signals: self.source_signals,
            created_at_unix_ms,
            source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
        };
        proposal.validate()?;
        Ok(proposal)
    }
}

fn validate_active_learning_entry(
    entry: &ActiveLearningQueueEntry,
) -> Result<(), MejepaInferError> {
    entry.validate().map_err(eval_error)?;
    Ok(())
}

fn default_dim_for_entry(entry: &ActiveLearningQueueEntry) -> usize {
    match &entry.kind {
        ActiveLearningKind::UnknownFingerprint { candidate } => candidate
            .observation_by_embedder
            .values()
            .map(Vec::len)
            .sum::<usize>()
            .max(1),
        ActiveLearningKind::NovelCluster { candidate } => candidate
            .observation_by_embedder
            .values()
            .map(Vec::len)
            .sum::<usize>()
            .max(1),
        _ => 64,
    }
}

fn structural_gap_score(report: &MincutReport) -> f32 {
    (1.0 / (1.0 + report.cut_value + report.conductance)).clamp(0.0, 1.0)
}

fn normalize_text_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

fn validate_unit(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(field, format!("must be finite and in [0,1], got {value}"));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
    if value.trim().is_empty() {
        return invalid(field, "must be non-empty");
    }
    if value.len() > max_len {
        return invalid(field, format!("exceeds {max_len} bytes"));
    }
    if value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return invalid(field, "contains a control character");
    }
    Ok(())
}

fn eval_error(err: crate::eval::EvalError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "active_learning_queue".to_string(),
        detail: err.to_string(),
    }
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

    #[test]
    fn ranks_and_dedupes_synthetic_absence_shapes() {
        let ts = 1_779_080_000_000;
        let duplicate = synthetic_signal(
            EmbedderAbsenceSignalKind::MincutStructuralHole,
            "mincut:a",
            "panel_gap",
            4,
            0.8,
            0.7,
        );
        let report = propose_embedder_proposals(
            &[
                duplicate.clone(),
                synthetic_signal(
                    EmbedderAbsenceSignalKind::Curiosity,
                    "curiosity:b",
                    "curiosity_gap",
                    8,
                    0.6,
                    0.5,
                ),
                EmbedderAbsenceSignal {
                    source_ref: "mincut:a-duplicate".to_string(),
                    ..duplicate
                },
            ],
            &[],
            EmbedderProposalConfig::default(),
            ts,
        )
        .unwrap();
        assert_eq!(report.proposals.len(), 2);
        assert_eq!(report.duplicates_deduped, 1);
        assert!(report.proposals[0].composite_score > report.proposals[1].composite_score);
        assert_eq!(report.proposals[0].source_signals.len(), 2);
    }

    #[test]
    fn empty_substrate_is_explicitly_fail_closed() {
        let report = propose_embedder_proposals(
            &[],
            &[],
            EmbedderProposalConfig::default(),
            1_779_080_000_000,
        )
        .unwrap();
        assert!(report.proposals.is_empty());
        assert_eq!(
            report.empty_reason.as_deref(),
            Some(MEJEPA_PENDING_EMBEDDER_PROPOSALS_EMPTY_SUBSTRATE)
        );
    }

    fn synthetic_signal(
        kind: EmbedderAbsenceSignalKind,
        source_ref: &str,
        input_modality: &str,
        suggested_dim: usize,
        signal_magnitude: f32,
        predicted_delta_cp_phi: f32,
    ) -> EmbedderAbsenceSignal {
        EmbedderAbsenceSignal {
            kind,
            source_ref: source_ref.to_string(),
            descriptor: AbsenceShapeDescriptor {
                input_modality: input_modality.to_string(),
                suggested_dim,
                suggested_objective: "synthetic_objective".to_string(),
                reality_channel_for_falsification: "synthetic_holdout".to_string(),
                signal_magnitude,
            },
            predicted_delta_cp_phi,
            used_window_ids: vec!["synthetic-window".to_string()],
        }
    }
}
