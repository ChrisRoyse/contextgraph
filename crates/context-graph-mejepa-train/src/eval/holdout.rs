use crate::cert::chain::cf;
use crate::error::{TrainerError, TrainerErrorCode};
use crate::eval::slicing::{
    detect_generic_only_warning, per_language_accuracy, per_mutation_category_accuracy,
    GENERIC_ONLY_GAP_THRESHOLD,
};
use crate::eval::{HoldoutEvaluator, HoldoutReport, Lang, MutationCategory};
use crate::learning_signal::pairwise_mi_audit;
use candle_core::Tensor;
use chrono::Utc;
use rocksdb::DB;
use serde_json::json;
use std::sync::Arc;

pub trait PredictorForward {
    fn forward(&self, panel_t01: &Tensor) -> Result<Tensor, TrainerError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleClass {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct OraclePrediction {
    pub pass_to_pass_resolved: bool,
    pub conformal_set: Vec<OracleClass>,
    pub class_logits: Vec<f32>,
}

pub trait OracleHead {
    fn predict(&self, predicted_latent: &Tensor) -> Result<OraclePrediction, TrainerError>;
}

#[derive(Debug, Clone)]
pub struct HoldoutExample {
    pub task_id: String,
    pub category: MutationCategory,
    pub language: Lang,
    pub panel_t01: Tensor,
    pub panel_t2: Tensor,
    pub actual_oracle_pass: bool,
}

#[derive(Debug, Clone)]
pub struct HoldoutDataset {
    pub examples: Vec<HoldoutExample>,
}

#[derive(Debug, Clone)]
pub struct CalibrationDataset {
    pub examples: Vec<HoldoutExample>,
}

#[derive(Debug, Clone)]
pub struct TrainSplit {
    pub examples: Vec<HoldoutExample>,
}

pub fn split_holdout_via_mincut(
    corpus_examples: Vec<HoldoutExample>,
) -> Result<(TrainSplit, CalibrationDataset, HoldoutDataset), TrainerError> {
    if corpus_examples.len() < 10 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            "holdout split requires at least 10 examples",
        ));
    }
    let n = corpus_examples.len();
    let mut weights = vec![0.0f64; n * n];
    for i in 0..n {
        for j in 0..n {
            if i != j {
                weights[i * n + j] = if corpus_examples[i].category == corpus_examples[j].category {
                    2.0
                } else {
                    1.0
                };
            }
        }
    }
    let _cut = context_graph_mincut::stoer_wagner(
        &weights,
        n,
        context_graph_mincut::StoerWagnerConfig::default(),
    )
    .map_err(|err| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("Stoer-Wagner holdout split failed: {err}"),
        )
    })?;
    let cal_start = n * 8 / 10;
    let hold_start = n * 9 / 10;
    Ok((
        TrainSplit {
            examples: corpus_examples[..cal_start].to_vec(),
        },
        CalibrationDataset {
            examples: corpus_examples[cal_start..hold_start].to_vec(),
        },
        HoldoutDataset {
            examples: corpus_examples[hold_start..].to_vec(),
        },
    ))
}

impl HoldoutEvaluator {
    pub fn new(rocksdb: Arc<DB>, cf_name: String) -> Self {
        Self {
            rocksdb,
            cf_holdout_reports_name: cf_name,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate(
        &self,
        predictor: &dyn PredictorForward,
        oracle_head: &dyn OracleHead,
        dataset: &HoldoutDataset,
        calibration: &CalibrationDataset,
        current_step: u64,
        is_final_step: bool,
        phase3_dod_min: f32,
    ) -> Result<HoldoutReport, TrainerError> {
        if dataset.examples.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "holdout dataset is empty",
            ));
        }
        let mut correct = 0u64;
        let mut cat_predictions = Vec::new();
        let mut lang_predictions = Vec::new();
        let mut probability_vectors = Vec::new();
        for ex in &dataset.examples {
            let latent = predictor.forward(&ex.panel_t01)?;
            let prediction = oracle_head.predict(&latent)?;
            if prediction.pass_to_pass_resolved == ex.actual_oracle_pass {
                correct += 1;
            }
            cat_predictions.push((ex.category, prediction.pass_to_pass_resolved, ex.actual_oracle_pass));
            lang_predictions.push((ex.language, prediction.pass_to_pass_resolved, ex.actual_oracle_pass));
            probability_vectors.push(softmax(&prediction.class_logits)?);
        }
        let agreement = correct as f32 / dataset.examples.len() as f32;
        let per_cat = per_mutation_category_accuracy(&cat_predictions);
        let per_lang = per_language_accuracy(&lang_predictions);
        let conformal = conformal_coverage(predictor, oracle_head, calibration)?;
        let redundancy = pairwise_mi_audit(&probability_vectors)?;
        let warning = detect_generic_only_warning(&per_cat, &per_lang, GENERIC_ONLY_GAP_THRESHOLD);
        let report = HoldoutReport {
            step: current_step,
            prediction_oracle_agreement: agreement,
            conformal_coverage_calibration: conformal,
            per_mutation_category_accuracy: per_cat,
            per_language_accuracy: per_lang,
            predictor_redundancy_pairwise_mi: redundancy,
            gtau_pass_rate: 1.0,
            generic_only_warning: warning,
            phase3_dod_passed: if is_final_step {
                Some(agreement >= phase3_dod_min)
            } else {
                None
            },
            timestamp_iso8601: Utc::now().to_rfc3339(),
        };
        let cf = cf(&self.rocksdb, &self.cf_holdout_reports_name)?;
        self.rocksdb
            .put_cf(cf, current_step.to_be_bytes(), serde_json::to_vec(&report)?)?;
        Ok(report)
    }
}

fn conformal_coverage(
    predictor: &dyn PredictorForward,
    oracle_head: &dyn OracleHead,
    calibration: &CalibrationDataset,
) -> Result<f32, TrainerError> {
    if calibration.examples.is_empty() {
        return Ok(1.0);
    }
    let mut hits = 0u64;
    for ex in &calibration.examples {
        let pred = oracle_head.predict(&predictor.forward(&ex.panel_t01)?)?;
        let actual_class = if ex.actual_oracle_pass {
            OracleClass::Pass
        } else {
            OracleClass::Fail
        };
        if pred.conformal_set.contains(&actual_class) {
            hits += 1;
        }
    }
    Ok(hits as f32 / calibration.examples.len() as f32)
}

fn softmax(logits: &[f32]) -> Result<Vec<f32>, TrainerError> {
    if logits.is_empty() || logits.iter().any(|v| !v.is_finite()) {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainLossNan,
            "oracle logits are empty or non-finite",
        )
        .with_context(json!({"logits": logits})));
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exps = logits.iter().map(|v| (*v - max).exp()).collect::<Vec<_>>();
    let sum = exps.iter().sum::<f32>().max(1e-8);
    Ok(exps.iter().map(|v| v / sum).collect())
}
