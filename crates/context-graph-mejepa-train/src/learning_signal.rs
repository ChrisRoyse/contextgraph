use crate::error::{TrainerError, TrainerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fmt;

pub mod classifier;
pub mod consolidation;
pub mod continual_backprop;
pub mod embedder_coherence;
pub mod pairwise_mi_matrix;
pub mod state_transfer;

pub use classifier::{
    classify_utml_state, prune_decision, ClassifierVerdict, Intervention, LStepWindow,
    PruneDecision, UtmlState,
};
pub use consolidation::{detect_dissipating, update_m_t, ConsolidationEvidence, ConsolidationMt};
pub use continual_backprop::{
    detect_dormant_units, reinit_dormant_units, DormantLayer, DormantUnitDetector,
    DormantUnitReport,
};
pub use embedder_coherence::{
    compute_embedder_coherence, ContentEmbedder, EmbedderCoherenceReport,
};
pub use pairwise_mi_matrix::{
    load_pairwise_mi_matrix_csv, PairwiseMiAuditor, PairwiseMiMatrix, PairwiseMiSummary,
};
pub use state_transfer::{
    compute_state_transfer, performance_deploy, DivergenceMetric, StateTransferDiagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UtmlErrorCode {
    InvalidSignal,
    MejepaInstrumentGradientLeak,
    NonFinite,
    OutOfRange,
    AmbiguousClassifierState,
    EmptyInput,
    Io,
    MissingSourceOfTruth,
    ReadbackMismatch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UtmlError {
    pub code: UtmlErrorCode,
    pub message: String,
}

impl UtmlError {
    pub fn new(code: UtmlErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self.code {
            UtmlErrorCode::InvalidSignal => "UTML_INVALID_SIGNAL",
            UtmlErrorCode::MejepaInstrumentGradientLeak => "MEJEPA_INSTRUMENT_GRADIENT_LEAK",
            UtmlErrorCode::NonFinite => "UTML_NON_FINITE",
            UtmlErrorCode::OutOfRange => "UTML_OUT_OF_RANGE",
            UtmlErrorCode::AmbiguousClassifierState => "UTML_AMBIGUOUS_CLASSIFIER_STATE",
            UtmlErrorCode::EmptyInput => "UTML_EMPTY_INPUT",
            UtmlErrorCode::Io => "UTML_IO",
            UtmlErrorCode::MissingSourceOfTruth => "UTML_MISSING_SOURCE_OF_TRUTH",
            UtmlErrorCode::ReadbackMismatch => "UTML_READBACK_MISMATCH",
        }
    }
}

impl fmt::Display for UtmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message)
    }
}

impl std::error::Error for UtmlError {}

impl From<UtmlError> for TrainerError {
    fn from(value: UtmlError) -> Self {
        let trainer_code = match value.code {
            UtmlErrorCode::MejepaInstrumentGradientLeak => {
                TrainerErrorCode::MejepaInstrumentGradientLeak
            }
            _ => TrainerErrorCode::MejepaTrainConfigInvalid,
        };
        TrainerError::new(trainer_code, format!("{}: {}", value.code(), value.message))
            .with_context(json!({"utml_code": value.code()}))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    None,
    LowSignal,
    Duplicate,
    MissingOracle,
    NonFinite,
    CertChainBroken,
    GradExplode,
    VramOom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct DeltaPComponents {
    pub oracle_disagreement: f32,
    pub prediction_residual: f32,
    pub uncertainty: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct DeltaKComponents {
    pub operator_override: f32,
    pub novel_failure_mode: f32,
    pub sparse_slice_bonus: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct DeltaOmegaComponents {
    pub runtime_delta: f32,
    pub warning_delta: f32,
    pub memory_delta: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct DeltaXiComponents {
    pub target_collapse: f32,
    pub predictor_collapse: f32,
    pub redundancy_mi: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningSignal {
    pub delta_p: f32,
    pub delta_k: f32,
    pub delta_omega: f32,
    pub delta_xi: f32,
    pub l_step: f32,
    pub delta_p_components: DeltaPComponents,
    pub delta_k_components: DeltaKComponents,
    pub delta_omega_components: DeltaOmegaComponents,
    pub delta_xi_components: DeltaXiComponents,
    pub skip_reason: SkipReason,
}

impl LearningSignal {
    pub fn components_as_map(&self) -> HashMap<String, f32> {
        HashMap::from([
            ("delta_p".to_string(), self.delta_p),
            ("delta_k".to_string(), self.delta_k),
            ("delta_omega".to_string(), self.delta_omega),
            ("delta_xi".to_string(), self.delta_xi),
            ("l_step".to_string(), self.l_step),
            (
                "target_collapse".to_string(),
                self.delta_xi_components.target_collapse,
            ),
        ])
    }

    pub fn validate(&self) -> Result<(), UtmlError> {
        for (name, value) in [
            ("delta_p", self.delta_p),
            ("delta_k", self.delta_k),
            ("delta_omega", self.delta_omega),
            ("delta_xi", self.delta_xi),
            ("l_step", self.l_step),
            ("target_collapse", self.delta_xi_components.target_collapse),
        ] {
            if !value.is_finite() {
                return Err(UtmlError::new(
                    UtmlErrorCode::NonFinite,
                    format!("{name} is non-finite"),
                ));
            }
            if !(0.0..=1.0).contains(&value) {
                return Err(UtmlError::new(
                    UtmlErrorCode::OutOfRange,
                    format!("{name}={value} is outside [0,1]"),
                ));
            }
        }
        Ok(())
    }
}

pub fn clamp01(value: f32) -> f32 {
    if value.is_nan() {
        value
    } else {
        value.clamp(0.0, 1.0)
    }
}

pub fn compute_delta_p(c: DeltaPComponents) -> Result<f32, UtmlError> {
    mean_checked(
        "delta_p",
        [c.oracle_disagreement, c.prediction_residual, c.uncertainty],
    )
}

pub fn compute_delta_k(c: DeltaKComponents) -> Result<f32, UtmlError> {
    mean_checked(
        "delta_k",
        [
            c.operator_override,
            c.novel_failure_mode,
            c.sparse_slice_bonus,
        ],
    )
}

pub fn compute_delta_omega(c: DeltaOmegaComponents) -> Result<f32, UtmlError> {
    mean_checked(
        "delta_omega",
        [c.runtime_delta, c.warning_delta, c.memory_delta],
    )
}

pub fn compute_delta_xi(c: DeltaXiComponents) -> Result<f32, UtmlError> {
    if !c.target_collapse.is_finite() {
        return Err(UtmlError::new(
            UtmlErrorCode::MejepaInstrumentGradientLeak,
            format!(
                "ME-JEPA target_collapse must be 0.0 by construction; got {}",
                c.target_collapse
            ),
        ));
    }
    if c.target_collapse != 0.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::MejepaInstrumentGradientLeak,
            format!(
                "ME-JEPA target_collapse must be 0.0 by construction; instrument has gradient flow; target_collapse={}, predictor_collapse={}, redundancy_mi={}",
                c.target_collapse, c.predictor_collapse, c.redundancy_mi
            ),
        ));
    }
    for (name, value) in [
        ("predictor_collapse", c.predictor_collapse),
        ("redundancy_mi", c.redundancy_mi),
    ] {
        if !value.is_finite() {
            return Err(UtmlError::new(
                UtmlErrorCode::NonFinite,
                format!("delta_xi {name} is non-finite"),
            ));
        }
        if !(0.0..=1.0).contains(&value) {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!("delta_xi {name}={value} outside [0,1]"),
            ));
        }
    }
    Ok(clamp01(
        (1.0 - c.target_collapse).powf(0.5)
            * (1.0 - c.predictor_collapse).powf(0.3)
            * (1.0 - c.redundancy_mi).powf(0.2),
    ))
}

pub fn compute_l_step(
    p: DeltaPComponents,
    k: DeltaKComponents,
    o: DeltaOmegaComponents,
    x: DeltaXiComponents,
) -> Result<LearningSignal, UtmlError> {
    let delta_p = compute_delta_p(p)?;
    let delta_k = compute_delta_k(k)?;
    let delta_omega = compute_delta_omega(o)?;
    let delta_xi = compute_delta_xi(x)?;
    let l_step = clamp01(delta_p * delta_k * delta_omega * delta_xi);
    let signal = LearningSignal {
        delta_p,
        delta_k,
        delta_omega,
        delta_xi,
        l_step,
        delta_p_components: p,
        delta_k_components: k,
        delta_omega_components: o,
        delta_xi_components: x,
        skip_reason: SkipReason::None,
    };
    signal.validate()?;
    Ok(signal)
}

pub fn sampling_weight(l_step: f32, operator_override: bool, age_days: f32, age_decay: f32) -> f32 {
    if !l_step.is_finite() || !age_days.is_finite() || !age_decay.is_finite() {
        return f32::NAN;
    }
    let override_factor = if operator_override { 6.0 } else { 1.0 };
    l_step.max(0.0) * override_factor * age_decay.powf(age_days.max(0.0))
}

pub fn should_drop(weight: f32, threshold: f32) -> bool {
    weight < threshold
}

pub fn should_force_include(weight: f32, threshold: f32) -> bool {
    weight > threshold
}

pub fn pairwise_mi_audit(probabilities: &[Vec<f32>]) -> Result<f32, UtmlError> {
    if probabilities.len() < 2 {
        return Ok(0.0);
    }
    let mut acc = 0.0f32;
    let mut pairs = 0usize;
    for i in 0..probabilities.len() {
        for j in (i + 1)..probabilities.len() {
            let a = &probabilities[i];
            let b = &probabilities[j];
            if a.len() != b.len() || a.is_empty() {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    "pairwise MI audit requires same non-empty vector dimensions",
                ));
            }
            let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
            if na == 0.0 || nb == 0.0 {
                continue;
            }
            acc += (dot / (na * nb)).abs();
            pairs += 1;
        }
    }
    Ok(if pairs == 0 {
        0.0
    } else {
        clamp01(acc / pairs as f32)
    })
}

fn mean_checked<const N: usize>(name: &'static str, values: [f32; N]) -> Result<f32, UtmlError> {
    for v in values {
        if !v.is_finite() {
            return Err(UtmlError::new(
                UtmlErrorCode::NonFinite,
                format!("{name} has non-finite component"),
            ));
        }
        if !(0.0..=1.0).contains(&v) {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!("{name} component {v} outside [0,1]"),
            ));
        }
    }
    Ok(values.iter().sum::<f32>() / N as f32)
}

pub(crate) fn validate_finite(name: &str, value: f32) -> Result<(), UtmlError> {
    if !value.is_finite() {
        tracing::error!(
            target: "context_graph_mejepa_train::utml",
            utml_code = "UTML_NON_FINITE",
            field = name,
            value = %value,
            "UTML finite-value validation failed"
        );
        return Err(UtmlError::new(
            UtmlErrorCode::NonFinite,
            format!("{name} is non-finite: {value}"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_unit(name: &str, value: f32) -> Result<(), UtmlError> {
    validate_finite(name, value)?;
    if !(0.0..=1.0).contains(&value) {
        tracing::error!(
            target: "context_graph_mejepa_train::utml",
            utml_code = "UTML_OUT_OF_RANGE",
            field = name,
            value = value,
            "UTML unit-interval validation failed"
        );
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            format!("{name}={value} outside [0,1]"),
        ));
    }
    Ok(())
}

pub(crate) fn non_empty<'a, T>(name: &str, values: &'a [T]) -> Result<&'a [T], UtmlError> {
    if values.is_empty() {
        tracing::error!(
            target: "context_graph_mejepa_train::utml",
            utml_code = "UTML_EMPTY_INPUT",
            field = name,
            "UTML required non-empty input"
        );
        return Err(UtmlError::new(
            UtmlErrorCode::EmptyInput,
            format!("{name} must be non-empty"),
        ));
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l_step_known_values() {
        let sig = compute_l_step(
            DeltaPComponents {
                oracle_disagreement: 1.0,
                prediction_residual: 0.5,
                uncertainty: 0.0,
            },
            DeltaKComponents::default(),
            DeltaOmegaComponents::default(),
            DeltaXiComponents::default(),
        )
        .unwrap();
        assert!((sig.delta_p - 0.5).abs() < 1e-6);
        assert_eq!(sig.l_step, 0.0);
    }

    #[test]
    fn sampling_weight_override_boosts() {
        let regular = sampling_weight(0.2, false, 0.0, 0.995);
        let forced = sampling_weight(0.2, true, 0.0, 0.995);
        assert_eq!(regular, 0.2);
        assert_eq!(forced, 1.2);
    }

    #[test]
    fn delta_xi_uses_multiplicative_non_collapse_formula() {
        let value = compute_delta_xi(DeltaXiComponents {
            target_collapse: 0.0,
            predictor_collapse: 0.1,
            redundancy_mi: 0.05,
        })
        .unwrap();
        let expected = 1.0f32 * 0.9f32.powf(0.3) * 0.95f32.powf(0.2);
        assert!((value - expected).abs() < 1e-6);
    }

    #[test]
    fn delta_xi_fails_closed_on_target_collapse() {
        let err = compute_delta_xi(DeltaXiComponents {
            target_collapse: 0.001,
            predictor_collapse: 0.0,
            redundancy_mi: 0.0,
        })
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENT_GRADIENT_LEAK");
    }

    #[test]
    fn delta_xi_fails_closed_on_nan_target_collapse() {
        let err = compute_delta_xi(DeltaXiComponents {
            target_collapse: f32::NAN,
            predictor_collapse: 0.0,
            redundancy_mi: 0.0,
        })
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENT_GRADIENT_LEAK");
    }
}
