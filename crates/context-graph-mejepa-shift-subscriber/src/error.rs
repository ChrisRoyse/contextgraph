// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, SubscriberError>;

#[derive(Debug, Error)]
pub enum SubscriberError {
    #[error("MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT: field={field} detail={detail}")]
    InvalidInput { field: String, detail: String },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL: path={} byte_offset={byte_offset} detail={detail}", path.display())]
    LogParseFail {
        path: PathBuf,
        byte_offset: u64,
        detail: String,
    },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS: session_id={session_id} existing_offset={existing_offset} requested_offset={requested_offset}")]
    WatermarkBackwards {
        session_id: String,
        existing_offset: u64,
        requested_offset: u64,
    },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW: requested_bytes={requested_bytes} budget_bytes={budget_bytes}")]
    InstrumentCacheOverflow {
        requested_bytes: u64,
        budget_bytes: u64,
    },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_PROCESS_FAILED: shift_id={shift_id} detail={detail}")]
    ProcessFailed { shift_id: String, detail: String },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_PROCESS_PANIC: shift_id={shift_id} detail={detail}")]
    ProcessPanic { shift_id: String, detail: String },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_IO: op={op} path={} source={source}", path.display())]
    Io {
        op: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("MEJEPA_SHIFT_SUBSCRIBER_JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MEJEPA_SHIFT_SUBSCRIBER_ROCKSDB: {0}")]
    RocksDb(#[from] rocksdb::Error),
    #[error("MEJEPA_SHIFT_SUBSCRIBER_BINCODE: {0}")]
    Bincode(#[from] Box<bincode::ErrorKind>),
    #[error("MEJEPA_SHIFT_SUBSCRIBER_INFER: {0}")]
    Infer(#[from] context_graph_mejepa::MejepaInferError),
    #[error("MEJEPA_SHIFT_SUBSCRIBER_EMBED: {0}")]
    Embed(#[from] context_graph_mejepa_embedders::EmbedError),
    #[error("MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT: {0}")]
    Instrument(#[from] context_graph_mejepa_instruments::InstrumentError),
    #[error("MEJEPA_SHIFT_SUBSCRIBER_TRAIN: {0}")]
    Train(#[from] context_graph_mejepa_train::TrainerError),
}

impl SubscriberError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput { .. } => "MEJEPA_SHIFT_SUBSCRIBER_INVALID_INPUT",
            Self::LogParseFail { .. } => "MEJEPA_SHIFT_SUBSCRIBER_LOG_PARSE_FAIL",
            Self::WatermarkBackwards { .. } => "MEJEPA_SHIFT_SUBSCRIBER_WATERMARK_BACKWARDS",
            Self::InstrumentCacheOverflow { .. } => {
                "MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT_CACHE_OVERFLOW"
            }
            Self::ProcessFailed { .. } => "MEJEPA_SHIFT_SUBSCRIBER_PROCESS_FAILED",
            Self::ProcessPanic { .. } => "MEJEPA_SHIFT_SUBSCRIBER_PROCESS_PANIC",
            Self::Io { .. } => "MEJEPA_SHIFT_SUBSCRIBER_IO",
            Self::Json(_) => "MEJEPA_SHIFT_SUBSCRIBER_JSON",
            Self::RocksDb(_) => "MEJEPA_SHIFT_SUBSCRIBER_ROCKSDB",
            Self::Bincode(_) => "MEJEPA_SHIFT_SUBSCRIBER_BINCODE",
            Self::Infer(_) => "MEJEPA_SHIFT_SUBSCRIBER_INFER",
            Self::Embed(_) => "MEJEPA_SHIFT_SUBSCRIBER_EMBED",
            Self::Instrument(_) => "MEJEPA_SHIFT_SUBSCRIBER_INSTRUMENT",
            Self::Train(_) => "MEJEPA_SHIFT_SUBSCRIBER_TRAIN",
        }
    }

    pub(crate) fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::InvalidInput {
            field: field.into(),
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

    pub fn log_context(&self, shift_id: Option<&str>) {
        tracing::error!(
            error_code = self.code(),
            error = %self,
            shift_id,
            "ME-JEPA shift subscriber error"
        );
    }
}
