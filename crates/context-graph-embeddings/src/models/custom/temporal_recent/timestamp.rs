//! Timestamp parsing and extraction for the Temporal-Recent model.

use chrono::{DateTime, Utc};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelInput;

/// Parse timestamp from instruction string.
///
/// Supports formats:
/// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
/// - Unix epoch: "epoch:1705315800"
pub fn parse_timestamp(instruction: &str) -> Option<DateTime<Utc>> {
    parse_timestamp_result(instruction).ok()
}

/// Parse timestamp from instruction string, returning a fail-fast error on malformed input.
///
/// Supports formats:
/// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
/// - Unix epoch: "epoch:1705315800"
pub fn parse_timestamp_result(instruction: &str) -> EmbeddingResult<DateTime<Utc>> {
    let instruction = instruction.trim();

    // Try ISO 8601 format: "timestamp:2024-01-15T10:30:00Z"
    if let Some(ts_str) = instruction.strip_prefix("timestamp:") {
        let ts_str = ts_str.trim();
        if ts_str.is_empty() {
            return Err(invalid_temporal_instruction(
                "timestamp instruction is empty; expected timestamp:<RFC3339>",
            ));
        }
        return DateTime::parse_from_rfc3339(ts_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|err| {
                invalid_temporal_instruction(format!(
                    "invalid RFC3339 timestamp {ts_str:?}: {err}"
                ))
            });
    }

    // Try Unix epoch: "epoch:1705315800"
    if let Some(epoch_str) = instruction.strip_prefix("epoch:") {
        let epoch_str = epoch_str.trim();
        if epoch_str.is_empty() {
            return Err(invalid_temporal_instruction(
                "epoch instruction is empty; expected epoch:<seconds>",
            ));
        }
        let secs = epoch_str.parse::<i64>().map_err(|err| {
            invalid_temporal_instruction(format!("invalid epoch seconds {epoch_str:?}: {err}"))
        })?;
        return DateTime::from_timestamp(secs, 0).ok_or_else(|| {
            invalid_temporal_instruction(format!(
                "epoch seconds {secs} is outside chrono's DateTime<Utc> range"
            ))
        });
    }

    Err(invalid_temporal_instruction(format!(
        "unsupported temporal instruction {instruction:?}; expected timestamp:<RFC3339> or epoch:<seconds>"
    )))
}

/// Extract timestamp from ModelInput.
///
/// Attempts to parse timestamp from the instruction field:
/// - ISO 8601 format: "timestamp:2024-01-15T10:30:00Z"
/// - Unix epoch: "epoch:1705315800"
///
/// Missing or invalid temporal instructions fail closed; this path never
/// fabricates a current-time value.
pub fn extract_timestamp(input: &ModelInput) -> EmbeddingResult<DateTime<Utc>> {
    match input {
        ModelInput::Text { instruction, .. } => {
            let instruction = instruction.as_deref().ok_or_else(|| {
                invalid_temporal_instruction(
                    "missing temporal instruction; expected timestamp:<RFC3339> or epoch:<seconds>",
                )
            })?;
            parse_timestamp_result(instruction)
        }
        other => Err(invalid_temporal_instruction(format!(
            "unsupported input for temporal timestamp extraction: {other:?}"
        ))),
    }
}

fn invalid_temporal_instruction(message: impl Into<String>) -> EmbeddingError {
    EmbeddingError::ConfigError {
        message: format!("[TEMPORAL_INPUT_INVALID] E2 temporal-recent input rejected: {}", message.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_parse_timestamp_iso8601() {
        let instruction = "timestamp:2024-01-15T10:30:00Z";
        let result = parse_timestamp(instruction);

        assert!(result.is_some(), "Should parse ISO 8601");
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn test_parse_timestamp_unix_epoch() {
        let instruction = "epoch:1705315800";
        let result = parse_timestamp(instruction);

        assert!(result.is_some(), "Should parse Unix epoch");
    }

    #[test]
    fn test_parse_timestamp_invalid() {
        let invalid_inputs = vec![
            "not a timestamp",
            "timestamp:invalid",
            "epoch:notanumber",
            "random text",
            "",
        ];

        for input in invalid_inputs {
            let result = parse_timestamp(input);
            assert!(result.is_none(), "Should return None for '{}'", input);
        }
    }

    #[tokio::test]
    async fn test_extract_timestamp_with_iso8601() {
        let input = ModelInput::text_with_instruction("content", "timestamp:2024-01-15T10:30:00Z")
            .expect("Failed to create input");

        let timestamp = extract_timestamp(&input).expect("timestamp should parse");

        assert_eq!(timestamp.year(), 2024);
        assert_eq!(timestamp.month(), 1);
        assert_eq!(timestamp.day(), 15);
    }

    #[tokio::test]
    async fn test_extract_timestamp_rejects_missing_instruction() {
        let input = ModelInput::text("no timestamp").expect("Failed to create input");

        let err = extract_timestamp(&input).unwrap_err();

        assert!(
            err.to_string().contains("[TEMPORAL_INPUT_INVALID]"),
            "missing timestamp must fail closed, got {err}"
        );
    }
}
