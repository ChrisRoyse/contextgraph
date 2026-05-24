use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::config::GRAD_NORM_NOISE_FLOOR;
use crate::error::PredictorError;
use crate::frozen_target::FrozenTargetAdapter;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TensorId(pub String);

pub trait InstrumentGradHandle {
    fn instrument_id(&self) -> &str;
    fn tensor_ids(&self) -> &[TensorId];
}

#[derive(Debug, Clone)]
pub struct NoOpGradHandle {
    label: String,
}

impl NoOpGradHandle {
    pub fn with_label(label: &str) -> Self {
        Self {
            label: label.to_string(),
        }
    }
}

impl InstrumentGradHandle for NoOpGradHandle {
    fn instrument_id(&self) -> &str {
        &self.label
    }

    fn tensor_ids(&self) -> &[TensorId] {
        &[]
    }
}

#[derive(Debug, Clone, Default)]
pub struct GradStore {
    norms: BTreeMap<TensorId, f32>,
}

impl GradStore {
    pub fn insert_norm(&mut self, tensor_id: TensorId, norm: f32) -> Result<(), PredictorError> {
        if !norm.is_finite() || norm < 0.0 {
            return Err(PredictorError::FrozenTargetGrad {
                instrument_id: tensor_id.0,
                grad_norm: norm,
                threshold: GRAD_NORM_NOISE_FLOOR,
                fix_at: "file:crates/context-graph-mejepa/src/grad_hook.rs".to_string(),
            });
        }
        self.norms.insert(tensor_id, norm);
        Ok(())
    }

    pub fn get_norm(&self, tensor_id: &TensorId) -> Option<f32> {
        self.norms.get(tensor_id).copied()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GradientHookReport {
    pub instruments_checked: usize,
    pub tensors_checked: usize,
    pub max_grad_norm: f32,
    pub threshold: f32,
    pub passes: bool,
}

pub fn run_gradient_hook(
    adapter: &FrozenTargetAdapter,
    grads: &GradStore,
) -> Result<GradientHookReport, PredictorError> {
    let mut tensors_checked = 0usize;
    let mut max_grad_norm = 0.0f32;
    for handle in adapter.grad_handles() {
        for tensor_id in handle.tensor_ids() {
            tensors_checked += 1;
            // #620: previously `grads.get_norm(tensor_id).unwrap_or(0.0)` —
            // a registered tensor with no GradStore entry was silently
            // treated as a measured zero. The doctrinal contract is: if
            // the adapter claims a tensor is supervised, the GradStore
            // MUST have measured it. Fail closed otherwise.
            let grad_norm = grads.get_norm(tensor_id).ok_or_else(|| {
                PredictorError::FrozenTargetGradUnmeasured {
                    instrument_id: handle.instrument_id().to_string(),
                    tensor_id: tensor_id.0.clone(),
                    fix_at: "file:crates/context-graph-mejepa/src/grad_hook.rs".to_string(),
                }
            })?;
            max_grad_norm = max_grad_norm.max(grad_norm);
            if grad_norm > GRAD_NORM_NOISE_FLOOR {
                return Err(PredictorError::FrozenTargetGrad {
                    instrument_id: handle.instrument_id().to_string(),
                    grad_norm,
                    threshold: GRAD_NORM_NOISE_FLOOR,
                    fix_at: "file:crates/context-graph-mejepa/src/frozen_target.rs".to_string(),
                });
            }
        }
    }
    Ok(GradientHookReport {
        instruments_checked: adapter.grad_handles().len(),
        tensors_checked,
        max_grad_norm,
        threshold: GRAD_NORM_NOISE_FLOOR,
        passes: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_models::TargetProvenance;

    #[derive(Debug)]
    struct SyntheticHandle {
        ids: Vec<TensorId>,
    }

    impl InstrumentGradHandle for SyntheticHandle {
        fn instrument_id(&self) -> &str {
            "synthetic_frozen"
        }

        fn tensor_ids(&self) -> &[TensorId] {
            &self.ids
        }
    }

    #[test]
    fn clean_empty_adapter_passes() {
        let adapter = FrozenTargetAdapter::empty_for_test();
        let report = run_gradient_hook(&adapter, &GradStore::default()).expect("clean pass");
        assert_eq!(report.instruments_checked, 0);
        assert!(report.passes);
    }

    #[test]
    fn nonzero_grad_is_critical() {
        let adapter = FrozenTargetAdapter::with_grad_handles(
            TargetProvenance::new("test", BTreeMap::new(), 0, None),
            vec![Box::new(SyntheticHandle {
                ids: vec![TensorId("target.weight".to_string())],
            })],
        );
        let mut grads = GradStore::default();
        grads
            .insert_norm(TensorId("target.weight".to_string()), 1.0)
            .expect("valid norm");
        let err = run_gradient_hook(&adapter, &grads).expect_err("grad must fail");
        assert_eq!(err.code(), "MEJEPA_PRED_FROZEN_TARGET_GRAD");
        assert!(err.is_critical());
    }

    /// #620 regression: a frozen-target adapter that registers a tensor_id
    /// MUST have a corresponding GradStore entry. Previously, `unwrap_or(0.0)`
    /// silently treated absence as a measured zero — passing the hook even
    /// when no measurement had occurred. The new behavior is fail-closed
    /// with `MEJEPA_PRED_FROZEN_TARGET_GRAD_UNMEASURED`.
    #[test]
    fn gradient_unmeasured_tensor_fails_closed() {
        let adapter = FrozenTargetAdapter::with_grad_handles(
            TargetProvenance::new("test", BTreeMap::new(), 0, None),
            vec![Box::new(SyntheticHandle {
                ids: vec![TensorId("target.weight".to_string())],
            })],
        );
        // GradStore deliberately empty — no measurement for the registered
        // tensor_id. Old behavior: treated as 0.0 and passed. New: errors.
        let grads = GradStore::default();
        let err = run_gradient_hook(&adapter, &grads).expect_err(
            "unmeasured tensor on a registered handle must fail closed (#620)",
        );
        assert_eq!(err.code(), "MEJEPA_PRED_FROZEN_TARGET_GRAD_UNMEASURED");
        assert!(err.is_critical());
    }
}
