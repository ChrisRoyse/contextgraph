use candle_core::{DType, Device, Tensor};

use crate::error::TrainerError;

#[derive(Debug, Clone)]
pub struct AuxHeadEnsemble {
    pub head_names: Vec<String>,
}

impl Default for AuxHeadEnsemble {
    fn default() -> Self {
        Self {
            head_names: vec![
                "warnings".into(),
                "runtime".into(),
                "rss".into(),
                "traceback".into(),
                "hunk".into(),
                "collateral".into(),
                "reasoning".into(),
                "operator_match".into(),
            ],
        }
    }
}

impl AuxHeadEnsemble {
    pub fn forward_logits(&self, hidden: &Tensor) -> Result<Tensor, TrainerError> {
        let batch = *hidden.dims().first().unwrap_or(&1);
        Tensor::zeros((batch, self.head_names.len()), DType::F32, &Device::Cpu)
            .map_err(TrainerError::from)
    }
}
