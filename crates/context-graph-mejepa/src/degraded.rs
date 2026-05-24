use serde::{Deserialize, Serialize};

use crate::compiler::TrainCertSummary;
use crate::error::MejepaInferError;

pub const HEALTH_FLOOR: f32 = 0.4;
pub const MULTIPLIER_LO: f32 = 0.10;
pub const MULTIPLIER_HI: f32 = 0.95;
/// #699: when no real predictor-update certs are available (diagnostic mode
/// or fresh-DB bootstrap), the confidence multiplier MUST be 1.0 — i.e.,
/// "no adjustment". Scaling confidence by a pseudo-value derived from
/// hardcoded learning signals is a silent-success bug (see #683).
pub const MULTIPLIER_DIAGNOSTIC_NEUTRAL: f32 = 1.0;

/// #699: tells consumers which path produced `TrainHealthSummary` so that
/// callers can refuse to scale predictor confidence by pseudo-values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum TrainHealthSource {
    /// At least one cert with `predictor_parameter_update_count > 0` was
    /// averaged. `delta_omega_mean` / `delta_xi_mean` are real signals.
    Measured,
    /// Certs were present but every single one had
    /// `predictor_parameter_update_count == 0` — the trainer is in
    /// `DIAGNOSTIC_CERTIFICATE_ONLY` mode (see #683). The summary's
    /// `delta_*_mean` fields carry neutral 1.0 placeholders and the
    /// confidence multiplier MUST be `MULTIPLIER_DIAGNOSTIC_NEUTRAL`.
    DiagnosticCertificateOnlyNeutral,
    /// No certs at all. Bootstrap config values were placed in `delta_*_mean`
    /// for backwards compatibility, but they ALSO are not measurements.
    /// Treat the same as `DiagnosticCertificateOnlyNeutral` at the multiplier
    /// site — bootstrap config is a default, not a signal.
    BootstrapNoData,
}

impl TrainHealthSource {
    pub fn as_screaming_snake(&self) -> &'static str {
        match self {
            Self::Measured => "MEASURED",
            Self::DiagnosticCertificateOnlyNeutral => "DIAGNOSTIC_CERTIFICATE_ONLY_NEUTRAL",
            Self::BootstrapNoData => "BOOTSTRAP_NO_DATA",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainHealthSummary {
    pub delta_omega_mean: f32,
    pub delta_xi_mean: f32,
    pub degraded_status: bool,
    pub source_count: usize,
    pub bootstrap_used: bool,
    /// #699: required to gate the confidence multiplier honestly. See
    /// `TrainHealthSource` and `effective_confidence_multiplier`.
    pub source: TrainHealthSource,
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
            source: TrainHealthSource::BootstrapNoData,
        });
    }
    // #699: filter out diagnostic-only certs whose `predictor_parameter_update_count`
    // is zero. Including them would average pseudo-values from the
    // hardcoded learning-signal path (#683) into the confidence multiplier.
    let measured: Vec<&TrainCertSummary> = certs
        .iter()
        .filter(|c| c.predictor_parameter_update_count > 0)
        .collect();
    if measured.is_empty() {
        return Ok(TrainHealthSummary {
            delta_omega_mean: 1.0,
            delta_xi_mean: 1.0,
            degraded_status: true,
            source_count: certs.len(),
            bootstrap_used: false,
            source: TrainHealthSource::DiagnosticCertificateOnlyNeutral,
        });
    }
    let mut omega = 0.0f32;
    let mut xi = 0.0f32;
    for cert in &measured {
        validate_unit("delta_omega", cert.delta_omega)?;
        validate_unit("delta_xi", cert.delta_xi)?;
        omega += cert.delta_omega;
        xi += cert.delta_xi;
    }
    omega /= measured.len() as f32;
    xi /= measured.len() as f32;
    Ok(TrainHealthSummary {
        delta_omega_mean: omega,
        delta_xi_mean: xi,
        degraded_status: omega < HEALTH_FLOOR || xi < HEALTH_FLOOR,
        source_count: measured.len(),
        bootstrap_used: false,
        source: TrainHealthSource::Measured,
    })
}

pub fn calibrated_confidence_multiplier(omega: f32, xi: f32) -> Result<f32, MejepaInferError> {
    validate_unit("omega", omega)?;
    validate_unit("xi", xi)?;
    Ok((omega * xi).clamp(MULTIPLIER_LO, MULTIPLIER_HI))
}

/// #699: source-aware multiplier. Returns `MULTIPLIER_DIAGNOSTIC_NEUTRAL`
/// (= 1.0, no adjustment) when no real training is reflected in the
/// summary; otherwise delegates to `calibrated_confidence_multiplier`.
/// Compiler.rs must use this helper instead of calling
/// `calibrated_confidence_multiplier` directly with `summary.delta_*_mean`.
pub fn effective_confidence_multiplier(
    summary: &TrainHealthSummary,
) -> Result<f32, MejepaInferError> {
    match summary.source {
        TrainHealthSource::Measured => {
            calibrated_confidence_multiplier(summary.delta_omega_mean, summary.delta_xi_mean)
        }
        TrainHealthSource::DiagnosticCertificateOnlyNeutral | TrainHealthSource::BootstrapNoData => {
            Ok(MULTIPLIER_DIAGNOSTIC_NEUTRAL)
        }
    }
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

    fn measured_cert(omega: f32, xi: f32) -> TrainCertSummary {
        TrainCertSummary {
            step: 1,
            delta_omega: omega,
            delta_xi: xi,
            witness_offset: 0,
            predictor_parameter_update_count: 1,
        }
    }

    fn diagnostic_cert(omega: f32, xi: f32) -> TrainCertSummary {
        TrainCertSummary {
            step: 1,
            delta_omega: omega,
            delta_xi: xi,
            witness_offset: 0,
            predictor_parameter_update_count: 0,
        }
    }

    #[test]
    fn health_boundary_is_strict() {
        let certs = vec![measured_cert(0.4, 0.4)];
        let summary = compute_train_health(&certs, 0.5, 0.5).unwrap();
        assert!(!summary.degraded_status);
        assert_eq!(summary.source, TrainHealthSource::Measured);
    }

    #[test]
    fn multiplier_clamps() {
        assert_eq!(calibrated_confidence_multiplier(0.1, 0.1).unwrap(), 0.10);
        assert_eq!(calibrated_confidence_multiplier(1.0, 1.0).unwrap(), 0.95);
    }

    // #699 regression: when every cert is diagnostic-only
    // (predictor_parameter_update_count == 0), compute_train_health must
    // return source=DiagnosticCertificateOnlyNeutral with neutral mean
    // values, and effective_confidence_multiplier must return 1.0 (no
    // adjustment) rather than scaling confidence by pseudo-values.
    #[test]
    fn all_diagnostic_certs_collapse_to_diagnostic_neutral_source() {
        let certs = vec![
            diagnostic_cert(0.9, 0.9),
            diagnostic_cert(0.8, 0.85),
            diagnostic_cert(0.7, 0.7),
        ];
        let summary = compute_train_health(&certs, 0.5, 0.5).unwrap();
        assert_eq!(
            summary.source,
            TrainHealthSource::DiagnosticCertificateOnlyNeutral
        );
        assert!(summary.degraded_status);
        assert_eq!(summary.delta_omega_mean, 1.0);
        assert_eq!(summary.delta_xi_mean, 1.0);
        assert!(!summary.bootstrap_used);
        let mult = effective_confidence_multiplier(&summary).unwrap();
        assert_eq!(mult, MULTIPLIER_DIAGNOSTIC_NEUTRAL);
        assert_eq!(mult, 1.0);
    }

    #[test]
    fn empty_certs_source_is_bootstrap_no_data_and_multiplier_is_neutral() {
        let summary = compute_train_health(&[], 0.5, 0.5).unwrap();
        assert_eq!(summary.source, TrainHealthSource::BootstrapNoData);
        assert!(summary.bootstrap_used);
        let mult = effective_confidence_multiplier(&summary).unwrap();
        assert_eq!(mult, MULTIPLIER_DIAGNOSTIC_NEUTRAL);
    }

    #[test]
    fn mixed_certs_use_only_measured_subset_for_mean() {
        let certs = vec![
            diagnostic_cert(0.1, 0.1),
            measured_cert(0.9, 0.9),
            measured_cert(0.8, 0.8),
        ];
        let summary = compute_train_health(&certs, 0.5, 0.5).unwrap();
        assert_eq!(summary.source, TrainHealthSource::Measured);
        // mean of (0.9, 0.8) = 0.85, not the diluted (0.9+0.8+0.1)/3=0.6
        assert!((summary.delta_omega_mean - 0.85).abs() < 1e-5);
        assert!((summary.delta_xi_mean - 0.85).abs() < 1e-5);
        assert_eq!(summary.source_count, 2);
        let mult = effective_confidence_multiplier(&summary).unwrap();
        assert!((mult - (0.85_f32 * 0.85_f32).clamp(MULTIPLIER_LO, MULTIPLIER_HI)).abs() < 1e-5);
    }
}
