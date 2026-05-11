use super::error::{EvalError, EvalErrorCode};
use super::metrics::{
    bayesian_shrinkage, compute_correlations, conformal_health, gtau_pass_rate_by_language,
    ood_auc_by_language, oracle_target_opt, ship_gate_failures,
};
use super::queue::ActiveLearningQueueState;
use super::report::seed_open_research_questions;
use super::store::RocksDbEvalStore;
use super::types::{
    ActiveLearningSummary, EvalConfig, EvalObservation, EvalProvenance, EvalReport, HoldoutPanel,
    RegressionCheck, StateTransferDiagnostic,
};
use crate::compiler::MeJepaCompiler;
use crate::types::{FailedGate, TaskId, VerifyVerdict};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Instant;

pub struct EvalRunner {
    compiler: Arc<MeJepaCompiler>,
    store: RocksDbEvalStore,
    config: EvalConfig,
}

impl EvalRunner {
    pub fn new(
        compiler: Arc<MeJepaCompiler>,
        store: RocksDbEvalStore,
        config: EvalConfig,
    ) -> Result<Self, EvalError> {
        config.validate()?;
        Ok(Self {
            compiler,
            store,
            config,
        })
    }

    pub fn run_holdout_eval(
        &self,
        holdout: &[HoldoutPanel],
        train_task_ids: &[TaskId],
        report_date: impl Into<String>,
        provenance: EvalProvenance,
    ) -> Result<EvalReport, EvalError> {
        if holdout.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::EmptyHoldout,
                "holdout panel set is empty",
            ));
        }
        let started = Instant::now();
        let train_ids = train_task_ids.iter().collect::<BTreeSet<_>>();
        let mut observations = Vec::with_capacity(holdout.len());
        for panel in holdout {
            panel.validate()?;
            if train_ids.contains(&panel.task_id) {
                return Err(EvalError::new(
                    EvalErrorCode::LeakageDetected,
                    format!("holdout task {} is present in train ids", panel.task_id.0),
                ));
            }
            let compile_start = Instant::now();
            let prediction = self.compiler.compile(&panel.patch, &panel.context)?;
            let latency_ms = compile_start.elapsed().as_secs_f32() * 1000.0;
            let verdict = self.compiler.verify(&panel.patch, &panel.context)?;
            let (gtau_passed, approved, live_prediction_readback) =
                verification_state(self.compiler.as_ref(), &verdict)?;
            observations.push(EvalObservation {
                task_id: panel.task_id.clone(),
                mutation_category: panel.mutation_category,
                language: panel.language,
                actual_oracle: panel.actual_oracle,
                prediction,
                gtau_passed,
                approved,
                live_prediction_readback,
                latency_ms,
            });
        }

        let (overall, per_category, per_language, per_cell) =
            compute_correlations(&observations, &self.config)?;
        let conformal = conformal_health(&observations, &self.config)?;
        let ood = ood_auc_by_language(&observations)?;
        let gtau = gtau_pass_rate_by_language(&observations)?;
        let previous_report = self.store.load_latest_report()?;
        let regression_checks = regression_checks(previous_report.as_ref(), overall, &self.config);
        let q1 = q1_pass_rate(&observations)?;
        let q3 = q3_side_effect_agreement(&observations);
        let failures = ship_gate_failures(
            &observations,
            overall,
            &conformal,
            &ood,
            &gtau,
            &regression_checks,
            q1,
            q3,
            &self.config,
        );

        let mut queue = ActiveLearningQueueState::new(16)?;
        for obs in &observations {
            queue.enqueue_uncertain(obs, 2, 0.80)?;
        }
        self.store.persist_queue(&queue)?;

        let research = seed_open_research_questions();
        self.store.persist_research_questions(&research)?;

        let report = EvalReport {
            report_date: report_date.into(),
            generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            rolling_window_size: self.config.rolling_window_size,
            holdout_count: observations.len(),
            overall_correlation: overall,
            per_category_correlation: per_category,
            per_language_correlation: per_language,
            per_cell_correlation: per_cell.clone(),
            bayesian_shrinkage: bayesian_shrinkage(&per_cell, overall, 4.0)?,
            conformal_coverage_health: conformal,
            ood_calibration_health: ood,
            gtau_pass_rate: gtau,
            active_learning: ActiveLearningSummary {
                queued_count: queue.entries.len(),
                evicted_count: queue.evicted.len(),
                ood_escalation_count: queue.ood_escalations.len(),
            },
            state_transfer_diagnostic: state_transfer_from_observations(&observations)?,
            aux_head_distillation: None,
            regression_checks,
            open_research_questions: research,
            q1_pass_rate: q1,
            q2_report_correlation: overall,
            q3_side_effect_agreement: q3,
            ship_gate_passed: failures.is_empty(),
            ship_gate_failures: failures,
            provenance,
            wall_clock_seconds: started.elapsed().as_secs_f32(),
        };
        report.validate()?;
        self.store.persist_report(&report)?;
        Ok(report)
    }
}

fn verification_state(
    compiler: &MeJepaCompiler,
    verdict: &VerifyVerdict,
) -> Result<(bool, bool, bool), EvalError> {
    match verdict {
        VerifyVerdict::Approve {
            reality_prediction, ..
        } => {
            let persisted = compiler
                .store()
                .read_live_predictions(reality_prediction.session_id, 1000)?;
            let found = persisted.iter().any(|candidate| {
                candidate.prediction_id == reality_prediction.prediction_id
                    && candidate.task_id == reality_prediction.task_id
                    && candidate.session_id == reality_prediction.session_id
            });
            Ok((true, true, found))
        }
        VerifyVerdict::EscalateToHuman {
            failed_gate,
            reality_prediction,
            ..
        } => {
            if let Some(prediction) = reality_prediction {
                let persisted = compiler
                    .store()
                    .read_live_predictions(prediction.session_id, 1000)?;
                if persisted.iter().any(|candidate| {
                    candidate.prediction_id == prediction.prediction_id
                        && candidate.task_id == prediction.task_id
                        && candidate.session_id == prediction.session_id
                }) {
                    return Err(EvalError::new(
                        EvalErrorCode::ReadbackMismatch,
                        format!(
                            "escalated prediction {} was present in live-prediction source of truth",
                            prediction.task_id.0
                        ),
                    ));
                }
            }
            Ok((
                !matches!(failed_gate, FailedGate::ConstellationGuardRejected { .. }),
                false,
                false,
            ))
        }
    }
}

fn q1_pass_rate(observations: &[EvalObservation]) -> Result<f32, EvalError> {
    if observations.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::EmptyHoldout,
            "cannot compute q1_pass_rate for empty observations",
        ));
    }
    let passed = observations
        .iter()
        .filter(|obs| {
            oracle_target_opt(obs.actual_oracle)
                .map(|actual| (obs.prediction.predicted_oracle_pass >= 0.5) == (actual >= 0.5))
                .unwrap_or(true)
        })
        .count();
    Ok(passed as f32 / observations.len() as f32)
}

fn state_transfer_from_observations(
    observations: &[EvalObservation],
) -> Result<Option<StateTransferDiagnostic>, EvalError> {
    let predicted = observations
        .iter()
        .filter_map(|obs| {
            oracle_target_opt(obs.actual_oracle).map(|_| obs.prediction.predicted_oracle_pass)
        })
        .collect::<Vec<_>>();
    let actual = observations
        .iter()
        .filter_map(|obs| oracle_target_opt(obs.actual_oracle))
        .collect::<Vec<_>>();
    if predicted.len() < 2 || predicted.len() != actual.len() {
        return Ok(None);
    }
    let mut p = predicted.clone();
    let mut a = actual.clone();
    p.sort_by(f32::total_cmp);
    a.sort_by(f32::total_cmp);
    let w1 = p.iter().zip(&a).map(|(x, y)| (x - y).abs()).sum::<f32>() / p.len() as f32;
    let transfer = (-w1 / 0.5).exp().clamp(0.0, 1.0);
    Ok(Some(StateTransferDiagnostic {
        wasserstein_1: w1,
        transfer_score: transfer,
        performance_deploy: transfer * predicted.iter().sum::<f32>() / predicted.len() as f32,
    }))
}

fn q3_side_effect_agreement(observations: &[EvalObservation]) -> Option<f32> {
    let approved = observations.iter().filter(|obs| obs.approved).count();
    if approved == 0 {
        return None;
    }
    let readback = observations
        .iter()
        .filter(|obs| obs.approved && obs.live_prediction_readback)
        .count();
    Some(readback as f32 / approved as f32)
}

fn regression_checks(
    previous_report: Option<&EvalReport>,
    overall: Option<f32>,
    config: &EvalConfig,
) -> Vec<RegressionCheck> {
    let current = overall.unwrap_or(0.0);
    let Some(previous_report) = previous_report else {
        return vec![RegressionCheck {
            name: "overall_correlation_no_previous_report".to_string(),
            previous: current,
            current,
            drop: 0.0,
            passed: false,
        }];
    };
    let previous = previous_report.overall_correlation.unwrap_or(0.0);
    let drop = (previous - current).max(0.0);
    vec![RegressionCheck {
        name: "overall_correlation".to_string(),
        previous,
        current,
        drop,
        passed: drop <= config.regression_max_drop,
    }]
}

pub fn corpus_sha_from_holdout(holdout: &[HoldoutPanel]) -> String {
    let mut hasher = Sha256::new();
    for panel in holdout {
        hasher.update(panel.task_id.0.as_bytes());
        hasher.update(panel.patch.patch_sha);
        hasher.update(panel.panel_sha);
    }
    hex::encode(hasher.finalize())
}
