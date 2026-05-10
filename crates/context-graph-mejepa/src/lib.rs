//! ME-JEPA-Code Phase 2 predictor.
//!
//! This crate is intentionally CUDA-only for the production predictor path.
//! CPU construction is rejected instead of silently falling back.

pub mod config;
pub mod data_models;
pub mod error;
pub mod evidence;
pub mod frozen_target;
pub mod fsv_writer;
pub mod grad_hook;
pub mod loss;
pub mod oracle_head;
pub mod panel_source;
pub mod predictor;
pub mod vram;

pub use config::{
    PredictorConfig, CONCAT_INPUT_DIM, DEFAULT_FF_EXPANSION, DEFAULT_HIDDEN_DIM,
    DEFAULT_LAYER_NORM_EPS, DEFAULT_NUM_HEADS, DEFAULT_NUM_LAYERS, GRAD_NORM_NOISE_FLOOR,
    PANEL_DIM, VICREG_COV_LAMBDA, VICREG_GAMMA, VICREG_INV_LAMBDA, VICREG_VAR_LAMBDA,
    VRAM_STEADY_STATE_TARGET_BYTES, VRAM_WARN_THRESHOLD_BYTES,
};
pub use data_models::{OracleLogits, PredictedPanel, TargetPanel, TargetProvenance};
pub use error::{LossError, NanSource, PredictorError};
pub use evidence::{
    AdversarialEvidence, DeterminismEvidence, ForwardPassEvidence, LossOutputs, VicregLambdas,
    VicregLossEvidence,
};
pub use frozen_target::FrozenTargetAdapter;
pub use grad_hook::{
    run_gradient_hook, GradStore, GradientHookReport, InstrumentGradHandle, NoOpGradHandle,
    TensorId,
};
pub use loss::{huber_loss_delta_one, vicreg_loss, VarianceFloorHistory};
pub use oracle_head::OracleHead;
pub use predictor::{ArchitectureSummary, MeJepaPredictor, TransformerLayer};
pub use vram::{check_vram_steady_state, enforce_vram_report, vram_resident_bytes, VramReport};
