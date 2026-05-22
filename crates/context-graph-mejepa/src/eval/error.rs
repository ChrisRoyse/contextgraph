use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvalErrorCode {
    EmptyHoldout,
    InvalidConfig,
    LeakageDetected,
    OracleMissing,
    ReportPersistFail,
    InvalidInput,
    Store,
    ReadbackMismatch,
    Compiler,
    FixtureEvalDisabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalError {
    pub code: EvalErrorCode,
    pub message: String,
}

impl EvalError {
    pub fn new(code: EvalErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn code(&self) -> &'static str {
        match self.code {
            EvalErrorCode::EmptyHoldout => "MEJEPA_EVAL_EMPTY_HOLDOUT",
            EvalErrorCode::InvalidConfig => "MEJEPA_EVAL_INVALID_CONFIG",
            EvalErrorCode::LeakageDetected => "MEJEPA_EVAL_LEAKAGE_DETECTED",
            EvalErrorCode::OracleMissing => "MEJEPA_EVAL_ORACLE_MISSING",
            EvalErrorCode::ReportPersistFail => "MEJEPA_EVAL_REPORT_PERSIST_FAIL",
            EvalErrorCode::InvalidInput => "MEJEPA_EVAL_INVALID_INPUT",
            EvalErrorCode::Store => "MEJEPA_EVAL_STORE",
            EvalErrorCode::ReadbackMismatch => "MEJEPA_EVAL_READBACK_MISMATCH",
            EvalErrorCode::Compiler => "MEJEPA_EVAL_COMPILER",
            EvalErrorCode::FixtureEvalDisabled => "MEJEPA_EVAL_FIXTURE_PATH_DISABLED",
        }
    }

    pub fn log_context(&self, source_file: &'static str) {
        tracing::error!(
            target: "context_graph_mejepa::eval",
            eval_code = self.code(),
            source_file,
            message = %self.message,
            "ME-JEPA eval failure"
        );
    }
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message)
    }
}

impl std::error::Error for EvalError {}

impl From<crate::error::MejepaInferError> for EvalError {
    fn from(value: crate::error::MejepaInferError) -> Self {
        Self::new(EvalErrorCode::Compiler, value.to_string())
    }
}

impl From<rocksdb::Error> for EvalError {
    fn from(value: rocksdb::Error) -> Self {
        Self::new(EvalErrorCode::Store, value.to_string())
    }
}

impl From<bincode::Error> for EvalError {
    fn from(value: bincode::Error) -> Self {
        Self::new(EvalErrorCode::Store, value.to_string())
    }
}

impl From<serde_json::Error> for EvalError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(EvalErrorCode::InvalidInput, value.to_string())
    }
}

impl From<std::io::Error> for EvalError {
    fn from(value: std::io::Error) -> Self {
        Self::new(EvalErrorCode::Store, value.to_string())
    }
}
