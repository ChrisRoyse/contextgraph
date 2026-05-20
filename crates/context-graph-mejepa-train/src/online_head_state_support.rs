use crate::error::{TrainerError, TrainerErrorCode};
use context_graph_mejepa_cf::CF_MEJEPA_ONLINE_HEAD_STATE;
use rocksdb::DB;
use serde_json::json;

const MAX_ID_BYTES: usize = 512;

pub(crate) fn online_head_cf(db: &DB) -> Result<&rocksdb::ColumnFamily, TrainerError> {
    db.cf_handle(CF_MEJEPA_ONLINE_HEAD_STATE).ok_or_else(|| {
        invalid(
            "rocksdb.column_family",
            "missing CF_MEJEPA_ONLINE_HEAD_STATE",
        )
    })
}

pub(crate) fn validate_optional_id(
    field: &str,
    value: &Option<String>,
) -> Result<(), TrainerError> {
    if let Some(value) = value {
        validate_id(field, value)?;
    }
    Ok(())
}

pub(crate) fn validate_id(field: &str, value: &str) -> Result<(), TrainerError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must be non-empty"));
    }
    if value.len() > MAX_ID_BYTES || value.chars().any(char::is_control) {
        return Err(invalid(field, "must be single-line text up to 512 bytes"));
    }
    Ok(())
}

pub(crate) fn invalid(field: impl Into<String>, message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field.into(),
        "file": "file:crates/context-graph-mejepa-train/src/online_head_state.rs",
        "remediation": "repair the online mistake update; head state must be label/skill-aware and fail closed"
    }))
}

pub(crate) fn map_rocksdb_error(err: rocksdb::Error) -> TrainerError {
    invalid("rocksdb", err.to_string())
}

pub(crate) fn map_bincode_error(err: Box<bincode::ErrorKind>) -> TrainerError {
    invalid("bincode", err.to_string())
}
