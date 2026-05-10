use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibrationCertificate {
    pub embedder: EmbedderId,
    pub dataset_sha256: String,
    pub evaluator_sha256: String,
    pub signed_by: String,
    pub signature_sha256: String,
    pub ece: f32,
    pub min_accuracy: f32,
}

impl CalibrationCertificate {
    pub fn validate_for(&self, embedder: EmbedderId) -> EmbedResult<()> {
        if self.embedder != embedder {
            return Err(EmbedError::invalid(
                "CalibrationCertificate.embedder",
                format!("certificate is for {}, not {embedder}", self.embedder),
                "use the certificate generated for the requested learner-state embedder",
            ));
        }
        validate_sha("dataset_sha256", &self.dataset_sha256)?;
        validate_sha("evaluator_sha256", &self.evaluator_sha256)?;
        validate_sha("signature_sha256", &self.signature_sha256)?;
        if self.signed_by.trim().is_empty() {
            return Err(EmbedError::invalid(
                "CalibrationCertificate.signed_by",
                "signed_by is empty",
                "record the verifier identity or key id that produced the certificate",
            ));
        }
        if !self.ece.is_finite() || !(0.0..=1.0).contains(&self.ece) {
            return Err(EmbedError::invalid(
                "CalibrationCertificate.ece",
                format!("ECE must be in [0,1], got {}", self.ece),
                "rerun calibration and persist a bounded expected-calibration-error value",
            ));
        }
        if !self.min_accuracy.is_finite() || !(0.0..=1.0).contains(&self.min_accuracy) {
            return Err(EmbedError::invalid(
                "CalibrationCertificate.min_accuracy",
                format!("min_accuracy must be in [0,1], got {}", self.min_accuracy),
                "rerun calibration and persist a bounded accuracy value",
            ));
        }
        if self.ece > 0.12 || self.min_accuracy < 0.70 {
            return Err(EmbedError::E17Uncalibrated {
                cert_path: Path::new("<in-memory>").to_path_buf(),
                message: format!(
                    "certificate quality below gate: ece={} min_accuracy={}",
                    self.ece, self.min_accuracy
                ),
                remediation: "retrain/recalibrate E17 before using agent_state_score",
            });
        }
        Ok(())
    }
}

pub fn verify_calibration_certificate(
    cert_path: impl AsRef<Path>,
    embedder: EmbedderId,
) -> EmbedResult<CalibrationCertificate> {
    let cert_path = cert_path.as_ref();
    let text = std::fs::read_to_string(cert_path).map_err(|err| EmbedError::E17Uncalibrated {
        cert_path: cert_path.to_path_buf(),
        message: err.to_string(),
        remediation: "write a signed calibration certificate under memory/decisions before calling learner-state scoring",
    })?;
    let cert: CalibrationCertificate =
        serde_json::from_str(&text).map_err(|err| EmbedError::E17Uncalibrated {
            cert_path: cert_path.to_path_buf(),
            message: err.to_string(),
            remediation: "regenerate the calibration certificate JSON with the current schema",
        })?;
    cert.validate_for(embedder)?;
    Ok(cert)
}

pub fn agent_state_score(
    cert_path: impl AsRef<Path>,
    valence: f32,
    arousal: f32,
) -> EmbedResult<f32> {
    verify_calibration_certificate(cert_path, EmbedderId::E17)?;
    if !valence.is_finite() || !arousal.is_finite() {
        return Err(EmbedError::invalid(
            "agent_state_score.inputs",
            "valence/arousal must be finite",
            "pass finite normalized E17 scalar outputs",
        ));
    }
    if !(-1.0..=1.0).contains(&valence) || !(0.0..=1.0).contains(&arousal) {
        return Err(EmbedError::invalid(
            "agent_state_score.inputs",
            format!("valence={valence}, arousal={arousal} outside expected ranges"),
            "normalize valence to [-1,1] and arousal to [0,1]",
        ));
    }
    let inverted_u = 1.0 - (2.0 * (arousal - 0.5)).abs();
    Ok(((valence + 1.0) * 0.5 * inverted_u).clamp(0.0, 1.0))
}

fn validate_sha(field: &'static str, value: &str) -> EmbedResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(EmbedError::invalid(
            field,
            format!("expected 64 lowercase hex chars, got {value:?}"),
            "write a real lowercase SHA-256 digest",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e17_score_requires_certificate() {
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("missing.json");
        assert_eq!(
            agent_state_score(&missing, 0.1, 0.5).unwrap_err().code(),
            "MEJEPA_EMBED_E17_UNCALIBRATED"
        );
    }

    #[test]
    fn valid_certificate_scores_agent_state() {
        let temp = tempfile::tempdir().unwrap();
        let cert_path = temp.path().join("cert.json");
        let cert = CalibrationCertificate {
            embedder: EmbedderId::E17,
            dataset_sha256: "a".repeat(64),
            evaluator_sha256: "b".repeat(64),
            signed_by: "unit-test-verifier".into(),
            signature_sha256: "c".repeat(64),
            ece: 0.05,
            min_accuracy: 0.90,
        };
        std::fs::write(&cert_path, serde_json::to_string(&cert).unwrap()).unwrap();
        let score = agent_state_score(&cert_path, 0.5, 0.5).unwrap();
        assert!(score > 0.7);
    }
}
