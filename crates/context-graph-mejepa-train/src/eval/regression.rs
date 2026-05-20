use crate::error::{TrainerError, TrainerErrorCode};
use context_graph_mejepa::{
    export_trained_predictor_checkpoint, ExportedPredictorCheckpoint, MeJepaPredictor,
    PredictorCheckpointExportMetadata,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path, PathBuf};

pub const PROMOTION_EPSILON: f32 = 0.005;
pub const REGRESSION_THRESHOLD: f32 = 0.02;
pub const CONSECUTIVE_REGRESSION_HALT: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionDecision {
    pub promoted: bool,
    pub regression_emitted: bool,
    pub halt_triggered: bool,
}

pub fn promote_best(
    checkpoint_dir: &Path,
    current_step: u64,
    current_agreement: f32,
    best_so_far: f32,
    promotion_threshold: f32,
) -> Result<Option<PathBuf>, TrainerError> {
    if current_agreement - best_so_far < promotion_threshold {
        return Ok(None);
    }
    Err(TrainerError::new(
        TrainerErrorCode::MejepaTrainCheckpointCorrupt,
        "text-stub best.safetensors promotion is retired; call promote_trained_predictor_checkpoint with a trained MeJepaPredictor",
    )
    .with_context(json!({
        "checkpoint_dir": checkpoint_dir,
        "current_step": current_step,
        "current_agreement": current_agreement,
        "best_so_far": best_so_far,
        "promotion_threshold": promotion_threshold,
        "retired_stub_magic": "MEJEPA_BEST_STUB_V1"
    })))
}

pub fn promote_trained_predictor_checkpoint(
    checkpoint_dir: &Path,
    current_step: u64,
    current_agreement: f32,
    best_so_far: f32,
    promotion_threshold: f32,
    predictor: &MeJepaPredictor,
    metadata: PredictorCheckpointExportMetadata,
) -> Result<Option<ExportedPredictorCheckpoint>, TrainerError> {
    if current_agreement - best_so_far < promotion_threshold {
        return Ok(None);
    }
    if current_step == 0 || metadata.payload_step == 0 {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainCheckpointCorrupt,
            "trained checkpoint promotion requires a non-zero training step",
        )
        .with_context(json!({
            "current_step": current_step,
            "metadata_payload_step": metadata.payload_step
        })));
    }
    export_trained_predictor_checkpoint(predictor, checkpoint_dir, predictor.config(), metadata)
        .map(Some)
        .map_err(|err| {
            TrainerError::new(
                TrainerErrorCode::MejepaTrainCheckpointCorrupt,
                format!("trained predictor checkpoint export failed: {err}"),
            )
            .with_context(json!({
                "predictor_error_code": err.code(),
                "checkpoint_dir": checkpoint_dir
            }))
        })
}

pub fn detect_regression(
    current_agreement: f32,
    best_so_far: f32,
    regression_threshold: f32,
) -> Option<TrainerError> {
    if best_so_far > 0.0 && best_so_far - current_agreement > regression_threshold {
        Some(
            TrainerError::new(
                TrainerErrorCode::MejepaTrainHoldoutRegression,
                format!(
                    "holdout agreement regressed from {best_so_far:.6} to {current_agreement:.6}"
                ),
            )
            .with_context(json!({
                "best_so_far": best_so_far,
                "current_agreement": current_agreement,
                "regression_threshold": regression_threshold
            })),
        )
    } else {
        None
    }
}

pub fn track_consecutive_regressions(history: &[bool], window: usize) -> bool {
    history.len() >= window && history[history.len() - window..].iter().all(|v| *v)
}
