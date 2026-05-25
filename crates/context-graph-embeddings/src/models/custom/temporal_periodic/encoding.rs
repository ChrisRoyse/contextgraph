//! Fourier encoding and timestamp parsing for Temporal-Periodic model.
//!
//! Contains the core embedding computation and timestamp extraction logic.

use chrono::{DateTime, Utc};

use crate::error::{EmbeddingError, EmbeddingResult};
use crate::types::ModelInput;

use super::constants::TEMPORAL_PERIODIC_DIMENSION;
use super::model::TemporalPeriodicModel;

impl TemporalPeriodicModel {
    /// Compute the Fourier embedding for a given timestamp.
    ///
    /// The embedding encodes cyclical patterns at multiple time scales
    /// using sin/cos pairs (Fourier basis functions).
    pub(crate) fn compute_fourier_embedding(&self, timestamp: DateTime<Utc>) -> Vec<f32> {
        let timestamp_secs = timestamp.timestamp() as f64;
        let mut vector = Vec::with_capacity(TEMPORAL_PERIODIC_DIMENSION);

        for &period in &self.periods {
            let period_f64 = period as f64;

            // Phase angle in range [0, 2pi)
            // Use modulo to get position within the cycle
            let position_in_cycle = timestamp_secs.rem_euclid(period_f64);
            let phase = 2.0 * std::f64::consts::PI * position_in_cycle / period_f64;

            // Generate harmonics: sin(n*theta), cos(n*theta) for n = 1..num_harmonics
            for n in 1..=self.num_harmonics {
                let n_f64 = n as f64;
                let angle = n_f64 * phase;
                let sin_val = angle.sin() as f32;
                let cos_val = angle.cos() as f32;
                vector.push(sin_val);
                vector.push(cos_val);
            }
        }

        // Pad to exactly 512 dimensions if needed
        while vector.len() < TEMPORAL_PERIODIC_DIMENSION {
            vector.push(0.0);
        }

        // L2 normalize
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > f32::EPSILON {
            for v in &mut vector {
                *v /= norm;
            }
        }

        vector
    }

    /// Extract timestamp from ModelInput.
    ///
    /// Attempts to parse timestamp from the instruction field:
    /// - ISO 8601 format: "timestamp:2024-01-15T10:30:00Z"
    /// - Unix epoch: "epoch:1705315800"
    ///
    /// Missing or invalid temporal instructions fail closed; this path never
    /// fabricates a current-time value.
    pub(crate) fn extract_timestamp(&self, input: &ModelInput) -> EmbeddingResult<DateTime<Utc>> {
        match input {
            ModelInput::Text { instruction, .. } => {
                let instruction = instruction.as_deref().ok_or_else(|| {
                    invalid_temporal_instruction(
                        "missing temporal instruction; expected timestamp:<RFC3339> or epoch:<seconds>",
                    )
                })?;
                Self::parse_timestamp_result(instruction)
            }
            other => Err(invalid_temporal_instruction(format!(
                "unsupported input for temporal timestamp extraction: {other:?}"
            ))),
        }
    }

    /// Parse timestamp from instruction string.
    ///
    /// Supports formats:
    /// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
    /// - Unix epoch: "epoch:1705315800"
    #[allow(dead_code)]
    pub(crate) fn parse_timestamp(instruction: &str) -> Option<DateTime<Utc>> {
        Self::parse_timestamp_result(instruction).ok()
    }

    /// Parse timestamp from instruction string, returning a fail-fast error on malformed input.
    ///
    /// Supports formats:
    /// - ISO 8601: "timestamp:2024-01-15T10:30:00Z"
    /// - Unix epoch: "epoch:1705315800"
    pub(crate) fn parse_timestamp_result(instruction: &str) -> EmbeddingResult<DateTime<Utc>> {
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
}

fn invalid_temporal_instruction(message: impl Into<String>) -> EmbeddingError {
    EmbeddingError::ConfigError {
        message: format!(
            "[TEMPORAL_INPUT_INVALID] E3 temporal-periodic input rejected: {}",
            message.into()
        ),
    }
}
