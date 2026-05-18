use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS;
use rocksdb::DB;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::dynamic_embedder::{DynamicEmbedderKind, RuntimeEmbedderId};
use crate::embedder_falsification::{
    embedder_architecture_has_rejection, evaluate_and_persist_embedder_falsification,
    proposer_used_window_ids_from_proposal, write_embedder_proposal_rejection_sync_readback,
    CellFalsificationDelta, EmbedderCandidateHoldoutComparison, EmbedderFalsificationDecision,
    EmbedderFalsificationGate, EmbedderProposalRejectionRecord,
    EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
};
use crate::embedder_proposal::PendingEmbedderProposal;
use crate::error::MejepaInferError;
use crate::heal::per_cell_promotion::PromotionScore;
use crate::heal::promote::ModeWinner;

pub const ALGORITHMIC_EMBEDDER_SYNTHESIS_SCHEMA_VERSION: u32 = 1;
pub const DEFAULT_ALGORITHMIC_VARIANCE_FLOOR: f32 = 1e-4;
pub const MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED: &str =
    "MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED";
pub const MEJEPA_ALGORITHMIC_EMBEDDER_TEST_FAILED: &str = "MEJEPA_ALGORITHMIC_EMBEDDER_TEST_FAILED";
pub const MEJEPA_ALGORITHMIC_EMBEDDER_FUZZ_FAILED: &str = "MEJEPA_ALGORITHMIC_EMBEDDER_FUZZ_FAILED";
pub const MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED: &str = "MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED";
pub const MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_REPLAY_BLOCKED: &str =
    "MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_REPLAY_BLOCKED";

const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_STDOUT_BYTES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmicKernelTemplate {
    IdentifierLength,
    CompileFailureFixture,
    ConstantCollapseFixture,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlgorithmicEmbedderSynthesisConfig {
    pub rustc_path: PathBuf,
    pub work_root: PathBuf,
    pub artifact_root: PathBuf,
    pub variance_floor: f32,
    pub fuzz_inputs: Vec<String>,
}

impl AlgorithmicEmbedderSynthesisConfig {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.rustc_path.as_os_str().is_empty() {
            return invalid("algorithmic_synthesis.rustc_path", "must be non-empty");
        }
        if self.work_root.as_os_str().is_empty() {
            return invalid("algorithmic_synthesis.work_root", "must be non-empty");
        }
        if self.artifact_root.as_os_str().is_empty() {
            return invalid("algorithmic_synthesis.artifact_root", "must be non-empty");
        }
        if !self.variance_floor.is_finite() || self.variance_floor <= 0.0 {
            return invalid(
                "algorithmic_synthesis.variance_floor",
                "must be positive and finite",
            );
        }
        if self.fuzz_inputs.len() < 3 {
            return invalid(
                "algorithmic_synthesis.fuzz_inputs",
                "need at least three deterministic fuzz inputs",
            );
        }
        for input in &self.fuzz_inputs {
            validate_single_line("algorithmic_synthesis.fuzz_input", input, 1024)?;
        }
        Ok(())
    }
}

impl Default for AlgorithmicEmbedderSynthesisConfig {
    fn default() -> Self {
        Self {
            rustc_path: PathBuf::from("rustc"),
            work_root: std::env::temp_dir().join("contextgraph-algorithmic-embedder-synthesis"),
            artifact_root: PathBuf::from("/mnt/d/contextgraph/models/dynamic"),
            variance_floor: DEFAULT_ALGORITHMIC_VARIANCE_FLOOR,
            fuzz_inputs: default_fuzz_inputs(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlgorithmicEmbedderCandidate {
    pub schema_version: u32,
    pub proposal_id: [u8; 16],
    pub candidate_id: RuntimeEmbedderId,
    pub candidate_name: String,
    pub template: AlgorithmicKernelTemplate,
    pub dimension: usize,
    pub architecture_prompt: String,
    pub architecture_signature: String,
    pub source_code: String,
    pub source_sha256: String,
    pub proposal_source_refs: Vec<String>,
    pub proposer_used_window_ids: Vec<String>,
    pub created_at_unix_ms: i64,
}

impl AlgorithmicEmbedderCandidate {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != ALGORITHMIC_EMBEDDER_SYNTHESIS_SCHEMA_VERSION {
            return invalid(
                "algorithmic_candidate.schema_version",
                format!(
                    "expected {ALGORITHMIC_EMBEDDER_SYNTHESIS_SCHEMA_VERSION}, got {}",
                    self.schema_version
                ),
            );
        }
        if self.proposal_id.iter().all(|byte| *byte == 0) {
            return invalid("algorithmic_candidate.proposal_id", "must be non-zero");
        }
        self.candidate_id.validate().map_err(embed_error)?;
        if !self.candidate_id.is_dynamic() {
            return invalid("algorithmic_candidate.id", "candidate id must be EDynamic");
        }
        validate_single_line(
            "algorithmic_candidate.candidate_name",
            &self.candidate_name,
            128,
        )?;
        if self.dimension == 0 || self.dimension > 8192 {
            return invalid(
                "algorithmic_candidate.dimension",
                "dimension must be in [1, 8192]",
            );
        }
        validate_single_line(
            "algorithmic_candidate.architecture_signature",
            &self.architecture_signature,
            512,
        )?;
        validate_sha256("algorithmic_candidate.source_sha256", &self.source_sha256)?;
        if self.source_code.is_empty() || self.source_code.len() > MAX_SOURCE_BYTES {
            return invalid(
                "algorithmic_candidate.source_code",
                format!("source must be non-empty and <= {MAX_SOURCE_BYTES} bytes"),
            );
        }
        validate_sandbox_source(&self.source_code)?;
        validate_non_empty_texts(
            "algorithmic_candidate.proposal_source_refs",
            &self.proposal_source_refs,
            512,
        )?;
        validate_non_empty_texts(
            "algorithmic_candidate.proposer_used_window_ids",
            &self.proposer_used_window_ids,
            256,
        )?;
        if self.created_at_unix_ms <= 0 {
            return invalid(
                "algorithmic_candidate.created_at_unix_ms",
                "must be positive",
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlgorithmicSandboxReport {
    pub source_path: PathBuf,
    pub test_binary_path: PathBuf,
    pub artifact_path: PathBuf,
    pub rustc_test_status: Option<i32>,
    pub test_binary_status: Option<i32>,
    pub rustc_cdylib_status: Option<i32>,
    pub rustc_test_stdout: String,
    pub rustc_test_stderr: String,
    pub test_stdout: String,
    pub test_stderr: String,
    pub rustc_cdylib_stdout: String,
    pub rustc_cdylib_stderr: String,
    pub artifact_sha256: Option<String>,
}

impl AlgorithmicSandboxReport {
    pub fn passed(&self) -> bool {
        self.rustc_test_status == Some(0)
            && self.test_binary_status == Some(0)
            && self.rustc_cdylib_status == Some(0)
            && self.artifact_sha256.is_some()
    }

    pub fn first_failure_code(&self) -> Option<&'static str> {
        if self.rustc_test_status != Some(0) || self.rustc_cdylib_status != Some(0) {
            Some(MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED)
        } else if self.test_binary_status != Some(0) {
            Some(MEJEPA_ALGORITHMIC_EMBEDDER_TEST_FAILED)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlgorithmicCollapseReport {
    pub sample_count: usize,
    pub dimension: usize,
    pub min_dimension_variance: f32,
    pub mean_dimension_variance: f32,
    pub variance_floor: f32,
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AlgorithmicEmbedderSynthesisReport {
    pub schema_version: u32,
    pub candidate: AlgorithmicEmbedderCandidate,
    pub sandbox: AlgorithmicSandboxReport,
    pub collapse: Option<AlgorithmicCollapseReport>,
    pub falsification_decision: Option<EmbedderFalsificationDecision>,
    pub accepted: bool,
    pub reason_code: Option<String>,
    pub rejection_key_hex: Option<String>,
    pub training_cert_chain_hash: String,
    pub source_of_truth_rejection_cf: String,
}

impl AlgorithmicEmbedderSynthesisReport {
    pub fn artifact_sha256(&self) -> Option<&str> {
        self.sandbox.artifact_sha256.as_deref()
    }
}

pub fn synthesize_algorithmic_embedder_candidate(
    proposal: &PendingEmbedderProposal,
    sequence: u32,
    template: AlgorithmicKernelTemplate,
    created_at_unix_ms: i64,
) -> Result<AlgorithmicEmbedderCandidate, MejepaInferError> {
    proposal.validate()?;
    if sequence == 0 {
        return invalid("algorithmic_candidate.sequence", "must be non-zero");
    }
    if created_at_unix_ms <= 0 {
        return invalid(
            "algorithmic_candidate.created_at_unix_ms",
            "must be positive",
        );
    }
    let candidate_name = candidate_name_for(proposal, template);
    let candidate_id =
        RuntimeEmbedderId::dynamic(sequence, candidate_name.clone()).map_err(embed_error)?;
    let dimension = proposal.descriptor.suggested_dim.clamp(3, 256);
    let architecture_prompt = algorithmic_kernel_prompt(proposal, template)?;
    let architecture_signature = architecture_signature(proposal, template, dimension);
    let source_code = render_kernel_source(template, dimension);
    let source_sha256 = sha256_hex(source_code.as_bytes());
    let proposer_used_window_ids = proposer_used_window_ids_from_proposal(proposal)?;
    let candidate = AlgorithmicEmbedderCandidate {
        schema_version: ALGORITHMIC_EMBEDDER_SYNTHESIS_SCHEMA_VERSION,
        proposal_id: proposal.proposal_id,
        candidate_id,
        candidate_name,
        template,
        dimension,
        architecture_prompt,
        architecture_signature,
        source_code,
        source_sha256,
        proposal_source_refs: proposal
            .source_signals
            .iter()
            .map(|signal| signal.source_ref.clone())
            .collect(),
        proposer_used_window_ids,
        created_at_unix_ms,
    };
    candidate.validate()?;
    Ok(candidate)
}

pub fn evaluate_algorithmic_embedder_synthesis(
    db: &DB,
    candidate: AlgorithmicEmbedderCandidate,
    config: &AlgorithmicEmbedderSynthesisConfig,
    comparison: Option<EmbedderCandidateHoldoutComparison>,
    gate: EmbedderFalsificationGate,
) -> Result<AlgorithmicEmbedderSynthesisReport, MejepaInferError> {
    candidate.validate()?;
    config.validate()?;
    gate.validate()?;
    fs::create_dir_all(&config.work_root)
        .map_err(|err| MejepaInferError::io("create_dir_all", &config.work_root, err))?;
    fs::create_dir_all(&config.artifact_root)
        .map_err(|err| MejepaInferError::io("create_dir_all", &config.artifact_root, err))?;

    let training_cert_chain_hash = sha256_json_like(&[
        (
            "architecture_signature",
            candidate.architecture_signature.as_str(),
        ),
        ("source_sha256", candidate.source_sha256.as_str()),
        ("template", template_name(candidate.template)),
    ]);

    if embedder_architecture_has_rejection(db, &candidate.architecture_signature)? {
        let rejection = algorithmic_rejection_record(
            &candidate,
            MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_REPLAY_BLOCKED,
            "architecture signature already appears in CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS",
            None,
            &training_cert_chain_hash,
        )?;
        let key = write_embedder_proposal_rejection_sync_readback(db, &rejection)?;
        return Ok(report_for_rejection(
            candidate,
            empty_sandbox_report(),
            None,
            None,
            Some(hex::encode(key)),
            training_cert_chain_hash,
            MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_REPLAY_BLOCKED,
        ));
    }

    let sandbox = run_algorithmic_kernel_sandbox(&candidate, config)?;
    if !sandbox.passed() {
        let reason_code = sandbox
            .first_failure_code()
            .unwrap_or(MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED);
        let rejection = algorithmic_rejection_record(
            &candidate,
            reason_code,
            "algorithmic embedder candidate failed sandbox compile/test",
            sandbox.artifact_sha256.as_deref(),
            &training_cert_chain_hash,
        )?;
        let key = write_embedder_proposal_rejection_sync_readback(db, &rejection)?;
        return Ok(report_for_rejection(
            candidate,
            sandbox,
            None,
            None,
            Some(hex::encode(key)),
            training_cert_chain_hash,
            reason_code,
        ));
    }

    let collapse = evaluate_algorithmic_collapse(&candidate, config)?;
    if collapse.collapsed {
        let rejection = algorithmic_rejection_record(
            &candidate,
            MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED,
            "algorithmic embedder candidate collapsed below SIGReg/VICReg variance floor",
            sandbox.artifact_sha256.as_deref(),
            &training_cert_chain_hash,
        )?;
        let key = write_embedder_proposal_rejection_sync_readback(db, &rejection)?;
        return Ok(report_for_rejection(
            candidate,
            sandbox,
            Some(collapse),
            None,
            Some(hex::encode(key)),
            training_cert_chain_hash,
            MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED,
        ));
    }

    let mut comparison = comparison.ok_or_else(|| MejepaInferError::InvalidInput {
        field: "algorithmic_synthesis.comparison".to_string(),
        detail: "accepted sandbox candidates require held-out falsification comparison".to_string(),
    })?;
    if comparison.candidate_id != candidate.candidate_id
        || comparison.proposal_id != candidate.proposal_id
        || comparison.candidate_architecture_signature != candidate.architecture_signature
    {
        return invalid(
            "algorithmic_synthesis.comparison",
            "comparison must match candidate id, proposal id, and architecture signature",
        );
    }
    if let Some(artifact_sha256) = &sandbox.artifact_sha256 {
        comparison.candidate_artifact_sha256 = artifact_sha256.clone();
    }
    comparison.training_cert_chain_hash = training_cert_chain_hash.clone();
    let decision = evaluate_and_persist_embedder_falsification(db, &comparison, gate)?;
    let accepted = decision.accepted;
    let reason_code = decision.reason_code.clone();
    Ok(AlgorithmicEmbedderSynthesisReport {
        schema_version: ALGORITHMIC_EMBEDDER_SYNTHESIS_SCHEMA_VERSION,
        candidate,
        sandbox,
        collapse: Some(collapse),
        falsification_decision: Some(decision),
        accepted,
        reason_code,
        rejection_key_hex: None,
        training_cert_chain_hash,
        source_of_truth_rejection_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    })
}

pub fn run_algorithmic_kernel_sandbox(
    candidate: &AlgorithmicEmbedderCandidate,
    config: &AlgorithmicEmbedderSynthesisConfig,
) -> Result<AlgorithmicSandboxReport, MejepaInferError> {
    candidate.validate()?;
    config.validate()?;
    let safe_name = safe_file_component(&candidate.candidate_name);
    let work_dir = config.work_root.join(format!(
        "{}-{}",
        safe_name,
        hex::encode(candidate.proposal_id)
    ));
    fs::create_dir_all(&work_dir)
        .map_err(|err| MejepaInferError::io("create_dir_all", &work_dir, err))?;
    let source_path = work_dir.join("candidate.rs");
    let test_binary_path = work_dir.join("candidate-tests");
    let artifact_path = config
        .artifact_root
        .join(candidate.candidate_id.slug().replace(':', "_"))
        .join("forward.so");
    if let Some(parent) = artifact_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| MejepaInferError::io("create_dir_all", parent, err))?;
    }
    fs::write(&source_path, candidate.source_code.as_bytes())
        .map_err(|err| MejepaInferError::io("write", &source_path, err))?;
    let source_readback =
        fs::read(&source_path).map_err(|err| MejepaInferError::io("read", &source_path, err))?;
    if sha256_hex(&source_readback) != candidate.source_sha256 {
        return invalid(
            "algorithmic_synthesis.source_readback",
            "source readback sha256 differs from candidate source sha256",
        );
    }

    let rustc_test = Command::new(&config.rustc_path)
        .arg("--edition=2021")
        .arg("--test")
        .arg(&source_path)
        .arg("-O")
        .arg("-o")
        .arg(&test_binary_path)
        .output()
        .map_err(|err| MejepaInferError::io("exec", &config.rustc_path, err))?;
    let mut test_binary_status = None;
    let mut test_stdout = String::new();
    let mut test_stderr = String::new();
    if rustc_test.status.success() {
        let test_run = Command::new(&test_binary_path)
            .output()
            .map_err(|err| MejepaInferError::io("exec", &test_binary_path, err))?;
        test_binary_status = test_run.status.code();
        test_stdout = bounded_output(&test_run.stdout);
        test_stderr = bounded_output(&test_run.stderr);
    }
    let rustc_cdylib = Command::new(&config.rustc_path)
        .arg("--edition=2021")
        .arg("--crate-type")
        .arg("cdylib")
        .arg(&source_path)
        .arg("-O")
        .arg("-o")
        .arg(&artifact_path)
        .output()
        .map_err(|err| MejepaInferError::io("exec", &config.rustc_path, err))?;
    let artifact_sha256 = if rustc_cdylib.status.success() {
        let bytes = fs::read(&artifact_path)
            .map_err(|err| MejepaInferError::io("read", &artifact_path, err))?;
        if bytes.is_empty() {
            None
        } else {
            Some(sha256_hex(&bytes))
        }
    } else {
        None
    };
    Ok(AlgorithmicSandboxReport {
        source_path,
        test_binary_path,
        artifact_path,
        rustc_test_status: rustc_test.status.code(),
        test_binary_status,
        rustc_cdylib_status: rustc_cdylib.status.code(),
        rustc_test_stdout: bounded_output(&rustc_test.stdout),
        rustc_test_stderr: bounded_output(&rustc_test.stderr),
        test_stdout,
        test_stderr,
        rustc_cdylib_stdout: bounded_output(&rustc_cdylib.stdout),
        rustc_cdylib_stderr: bounded_output(&rustc_cdylib.stderr),
        artifact_sha256,
    })
}

pub fn evaluate_algorithmic_collapse(
    candidate: &AlgorithmicEmbedderCandidate,
    config: &AlgorithmicEmbedderSynthesisConfig,
) -> Result<AlgorithmicCollapseReport, MejepaInferError> {
    candidate.validate()?;
    config.validate()?;
    let vectors = config
        .fuzz_inputs
        .iter()
        .map(|input| {
            deterministic_kernel_projection(candidate.template, input, candidate.dimension)
        })
        .collect::<Vec<_>>();
    let mut variances = vec![0.0f32; candidate.dimension];
    for dim in 0..candidate.dimension {
        let mean = vectors.iter().map(|vector| vector[dim]).sum::<f32>() / vectors.len() as f32;
        variances[dim] = vectors
            .iter()
            .map(|vector| {
                let delta = vector[dim] - mean;
                delta * delta
            })
            .sum::<f32>()
            / vectors.len() as f32;
    }
    let min_dimension_variance = variances
        .iter()
        .copied()
        .fold(f32::INFINITY, |left, right| left.min(right));
    let mean_dimension_variance = variances.iter().sum::<f32>() / variances.len() as f32;
    let collapsed = mean_dimension_variance < config.variance_floor;
    Ok(AlgorithmicCollapseReport {
        sample_count: vectors.len(),
        dimension: candidate.dimension,
        min_dimension_variance,
        mean_dimension_variance,
        variance_floor: config.variance_floor,
        collapsed,
    })
}

pub fn algorithmic_registry_kind() -> DynamicEmbedderKind {
    DynamicEmbedderKind::Algorithmic
}

pub fn algorithmic_kernel_prompt(
    proposal: &PendingEmbedderProposal,
    template: AlgorithmicKernelTemplate,
) -> Result<String, MejepaInferError> {
    proposal.validate()?;
    Ok(format!(
        "Write one deterministic Rust algorithmic embedder kernel for candidate={} template={} input_modality={} dim={} objective={} falsification_channel={}. The kernel must be pure, bounded, deterministic, no file/network/process access, and must expose contextgraph_dynamic_embedder_abi_version plus contextgraph_dynamic_embedder_dimension.",
        proposal.candidate_name,
        template_name(template),
        proposal.descriptor.input_modality,
        proposal.descriptor.suggested_dim,
        proposal.descriptor.suggested_objective,
        proposal.descriptor.reality_channel_for_falsification
    ))
}

fn algorithmic_rejection_record(
    candidate: &AlgorithmicEmbedderCandidate,
    reason_code: &str,
    reason: &str,
    artifact_sha256: Option<&str>,
    training_cert_chain_hash: &str,
) -> Result<EmbedderProposalRejectionRecord, MejepaInferError> {
    let reason_code = reason_code.to_string();
    let artifact_sha256 = artifact_sha256
        .unwrap_or(&candidate.source_sha256)
        .to_string();
    let compared_cells = BTreeMap::from([(
        "algorithmic_sandbox:rust".to_string(),
        CellFalsificationDelta {
            before: 0.0,
            after: Some(0.0),
            delta: 0.0,
            holds_or_improves: true,
        },
    )]);
    let record = EmbedderProposalRejectionRecord {
        schema_version: EMBEDDER_FALSIFICATION_SCHEMA_VERSION,
        rejection_id: rejection_id(candidate, &reason_code),
        proposal_id: candidate.proposal_id,
        candidate_id: candidate.candidate_id.clone(),
        candidate_name: candidate.candidate_name.clone(),
        candidate_architecture_signature: candidate.architecture_signature.clone(),
        candidate_artifact_sha256: artifact_sha256,
        training_cert_chain_hash: training_cert_chain_hash.to_string(),
        proposal_source_refs: candidate.proposal_source_refs.clone(),
        reason_code,
        reason: reason.to_string(),
        winner: ModeWinner::A,
        global_delta: 0.0,
        min_cell_delta: 0.0,
        mode_a_score: zero_score(),
        mode_b_score: zero_score(),
        mode_c_score: zero_score(),
        compared_cells,
        regressing_cells: Vec::new(),
        overlapping_window_ids: Vec::new(),
        proposer_used_window_ids: candidate.proposer_used_window_ids.clone(),
        heldout_window_ids: vec!["algorithmic-sandbox:no-heldout-after-rejection".to_string()],
        created_at_unix_ms: candidate.created_at_unix_ms,
        source_of_truth_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    };
    record.validate()?;
    Ok(record)
}

fn report_for_rejection(
    candidate: AlgorithmicEmbedderCandidate,
    sandbox: AlgorithmicSandboxReport,
    collapse: Option<AlgorithmicCollapseReport>,
    falsification_decision: Option<EmbedderFalsificationDecision>,
    rejection_key_hex: Option<String>,
    training_cert_chain_hash: String,
    reason_code: &str,
) -> AlgorithmicEmbedderSynthesisReport {
    AlgorithmicEmbedderSynthesisReport {
        schema_version: ALGORITHMIC_EMBEDDER_SYNTHESIS_SCHEMA_VERSION,
        candidate,
        sandbox,
        collapse,
        falsification_decision,
        accepted: false,
        reason_code: Some(reason_code.to_string()),
        rejection_key_hex,
        training_cert_chain_hash,
        source_of_truth_rejection_cf: CF_MEJEPA_EMBEDDER_PROPOSAL_REJECTIONS.to_string(),
    }
}

fn render_kernel_source(template: AlgorithmicKernelTemplate, dimension: usize) -> String {
    match template {
        AlgorithmicKernelTemplate::IdentifierLength => render_identifier_length_source(dimension),
        AlgorithmicKernelTemplate::CompileFailureFixture => {
            "pub fn contextgraph_dynamic_embed_for_test(input: &str) -> Vec<f32> { let _ = input; vec![0.0; 3] ".to_string()
        }
        AlgorithmicKernelTemplate::ConstantCollapseFixture => render_constant_source(dimension),
    }
}

fn render_identifier_length_source(dimension: usize) -> String {
    format!(
        r#"
const DIMENSION: usize = {dimension};

#[no_mangle]
pub extern "C" fn contextgraph_dynamic_embedder_abi_version() -> u32 {{ 1 }}

#[no_mangle]
pub extern "C" fn contextgraph_dynamic_embedder_dimension() -> usize {{ DIMENSION }}

pub fn contextgraph_dynamic_embed_for_test(input: &str) -> Vec<f32> {{
    let mut out = vec![0.0_f32; DIMENSION];
    let identifiers = input
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .filter(|part| !part.is_empty())
        .count() as f32;
    let byte_len = input.len() as f32;
    let line_count = input.lines().count().max(1) as f32;
    out[0] = identifiers / (identifiers + 1.0);
    if DIMENSION > 1 {{
        out[1] = (byte_len % 97.0) / 97.0;
    }}
    if DIMENSION > 2 {{
        out[2] = line_count / (line_count + 8.0);
    }}
    let mut idx = 3;
    while idx < DIMENSION {{
        let value = ((byte_len + idx as f32 * 13.0).sin() + 1.0) * 0.5;
        out[idx] = value;
        idx += 1;
    }}
    out
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn identifier_length_kernel_is_deterministic_and_nonconstant() {{
        let short = contextgraph_dynamic_embed_for_test("fn a() {{}}");
        let long = contextgraph_dynamic_embed_for_test("fn parse_identifier_length_alpha_beta(value: usize) -> usize {{ value + 1 }}");
        assert_eq!(short.len(), DIMENSION);
        assert_eq!(short, contextgraph_dynamic_embed_for_test("fn a() {{}}"));
        assert_ne!(short, long);
        assert!(long[0] >= short[0]);
    }}
}}
"#
    )
}

fn render_constant_source(dimension: usize) -> String {
    format!(
        r#"
const DIMENSION: usize = {dimension};

#[no_mangle]
pub extern "C" fn contextgraph_dynamic_embedder_abi_version() -> u32 {{ 1 }}

#[no_mangle]
pub extern "C" fn contextgraph_dynamic_embedder_dimension() -> usize {{ DIMENSION }}

pub fn contextgraph_dynamic_embed_for_test(input: &str) -> Vec<f32> {{
    let _ = input;
    vec![0.25_f32; DIMENSION]
}}

#[cfg(test)]
mod tests {{
    use super::*;

    #[test]
    fn constant_kernel_is_deterministic() {{
        assert_eq!(contextgraph_dynamic_embed_for_test("a"), contextgraph_dynamic_embed_for_test("b"));
    }}
}}
"#
    )
}

fn deterministic_kernel_projection(
    template: AlgorithmicKernelTemplate,
    input: &str,
    dimension: usize,
) -> Vec<f32> {
    match template {
        AlgorithmicKernelTemplate::IdentifierLength => {
            let mut out = vec![0.0f32; dimension];
            let identifiers = input
                .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .filter(|part| !part.is_empty())
                .count() as f32;
            let byte_len = input.len() as f32;
            let line_count = input.lines().count().max(1) as f32;
            out[0] = identifiers / (identifiers + 1.0);
            if dimension > 1 {
                out[1] = (byte_len % 97.0) / 97.0;
            }
            if dimension > 2 {
                out[2] = line_count / (line_count + 8.0);
            }
            for (idx, item) in out.iter_mut().enumerate().skip(3) {
                *item = ((byte_len + idx as f32 * 13.0).sin() + 1.0) * 0.5;
            }
            out
        }
        AlgorithmicKernelTemplate::CompileFailureFixture
        | AlgorithmicKernelTemplate::ConstantCollapseFixture => vec![0.25; dimension],
    }
}

fn candidate_name_for(
    proposal: &PendingEmbedderProposal,
    template: AlgorithmicKernelTemplate,
) -> String {
    let base = safe_file_component(&proposal.candidate_name);
    match template {
        AlgorithmicKernelTemplate::IdentifierLength => format!("{base}_identifier_length_v1"),
        AlgorithmicKernelTemplate::CompileFailureFixture => format!("{base}_compile_fail_v1"),
        AlgorithmicKernelTemplate::ConstantCollapseFixture => format!("{base}_constant_v1"),
    }
}

fn architecture_signature(
    proposal: &PendingEmbedderProposal,
    template: AlgorithmicKernelTemplate,
    dimension: usize,
) -> String {
    format!(
        "algorithmic-kernel:{}:proposal:{}:dim{}:objective:{}",
        template_name(template),
        hex::encode(proposal.proposal_id),
        dimension,
        safe_file_component(&proposal.descriptor.suggested_objective)
    )
}

fn template_name(template: AlgorithmicKernelTemplate) -> &'static str {
    match template {
        AlgorithmicKernelTemplate::IdentifierLength => "identifier_length",
        AlgorithmicKernelTemplate::CompileFailureFixture => "compile_failure_fixture",
        AlgorithmicKernelTemplate::ConstantCollapseFixture => "constant_collapse_fixture",
    }
}

fn validate_sandbox_source(source: &str) -> Result<(), MejepaInferError> {
    if !source.is_ascii() {
        return invalid("algorithmic_candidate.source_code", "source must be ASCII");
    }
    let forbidden = [
        "std::fs",
        "std::process",
        "std::net",
        "include!",
        "include_str!",
        "include_bytes!",
        "env!",
        "unsafe",
        "extern crate",
        "#![feature",
        "Command",
        "TcpStream",
        "UdpSocket",
    ];
    for token in forbidden {
        if source.contains(token) {
            return invalid(
                "algorithmic_candidate.source_code",
                format!("source contains forbidden token {token}"),
            );
        }
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

fn validate_sha256(field: &str, value: &str) -> Result<(), MejepaInferError> {
    validate_single_line(field, value, 64)?;
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return invalid(field, "must be a 64-character sha256 hex digest");
    }
    Ok(())
}

fn rejection_id(candidate: &AlgorithmicEmbedderCandidate, reason_code: &str) -> [u8; 16] {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_ALGORITHMIC_EMBEDDER_REJECTION_V1");
    hasher.update(candidate.proposal_id);
    hasher.update(candidate.candidate_id.slug().as_bytes());
    hasher.update(candidate.architecture_signature.as_bytes());
    hasher.update(candidate.source_sha256.as_bytes());
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

fn empty_sandbox_report() -> AlgorithmicSandboxReport {
    AlgorithmicSandboxReport {
        source_path: PathBuf::new(),
        test_binary_path: PathBuf::new(),
        artifact_path: PathBuf::new(),
        rustc_test_status: None,
        test_binary_status: None,
        rustc_cdylib_status: None,
        rustc_test_stdout: String::new(),
        rustc_test_stderr: String::new(),
        test_stdout: String::new(),
        test_stderr: String::new(),
        rustc_cdylib_stdout: String::new(),
        rustc_cdylib_stderr: String::new(),
        artifact_sha256: None,
    }
}

fn safe_file_component(value: &str) -> String {
    let mut out = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    while out.contains("__") {
        out = out.replace("__", "_");
    }
    out.trim_matches('_').chars().take(64).collect()
}

fn bounded_output(bytes: &[u8]) -> String {
    let mut text = String::from_utf8_lossy(bytes).into_owned();
    if text.len() > MAX_STDOUT_BYTES {
        text.truncate(MAX_STDOUT_BYTES);
        text.push_str("...[truncated]");
    }
    text
}

fn default_fuzz_inputs() -> Vec<String> {
    vec![
        "fn a() {}".to_string(),
        "fn parse_identifier_length_alpha_beta(value: usize) -> usize { value + 1 }".to_string(),
        "class User: pass".to_string(),
        "def handle_request(user_id): return db.fetch_user(user_id)".to_string(),
        "import pathlib\npathlib.Path('x').read_text()".replace('\n', "\\n"),
    ]
}

fn sha256_json_like(items: &[(&str, &str)]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"MEJEPA_ALGORITHMIC_EMBEDDER_TRAINING_CERT_V1");
    for (key, value) in items {
        hasher.update(key.as_bytes());
        hasher.update(b"\0");
        hasher.update(value.as_bytes());
        hasher.update(b"\0");
    }
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn embed_error(err: context_graph_mejepa_embedders::EmbedError) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: "algorithmic_candidate.id".to_string(),
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
    fn identifier_length_candidate_passes_sandbox_and_falsification() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let config = AlgorithmicEmbedderSynthesisConfig {
            work_root: temp.path().join("work"),
            artifact_root: temp.path().join("models"),
            ..AlgorithmicEmbedderSynthesisConfig::default()
        };
        let proposal = proposal_fixture();
        let candidate = synthesize_algorithmic_embedder_candidate(
            &proposal,
            11,
            AlgorithmicKernelTemplate::IdentifierLength,
            1_779_100_000_000,
        )
        .unwrap();
        let comparison = comparison_fixture(&candidate, 0.907);

        let report = evaluate_algorithmic_embedder_synthesis(
            db.as_ref(),
            candidate,
            &config,
            Some(comparison),
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(report.accepted);
        assert!(report.sandbox.passed());
        assert!(!report.collapse.as_ref().unwrap().collapsed);
        assert!(report.artifact_sha256().is_some());
    }

    #[test]
    fn compile_failure_is_persisted_as_rejection() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let config = AlgorithmicEmbedderSynthesisConfig {
            work_root: temp.path().join("work"),
            artifact_root: temp.path().join("models"),
            ..AlgorithmicEmbedderSynthesisConfig::default()
        };
        let candidate = synthesize_algorithmic_embedder_candidate(
            &proposal_fixture(),
            12,
            AlgorithmicKernelTemplate::CompileFailureFixture,
            1_779_100_000_000,
        )
        .unwrap();

        let report = evaluate_algorithmic_embedder_synthesis(
            db.as_ref(),
            candidate,
            &config,
            None,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!report.accepted);
        assert_eq!(
            report.reason_code.as_deref(),
            Some(MEJEPA_ALGORITHMIC_EMBEDDER_COMPILE_FAILED)
        );
        assert!(report.rejection_key_hex.is_some());
    }

    #[test]
    fn constant_candidate_is_rejected_by_collapse_defense() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_infer_rocksdb(temp.path().join("db")).unwrap();
        let config = AlgorithmicEmbedderSynthesisConfig {
            work_root: temp.path().join("work"),
            artifact_root: temp.path().join("models"),
            ..AlgorithmicEmbedderSynthesisConfig::default()
        };
        let candidate = synthesize_algorithmic_embedder_candidate(
            &proposal_fixture(),
            13,
            AlgorithmicKernelTemplate::ConstantCollapseFixture,
            1_779_100_000_000,
        )
        .unwrap();

        let report = evaluate_algorithmic_embedder_synthesis(
            db.as_ref(),
            candidate,
            &config,
            None,
            EmbedderFalsificationGate::default(),
        )
        .unwrap();

        assert!(!report.accepted);
        assert_eq!(
            report.reason_code.as_deref(),
            Some(MEJEPA_ALGORITHMIC_EMBEDDER_COLLAPSED)
        );
        assert!(report.collapse.unwrap().collapsed);
    }

    fn proposal_fixture() -> PendingEmbedderProposal {
        PendingEmbedderProposal {
            schema_version: EMBEDDER_PROPOSAL_SCHEMA_VERSION,
            proposal_id: [3u8; 16],
            candidate_name: "identifier_length_gap".to_string(),
            descriptor: AbsenceShapeDescriptor {
                input_modality: "rust_identifier_tokens".to_string(),
                suggested_dim: 8,
                suggested_objective: "separate identifier-length-sensitive failures".to_string(),
                reality_channel_for_falsification: "heldout-window:identifier-length".to_string(),
                signal_magnitude: 0.42,
            },
            composite_score: 0.80,
            predicted_delta_cp_phi: 0.07,
            novelty_vs_existing_proposals: 0.90,
            source_signals: vec![EmbedderProposalSourceEvidence {
                kind: EmbedderAbsenceSignalKind::MincutStructuralHole,
                source_ref: "mincut:identifier-length".to_string(),
                signal_magnitude: 0.42,
                predicted_delta_cp_phi: 0.07,
                used_window_ids: vec!["proposal-window:0".to_string()],
            }],
            created_at_unix_ms: 1_779_100_000_000,
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_EMBEDDER_PROPOSALS.to_string(),
        }
    }

    fn comparison_fixture(
        candidate: &AlgorithmicEmbedderCandidate,
        mode_b_global: f32,
    ) -> EmbedderCandidateHoldoutComparison {
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
            heldout_window_ids: vec!["heldout-window:1".to_string()],
            mode_a: eval(
                0.900,
                BTreeMap::from([
                    ("identifier_length:rust".to_string(), 0.850),
                    ("mutation:rust".to_string(), 0.861),
                ]),
            ),
            mode_b: eval(
                mode_b_global,
                BTreeMap::from([
                    ("identifier_length:rust".to_string(), 0.858),
                    ("mutation:rust".to_string(), 0.868),
                ]),
            ),
            mode_c: eval(
                0.899,
                BTreeMap::from([
                    ("identifier_length:rust".to_string(), 0.849),
                    ("mutation:rust".to_string(), 0.860),
                ]),
            ),
            created_at_unix_ms: 1_779_100_000_001,
        }
    }

    fn eval(global: f32, cells: BTreeMap<String, f32>) -> HoldoutEval {
        HoldoutEval::try_new_with_cells(0.95, global, 0.01, 128, [4u8; 32], cells).unwrap()
    }
}
