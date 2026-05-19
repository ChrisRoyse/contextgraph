use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::error::MejepaInferError;

use super::{
    Q4EvidenceCatalog, Q4HeadEvidence, Q4HeadKind, Q4HeadReadiness, Q4HeadRequirement,
    Q4TrustGateSourceOfTruth, Q4_DEFAULT_MIN_CALIBRATION_ROWS, Q4_DEFAULT_MIN_PRODUCER_ROWS,
    Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT, Q4_DEFAULT_REQUIRED_SLOT_COUNT,
    Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS,
};

pub(super) fn requirement(
    head: Q4HeadKind,
    producer_issue: u32,
    producer_task: &str,
    producer_fsv_slug: &str,
) -> Q4HeadRequirement {
    Q4HeadRequirement {
        head,
        producer_issue,
        producer_task: producer_task.to_string(),
        producer_fsv_root: format!("/zfs/archive/contextgraph/fsv/{producer_fsv_slug}/"),
        calibration_issue: 127,
        calibration_task: "TASK-PY-G-012".to_string(),
        calibration_fsv_root:
            "/zfs/archive/contextgraph/fsv/task-py-g-012-q4-confidence-calibration-fsv/".to_string(),
        per_slot_evidence_root: Q4_DEFAULT_PER_SLOT_EVIDENCE_ROOT.to_string(),
        min_producer_rows: Q4_DEFAULT_MIN_PRODUCER_ROWS,
        min_calibration_rows: Q4_DEFAULT_MIN_CALIBRATION_ROWS,
        required_slot_count: Q4_DEFAULT_REQUIRED_SLOT_COUNT,
    }
}

pub(super) fn evaluate_head(
    requirement: &Q4HeadRequirement,
    evidence: &Q4HeadEvidence,
) -> Result<Q4HeadReadiness, MejepaInferError> {
    let producer_root = evidence.producer_fsv_root.clone().unwrap_or_default();
    let calibration_root = evidence.calibration_fsv_root.clone().unwrap_or_default();
    let slot_root = evidence.per_slot_evidence_root.clone().unwrap_or_default();
    let slot_audit = audit_slots(&evidence.slots_with_evidence);

    let producer_supported = !producer_root.trim().is_empty()
        && producer_root == requirement.producer_fsv_root
        && evidence.producer_rows >= requirement.min_producer_rows;
    let calibration_supported = !calibration_root.trim().is_empty()
        && calibration_root == requirement.calibration_fsv_root
        && evidence.calibration_rows >= requirement.min_calibration_rows;
    let per_slot_supported = slot_root == requirement.per_slot_evidence_root
        && slot_audit.missing_slots.is_empty()
        && slot_audit.unexpected_slots.is_empty()
        && slot_audit.duplicate_slots.is_empty()
        && slot_audit.unique_slot_count == requirement.required_slot_count;

    let mut missing_requirements = Vec::new();
    if !producer_supported {
        missing_requirements.push(format!(
            "producer support missing for #{} {} at {} (rows {} < {})",
            requirement.producer_issue,
            requirement.producer_task,
            requirement.producer_fsv_root,
            evidence.producer_rows,
            requirement.min_producer_rows
        ));
    }
    if !calibration_supported {
        missing_requirements.push(format!(
            "calibration support missing for #{} {} at {} (rows {} < {})",
            requirement.calibration_issue,
            requirement.calibration_task,
            requirement.calibration_fsv_root,
            evidence.calibration_rows,
            requirement.min_calibration_rows
        ));
    }
    if !per_slot_supported {
        missing_requirements.push(format!(
            "slot-preserving evidence missing at {}: observed {} exact active slots, missing={:?}, unexpected={:?}, duplicate={:?}",
            requirement.per_slot_evidence_root,
            slot_audit.unique_slot_count,
            slot_audit.missing_slots,
            slot_audit.unexpected_slots,
            slot_audit.duplicate_slots
        ));
    }

    let q4_head_ready = missing_requirements.is_empty();
    let readiness = Q4HeadReadiness {
        head: requirement.head,
        q4_head_ready,
        trusted_in_decision: q4_head_ready,
        producer_supported,
        calibration_supported,
        per_slot_supported,
        producer_rows: evidence.producer_rows,
        calibration_rows: evidence.calibration_rows,
        slots_with_evidence: slot_audit.unique_slot_count,
        producer_fsv_root: producer_root,
        calibration_fsv_root: calibration_root,
        per_slot_evidence_root: slot_root,
        required_slots: active_slots(),
        missing_slots: slot_audit.missing_slots,
        unexpected_slots: slot_audit.unexpected_slots,
        duplicate_slots: slot_audit.duplicate_slots,
        missing_requirements,
    };
    readiness.validate()?;
    Ok(readiness)
}

#[derive(Debug)]
struct SlotAudit {
    unique_slot_count: usize,
    missing_slots: Vec<String>,
    unexpected_slots: Vec<String>,
    duplicate_slots: Vec<String>,
}

fn audit_slots(slots: &[String]) -> SlotAudit {
    let expected = active_slot_set();
    let mut seen = BTreeSet::new();
    let mut duplicate_slots = BTreeSet::new();
    let mut unexpected_slots = BTreeSet::new();

    for slot in slots {
        let trimmed = slot.trim();
        if trimmed.is_empty() {
            unexpected_slots.insert(slot.clone());
            continue;
        }
        if !seen.insert(trimmed.to_string()) {
            duplicate_slots.insert(trimmed.to_string());
        }
        if !expected.contains(trimmed) {
            unexpected_slots.insert(trimmed.to_string());
        }
    }

    let missing_slots = expected
        .iter()
        .filter(|slot| !seen.contains(**slot))
        .map(|slot| (*slot).to_string())
        .collect();

    SlotAudit {
        unique_slot_count: seen.len(),
        missing_slots,
        unexpected_slots: unexpected_slots.into_iter().collect(),
        duplicate_slots: duplicate_slots.into_iter().collect(),
    }
}

pub(super) fn active_slots() -> Vec<String> {
    Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS
        .into_iter()
        .map(str::to_string)
        .collect()
}

pub(super) fn active_slot_set() -> BTreeSet<&'static str> {
    Q4_REQUIRED_ACTIVE_EMBEDDER_SLOTS.into_iter().collect()
}

pub(super) fn manual_source(catalog_head_count: usize) -> Q4TrustGateSourceOfTruth {
    Q4TrustGateSourceOfTruth {
        catalog_path: "in-memory".to_string(),
        catalog_loaded: true,
        catalog_required: false,
        catalog_head_count,
        catalog_format: "q4-evidence-catalog-v1".to_string(),
    }
}

pub(super) fn missing_default_source(path: PathBuf) -> Q4TrustGateSourceOfTruth {
    Q4TrustGateSourceOfTruth {
        catalog_path: path.display().to_string(),
        catalog_loaded: false,
        catalog_required: false,
        catalog_head_count: 0,
        catalog_format: "q4-evidence-catalog-v1".to_string(),
    }
}

pub(super) fn loaded_source(
    path: PathBuf,
    catalog_head_count: usize,
    catalog_required: bool,
) -> Q4TrustGateSourceOfTruth {
    Q4TrustGateSourceOfTruth {
        catalog_path: path.display().to_string(),
        catalog_loaded: true,
        catalog_required,
        catalog_head_count,
        catalog_format: "q4-evidence-catalog-v1".to_string(),
    }
}

pub(super) fn read_catalog(path: &Path) -> Result<Q4EvidenceCatalog, MejepaInferError> {
    let bytes = std::fs::read(path)
        .map_err(|source| MejepaInferError::io("read_q4_evidence_catalog", path, source))?;
    serde_json::from_slice(&bytes).map_err(MejepaInferError::from)
}
