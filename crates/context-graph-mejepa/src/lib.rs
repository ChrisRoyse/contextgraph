//! ME-JEPA-Code Phase 2 predictor.
//!
//! This crate is intentionally CUDA-only for the production predictor path.
//! CPU construction is rejected instead of silently falling back.

pub mod calibration;
pub mod calibration_types;
pub mod cli;
pub mod compiler;
pub mod config;
pub mod conformal;
pub mod data_models;
pub mod degraded;
pub mod error;
pub mod evidence;
pub mod fixtures;
pub mod frozen_target;
pub mod fsv_writer;
pub mod gates;
pub mod grad_hook;
pub mod heal;
pub mod loss;
pub mod ood;
pub mod oracle_head;
pub mod panel_source;
pub mod predictor;
pub mod provenance;
pub mod store;
pub mod types;
pub mod vram;

pub use calibration::*;
pub use calibration_types::*;
pub use cli::*;
pub use compiler::{
    materialize_inference_panels, panel_sha, residual_norm_sq, ConstellationDecision,
    ConstellationGuard, DeterministicConstellationGuard, DeterministicOracleHead,
    DeterministicPredictor, FrozenTarget, IdentityFrozenTarget, MeJepaCompiler, MejepaStore,
    OracleScores, PatchWitnessReader, Predictor, TctConstellationGuard, TrainCertSummary,
    WitnessChainReader,
};
pub use config::{
    MeJepaInferConfig, PredictorConfig, CONCAT_INPUT_DIM, DEFAULT_FF_EXPANSION, DEFAULT_HIDDEN_DIM,
    DEFAULT_LAYER_NORM_EPS, DEFAULT_NUM_HEADS, DEFAULT_NUM_LAYERS, GRAD_NORM_NOISE_FLOOR,
    INFER_DEFAULT_ALPHA, INFER_DEFAULT_BOOTSTRAP_DELTA_OMEGA, INFER_DEFAULT_BOOTSTRAP_DELTA_XI,
    INFER_DEFAULT_INSTRUMENT_CACHE_CAPACITY, INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS,
    INFER_DEFAULT_MAX_CONSTELLATION_AGE_DAYS, INFER_DEFAULT_OOD_REFUSE_THRESHOLD,
    INFER_DEFAULT_OUTCOME_SET_MAX, INFER_DEFAULT_P_TEST_THRESHOLD, INFER_DEFAULT_P_THRESHOLD,
    INFER_DEFAULT_TRAIN_CERT_WINDOW_STEPS, PANEL_DIM, VICREG_COV_LAMBDA, VICREG_GAMMA,
    VICREG_INV_LAMBDA, VICREG_VAR_LAMBDA, VRAM_STEADY_STATE_TARGET_BYTES,
    VRAM_WARN_THRESHOLD_BYTES,
};
pub use conformal::*;
pub use data_models::{OracleLogits, PredictedPanel, TargetPanel, TargetProvenance};
pub use degraded::*;
pub use error::{LossError, MejepaInferError, NanSource, PredictorError};
pub use evidence::{
    AdversarialEvidence, DeterminismEvidence, ForwardPassEvidence, LossOutputs, VicregLambdas,
    VicregLossEvidence,
};
pub use fixtures::*;
pub use frozen_target::FrozenTargetAdapter;
pub use gates::*;
pub use grad_hook::{
    run_gradient_hook, GradStore, GradientHookReport, InstrumentGradHandle, NoOpGradHandle,
    TensorId,
};
pub use loss::{huber_loss_delta_one, vicreg_loss, VarianceFloorHistory};
pub use ood::*;
pub use oracle_head::OracleHead;
pub use predictor::{ArchitectureSummary, MeJepaPredictor, TransformerLayer};
pub use store::*;
pub use types::*;
pub use vram::{check_vram_steady_state, enforce_vram_report, vram_resident_bytes, VramReport};
