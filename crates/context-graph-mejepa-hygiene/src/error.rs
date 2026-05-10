// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum OpsErrorKind {
    InvalidConfig {
        field: String,
        detail: String,
    },
    MissingColumnFamily {
        cf_name: String,
    },
    CorruptMetadata {
        key_hex: String,
        detail: String,
    },
    TierCodec {
        detail: String,
    },
    QuotaExceeded {
        category: String,
        used_bytes: u64,
        budget_bytes: u64,
    },
    QuotaUnrecoverable {
        category: String,
        used_bytes: u64,
        budget_bytes: u64,
    },
    WitnessChainBroken {
        offset: u64,
        detail: String,
    },
    WitnessArchiveInvalid {
        path: PathBuf,
        detail: String,
    },
    Io {
        op: &'static str,
        path: PathBuf,
        detail: String,
    },
    RocksDb {
        detail: String,
    },
    Json {
        detail: String,
    },
    Lock {
        path: PathBuf,
        detail: String,
    },
    CrossCuttingDeferred {
        operation: String,
        detail: String,
    },
}

#[derive(Debug, Clone, Error, Serialize, Deserialize, PartialEq)]
#[error("{code}: {kind:?}")]
pub struct OpsError {
    pub code: &'static str,
    pub kind: OpsErrorKind,
}

impl OpsError {
    pub fn new(kind: OpsErrorKind) -> Self {
        let code = match &kind {
            OpsErrorKind::InvalidConfig { .. } => "MEJEPA_HYGIENE_INVALID_CONFIG",
            OpsErrorKind::MissingColumnFamily { .. } => "MEJEPA_HYGIENE_MISSING_CF",
            OpsErrorKind::CorruptMetadata { .. } => "MEJEPA_HYGIENE_CORRUPT_METADATA",
            OpsErrorKind::TierCodec { .. } => "MEJEPA_HYGIENE_TIER_CODEC",
            OpsErrorKind::QuotaExceeded { .. } => "MEJEPA_HYGIENE_QUOTA_EXCEEDED",
            OpsErrorKind::QuotaUnrecoverable { .. } => "MEJEPA_HYGIENE_QUOTA_UNRECOVERABLE",
            OpsErrorKind::WitnessChainBroken { .. } => "MEJEPA_HYGIENE_WITNESS_BROKEN",
            OpsErrorKind::WitnessArchiveInvalid { .. } => "MEJEPA_HYGIENE_WITNESS_ARCHIVE_INVALID",
            OpsErrorKind::Io { .. } => "MEJEPA_HYGIENE_IO",
            OpsErrorKind::RocksDb { .. } => "MEJEPA_HYGIENE_ROCKSDB",
            OpsErrorKind::Json { .. } => "MEJEPA_HYGIENE_JSON",
            OpsErrorKind::Lock { .. } => "MEJEPA_HYGIENE_LOCK",
            OpsErrorKind::CrossCuttingDeferred { .. } => "MEJEPA_HYGIENE_CROSS_CUTTING_DEFERRED",
        };
        Self { code, kind }
    }

    pub fn invalid(field: impl Into<String>, detail: impl Into<String>) -> Self {
        Self::new(OpsErrorKind::InvalidConfig {
            field: field.into(),
            detail: detail.into(),
        })
    }

    pub fn io(op: &'static str, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::new(OpsErrorKind::Io {
            op,
            path: path.into(),
            detail: source.to_string(),
        })
    }

    pub fn log_context(&self, file: &'static str) {
        tracing::error!(
            error_code = self.code,
            error = ?self.kind,
            file,
            "ME-JEPA hygiene operation failed"
        );
    }
}

pub type OpsResult<T> = Result<T, OpsError>;

impl From<rocksdb::Error> for OpsError {
    fn from(value: rocksdb::Error) -> Self {
        Self::new(OpsErrorKind::RocksDb {
            detail: value.to_string(),
        })
    }
}

impl From<serde_json::Error> for OpsError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(OpsErrorKind::Json {
            detail: value.to_string(),
        })
    }
}

impl From<context_graph_tier_compression::TierCompressionError> for OpsError {
    fn from(value: context_graph_tier_compression::TierCompressionError) -> Self {
        Self::new(OpsErrorKind::TierCodec {
            detail: format!("{}: {value}", value.code()),
        })
    }
}
