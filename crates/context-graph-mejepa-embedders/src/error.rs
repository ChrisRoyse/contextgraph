use crate::embedder_id::EmbedderId;
use std::path::PathBuf;
use thiserror::Error;

pub type EmbedResult<T> = Result<T, EmbedError>;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("invalid embedder input at {field}: {message}; remediation: {remediation}")]
    InvalidInput {
        field: &'static str,
        message: String,
        remediation: &'static str,
    },
    #[error("models config read failed at {path}: {message}; remediation: {remediation}")]
    ConfigRead {
        path: PathBuf,
        message: String,
        remediation: &'static str,
    },
    #[error("models config parse failed at {path}: {message}; remediation: {remediation}")]
    ConfigParse {
        path: PathBuf,
        message: String,
        remediation: &'static str,
    },
    #[error("missing registration for {embedder}; remediation: {remediation}")]
    MissingRegistration {
        embedder: EmbedderId,
        remediation: &'static str,
    },
    #[error("weight file missing for {embedder} at {path}; remediation: {remediation}")]
    WeightMissing {
        embedder: EmbedderId,
        path: PathBuf,
        remediation: &'static str,
    },
    #[error("weight digest mismatch for {embedder}: expected {expected}, actual {actual}; remediation: {remediation}")]
    DigestMismatch {
        embedder: EmbedderId,
        expected: String,
        actual: String,
        remediation: &'static str,
    },
    #[error("embedder routing missing for language={language} entity_type={entity_type}: {message}; remediation: {remediation}")]
    RoutingMissing {
        language: String,
        entity_type: String,
        message: String,
        remediation: &'static str,
    },
    #[error("CUDA/GPU unavailable: {message}; remediation: {remediation}")]
    GpuUnavailable {
        message: String,
        remediation: &'static str,
    },
    #[error("VRAM budget exceeded: required={required_bytes} free={free_bytes} total={total_bytes}; remediation: {remediation}")]
    VramExceeded {
        required_bytes: u64,
        free_bytes: u64,
        total_bytes: u64,
        remediation: &'static str,
    },
    #[error("E17 calibration required at {cert_path}: {message}; remediation: {remediation}")]
    E17Uncalibrated {
        cert_path: PathBuf,
        message: String,
        remediation: &'static str,
    },
    #[error("forward pass failed for {embedder}: {message}; remediation: {remediation}")]
    ForwardFailed {
        embedder: EmbedderId,
        message: String,
        remediation: &'static str,
    },
    #[error("[MEJEPA_EMBED_TRUE_BATCH_EMPTY] true-batch input empty for {embedder}: batch_size={batch_size}; remediation: {remediation}")]
    TrueBatchEmpty {
        embedder: EmbedderId,
        batch_size: usize,
        remediation: &'static str,
    },
    #[error("[MEJEPA_EMBED_TRUE_BATCH_UNSUPPORTED] true-batch forward unsupported for {embedder}: batch_size={batch_size}; message: {message}; remediation: {remediation}")]
    TrueBatchUnsupported {
        embedder: EmbedderId,
        batch_size: usize,
        message: String,
        remediation: &'static str,
    },
}

impl EmbedError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_EMBED_INVALID_INPUT",
            Self::ConfigRead { .. } => "MEJEPA_EMBED_CONFIG_READ",
            Self::ConfigParse { .. } => "MEJEPA_EMBED_CONFIG_PARSE",
            Self::MissingRegistration { .. } => "MEJEPA_EMBED_REGISTRATION_MISSING",
            Self::WeightMissing { .. } => "MEJEPA_EMBED_WEIGHT_MISSING",
            Self::DigestMismatch { .. } => "MEJEPA_EMBED_DIGEST_MISMATCH",
            Self::RoutingMissing { .. } => "MEJEPA_EMBED_ROUTING_MISSING",
            Self::GpuUnavailable { .. } => "MEJEPA_EMBED_GPU_UNAVAILABLE",
            Self::VramExceeded { .. } => "MEJEPA_EMBED_VRAM_EXCEEDED",
            Self::E17Uncalibrated { .. } => "MEJEPA_EMBED_E17_UNCALIBRATED",
            Self::ForwardFailed { .. } => "MEJEPA_EMBED_FORWARD_FAILED",
            Self::TrueBatchEmpty { .. } => "MEJEPA_EMBED_TRUE_BATCH_EMPTY",
            Self::TrueBatchUnsupported { .. } => "MEJEPA_EMBED_TRUE_BATCH_UNSUPPORTED",
        }
    }

    pub(crate) fn invalid(
        field: &'static str,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::InvalidInput {
            field,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn forward(
        embedder: EmbedderId,
        message: impl Into<String>,
        remediation: &'static str,
    ) -> Self {
        Self::ForwardFailed {
            embedder,
            message: message.into(),
            remediation,
        }
    }

    pub(crate) fn true_batch_empty(embedder: EmbedderId) -> Self {
        Self::TrueBatchEmpty {
            embedder,
            batch_size: 0,
            remediation:
                "submit at least one input; empty batches are caller bugs, not no-op successes",
        }
    }

    pub(crate) fn true_batch_unsupported(
        embedder: EmbedderId,
        batch_size: usize,
        message: impl Into<String>,
    ) -> Self {
        Self::TrueBatchUnsupported {
            embedder,
            batch_size,
            message: message.into(),
            remediation:
                "implement a native true-batch model path for this embedder before claiming CUDA-batched throughput",
        }
    }
}
