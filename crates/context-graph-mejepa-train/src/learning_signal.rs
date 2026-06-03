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
pub mod per_head;
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
    compute_pairwise_mi_matrix, load_pairwise_mi_matrix_csv, PairwiseMiAuditor, PairwiseMiMatrix,
    PairwiseMiSummary,
};
pub use per_head::{compute_per_head_learning_signal, HeadSignalInput, PerHeadLearningSignal};
pub use state_transfer::{
    compute_state_transfer, performance_deploy, DivergenceMetric, StateTransferDiagnostic,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UtmlErrorCode {
    InvalidSignal,
    MejepaInstrumentGradientLeak,
    CatastrophicForgetting,
    NonFinite,
    OutOfRange,
    SnrBelowFloor,
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
            UtmlErrorCode::CatastrophicForgetting => "UTML_CATASTROPHIC_FORGETTING",
            UtmlErrorCode::NonFinite => "UTML_NON_FINITE",
            UtmlErrorCode::OutOfRange => "UTML_OUT_OF_RANGE",
            UtmlErrorCode::SnrBelowFloor => "UTML_SNR_BELOW_FLOOR",
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

pub const DEFAULT_SNR_FLOOR: f32 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeltaPAggregator {
    #[default]
    Mean,
    Max,
    TopKMean {
        k: usize,
    },
    EntityWeighted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeltaPComponents {
    pub delta_p_real: f32,
    /// Optional counterfactual ("imagined") trajectory probability mass.
    ///
    /// F-027 contract:
    /// * `None` is a legitimate semantic state meaning "no imagined trajectory
    ///   was generated for this training row" (e.g., real-only batches, trainer
    ///   driver paths, replay rows that predate the imagined-trajectory
    ///   feature). When `None`, `compute_delta_p` treats the imagined component
    ///   as `0.0`, which collapses `max(real, gamma * imagined)` to `real`.
    ///   That is the intended contract for "no imagined branch was sampled."
    /// * `Some(value)` requires `value` to be finite and in `[0, 1]`. This is
    ///   enforced by `DeltaPComponents::validate()` via `validate_unit` BEFORE
    ///   any `unwrap_or(0.0)` collapse, so corrupted rows (NaN, Inf,
    ///   out-of-range) surface as `UtmlErrorCode::NonFinite` /
    ///   `UtmlErrorCode::OutOfRange` rather than degrading to "real-only"
    ///   silently. Regression tests in this module exercise both legitimate
    ///   states and every corrupted state to keep the contract durable.
    pub delta_p_imagined: Option<f32>,
    pub snr: f32,
    pub exploration_bonus: f32,
    pub gamma: f32,
    pub aggregator: DeltaPAggregator,
    pub per_chunk_values: Vec<f32>,
}

impl Default for DeltaPComponents {
    fn default() -> Self {
        Self {
            delta_p_real: 0.0,
            delta_p_imagined: None,
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.0],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeltaKComponents {
    pub cos_align: f32,
    pub fisher_violation: f32,
    #[serde(default = "default_fisher_violation_source")]
    pub fisher_violation_source: String,
    pub ece: f32,
    pub embedder_coherence: f32,
    pub embedder_coherence_source: String,
}

fn default_fisher_violation_source() -> String {
    "bootstrap_neutral_no_fisher_snapshot".to_string()
}

impl Default for DeltaKComponents {
    fn default() -> Self {
        Self {
            cos_align: 0.0,
            fisher_violation: 0.0,
            fisher_violation_source: default_fisher_violation_source(),
            ece: 0.0,
            embedder_coherence: 0.5,
            embedder_coherence_source: "bootstrap_neutral_n_lt_1000".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeltaOmegaComponents {
    pub effective_plasticity: f32,
    pub landscape_health: f32,
    pub stability_floor: f32,
    pub agent_state_score: f32,
    pub agent_state_source: String,
}

impl Default for DeltaOmegaComponents {
    fn default() -> Self {
        Self {
            effective_plasticity: 1.0,
            landscape_health: 1.0,
            stability_floor: 1.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DeltaXiComponents {
    pub target_collapse: f32,
    pub predictor_redundancy: f32,
    pub constellation_violation_rate: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingWeightComponents {
    pub base_weight: f32,
    pub l_step: f32,
    pub operator_override: bool,
    pub age_days: f32,
    pub age_decay: f32,
    pub agent_surprise_severity_score: f32,
    pub foundationality_score: f32,
    pub lambda_foundationality: f32,
    pub curiosity_score: f32,
    pub lambda_curiosity: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibratedConfidenceComponents {
    pub raw_confidence: f32,
    pub convergence_rate: f32,
    pub strategy_agreement: f32,
    pub evidence_factor: f32,
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RewardComponents {
    pub calibrated_confidence: f32,
    pub mean_l_step_delta_xi: f32,
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
        self.delta_p_components.validate()?;
        self.delta_k_components.validate()?;
        self.delta_omega_components.validate()?;
        self.delta_xi_components.validate()?;
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

impl DeltaPComponents {
    pub fn validate(&self) -> Result<(), UtmlError> {
        validate_unit("delta_p.delta_p_real", self.delta_p_real)?;
        if let Some(value) = self.delta_p_imagined {
            validate_unit("delta_p.delta_p_imagined", value)?;
        }
        if !self.snr.is_finite() {
            return Err(UtmlError::new(
                UtmlErrorCode::NonFinite,
                format!("delta_p.snr is non-finite: {}", self.snr),
            ));
        }
        if self.snr < DEFAULT_SNR_FLOOR {
            return Err(UtmlError::new(
                UtmlErrorCode::SnrBelowFloor,
                format!(
                    "delta_p.snr={} is below floor {}",
                    self.snr, DEFAULT_SNR_FLOOR
                ),
            ));
        }
        if !self.gamma.is_finite() || !(0.6..=0.8).contains(&self.gamma) {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!("delta_p.gamma={} outside [0.6,0.8]", self.gamma),
            ));
        }
        if self.exploration_bonus != 0.0 && self.exploration_bonus != 0.5 {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!(
                    "delta_p.exploration_bonus={} must be exactly 0.0 or 0.5",
                    self.exploration_bonus
                ),
            ));
        }
        let aggregate = compute_delta_p_aggregate(&self.per_chunk_values, self.aggregator, None)?;
        if (aggregate - self.delta_p_real).abs() > 1e-5 {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                format!(
                    "delta_p_real={} does not match {:?} per-chunk aggregate {}",
                    self.delta_p_real, self.aggregator, aggregate
                ),
            ));
        }
        Ok(())
    }
}

impl DeltaKComponents {
    pub fn validate(&self) -> Result<(), UtmlError> {
        validate_finite("delta_k.cos_align", self.cos_align)?;
        if !(-1.0..=1.0).contains(&self.cos_align) {
            return Err(UtmlError::new(
                UtmlErrorCode::OutOfRange,
                format!("delta_k.cos_align={} outside [-1,1]", self.cos_align),
            ));
        }
        validate_unit("delta_k.fisher_violation", self.fisher_violation)?;
        validate_source(
            "delta_k.fisher_violation_source",
            &self.fisher_violation_source,
            &["computed", "bootstrap_neutral_no_fisher_snapshot"],
        )?;
        if self.fisher_violation_source == "bootstrap_neutral_no_fisher_snapshot"
            && self.fisher_violation != 0.0
        {
            return Err(UtmlError::new(
                UtmlErrorCode::InvalidSignal,
                "delta_k.fisher_violation must be 0.0 when no Fisher snapshot source is present",
            ));
        }
        validate_unit("delta_k.ece", self.ece)?;
        validate_unit("delta_k.embedder_coherence", self.embedder_coherence)?;
        validate_source(
            "delta_k.embedder_coherence_source",
            &self.embedder_coherence_source,
            &["computed", "bootstrap_neutral_n_lt_1000"],
        )
    }
}

impl DeltaOmegaComponents {
    pub fn validate(&self) -> Result<(), UtmlError> {
        validate_unit(
            "delta_omega.effective_plasticity",
            self.effective_plasticity,
        )?;
        validate_unit("delta_omega.landscape_health", self.landscape_health)?;
        validate_stability_floor(self.stability_floor)?;
        validate_unit("delta_omega.agent_state_score", self.agent_state_score)?;
        validate_source(
            "delta_omega.agent_state_source",
            &self.agent_state_source,
            &["e17_transcript", "default_neutral_no_transcript"],
        )
    }
}

impl DeltaXiComponents {
    pub fn validate(&self) -> Result<(), UtmlError> {
        if !self.target_collapse.is_finite() || self.target_collapse != 0.0 {
            return Err(UtmlError::new(
                UtmlErrorCode::MejepaInstrumentGradientLeak,
                format!(
                    "ME-JEPA target_collapse must be exactly 0.0; got {}",
                    self.target_collapse
                ),
            ));
        }
        validate_unit("delta_xi.predictor_redundancy", self.predictor_redundancy)?;
        validate_unit(
            "delta_xi.constellation_violation_rate",
            self.constellation_violation_rate,
        )
    }
}

pub fn compute_delta_p(c: DeltaPComponents) -> Result<f32, UtmlError> {
    // F-027 contract: `validate()` runs BEFORE the `unwrap_or(0.0)` collapse.
    // Corrupted rows (NaN, Inf, out-of-range) are rejected here as
    // `UtmlErrorCode::NonFinite` or `OutOfRange`, so the only way to reach the
    // `unwrap_or(0.0)` line below is the legitimate "no imagined trajectory"
    // semantic state documented on `DeltaPComponents::delta_p_imagined`.
    // The `max(real, gamma * 0.0) = real` collapse is the intended contract
    // for that case.
    c.validate()?;
    let imagined = c.delta_p_imagined.unwrap_or(0.0);
    Ok(clamp01(
        (c.delta_p_real.max(c.gamma * imagined) + 0.5 * c.exploration_bonus) / c.snr,
    ))
}

pub fn compute_delta_k(c: DeltaKComponents) -> Result<f32, UtmlError> {
    c.validate()?;
    if c.cos_align < 0.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::CatastrophicForgetting,
            format!(
                "delta_k.cos_align={} is negative; update is destructive to prior gradient direction",
                c.cos_align
            ),
        ));
    }
    let cos_align_normalized = (c.cos_align + 1.0) / 2.0;
    let non_fisher =
        0.5 * cos_align_normalized + 0.15 * (1.0 - c.ece) + 0.05 * c.embedder_coherence;
    if c.fisher_violation_source == "computed" {
        Ok(clamp01(non_fisher + 0.3 * (1.0 - c.fisher_violation)))
    } else {
        Ok(clamp01(non_fisher / 0.7))
    }
}

pub fn compute_delta_omega(c: DeltaOmegaComponents) -> Result<f32, UtmlError> {
    c.validate()?;
    Ok(clamp01(
        c.effective_plasticity.powf(0.4)
            * c.landscape_health.powf(0.4)
            * c.stability_floor.powf(0.1)
            * c.agent_state_score.powf(0.1),
    ))
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
                "ME-JEPA target_collapse must be 0.0 by construction; instrument has gradient flow; target_collapse={}, predictor_redundancy={}, constellation_violation_rate={}",
                c.target_collapse, c.predictor_redundancy, c.constellation_violation_rate
            ),
        ));
    }
    for (name, value) in [
        ("predictor_redundancy", c.predictor_redundancy),
        (
            "constellation_violation_rate",
            c.constellation_violation_rate,
        ),
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
            * (1.0 - c.predictor_redundancy).powf(0.3)
            * (1.0 - c.constellation_violation_rate).powf(0.2),
    ))
}

pub fn compute_delta_p_aggregate(
    per_chunk: &[f32],
    aggregator: DeltaPAggregator,
    entity_weights: Option<&[f32]>,
) -> Result<f32, UtmlError> {
    non_empty("delta_p.per_chunk_values", per_chunk)?;
    for (idx, value) in per_chunk.iter().enumerate() {
        validate_unit(&format!("delta_p.per_chunk_values[{idx}]"), *value)?;
    }
    let value = match aggregator {
        DeltaPAggregator::Mean => per_chunk.iter().sum::<f32>() / per_chunk.len() as f32,
        DeltaPAggregator::Max => per_chunk.iter().copied().fold(0.0, f32::max),
        DeltaPAggregator::TopKMean { k } => {
            if k == 0 {
                return Err(UtmlError::new(
                    UtmlErrorCode::OutOfRange,
                    "delta_p TopKMean.k must be greater than zero",
                ));
            }
            let mut sorted = per_chunk.to_vec();
            sorted.sort_by(|a, b| b.total_cmp(a));
            let take = k.min(sorted.len());
            sorted.iter().take(take).sum::<f32>() / take as f32
        }
        DeltaPAggregator::EntityWeighted => {
            let weights = entity_weights.ok_or_else(|| {
                UtmlError::new(
                    UtmlErrorCode::MissingSourceOfTruth,
                    "delta_p EntityWeighted requires entity_weights source of truth",
                )
            })?;
            if weights.len() != per_chunk.len() {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    format!(
                        "delta_p EntityWeighted weight count {} != chunk count {}",
                        weights.len(),
                        per_chunk.len()
                    ),
                ));
            }
            let mut weighted_sum = 0.0f32;
            let mut weight_sum = 0.0f32;
            for (idx, (value, weight)) in per_chunk.iter().zip(weights).enumerate() {
                validate_finite(&format!("delta_p.entity_weights[{idx}]"), *weight)?;
                if *weight < 0.0 {
                    return Err(UtmlError::new(
                        UtmlErrorCode::OutOfRange,
                        format!("delta_p.entity_weights[{idx}]={} is negative", weight),
                    ));
                }
                weighted_sum += value * weight;
                weight_sum += weight;
            }
            if weight_sum <= f32::EPSILON {
                return Err(UtmlError::new(
                    UtmlErrorCode::InvalidSignal,
                    "delta_p EntityWeighted requires positive weight mass",
                ));
            }
            weighted_sum / weight_sum
        }
    };
    Ok(clamp01(value))
}

pub fn compute_l_step(
    p: DeltaPComponents,
    k: DeltaKComponents,
    o: DeltaOmegaComponents,
    x: DeltaXiComponents,
) -> Result<LearningSignal, UtmlError> {
    let delta_p = compute_delta_p(p.clone())?;
    let delta_k = compute_delta_k(k.clone())?;
    let delta_omega = compute_delta_omega(o.clone())?;
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
    sampling_weight_checked(SamplingWeightComponents {
        base_weight: 1.0,
        l_step,
        operator_override,
        age_days,
        age_decay,
        agent_surprise_severity_score: 0.0,
        foundationality_score: 0.0,
        lambda_foundationality: 1.0,
        curiosity_score: 0.0,
        lambda_curiosity: 1.0,
    })
    .expect("sampling_weight inputs must be finite and in range; use sampling_weight_checked to handle errors")
}

pub fn sampling_weight_checked(c: SamplingWeightComponents) -> Result<f32, UtmlError> {
    sampling_weight_checked_with_operator_override_multiplier(c, 6.0)
}

pub fn sampling_weight_checked_with_operator_override_multiplier(
    c: SamplingWeightComponents,
    operator_override_multiplier: f32,
) -> Result<f32, UtmlError> {
    validate_nonnegative("sampling.base_weight", c.base_weight)?;
    validate_unit("sampling.l_step", c.l_step)?;
    validate_nonnegative("sampling.age_days", c.age_days)?;
    validate_unit("sampling.age_decay", c.age_decay)?;
    validate_unit(
        "sampling.agent_surprise_severity_score",
        c.agent_surprise_severity_score,
    )?;
    validate_unit("sampling.foundationality_score", c.foundationality_score)?;
    validate_nonnegative("sampling.lambda_foundationality", c.lambda_foundationality)?;
    validate_unit("sampling.curiosity_score", c.curiosity_score)?;
    validate_nonnegative("sampling.lambda_curiosity", c.lambda_curiosity)?;
    validate_nonnegative(
        "sampling.operator_override_multiplier",
        operator_override_multiplier,
    )?;
    if c.operator_override && operator_override_multiplier < 1.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!(
                "sampling.operator_override_multiplier must be >= 1.0 when operator_override=true; got {operator_override_multiplier}"
            ),
        ));
    }
    let override_factor = if c.operator_override {
        operator_override_multiplier
    } else {
        1.0
    };
    let surprise_factor = 1.0 + 2.0 * c.agent_surprise_severity_score;
    let foundationality_factor = 1.0 + c.lambda_foundationality * c.foundationality_score;
    let curiosity_factor = 1.0 + c.lambda_curiosity * c.curiosity_score;
    let value = c.base_weight
        * c.l_step
        * override_factor
        * surprise_factor
        * foundationality_factor
        * curiosity_factor
        * c.age_decay.powf(c.age_days);
    validate_finite("sampling.weight", value)?;
    Ok(value)
}

pub fn calibrated_confidence(c: CalibratedConfidenceComponents) -> Result<f32, UtmlError> {
    validate_unit("confidence.raw_confidence", c.raw_confidence)?;
    validate_unit("confidence.convergence_rate", c.convergence_rate)?;
    validate_unit("confidence.strategy_agreement", c.strategy_agreement)?;
    validate_unit("confidence.evidence_factor", c.evidence_factor)?;
    validate_unit("confidence.delta_omega_mean", c.delta_omega_mean)?;
    validate_unit("confidence.delta_xi_mean", c.delta_xi_mean)?;
    let raw = c.raw_confidence
        * c.convergence_rate
        * c.strategy_agreement
        * c.evidence_factor
        * c.delta_omega_mean
        * c.delta_xi_mean;
    validate_finite("confidence.calibrated", raw)?;
    Ok(raw.clamp(0.10, 0.95))
}

pub fn reward(c: RewardComponents) -> Result<f32, UtmlError> {
    validate_unit("reward.calibrated_confidence", c.calibrated_confidence)?;
    validate_unit("reward.mean_l_step_delta_xi", c.mean_l_step_delta_xi)?;
    Ok(clamp01(
        0.6 * c.calibrated_confidence + 0.4 * c.mean_l_step_delta_xi,
    ))
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

fn validate_stability_floor(value: f32) -> Result<(), UtmlError> {
    validate_unit("delta_omega.stability_floor", value)?;
    if [1.0, 0.4, 0.1, 0.0]
        .iter()
        .any(|allowed| (value - allowed).abs() <= f32::EPSILON)
    {
        Ok(())
    } else {
        Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            format!("delta_omega.stability_floor={value} must be one of 1.0, 0.4, 0.1, 0.0"),
        ))
    }
}

fn validate_source(name: &str, value: &str, allowed: &[&str]) -> Result<(), UtmlError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(UtmlError::new(
            UtmlErrorCode::InvalidSignal,
            format!("{name}={value:?} must be one of {allowed:?}"),
        ))
    }
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

fn validate_nonnegative(name: &str, value: f32) -> Result<(), UtmlError> {
    validate_finite(name, value)?;
    if value < 0.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            format!("{name}={value} must be non-negative"),
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
                delta_p_real: 0.0,
                delta_p_imagined: None,
                snr: 1.0,
                exploration_bonus: 0.0,
                gamma: 0.7,
                aggregator: DeltaPAggregator::Mean,
                per_chunk_values: vec![0.0],
            },
            DeltaKComponents::default(),
            DeltaOmegaComponents::default(),
            DeltaXiComponents::default(),
        )
        .unwrap();
        assert_eq!(sig.delta_p, 0.0);
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
    fn sampling_weight_agent_surprise_reaches_three_x_at_catastrophic() {
        let base = sampling_weight_checked(SamplingWeightComponents {
            base_weight: 1.0,
            l_step: 0.5,
            operator_override: false,
            age_days: 0.0,
            age_decay: 1.0,
            agent_surprise_severity_score: 0.0,
            foundationality_score: 0.0,
            lambda_foundationality: 1.0,
            curiosity_score: 0.0,
            lambda_curiosity: 1.0,
        })
        .expect("base weight");
        let catastrophic = sampling_weight_checked(SamplingWeightComponents {
            base_weight: 1.0,
            l_step: 0.5,
            operator_override: false,
            age_days: 0.0,
            age_decay: 1.0,
            agent_surprise_severity_score: 1.0,
            foundationality_score: 0.0,
            lambda_foundationality: 1.0,
            curiosity_score: 0.0,
            lambda_curiosity: 1.0,
        })
        .expect("catastrophic weight");
        assert!((catastrophic / base - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn sampling_weight_foundationality_and_curiosity_compose() {
        let value = sampling_weight_checked(SamplingWeightComponents {
            base_weight: 1.0,
            l_step: 0.5,
            operator_override: false,
            age_days: 0.0,
            age_decay: 1.0,
            agent_surprise_severity_score: 0.0,
            foundationality_score: 0.75,
            lambda_foundationality: 2.0,
            curiosity_score: 0.5,
            lambda_curiosity: 1.0,
        })
        .expect("weighted sampling");
        assert!((value - 1.875).abs() < 1e-6);
    }

    #[test]
    fn delta_xi_uses_multiplicative_non_collapse_formula() {
        let value = compute_delta_xi(DeltaXiComponents {
            target_collapse: 0.0,
            predictor_redundancy: 0.1,
            constellation_violation_rate: 0.05,
        })
        .unwrap();
        let expected = 1.0f32 * 0.9f32.powf(0.3) * 0.95f32.powf(0.2);
        assert!((value - expected).abs() < 1e-6);
    }

    #[test]
    fn delta_xi_fails_closed_on_target_collapse() {
        let err = compute_delta_xi(DeltaXiComponents {
            target_collapse: 0.001,
            predictor_redundancy: 0.0,
            constellation_violation_rate: 0.0,
        })
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENT_GRADIENT_LEAK");
    }

    #[test]
    fn delta_xi_fails_closed_on_nan_target_collapse() {
        let err = compute_delta_xi(DeltaXiComponents {
            target_collapse: f32::NAN,
            predictor_redundancy: 0.0,
            constellation_violation_rate: 0.0,
        })
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENT_GRADIENT_LEAK");
    }

    #[test]
    fn delta_k_uses_weighted_formula_and_fails_on_negative_alignment() {
        let value = compute_delta_k(DeltaKComponents {
            cos_align: 0.8,
            fisher_violation: 0.2,
            fisher_violation_source: "computed".to_string(),
            ece: 0.1,
            embedder_coherence: 0.6,
            embedder_coherence_source: "computed".to_string(),
        })
        .unwrap();
        let expected = 0.5 * 0.9 + 0.3 * 0.8 + 0.15 * 0.9 + 0.05 * 0.6;
        assert!((value - expected).abs() < 1e-6);
        let err = compute_delta_k(DeltaKComponents {
            cos_align: -0.01,
            fisher_violation: 0.0,
            fisher_violation_source: "computed".to_string(),
            ece: 0.0,
            embedder_coherence: 1.0,
            embedder_coherence_source: "computed".to_string(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "UTML_CATASTROPHIC_FORGETTING");
    }

    #[test]
    fn delta_k_bootstrap_fisher_source_excludes_fisher_term() {
        let value = compute_delta_k(DeltaKComponents {
            cos_align: 0.8,
            fisher_violation: 0.0,
            fisher_violation_source: "bootstrap_neutral_no_fisher_snapshot".to_string(),
            ece: 0.1,
            embedder_coherence: 0.6,
            embedder_coherence_source: "computed".to_string(),
        })
        .unwrap();
        let expected = (0.5 * 0.9 + 0.15 * 0.9 + 0.05 * 0.6) / 0.7;
        assert!((value - expected).abs() < 1e-6);
    }

    #[test]
    fn delta_k_fails_closed_on_nonzero_bootstrap_fisher() {
        let err = compute_delta_k(DeltaKComponents {
            cos_align: 0.8,
            fisher_violation: 0.2,
            fisher_violation_source: "bootstrap_neutral_no_fisher_snapshot".to_string(),
            ece: 0.1,
            embedder_coherence: 0.6,
            embedder_coherence_source: "computed".to_string(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "UTML_INVALID_SIGNAL");
    }

    #[test]
    fn delta_omega_uses_geometric_formula_and_zeroes_on_stability_floor() {
        let value = compute_delta_omega(DeltaOmegaComponents {
            effective_plasticity: 0.8,
            landscape_health: 0.7,
            stability_floor: 1.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        })
        .unwrap();
        let expected = 0.8f32.powf(0.4) * 0.7f32.powf(0.4) * 1.0f32 * 0.7f32.powf(0.1);
        assert!((value - expected).abs() < 1e-6);
        let zeroed = compute_delta_omega(DeltaOmegaComponents {
            effective_plasticity: 0.8,
            landscape_health: 0.7,
            stability_floor: 0.0,
            agent_state_score: 0.7,
            agent_state_source: "default_neutral_no_transcript".to_string(),
        })
        .unwrap();
        assert_eq!(zeroed, 0.0);
    }

    #[test]
    fn delta_p_validates_per_chunk_source_of_truth() {
        let value = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.6,
            delta_p_imagined: Some(0.9),
            snr: 1.0,
            exploration_bonus: 0.5,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Max,
            per_chunk_values: vec![0.2, 0.6],
        })
        .unwrap();
        assert!((value - 0.88).abs() < 1e-6);
        let err = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.2,
            delta_p_imagined: None,
            snr: 0.01,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.2],
        })
        .unwrap_err();
        assert_eq!(err.code(), "UTML_SNR_BELOW_FLOOR");
    }

    // --- F-027 regression coverage ---
    //
    // `delta_p_imagined: Option<f32>` has TWO legitimate states:
    //   1) `None`         → "no imagined trajectory" — fold to 0.0
    //   2) `Some(value)`  → imagined branch generated; value finite, in [0,1]
    //
    // Corrupted rows (NaN/Inf, out-of-range) must NOT silently degrade the
    // signal. They must surface as `UtmlErrorCode::NonFinite` or `OutOfRange`
    // BEFORE compute_delta_p ever reaches `unwrap_or(0.0)`.

    #[test]
    fn delta_p_imagined_none_collapses_to_real_only() {
        let real = 0.42_f32;
        let signal = compute_delta_p(DeltaPComponents {
            delta_p_real: real,
            delta_p_imagined: None,
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![real],
        })
        .expect("legitimate None must be accepted");
        assert!(
            (signal - real).abs() < 1e-6,
            "expected real-only collapse {real}, got {signal}"
        );
    }

    #[test]
    fn delta_p_imagined_some_dominates_when_above_real() {
        let signal = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.10,
            delta_p_imagined: Some(0.80),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.10],
        })
        .expect("legitimate Some must be accepted");
        // expected = max(0.10, 0.7 * 0.80) = 0.56
        assert!((signal - 0.56).abs() < 1e-6, "got {signal}");
    }

    #[test]
    fn delta_p_imagined_nan_rejected_by_validate() {
        let err = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.5,
            delta_p_imagined: Some(f32::NAN),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.5],
        })
        .expect_err("NaN imagined must fail closed");
        assert_eq!(err.code(), "UTML_NON_FINITE");
        assert!(err.message.contains("delta_p_imagined"));
    }

    #[test]
    fn delta_p_imagined_infinite_rejected_by_validate() {
        let err = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.5,
            delta_p_imagined: Some(f32::INFINITY),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.5],
        })
        .expect_err("infinite imagined must fail closed");
        assert_eq!(err.code(), "UTML_NON_FINITE");
    }

    #[test]
    fn delta_p_imagined_above_unit_interval_rejected() {
        let err = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.5,
            delta_p_imagined: Some(1.5),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.5],
        })
        .expect_err("imagined > 1 must fail closed");
        assert_eq!(err.code(), "UTML_OUT_OF_RANGE");
        assert!(err.message.contains("delta_p_imagined"));
    }

    #[test]
    fn delta_p_imagined_negative_rejected() {
        let err = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.5,
            delta_p_imagined: Some(-0.01),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.5],
        })
        .expect_err("negative imagined must fail closed");
        assert_eq!(err.code(), "UTML_OUT_OF_RANGE");
    }

    #[test]
    fn delta_p_some_zero_equals_none_branch() {
        // Sanity: `Some(0.0)` and `None` MUST yield IDENTICAL output. This
        // pins the documented contract that None folds to 0.0 inside
        // compute_delta_p.
        let with_none = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.42,
            delta_p_imagined: None,
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.42],
        })
        .expect("None");
        let with_some_zero = compute_delta_p(DeltaPComponents {
            delta_p_real: 0.42,
            delta_p_imagined: Some(0.0),
            snr: 1.0,
            exploration_bonus: 0.0,
            gamma: 0.7,
            aggregator: DeltaPAggregator::Mean,
            per_chunk_values: vec![0.42],
        })
        .expect("Some(0.0)");
        assert!((with_none - with_some_zero).abs() < 1e-6);
    }
}
