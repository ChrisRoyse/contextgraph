use serde::{Deserialize, Serialize};

use crate::compiler::TrainCertSummary;
use crate::error::MejepaInferError;

pub const HEALTH_FLOOR: f32 = 0.4;
pub const MULTIPLIER_LO: f32 = 0.10;
pub const MULTIPLIER_HI: f32 = 0.95;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainHealthSummary {
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
    pub degraded_status: bool,
    pub source_count: usize,
    pub bootstrap_used: bool,
}

pub fn compute_train_health(
    certs: &[TrainCertSummary],
    bootstrap_omega: f32,
    bootstrap_xi: f32,
) -> Result<TrainHealthSummary, MejepaInferError> {
    if certs.is_empty() {
        validate_unit("bootstrap_omega", bootstrap_omega)?;
        validate_unit("bootstrap_xi", bootstrap_xi)?;
        return Ok(TrainHealthSummary {
            delta_omega_mean: bootstrap_omega,
            delta_xi_mean: bootstrap_xi,
            degraded_status: true,
            source_count: 0,
            bootstrap_used: true,
        });
    }
    let mut omega = 0.0f32;
    let mut xi = 0.0f32;
    for cert in certs {
        validate_unit("delta_omega", cert.delta_omega)?;
        validate_unit("delta_xi", cert.delta_xi)?;
        omega += cert.delta_omega;
        xi += cert.delta_xi;
    }
    omega /= certs.len() as f32;
    xi /= certs.len() as f32;
    Ok(TrainHealthSummary {
        delta_omega_mean: omega,
        delta_xi_mean: xi,
        degraded_status: omega < HEALTH_FLOOR || xi < HEALTH_FLOOR,
        source_count: certs.len(),
        bootstrap_used: false,
    })
}

pub fn calibrated_confidence_multiplier(omega: f32, xi: f32) -> Result<f32, MejepaInferError> {
    validate_unit("omega", omega)?;
    validate_unit("xi", xi)?;
    Ok((omega * xi).clamp(MULTIPLIER_LO, MULTIPLIER_HI))
}

pub fn calibrated_confidence(
    raw_conf: f32,
    conv_rate: f32,
    strategy_agreement: f32,
    evidence_factor: f32,
    omega: f32,
    xi: f32,
) -> Result<f32, MejepaInferError> {
    for (name, value) in [
        ("raw_conf", raw_conf),
        ("conv_rate", conv_rate),
        ("strategy_agreement", strategy_agreement),
        ("evidence_factor", evidence_factor),
        ("omega", omega),
        ("xi", xi),
    ] {
        validate_unit(name, value)?;
    }
    Ok(
        (raw_conf * conv_rate * strategy_agreement * evidence_factor * omega * xi)
            .clamp(MULTIPLIER_LO, MULTIPLIER_HI),
    )
}

pub fn evidence_factor(num_paths: u32, threshold: u32) -> Result<f32, MejepaInferError> {
    if threshold == 0 {
        return Err(MejepaInferError::InvalidInput {
            field: "threshold".to_string(),
            detail: "evidence_factor threshold must be >= 1".to_string(),
        });
    }
    Ok(0.5 + 0.5 * (num_paths as f32 / threshold as f32).min(1.0))
}

fn validate_unit(name: &str, value: f32) -> Result<(), MejepaInferError> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(MejepaInferError::NanDetected {
            nan_source: name.to_string(),
            detail: format!("{name} must be finite and in [0, 1]; got {value}"),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_certs_use_bootstrap_and_degraded() {
        let summary = compute_train_health(&[], 0.5, 0.5).unwrap();
        assert!(summary.bootstrap_used);
        assert!(summary.degraded_status);
    }

    #[test]
    fn health_boundary_is_strict() {
        let certs = vec![TrainCertSummary {
            step: 1,
            delta_omega: 0.4,
            delta_xi: 0.4,
            witness_offset: 0,
        }];
        let summary = compute_train_health(&certs, 0.5, 0.5).unwrap();
        assert!(!summary.degraded_status);
    }

    #[test]
    fn multiplier_clamps() {
        assert_eq!(calibrated_confidence_multiplier(0.1, 0.1).unwrap(), 0.10);
        assert_eq!(calibrated_confidence_multiplier(1.0, 1.0).unwrap(), 0.95);
    }
}
