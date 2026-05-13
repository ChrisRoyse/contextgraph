use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::error::{EvalError, EvalErrorCode};
use super::metrics::oracle_target_opt;
use super::types::{validate_unit, EvalObservation};
use crate::types::OracleOutcome;

pub const DEFAULT_CALIBRATION_BIN_COUNT: usize = 10;
pub const DEFAULT_ECE_TOLERANCE: f32 = 0.02;

const CLASS_ORACLE_PASS: &str = "q2_oracle_pass";
const CLASS_ORACLE_FAIL: &str = "q2_oracle_fail";
const CLASS_OOD_REJECTION: &str = "ood_rejection";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationSample {
    pub confidence: f32,
    pub actual: bool,
}

impl CalibrationSample {
    pub fn try_new(confidence: f32, actual: bool) -> Result<Self, EvalError> {
        validate_unit("calibration_sample.confidence", confidence)?;
        Ok(Self { confidence, actual })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PredictionClassCalibrationBin {
    pub lower_bound: f32,
    pub upper_bound: f32,
    pub sample_count: usize,
    pub mean_confidence: f32,
    pub empirical_accuracy: f32,
    pub calibration_error: f32,
}

impl PredictionClassCalibrationBin {
    pub fn validate(&self, name: &str) -> Result<(), EvalError> {
        validate_unit(&format!("{name}.lower_bound"), self.lower_bound)?;
        validate_unit(&format!("{name}.upper_bound"), self.upper_bound)?;
        if self.lower_bound > self.upper_bound {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name} lower_bound exceeds upper_bound"),
            ));
        }
        validate_unit(&format!("{name}.mean_confidence"), self.mean_confidence)?;
        validate_unit(
            &format!("{name}.empirical_accuracy"),
            self.empirical_accuracy,
        )?;
        validate_unit(&format!("{name}.calibration_error"), self.calibration_error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PredictionClassCalibration {
    pub class_name: String,
    pub sample_count: usize,
    pub bin_count: usize,
    pub expected_calibration_error: f32,
    pub mean_confidence: f32,
    pub empirical_accuracy: f32,
    pub within_tolerance: bool,
    pub bins: Vec<PredictionClassCalibrationBin>,
}

impl PredictionClassCalibration {
    pub fn validate(&self, name: &str) -> Result<(), EvalError> {
        validate_class_name(&self.class_name)?;
        if self.sample_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::EmptyHoldout,
                format!("{name}.sample_count must be greater than zero"),
            ));
        }
        if self.bin_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!("{name}.bin_count must be greater than zero"),
            ));
        }
        if self.bins.len() != self.bin_count {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "{name}.bins length {} does not match bin_count {}",
                    self.bins.len(),
                    self.bin_count
                ),
            ));
        }
        validate_unit(
            &format!("{name}.expected_calibration_error"),
            self.expected_calibration_error,
        )?;
        validate_unit(&format!("{name}.mean_confidence"), self.mean_confidence)?;
        validate_unit(
            &format!("{name}.empirical_accuracy"),
            self.empirical_accuracy,
        )?;
        for (idx, bin) in self.bins.iter().enumerate() {
            bin.validate(&format!("{name}.bins[{idx}]"))?;
        }
        Ok(())
    }
}

pub fn compute_prediction_class_calibration(
    observations: &[EvalObservation],
) -> Result<BTreeMap<String, PredictionClassCalibration>, EvalError> {
    if observations.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::EmptyHoldout,
            "cannot compute prediction-class calibration for empty observations",
        ));
    }

    let mut oracle_pass_samples = Vec::new();
    let mut oracle_fail_samples = Vec::new();
    let mut ood_samples = Vec::with_capacity(observations.len());

    for observation in observations {
        validate_unit(
            "prediction.predicted_oracle_pass",
            observation.prediction.predicted_oracle_pass,
        )?;
        validate_unit("prediction.ood_score", observation.prediction.ood_score)?;
        if let Some(actual) = oracle_target_opt(observation.actual_oracle) {
            let actual_pass = actual >= 0.5;
            oracle_pass_samples.push(CalibrationSample::try_new(
                observation.prediction.predicted_oracle_pass,
                actual_pass,
            )?);
            oracle_fail_samples.push(CalibrationSample::try_new(
                1.0 - observation.prediction.predicted_oracle_pass,
                !actual_pass,
            )?);
        }
        ood_samples.push(CalibrationSample::try_new(
            observation.prediction.ood_score,
            observation.actual_oracle == OracleOutcome::OutOfDistribution,
        )?);
    }

    let mut out = BTreeMap::new();
    if !oracle_pass_samples.is_empty() {
        insert_calibration(&mut out, CLASS_ORACLE_PASS, &oracle_pass_samples)?;
        insert_calibration(&mut out, CLASS_ORACLE_FAIL, &oracle_fail_samples)?;
    }
    insert_calibration(&mut out, CLASS_OOD_REJECTION, &ood_samples)?;
    Ok(out)
}

fn insert_calibration(
    out: &mut BTreeMap<String, PredictionClassCalibration>,
    class_name: &str,
    samples: &[CalibrationSample],
) -> Result<(), EvalError> {
    out.insert(
        class_name.to_string(),
        compute_calibration_for_samples(
            class_name,
            samples,
            DEFAULT_CALIBRATION_BIN_COUNT,
            DEFAULT_ECE_TOLERANCE,
        )?,
    );
    Ok(())
}

pub fn compute_calibration_for_samples(
    class_name: &str,
    samples: &[CalibrationSample],
    bin_count: usize,
    tolerance: f32,
) -> Result<PredictionClassCalibration, EvalError> {
    validate_class_name(class_name)?;
    if samples.is_empty() {
        return Err(EvalError::new(
            EvalErrorCode::EmptyHoldout,
            format!("calibration class {class_name} has no samples"),
        ));
    }
    if bin_count == 0 {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "calibration bin_count must be greater than zero",
        ));
    }
    validate_unit("calibration.ece_tolerance", tolerance)?;

    let mut bin_counts = vec![0usize; bin_count];
    let mut bin_confidence = vec![0.0f32; bin_count];
    let mut bin_actual = vec![0.0f32; bin_count];
    let mut confidence_sum = 0.0f32;
    let mut actual_sum = 0.0f32;

    for (idx, sample) in samples.iter().enumerate() {
        validate_unit(
            &format!("calibration.samples[{idx}].confidence"),
            sample.confidence,
        )?;
        let bin_idx = confidence_bin(sample.confidence, bin_count);
        bin_counts[bin_idx] += 1;
        bin_confidence[bin_idx] += sample.confidence;
        if sample.actual {
            bin_actual[bin_idx] += 1.0;
            actual_sum += 1.0;
        }
        confidence_sum += sample.confidence;
    }

    let sample_count = samples.len();
    let mut expected_calibration_error = 0.0f32;
    let mut bins = Vec::with_capacity(bin_count);
    for idx in 0..bin_count {
        let count = bin_counts[idx];
        let (mean_confidence, empirical_accuracy, calibration_error) = if count == 0 {
            (0.0, 0.0, 0.0)
        } else {
            let mean = bin_confidence[idx] / count as f32;
            let accuracy = bin_actual[idx] / count as f32;
            let error = (mean - accuracy).abs();
            expected_calibration_error += (count as f32 / sample_count as f32) * error;
            (mean, accuracy, error)
        };
        bins.push(PredictionClassCalibrationBin {
            lower_bound: idx as f32 / bin_count as f32,
            upper_bound: (idx + 1) as f32 / bin_count as f32,
            sample_count: count,
            mean_confidence,
            empirical_accuracy,
            calibration_error,
        });
    }

    let calibration = PredictionClassCalibration {
        class_name: class_name.to_string(),
        sample_count,
        bin_count,
        expected_calibration_error,
        mean_confidence: confidence_sum / sample_count as f32,
        empirical_accuracy: actual_sum / sample_count as f32,
        within_tolerance: expected_calibration_error <= tolerance,
        bins,
    };
    calibration.validate("prediction_class_calibration")?;
    Ok(calibration)
}

fn confidence_bin(confidence: f32, bin_count: usize) -> usize {
    if confidence >= 1.0 {
        return bin_count - 1;
    }
    ((confidence * bin_count as f32).floor() as usize).min(bin_count - 1)
}

fn validate_class_name(class_name: &str) -> Result<(), EvalError> {
    if class_name.trim().is_empty() || class_name.chars().any(char::is_control) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "calibration class_name must be non-empty and contain no control characters",
        ));
    }
    Ok(())
}
