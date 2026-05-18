use std::collections::BTreeMap;

use candle_core::Tensor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PredictedPanel {
    pub tensor: Tensor,
    pub batch_size: usize,
    pub panel_dim: usize,
    pub dtype: String,
}

#[derive(Debug, Clone)]
pub struct PredictedInverseMap {
    pub predicted_input_panel: PredictedPanel,
    pub predicted_action: Tensor,
    pub action_dim: usize,
    pub dtype: String,
}

#[derive(Debug, Clone)]
pub struct OracleLogits {
    pub tensor: Tensor,
    pub batch_size: usize,
    pub logits_dim: usize,
}

#[derive(Debug, Clone)]
pub struct TargetPanel {
    pub tensor: Tensor,
    pub batch_size: usize,
    pub panel_dim: usize,
    pub dtype: String,
    pub provenance: TargetProvenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetProvenance {
    pub source: String,
    pub instrument_versions: BTreeMap<String, String>,
    pub frozen_at_unix_ms: i64,
    pub panel_hash: Option<String>,
}

impl TargetProvenance {
    pub fn new(
        source: impl Into<String>,
        instrument_versions: BTreeMap<String, String>,
        frozen_at_unix_ms: i64,
        panel_hash: Option<String>,
    ) -> Self {
        Self {
            source: source.into(),
            instrument_versions,
            frozen_at_unix_ms,
            panel_hash,
        }
    }
}
