use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::{
    CF_MEJEPA_ACTIVE_LEARNING_QUEUE, CF_MEJEPA_INSTRUMENT_PROPOSALS, CF_MEJEPA_MINCUT_REPORTS,
};
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::calibration::cf;
use crate::error::MejepaInferError;
use crate::eval::{
    ActiveLearningKind, ActiveLearningQueueEntry, ActiveLearningQueueState,
    UnknownFingerprintCandidate,
};
use crate::mincut_panel::{MincutPartitionReport, MincutReport};
use crate::pairwise_mi::{read_pairwise_mi_matrix, PairwiseMiPersistedMatrix};

pub const INSTRUMENT_PROPOSAL_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_INSTRUMENT_PROPOSAL_MIN_CLUSTER_SIZE: usize = 30;
pub const DEFAULT_INSTRUMENT_PROPOSAL_TAU_INTRA: f32 = 0.85;
pub const DEFAULT_INSTRUMENT_PROPOSAL_TAU_FAR: f32 = 0.4;
pub const DEFAULT_INSTRUMENT_PROPOSAL_MIN_DELTA: f32 = 0.01;
pub const DEFAULT_INSTRUMENT_PROPOSAL_MAX_PROPOSALS: usize = 16;
pub const MEJEPA_PROPOSE_INSTRUMENT_EMPTY_QUEUE: &str = "MEJEPA_PROPOSE_INSTRUMENT_EMPTY_QUEUE";
pub const MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER: &str =
    "MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentProposalModality {
    UnknownCluster,
    StructuralHole,
    HybridResidual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentProposalStatus {
    Candidate,
    UnderReview,
    Accepted,
    RejectedByFalsification,
    RejectedByOperator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentProposalDecision {
    Accept,
    Reject,
    MarkUnderReview,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposalConfig {
    pub min_cluster_size: usize,
    pub tau_intra: f32,
    pub tau_far: f32,
    pub min_expected_holdout_improvement: f32,
    pub max_proposals: usize,
    pub pairwise_mi_max_rows: usize,
}

impl Default for InstrumentProposalConfig {
    fn default() -> Self {
        Self {
            min_cluster_size: DEFAULT_INSTRUMENT_PROPOSAL_MIN_CLUSTER_SIZE,
            tau_intra: DEFAULT_INSTRUMENT_PROPOSAL_TAU_INTRA,
            tau_far: DEFAULT_INSTRUMENT_PROPOSAL_TAU_FAR,
            min_expected_holdout_improvement: DEFAULT_INSTRUMENT_PROPOSAL_MIN_DELTA,
            max_proposals: DEFAULT_INSTRUMENT_PROPOSAL_MAX_PROPOSALS,
            pairwise_mi_max_rows: 1_000_000,
        }
    }
}

impl InstrumentProposalConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.min_cluster_size == 0 {
            return invalid("min_cluster_size", "must be greater than zero");
        }
        validate_unit("tau_intra", self.tau_intra)?;
        validate_unit("tau_far", self.tau_far)?;
        validate_unit(
            "min_expected_holdout_improvement",
            self.min_expected_holdout_improvement,
        )?;
        if self.max_proposals == 0 || self.max_proposals > 1024 {
            return invalid(
                "max_proposals",
                format!("must be in [1, 1024], got {}", self.max_proposals),
            );
        }
        if self.pairwise_mi_max_rows == 0 {
            return invalid("pairwise_mi_max_rows", "must be greater than zero");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposalMiSignature {
    pub corpus_shard_hash: String,
    pub created_at_unix_ms: i64,
    pub effective_signal_count: f32,
    pub mean_off_diagonal: f32,
    pub max_redundancy_pair: Option<String>,
    pub max_redundancy: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposalClusterRef {
    pub cluster_id: [u8; 16],
    pub member_candidate_ids: Vec<[u8; 16]>,
    pub member_task_ids: Vec<String>,
    pub mean_ood_score: f32,
    pub mean_embedder_disagreement_score: f32,
    pub mean_intra_cluster_cosine: f32,
    pub distance_to_nearest_known: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposalDecisionRecord {
    pub decision: InstrumentProposalDecision,
    pub decided_at_unix_ms: i64,
    pub min_delta_required: f32,
    pub observed_holdout_delta: f32,
    pub accepted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposal {
    pub schema_version: u32,
    pub proposal_id: [u8; 16],
    pub candidate_name: String,
    pub source_modality: InstrumentProposalModality,
    pub expected_dim: usize,
    pub residual_direction: Vec<f32>,
    pub structural_hole_partition: Option<MincutPartitionReport>,
    pub justifying_unknown_cluster: Option<InstrumentProposalClusterRef>,
    pub justifying_pairwise_mi_signature: Option<InstrumentProposalMiSignature>,
    pub expected_holdout_improvement: f32,
    pub confidence: f32,
    pub status: InstrumentProposalStatus,
    pub decision: Option<InstrumentProposalDecisionRecord>,
    pub created_at_unix_ms: i64,
    pub source_of_truth_cf: String,
}

impl InstrumentProposal {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != INSTRUMENT_PROPOSAL_SCHEMA_VERSION {
            return invalid(
                "schema_version",
                format!(
                    "expected {INSTRUMENT_PROPOSAL_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        if self.proposal_id.iter().all(|byte| *byte == 0) {
            return invalid("proposal_id", "must be non-zero");
        }
        validate_text("candidate_name", &self.candidate_name, 256)?;
        if self.expected_dim == 0 {
            return invalid("expected_dim", "must be greater than zero");
        }
        if self.residual_direction.len() != self.expected_dim {
            return Err(MejepaInferError::DimMismatch {
                expected: self.expected_dim,
                actual: self.residual_direction.len(),
                context: "instrument proposal residual_direction".to_string(),
            });
        }
        validate_unit_vector_or_nonzero("residual_direction", &self.residual_direction)?;
        validate_unit(
            "expected_holdout_improvement",
            self.expected_holdout_improvement,
        )?;
        validate_unit("confidence", self.confidence)?;
        if self.created_at_unix_ms <= 0 {
            return invalid("created_at_unix_ms", "must be positive");
        }
        if self.source_of_truth_cf != CF_MEJEPA_INSTRUMENT_PROPOSALS {
            return invalid(
                "source_of_truth_cf",
                format!("expected {CF_MEJEPA_INSTRUMENT_PROPOSALS}"),
            );
        }
        if let Some(cluster) = &self.justifying_unknown_cluster {
            validate_unit("cluster.mean_ood_score", cluster.mean_ood_score)?;
            validate_unit_or_more(
                "cluster.mean_embedder_disagreement_score",
                cluster.mean_embedder_disagreement_score,
            )?;
            validate_unit(
                "cluster.mean_intra_cluster_cosine",
                cluster.mean_intra_cluster_cosine,
            )?;
            validate_unit(
                "cluster.distance_to_nearest_known",
                cluster.distance_to_nearest_known,
            )?;
            if cluster.member_candidate_ids.is_empty() || cluster.member_task_ids.is_empty() {
                return invalid("cluster", "must have at least one member");
            }
            if cluster.member_candidate_ids.len() != cluster.member_task_ids.len() {
                return invalid("cluster", "member id/task count mismatch");
            }
        }
        if let Some(sig) = &self.justifying_pairwise_mi_signature {
            validate_text("mi.corpus_shard_hash", &sig.corpus_shard_hash, 64)?;
            if sig.created_at_unix_ms <= 0 {
                return invalid("mi.created_at_unix_ms", "must be positive");
            }
            validate_unit_or_more("mi.effective_signal_count", sig.effective_signal_count)?;
            validate_unit("mi.mean_off_diagonal", sig.mean_off_diagonal)?;
            validate_unit("mi.max_redundancy", sig.max_redundancy)?;
        }
        if let Some(decision) = &self.decision {
            if decision.decided_at_unix_ms <= 0 {
                return invalid("decision.decided_at_unix_ms", "must be positive");
            }
            validate_unit("decision.min_delta_required", decision.min_delta_required)?;
            validate_delta(
                "decision.observed_holdout_delta",
                decision.observed_holdout_delta,
            )?;
            validate_text("decision.reason", &decision.reason, 1024)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposalReport {
    pub proposals: Vec<InstrumentProposal>,
    pub empty_reason: Option<String>,
    pub active_learning_entries: usize,
    pub unknown_candidates_seen: usize,
    pub clusters_considered: usize,
    pub mincut_reports_seen: usize,
    pub pairwise_mi_available: bool,
}

impl InstrumentProposalReport {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        for proposal in &self.proposals {
            proposal.validate()?;
        }
        if let Some(reason) = &self.empty_reason {
            validate_text("empty_reason", reason, 256)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstrumentProposalWriteSummary {
    pub rows_written: usize,
    pub byte_identical_readback: bool,
    pub source_of_truth_cf: String,
}

pub fn propose_instruments_from_db(
    db: &DB,
    config: InstrumentProposalConfig,
    created_at_unix_ms: i64,
) -> Result<InstrumentProposalReport, MejepaInferError> {
    let queue = read_active_learning_queue(db)?;
    let mincut_reports = read_all_mincut_reports(db)?;
    let pairwise_mi = read_pairwise_mi_matrix(db, None, None, config.pairwise_mi_max_rows).ok();
    propose_instruments(
        &queue,
        &mincut_reports,
        pairwise_mi.as_ref(),
        config,
        created_at_unix_ms,
    )
}

pub fn propose_instruments(
    queue: &ActiveLearningQueueState,
    mincut_reports: &[MincutReport],
    pairwise_mi: Option<&PairwiseMiPersistedMatrix>,
    config: InstrumentProposalConfig,
    created_at_unix_ms: i64,
) -> Result<InstrumentProposalReport, MejepaInferError> {
    config.validate()?;
    if created_at_unix_ms <= 0 {
        return invalid("created_at_unix_ms", "must be positive");
    }
    let unknowns = active_unknown_candidates(queue)?;
    let mi_signature = pairwise_mi.map(mi_signature).transpose()?;
    let mut proposals = Vec::new();
    let mut clusters_considered = 0usize;
    if !unknowns.is_empty() {
        let clusters = cluster_unknown_candidates(&unknowns, config.tau_intra)?;
        clusters_considered = clusters.len();
        for cluster in clusters {
            if cluster.members.len() < config.min_cluster_size {
                continue;
            }
            if cluster.distance_to_nearest_known < config.tau_far {
                continue;
            }
            let proposal = proposal_from_cluster(
                &cluster,
                latest_mincut_report(mincut_reports),
                mi_signature.clone(),
                created_at_unix_ms,
            )?;
            if proposal.expected_holdout_improvement >= config.min_expected_holdout_improvement {
                proposals.push(proposal);
            }
        }
    }
    if proposals.is_empty() {
        if let Some(report) = latest_mincut_report(mincut_reports) {
            if let Some(direction) = report.recommended_addition_directions.first() {
                let proposal = proposal_from_mincut(
                    report,
                    direction,
                    mi_signature.clone(),
                    created_at_unix_ms,
                )?;
                if proposal.expected_holdout_improvement >= config.min_expected_holdout_improvement
                {
                    proposals.push(proposal);
                }
            }
        }
    }
    proposals.sort_by(|left, right| {
        proposal_score(right)
            .total_cmp(&proposal_score(left))
            .then_with(|| left.candidate_name.cmp(&right.candidate_name))
    });
    proposals.truncate(config.max_proposals);
    let empty_reason = if proposals.is_empty() {
        if queue.entries.is_empty() {
            Some(MEJEPA_PROPOSE_INSTRUMENT_EMPTY_QUEUE.to_string())
        } else {
            Some(MEJEPA_PROPOSE_INSTRUMENT_NO_SURVIVING_CLUSTER.to_string())
        }
    } else {
        None
    };
    let report = InstrumentProposalReport {
        proposals,
        empty_reason,
        active_learning_entries: queue.entries.len(),
        unknown_candidates_seen: unknowns.len(),
        clusters_considered,
        mincut_reports_seen: mincut_reports.len(),
        pairwise_mi_available: pairwise_mi.is_some(),
    };
    report.validate()?;
    Ok(report)
}

pub fn write_instrument_proposals_sync_readback(
    db: &DB,
    proposals: &[InstrumentProposal],
) -> Result<InstrumentProposalWriteSummary, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_INSTRUMENT_PROPOSALS)?;
    let mut opts = WriteOptions::default();
    opts.set_sync(true);
    let mut encoded = Vec::with_capacity(proposals.len());
    for proposal in proposals {
        proposal.validate()?;
        let key = instrument_proposal_key(proposal.proposal_id);
        let value = bincode::serialize(proposal)?;
        db.put_cf_opt(cf, &key, &value, &opts)?;
        encoded.push((key, value));
    }
    db.flush_cf(cf)?;
    for (key, value) in &encoded {
        let readback = db
            .get_cf(cf, key)?
            .ok_or_else(|| MejepaInferError::InvalidInput {
                field: "instrument_proposal.readback".to_string(),
                detail: "read-after-write could not find persisted proposal".to_string(),
            })?;
        if &readback != value {
            return invalid(
                "instrument_proposal.readback",
                "read-after-write bytes differ from encoded proposal",
            );
        }
    }
    Ok(InstrumentProposalWriteSummary {
        rows_written: proposals.len(),
        byte_identical_readback: true,
        source_of_truth_cf: CF_MEJEPA_INSTRUMENT_PROPOSALS.to_string(),
    })
}

pub fn read_instrument_proposal(
    db: &DB,
    proposal_id: [u8; 16],
) -> Result<Option<InstrumentProposal>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_INSTRUMENT_PROPOSALS)?;
    let Some(bytes) = db.get_cf(cf, instrument_proposal_key(proposal_id))? else {
        return Ok(None);
    };
    let proposal: InstrumentProposal = bincode::deserialize(&bytes)?;
    proposal.validate()?;
    Ok(Some(proposal))
}

pub fn read_instrument_proposals(db: &DB) -> Result<Vec<InstrumentProposal>, MejepaInferError> {
    let cf = cf(db, CF_MEJEPA_INSTRUMENT_PROPOSALS)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let row: InstrumentProposal = bincode::deserialize(&value)?;
        row.validate()?;
        rows.push(row);
    }
    rows.sort_by(|left, right| left.candidate_name.cmp(&right.candidate_name));
    Ok(rows)
}

pub fn promote_instrument_proposal(
    db: &DB,
    proposal_id: [u8; 16],
    decision: InstrumentProposalDecision,
    observed_holdout_delta: f32,
    min_delta_required: f32,
    decided_at_unix_ms: i64,
) -> Result<InstrumentProposal, MejepaInferError> {
    validate_delta("observed_holdout_delta", observed_holdout_delta)?;
    validate_unit("min_delta_required", min_delta_required)?;
    if decided_at_unix_ms <= 0 {
        return invalid("decided_at_unix_ms", "must be positive");
    }
    let mut proposal = read_instrument_proposal(db, proposal_id)?.ok_or_else(|| {
        MejepaInferError::InstrumentProposalNotFound {
            proposal_id: hex::encode(proposal_id),
        }
    })?;
    if proposal.status == InstrumentProposalStatus::UnderReview
        && decision != InstrumentProposalDecision::Accept
        && decision != InstrumentProposalDecision::Reject
    {
        return Err(MejepaInferError::InstrumentProposalUnderReview {
            proposal_id: hex::encode(proposal_id),
        });
    }
    let (status, accepted, reason) = match decision {
        InstrumentProposalDecision::MarkUnderReview => {
            if proposal.status == InstrumentProposalStatus::UnderReview {
                return Err(MejepaInferError::InstrumentProposalUnderReview {
                    proposal_id: hex::encode(proposal_id),
                });
            }
            (
                InstrumentProposalStatus::UnderReview,
                false,
                "proposal marked under_review for operator validation".to_string(),
            )
        }
        InstrumentProposalDecision::Accept => {
            if observed_holdout_delta >= min_delta_required {
                (
                    InstrumentProposalStatus::Accepted,
                    true,
                    "accepted: held-out delta cleared promotion threshold".to_string(),
                )
            } else {
                (
                    InstrumentProposalStatus::RejectedByFalsification,
                    false,
                    "rejected_by_falsification: held-out delta below threshold".to_string(),
                )
            }
        }
        InstrumentProposalDecision::Reject => (
            InstrumentProposalStatus::RejectedByOperator,
            false,
            "rejected_by_operator".to_string(),
        ),
    };
    proposal.status = status;
    proposal.decision = Some(InstrumentProposalDecisionRecord {
        decision,
        decided_at_unix_ms,
        min_delta_required,
        observed_holdout_delta,
        accepted,
        reason,
    });
    write_instrument_proposals_sync_readback(db, &[proposal.clone()])?;
    Ok(proposal)
}

pub fn instrument_proposal_key(proposal_id: [u8; 16]) -> Vec<u8> {
    proposal_id.to_vec()
}

pub fn instrument_proposal_id(candidate_name: &str, residual_direction: &[f32]) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_INSTRUMENT_PROPOSAL_V1");
    hasher.update(candidate_name.as_bytes());
    for value in residual_direction {
        hasher.update(value.to_bits().to_be_bytes());
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
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

fn active_unknown_candidates(
    queue: &ActiveLearningQueueState,
) -> Result<Vec<&UnknownFingerprintCandidate>, MejepaInferError> {
    let mut out = Vec::new();
    for entry in queue.entries.values() {
        validate_entry(entry)?;
        if let ActiveLearningKind::UnknownFingerprint { candidate } = &entry.kind {
            candidate.validate().map_err(eval_error)?;
            out.push(candidate.as_ref());
        }
    }
    Ok(out)
}

struct CandidateCluster<'a> {
    members: Vec<&'a UnknownFingerprintCandidate>,
    centroid: Vec<f32>,
    mean_ood_score: f32,
    mean_disagreement: f32,
    mean_intra_cosine: f32,
    distance_to_nearest_known: f32,
}

fn cluster_unknown_candidates<'a>(
    candidates: &[&'a UnknownFingerprintCandidate],
    tau_intra: f32,
) -> Result<Vec<CandidateCluster<'a>>, MejepaInferError> {
    let mut assigned = BTreeSet::new();
    let mut clusters = Vec::new();
    for seed in candidates {
        if assigned.contains(&seed.candidate_id) {
            continue;
        }
        let seed_vec = flatten_observation(&seed.observation_by_embedder)?;
        let mut members = vec![*seed];
        for candidate in candidates {
            if candidate.candidate_id == seed.candidate_id
                || assigned.contains(&candidate.candidate_id)
            {
                continue;
            }
            let candidate_vec = flatten_observation(&candidate.observation_by_embedder)?;
            if cosine(&seed_vec, &candidate_vec)? >= tau_intra {
                members.push(*candidate);
            }
        }
        for member in &members {
            assigned.insert(member.candidate_id);
        }
        let vectors = members
            .iter()
            .map(|member| flatten_observation(&member.observation_by_embedder))
            .collect::<Result<Vec<_>, _>>()?;
        let centroid = normalize(&mean_vector(&vectors)?)?;
        let mean_intra_cosine = mean_pairwise_cosine(&vectors)?;
        let mean_ood_score =
            members.iter().map(|member| member.ood_score).sum::<f32>() / members.len() as f32;
        let mean_disagreement = members
            .iter()
            .map(|member| member.embedder_disagreement_score)
            .sum::<f32>()
            / members.len() as f32;
        let distance_to_nearest_known = members
            .iter()
            .map(|member| {
                member
                    .nearest_fingerprints
                    .iter()
                    .map(|candidate| candidate.mean_cosine)
                    .max_by(f32::total_cmp)
                    .map(|value| (1.0 - value).clamp(0.0, 1.0))
                    .unwrap_or(1.0)
            })
            .fold(1.0f32, f32::min);
        clusters.push(CandidateCluster {
            members,
            centroid,
            mean_ood_score,
            mean_disagreement,
            mean_intra_cosine,
            distance_to_nearest_known,
        });
    }
    Ok(clusters)
}

fn proposal_from_cluster(
    cluster: &CandidateCluster<'_>,
    mincut: Option<&MincutReport>,
    mi_signature: Option<InstrumentProposalMiSignature>,
    created_at_unix_ms: i64,
) -> Result<InstrumentProposal, MejepaInferError> {
    let residual_direction = cluster.centroid.clone();
    let name = format!(
        "instrument_unknown_residual_{}",
        hex::encode(&cluster.members[0].candidate_id[..4])
    );
    let structural_gap = mincut.map(structural_gap_score).unwrap_or(0.5);
    let mi_novelty = mi_signature
        .as_ref()
        .map(|sig| 1.0 - sig.mean_off_diagonal)
        .unwrap_or(0.5);
    let expected_holdout_improvement = ((cluster.mean_ood_score
        + cluster.mean_disagreement.clamp(0.0, 1.0)
        + structural_gap
        + mi_novelty)
        / 4.0)
        .clamp(0.0, 1.0);
    let confidence = (cluster.mean_intra_cosine
        * (cluster.members.len() as f32 / 6.0).min(1.0)
        * (1.0 - cluster.distance_to_nearest_known * 0.25))
        .clamp(0.0, 1.0);
    build_proposal(ProposalBuildInput {
        candidate_name: name,
        source_modality: InstrumentProposalModality::HybridResidual,
        residual_direction,
        structural_hole_partition: mincut.map(|report| report.partition.clone()),
        justifying_unknown_cluster: Some(cluster_ref(cluster)?),
        justifying_pairwise_mi_signature: mi_signature,
        expected_holdout_improvement,
        confidence,
        created_at_unix_ms,
    })
}

fn proposal_from_mincut(
    mincut: &MincutReport,
    direction: &[f32],
    mi_signature: Option<InstrumentProposalMiSignature>,
    created_at_unix_ms: i64,
) -> Result<InstrumentProposal, MejepaInferError> {
    let residual_direction = normalize(direction)?;
    let name = format!("instrument_structural_hole_{}", &mincut.report_id[..8]);
    let structural_gap = structural_gap_score(mincut);
    let mi_novelty = mi_signature
        .as_ref()
        .map(|sig| 1.0 - sig.mean_off_diagonal)
        .unwrap_or(0.5);
    let expected_holdout_improvement = (structural_gap * 0.7 + mi_novelty * 0.3).clamp(0.0, 1.0);
    build_proposal(ProposalBuildInput {
        candidate_name: name,
        source_modality: InstrumentProposalModality::StructuralHole,
        residual_direction,
        structural_hole_partition: Some(mincut.partition.clone()),
        justifying_unknown_cluster: None,
        justifying_pairwise_mi_signature: mi_signature,
        expected_holdout_improvement,
        confidence: 0.5 + structural_gap * 0.5,
        created_at_unix_ms,
    })
}

struct ProposalBuildInput {
    candidate_name: String,
    source_modality: InstrumentProposalModality,
    residual_direction: Vec<f32>,
    structural_hole_partition: Option<MincutPartitionReport>,
    justifying_unknown_cluster: Option<InstrumentProposalClusterRef>,
    justifying_pairwise_mi_signature: Option<InstrumentProposalMiSignature>,
    expected_holdout_improvement: f32,
    confidence: f32,
    created_at_unix_ms: i64,
}

fn build_proposal(input: ProposalBuildInput) -> Result<InstrumentProposal, MejepaInferError> {
    let proposal = InstrumentProposal {
        schema_version: INSTRUMENT_PROPOSAL_SCHEMA_VERSION,
        proposal_id: instrument_proposal_id(&input.candidate_name, &input.residual_direction),
        candidate_name: input.candidate_name,
        source_modality: input.source_modality,
        expected_dim: input.residual_direction.len(),
        residual_direction: input.residual_direction,
        structural_hole_partition: input.structural_hole_partition,
        justifying_unknown_cluster: input.justifying_unknown_cluster,
        justifying_pairwise_mi_signature: input.justifying_pairwise_mi_signature,
        expected_holdout_improvement: input.expected_holdout_improvement,
        confidence: input.confidence,
        status: InstrumentProposalStatus::Candidate,
        decision: None,
        created_at_unix_ms: input.created_at_unix_ms,
        source_of_truth_cf: CF_MEJEPA_INSTRUMENT_PROPOSALS.to_string(),
    };
    proposal.validate()?;
    Ok(proposal)
}

fn cluster_ref(
    cluster: &CandidateCluster<'_>,
) -> Result<InstrumentProposalClusterRef, MejepaInferError> {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_INSTRUMENT_PROPOSAL_CLUSTER_V1");
    for member in &cluster.members {
        hasher.update(member.candidate_id);
    }
    let digest = hasher.finalize();
    let mut cluster_id = [0u8; 16];
    cluster_id.copy_from_slice(&digest[..16]);
    Ok(InstrumentProposalClusterRef {
        cluster_id,
        member_candidate_ids: cluster
            .members
            .iter()
            .map(|member| member.candidate_id)
            .collect(),
        member_task_ids: cluster
            .members
            .iter()
            .map(|member| member.task_id.0.clone())
            .collect(),
        mean_ood_score: cluster.mean_ood_score,
        mean_embedder_disagreement_score: cluster.mean_disagreement,
        mean_intra_cluster_cosine: cluster.mean_intra_cosine,
        distance_to_nearest_known: cluster.distance_to_nearest_known,
    })
}

fn mi_signature(
    matrix: &PairwiseMiPersistedMatrix,
) -> Result<InstrumentProposalMiSignature, MejepaInferError> {
    matrix.validate()?;
    let max_row = matrix.pair_rows.iter().max_by(|left, right| {
        left.mi
            .total_cmp(&right.mi)
            .then_with(|| left.embedder_pair.cmp(&right.embedder_pair))
    });
    Ok(InstrumentProposalMiSignature {
        corpus_shard_hash: matrix.corpus_shard_hash.clone(),
        created_at_unix_ms: matrix.created_at_unix_ms,
        effective_signal_count: matrix.health.effective_signal_count,
        mean_off_diagonal: matrix.health.mean_off_diagonal,
        max_redundancy_pair: max_row.map(|row| row.embedder_pair.clone()),
        max_redundancy: max_row.map(|row| row.mi).unwrap_or(0.0),
    })
}

fn latest_mincut_report(reports: &[MincutReport]) -> Option<&MincutReport> {
    reports
        .iter()
        .max_by_key(|report| report.created_at_unix_ms)
}

fn structural_gap_score(report: &MincutReport) -> f32 {
    (1.0 / (1.0 + report.cut_value + report.conductance)).clamp(0.0, 1.0)
}

fn proposal_score(proposal: &InstrumentProposal) -> f32 {
    proposal.expected_holdout_improvement * proposal.confidence
}

fn validate_entry(entry: &ActiveLearningQueueEntry) -> Result<(), MejepaInferError> {
    entry.task_id.validate("active_learning_entry.task_id")?;
    validate_unit_or_more("active_learning_entry.score", entry.score)?;
    validate_unit("active_learning_entry.ood_score", entry.ood_score)?;
    validate_unit(
        "active_learning_entry.curiosity_score",
        entry.curiosity_score,
    )?;
    Ok(())
}

fn flatten_observation(
    observation: &BTreeMap<crate::types::EmbedderId, Vec<f32>>,
) -> Result<Vec<f32>, MejepaInferError> {
    let mut out = Vec::new();
    for (embedder, vector) in observation {
        embedder.validate("instrument_proposal.embedder")?;
        if vector.is_empty() {
            return invalid(
                "instrument_proposal.vector",
                "embedder vector must be non-empty",
            );
        }
        for value in vector {
            if !value.is_finite() {
                return Err(MejepaInferError::NanDetected {
                    nan_source: format!("instrument_proposal.{}", embedder.0),
                    detail: format!("non-finite residual vector value {value}"),
                });
            }
            out.push(*value);
        }
    }
    normalize(&out)
}

fn mean_vector(vectors: &[Vec<f32>]) -> Result<Vec<f32>, MejepaInferError> {
    let first = vectors
        .first()
        .ok_or_else(|| MejepaInferError::InvalidInput {
            field: "vectors".to_string(),
            detail: "mean vector requires at least one vector".to_string(),
        })?;
    let mut out = vec![0.0f32; first.len()];
    for vector in vectors {
        if vector.len() != first.len() {
            return Err(MejepaInferError::DimMismatch {
                expected: first.len(),
                actual: vector.len(),
                context: "instrument proposal cluster vector width".to_string(),
            });
        }
        for (idx, value) in vector.iter().enumerate() {
            out[idx] += *value / vectors.len() as f32;
        }
    }
    Ok(out)
}

fn mean_pairwise_cosine(vectors: &[Vec<f32>]) -> Result<f32, MejepaInferError> {
    if vectors.len() < 2 {
        return Ok(1.0);
    }
    let mut sum = 0.0;
    let mut count = 0usize;
    for idx in 0..vectors.len() {
        for jdx in (idx + 1)..vectors.len() {
            sum += cosine(&vectors[idx], &vectors[jdx])?;
            count += 1;
        }
    }
    Ok((sum / count as f32).clamp(0.0, 1.0))
}

fn normalize(vector: &[f32]) -> Result<Vec<f32>, MejepaInferError> {
    if vector.is_empty() {
        return invalid("vector", "must be non-empty");
    }
    let norm = vector
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt();
    if !norm.is_finite() || norm <= 1.0e-12 {
        return invalid("vector", "norm must be positive and finite");
    }
    Ok(vector
        .iter()
        .map(|value| (*value as f64 / norm) as f32)
        .collect())
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f32, MejepaInferError> {
    if left.len() != right.len() {
        return Err(MejepaInferError::DimMismatch {
            expected: left.len(),
            actual: right.len(),
            context: "instrument proposal cosine".to_string(),
        });
    }
    let left = normalize(left)?;
    let right = normalize(right)?;
    Ok(left
        .iter()
        .zip(right.iter())
        .map(|(lhs, rhs)| lhs * rhs)
        .sum::<f32>()
        .clamp(-1.0, 1.0))
}

fn validate_unit_vector_or_nonzero(field: &str, values: &[f32]) -> Result<(), MejepaInferError> {
    if values.is_empty() {
        return invalid(field, "must be non-empty");
    }
    let mut norm_sq = 0.0f64;
    for value in values {
        if !value.is_finite() {
            return Err(MejepaInferError::NanDetected {
                nan_source: field.to_string(),
                detail: format!("non-finite value {value}"),
            });
        }
        norm_sq += f64::from(*value) * f64::from(*value);
    }
    if norm_sq <= 1.0e-12 {
        return invalid(field, "must have non-zero norm");
    }
    Ok(())
}

fn validate_unit(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return invalid(field, format!("must be finite and in [0,1], got {value}"));
    }
    Ok(())
}

fn validate_unit_or_more(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || value < 0.0 {
        return invalid(
            field,
            format!("must be finite and non-negative, got {value}"),
        );
    }
    Ok(())
}

fn validate_delta(field: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return invalid(field, format!("must be finite and in [-1,1], got {value}"));
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
    use crate::types::{EmbedderId, PredictionId, TaskId};

    #[test]
    fn structural_gap_is_higher_for_low_cut_reports() {
        let low = structural_gap_score(&report(0.1, 0.1));
        let high = structural_gap_score(&report(10.0, 10.0));
        assert!(low > high);
    }

    #[test]
    fn proposal_ids_are_stable() {
        let id1 = instrument_proposal_id("candidate", &[1.0, 0.0]);
        let id2 = instrument_proposal_id("candidate", &[1.0, 0.0]);
        assert_eq!(id1, id2);
    }

    fn report(cut_value: f32, conductance: f32) -> MincutReport {
        MincutReport {
            schema_version: crate::mincut_panel::MINCUT_PANEL_SCHEMA_VERSION,
            report_id: hex::encode(Sha256::digest(format!("{cut_value}:{conductance}"))),
            created_at_unix_ms: 1,
            graph_source_hash: hex::encode(Sha256::digest(b"graph")),
            graph_id: "graph".to_string(),
            graph_source_kind: "inline_weighted_graph".to_string(),
            algorithm: crate::mincut_panel::MincutAlgorithm::StoerWagner,
            node_ids: vec!["a".to_string(), "b".to_string()],
            cut_value,
            partition: MincutPartitionReport {
                left: vec!["a".to_string()],
                right: vec!["b".to_string()],
            },
            recommended_addition_directions: vec![vec![1.0, 0.0]],
            conductance,
            edge_count: 1,
            source_row_count: 1,
            warnings: Vec::new(),
            source_of_truth_cf: CF_MEJEPA_MINCUT_REPORTS.to_string(),
        }
    }

    #[allow(dead_code)]
    fn candidate(id: u8, x: f32, y: f32) -> UnknownFingerprintCandidate {
        UnknownFingerprintCandidate {
            candidate_id: [id; 16],
            prediction_id: PredictionId([id; 16]),
            task_id: TaskId(format!("candidate-{id}")),
            session_id: [id; 16],
            observed_at_unix_ms: 1,
            ood_score: 0.9,
            embedder_disagreement_score: 0.8,
            active_learning_priority: 5,
            observation_by_embedder: BTreeMap::from([(
                EmbedderId("E_Test".to_string()),
                vec![x, y],
            )]),
            nearest_fingerprints: Vec::new(),
        }
    }
}
