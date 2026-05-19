use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::{OracleOutcome, RealityPrediction, Verdict};

pub const DEFAULT_PAUSE_STATE_PATH: &str =
    "/zfs/archive/contextgraph/state/cgreality/predictions_paused_until.json";
pub const ENV_PAUSE_STATE_PATH: &str = "CONTEXTGRAPH_MEJEPA_PAUSE_PATH";
pub const PAUSE_REASON_CODE: &str = "MEJEPA_VERIFY_PAUSED_BY_OPERATOR";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct PauseState {
    pub paused_until_unix_ms: i64,
    pub set_at_unix_ms: i64,
    pub reason: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PauseReadWarning {
    pub code: String,
    pub message: String,
    pub state_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, tag = "status", rename_all = "snake_case")]
pub enum PauseReadOutcome {
    Active { state: PauseState },
    Inactive { state: Option<PauseState> },
    IgnoredInvalid { warning: PauseReadWarning },
}

impl PauseReadOutcome {
    pub fn active_state(&self) -> Option<&PauseState> {
        match self {
            Self::Active { state } => Some(state),
            Self::Inactive { .. } | Self::IgnoredInvalid { .. } => None,
        }
    }
}

pub fn pause_state_path_from_env_or_default() -> PathBuf {
    match std::env::var(ENV_PAUSE_STATE_PATH) {
        Ok(raw) if !raw.trim().is_empty() => PathBuf::from(raw),
        _ => PathBuf::from(DEFAULT_PAUSE_STATE_PATH),
    }
}

pub fn current_unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

pub fn read_pause_state(path: &Path, now_unix_ms: i64) -> PauseReadOutcome {
    if path.as_os_str().is_empty() {
        return ignored(
            path,
            "MEJEPA_PAUSE_STATE_PATH_EMPTY",
            "pause-state path is empty",
        );
    }
    if !path.exists() {
        return PauseReadOutcome::Inactive { state: None };
    }
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) => {
            return ignored(
                path,
                "MEJEPA_PAUSE_STATE_READ_FAILED",
                &format!("failed to read pause-state file: {err}"),
            );
        }
    };
    if bytes.is_empty() {
        return ignored(
            path,
            "MEJEPA_PAUSE_STATE_EMPTY",
            "pause-state file is empty",
        );
    }
    let state: PauseState = match serde_json::from_slice(&bytes) {
        Ok(state) => state,
        Err(err) => {
            return ignored(
                path,
                "MEJEPA_PAUSE_STATE_DESERIALIZE_FAILED",
                &format!("failed to deserialize pause-state JSON: {err}"),
            );
        }
    };
    if let Err(message) = validate_pause_state(&state) {
        return ignored(path, "MEJEPA_PAUSE_STATE_INVALID", &message);
    }
    if state.paused_until_unix_ms > now_unix_ms {
        PauseReadOutcome::Active { state }
    } else {
        PauseReadOutcome::Inactive { state: Some(state) }
    }
}

pub fn validate_pause_state(state: &PauseState) -> Result<(), String> {
    if state.set_at_unix_ms < 0 {
        return Err("setAtUnixMs must be non-negative".to_string());
    }
    if state.paused_until_unix_ms <= state.set_at_unix_ms {
        return Err("pausedUntilUnixMs must be greater than setAtUnixMs".to_string());
    }
    validate_text("reason", &state.reason)?;
    validate_text("source", &state.source)?;
    Ok(())
}

pub fn prediction_is_operator_paused(prediction: &RealityPrediction) -> bool {
    prediction.verdict == Verdict::Abstain
        && prediction.calibration_version == PAUSE_REASON_CODE
        && prediction.provenance.calibration_version == PAUSE_REASON_CODE
        && prediction
            .provenance
            .active_pointer
            .starts_with("paused_until_")
        && prediction.outcome_set.outcomes == [OracleOutcome::Abstain]
}

fn validate_text(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must contain no control characters"));
    }
    Ok(())
}

fn ignored(path: &Path, code: &str, message: &str) -> PauseReadOutcome {
    PauseReadOutcome::IgnoredInvalid {
        warning: PauseReadWarning {
            code: code.to_string(),
            message: message.to_string(),
            state_path: path.to_path_buf(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_expired_missing_and_malformed_pause_states_are_classified() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("pause.json");
        assert!(matches!(
            read_pause_state(&path, 1_000),
            PauseReadOutcome::Inactive { state: None }
        ));

        std::fs::write(
            &path,
            br#"{"pausedUntilUnixMs":2000,"setAtUnixMs":1000,"reason":"unit","source":"test"}"#,
        )
        .unwrap();
        assert!(matches!(
            read_pause_state(&path, 1_500),
            PauseReadOutcome::Active { .. }
        ));
        assert!(matches!(
            read_pause_state(&path, 2_000),
            PauseReadOutcome::Inactive { state: Some(_) }
        ));

        std::fs::write(&path, b"{").unwrap();
        let outcome = read_pause_state(&path, 1_500);
        assert!(matches!(outcome, PauseReadOutcome::IgnoredInvalid { .. }));
    }
}
