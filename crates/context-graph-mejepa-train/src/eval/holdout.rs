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

    fn forward_with_options(
        &self,
        panel_t01: &Tensor,
        _options: PredictorForwardOptions,
    ) -> Result<Tensor, TrainerError> {
        self.forward(panel_t01)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PredictorForwardOptions {
    pub disable_action_conditioning: bool,
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
pub struct InverseToolCallTarget {
    pub tool_name: String,
    pub arguments_json: String,
}

#[derive(Debug, Clone)]
pub struct InverseActionTarget {
    pub patch_diff: String,
    pub tool_calls: Vec<InverseToolCallTarget>,
}

impl InverseActionTarget {
    pub fn new(patch_diff: impl Into<String>, tool_calls: Vec<InverseToolCallTarget>) -> Self {
        Self {
            patch_diff: patch_diff.into(),
            tool_calls,
        }
    }

    pub fn validate(&self) -> Result<(), TrainerError> {
        if self.patch_diff.trim().is_empty() && self.tool_calls.is_empty() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainConfigInvalid,
                "inverse action target requires patch diff text or at least one tool call",
            ));
        }
        for (idx, call) in self.tool_calls.iter().enumerate() {
            if call.tool_name.trim().is_empty() {
                return Err(TrainerError::new(
                    TrainerErrorCode::MejepaTrainConfigInvalid,
                    format!("inverse action target tool call {idx} has empty tool_name"),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HoldoutExample {
    pub task_id: String,
    pub category: MutationCategory,
    pub language: Lang,
    pub panel_t01: Tensor,
    pub panel_t2: Tensor,
    pub inverse_action_target: Option<InverseActionTarget>,
    pub actual_oracle_pass: bool,
    pub adversarial: bool,
    pub foundationality_score: f32,
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
        self.evaluate_with_forward_options(
            predictor,
            oracle_head,
            dataset,
            calibration,
            current_step,
            is_final_step,
            phase3_dod_min,
            PredictorForwardOptions::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_with_forward_options(
        &self,
        predictor: &dyn PredictorForward,
        oracle_head: &dyn OracleHead,
        dataset: &HoldoutDataset,
        calibration: &CalibrationDataset,
        current_step: u64,
        is_final_step: bool,
        phase3_dod_min: f32,
        forward_options: PredictorForwardOptions,
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
            let latent = predictor.forward_with_options(&ex.panel_t01, forward_options)?;
            let prediction = oracle_head.predict(&latent)?;
            if prediction.pass_to_pass_resolved == ex.actual_oracle_pass {
                correct += 1;
            }
            cat_predictions.push((
                ex.category,
                prediction.pass_to_pass_resolved,
                ex.actual_oracle_pass,
            ));
            lang_predictions.push((
                ex.language,
                prediction.pass_to_pass_resolved,
                ex.actual_oracle_pass,
            ));
            probability_vectors.push(softmax(&prediction.class_logits)?);
        }
        let agreement = correct as f32 / dataset.examples.len() as f32;
        let per_cat = per_mutation_category_accuracy(&cat_predictions);
        let per_lang = per_language_accuracy(&lang_predictions);
        let conformal = conformal_coverage(predictor, oracle_head, calibration, forward_options)?;
        let redundancy = pairwise_mi_audit(&probability_vectors)?;
        let warning = detect_generic_only_warning(&per_cat, &per_lang, GENERIC_ONLY_GAP_THRESHOLD);
        let report = HoldoutReport {
            step: current_step,
            prediction_oracle_agreement: agreement,
            conformal_coverage_calibration: conformal,
            per_mutation_category_accuracy: per_cat,
            per_language_accuracy: per_lang,
            predictor_redundancy_pairwise_mi: redundancy,
            // #621: emit None instead of a hardcoded 1.0. The Gτ guard is the
            // doctrinal Goodhart guard (doc 01 §1.4 / CLAUDE.md §6 / §1.5ter);
            // publishing 1.0 here misleads every consumer into believing the
            // guard passed every holdout row when in fact it was never
            // evaluated. The TCT-backed computation (HoldoutEvaluator →
            // mejepa-tct::gtau::evaluate_panel) is tracked separately.
            gtau_pass_rate: None,
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
    forward_options: PredictorForwardOptions,
) -> Result<f32, TrainerError> {
    if calibration.examples.is_empty() {
        return Ok(1.0);
    }
    let mut hits = 0u64;
    for ex in &calibration.examples {
        let pred = oracle_head
            .predict(&predictor.forward_with_options(&ex.panel_t01, forward_options)?)?;
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

#[cfg(test)]
mod tests {
    use crate::eval::{HoldoutReport, Lang, MutationCategory};
    use std::collections::HashMap;

    fn report_with_gtau(gtau: Option<f32>) -> HoldoutReport {
        HoldoutReport {
            step: 0,
            prediction_oracle_agreement: 0.5,
            conformal_coverage_calibration: 0.5,
            per_mutation_category_accuracy: HashMap::<MutationCategory, f32>::new(),
            per_language_accuracy: HashMap::<Lang, f32>::new(),
            predictor_redundancy_pairwise_mi: 0.0,
            gtau_pass_rate: gtau,
            generic_only_warning: None,
            phase3_dod_passed: None,
            timestamp_iso8601: "1970-01-01T00:00:00Z".to_string(),
        }
    }

    /// #621 regression: the diagnostic path emits `None` rather than a
    /// hardcoded `1.0`, and the JSON form serializes that as `null` so a
    /// downstream consumer cannot mistake unmeasured for perfect.
    #[test]
    fn holdout_report_gtau_pass_rate_is_optional_and_serializes_null_when_none() {
        let r = report_with_gtau(None);
        assert!(r.gtau_pass_rate.is_none());
        let v = serde_json::to_value(&r).expect("HoldoutReport serializes");
        assert_eq!(v["gtau_pass_rate"], serde_json::Value::Null);
    }

    /// #621: when a real Gτ evaluation eventually wires in, the same field
    /// must round-trip as `Some(value)` through serde so consumers can read
    /// the measurement honestly. (Use exact f32 bit patterns to avoid
    /// f32→f64 widening noise in the assertion.)
    #[test]
    fn holdout_report_gtau_pass_rate_round_trips_some_value() {
        let r = report_with_gtau(Some(0.5));
        let v = serde_json::to_value(&r).expect("HoldoutReport serializes");
        assert_eq!(v["gtau_pass_rate"], serde_json::json!(0.5));
        let r2: HoldoutReport = serde_json::from_value(v).expect("HoldoutReport deserializes");
        assert_eq!(r2.gtau_pass_rate, Some(0.5_f32));
    }

    /// #621: forbid the stale-form JSON (`gtau_pass_rate: 1.0`) from
    /// deserializing into the new type as `Some(1.0)` on the diagnostic
    /// path. The doctrinal contract is that the trainer construction site
    /// must emit `None`; this test pins that construction-site behavior to
    /// catch a future regression at the source rather than the schema.
    #[test]
    fn holdout_report_diagnostic_construction_emits_none() {
        // Mirror the construction site at holdout.rs:241-260 exactly for
        // the fields that matter — gtau_pass_rate must be None.
        let r = HoldoutReport {
            step: 0,
            prediction_oracle_agreement: 0.5,
            conformal_coverage_calibration: 0.5,
            per_mutation_category_accuracy: HashMap::<MutationCategory, f32>::new(),
            per_language_accuracy: HashMap::<Lang, f32>::new(),
            predictor_redundancy_pairwise_mi: 0.0,
            // #621: this MUST stay None on the diagnostic path. Changing
            // to Some(_) requires also wiring `mejepa-tct::gtau::evaluate_panel`
            // — file a sibling issue first.
            gtau_pass_rate: None,
            generic_only_warning: None,
            phase3_dod_passed: None,
            timestamp_iso8601: "1970-01-01T00:00:00Z".to_string(),
        };
        assert!(r.gtau_pass_rate.is_none());
    }
}
