use super::{non_empty, validate_unit, UtmlError, UtmlErrorCode};
use crate::cert::TrainingCertificate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationEvidence {
    pub step: u64,
    pub delta_omega: f32,
    pub delta_xi: f32,
    pub m_t: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolidationMt {
    pub alpha: f32,
    pub current_m_t: f32,
    pub dissipating: bool,
    pub threshold: f32,
    pub window: Vec<ConsolidationEvidence>,
}

impl ConsolidationMt {
    pub fn compute_m_t(
        certs: &[TrainingCertificate],
        alpha: f32,
        threshold: f32,
    ) -> Result<Self, UtmlError> {
        non_empty("training_certificates", certs)?;
        validate_alpha(alpha)?;
        validate_unit("threshold", threshold)?;
        let mut current = certs[0].signal.delta_omega * certs[0].signal.delta_xi;
        validate_unit("initial_m_t", current)?;
        let mut window = Vec::with_capacity(certs.len());
        for cert in certs {
            cert.signal.validate().map_err(|err| {
                UtmlError::new(
                    err.code,
                    format!(
                        "certificate step {} failed signal validation: {}",
                        cert.step, err.message
                    ),
                )
            })?;
            current = update_m_t(
                current,
                cert.signal.delta_omega,
                cert.signal.delta_xi,
                alpha,
            )?;
            window.push(ConsolidationEvidence {
                step: cert.step,
                delta_omega: cert.signal.delta_omega,
                delta_xi: cert.signal.delta_xi,
                m_t: current,
            });
        }
        Ok(Self {
            alpha,
            current_m_t: current,
            dissipating: detect_dissipating(&window, threshold)?,
            threshold,
            window,
        })
    }
}

pub fn update_m_t(
    previous_m_t: f32,
    delta_omega: f32,
    delta_xi: f32,
    alpha: f32,
) -> Result<f32, UtmlError> {
    validate_unit("previous_m_t", previous_m_t)?;
    validate_unit("delta_omega", delta_omega)?;
    validate_unit("delta_xi", delta_xi)?;
    validate_alpha(alpha)?;
    let raw = (1.0 - alpha) * previous_m_t + alpha * (delta_omega * delta_xi);
    validate_unit("updated_m_t", raw)?;
    Ok(raw)
}

pub fn detect_dissipating(
    window: &[ConsolidationEvidence],
    threshold: f32,
) -> Result<bool, UtmlError> {
    non_empty("consolidation_window", window)?;
    validate_unit("threshold", threshold)?;
    for (idx, evidence) in window.iter().enumerate() {
        validate_unit(&format!("window[{idx}].m_t"), evidence.m_t)?;
    }
    if window.len() < 3 {
        return Ok(false);
    }
    let latest = window
        .last()
        .ok_or_else(|| UtmlError::new(UtmlErrorCode::EmptyInput, "consolidation_window is empty"))?
        .m_t;
    if latest > threshold {
        return Ok(false);
    }
    let mut monotone_down = true;
    for pair in window.windows(2) {
        if pair[1].m_t > pair[0].m_t {
            monotone_down = false;
            break;
        }
    }
    Ok(monotone_down)
}

fn validate_alpha(alpha: f32) -> Result<(), UtmlError> {
    if !alpha.is_finite() || alpha <= 0.0 || alpha > 1.0 {
        return Err(UtmlError::new(
            UtmlErrorCode::OutOfRange,
            format!("alpha must be finite and in (0,1]; got {alpha}"),
        ));
    }
    Ok(())
}
