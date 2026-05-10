use crate::error::PredictorError;

#[derive(Debug, Clone)]
pub struct VarianceFloorHistory {
    window: usize,
    flags: Vec<bool>,
}

impl VarianceFloorHistory {
    pub fn new(window: usize) -> Result<Self, PredictorError> {
        if window == 0 || window > u8::MAX as usize {
            return Err(PredictorError::ConfigInvalid {
                detail: format!("variance floor window must be in 1..=255; got {window}"),
            });
        }
        Ok(Self {
            window,
            flags: Vec::with_capacity(window),
        })
    }

    pub fn record_pass(
        &mut self,
        low_variance_dim_count: usize,
        total_dims: usize,
        gamma: f32,
    ) -> Result<(), PredictorError> {
        if total_dims == 0 {
            return Err(PredictorError::ConfigInvalid {
                detail: "total_dims must be > 0".to_string(),
            });
        }
        let degenerate = low_variance_dim_count > 0;
        if self.flags.len() == self.window {
            self.flags.remove(0);
        }
        self.flags.push(degenerate);
        if self.flags.len() == self.window && self.flags.iter().all(|flag| *flag) {
            return Err(PredictorError::VicregDegenerate {
                low_variance_dim_count,
                total_dims,
                gamma,
                consecutive_passes: self.window as u8,
            });
        }
        Ok(())
    }

    pub fn flags(&self) -> &[bool] {
        &self.flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_fires_after_consecutive_degenerate_passes() {
        let mut history = VarianceFloorHistory::new(2).expect("valid history");
        history
            .record_pass(1, 10, 1.0)
            .expect("first pass warns only");
        let err = history
            .record_pass(1, 10, 1.0)
            .expect_err("second consecutive pass fails");
        assert_eq!(err.code(), "MEJEPA_PRED_VICREG_DEGENERATE");
    }
}
