use serde::{Deserialize, Serialize};

use crate::eval::EvalReport;
use crate::heal::errors::HealError;
use crate::heal::policy::{load_policy_record, persist_policy_record, policy_key};
use crate::heal::store::HealRocksStore;
use crate::types::HeadId;

pub const MAX_LAMBDA_RAMP_PER_WEEK: f32 = 0.05;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LambdaRampState {
    pub head: HeadId,
    pub lambda: f32,
    pub last_updated_unix_ms: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LambdaRampDecision {
    pub head: HeadId,
    pub previous_lambda: f32,
    pub requested_lambda: f32,
    pub new_lambda: f32,
    pub holdout_metric: f32,
    pub critical_head_degraded: bool,
    pub updated: bool,
    pub source_of_truth_key_hex: String,
}

pub fn ramp_lambda_for_head(
    storage: &HealRocksStore,
    head: HeadId,
    holdout_metric: f32,
    requested_lambda: f32,
    critical_head_degraded: bool,
) -> Result<LambdaRampDecision, HealError> {
    if !holdout_metric.is_finite() || !(0.0..=1.0).contains(&holdout_metric) {
        return Err(HealError::invalid(
            "lambda_ramp.holdout_metric",
            "holdout metric must be finite in [0,1]",
        ));
    }
    if !requested_lambda.is_finite() || requested_lambda < 0.0 {
        return Err(HealError::invalid(
            "lambda_ramp.requested_lambda",
            "requested lambda must be finite and non-negative",
        ));
    }
    let key = lambda_key(head)?;
    let previous = load_policy_record::<LambdaRampState>(storage, &key)?
        .map(|state| state.lambda)
        .unwrap_or(default_lambda(head));
    let gate = head_ramp_gate(head);
    let mut new_lambda = previous;
    let mut updated = false;
    if !critical_head_degraded && holdout_metric >= gate && requested_lambda > previous {
        new_lambda = requested_lambda.min(previous + MAX_LAMBDA_RAMP_PER_WEEK);
        updated = new_lambda > previous;
    }
    let state = LambdaRampState {
        head,
        lambda: new_lambda,
        last_updated_unix_ms: chrono::Utc::now().timestamp_millis(),
        source: format!("holdout_metric={holdout_metric};gate={gate}"),
    };
    persist_policy_record(storage, &key, &state)?;
    Ok(LambdaRampDecision {
        head,
        previous_lambda: previous,
        requested_lambda,
        new_lambda,
        holdout_metric,
        critical_head_degraded,
        updated,
        source_of_truth_key_hex: hex::encode(key),
    })
}

pub fn read_lambda(storage: &HealRocksStore, head: HeadId) -> Result<f32, HealError> {
    Ok(
        load_policy_record::<LambdaRampState>(storage, &lambda_key(head)?)?
            .map(|state| state.lambda)
            .unwrap_or(default_lambda(head)),
    )
}

pub fn tick_lambda_ramp(storage: &HealRocksStore) -> Result<Vec<LambdaRampDecision>, HealError> {
    let Some(report) = latest_eval_report(storage)? else {
        return Ok(Vec::new());
    };
    let mut decisions = Vec::new();
    let critical_head_degraded = report
        .overall_correlation
        .map(|value| value < 0.80)
        .unwrap_or(false);
    for head in HeadId::ALL {
        let Some(metric) = head_metric(&report, head) else {
            continue;
        };
        decisions.push(ramp_lambda_for_head(
            storage,
            head,
            metric,
            target_lambda(head),
            critical_head_degraded,
        )?);
    }
    Ok(decisions)
}

pub fn head_ramp_gate(head: HeadId) -> f32 {
    match head {
        HeadId::FailureMode => 0.60,
        HeadId::EdgeCase => 0.50,
        HeadId::TechDebt => 0.50,
        HeadId::Perf => 0.90,
        HeadId::Security => 0.70,
        HeadId::Accuracy => 0.95,
        HeadId::Cost => 0.80,
        HeadId::Reasoning => 0.60,
        HeadId::Panel | HeadId::Oracle => 0.95,
    }
}

fn default_lambda(head: HeadId) -> f32 {
    match head {
        HeadId::Panel | HeadId::Oracle => 1.0,
        _ => 0.05,
    }
}

fn target_lambda(head: HeadId) -> f32 {
    match head {
        HeadId::Panel | HeadId::Oracle => 1.0,
        HeadId::FailureMode => 0.30,
        HeadId::EdgeCase => 0.20,
        HeadId::TechDebt => 0.10,
        HeadId::Perf => 0.15,
        HeadId::Security => 0.20,
        HeadId::Accuracy => 0.10,
        HeadId::Cost => 0.05,
        HeadId::Reasoning => 0.10,
    }
}

fn head_metric(report: &EvalReport, head: HeadId) -> Option<f32> {
    match head {
        HeadId::Panel => {
            if report.gtau_pass_rate.is_empty() {
                None
            } else {
                Some(
                    report.gtau_pass_rate.values().sum::<f32>()
                        / report.gtau_pass_rate.len() as f32,
                )
            }
        }
        HeadId::Oracle => report
            .overall_correlation
            .map(|value| value.clamp(0.0, 1.0)),
        HeadId::FailureMode => report
            .q2_report_correlation
            .map(|value| value.clamp(0.0, 1.0)),
        HeadId::EdgeCase => report.q3_side_effect_agreement,
        HeadId::TechDebt => report.q3_side_effect_agreement,
        HeadId::Perf => metric_from_regression(report, "perf"),
        HeadId::Security => Some(report.q1_pass_rate),
        HeadId::Accuracy => report
            .q2_report_correlation
            .map(|value| value.clamp(0.0, 1.0)),
        HeadId::Cost => metric_from_regression(report, "cost"),
        HeadId::Reasoning => report.q3_side_effect_agreement,
    }
}

fn metric_from_regression(report: &EvalReport, needle: &str) -> Option<f32> {
    report
        .regression_checks
        .iter()
        .find(|check| check.name.contains(needle))
        .map(|check| (1.0 - check.drop.max(0.0)).clamp(0.0, 1.0))
}

fn latest_eval_report(storage: &HealRocksStore) -> Result<Option<EvalReport>, HealError> {
    let db = storage.db();
    let cf = db
        .cf_handle(context_graph_mejepa_cf::CF_MEJEPA_EVAL_REPORTS)
        .ok_or_else(|| HealError::invalid("lambda_ramp.eval_cf", "missing eval report CF"))?;
    let mut latest: Option<EvalReport> = None;
    for item in db.iterator_cf(cf, rocksdb::IteratorMode::Start) {
        let (_key, value) = item?;
        let report: EvalReport = bincode::deserialize(&value)?;
        report
            .validate()
            .map_err(|err| HealError::invalid("lambda_ramp.eval_report", err.to_string()))?;
        let replace = latest
            .as_ref()
            .map(|current| report.generated_at_unix_ms > current.generated_at_unix_ms)
            .unwrap_or(true);
        if replace {
            latest = Some(report);
        }
    }
    Ok(latest)
}

fn lambda_key(head: HeadId) -> Result<Vec<u8>, HealError> {
    policy_key(&["phase_e", "lambda", head.as_str()])
}
