use serde::{Deserialize, Serialize};

use crate::config::{VICREG_COV_LAMBDA, VICREG_GAMMA, VICREG_INV_LAMBDA, VICREG_VAR_LAMBDA};
use crate::error::LossError;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VicregLambdas {
    pub var: f32,
    pub cov: f32,
    pub inv: f32,
    pub gamma: f32,
}

impl Default for VicregLambdas {
    fn default() -> Self {
        Self {
            var: VICREG_VAR_LAMBDA,
            cov: VICREG_COV_LAMBDA,
            inv: VICREG_INV_LAMBDA,
            gamma: VICREG_GAMMA,
        }
    }
}

impl VicregLambdas {
    pub fn validate_finite(&self) -> Result<(), LossError> {
        if self.var.is_finite()
            && self.cov.is_finite()
            && self.inv.is_finite()
            && self.gamma.is_finite()
        {
            Ok(())
        } else {
            Err(LossError::NonFiniteLambda {
                lambdas_dump: format!("{self:?}"),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LossOutputs {
    pub l_predict: f32,
    pub l_variance: f32,
    pub l_covariance: f32,
    pub l_invariance: f32,
    pub l_total: f32,
    pub low_variance_dim_count: usize,
    pub formula_check: bool,
}

impl LossOutputs {
    pub fn finite(&self) -> bool {
        self.l_predict.is_finite()
            && self.l_variance.is_finite()
            && self.l_covariance.is_finite()
            && self.l_invariance.is_finite()
            && self.l_total.is_finite()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardPassEvidence {
    pub source_of_truth: String,
    pub input_panel_path: String,
    pub panel_dim: usize,
    pub batch_size: usize,
    pub warmup_calls: usize,
    pub measured_calls: usize,
    pub forward_latency_ms_p50: f32,
    pub forward_latency_ms_p99: f32,
    pub output_sha256_f32: String,
    pub output_finite: bool,
    pub output_dtype: String,
    pub vram_resident_bytes: u64,
    pub dod_pass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VicregLossEvidence {
    pub source_of_truth: String,
    pub input_panel_path: String,
    pub lambdas: VicregLambdas,
    pub outputs: LossOutputs,
    pub finite: bool,
    pub total_in_dod_band: bool,
    pub dod_pass: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeterminismEvidence {
    pub source_of_truth: String,
    pub sha256_a: String,
    pub sha256_b: String,
    pub byte_equal: bool,
    pub passes: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdversarialEvidence {
    pub case_name: String,
    pub before_state: serde_json::Value,
    pub after_state: serde_json::Value,
    pub expected_error_code: String,
    pub actual_error_code: String,
    pub passes: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lambdas_default_and_json_roundtrip() {
        let lambdas = VicregLambdas::default();
        assert_eq!(lambdas.var, 25.0);
        assert_eq!(lambdas.cov, 1.0);
        assert_eq!(lambdas.inv, 25.0);
        assert_eq!(lambdas.gamma, 1.0);
        let json = serde_json::to_string(&lambdas).expect("serialize lambdas");
        let readback: VicregLambdas = serde_json::from_str(&json).expect("deserialize lambdas");
        assert_eq!(readback, lambdas);
    }
}
