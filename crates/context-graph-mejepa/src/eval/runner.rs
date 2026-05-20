use super::cell_exemptions::{default_cell_exemptions_path, load_cell_exemptions};
use super::convergence_tracker::compute_convergence_eta_from_reports;
use super::error::{EvalError, EvalErrorCode};
use super::metrics::{
    bayesian_shrinkage, compute_correlations, compute_failure_mode_class_metrics,
    compute_state_transfer_per_cell, conformal_health, gtau_pass_rate_by_language,
    ood_auc_by_language, oracle_target_opt, ship_gate_failures, state_transfer_from_observations,
    ShipGateInputs,
};
use super::per_head_calibration_tracker::compute_prediction_class_calibration;
use super::queue::ActiveLearningQueueState;
use super::report::seed_open_research_questions;
use super::store::RocksDbEvalStore;
use super::types::{
    ActiveLearningSummary, EvalConfig, EvalObservation, EvalProvenance, EvalReport, HoldoutPanel,
    RegressionCheck, StateTransferDiagnostic,
};
use crate::compiler::MeJepaCompiler;
use crate::heal::drift_attribution::{
    classify_failing_cell, FailingCellClassification, FailingCellSignature,
};
use crate::types::{FailedGate, RealityPrediction, TaskId, VerifyVerdict};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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
                actual_failure_modes: panel.actual_failure_modes.clone(),
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
        let per_prediction_class_calibration = compute_prediction_class_calibration(&observations)?;
        let per_failure_mode_class =
            compute_failure_mode_class_metrics(&observations, &self.config)?;
        let previous_report = self.store.load_latest_report()?;
        let regression_checks = regression_checks(previous_report.as_ref(), overall, &self.config);
        let q1 = q1_pass_rate(&observations)?;
        let q3 = q3_side_effect_agreement(&observations);
        let cell_exemptions = load_cell_exemptions(
            default_cell_exemptions_path(),
            chrono::Utc::now().timestamp_millis(),
        )?;
        let failures = ship_gate_failures(ShipGateInputs {
            observations: &observations,
            report_overall: overall,
            per_cell: &per_cell,
            cell_exemptions: &cell_exemptions,
            conformal: &conformal,
            ood: &ood,
            gtau: &gtau,
            per_failure_mode_class: &per_failure_mode_class,
            regression_checks: &regression_checks,
            q1_pass_rate: q1,
            q3_side_effect_agreement: q3,
            config: &self.config,
        });

        let mut queue = ActiveLearningQueueState::new(16)?;
        for obs in &observations {
            queue.enqueue_uncertain(obs, 2, 0.80)?;
        }
        self.store.persist_queue(&queue)?;

        let research = seed_open_research_questions();
        self.store.persist_research_questions(&research)?;
        let per_cell_state_transfer = compute_state_transfer_per_cell(&observations)?;
        let failing_cell_classifications = classify_report_failing_cells(
            &per_cell,
            &observations,
            &per_cell_state_transfer,
            &self.config,
        )?;

        let mut report = EvalReport {
            report_date: report_date.into(),
            generated_at_unix_ms: chrono::Utc::now().timestamp_millis(),
            rolling_window_size: self.config.rolling_window_size,
            holdout_count: observations.len(),
            overall_correlation: overall,
            per_category_correlation: per_category,
            per_language_correlation: per_language,
            per_cell_correlation: per_cell.clone(),
            cell_exemptions,
            bayesian_shrinkage: bayesian_shrinkage(&per_cell, overall, 4.0)?,
            conformal_coverage_health: conformal,
            ood_calibration_health: ood,
            gtau_pass_rate: gtau,
            per_prediction_class_calibration,
            per_failure_mode_class,
            per_cell_convergence_eta: BTreeMap::new(),
            active_learning: ActiveLearningSummary {
                queued_count: queue.entries.len(),
                evicted_count: queue.evicted.len(),
                ood_escalation_count: queue.ood_escalations.len(),
            },
            state_transfer_diagnostic: state_transfer_from_observations(&observations)?,
            per_cell_state_transfer,
            failing_cell_classifications,
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
        let mut convergence_history = self
            .store
            .load_recent_reports(self.config.convergence_history_windows.saturating_sub(1))?;
        convergence_history.push(report.clone());
        report.per_cell_convergence_eta = compute_convergence_eta_from_reports(
            &convergence_history,
            self.config.convergence_target_correlation,
            self.config.convergence_history_windows,
            self.config.convergence_eta_min_points,
        )?;
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
            let found = prediction_persisted(compiler, reality_prediction)?;
            Ok((true, true, found))
        }
        VerifyVerdict::EscalateToHuman {
            failed_gate,
            reality_prediction,
            ..
        } => {
            let live_prediction_readback = reality_prediction
                .as_ref()
                .map(|prediction| prediction_persisted(compiler, prediction))
                .transpose()?
                .unwrap_or(false);
            Ok((
                !matches!(
                    failed_gate,
                    FailedGate::ConstellationGuardRejected { .. }
                        | FailedGate::SafetyConstraintViolation { .. }
                ),
                false,
                live_prediction_readback,
            ))
        }
    }
}

fn prediction_persisted(
    compiler: &MeJepaCompiler,
    prediction: &RealityPrediction,
) -> Result<bool, EvalError> {
    let persisted = compiler
        .store()
        .read_live_predictions(prediction.session_id, 1000)?;
    Ok(persisted.iter().any(|candidate| {
        candidate.prediction_id == prediction.prediction_id
            && candidate.task_id == prediction.task_id
            && candidate.session_id == prediction.session_id
    }))
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

fn classify_report_failing_cells(
    per_cell: &BTreeMap<String, Option<f32>>,
    observations: &[EvalObservation],
    per_cell_state_transfer: &BTreeMap<String, Option<StateTransferDiagnostic>>,
    config: &EvalConfig,
) -> Result<BTreeMap<String, FailingCellClassification>, EvalError> {
    let mut counts = BTreeMap::<String, usize>::new();
    for observation in observations {
        let key = super::types::cell_key(observation.mutation_category, observation.language);
        *counts.entry(key).or_insert(0) += 1;
    }
    let mut classifications = BTreeMap::new();
    for (cell, correlation) in per_cell {
        if matches!(correlation, Some(value) if *value >= 0.95) {
            continue;
        }
        let distribution_shift_score = per_cell_state_transfer
            .get(cell)
            .and_then(|diagnostic| diagnostic.as_ref())
            .map(|diagnostic| diagnostic.wasserstein_1);
        let signature = FailingCellSignature {
            cell: cell.clone(),
            correlation: *correlation,
            holdout_count: counts.get(cell).copied().unwrap_or(0),
            min_holdout_count: config.min_samples_per_slice,
            embedder_pairwise_mi: None,
            blind_spot_z: None,
            label_disagreement_rate: None,
            oracle_flake_rate: None,
            distribution_shift_score,
        };
        let classification = classify_failing_cell(&signature).map_err(|err| {
            EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("failed to classify failing cell {cell}: {err}"),
            )
        })?;
        classifications.insert(cell.clone(), classification);
    }
    Ok(classifications)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::fixtures::{build_eval_compiler, synthetic_holdout};

    #[test]
    fn verification_state_accepts_persisted_escalations_for_replay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db_path = temp.path().join("rocksdb");
        let repo_root = temp.path().join("repo");
        let (_db, compiler, _eval_store, _calibration) =
            build_eval_compiler(&db_path, &repo_root).expect("eval compiler");
        let holdout = synthetic_holdout(&repo_root).expect("synthetic holdout");
        let panel = holdout
            .iter()
            .find(|panel| panel.task_id.0 == "phase8-predicted_failure_subtle_flip")
            .expect("predicted failure panel");

        let verdict = compiler
            .verify(&panel.patch, &panel.context)
            .expect("verify predicted failure panel");
        assert!(
            matches!(verdict, VerifyVerdict::EscalateToHuman { .. }),
            "predicted failure must produce an escalation verdict"
        );
        let (_gtau_passed, approved, live_prediction_readback) =
            verification_state(compiler.as_ref(), &verdict).expect("verification state");

        assert!(!approved, "predicted failure must escalate");
        assert!(
            live_prediction_readback,
            "compiled escalations must remain replayable from CF_MEJEPA_LIVE_PREDICTIONS"
        );
    }
}
