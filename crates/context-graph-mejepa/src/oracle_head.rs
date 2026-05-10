use candle_core::Tensor;
use candle_nn::{linear, ops, Linear, Module, VarBuilder};

use crate::config::PANEL_DIM;
use crate::data_models::{OracleLogits, PredictedPanel};
use crate::error::{NanSource, PredictorError};
use crate::predictor::ensure_finite;

pub const ORACLE_HEAD_NUM_TESTS_MIN: usize = 1;
pub const ORACLE_HEAD_NUM_TESTS_MAX: usize = 10_000;

#[derive(Debug, Clone)]
pub struct OracleHead {
    project_to_logits: Linear,
    num_tests: usize,
}

impl OracleHead {
    pub fn new(num_tests: usize, vb: VarBuilder) -> Result<Self, PredictorError> {
        if !(ORACLE_HEAD_NUM_TESTS_MIN..=ORACLE_HEAD_NUM_TESTS_MAX).contains(&num_tests) {
            return Err(PredictorError::ConfigInvalid {
                detail: format!(
                    "num_tests must be in {ORACLE_HEAD_NUM_TESTS_MIN}..={ORACLE_HEAD_NUM_TESTS_MAX}; got {num_tests}"
                ),
            });
        }
        Ok(Self {
            project_to_logits: linear(PANEL_DIM, num_tests + 1, vb.pp("project_to_logits"))?,
            num_tests,
        })
    }

    pub fn num_tests(&self) -> usize {
        self.num_tests
    }

    pub fn trainable_parameter_count(&self) -> usize {
        if self.project_to_logits.bias().is_some() {
            2
        } else {
            1
        }
    }

    pub fn predict_logits(
        &self,
        predicted: &PredictedPanel,
    ) -> Result<OracleLogits, PredictorError> {
        validate_predicted_tensor(&predicted.tensor, "oracle_head.predicted")?;
        let logits = self.project_to_logits.forward(&predicted.tensor)?;
        ensure_finite(&logits, NanSource::OracleHead, None, "oracle_logits")?;
        Ok(OracleLogits {
            batch_size: predicted.batch_size,
            logits_dim: self.num_tests + 1,
            tensor: logits,
        })
    }

    pub fn predict_probabilities(
        &self,
        logits: &OracleLogits,
    ) -> Result<OracleLogits, PredictorError> {
        if logits.tensor.dims().len() != 2 || logits.tensor.dims()[1] != self.num_tests + 1 {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "oracle probabilities expect logits shape (B, {}); got {:?}",
                    self.num_tests + 1,
                    logits.tensor.dims()
                ),
                observed: serde_json::json!({ "logits": logits.tensor.dims() }),
                expected_panel_dim: PANEL_DIM,
            });
        }
        let probabilities = ops::softmax(&logits.tensor, 1)?;
        ensure_finite(
            &probabilities,
            NanSource::OracleHead,
            None,
            "oracle_probabilities",
        )?;
        Ok(OracleLogits {
            tensor: probabilities,
            batch_size: logits.batch_size,
            logits_dim: logits.logits_dim,
        })
    }
}

fn validate_predicted_tensor(tensor: &Tensor, tensor_name: &str) -> Result<(), PredictorError> {
    if tensor.dims().len() != 2 || tensor.dims()[1] != PANEL_DIM {
        return Err(PredictorError::DimMismatch {
            detail: format!(
                "{tensor_name} expects (B, {PANEL_DIM}); got {:?}",
                tensor.dims()
            ),
            observed: serde_json::json!({ tensor_name: tensor.dims() }),
            expected_panel_dim: PANEL_DIM,
        });
    }
    Ok(())
}
