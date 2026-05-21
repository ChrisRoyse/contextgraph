use std::collections::BTreeMap;
use std::error::Error;

use serde::{Deserialize, Serialize};

pub type DockerOutcomeResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub const DOCKER_OUTCOME_LABEL_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerOutcomeLabelRow {
    pub schema_version: u32,
    pub task_instance_id: String,
    pub mutation_category: String,
    pub key: String,
    pub oracle_pass: bool,
    pub oracle_fail_or_error: bool,
    pub harness_exception_or_error: bool,
    pub oracle_per_test_count: u64,
    pub oracle_verdict_sha256: String,
    pub docker_resolved_clean: bool,
    pub target_fail_to_pass_tests_failed: bool,
    pub regression_pass_to_pass_tests_failed: bool,
    pub target_and_regression_tests_failed: bool,
    pub patch_apply_failure: bool,
    pub docker_report_missing: bool,
    pub test_phase_vector: TestPhaseVectorLabels,
    pub hashed_test_identifiers_by_bucket: BTreeMap<String, Vec<String>>,
    pub index_patch_path: Option<String>,
    pub index_patch_sha256: Option<String>,
    pub selected_report_path: Option<String>,
    pub selected_report_sha256: Option<String>,
    pub prediction_jsonl_path: Option<String>,
    pub prediction_patch_sha256: Option<String>,
    pub resolved_index_mismatch: bool,
    pub mismatch_exception: Option<String>,
}

impl DockerOutcomeLabelRow {
    pub fn storage_key(task_instance_id: &str, mutation_category: &str) -> String {
        format!("{task_instance_id}::{mutation_category}")
    }

    pub fn validate(&self) -> DockerOutcomeResult<()> {
        require(
            self.schema_version == DOCKER_OUTCOME_LABEL_SCHEMA_VERSION,
            "docker outcome schema version mismatch",
        )?;
        validate_id("task_instance_id", &self.task_instance_id)?;
        validate_id("mutation_category", &self.mutation_category)?;
        require(
            self.key == Self::storage_key(&self.task_instance_id, &self.mutation_category),
            "docker outcome row key mismatch",
        )?;
        require(
            self.oracle_pass != self.oracle_fail_or_error,
            "oracle_pass and oracle_fail_or_error must be complements",
        )?;
        if self.docker_report_missing {
            require(
                self.selected_report_path.is_none() && self.selected_report_sha256.is_none(),
                "missing report row must not carry selected report evidence",
            )?;
        }
        self.test_phase_vector.validate()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestPhaseVectorLabels {
    pub fail_to_pass_success_count: u64,
    pub fail_to_pass_failure_count: u64,
    pub pass_to_pass_success_count: u64,
    pub pass_to_pass_failure_count: u64,
    pub fail_to_fail_success_count: u64,
    pub fail_to_fail_failure_count: u64,
    pub pass_to_fail_success_count: u64,
    pub pass_to_fail_failure_count: u64,
}

impl TestPhaseVectorLabels {
    pub fn total(self) -> u64 {
        self.fail_to_pass_success_count
            + self.fail_to_pass_failure_count
            + self.pass_to_pass_success_count
            + self.pass_to_pass_failure_count
            + self.fail_to_fail_success_count
            + self.fail_to_fail_failure_count
            + self.pass_to_fail_success_count
            + self.pass_to_fail_failure_count
    }

    pub fn validate(&self) -> DockerOutcomeResult<()> {
        require(
            self.total() < 1_000_000,
            "test-phase vector count is implausibly large",
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DockerOutcomeMaterializationReport {
    pub schema_version: u32,
    pub corpus_root: String,
    pub db_path: String,
    pub rows_persisted: u64,
    pub unique_keys: u64,
    pub expected_index_entries: u64,
    pub category_counts: BTreeMap<String, u64>,
    pub expected_category_counts: BTreeMap<String, u64>,
    pub docker_label_counts: BTreeMap<String, u64>,
    pub selected_report_count: u64,
    pub docker_report_missing_count: u64,
    pub resolved_index_mismatch_count: u64,
    pub oracle_per_test_count_mismatch_count: u64,
    pub prediction_patch_rows_attached: u64,
}

impl DockerOutcomeMaterializationReport {
    pub fn all_acceptance_passed(&self) -> bool {
        self.rows_persisted == self.expected_index_entries
            && self.unique_keys == self.expected_index_entries
            && self.category_counts == self.expected_category_counts
            && self.docker_report_missing_count == 0
            && self.resolved_index_mismatch_count == 0
            && self.oracle_per_test_count_mismatch_count == 0
    }
}

pub(crate) fn validate_id(field: &str, value: &str) -> DockerOutcomeResult<()> {
    require(
        !value.trim().is_empty() && !value.contains('\n') && !value.contains('\r'),
        format!("{field} must be a non-empty single-line string"),
    )
}

pub(crate) fn require(condition: bool, detail: impl Into<String>) -> DockerOutcomeResult<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(detail))
    }
}

pub(crate) fn invalid(detail: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::new(std::io::ErrorKind::InvalidData, detail.into()).into()
}
