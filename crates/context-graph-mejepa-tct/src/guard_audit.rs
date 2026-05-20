use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Digest;

use crate::error::TctError;

pub const GTAU_GUARD_AUDIT_SCHEMA_VERSION: u16 = 1;
pub const GTAU_GUARD_AUDIT_FORMULA_VERSION: &str = "python_gtau_guard_audit_per_slot_v1";
pub const GTAU_GUARD_AUDIT_DEFAULT_MIN_PASS_RATE: f64 = 0.95;
pub const GTAU_GUARD_AUDIT_DEFAULT_MAX_SLOT_ROWS: usize = 1_000_000;

const EXPECTED_PYTHON_CELLS: [&str; 8] = [
    "python:known_good",
    "python:subtle_flip",
    "python:off_by_one",
    "python:swap_variable",
    "python:delete_test_call",
    "python:wrong_file",
    "python:over_engineer",
    "python:compile_error",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtauGuardAuditPaths {
    pub tensor_root: PathBuf,
    pub calibration_root: PathBuf,
}

impl GtauGuardAuditPaths {
    pub fn new(tensor_root: PathBuf, calibration_root: PathBuf) -> Self {
        Self {
            tensor_root,
            calibration_root,
        }
    }

    fn slot_distribution_path(&self) -> PathBuf {
        self.tensor_root.join("slot_distribution_tensors.jsonl")
    }

    fn pairwise_tensor_path(&self) -> PathBuf {
        self.tensor_root.join("pairwise_correlation_tensors.jsonl")
    }

    fn tensor_manifest_path(&self) -> PathBuf {
        self.tensor_root.join("constellation_tensor_manifest.json")
    }

    fn tensor_report_path(&self) -> PathBuf {
        self.tensor_root.join("constellation_tensor_report.json")
    }

    fn per_slot_calibration_path(&self) -> PathBuf {
        self.calibration_root.join("per_slot_calibration.jsonl")
    }

    fn per_cell_slot_calibration_path(&self) -> PathBuf {
        self.calibration_root
            .join("per_cell_slot_calibration.jsonl")
    }

    fn per_pair_calibration_path(&self) -> PathBuf {
        self.calibration_root.join("per_pair_calibration.jsonl")
    }

    fn ood_guard_policy_path(&self) -> PathBuf {
        self.calibration_root.join("ood_guard_policy.json")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtauGuardAuditConfig {
    pub min_guard_pass_rate: f64,
    pub max_slot_tensor_rows: usize,
    pub now_utc: DateTime<Utc>,
}

impl Default for GtauGuardAuditConfig {
    fn default() -> Self {
        Self {
            min_guard_pass_rate: GTAU_GUARD_AUDIT_DEFAULT_MIN_PASS_RATE,
            max_slot_tensor_rows: GTAU_GUARD_AUDIT_DEFAULT_MAX_SLOT_ROWS,
            now_utc: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GtauGuardAuditReport {
    pub artifact_kind: String,
    pub schema_version: u16,
    pub formula_version: String,
    pub task_id: String,
    pub created_at_utc: String,
    pub source_of_truth: GtauGuardAuditSourceOfTruth,
    pub policy: GtauGuardAuditPolicySummary,
    pub summary: GtauGuardAuditSummary,
    pub cell_reports: Vec<GtauCellAuditReport>,
    pub missing_reward_signal_neighborhoods: Vec<RewardSignalNeighborhoodGap>,
    pub passes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GtauGuardAuditSourceOfTruth {
    pub tensor_root: PathBuf,
    pub calibration_root: PathBuf,
    pub slot_distribution_tensors: PathBuf,
    pub pairwise_correlation_tensors: PathBuf,
    pub per_slot_calibration: PathBuf,
    pub per_cell_slot_calibration: PathBuf,
    pub per_pair_calibration: PathBuf,
    pub ood_guard_policy: PathBuf,
    pub tensor_manifest_sha256: Option<String>,
    pub tensor_report_sha256: Option<String>,
    pub policy_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GtauGuardAuditPolicySummary {
    pub required_slots: Vec<String>,
    pub strict_conjunction: bool,
    pub aggregate_score_never_overrides_slot_violation: bool,
    pub reject_missing_or_stale_calibration: bool,
    pub global_slot_calibration_ready: bool,
    pub ship_gate_countable_before_audit: bool,
    pub one_bad_slot_rejects_even_when_aggregate_passes: bool,
    pub missing_required_slot_rejects: bool,
    pub stale_calibration_rejects: bool,
    pub nonfinite_slot_score_rejects: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GtauGuardAuditSummary {
    pub expected_cell_count: usize,
    pub reported_cell_count: usize,
    pub missing_cell_count: usize,
    pub low_pass_cell_count: usize,
    pub fail_closed_cell_count: usize,
    pub total_slot_guard_pass_count: usize,
    pub total_slot_guard_fail_count: usize,
    pub total_slot_rows_seen: usize,
    pub total_valid_slot_rows: usize,
    pub global_calibrated_slots: usize,
    pub required_slot_count: usize,
    pub all_expected_cells_reported: bool,
    pub slot_identity_preserved: bool,
    pub flat_vector_concat_used: bool,
    pub strict_policy_ready: bool,
    pub guard_green: bool,
    pub ship_gate_countable_after_audit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GtauCellAuditReport {
    pub cell_key: String,
    pub language: String,
    pub mutation_category: String,
    pub slot_rows_seen: usize,
    pub valid_slot_rows: usize,
    pub slot_guard_pass_count: usize,
    pub slot_guard_fail_count: usize,
    pub guard_pass_rate: Option<f64>,
    pub required_slots_present: Vec<String>,
    pub missing_required_slots: Vec<String>,
    pub per_slot: BTreeMap<String, GtauSlotAuditReport>,
    pub pair_surface_counts: BTreeMap<String, usize>,
    pub unsupported_pair_rows: usize,
    pub per_cell_calibrated_slots: usize,
    pub per_cell_invalid_slots: usize,
    pub calibration_fail_closed: bool,
    pub low_pass: bool,
    pub status: GtauCellAuditStatus,
    pub remediation: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GtauCellAuditStatus {
    Ready,
    LowPassRate,
    MissingSubstrate,
    FailClosedCalibration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GtauSlotAuditReport {
    pub rows_seen: usize,
    pub valid_rows: usize,
    pub pass_count: usize,
    pub fail_count: usize,
    pub invalid_count: usize,
    pub distance_mean_tau: Option<f64>,
    pub distance_max_tau: Option<f64>,
    pub variance_mean_tau: Option<f64>,
    pub ood_tau: Option<f64>,
    pub max_distance_mean: Option<f64>,
    pub max_distance_max: Option<f64>,
    pub max_variance_mean: Option<f64>,
    pub fail_reasons: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RewardSignalNeighborhoodGap {
    pub cell_key: String,
    pub slot_id: Option<String>,
    pub pair: Option<String>,
    pub reason: String,
    pub remediation: String,
}

#[derive(Debug, Clone)]
struct SlotCalibration {
    valid: bool,
    stale_after_utc: Option<DateTime<Utc>>,
    distance_mean_tau: Option<f64>,
    distance_max_tau: Option<f64>,
    variance_mean_tau: Option<f64>,
    ood_tau: Option<f64>,
}

#[derive(Debug, Clone, Default)]
struct CellAccumulator {
    language: String,
    mutation_category: String,
    per_slot: BTreeMap<String, GtauSlotAuditReport>,
    pair_surface_counts: BTreeMap<String, usize>,
    unsupported_pair_rows: usize,
    per_cell_calibrated_slots: usize,
    per_cell_invalid_slots: usize,
    per_cell_invalid_reasons: BTreeMap<String, usize>,
}

pub fn expected_python_gtau_cells() -> Vec<String> {
    EXPECTED_PYTHON_CELLS
        .iter()
        .map(|cell| (*cell).to_string())
        .collect()
}

pub fn run_gtau_guard_audit(
    paths: &GtauGuardAuditPaths,
    config: &GtauGuardAuditConfig,
) -> Result<GtauGuardAuditReport, TctError> {
    if !config.min_guard_pass_rate.is_finite() || !(0.0..=1.0).contains(&config.min_guard_pass_rate)
    {
        return Err(TctError::invalid(
            "GtauGuardAuditConfig.min_guard_pass_rate",
            format!(
                "expected finite probability in [0,1], got {}",
                config.min_guard_pass_rate
            ),
        ));
    }
    if config.max_slot_tensor_rows == 0 {
        return Err(TctError::invalid(
            "GtauGuardAuditConfig.max_slot_tensor_rows",
            "must be at least 1",
        ));
    }

    let policy_value = read_json_file(&paths.ood_guard_policy_path())?;
    let policy = parse_policy(&policy_value)?;
    if policy.required_slots.is_empty() {
        return Err(TctError::invalid(
            "ood_guard_policy.required_slots",
            "required_slots must not be empty",
        ));
    }

    let global_calibrations = read_global_calibrations(paths, config)?;
    let mut cells = BTreeMap::<String, CellAccumulator>::new();
    let mut slot_rows_seen = 0usize;
    let mut slot_identity_preserved = true;
    let mut flat_vector_concat_used = false;

    for row in read_jsonl_file(&paths.slot_distribution_path())? {
        slot_rows_seen += 1;
        if slot_rows_seen > config.max_slot_tensor_rows {
            return Err(TctError::invalid(
                "slot_distribution_tensors",
                format!(
                    "row count exceeded max_slot_tensor_rows={}",
                    config.max_slot_tensor_rows
                ),
            ));
        }
        let language = required_str(&row, "language")?;
        if language != "python" {
            return Err(TctError::invalid(
                "slot_distribution_tensors.language",
                format!("TASK-PY-G-015 audits Python only; got {language:?}"),
            ));
        }
        let mutation_category = required_str(&row, "mutation_category")?;
        let cell_key = required_str(&row, "cell_key")?;
        let slot_id = required_str(&row, "slot_id")?;
        let embedder_id = required_str(&row, "embedder_id")?;
        if slot_id != embedder_id {
            return Err(TctError::invalid(
                "slot_distribution_tensors.slot_id",
                format!("slot_id {slot_id:?} did not match embedder_id {embedder_id:?}"),
            ));
        }
        if cell_key != format!("{language}:{mutation_category}") {
            return Err(TctError::invalid(
                "slot_distribution_tensors.cell_key",
                format!(
                    "cell_key {cell_key:?} did not match language/mutation {language}:{mutation_category}"
                ),
            ));
        }
        slot_identity_preserved &= required_bool(&row, "slot_identity_preserved")?;
        flat_vector_concat_used |= required_bool(&row, "flat_vector_concat_used")?;

        let acc = cells
            .entry(cell_key.to_string())
            .or_insert_with(|| CellAccumulator {
                language: language.to_string(),
                mutation_category: mutation_category.to_string(),
                ..CellAccumulator::default()
            });
        let slot_report = acc.per_slot.entry(slot_id.to_string()).or_default();
        slot_report.rows_seen += 1;
        let valid = required_bool(&row, "valid")?;
        if !valid {
            slot_report.invalid_count += 1;
            bump(
                &mut slot_report.fail_reasons,
                optional_str(&row, "invalid_reason")
                    .unwrap_or("INVALID_SLOT_TENSOR_ROW")
                    .to_string(),
            );
            continue;
        }
        slot_report.valid_rows += 1;
        let distance_mean = required_f64(&row, "distance_mean")?;
        let distance_max = required_f64(&row, "distance_max")?;
        let variance_mean = required_f64(&row, "variance_mean")?;
        slot_report.max_distance_mean = max_opt(slot_report.max_distance_mean, distance_mean);
        slot_report.max_distance_max = max_opt(slot_report.max_distance_max, distance_max);
        slot_report.max_variance_mean = max_opt(slot_report.max_variance_mean, variance_mean);

        let Some(calibration) = global_calibrations.get(slot_id) else {
            slot_report.fail_count += 1;
            bump(
                &mut slot_report.fail_reasons,
                "MISSING_GLOBAL_SLOT_CALIBRATION",
            );
            continue;
        };
        slot_report.distance_mean_tau = calibration.distance_mean_tau;
        slot_report.distance_max_tau = calibration.distance_max_tau;
        slot_report.variance_mean_tau = calibration.variance_mean_tau;
        slot_report.ood_tau = calibration.ood_tau;
        let fail_reason =
            slot_guard_fail_reason(calibration, distance_mean, distance_max, variance_mean);
        if let Some(reason) = fail_reason {
            slot_report.fail_count += 1;
            bump(&mut slot_report.fail_reasons, reason);
        } else {
            slot_report.pass_count += 1;
        }
    }

    for row in read_jsonl_file(&paths.pairwise_tensor_path())? {
        let language = required_str(&row, "language")?;
        if language != "python" {
            return Err(TctError::invalid(
                "pairwise_correlation_tensors.language",
                format!("TASK-PY-G-015 audits Python only; got {language:?}"),
            ));
        }
        let cell_key = required_str(&row, "cell_key")?;
        let surface = required_str(&row, "surface_class")?;
        let acc = cells.entry(cell_key.to_string()).or_insert_with(|| {
            let mutation_category = required_str(&row, "mutation_category")
                .unwrap_or("unknown")
                .to_string();
            CellAccumulator {
                language: language.to_string(),
                mutation_category,
                ..CellAccumulator::default()
            }
        });
        bump(&mut acc.pair_surface_counts, surface.to_string());
        if surface == "unsupported" {
            acc.unsupported_pair_rows += 1;
        }
    }

    for row in read_jsonl_file(&paths.per_cell_slot_calibration_path())? {
        let cell_key = required_str(&row, "cell_key")?;
        // Fail closed on malformed `cell_key` (F-031). Pre-compute the components so the
        // `?` propagates out of the surrounding `for` loop rather than being swallowed
        // inside an `or_insert_with` closure.
        let acc = if let Some(existing) = cells.get_mut(cell_key) {
            existing
        } else {
            let (language, mutation_category) = split_cell_key(cell_key)?;
            cells
                .entry(cell_key.to_string())
                .or_insert(CellAccumulator {
                    language,
                    mutation_category,
                    ..CellAccumulator::default()
                })
        };
        if required_bool(&row, "valid")? {
            acc.per_cell_calibrated_slots += 1;
        } else {
            acc.per_cell_invalid_slots += 1;
            bump(
                &mut acc.per_cell_invalid_reasons,
                optional_str(&row, "invalid_reason")
                    .unwrap_or("INVALID_PER_CELL_CALIBRATION")
                    .to_string(),
            );
        }
    }

    let per_pair_rows = read_jsonl_file(&paths.per_pair_calibration_path())?;
    if per_pair_rows.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "per_pair_calibration".to_string(),
            observed: 0,
            required: 1,
        });
    }

    let expected_cells = expected_python_gtau_cells();
    let mut missing_neighborhoods = Vec::new();
    let mut cell_reports = Vec::with_capacity(expected_cells.len());
    let required_slots = policy
        .required_slots
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    // Count global slots that are both valid and fresh. `is_stale` returns Err when
    // `stale_after_utc` is missing (F-002 fail-closed), so we explicitly propagate `?` per
    // slot rather than silently degrading. Note: `read_global_calibrations` already rejects
    // rows missing `stale_after_utc`, so this loop should not actually error in practice;
    // the explicit propagation defends against future divergence between the two paths.
    let mut global_calibrated_slots: usize = 0;
    for (slot_id, cal) in global_calibrations.iter() {
        if cal.valid && !is_stale(slot_id, "global", cal, config.now_utc)? {
            global_calibrated_slots += 1;
        }
    }
    let mut summary = GtauGuardAuditSummary {
        expected_cell_count: expected_cells.len(),
        reported_cell_count: 0,
        missing_cell_count: 0,
        low_pass_cell_count: 0,
        fail_closed_cell_count: 0,
        total_slot_guard_pass_count: 0,
        total_slot_guard_fail_count: 0,
        total_slot_rows_seen: 0,
        total_valid_slot_rows: 0,
        global_calibrated_slots,
        required_slot_count: required_slots.len(),
        all_expected_cells_reported: true,
        slot_identity_preserved,
        flat_vector_concat_used,
        strict_policy_ready: policy.strict_conjunction
            && policy.aggregate_score_never_overrides_slot_violation
            && policy.reject_missing_or_stale_calibration
            && policy.global_slot_calibration_ready
            && policy.one_bad_slot_rejects_even_when_aggregate_passes
            && policy.missing_required_slot_rejects
            && policy.stale_calibration_rejects
            && policy.nonfinite_slot_score_rejects,
        guard_green: false,
        ship_gate_countable_after_audit: false,
    };

    for cell_key in expected_cells {
        // EXPECTED_PYTHON_CELLS is curated and always well-formed, but we still propagate
        // the structured error so future expansion (e.g. dynamic cell registries) inherits
        // fail-closed semantics (F-031).
        let (language, mutation_category) = split_cell_key(&cell_key)?;
        let acc = cells.remove(&cell_key).unwrap_or_else(|| CellAccumulator {
            language,
            mutation_category,
            ..CellAccumulator::default()
        });
        let present_slots = acc.per_slot.keys().cloned().collect::<BTreeSet<_>>();
        let missing_slots = required_slots
            .difference(&present_slots)
            .cloned()
            .collect::<Vec<_>>();
        let mut slot_pass = 0usize;
        let mut slot_fail = 0usize;
        let mut valid_rows = 0usize;
        let mut rows_seen = 0usize;
        for slot in acc.per_slot.values() {
            slot_pass += slot.pass_count;
            slot_fail += slot.fail_count + slot.invalid_count;
            valid_rows += slot.valid_rows;
            rows_seen += slot.rows_seen;
        }
        let guard_pass_rate = if slot_pass + slot_fail > 0 {
            Some(slot_pass as f64 / (slot_pass + slot_fail) as f64)
        } else {
            None
        };
        let missing_substrate = rows_seen == 0 || !missing_slots.is_empty();
        let calibration_fail_closed =
            acc.per_cell_calibrated_slots < required_slots.len() || acc.per_cell_invalid_slots > 0;
        let low_pass = guard_pass_rate
            .map(|rate| rate < config.min_guard_pass_rate)
            .unwrap_or(true);
        let status = if missing_substrate {
            GtauCellAuditStatus::MissingSubstrate
        } else if calibration_fail_closed {
            GtauCellAuditStatus::FailClosedCalibration
        } else if low_pass {
            GtauCellAuditStatus::LowPassRate
        } else {
            GtauCellAuditStatus::Ready
        };
        let mut remediation = Vec::new();
        if missing_substrate {
            remediation.push(
                "materialize_missing_cell_slot_tensor_rows_from_aiwonder_forward_cache".to_string(),
            );
        }
        if calibration_fail_closed {
            remediation.push("harvest_cell_specific_known_good_and_failure_rows_then_rerun_per_cell_slot_calibration".to_string());
        }
        if low_pass {
            remediation
                .push("inspect_slot_named_tau_failures_before_promoting_gtau_guard".to_string());
        }
        if acc.unsupported_pair_rows > 0 {
            remediation.push(
                "materialize_pairwise_reward_signal_neighborhoods_for_unsupported_pairs"
                    .to_string(),
            );
        }
        for slot_id in &missing_slots {
            missing_neighborhoods.push(RewardSignalNeighborhoodGap {
                cell_key: cell_key.clone(),
                slot_id: Some(slot_id.clone()),
                pair: None,
                reason: "MISSING_REQUIRED_SLOT_TENSOR_ROWS".to_string(),
                remediation: "rerun/copy the active-slot forward-cache rows for this cell under /zfs/archive/contextgraph".to_string(),
            });
        }
        for (reason, count) in &acc.per_cell_invalid_reasons {
            missing_neighborhoods.push(RewardSignalNeighborhoodGap {
                cell_key: cell_key.clone(),
                slot_id: None,
                pair: None,
                reason: format!("PER_CELL_CALIBRATION_{reason}_COUNT_{count}"),
                remediation:
                    "increase per-cell labeled support and rerun #389 per-slot calibration"
                        .to_string(),
            });
        }
        if acc.unsupported_pair_rows > 0 {
            missing_neighborhoods.push(RewardSignalNeighborhoodGap {
                cell_key: cell_key.clone(),
                slot_id: None,
                pair: Some("cross_embedder_pairs".to_string()),
                reason: format!(
                    "UNSUPPORTED_PAIRWISE_NEIGHBORHOOD_ROWS_{}",
                    acc.unsupported_pair_rows
                ),
                remediation: "increase row overlap for pairwise tensor windows and rerun #388"
                    .to_string(),
            });
        }

        summary.reported_cell_count += 1;
        summary.missing_cell_count += usize::from(missing_substrate);
        summary.low_pass_cell_count += usize::from(low_pass);
        summary.fail_closed_cell_count += usize::from(calibration_fail_closed);
        summary.total_slot_guard_pass_count += slot_pass;
        summary.total_slot_guard_fail_count += slot_fail;
        summary.total_slot_rows_seen += rows_seen;
        summary.total_valid_slot_rows += valid_rows;

        cell_reports.push(GtauCellAuditReport {
            cell_key,
            language: acc.language,
            mutation_category: acc.mutation_category,
            slot_rows_seen: rows_seen,
            valid_slot_rows: valid_rows,
            slot_guard_pass_count: slot_pass,
            slot_guard_fail_count: slot_fail,
            guard_pass_rate,
            required_slots_present: present_slots.into_iter().collect(),
            missing_required_slots: missing_slots,
            per_slot: acc.per_slot,
            pair_surface_counts: acc.pair_surface_counts,
            unsupported_pair_rows: acc.unsupported_pair_rows,
            per_cell_calibrated_slots: acc.per_cell_calibrated_slots,
            per_cell_invalid_slots: acc.per_cell_invalid_slots,
            calibration_fail_closed,
            low_pass,
            status,
            remediation,
        });
    }

    summary.all_expected_cells_reported =
        summary.reported_cell_count == summary.expected_cell_count;
    summary.guard_green = summary.all_expected_cells_reported
        && summary.low_pass_cell_count == 0
        && summary.missing_cell_count == 0
        && summary.fail_closed_cell_count == 0
        && summary.global_calibrated_slots == summary.required_slot_count
        && summary.strict_policy_ready
        && summary.slot_identity_preserved
        && !summary.flat_vector_concat_used;
    summary.ship_gate_countable_after_audit = summary.guard_green;

    let source_of_truth = GtauGuardAuditSourceOfTruth {
        tensor_root: paths.tensor_root.clone(),
        calibration_root: paths.calibration_root.clone(),
        slot_distribution_tensors: paths.slot_distribution_path(),
        pairwise_correlation_tensors: paths.pairwise_tensor_path(),
        per_slot_calibration: paths.per_slot_calibration_path(),
        per_cell_slot_calibration: paths.per_cell_slot_calibration_path(),
        per_pair_calibration: paths.per_pair_calibration_path(),
        ood_guard_policy: paths.ood_guard_policy_path(),
        tensor_manifest_sha256: optional_sha256_file(&paths.tensor_manifest_path())?,
        tensor_report_sha256: optional_sha256_file(&paths.tensor_report_path())?,
        policy_sha256: sha256_file(&paths.ood_guard_policy_path())?,
    };
    let passes = summary.all_expected_cells_reported
        && summary.strict_policy_ready
        && summary.slot_identity_preserved
        && !summary.flat_vector_concat_used;

    Ok(GtauGuardAuditReport {
        artifact_kind: "python_gtau_guard_audit_report".to_string(),
        schema_version: GTAU_GUARD_AUDIT_SCHEMA_VERSION,
        formula_version: GTAU_GUARD_AUDIT_FORMULA_VERSION.to_string(),
        task_id: "TASK-PY-G-015".to_string(),
        created_at_utc: config.now_utc.to_rfc3339(),
        source_of_truth,
        policy,
        summary,
        cell_reports,
        missing_reward_signal_neighborhoods: missing_neighborhoods,
        passes,
    })
}

fn read_global_calibrations(
    paths: &GtauGuardAuditPaths,
    config: &GtauGuardAuditConfig,
) -> Result<BTreeMap<String, SlotCalibration>, TctError> {
    let mut out = BTreeMap::new();
    for row in read_jsonl_file(&paths.per_slot_calibration_path())? {
        if optional_str(&row, "scope") != Some("global") {
            continue;
        }
        let slot_id = required_str(&row, "slot_id")?.to_string();
        let valid = required_bool(&row, "valid")?;
        let stale_after_utc = optional_str(&row, "stale_after_utc")
            .map(parse_utc)
            .transpose()?;
        let thresholds = row.get("thresholds").unwrap_or(&Value::Null);
        let calibration = SlotCalibration {
            valid,
            stale_after_utc,
            distance_mean_tau: nested_f64(thresholds, "distance_mean_tau")?,
            distance_max_tau: nested_f64(thresholds, "distance_max_tau")?,
            variance_mean_tau: nested_f64(thresholds, "variance_mean_tau")?,
            ood_tau: nested_f64(thresholds, "ood_tau")?,
        };
        if is_stale(&slot_id, "global", &calibration, config.now_utc)? {
            out.insert(
                slot_id,
                SlotCalibration {
                    valid: false,
                    ..calibration
                },
            );
        } else {
            out.insert(slot_id, calibration);
        }
    }
    Ok(out)
}

fn parse_policy(value: &Value) -> Result<GtauGuardAuditPolicySummary, TctError> {
    let required_slots = value
        .get("required_slots")
        .and_then(Value::as_array)
        .ok_or_else(|| TctError::invalid("ood_guard_policy.required_slots", "missing array"))?
        .iter()
        .map(|slot| {
            slot.as_str().map(str::to_string).ok_or_else(|| {
                TctError::invalid("ood_guard_policy.required_slots", "non-string slot")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let edge = value.get("edge_case_summary").unwrap_or(&Value::Null);
    Ok(GtauGuardAuditPolicySummary {
        required_slots,
        strict_conjunction: required_bool(value, "strict_conjunction")?,
        aggregate_score_never_overrides_slot_violation: required_bool(
            value,
            "aggregate_score_never_overrides_slot_violation",
        )?,
        reject_missing_or_stale_calibration: required_bool(
            value,
            "reject_missing_or_stale_calibration",
        )?,
        global_slot_calibration_ready: required_bool(value, "global_slot_calibration_ready")?,
        ship_gate_countable_before_audit: required_bool(value, "ship_gate_countable")?,
        one_bad_slot_rejects_even_when_aggregate_passes: required_bool(
            edge,
            "one_bad_slot_rejects_even_when_aggregate_passes",
        )?,
        missing_required_slot_rejects: required_bool(edge, "missing_required_slot_rejects")?,
        stale_calibration_rejects: required_bool(edge, "stale_calibration_rejects")?,
        nonfinite_slot_score_rejects: required_bool(edge, "nonfinite_slot_score_rejects")?,
    })
}

fn slot_guard_fail_reason(
    calibration: &SlotCalibration,
    distance_mean: f64,
    distance_max: f64,
    variance_mean: f64,
) -> Option<&'static str> {
    if !calibration.valid {
        return Some("MISSING_OR_STALE_GLOBAL_SLOT_CALIBRATION");
    }
    let Some(distance_mean_tau) = calibration.distance_mean_tau else {
        return Some("MISSING_DISTANCE_MEAN_TAU");
    };
    let Some(distance_max_tau) = calibration.distance_max_tau else {
        return Some("MISSING_DISTANCE_MAX_TAU");
    };
    let Some(variance_mean_tau) = calibration.variance_mean_tau else {
        return Some("MISSING_VARIANCE_MEAN_TAU");
    };
    if distance_mean > distance_mean_tau {
        return Some("DISTANCE_MEAN_TAU_EXCEEDED");
    }
    if distance_max > distance_max_tau {
        return Some("DISTANCE_MAX_TAU_EXCEEDED");
    }
    if variance_mean > variance_mean_tau {
        return Some("VARIANCE_MEAN_TAU_EXCEEDED");
    }
    None
}

fn read_json_file(path: &Path) -> Result<Value, TctError> {
    let bytes = fs::read(path).map_err(|err| TctError::io("read", path, err))?;
    serde_json::from_slice(&bytes).map_err(TctError::from)
}

fn read_jsonl_file(path: &Path) -> Result<Vec<Value>, TctError> {
    let text = fs::read_to_string(path).map_err(|err| TctError::io("read", path, err))?;
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(idx, line)| {
            serde_json::from_str::<Value>(line).map_err(|err| {
                TctError::invalid(
                    "jsonl",
                    format!("{} line {} did not parse: {err}", path.display(), idx + 1),
                )
            })
        })
        .collect()
}

fn required_str<'a>(value: &'a Value, field: &str) -> Result<&'a str, TctError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| TctError::invalid(field, "missing string field"))
}

fn optional_str<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn required_bool(value: &Value, field: &str) -> Result<bool, TctError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| TctError::invalid(field, "missing bool field"))
}

fn required_f64(value: &Value, field: &str) -> Result<f64, TctError> {
    let number = value
        .get(field)
        .and_then(Value::as_f64)
        .ok_or_else(|| TctError::invalid(field, "missing numeric field"))?;
    if !number.is_finite() {
        return Err(TctError::nan(field, format!("non-finite value {number}")));
    }
    Ok(number)
}

fn nested_f64(value: &Value, field: &str) -> Result<Option<f64>, TctError> {
    if value.is_null() {
        return Ok(None);
    }
    match value.get(field) {
        Some(Value::Null) | None => Ok(None),
        Some(item) => {
            let number = item
                .as_f64()
                .ok_or_else(|| TctError::invalid(field, "expected numeric threshold"))?;
            if !number.is_finite() {
                return Err(TctError::nan(field, format!("non-finite value {number}")));
            }
            Ok(Some(number))
        }
    }
}

fn parse_utc(value: &str) -> Result<DateTime<Utc>, TctError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| {
            TctError::invalid(
                "stale_after_utc",
                format!("expected RFC3339 timestamp, got {value:?}: {err}"),
            )
        })
}

/// Returns `Ok(true)` when the calibration is stale relative to `now`, `Ok(false)` when fresh.
/// Returns `Err(TctError::CalibrationMissingFreshnessTimestamp)` when `stale_after_utc` is `None`.
///
/// A `SlotCalibration` without a freshness fence is a doctrinal invariant violation per
/// CLAUDE.md §1 Q2 (TCT G-tau guard) + FSV-PROTOCOL §3.5 (missing required metadata fails closed).
/// Treating `None` as "permanently fresh" (the original F-002 bug) silently accepts arbitrarily
/// old calibration rows; treating `None` as "always stale" hides the malformed input from
/// operators. The fail-closed contract surfaces the missing metadata as a structured error so
/// the calibration generator can be corrected at source.
fn is_stale(
    slot_id: &str,
    scope: &str,
    calibration: &SlotCalibration,
    now: DateTime<Utc>,
) -> Result<bool, TctError> {
    match calibration.stale_after_utc {
        Some(stale_after) => Ok(now > stale_after),
        None => Err(TctError::CalibrationMissingFreshnessTimestamp {
            slot: slot_id.to_string(),
            scope: scope.to_string(),
        }),
    }
}

/// Splits a `language:mutation_category` cell key into its two components.
///
/// Returns `Err(TctError::CellKeyMalformed)` when the key does not contain exactly one
/// non-empty `language` and one non-empty `mutation_category` separated by ':'.
/// The original F-031 implementation silently substituted `"unknown"` for both halves,
/// which caused multiple distinct malformed keys to fold into the same BTreeMap bucket
/// and corrupted per-cell aggregation (FSV-PROTOCOL §3.5).
fn split_cell_key(cell_key: &str) -> Result<(String, String), TctError> {
    let mut parts = cell_key.splitn(2, ':');
    let language = parts.next().unwrap_or("");
    let mutation_category = parts.next().unwrap_or("");
    if language.is_empty() || mutation_category.is_empty() {
        return Err(TctError::CellKeyMalformed {
            value: cell_key.to_string(),
            context:
                "cell_key must match '<language>:<mutation_category>' with both halves non-empty"
                    .to_string(),
        });
    }
    Ok((language.to_string(), mutation_category.to_string()))
}

fn bump(map: &mut BTreeMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_insert(0) += 1;
}

fn max_opt(current: Option<f64>, candidate: f64) -> Option<f64> {
    Some(
        current
            .map(|value| value.max(candidate))
            .unwrap_or(candidate),
    )
}

fn sha256_file(path: &Path) -> Result<String, TctError> {
    let bytes = fs::read(path).map_err(|err| TctError::io("read", path, err))?;
    Ok(format!(
        "sha256:{}",
        hex::encode(sha2::Sha256::digest(bytes))
    ))
}

fn optional_sha256_file(path: &Path) -> Result<Option<String>, TctError> {
    if !path.exists() {
        return Ok(None);
    }
    sha256_file(path).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn calibration(stale_after_utc: Option<DateTime<Utc>>) -> SlotCalibration {
        SlotCalibration {
            valid: true,
            stale_after_utc,
            distance_mean_tau: Some(0.1),
            distance_max_tau: Some(0.2),
            variance_mean_tau: Some(0.3),
            ood_tau: Some(0.4),
        }
    }

    #[test]
    fn missing_stale_after_timestamp_fails_closed() {
        let now = Utc::now();

        let err = is_stale("E_AST", "global", &calibration(None), now)
            .expect_err("missing freshness timestamp must fail closed");
        assert_eq!(
            err.code(),
            "MEJEPA_TCT_CALIBRATION_MISSING_FRESHNESS_TIMESTAMP"
        );
    }

    #[test]
    fn future_stale_after_timestamp_is_fresh() {
        let now = Utc::now();

        assert!(!is_stale(
            "E_AST",
            "global",
            &calibration(Some(now + Duration::minutes(1))),
            now
        )
        .expect("future timestamp should be fresh"));
    }

    #[test]
    fn past_stale_after_timestamp_is_stale() {
        let now = Utc::now();

        assert!(is_stale(
            "E_AST",
            "global",
            &calibration(Some(now - Duration::minutes(1))),
            now
        )
        .expect("past timestamp should be stale"));
    }

    /// F-002 regression: the slot_id and scope must be carried into the structured error so
    /// operators can locate the malformed calibration row in /zfs/archive/.../per_slot_calibration.jsonl.
    #[test]
    fn missing_stale_after_error_carries_slot_and_scope() {
        let now = Utc::now();
        let err = is_stale(
            "E_DATA_FLOW",
            "per_cell:python:wrong_file",
            &calibration(None),
            now,
        )
        .expect_err("missing freshness must fail closed");
        match err {
            TctError::CalibrationMissingFreshnessTimestamp { slot, scope } => {
                assert_eq!(slot, "E_DATA_FLOW");
                assert_eq!(scope, "per_cell:python:wrong_file");
            }
            other => panic!("expected CalibrationMissingFreshnessTimestamp, got: {other:?}"),
        }
    }

    /// F-031 regression: well-formed cell_keys split cleanly into both halves.
    #[test]
    fn split_cell_key_accepts_well_formed_key() {
        let (lang, mutation) = split_cell_key("python:known_good").expect("well-formed");
        assert_eq!(lang, "python");
        assert_eq!(mutation, "known_good");
    }

    /// F-031 regression: a key with no ':' separator must fail closed rather than
    /// silently substitute "unknown".
    #[test]
    fn split_cell_key_missing_separator_fails_closed() {
        let err = split_cell_key("python_only").expect_err("must fail closed");
        assert_eq!(err.code(), "MEJEPA_TCT_CELL_KEY_MALFORMED");
        match err {
            TctError::CellKeyMalformed { value, .. } => assert_eq!(value, "python_only"),
            other => panic!("expected CellKeyMalformed, got: {other:?}"),
        }
    }

    /// F-031 regression: the empty string must fail closed; before the fix, multiple distinct
    /// malformed keys folded into ("unknown", "unknown") and collided in the BTreeMap.
    #[test]
    fn split_cell_key_empty_input_fails_closed() {
        let err = split_cell_key("").expect_err("empty must fail closed");
        assert_eq!(err.code(), "MEJEPA_TCT_CELL_KEY_MALFORMED");
    }

    /// F-031 regression: an empty language half must fail closed.
    #[test]
    fn split_cell_key_empty_language_fails_closed() {
        let err = split_cell_key(":subtle_flip").expect_err("empty language must fail closed");
        assert_eq!(err.code(), "MEJEPA_TCT_CELL_KEY_MALFORMED");
    }

    /// F-031 regression: an empty mutation_category half must fail closed.
    #[test]
    fn split_cell_key_empty_mutation_category_fails_closed() {
        let err = split_cell_key("python:").expect_err("empty mutation must fail closed");
        assert_eq!(err.code(), "MEJEPA_TCT_CELL_KEY_MALFORMED");
    }

    /// F-031 regression: a key with two colons treats the second as part of the mutation_category
    /// (splitn(2,':')); both halves remain non-empty so this is a legitimate Ok.
    #[test]
    fn split_cell_key_extra_colon_kept_in_mutation_category() {
        let (lang, mutation) = split_cell_key("python:sub:category").expect("well-formed");
        assert_eq!(lang, "python");
        assert_eq!(mutation, "sub:category");
    }
}
