use super::error::{EvalError, EvalErrorCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LatencyTelemetry {
    pub p50_ms: f32,
    pub p95_ms: f32,
    pub p99_ms: f32,
    pub max_ms: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionTelemetryWindow {
    pub window_id: String,
    pub captured_at_unix_ms: i64,
    pub corpus_sha: String,
    pub sample_count: usize,
    pub conformal_ece: f32,
    pub ood_auc: f32,
    pub gtau_pass_rate: f32,
    pub prediction_oracle_agreement: f32,
    pub ship_gate_target: f32,
    pub ship_gate_passed: bool,
    pub latency: LatencyTelemetry,
    pub vram_free_bytes: u64,
    pub vram_total_bytes: u64,
    pub vram_required_bytes: u64,
    pub source: String,
}

impl ProductionTelemetryWindow {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.window_id.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry window_id must be non-empty",
            ));
        }
        if self.window_id.chars().any(char::is_control) {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry window_id must be single-line text",
            ));
        }
        if self.corpus_sha.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry corpus_sha must be non-empty",
            ));
        }
        if self.sample_count == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry sample_count must be greater than zero",
            ));
        }
        validate_unit("conformal_ece", self.conformal_ece)?;
        validate_unit("ood_auc", self.ood_auc)?;
        validate_unit("gtau_pass_rate", self.gtau_pass_rate)?;
        validate_unit(
            "prediction_oracle_agreement",
            self.prediction_oracle_agreement,
        )?;
        validate_unit("ship_gate_target", self.ship_gate_target)?;
        let computed_ship_gate_passed = self.prediction_oracle_agreement >= self.ship_gate_target;
        if self.ship_gate_passed != computed_ship_gate_passed {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "production telemetry ship_gate_passed={} contradicts prediction_oracle_agreement={} and ship_gate_target={}",
                    self.ship_gate_passed, self.prediction_oracle_agreement, self.ship_gate_target
                ),
            ));
        }
        self.latency.validate()?;
        if self.vram_total_bytes == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry vram_total_bytes must be greater than zero",
            ));
        }
        if self.vram_required_bytes == 0 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry vram_required_bytes must be greater than zero",
            ));
        }
        if self.vram_free_bytes > self.vram_total_bytes {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "production telemetry vram_free_bytes {} exceeds total {}",
                    self.vram_free_bytes, self.vram_total_bytes
                ),
            ));
        }
        if self.vram_required_bytes > self.vram_total_bytes {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "production telemetry vram_required_bytes {} exceeds total {}",
                    self.vram_required_bytes, self.vram_total_bytes
                ),
            ));
        }
        if self.source.trim().is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "production telemetry source must be non-empty",
            ));
        }
        Ok(())
    }
}

impl LatencyTelemetry {
    pub fn validate(&self) -> Result<(), EvalError> {
        for (name, value) in [
            ("latency.p50_ms", self.p50_ms),
            ("latency.p95_ms", self.p95_ms),
            ("latency.p99_ms", self.p99_ms),
            ("latency.max_ms", self.max_ms),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("{name} must be finite and non-negative; got {value}"),
                ));
            }
        }
        if !(self.p50_ms <= self.p95_ms && self.p95_ms <= self.p99_ms && self.p99_ms <= self.max_ms)
        {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "latency percentiles must be monotonic; got p50={} p95={} p99={} max={}",
                    self.p50_ms, self.p95_ms, self.p99_ms, self.max_ms
                ),
            ));
        }
        Ok(())
    }
}

fn validate_unit(name: &str, value: f32) -> Result<(), EvalError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!("{name} must be finite and in [0,1]; got {value}"),
        ));
    }
    Ok(())
}
