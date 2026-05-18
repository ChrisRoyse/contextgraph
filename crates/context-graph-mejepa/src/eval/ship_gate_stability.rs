use rocksdb::IteratorMode;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::ablation::{
    negative_action_ablation_gate_status, AblationReport, NegativeActionAblationGateStatus,
    NEGATIVE_ACTION_ABLATION_BLOCKER,
};
use super::cell_exemptions::CellExemption;
use super::error::{EvalError, EvalErrorCode};
use super::store::{RocksDbEvalStore, CF_MEJEPA_EVAL_REPORTS, CF_MEJEPA_MODEL_PROMOTIONS};
use super::types::{validate_active_python_ship_gate_report, EvalReport};

pub const SHIP_GATE_REQUIRED_CONSECUTIVE_WINDOWS: usize = 4;
pub const SHIP_GATE_STABILITY_CORRELATION_THRESHOLD: f32 = 0.95;
pub const SHIP_GATE_STABILITY_BLOCKER: &str = "MEJEPA_EVAL_SHIP_GATE_STABILITY_PENDING";

const MODEL_PROMOTION_RESET_PREFIXES: [&[u8]; 3] = [
    b"phase_e/per-cell-promotion/",
    b"phase_e/model-promotion/",
    b"model-promotion/",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ShipGateStabilityStatus {
    pub consecutive_passing_windows: usize,
    pub required_consecutive_windows: usize,
    pub ready: bool,
    pub latest_report_date: Option<String>,
    pub latest_report_passed_window: bool,
    pub latest_report_correlation: Option<f32>,
    pub latest_effective_correlation: Option<f32>,
    pub correlation_threshold: f32,
    pub evaluated_window_count: usize,
    pub model_promotion_reset_count: usize,
    pub latest_reset_reason: Option<String>,
    pub latest_reset_unix_ms: Option<i64>,
    pub negative_action_ablation_ready: bool,
    pub negative_action_ablation: NegativeActionAblationGateStatus,
}

pub fn ship_gate_stability_status(
    store: &RocksDbEvalStore,
) -> Result<ShipGateStabilityStatus, EvalError> {
    ship_gate_stability_status_with_exemptions(store, &BTreeMap::new())
}

pub fn ship_gate_stability_status_with_exemptions(
    store: &RocksDbEvalStore,
    cell_exemptions: &BTreeMap<String, CellExemption>,
) -> Result<ShipGateStabilityStatus, EvalError> {
    ship_gate_stability_status_with_requirements(
        store,
        SHIP_GATE_REQUIRED_CONSECUTIVE_WINDOWS,
        SHIP_GATE_STABILITY_CORRELATION_THRESHOLD,
        cell_exemptions,
    )
}

pub fn ship_gate_stability_status_with_requirements(
    store: &RocksDbEvalStore,
    required_consecutive_windows: usize,
    correlation_threshold: f32,
    cell_exemptions: &BTreeMap<String, CellExemption>,
) -> Result<ShipGateStabilityStatus, EvalError> {
    if required_consecutive_windows == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            "required_consecutive_windows must be greater than zero",
        ));
    }
    if !correlation_threshold.is_finite() || !(-1.0..=1.0).contains(&correlation_threshold) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidConfig,
            format!("correlation_threshold must be finite in [-1,1]; got {correlation_threshold}"),
        ));
    }
    let reports = load_eval_reports_chronological(store)?;
    let resets = load_model_promotion_reset_events(store)?;
    let ablation_reports = store.load_ablation_reports_chronological()?;
    Ok(compute_ship_gate_stability(
        &reports,
        &resets,
        &ablation_reports,
        required_consecutive_windows,
        correlation_threshold,
        cell_exemptions,
    ))
}

fn load_eval_reports_chronological(store: &RocksDbEvalStore) -> Result<Vec<EvalReport>, EvalError> {
    let db = store.db();
    let cf = crate::calibration::cf(&db, CF_MEJEPA_EVAL_REPORTS).map_err(EvalError::from)?;
    let mut reports = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) = item?;
        let report: EvalReport = bincode::deserialize(&value)?;
        report.validate()?;
        validate_active_python_ship_gate_report(&report)?;
        reports.push(report);
    }
    reports.sort_by(|left, right| {
        left.generated_at_unix_ms
            .cmp(&right.generated_at_unix_ms)
            .then_with(|| left.report_date.cmp(&right.report_date))
    });
    Ok(reports)
}

fn load_model_promotion_reset_events(
    store: &RocksDbEvalStore,
) -> Result<Vec<ModelPromotionResetEvent>, EvalError> {
    let db = store.db();
    let cf = crate::calibration::cf(&db, CF_MEJEPA_MODEL_PROMOTIONS).map_err(EvalError::from)?;
    let mut resets = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (key, _value) = item?;
        if let Some(timestamp_millis) = model_promotion_reset_timestamp_from_key(key.as_ref()) {
            resets.push(ModelPromotionResetEvent {
                timestamp_millis,
                key: String::from_utf8_lossy(key.as_ref()).to_string(),
            });
        }
    }
    resets.sort_by(|left, right| {
        left.timestamp_millis
            .cmp(&right.timestamp_millis)
            .then_with(|| left.key.cmp(&right.key))
    });
    Ok(resets)
}

fn compute_ship_gate_stability(
    reports: &[EvalReport],
    resets: &[ModelPromotionResetEvent],
    ablation_reports: &[AblationReport],
    required_consecutive_windows: usize,
    correlation_threshold: f32,
    cell_exemptions: &BTreeMap<String, CellExemption>,
) -> ShipGateStabilityStatus {
    let mut consecutive_passing_windows = 0usize;
    let mut reset_idx = 0usize;
    let mut latest_reset_reason = None;
    let mut latest_reset_unix_ms = None;
    let mut latest_report_date = None;
    let mut latest_report_passed_window = false;
    let mut latest_report_correlation = None;
    let mut latest_effective_correlation = None;

    for report in reports {
        while let Some(reset) = resets.get(reset_idx) {
            if reset.timestamp_millis > report.generated_at_unix_ms {
                break;
            }
            consecutive_passing_windows = 0;
            latest_reset_reason = Some(format!("model_promotion:{}", reset.key));
            latest_reset_unix_ms = Some(reset.timestamp_millis);
            reset_idx += 1;
        }

        latest_report_date = Some(report.report_date.clone());
        latest_report_correlation = report.overall_correlation;
        latest_effective_correlation = effective_ship_gate_correlation(report, cell_exemptions);
        latest_report_passed_window =
            report_passes_stability_window(report, correlation_threshold, cell_exemptions);
        if latest_report_passed_window {
            consecutive_passing_windows += 1;
        } else {
            consecutive_passing_windows = 0;
            latest_reset_reason = Some(format!("failing_window:{}", report.report_date));
            latest_reset_unix_ms = Some(report.generated_at_unix_ms);
        }
    }

    while let Some(reset) = resets.get(reset_idx) {
        consecutive_passing_windows = 0;
        latest_reset_reason = Some(format!("model_promotion:{}", reset.key));
        latest_reset_unix_ms = Some(reset.timestamp_millis);
        reset_idx += 1;
    }

    let negative_action_ablation = negative_action_ablation_gate_status(ablation_reports)
        .unwrap_or_else(|err| NegativeActionAblationGateStatus {
            source_of_truth_cf: context_graph_mejepa_cf::CF_MEJEPA_ABLATION_REPORTS.to_string(),
            ready: false,
            latest_report_id: None,
            latest_report_date: None,
            latest_verdict: None,
            effective_report_id: None,
            effective_report_date: None,
            effective_verdict: None,
            effective_score_drop_pct: None,
            blocker: Some(format!("{NEGATIVE_ACTION_ABLATION_BLOCKER}: {err}")),
            warning: None,
            incomplete_warning_count: 0,
        });
    if let Some(blocker) = &negative_action_ablation.blocker {
        latest_reset_reason = Some(format!("negative_action_ablation:{blocker}"));
        if let Some(timestamp_millis) = ablation_reports
            .iter()
            .rev()
            .find(|report| report.verdict.blocks_ship_gate())
            .map(|report| report.generated_at_unix_ms)
        {
            latest_reset_unix_ms = Some(timestamp_millis);
        }
    }
    let ready = consecutive_passing_windows >= required_consecutive_windows
        && negative_action_ablation.ready;

    ShipGateStabilityStatus {
        consecutive_passing_windows,
        required_consecutive_windows,
        ready,
        latest_report_date,
        latest_report_passed_window,
        latest_report_correlation,
        latest_effective_correlation,
        correlation_threshold,
        evaluated_window_count: reports.len(),
        model_promotion_reset_count: resets.len(),
        latest_reset_reason,
        latest_reset_unix_ms,
        negative_action_ablation_ready: negative_action_ablation.ready,
        negative_action_ablation,
    }
}

pub fn effective_ship_gate_correlation(
    report: &EvalReport,
    cell_exemptions: &BTreeMap<String, CellExemption>,
) -> Option<f32> {
    let mut worst_non_exempt = None::<f32>;
    for (cell, value) in &report.per_cell_correlation {
        if cell_exemptions.contains_key(cell) {
            continue;
        }
        match value {
            Some(value) => {
                worst_non_exempt = Some(worst_non_exempt.map_or(*value, |worst| worst.min(*value)));
            }
            None => return None,
        }
    }
    worst_non_exempt.or(report.overall_correlation)
}

pub fn non_exempt_ship_gate_failures(
    report: &EvalReport,
    cell_exemptions: &BTreeMap<String, CellExemption>,
) -> Vec<String> {
    report
        .ship_gate_failures
        .iter()
        .filter(|failure| {
            !cell_exemptions
                .keys()
                .any(|cell| failure.contains(cell.as_str()))
        })
        .cloned()
        .collect()
}

fn report_passes_stability_window(
    report: &EvalReport,
    correlation_threshold: f32,
    cell_exemptions: &BTreeMap<String, CellExemption>,
) -> bool {
    if !non_exempt_ship_gate_failures(report, cell_exemptions).is_empty() {
        return false;
    }
    effective_ship_gate_correlation(report, cell_exemptions)
        .map(|value| value >= correlation_threshold)
        .unwrap_or(false)
}

fn model_promotion_reset_timestamp_from_key(key: &[u8]) -> Option<i64> {
    for prefix in MODEL_PROMOTION_RESET_PREFIXES {
        let Some(rest) = key.strip_prefix(prefix) else {
            continue;
        };
        return parse_leading_i64(rest);
    }
    None
}

fn parse_leading_i64(bytes: &[u8]) -> Option<i64> {
    let end = bytes
        .iter()
        .position(|byte| !byte.is_ascii_digit())
        .unwrap_or(bytes.len());
    if end == 0 {
        return None;
    }
    std::str::from_utf8(&bytes[..end]).ok()?.parse().ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelPromotionResetEvent {
    timestamp_millis: i64,
    key: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::types::{
        ActiveLearningSummary, EvalProvenance, MutationCategory, OpenResearchQuestionStatus,
    };
    use std::collections::BTreeMap;

    #[test]
    fn three_passing_windows_are_not_ready_fourth_is_ready() {
        let reports = vec![
            report("2026-05-13", 1_000, 0.96, true),
            report("2026-05-14", 2_000, 0.96, true),
            report("2026-05-15", 3_000, 0.96, true),
        ];
        let status = compute_ship_gate_stability(&reports, &[], &[], 4, 0.95, &BTreeMap::new());
        assert_eq!(status.consecutive_passing_windows, 3);
        assert!(!status.ready);

        let mut reports = reports;
        reports.push(report("2026-05-16", 4_000, 0.96, true));
        let status = compute_ship_gate_stability(&reports, &[], &[], 4, 0.95, &BTreeMap::new());
        assert_eq!(status.consecutive_passing_windows, 4);
        assert!(status.ready);
    }

    #[test]
    fn failing_window_and_model_promotion_reset_streak() {
        let reports = vec![
            report("2026-05-13", 1_000, 0.96, true),
            report("2026-05-14", 2_000, 0.96, true),
            report("2026-05-15", 3_000, 0.90, false),
        ];
        let status = compute_ship_gate_stability(&reports, &[], &[], 4, 0.95, &BTreeMap::new());
        assert_eq!(status.consecutive_passing_windows, 0);
        assert_eq!(
            status.latest_reset_reason,
            Some("failing_window:2026-05-15".to_string())
        );

        let reports = vec![
            report("2026-05-13", 1_000, 0.96, true),
            report("2026-05-14", 2_000, 0.96, true),
        ];
        let resets = vec![ModelPromotionResetEvent {
            timestamp_millis: 3_000,
            key: "phase_e/per-cell-promotion/00000000000000003000".to_string(),
        }];
        let status = compute_ship_gate_stability(&reports, &resets, &[], 4, 0.95, &BTreeMap::new());
        assert_eq!(status.consecutive_passing_windows, 0);
        assert_eq!(status.model_promotion_reset_count, 1);
        assert_eq!(
            status.latest_reset_reason,
            Some("model_promotion:phase_e/per-cell-promotion/00000000000000003000".to_string())
        );
    }

    #[test]
    fn exempt_low_cell_does_not_reset_passing_window() {
        let mut report = report("2026-05-13", 1_000, 0.96, false);
        report
            .per_cell_correlation
            .insert("compile_error::python".to_string(), Some(0.10));
        report.ship_gate_failures =
            vec!["per_cell_correlation compile_error::python 0.100000 < 0.950000".to_string()];
        let exemptions = BTreeMap::from([(
            "compile_error::python".to_string(),
            CellExemption {
                cell: "compile_error::python".to_string(),
                reason: "operator marked impossible".to_string(),
                operator_id: "operator".to_string(),
                expires_unix_ms: None,
            },
        )]);

        let status = compute_ship_gate_stability(&[report], &[], &[], 1, 0.95, &exemptions);
        assert_eq!(status.consecutive_passing_windows, 1);
        assert!(status.ready);
        assert_eq!(status.latest_effective_correlation, Some(0.96));
    }

    fn report(date: &str, timestamp_millis: i64, correlation: f32, passed: bool) -> EvalReport {
        let mut per_cell = BTreeMap::new();
        per_cell.insert(
            crate::eval::cell_key(MutationCategory::KnownGood, crate::Language::Python),
            Some(correlation),
        );
        let per_cell_convergence_eta =
            crate::eval::baseline_convergence_eta_for_cells(&per_cell, 0.95);
        EvalReport {
            report_date: date.to_string(),
            generated_at_unix_ms: timestamp_millis,
            rolling_window_size: 100,
            holdout_count: 100,
            overall_correlation: Some(correlation),
            per_category_correlation: BTreeMap::new(),
            per_language_correlation: BTreeMap::new(),
            per_cell_correlation: per_cell,
            cell_exemptions: BTreeMap::new(),
            bayesian_shrinkage: BTreeMap::new(),
            conformal_coverage_health: BTreeMap::new(),
            ood_calibration_health: BTreeMap::new(),
            gtau_pass_rate: BTreeMap::new(),
            per_prediction_class_calibration: BTreeMap::new(),
            per_failure_mode_class: crate::eval::empty_failure_mode_class_metrics(
                100,
                &crate::eval::EvalConfig::default(),
            ),
            per_cell_convergence_eta,
            active_learning: ActiveLearningSummary {
                queued_count: 0,
                evicted_count: 0,
                ood_escalation_count: 0,
            },
            state_transfer_diagnostic: None,
            per_cell_state_transfer: BTreeMap::new(),
            failing_cell_classifications: BTreeMap::new(),
            aux_head_distillation: None,
            regression_checks: Vec::new(),
            open_research_questions: vec![OpenResearchQuestionStatus {
                id: "none".to_string(),
                question: "none".to_string(),
                status: "closed".to_string(),
            }],
            q1_pass_rate: if passed { 1.0 } else { 0.0 },
            q2_report_correlation: Some(correlation),
            q3_side_effect_agreement: Some(1.0),
            ship_gate_passed: passed,
            ship_gate_failures: if passed {
                Vec::new()
            } else {
                vec!["per_window_correlation_below_0.95".to_string()]
            },
            provenance: EvalProvenance {
                corpus_sha: "synthetic".to_string(),
                eval_code_version: "test".to_string(),
                calibration_version: "test".to_string(),
                generated_by: "ship_gate_stability_test".to_string(),
            },
            wall_clock_seconds: 0.01,
        }
    }
}
