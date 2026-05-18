//! ME-JEPA-Code Phase 2 predictor.
//!
//! This crate is intentionally CUDA-only for the production predictor path.
//! CPU construction is rejected instead of silently falling back.

pub mod adversarial_corpus;
pub mod calibration;
pub mod calibration_types;
pub mod candidate_ranking;
pub mod claim_graph;
pub mod cli;
pub mod compiler;
pub mod config;
pub mod conformal;
pub mod cross_language_label_gating;
pub mod data_models;
pub mod dda_features;
pub mod degraded;
pub mod error;
pub mod eval;
pub mod evidence;
pub mod failure_fingerprint;
pub mod fixtures;
pub mod frozen_target;
pub mod fsv_writer;
pub mod gates;
pub mod grad_hook;
pub mod head_projection;
pub mod heal;
pub mod hierarchical;
pub mod instrument_proposal;
pub mod intent_north_star;
pub mod label_transfer_audit;
pub mod latent_search;
pub mod loss;
pub mod mincut_panel;
pub mod objective_safety;
pub mod ood;
pub mod operator_override;
pub mod oracle_head;
pub mod pairwise_mi;
pub mod panel_source;
pub mod park_list;
pub mod patch_similarity;
pub mod pause_state;
pub mod prediction_replay;
pub mod prediction_surfaces;
pub mod predictor;
pub mod project_cache;
pub mod provenance;
pub mod reality_impact;
pub mod reward_signal_audit;
pub mod secret_redaction;
pub mod store;
pub mod system_cost;
pub mod toolchain_detect;
pub mod types;
pub mod verdict_assembly;
pub mod verify_cli;
pub mod vram;

pub use adversarial_corpus::*;
pub use calibration::*;
pub use calibration_types::*;
pub use candidate_ranking::*;
pub use claim_graph::*;
pub use cli::*;
pub use compiler::{
    materialize_inference_panels, panel_sha, residual_norm_sq, ColdCellMetric,
    ConstellationCellSupport, ConstellationDecision, ConstellationGuard,
    DeterministicConstellationGuard, DeterministicOracleHead, DeterministicPredictor, FrozenTarget,
    IdentityFrozenTarget, MeJepaCompiler, MejepaStore, OracleScores, PatchWitnessReader, Predictor,
    TctConstellationGuard, TrainCertSummary, WitnessChainReader,
};
pub use config::{
    MeJepaInferConfig, PredictorConfig, CONCAT_INPUT_DIM, DEFAULT_FF_EXPANSION, DEFAULT_HIDDEN_DIM,
    DEFAULT_LAYER_NORM_EPS, DEFAULT_NUM_HEADS, DEFAULT_NUM_LAYERS, GRAD_NORM_NOISE_FLOOR,
    INFER_DEFAULT_ALPHA, INFER_DEFAULT_BOOTSTRAP_DELTA_OMEGA, INFER_DEFAULT_BOOTSTRAP_DELTA_XI,
    INFER_DEFAULT_DDA_EXPECTED_EMBEDDER_COUNT, INFER_DEFAULT_INSTRUMENT_CACHE_CAPACITY,
    INFER_DEFAULT_MAX_CALIBRATION_AGE_DAYS, INFER_DEFAULT_MAX_CONSTELLATION_AGE_DAYS,
    INFER_DEFAULT_MIN_CELL_SUPPORT_FOR_VERDICT, INFER_DEFAULT_OOD_REFUSE_THRESHOLD,
    INFER_DEFAULT_OUTCOME_SET_MAX, INFER_DEFAULT_P_TEST_THRESHOLD, INFER_DEFAULT_P_THRESHOLD,
    INFER_DEFAULT_REQUIRE_DDA_FEATURES, INFER_DEFAULT_TRAIN_CERT_WINDOW_STEPS,
    INFER_MAX_DDA_EXPECTED_EMBEDDER_COUNT, INVERSE_ACTION_DIM, PANEL_DIM, VICREG_COV_LAMBDA,
    VICREG_GAMMA, VICREG_INV_LAMBDA, VICREG_VAR_LAMBDA, VRAM_STEADY_STATE_TARGET_BYTES,
    VRAM_WARN_THRESHOLD_BYTES,
};
pub use conformal::*;
pub use data_models::{
    OracleLogits, PredictedInverseMap, PredictedPanel, TargetPanel, TargetProvenance,
};
pub use dda_features::*;
pub use degraded::*;
pub use error::{LossError, MejepaInferError, NanSource, PredictorError};
pub use eval::*;
pub use evidence::{
    AdversarialEvidence, DeterminismEvidence, ForwardPassEvidence, LossOutputs, VicregLambdas,
    VicregLossEvidence,
};
pub use failure_fingerprint::*;
pub use fixtures::*;
pub use frozen_target::FrozenTargetAdapter;
pub use gates::*;
pub use grad_hook::{
    run_gradient_hook, GradStore, GradientHookReport, InstrumentGradHandle, NoOpGradHandle,
    TensorId,
};
pub use head_projection::*;
pub use hierarchical::*;
pub use instrument_proposal::*;
pub use intent_north_star::*;
pub use label_transfer_audit::*;
pub use latent_search::*;
pub use loss::{huber_loss_delta_one, vicreg_loss, VarianceFloorHistory};
pub use mincut_panel::*;
pub use objective_safety::*;
pub use ood::*;
pub use operator_override::*;
pub use oracle_head::OracleHead;
pub use pairwise_mi::*;
pub use park_list::*;
pub use patch_similarity::*;
pub use pause_state::*;
pub use prediction_replay::*;
pub use prediction_surfaces::*;
pub use predictor::{ArchitectureSummary, MeJepaPredictor, TransformerLayer};
pub use project_cache::*;
pub use reality_impact::*;
pub use reward_signal_audit::*;
pub use secret_redaction::*;
pub use store::*;
pub use system_cost::*;
pub use types::*;
pub use verdict_assembly::*;
pub use vram::{check_vram_steady_state, enforce_vram_report, vram_resident_bytes, VramReport};
