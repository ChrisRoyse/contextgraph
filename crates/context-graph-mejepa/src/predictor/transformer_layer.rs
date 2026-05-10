use candle_core::{DType, Tensor};
use candle_nn::{layer_norm, linear, LayerNorm, Linear, Module, VarBuilder};

use crate::error::{NanSource, PredictorError};
use crate::predictor::ensure_finite;

#[derive(Debug, Clone)]
pub struct TransformerLayer {
    self_attn_qkv: Linear,
    self_attn_out: Linear,
    ffn_up: Linear,
    ffn_down: Linear,
    attn_ln: LayerNorm,
    ffn_ln: LayerNorm,
    hidden_dim: usize,
    num_heads: u8,
    head_dim: usize,
    ff_inner_dim: usize,
}

impl TransformerLayer {
    pub fn new(
        hidden_dim: usize,
        num_heads: u8,
        ff_expansion: u8,
        layer_norm_eps: f64,
        vb: VarBuilder,
    ) -> Result<Self, PredictorError> {
        if num_heads == 0 || !hidden_dim.is_multiple_of(num_heads as usize) {
            return Err(PredictorError::ConfigInvalid {
                detail: format!(
                    "hidden_dim {hidden_dim} must be divisible by num_heads {num_heads}"
                ),
            });
        }
        let ff_inner_dim = hidden_dim
            .checked_mul(ff_expansion as usize)
            .ok_or_else(|| PredictorError::ConfigInvalid {
                detail: "ff_inner_dim overflow".to_string(),
            })?;
        Ok(Self {
            self_attn_qkv: linear(hidden_dim, hidden_dim * 3, vb.pp("self_attn_qkv"))?,
            self_attn_out: linear(hidden_dim, hidden_dim, vb.pp("self_attn_out"))?,
            ffn_up: linear(hidden_dim, ff_inner_dim, vb.pp("ffn_up"))?,
            ffn_down: linear(ff_inner_dim, hidden_dim, vb.pp("ffn_down"))?,
            attn_ln: layer_norm(hidden_dim, layer_norm_eps, vb.pp("attn_ln"))?,
            ffn_ln: layer_norm(hidden_dim, layer_norm_eps, vb.pp("ffn_ln"))?,
            hidden_dim,
            num_heads,
            head_dim: hidden_dim / num_heads as usize,
            ff_inner_dim,
        })
    }

    pub fn forward(
        &self,
        x: &Tensor,
        validate_finite_in_dryrun: bool,
    ) -> Result<Tensor, PredictorError> {
        self.forward_with_layer_idx(x, 0, validate_finite_in_dryrun)
    }

    pub(crate) fn forward_with_layer_idx(
        &self,
        x: &Tensor,
        layer_idx: u8,
        validate_finite_in_dryrun: bool,
    ) -> Result<Tensor, PredictorError> {
        if x.dims().len() != 2 || x.dims()[1] != self.hidden_dim {
            return Err(PredictorError::DimMismatch {
                detail: format!(
                    "transformer layer expects (B, {}); got {:?}",
                    self.hidden_dim,
                    x.dims()
                ),
                observed: serde_json::json!({ "transformer_input": x.dims() }),
                expected_panel_dim: self.hidden_dim,
            });
        }
        if validate_finite_in_dryrun {
            ensure_finite(x, NanSource::Layer, Some(layer_idx), "transformer_input")?;
        }

        let attn_in = self.attn_ln.forward(x)?;
        let qkv = self.self_attn_qkv.forward(&attn_in)?;
        let q = qkv.narrow(1, 0, self.hidden_dim)?;
        let k = qkv.narrow(1, self.hidden_dim, self.hidden_dim)?;
        let v = qkv.narrow(1, self.hidden_dim * 2, self.hidden_dim)?;
        let gate =
            (q.broadcast_mul(&k)?.mean_keepdim(1)? / (self.head_dim as f64).sqrt())?.tanh()?;
        let gated_v = v.broadcast_mul(&(gate + 1.0)?)?;
        let attn_out = self.self_attn_out.forward(&gated_v)?;
        let x = (x + attn_out)?;

        let ffn_in = self.ffn_ln.forward(&x)?;
        let ffn_hidden = self.ffn_up.forward(&ffn_in)?.gelu()?;
        let ffn_out = self.ffn_down.forward(&ffn_hidden)?;
        let y = (x + ffn_out)?.to_dtype(DType::BF16)?;
        if validate_finite_in_dryrun {
            ensure_finite(&y, NanSource::Layer, Some(layer_idx), "transformer_output")?;
        }
        Ok(y)
    }

    pub fn architecture(&self) -> (u8, u32, u32) {
        (
            self.num_heads,
            self.head_dim as u32,
            self.ff_inner_dim as u32,
        )
    }
}
