use crate::error::{TrainerError, TrainerErrorCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::OpenOptions;
use std::io::Write;
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
    std::fs::create_dir_all(checkpoint_dir)?;
    let final_path = checkpoint_dir.join("best.safetensors");
    let tmp_path = checkpoint_dir.join("best.safetensors.tmp");
    let bytes = format!(
        "MEJEPA_BEST_STUB_V1\nstep={current_step}\nagreement={current_agreement:.6}\n"
    );
    {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp_path)?;
        file.write_all(bytes.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, &final_path)?;
    Ok(Some(final_path))
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

