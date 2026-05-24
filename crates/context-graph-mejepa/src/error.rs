use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::PANEL_DIM;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NanSource {
    Input,
    Layer,
    Output,
    Loss,
    OracleHead,
    FrozenTarget,
}

#[derive(Debug, Error)]
pub enum PredictorError {
    #[error("MEJEPA_PRED_CONFIG_INVALID: {detail}")]
    ConfigInvalid { detail: String },
    #[error("MEJEPA_PRED_DEVICE_UNAVAILABLE: {detail}")]
    DeviceUnavailable { detail: String },
    #[error("MEJEPA_PRED_DIM_MISMATCH: {detail}; observed={observed}; expected_panel_dim={expected_panel_dim}")]
    DimMismatch {
        detail: String,
        observed: Value,
        expected_panel_dim: usize,
    },
    #[error(
        "MEJEPA_PRED_NAN_DETECTED: source={nan_source:?} layer_id={layer_id:?} tensor={tensor_name:?}"
    )]
    NanDetected {
        nan_source: NanSource,
        layer_id: Option<u8>,
        tensor_name: Option<String>,
    },
    #[error("MEJEPA_PRED_VRAM_EXCEEDED: vram_resident_bytes={vram_resident_bytes} threshold_bytes={threshold_bytes}")]
    VramExceeded {
        vram_resident_bytes: u64,
        threshold_bytes: u64,
    },
    #[error("MEJEPA_PRED_FROZEN_TARGET_GRAD: instrument_id={instrument_id} grad_norm={grad_norm:.6e} threshold={threshold:.6e} fix_at={fix_at}")]
    FrozenTargetGrad {
        instrument_id: String,
        grad_norm: f32,
        threshold: f32,
        fix_at: String,
    },
    /// #620: a frozen-target adapter listed a tensor_id but the GradStore
    /// has no entry for it. Previously this was silently treated as
    /// `grad_norm = 0.0` ("safe"), which blurs measured-zero apart from
    /// "I forgot to measure". The doctrinal contract is: if the adapter
    /// claims a tensor is supervised, the GradStore MUST have measured
    /// it. Fail closed.
    #[error("MEJEPA_PRED_FROZEN_TARGET_GRAD_UNMEASURED: instrument_id={instrument_id} tensor_id={tensor_id} fix_at={fix_at}")]
    FrozenTargetGradUnmeasured {
        instrument_id: String,
        tensor_id: String,
        fix_at: String,
    },
    #[error("MEJEPA_PRED_VICREG_DEGENERATE: low_variance_dim_count={low_variance_dim_count} total_dims={total_dims} gamma={gamma} consecutive_passes={consecutive_passes}")]
    VicregDegenerate {
        low_variance_dim_count: usize,
        total_dims: usize,
        gamma: f32,
        consecutive_passes: u8,
    },
    #[error("MEJEPA_HEAD_FAILURE: head={head} code={code} detail={detail}")]
    HeadFailure {
        head: String,
        code: String,
        detail: String,
    },
    #[error("MEJEPA_PRED_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
    #[error("MEJEPA_PRED_IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("MEJEPA_PRED_JSON: {0}")]
    Json(#[from] serde_json::Error),
}

impl PredictorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConfigInvalid { .. } => "MEJEPA_PRED_CONFIG_INVALID",
            Self::DeviceUnavailable { .. } => "MEJEPA_PRED_DEVICE_UNAVAILABLE",
            Self::DimMismatch { .. } => "MEJEPA_PRED_DIM_MISMATCH",
            Self::NanDetected { .. } => "MEJEPA_PRED_NAN_DETECTED",
            Self::VramExceeded { .. } => "MEJEPA_PRED_VRAM_EXCEEDED",
            Self::FrozenTargetGrad { .. } => "MEJEPA_PRED_FROZEN_TARGET_GRAD",
            Self::FrozenTargetGradUnmeasured { .. } => "MEJEPA_PRED_FROZEN_TARGET_GRAD_UNMEASURED",
            Self::VicregDegenerate { .. } => "MEJEPA_PRED_VICREG_DEGENERATE",
            Self::HeadFailure { .. } => "MEJEPA_HEAD_FAILURE",
            Self::Candle(_) => "MEJEPA_PRED_CANDLE",
            Self::Io(_) => "MEJEPA_PRED_IO",
            Self::Json(_) => "MEJEPA_PRED_JSON",
        }
    }

    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::FrozenTargetGrad { .. } | Self::FrozenTargetGradUnmeasured { .. }
        )
    }
}

#[derive(Debug, Error)]
pub enum LossError {
    #[error("MEJEPA_PRED_DIM_MISMATCH (loss): {detail}")]
    DimMismatch { detail: String },
    #[error("MEJEPA_PRED_NAN_DETECTED (loss component={component}): {detail}")]
    NanDetected {
        component: &'static str,
        detail: String,
    },
    #[error("MEJEPA_PRED_BATCH_TOO_SMALL: batch={batch} minimum=2")]
    BatchTooSmall { batch: usize },
    #[error("MEJEPA_PRED_NON_FINITE_LAMBDA: lambdas={lambdas_dump}")]
    NonFiniteLambda { lambdas_dump: String },
    #[error("MEJEPA_PRED_CANDLE: {0}")]
    Candle(#[from] candle_core::Error),
}

impl LossError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::DimMismatch { .. } => "MEJEPA_PRED_DIM_MISMATCH",
            Self::NanDetected { .. } => "MEJEPA_PRED_NAN_DETECTED",
            Self::BatchTooSmall { .. } => "MEJEPA_PRED_BATCH_TOO_SMALL",
            Self::NonFiniteLambda { .. } => "MEJEPA_PRED_NON_FINITE_LAMBDA",
            Self::Candle(_) => "MEJEPA_PRED_CANDLE",
        }
    }
}

impl From<LossError> for PredictorError {
    fn from(value: LossError) -> Self {
        match value {
            LossError::DimMismatch { detail } => PredictorError::DimMismatch {
                detail,
                observed: Value::Null,
                expected_panel_dim: PANEL_DIM,
            },
            LossError::NanDetected { component, .. } => PredictorError::NanDetected {
                nan_source: NanSource::Loss,
                layer_id: None,
                tensor_name: Some(component.to_string()),
            },
            LossError::BatchTooSmall { batch } => PredictorError::DimMismatch {
                detail: format!("loss batch must be at least 2; got {batch}"),
                observed: serde_json::json!({ "batch": batch }),
                expected_panel_dim: PANEL_DIM,
            },
            LossError::NonFiniteLambda { lambdas_dump } => PredictorError::ConfigInvalid {
                detail: format!("VICReg lambdas must be finite: {lambdas_dump}"),
            },
            LossError::Candle(err) => PredictorError::Candle(err),
        }
    }
}

#[derive(Debug, Error)]
pub enum MejepaInferError {
    #[error("MEJEPA_INFER_OOD_REFUSE: ood_score={ood_score:.6} threshold={threshold:.6} reason={reason}")]
    OodRefuse {
        ood_score: f32,
        threshold: f32,
        reason: String,
    },
    #[error("MEJEPA_INFER_OOD_CALIBRATOR_MISSING: {detail}")]
    OodCalibratorMissing { detail: String },
    #[error("MEJEPA_INFER_OOD_PER_SLOT_CALIBRATOR_MISSING: {detail}")]
    OodPerSlotCalibratorMissing { detail: String },
    #[error("MEJEPA_INFER_DIM_MISMATCH: expected={expected} actual={actual} context={context}")]
    DimMismatch {
        expected: usize,
        actual: usize,
        context: String,
    },
    #[error("MEJEPA_INFER_INSTRUMENT_MISSING: slot={slot} context={context}")]
    InstrumentMissing { slot: String, context: String },
    #[error("MEJEPA_INFER_NAN_DETECTED: source={nan_source} detail={detail}")]
    NanDetected { nan_source: String, detail: String },
    #[error("MEJEPA_INFER_CALIBRATION_STALE: version={version} age_days={age_days}")]
    CalibrationStale { version: String, age_days: u32 },
    #[error("MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES: language={language:?} expected={expected} actual={actual}")]
    ConformalInsufficientSamples {
        language: Option<String>,
        expected: usize,
        actual: usize,
    },
    #[error("MEJEPA_INFER_WITNESS_CHAIN_BROKEN: offset={offset:?} reason={reason}")]
    WitnessChainBroken { offset: Option<u64>, reason: String },
    #[error("MEJEPA_INFER_SOURCE_SHA_DRIFT: path={} claimed={} observed={}", path.display(), hex::encode(claimed), hex::encode(observed))]
    SourceShaDrift {
        path: PathBuf,
        claimed: [u8; 32],
        observed: [u8; 32],
    },
    #[error("MEJEPA_INFER_GTAU_STALE_CONSTELLATION: version={version} age_days={age_days}")]
    GtauStaleConstellation { version: String, age_days: u32 },
    #[error("MEJEPA_INFER_FEATURE_DISABLED: feature={feature}")]
    FeatureDisabled { feature: String },
    #[error(
        "MEJEPA_INFER_DDA_FEATURE_MISSING: schema={schema} panel_id={panel_id} chunk_id={chunk_id}"
    )]
    DdaFeatureMissing {
        schema: String,
        panel_id: String,
        chunk_id: String,
    },
    #[error("HEAD_PROJECTION_MISSING_SLICE: head={head} slice={slice} panel_id={panel_id} chunk_id={chunk_id}")]
    HeadProjectionMissingSlice {
        head: String,
        slice: String,
        panel_id: String,
        chunk_id: String,
    },
    #[error("HEAD_PROJECTION_SCHEMA_MISMATCH: expected={expected} actual={actual}")]
    HeadProjectionSchemaMismatch { expected: String, actual: String },
    #[error("HEAD_PROJECTION_NO_DDA: panel_id={panel_id}")]
    HeadProjectionNoDda { panel_id: String },
    #[error("MEJEPA_INSTRUMENT_PROPOSAL_NOT_FOUND: proposal_id={proposal_id}")]
    InstrumentProposalNotFound { proposal_id: String },
    #[error("MEJEPA_INSTRUMENT_PROPOSAL_UNDER_REVIEW: proposal_id={proposal_id}")]
    InstrumentProposalUnderReview { proposal_id: String },
    #[error("MEJEPA_FINGERPRINT_FISHER_MISSING: fingerprint_id={fingerprint_id}")]
    FingerprintFisherMissing { fingerprint_id: String },
    #[error(
        "MEJEPA_ADVERSARIAL_FINGERPRINT_MATERIALIZATION_FAILED: case_id={case_id} reason={reason}"
    )]
    AdversarialFingerprintMaterializationFailed { case_id: String, reason: String },
    #[error("CLAIM_GRAPH_UNSUPPORTED_LANGUAGE: path={} reason={reason}", path.display())]
    ClaimGraphUnsupportedLanguage { path: PathBuf, reason: String },
    #[error("MEJEPA_INFER_INVALID_INPUT: field={field} detail={detail}")]
    InvalidInput { field: String, detail: String },
    #[error("MEJEPA_PATCH_SIMILARITY_DEGENERATE_QUERY: detail={detail}")]
    PatchSimilarityDegenerateQuery { detail: String },
    #[error("MEJEPA_PATCH_SIMILARITY_STALE_INDEX: expected_corpus_snapshot_hash={expected} actual_corpus_snapshot_hash={actual}")]
    PatchSimilarityStaleIndex { expected: String, actual: String },
    #[error("MEJEPA_INFER_IO: op={op} path={} source={source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MEJEPA_INFER_JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MEJEPA_INFER_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("MEJEPA_INFER_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
    #[error("MEJEPA_INFER_INSTRUMENT: {0}")]
    Instrument(#[from] context_graph_mejepa_instruments::InstrumentError),
    #[error("MEJEPA_INFER_TCT: {0}")]
    Tct(#[from] context_graph_mejepa_tct::TctError),
    #[error("MEJEPA_INFER_PREDICTOR: {0}")]
    Predictor(#[from] PredictorError),
    #[error(
        "MEJEPA_INFER_CONSTELLATION_INTELLIGENCE_UNAVAILABLE: prediction_id={prediction_id_hex}"
    )]
    ConstellationIntelligenceUnavailable { prediction_id_hex: String },
    #[error("MEJEPA_INFER_CONSTELLATION_VERSION_ID_MISSING: detail={detail}")]
    ConstellationVersionIdMissing { detail: String },
    #[error("MEJEPA_INFER_COSINE_MEAN_UNDEFINED_NO_SAMPLES: context={context}")]
    CosineMeanUndefinedNoSamples { context: String },
    #[error("MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE: detail={detail}")]
    InferTestCalibrationHoldoutUnavailable { detail: String },
}

impl MejepaInferError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::OodRefuse { .. } => "MEJEPA_INFER_OOD_REFUSE",
            Self::OodCalibratorMissing { .. } => "MEJEPA_INFER_OOD_CALIBRATOR_MISSING",
            Self::OodPerSlotCalibratorMissing { .. } => {
                "MEJEPA_INFER_OOD_PER_SLOT_CALIBRATOR_MISSING"
            }
            Self::DimMismatch { .. } => "MEJEPA_INFER_DIM_MISMATCH",
            Self::InstrumentMissing { .. } => "MEJEPA_INFER_INSTRUMENT_MISSING",
            Self::NanDetected { .. } => "MEJEPA_INFER_NAN_DETECTED",
            Self::CalibrationStale { .. } => "MEJEPA_INFER_CALIBRATION_STALE",
            Self::ConformalInsufficientSamples { .. } => {
                "MEJEPA_INFER_CONFORMAL_INSUFFICIENT_SAMPLES"
            }
            Self::WitnessChainBroken { .. } => "MEJEPA_INFER_WITNESS_CHAIN_BROKEN",
            Self::SourceShaDrift { .. } => "MEJEPA_INFER_SOURCE_SHA_DRIFT",
            Self::GtauStaleConstellation { .. } => "MEJEPA_INFER_GTAU_STALE_CONSTELLATION",
            Self::FeatureDisabled { .. } => "MEJEPA_INFER_FEATURE_DISABLED",
            Self::DdaFeatureMissing { .. } => "MEJEPA_INFER_DDA_FEATURE_MISSING",
            Self::HeadProjectionMissingSlice { .. } => "HEAD_PROJECTION_MISSING_SLICE",
            Self::HeadProjectionSchemaMismatch { .. } => "HEAD_PROJECTION_SCHEMA_MISMATCH",
            Self::HeadProjectionNoDda { .. } => "HEAD_PROJECTION_NO_DDA",
            Self::InstrumentProposalNotFound { .. } => "MEJEPA_INSTRUMENT_PROPOSAL_NOT_FOUND",
            Self::InstrumentProposalUnderReview { .. } => "MEJEPA_INSTRUMENT_PROPOSAL_UNDER_REVIEW",
            Self::FingerprintFisherMissing { .. } => "MEJEPA_FINGERPRINT_FISHER_MISSING",
            Self::AdversarialFingerprintMaterializationFailed { .. } => {
                "MEJEPA_ADVERSARIAL_FINGERPRINT_MATERIALIZATION_FAILED"
            }
            Self::ClaimGraphUnsupportedLanguage { .. } => "CLAIM_GRAPH_UNSUPPORTED_LANGUAGE",
            Self::InvalidInput { .. } => "MEJEPA_INFER_INVALID_INPUT",
            Self::PatchSimilarityDegenerateQuery { .. } => {
                "MEJEPA_PATCH_SIMILARITY_DEGENERATE_QUERY"
            }
            Self::PatchSimilarityStaleIndex { .. } => "MEJEPA_PATCH_SIMILARITY_STALE_INDEX",
            Self::Io { .. } => "MEJEPA_INFER_IO",
            Self::Json(_) => "MEJEPA_INFER_JSON",
            Self::RocksDb(_) => "MEJEPA_INFER_ROCKSDB",
            Self::Bincode(_) => "MEJEPA_INFER_BINCODE",
            Self::Instrument(_) => "MEJEPA_INFER_INSTRUMENT",
            Self::Tct(_) => "MEJEPA_INFER_TCT",
            Self::Predictor(_) => "MEJEPA_INFER_PREDICTOR",
            Self::ConstellationIntelligenceUnavailable { .. } => {
                "MEJEPA_INFER_CONSTELLATION_INTELLIGENCE_UNAVAILABLE"
            }
            Self::ConstellationVersionIdMissing { .. } => {
                "MEJEPA_INFER_CONSTELLATION_VERSION_ID_MISSING"
            }
            Self::CosineMeanUndefinedNoSamples { .. } => {
                "MEJEPA_INFER_COSINE_MEAN_UNDEFINED_NO_SAMPLES"
            }
            Self::InferTestCalibrationHoldoutUnavailable { .. } => {
                "MEJEPA_INFER_TEST_CALIBRATION_HOLDOUT_UNAVAILABLE"
            }
        }
    }

    pub fn log_context(&self, file: &'static str) {
        tracing::error!(
            error_code = self.code(),
            error = %self,
            file,
            "ME-JEPA inference error"
        );
    }

    pub fn io(op: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_codes_are_stable() {
        let err = PredictorError::FrozenTargetGrad {
            instrument_id: "e_ast".to_string(),
            grad_norm: 1.0,
            threshold: 1e-12,
            fix_at: "file:crates/context-graph-mejepa/src/frozen_target.rs".to_string(),
        };
        assert_eq!(err.code(), "MEJEPA_PRED_FROZEN_TARGET_GRAD");
        assert!(err.is_critical());
    }
}
