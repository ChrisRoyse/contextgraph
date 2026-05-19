use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::MejepaInferError;
use crate::types::ReasoningClass;
use crate::{
    PredictedAccuracyDegradation, PredictedCostRegression, PredictedPerfRegression,
    RealityPrediction,
};

#[path = "q4_trust_gate_support.rs"]
mod q4_trust_gate_support;

use q4_trust_gate_support::{
    active_slot_set, evaluate_head, loaded_source, manual_source, missing_default_source,
    read_catalog, requirement,
};

pub const Q4_TRUST_GATE_SCHEMA_VERSION: u32 = 1;
pub const Q4_DEFAULT_MIN_PRODUCER_ROWS: u64 = 30;
pub const Q4_DEFAULT_MIN_CALIBRATION_ROWS: u64 = 30;
pub const Q4_DEFAULT_REQUIRED_SLOT_COUNT: usize = 12;
pub const Q4_DEFAULT_EVIDENCE_CATALOG_PATH: &str =
    "/zfs/archive/contextgraph/state/q4-trust-gate/evidence-catalog.json";
pub const Q4_EVIDENCE_CATALOG_ENV: &str = "CG_MEJEPA_Q4_EVIDENCE_CATALOG_PATH";
pub const Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT: &str =
    "/zfs/archive/contextgraph/state/calibration/per-slot-v1/";
pub const Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS: [&str; 12] = [
    "e1", "e2", "e3", "e4", "e6", "e7", "e8", "e9", "e10", "e12", "e13", "e14",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Q4HeadKind {
    Perf,
    Accuracy,
    Cost,
    Reasoning,
    NonTrivialOnPass,
}

impl Q4HeadKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Perf => "perf",
            Self::Accuracy => "accuracy",
            Self::Cost => "cost",
            Self::Reasoning => "reasoning",
            Self::NonTrivialOnPass => "non_trivial_on_pass",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Q4HeadRequirement {
    pub head: Q4HeadKind,
    pub producer_issue: u32,
    pub producer_task: String,
    pub producer_fsv_root: String,
    pub calibration_issue: u32,
    pub calibration_task: String,
    pub calibration_fsv_root: String,
    pub per_slot_evidence_root: String,
    pub min_producer_rows: u64,
    pub min_calibration_rows: u64,
    pub required_slot_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Q4HeadEvidence {
    pub producer_fsv_root: Option<String>,
    pub producer_rows: u64,
    pub calibration_fsv_root: Option<String>,
    pub calibration_rows: u64,
    pub per_slot_evidence_root: Option<String>,
    pub slots_with_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Q4HeadReadiness {
    pub head: Q4HeadKind,
    pub q4_head_ready: bool,
    pub trusted_in_decision: bool,
    pub producer_supported: bool,
    pub calibration_supported: bool,
    pub per_slot_supported: bool,
    pub producer_rows: u64,
    pub calibration_rows: u64,
    pub slots_with_evidence: usize,
    pub producer_fsv_root: String,
    pub calibration_fsv_root: String,
    pub per_slot_evidence_root: String,
    pub required_slots: Vec<String>,
    pub missing_slots: Vec<String>,
    pub unexpected_slots: Vec<String>,
    pub duplicate_slots: Vec<String>,
    pub missing_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Q4TrustGateSourceOfTruth {
    pub catalog_path: String,
    pub catalog_loaded: bool,
    pub catalog_required: bool,
    pub catalog_head_count: usize,
    pub catalog_format: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Q4TrustGateReport {
    pub schema_version: u32,
    pub q4_head_ready: bool,
    pub q4_trust_decision_eligible: bool,
    pub slot_preserving_required: bool,
    pub untrusted_output_policy: String,
    pub source_of_truth: Q4TrustGateSourceOfTruth,
    pub heads: Vec<Q4HeadReadiness>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Q4EvidenceCatalog {
    pub heads: BTreeMap<Q4HeadKind, Q4HeadEvidence>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrustedQ4Consequences {
    pub perf_regressions: Vec<PredictedPerfRegression>,
    pub accuracy_degradations: Vec<PredictedAccuracyDegradation>,
    pub cost_regressions: Vec<PredictedCostRegression>,
    pub reasoning_class: Option<ReasoningClass>,
    pub non_trivial_on_pass_ready: bool,
    pub omitted_heads: Vec<Q4HeadKind>,
    pub q4_trust_decision_eligible: bool,
}

pub fn default_q4_requirements() -> Vec<Q4HeadRequirement> {
    vec![
        requirement(
            Q4HeadKind::Perf,
            266,
            "TASK-PY-G-047",
            "task-py-g-047-q4-perf-label-producer-fsv",
        ),
        requirement(
            Q4HeadKind::Accuracy,
            267,
            "TASK-PY-G-048",
            "task-py-g-048-q4-accuracy-label-producer-fsv",
        ),
        requirement(
            Q4HeadKind::Cost,
            268,
            "TASK-PY-G-049",
            "task-py-g-049-q4-cost-label-producer-fsv",
        ),
        requirement(
            Q4HeadKind::Reasoning,
            269,
            "TASK-PY-G-050",
            "task-py-g-050-q4-reasoning-label-producer-fsv",
        ),
        requirement(
            Q4HeadKind::NonTrivialOnPass,
            270,
            "TASK-PY-G-051",
            "task-py-g-051-q4-non-trivial-on-pass-fsv",
        ),
    ]
}

pub fn evaluate_q4_trust_gate(
    catalog: &Q4EvidenceCatalog,
) -> Result<Q4TrustGateReport, MejepaInferError> {
    evaluate_q4_trust_gate_with_source(catalog, manual_source(catalog.heads.len()))
}

pub fn q4_trust_gate_report_from_catalog_path(
    path: impl AsRef<Path>,
) -> Result<Q4TrustGateReport, MejepaInferError> {
    let path = path.as_ref();
    let catalog = read_catalog(path)?;
    evaluate_q4_trust_gate_with_source(
        &catalog,
        loaded_source(path.to_path_buf(), catalog.heads.len(), true),
    )
}

pub fn default_q4_trust_gate_report() -> Result<Q4TrustGateReport, MejepaInferError> {
    let (catalog, source) = load_default_q4_evidence_catalog()?;
    evaluate_q4_trust_gate_with_source(&catalog, source)
}

pub fn load_default_q4_evidence_catalog(
) -> Result<(Q4EvidenceCatalog, Q4TrustGateSourceOfTruth), MejepaInferError> {
    let override_path = std::env::var(Q4_EVIDENCE_CATALOG_ENV).ok();
    let catalog_required = override_path.is_some();
    let path = override_path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(Q4_DEFAULT_EVIDENCE_CATALOG_PATH));

    if path.exists() {
        let catalog = read_catalog(&path)?;
        let source = loaded_source(path, catalog.heads.len(), catalog_required);
        Ok((catalog, source))
    } else if catalog_required {
        Err(MejepaInferError::io(
            "read_q4_evidence_catalog",
            &path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "Q4 evidence catalog missing"),
        ))
    } else {
        Ok((Q4EvidenceCatalog::default(), missing_default_source(path)))
    }
}

fn evaluate_q4_trust_gate_with_source(
    catalog: &Q4EvidenceCatalog,
    source_of_truth: Q4TrustGateSourceOfTruth,
) -> Result<Q4TrustGateReport, MejepaInferError> {
    let mut heads = Vec::new();
    for requirement in default_q4_requirements() {
        let evidence = catalog
            .heads
            .get(&requirement.head)
            .cloned()
            .unwrap_or_default();
        heads.push(evaluate_head(&requirement, &evidence)?);
    }
    let q4_head_ready = heads.iter().all(|head| head.q4_head_ready);
    let report = Q4TrustGateReport {
        schema_version: Q4_TRUST_GATE_SCHEMA_VERSION,
        q4_head_ready,
        q4_trust_decision_eligible: q4_head_ready,
        slot_preserving_required: true,
        untrusted_output_policy:
            "raw Q4 fields may be displayed as untrusted observations, but only ready heads may influence trust decisions"
                .to_string(),
        source_of_truth,
        heads,
    };
    report.validate()?;
    Ok(report)
}

pub fn trusted_q4_consequences(
    prediction: &RealityPrediction,
    report: &Q4TrustGateReport,
) -> TrustedQ4Consequences {
    let ready = report
        .heads
        .iter()
        .filter(|head| head.trusted_in_decision)
        .map(|head| head.head)
        .collect::<BTreeSet<_>>();
    let omitted_heads = report
        .heads
        .iter()
        .filter(|head| !head.trusted_in_decision)
        .map(|head| head.head)
        .collect();

    TrustedQ4Consequences {
        perf_regressions: if ready.contains(&Q4HeadKind::Perf) {
            prediction.predicted_perf_regressions.clone()
        } else {
            Vec::new()
        },
        accuracy_degradations: if ready.contains(&Q4HeadKind::Accuracy) {
            prediction.predicted_accuracy_degradations.clone()
        } else {
            Vec::new()
        },
        cost_regressions: if ready.contains(&Q4HeadKind::Cost) {
            prediction.predicted_cost_regressions.clone()
        } else {
            Vec::new()
        },
        reasoning_class: if ready.contains(&Q4HeadKind::Reasoning) {
            Some(prediction.predicted_reasoning_class)
        } else {
            None
        },
        non_trivial_on_pass_ready: ready.contains(&Q4HeadKind::NonTrivialOnPass),
        omitted_heads,
        q4_trust_decision_eligible: report.q4_trust_decision_eligible,
    }
}

impl Q4TrustGateReport {
    pub fn head(&self, head: Q4HeadKind) -> Option<&Q4HeadReadiness> {
        self.heads.iter().find(|item| item.head == head)
    }

    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.schema_version != Q4_TRUST_GATE_SCHEMA_VERSION {
            return Err(invalid(
                "q4_trust_gate.schema_version",
                format!(
                    "expected {}, got {}",
                    Q4_TRUST_GATE_SCHEMA_VERSION, self.schema_version
                ),
            ));
        }
        if self.untrusted_output_policy.trim().is_empty() {
            return Err(invalid(
                "q4_trust_gate.untrusted_output_policy",
                "policy must be non-empty",
            ));
        }
        self.source_of_truth.validate()?;
        if self.heads.len() != default_q4_requirements().len() {
            return Err(invalid(
                "q4_trust_gate.heads",
                "report must contain every required Q4 head",
            ));
        }
        let mut seen = BTreeSet::new();
        for head in &self.heads {
            head.validate()?;
            if !seen.insert(head.head) {
                return Err(invalid(
                    "q4_trust_gate.heads",
                    format!("duplicate head {}", head.head.as_str()),
                ));
            }
        }
        let aggregate_ready = self.heads.iter().all(|head| head.q4_head_ready);
        if self.q4_head_ready != aggregate_ready {
            return Err(invalid(
                "q4_trust_gate.q4_head_ready",
                "aggregate readiness must equal conjunction of head readiness",
            ));
        }
        if self.q4_trust_decision_eligible != self.q4_head_ready {
            return Err(invalid(
                "q4_trust_gate.q4_trust_decision_eligible",
                "trust decision eligibility must fail closed with aggregate readiness",
            ));
        }
        Ok(())
    }
}

impl Q4TrustGateSourceOfTruth {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.catalog_path.trim().is_empty() {
            return Err(invalid(
                "q4_trust_gate.source_of_truth.catalog_path",
                "catalog path must be non-empty",
            ));
        }
        if self.catalog_format != "q4-evidence-catalog-v1" {
            return Err(invalid(
                "q4_trust_gate.source_of_truth.catalog_format",
                "catalog format must be q4-evidence-catalog-v1",
            ));
        }
        Ok(())
    }
}

impl Q4HeadReadiness {
    pub fn validate(&self) -> Result<(), MejepaInferError> {
        if self.q4_head_ready != self.trusted_in_decision {
            return Err(invalid(
                "q4_head_readiness.trusted_in_decision",
                "trusted_in_decision must equal q4_head_ready",
            ));
        }
        let ready =
            self.producer_supported && self.calibration_supported && self.per_slot_supported;
        if self.q4_head_ready != ready {
            return Err(invalid(
                "q4_head_readiness.q4_head_ready",
                "head readiness must require producer, calibration, and per-slot support",
            ));
        }
        if self.q4_head_ready && !self.missing_requirements.is_empty() {
            return Err(invalid(
                "q4_head_readiness.missing_requirements",
                "ready heads must not carry missing requirements",
            ));
        }
        if self.per_slot_supported {
            let expected = active_slot_set();
            let observed = self
                .required_slots
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>();
            if observed != expected {
                return Err(invalid(
                    "q4_head_readiness.required_slots",
                    "required slot list must equal the active embedder slot set",
                ));
            }
            if !self.missing_slots.is_empty()
                || !self.unexpected_slots.is_empty()
                || !self.duplicate_slots.is_empty()
                || self.slots_with_evidence != expected.len()
            {
                return Err(invalid(
                    "q4_head_readiness.per_slot_supported",
                    "per-slot support requires exact active embedder slot evidence",
                ));
            }
        }
        Ok(())
    }
}

fn invalid(field: &str, detail: impl Into<String>) -> MejepaInferError {
    MejepaInferError::InvalidInput {
        field: field.to_string(),
        detail: detail.into(),
    }
}

#[cfg(test)]
#[path = "q4_trust_gate_tests.rs"]
mod q4_trust_gate_tests;
