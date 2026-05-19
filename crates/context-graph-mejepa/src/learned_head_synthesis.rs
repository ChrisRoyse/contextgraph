use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::dynamic_embedder::{DynamicEmbedderKind, RuntimeEmbedderId};
use crate::embedder_falsification::{
    evaluate_and_persist_embedder_falsification, proposer_used_window_ids_from_proposal,
    write_embedder_proposal_rejection_sync_readback, CellFalsificationDelta,
    EmbedderCandidateHoldoutComparison, EmbedderFalsificationDecision, EmbedderFalsificationGate,
    EmbedderProposalRejectionRecord, EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
};
use crate::embedder_proposal::PendingEmbedderProposal;
use crate::error::MejepaInferError;
use crate::heal::per_cell_promotion::PromotionScore;
use crate::heal::promote::ModeWinner;

pub const LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_LEARNED_HEAD_INPUT_DIM: usize = 16;
pub const DEFAULT_LEARNED_HEAD_OUTPUT_DIM: usize = 128;
pub const DEFAULT_LEARNED_HEAD_EPOCHS: usize = 240;
pub const DEFAULT_LEARNED_HEAD_LEARNING_RATE: f32 = 0.05;
pub const DEFAULT_LEARNED_HEAD_VARIANCE_FLOOR: f32 = 1e-4;
pub const DEFAULT_LEARNED_HEAD_DIVERGENCE_LOSS_CEILING: f32 = 1.0e6;
pub const MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED: &str = "MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED";
pub const MEJEPA_LEARNED_HEAD_COLLAPSED: &str = "MEJEPA_LEARNED_HEAD_COLLAPSED";
pub const MEJEPA_LEARNED_HEAD_NO_CONTRASTIVE_GAIN: &str = "MEJEPA_LEARNED_HEAD_NO_CONTRASTIVE_GAIN";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnedHeadSynthesisMode {
    ContrastiveTrain,
    DivergenceFixture,
    ConstantCollapseFixture,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrozenBackboneExample {
    pub example_id: String,
    pub backbone_embedder_id: String,
    pub vector: Vec<f32>,
    pub class_id: u32,
    pub source_ref: String,
}

impl FrozenBackboneExample {
    pub fn validate(&self, expected_dim: usize) -> Result<(), MejepaInferError> {
        validate_single_line("learned_head.example_id", &self.example_id, 128)?;
        validate_single_line(
            "learned_head.backbone_embedder_id",
            &self.backbone_embedder_id,
            64,
        )?;
        if self.vector.len() != expected_dim {
            return invalid(
                "learned_head.vector",
                format!("expected dim {expected_dim}, got {}", self.vector.len()),
            );
        }
        if self.vector.iter().any(|value| !value.is_finite()) {
            return invalid("learned_head.vector", "values must be finite");
        }
        validate_single_line("learned_head.source_ref", &self.source_ref, 512)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnedHeadTrainingConfig {
    pub input_dim: usize,
    pub output_dim: usize,
    pub epochs: usize,
    pub learning_rate: f32,
    pub variance_floor: f32,
    pub divergence_loss_ceiling: f32,
    pub artifact_root: PathBuf,
}

impl Default for LearnedHeadTrainingConfig {
    fn default() -> Self {
        Self {
            input_dim: DEFAULT_LEARNED_HEAD_INPUT_DIM,
            output_dim: DEFAULT_LEARNED_HEAD_OUTPUT_DIM,
            epochs: DEFAULT_LEARNED_HEAD_EPOCHS,
            learning_rate: DEFAULT_LEARNED_HEAD_LEARNING_RATE,
            variance_floor: DEFAULT_LEARNED_HEAD_VARIANCE_FLOOR,
            divergence_loss_ceiling: DEFAULT_LEARNED_HEAD_DIVERGENCE_LOSS_CEILING,
            artifact_root: PathBuf::from("/zfs/archive/contextgraph/models/dynamic"),
        }
    }
}

impl LearnedHeadTrainingConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.input_dim == 0 || self.input_dim > 4096 {
            return invalid("learned_head.input_dim", "must be in [1, 4096]");
        }
        if self.output_dim == 0 || self.output_dim > 4096 {
            return invalid("learned_head.output_dim", "must be in [1, 4096]");
        }
        if self.epochs == 0 || self.epochs > 100_000 {
            return invalid("learned_head.epochs", "must be in [1, 100000]");
        }
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            return invalid("learned_head.learning_rate", "must be positive and finite");
        }
        if !self.variance_floor.is_finite() || self.variance_floor <= 0.0 {
            return invalid("learned_head.variance_floor", "must be positive and finite");
        }
        if !self.divergence_loss_ceiling.is_finite() || self.divergence_loss_ceiling <= 0.0 {
            return invalid(
                "learned_head.divergence_loss_ceiling",
                "must be positive and finite",
            );
        }
        if self.artifact_root.as_os_str().is_empty() {
            return invalid("learned_head.artifact_root", "must be non-empty");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnedHeadCandidate {
    pub schema_version: u32,
    pub proposal_id: [u8; 16],
    pub candidate_id: RuntimeEmbedderId,
    pub candidate_name: String,
    pub mode: LearnedHeadSynthesisMode,
    pub backbone_embedder_id: String,
    pub input_dim: usize,
    pub output_dim: usize,
    pub architecture_signature: String,
    pub proposal_source_refs: Vec<String>,
    pub proposer_used_window_ids: Vec<String>,
    pub created_at_unix_ms: i64,
}

impl LearnedHeadCandidate {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION {
            return invalid(
                "learned_head_candidate.schema_version",
                format!(
                    "expected {LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        if self.proposal_id.iter().all(|byte| *byte == 0) {
            return invalid("learned_head_candidate.proposal_id", "must be non-zero");
        }
        self.candidate_id.validate().map_err(embed_error)?;
        if !self.candidate_id.is_dynamic() {
            return invalid("learned_head_candidate.id", "candidate id must be EDynamic");
        }
        validate_single_line("learned_head_candidate.name", &self.candidate_name, 128)?;
        validate_single_line(
            "learned_head_candidate.backbone_embedder_id",
            &self.backbone_embedder_id,
            64,
        )?;
        if self.input_dim == 0 || self.output_dim == 0 {
            return invalid(
                "learned_head_candidate.dim",
                "input and output dimensions must be non-zero",
            );
        }
        validate_single_line(
            "learned_head_candidate.architecture_signature",
            &self.architecture_signature,
            512,
        )?;
        validate_non_empty_texts(
            "learned_head_candidate.proposal_source_refs",
            &self.proposal_source_refs,
            512,
        )?;
        validate_non_empty_texts(
            "learned_head_candidate.proposer_used_window_ids",
            &self.proposer_used_window_ids,
            256,
        )?;
        if self.created_at_unix_ms <= 0 {
            return invalid(
                "learned_head_candidate.created_at_unix_ms",
                "must be positive",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectionHeadModel {
    pub input_dim: usize,
    pub output_dim: usize,
    pub weights: Vec<Vec<f32>>,
    pub bias: Vec<f32>,
}

impl ProjectionHeadModel {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.input_dim == 0 || self.output_dim == 0 {
            return invalid("learned_head_model.dim", "dimensions must be non-zero");
        }
        if self.weights.len() != self.output_dim || self.bias.len() != self.output_dim {
            return invalid(
                "learned_head_model.shape",
                "weight/bias output shape mismatch",
            );
        }
        for row in &self.weights {
            if row.len() != self.input_dim {
                return invalid("learned_head_model.shape", "weight input shape mismatch");
            }
            if row.iter().any(|value| !value.is_finite()) {
                return invalid("learned_head_model.weights", "weights must be finite");
            }
        }
        if self.bias.iter().any(|value| !value.is_finite()) {
            return invalid("learned_head_model.bias", "bias values must be finite");
        }
        Ok(())
    }

    pub fn project(&self, input: &[f32]) -> Result<Vec<f32>, MejepaInferError> {
        self.validate()?;
        if input.len() != self.input_dim {
            return invalid(
                "learned_head_model.project.input",
                format!("expected {}, got {}", self.input_dim, input.len()),
            );
        }
        if input.iter().any(|value| !value.is_finite()) {
            return invalid("learned_head_model.project.input", "input must be finite");
        }
        let mut out = self.bias.clone();
        for (dim, row) in self.weights.iter().enumerate() {
            out[dim] += row
                .iter()
                .zip(input)
                .map(|(weight, value)| weight * value)
                .sum::<f32>();
        }
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnedHeadTrainingReport {
    pub initial_loss: f32,
    pub final_loss: f32,
    pub initial_contrastive_loss: f32,
    pub final_contrastive_loss: f32,
    pub epochs_completed: usize,
    pub sample_count: usize,
    pub input_dim: usize,
    pub output_dim: usize,
    pub converged: bool,
    pub diverged: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnedHeadCollapseReport {
    pub sample_count: usize,
    pub output_dim: usize,
    pub min_dimension_variance: f32,
    pub mean_dimension_variance: f32,
    pub variance_floor: f32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnedHeadArtifactReport {
    pub artifact_path: PathBuf,
    pub artifact_sha256: String,
    pub artifact_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearnedHeadSynthesisReport {
    pub schema_version: u32,
    pub candidate: LearnedHeadCandidate,
    pub training: LearnedHeadTrainingReport,
    pub collapse: Option<LearnedHeadCollapseReport>,
    pub artifact: Option<LearnedHeadArtifactReport>,
    pub falsification_decision: Option<EmbedderFalsificationDecision>,
    pub accepted: bool,
    pub reason_code: Option<String>,
    pub rejection_key_hex: Option<String>,
    pub training_cert_chain_hash: String,
    pub source_of_truth_rejection_cf: String,
}

pub fn synthesize_learned_head_candidate(
    proposal: &PendingEmbedderProposal,
    sequence: u32,
    backbone_embedder_id: impl Into<String>,
    mode: LearnedHeadSynthesisMode,
    config: &LearnedHeadTrainingConfig,
    created_at_unix_ms: i64,
) -> Result<LearnedHeadCandidate, MejepaInferError> {
    proposal.validate()?;
    config.validate()?;
    if sequence == 0 {
        return invalid("learned_head_candidate.sequence", "must be non-zero");
    }
    if created_at_unix_ms <= 0 {
        return invalid(
            "learned_head_candidate.created_at_unix_ms",
            "must be positive",
        );
    }
    let backbone_embedder_id = backbone_embedder_id.into();
    validate_single_line(
        "learned_head_candidate.backbone_embedder_id",
        &backbone_embedder_id,
        64,
    )?;
    let candidate_name = learned_head_candidate_name(proposal, mode);
    let candidate_id =
        RuntimeEmbedderId::dynamic(sequence, candidate_name.clone()).map_err(embed_error)?;
    let architecture_signature = format!(
        "learned-head:{}:backbone:{}:proposal:{}:in{}:out{}:objective:{}",
        mode_name(mode),
        safe_file_component(&backbone_embedder_id),
        hex::encode(proposal.proposal_id),
        config.input_dim,
        config.output_dim,
        safe_file_component(&proposal.descriptor.suggested_objective)
    );
    let candidate = LearnedHeadCandidate {
        schema_version: LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION,
        proposal_id: proposal.proposal_id,
        candidate_id,
        candidate_name,
        mode,
        backbone_embedder_id,
        input_dim: config.input_dim,
        output_dim: config.output_dim,
        architecture_signature,
        proposal_source_refs: proposal
            .source_signals
            .iter()
            .map(|signal| signal.source_ref.clone())
            .collect(),
        proposer_used_window_ids: proposer_used_window_ids_from_proposal(proposal)?,
        created_at_unix_ms,
    };
    candidate.validate()?;
    Ok(candidate)
}

pub fn train_and_evaluate_learned_head_synthesis(
    db: &DB,
    candidate: LearnedHeadCandidate,
    examples: &[FrozenBackboneExample],
    config: &LearnedHeadTrainingConfig,
    comparison: Option<EmbedderCandidateHoldoutComparison>,
    gate: EmbedderFalsificationGate,
) -> Result<LearnedHeadSynthesisReport, MejepaInferError> {
    candidate.validate()?;
    config.validate()?;
    gate.validate()?;
    validate_examples(examples, config.input_dim)?;
    fs::create_dir_all(&config.artifact_root)
        .map_err(|err| MejepaInferError::io("create_dir_all", &config.artifact_root, err))?;

    let (mut model, mut training) = train_projection_head(&candidate, examples, config)?;
    if candidate.mode == LearnedHeadSynthesisMode::ConstantCollapseFixture {
        model = constant_projection_model(config.input_dim, config.output_dim);
        training.final_loss = mse_loss(&model, examples)?;
        training.final_contrastive_loss =
            contrastive_loss(&project_examples(&model, examples)?, examples)?;
        training.converged = false;
    }
    let training_cert_chain_hash = training_cert_hash(&candidate, &training);
    if training.diverged {
        let rejection = learned_head_rejection_record(
            &candidate,
            MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED,
            "learned projection-head training diverged or produced non-finite loss",
            None,
            &training_cert_chain_hash,
        )?;
        let key = write_embedder_proposal_rejection_sync_readback(db, &rejection)?;
        return Ok(report_for_rejection(
            candidate,
            training,
            None,
            None,
            None,
            Some(hex::encode(key)),
            training_cert_chain_hash,
            MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED,
        ));
    }

    let projections = project_examples(&model, examples)?;
    let collapse = evaluate_learned_head_collapse(&projections, config)?;
    if collapse.collapsed {
        let rejection = learned_head_rejection_record(
            &candidate,
            MEJEPA_LEARNED_HEAD_COLLAPSED,
            "learned projection head collapsed below SIGReg/VICReg variance floor",
            None,
            &training_cert_chain_hash,
        )?;
        let key = write_embedder_proposal_rejection_sync_readback(db, &rejection)?;
        return Ok(report_for_rejection(
            candidate,
            training,
            Some(collapse),
            None,
            None,
            Some(hex::encode(key)),
            training_cert_chain_hash,
            MEJEPA_LEARNED_HEAD_COLLAPSED,
        ));
    }
    if !training.converged {
        let rejection = learned_head_rejection_record(
            &candidate,
            MEJEPA_LEARNED_HEAD_NO_CONTRASTIVE_GAIN,
            "learned projection head failed to improve contrastive objective",
            None,
            &training_cert_chain_hash,
        )?;
        let key = write_embedder_proposal_rejection_sync_readback(db, &rejection)?;
        return Ok(report_for_rejection(
            candidate,
            training,
            Some(collapse),
            None,
            None,
            Some(hex::encode(key)),
            training_cert_chain_hash,
            MEJEPA_LEARNED_HEAD_NO_CONTRASTIVE_GAIN,
        ));
    }

    let artifact = write_learned_head_artifact(&candidate, &model, &training, config)?;
    let mut comparison = comparison.ok_or_else(|| MejepaInferError::InvalidInput {
        field: "learned_head_synthesis.comparison".to_string(),
        detail: "accepted learned-head candidates require held-out falsification comparison"
            .to_string(),
    })?;
    if comparison.candidate_id != candidate.candidate_id
        || comparison.proposal_id != candidate.proposal_id
        || comparison.candidate_architecture_signature != candidate.architecture_signature
    {
        return invalid(
            "learned_head_synthesis.comparison",
            "comparison must match candidate id, proposal id, and architecture signature",
        );
    }
    comparison.candidate_artifact_sha256 = artifact.artifact_sha256.clone();
    comparison.training_cert_chain_hash = training_cert_chain_hash.clone();
    let decision = evaluate_and_persist_embedder_falsification(db, &comparison, gate)?;
    let accepted = decision.accepted;
    let reason_code = decision.reason_code.clone();
    Ok(LearnedHeadSynthesisReport {
        schema_version: LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION,
        candidate,
        training,
        collapse: Some(collapse),
        artifact: Some(artifact),
        falsification_decision: Some(decision),
        accepted,
        reason_code,
        rejection_key_hex: None,
        training_cert_chain_hash,
        source_of_truth_rejection_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    })
}

pub fn learned_head_registry_kind() -> DynamicEmbedderKind {
    DynamicEmbedderKind::LearnedHead
}

pub fn train_projection_head(
    candidate: &LearnedHeadCandidate,
    examples: &[FrozenBackboneExample],
    config: &LearnedHeadTrainingConfig,
) -> Result<(ProjectionHeadModel, LearnedHeadTrainingReport), MejepaInferError> {
    candidate.validate()?;
    config.validate()?;
    validate_examples(examples, config.input_dim)?;
    if candidate.mode == LearnedHeadSynthesisMode::DivergenceFixture {
        return Ok((
            initial_model(config.input_dim, config.output_dim),
            LearnedHeadTrainingReport {
                initial_loss: f32::INFINITY,
                final_loss: f32::INFINITY,
                initial_contrastive_loss: f32::INFINITY,
                final_contrastive_loss: f32::INFINITY,
                epochs_completed: 1,
                sample_count: examples.len(),
                input_dim: config.input_dim,
                output_dim: config.output_dim,
                converged: false,
                diverged: true,
            },
        ));
    }
    let mut model = initial_model(config.input_dim, config.output_dim);
    let initial_loss = mse_loss(&model, examples)?;
    let initial_contrastive_loss =
        contrastive_loss(&project_examples(&model, examples)?, examples)?;
    let mut final_loss = initial_loss;
    let mut epochs_completed = 0usize;
    let mut diverged = false;
    for epoch in 0..config.epochs {
        let mut grad_w = vec![vec![0.0f32; config.input_dim]; config.output_dim];
        let mut grad_b = vec![0.0f32; config.output_dim];
        let scale = 2.0 / (examples.len() * config.output_dim) as f32;
        for example in examples {
            let pred = model.project(&example.vector)?;
            let target = class_target(example.class_id, config.output_dim);
            for (out_dim, grad_row) in grad_w.iter_mut().enumerate() {
                let err = (pred[out_dim] - target[out_dim]) * scale;
                grad_b[out_dim] += err;
                for (in_dim, grad_item) in grad_row.iter_mut().enumerate() {
                    *grad_item += err * example.vector[in_dim];
                }
            }
        }
        for (out_dim, grad_row) in grad_w.iter().enumerate() {
            model.bias[out_dim] -= config.learning_rate * grad_b[out_dim];
            for (in_dim, grad_item) in grad_row.iter().enumerate() {
                model.weights[out_dim][in_dim] -= config.learning_rate * grad_item;
            }
        }
        final_loss = mse_loss(&model, examples)?;
        epochs_completed = epoch + 1;
        if !final_loss.is_finite() || final_loss > config.divergence_loss_ceiling {
            diverged = true;
            break;
        }
    }
    let final_contrastive_loss = if diverged {
        f32::INFINITY
    } else {
        contrastive_loss(&project_examples(&model, examples)?, examples)?
    };
    let converged =
        !diverged && final_loss < initial_loss && final_contrastive_loss < initial_contrastive_loss;
    Ok((
        model,
        LearnedHeadTrainingReport {
            initial_loss,
            final_loss,
            initial_contrastive_loss,
            final_contrastive_loss,
            epochs_completed,
            sample_count: examples.len(),
            input_dim: config.input_dim,
            output_dim: config.output_dim,
            converged,
            diverged,
        },
    ))
}

pub fn evaluate_learned_head_collapse(
    projections: &[Vec<f32>],
    config: &LearnedHeadTrainingConfig,
) -> Result<LearnedHeadCollapseReport, MejepaInferError> {
    config.validate()?;
    if projections.len() < 2 {
        return invalid("learned_head.projections", "need at least two projections");
    }
    for projection in projections {
        if projection.len() != config.output_dim {
            return invalid(
                "learned_head.projection",
                format!(
                    "expected dim {}, got {}",
                    config.output_dim,
                    projection.len()
                ),
            );
        }
        if projection.iter().any(|value| !value.is_finite()) {
            return invalid(
                "learned_head.projection",
                "projection values must be finite",
            );
        }
    }
    let mut variances = vec![0.0f32; config.output_dim];
    for dim in 0..config.output_dim {
        let mean = projections.iter().map(|row| row[dim]).sum::<f32>() / projections.len() as f32;
        variances[dim] = projections
            .iter()
            .map(|row| {
                let delta = row[dim] - mean;
                delta * delta
            })
            .sum::<f32>()
            / projections.len() as f32;
    }
    let min_dimension_variance = variances
        .iter()
        .copied()
        .fold(f32::INFINITY, |left, right| left.min(right));
    let mean_dimension_variance = variances.iter().sum::<f32>() / variances.len() as f32;
    Ok(LearnedHeadCollapseReport {
        sample_count: projections.len(),
        output_dim: config.output_dim,
        min_dimension_variance,
        mean_dimension_variance,
        variance_floor: config.variance_floor,
        collapsed: mean_dimension_variance < config.variance_floor,
    })
}

fn write_learned_head_artifact(
    candidate: &LearnedHeadCandidate,
    model: &ProjectionHeadModel,
    training: &LearnedHeadTrainingReport,
    config: &LearnedHeadTrainingConfig,
) -> Result<LearnedHeadArtifactReport, MejepaInferError> {
    model.validate()?;
    let artifact_path = config
        .artifact_root
        .join(candidate.candidate_id.slug().replace(':', "_"))
        .join("projection_head.json");
    if let Some(parent) = artifact_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| MejepaInferError::io("create_dir_all", parent, err))?;
    }
    let bytes = serde_json::to_vec_pretty(&json!({
        "schemaVersion": LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION,
        "candidate": candidate,
        "model": model,
        "training": training,
        "frozen": true,
        "targetCollapse": 0.0
    }))?;
    fs::write(&artifact_path, &bytes)
        .map_err(|err| MejepaInferError::io("write", &artifact_path, err))?;
    let readback = fs::read(&artifact_path)
        .map_err(|err| MejepaInferError::io("read", &artifact_path, err))?;
    if readback != bytes {
        return invalid("learned_head.artifact_readback", "artifact bytes changed");
    }
    Ok(LearnedHeadArtifactReport {
        artifact_path,
        artifact_sha256: sha256_hex(&readback),
        artifact_bytes: readback.len() as u64,
    })
}

fn learned_head_rejection_record(
    candidate: &LearnedHeadCandidate,
    reason_code: &str,
    reason: &str,
    artifact_sha256: Option<&str>,
    training_cert_chain_hash: &str,
) -> Result<EmbedderProposalRejectionRecord, MejepaInferError> {
    let candidate_artifact_sha256 = artifact_sha256
        .map(str::to_string)
        .unwrap_or_else(|| sha256_hex(candidate.architecture_signature.as_bytes()));
    let record = EmbedderProposalRejectionRecord {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        rejection_id: rejection_id(candidate, reason_code),
        proposal_id: candidate.proposal_id,
        candidate_id: candidate.candidate_id.clone(),
        candidate_name: candidate.candidate_name.clone(),
        candidate_architecture_signature: candidate.architecture_signature.clone(),
        candidate_artifact_sha256,
        training_cert_chain_hash: training_cert_chain_hash.to_string(),
        proposal_source_refs: candidate.proposal_source_refs.clone(),
        reason_code: reason_code.to_string(),
        reason: reason.to_string(),
        winner: ModeWinner::A,
        global_delta: 0.0,
        min_cell_delta: 0.0,
        mode_a_score: zero_score(),
        mode_b_score: zero_score(),
        mode_c_score: zero_score(),
        compared_cells: BTreeMap::from([(
            "learned_head_training:e7".to_string(),
            CellFalsificationDelta {
                before: 0.0,
                after: Some(0.0),
                delta: 0.0,
                holds_or_improves: true,
            },
        )]),
        regressing_cells: Vec::new(),
        overlapping_window_ids: Vec::new(),
        proposer_used_window_ids: candidate.proposer_used_window_ids.clone(),
        heldout_window_ids: vec!["learned-head:no-heldout-after-rejection".to_string()],
        created_at_unix_ms: candidate.created_at_unix_ms,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    };
    record.validate()?;
    Ok(record)
}

fn report_for_rejection(
    candidate: LearnedHeadCandidate,
    training: LearnedHeadTrainingReport,
    collapse: Option<LearnedHeadCollapseReport>,
    artifact: Option<LearnedHeadArtifactReport>,
    falsification_decision: Option<EmbedderFalsificationDecision>,
    rejection_key_hex: Option<String>,
    training_cert_chain_hash: String,
    reason_code: &str,
) -> LearnedHeadSynthesisReport {
    LearnedHeadSynthesisReport {
        schema_version: LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION,
        candidate,
        training,
        collapse,
        artifact,
        falsification_decision,
        accepted: false,
        reason_code: Some(reason_code.to_string()),
        rejection_key_hex,
        training_cert_chain_hash,
        source_of_truth_rejection_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    }
}

fn validate_examples(
    examples: &[FrozenBackboneExample],
    expected_dim: usize,
) -> Result<(), MejepaInferError> {
    if examples.len() < 4 {
        return invalid("learned_head.examples", "need at least four examples");
    }
    let mut labels = BTreeMap::<u32, usize>::new();
    for example in examples {
        example.validate(expected_dim)?;
        *labels.entry(example.class_id).or_default() += 1;
    }
    if labels.len() < 2 || labels.values().any(|count| *count < 2) {
        return invalid(
            "learned_head.examples",
            "need at least two labels with at least two examples each",
        );
    }
    Ok(())
}

fn initial_model(input_dim: usize, output_dim: usize) -> ProjectionHeadModel {
    let mut weights = vec![vec![0.0; input_dim]; output_dim];
    for (out_dim, row) in weights.iter_mut().enumerate() {
        for (in_dim, item) in row.iter_mut().enumerate() {
            *item = ((out_dim as f32 + 1.0) * (in_dim as f32 + 3.0)).sin() * 0.005;
        }
    }
    ProjectionHeadModel {
        input_dim,
        output_dim,
        weights,
        bias: vec![0.0; output_dim],
    }
}

fn constant_projection_model(input_dim: usize, output_dim: usize) -> ProjectionHeadModel {
    ProjectionHeadModel {
        input_dim,
        output_dim,
        weights: vec![vec![0.0; input_dim]; output_dim],
        bias: vec![0.125; output_dim],
    }
}

fn mse_loss(
    model: &ProjectionHeadModel,
    examples: &[FrozenBackboneExample],
) -> Result<f32, MejepaInferError> {
    let mut loss = 0.0f32;
    for example in examples {
        let pred = model.project(&example.vector)?;
        let target = class_target(example.class_id, model.output_dim);
        loss += pred
            .iter()
            .zip(target)
            .map(|(left, right)| {
                let delta = left - right;
                delta * delta
            })
            .sum::<f32>()
            / model.output_dim as f32;
    }
    Ok(loss / examples.len() as f32)
}

fn project_examples(
    model: &ProjectionHeadModel,
    examples: &[FrozenBackboneExample],
) -> Result<Vec<Vec<f32>>, MejepaInferError> {
    examples
        .iter()
        .map(|example| model.project(&example.vector))
        .collect()
}

fn contrastive_loss(
    projections: &[Vec<f32>],
    examples: &[FrozenBackboneExample],
) -> Result<f32, MejepaInferError> {
    if projections.len() != examples.len() {
        return invalid(
            "learned_head.contrastive",
            "projection/example count mismatch",
        );
    }
    let mut loss = 0.0f32;
    let mut pairs = 0usize;
    for left in 0..projections.len() {
        for right in (left + 1)..projections.len() {
            let cosine = cosine(&projections[left], &projections[right])?;
            if examples[left].class_id == examples[right].class_id {
                loss += 1.0 - cosine;
            } else {
                loss += cosine.max(0.0);
            }
            pairs += 1;
        }
    }
    Ok(loss / pairs.max(1) as f32)
}

fn class_target(class_id: u32, dim: usize) -> Vec<f32> {
    (0..dim)
        .map(|idx| {
            let raw = ((class_id as f32 + 1.0) * (idx as f32 + 1.0) * 0.73).sin();
            raw * 0.75
        })
        .collect()
}

fn cosine(left: &[f32], right: &[f32]) -> Result<f32, MejepaInferError> {
    if left.len() != right.len() || left.is_empty() {
        return invalid("learned_head.cosine", "vectors must have same non-zero dim");
    }
    let dot = left.iter().zip(right).map(|(l, r)| l * r).sum::<f32>();
    let left_norm = left.iter().map(|value| value * value).sum::<f32>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f32>().sqrt();
    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        Ok(0.0)
    } else {
        Ok(dot / (left_norm * right_norm))
    }
}

fn learned_head_candidate_name(
    proposal: &PendingEmbedderProposal,
    mode: LearnedHeadSynthesisMode,
) -> String {
    let base = safe_file_component(&proposal.candidate_name);
    match mode {
        LearnedHeadSynthesisMode::ContrastiveTrain => format!("{base}_head_v1"),
        LearnedHeadSynthesisMode::DivergenceFixture => format!("{base}_head_diverge_v1"),
        LearnedHeadSynthesisMode::ConstantCollapseFixture => format!("{base}_head_constant_v1"),
    }
}

fn mode_name(mode: LearnedHeadSynthesisMode) -> &'static str {
    match mode {
        LearnedHeadSynthesisMode::ContrastiveTrain => "contrastive_train",
        LearnedHeadSynthesisMode::DivergenceFixture => "divergence_fixture",
        LearnedHeadSynthesisMode::ConstantCollapseFixture => "constant_collapse_fixture",
    }
}

fn training_cert_hash(
    candidate: &LearnedHeadCandidate,
    training: &LearnedHeadTrainingReport,
) -> String {
    sha256_hex(
        serde_json::to_vec(&json!({
            "schemaVersion": LEARNED_HEAD_SYNTHESIS_SCHEMA_VERSION,
            "candidate": candidate,
            "training": training
        }))
        .expect("learned-head cert JSON serialization should be infallible for finite report")
        .as_slice(),
    )
}

fn rejection_id(candidate: &LearnedHeadCandidate, reason_code: &str) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_LEARNED_HEAD_REJECTION_V1");
    hasher.update(candidate.proposal_id);
    hasher.update(candidate.candidate_id.slug().as_bytes());
    hasher.update(candidate.architecture_signature.as_bytes());
    hasher.update(reason_code.as_bytes());
    let digest = hasher.finalize();
    let mut out = [0u8; 16];
    out.copy_from_slice(&digest[..16]);
    out
}

fn zero_score() -> PromotionScore {
    PromotionScore {
        holdout_correlation: 0.0,
        latency_multiplier: 1.0,
        score: 0.0,
    }
}

fn validate_non_empty_texts(
    field: &str,
    values: &[String],
    max_len: usize,
) -> Result<(), MejepaInferError> {
    if values.is_empty() {
        return invalid(field, "must be non-empty");
    }
    for value in values {
        validate_single_line(field, value, max_len)?;
    }
    Ok(())
}

fn validate_single_line(field: &str, value: &str, max_len: usize) -> Result<(), MejepaInferError> {
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

fn safe_file_component(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').chars().take(48).collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "learned_head_candidate.id".to_string(),
        detail: err.to_string(),
    }
}

fn invalid<T>(field: impl Into<String>, detail: impl Into<String>) -> Result<T, MejepaInferError> {
    Err(MejepaInferError::InvalidInput {
        field: field.into(),
        detail: detail.into(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::embedder_proposal::{
        AbsenceShapeDescriptor, EmbedderAbsenceSignalKind, EmbedderProposalSourceEvidence,
        PendingEmbedderProposal, EMBEDDER_PROPOSAL_SCHEMA_VERSION,
    };
    use crate::heal::promote::HoldoutEval;
    use crate::{open_infer_rocksdb, EMBEDDER_FALSIFICATION_SCHEMA_VERSION};

    use super::*;

    #[test]
    fn contrastive_head_trains_and_passes_falsification() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let config = LearnedHeadTrainingConfig {
            input_dim: 6,
            output_dim: 8,
            epochs: 200,
            artifact_root: temp.path().join("models"),
            ..LearnedHeadTrainingConfig::default()
        };
        let proposal = proposal_fixture();
        let candidate = synthesize_learned_head_candidate(
            &proposal,
            31,
            "E7",
            LearnedHeadSynthesisMode::ContrastiveTrain,
            &config,
            1_779_100_000_000,
        )
        .unwrap();
        let report = train_and_evaluate_learned_head_synthesis(
            db.as_ref(),
            candidate.clone(),
            &examples(),
            &config,
            Some(comparison_fixture(&candidate)),
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(report.accepted);
        assert!(report.training.converged);
        assert!(report.training.final_loss < report.training.initial_loss);
        assert!(!report.collapse.as_ref().unwrap().collapsed);
        assert!(report.artifact.is_some());
    }

    #[test]
    fn divergent_training_is_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let config = LearnedHeadTrainingConfig {
            input_dim: 6,
            output_dim: 8,
            artifact_root: temp.path().join("models"),
            ..LearnedHeadTrainingConfig::default()
        };
        let candidate = synthesize_learned_head_candidate(
            &proposal_fixture(),
            32,
            "E7",
            LearnedHeadSynthesisMode::DivergenceFixture,
            &config,
            1_779_100_000_000,
        )
        .unwrap();
        let report = train_and_evaluate_learned_head_synthesis(
            db.as_ref(),
            candidate,
            &examples(),
            &config,
            None,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!report.accepted);
        assert_eq!(
            report.reason_code.as_deref(),
            Some(MEJEPA_LEARNED_HEAD_TRAINING_DIVERGED)
        );
        assert!(report.rejection_key_hex.is_some());
    }

    #[test]
    fn constant_head_is_rejected_by_collapse_guard() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let config = LearnedHeadTrainingConfig {
            input_dim: 6,
            output_dim: 8,
            artifact_root: temp.path().join("models"),
            ..LearnedHeadTrainingConfig::default()
        };
        let candidate = synthesize_learned_head_candidate(
            &proposal_fixture(),
            33,
            "E7",
            LearnedHeadSynthesisMode::ConstantCollapseFixture,
            &config,
            1_779_100_000_000,
        )
        .unwrap();
        let report = train_and_evaluate_learned_head_synthesis(
            db.as_ref(),
            candidate,
            &examples(),
            &config,
            None,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!report.accepted);
        assert_eq!(
            report.reason_code.as_deref(),
            Some(MEJEPA_LEARNED_HEAD_COLLAPSED)
        );
        assert!(report.collapse.unwrap().collapsed);
    }

    fn proposal_fixture() -> PendingEmbedderProposal {
        PendingEmbedderProposal {
            schema_version: EMBEDDER_PROPOSAL_SCHEMA_VERSION,
            proposal_id: [5u8; 16],
            candidate_name: "e7_blind_spot".to_string(),
            descriptor: AbsenceShapeDescriptor {
                input_modality: "e7_frozen_content_vector".to_string(),
                suggested_dim: 8,
                suggested_objective: "contrastive blind-spot projection".to_string(),
                reality_channel_for_falsification: "heldout-window:e7-blind-spot".to_string(),
                signal_magnitude: 0.44,
            },
            composite_score: 0.84,
            predicted_delta_cp_phi: 0.08,
            novelty_vs_existing_proposals: 0.90,
            source_signals: vec![EmbedderProposalSourceEvidence {
                kind: EmbedderAbsenceSignalKind::UnknownFingerprintCluster,
                source_ref: "unknown-cluster:e7-blind-spot".to_string(),
                signal_magnitude: 0.44,
                predicted_delta_cp_phi: 0.08,
                used_window_ids: vec!["proposal-window:e7:0".to_string()],
            }],
            created_at_unix_ms: 1_779_100_000_000,
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
        }
    }

    fn examples() -> Vec<FrozenBackboneExample> {
        vec![
            example("a0", 0, [0.90, 0.10, 0.20, 0.00, 0.15, 0.30]),
            example("a1", 0, [0.86, 0.12, 0.22, 0.02, 0.12, 0.28]),
            example("a2", 0, [0.88, 0.09, 0.19, 0.01, 0.16, 0.31]),
            example("b0", 1, [0.05, 0.82, 0.15, 0.76, 0.20, 0.12]),
            example("b1", 1, [0.04, 0.86, 0.14, 0.73, 0.18, 0.10]),
            example("b2", 1, [0.06, 0.80, 0.18, 0.78, 0.21, 0.11]),
        ]
    }

    fn example(id: &str, class_id: u32, values: [f32; 6]) -> FrozenBackboneExample {
        FrozenBackboneExample {
            example_id: id.to_string(),
            backbone_embedder_id: "E7".to_string(),
            vector: values.to_vec(),
            class_id,
            source_ref: format!("fixture:{id}"),
        }
    }

    fn comparison_fixture(candidate: &LearnedHeadCandidate) -> EmbedderCandidateHoldoutComparison {
        EmbedderCandidateHoldoutComparison {
            schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
            proposal_id: candidate.proposal_id,
            candidate_id: candidate.candidate_id.clone(),
            candidate_name: candidate.candidate_name.clone(),
            candidate_architecture_signature: candidate.architecture_signature.clone(),
            candidate_artifact_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            training_cert_chain_hash:
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            proposal_source_refs: candidate.proposal_source_refs.clone(),
            proposer_used_window_ids: candidate.proposer_used_window_ids.clone(),
            heldout_window_ids: vec!["heldout-window:e7:1".to_string()],
            mode_a: eval(
                0.900,
                BTreeMap::from([
                    ("e7_blind_spot:python".to_string(), 0.848),
                    ("mutation:python".to_string(), 0.862),
                ]),
            ),
            mode_b: eval(
                0.908,
                BTreeMap::from([
                    ("e7_blind_spot:python".to_string(), 0.858),
                    ("mutation:python".to_string(), 0.869),
                ]),
            ),
            mode_c: eval(
                0.899,
                BTreeMap::from([
                    ("e7_blind_spot:python".to_string(), 0.847),
                    ("mutation:python".to_string(), 0.861),
                ]),
            ),
            created_at_unix_ms: 1_779_100_000_001,
        }
    }

    fn eval(global: f32, cells: BTreeMap<String, f32>) -> HoldoutEval {
        HoldoutEval::try_new_with_cells(0.95, global, 0.01, 128, [6u8; 32], cells).unwrap()
    }
}
