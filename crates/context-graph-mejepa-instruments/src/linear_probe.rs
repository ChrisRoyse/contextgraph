use crate::{InstrumentError, InstrumentResult};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const LINEAR_PROBE_MIN_ACCURACY: f32 = 0.70;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeReport {
    pub sample_count: usize,
    pub train_count: usize,
    pub holdout_count: usize,
    pub feature_dim: usize,
    pub class_count: usize,
    pub top1_accuracy: f32,
    pub confusion: BTreeMap<u8, BTreeMap<u8, u64>>,
    pub passes: bool,
}

pub fn train_linear_probe(
    features: &[Vec<f32>],
    labels: &[u8],
    holdout_frac: f32,
    seed: u64,
) -> InstrumentResult<ProbeReport> {
    validate_dataset(features, labels, holdout_frac)?;
    let dim = features[0].len();
    let mut train = Vec::new();
    let mut holdout = Vec::new();
    for idx in 0..features.len() {
        let bucket = ((idx as u64).wrapping_mul(1_103_515_245).wrapping_add(seed)) % 10_000;
        if (bucket as f32) < holdout_frac * 10_000.0 {
            holdout.push(idx);
        } else {
            train.push(idx);
        }
    }
    if train.is_empty() || holdout.is_empty() {
        return Err(InstrumentError::invalid(
            "linear_probe.split",
            format!(
                "train/holdout split is empty: train={}, holdout={}",
                train.len(),
                holdout.len()
            ),
            "use a larger labeled dataset or a holdout fraction in the supported range",
        ));
    }

    let classes: BTreeSet<u8> = labels.iter().copied().collect();
    let mut sums: BTreeMap<u8, Vec<f32>> = classes.iter().map(|c| (*c, vec![0.0; dim])).collect();
    let mut counts: BTreeMap<u8, usize> = classes.iter().map(|c| (*c, 0)).collect();
    for idx in &train {
        let label = labels[*idx];
        let sum = sums.get_mut(&label).ok_or_else(|| {
            InstrumentError::invalid(
                "linear_probe.labels",
                format!("label {label} missing from centroid table"),
                "inspect label construction before training",
            )
        })?;
        for (slot, value) in sum.iter_mut().zip(features[*idx].iter()) {
            *slot += *value;
        }
        *counts.get_mut(&label).expect("class count exists") += 1;
    }
    for class in &classes {
        let count = *counts.get(class).expect("class count exists");
        if count == 0 {
            return Err(InstrumentError::invalid(
                "linear_probe.split",
                format!("class {class} has no training examples"),
                "stratify the holdout or supply more labeled examples",
            ));
        }
        for value in sums.get_mut(class).expect("class sum exists") {
            *value /= count as f32;
        }
    }

    let mut correct = 0usize;
    let mut confusion: BTreeMap<u8, BTreeMap<u8, u64>> = BTreeMap::new();
    for idx in &holdout {
        let expected = labels[*idx];
        let predicted = predict(&features[*idx], &sums)?;
        if predicted == expected {
            correct += 1;
        }
        *confusion
            .entry(expected)
            .or_default()
            .entry(predicted)
            .or_default() += 1;
    }
    let top1_accuracy = correct as f32 / holdout.len() as f32;
    Ok(ProbeReport {
        sample_count: features.len(),
        train_count: train.len(),
        holdout_count: holdout.len(),
        feature_dim: dim,
        class_count: classes.len(),
        top1_accuracy,
        confusion,
        passes: top1_accuracy >= LINEAR_PROBE_MIN_ACCURACY,
    })
}

pub fn assert_probe_passes(report: &ProbeReport) -> InstrumentResult<()> {
    if report.passes {
        Ok(())
    } else {
        Err(InstrumentError::invalid(
            "linear_probe.top1_accuracy",
            format!(
                "top1_accuracy {:.4} is below required {:.4}",
                report.top1_accuracy, LINEAR_PROBE_MIN_ACCURACY
            ),
            "inspect the instrument vectors and mutation labels; do not promote the instrument",
        ))
    }
}

fn validate_dataset(
    features: &[Vec<f32>],
    labels: &[u8],
    holdout_frac: f32,
) -> InstrumentResult<()> {
    if features.len() != labels.len() {
        return Err(InstrumentError::invalid(
            "linear_probe.dataset",
            format!(
                "features/labels length mismatch: {} vs {}",
                features.len(),
                labels.len()
            ),
            "pass one label for every feature vector",
        ));
    }
    if features.len() < 4 {
        return Err(InstrumentError::invalid(
            "linear_probe.dataset",
            format!("at least 4 examples required, got {}", features.len()),
            "supply a real labeled holdout with at least two classes",
        ));
    }
    if !(0.05..=0.95).contains(&holdout_frac) || !holdout_frac.is_finite() {
        return Err(InstrumentError::invalid(
            "linear_probe.holdout_frac",
            format!("holdout_frac must be finite in [0.05, 0.95], got {holdout_frac}"),
            "choose a holdout split that leaves train and holdout examples",
        ));
    }
    let dim = features.first().map(Vec::len).ok_or_else(|| {
        InstrumentError::invalid(
            "linear_probe.dataset",
            "no feature vectors supplied",
            "run real instrument encoders before training the probe",
        )
    })?;
    if dim == 0 {
        return Err(InstrumentError::invalid(
            "linear_probe.feature_dim",
            "feature vectors are empty",
            "use the fixed-dimension instrument outputs",
        ));
    }
    let mut classes = BTreeSet::new();
    for (row, feature) in features.iter().enumerate() {
        if feature.len() != dim {
            return Err(InstrumentError::invalid(
                "linear_probe.feature_dim",
                format!(
                    "feature row {row} has dim {}, expected {dim}",
                    feature.len()
                ),
                "do not mix instrument slots or schema versions in one probe",
            ));
        }
        for (col, value) in feature.iter().enumerate() {
            if !value.is_finite() {
                return Err(InstrumentError::invariant(
                    "linear_probe.features",
                    format!("features[{row}][{col}] is non-finite: {value}"),
                    "reject NaN/Inf before probe training",
                ));
            }
        }
        classes.insert(labels[row]);
    }
    if classes.len() < 2 {
        return Err(InstrumentError::invalid(
            "linear_probe.labels",
            "at least two labels are required",
            "supply a real classification holdout, not a single-class set",
        ));
    }
    Ok(())
}

fn predict(feature: &[f32], centroids: &BTreeMap<u8, Vec<f32>>) -> InstrumentResult<u8> {
    let mut best = None;
    for (class, centroid) in centroids {
        let score = dot(feature, centroid)?;
        if best
            .as_ref()
            .map(|(_, best_score)| score > *best_score)
            .unwrap_or(true)
        {
            best = Some((*class, score));
        }
    }
    best.map(|(class, _)| class).ok_or_else(|| {
        InstrumentError::invalid(
            "linear_probe.centroids",
            "no centroids were trained",
            "inspect training split and labels",
        )
    })
}

fn dot(left: &[f32], right: &[f32]) -> InstrumentResult<f32> {
    if left.len() != right.len() {
        return Err(InstrumentError::invalid(
            "linear_probe.dot",
            format!(
                "dot-product dim mismatch: {} vs {}",
                left.len(),
                right.len()
            ),
            "use feature vectors from one instrument slot",
        ));
    }
    Ok(left.iter().zip(right.iter()).map(|(a, b)| a * b).sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_probe_separates_real_vectors() {
        let mut features = Vec::new();
        let mut labels = Vec::new();
        for i in 0..20 {
            if i % 2 == 0 {
                features.push(vec![1.0, 0.0, i as f32 / 100.0]);
                labels.push(0);
            } else {
                features.push(vec![0.0, 1.0, i as f32 / 100.0]);
                labels.push(1);
            }
        }
        let report = train_linear_probe(&features, &labels, 0.3, 7).unwrap();
        assert_probe_passes(&report).unwrap();
        assert!(report.top1_accuracy >= 0.70);
    }

    #[test]
    fn linear_probe_rejects_broken_datasets() {
        assert_eq!(
            train_linear_probe(&[vec![1.0]], &[0, 1], 0.2, 0)
                .unwrap_err()
                .code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
        let single_class = vec![vec![1.0]; 4];
        assert_eq!(
            train_linear_probe(&single_class, &[0, 0, 0, 0], 0.2, 0)
                .unwrap_err()
                .code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}
