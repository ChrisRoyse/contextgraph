use super::{non_empty, validate_finite, UtmlError, UtmlErrorCode};
use context_graph_mejepa_corpus::prng::SplitMix64;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DormantLayer {
    pub activations: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DormantUnitDetector {
    pub activation_threshold: f32,
    pub dormant_fraction_threshold: f32,
}

impl DormantUnitDetector {
    pub fn validate(&self) -> Result<(), UtmlError> {
        if !self.activation_threshold.is_finite() || self.activation_threshold < 0.0 {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!(
                    "activation_threshold must be finite and non-negative; got {}",
                    self.activation_threshold
                ),
            ));
        }
        if !self.dormant_fraction_threshold.is_finite()
            || !(0.0..=1.0).contains(&self.dormant_fraction_threshold)
        {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!(
                    "dormant_fraction_threshold must be in [0,1]; got {}",
                    self.dormant_fraction_threshold
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DormantUnitReport {
    pub sample_count: usize,
    pub unit_count: usize,
    pub dormant_units: Vec<usize>,
    pub dormant_fraction: f32,
    pub exceeds_threshold: bool,
}

pub fn detect_dormant_units(
    detector: &DormantUnitDetector,
    layer: &DormantLayer,
) -> Result<DormantUnitReport, UtmlError> {
    detector.validate()?;
    non_empty("layer.activations", &layer.activations)?;
    let unit_count = layer.activations[0].len();
    if unit_count == 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::EmptyInput,
            "activation rows must have at least one unit",
        ));
    }
    let mut mean_abs = vec![0.0f32; unit_count];
    for (row_idx, row) in layer.activations.iter().enumerate() {
        if row.len() != unit_count {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "activation row {row_idx} has {} units; expected {unit_count}",
                    row.len()
                ),
            ));
        }
        for (unit_idx, value) in row.iter().enumerate() {
            validate_finite(&format!("activations[{row_idx}][{unit_idx}]"), *value)?;
            mean_abs[unit_idx] += value.abs();
        }
    }
    let sample_count = layer.activations.len();
    let dormant_units = mean_abs
        .iter()
        .enumerate()
        .filter_map(|(idx, total)| {
            let mean = *total / sample_count as f32;
            (mean <= detector.activation_threshold).then_some(idx)
        })
        .collect::<Vec<_>>();
    let dormant_fraction = dormant_units.len() as f32 / unit_count as f32;
    Ok(DormantUnitReport {
        sample_count,
        unit_count,
        exceeds_threshold: dormant_fraction >= detector.dormant_fraction_threshold,
        dormant_units,
        dormant_fraction,
    })
}

pub fn reinit_dormant_units(
    weights: &mut [Vec<f32>],
    biases: &mut [f32],
    dormant_units: &[usize],
    seed: u64,
) -> Result<(), UtmlError> {
    non_empty("weights", weights)?;
    non_empty("biases", biases)?;
    if weights.len() != biases.len() {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "weights row count {} must equal bias count {}",
                weights.len(),
                biases.len()
            ),
        ));
    }
    let fan_in = weights[0].len();
    if fan_in == 0 {
        return Err(UtmlError::new(
            UtmlErrorCode::EmptyInput,
            "weights rows must be non-empty",
        ));
    }
    for (row_idx, row) in weights.iter().enumerate() {
        if row.len() != fan_in {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!("weights[{row_idx}] len {} expected {fan_in}", row.len()),
            ));
        }
        for (col_idx, value) in row.iter().enumerate() {
            validate_finite(&format!("weights[{row_idx}][{col_idx}]"), *value)?;
        }
    }
    for (idx, value) in biases.iter().enumerate() {
        validate_finite(&format!("biases[{idx}]"), *value)?;
    }
    non_empty("dormant_units", dormant_units)?;
    let fan_out = weights.len();
    let bound = (6.0f32 / (fan_in + fan_out) as f32).sqrt();
    let mut rng = SplitMix64::new(seed);
    for &unit in dormant_units {
        if unit >= weights.len() {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!(
                    "dormant unit index {unit} >= weights rows {}",
                    weights.len()
                ),
            ));
        }
        for value in &mut weights[unit] {
            *value = rng.next_f32_signed() * bound;
        }
        biases[unit] = 0.0;
    }
    Ok(())
}
