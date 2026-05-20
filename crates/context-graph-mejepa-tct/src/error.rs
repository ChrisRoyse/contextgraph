use std::path::PathBuf;

use thiserror::Error;

use crate::types::EmbedderId;

#[derive(Debug, Error)]
pub enum TctError {
    #[error("MEJEPA_INFER_CONSTELLATION_VIOLATION: {detail}")]
    ConstellationViolation { detail: String },
    #[error("MEJEPA_INFER_GTAU_STALE_CONSTELLATION: frozen_at={frozen_at_iso} age_days={age_days} max_age_days={max_age_days}")]
    StaleConstellation {
        frozen_at_iso: String,
        age_days: u32,
        max_age_days: u32,
    },
    #[error(
        "MEJEPA_TCT_INSUFFICIENT_SAMPLES: cell={cell} observed={observed} required={required}"
    )]
    InsufficientSamples {
        cell: String,
        observed: usize,
        required: usize,
    },
    #[error("MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL: {detail}")]
    ThresholdCalibrationFail { detail: String },
    #[error(
        "MEJEPA_TCT_PROVENANCE_MISMATCH: embedder={embedder} expected={} observed={}",
        hex::encode(expected),
        hex::encode(observed)
    )]
    ProvenanceMismatch {
        embedder: EmbedderId,
        expected: [u8; 32],
        observed: [u8; 32],
    },
    #[error("MEJEPA_INSTR_FROZEN_VIOLATION: {detail}")]
    FrozenViolation { detail: String },
    #[error("MEJEPA_INFER_DIM_MISMATCH: expected={expected} actual={actual} context={context}")]
    DimMismatch {
        expected: usize,
        actual: usize,
        context: String,
    },
    #[error("MEJEPA_INFER_NAN_DETECTED: field={field} detail={detail}")]
    NanDetected { field: String, detail: String },
    #[error("MEJEPA_TCT_MISSING_CENTROID: {detail}")]
    MissingCentroid { detail: String },
    #[error("MEJEPA_TCT_INVALID_INPUT: field={field} detail={detail}")]
    InvalidInput { field: String, detail: String },
    #[error("MEJEPA_TCT_STORE: operation={operation} cf={cf} detail={detail}")]
    Store {
        operation: &'static str,
        cf: &'static str,
        detail: String,
    },
    #[error("MEJEPA_TCT_IO: op={op} path={} source={source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MEJEPA_TCT_JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MEJEPA_TCT_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
    #[error("MEJEPA_TCT_CALIBRATION_MISSING_FRESHNESS_TIMESTAMP: slot={slot} scope={scope}")]
    CalibrationMissingFreshnessTimestamp { slot: String, scope: String },
    #[error("MEJEPA_TCT_CELL_KEY_MALFORMED: value={value:?} context={context}")]
    CellKeyMalformed { value: String, context: String },
}

impl TctError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ConstellationViolation { .. } => "MEJEPA_INFER_CONSTELLATION_VIOLATION",
            Self::StaleConstellation { .. } => "MEJEPA_INFER_GTAU_STALE_CONSTELLATION",
            Self::InsufficientSamples { .. } => "MEJEPA_TCT_INSUFFICIENT_SAMPLES",
            Self::ThresholdCalibrationFail { .. } => "MEJEPA_TCT_THRESHOLD_CALIBRATION_FAIL",
            Self::ProvenanceMismatch { .. } => "MEJEPA_TCT_PROVENANCE_MISMATCH",
            Self::FrozenViolation { .. } => "MEJEPA_INSTR_FROZEN_VIOLATION",
            Self::DimMismatch { .. } => "MEJEPA_INFER_DIM_MISMATCH",
            Self::NanDetected { .. } => "MEJEPA_INFER_NAN_DETECTED",
            Self::MissingCentroid { .. } => "MEJEPA_TCT_MISSING_CENTROID",
            Self::InvalidInput { .. } => "MEJEPA_TCT_INVALID_INPUT",
            Self::Store { .. } => "MEJEPA_TCT_STORE",
            Self::Io { .. } => "MEJEPA_TCT_IO",
            Self::Json(_) => "MEJEPA_TCT_JSON",
            Self::Bincode(_) => "MEJEPA_TCT_BINCODE",
            Self::CalibrationMissingFreshnessTimestamp { .. } => {
                "MEJEPA_TCT_CALIBRATION_MISSING_FRESHNESS_TIMESTAMP"
            }
            Self::CellKeyMalformed { .. } => "MEJEPA_TCT_CELL_KEY_MALFORMED",
        }
    }

    pub fn log_context(&self, file: &'static str) {
        tracing::error!(
            error_code = self.code(),
            error = %self,
            file,
            "ME-JEPA TCT error"
        );
    }

    pub(crate) fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::InvalidInput {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn dim(expected: usize, actual: usize, context: impl Into<String>) -> Self {
        Self::DimMismatch {
            expected,
            actual,
            context: context.into(),
        }
    }

    pub(crate) fn nan(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::NanDetected {
            field: field.into(),
            detail: detail.into(),
        }
    }

    pub(crate) fn store(
        operation: &'static str,
        cf: &'static str,
        detail: impl Into<String>,
    ) -> Self {
        Self::Store {
            operation,
            cf,
            detail: detail.into(),
        }
    }

    pub(crate) fn io(op: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            op,
            path: path.into(),
            source,
        }
    }
}

impl From<rocksdb::Error> for TctError {
    fn from(value: rocksdb::Error) -> Self {
        Self::store("rocksdb", "<unknown>", value.to_string())
    }
}

impl From<context_graph_mejepa_instruments::InstrumentError> for TctError {
    fn from(value: context_graph_mejepa_instruments::InstrumentError) -> Self {
        Self::InvalidInput {
            field: "panel".to_string(),
            detail: value.to_string(),
        }
    }
}
