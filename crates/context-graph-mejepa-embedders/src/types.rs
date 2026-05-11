use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderInput {
    pub embedder: EmbedderId,
    pub text: String,
    pub source_id: String,
}

impl EmbedderInput {
    pub fn validate(&self) -> EmbedResult<()> {
        if self.text.trim().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderInput.text",
                "input text is empty",
                "pass the AST chunk, problem text, trace text, or learner observation that should be embedded",
            ));
        }
        if self.source_id.trim().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderInput.source_id",
                "source_id is empty",
                "record the corpus row, AST chunk hash, or learner observation id",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmbedderOutput {
    pub embedder: EmbedderId,
    pub source_id: String,
    pub vector: Vec<f32>,
    pub model_version: String,
    pub precision_class: String,
}

impl EmbedderOutput {
    pub fn validate(&self) -> EmbedResult<()> {
        let expected = self.embedder.dimension();
        if self.vector.len() != expected {
            return Err(EmbedError::invalid(
                "EmbedderOutput.vector",
                format!(
                    "{} output dimension mismatch: got {}, expected {}",
                    self.embedder,
                    self.vector.len(),
                    expected
                ),
                "fix the wrapper projection before writing this vector",
            ));
        }
        if !self.vector.iter().all(|value| value.is_finite()) {
            return Err(EmbedError::invalid(
                "EmbedderOutput.vector",
                "vector contains NaN or Inf",
                "reject the model output and inspect tokenizer/model precision",
            ));
        }
        if self.model_version.trim().is_empty() {
            return Err(EmbedError::invalid(
                "EmbedderOutput.model_version",
                "model_version is empty",
                "record the SHA-pinned model registration version",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutingResult {
    pub language: String,
    pub entity_type: String,
    pub embedders: BTreeSet<EmbedderId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoraAdapterRegistration {
    pub embedder: EmbedderId,
    pub adapter_path: String,
    pub adapter_sha256: String,
    pub rank: u16,
    pub alpha: u16,
}

impl LoraAdapterRegistration {
    pub fn validate(&self) -> EmbedResult<()> {
        if self.adapter_path.trim().is_empty() {
            return Err(EmbedError::invalid(
                "LoraAdapterRegistration.adapter_path",
                "adapter_path is empty",
                "persist LoRA adapters to an auditable safetensors file",
            ));
        }
        if self.adapter_sha256.len() != 64
            || !self
                .adapter_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(EmbedError::invalid(
                "LoraAdapterRegistration.adapter_sha256",
                "adapter_sha256 must be 64 lowercase hex characters",
                "hash the adapter file with SHA-256 and record lowercase hex",
            ));
        }
        if self.rank == 0 || self.alpha == 0 {
            return Err(EmbedError::invalid(
                "LoraAdapterRegistration.rank_alpha",
                "rank and alpha must be non-zero",
                "choose an explicit LoRA rank/alpha from the training config",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VramBudgetReport {
    pub required_bytes: u64,
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub gpu_name: String,
    pub compute_capability: String,
    pub telemetry_source: String,
    pub nvidia_smi_status: String,
    pub nvidia_smi_total_mb: Option<u64>,
    pub passes: bool,
}
