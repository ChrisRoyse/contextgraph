use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{DatasetId, DomainPackId, ModelArtifactId, TrainingRunId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const MODEL_ARTIFACT_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactRecord {
    pub header: DjRecordHeader,
    pub artifact_id: ModelArtifactId,
    pub training_run_id: TrainingRunId,
    pub domain_pack_id: DomainPackId,
    pub domain_pack_version: String,
    pub dataset_id: DatasetId,
    pub artifact_root: PathBuf,
    pub files: Vec<ArtifactFile>,
    pub model_config_hash: [u8; 32],
    pub evaluation_report_hash: [u8; 32],
    pub created_at_unix_ms: i64,
    pub status: ArtifactStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactFile {
    pub relative_path: String,
    pub sha256: [u8; 32],
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactStatus {
    Active,
    Quarantined { reason: String },
}

impl Validate for ArtifactFile {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.relative_path.trim().is_empty()
            || self.relative_path.starts_with('/')
            || self.relative_path.contains("..")
        {
            return Err(DynamicJepaError::validation(
                "ArtifactFile.relative_path",
                format!("invalid relative_path {:?}", self.relative_path),
                "store sorted relative paths under artifact_root only",
            ));
        }
        if self.sha256 == [0; 32] || self.size_bytes == 0 {
            return Err(DynamicJepaError::validation(
                "ArtifactFile",
                "sha256 and size_bytes must describe a real artifact file",
                "compute streaming SHA-256 after writing the file",
            ));
        }
        Ok(())
    }
}

impl Validate for ModelArtifactRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.artifact_id.validate()?;
        self.training_run_id.validate()?;
        self.domain_pack_id.validate()?;
        self.dataset_id.validate()?;
        if self.domain_pack_version.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "ModelArtifactRecord.domain_pack_version",
                "domain_pack_version must not be empty",
                "copy the registered domain pack version into the artifact row",
            ));
        }
        if self.artifact_root.as_os_str().is_empty() {
            return Err(DynamicJepaError::validation(
                "ModelArtifactRecord.artifact_root",
                "artifact_root must not be empty",
                "canonicalize the artifact root before registering it",
            ));
        }
        if self.files.is_empty() {
            return Err(DynamicJepaError::ArtifactHashMismatch {
                artifact_id: self.artifact_id.0,
                file: "manifest".to_string(),
                expected: "at least one file".to_string(),
                actual: "none".to_string(),
            });
        }
        for file in &self.files {
            file.validate()?;
        }
        if self.model_config_hash == [0; 32] || self.evaluation_report_hash == [0; 32] {
            return Err(DynamicJepaError::validation(
                "ModelArtifactRecord.hashes",
                "model_config_hash and evaluation_report_hash must be computed",
                "hash canonical config/evaluation files before registry write",
            ));
        }
        if self.created_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "ModelArtifactRecord.created_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        if let ArtifactStatus::Quarantined { reason } = &self.status {
            if reason.trim().is_empty() {
                return Err(DynamicJepaError::validation(
                    "ModelArtifactRecord.status",
                    "quarantine reason must not be empty",
                    "record an operator-visible quarantine reason",
                ));
            }
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    ModelArtifactRecord,
    MODEL_ARTIFACT_RECORD_VERSION,
    "ModelArtifactRecord"
);
