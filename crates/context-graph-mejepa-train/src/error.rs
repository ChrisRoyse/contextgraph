use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainerErrorCode {
    MejepaTrainLossNan,
    MejepaTrainGradExplode,
    MejepaTrainVramOom,
    MejepaTrainCheckpointCorrupt,
    MejepaTrainCertChainBroken,
    MejepaTrainHoldoutRegression,
    MejepaInstrumentGradientLeak,
    MejepaTrainDodFailed,
    MejepaTrainConfigInvalid,
    MejepaTrainEmbedderDigestMismatch,
    ReplayBufferDegeneratePriority,
    /// Trainer-side gate (F-001): observed (adversarial + cross_task) fallback ratio
    /// exceeded `DEFAULT_FALLBACK_WARNING_RATIO` for the current batch plan. Emitted
    /// by `sampler::cross_task::enforce_fallback_warning_ratio`.
    MejepaTrainFallbackRatioExceeded,
    /// Patch-similarity cosine helper (F-008/F-009): the two vectors disagreed on
    /// dimension. This is a structural slot-identity violation per CLAUDE.md §6.2,
    /// not a runtime "low similarity" event.
    MejepaTrainPatchSimilarityDimMismatch,
    /// Patch-similarity cosine helper (F-008/F-009): one or both vectors had an L2
    /// norm of zero. A zero vector has no defined cosine and must not silently
    /// collapse to 0.0 (orthogonal).
    MejepaTrainPatchSimilarityZeroNormVector,
    /// TASK-IP-001: latent entropy was requested for a constant or effectively
    /// one-dimensional latent batch. The entropy term must fail closed rather
    /// than convert collapse into a finite training reward.
    UtmlLatentEntropyDegenerate,
    /// #687: a public Trainer API exists in the type surface but has no
    /// implementation yet. Fail closed instead of silently returning Ok so that
    /// a future caller cannot accidentally treat a no-op as success.
    MejepaTrainNotImplemented,
}

impl TrainerErrorCode {
    pub fn as_screaming_snake(&self) -> &'static str {
        match self {
            Self::MejepaTrainLossNan => "MEJEPA_TRAIN_LOSS_NAN",
            Self::MejepaTrainGradExplode => "MEJEPA_TRAIN_GRAD_EXPLODE",
            Self::MejepaTrainVramOom => "MEJEPA_TRAIN_VRAM_OOM",
            Self::MejepaTrainCheckpointCorrupt => "MEJEPA_TRAIN_CHECKPOINT_CORRUPT",
            Self::MejepaTrainCertChainBroken => "MEJEPA_TRAIN_CERT_CHAIN_BROKEN",
            Self::MejepaTrainHoldoutRegression => "MEJEPA_TRAIN_HOLDOUT_REGRESSION",
            Self::MejepaInstrumentGradientLeak => "MEJEPA_INSTRUMENT_GRADIENT_LEAK",
            Self::MejepaTrainDodFailed => "MEJEPA_TRAIN_DOD_FAILED",
            Self::MejepaTrainConfigInvalid => "MEJEPA_TRAIN_CONFIG_INVALID",
            Self::MejepaTrainEmbedderDigestMismatch => "MEJEPA_TRAIN_EMBEDDER_DIGEST_MISMATCH",
            Self::ReplayBufferDegeneratePriority => "REPLAY_BUFFER_DEGENERATE_PRIORITY",
            Self::MejepaTrainFallbackRatioExceeded => "MEJEPA_TRAIN_FALLBACK_RATIO_EXCEEDED",
            Self::MejepaTrainPatchSimilarityDimMismatch => {
                "MEJEPA_TRAIN_PATCH_SIMILARITY_DIM_MISMATCH"
            }
            Self::MejepaTrainPatchSimilarityZeroNormVector => {
                "MEJEPA_TRAIN_PATCH_SIMILARITY_ZERO_NORM_VECTOR"
            }
            Self::UtmlLatentEntropyDegenerate => "UTML_LATENT_ENTROPY_DEGENERATE",
            Self::MejepaTrainNotImplemented => "MEJEPA_TRAIN_NOT_IMPLEMENTED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerError {
    pub code: TrainerErrorCode,
    pub message: String,
    pub step: Option<u64>,
    pub context: Value,
}

impl TrainerError {
    pub fn new(code: TrainerErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            step: None,
            context: json!({}),
        }
    }

    pub fn with_step(mut self, step: u64) -> Self {
        self.step = Some(step);
        self
    }

    pub fn with_context(mut self, ctx: Value) -> Self {
        self.context = ctx;
        self
    }

    pub fn code(&self) -> &'static str {
        self.code.as_screaming_snake()
    }
}

impl fmt::Display for TrainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message)
    }
}

impl std::error::Error for TrainerError {}

impl From<rocksdb::Error> for TrainerError {
    fn from(value: rocksdb::Error) -> Self {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("RocksDB operation failed: {value}"),
        )
        .with_context(json!({
            "remediation": "inspect RocksDB path, lock ownership, column families, and free disk"
        }))
    }
}

impl From<std::io::Error> for TrainerError {
    fn from(value: std::io::Error) -> Self {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCheckpointCorrupt,
            format!("file operation failed: {value}"),
        )
        .with_context(json!({
            "remediation": "inspect file permissions, filesystem health, and available space"
        }))
    }
}

impl From<serde_json::Error> for TrainerError {
    fn from(value: serde_json::Error) -> Self {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("JSON serialization failed: {value}"),
        )
    }
}

impl From<toml::de::Error> for TrainerError {
    fn from(value: toml::de::Error) -> Self {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("TOML config parse failed: {value}"),
        )
    }
}

impl From<candle_core::Error> for TrainerError {
    fn from(value: candle_core::Error) -> Self {
        let message = value.to_string();
        let lower = message.to_ascii_lowercase();
        let code = if lower.contains("out of memory") || lower.contains("oom") {
            TrainerErrorCode::MejepaTrainVramOom
        } else if lower.contains("nan") || lower.contains("inf") {
            TrainerErrorCode::MejepaTrainLossNan
        } else {
            TrainerErrorCode::MejepaTrainGradExplode
        };
        TrainerError::new(code, format!("Candle tensor operation failed: {message}"))
            .with_context(json!({
                "remediation": "inspect tensor shapes, dtype/device placement, CUDA memory, and gradient finite checks"
            }))
    }
}
