use crate::dynamicjepa::artifact::ArtifactFile;
use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::VerificationRunId;
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub const VERIFICATION_RUN_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationRunRecord {
    pub header: DjRecordHeader,
    pub verification_run_id: VerificationRunId,
    pub test_name: String,
    pub db_path_hash: [u8; 32],
    pub fixture_hashes: Vec<[u8; 32]>,
    pub before_counts: BTreeMap<String, u64>,
    pub after_counts: BTreeMap<String, u64>,
    pub commands_executed: Vec<String>,
    pub artifact_hash_checks: Vec<ArtifactHashCheck>,
    pub decoded_record_excerpts: BTreeMap<String, Value>,
    pub expected_results: BTreeMap<String, Value>,
    pub actual_results: BTreeMap<String, Value>,
    pub status: VerificationStatus,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactHashCheck {
    pub relative_path: String,
    pub registry_sha256: [u8; 32],
    pub recomputed_sha256: [u8; 32],
    pub size_bytes: u64,
    pub equal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationStatus {
    Passed,
    Failed { failure_details: String },
}

impl From<ArtifactFile> for ArtifactHashCheck {
    fn from(file: ArtifactFile) -> Self {
        Self {
            relative_path: file.relative_path,
            registry_sha256: file.sha256,
            recomputed_sha256: file.sha256,
            size_bytes: file.size_bytes,
            equal: true,
        }
    }
}

impl Validate for ArtifactHashCheck {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.relative_path.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "ArtifactHashCheck.relative_path",
                "relative_path must not be empty",
                "record the artifact file path that was checked",
            ));
        }
        if self.registry_sha256 == [0; 32] || self.recomputed_sha256 == [0; 32] {
            return Err(DynamicJepaError::validation(
                "ArtifactHashCheck.sha256",
                "hash checks must use non-zero SHA-256 values",
                "recompute hashes from disk before writing verification record",
            ));
        }
        if self.equal && self.registry_sha256 != self.recomputed_sha256 {
            return Err(DynamicJepaError::ArtifactHashMismatch {
                artifact_id: uuid::Uuid::nil(),
                file: self.relative_path.clone(),
                expected: hexish(&self.registry_sha256),
                actual: hexish(&self.recomputed_sha256),
            });
        }
        Ok(())
    }
}

impl Validate for VerificationRunRecord {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.verification_run_id.validate()?;
        if self.test_name.trim().is_empty() {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: self.test_name.clone(),
                message: "test_name must not be empty".to_string(),
                evidence_path: String::new(),
            });
        }
        if self.db_path_hash == [0; 32] {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: self.test_name.clone(),
                message: "db_path_hash must be computed".to_string(),
                evidence_path: String::new(),
            });
        }
        if self.before_counts.is_empty() || self.after_counts.is_empty() {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: self.test_name.clone(),
                message: "before_counts and after_counts must not be empty".to_string(),
                evidence_path: String::new(),
            });
        }
        for check in &self.artifact_hash_checks {
            check.validate()?;
        }
        if matches!(self.status, VerificationStatus::Passed)
            && (self.expected_results.is_empty() || self.actual_results.is_empty())
        {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: self.test_name.clone(),
                message: "passed verification must include expected and actual results".to_string(),
                evidence_path: String::new(),
            });
        }
        if self.created_at_unix_ms < 0 {
            return Err(DynamicJepaError::VerificationFailed {
                test_name: self.test_name.clone(),
                message: "created_at_unix_ms must be non-negative".to_string(),
                evidence_path: String::new(),
            });
        }
        Ok(())
    }
}

fn hexish(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

crate::impl_dynamic_jepa_record!(
    VerificationRunRecord,
    VERIFICATION_RUN_RECORD_VERSION,
    "VerificationRunRecord"
);
